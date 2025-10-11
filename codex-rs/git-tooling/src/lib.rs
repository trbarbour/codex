use std::fmt;
use std::path::Path;
use std::path::PathBuf;

use codex_core::revision_control::RevisionControlKind;
use codex_core::revision_control::RevisionControlSystem;

mod darcs_snapshots;
mod errors;
mod ghost_commits;
mod operations;
mod platform;

use darcs_snapshots::DarcsSnapshot;
pub use errors::GitToolingError;
pub use errors::SnapshotError;
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

/// Snapshot captured from a repository's working tree.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Snapshot {
    Git(GhostCommit),
    Darcs(DarcsSnapshot),
}

impl Snapshot {
    /// Backend kind associated with this snapshot.
    pub fn kind(&self) -> RevisionControlKind {
        match self {
            Snapshot::Git(_) => RevisionControlKind::Git,
            Snapshot::Darcs(_) => RevisionControlKind::Darcs,
        }
    }

    /// Stable identifier for the snapshot.
    pub fn id(&self) -> &str {
        match self {
            Snapshot::Git(commit) => commit.id(),
            Snapshot::Darcs(snapshot) => snapshot.id(),
        }
    }

    /// Abbreviated identifier suitable for display.
    pub fn short_id(&self) -> String {
        self.id().chars().take(8).collect()
    }
}

/// Backend-aware snapshot manager that dispatches to Git and Darcs implementations.
pub struct RepoSnapshotManager<'a> {
    backend: &'a dyn RevisionControlSystem,
    storage_root: PathBuf,
}

impl<'a> RepoSnapshotManager<'a> {
    /// Create a new snapshot manager for the provided revision control backend.
    pub fn new(backend: &'a dyn RevisionControlSystem) -> Self {
        Self {
            backend,
            storage_root: std::env::temp_dir(),
        }
    }

    /// Override the storage root used for backend-specific snapshot assets.
    pub fn with_storage_root(mut self, storage_root: impl Into<PathBuf>) -> Self {
        self.storage_root = storage_root.into();
        self
    }

    /// Create a snapshot of the repository's working tree.
    pub fn create_snapshot(
        &self,
        options: &CreateGhostCommitOptions<'_>,
    ) -> Result<Snapshot, SnapshotError> {
        match self.backend.kind() {
            RevisionControlKind::Git => ghost_commits::create_ghost_commit(options)
                .map(Snapshot::Git)
                .map_err(SnapshotError::from),
            RevisionControlKind::Darcs => darcs_snapshots::create_snapshot(
                self.backend.root(),
                options.repo_path,
                &self.storage_root,
            )
            .map(Snapshot::Darcs),
        }
    }

    /// Restore the working tree to the provided snapshot.
    pub fn restore_snapshot(
        &self,
        repo_path: &Path,
        snapshot: &Snapshot,
    ) -> Result<(), SnapshotError> {
        match (self.backend.kind(), snapshot) {
            (RevisionControlKind::Git, Snapshot::Git(commit)) => {
                ghost_commits::restore_ghost_commit(repo_path, commit).map_err(SnapshotError::from)
            }
            (RevisionControlKind::Darcs, Snapshot::Darcs(darcs_snapshot)) => {
                darcs_snapshots::restore_snapshot(self.backend.root(), repo_path, darcs_snapshot)
            }
            (expected, other) => Err(SnapshotError::MismatchedSnapshot {
                expected,
                actual: other.kind(),
            }),
        }
    }

    /// Restore the working tree to the provided commit id.
    pub fn restore_to_commit(
        &self,
        repo_path: &Path,
        commit_id: &str,
    ) -> Result<(), SnapshotError> {
        match self.backend.kind() {
            RevisionControlKind::Git => {
                ghost_commits::restore_to_commit(repo_path, commit_id).map_err(SnapshotError::from)
            }
            other => Err(SnapshotError::UnsupportedRevisionControl { kind: other }),
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
            tooling_error: None,
        }
    }

    #[test]
    fn manager_reports_missing_tool_for_darcs_without_cli() {
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
            SnapshotError::MissingTool { tool } => {
                assert_eq!(tool, "darcs");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn manager_creates_and_restores_snapshots() -> Result<(), SnapshotError> {
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
        assert!(matches!(snapshot, Snapshot::Git(_)));

        std::fs::write(repo.join("test.txt"), "overwritten").unwrap();
        manager.restore_snapshot(repo, &snapshot)?;

        let restored = std::fs::read_to_string(repo.join("test.txt")).unwrap();
        assert_eq!(restored, "modified");
        Ok(())
    }
}
