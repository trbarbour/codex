use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

use tracing::warn;

const DARCS_MISSING_MESSAGE: &str = "Darcs repository detected but the `darcs` CLI is not installed. Install it to enable Codex's Darcs integration.";

static DARCS_WARNING_EMITTED: OnceLock<()> = OnceLock::new();

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
        eprintln!("{DARCS_MISSING_MESSAGE}");
    }

    Some(DARCS_MISSING_MESSAGE.to_string())
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
