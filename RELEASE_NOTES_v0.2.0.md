# UsageDock v0.2.0

UsageDock `v0.2.0` is a larger product milestone focused on a cleaner tray experience, stronger provider behavior, and a more polished Windows desktop app.

## Highlights

- Refreshed `UsageDock` branding with a new tray/app icon and matching in-app logo treatment.
- Reworked the dock UI with a subtler visual system, improved provider hierarchy, and cleaner card styling.
- Prioritized reporting providers at the top of the panel and moved unavailable providers into a collapsible section.
- Added auto-refresh controls:
  - Turn auto-refresh on or off
  - Choose refresh intervals of `5`, `10`, `15`, `30`, or `60` minutes
- Replaced the native interval picker with a custom dark menu that matches the dock UI.

## Fixes

- Fixed the Windows refresh flow so manual refresh no longer flashes a console window or collapses the panel because of transient focus loss.
- Fixed Windsurf provider detection by discovering the correct local language-server process and probing the real working endpoint before requesting usage data.
- Fixed duplicate tray instances by enforcing single-instance app behavior.
- Fixed the interval control so only the custom dropdown chevron is shown.

## Installation

- Windows installer: `UsageDock_0.2.0_x64-setup.exe`
- Platform: Windows x64
- Install mode: per-user, no administrator privileges required
