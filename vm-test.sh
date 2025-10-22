#!/usr/bin/env bash

set -euo pipefail

usage() {
  cat <<'EOF'
vm-test.sh - Run the Codex test suite inside an Ubuntu Multipass VM

Usage:
  ./vm-test.sh [options]

Options:
  --name NAME         Name for the Multipass instance (default: codex-test-vm)
  --image RELEASE     Ubuntu release to launch (default: 22.04)
  --cpus COUNT        Number of virtual CPUs (default: 4)
  --memory SIZE       Amount of RAM to allocate (default: 8G)
  --disk SIZE         Disk size for the VM (default: 40G)
  --keep-vm           Leave the VM running after the script exits
  -h, --help          Show this help message

Environment variables:
  VM_NAME, VM_IMAGE, VM_CPUS, VM_MEM, VM_DISK mirror the matching flags.
  KEEP_VM=1 behaves like --keep-vm.

Prerequisites:
  - Multipass must be installed on the host.
  - The host needs outbound network access so the VM can install Nix.

The script copies the current repository into the VM, installs Nix (if
necessary), and executes ./nixos-test.sh inside the virtual machine.
EOF
}

ROOT=$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null)
if [[ -z "${ROOT:-}" ]]; then
  echo "vm-test.sh: failed to locate repository root" >&2
  exit 1
fi

cd "$ROOT"

VM_NAME=${VM_NAME:-codex-test-vm}
VM_IMAGE=${VM_IMAGE:-22.04}
VM_CPUS=${VM_CPUS:-4}
VM_MEM=${VM_MEM:-8G}
VM_DISK=${VM_DISK:-40G}
KEEP_VM_FLAG=${KEEP_VM:-0}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --name)
      VM_NAME="$2"
      shift 2
      ;;
    --image)
      VM_IMAGE="$2"
      shift 2
      ;;
    --cpus)
      VM_CPUS="$2"
      shift 2
      ;;
    --memory)
      VM_MEM="$2"
      shift 2
      ;;
    --mem)
      echo "vm-test.sh: '--mem' is deprecated, use '--memory' instead" >&2
      VM_MEM="$2"
      shift 2
      ;;
    --disk)
      VM_DISK="$2"
      shift 2
      ;;
    --keep-vm)
      KEEP_VM_FLAG=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "vm-test.sh: unknown option '$1'" >&2
      echo >&2
      usage >&2
      exit 1
      ;;
  esac
done

if ! command -v multipass >/dev/null 2>&1; then
  echo "vm-test.sh: Multipass is not available on PATH."
  echo "==> Running nixos-test.sh directly on the host instead"
  ./nixos-test.sh
  exit $?
fi

DELETE_ON_EXIT=0

if multipass info "$VM_NAME" >/dev/null 2>&1; then
  echo "==> Reusing existing Multipass instance '$VM_NAME'"
else
  echo "==> Launching Multipass instance '$VM_NAME'"
  multipass launch "${VM_IMAGE}" --name "$VM_NAME" --cpus "$VM_CPUS" --memory "$VM_MEM" --disk "$VM_DISK"
  DELETE_ON_EXIT=1
fi

if [[ "$KEEP_VM_FLAG" == "1" ]]; then
  DELETE_ON_EXIT=0
fi

TMP_ARCHIVE=$(mktemp -t codex-vm-src.XXXXXX.tar.gz)

cleanup() {
  rm -f "$TMP_ARCHIVE"
  if [[ "$DELETE_ON_EXIT" == "1" ]]; then
    echo "==> Deleting Multipass instance '$VM_NAME'"
    multipass delete "$VM_NAME" >/dev/null 2>&1 || true
    multipass purge >/dev/null 2>&1 || true
  else
    echo "==> Leaving Multipass instance '$VM_NAME' intact"
  fi
}

trap cleanup EXIT

echo "==> Creating source archive"
tar \
  --exclude='.git' \
  --exclude='target' \
  --exclude='node_modules' \
  --exclude='sdk/typescript/node_modules' \
  --exclude='sdk/typescript/dist' \
  --exclude='pnpm-store' \
  -czf "$TMP_ARCHIVE" \
  -C "$ROOT" .

REMOTE_ARCHIVE=/home/ubuntu/codex-src.tar.gz

echo "==> Transferring repository to VM"
multipass transfer "$TMP_ARCHIVE" "$VM_NAME":"$REMOTE_ARCHIVE"

echo "==> Preparing workspace inside VM"
multipass exec "$VM_NAME" -- bash -lc "\
  set -euo pipefail; \
  cd /home/ubuntu; \
  rm -rf codex; \
  mkdir -p codex; \
  tar -xzf codex-src.tar.gz -C codex; \
  rm -f codex-src.tar.gz; \
  chmod +x codex/nixos-test.sh codex/nixos-build.sh >/dev/null 2>&1 || true; \
  true"

echo "==> Installing prerequisites and running nixos-test.sh"
if ! multipass exec "$VM_NAME" -- bash -lc "command -v nix >/dev/null 2>&1"; then
  multipass exec "$VM_NAME" -- bash -lc "\
    set -euo pipefail; \
    sudo apt-get update; \
    sudo apt-get install -y curl git xz-utils; \
    sh <(curl -L https://nixos.org/nix/install) --no-daemon --yes; \
    source \"\$HOME/.nix-profile/etc/profile.d/nix.sh\"; \
    nix --version"
else
  multipass exec "$VM_NAME" -- bash -lc "source \"\$HOME/.nix-profile/etc/profile.d/nix.sh\"; nix --version"
fi

TEST_STATUS=0
multipass exec "$VM_NAME" -- bash -lc "\
  set -euo pipefail; \
  source \"\$HOME/.nix-profile/etc/profile.d/nix.sh\"; \
  cd \"\$HOME/codex\"; \
  ./nixos-test.sh" || TEST_STATUS=$?
if [[ "$TEST_STATUS" -ne 0 ]]; then
  echo "==> nixos-test.sh failed inside the VM" >&2
  if [[ "$DELETE_ON_EXIT" == "1" && "$KEEP_VM_FLAG" != "1" ]]; then
    echo "==> Keeping the VM running for debugging. Run 'multipass delete $VM_NAME' and 'multipass purge' when finished." >&2
    DELETE_ON_EXIT=0
  else
    echo "==> Rerun with --keep-vm to inspect the environment." >&2
  fi
  exit "$TEST_STATUS"
fi

echo "==> Codex tests finished successfully"
