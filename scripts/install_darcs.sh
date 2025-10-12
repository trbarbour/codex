#!/usr/bin/env bash
set -euo pipefail

if command -v darcs >/dev/null 2>&1; then
  echo "darcs already installed: $(command -v darcs)"
  exit 0
fi

if [ "${EUID}" -ne 0 ]; then
  echo "Re-running with sudo to install darcs via apt." >&2
  exec sudo "$0" "$@"
fi

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends darcs
