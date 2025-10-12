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

# Some Codex environments have additional apt sources preconfigured (for
# example, the mise repository) that may intermittently return 403 errors. Those
# failures cause `apt-get update` to exit with code 100 which prevents the setup
# hook from finishing. Generate a clean sources list that contains the standard
# Ubuntu repositories only so the script succeeds even when optional
# repositories are unavailable.
. /etc/os-release
temp_sources="$(mktemp)"
temp_sources_dir="$(mktemp -d)"
cleanup_files=("${temp_sources}" "${temp_sources_dir}")
cleanup() {
  rm -rf "${cleanup_files[@]}"
}
trap cleanup EXIT

cat >"${temp_sources}" <<REPOS
deb http://archive.ubuntu.com/ubuntu ${VERSION_CODENAME} main universe multiverse restricted
deb http://archive.ubuntu.com/ubuntu ${VERSION_CODENAME}-updates main universe multiverse restricted
deb http://security.ubuntu.com/ubuntu ${VERSION_CODENAME}-security main universe multiverse restricted
REPOS

APT_ARGS=(-o Dir::Etc::sourcelist="${temp_sources}" -o Dir::Etc::sourceparts="${temp_sources_dir}")

apt-get update "${APT_ARGS[@]}"
if ! apt-get install "${APT_ARGS[@]}" -y --no-install-recommends darcs; then
  echo "Falling back to installing darcs from the Ubuntu archive" >&2
  archive_url="http://archive.ubuntu.com/ubuntu/pool/universe/d/darcs/"
  deb_name="$(
    python3 - "$archive_url" <<'PY'
import re
import sys
import urllib.request

url = sys.argv[1]
with urllib.request.urlopen(url) as resp:
    html = resp.read().decode()

pattern = re.compile(r'href="(darcs_[0-9][^"]*_amd64\.deb)"')
candidates = pattern.findall(html)
if not candidates:
    sys.exit(1)

parts_re = re.compile(r'(\d+)')

def sort_key(name: str):
    parts = parts_re.split(name)
    return [int(part) if part.isdigit() else part for part in parts]

print(sorted(candidates, key=sort_key)[-1])
PY
  )" || {
    echo "Unable to locate a darcs .deb in ${archive_url}" >&2
    exit 1
  }

  tmp_deb="$(mktemp --suffix=.deb)"
  cleanup_files+=("${tmp_deb}")
  curl -fsSL "${archive_url}${deb_name}" -o "${tmp_deb}"
  chmod 644 "${tmp_deb}"
  apt-get install "${APT_ARGS[@]}" -y --no-install-recommends "${tmp_deb}"
fi
