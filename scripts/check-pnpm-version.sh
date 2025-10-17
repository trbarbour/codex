#!/usr/bin/env bash
set -euo pipefail

# Resolve repository root even when invoked through symlinks.
if ROOT=$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null); then
  cd "$ROOT"
else
  SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd -P)
  cd "$SCRIPT_DIR/.."
fi

if ! command -v pnpm >/dev/null 2>&1; then
  echo "check-pnpm-version: pnpm not found in PATH" >&2
  exit 1
fi

if ! command -v node >/dev/null 2>&1; then
  echo "check-pnpm-version: node not found in PATH" >&2
  exit 1
fi

PINNED_VERSION=$(node -p "require('./package.json').packageManager.split('@')[1]")
CURRENT_VERSION=$(pnpm --version)

if [[ "$CURRENT_VERSION" != "$PINNED_VERSION" ]]; then
  cat <<MSG >&2
Detected pnpm version drift.
  pinned:  $PINNED_VERSION
  current: $CURRENT_VERSION
Please refresh the pnpm pin to realign the development shell.
MSG
  exit 1
fi

printf 'pnpm %s matches repository pin.\n' "$CURRENT_VERSION"
