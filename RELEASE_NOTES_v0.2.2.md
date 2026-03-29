# UsageDock v0.2.2

UsageDock `v0.2.2` adds the first auto-update path for packaged releases and keeps the updater quiet inside the tray UI until a newer signed build is actually available.

## Highlights

- Added in-app update checks and install flow for updater-enabled builds.
- Added release workflow support for signed updater artifacts and `latest.json` publication when updater secrets are configured.
- Kept local builds safe by hiding updater UI when updater keys and endpoint were not compiled into the app.
- Documented the updater signing and release setup for future releases.

## Installation

- Windows installer: `UsageDock_0.2.2_x64-setup.exe`
- Platform: Windows x64
- Install mode: per-user, no administrator privileges required
