//! The preferences window: tabbed view of all application settings
//! (port of `PreferenceDrawer.xaml` / `DynamicLodPreference.xaml`).

use super::snapshot::Snapshot;
use super::theme;
use super::widgets::{ResponseExt, heading, link, toggle_labeled};
use crate::core::Core;
use crate::flight_sim::DynamicLodSimData;
use crate::keybinding::display_binding;
use crate::model::{AfterPopOutCameraViewType, ApplicationSetting};
use eframe::egui::{self, Context, DragValue, RichText, ScrollArea, TextEdit, TextStyle, Ui};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    General,
    AutoPopOut,
    PopOut,
    GameRefocus,
    KeyboardShortcut,
    Touch,
    TrackIr,
    WindowedMode,
    DynamicLod,
}

impl Tab {
    const ALL: [Tab; 9] = [
        Tab::General,
        Tab::AutoPopOut,
        Tab::PopOut,
        Tab::GameRefocus,
        Tab::KeyboardShortcut,
        Tab::Touch,
        Tab::TrackIr,
        Tab::WindowedMode,
        Tab::DynamicLod,
    ];

    fn label(self) -> &'static str {
        match self {
            Tab::General => "General Settings",
            Tab::AutoPopOut => "Auto Pop Out Panel Settings",
            Tab::PopOut => "Pop Out Settings",
            Tab::GameRefocus => "Game Refocus Settings",
            Tab::KeyboardShortcut => "Keyboard Shortcut Settings",
            Tab::Touch => "Touch Settings",
            Tab::TrackIr => "Track IR Settings",
            Tab::WindowedMode => "Windowed Mode Settings",
            Tab::DynamicLod => "Dynamic LOD Settings",
        }
    }
}

