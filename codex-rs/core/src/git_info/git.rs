use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

use codex_app_server_protocol::GitSha;
use codex_protocol::protocol::GitInfo;
use futures::future::join_all;
use serde::Deserialize;
use serde::Serialize;
use tokio::process::Command;
use tokio::time::Duration as TokioDuration;
use tokio::time::timeout;

use crate::revision_control::git::get_git_repo_root;

/// Timeout for git commands to prevent freezing on large repositories
const GIT_COMMAND_TIMEOUT: TokioDuration = TokioDuration::from_secs(5);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GitDiffToRemote {
    pub sha: GitSha,
    pub diff: String,
}

/// Collect git repository information from the given working directory using command-line git.
/// Returns None if no git repository is found or if git operations fail.
/// Uses timeouts to prevent freezing on large repositories.
/// All git commands (except the initial repo check) run in parallel for better performance.
pub(super) async fn collect_git_info(cwd: &Path) -> Option<GitInfo> {
    // Check if we're in a git repository first
    let is_git_repo = run_git_command_with_timeout(&["rev-parse", "--git-dir"], cwd)
        .await?
        .status
        .success();

    if !is_git_repo {
        return None;
    }

    // Run all git info collection commands in parallel
    let (commit_result, branch_result, url_result) = tokio::join!(
        run_git_command_with_timeout(&["rev-parse", "HEAD"], cwd),
        run_git_command_with_timeout(&["rev-parse", "--abbrev-ref", "HEAD"], cwd),
        run_git_command_with_timeout(&["remote", "get-url", "origin"], cwd)
    );

    let mut git_info = GitInfo {
        commit_hash: None,
        branch: None,
        repository_url: None,
    };

    // Process commit hash
    if let Some(output) = commit_result
        && output.status.success()
        && let Ok(hash) = String::from_utf8(output.stdout)
    {
        git_info.commit_hash = Some(hash.trim().to_string());
    }

    // Process branch name
    if let Some(output) = branch_result
        && output.status.success()
        && let Ok(branch) = String::from_utf8(output.stdout)
    {
        let branch = branch.trim();
        if branch != "HEAD" {
            git_info.branch = Some(branch.to_string());
        }
    }

    // Process repository URL
    if let Some(output) = url_result
        && output.status.success()
        && let Ok(url) = String::from_utf8(output.stdout)
    {
        git_info.repository_url = Some(url.trim().to_string());
    }

    Some(git_info)
}

/// A minimal commit summary entry used for pickers (subject + timestamp + sha).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitLogEntry {
    pub sha: String,
    /// Unix timestamp (seconds since epoch) of the commit time (committer time).
    pub timestamp: i64,
    /// Single-line subject of the commit message.
    pub subject: String,
}

/// Return the last `limit` commits reachable from HEAD for the current branch.
/// Each entry contains the SHA, commit timestamp (seconds), and subject line.
/// Returns an empty vector if not in a git repo or on error/timeout.
pub(super) async fn recent_commits(cwd: &Path, limit: usize) -> Vec<CommitLogEntry> {
    // Ensure we're in a git repo first to avoid noisy errors.
    let Some(out) = run_git_command_with_timeout(&["rev-parse", "--git-dir"], cwd).await else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }

    let fmt = "%H%x1f%ct%x1f%s"; // <sha> <US> <commit_time> <US> <subject>
    let n = limit.max(1).to_string();
    let Some(log_out) =
        run_git_command_with_timeout(&["log", "-n", &n, &format!("--pretty=format:{fmt}")], cwd)
            .await
    else {
        return Vec::new();
    };
    if !log_out.status.success() {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(&log_out.stdout);
    let mut entries: Vec<CommitLogEntry> = Vec::new();
    for line in text.lines() {
        let mut parts = line.split('\u{001f}');
        let sha = parts.next().unwrap_or("").trim();
        let ts_s = parts.next().unwrap_or("").trim();
        let subject = parts.next().unwrap_or("").trim();
        if sha.is_empty() || ts_s.is_empty() {
            continue;
        }
        let timestamp = ts_s.parse::<i64>().unwrap_or(0);
        entries.push(CommitLogEntry {
            sha: sha.to_string(),
            timestamp,
            subject: subject.to_string(),
        });
    }

    entries
}

