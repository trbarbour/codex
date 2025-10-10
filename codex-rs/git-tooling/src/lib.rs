use std::fmt;
use std::path::Path;

use codex_core::revision_control::RevisionControlKind;
use codex_core::revision_control::RevisionControlSystem;

mod errors;
mod ghost_commits;
mod operations;
mod platform;

pub use errors::GitToolingError;
pub use ghost_commits::CreateGhostCommitOptions;
pub use platform::create_symlink;

/// Details of a ghost commit created from a repository state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhostCommit {
    id: String,
    parent: Option<String>,
}

impl GhostCommit {
    /// Create a new ghost commit wrapper from a raw commit ID and optional parent.
    pub fn new(id: String, parent: Option<String>) -> Self {
        Self { id, parent }
    }

    /// Commit ID for the snapshot.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Parent commit ID, if the repository had a `HEAD` at creation time.
    pub fn parent(&self) -> Option<&str> {
        self.parent.as_deref()
    }
}

impl fmt::Display for GhostCommit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.id)
    }
}

/// Backend-aware snapshot manager that dispatches to Git implementations today.
pub struct RepoSnapshotManager<'a> {
    backend: &'a dyn RevisionControlSystem,
}

impl<'a> RepoSnapshotManager<'a> {
    /// Create a new snapshot manager for the provided revision control backend.
    pub fn new(backend: &'a dyn RevisionControlSystem) -> Self {
        Self { backend }
    }

    /// Create a snapshot of the repository's working tree.
    pub fn create_snapshot(
        &self,
        options: &CreateGhostCommitOptions<'_>,
    ) -> Result<GhostCommit, GitToolingError> {
        self.with_git(|| ghost_commits::create_ghost_commit(options))
    }

    /// Restore the working tree to the provided snapshot.
    pub fn restore_snapshot(
        &self,
        repo_path: &Path,
        commit: &GhostCommit,
    ) -> Result<(), GitToolingError> {
        self.with_git(|| ghost_commits::restore_ghost_commit(repo_path, commit))
    }

    /// Restore the working tree to the provided commit id.
    pub fn restore_to_commit(
        &self,
        repo_path: &Path,
        commit_id: &str,
    ) -> Result<(), GitToolingError> {
        self.with_git(|| ghost_commits::restore_to_commit(repo_path, commit_id))
    }

    fn with_git<T>(
        &self,
        op: impl FnOnce() -> Result<T, GitToolingError>,
    ) -> Result<T, GitToolingError> {
        match self.backend.kind() {
            RevisionControlKind::Git => op(),
            other => Err(GitToolingError::UnsupportedRevisionControl { kind: other }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::revision_control::DetectedRevisionControl;
    use codex_core::revision_control::RevisionControlCapabilities;
    use codex_core::revision_control::RevisionControlKind;
    use std::path::Path;
    use std::process::Command;
    use tempfile::tempdir;

    fn git_backend(root: &Path) -> DetectedRevisionControl {
        DetectedRevisionControl {
            kind: RevisionControlKind::Git,
            root: root.to_path_buf(),
            capabilities: RevisionControlCapabilities::new(true, true),
        }
    }

    #[test]
    fn manager_rejects_unsupported_backends() {
        struct Dummy;

        impl RevisionControlSystem for Dummy {
            fn kind(&self) -> RevisionControlKind {
                RevisionControlKind::Darcs
            }

            fn root(&self) -> &Path {
                Path::new("/tmp")
            }

            fn capabilities(&self) -> RevisionControlCapabilities {
                RevisionControlCapabilities::new(false, false)
            }
        }

        let backend = Dummy;
        let manager = RepoSnapshotManager::new(&backend);
        let options = CreateGhostCommitOptions::new(Path::new("/tmp"));
        let err = manager
            .create_snapshot(&options)
            .expect_err("expected unsupported backend error");

        match err {
            GitToolingError::UnsupportedRevisionControl { kind } => {
                assert_eq!(kind, RevisionControlKind::Darcs);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn manager_creates_and_restores_snapshots() -> Result<(), GitToolingError> {
        let temp_dir = tempdir().unwrap();
        let repo = temp_dir.path();

        Command::new("git")
            .args(["init", "--initial-branch", "main"])
            .current_dir(repo)
            .status()
            .expect("git init must succeed");

        std::fs::write(repo.join("test.txt"), "initial").unwrap();
        Command::new("git")
            .args(["add", "test.txt"])
            .current_dir(repo)
            .status()
            .expect("git add must succeed");
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(repo)
            .status()
            .expect("git commit must succeed");

        std::fs::write(repo.join("test.txt"), "modified").unwrap();

        let backend = git_backend(repo);
        let manager = RepoSnapshotManager::new(&backend);
        let snapshot = manager.create_snapshot(&CreateGhostCommitOptions::new(repo))?;

        std::fs::write(repo.join("test.txt"), "overwritten").unwrap();
        manager.restore_snapshot(repo, &snapshot)?;

        let restored = std::fs::read_to_string(repo.join("test.txt")).unwrap();
        assert_eq!(restored, "modified");
        Ok(())
    }
}