#[derive(Default)]
pub struct Preferences {
    open: bool,
    tab: Tab,
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
            .default_size([820.0, 620.0])
            .min_size([560.0, 380.0])
            .show(ctx, |ui| {
                self.show_content(ui, core);
            });
        self.open = open;
    }

    fn show_content(&mut self, ui: &mut Ui, core: &Arc<Core>) {
        // Category selector (mirrors the upstream TreeView of preference categories).
        ui.horizontal_wrapped(|ui| {
            for tab in Tab::ALL {
                if ui.selectable_label(self.tab == tab, tab.label()).clicked() {
                    self.tab = tab;
                }
            }
        });
        ui.separator();

        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                match self.tab {
                    Tab::General => self.general(ui, core),
                    Tab::AutoPopOut => self.auto_pop_out(ui, core),
                    Tab::PopOut => self.pop_out(ui, core),
                    Tab::GameRefocus => self.game_refocus(ui, core),
                    Tab::KeyboardShortcut => self.keyboard_shortcut(ui, core),
                    Tab::Touch => self.touch(ui, core),
                    Tab::TrackIr => self.track_ir(ui, core),
                    Tab::WindowedMode => self.windowed_mode(ui, core),
                    Tab::DynamicLod => self.dynamic_lod(ui, core),
                }
            });
    }

    fn section(&mut self, ui: &mut Ui, title: &str) {
        heading(ui, title);
        ui.add_space(2.0);
    }

    fn general(&mut self, ui: &mut Ui, core: &Arc<Core>) {
        setting_bool(
            ui,
            core,
            "Always on Top",
            "Pin the application on top of all open windows.",
            |s| s.general_setting.always_on_top,
            |s, v| s.general_setting.always_on_top = v,
        );
        setting_bool(
            ui,
            core,
            "Auto Start",
            "Enable auto start application when MSFS starts. This adds a XML config entry in EXE.xml file.",
            |_s| crate::autostart::is_auto_start(),
            |_, v| {
                let r = if v {
                    crate::autostart::activate()
                } else {
                    crate::autostart::deactivate()
                };
                if let Err(e) = r {
                    log::error!("auto start change failed: {e}");
                }
            },
        );

        setting_bool(
            ui,
            core,
            "Start Minimized",
            "Start the application in minimized mode.",
            |s| s.general_setting.start_minimized,
            |s, v| s.general_setting.start_minimized = v,
        );
        setting_bool(
            ui,
            core,
            "Auto Close When Exiting MSFS",
            "Automatically close the application when exiting MSFS.",
            |s| s.general_setting.auto_close,
            |s, v| s.general_setting.auto_close = v,
        );

        self.section(ui, "Folder Path to Store POPM Configuration and Profiles");
        let mut use_app_data = core.read(|s| s.settings.general_setting.use_application_data_path);
        egui::ComboBox::from_id_salt("data_path_choice")
            .selected_text(if use_app_data {
                "AppData Roaming folder"
            } else {
                "Documents folder (Default)"
            })
            .width(220.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut use_app_data, false, "Documents folder (Default)");
                ui.selectable_value(&mut use_app_data, true, "AppData Roaming folder");
            });
        ui.horizontal_wrapped(|ui| {
            ui.add_space(8.0);
            let path = crate::storage::data_dir(use_app_data);
            link(ui, &path.to_string_lossy(), &path.to_string_lossy());
        });
        if use_app_data != core.read(|s| s.settings.general_setting.use_application_data_path) {
            core.write(|s| s.settings.general_setting.use_application_data_path = use_app_data);
        }
        ui.add_space(10.0);

        setting_bool(
            ui,
            core,
            "Turbo Mode",
            "WARNING! This may not work for all PC. Enable turbo mode to pop out panels as fast as possible.",
            |s| s.general_setting.turbo_mode,
            |s, v| s.general_setting.turbo_mode = v,
        );
    }

    fn auto_pop_out(&mut self, ui: &mut Ui, core: &Arc<Core>) {
        setting_bool(
            ui,
            core,
            "Enable Auto Pop Out Panels",
            "Automatic pop out panels when an aircraft livery is bound to a profile. \
             Detects the flight start signal using SimConnect, waits for the cockpit view, \
             optionally powers on the instrumentation on cold start, then pops out the panels.",
            |s| s.auto_pop_out_setting.is_enabled,
            |s, v| s.auto_pop_out_setting.is_enabled = v,
        );

        self.section(ui, "Auto Pop Out Panel Delay");
        let mut delay = core.read(|s| s.settings.auto_pop_out_setting.ready_to_fly_delay);
        ui.horizontal(|ui| {
            if ui
                .add(DragValue::new(&mut delay).range(0..=10).speed(1.0))
                .changed()
            {
                core.write(|s| s.settings.auto_pop_out_setting.ready_to_fly_delay = delay);
            }
            ui.label(
                RichText::new(
                    "Amount of time in seconds to delay auto pop out panels from starting after \
                 the ready to fly button has been pressed automatically. Extending this delay \
                 helps resolve auto pop out failure because the cockpit has not been loaded \
                 completely yet on slower PCs.",
                )
                .small()
                .weak(),
            );
        });
    }

    fn pop_out(&mut self, ui: &mut Ui, core: &Arc<Core>) {
        // Auto panning (legacy custom camera only)
        setting_bool(
            ui,
            core,
            "Enable Auto Panning (For use with legacy custom camera to select panel source only)",
            "Enable automatic panning of cockpit view when popping out panels. Auto Panning \
             remembers the custom cockpit camera angle you used when defining the locations \
             of pop out panels.",
            |s| s.pop_out_setting.auto_panning.is_enabled,
            |s, v| s.pop_out_setting.auto_panning.is_enabled = v,
        );
        if core.read(|s| s.settings.pop_out_setting.auto_panning.is_enabled) {
            ui.indent("auto_panning_key", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Ctrl-Alt-");
                    digit_key_combo(
                        ui,
                        core,
                        "auto_panning_key",
                        |s| s.pop_out_setting.auto_panning.key_binding.clone(),
                        |s, key| s.pop_out_setting.auto_panning.key_binding = key,
                    );
                    ui.label(
                        RichText::new(
                            "Key binding for saving and recalling the custom MSFS cockpit camera \
                         view (default Ctrl-Alt-0 to save, Alt-0 to load). Requires binding the \
                         keystroke to a custom camera in the MSFS control settings.",
                        )
                        .small()
                        .weak(),
                    );
                });
            });
            ui.add_space(10.0);
        }

        setting_bool(
            ui,
            core,
            "Minimize Pop Out Panel Manager During Pop Out",
            "Minimize Pop Out Panel Manager during pop out process.",
            |s| s.pop_out_setting.minimize_during_pop_out,
            |s, v| s.pop_out_setting.minimize_during_pop_out = v,
        );
        setting_bool(
            ui,
            core,
            "Minimize Pop Out Panel Manager After Pop Out",
            "Minimize Pop Out Panel Manager after the pop out process completed.",
            |s| s.pop_out_setting.minimize_after_pop_out,
            |s, v| s.pop_out_setting.minimize_after_pop_out = v,
        );
        setting_bool(
            ui,
            core,
            "Enable Active Pause",
            "Automatically pause the simulation when the pop out process starts and unpause after.",
            |s| s.pop_out_setting.auto_active_pause,
            |s, v| s.pop_out_setting.auto_active_pause = v,
        );

        // Return to predefined camera view after pop out
        setting_bool(
            ui,
            core,
            "Enable Return to Predefined Camera View After Pop Out",
            "Return to a predefined camera view after the pop out process.",
            |s| s.pop_out_setting.after_pop_out_camera_view.is_enabled,
            |s, v| s.pop_out_setting.after_pop_out_camera_view.is_enabled = v,
        );
        let camera_view =
            core.read(|s| s.settings.pop_out_setting.after_pop_out_camera_view.clone());
        if camera_view.is_enabled {
            ui.indent("camera_view", |ui| {
                let mut selected = camera_view.camera_view;
                egui::ComboBox::from_id_salt("camera_view_type")
                    .selected_text(match selected {
                        AfterPopOutCameraViewType::CockpitCenterView => "Cockpit Center View",
                        AfterPopOutCameraViewType::CustomCameraView => "Custom Camera View",
                    })
                    .width(180.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut selected,
                            AfterPopOutCameraViewType::CockpitCenterView,
                            "Cockpit Center View",
                        );
                        ui.selectable_value(
                            &mut selected,
                            AfterPopOutCameraViewType::CustomCameraView,
                            "Custom Camera View",
                        );
                    });
                if selected != camera_view.camera_view {
                    core.write(|s| {
                        s.settings
                            .pop_out_setting
                            .after_pop_out_camera_view
                            .camera_view = selected
                    });
                }

                if selected == AfterPopOutCameraViewType::CustomCameraView {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label("Alt-");
                        digit_key_combo(
                            ui,
                            core,
                            "camera_key",
                            |s| {
                                s.pop_out_setting
                                    .after_pop_out_camera_view
                                    .key_binding
                                    .clone()
                            },
                            |s, key| s.pop_out_setting.after_pop_out_camera_view.key_binding = key,
                        );
                        ui.label(
                            RichText::new(
                                "Configure key binding for custom camera view to load. Requires \
                                 binding keystroke to custom camera in MSFS control setting. \
                                 (Default: Alt-1 to load).",
                            )
                            .small()
                            .weak(),
                        );
                    });
                }
            });
            ui.add_space(10.0);
        }

        setting_bool(
            ui,
            core,
            "Enable Tracking of Panels When Profile Is Locked",
            "Reset the position and size of panels to the profile values when the profile is locked.",
            |s| s.pop_out_setting.enable_panel_reset_when_locked,
            |s, v| s.pop_out_setting.enable_panel_reset_when_locked = v,
        );
        setting_bool(
            ui,
            core,
            "Enable Pop Out Progress Messages",
            "Show pop out progress messages in the status window.",
            |s| s.pop_out_setting.enable_pop_out_messages,
            |s, v| s.pop_out_setting.enable_pop_out_messages = v,
        );

        // Title bar color customization
        setting_bool(
            ui,
            core,
            "Pop Out Title Bar Color Customization",
            "Customize the color of the pop out panel title bar.",
            |s| s.pop_out_setting.pop_out_title_bar_customization.is_enabled,
            |s, v| s.pop_out_setting.pop_out_title_bar_customization.is_enabled = v,
        );
        if core.read(|s| {
            s.settings
                .pop_out_setting
                .pop_out_title_bar_customization
                .is_enabled
        }) {
            ui.indent("title_bar_color", |ui| {
                ui.horizontal(|ui| {
                    ui.label("Color: #");
                    let mut hex = core.read(|s| {
                        s.settings
                            .pop_out_setting
                            .pop_out_title_bar_customization
                            .hex_color
                            .clone()
                    });
                    let response = ui.add_enabled(
                        true,
                        TextEdit::singleline(&mut hex)
                            .desired_width(70.0)
                            .font(TextStyle::Monospace),
                    );
                    if response.changed() {
                        let valid = theme::parse_hex_color(&hex).is_some();
                        if valid {
                            core.write(|s| {
                                s.settings
                                    .pop_out_setting
                                    .pop_out_title_bar_customization
                                    .hex_color = hex.trim().trim_start_matches('#').to_string();
                            });
                        }
                    }
                    if theme::parse_hex_color(&hex).is_none() {
                        ui.colored_label(theme::ERROR, "invalid (use RRGGBB)");
                    } else if let Some(c) = theme::parse_hex_color(&hex) {
                        let (rect, _resp) =
                            ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 3, c);
                    }
                });
            });
            ui.add_space(10.0);
        }

        setting_bool(
            ui,
            core,
            "Use Left Control + Right Control to Pop Out Panel",
            "If your keyboard does not have a Right-Alt key to perform left click to pop out a \
             panel, map Left Ctrl + Right Ctrl in MSFS control setting (Control Options => \
             Miscellaneous => New UI Window Mode) to pop out panels instead.",
            |s| s.pop_out_setting.use_left_right_control_to_pop_out,
            |s, v| s.pop_out_setting.use_left_right_control_to_pop_out = v,
        );
    }

    fn game_refocus(&mut self, ui: &mut Ui, core: &Arc<Core>) {
        setting_bool(
            ui,
            core,
            "Refocus Game Window",
            "Automatically set focus back to the game window after a period of inactivity when \
             either clicking on a panel or touching a panel when touch feature is enabled. This \
             setting needs to be enabled for each profile and each pop out panel's automatic \
             refocus setting to work.",
            |s| s.refocus_setting.refocus_game_window.is_enabled,
            |s, v| s.refocus_setting.refocus_game_window.is_enabled = v,
        );
        if core.read(|s| s.settings.refocus_setting.refocus_game_window.is_enabled) {
            ui.indent("refocus_delay", |ui| {
                ui.horizontal(|ui| {
                    let mut delay =
                        core.read(|s| s.settings.refocus_setting.refocus_game_window.delay);
                    if ui
                        .add(
                            DragValue::new(&mut delay)
                                .range(0.5..=10.0)
                                .speed(0.1)
                                .max_decimals(1),
                        )
                        .changed()
                    {
                        core.write(|s| {
                            s.settings.refocus_setting.refocus_game_window.delay = delay
                        });
                    }
                    ui.label(
                        RichText::new(
                            "Amount of time in seconds to wait for touch inactivity before input \
                         focus goes back to the game window.",
                        )
                        .small()
                        .weak(),
                    );
                });
            });
        }
    }

    fn keyboard_shortcut(&mut self, ui: &mut Ui, core: &Arc<Core>) {
        setting_bool(
            ui,
            core,
            "Keyboard Shortcuts",
            "Enable using of keyboard shortcuts to control the application.",
            |s| s.keyboard_shortcut_setting.is_enabled,
            |s, v| s.keyboard_shortcut_setting.is_enabled = v,
        );
        if core.read(|s| s.settings.keyboard_shortcut_setting.is_enabled) {
            ui.indent("pop_out_shortcut", |ui| {
                self.section(ui, "Start Pop Out");
                ui.horizontal(|ui| {
                    ui.label("Keyboard Binding:");
                    let (binding, detecting) = core.read(|s| {
                        (
                            s.settings
                                .keyboard_shortcut_setting
                                .pop_out_keyboard_binding
                                .clone(),
                            s.settings.keyboard_shortcut_setting.is_detecting_keystroke,
                        )
                    });
                    if detecting {
                        ui.colored_label(theme::ACCENT, "Scanning...");
                    } else {
                        ui.label(
                            RichText::new(
                                binding
                                    .as_deref()
                                    .map(display_binding)
                                    .unwrap_or_else(|| "(none)".into()),
                            )
                            .monospace(),
                        );
                    }
                    if ui
                        .add_enabled(!detecting, egui::Button::new("Detect"))
                        .tip("Press the key combination to use for Start Pop Out")
                        .clicked()
                    {
                        core.start_detect_shortcut();
                    }
                    if binding.is_some() && ui.button("Clear").clicked() {
                        core.write(|s| {
                            s.settings
                                .keyboard_shortcut_setting
                                .pop_out_keyboard_binding = None
                        });
                    }
                });
            });
        }
    }

    fn touch(&mut self, ui: &mut Ui, core: &Arc<Core>) {
        self.section(ui, "Touch Down Touch Up Delay");
        ui.horizontal(|ui| {
            let mut delay = core.read(|s| s.settings.touch_setting.touch_down_up_delay);
            if ui
                .add(DragValue::new(&mut delay).range(0..=100).speed(5.0))
                .changed()
            {
                core.write(|s| s.settings.touch_setting.touch_down_up_delay = delay);
            }
            ui.label(RichText::new(
                "Amount of time in milliseconds to delay touch down and then touch up event when \
                 operating a touch enabled panel. If your touch is not registering consistently, \
                 increasing this value may help. For a directly connected touch monitor 0 ms works \
                 well; for a tablet using software such as SpaceDesk, increasing this value 5 ms \
                 at a time may compensate for the higher latency.",
            ).small().weak());
        });
    }

    fn track_ir(&mut self, ui: &mut Ui, core: &Arc<Core>) {
        setting_bool(
            ui,
            core,
            "Auto Disable Track IR",
            "Automatically disable Track IR during panel selections and the pop out process. \
             Track IR will be re-enabled once these processes are completed.",
            |s| s.track_ir_setting.auto_disable_track_ir,
            |s, v| s.track_ir_setting.auto_disable_track_ir = v,
        );
    }

    fn windowed_mode(&mut self, ui: &mut Ui, core: &Arc<Core>) {
        setting_bool(
            ui,
            core,
            "Auto Resize MSFS Game Window (Used with Windowed Display Mode only)",
            "Enable automatic resize of the MSFS game window when using Windowed Display Mode. \
             The game window is resized to match the original size and location when the panel \
             profile was initially defined.",
            |s| s.windowed_mode_setting.auto_resize_msfs_game_window,
            |s, v| s.windowed_mode_setting.auto_resize_msfs_game_window = v,
        );
    }

    fn dynamic_lod(&mut self, ui: &mut Ui, core: &Arc<Core>) {
        ui.horizontal(|ui| {
            ui.colored_label(theme::ERROR, "(Unsupported Feature)");
            heading(ui, "Dynamically adjust Terrain and Object Level of Details");
        });
        ui.add_space(4.0);
        setting_bool(
            ui,
            core,
            "",
            "Enable automatic adjustments of TLOD and OLOD to get the desired targeted frame rate (FPS).",
            |s| s.dynamic_lod_setting.is_enabled,
            |s, v| {
                s.dynamic_lod_setting.is_enabled = v;
            },
        );
        if !core.read(|s| s.settings.dynamic_lod_setting.is_enabled) {
            return;
        }

        ui.indent("lod_options", |ui| {
            sim_values(ui, &core.read(|s| s.sim.dynamic_lod.clone()));
            self.section(ui, "General Options");
            lod_int(
                ui,
                core,
                "Targeted FPS",
                |s| s.dynamic_lod_setting.targeted_fps,
                |s, v| s.dynamic_lod_setting.targeted_fps = v,
                1..=240,
            );
            lod_int(
                ui,
                core,
                "FPS Tolerance",
                |s| s.dynamic_lod_setting.fps_tolerance,
                |s, v| s.dynamic_lod_setting.fps_tolerance = v,
                0..=60,
            );
            setting_bool(
                ui,
                core,
                "Pause When MSFS Loses Focus",
                "",
                |s| s.dynamic_lod_setting.pause_when_msfs_lose_focus,
                |s, v| s.dynamic_lod_setting.pause_when_msfs_lose_focus = v,
            );
            setting_bool(
                ui,
                core,
                "Pause Outside Cockpit View",
                "",
                |s| s.dynamic_lod_setting.pause_outside_cockpit_view,
                |s, v| s.dynamic_lod_setting.pause_outside_cockpit_view = v,
            );

            self.section(ui, "TLOD Options");
            lod_int(
                ui,
                core,
                "TLOD Min",
                |s| s.dynamic_lod_setting.tlod_min,
                |s, v| s.dynamic_lod_setting.tlod_min = v,
                10..=200,
            );
            lod_int(
                ui,
                core,
                "TLOD Max",
                |s| s.dynamic_lod_setting.tlod_max,
                |s, v| s.dynamic_lod_setting.tlod_max = v,
                10..=400,
            );
            lod_int(
                ui,
                core,
                "Altitude TLOD Base",
                |s| s.dynamic_lod_setting.alt_tlod_base,
                |s, v| s.dynamic_lod_setting.alt_tlod_base = v,
                0..=100000,
            );
            setting_bool(
                ui,
                core,
                "Use TLOD Min On Ground",
                "",
                |s| s.dynamic_lod_setting.tlod_min_on_ground,
                |s, v| s.dynamic_lod_setting.tlod_min_on_ground = v,
            );

            self.section(ui, "OLOD Options");
            lod_int(
                ui,
                core,
                "OLOD Base",
                |s| s.dynamic_lod_setting.olod_base,
                |s, v| s.dynamic_lod_setting.olod_base = v,
                10..=400,
            );
            lod_int(
                ui,
                core,
                "OLOD Top",
                |s| s.dynamic_lod_setting.olod_top,
                |s, v| s.dynamic_lod_setting.olod_top = v,
                10..=400,
            );
            lod_int(
                ui,
                core,
                "Altitude OLOD Base",
                |s| s.dynamic_lod_setting.alt_olod_base,
                |s, v| s.dynamic_lod_setting.alt_olod_base = v,
                0..=100000,
            );
            lod_int(
                ui,
                core,
                "Altitude OLOD Top",
                |s| s.dynamic_lod_setting.alt_olod_top,
                |s, v| s.dynamic_lod_setting.alt_olod_top = v,
                0..=100000,
            );

            self.section(ui, "Cloud Options");
            lod_int(
                ui,
                core,
                "Cloud Recovery TLOD",
                |s| s.dynamic_lod_setting.cloud_recovery_tlod,
                |s, v| s.dynamic_lod_setting.cloud_recovery_tlod = v,
                10..=400,
            );
            setting_bool(
                ui,
                core,
                "Decrease Cloud Quality",
                "",
                |s| s.dynamic_lod_setting.decrease_cloud_quality,
                |s, v| s.dynamic_lod_setting.decrease_cloud_quality = v,
            );

            self.section(ui, "Reset TLOD and OLOD on Exit");
            setting_bool(
                ui,
                core,
                "Enable Reset",
                "",
                |s| s.dynamic_lod_setting.reset_enabled,
                |s, v| s.dynamic_lod_setting.reset_enabled = v,
            );
            if core.read(|s| s.settings.dynamic_lod_setting.reset_enabled) {
                lod_int(
                    ui,
                    core,
                    "Reset TLOD",
                    |s| s.dynamic_lod_setting.reset_tlod,
                    |s, v| s.dynamic_lod_setting.reset_tlod = v,
                    10..=400,
                );
                lod_int(
                    ui,
                    core,
                    "Reset OLOD",
                    |s| s.dynamic_lod_setting.reset_olod,
                    |s, v| s.dynamic_lod_setting.reset_olod = v,
                    10..=400,
                );
            }
        });
    }
}

