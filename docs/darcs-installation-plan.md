# Darcs installation plan

To ensure integration tests that rely on the `darcs` binary can run without being skipped, we will install the Debian package in the setup hook.

## Approach

1. Reuse the system package manager instead of building `darcs` from source. The Debian package is sufficient for the tests and keeps installation time short.
2. Provide an idempotent helper script (`scripts/install_darcs.sh`) that the Codex setup environment hook can call. The script checks for an existing `darcs` binary before running `apt-get` so it can be safely re-run.
3. Warm the Rust dependency cache for git-sourced crates (currently `ratatui`) via `scripts/prefetch_ratatui_git_dependency.sh` so offline test runs succeed after the setup step completes.
4. Orchestrate both steps through `scripts/codex-environment-setup.sh` so the setup and maintenance hooks can rely on a single entry point.

## Usage

Add the following invocation to the Codex environment setup hook:

```bash
/workspace/codex/scripts/codex-environment-setup.sh
```

If the maintenance hook runs on every container start, point it at `scripts/codex-environment-maintenance.sh` to keep dependencies warm.

This ensures both the `darcs` binary and the patched `ratatui` dependency are available before the tests execute.
