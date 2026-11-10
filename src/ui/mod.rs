//! GUI (egui / eframe).
//!
//! Rules for talking to the core (the state mutex is not reentrant):
//! * Take a short `core.read(..)` that clones what the frame needs ([`snapshot`]), render
//!   against the copy, and apply every edit on its own with a short `core.write(..)` that
//!   targets one field by id (or call the matching `Core` method). Never write whole
//!   profiles back: background threads update runtime fields concurrently.
//! * Never call a `Core` method from inside a `read`/`write` closure.

mod dialogs;
mod edit;
mod main_window;
mod panel_card;
mod preferences;
mod profile_card;
mod snapshot;
mod theme;
mod viewports;
mod widgets;

use crate::core::Core;
use crate::win::window;
use eframe::egui::{self, ViewportCommand, WindowLevel};
use main_window::MainWindow;
use std::sync::Arc;
use std::time::Duration;
use viewports::{hud_bar, num_pad};

pub const MAIN_TITLE: &str = "MSFS Pop Out Panel Manager";

const REPAINT_INTERVAL: Duration = Duration::from_millis(250);
/// Frames to wait before honouring "start minimized" so the window is shown first.
const START_MINIMIZED_FRAME: u32 = 3;

struct App {
    core: Arc<Core>,
    main_window: MainWindow,
    app_hwnd_registered: bool,
    applied_always_on_top: Option<bool>,
    ui_frames: u32,
}

impl App {
    fn new(core: Arc<Core>) -> Self {
        Self {
            core,
            main_window: MainWindow::default(),
            app_hwnd_registered: false,
            applied_always_on_top: None,
            ui_frames: 0,
        }
    }

    /// Everything that must keep working while the main window is minimized (no `ui` pass).
    fn sync_main_window(&mut self, ctx: &egui::Context) {
        let (always_on_top, exit_requested) = self.core.read(|s| {
            (
                s.settings.general_setting.always_on_top,
                s.ui.exit_requested,
            )
        });

        if exit_requested {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }

        if !self.app_hwnd_registered {
            if let Some(hwnd) = window::find_own_window(MAIN_TITLE) {
                self.core.set_app_hwnd(hwnd);
                self.app_hwnd_registered = true;
            }
        }

        if self.applied_always_on_top != Some(always_on_top) {
            self.applied_always_on_top = Some(always_on_top);
            ctx.send_viewport_cmd(ViewportCommand::WindowLevel(if always_on_top {
                WindowLevel::AlwaysOnTop
            } else {
                WindowLevel::Normal
            }));
        }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(REPAINT_INTERVAL);
        self.sync_main_window(ctx);

        // Show deferred viewports based on core state
        let core = self.core.clone();
        core.read(|s| {
            if s.active_profile()
                .map_or(false, |p| p.profile_setting.hud_bar_config.is_enabled)
            {
                hud_bar::show(ctx, core.clone());
            }

            if s.active_profile()
                .map_or(false, |p| p.profile_setting.num_pad_config.is_enabled)
            {
                num_pad::show(ctx, core.clone());
            }
        });
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ctx.request_repaint_after(REPAINT_INTERVAL);

        self.ui_frames = self.ui_frames.saturating_add(1);
        if self.ui_frames == START_MINIMIZED_FRAME
            && self
                .core
                .read(|s| s.settings.general_setting.start_minimized)
        {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }

        self.main_window.show(ui, &self.core);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {}
}

/// Run the GUI until the main window is closed.
pub fn run(core: Arc<Core>) -> eframe::Result<()> {
    let always_on_top = core.read(|s| s.settings.general_setting.always_on_top);
    let mut viewport = egui::ViewportBuilder::default()
        .with_title(MAIN_TITLE)
        .with_inner_size([1100.0, 760.0])
        .with_min_inner_size([900.0, 600.0]);
    if always_on_top {
        viewport = viewport.with_always_on_top();
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        MAIN_TITLE,
        options,
        Box::new(move |cc| {
            theme::apply(&cc.egui_ctx);
            core.set_egui_context(cc.egui_ctx.clone());
            Ok(Box::new(App::new(core)))
        }),
    )
}