/// One labelled boolean application setting; applied immediately on change.
fn setting_bool(
    ui: &mut Ui,
    core: &Arc<Core>,
    title: &str,
    description: &str,
    get: impl Fn(&ApplicationSetting) -> bool,
    set: impl Fn(&mut ApplicationSetting, bool),
) {
    if !title.is_empty() {
        heading(ui, title);
        ui.add_space(2.0);
    }
    let mut value = core.read(|s| get(&s.settings));
    let changed = toggle_labeled(ui, &mut value, "").changed();
    if !description.is_empty() {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(42.0);
            ui.label(RichText::new(description).small().weak());
        });
    }
    if changed {
        core.write(|s| set(&mut s.settings, value));
    }
    ui.add_space(10.0);
}

/// The read-only "Sim Values" group shown at the top of the Dynamic LOD settings.
fn sim_values(ui: &mut Ui, sim: &DynamicLodSimData) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(RichText::new("Sim Values").small());
        ui.horizontal(|ui| {
            let values = [
                ("FPS", sim.fps.to_string()),
                ("TLOD", sim.tlod.to_string()),
                ("OLOD", sim.olod.to_string()),
                ("AGL", format!("{:.0}", sim.agl)),
                ("Clouds", sim.cloud_quality.clone()),
            ];
            for (caption, value) in values {
                ui.vertical(|ui| {
                    ui.label(RichText::new(caption).small().weak());
                    ui.label(RichText::new(value).monospace());
                });
                ui.add_space(14.0);
            }
        });
    });
    ui.add_space(10.0);
}

