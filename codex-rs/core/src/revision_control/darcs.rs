use std::path::{Path, PathBuf};

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
