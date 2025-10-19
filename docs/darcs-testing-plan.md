# Comprehensive Darcs testing plan

This document captures the end-to-end verification plan for the Darcs support
work that landed on the `trbarbour/darcs_support` branch. It enumerates the
objectives, automated coverage, and manual validation needed to ensure Darcs
repositories behave on par with Git across the Codex product surface.

## Objectives

- Validate Darcs repositories are detected, warn when the CLI is missing, and
  feed metadata/diffs through the new backend so the UI and rollouts behave like
  Git workspaces.
- Confirm Codex’s onboarding, snapshots/undo flow, review prompts, rollout
  metadata, and CLI gating all adapt to Darcs repositories.
- Exercise CI/tooling paths that install Darcs, ensure tests skip gracefully
  when the CLI is unavailable, and verify documentation/release tooling covers
  Darcs workflows.

## Environment & tooling setup

- Install the `darcs` binary before running tests or manual checks via
  `scripts/install_darcs.sh` or `just install`, both of which fall back across
  package managers.
- Capture baseline `darcs --version` output and record PATH manipulations used
  for negative tests.
- Prepare throwaway Darcs repos (`darcs init`) with tracked/untracked files for
  functional scenarios.

## Automated regression suite

1. **Core Darcs integration tests** – `cargo test -p codex-core
   revision_control_darcs` to cover metadata collection, workspace diffs, and
   rollout persistence in real Darcs repos (skips automatically if the CLI is
   absent).
2. **Initial guidance regression** – `cargo test -p codex-core
   darcs_repositories_emit_initial_guidance` to ensure user guidance is injected
   into new sessions.
3. **Seatbelt permissions (macOS)** – On a macOS runner, `cargo test -p
   codex-core seatbelt` to verify `_darcs` directories become read-only when the
   repo root is writable.
4. **Snapshot manager compilation/tests** – `cargo test -p codex-git-tooling`
   to exercise Darcs snapshot code paths alongside Git ghosts.
5. **Cloud tasks** – `cargo test -p codex-cloud-tasks` (and add targeted cases if
   missing) to confirm Darcs remotes are parsed from `_darcs/prefs/repos`/`darcs
   show repo`.
6. **Full workspace sanity** – `cargo test --workspace --all-features` with
   Darcs installed to smoke-test unrelated crates for regressions.

## Manual functional verification

### Repository detection & onboarding

- Start Codex inside a Darcs repo; confirm onboarding/trust steps flag the repo,
  highlight Darcs-specific messaging, and surface tooling errors if the CLI is
  missing.
- Launch a new session and verify the initial context message advising the
  Darcs CLI appears exactly once and includes the CLI warning when PATH lacks
  `darcs`.

### Diff & metadata plumbing

- From the TUI `/diff` view, confirm it invokes `darcs::workspace_diff`,
  includes untracked additions, and gracefully handles empty diffs; cross-check
  against `darcs whatsnew --unified --look-for-adds`.
- Trigger rollout generation (e.g., non-interactive `codex exec`) and inspect
  JSON lines to verify Darcs metadata (patch hash, remote) is persisted in
  session meta entries.

### Snapshots & undo

- With a Darcs repo, send a user message to capture a snapshot; then modify
  files and invoke undo to ensure `RepoSnapshotManager` restores the working tree
  and surfaces errors when the CLI is absent.
- Validate snapshot storage under `codex_home/tmp/snapshots` is cleaned up and
  respects scoped restores.

### Review & UI flows

- Open the review popup in a Darcs repo to confirm Darcs-specific options
  (“Review pending patches”, tailored workspace prompt) appear and trigger the
  correct prompts/actions.
- Verify `/review` commands respect Darcs context and gather pending patch info
  via CLI.

### Exec & trust gating

- Run `codex exec ...` outside any repo and within a Darcs repo to confirm the
  CLI enforces the repo requirement unless `--skip-git-repo-check` is passed.
- Exercise trust decisions to ensure Darcs roots become eligible for persistent
  trust storage.

### Sandbox & filesystem protection

- On macOS, manually probe Seatbelt policies (or rely on tests) to ensure
  `_darcs` contents are blocked when only the repo root is writable.
- On other platforms, spot-check container policies so Darcs metadata isn’t
  mutated unintentionally.

### Environment detection & releases

- Populate `_darcs/prefs/repos` with multiple remotes and confirm environment
  discovery (e.g., via any CLI surfaces using `list_environments`) reports Darcs
  origins.
- In a Darcs repo, run `python codex-rs/scripts/create_github_release --backend
  darcs --dry-run` to ensure tagging/push steps call the Darcs CLI and error
  messages are user-friendly when tooling is missing.

## Negative / skip scenarios

- Temporarily hide `darcs` from PATH and verify detection returns a tooling
  error, onboarding displays the warning, snapshots disable themselves with
  actionable hints, and integration tests skip with a message rather than
  failing.
- Run release tooling and environment detection without Darcs installed to
  confirm graceful failures.

## Cross-platform & CI validation

- Ensure GitHub Actions runners install Darcs successfully or log skip warnings;
  replicate locally by invoking the workflow steps or `just install` on
  macOS/Linux/Windows shells.
- Smoke-test Darcs operations on Windows (PowerShell) since the release script
  and installers cover that platform.

## Documentation & support checks

- Review updated docs (getting started, exec, revision control) to confirm they
  match observed behavior (e.g., onboarding warnings, required CLI) and link to
  installation guidance.
- Verify `docs/queues/darcs.md` roadmap items marked as complete align with the
  implemented/tested features.

## Observability

- Capture logs for Darcs CLI invocations and warning messages (from tracing
  output) to aid debugging if commands time out or fail.

## Testing status

- ⚠️ Tests not run (QA planning only).

