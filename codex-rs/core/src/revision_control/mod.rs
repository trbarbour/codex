use std::path::Path;
use std::path::PathBuf;

use crate::git_info;

pub mod darcs;
pub mod git;

/// Enumeration of revision control backends supported by Codex.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RevisionControlKind {
    Git,
    Darcs,
}

impl RevisionControlKind {
    /// Human readable display name for the backend.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Git => "Git",
            Self::Darcs => "Darcs",
        }
    }
}

/// Capabilities supported by the detected revision control backend.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RevisionControlCapabilities {
    pub supports_diffs: bool,
    pub supports_snapshots: bool,
}

impl RevisionControlCapabilities {
    pub const fn new(supports_diffs: bool, supports_snapshots: bool) -> Self {
        Self {
            supports_diffs,
            supports_snapshots,
        }
    }

    const fn for_kind(kind: RevisionControlKind) -> Self {
        match kind {
            RevisionControlKind::Git => Self::new(true, true),
            // Darcs support is a work in progress. Report capabilities as disabled
            // until the dedicated implementations land.
            RevisionControlKind::Darcs => Self::new(false, false),
        }
    }
}

pub trait RevisionControlSystem {
    fn kind(&self) -> RevisionControlKind;
    fn root(&self) -> &Path;
    fn capabilities(&self) -> RevisionControlCapabilities;

    fn display_name(&self) -> &'static str {
        self.kind().display_name()
    }
}

/// Information about the detected revision control system for a workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectedRevisionControl {
    pub kind: RevisionControlKind,
    pub root: PathBuf,
    pub capabilities: RevisionControlCapabilities,
}

impl DetectedRevisionControl {
    pub fn new(kind: RevisionControlKind, root: PathBuf) -> Self {
        let capabilities = RevisionControlCapabilities::for_kind(kind);
        Self {
            kind,
            root,
            capabilities,
        }
    }
}

impl RevisionControlSystem for DetectedRevisionControl {
    fn kind(&self) -> RevisionControlKind {
        self.kind
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn capabilities(&self) -> RevisionControlCapabilities {
        self.capabilities
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

pub fn resolve_revision_control_project_for_trust(
    base_dir: &Path,
    detected: Option<&DetectedRevisionControl>,
) -> Option<PathBuf> {
    let detected = match detected {
        Some(info) => info.clone(),
        None => detect_revision_control(base_dir)?,
    };

    match detected.kind {
        RevisionControlKind::Git => git_info::resolve_root_git_project_for_trust(base_dir),
        RevisionControlKind::Darcs => Some(detected.root),
    }
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
        assert_eq!(
            detected.capabilities,
            RevisionControlCapabilities::new(true, true)
        );
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
        assert_eq!(
            detected.capabilities,
            RevisionControlCapabilities::new(false, false)
        );
    }

    #[test]
    fn resolve_trust_root_for_darcs_repo() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("_darcs")).unwrap();

        let detected = detect_revision_control(dir.path()).unwrap();
        let resolved = resolve_revision_control_project_for_trust(dir.path(), Some(&detected));

        assert_eq!(resolved, Some(dir.path().to_path_buf()));
    }
}
