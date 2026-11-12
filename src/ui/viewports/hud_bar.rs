//! The HUD bar viewport (port of `HudBar.xaml` / `HudBarViewModel.cs`):
//! aircraft data (trim, flaps, brake, gear), a timer and sim rate controls.

use crate::core::Core;
use crate::flight_sim::HudBarData;
use eframe::egui::{self, Align, Button, Color32, Layout, RichText, Sense, Ui, ViewportCommand};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const HUD_BAR_TITLE: &str = "HudBar - POPM";

/// Timer state (upstream `HudBarViewModel.IsTimerStarted` / `Timer`). Owned by the app so it
/// survives the viewport being re-registered every frame.
#[derive(Default)]
pub struct HudTimer {
    started: bool,
    /// Seconds accumulated before the current run.
    base_secs: u64,
    /// When the current run started (None while stopped).
    run_start: Option<Instant>,
}

impl HudTimer {
    fn elapsed_secs(&self) -> u64 {
        self.base_secs + self.run_start.map(|t| t.elapsed().as_secs()).unwrap_or(0)
    }

    fn text(&self) -> String {
        let s = self.elapsed_secs();
        format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    }
}

pub fn show(ctx: &egui::Context, core: Arc<Core>, timer: Arc<Mutex<HudTimer>>) {
    let viewport_id = egui::ViewportId::from_hash_of(HUD_BAR_TITLE);
    let builder = egui::ViewportBuilder::default()
        .with_title(HUD_BAR_TITLE)
        .with_inner_size([1920.0, 40.0])
        .with_min_inner_size([1920.0, 40.0])
        .with_max_inner_size([1920.0, 40.0])
        .with_resizable(false)
        .with_decorations(false)
        .with_always_on_top()
        .with_active(false)
        .with_taskbar(false);
    // Note: no mouse passthrough - the HUD bar has clickable buttons (timer, sim rate, close).

    ctx.show_viewport_deferred(viewport_id, builder, move |ctx, class| {
        if class != egui::ViewportClass::Deferred {
            return;
        }
        // Closing the window with the OS (e.g. Alt+F4) must also clear the open flag, or the
        // viewport would be registered again on the next frame.
        if ctx.input(|i| i.viewport().close_requested()) {
            core.close_hud_bar();
        }

        let start_drag = egui::CentralPanel::default()
            .show(ctx, |ui| {
                ui.visuals_mut().dark_mode = true;
                ui.visuals_mut().panel_fill = Color32::from_rgba_unmultiplied(0, 0, 0, 200);

                // Registered before the row so that the buttons take their own clicks; a drag
                // anywhere else moves the window (upstream `HudBar_MouseLeftButtonDown`).
                let drag = ui.interact(
                    ui.max_rect(),
                    egui::Id::unique("hud_bar_drag"),
                    Sense::drag(),
                );

                let data = core.read(|s| s.sim.hud_bar_data.clone());
                let connected = core.read(|s| s.sim.is_sim_connect_active);
                let mut timer = timer.lock().unwrap_or_else(|e| e.into_inner());

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let title_color = if connected {
                        Color32::WHITE
                    } else {
                        Color32::RED
                    };
                    ui.label(
                        RichText::new(data.hud_bar_type.label())
                            .size(13.0)
                            .color(title_color)
                            .strong(),
                    );
                    sep(ui);

                    field(ui, "Elevator Trim:", &data.elevator_trim_text());
                    sep(ui);
                    field(ui, "Aileron Trim:", &data.aileron_trim_text());
                    sep(ui);
                    field(ui, "Rudder Trim:", &data.rudder_trim_text());
                    sep(ui);
                    field(ui, "Flaps:", &data.flap_text());
                    sep(ui);
                    field_colored(
                        ui,
                        "Brake:",
                        data.parking_brake_text(),
                        match data.parking_brake_text() {
                            "Engaged" => Color32::RED,
                            _ => Color32::from_rgb(105, 105, 105),
                        },
                    );
                    sep(ui);
                    gear(ui, &data);
                    sep(ui);

                    // Timer
                    ui.label(RichText::new("Timer:").size(13.0).color(Color32::WHITE));
                    ui.label(
                        RichText::new(timer.text())
                            .size(13.0)
                            .monospace()
                            .color(Color32::from_rgb(0, 255, 0)),
                    );
                    if ui
                        .add(Button::new(if timer.started { "STOP" } else { "START" }).frame(true))
                        .clicked()
                    {
                        if timer.started {
                            timer.base_secs = timer.elapsed_secs();
                            timer.run_start = None;
                            timer.started = false;
                        } else {
                            timer.run_start = Some(Instant::now());
                            timer.started = true;
                        }
                    }
                    if ui.add(Button::new("RESET").frame(true)).clicked() {
                        timer.started = false;
                        timer.base_secs = 0;
                        timer.run_start = None;
                    }
                    sep(ui);

                    // Sim rate
                    ui.label(RichText::new("Sim Rate:").size(13.0).color(Color32::WHITE));
                    ui.label(
                        RichText::new(format!("{}X", data.sim_rate_text()))
                            .size(13.0)
                            .monospace()
                            .color(Color32::from_rgb(0, 255, 0)),
                    );
                    if ui
                        .add(Button::new(RichText::new("▲").size(12.0)).frame(false))
                        .clicked()
                    {
                        core.increase_sim_rate();
                    }
                    if ui
                        .add(Button::new(RichText::new("▼").size(12.0)).frame(false))
                        .clicked()
                    {
                        core.decrease_sim_rate();
                    }

                    // Push the close button to the right edge
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add(Button::new(RichText::new("Close").size(13.0)).frame(true))
                            .clicked()
                        {
                            core.close_hud_bar();
                        }
                    });
                });

                drag.drag_started()
            })
            .inner;

        if start_drag {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }

        // 1 Hz while the timer runs, otherwise slower refresh for the live data.
        let running = timer.lock().map(|t| t.started).unwrap_or(false);
        let interval = if running {
            Duration::from_millis(500)
        } else {
            Duration::from_millis(250)
        };
        ctx.request_repaint_after(interval);
    });
}

fn sep(ui: &mut Ui) {
    ui.label(
        RichText::new("|")
            .size(13.0)
            .color(Color32::from_rgb(120, 120, 120)),
    );
}

fn field(ui: &mut Ui, label: &str, value: &str) {
    ui.label(RichText::new(label).size(13.0).color(Color32::WHITE));
    ui.label(
        RichText::new(value)
            .size(13.0)
            .monospace()
            .color(Color32::from_rgb(0, 255, 0)),
    );
}

fn field_colored(ui: &mut Ui, label: &str, value: &str, color: Color32) {
    ui.label(RichText::new(label).size(13.0).color(Color32::WHITE));
    ui.label(RichText::new(value).size(13.0).monospace().color(color));
}

/// Gear indicator: L / C / R letters coloured by state, DOWN = lime, UP = dim gray, MOVING = red.
fn gear(ui: &mut Ui, data: &HudBarData) {
    ui.label(RichText::new("Gear:").size(13.0).color(Color32::WHITE));
    for (tag, state) in [
        ("L", data.gear_left_text()),
        ("C", data.gear_center_text()),
        ("R", data.gear_right_text()),
    ] {
        let color = match state {
            "DOWN" => Color32::from_rgb(0, 255, 0),
            "UP" => Color32::from_rgb(105, 105, 105),
            _ => Color32::RED,
        };
        ui.label(RichText::new(tag).size(13.0).monospace().color(color));
    }
}
