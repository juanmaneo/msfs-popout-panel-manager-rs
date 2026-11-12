//! Application core: shared state plus the worker threads that orchestrate the simulator,
//! the input hooks and the pop out workflows.
//!
//! Threading rules:
//! * [`State`] lives behind one mutex. **Never hold the lock across a Win32 call, a sleep or a
//!   SimConnect command** - the GUI thread takes the same lock and Win32 calls on our own
//!   windows are synchronous with the GUI thread.
//! * Long running workflows (pop out, editing panel sources, panel property changes) run on
//!   short lived worker threads and use [`Core::read`] / [`Core::write`] for brief accesses.
//! * One "core loop" thread processes simulator and hook events, keeps the set of installed
//!   hooks in sync with the state, and autosaves the data files.

pub mod config;
mod flightsim;
mod popout;
mod profile;
mod source;
mod winevents;

use crate::flight_sim::{FlightDetector, FlightSimData};
use crate::fps::FpsCalc;
use crate::hook_plan::{self, HookPlan, PlanInput};
use crate::keybinding::KeyCapture;
use crate::lod::DynamicLod;
use crate::model::{ApplicationSetting, PanelHandle, UserProfile};
use crate::simconnect::{self, SimEvent, SimHandle};
use crate::status::StatusLog;
use crate::storage;
use crate::touch::collect_touch_panels;
use crate::win::hooks::{HookCmd, HookEvent, HookHandle};
use crate::win::process::ProcessMemory;
use crate::win::touch_hook::{self, TouchSnapshot};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use uuid::Uuid;

pub use source::fixed_camera_configs;

/// Which size/kind of progress window to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    PopOut,
    Regular,
}

/// What the next detected keystroke is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeystrokeTarget {
    /// Floating panel toggle key of a panel.
    Panel(Uuid),
    /// The "start pop out" shortcut in the preferences.
    Shortcut,
}

/// UI related runtime state (never persisted).
#[derive(Debug, Default)]
pub struct UiState {
    pub hud_bar_open: bool,
    pub num_pad_open: bool,
    pub message: Option<MessageKind>,
    pub keystroke: Option<KeystrokeTarget>,
    /// The application should close (simulator exited with auto close enabled).
    pub exit_requested: bool,
    /// The main window was minimized for the pop out and should be restored afterwards.
    pub minimized_for_pop_out: bool,
    /// Last problem reported by SimConnect (e.g. missing DLL).
    pub sim_error: Option<String>,
}

pub struct State {
    pub settings: ApplicationSetting,
    /// Always sorted by lower-cased name.
    pub profiles: Vec<UserProfile>,
    pub active: Option<Uuid>,
    pub sim: FlightSimData,
    pub ui: UiState,
    /// Panel waiting for the user to click its panel source point.
    pub selecting_panel: Option<Uuid>,
    /// A pop out is running (or about to).
    pub pop_out_in_progress: bool,
    /// Panel source editing session is active (workers must not start a second one).
    pub editing_lock: bool,
}

