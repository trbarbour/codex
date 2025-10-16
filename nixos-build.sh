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
  echo "nixos-build.sh: nix command not found; install Nix or run inside NixOS." >&2
  exit 1
fi

echo "==> Building Rust workspace (codex-rs)"
nix develop .#codex-rs --command bash -c 'cd codex-rs && cargo build --workspace --locked'

echo

echo "==> Installing CLI npm dependencies"
nix develop .#codex-cli --command bash -c 'cd codex-cli && pnpm install --frozen-lockfile'

echo

echo "==> Building TypeScript SDK"
nix develop .#codex-cli --command bash -c 'cd sdk/typescript && pnpm install --frozen-lockfile && pnpm run build'

echo

echo "Build complete. Artifacts live under target/ for Rust and sdk/typescript/dist for the SDK."
