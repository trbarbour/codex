#!/usr/bin/env bash
set -euo pipefail

# Ensures Multipass is available in the current environment. The helper tries
# to install the package when running on a Debian-based system. If installation
# is not possible, the script prints a warning but does not fail so callers can
# decide how to proceed (for example, vm-test.sh will fall back to a local run).

if command -v multipass >/dev/null 2>&1; then
  exit 0
fi

if command -v apt-get >/dev/null 2>&1; then
  export DEBIAN_FRONTEND=noninteractive

  apt_updated=0
  ensure_apt_updated() {
    if [[ ${apt_updated} -eq 0 ]]; then
      apt-get update
      apt_updated=1
    fi
  }

  # Only attempt the installation if the package exists in the repository to
  # avoid noisy errors on distributions where Multipass is distributed via snap.
  if apt-cache show multipass >/dev/null 2>&1; then
    ensure_apt_updated
    if ! dpkg-query -W -f='${Status}' multipass 2>/dev/null | grep -q 'install ok installed'; then
      apt-get install -y multipass
    fi
  fi
fi

if command -v multipass >/dev/null 2>&1; then
  exit 0
fi

cat >&2 <<'EOF'
warning: Multipass is not available on PATH. Install it manually to enable the
full vm-test.sh workflow.
EOF
