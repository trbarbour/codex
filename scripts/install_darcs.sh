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
arch="$(dpkg --print-architecture)"
temp_sources="$(mktemp)"
temp_sources_dir="$(mktemp -d)"
cleanup_files=("${temp_sources}" "${temp_sources_dir}")
cleanup() {
  rm -rf "${cleanup_files[@]}"
}
trap cleanup EXIT

primary_mirror="$({
  python3 - "${VERSION_CODENAME}" "${arch}" "${temp_sources}" <<'PY'
import glob
import sys

codename, arch, dest = sys.argv[1:4]


def default_mirror(arch_name: str) -> str:
    ports_arches = {
        "arm64",
        "armel",
        "armhf",
        "ppc64el",
        "riscv64",
        "s390x",
    }
    if arch_name in ports_arches:
        return "http://ports.ubuntu.com/ubuntu-ports"
    return "http://archive.ubuntu.com/ubuntu"


def load_entries():
    paths = ["/etc/apt/sources.list"]
    paths.extend(sorted(glob.glob("/etc/apt/sources.list.d/*.list")))
    for path in paths:
        try:
            with open(path, "r", encoding="utf-8") as handle:
                for raw_line in handle:
                    line = raw_line.strip()
                    if not line or line.startswith("#"):
                        continue
                    parts = line.split()
                    if parts[0] != "deb" or len(parts) < 3:
                        continue
                    uri = parts[1].rstrip("/")
                    suite = parts[2]
                    components = parts[3:]
                    yield uri, suite, components
        except FileNotFoundError:
            continue


entries = list(load_entries())


def lookup_suite(suite: str):
    for uri, entry_suite, components in entries:
        if entry_suite == suite:
            return uri, components
    return None


lines = []
suite_names = [
    (codename, False),
    (f"{codename}-updates", False),
    (f"{codename}-security", True),
]
primary_uri = None

for suite, is_security in suite_names:
    entry = lookup_suite(suite)
    if entry is None:
        if is_security:
            uri = "http://security.ubuntu.com/ubuntu"
        else:
            uri = default_mirror(arch)
        components = ["main", "universe", "multiverse", "restricted"]
    else:
        uri, components = entry
        if not components:
            components = ["main", "universe", "multiverse", "restricted"]
    if primary_uri is None:
        primary_uri = uri
    component_str = " ".join(components)
    lines.append(f"deb {uri} {suite} {component_str}\n")

with open(dest, "w", encoding="utf-8") as dest_file:
    dest_file.writelines(lines)

print(primary_uri)
PY
} | tr -d '\n')"

APT_ARGS=(-o Dir::Etc::sourcelist="${temp_sources}" -o Dir::Etc::sourceparts="${temp_sources_dir}")

apt-get update "${APT_ARGS[@]}"
if ! apt-get install "${APT_ARGS[@]}" -y --no-install-recommends darcs; then
  echo "Falling back to installing darcs from the Ubuntu archive" >&2
  archive_url="${primary_mirror%/}/pool/universe/d/darcs/"
  deb_name="$(
    python3 - "$archive_url" "${arch}" <<'PY'
import re
import sys
import urllib.request

url, arch = sys.argv[1:3]
with urllib.request.urlopen(url) as resp:
    html = resp.read().decode()

pattern = re.compile(rf'href="(darcs_[0-9][^"]*_{re.escape(arch)}\.deb)"')
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
