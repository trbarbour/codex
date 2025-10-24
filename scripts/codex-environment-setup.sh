#!/usr/bin/env bash
set -euo pipefail

# This helper prepares the Codex development environment when a workspace is
# first created. It installs required tooling and warms dependency caches so
# subsequent builds and tests can run without network access.

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"

"${script_dir}/ensure_nix.sh"
"${script_dir}/ensure_multipass.sh"
"${script_dir}/prefetch_ratatui_git_dependency.sh"
"${script_dir}/install_darcs.sh"