impl State {
    pub fn active_profile(&self) -> Option<&UserProfile> {
        let id = self.active?;
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn active_profile_mut(&mut self) -> Option<&mut UserProfile> {
        let id = self.active?;
        self.profiles.iter_mut().find(|p| p.id == id)
    }

    pub fn is_aircraft_bound_to_active_profile(&self) -> bool {
        match (self.active_profile(), &self.sim.aircraft_name) {
            (Some(p), Some(a)) => p.aircraft_bindings.iter().any(|b| b == a),
            _ => false,
        }
    }

    /// Whether the current aircraft may be bound to the active profile (it is not bound
    /// to a different profile yet).
    pub fn is_allowed_add_aircraft_binding(&self) -> bool {
        let Some(active) = self.active_profile() else {
            return true;
        };
        let Some(name) = self.sim.aircraft_name.as_deref().filter(|n| !n.is_empty()) else {
            return true;
        };
        match self
            .profiles
            .iter()
            .find(|p| p.aircraft_bindings.iter().any(|b| b == name))
        {
            Some(p) if p.id != active.id => false,
            _ => true,
        }
    }
}

pub(crate) enum CoreMsg {
    Sim(SimEvent),
    Hook(HookEvent),
    Wake,
    Shutdown,
}

pub(crate) enum WinMsg {
    Event {
        event: u32,
        hwnd: isize,
    },
    /// Hooks were re-installed: forget transient per-window state.
    Reset,
    Shutdown,
}

/// Flags remembering what the pop out workflow switched on/off in the simulator.
#[derive(Debug, Default)]
pub(crate) struct SimFlags {
    pub turned_on_power: bool,
    pub turned_on_avionics: bool,
    pub power_on_for_pop_out: bool,
    pub avionics_on_for_pop_out: bool,
    pub track_ir_managed: bool,
}

pub(crate) struct LodRuntime {
    pub lod: DynamicLod,
    pub mem: Option<ProcessMemory>,
    pub last_pause: Instant,
    pub paused_previously: bool,
    pub last_sample: Instant,
}

pub struct Core {
    state: Mutex<State>,
    pub status: StatusLog,
    pub sim: SimHandle,
    pub hooks: HookHandle,
    ctx: Mutex<Option<eframe::egui::Context>>,
    app_hwnd: AtomicIsize,
    tx: Sender<CoreMsg>,
    win_tx: Sender<WinMsg>,
    pub(crate) lod: Mutex<LodRuntime>,
    detector: Mutex<FlightDetector>,
    fps: Mutex<FpsCalc>,
    current_fps: AtomicI32,
    pub(crate) flags: Mutex<SimFlags>,
    shutting_down: AtomicBool,
}

impl Core {
    /// Load the data files and start all background threads.
    pub fn new() -> Arc<Core> {
        let settings = storage::read_app_setting().unwrap_or_default();
        let roaming = settings.general_setting.use_application_data_path;
        crate::logger::init(storage::log_dir(roaming));

        let profiles = storage::read_profiles(roaming);
        let mut state = State {
            settings,
            profiles,
            active: None,
            sim: FlightSimData::default(),
            ui: UiState::default(),
            selecting_panel: None,
            pop_out_in_progress: false,
            editing_lock: false,
        };
        profile::select_initial_profile(&mut state);

        let (tx, rx) = channel();
        let (win_tx, win_rx) = channel();

        let hook_tx = tx.clone();
        let hook_win_tx = win_tx.clone();
        let hooks = HookHandle::spawn(move |e| match e {
            HookEvent::WinEvent { event, hwnd } => {
                let _ = hook_win_tx.send(WinMsg::Event { event, hwnd });
            }
            other => {
                let _ = hook_tx.send(CoreMsg::Hook(other));
            }
        });

        let sim_tx = tx.clone();
        let sim = simconnect::spawn(move |e| {
            let _ = sim_tx.send(CoreMsg::Sim(e));
        });

        let core = Arc::new(Core {
            state: Mutex::new(state),
            status: StatusLog::new(),
            sim,
            hooks,
            ctx: Mutex::new(None),
            app_hwnd: AtomicIsize::new(0),
            tx,
            win_tx,
            lod: Mutex::new(LodRuntime {
                lod: DynamicLod::new(),
                mem: None,
                last_pause: Instant::now(),
                paused_previously: true,
                last_sample: Instant::now(),
            }),
            detector: Mutex::new(FlightDetector::default()),
            fps: Mutex::new(FpsCalc::new()),
            current_fps: AtomicI32::new(0),
            flags: Mutex::new(SimFlags::default()),
            shutting_down: AtomicBool::new(false),
        });

        let c = core.clone();
        std::thread::Builder::new()
            .name("popm-core".into())
            .spawn(move || c.run_loop(rx))
            .expect("spawn core thread");

        let c = core.clone();
        std::thread::Builder::new()
            .name("popm-winevents".into())
            .spawn(move || c.run_win_event_worker(win_rx))
            .expect("spawn window event thread");

        core
    }

    // --- state access ---------------------------------------------------------------

