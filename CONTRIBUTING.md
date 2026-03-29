# Contributing to UsageDock

Thanks for contributing.

## Before You Start

- Check open issues before starting a larger change.
- Prefer small, reviewable pull requests over broad refactors.
- Keep the tray app compact and utility-first. Avoid adding dashboard-style clutter.
- Preserve local-first behavior. Do not introduce hosted dependencies for core usage tracking.

## Local Setup

```bash
git clone https://github.com/anshuman55a/UsageDock.git
cd UsageDock/devmeter
npm install
npm run tauri dev
```

Windows builds require Visual Studio Build Tools with the C++ workload installed.

## Development Expectations

- Keep provider logic isolated to `src-tauri/src/providers/`.
- Prefer straightforward UI changes over novelty. This is a desktop utility, not a marketing surface.
- Do not commit secrets, tokens, private keys, or local machine paths.
- When changing auth or provider fetch logic, test with the real provider if possible.
- If a provider must fail, prefer clear secure-fail behavior over insecure fallback behavior.

## Checks

Run these before opening a pull request:

```bash
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

If you touch release or updater code, also verify a packaged build:

```bash
npx tauri build --bundles nsis
```

## Pull Requests

- Explain the user-visible change clearly.
- Mention any provider-specific limitations or platform caveats.
- Include screenshots for UI changes when helpful.
- Keep unrelated cleanup out of the same PR unless it is required for the change.

## Release Notes

If the change affects shipped behavior, add a short entry to `CHANGELOG.md` or the release notes for the next version as appropriate.
