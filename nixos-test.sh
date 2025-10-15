#!/usr/bin/env bash
set -euo pipefail

# Resolve repository root even when invoked via a symlink.
if ROOT=$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null); then
  cd "$ROOT"
else
  SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd -P)
  cd "$SCRIPT_DIR"
fi

if ! command -v nix >/dev/null 2>&1; then
  echo "nixos-test.sh: nix command not found; install Nix or run inside NixOS." >&2
  exit 1
fi

echo "==> Running Rust tests (workspace, all features)"
nix develop .#codex-rs --command bash -c '
  set -euo pipefail
  cd codex-rs
  cargo test --workspace --all-features
'

echo

echo "==> Building Codex CLI binary for SDK tests"
nix develop .#codex-rs --command bash -c '
  set -euo pipefail
  cd codex-rs
  cargo build -p codex-cli
'

echo

echo "==> Running TypeScript SDK tests"
nix develop .#codex-cli --command bash -c '
  set -euo pipefail
  cd sdk/typescript
  pnpm install --frozen-lockfile
  pnpm test
'

echo

echo "Test suite finished. Some integration tests may be skipped when sandbox variables are detected."
