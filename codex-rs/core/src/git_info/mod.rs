use std::path::Path;
use std::path::PathBuf;

use codex_protocol::protocol::GitInfo;

use crate::revision_control::RevisionControlKind;
use crate::revision_control::RevisionControlSystem;

mod git;

pub use git::CommitLogEntry;
pub use git::GitDiffToRemote;

pub use crate::revision_control::git::get_git_repo_root;

/// Collect repository metadata for the provided revision control backend.
pub async fn collect_git_info(
    revision_control: &dyn RevisionControlSystem,
    cwd: &Path,
) -> Option<GitInfo> {
    if revision_control.kind() != RevisionControlKind::Git {
        return None;
    }

    git::collect_git_info(cwd).await
}

pub async fn recent_commits(cwd: &Path, limit: usize) -> Vec<CommitLogEntry> {
    git::recent_commits(cwd, limit).await
}

pub async fn git_diff_to_remote(cwd: &Path) -> Option<GitDiffToRemote> {
    git::git_diff_to_remote(cwd).await
}

pub async fn local_git_branches(cwd: &Path) -> Vec<String> {
    git::local_git_branches(cwd).await
}

pub async fn current_branch_name(cwd: &Path) -> Option<String> {
    git::current_branch_name(cwd).await
}

pub fn resolve_root_git_project_for_trust(cwd: &Path) -> Option<PathBuf> {
    git::resolve_root_git_project_for_trust(cwd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::revision_control::DetectedRevisionControl;
    use crate::revision_control::RevisionControlKind;
    use codex_protocol::protocol::GitInfo;
    use pretty_assertions::assert_eq;
    use serde_json::Value;
    use std::path::Path;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn git_backend(root: PathBuf) -> DetectedRevisionControl {
        DetectedRevisionControl::new(RevisionControlKind::Git, root)
    }

    #[tokio::test]
    async fn collect_git_info_non_git_directory() {
        let temp_dir = tempdir().unwrap();
        let backend = git_backend(temp_dir.path().to_path_buf());

        let result = collect_git_info(&backend, temp_dir.path()).await;

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn collect_git_info_git_repository() {
        let temp_dir = tempdir().unwrap();
        let repo_path = temp_dir.path();

        // Initialize a git repository
        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["init", "--initial-branch", "main"])
            .output()
            .unwrap();

        std::fs::write(repo_path.join("README.md"), "# Test Repo").unwrap();

        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["add", "README.md"])
            .output()
            .unwrap();

        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["commit", "-m", "Initial commit"])
            .output()
            .unwrap();

        let backend = git_backend(repo_path.to_path_buf());

        let git_info = collect_git_info(&backend, repo_path)
            .await
            .expect("git info should be collected");

        assert!(git_info.commit_hash.is_some());
    }

    #[tokio::test]
    async fn collect_git_info_with_remote() {
        let temp_dir = tempdir().unwrap();
        let repo_path = temp_dir.path();

        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["init", "--initial-branch", "main"])
            .output()
            .unwrap();

        std::fs::write(repo_path.join("README.md"), "# Test Repo").unwrap();
        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["add", "README.md"])
            .output()
            .unwrap();

        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["commit", "-m", "Initial commit"])
            .output()
            .unwrap();

        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["remote", "add", "origin", "https://example.com/test.git"])
            .output()
            .unwrap();

        let backend = git_backend(repo_path.to_path_buf());

        let git_info = collect_git_info(&backend, repo_path)
            .await
            .expect("git info should be collected");

        assert_eq!(
            git_info.repository_url.as_deref(),
            Some("https://example.com/test.git"),
        );
    }

    #[tokio::test]
    async fn collect_git_info_detached_head() {
        let temp_dir = tempdir().unwrap();
        let repo_path = temp_dir.path();

        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["init", "--initial-branch", "main"])
            .output()
            .unwrap();

        std::fs::write(repo_path.join("README.md"), "# Test Repo").unwrap();
        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["add", "README.md"])
            .output()
            .unwrap();

        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["commit", "-m", "Initial commit"])
            .output()
            .unwrap();

        let commit_hash = String::from_utf8(
            std::process::Command::new("git")
                .current_dir(repo_path)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();
        let commit_hash = commit_hash.trim().to_string();

        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["checkout", &commit_hash])
            .output()
            .unwrap();

        let backend = git_backend(repo_path.to_path_buf());

        let git_info = collect_git_info(&backend, repo_path)
            .await
            .expect("git info should be collected");

        assert!(git_info.branch.is_none());
    }

    #[tokio::test]
    async fn collect_git_info_with_branch() {
        let temp_dir = tempdir().unwrap();
        let repo_path = temp_dir.path();

        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["init", "--initial-branch", "main"])
            .output()
            .unwrap();

        std::fs::write(repo_path.join("README.md"), "# Test Repo").unwrap();
        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["add", "README.md"])
            .output()
            .unwrap();

        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["commit", "-m", "Initial commit"])
            .output()
            .unwrap();

        std::process::Command::new("git")
            .current_dir(repo_path)
            .args(["checkout", "-b", "feature"])
            .output()
            .unwrap();

        let backend = git_backend(repo_path.to_path_buf());

        let git_info = collect_git_info(&backend, repo_path)
            .await
            .expect("git info should be collected");

        assert_eq!(git_info.branch.as_deref(), Some("feature"));
    }

    #[tokio::test]
    async fn collect_git_info_returns_none_for_non_git_backend() {
        struct DummyBackend;

        impl RevisionControlSystem for DummyBackend {
            fn kind(&self) -> RevisionControlKind {
                RevisionControlKind::Darcs
            }

            fn root(&self) -> &Path {
                Path::new("/")
            }

            fn capabilities(&self) -> crate::revision_control::RevisionControlCapabilities {
                crate::revision_control::RevisionControlCapabilities::new(false, false)
            }
        }

        let backend = DummyBackend;
        let result = collect_git_info(&backend, Path::new("/tmp")).await;

        assert!(result.is_none());
    }

    #[test]
    fn git_info_serialization_includes_fields() {
        let info = GitInfo {
            commit_hash: Some("abc123def456".to_string()),
            branch: Some("main".to_string()),
            repository_url: Some("https://example.com/repo.git".to_string()),
        };

        let json = serde_json::to_string(&info).expect("serialization should succeed");
        let parsed: Value = serde_json::from_str(&json).expect("json should parse");

        assert_eq!(parsed["commit_hash"], "abc123def456");
        assert_eq!(parsed["branch"], "main");
        assert_eq!(parsed["repository_url"], "https://example.com/repo.git");
    }

    #[test]
    fn git_info_serialization_skips_nones() {
        let info = GitInfo {
            commit_hash: None,
            branch: None,
            repository_url: None,
        };

        let json = serde_json::to_string(&info).expect("serialization should succeed");
        let parsed: Value = serde_json::from_str(&json).expect("json should parse");

        let object = parsed.as_object().expect("expected json object");
        assert!(!object.contains_key("commit_hash"));
        assert!(!object.contains_key("branch"));
        assert!(!object.contains_key("repository_url"));
    }
}
