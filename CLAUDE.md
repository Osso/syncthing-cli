# syncthing CLI

Rust CLI for Syncthing REST API. Auto-discovers API key from `~/.config/syncthing/config.xml`.

## Build

```bash
cargo build --release
```

## Commands

```bash
syncthing status          # System status, uptime, memory, sync progress
syncthing folders         # List folders with sync status
syncthing folders -i <id> # Detailed folder info (JSON)
syncthing devices         # List devices with connection status
syncthing scan [folder]   # Trigger rescan (all folders if none specified)
syncthing errors          # Show sync errors
syncthing errors --clear  # Clear all errors
syncthing pending         # Show pending devices/folders to approve
syncthing events          # Show recent events
syncthing restart         # Restart syncthing
syncthing shutdown        # Shutdown syncthing
syncthing config          # Show current config
syncthing config --api-key <KEY> --host <URL>  # Configure manually
```

## API Key

Automatically read from `~/.config/syncthing/config.xml`. Override with:
```bash
syncthing config --api-key YOUR_KEY
```

## Architecture

- `config.rs` - Config loading, auto-discovers API key from syncthing config
- `api.rs` - REST API client
- `main.rs` - CLI commands

## Syncthing REST API Reference

- Docs: https://docs.syncthing.net/dev/rest.html
- Source: ~/Repos/syncthing
