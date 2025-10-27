#!/usr/bin/env bash
set -euo pipefail

# Ensures that Multipass is installed and configured to provide a kernel with
# Landlock support. On Linux this means preferring the QEMU driver (which boots
# full virtual machines with their own kernels) instead of the default LXD
# driver that reuses the host kernel.

timestamp() {
  date -u +"%Y-%m-%dT%H:%M:%SZ"
}

log_dir=${CODEX_ENV_LOG_DIR:-/var/tmp/codex-setup}
mkdir -p "${log_dir}"

if [[ -n "${CODEX_ENV_SETUP_LOG_FILE:-}" ]]; then
  log_file="${CODEX_ENV_SETUP_LOG_FILE}"
else
  log_file="${log_dir}/$(timestamp)-ensure_multipass-$$.log"
fi

exec > >(tee -a "${log_file}")
exec 2>&1

log() {
  local level=$1
  shift
  printf '[%s] [ensure_multipass] %s: %s\n' "$(timestamp)" "${level}" "$*"
}

log_info() {
  log INFO "$@"
}

log_warn() {
  log WARN "$@"
}

log_error() {
  log ERROR "$@"
}

log_info "ensure_multipass.sh starting (PID $$)"

maybe_sudo() {
  if [[ ${EUID:-$(id -u)} -eq 0 ]]; then
    "$@"
  elif command -v sudo >/dev/null 2>&1; then
    sudo "$@"
  else
    "$@"
  fi
}

install_multipass_linux() {
  log_info "Attempting Linux installation paths for Multipass"

  if command -v snap >/dev/null 2>&1; then
    log_info "snap detected; checking for existing Multipass snap"
    if snap list multipass >/dev/null 2>&1; then
      log_info "Multipass snap is already installed"
      return 0
    else
      local status=$?
      log_warn "snap list multipass exited with status ${status}; attempting snap installation"
    fi

    if maybe_sudo snap install multipass --classic; then
      log_info "Successfully installed Multipass via snap"
      return 0
    else
      local status=$?
      log_warn "snap install multipass --classic failed with exit code ${status}"
    fi
  fi

  if command -v apt-get >/dev/null 2>&1; then
    log_info "apt-get detected; preparing to use APT installers"
    export DEBIAN_FRONTEND=noninteractive

    # Multipass is primarily distributed as a snap.  If snapd is not available
    # yet, attempt to install it via apt so we can fall back to the snap
    # installer.
    if ! command -v snap >/dev/null 2>&1; then
      log_info "snap command not present; attempting to install snapd via apt"

      if maybe_sudo apt-get update; then
        log_info "apt-get update completed before snapd installation"
      else
        local status=$?
        log_warn "apt-get update failed with exit code ${status} before snapd installation; continuing"
      fi

      if maybe_sudo apt-get install -y snapd; then
        log_info "Successfully installed snapd"
        if command -v snap >/dev/null 2>&1; then
          log_info "snap command now available; checking for Multipass snap"
          if snap list multipass >/dev/null 2>&1; then
            log_info "Multipass snap is already installed after installing snapd"
            return 0
          else
            local status=$?
            log_warn "snap list multipass exited with status ${status} after installing snapd"
          fi

          if maybe_sudo snap install multipass --classic; then
            log_info "Successfully installed Multipass via snap after installing snapd"
            return 0
          else
            local status=$?
            log_warn "snap install multipass --classic failed with exit code ${status} after installing snapd"
          fi
        else
          log_warn "snap command still unavailable after installing snapd"
        fi
      else
        local status=$?
        log_warn "apt-get install -y snapd failed with exit code ${status}"
      fi
    fi

    if maybe_sudo apt-get update; then
      log_info "apt-get update completed before attempting direct Multipass install"
    else
      local status=$?
      log_warn "apt-get update failed with exit code ${status} before direct Multipass install"
    fi

    if maybe_sudo apt-get install -y multipass; then
      log_info "Successfully installed Multipass via apt"
      return 0
    else
      local status=$?
      log_warn "apt-get install -y multipass failed with exit code ${status}"
    fi
  fi

  log_warn "All Linux installation attempts for Multipass failed"
  return 1
}

install_multipass_darwin() {
  log_info "Attempting macOS installation paths for Multipass"
  if command -v brew >/dev/null 2>&1; then
    if brew list --versions multipass >/dev/null 2>&1; then
      log_info "Multipass already installed via Homebrew"
      return 0
    fi

    log_info "Installing Multipass via Homebrew"
    if brew install multipass; then
      log_info "Successfully installed Multipass via Homebrew"
      return 0
    else
      local status=$?
      log_warn "brew install multipass failed with exit code ${status}"
      return "${status}"
    fi
  fi

  log_warn "Homebrew is not available; cannot install Multipass on macOS"
  return 1
}

ensure_linux_driver() {
  local driver
  if ! driver=$(multipass get local.driver 2>/dev/null); then
    # Older Multipass releases may not support querying the driver; assume
    # defaults in that case.
    return 0
  fi

  driver=${driver,,}
  if [[ "${driver}" == "qemu" ]]; then
    return 0
  fi

  log_info "Configuring Multipass to use the 'qemu' driver for Landlock support"
  if multipass set local.driver=qemu >/dev/null 2>&1; then
    return 0
  fi

  log_warn "Failed to switch Multipass to the 'qemu' driver; Landlock support depends on the host kernel"
  return 1
}

case "$(uname -s)" in
  Linux)
    log_info "Detected Linux host"
    if ! command -v multipass >/dev/null 2>&1; then
      if install_multipass_linux; then
        log_info "Installation routine reported success"
      else
        status=$?
        log_warn "Installation routine exited with status ${status}; Multipass may still be unavailable"
        unset status
      fi
    fi
    ;;
  Darwin)
    log_info "Detected macOS host"
    if ! command -v multipass >/dev/null 2>&1; then
      if install_multipass_darwin; then
        log_info "Installation routine reported success"
      else
        status=$?
        log_warn "Installation routine exited with status ${status}; Multipass may still be unavailable"
        unset status
      fi
    fi
    ;;
  *)
    # Unsupported platform; nothing to do.
    log_warn "Unsupported platform '$(uname -s)'; skipping Multipass installation"
    ;;
 esac

if ! command -v multipass >/dev/null 2>&1; then
  log_warn "Multipass is not available; install it manually if you plan to run ./vm-test.sh"
  exit 0
fi

if [[ "$(uname -s)" == "Linux" ]]; then
  if ensure_linux_driver; then
    log_info "Confirmed Multipass is configured to use the qemu driver"
  else
    status=$?
    log_warn "Attempt to ensure the qemu driver exited with status ${status}"
    unset status
  fi
fi

if multipass version >/dev/null 2>&1; then
  log_info "Successfully queried Multipass version"
else
  status=$?
  log_warn "multipass version command failed with exit code ${status}"
  unset status
fi

log_info "ensure_multipass.sh completed"
