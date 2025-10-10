# Git and GitHub integration

Codex relies on Git for both runtime features and release automation. This document maps each integration point to the
implementation so the behavior can be replicated elsewhere.

## Repository prerequisites and detection

* `codex exec` refuses to run outside a Git or Darcs checkout unless the caller opts out with
  `--skip-git-repo-check` to protect users from destructive edits in ad-hoc directories.【F:docs/exec.md†L86-L88】
* `codex_core::revision_control::git::get_git_repo_root` walks up from the configured working directory until it finds a `.git`
  directory or file, allowing the application to decide whether Git features should be enabled without shelling out to
  `git` itself.【F:codex-rs/core/src/revision_control/git.rs†L1-L34】
* `codex_core::revision_control::detect_revision_control` provides a single entry point for identifying the
  repository backend and now recognises both Git and Darcs checkouts without forcing every caller to reimplement the
  detection logic.【F:codex-rs/core/src/revision_control/mod.rs†L1-L57】
* When Codex is pointed at a non-Git directory, higher-level features such as ghost snapshots are disabled and the UI emits an
  informational message explaining why, preventing repeated failures.【F:codex-rs/tui/src/chatwidget.rs†L1288-L1322】

## Collecting repository metadata

The `codex_core::git_info` module centralizes Git queries and protects the UI from expensive or hanging processes by
wrapping every subprocess call in a five-second timeout.【F:codex-rs/core/src/git_info.rs†L39-L118】 Key helpers include:

* `collect_git_info`: concurrently collects the HEAD commit hash, current branch (ignoring detached HEAD), and the `origin`
  remote URL via `git rev-parse`/`git remote get-url`, returning `None` when Git is unavailable.【F:codex-rs/core/src/git_info.rs†L45-L96】
* `recent_commits`: shells out to `git log` with a stable `--pretty` format and parses the results into `(sha, timestamp,
  subject)` tuples for pickers and history views.【F:codex-rs/core/src/git_info.rs†L100-L155】
* `git_diff_to_remote`: identifies the nearest remote-tracking commit by enumerating remotes, inferring the default branch,
  and computing the diff between the working tree and that commit. The helper composes `get_git_remotes`,
  `branch_ancestry`, `find_closest_sha`, and `diff_against_sha` to produce both the base SHA and a diff blob.【F:codex-rs/core/src/git_info.rs†L157-L321】
* `local_git_branches` and `current_branch_name` expose branch pickers by scraping `git branch` output and moving the default
  branch (detected via symbolic refs or fallbacks to `main`/`master`) to the top of the list.【F:codex-rs/core/src/git_info.rs†L523-L604】

## Workspace diffs and safety snapshots

Two Rust components consume the metadata helpers to deliver user-facing functionality:

* `codex_tui::get_git_diff` mirrors the legacy TypeScript CLI by returning a combined diff that includes tracked changes and
  untracked files. It first checks whether Git is installed and the directory is a repository, then runs
  `git diff --color` and `git ls-files --others --exclude-standard` in parallel, synthesising `--no-index` diffs for each
  untracked path.【F:codex-rs/tui/src/get_git_diff.rs†L1-L75】
* The chat widget captures "ghost" snapshots before every user turn to enable undo. `create_ghost_commit` constructs a
  temporary index, stages the desired paths, writes the tree, and invokes `git commit-tree` with a synthetic identity, while
  `restore_ghost_commit` replays the snapshot back into the working tree via `git restore`.
  Errors (such as running outside a repo) are surfaced to the user and disable further snapshots until Codex restarts.
  【F:codex-rs/git-tooling/src/ghost_commits.rs†L63-L170】【F:codex-rs/tui/src/chatwidget.rs†L1254-L1334】

`GitToolingError` provides structured error reporting for all ghost-snapshot helpers so that UI components can decide when to
show hints or retry.【F:codex-rs/git-tooling/src/errors.rs†L8-L33】

## GitHub release automation

Codex administrators publish binaries via the `codex-rs/scripts/create_github_release` helper. The script drives the GitHub API
through the `gh` CLI to:

