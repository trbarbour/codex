#!/usr/bin/env bash
set -euo pipefail

# Ensures that Multipass is installed and configured to provide a kernel with
# Landlock support. On Linux this means preferring the QEMU driver (which boots
# full virtual machines with their own kernels) instead of the default LXD
# driver that reuses the host kernel.

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
  if command -v snap >/dev/null 2>&1; then
    if snap list multipass >/dev/null 2>&1; then
      return 0
    fi
    if maybe_sudo snap install multipass --classic; then
      return 0
    fi
  fi

  if command -v apt-get >/dev/null 2>&1; then
    export DEBIAN_FRONTEND=noninteractive

    # Multipass is primarily distributed as a snap.  If snapd is not available
    # yet, attempt to install it via apt so we can fall back to the snap
    # installer.
    if ! command -v snap >/dev/null 2>&1; then
      maybe_sudo apt-get update || true
      if maybe_sudo apt-get install -y snapd; then
        if command -v snap >/dev/null 2>&1; then
          if snap list multipass >/dev/null 2>&1; then
            return 0
          fi
          if maybe_sudo snap install multipass --classic; then
            return 0
          fi
        fi
      fi
    fi

    maybe_sudo apt-get update
    if maybe_sudo apt-get install -y multipass; then
      return 0
    fi
  fi

  return 1
}

install_multipass_darwin() {
  if command -v brew >/dev/null 2>&1; then
    brew list --versions multipass >/dev/null 2>&1 || brew install multipass
    return 0
  fi
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

  echo "ensure_multipass.sh: configuring Multipass to use the 'qemu' driver for Landlock support" >&2
  if multipass set local.driver=qemu >/dev/null 2>&1; then
    return 0
  fi

  echo "ensure_multipass.sh: failed to switch Multipass to the 'qemu' driver; Landlock support depends on the host kernel" >&2
  return 1
}

case "$(uname -s)" in
  Linux)
    if ! command -v multipass >/dev/null 2>&1; then
      install_multipass_linux || true
    fi
    ;;
  Darwin)
    if ! command -v multipass >/dev/null 2>&1; then
      install_multipass_darwin || true
    fi
    ;;
  *)
    # Unsupported platform; nothing to do.
    :
    ;;
 esac

if ! command -v multipass >/dev/null 2>&1; then
  echo "ensure_multipass.sh: Multipass is not available; install it manually if you plan to run ./vm-test.sh" >&2
  exit 0
fi

if [[ "$(uname -s)" == "Linux" ]]; then
  ensure_linux_driver || true
fi

multipass version >/dev/null 2>&1 || true
