# VibeLights CLI

Run VibeLights without the GUI. External tools (Claude Code, scripts, MCP server) can fully control it from the terminal.

## Installation

```bash
# Build the CLI binary (no GUI dependencies)
cd src-tauri
cargo build --bin vibelights-cli --no-default-features --release

# Or install it system-wide
cargo install --path src-tauri --bin vibelights-cli --no-default-features
```

## Quick Start

### 1. Start the server

```bash
vibelights-cli serve --data-dir ~/vibelights-data
```

### 2. In another terminal, use subcommands

```bash
# Create and open a setup
vibelights-cli setups create "My House"
vibelights-cli setups open my-house

# Create and open a sequence
vibelights-cli sequences create "Christmas Show"
vibelights-cli sequences open christmas-show

# Add a track and effect
vibelights-cli add-track --name "Roofline" --fixture 1
vibelights-cli add-effect --track 0 --kind Rainbow --start 0 --end 10

# See what you've built
vibelights-cli describe
```

## Two-Mode Design

### Server mode: `vibelights-cli serve`

Starts a long-running HTTP API server. All other commands connect to this.

```
vibelights-cli serve [OPTIONS]
  --data-dir <PATH>     Data directory (overrides settings.json)
  --setup <SLUG>        Open this setup on startup
  --sequence <SLUG>     Open this sequence on startup (requires --setup)
  --port <PORT>         Bind to specific port (default: OS-assigned)
  --api-key <KEY>       Set Claude API key
```

### Client mode: `vibelights-cli <command>`

Connects to a running server (discovers port from `.vibelights-port` file or `--port` flag).

## Command Reference

### Setup Commands

```bash
vibelights-cli setups list                  # List all setups
vibelights-cli setups create "My House"     # Create a new setup
vibelights-cli setups open my-house         # Open/load a setup
vibelights-cli setups delete my-house       # Delete a setup
vibelights-cli setups save                  # Save current setup
```

### Sequence Commands

```bash
vibelights-cli sequences list                     # List sequences
vibelights-cli sequences create "Christmas Show"  # Create a sequence
vibelights-cli sequences open christmas-show      # Open a sequence
vibelights-cli sequences delete christmas-show    # Delete a sequence
vibelights-cli sequences save                     # Save current sequence
```

### Effect Editing

```bash
vibelights-cli add-effect --track 0 --kind Rainbow --start 0 --end 10
vibelights-cli add-track --name "Roofline" --fixture 1
vibelights-cli add-track --name "Eaves" --fixture 2 --blend-mode Add
vibelights-cli delete-effects --targets "0:2,0:3"
vibelights-cli update-param --track 0 --effect 0 --key color \
  --value '{"Color":{"r":255,"g":0,"b":0,"a":255}}'
vibelights-cli update-time --track 0 --effect 0 --start 2.0 --end 8.0
vibelights-cli move-effect --from-track 0 --effect 0 --to-track 1
```

### Playback

```bash
vibelights-cli play
vibelights-cli pause
vibelights-cli seek 5.0
```

### Inspection

```bash
vibelights-cli show                           # Full show JSON
vibelights-cli describe                       # Human-readable summary
vibelights-cli describe --frame-time 5.0      # Include frame state at t=5s
vibelights-cli frame 5.0                      # Frame data at time
vibelights-cli effects                        # Available effect types + schemas
vibelights-cli effect-detail 0 0 0            # Detail for specific effect
```

### Undo/Redo

```bash
vibelights-cli undo
vibelights-cli redo
vibelights-cli undo-state
```

### Chat (AI Assistant)

```bash
vibelights-cli chat "Add a rainbow effect to all fixtures from 0 to 10s"
vibelights-cli chat-history
vibelights-cli chat-clear
```

### Vixen Import

```bash
vibelights-cli vixen-scan "C:\Vixen3"
vibelights-cli vixen-import '<config-json>'
```

### Global Flags

```bash
--port <PORT>    Connect to specific port (default: auto-discover)
--json           Output raw JSON instead of formatted text
```

<!-- SSOT: endpoint tables must match command registry (src-tauri/src/registry/mod.rs) and HTTP routes (src-tauri/src/bin/cli.rs) -->

## HTTP API Reference

The CLI wraps the HTTP API. You can also call endpoints directly.

### Existing Endpoints (12)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/show` | Full show model |
| GET | `/api/effects` | Available effect types |
| GET | `/api/playback` | Playback state |
| GET | `/api/effect/{seq}/{track}/{idx}` | Effect detail |
| GET | `/api/undo-state` | Undo/redo state |
| POST | `/api/command` | Execute edit command |
| POST | `/api/undo` | Undo |
| POST | `/api/redo` | Redo |
| POST | `/api/play` | Start playback |
| POST | `/api/pause` | Pause playback |
| POST | `/api/seek` | Seek to time |
| POST | `/api/save` | Save sequence |

### New Endpoints (27)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/settings` | App settings |
| POST | `/api/settings/initialize` | Initialize data directory |
| GET | `/api/setups` | List setups |
| POST | `/api/setups` | Create setup |
| GET | `/api/setups/{slug}` | Open/load setup |
| DELETE | `/api/setups/{slug}` | Delete setup |
| POST | `/api/setups/{slug}/save` | Save setup |
| PUT | `/api/setups/{slug}/fixtures` | Update fixtures/groups |
| PUT | `/api/setups/{slug}/outputs` | Update controllers/patches |
| GET | `/api/setups/{slug}/sequences` | List sequences |
| POST | `/api/setups/{slug}/sequences` | Create sequence |
| GET | `/api/setups/{slug}/sequences/{seq}` | Open sequence |
| DELETE | `/api/setups/{slug}/sequences/{seq}` | Delete sequence |
| GET | `/api/setups/{slug}/media` | List media files |
| POST | `/api/setups/{slug}/media` | Import media |
| DELETE | `/api/setups/{slug}/media/{filename}` | Delete media |
| GET | `/api/frame?time=5.0` | Render frame at time |
| GET | `/api/describe` | Human-readable description |
| POST | `/api/chat` | Send chat message |
| GET | `/api/chat/history` | Chat history |
| POST | `/api/chat/clear` | Clear chat |
| POST | `/api/chat/stop` | Cancel chat |
| PUT | `/api/chat/api-key` | Set Claude API key |
| POST | `/api/vixen/scan` | Scan Vixen directory |
| POST | `/api/vixen/check-preview` | Check preview file |
| POST | `/api/vixen/execute` | Execute Vixen import |

## Integration with Claude Code

1. Start the CLI server:
   ```bash
   vibelights-cli serve --data-dir ~/vibelights-data --setup my-house --sequence christmas
   ```

2. Claude Code can then use the HTTP API directly, or the MCP server auto-discovers the running instance via the `.vibelights-port` file.

## Integration with MCP Server

The MCP server reads the `.vibelights-port` file from the app config directory to discover the running API server. Start the CLI server first, then the MCP server will auto-connect.

## Port Discovery

The server writes its port to `<config-dir>/.vibelights-port`. Client commands read this file to find the server. You can override with `--port`.

Config directory locations:
- Windows: `%APPDATA%\com.vibelights.app\`
- macOS: `~/Library/Application Support/com.vibelights.app/`
- Linux: `~/.config/com.vibelights.app/`
