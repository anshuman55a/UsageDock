# AIUsageHub

A cross-platform system tray app that tracks your AI coding subscription usage — all in one place.

Built with **Tauri v2**, **React**, **TypeScript**, and **Rust**.

## Supported Providers

| Provider | Auth Source | Data Fetched |
|----------|-----------|--------------|
| **Cursor** | SQLite DB (`state.vscdb`) | Plan usage, on-demand spend |
| **Claude** | `~/.claude/.credentials.json` | Session, weekly, extra usage |
| **GitHub Copilot** | `gh` CLI config (`hosts.yml`) | Premium, chat, completions quota |
| **Codex** (OpenAI) | `~/.codex/auth.json` | Session, weekly, credits |
| **Windsurf** | SQLite DB (`state.vscdb`) + local LS | Prompt & flex credits |

## Features

- 🖥️ **System tray** — lives in your taskbar, click to open
- 📊 **Usage progress bars** — color-coded green → yellow → orange → red
- 🔄 **Auto-refresh** — updates every 15 minutes
- 🎨 **Official icons** — each provider shows its real logo
- ⚡ **Per-provider refresh** — refresh individual providers on demand
- 🔒 **Local-only** — credentials never leave your machine
- 🪟 **Cross-platform** — Windows & Linux (macOS planned)

## Screenshots

_Coming soon_

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) (v18+)
- [Rust](https://rustup.rs/) (stable)
- **Windows**: [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with C++ workload

### Install & Run

```bash
# Clone the repo
git clone https://github.com/anshuman55a/AIUsageHub.git
cd AIUsageHub

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

1. **Credentials** are read from each provider's local config files (SQLite DBs, JSON files, CLI configs)
2. **OAuth tokens** are refreshed automatically when expired
3. **Usage data** is fetched from each provider's API
4. **Results** are displayed as progress bars in a compact tray popup

No API keys to configure — if you're signed into the tool, AIUsageHub picks up your credentials automatically.

## Tech Stack

- **Frontend**: React + TypeScript + TailwindCSS
- **Backend**: Rust (Tauri v2)
- **HTTP**: reqwest
- **SQLite**: rusqlite
- **Build**: Vite + Cargo

## Project Structure

```
src/                    # React frontend
├── App.tsx             # Main UI with provider cards
├── App.css             # Dark theme styles
├── ProviderIcons.tsx   # Official SVG icons
└── main.tsx            # Entry point

src-tauri/              # Rust backend
├── src/
│   ├── lib.rs          # Tray icon, window management
│   └── providers/
│       ├── mod.rs      # Provider types & dispatcher
│       ├── cursor.rs
│       ├── claude.rs
│       ├── copilot.rs
│       ├── codex.rs
│       └── windsurf.rs
├── Cargo.toml
└── tauri.conf.json
```

## License

MIT