/// Returns the closest git sha to HEAD that is on a remote as well as the diff to that sha.
pub(super) async fn git_diff_to_remote(cwd: &Path) -> Option<GitDiffToRemote> {
    get_git_repo_root(cwd)?;

    let remotes = get_git_remotes(cwd).await?;
    let branches = branch_ancestry(cwd).await?;
    let base_sha = find_closest_sha(cwd, &branches, &remotes).await?;
    let diff = diff_against_sha(cwd, &base_sha).await?;

    Some(GitDiffToRemote {
        sha: base_sha,
        diff,
    })
}

/// Run a git command with a timeout to prevent blocking on large repositories
async fn run_git_command_with_timeout(args: &[&str], cwd: &Path) -> Option<std::process::Output> {
    let result = timeout(
        GIT_COMMAND_TIMEOUT,
        Command::new("git").args(args).current_dir(cwd).output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => Some(output),
        _ => None, // Timeout or error
    }
}

async fn get_git_remotes(cwd: &Path) -> Option<Vec<String>> {
    let output = run_git_command_with_timeout(&["remote"], cwd).await?;
    if !output.status.success() {
        return None;
    }
    let mut remotes: Vec<String> = String::from_utf8(output.stdout)
        .ok()?
        .lines()
        .map(str::to_string)
        .collect();
    if let Some(pos) = remotes.iter().position(|r| r == "origin") {
        let origin = remotes.remove(pos);
        remotes.insert(0, origin);
    }
    Some(remotes)
}

/// Attempt to determine the repository's default branch name.
///
/// Preference order:
/// 1) The symbolic ref at `refs/remotes/<remote>/HEAD` for the first remote (origin prioritized)
/// 2) `git remote show <remote>` parsed for "HEAD branch: <name>"
/// 3) Local fallback to existing `main` or `master` if present
async fn get_default_branch(cwd: &Path) -> Option<String> {
    // Prefer the first remote (with origin prioritized)
    let remotes = get_git_remotes(cwd).await.unwrap_or_default();
    for remote in remotes {
        // Try symbolic-ref, which returns something like: refs/remotes/origin/main
        if let Some(symref_output) = run_git_command_with_timeout(
            &[
                "symbolic-ref",
                "--quiet",
                &format!("refs/remotes/{remote}/HEAD"),
            ],
            cwd,
        )
        .await
            && symref_output.status.success()
            && let Ok(sym) = String::from_utf8(symref_output.stdout)
        {
            let trimmed = sym.trim();
            if let Some((_, name)) = trimmed.rsplit_once('/') {
                return Some(name.to_string());
            }
        }

        // Fall back to parsing `git remote show <remote>` output
        if let Some(show_output) =
            run_git_command_with_timeout(&["remote", "show", &remote], cwd).await
            && show_output.status.success()
            && let Ok(text) = String::from_utf8(show_output.stdout)
        {
            for line in text.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("HEAD branch:") {
                    let name = rest.trim();
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
        }
    }

    // No remote-derived default; try common local defaults if they exist
    get_default_branch_local(cwd).await
}

/// Attempt to determine the repository's default branch name from local branches.
async fn get_default_branch_local(cwd: &Path) -> Option<String> {
    for candidate in ["main", "master"] {
        if let Some(verify) = run_git_command_with_timeout(
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("refs/heads/{candidate}"),
            ],
            cwd,
        )
        .await
            && verify.status.success()
        {
            return Some(candidate.to_string());
        }
    }

    None
}

