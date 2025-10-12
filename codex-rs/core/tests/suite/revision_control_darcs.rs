use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use codex_core::revision_control::RevisionControlKind;
use codex_core::revision_control::collect_revision_control_summary;
use codex_core::revision_control::darcs;
use codex_core::revision_control::detect_revision_control;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn darcs_cli_installed() -> bool {
    darcs::darcs_cli_available()
}

fn run_darcs<I, S>(repo: &Path, args: I) -> std::process::Output
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let args_vec: Vec<OsString> = args
        .into_iter()
        .map(|arg| arg.as_ref().to_os_string())
        .collect();

    let output = Command::new("darcs")
        .args(&args_vec)
        .current_dir(repo)
        .output()
        .expect("failed to spawn darcs");

    if !output.status.success() {
        panic!(
            "darcs {} failed: {}",
            args_vec
                .iter()
                .map(|arg| arg.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    output
}

fn init_darcs_repo() -> TempDir {
    let dir = TempDir::new().expect("create temp dir");
    run_darcs(dir.path(), ["init"]);
    dir
}

fn record_all(repo: &Path, message: &str) {
    run_darcs(
        repo,
        [
            "record",
            "-a",
            "--look-for-adds",
            "--author=Codex Tests <codex@example.com>",
            "-m",
            message,
        ],
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn collect_darcs_info_reports_repo_metadata() -> TestResult {
    if !darcs_cli_installed() {
        eprintln!("skipping Darcs integration tests because darcs is not available");
        return Ok(());
    }

    let repo = init_darcs_repo();
    let repo_path = repo.path();

    std::fs::write(repo_path.join("tracked.txt"), "initial contents\n")?;
    run_darcs(repo_path, ["add", "tracked.txt"]);
    record_all(repo_path, "Initial snapshot");

    let changes_output = run_darcs(repo_path, ["changes", "--last=1"]);
    let changes_text = String::from_utf8_lossy(&changes_output.stdout);
    let expected_hash = changes_text
        .lines()
        .find_map(|line| line.strip_prefix("patch "))
        .map(str::trim)
        .expect("Darcs should report the latest patch hash")
        .to_string();

    std::fs::create_dir_all(repo_path.join("_darcs/prefs"))?;
    std::fs::write(
        repo_path.join("_darcs/prefs/defaultrepo"),
        "https://example.com/upstream\n",
    )?;

    let info = darcs::collect_darcs_info(repo_path)
        .await
        .expect("Darcs metadata should be collected");

    assert_eq!(info.patch_hash.as_deref(), Some(expected_hash.as_str()));
    assert_eq!(
        info.default_remote.as_deref(),
        Some("https://example.com/upstream")
    );
    assert_eq!(info.branch, None);

    let backend = detect_revision_control(repo_path).expect("Darcs repo should be detected");
    assert_eq!(backend.kind, RevisionControlKind::Darcs);

    let summary = collect_revision_control_summary(&backend, repo_path)
        .await
        .expect("Darcs summary should be returned");

    assert_eq!(
        summary.kind,
        codex_protocol::protocol::RevisionControlBackend::Darcs
    );
    assert_eq!(summary.darcs, Some(info));
    assert_eq!(summary.git, None);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_diff_reports_pending_changes() -> TestResult {
    if !darcs_cli_installed() {
        eprintln!("skipping Darcs integration tests because darcs is not available");
        return Ok(());
    }

    let repo = init_darcs_repo();
    let repo_path = repo.path();

    std::fs::write(repo_path.join("tracked.txt"), "line one\n")?;
    run_darcs(repo_path, ["add", "tracked.txt"]);
    record_all(repo_path, "Initial state");

    std::fs::write(repo_path.join("tracked.txt"), "line one\nline two\n")?;
    std::fs::write(repo_path.join("untracked.txt"), "new file\n")?;

    let diff = darcs::workspace_diff(repo_path).await?;

    assert!(diff.contains("hunk ./tracked.txt"), "diff: {diff}");
    assert!(diff.contains("+line two"), "diff: {diff}");
    assert!(diff.contains("addfile ./untracked.txt"), "diff: {diff}");

    // Restore repository to clean state and confirm the diff is empty.
    run_darcs(repo_path, ["add", "untracked.txt"]);
    record_all(repo_path, "Capture untracked file");

    let clean_diff = darcs::workspace_diff(repo_path).await?;
    assert_eq!(clean_diff.trim(), "");

    Ok(())
}
