# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Piri is a Rust binary extending the [Niri](https://github.com/YaLTeR/niri) Wayland compositor via IPC. It runs as a daemon that listens for Niri events and exposes CLI subcommands over a Unix socket. Plugins are the primary extension mechanism — each implements the `Plugin` trait and registers in `src/plugins/mod.rs`.

## Build & Run

```bash
# Build (debug)
cargo build

# Build (release)
cargo build --release

# Install via script (recommended, builds release + copies config)
./install.sh

# Install via cargo
cargo install --path .

# Run daemon (foreground)
piri daemon          # info level
piri --debug daemon  # debug level

# Stop daemon
piri stop
```

## Formatting & Linting

```bash
cargo fmt                        # Format (config: rustfmt.toml, 100 col, 4-space indent)
cargo fmt -- --check             # Check format
cargo clippy -- -D warnings      # Lint (warnings are errors)
trunk check                      # Full lint suite (clippy + rustfmt + shellcheck)
```

Pre-commit hooks run `cargo fmt --check` and `cargo clippy -- -D warnings` on Rust files, plus markdown link checking. Run `pre-commit install` to enable locally.

## Testing

No automated tests exist. CI is release-only (`.github/workflows/release.yml`). Verify changes by building and testing manually against a running Niri session.

## Configuration

User config lives at `~/.config/niri/piri.toml` (copy from `config.example.toml`). The daemon watches this file for changes and hot-reloads without restart. Config parsing is in `src/config.rs` — the `Config` struct maps directly to TOML sections.

## Architecture

### Entry Point & Daemon Loop

`src/main.rs` defines the CLI (clap) and dispatches to `daemon::run()`. The daemon (`src/daemon.rs`) runs a `tokio::select!` loop handling three streams: Unix signals (SIGTERM/SIGINT), Niri events (via `mpsc` channel), and IPC connections from the CLI client.

### IPC Protocol

Client-server communication uses Unix domain sockets (`$XDG_RUNTIME_DIR/piri.sock`). Messages are length-prefixed JSON (`IpcRequest`/`IpcResponse` enums in `src/ipc.rs`). The client (`IpcClient`) connects, sends a request, and reads the response with 5-second timeouts.

### Niri Interaction

`src/niri.rs` wraps `niri-ipc` (the `niri_ipc::Socket` crate). `NiriIpc` is `Clone`-able (internally `Arc`-wrapped), manages socket connections with automatic reconnect, and caches output dimensions. All niri IPC calls go through `send_request()` which uses `spawn_blocking` for the synchronous socket operations.

### Plugin System (`src/plugins/`)

Each plugin implements the `Plugin` trait:
- `new()` — constructor receiving `NiriIpc` and plugin-specific config
- `handle_event()` — processes Niri events (window opened, workspace changed, etc.)
- `handle_ipc_request()` — responds to CLI commands
- `is_interested_in_event()` — event filter for the unified dispatcher
- `update_config()` — hot-reload support preserving runtime state

The `register_plugins!` macro generates `PluginEnum` (enum dispatch) and `PluginManager::init()` (creates/updates/disables plugins based on config). Plugins are registered by name string in `src/plugins/mod.rs:141-151`.

The `FromConfig` trait extracts plugin config from the global `Config`. Returning `None` disables the plugin.

### Event Distribution

`PluginManager` runs a single Niri event listener loop (`start_event_listener`), receives events via `mpsc::UnboundedSender`, and distributes them in `distribute_event()`. Only plugins whose `is_interested_in_event()` returns `true` receive each event — avoid re-checking event types in `handle_event()`.

### Key Modules

- `src/commands.rs` — `CommandHandler` bridges CLI, config, NiriIpc, and PluginManager
- `src/config.rs` — all config structs, TOML serde, hot-reload logic
- `src/niri.rs` — `NiriIpc` async wrapper around niri-ipc socket
- `src/ipc.rs` — `IpcServer`, `IpcClient`, request/response enums, `handle_request()`
- `src/plugins/window_utils.rs` — geometry calculations and regex-based window matching (cached via `WindowMatcherCache`)

### Adding a New Plugin

1. Create `src/plugins/myplugin.rs`, implement `Plugin` trait
2. Register in `register_plugins!` macro in `src/plugins/mod.rs`
3. Add enable flag in `PluginsConfig` (`src/config.rs`) and wire `is_enabled()` match arm
4. Add config structs and `FromConfig` impl
5. If it handles IPC commands: add variants to `IpcRequest`/`IpcResponse` in `src/ipc.rs` and wire in `handle_request()`
6. If it needs CLI: add subcommand in `src/main.rs`
7. Update `config.example.toml`