    pub fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn read<R>(&self, f: impl FnOnce(&State) -> R) -> R {
        f(&self.lock())
    }

    pub fn write<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        f(&mut self.lock())
    }

    // --- application window ---------------------------------------------------------

    /// Called by the UI once the main window exists.
    pub fn set_app_hwnd(&self, h: isize) {
        self.app_hwnd.store(h, Ordering::SeqCst);
    }

    pub fn app_hwnd(&self) -> isize {
        self.app_hwnd.load(Ordering::SeqCst)
    }

    pub fn set_egui_context(&self, ctx: eframe::egui::Context) {
        *self.ctx.lock().unwrap_or_else(|e| e.into_inner()) = Some(ctx);
    }

    /// Ask the GUI to redraw soon (used after state changes made by background threads).
    pub fn repaint(&self) {
        if let Some(ctx) = self.ctx.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            ctx.request_repaint();
        }
    }

    // --- shutdown ---------------------------------------------------------------------

    /// Stop background threads (hooks are removed, lod is reset, data saved).
    pub fn shutdown(&self) {
        if self.shutting_down.swap(true, Ordering::SeqCst) {
            return;
        }
        let _ = self.tx.send(CoreMsg::Shutdown);
        let _ = self.win_tx.send(WinMsg::Shutdown);
        // give the core loop a moment to flush the data files
        std::thread::sleep(Duration::from_millis(300));
    }

    // --- the core loop ----------------------------------------------------------------

    fn run_loop(self: Arc<Self>, rx: Receiver<CoreMsg>) {
        let mut applied = HookPlan::default();
        let mut applied_hwnds: Vec<isize> = Vec::new();
        let mut applied_touch: Option<TouchSnapshot> = None;
        let mut capture = KeyCapture::new();
        let mut saver = AutoSaver::new(&self);

        let mut last_tick = Instant::now();
        let mut last_save = Instant::now();
        let mut last_exit_check = Instant::now();

        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(CoreMsg::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(CoreMsg::Sim(e)) => self.handle_sim_event(e),
                Ok(CoreMsg::Hook(e)) => self.handle_hook_event(e, &mut capture),
                Ok(CoreMsg::Wake) | Err(RecvTimeoutError::Timeout) => {}
            }

            let now = Instant::now();
            if now.duration_since(last_tick) >= Duration::from_millis(100) {
                last_tick = now;
                self.reconcile_hooks(&mut applied, &mut applied_hwnds, &mut applied_touch);
                self.maintain_dynamic_lod();
                self.sync_auxiliary_windows();
            }
            if now.duration_since(last_save) >= Duration::from_millis(500) {
                last_save = now;
                saver.save_if_changed(&self);
            }
            if now.duration_since(last_exit_check) >= Duration::from_secs(3) {
                last_exit_check = now;
                self.check_simulator_exit();
            }
        }

        saver.save_if_changed(&self);
        self.detach_dynamic_lod();
        self.hooks.send(HookCmd::Shutdown);
        self.sim.send(simconnect::SimCmd::Shutdown);
    }

    /// Install/remove hooks so that they match what the current state needs.
    fn reconcile_hooks(
        &self,
        applied: &mut HookPlan,
        hwnds: &mut Vec<isize>,
        touch: &mut Option<TouchSnapshot>,
    ) {
        let (plan, panel_hwnds, snapshot) = self.read(|s| {
            let profile = s.active_profile();
            let input = PlanInput {
                selecting_panel: s.selecting_panel,
                detecting_keystroke: s.ui.keystroke.is_some(),
            };
            let plan = hook_plan::compute(&s.settings, profile, input);
            let panel_hwnds: Vec<isize> = profile
                .map(|p| {
                    p.panel_configs
                        .iter()
                        .filter_map(|c| c.panel_handle.hwnd())
                        .collect()
                })
                .unwrap_or_default();
            let snapshot = TouchSnapshot {
                panels: profile.map(collect_touch_panels).unwrap_or_default(),
                touch_down_up_delay_ms: s.settings.touch_setting.touch_down_up_delay.max(0) as u64,
                refocus_enabled: s.settings.refocus_setting.refocus_game_window.is_enabled,
                refocus_delay_ms: (s.settings.refocus_setting.refocus_game_window.delay * 1000.0)
                    .max(0.0) as u64,
            };
            (plan, panel_hwnds, snapshot)
        });

        if *hwnds != panel_hwnds {
            crate::win::hooks::set_panel_hwnds(panel_hwnds.clone());
            *hwnds = panel_hwnds;
        }

        if touch.as_ref() != Some(&snapshot) {
            touch_hook::set_snapshot(snapshot.clone());
            *touch = Some(snapshot);
        }

        if plan.keyboard != applied.keyboard {
            self.hooks.send(HookCmd::Keyboard(plan.keyboard));
        }
        if plan.click_capture != applied.click_capture {
            self.hooks
                .send(HookCmd::MouseClickCapture(plan.click_capture));
        }
        if plan.touch != applied.touch {
            self.hooks.send(HookCmd::Touch(plan.touch));
        }
        if plan.win_event != applied.win_event {
            self.hooks.send(HookCmd::WinEvent(plan.win_event));
            let _ = self.win_tx.send(WinMsg::Reset);
        }
        *applied = plan;
    }

    /// Detect that MSFS went away and optionally close ourselves.
    fn check_simulator_exit(&self) {
        let started = self.read(|s| s.sim.is_simulator_started);
        if !started || crate::win::process::is_sim_running() {
            return;
        }
        self.write(|s| {
            if s.settings.general_setting.auto_close {
                s.ui.exit_requested = true;
            } else {
                s.sim.reset();
                s.sim.is_sim_connect_active = false;
            }
        });
        self.repaint();
    }
}

