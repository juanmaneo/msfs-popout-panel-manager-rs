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
mod source_card;
mod theme;
mod viewports;
mod widgets;

use crate::core::{Core, MessageKind};
use crate::win::window;
use eframe::egui::{self, ViewportCommand, WindowLevel};
use main_window::MainWindow;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use viewports::{hud_bar, num_pad, status_window};

pub const MAIN_TITLE: &str = "MSFS Pop Out Panel Manager";

const REPAINT_INTERVAL: Duration = Duration::from_millis(250);
/// Frame on which "start minimized" is honoured, so the window has been shown first.
const START_MINIMIZED_FRAME: u32 = 3;

struct App {
    core: Arc<Core>,
    main_window: MainWindow,
    app_hwnd_registered: bool,
    applied_always_on_top: Option<bool>,
    was_minimized: bool,
    pointer_inside: bool,
    logic_frames: u32,
    /// Where the pop out message window goes, computed once per message.
    message_anchor: Option<(MessageKind, Option<egui::Pos2>)>,
    hud_timer: Arc<Mutex<hud_bar::HudTimer>>,
}

impl App {
    fn new(core: Arc<Core>) -> Self {
        Self {
            core,
            main_window: MainWindow::default(),
            app_hwnd_registered: false,
            applied_always_on_top: None,
            was_minimized: false,
            pointer_inside: false,
            logic_frames: 0,
            message_anchor: None,
            hud_timer: Arc::default(),
        }
    }

    /// Top-left of the message window, centred on the simulator window (upstream
    /// `CenterDialogToGameWindow`). Computed when the message appears so the window does
    /// not jump around while it is shown.
    fn message_position(&mut self, ctx: &egui::Context, kind: MessageKind) -> Option<egui::Pos2> {
        if let Some((k, pos)) = self.message_anchor
            && k == kind
        {
            return pos;
        }
        let ppp = ctx.pixels_per_point();
        let pos = window::msfs_game_window().and_then(|h| {
            let r = window::rect_dwm(h);
            if r.width <= 0 || r.height <= 0 {
                return None;
            }
            let size = status_window::message_size(kind);
            Some(egui::pos2(
                (r.x as f32 + r.width as f32 / 2.0) / ppp - size.x / 2.0,
                (r.y as f32 + r.height as f32 / 2.0) / ppp - size.y / 2.0,
            ))
        });
        self.message_anchor = Some((kind, pos));
        pos
    }

    /// Everything that must keep working while the main window is minimized (no `ui` pass).
    fn sync_main_window(&mut self, ctx: &egui::Context) {
        let (always_on_top, exit_requested, start_minimized) = self.core.read(|s| {
            (
                s.settings.general_setting.always_on_top,
                s.ui.exit_requested,
                s.settings.general_setting.start_minimized,
            )
        });

        if exit_requested {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }

        // "Start minimized" (to the taskbar: the tray icon is not ported).
        self.logic_frames = self.logic_frames.saturating_add(1);
        if start_minimized && self.logic_frames == START_MINIMIZED_FRAME {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }

        if !self.app_hwnd_registered
            && let Some(hwnd) = window::find_own_window(MAIN_TITLE)
        {
            self.core.set_app_hwnd(hwnd);
            self.app_hwnd_registered = true;
        }

        // Windows can drop the topmost state of a window that was minimized; re-apply it on
        // restore like upstream's `AppWindow_StateChanged`.
        let minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        if self.was_minimized && !minimized && always_on_top {
            let hwnd = self.core.app_hwnd();
            if hwnd != 0 {
                window::apply_always_on_top(hwnd, true);
            }
        }
        self.was_minimized = minimized;

        // Upstream `AppMainWindow` gives the window focus when the mouse enters it, so the
        // first click is not lost to an always-on-top window sitting over the simulator.
        let pointer_inside = ctx.input(|i| i.pointer.has_pointer());
        if pointer_inside && !self.pointer_inside && !minimized {
            let hwnd = self.core.app_hwnd();
            if hwnd != 0 && window::foreground_window() != hwnd {
                window::set_window_focus(hwnd);
            }
        }
        self.pointer_inside = pointer_inside;

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

        // Show deferred viewports based on core state. The HUD bar / NumPad are driven by
        // `ui.hud_bar_open` / `ui.num_pad_open` (set by the pop out workflow, cleared by
        // their Close buttons and `sync_auxiliary_windows`).
        let core = self.core.clone();
        let (hud_open, pad_open, hud_panel, pad_panel) = core.read(|s| {
            // Panel id when its window handle is not registered yet.
            let unregistered = |t: crate::model::PanelType| {
                s.active_profile().and_then(|p| {
                    p.panel_configs
                        .iter()
                        .find(|c| c.panel_type == t && c.panel_handle.hwnd().is_none())
                        .map(|c| c.id)
                })
            };
            (
                s.ui.hud_bar_open,
                s.ui.num_pad_open,
                unregistered(crate::model::PanelType::HudBarWindow),
                unregistered(crate::model::PanelType::NumPadWindow),
            )
        });

        if hud_open {
            hud_bar::show(ctx, core.clone(), self.hud_timer.clone());
            // The pop out workflow waits for the window handle to position the HUD bar.
            if let Some(id) = hud_panel
                && let Some(hwnd) = window::find_own_window(hud_bar::HUD_BAR_TITLE)
            {
                core.set_panel_window_handle(id, hwnd);
            }
        }

        if pad_open {
            num_pad::show(ctx, core.clone());
            if let Some(id) = pad_panel
                && let Some(hwnd) = window::find_own_window(num_pad::NUM_PAD_TITLE)
            {
                core.set_panel_window_handle(id, hwnd);
            }
        }

        // The status window shows progress messages while a pop out / source edit runs
        // (upstream MessageWindow), and can also be opened manually from the header.
        let (status_open, message) = core.read(|s| (s.ui.status_open, s.ui.message));
        match (status_open, message) {
            (true, _) => status_window::show(ctx, core.clone(), status_window::View::Manual),
            (false, Some(kind)) => {
                let position = self.message_position(ctx, kind);
                status_window::show(
                    ctx,
                    core.clone(),
                    status_window::View::Message { kind, position },
                );
            }
            (false, None) => self.message_anchor = None,
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        ctx.request_repaint_after(REPAINT_INTERVAL);

        self.main_window
            .show(ui, &self.core, &mut || self.core.toggle_status_window());
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
