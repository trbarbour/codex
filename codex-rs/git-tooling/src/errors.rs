use std::path::PathBuf;
use std::process::ExitStatus;
use std::string::FromUtf8Error;

use codex_core::revision_control::RevisionControlKind;
use thiserror::Error;
use walkdir::Error as WalkdirError;

/// Errors returned while managing git worktree snapshots.
#[derive(Debug, Error)]
pub enum GitToolingError {
    #[error("git command `{command}` failed with status {status}: {stderr}")]
    GitCommand {
        command: String,
        status: ExitStatus,
        stderr: String,
    },
    #[error("git command `{command}` produced non-UTF-8 output")]
    GitOutputUtf8 {
        command: String,
        #[source]
        source: FromUtf8Error,
    },
    #[error("{path:?} is not a git repository")]
    NotAGitRepository { path: PathBuf },
    #[error("path {path:?} must be relative to the repository root")]
    NonRelativePath { path: PathBuf },
    #[error("path {path:?} escapes the repository root")]
    PathEscapesRepository { path: PathBuf },
    #[error("failed to process path inside worktree")]
    PathPrefix(#[from] std::path::StripPrefixError),
    #[error(transparent)]
    Walkdir(#[from] WalkdirError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Errors encountered while snapshotting Darcs workspaces.
#[derive(Debug, Error)]
pub enum DarcsSnapshotError {
    #[error("darcs CLI is not installed")]
    CliMissing,
    #[error("darcs command `{command}` failed with status {status}: {stderr}")]
    CommandFailed {
        command: String,
        status: ExitStatus,
        stderr: String,
    },
    #[error("darcs command `{command}` produced non-UTF-8 output")]
    OutputUtf8 {
        command: String,
        #[source]
        source: FromUtf8Error,
    },
    #[error("{path:?} is outside the Darcs repository rooted at {root:?}")]
    PathOutsideRepository { path: PathBuf, root: PathBuf },
    #[error("failed to create snapshot storage at {path:?}")]
    StoragePath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Walkdir(#[from] WalkdirError),
    #[error(transparent)]
    StripPrefix(#[from] std::path::StripPrefixError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[cfg(unix)]
    #[error("failed to create symlink {link:?} -> {target:?}")]
    Symlink {
        target: PathBuf,
        link: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[cfg(windows)]
    #[error("failed to create symlink {link:?} -> {target:?}")]
    Symlink {
        target: PathBuf,
        link: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Backend-neutral error surface for snapshot operations.
#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("{path:?} is not managed by a supported revision control system")]
    NotARepository { path: PathBuf },
    #[error("{kind:?} repositories are not supported for snapshot operations")]
    UnsupportedRevisionControl { kind: RevisionControlKind },
    #[error("missing required tooling: {tool}")]
    MissingTool { tool: &'static str },
    #[error("Git snapshot error: {0}")]
    Git(GitToolingError),
    #[error("Darcs snapshot error: {0}")]
    Darcs(DarcsSnapshotError),
    #[error("snapshot of a {actual:?} repository cannot be restored in a {expected:?} repository")]
    MismatchedSnapshot {
        expected: RevisionControlKind,
        actual: RevisionControlKind,
    },
}

impl From<DarcsSnapshotError> for SnapshotError {
    fn from(value: DarcsSnapshotError) -> Self {
        match value {
            DarcsSnapshotError::CliMissing => SnapshotError::MissingTool { tool: "darcs" },
            DarcsSnapshotError::PathOutsideRepository { path, .. } => {
                SnapshotError::NotARepository { path }
            }
            other => SnapshotError::Darcs(other),
        }
    }
}

impl From<GitToolingError> for SnapshotError {
    fn from(value: GitToolingError) -> Self {
        match value {
            GitToolingError::NotAGitRepository { path } => SnapshotError::NotARepository { path },
            other => SnapshotError::Git(other),
        }
    }
}
