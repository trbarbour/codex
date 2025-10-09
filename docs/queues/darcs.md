# Darcs integration backlog

This queue tracks the Darcs support roadmap described in [docs/git-and-github.md](../git-and-github.md).

Initial scaffolding for the abstraction now lives in
`codex_core::revision_control`, which exposes a shared detection entry point
that already handles Git and Darcs repositories so future backend-specific
logic can slot in next to the existing Git implementation.【F:codex-rs/core/src/revision_control/mod.rs†L1-L57】

## Implementation roadmap & work queue

### 1. Abstract repository/revision detection
Introduce a `RevisionControlSystem` trait that reports repository type, root, and capabilities. Update config loading, CLI
gating, and trust onboarding to use the abstraction instead of `.git` probes.

:::task-stub{title="Abstract revision-control detection across Git and Darcs"}
1. Add a new `codex_core::revision_control` module defining a `RevisionControlSystem` trait plus enums describing repository
   type/capabilities; move `get_git_repo_root` logic behind the Git implementation.
2. Update callers in `codex-rs/core/src/config.rs` (trust resolution), `codex-rs/core/src/git_info.rs`, `docs/exec.md` gating,
   and the TUI onboarding widget to depend on the trait instead of hard-coding Git.
3. Extend CLI flags (`--skip-git-repo-check`) and config parsing to report the detected backend (Git or Darcs) and gracefully
   handle “no revision control” cases.
:::

### 2. Factor existing Git helpers behind the abstraction
Refactor `codex_core::git_info`, `git-tooling`, and related modules to implement `RevisionControlSystem` for Git without
changing behaviour for current users.

:::task-stub{title="Refactor Git helpers to implement RevisionControlSystem"}
1. Split `codex_core::git_info` into a backend-neutral facade plus a Git-specific module that implements metadata fetchers,
   history queries, and default-branch resolution.
2. Update ghost snapshot code in `codex-rs/git-tooling` to consume the trait (e.g., rename to `RepoSnapshotManager`) while
   keeping Git logic unchanged; ensure error types remain descriptive.
3. Adjust rollouts (`codex-rs/core/src/rollout/recorder.rs`) and any tests that call `collect_git_info` to use the abstracted
   Git backend via dependency injection.
:::

### 3. Implement a Darcs backend for metadata and diffs
Create a Darcs implementation of the trait using commands like `darcs show repo`, `darcs changes`, and `darcs whatsnew`,
respecting the existing timeout and parallelism patterns.

:::task-stub{title="Add Darcs metadata and diff backend"}
1. Introduce `codex_core::revision_control::darcs` with helpers to locate `_darcs`, collect repo info (current patch hash,
   default remote, named branch), list recent patches, and compute workspace diffs/untracked files.
2. Mirror the timeout behaviour by wrapping Darcs subprocesses (e.g., `darcs show repo --xml`) and parse outputs into the
   shared `GitInfo`-like structures used by rollouts and the TUI.
3. Replace TUI diff plumbing (`codex-rs/tui/src/get_git_diff.rs`) with backend selection logic so it dispatches to either Git or
   Darcs implementations at runtime.
:::

### 4. Provide Darcs-backed ghost snapshots and undo
Design a snapshot strategy compatible with Darcs’ patch semantics to retain undo/redo functionality in the chat widget.

:::task-stub{title="Implement Darcs snapshot manager"}
1. Add a Darcs-specific `SnapshotManager` that uses `darcs record --dry-run` to capture staged changes and stores patch bundles
   or working-tree archives under Codex’s temp directory.
2. Implement restoration using Darcs commands (e.g., `darcs revert`, `darcs obliterate`, or applying saved bundles) and expose
   errors through a backend-neutral `SnapshotError`.
3. Update `chatwidget` to choose the appropriate manager based on detected revision control and adjust user-facing error
   messages (e.g., “current directory is not a Darcs repository”).
:::

### 5. Update UI/UX text and workflows for multiple revision-control backends
Ensure onboarding, slash commands, and informational messages adapt to Git or Darcs contexts, and expose Darcs-specific tooling
where appropriate.

:::task-stub{title="Localise UI flows for Git and Darcs contexts"}
1. Audit the TUI (`onboarding`, `slash_command`, diff panels) and CLI prompts to replace Git-only wording with backend-aware
   messaging.
2. Add Darcs-focused command suggestions (e.g., “review pending patches” slash command) and ensure diff viewers label patches
   appropriately.
3. Update history/metadata panes to surface Darcs patch identifiers, dependencies, and remote status alongside Git commit
   information.
:::

### 6. Extend sandbox and trust policies to recognise Darcs repositories
Modify seatbelt policies and trust resolution so `_darcs` directories receive appropriate protection (read-only by default when
the repo root is writable).

:::task-stub{title="Teach sandbox/trust layers about Darcs directories"}
1. Update sandbox root discovery to treat `_darcs` similarly to `.git`, ensuring read-only enforcement when the Darcs repo root
   is whitelisted.
2. Expand tests in `core/tests/suite/seatbelt.rs` to cover Darcs repositories (creating temporary `_darcs` structures) and
   verify permissions.
3. Adjust trust resolution helpers (`resolve_root_git_project_for_trust`) to return Darcs roots and persist trust decisions per
   backend.
:::

### 7. Integrate Darcs with environment detection and release tooling
Allow cloud environment detection to parse Darcs remotes and extend release scripts to optionally publish Darcs artifacts
alongside GitHub releases.

:::task-stub{title="Support Darcs in environment detection and publishing"}
1. Enhance `cloud-tasks/src/env_detect.rs` with functions that read `_darcs/prefs/repos` or `darcs show repo` to collect remote
   URLs, merging them with existing Git origins logic.
2. Gate `scripts/create_github_release` so it only runs in Git contexts and add a parallel Darcs release workflow (e.g.,
   invoking `darcs push` or uploading bundles) configurable via CLI flags.
3. Document how operators can choose between Git and Darcs publishing pipelines and ensure CI detects missing Darcs
   credentials gracefully.
:::

### 8. Update documentation and configuration guidance
Document Darcs setup, configuration flags, and feature parity in `docs/git-and-github.md`, onboarding guides, and config docs.

:::task-stub{title="Document Darcs support across guides"}
1. Revise `docs/git-and-github.md`, `docs/exec.md`, and onboarding docs to describe dual-backend behaviour, CLI flags for
   choosing revision control, and Darcs-specific workflows.
2. Add configuration examples (e.g., selecting Darcs as default) to `docs/config.md` and update installation instructions to
   mention the `darcs` dependency.
3. Provide migration guidance for teams moving existing Git projects to Darcs or running mixed backends.
:::

### 9. Build automated test coverage for Darcs
Extend unit and integration tests to exercise both Git and Darcs backends, with graceful skips if `darcs` is unavailable in CI.

:::task-stub{title="Add Darcs-aware test suites"}
1. Create Darcs fixtures in integration tests (mirroring `integration_git_info_unit_test`) that initialise repositories,
   generate patches, and validate metadata/diff parity.
2. Add backend matrix tests for ghost snapshots and rollout recording, verifying undo/redo and metadata persistence across Git
   and Darcs.
3. Update CI scripts/Justfile to install Darcs where possible and skip Darcs suites when the binary is missing, documenting the
   expected behaviour.
:::
