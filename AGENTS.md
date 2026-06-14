# AGENTS.md

## Overview
Rust binary extending the Niri compositor. Requires Rust toolchain.

## Build & Install
- Debug: `cargo build`, Release: `cargo build --release`
- Recommended install: `./install.sh` (builds release, installs to `~/.local/bin/piri` or `/usr/local/bin/piri`, copies config to `~/.config/niri/piri.toml`)
- Manual install: `cargo install --path .` (user) / `sudo cargo install --path . --root /usr/local` (system)

## Formatting & Linting
- Format: `cargo fmt` (config `rustfmt.toml`: 100 max width, 4 spaces)
- Check format: `cargo fmt -- --check`
- Lint: `trunk check` (runs clippy, rustfmt, shellcheck; config `.trunk/trunk.yaml`)
- Rust lint: `cargo clippy`

## Testing
No tests exist (no `#[test]` attributes in `src/`).

## Configuration
User config: `~/.config/niri/piri.toml` (copy from `config.example.toml`).

## Daemon
- Start: `piri daemon`
- Debug logs: `piri --debug daemon`

## Architecture
- Entry: `src/main.rs`, daemon: `src/daemon.rs`
- Plugins: `src/plugins/`, implement `Plugin` trait, register in `src/plugins/mod.rs`
- Key files: `src/config.rs` (config), `src/ipc.rs` (IPC), `src/niri.rs` (Niri IPC client)

## CI
Release-only workflow (`.github/workflows/release.yml`), no test/lint steps. Run lint/format locally before committing.

## References
- Development guide: `docs/en/development.md`
- Plugin docs: `docs/en/plugins/`
