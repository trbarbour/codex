#!/usr/bin/env bash
set -euo pipefail

# Ensures that the Nix package manager is installed and usable in the current
# environment. If Nix is already available, the script exits quickly.

if command -v nix >/dev/null 2>&1; then
  exit 0
fi

apt_updated=0
ensure_apt_package() {
  local pkg="$1"
  if ! command -v apt-get >/dev/null 2>&1; then
    return 1
  fi
  if [[ ${apt_updated} -eq 0 ]]; then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt_updated=1
  fi
  apt-get install -y "${pkg}"
}

ensure_command() {
  local cmd="$1"
  local pkg="$2"
  if command -v "${cmd}" >/dev/null 2>&1; then
    return 0
  fi
  if ensure_apt_package "${pkg}"; then
    return 0
  fi
  echo "error: required command '${cmd}' is not available and automatic installation failed" >&2
  return 1
}

ensure_command curl curl
ensure_command xz xz-utils
ensure_command tar tar

installer_url="https://install.determinate.systems/nix"
installer_dir="$(mktemp -d)"
trap 'rm -rf "${installer_dir}"' EXIT

curl --fail --location --silent --show-error "${installer_url}" -o "${installer_dir}/nix-installer.sh"
chmod +x "${installer_dir}/nix-installer.sh"
"${installer_dir}/nix-installer.sh" install --no-confirm

# Source the profile so that nix is available in the current shell as well.
if [[ -f /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh ]]; then
  # shellcheck disable=SC1091
  source /nix/var/nix/profiles/default/etc/profile.d/nix-daemon.sh
elif [[ -f "${HOME}/.nix-profile/etc/profile.d/nix.sh" ]]; then
  # shellcheck disable=SC1091
  source "${HOME}/.nix-profile/etc/profile.d/nix.sh"
fi

nix --version >/dev/null
