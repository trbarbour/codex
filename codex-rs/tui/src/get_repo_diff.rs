//! Utility to compute the current diff for the active revision-control backend.
//!
//! The helper detects whether the working directory is managed by Git or
//! Darcs and shells out to the corresponding CLI to collect the diff. When no
//! supported backend is detected the function returns `Ok((None, String::new()))`.

use std::env;
use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::Stdio;

use codex_core::revision_control::RevisionControlKind;
use codex_core::revision_control::darcs;
use codex_core::revision_control::detect_revision_control;
use tokio::process::Command;
use tokio::task::JoinSet;

/// Return value of [`get_repo_diff`].
///
/// * `Option<RevisionControlKind>` – Detected backend (if any).
/// * `String` – The concatenated diff (may be empty).
pub(crate) async fn get_repo_diff() -> io::Result<(Option<RevisionControlKind>, String)> {
    let cwd = env::current_dir()?;
    let detected = detect_revision_control(&cwd);

    let Some(detected) = detected else {
        return Ok((None, String::new()));
    };

    let diff = match detected.kind {
        RevisionControlKind::Git => get_git_diff(&cwd).await?,
        RevisionControlKind::Darcs => darcs::workspace_diff(&cwd).await?,
    };

    Ok((Some(detected.kind), diff))
}

async fn get_git_diff(cwd: &Path) -> io::Result<String> {
    if !inside_git_repo(cwd).await? {
        return Ok(String::new());
    }

    // Run tracked diff and untracked file listing in parallel.
    let (tracked_diff_res, untracked_output_res) = tokio::join!(
        run_git_capture_diff(cwd, ["diff", "--color"]),
        run_git_capture_stdout(cwd, ["ls-files", "--others", "--exclude-standard"]),
    );
    let tracked_diff = tracked_diff_res?;
    let untracked_output = untracked_output_res?;

    let mut untracked_diff = String::new();
    let null_device: &Path = if cfg!(windows) {
        Path::new("NUL")
    } else {
        Path::new("/dev/null")
    };

    let null_path = null_device.to_str().unwrap_or("/dev/null").to_string();
    let mut join_set: JoinSet<io::Result<String>> = JoinSet::new();
    for file in untracked_output
        .split('\n')
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let cwd = cwd.to_path_buf();
        let null_path = null_path.clone();
        let file = file.to_string();
        join_set.spawn(async move {
            run_git_capture_diff(
                &cwd,
                vec![
                    "diff".into(),
                    "--color".into(),
                    "--no-index".into(),
                    "--".into(),
                    null_path,
                    file,
                ],
            )
            .await
        });
    }
    while let Some(res) = join_set.join_next().await {
        match res {
            Ok(Ok(diff)) => untracked_diff.push_str(&diff),
            Ok(Err(err)) if err.kind() == io::ErrorKind::NotFound => {}
            Ok(Err(err)) => return Err(err),
            Err(_) => {}
        }
    }

    Ok(format!("{tracked_diff}{untracked_diff}"))
}

/// Helper that executes `git` with the given `args` and returns `stdout` as a
/// UTF-8 string. Any non-zero exit status is considered an *error*.
async fn run_git_capture_stdout<I, S>(cwd: &Path, args: I) -> io::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("git")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .current_dir(cwd)
        .output()
        .await?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(io::Error::other(format!(
            "git command failed with status {}",
            output.status
        )))
    }
}

/// Like [`run_git_capture_stdout`] but treats exit status 1 as success and
/// returns stdout. Git returns 1 for diffs when differences are present.
async fn run_git_capture_diff<I, S>(cwd: &Path, args: I) -> io::Result<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("git")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .current_dir(cwd)
        .output()
        .await?;

    if output.status.success() || output.status.code() == Some(1) {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(io::Error::other(format!(
            "git command failed with status {}",
            output.status
        )))
    }
}

/// Determine if the specified directory is inside a Git repository.
async fn inside_git_repo(cwd: &Path) -> io::Result<bool> {
    let status = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .current_dir(cwd)
        .status()
        .await;

    match status {
        Ok(s) if s.success() => Ok(true),
        Ok(_) => Ok(false),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false), // git not installed
        Err(e) => Err(e),
    }
}
