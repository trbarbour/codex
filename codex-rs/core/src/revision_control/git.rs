use std::path::Path;
use std::path::PathBuf;

/// Return `true` if the project folder specified by the `Config` is inside a
/// Git repository.
///
/// The check walks up the directory hierarchy looking for a `.git` file or
/// directory (note `.git` can be a file that contains a `gitdir` entry). This
/// approach does **not** require the `git` binary or the `git2` crate and is
/// therefore fairly lightweight.
///
/// Note that this does **not** detect *work-trees* created with
/// `git worktree add` where the checkout lives outside the main repository
/// directory. If you need Codex to work from such a checkout simply pass the
/// `--allow-no-git-exec` CLI flag that disables the repo requirement.
pub fn get_git_repo_root(base_dir: &Path) -> Option<PathBuf> {
    let mut dir = base_dir.to_path_buf();

    loop {
        if dir.join(".git").exists() {
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
    fn detects_nested_git_repository() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();

        let subdir = dir.path().join("nested");
        std::fs::create_dir(&subdir).unwrap();

        assert_eq!(get_git_repo_root(&subdir), Some(dir.path().to_path_buf()));
    }

    #[test]
    fn returns_none_for_non_repo() {
        let dir = tempdir().unwrap();
        assert!(get_git_repo_root(dir.path()).is_none());
    }
}
