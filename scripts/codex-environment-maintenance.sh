#!/usr/bin/env bash
set -euo pipefail

# This helper runs on subsequent Codex development environment starts. It keeps
# required tooling available and refreshes dependency caches so the environment
# stays ready for offline work.

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"

"${script_dir}/ensure_nix.sh"
"${script_dir}/prefetch_ratatui_git_dependency.sh"
"${script_dir}/install_darcs.sh"
