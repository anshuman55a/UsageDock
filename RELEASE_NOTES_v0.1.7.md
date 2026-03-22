# UsageDock v0.1.7

UsageDock `v0.1.7` focuses on polishing the Windows tray experience, improving provider readability, and fixing Windsurf reliability.

## What's New

- Refreshed `UsageDock` branding with a new tray/app icon and matching in-app logo treatment.
- Improved the dock UI with a subtler visual system and cleaner provider card hierarchy.
- Prioritized reporting providers at the top of the panel.
- Moved unavailable providers into a collapsible section to reduce clutter.
- Added auto-refresh settings:
  - Turn auto-refresh on or off
  - Choose refresh intervals of `5`, `10`, `15`, `30`, or `60` minutes
- Replaced the native interval dropdown with a custom dark menu so it matches the dock UI.

## Fixes

- Fixed the Windows refresh flow so manual refresh no longer flashes a console window or collapses the panel because of Windsurf process probing.
- Fixed Windsurf provider detection by discovering the correct local language-server process and probing the real working endpoint before requesting usage data.

## Installation

- Windows installer: `UsageDock_0.1.7_x64-setup.exe`
- Platform: Windows x64
- Install mode: per-user, no administrator privileges required

