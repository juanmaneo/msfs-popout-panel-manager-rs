# MSFS Pop Out Panel Manager - Rust Port

This is a Rust port of the [MSFS Pop Out Panel Manager](https://github.com/hawkeye-stan/msfs-popout-panel-manager) by Stanley Kwok.

## About This Port

This is a complete rewrite of the original C#/WPF application in Rust, maintaining full feature compatibility with the original application (except for the auto-updater, tray icon, VCC library download and .NET app-cache cleanup).

### Key Features
- ✅ Full feature parity with the original application (except the auto-updater, tray icon, VCC library download and .NET app-cache cleanup)
- ✅ Data file compatibility with the original JSON files (`userprofiledata.json` / `appsettingdata.json`)
- ✅ Cross-platform design (though currently only Windows is supported due to SimConnect dependency and using MSFS on Linux or BSD via proton/wine is adventurous)
- ✅ Modern Rust codebase with improved error handling and memory safety

## Installation

1. Download the latest release from the [Releases page](https://github.com/yourusername/msfs-popout-panel-manager-rs/releases)
2. Extract the zip file to a folder of your choice
3. Copy `SimConnect.dll` from your MSFS installation directory to the same folder as the executable
4. Run `msfs-popout-panel-manager.exe`

## Building from Source

1. Install Rust from [https://www.rust-lang.org/](https://www.rust-lang.org/)
2. Clone this repository
3. Run `cargo build --release --target x86_64-pc-windows-gnu`
4. The executable will be in `target/x86_64-pc-windows-gnu/release/`

## Differences from the Original

- **Auto Updater**: Not implemented in this port
- **Tray Icon**: Not implemented in this port
- **VCC Library Download / .NET Cache Cleanup**: Not applicable to a native binary
- **Help**: Keyboard commands, user guide / release / support links and About (with licence) are ported; the in-app release notes (`VERSION.md`) are not
- **Panel overlay**: The blinking ring is not drawn - upstream declares `IsShownPanelSource` but never sets it
- **Panel reordering**: Up/down buttons instead of drag and drop
- **Logging**: Errors only, to `error.log` (rolling, 25 MB x 5), plus a panic hook
- **UI Framework**: Uses egui instead of WPF for the user interface

## License

This project is licensed under the GNU Affero General Public License v3.0 (AGPL-3.0-only). See the [LICENSE](LICENSE) file for details.

## Credits

- Original application by Stanley Kwok - [https://github.com/hawkeye-stan/msfs-popout-panel-manager](https://github.com/hawkeye-stan/msfs-popout-panel-manager)
- All original credits from the upstream project apply to this port as well