/// A 0-9 combo box bound to a key-binding string setting. An empty setting shows as "0".
fn digit_key_combo(
    ui: &mut Ui,
    core: &Arc<Core>,
    id: &str,
    get: impl Fn(&ApplicationSetting) -> String,
    set: impl Fn(&mut ApplicationSetting, String),
) {
    let key = core.read(|s| get(&s.settings));
    let mut selected = key.clone();
    egui::ComboBox::from_id_salt(id)
        .selected_text(if key.is_empty() { "0" } else { &key })
        .width(50.0)
        .show_ui(ui, |ui| {
            for digit in 0..=9 {
                let text = digit.to_string();
                ui.selectable_value(&mut selected, text.clone(), text);
            }
        });
    if selected != key {
        core.write(|s| set(&mut s.settings, selected));
    }
}

/// One labelled integer application setting with a drag value.
fn lod_int(
    ui: &mut Ui,
    core: &Arc<Core>,
    title: &str,
    get: impl Fn(&ApplicationSetting) -> i32,
    set: impl Fn(&mut ApplicationSetting, i32),
    range: std::ops::RangeInclusive<i32>,
) {
    ui.horizontal(|ui| {
        ui.label(format!("{title}:"));
        let mut value = core.read(|s| get(&s.settings));
        if ui
            .add(DragValue::new(&mut value).range(range).speed(1.0))
            .changed()
        {
            core.write(|s| set(&mut s.settings, value));
        }
    });
}
