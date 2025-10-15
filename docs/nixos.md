# Building Codex on NixOS

This guide explains how to build, test, and manually run Codex on an x86-64 machine running **NixOS 25.11**. The steps rely on the Nix flake that ships with Codex, so they also work for upstream clones that do not contain this documentation.

## Prerequisites

- A machine running NixOS 25.11 (or another NixOS release with a recent `nix` command, version 2.19 or newer).
- Enable Flakes and the new command-line interface in your global `/etc/nix/nix.conf` (or the user configuration in `~/.config/nix/nix.conf`) if they are not already enabled:

  ```ini
  experimental-features = nix-command flakes
  ```

- Make sure you have enough disk space (building Codex downloads Rust toolchains, Node.js runtimes, and pnpm stores) and at least 8 GB of RAM for comfortable builds.

## Cloning the repository

```bash
git clone https://github.com/openai/codex.git
cd codex
```

The remainder of this document assumes commands are run from the repository root.

## Using the provided helper scripts

This repository now includes two helper scripts tailored for NixOS users:

- `./nixos-build.sh` builds the Rust workspace, the CLI npm package, and the TypeScript SDK.
- `./nixos-test.sh` runs the full automated test suite (Rust + TypeScript).

Both scripts work with the upstream Codex repository because they only call into existing Nix flake targets and package.json scripts. Run them directly from the repository root:

```bash
./nixos-build.sh
./nixos-test.sh
```

The scripts automatically enter the appropriate Nix development shells, so you do not need to install Rust, Node.js, or pnpm globally.

## Manual build and test workflow

If you prefer to run the commands yourself (or need to debug a failure), the following sections break down what the helper scripts do.

### Prepare the Rust toolchain

Enter the Rust development shell defined by the flake and build the workspace:

```bash
nix develop .#codex-rs --command cargo build --workspace --locked
```

To run the Rust tests with all available features:

```bash
nix develop .#codex-rs --command cargo test --workspace --all-features
```

### Build the CLI npm package

The CLI bundles prebuilt binaries and scripts. Use the Node.js/pnpm development shell to install dependencies:

```bash
nix develop .#codex-cli --command bash -c 'cd codex-cli && pnpm install --frozen-lockfile'
```

The CLI is primarily a thin wrapper around the Rust binary, so there is no separate `pnpm run build` step.

### Build and test the TypeScript SDK

From the same development shell, install dependencies and run the SDK build:

```bash
nix develop .#codex-cli --command bash -c 'cd sdk/typescript && pnpm install --frozen-lockfile && pnpm run build'
```

Run the TypeScript unit tests and lint checks:

```bash
nix develop .#codex-cli --command bash -c 'cd sdk/typescript && pnpm test'
```

### Optional: Repo-wide formatting checks

To keep the documentation and JSON files formatted consistently:

```bash
nix develop .#codex-cli --command pnpm run format
```

(Use `pnpm run format:fix` to automatically apply fixes.)

## Manual CLI testing

Once the Rust workspace has been built, you can launch the CLI directly from the development shell:

```bash
nix develop .#codex-rs --command cargo run --bin codex
```

This starts the interactive TUI. When running Codex inside Nix shells you can still authenticate with ChatGPT accounts or API keys as described in [Authentication](./authentication.md).

## Troubleshooting

- **`nix develop` complains about missing flakes** – Verify the `experimental-features` setting includes both `nix-command` and `flakes`.
- **`cargo` runs out of memory** – Build with fewer parallel jobs, e.g. `nix develop .#codex-rs --command cargo build -j4`.
- **`pnpm` cannot create `node_modules`** – Ensure you are operating in a writable working tree (Nix shells default to writable, but read-only filesystem mounts will fail).
- **Tests skip sandboxed integrations** – Some tests detect sandbox environments and exit early. This is expected; re-run without sandboxing if you need those integration checks.

For more background on installing Codex on other platforms, see [Install & build](./install.md).
