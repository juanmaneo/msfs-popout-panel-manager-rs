//! MSFS Pop Out Panel Manager - Rust port of
//! <https://github.com/hawkeye-stan/msfs-popout-panel-manager> (AGPL-3.0).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod autostart;
#[cfg(windows)]
mod core;
mod flight_sim;
mod fps;
mod hook_plan;
mod keybinding;
mod lod;
mod logger;
mod model;
mod overlay_raster;
mod simconnect;
mod simdefs;
mod status;
mod storage;
mod touch;
#[cfg(windows)]
mod ui;
#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
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