/// Build an ancestry of branches starting at the current branch and ending at the
/// repository's default branch (if determinable)..
async fn branch_ancestry(cwd: &Path) -> Option<Vec<String>> {
    // Discover current branch (ignore detached HEAD by treating it as None)
    let current_branch = run_git_command_with_timeout(&["rev-parse", "--abbrev-ref", "HEAD"], cwd)
        .await
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string())
        .filter(|s| s != "HEAD");

    // Discover default branch
    let default_branch = get_default_branch(cwd).await;

    let mut ancestry: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    if let Some(cb) = current_branch.clone() {
        seen.insert(cb.clone());
        ancestry.push(cb);
    }
    if let Some(db) = default_branch
        && !seen.contains(&db)
    {
        seen.insert(db.clone());
        ancestry.push(db);
    }

    // Expand candidates: include any remote branches that already contain HEAD.
    // This addresses cases where we're on a new local-only branch forked from a
    // remote branch that isn't the repository default. We prioritize remotes in
    // the order returned by get_git_remotes (origin first).
    let remotes = get_git_remotes(cwd).await.unwrap_or_default();
    for remote in remotes {
        if let Some(output) = run_git_command_with_timeout(
            &[
                "for-each-ref",
                "--format=%(refname:short)",
                "--contains=HEAD",
                &format!("refs/remotes/{remote}"),
            ],
            cwd,
        )
        .await
            && output.status.success()
            && let Ok(text) = String::from_utf8(output.stdout)
        {
            for line in text.lines() {
                let short = line.trim();
                // Expect format like: "origin/feature"; extract the branch path after "remote/"
                if let Some(stripped) = short.strip_prefix(&format!("{remote}/"))
                    && !stripped.is_empty()
                    && !seen.contains(stripped)
                {
                    seen.insert(stripped.to_string());
                    ancestry.push(stripped.to_string());
                }
            }
        }
    }

    // Ensure we return Some vector, even if empty, to allow caller logic to proceed
    Some(ancestry)
}

// Helper for a single branch: return the remote SHA if present on any remote
// and the distance (commits ahead of HEAD) for that branch. The first item is
// None if the branch is not present on any remote. Returns None if distance
// could not be computed due to git errors/timeouts.
async fn branch_remote_and_distance(
    cwd: &Path,
    branch: &str,
    remotes: &[String],
) -> Option<(Option<GitSha>, usize)> {
    // Try to find the first remote ref that exists for this branch (origin prioritized by caller).
    let mut found_remote_sha: Option<GitSha> = None;
    let mut found_remote_ref: Option<String> = None;
    for remote in remotes {
        let remote_ref = format!("refs/remotes/{remote}/{branch}");
        let Some(verify_output) =
            run_git_command_with_timeout(&["rev-parse", "--verify", "--quiet", &remote_ref], cwd)
                .await
        else {
            // Mirror previous behavior: if the verify call times out/fails at the process level,
            // treat the entire branch as unusable.
            return None;
        };
        if !verify_output.status.success() {
            continue;
        }
        let Ok(sha) = String::from_utf8(verify_output.stdout) else {
            // Mirror previous behavior and skip the entire branch on parse failure.
            return None;
        };
        found_remote_sha = Some(GitSha::new(sha.trim()));
        found_remote_ref = Some(remote_ref);
        break;
    }

    // Compute distance as the number of commits HEAD is ahead of the branch.
    // Prefer local branch name if it exists; otherwise fall back to the remote ref (if any).
    let count_output = if let Some(local_count) =
        run_git_command_with_timeout(&["rev-list", "--count", &format!("{branch}..HEAD")], cwd)
            .await
    {
        if local_count.status.success() {
            local_count
        } else if let Some(remote_ref) = &found_remote_ref {
            match run_git_command_with_timeout(
                &["rev-list", "--count", &format!("{remote_ref}..HEAD")],
                cwd,
            )
            .await
            {
                Some(remote_count) => remote_count,
                None => return None,
            }
        } else {
            return None;
        }
    } else if let Some(remote_ref) = &found_remote_ref {
        match run_git_command_with_timeout(
            &["rev-list", "--count", &format!("{remote_ref}..HEAD")],
            cwd,
        )
        .await
        {
            Some(remote_count) => remote_count,
            None => return None,
        }
    } else {
        return None;
    };

    if !count_output.status.success() {
        return None;
    }
    let Ok(distance_str) = String::from_utf8(count_output.stdout) else {
        return None;
    };
    let Ok(distance) = distance_str.trim().parse::<usize>() else {
        return None;
    };

    Some((found_remote_sha, distance))
}

// Finds the closest sha that exist on any of branches and also exists on any of the remotes.
async fn find_closest_sha(cwd: &Path, branches: &[String], remotes: &[String]) -> Option<GitSha> {
    // A sha and how many commits away from HEAD it is.
    let mut closest_sha: Option<(GitSha, usize)> = None;
    for branch in branches {
        let Some((maybe_remote_sha, distance)) =
            branch_remote_and_distance(cwd, branch, remotes).await
        else {
            continue;
        };
        let Some(remote_sha) = maybe_remote_sha else {
            // Preserve existing behavior: skip branches that are not present on a remote.
            continue;
        };
        match &closest_sha {
            None => closest_sha = Some((remote_sha, distance)),
            Some((_, best_distance)) if distance < *best_distance => {
                closest_sha = Some((remote_sha, distance));
            }
            _ => {}
        }
    }
    closest_sha.map(|(sha, _)| sha)
}

