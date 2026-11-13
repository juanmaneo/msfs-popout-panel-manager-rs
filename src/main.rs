//! MSFS Pop Out Panel Manager - Rust port of
//! <https://github.com/hawkeye-stan/msfs-popout-panel-manager> (AGPL-3.0).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
// `cargo test` on Linux compiles the pure modules without the Windows-only `core`/`ui`
// modules that consume them, so rustc's dead-code lint would flag items that are live
// in the real (Windows) build. Production and `cargo check`/`clippy` still deny warnings.
#![cfg_attr(test, allow(dead_code))]

#[cfg(any(windows, test))]
mod autostart;
#[cfg(windows)]
mod core;
#[cfg(any(windows, test))]
mod flight_sim;
#[cfg(any(windows, test))]
mod fps;
#[cfg(any(windows, test))]
mod hook_plan;
#[cfg(any(windows, test))]
mod keybinding;
#[cfg(any(windows, test))]
mod lod;
#[cfg(windows)]
mod logger;
#[cfg(any(windows, test))]
mod model;
#[cfg(any(windows, test))]
mod overlay_raster;
#[cfg(any(windows, test))]
mod simconnect;
#[cfg(any(windows, test))]
mod simdefs;
#[cfg(any(windows, test))]
mod status;
#[cfg(any(windows, test))]
mod storage;
#[cfg(any(windows, test))]
mod touch;
#[cfg(windows)]
mod ui;
#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
    logger::install_panic_hook();
    win::set_dpi_aware();

    if !win::acquire_single_instance("Local\\MSFSPopoutPanelManagerRs-SingleInstance") {
        win::message_box(
            "MSFS Pop Out Panel Manager",
            "MSFS Pop Out Panel Manager is already running.",
        );
        return;
    }

    let core = match core::Core::new() {
        Ok(core) => core,
        Err(e) => {
            log::error!("failed to start background threads: {e}");
            win::message_box(
                "MSFS Pop Out Panel Manager",
                &format!("The application could not start: {e}"),
            );
            return;
        }
    };
    let result = ui::run(core.clone());
    core.shutdown();

    if let Err(e) = result {
        log::error!("GUI terminated with an error: {e}");
        win::message_box(
            "MSFS Pop Out Panel Manager",
            &format!("The application stopped: {e}"),
        );
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!(
        "MSFS Pop Out Panel Manager only runs on Windows (it controls Microsoft Flight Simulator)."
    );
    std::process::exit(1);
}
