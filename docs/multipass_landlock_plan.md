# Plan to Diagnose and Restore Multipass with Landlock Support

## Updated context

The immediate symptom is that `./vm-test.sh` exits before running any checks because the `multipass` CLI is missing from `PATH`. The Codex setup pipeline is intended to install Multipass automatically through `scripts/codex-environment-setup.sh`, which in turn delegates to `scripts/ensure_multipass.sh`. Because the automation hooks already execute `codex-environment-setup.sh` during provisioning, the absence of `multipass` implies that `ensure_multipass.sh` exited early or silently failed. The current tasks therefore focus on improving observability, understanding installer constraints inside the sandboxed container, and validating the prerequisites for Landlock once Multipass becomes available.

## Immediate next steps (prioritized)

1. **Re-run baseline discovery checks (Phase 1.1)** to confirm that `multipass` remains absent before instrumenting anything else. This ensures new findings can be attributed to fresh actions rather than stale state.
2. **Implement persistent logging instrumentation (Phase 2.1)** by updating `scripts/ensure_multipass.sh` (and the setup wrapper, if necessary) to emit structured logs into a predictable directory before any installer probes execute.
3. **Repeat the traced installer run with instrumentation enabled (Phase 2.2)** using `INSTALL_MULTIPASS_VERBOSE=1 bash -x ...`, verifying that the log captures the exact point of failure and that the script still exits non-zero on hangs/timeouts.
4. **Collect and archive the generated log artifacts (Phase 2.3)** so subsequent debugging can proceed without rerunning the installer, and summarize the captured evidence in this document.

## Phase 1 – Establish the current environment

1. **Baseline discovery**
   - Run `command -v multipass` and `multipass version` (guarded with `if command -v multipass >/dev/null`) to confirm the binary is truly absent or determine whether it was installed in a non-standard location.
   - Inspect `scripts/ensure_multipass.sh` for gating environment variables such as `INSTALL_MULTIPASS_SKIP` or `INSTALL_MULTIPASS_FORCE` and document their defaults so subsequent reruns of the setup script can be controlled deterministically.
   - **Findings (2025-10-26 21:29:18Z)**
     - `multipass` is not present on `PATH` (`command -v multipass` returned no result), so `multipass version` was not runnable.
     - `scripts/ensure_multipass.sh` does not define or consult any gating environment variables; installation attempts are driven solely by runtime detection of `snap`, `apt-get`, and `brew` availability.

2. **Capture provisioning metadata**
   - Collect the timestamped setup logs from `/tmp/codex-setup*.log` or the automation hook output to understand which setup iteration removed or skipped Multipass.
   - Record the UID/GID of the automation user, available sudo privileges, and the effective shell so installer scripts can be reproduced faithfully in manual debugging sessions.
   - **Findings (2025-10-26 21:38:41Z)**
     - No files matching `/tmp/codex-setup*.log` are present on the container, so prior setup runs either did not emit logs or they have already been removed.
     - The automation context runs as `root` (`uid=0`, `gid=0`) with implicit full sudo privileges and uses `/bin/bash` as the default shell, matching the expectations of the setup scripts.

## Phase 2 – Reproduce and capture failure details

1. **Instrument the installer for persistent logs**
   - Patch `scripts/ensure_multipass.sh` so that it always initializes a writable log directory (for example, `${TMPDIR:-/var/tmp}/codex-multipass-logs`) and redirects both stdout and stderr through `tee -a "$log_file"`. Ensure the directory creation and redirection happen before any installer probes (including the hanging `snap list multipass` call) so the log captures the exact failure point even when the script is invoked from automation hooks.
   - Introduce a lightweight helper (for example, `scripts/support/write-multipass-log.sh`) that formats log file names with UTC timestamps, hostnames, and PID values. Reuse it from both `codex-environment-setup.sh` and `ensure_multipass.sh` so every entry point shares the same logging contract, and document the final path in the setup README.
   - Confirm via a manual dry run that the instrumentation still exits with non-zero status codes when the installer fails, avoiding accidental masking of errors due to pipeline redirection.
   - **Findings (2025-10-27 01:00:28Z)**
     - Current scripts do not initialize any persistent log path, so the automation hook discards diagnostic output once the session ends. Instrumentation must therefore run at the very beginning of each script to guarantee the logs survive hook execution.

2. **Rerun the installer with tracing**
   - Execute `env INSTALL_MULTIPASS_VERBOSE=1 bash -x scripts/codex-environment-setup.sh` to capture the full control flow and ensure the Multipass branch is executed.
   - Confirm whether `ensure_multipass.sh` aborts, returns success despite not installing anything, or skips installation based on gating logic (for example, short-circuiting when running without root privileges).
   - **Findings (2025-10-26 23:41:56Z)**
     - The traced run blocks indefinitely inside `snap list multipass`; the command never prints output and required a manual interrupt. `bash -x` shows `ensure_multipass.sh` invoking `snap list multipass` before timing out, so the script never reaches the APT fallback path.
     - Running `timeout 10s env INSTALL_MULTIPASS_VERBOSE=1 bash -x scripts/ensure_multipass.sh` confirms the hang originates from the `snap list multipass` probe, which exits only when the timeout (exit code 124) kills the process, implying the `snapd` socket is unavailable in this container.

