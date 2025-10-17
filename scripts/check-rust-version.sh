#!/usr/bin/env bash
set -euo pipefail

# Resolve repository root even when invoked through symlinks.
if ROOT=$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null); then
  cd "$ROOT"
else
  SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd -P)
  cd "$SCRIPT_DIR/.."
fi

if ! command -v rustc >/dev/null 2>&1; then
  echo "check-rust-version: rustc not found in PATH" >&2
  exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "check-rust-version: cargo not found in PATH" >&2
  exit 1
fi

PINNED_VERSION=$(sed -n 's/^channel = "\(.*\)"/\1/p' codex-rs/rust-toolchain.toml | head -n1)
if [[ -z "$PINNED_VERSION" ]]; then
  echo "check-rust-version: unable to determine pinned version from codex-rs/rust-toolchain.toml" >&2
  exit 1
fi

CURRENT_RUSTC=$(rustc --version | awk '{print $2}')
CURRENT_CARGO=$(cargo --version | awk '{print $2}')

if [[ "$CURRENT_RUSTC" != "$PINNED_VERSION" || "$CURRENT_CARGO" != "$PINNED_VERSION" ]]; then
  cat <<MSG >&2
Detected Rust toolchain version drift.
  pinned:  $PINNED_VERSION
  rustc:   $CURRENT_RUSTC
  cargo:   $CURRENT_CARGO
Please refresh the Rust pin in codex-rs/rust-toolchain.toml and rebuild the development shell.
MSG
  exit 1
fi

printf 'rustc %s / cargo %s match repository pin.\n' "$CURRENT_RUSTC" "$CURRENT_CARGO"
