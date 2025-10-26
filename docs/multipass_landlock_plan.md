# Plan to Diagnose and Restore Multipass with Landlock Support

## Context
- Running `./vm-test.sh` fails immediately because `multipass` is not on the `PATH`.
- `scripts/codex-environment-setup.sh` invokes `scripts/ensure_multipass.sh`, which attempts to install Multipass via Snap or APT on Linux.
- The automation hooks (Setup/Maintenance scripts) used for container provisioning already call `codex-environment-setup.sh`, so any failure occurs within that script or its children rather than from a missed invocation.
- The current Codex container does not have Multipass, indicating the setup script did not install it successfully (likely due to unavailable Snap/Multipass packages in the sandbox environment).

## Diagnosis Steps
1. **Confirm setup execution**  
   - Inspect workspace provisioning logs or rerun `scripts/codex-environment-setup.sh` with `set -x` to ensure `ensure_multipass.sh` ran and to capture any error messages that may have been suppressed.
   - Since the automation hooks already invoke `codex-environment-setup.sh`, concentrate on tracing the control flow inside `ensure_multipass.sh` to pinpoint the failure point (e.g., Snap vs. APT branch) and record any exit codes.

2. **Identify available package sources**  
   - Check if `snapd` is installed and functional (e.g., `snap version`). If Snap is unavailable, note the reason (common issues: service not running, cgroup limitations, or missing kernel features).
   - Check APT repositories for `multipass` availability (`apt-cache policy multipass`). Determine whether package installation fails because repositories are missing, outdated, or require additional keys.
   - If neither Snap nor APT can install Multipass, document the container limitations (e.g., running inside Docker without systemd) that prevent Snap-based installation.

3. **Verify virtualization support**  
   - Confirm whether the host permits nested virtualization required by Multipass/QEMU (`egrep -c '(vmx|svm)' /proc/cpuinfo`, `lsmod | grep kvm`).
   - If virtualization is unavailable, identify required host configuration changes.

4. **Landlock capability assessment**  
   - Once Multipass is installable, ensure the QEMU driver is active by running `multipass get local.driver` (expect `qemu`).
   - Boot a test instance and confirm the guest kernel supports Landlock (`grep LANDLOCK /boot/config-$(uname -r)` inside the VM or `uname -r` vs kernel >= 5.13).

## Remediation Plan
1. **Adjust installation script**  
   - Enhance `scripts/ensure_multipass.sh` to emit verbose diagnostics and fail loudly when installation paths are exhausted, so provisioning surfaces the root cause.
   - Add a fallback installer (e.g., download the official Multipass `.deb` release) when Snap and APT paths are unavailable.

2. **Provision host prerequisites**  
   - If Snap is required, update the workspace setup to enable `snapd` (start its daemon) and configure necessary cgroups/systemd integration.
   - Ensure nested virtualization modules (`kvm`, `kvm_intel`/`kvm_amd`) are loaded on the host; document steps for the infrastructure team if manual intervention is necessary.

3. **Automated verification**  
   - Extend `ensure_multipass.sh` to run `multipass info --all` or launch a lightweight VM to confirm the QEMU driver and kernel Landlock support after installation.
   - Update CI or maintenance scripts to periodically run `./vm-test.sh --smoke-check` (if available) to catch regressions in Multipass availability early.

4. **Documentation**  
   - Document the required host capabilities and troubleshooting steps in the repository (e.g., `docs/multipass.md`) so future environment setups can resolve Multipass/Landlock issues quickly.

Following this plan will surface why Multipass is currently missing, make the installation reliable within the Codex environment, and guarantee that virtual machines boot with Landlock-capable kernels before rerunning `./vm-test.sh`.
