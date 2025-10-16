{
  description = "Development Nix flake for OpenAI Codex CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { nixpkgs, flake-utils, rust-overlay, ... }: 
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };
        pnpmVersion = "10.8.1";
        pnpmPinned = pkgs.stdenvNoCC.mkDerivation {
          pname = "pnpm";
          version = pnpmVersion;
          src = pkgs.fetchurl {
            url = "https://registry.npmjs.org/pnpm/-/pnpm-${pnpmVersion}.tgz";
            sha256 = "1iya8w749nz94swvggfyd9mz245r82b5rd56xi4w60ngcnyfpcnq";
          };
          dontConfigure = true;
          dontBuild = true;
          unpackPhase = ''
            runHook preUnpack
            tar -xzf "$src"
            runHook postUnpack
          '';
          installPhase = ''
            runHook preInstall
            mkdir -p "$out/lib/node_modules" "$out/bin"
            cp -r package "$out/lib/node_modules/pnpm"
            ln -s "$out/lib/node_modules/pnpm/bin/pnpm.cjs" "$out/bin/pnpm"
            ln -s "$out/lib/node_modules/pnpm/bin/pnpx.cjs" "$out/bin/pnpx"
            runHook postInstall
          '';
          meta = with pkgs.lib; {
            description = "Fast, disk space efficient package manager";
            homepage = "https://pnpm.io/";
            license = licenses.mit;
            mainProgram = "pnpm";
          };
        };
        pkgsWithRust = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        monorepo-deps = with pkgs; [
          # for precommit hook
          pnpmPinned
          husky
          darcs
        ];
        codex-cli = import ./codex-cli {
          inherit pkgs monorepo-deps;
        };
        codex-rs = import ./codex-rs {
          pkgs = pkgsWithRust;
          inherit monorepo-deps;
        };
      in
      rec {
        packages = {
          codex-cli = codex-cli.package;
          codex-rs = codex-rs.package;
        };

        devShells = {
          codex-cli = codex-cli.devShell;
          codex-rs = codex-rs.devShell;
        };

        apps = {
          codex-cli = codex-cli.app;
          codex-rs = codex-rs.app;
        };

        defaultPackage = packages.codex-cli;
        defaultApp = apps.codex-cli;
        defaultDevShell = devShells.codex-cli;
      }
    );
}
