# UsageDock

A cross-platform system tray app that tracks your AI coding subscription usage in one place.

Built with **Tauri v2**, **React**, **TypeScript**, and **Rust**.

## Supported Providers

| Provider | Auth Source | Data Fetched |
|----------|-------------|--------------|
| **Cursor** | SQLite DB (`state.vscdb`) | Plan usage, on-demand spend |
| **Claude** | `~/.claude/.credentials.json` | Session, weekly, extra usage |
| **GitHub Copilot** | `gh` CLI config (`hosts.yml`) | Premium, chat, completions quota |
| **Codex** (OpenAI) | `~/.codex/auth.json` | Session, weekly, credits |
| **Windsurf** | SQLite DB (`state.vscdb`) + local LS | Prompt and flex credits |

## Features

- System tray app that lives in your taskbar
- Usage progress bars with color-coded thresholds
- Auto-refresh every 15 minutes
- Official provider icons
- Per-provider refresh on demand
- Local-only credential access
- Cross-platform support for Windows and Linux

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/) (stable)
- **Windows**: [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the C++ workload

### Install and Run

```bash
# Clone the repo
git clone https://github.com/anshuman55a/UsageDock.git
cd UsageDock

# Install dependencies
npm install

# Run in development mode
npm run tauri dev
```

### Build for Production

```bash
npm run tauri build
```

The installer will be in `src-tauri/target/release/bundle/`.

## How It Works

1. Credentials are read from each provider's local config files.
2. OAuth tokens are refreshed automatically when expired.
3. Usage data is fetched from each provider's API.
4. Results are displayed in a compact tray popup.

No API keys to configure. If you're signed into the tool, UsageDock picks up your credentials automatically.

## Tech Stack

- **Frontend**: React + TypeScript + TailwindCSS
- **Backend**: Rust (Tauri v2)
- **HTTP**: reqwest
- **SQLite**: rusqlite
- **Build**: Vite + Cargo

## Project Structure

```text
src/                    # React frontend
|- App.tsx              # Main UI with provider cards
|- App.css              # Dark theme styles
|- ProviderIcons.tsx    # Official SVG icons
`- main.tsx             # Entry point

src-tauri/              # Rust backend
|- src/
|  |- lib.rs            # Tray icon, window management
|  `- providers/
|     |- mod.rs         # Provider types and dispatcher
|     |- cursor.rs
|     |- claude.rs
|     |- copilot.rs
|     |- codex.rs
|     `- windsurf.rs
|- Cargo.toml
`- tauri.conf.json
```

## License

MIT
