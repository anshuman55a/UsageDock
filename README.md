# UsageDock

UsageDock is a local-first desktop tray app for checking AI coding tool usage without opening another dashboard.

It reads the credentials you already use on your machine, fetches provider usage locally, and presents the result in a compact dock-style popup.

## Why UsageDock

- Tray-first UI built to be checked in seconds
- Active providers surface first and unavailable ones stay collapsed until needed
- Manual refresh per provider plus configurable auto refresh
- Local-only credential discovery with no hosted relay
- In-app updates for packaged releases

## Supported Providers

| Provider | Auth source | Usage shown |
|----------|-------------|-------------|
| **Cursor** | SQLite DB (`state.vscdb`) | Plan usage, on-demand spend |
| **Claude** | `~/.claude/.credentials.json` | Session, weekly, extra usage |
| **GitHub Copilot** | `gh` CLI config (`hosts.yml`) | Chat and quota usage |
| **Codex** | `~/.codex/auth.json` | Session and weekly usage |
| **Windsurf** | SQLite DB (`state.vscdb`) + local LS | Prompt and flex credits |

## Platform Support

- Windows: primary supported platform
- Linux: supported in code, but release quality should be treated as secondary until Linux-specific security and packaging review is complete
- macOS: planned, not shipped yet

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) 18+
- [Rust](https://rustup.rs/) stable
- Windows builds: [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the C++ workload

### Run locally

```bash
git clone https://github.com/anshuman55a/UsageDock.git
cd UsageDock/devmeter
npm install
npm run tauri dev
```

### Build a release

```bash
npm run tauri build
```

Windows installers are written to `src-tauri/target/release/bundle/`.

## How It Works

1. UsageDock reads auth state from provider files or local tooling already present on your machine.
2. Tokens are refreshed when the provider flow supports it.
3. Usage data is requested directly from provider endpoints or local provider services.
4. The tray panel shows compact progress bars, reset timing, and provider-specific states.

No extra API key setup is required for the supported flows.

## Project Structure

```text
src/                    React frontend
|- App.tsx              Main tray UI
|- App.css              App styling
|- ProviderIcons.tsx    Provider marks
`- main.tsx             Frontend entry

src-tauri/              Rust backend
|- src/
|  |- lib.rs            Tauri app setup and commands
|  `- providers/        Provider integrations
|- capabilities/        Tauri capability configuration
|- icons/               App and tray icons
`- tauri.conf.json      App configuration
```

## Contributing

Contributions are welcome. Start with [CONTRIBUTING.md](./CONTRIBUTING.md) for setup and contribution expectations.

For security issues, do not open a public issue. Use [SECURITY.md](./SECURITY.md).

## License

UsageDock is released under the [MIT License](./LICENSE).
