# UsageDock

UsageDock is a local-first tray app for checking AI coding tool usage without opening another dashboard.

It reads the auth state you already use on your machine, fetches provider usage locally, and shows the result in a compact dock-style popup.

## What It Does

- Lives in the system tray instead of taking over the desktop
- Surfaces active providers first and collapses unavailable ones until needed
- Supports manual refresh plus configurable auto refresh
- Shows progress bars, reset timing, and provider-specific status in one compact view
- Keeps credentials local and does not require a hosted relay
- Supports in-app updates for packaged releases

## Supported Providers

| Provider | Auth source | Usage shown |
|---|---|---|
| Cursor | local SQLite state | plan usage, included usage, on-demand spend |
| Claude | `~/.claude/.credentials.json` | session, weekly, extra usage |
| GitHub Copilot | GitHub CLI auth (`gh auth login`) | chat and quota usage |
| Codex | `~/.codex/auth.json` | session and weekly usage |
| Windsurf | local SQLite state + local language server | prompt and flex credits |

## Platform Support

- Windows: primary supported platform
- Linux: supported in code and CI, but still secondary to Windows for day-to-day validation
- macOS: planned, not shipped yet

## Privacy Model

UsageDock is designed to be local-first.

- It reads credentials and local state from tools already installed on your machine.
- It calls provider endpoints or local provider services directly.
- It does not require you to paste provider tokens into UsageDock.
- It does not run a hosted backend for core usage tracking.

That said, UsageDock necessarily touches sensitive local auth state in order to work. Review the code before using it if that threat model matters for your environment.

## Tech Stack

- Tauri v2
- React 19
- TypeScript
- Rust
- Rusqlite for local provider state reads

## Repository Layout

```text
src/                           React tray UI
|- App.tsx                     Main app UI and interaction logic
|- App.css                     Tray UI styling
|- ProviderIcons.tsx           Provider marks
`- main.tsx                    Frontend entry

src-tauri/                     Rust backend
|- src/
|  |- lib.rs                   Tauri startup, tray behavior, commands
|  `- providers/               Provider integrations
|- capabilities/               Tauri capability configuration
|- icons/                      App and tray icons
|- Cargo.toml                  Rust dependencies
`- tauri.conf.json             Tauri app configuration
```

## Prerequisites

### Required on all platforms

- Node.js 18 or newer
- npm
- Rust stable via `rustup`

### Windows build prerequisites

For local Windows builds, install:

- Visual Studio Build Tools 2022
- the `Desktop development with C++` workload
- Windows 10/11 SDK as part of the Build Tools install
- Microsoft Edge WebView2 Runtime

Recommended Rust target on Windows:

```powershell
rustup default stable-x86_64-pc-windows-msvc
```

Notes:

- `npx tauri build --bundles nsis` uses the Windows toolchain and NSIS packaging flow configured in this repo.
- WebView2 is usually already present on modern Windows installs, but packaged app execution depends on it.

### Linux build notes

Linux builds are produced in CI, but local Linux setup depends on your distro packages. Expect to need:

- `pkg-config`
- WebKitGTK development packages
- GTK development packages
- standard C/C++ build tooling

Windows remains the most validated local development path for this repo.

## Getting Started

```bash
git clone https://github.com/anshuman55a/UsageDock.git
cd UsageDock/devmeter
npm install
```

## Run in Development

```bash
npm run tauri dev
```

That starts the Vite frontend and the Tauri shell together.

## Build a Local Release

```bash
npx tauri build --bundles nsis
```

Useful verification commands:

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

Windows release artifacts are written to:

```text
src-tauri/target/release/
src-tauri/target/release/bundle/nsis/
```

## Using Copilot

GitHub Copilot support depends on GitHub CLI auth being present locally.

Install GitHub CLI and log in:

```powershell
gh auth login
gh auth status
```

Without that local auth state, UsageDock cannot read Copilot usage for the current implementation.

## Release Process

Public releases are cut from `main`.

Before tagging a release, keep versions aligned in:

- `package.json`
- `package-lock.json`
- `src-tauri/Cargo.toml`
- `src-tauri/Cargo.lock`
- `src-tauri/tauri.conf.json`

Releases are triggered by pushing a tag matching `v*`.

Example:

```bash
git tag v0.2.10
git push origin main
git push origin v0.2.10
```

The GitHub Actions release workflow lives at:

```text
.github/workflows/release.yml
```

## Updater Notes

UsageDock supports in-app updates for packaged releases.

Updater signing and release automation details are maintained separately from this README.

If you are working on release infrastructure, updater signing, or GitHub Actions release setup, use:

- [UPDATER_SETUP.md](./UPDATER_SETUP.md)
- the release workflow at `.github/workflows/release.yml`

## Open Source Project Files

This repo includes:

- [LICENSE](./LICENSE)
- [CONTRIBUTING.md](./CONTRIBUTING.md)
- [SECURITY.md](./SECURITY.md)
- [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md)
- [CHANGELOG.md](./CHANGELOG.md)

If you are contributing, start with [CONTRIBUTING.md](./CONTRIBUTING.md).

If you are reporting a security issue, do not file a public bug report. Use [SECURITY.md](./SECURITY.md).

## Current Caveats

- Windsurf depends on local language-server behavior and is the most environment-sensitive provider.


## License

UsageDock is released under the [MIT License](./LICENSE).
