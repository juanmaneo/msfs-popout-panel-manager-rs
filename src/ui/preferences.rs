use super::snapshot::Snapshot;
use super::widgets::{heading, toggle_labeled};
use crate::core::Core;
use eframe::egui::{self, Context, Ui};
use std::sync::Arc;

#[derive(Default)]
pub struct Preferences {
    open: bool,
}

impl Preferences {
    pub fn open(&mut self) {
        self.open = true;
    }

    pub fn show(&mut self, ctx: &Context, core: &Arc<Core>, _snap: &Snapshot) {
        let mut open = self.open;
        egui::Window::new("Preferences")
            .open(&mut open)
            .resizable(true)
            .default_width(600.0)
            .default_height(500.0)
            .show(ctx, |ui| {
                self.show_content(ui, core);
            });
        self.open = open;
    }

    fn show_content(&mut self, ui: &mut Ui, core: &Arc<Core>) {
        heading(ui, "General Settings");
        ui.add_space(8.0);

        core.read(|s| {
            let mut settings = s.settings.general_setting.clone();
            let mut changed = false;

            changed |= toggle_labeled(ui, &mut settings.always_on_top, "Always on top").changed();
            changed |=
                toggle_labeled(ui, &mut settings.start_minimized, "Start minimized").changed();
            changed |=
                toggle_labeled(ui, &mut settings.minimize_to_tray, "Minimize to tray").changed();

            if changed {
                core.write(|s| s.settings.general_setting = settings);
            }
        });
    }
}
