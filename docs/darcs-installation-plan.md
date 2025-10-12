# Darcs installation plan

To ensure integration tests that rely on the `darcs` binary can run without being skipped, we will install the Debian package in the setup hook.

## Approach

1. Reuse the system package manager instead of building `darcs` from source. The Debian package is sufficient for the tests and keeps installation time short.
2. Provide an idempotent helper script (`scripts/install_darcs.sh`) that the Codex setup environment hook can call. The script checks for an existing `darcs` binary before running `apt-get` so it can be safely re-run.
3. Run the script from the setup hook with network access. The script escalates with `sudo` when necessary, updates the package index, and installs `darcs` with minimal dependencies.

## Usage

Add the following invocation to the Codex setup environment hook:

```bash
/workspace/codex/scripts/install_darcs.sh
```

This will ensure the `darcs` binary is available before the tests execute.
