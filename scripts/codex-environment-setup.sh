#!/usr/bin/env bash
set -euo pipefail

# This helper prepares the Codex development environment when a workspace is
# first created. It installs required tooling and warms dependency caches so
# subsequent builds and tests can run without network access.

timestamp() {
  date -u +"%Y-%m-%dT%H:%M:%SZ"
}

log_dir=${CODEX_ENV_LOG_DIR:-/var/tmp/codex-setup}
mkdir -p "${log_dir}"

log_file=${CODEX_ENV_SETUP_LOG_FILE:-"${log_dir}/$(timestamp)-codex-environment-setup-$$.log"}
touch "${log_file}"
export CODEX_ENV_SETUP_LOG_FILE="${log_file}"

exec > >(tee -a "${log_file}")
exec 2>&1

log_info() {
  printf '[%s] [codex-environment-setup] %s\n' "$(timestamp)" "$*"
}

log_error() {
  printf '[%s] [codex-environment-setup] ERROR: %s\n' "$(timestamp)" "$*"
}

run_step() {
  local description=$1
  shift

  log_info "Starting ${description}"
  if "$@"; then
    log_info "Finished ${description}"
    return 0
  fi

  local status=$?
  log_error "${description} failed with exit code ${status}"
  return "${status}"
}

run_step_or_exit() {
  if run_step "$@"; then
    return 0
  fi

  local status=$?
  exit "${status}"
}

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"

log_info "codex-environment-setup.sh starting (PID $$)"

run_step_or_exit "ensure_nix.sh" "${script_dir}/ensure_nix.sh"
run_step_or_exit "ensure_multipass.sh" "${script_dir}/ensure_multipass.sh"
run_step_or_exit "prefetch_ratatui_git_dependency.sh" "${script_dir}/prefetch_ratatui_git_dependency.sh"
run_step_or_exit "install_darcs.sh" "${script_dir}/install_darcs.sh"

log_info "codex-environment-setup.sh completed successfully"
