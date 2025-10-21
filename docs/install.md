## Install & build

### System requirements

| Requirement                 | Details                                                         |
| --------------------------- | --------------------------------------------------------------- |
| Operating systems           | macOS 12+, Ubuntu 20.04+/Debian 10+, or Windows 11 **via WSL2** |
| Git (optional, recommended) | 2.23+ for built-in PR helpers                                   |
| Darcs CLI (Darcs repos)     | Install via your package manager so Codex can diff `_darcs` workspaces |
| RAM                         | 4-GB minimum (8-GB recommended)                                 |

Codex shells out to the `darcs` executable whenever it detects a Darcs repository, so install the CLI before
launching Codex in those workspaces. A quick smoke test such as `darcs --version` verifies that the binary is
available on `PATH`; without it Codex will surface a warning and fall back to read-only metadata when you open a
Darcs checkout.

### DotSlash

The GitHub Release also contains a [DotSlash](https://dotslash-cli.com/) file for the Codex CLI named `codex`. Using a DotSlash file makes it possible to make a lightweight commit to source control to ensure all contributors use the same version of an executable, regardless of what platform they use for development.

### Build from source

```bash
# Clone the repository and navigate to the root of the Cargo workspace.
git clone https://github.com/openai/codex.git
cd codex/codex-rs

# Install the Rust toolchain, if necessary.
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup component add rustfmt
rustup component add clippy

# Build Codex.
cargo build

# Launch the TUI with a sample prompt.
cargo run --bin codex -- "explain this codebase to me"

# After making changes, ensure the code is clean.
cargo fmt -- --config imports_granularity=Item
cargo clippy --tests

# Run the tests.
cargo test
```

### Run the Codex tests inside a virtual machine

If your local environment cannot execute the full test suite (for example,
because of sandbox limitations), you can offload the work to a
clean Ubuntu VM using the helper script at the repository root:

```bash
./vm-test.sh
```

The script uses [Multipass](https://multipass.run/) to start an Ubuntu 22.04 VM,
copies the current checkout into the guest, installs Nix, and executes
`./nixos-test.sh` inside the VM. By default it deletes the VM after the run; pass
`--keep-vm` (or set `KEEP_VM=1`) if you want to inspect the machine afterwards.
You can customise the instance name, CPU, memory, disk, or Ubuntu release by
providing `--name`, `--cpus`, `--mem`, `--disk`, or `--image` flags respectively.