3. **Collect existing provisioning logs**
   - Inspect `/tmp/codex-setup*.log` (if the setup script already writes logs) or container provisioning output to understand earlier runs.
   - Note any repeated transient failures (network timeouts, repository errors) that may necessitate retries or mirrors.
   - **Findings (2025-10-26 23:41:56Z)**
     - No new `/tmp/codex-setup*.log` files were generated by the traced installer run, matching the absence observed during Phase 1.

## Phase 3 – Determine viable installation sources

1. **Evaluate Snap path**
   - Check whether `snapd` is installed and running (`snap version`, `systemctl status snapd`, `journalctl -u snapd`).
   - Document sandbox limitations such as lack of `systemd`, missing cgroup controllers, or read-only filesystems that prevent Snap from working, and capture the exact error messages.
   - **Findings (2025-10-27 02:40:50Z)**
     - `snap version` blocks indefinitely waiting on the snapd socket; the command produced no output until it was interrupted manually with `Ctrl+C`.
     - `systemctl status snapd` fails because the container is not booted with systemd (`System has not been booted with systemd as init system (PID 1). Can't operate.`).
     - `journalctl -u snapd` reports `No journal files were found. -- No entries --`, confirming that no snapd logs are available in this environment.

2. **Evaluate APT path**
   - Inspect `apt-cache policy multipass` and `apt-cache show multipass` to confirm package availability and required repository components.
   - Verify that `apt-get update` succeeds and that Multipass dependencies (notably `qemu`, `libvirt-bin`, and kernel modules) can be installed without conflicting with the base image.
   - Capture the output of `apt-get install --dry-run multipass` to enumerate the exact dependency chain and identify pre/post-install scripts that might fail under sandboxed conditions.
   - **Findings (2025-10-27 04:07:12Z)**
     - `apt-get update` completed for the Ubuntu archives but emitted a warning because the third-party `https://mise.jdx.dev` repository returned HTTP 403 responses; the failure was ignored and cached indices were reused.
     - `apt-cache policy multipass`, `apt-cache show multipass`, and `apt-get install --dry-run multipass` all report “Unable to locate package multipass,” indicating that the package is absent from the default Noble repositories.
     - `apt-cache search multipass` only lists the unrelated `ruby-omniauth-multipassword` package, confirming that no Multipass binary is currently published via APT for this release.

3. **Fallback installers**
   - Identify official `.deb` artifacts from Canonical (see https://multipass.run/download/linux) and confirm whether they can be downloaded with `curl`/`wget` inside the sandbox.
   - Validate checksums against the published SHA256 values and store them alongside the downloaded artifacts so subsequent CI runs can reuse the same binary without re-downloading.
   - If direct installation fails because the package expects system services that are unavailable, capture the failing maintainer scripts and evaluate whether they can be stubbed or skipped.
   - **Findings (2025-10-27 04:31:36Z)**
     - Requests to `https://multipass.run` follow an HTTP 301 redirect to `https://canonical.com/multipass`, where the proxy at `proxy:8080` responds with `HTTP/1.1 403 Forbidden`; the MITM proxy therefore blocks fetching Canonical-hosted installers directly from the container.
     - Direct access to Canonical APIs (for example, `https://api.snapcraft.io/api/v1/snaps/details/multipass`) fails with the same proxy-level 403, so downloading Snap metadata or `.deb` assets from Canonical mirrors is currently impossible.
     - GitHub remains reachable: `https://api.github.com/repos/canonical/multipass/releases/latest` lists only macOS and Windows artifacts for v1.16.1, and downloading `multipass-1.16.1+mac-Darwin.pkg` (SHA256 `758d10dc1b71872b0ee7a17070b93fc788dba5ba45c36b980e42fd895d273489`) succeeds, confirming an alternate trusted source even though no Linux `.deb` is published alongside that release.

## Phase 4 – Validate virtualization prerequisites

1. **Nested virtualization**
   - Inspect `/proc/cpuinfo` for `vmx` (Intel) or `svm` (AMD) flags and ensure `lsmod | grep kvm` shows both the core `kvm` module and the architecture-specific module.
   - Attempt to load modules manually (`sudo modprobe kvm kvm_intel`) and document any permission or kernel configuration errors.
   - **Findings (2025-10-29 18:49:55Z)**
     - `grep -m1 'flags' /proc/cpuinfo` lists numerous CPU features but no `vmx` or `svm` entries, indicating hardware virtualization extensions are not exposed inside this container.
     - `lsmod` and `modprobe` are unavailable on the PATH (`command -v lsmod`/`command -v modprobe` return nothing), and attempting `sudo modprobe kvm` fails with `sudo: modprobe: command not found`, so kernel module status cannot be verified from within the current environment.

2. **Device availability**
   - Ensure `/dev/kvm` exists and has the correct ownership/permissions. If the device is missing, note whether the host kernel simply lacks KVM support or if container runtime settings hide the device.
   - Record whether AppArmor, SELinux, or other MAC systems interfere with QEMU when executed from within the container.
   - **Findings (2025-10-29 19:26:18Z)**
     - `/dev/kvm` is absent (`ls -l /dev/kvm` reports “No such file or directory”), matching the missing virtualization flags observed earlier.
     - `aa-status` reports “apparmor not present.”, and SELinux tooling (`sestatus`, `getenforce`) is unavailable, so no mandatory access control layer is actively enforcing policies inside the container.
     - No loaded kernel modules matching `kvm` are visible under `/sys/module`, reinforcing that hardware virtualization support is not exposed by the runtime.

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
