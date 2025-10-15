# Nix Build and Test Report (2025-10-15)

## Overview
- `./nixos-build.sh`
- `./nixos-test.sh`

## Result
Both commands failed while attempting to fetch the custom `ratatui` Git repository (`https://github.com/nornagon/ratatui`, commit `9b2ad1298408c45918ee9f8241a6f95498cdbed2`). The requests reached GitHub, but the smart HTTP negotiation (`git-upload-pack`) uses an HTTP `POST`, which is blocked in this environment (only `GET`, `HEAD`, and `OPTIONS` are permitted). As a result, every Git fetch returns `403 Forbidden`, preventing the workspace from vendoring dependencies and causing the build/test derivations to abort before compiling project code.

## Suggested Follow-up
- Allow the Git smart HTTP endpoints (`POST` requests to `git-upload-pack`) so that cargo/Nix can fetch the dependency.
- Alternatively, vendor the required commit into this repository or switch to a source that can be downloaded with `GET`/`HEAD` only (e.g., a tarball mirrored on an allowed host).
