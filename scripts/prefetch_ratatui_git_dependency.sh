#!/usr/bin/env bash
set -euo pipefail

if [[ "${CODEX_SKIP_RATATUI_PREFETCH:-}" == "1" ]]; then
  exit 0
fi

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is not available; skipping ratatui prefetch." >&2
  exit 0
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"

cargo_home="${CARGO_HOME:-${HOME}/.cargo}"
git_checkout_dir="${cargo_home}/git/checkouts"
if [[ -d "${git_checkout_dir}" ]]; then
  ratatui_checkout="$(find "${git_checkout_dir}" -maxdepth 1 -type d -name 'ratatui-*' -print -quit 2>/dev/null || true)"
  if [[ -n "${ratatui_checkout}" ]]; then
    echo "ratatui git dependency already present at ${ratatui_checkout}"
    exit 0
  fi
fi

manifest="${repo_root}/codex-rs/Cargo.toml"
if [[ ! -f "${manifest}" ]]; then
  echo "Unable to locate workspace manifest at ${manifest}; skipping ratatui prefetch." >&2
  exit 0
fi

echo "Prefetching ratatui git dependency via cargo fetch"
cargo fetch --locked --manifest-path "${manifest}"
