# PortKill

System tray app for Windows that shows open localhost ports and lets you kill them instantly.

![Windows](https://img.shields.io/badge/Windows-11-0078D6?logo=windows)
![License](https://img.shields.io/badge/license-MIT-green)

## Features

- Lives in the system tray (next to clock/wifi)
- Shows all TCP ports in LISTENING state
- Kill any process with one click
- Auto-refreshes every 3 seconds
- Filter by port number or process name
- Dark theme (Catppuccin Mocha)
- Lightweight (~5MB)

## Install

Download the latest `.msi` or `.exe` from [Releases](https://github.com/chrlss11/portkill/releases).

## Build from source

Requirements: [Bun](https://bun.sh), [Rust](https://rustup.rs), Windows 10/11

```bash
bun install
bun run tauri build
```

The installer will be at `src-tauri/target/release/bundle/`.

## Stack

- **Frontend**: Svelte 5 + TypeScript
- **Backend**: Rust + Tauri v2
- **CI/CD**: GitHub Actions (builds on tag push)

## License

MIT