async fn diff_against_sha(cwd: &Path, sha: &GitSha) -> Option<String> {
    let output =
        run_git_command_with_timeout(&["diff", "--no-textconv", "--no-ext-diff", &sha.0], cwd)
            .await?;
    // 0 is success and no diff.
    // 1 is success but there is a diff.
    let exit_ok = output.status.code().is_some_and(|c| c == 0 || c == 1);
    if !exit_ok {
        return None;
    }
    let mut diff = String::from_utf8(output.stdout).ok()?;

    if let Some(untracked_output) =
        run_git_command_with_timeout(&["ls-files", "--others", "--exclude-standard"], cwd).await
        && untracked_output.status.success()
    {
        let untracked: Vec<String> = String::from_utf8(untracked_output.stdout)
            .ok()?
            .lines()
            .map(str::to_string)
            .filter(|s| !s.is_empty())
            .collect();

        if !untracked.is_empty() {
            // Use platform-appropriate null device and guard paths with `--`.
            let null_device: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };
            let futures_iter = untracked.into_iter().map(|file| async move {
                let file_owned = file;
                let args_vec: Vec<&str> = vec![
                    "diff",
                    "--no-textconv",
                    "--no-ext-diff",
                    "--binary",
                    "--no-index",
                    // -- ensures that filenames that start with - are not treated as options.
                    "--",
                    null_device,
                    &file_owned,
                ];
                run_git_command_with_timeout(&args_vec, cwd).await
            });
            let results = join_all(futures_iter).await;
            for extra in results.into_iter().flatten() {
                if extra.status.code().is_some_and(|c| c == 0 || c == 1)
                    && let Ok(s) = String::from_utf8(extra.stdout)
                {
                    diff.push_str(&s);
                }
            }
        }
    }

    Some(diff)
}

/// Resolve the path that should be used for trust checks. Similar to
/// `[get_git_repo_root]`, but resolves to the root of the main
/// repository. Handles worktrees.
pub fn resolve_root_git_project_for_trust(cwd: &Path) -> Option<PathBuf> {
    let base = if cwd.is_dir() { cwd } else { cwd.parent()? };

    // TODO: we should make this async, but it's primarily used deep in
    // callstacks of sync code, and should almost always be fast
    let git_dir_out = std::process::Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(base)
        .output()
        .ok()?;
    if !git_dir_out.status.success() {
        return None;
    }
    let git_dir_s = String::from_utf8(git_dir_out.stdout)
        .ok()?
        .trim()
        .to_string();

    let git_dir_path_raw = if Path::new(&git_dir_s).is_absolute() {
        PathBuf::from(&git_dir_s)
    } else {
        base.join(&git_dir_s)
    };

    // Normalize to handle macOS /var vs /private/var and resolve ".." segments.
    let git_dir_path = std::fs::canonicalize(&git_dir_path_raw).unwrap_or(git_dir_path_raw);
    git_dir_path.parent().map(Path::to_path_buf)
}

/// Returns a list of local git branches.
/// Includes the default branch at the beginning of the list, if it exists.
pub(super) async fn local_git_branches(cwd: &Path) -> Vec<String> {
    let mut branches: Vec<String> = if let Some(out) =
        run_git_command_with_timeout(&["branch", "--format=%(refname:short)"], cwd).await
        && out.status.success()
    {
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        Vec::new()
    };

    branches.sort_unstable();

    if let Some(base) = get_default_branch_local(cwd).await
        && let Some(pos) = branches.iter().position(|name| name == &base)
    {
        let base_branch = branches.remove(pos);
        branches.insert(0, base_branch);
    }

    branches
}

/// Returns the current checked out branch name.
pub(super) async fn current_branch_name(cwd: &Path) -> Option<String> {
    let out = run_git_command_with_timeout(&["branch", "--show-current"], cwd).await?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|name| !name.is_empty())
}