/// Writes the data files when their serialized form changed (replacing upstream's
/// property-changed driven saving).
struct AutoSaver {
    profiles_json: String,
    settings_json: String,
    roaming: bool,
}

impl AutoSaver {
    fn new(core: &Core) -> Self {
        core.read(|s| Self {
            profiles_json: storage::profiles_to_json(&s.profiles),
            settings_json: storage::app_setting_to_json(&s.settings),
            roaming: s.settings.general_setting.use_application_data_path,
        })
    }

    fn save_if_changed(&mut self, core: &Core) {
        let (profiles_json, settings_json, roaming, settings, profiles) = core.read(|s| {
            let pj = storage::profiles_to_json(&s.profiles);
            let sj = storage::app_setting_to_json(&s.settings);
            let changed = pj != self.profiles_json || sj != self.settings_json;
            let r = s.settings.general_setting.use_application_data_path;
            if changed || r != self.roaming {
                (
                    pj,
                    sj,
                    r,
                    Some(s.settings.clone()),
                    Some(s.profiles.clone()),
                )
            } else {
                (pj, sj, r, None, None)
            }
        });
        let (Some(settings), Some(profiles)) = (settings, profiles) else {
            return;
        };

        if roaming != self.roaming {
            storage::move_data(&settings, &profiles);
            crate::logger::set_dir(storage::log_dir(roaming));
        } else {
            let dir = storage::data_dir(roaming);
            if profiles_json != self.profiles_json {
                if let Err(e) = storage::write_profiles_to(&dir, &profiles) {
                    log::error!("Unable to write user data file: {e}");
                }
            }
            if settings_json != self.settings_json {
                if let Err(e) = storage::write_app_setting_to(&dir, &settings) {
                    log::error!("Unable to write app setting data file: {e}");
                }
            }
        }
        self.profiles_json = profiles_json;
        self.settings_json = settings_json;
        self.roaming = roaming;
    }
}

/// Convenience used by workflows: current handle of a panel if it was popped out.
pub(crate) fn panel_hwnd(profile: &UserProfile, panel_id: Uuid) -> Option<isize> {
    profile.panel(panel_id).and_then(|p| match p.panel_handle {
        PanelHandle::Hwnd(h) => Some(h),
        _ => None,
    })
}
