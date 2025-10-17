#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<USAGE
Usage: $0 <version>

Updates the pnpm version pin across the repository so the Nix dev shell and
package.json stay in sync.
USAGE
}

if [[ ${1:-} == "-h" || ${1:-} == "--help" ]]; then
  usage
  exit 0
fi

if [[ $# -ne 1 ]]; then
  usage >&2
  exit 1
fi

VERSION="$1"
TARBALL_URL="https://registry.npmjs.org/pnpm/-/pnpm-${VERSION}.tgz"

for tool in nix-prefetch-url perl node; do
  if ! command -v "$tool" >/dev/null 2>&1; then
    echo "set-pnpm-version: required tool '$tool' not found in PATH" >&2
    exit 1
  fi
done

if ROOT=$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null); then
  cd "$ROOT"
else
  SCRIPT_DIR=$(cd "$(dirname "$0")" && pwd -P)
  cd "$SCRIPT_DIR/.."
fi

echo "Fetching pnpm ${VERSION} tarball hash..." >&2
SHA=$(nix-prefetch-url "$TARBALL_URL" | tail -n1)

echo "Updating flake.nix..." >&2
perl -0pi -e "s/pnpmVersion = \"[^\"]+\";/pnpmVersion = \"${VERSION}\";/" flake.nix
perl -0pi -e "s#sha256 = \"[^\"]+\";#sha256 = \"${SHA}\";#" flake.nix

echo "Updating package.json..." >&2
VERSION_ENV="$VERSION" node <<'NODE'
const fs = require('fs');
const path = require('path');
const version = process.env.VERSION_ENV;
const pkgPath = path.join(process.cwd(), 'package.json');
const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
pkg.packageManager = `pnpm@${version}`;
fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
NODE

echo "Pinned pnpm ${VERSION} (sha256 ${SHA})." >&2
