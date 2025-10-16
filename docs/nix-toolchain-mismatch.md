# Rust toolchain mismatch during Nix builds

## Summary
- `nixos-build.sh` invokes `nix develop .#codex-rs` and builds the Rust workspace using the toolchain that ships with the `codex-rs` Nix flake.
- That flake wires `pkgs.rustPlatform.buildRustPackage` straight through from nixpkgs, so it inherits the default compiler (`rustc` 1.85.0) shipped by the pinned nixpkgs revision. 【F:codex-rs/default.nix†L8-L27】【924d74†L1-L1】
- Codex source relies heavily on `let`-chain syntax (for example `if cond && let Some(value) = opt`), which `rustc` 1.85.0 still treats as an unstable feature and rejects with error E0658.
- The repository’s `rust-toolchain.toml` pins Rust 1.90.0, a newer compiler where `let` chains are stabilized, so local development with `rustup` succeeds while the Nix build fails.

## Evidence
- `rust-toolchain.toml` requests channel `1.90.0`, indicating the minimum compiler Codex expects. 【F:codex-rs/rust-toolchain.toml†L1-L3】
- The build helpers run inside a `nix develop` shell, which spawns `/nix/store/.../rustc-1.85.0/bin/rustc` when compiling. 【c0973e†L1-L7】
- Codex code uses `let` chains, e.g. the `TMPDIR` handling in `protocol.rs`. 【F:codex-rs/protocol/src/protocol.rs†L355-L367】
- Compiling an equivalent snippet with `rustc` 1.85.0 reproduces the exact E0658 error that aborts the Nix build. 【49e142†L1-L23】

## Conclusion
The regression is not caused by new source edits alone—the source has been using `let` chains for some time—but by the fact that the Nix-based build pipeline still uses Rust 1.85.0. That compiler version rejects `let` chains, while the project expects Rust ≥ 1.90.0 (per `rust-toolchain.toml`).

## Why the flake still uses Rust 1.85.0
Although the top-level `flake.nix` brings in Oxalica’s overlay, the `codex-rs` subflake does not actually request a newer compiler from it. The package definition simply reuses `pkgs.rustPlatform.buildRustPackage`, and nixpkgs’ default `rustPlatform` is built on the channel that ships with the locked nixpkgs snapshot (currently Rust 1.85.0). 【F:flake.nix†L5-L36】【F:codex-rs/default.nix†L8-L27】 As a result, `nix develop` and `nix build` pick up 1.85.0 even though the repository expects ≥1.90.0, producing the `let`-chain E0658 failures when they compile the workspace.

To align the Nix pipeline with the repository toolchain, update `codex-rs/default.nix` to pull a Rust 1.90.x toolchain explicitly—e.g. via `pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml` or a fixed `pkgs.rust-bin.stable."1.90.0"` derivation—so that `buildRustPackage` and dev shells both use the stabilized compiler.

## Building Codex without Nix
Developers typically build Codex via `rustup` instead of Nix: the installation guide walks through cloning the repo, installing the pinned Rust toolchain, and running `cargo build`/`cargo test` directly. 【F:docs/install.md†L14-L40】 This path honors `rust-toolchain.toml`, so it already uses Rust 1.90.0 and does not hit the `let`-chain issue. The failing Nix pipeline is therefore the outlier rather than the normal development workflow.

**Next steps**
- Update the `codex-rs` flake to consume a Rust toolchain ≥ 1.90.0 so `nixos-build.sh` and `nixos-test.sh` match the repository’s pinned toolchain.
- Until that update lands, any `nix develop .#codex-rs --command cargo …` invocation will hit the same E0658 failure.
