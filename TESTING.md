# Testing

## Automated

Run on Windows (the crate only fully compiles there):

```
cargo build --locked --all-targets
cargo test --locked
cargo clippy --locked --all-targets
cargo build --release --locked
```

The unit tests do not need MSFS or `SimConnect.dll`. The same checks run in CI on a
Windows runner (`.github/workflows/ci.yml`).

## Manual test log

### 2026-11-13 - Windows 11

Environment:

- Windows 11 (build 26300), x64, single 1920x1080 display
- Rust 1.99.0, `stable-x86_64-pc-windows-msvc`, release build
- MSFS 2020 1.39.12 (Microsoft Store / Xbox app install)
- Original C# Pop Out Panel Manager 4.1.2 for comparison
- Existing user data from the C# app (4 profiles) in `Documents\MSFS Pop Out Panel Manager`

`SimConnect.dll` was copied next to the executable from the MSFS install
(`C:\XboxGames\Microsoft Flight Simulator\Content\SimConnect.dll` for an Xbox app install).

| Area | Result |
|---|---|
| Build, tests (107), clippy, release build | Pass |
| Startup without `SimConnect.dll` | Window opens; `error.log` records that the DLL could not be loaded |
| Single instance | A second launch does not start a second main window |
| Close | Exits cleanly |
| Reading the C# app's `userprofiledata.json` / `appsettingdata.json` | All 4 profiles and settings load; UI matches the C# app (profile, panel name, position and size) |
| Writing the data files back | JSON identical to the C# files, except the legacy `StartPopOutKeyBinding: null` is not written (read for migration only; the C# app treats a missing value as `null`) |
| Data files after a run | Unchanged (checksums verified) |
| Loading the real `SimConnect.dll` | Pass |
| SimConnect connection | Shows "Connected" once MSFS has finished loading (while it loads, both apps show disconnected); no setup errors in `error.log` |
| Running alongside the C# app | Both run at the same time |

Not tested yet (needs a loaded flight):

- Aircraft detection and profile binding
- Panel source selection, pop out and window positioning
- Touch, game refocus, HUD bar, NumPad
- Dynamic LOD
- Auto start via `exe.xml`
