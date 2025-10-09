use std::path::{Path, PathBuf};

pub mod darcs;
pub mod git;

/// Enumeration of revision control backends supported by Codex.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionControlKind {
    Git,
    Darcs,
}

/// Information about the detected revision control system for a workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectedRevisionControl {
    pub kind: RevisionControlKind,
    pub root: PathBuf,
}

impl DetectedRevisionControl {
    pub fn new(kind: RevisionControlKind, root: PathBuf) -> Self {
        Self { kind, root }
    }
}

/// Attempt to detect the revision control backend rooted at `base_dir`.
pub fn detect_revision_control(base_dir: &Path) -> Option<DetectedRevisionControl> {
    if let Some(root) = git::get_git_repo_root(base_dir) {
        return Some(DetectedRevisionControl::new(RevisionControlKind::Git, root));
    }

    darcs::get_darcs_repo_root(base_dir)
        .map(|root| DetectedRevisionControl::new(RevisionControlKind::Darcs, root))
}

pub use git::get_git_repo_root;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn detects_git_repository() {
        let dir = tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        fs::create_dir(&git_dir).unwrap();

        let detected = detect_revision_control(dir.path());

        assert!(detected.is_some());
        let detected = detected.unwrap();
        assert_eq!(detected.kind, RevisionControlKind::Git);
        assert_eq!(detected.root, dir.path());
    }

    #[test]
    fn returns_none_when_no_repo_found() {
        let dir = tempdir().unwrap();

        assert!(detect_revision_control(dir.path()).is_none());
    }

    #[test]
    fn detects_darcs_repository() {
        let dir = tempdir().unwrap();
        let darcs_dir = dir.path().join("_darcs");
        fs::create_dir(&darcs_dir).unwrap();

        let detected = detect_revision_control(dir.path());

        assert!(detected.is_some());
        let detected = detected.unwrap();
        assert_eq!(detected.kind, RevisionControlKind::Darcs);
        assert_eq!(detected.root, dir.path());
    }
}
