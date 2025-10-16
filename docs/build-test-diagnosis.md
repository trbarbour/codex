# nixos-build.sh and nixos-test.sh failure analysis

## pnpm installation failure during `nixos-build.sh`
- The Nix dev shell currently provides pnpm 10.8.0, as shown by `which pnpm` (`/nix/store/.../pnpm-10.8.0/bin/pnpm`).
- The monorepo pins pnpm 10.8.1 via the `packageManager` field in `package.json`, so pnpm attempts to self-upgrade by repeatedly executing `pnpm add pnpm@10.8.1`.
- Each self-upgrade attempt recurses and spawns additional Node processes, leading to runaway process trees and eventual `ERANGE: result too large, uv_cwd` errors observed in the build script log.

**Planned fix**
- Update the Nix flake (`flake.nix`) to depend on pnpm 10.8.1 (or newer) so the dev shell matches the repository's pinned package manager version.
- After bumping pnpm, re-run `./nixos-build.sh` to confirm that `pnpm install --frozen-lockfile` succeeds without triggering self-upgrade loops.
- Consider adding a CI assertion that `pnpm -v` matches the pinned version to catch future drift.

## Landlock sandbox errors during `nixos-test.sh`
- The failing tests (`python_multiprocessing_lock_works_under_sandbox` and `sandbox_distinguishes_command_and_policy_cwds`) rely on Linux Landlock enforcement.
- The container kernel lacks Landlock support (`/sys/kernel/security/landlock/features` is absent and `CONFIG_SECURITY_LANDLOCK` is disabled in `/proc/config.gz`), so applying the ruleset returns `RulesetStatus::NotEnforced` and `codex-linux-sandbox` emits `SandboxErr::LandlockRestrict`.

**Planned fix**
- Teach the Linux sandbox helper (and the affected tests) to detect when Landlock support is unavailable and skip or downgrade gracefully instead of treating it as a hard failure.
- Add documentation noting the kernel requirements (Landlock enabled) for running the sandbox test suite locally.
- Optionally extend CI to run Landlock-dependent tests only on builders with the feature enabled, while skipping them elsewhere.
