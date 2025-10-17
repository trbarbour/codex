# nixos-build.sh and nixos-test.sh failure analysis

## pnpm installation failure during `nixos-build.sh`
- The Nix dev shell currently provides pnpm 10.8.0, as shown by `which pnpm` (`/nix/store/.../pnpm-10.8.0/bin/pnpm`).
- The monorepo pins pnpm 10.8.1 via the `packageManager` field in `package.json`, so pnpm attempts to self-upgrade by repeatedly executing `pnpm add pnpm@10.8.1`.
- Each self-upgrade attempt recurses and spawns additional Node processes, leading to runaway process trees and eventual `ERANGE: result too large, uv_cwd` errors observed in the build script log.

**Fix**
- `flake.nix` now ships a custom `pnpm` derivation whose version matches the repository pin (currently 10.8.1). Invoking `nix develop` therefore exposes the correct CLI without relying on self-upgrades.
- `nixos-build.sh` performs an early guard via `scripts/check-pnpm-version.sh`, ensuring the in-shell pnpm binary matches the version declared in `package.json` before any install or build steps run. The script also validates the Rust toolchain with `scripts/check-rust-version.sh`, catching mismatched `rustc`/`cargo` builds before the workspace build starts.
- Added helper scripts for refreshing pins: `./scripts/set-pnpm-version.sh <pnpm-version>` rewrites the pnpm derivation and package metadata, while `./scripts/set-rust-version.sh <rust-version>` updates `codex-rs/rust-toolchain.toml` to keep Nix shells and `rustup`-managed toolchains aligned.
- If either check fails, rerun the corresponding setter script with the desired version and re-run `./nixos-build.sh`.

## Landlock sandbox errors during `nixos-test.sh`
- The failing tests (`python_multiprocessing_lock_works_under_sandbox` and `sandbox_distinguishes_command_and_policy_cwds`) rely on Linux Landlock enforcement.
- The container kernel lacks Landlock support (`/sys/kernel/security/landlock/features` is absent and `CONFIG_SECURITY_LANDLOCK` is disabled in `/proc/config.gz`), so applying the ruleset returns `RulesetStatus::NotEnforced` and `codex-linux-sandbox` emits `SandboxErr::LandlockRestrict`.

**Next steps**
- Teach the Linux sandbox helper (and the affected tests) to detect when Landlock support is unavailable and skip or downgrade gracefully instead of treating it as a hard failure.
- Add documentation noting the kernel requirements (Landlock enabled) for running the sandbox test suite locally.
- Optionally extend CI to run Landlock-dependent tests only on builders with the feature enabled, while skipping them elsewhere.
