# UsageDock v0.2.6

UsageDock `v0.2.6` is a security-focused patch release that preserves the current working provider behavior.

## Highlights

- Removed untrusted executable lookup fallbacks for GitHub CLI, `ps`, and PowerShell by using trusted absolute paths only.
- Parameterized SQLite reads in the touched providers instead of building the query strings manually.
- Kept the Windsurf local language-server path working on the current machine while preserving the stricter endpoint-selection logic introduced earlier.

## Installation

- Windows installer: `UsageDock_0.2.6_x64-setup.exe`
- Platform: Windows x64
- Install mode: per-user, no administrator privileges required
