use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::OnceLock;

use codex_protocol::protocol::DarcsInfo;
use tokio::process::Command;
use tokio::time::Duration as TokioDuration;
use tokio::time::timeout;
use tracing::warn;

const DARCS_MISSING_MESSAGE: &str = "Darcs repository detected but the `darcs` CLI is not installed. Install it to enable Codex's Darcs integration.";

static DARCS_WARNING_EMITTED: OnceLock<()> = OnceLock::new();

const DARCS_COMMAND_TIMEOUT: TokioDuration = TokioDuration::from_secs(5);

/// Return the Darcs repository root if the provided directory is inside a Darcs
/// checkout.
///
/// The check mirrors the Git implementation by walking up the directory tree
/// looking for a `_darcs` directory. Darcs stores repository metadata inside
/// that folder, so its presence is sufficient to identify the workspace as a
/// Darcs checkout without invoking the `darcs` binary.
pub fn get_darcs_repo_root(base_dir: &Path) -> Option<PathBuf> {
    let mut dir = base_dir.to_path_buf();

    loop {
        if dir.join("_darcs").exists() {
            return Some(dir);
        }

        if !dir.pop() {
            break;
        }
    }

    None
}

/// Returns `true` when the `darcs` executable is available on `PATH`.
pub fn darcs_cli_available() -> bool {
    which::which("darcs").is_ok()
}

/// Emit a warning (only once per process) when a Darcs repository is detected but the
/// CLI is missing. The message is also returned so callers can surface it in the UI.
pub fn warn_missing_darcs_cli() -> Option<String> {
    if darcs_cli_available() {
        return None;
    }

    if DARCS_WARNING_EMITTED.set(()).is_ok() {
        warn!("{DARCS_MISSING_MESSAGE}");
    }

    Some(DARCS_MISSING_MESSAGE.to_string())
}

pub async fn collect_darcs_info(cwd: &Path) -> Option<DarcsInfo> {
    let repo_root = get_darcs_repo_root(cwd)?;
    if !darcs_cli_available() {
        return None;
    }

    let output = run_darcs_capture(&repo_root, ["show", "repo"]).await.ok()?;
    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let default_remote = extract_key_value(&text, "Default Remote")
        .or_else(|| extract_key_value(&text, "Default remote"));
    let branch = extract_key_value(&text, "Current branch")
        .or_else(|| extract_key_value(&text, "Current Branch"))
        .or_else(|| extract_key_value(&text, "Default branch"))
        .or_else(|| extract_key_value(&text, "Default Branch"));
    let patch_hash = latest_patch_hash(&repo_root).await;

    Some(DarcsInfo {
        patch_hash,
        branch,
        default_remote,
    })
}

pub async fn workspace_diff(cwd: &Path) -> io::Result<String> {
    if get_darcs_repo_root(cwd).is_none() {
        return Ok(String::new());
    }

    let output = timeout(
        DARCS_COMMAND_TIMEOUT,
        Command::new("darcs")
            .args(["whatsnew", "--unified", "--color=always", "--look-for-adds"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .current_dir(cwd)
            .output(),
    )
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "darcs whatsnew timed out"))??;

    if output.status.success() || output.status.code() == Some(1) {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(io::Error::other(format!(
            "darcs whatsnew failed with status {}",
            output.status
        )))
    }
}

async fn latest_patch_hash(cwd: &Path) -> Option<String> {
    if let Ok(output) = run_darcs_capture(cwd, ["changes", "--last=1", "--xml"]).await {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(hash) = find_attr_value(&text, "hash") {
                return Some(hash);
            }
        }
    }

    let output = run_darcs_capture(cwd, ["changes", "--last=1"]).await.ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    extract_key_value(&text, "Patch hash")
}

async fn run_darcs_capture<I, S>(cwd: &Path, args: I) -> io::Result<std::process::Output>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = timeout(
        DARCS_COMMAND_TIMEOUT,
        Command::new("darcs")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(cwd)
            .output(),
    )
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "darcs command timed out"))??;

    Ok(output)
}

fn extract_key_value(text: &str, key: &str) -> Option<String> {
    let key_lower = key.to_ascii_lowercase();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_ascii_lowercase();
        if lower.starts_with(&key_lower)
            && let Some(idx) = trimmed.find(':')
        {
            let value = trimmed[idx + 1..].trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

fn find_attr_value(text: &str, attr: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let pattern = format!("{attr}={quote}");
        if let Some(idx) = text.find(&pattern) {
            let rest = &text[idx + pattern.len()..];
            if let Some(end) = rest.find(quote) {
                let value = rest[..end].trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detects_nested_darcs_repository() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("_darcs")).unwrap();

        let subdir = dir.path().join("nested");
        std::fs::create_dir(&subdir).unwrap();

        assert_eq!(get_darcs_repo_root(&subdir), Some(dir.path().to_path_buf()));
    }

    #[test]
    fn returns_none_for_non_repo() {
        let dir = tempdir().unwrap();
        assert!(get_darcs_repo_root(dir.path()).is_none());
    }
}
