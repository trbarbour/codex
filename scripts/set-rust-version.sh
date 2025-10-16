#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 <version>

Updates the Rust toolchain pin in codex-rs/rust-toolchain.toml to the provided
version (for example, 1.90.0).
USAGE
}

if [[ ${1:-} == "-h" || ${1:-} == "--help" ]]; then
  usage
  exit 0
fi

if [[ $# -ne 1 ]]; then
  usage >&2
  exit 1
fi

VERSION="$1"

if ROOT=$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null); then
  cd "$ROOT"
else
  SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd -P)
  cd "$SCRIPT_DIR/.."
fi

if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ || "$VERSION" =~ ^(stable|beta|nightly)(-[0-9]{4}-[0-9]{2}-[0-9]{2})?$ ]]; then
  cat <<MSG >&2
set-rust-version: unexpected version format '$VERSION'.
Provide a full Rust release (e.g. 1.90.0) or toolchain channel (stable/beta/nightly).
MSG
  exit 1
fi

perl -0pi -e "s/channel = \"[^\"]+\"/channel = \"${VERSION}\"/" codex-rs/rust-toolchain.toml

echo "Pinned Rust toolchain channel ${VERSION}." >&2
echo "Run 'nix develop .#codex-rs' to refresh the shell with the new toolchain." >&2