1. Discover the `main` branch head and associated tree.
2. Fetch and rewrite `codex-rs/Cargo.toml` with the target version.
3. Upload the new blob, graft it onto the original tree, and create a commit tagged `rust-v<version>`.
4. Create an annotated tag object and a matching ref pointing at the commit.【F:codex-rs/scripts/create_github_release†L1-L151】

The public release process relies on a dedicated GitHub Actions workflow triggered by that tag; the follow-up steps for npm and
Homebrew are tracked in `docs/release_management.md` for human operators.【F:docs/release_management.md†L1-L40】

# Darcs integration plan

Codex’s runtime, UI, and release tooling currently assume Git-based revision control. To support Darcs without regressing any
existing features, we will introduce a revision-control abstraction backed by Git or Darcs implementations while embracing
Darcs-specific capabilities such as patch-based history. The plan below restates the roadmap using the corrected terminology and
maintains the work queue for tracking.

## Current Git-dependent behaviour
- Execution requires a Git checkout unless the user opts out, and trust onboarding distinguishes repositories by calling
  `get_git_repo_root` and related helpers.
- Metadata capture (`collect_git_info`, `recent_commits`, `git_diff_to_remote`) feeds rollouts, tool pickers, and branch
  selectors, and all shell out to `git` with timeouts.
- Workspace diffs and ghost snapshots are Git-specific, returning `git diff` output and staging temporary commits to allow
  undo.
- Sandbox policies treat `.git` folders specially, and remote environment detection parses Git remotes.
- Release automation publishes Git commits/tags via the `create_github_release` script.

## Target Darcs user experience
- **Repository detection & trust:** Codex should recognise both `.git` and `_darcs` roots, gating advanced features unless one
  of the supported revision-control backends is available. Trust onboarding should present tailored messaging (e.g., “Darcs
  repository detected”) and allow Darcs users to opt into unattended mode just as Git users can.
- **Metadata & history:** Metadata panels show the latest Darcs patch hash, author/date, and branch (using Darcs “named
  branches” or default repository properties) along with remote URLs. Recent history should expose Darcs patch selection
  advantages, e.g., highlighting patch dependencies or unapplied patches when comparing to remotes.
- **Diffs & review:** `/diff` should call Darcs commands (`darcs diff --color=always`) and include untracked files via `darcs
  whatsnew`. Users can cherry-pick patches or view pending changesets beyond Git’s linear diff.
- **Ghost snapshots & undo:** Undo should use Darcs’ patch primitives (e.g., temporary `darcs record --dry-run` snapshots
  coupled with `darcs obliterate --all --last=1` for rollback) while preserving the existing restore UX.
- **Environment detection & release:** Cloud environment matching should inspect Darcs remote URLs (e.g., `darcs show repo` or
  `_darcs/prefs/repos`) so Codex deploy flows still auto-detect CI environments. Release tooling should optionally push Darcs
  bundles or tags to hub.darcs.net alongside the Git process.
- **Darcs-specific advantages:** Surface features like patch reordering, selective pulls, and conflict-free merges by letting
  users preview and apply pending remote patches directly from Codex, optionally displaying patch metadata (authors,
  dependencies) to support more granular reviews.

## Implementation roadmap & work queue

The detailed Darcs backlog has moved to [docs/queues/darcs.md](./queues/darcs.md). That file preserves the numbered
`:::task-stub` entries for contributors who want to continue the implementation work.

## Testing strategy
- **Unit tests:** Exercise Git and Darcs implementations of the revision-control trait, diff helpers, and snapshot managers with
  temporary repositories.
- **Integration tests:** Extend CLI/TUI end-to-end suites to run in both Git and Darcs repos, ensuring diff displays, ghost
  snapshots, and rollout metadata behave identically.
- **Sandbox tests:** Re-run seatbelt permission tests for both `.git` and `_darcs` roots to guarantee safe defaults.
- **Release tests:** Add dry-run tests for Git and Darcs release scripts to ensure the correct tool is invoked in each context.
- **Manual verification:** Validate Darcs-specific features (interactive patch selection, remote pull previews) in a sample
  repo to confirm UX parity.

