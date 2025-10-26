# Plan to Diagnose and Restore Multipass with Landlock Support

## Updated context

The immediate symptom is that `./vm-test.sh` exits before running any checks because the `multipass` CLI is missing from `PATH`. The Codex setup pipeline is intended to install Multipass automatically through `scripts/codex-environment-setup.sh`, which in turn delegates to `scripts/ensure_multipass.sh`. Because the automation hooks already execute `codex-environment-setup.sh` during provisioning, the absence of `multipass` implies that `ensure_multipass.sh` exited early or silently failed. The current tasks therefore focus on improving observability, understanding installer constraints inside the sandboxed container, and validating the prerequisites for Landlock once Multipass becomes available.

## Phase 1 – Establish the current environment

1. **Baseline discovery**
   - Run `command -v multipass` and `multipass version` (guarded with `if command -v multipass >/dev/null`) to confirm the binary is truly absent or determine whether it was installed in a non-standard location.
   - Inspect `scripts/ensure_multipass.sh` for gating environment variables such as `INSTALL_MULTIPASS_SKIP` or `INSTALL_MULTIPASS_FORCE` and document their defaults so subsequent reruns of the setup script can be controlled deterministically.

2. **Capture provisioning metadata**
   - Collect the timestamped setup logs from `/tmp/codex-setup*.log` or the automation hook output to understand which setup iteration removed or skipped Multipass.
   - Record the UID/GID of the automation user, available sudo privileges, and the effective shell so installer scripts can be reproduced faithfully in manual debugging sessions.

## Phase 2 – Reproduce and capture failure details

1. **Rerun the installer with tracing**
   - Execute `env INSTALL_MULTIPASS_VERBOSE=1 bash -x scripts/codex-environment-setup.sh` to capture the full control flow and ensure the Multipass branch is executed.
   - Confirm whether `ensure_multipass.sh` aborts, returns success despite not installing anything, or skips installation based on gating logic (for example, short-circuiting when running without root privileges).

2. **Collect existing provisioning logs**
   - Inspect `/tmp/codex-setup*.log` (if the setup script already writes logs) or container provisioning output to understand earlier runs.
   - Note any repeated transient failures (network timeouts, repository errors) that may necessitate retries or mirrors.

## Phase 3 – Determine viable installation sources

1. **Evaluate Snap path**
   - Check whether `snapd` is installed and running (`snap version`, `systemctl status snapd`, `journalctl -u snapd`).
   - Document sandbox limitations such as lack of `systemd`, missing cgroup controllers, or read-only filesystems that prevent Snap from working, and capture the exact error messages.

2. **Evaluate APT path**
   - Inspect `apt-cache policy multipass` and `apt-cache show multipass` to confirm package availability and required repository components.
   - Verify that `apt-get update` succeeds and that Multipass dependencies (notably `qemu`, `libvirt-bin`, and kernel modules) can be installed without conflicting with the base image.
   - Capture the output of `apt-get install --dry-run multipass` to enumerate the exact dependency chain and identify pre/post-install scripts that might fail under sandboxed conditions.

3. **Fallback installers**
   - Identify official `.deb` artifacts from Canonical (see https://multipass.run/download/linux) and confirm whether they can be downloaded with `curl`/`wget` inside the sandbox.
   - Validate checksums against the published SHA256 values and store them alongside the downloaded artifacts so subsequent CI runs can reuse the same binary without re-downloading.
   - If direct installation fails because the package expects system services that are unavailable, capture the failing maintainer scripts and evaluate whether they can be stubbed or skipped.

## Phase 4 – Validate virtualization prerequisites

1. **Nested virtualization**
   - Inspect `/proc/cpuinfo` for `vmx` (Intel) or `svm` (AMD) flags and ensure `lsmod | grep kvm` shows both the core `kvm` module and the architecture-specific module.
   - Attempt to load modules manually (`sudo modprobe kvm kvm_intel`) and document any permission or kernel configuration errors.

2. **Device availability**
   - Ensure `/dev/kvm` exists and has the correct ownership/permissions. If the device is missing, note whether the host kernel simply lacks KVM support or if container runtime settings hide the device.
   - Record whether AppArmor, SELinux, or other MAC systems interfere with QEMU when executed from within the container.

## Phase 5 – Landlock capability assessment

1. **Confirm Multipass driver selection**
   - After successful installation, run `multipass get local.driver` to confirm the `qemu` driver is active. If the driver reports `lxd` or `none`, determine why Multipass fell back and whether QEMU support can be forced via configuration.

2. **Boot and inspect a micro VM**
   - Launch a minimal instance (`multipass launch --name landlock-check --cpus 1 --mem 512M --disk 5G --timeout 600 jammy`).
   - Once the instance is up, enter the guest with `multipass shell landlock-check` and run `grep LANDLOCK /boot/config-$(uname -r)` or, if `/boot/config-*` is unavailable, inspect `/proc/config.gz` to ensure the kernel enables `CONFIG_SECURITY_LANDLOCK=y` or `=m`.
   - Validate the running kernel version is ≥ 5.13 and supports Landlock by running a simple sample program (for example, build Canonical’s sample from https://github.com/landlock-lsm/landlock-samples) if compilation toolchains are available.

3. **Tear down and clean state**
   - Remove the test instance (`multipass delete landlock-check && multipass purge`) to ensure subsequent provisioning runs start from a clean slate.

## Phase 6 – Remediation and hardening

1. **Improve automation scripts**
   - Update `scripts/ensure_multipass.sh` to:
     - Fail fast if all installer paths are exhausted, surfacing actionable error messages.
     - Emit a structured log (JSON or tagged shell output) that can be persisted by the Setup/Maintenance hooks.
     - Provide explicit exit codes distinguishing between “installation skipped by design” and “installation failed.”
   - Extend `scripts/codex-environment-setup.sh` to write its summary (success/failure, installer path taken, Multipass version detected) to `/var/log/codex/multipass-setup.json` so the automation hooks can reuse the same diagnostics across container restarts.

2. **Introduce resilient installers**
   - Add support for installing Multipass from official release archives when Snap/APT are unavailable, including checksum validation.
   - Consider building a lightweight container image that bundles Multipass binaries and required kernel modules if the sandbox cannot host Snapd.

3. **Embed health checks**
   - Extend the setup script to run `multipass info --all` and a short `multipass launch --timeout 120 --name sanity-check --cloud-init <(printf 'runcmd:\n - uname -r')` invocation, verifying that VM boot succeeds and Landlock prerequisites are met.
   - Integrate these checks into CI (for example, extend `./vm-test.sh` with a `--smoke-check` mode) to catch regressions early.
   - Gate expensive checks behind an environment toggle (for example, `INSTALL_MULTIPASS_SMOKE_TEST=1`) so developers can opt in locally without slowing every CI job.

4. **Documentation and escalation paths**
   - Update repository documentation (`docs/multipass.md` or a new troubleshooting guide) with the exact steps required to enable Multipass in sandboxed environments, including host-level configuration (nested virtualization, cgroup v2, AppArmor allowances).
   - Record clear escalation paths when prerequisites cannot be satisfied (e.g., escalate to infrastructure to enable `/dev/kvm`, provide instructions for enabling virtualization in the hypervisor hosting the runners).

## Expected outcomes

Executing this plan should reveal why Multipass is currently absent, allow the automation scripts to recover by installing from a viable source, and verify that launched instances provide Landlock-capable kernels. Once these steps succeed, `./vm-test.sh` can be rerun confidently to validate higher-level VM workflows.
