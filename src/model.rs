//! Data model: user profiles and application settings.
//!
//! The JSON layout is intentionally compatible with the upstream C# application
//! (`userprofiledata.json` / `appsettingdata.json`, file version 4.0): PascalCase
//! keys, enums stored as integers, GUIDs as lowercase hyphenated strings.

use serde::{Deserialize, Deserializer, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use uuid::Uuid;

/// Deserialize `null` (or a missing value) as `T::default()`.
fn null_default<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize_repr, Deserialize_repr, Default)]
#[repr(i32)]
pub enum PanelType {
    PopOutManager = 0,
    BuiltInPopout = 1,
    #[default]
    CustomPopout = 2,
    FlightSimMainWindow = 3,
    HudBarWindow = 4,
    MultiMonitorWindow = 5,
    PanelSourceWindow = 6,
    StatusMessageWindow = 7,
    RefocusDisplay = 8,
    NumPadWindow = 9,
    Unknown = 100,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize_repr, Deserialize_repr, Default)]
#[repr(i32)]
pub enum CameraType {
    #[default]
    Cockpit = 1,
    Instrument = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize_repr, Deserialize_repr, Default)]
#[repr(i32)]
pub enum HudBarType {
    None = 0,
    #[default]
    GenericAircraft = 1,
    Pmdg737 = 2,
}

impl HudBarType {
    pub const SELECTABLE: [HudBarType; 2] = [HudBarType::GenericAircraft, HudBarType::Pmdg737];

    pub fn label(self) -> &'static str {
        match self {
            HudBarType::None => "None",
            HudBarType::GenericAircraft => "Generic Aircraft",
            HudBarType::Pmdg737 => "PMDG 737",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize_repr, Deserialize_repr, Default)]
#[repr(i32)]
pub enum AfterPopOutCameraViewType {
    #[default]
    CockpitCenterView = 0,
    CustomCameraView = 1,
}

// ---------------------------------------------------------------------------
// Runtime-only panel handle
// ---------------------------------------------------------------------------

/// Result of popping a panel out. Mirrors the upstream `IntPtr` convention:
/// `IntPtr.MaxValue` = not attempted yet, `IntPtr.Zero` = failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelHandle {
    #[default]
    Unset,
    Failed,
    Hwnd(isize),
}

impl PanelHandle {
    pub fn hwnd(self) -> Option<isize> {
        match self {
            PanelHandle::Hwnd(h) => Some(h),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Panel config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct PanelSource {
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct FixedCameraConfig {
    pub id: i32,
    pub camera_type: CameraType,
    pub camera_index: i32,
    #[serde(deserialize_with = "null_default")]
    pub name: String,
}

impl Default for FixedCameraConfig {
    fn default() -> Self {
        Self {
            id: 0,
            camera_type: CameraType::Cockpit,
            camera_index: 1,
            name: "Cockpit Pilot".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct FloatingPanel {
    pub is_enabled: bool,
    pub keyboard_binding: Option<String>,
    pub is_hidden_on_start: bool,
    #[serde(skip)]
    pub is_detecting_keystroke: bool,
}

impl FloatingPanel {
    pub fn has_keyboard_binding(&self) -> bool {
        self.keyboard_binding.as_deref().is_some_and(|s| !s.is_empty())
    }

    /// Enable/disable the floating panel feature, clearing dependent fields on disable.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.is_enabled = enabled;
        if !enabled {
            self.keyboard_binding = None;
            self.is_hidden_on_start = false;
            self.is_detecting_keystroke = false;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct PanelConfig {
    pub id: Uuid,
    pub panel_type: PanelType,
    #[serde(deserialize_with = "null_default")]
    pub panel_name: String,
    pub top: i32,
    pub left: i32,
    pub width: i32,
    pub height: i32,
    pub always_on_top: bool,
    pub hide_titlebar: bool,
    pub full_screen: bool,
    pub touch_enabled: bool,
    pub auto_game_refocus: bool,
    pub floating_panel: FloatingPanel,
    pub panel_source: PanelSource,
    pub fixed_camera_config: FixedCameraConfig,

    #[serde(skip)]
    pub panel_handle: PanelHandle,
    #[serde(skip)]
    pub is_editing_panel: bool,
    #[serde(skip)]
    pub is_selected_panel_source: bool,
    #[serde(skip)]
    pub is_shown_panel_source: bool,
    #[serde(skip, default = "default_true")]
    pub is_floating: bool,
    /// Host monitor of a full screen panel (used for touch hit testing).
    #[serde(skip)]
    pub full_screen_monitor: Option<MonitorInfo>,
}

fn default_true() -> bool {
    true
}

impl Default for PanelConfig {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            panel_type: PanelType::CustomPopout,
            panel_name: "Default Panel Name".into(),
            top: 0,
            left: 0,
            width: 0,
            height: 0,
            always_on_top: false,
            hide_titlebar: false,
            full_screen: false,
            touch_enabled: false,
            auto_game_refocus: true,
            floating_panel: FloatingPanel::default(),
            panel_source: PanelSource::default(),
            fixed_camera_config: FixedCameraConfig::default(),
            panel_handle: PanelHandle::Unset,
            is_editing_panel: false,
            is_selected_panel_source: false,
            is_shown_panel_source: false,
            is_floating: true,
            full_screen_monitor: None,
        }
    }
}

impl PanelConfig {
    pub fn new(panel_type: PanelType, name: &str) -> Self {
        Self {
            panel_type,
            panel_name: name.to_string(),
            ..Self::default()
        }
    }

    /// Deep copy with a fresh id and all runtime state cleared (used when copying profiles).
    pub fn copy_for_new_profile(&self) -> Self {
        let mut c = self.clone();
        c.id = Uuid::new_v4();
        c.panel_handle = PanelHandle::Unset;
        c.is_editing_panel = false;
        c.is_selected_panel_source = false;
        c.is_shown_panel_source = false;
        c.floating_panel.is_detecting_keystroke = false;
        c.full_screen_monitor = None;
        c
    }

    pub fn has_panel_source(&self) -> bool {
        self.panel_type == PanelType::BuiltInPopout
            || (self.panel_type == PanelType::CustomPopout && self.panel_source.x.is_some())
    }

    /// `None` = not attempted, `Some(true)` = popped out, `Some(false)` = failed.
    pub fn is_pop_out_success(&self) -> Option<bool> {
        match self.panel_handle {
            PanelHandle::Unset => None,
            PanelHandle::Failed => Some(false),
            PanelHandle::Hwnd(_) => Some(true),
        }
    }

    pub fn is_deletable(&self) -> bool {
        !matches!(
            self.panel_type,
            PanelType::HudBarWindow | PanelType::RefocusDisplay | PanelType::NumPadWindow
        )
    }

    pub fn is_touch_enable_panel(&self) -> bool {
        self.is_deletable()
    }

    pub fn is_custom_pop_out(&self) -> bool {
        self.panel_type == PanelType::CustomPopout
    }

    pub fn source_coordinate_text(&self) -> String {
        match (self.panel_source.x, self.panel_source.y) {
            (Some(x), Some(y)) => format!("Left: {x} / Top: {y}"),
            _ => "top: N/A, left: N/A".to_string(),
        }
    }

    // Invariants that upstream enforces through PropertyChanged handlers.

    pub fn set_full_screen(&mut self, value: bool) {
        self.full_screen = value;
        if value {
            self.always_on_top = false;
            self.hide_titlebar = false;
        }
    }

    pub fn set_touch_enabled(&mut self, value: bool) {
        self.touch_enabled = value;
        if value {
            self.auto_game_refocus = true;
        }
    }
}

// ---------------------------------------------------------------------------
// Profile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct MonitorInfo {
    #[serde(deserialize_with = "null_default")]
    pub name: String,
    pub is_selected: bool,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct HudBarConfig {
    pub is_enabled: bool,
    pub hud_bar_type: HudBarType,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct NumPadConfig {
    pub is_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct RefocusOnDisplay {
    pub is_enabled: bool,
    #[serde(deserialize_with = "null_default")]
    pub monitors: Vec<MonitorInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct ProfileSetting {
    pub power_on_required_for_cold_start: bool,
    pub include_in_game_panels: bool,
    pub hud_bar_config: HudBarConfig,
    pub refocus_on_display: RefocusOnDisplay,
    pub num_pad_config: NumPadConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct MsfsGameWindowConfig {
    pub top: i32,
    pub left: i32,
    pub width: i32,
    pub height: i32,
}

impl MsfsGameWindowConfig {
    pub fn is_valid(&self) -> bool {
        self.width != 0 && self.height != 0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct UserProfile {
    pub id: Uuid,
    #[serde(deserialize_with = "null_default")]
    pub name: String,
    pub is_locked: bool,
    #[serde(deserialize_with = "null_default")]
    pub aircraft_bindings: Vec<String>,
    #[serde(deserialize_with = "null_default")]
    pub panel_configs: Vec<PanelConfig>,
    pub profile_setting: ProfileSetting,
    pub msfs_game_window_config: MsfsGameWindowConfig,
    pub panel_source_cockpit_zoom_factor: i32,
    pub is_used_legacy_camera_system: bool,

    #[serde(skip)]
    pub is_editing_panel_source: bool,
    #[serde(skip)]
    pub is_selecting_panel_source: bool,
    #[serde(skip)]
    pub is_popped_out: bool,
    #[serde(skip)]
    pub current_move_resize_panel_id: Uuid,
}

impl Default for UserProfile {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            is_locked: false,
            aircraft_bindings: Vec::new(),
            panel_configs: Vec::new(),
            profile_setting: ProfileSetting::default(),
            msfs_game_window_config: MsfsGameWindowConfig::default(),
            panel_source_cockpit_zoom_factor: 50,
            is_used_legacy_camera_system: true,
            is_editing_panel_source: false,
            is_selecting_panel_source: false,
            is_popped_out: false,
            current_move_resize_panel_id: Uuid::nil(),
        }
    }
}

impl UserProfile {
    /// A brand new profile as created from the UI (new profiles use the fixed camera system).
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            is_used_legacy_camera_system: false,
            ..Self::default()
        }
    }

    /// Copy of `source` under a new name (panel ids regenerated, runtime state cleared).
    pub fn copy_of(name: &str, source: &UserProfile) -> Self {
        Self {
            name: name.to_string(),
            panel_configs: source
                .panel_configs
                .iter()
                .map(PanelConfig::copy_for_new_profile)
                .collect(),
            profile_setting: source.profile_setting.clone(),
            msfs_game_window_config: source.msfs_game_window_config.clone(),
            panel_source_cockpit_zoom_factor: source.panel_source_cockpit_zoom_factor,
            is_used_legacy_camera_system: source.is_used_legacy_camera_system,
            ..Self::default()
        }
    }

    pub fn sort_key(&self) -> String {
        self.name.to_lowercase()
    }

    pub fn has_unidentified_panel_source(&self) -> bool {
        self.panel_configs
            .iter()
            .any(|p| p.panel_type == PanelType::CustomPopout && p.panel_source.x.is_none())
    }

    pub fn has_custom_panels(&self) -> bool {
        self.panel_configs
            .iter()
            .any(|p| p.panel_type == PanelType::CustomPopout)
    }

    pub fn has_floating_panel(&self) -> bool {
        self.panel_configs.iter().any(|p| p.floating_panel.is_enabled)
    }

    pub fn panel(&self, id: Uuid) -> Option<&PanelConfig> {
        self.panel_configs.iter().find(|p| p.id == id)
    }

    pub fn panel_mut(&mut self, id: Uuid) -> Option<&mut PanelConfig> {
        self.panel_configs.iter_mut().find(|p| p.id == id)
    }

    pub fn panel_by_handle(&self, hwnd: isize) -> Option<&PanelConfig> {
        self.panel_configs
            .iter()
            .find(|p| p.panel_handle == PanelHandle::Hwnd(hwnd))
    }

    /// Mark the profile as not popped out and reset every panel's runtime handle.
    pub fn set_popped_out(&mut self, value: bool) {
        self.is_popped_out = value;
        if !value {
            for p in &mut self.panel_configs {
                p.panel_handle = PanelHandle::Unset;
            }
        }
    }

    /// Reset all runtime state (used when switching/starting).
    pub fn reset_runtime(&mut self) {
        self.current_move_resize_panel_id = Uuid::nil();
        self.is_editing_panel_source = false;
        self.is_selecting_panel_source = false;
        self.is_popped_out = false;
        for p in &mut self.panel_configs {
            p.is_selected_panel_source = false;
            p.is_editing_panel = false;
            p.is_shown_panel_source = false;
            p.panel_handle = PanelHandle::Unset;
            p.floating_panel.is_detecting_keystroke = false;
        }
    }

    /// Next unused panel-source colour (see [`PANEL_COLORS`]).
    pub fn next_available_color(&self) -> String {
        for name in PANEL_COLORS {
            if self
                .panel_configs
                .iter()
                .any(|p| p.panel_source.color.as_deref() == Some(name.0))
            {
                continue;
            }
            return name.0.to_string();
        }
        "White".to_string()
    }

    // --- UI helpers mirroring ProfileCardViewModel -------------------------

    pub fn add_custom_panel(&mut self) -> Uuid {
        let mut p = PanelConfig::new(PanelType::CustomPopout, "Default Panel Name");
        p.panel_source.color = Some(self.next_available_color());
        let id = p.id;
        self.panel_configs.push(p);
        id
    }

    pub fn on_include_in_game_panels_changed(&mut self) {
        if !self.profile_setting.include_in_game_panels {
            self.panel_configs
                .retain(|p| p.panel_type != PanelType::BuiltInPopout);
        }
    }

    pub fn on_hud_bar_changed(&mut self) {
        if self.profile_setting.hud_bar_config.is_enabled {
            if self
                .panel_configs
                .iter()
                .any(|p| p.panel_type == PanelType::HudBarWindow)
            {
                return;
            }
            let mut p = PanelConfig::new(PanelType::HudBarWindow, "HUD Bar");
            p.width = 1920;
            p.height = 40;
            p.auto_game_refocus = false;
            self.panel_configs.push(p);
        } else {
            self.panel_configs
                .retain(|p| p.panel_type != PanelType::HudBarWindow);
        }
    }

    pub fn on_num_pad_changed(&mut self) {
        if self.profile_setting.num_pad_config.is_enabled {
            if self
                .panel_configs
                .iter()
                .any(|p| p.panel_type == PanelType::NumPadWindow)
            {
                return;
            }
            let mut p = PanelConfig::new(PanelType::NumPadWindow, "Virtual NumPad");
            p.auto_game_refocus = false;
            self.panel_configs.push(p);
        } else {
            self.panel_configs
                .retain(|p| p.panel_type != PanelType::NumPadWindow);
        }
    }

    /// `monitors` is the current list of physical monitors (sorted by name), used to
    /// populate the refocus list the first time the feature is enabled.
    pub fn on_refocus_display_changed(&mut self, monitors: &[MonitorInfo]) {
        let r = &mut self.profile_setting.refocus_on_display;
        if r.monitors.is_empty() {
            r.monitors = monitors.to_vec();
        }
        if !r.is_enabled {
            for m in &mut r.monitors {
                m.is_selected = false;
            }
            self.panel_configs
                .retain(|p| p.panel_type != PanelType::RefocusDisplay);
        }
    }

    pub fn on_refocus_display_refreshed(&mut self, monitors: &[MonitorInfo]) {
        self.profile_setting.refocus_on_display.monitors = monitors.to_vec();
        self.panel_configs
            .retain(|p| p.panel_type != PanelType::RefocusDisplay);
    }

    pub fn on_refocus_display_selection_changed(&mut self, monitor_name: &str) {
        let Some(m) = self
            .profile_setting
            .refocus_on_display
            .monitors
            .iter()
            .find(|m| m.name == monitor_name)
            .cloned()
        else {
            return;
        };
        if !m.is_selected {
            self.panel_configs.retain(|p| {
                !(p.panel_name == monitor_name && p.panel_type == PanelType::RefocusDisplay)
            });
        } else {
            let mut p = PanelConfig::new(PanelType::RefocusDisplay, monitor_name);
            p.left = m.x;
            p.top = m.y;
            p.width = m.width;
            p.height = m.height;
            p.touch_enabled = true;
            self.panel_configs.push(p);
        }
    }

    pub fn move_panel_up(&mut self, id: Uuid) {
        if let Some(i) = self.panel_configs.iter().position(|p| p.id == id)
            && i > 0 {
                self.panel_configs.swap(i, i - 1);
            }
    }

    pub fn move_panel_down(&mut self, id: Uuid) {
        if let Some(i) = self.panel_configs.iter().position(|p| p.id == id)
            && i + 1 < self.panel_configs.len() {
                self.panel_configs.swap(i, i + 1);
            }
    }
}

/// `(name, 0xRRGGBB)` — the enumeration order matters (first unused is assigned).
pub const PANEL_COLORS: [(&str, u32); 12] = [
    ("LimeGreen", 0x32CD32),
    ("Red", 0xFF0000),
    ("Blue", 0x0000FF),
    ("Fuchsia", 0xFF00FF),
    ("Orange", 0xFFA500),
    ("Yellow", 0xFFFF00),
    ("Cyan", 0x00FFFF),
    ("Ivory", 0xFFFFF0),
    ("Pink", 0xFFC0CB),
    ("Indigo", 0x4B0082),
    ("Purple", 0x800080),
    ("Crimson", 0xDC143C),
];

/// Resolve a named colour to `0xRRGGBB` (white if unknown).
pub fn color_rgb(name: Option<&str>) -> u32 {
    name.and_then(|n| PANEL_COLORS.iter().find(|c| c.0.eq_ignore_ascii_case(n)))
        .map(|c| c.1)
        .unwrap_or(0xFFFFFF)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct UserProfileFile {
    #[serde(deserialize_with = "null_default")]
    pub file_version: String,
    #[serde(deserialize_with = "null_default")]
    pub profiles: Vec<UserProfile>,
}

impl Default for UserProfileFile {
    fn default() -> Self {
        Self {
            file_version: "4.0".into(),
            profiles: Vec::new(),
        }
    }
}

/// Sort profiles by lower-cased name (ordinal), like the upstream sorted collection.
pub fn sort_profiles(profiles: &mut [UserProfile]) {
    profiles.sort_by_key(|p| p.sort_key());
}

// ---------------------------------------------------------------------------
// Application settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct GeneralSetting {
    pub always_on_top: bool,
    pub auto_close: bool,
    pub minimize_to_tray: bool,
    pub start_minimized: bool,
    pub check_for_update: bool,
    pub turbo_mode: bool,
    pub use_application_data_path: bool,
}

impl Default for GeneralSetting {
    fn default() -> Self {
        Self {
            always_on_top: true,
            auto_close: true,
            minimize_to_tray: false,
            start_minimized: false,
            check_for_update: true,
            turbo_mode: false,
            use_application_data_path: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct AutoPopOutSetting {
    pub is_enabled: bool,
    pub ready_to_fly_delay: i32,
}

impl Default for AutoPopOutSetting {
    fn default() -> Self {
        Self {
            is_enabled: true,
            ready_to_fly_delay: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct AutoPanning {
    pub is_enabled: bool,
    #[serde(deserialize_with = "null_default")]
    pub key_binding: String,
}

impl Default for AutoPanning {
    fn default() -> Self {
        Self {
            is_enabled: true,
            key_binding: "0".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct AfterPopOutCameraView {
    pub is_enabled: bool,
    pub camera_view: AfterPopOutCameraViewType,
    #[serde(deserialize_with = "null_default")]
    pub key_binding: String,
}

impl Default for AfterPopOutCameraView {
    fn default() -> Self {
        Self {
            is_enabled: true,
            camera_view: AfterPopOutCameraViewType::CockpitCenterView,
            key_binding: "1".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct PopOutTitleBarCustomization {
    pub is_enabled: bool,
    #[serde(deserialize_with = "null_default")]
    pub hex_color: String,
}

impl Default for PopOutTitleBarCustomization {
    fn default() -> Self {
        Self {
            is_enabled: false,
            hex_color: "000000".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct PopOutSetting {
    pub auto_panning: AutoPanning,
    pub after_pop_out_camera_view: AfterPopOutCameraView,
    pub minimize_during_pop_out: bool,
    pub minimize_after_pop_out: bool,
    pub use_left_right_control_to_pop_out: bool,
    pub enable_panel_reset_when_locked: bool,
    pub enable_pop_out_messages: bool,
    pub auto_active_pause: bool,
    pub pop_out_title_bar_customization: PopOutTitleBarCustomization,
}

impl Default for PopOutSetting {
    fn default() -> Self {
        Self {
            auto_panning: AutoPanning::default(),
            after_pop_out_camera_view: AfterPopOutCameraView::default(),
            minimize_during_pop_out: true,
            minimize_after_pop_out: false,
            use_left_right_control_to_pop_out: false,
            enable_panel_reset_when_locked: true,
            enable_pop_out_messages: true,
            auto_active_pause: false,
            pop_out_title_bar_customization: PopOutTitleBarCustomization::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct RefocusGameWindow {
    pub is_enabled: bool,
    pub delay: f64,
}

impl Default for RefocusGameWindow {
    fn default() -> Self {
        Self {
            is_enabled: true,
            delay: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct RefocusSetting {
    pub refocus_game_window: RefocusGameWindow,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct TouchSetting {
    pub touch_down_up_delay: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct TrackIrSetting {
    #[serde(rename = "AutoDisableTrackIR")]
    pub auto_disable_track_ir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct WindowedModeSetting {
    pub auto_resize_msfs_game_window: bool,
}

impl Default for WindowedModeSetting {
    fn default() -> Self {
        Self {
            auto_resize_msfs_game_window: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct SystemSetting {
    #[serde(deserialize_with = "null_default")]
    pub auto_updater_url: String,
    pub last_used_profile_id: Uuid,
}

impl Default for SystemSetting {
    fn default() -> Self {
        Self {
            auto_updater_url:
                "https://raw.githubusercontent.com/hawkeye-stan/msfs-popout-panel-manager/master/autoupdate.xml"
                    .into(),
            last_used_profile_id: Uuid::nil(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct KeyboardShortcutSetting {
    pub is_enabled: bool,
    /// Legacy (v4.0.3 and earlier). Read for migration, never written.
    #[serde(skip_serializing)]
    pub start_pop_out_key_binding: Option<String>,
    pub pop_out_keyboard_binding: Option<String>,
    #[serde(skip)]
    pub is_detecting_keystroke: bool,
}

impl Default for KeyboardShortcutSetting {
    fn default() -> Self {
        Self {
            is_enabled: true,
            start_pop_out_key_binding: None,
            pop_out_keyboard_binding: None,
            is_detecting_keystroke: false,
        }
    }
}

impl KeyboardShortcutSetting {
    /// Convert the legacy single-letter binding to the new `a|b|c` format.
    pub fn migrate_legacy(&mut self) {
        if let Some(v) = self.start_pop_out_key_binding.take()
            && !v.is_empty() {
                self.pop_out_keyboard_binding = Some(crate::keybinding::sort_binding(&[
                    "LeftCtrl".to_string(),
                    "LeftShift".to_string(),
                    v.to_uppercase(),
                ]));
            }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct DynamicLodSetting {
    pub is_enabled: bool,
    pub pause_when_msfs_lose_focus: bool,
    pub pause_outside_cockpit_view: bool,
    pub reset_enabled: bool,
    pub reset_tlod: i32,
    pub reset_olod: i32,
    pub targeted_fps: i32,
    pub fps_tolerance: i32,
    pub tlod_min_on_ground: bool,
    pub alt_tlod_base: i32,
    pub tlod_min: i32,
    pub tlod_max: i32,
    pub cloud_recovery_tlod: i32,
    pub decrease_cloud_quality: bool,
    pub olod_top: i32,
    pub olod_base: i32,
    pub alt_olod_base: i32,
    pub alt_olod_top: i32,
}

impl Default for DynamicLodSetting {
    fn default() -> Self {
        Self {
            is_enabled: false,
            pause_when_msfs_lose_focus: true,
            pause_outside_cockpit_view: true,
            reset_enabled: false,
            reset_tlod: 100,
            reset_olod: 100,
            targeted_fps: 60,
            fps_tolerance: 4,
            tlod_min_on_ground: true,
            alt_tlod_base: 1000,
            tlod_min: 50,
            tlod_max: 200,
            cloud_recovery_tlod: 100,
            decrease_cloud_quality: false,
            olod_top: 50,
            olod_base: 200,
            alt_olod_base: 1000,
            alt_olod_top: 10000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "PascalCase", default)]
pub struct ApplicationSetting {
    pub general_setting: GeneralSetting,
    pub auto_pop_out_setting: AutoPopOutSetting,
    pub pop_out_setting: PopOutSetting,
    pub refocus_setting: RefocusSetting,
    pub touch_setting: TouchSetting,
    #[serde(rename = "TrackIRSetting")]
    pub track_ir_setting: TrackIrSetting,
    pub windowed_mode_setting: WindowedModeSetting,
    pub system_setting: SystemSetting,
    pub keyboard_shortcut_setting: KeyboardShortcutSetting,
    pub dynamic_lod_setting: DynamicLodSetting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct AppSettingFile {
    #[serde(deserialize_with = "null_default")]
    pub file_version: String,
    pub application_setting: ApplicationSetting,
}

impl Default for AppSettingFile {
    fn default() -> Self {
        Self {
            file_version: "4.0".into(),
            application_setting: ApplicationSetting::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_profile_json_round_trips() {
        let json = r#"{
  "FileVersion": "4.0",
  "Profiles": [
    {
      "Id": "1b4e28ba-2fa1-11d2-883f-0016d3cca427",
      "Name": "Cessna 172",
      "IsLocked": true,
      "AircraftBindings": ["CESSNA 172 G1000"],
      "PanelConfigs": [
        {
          "Id": "6f9619ff-8b86-d011-b42d-00c04fc964ff",
          "PanelType": 2,
          "PanelName": "G1000 PFD (Custom)",
          "Top": 10, "Left": 20, "Width": 800, "Height": 600,
          "AlwaysOnTop": true, "HideTitlebar": true, "FullScreen": false,
          "TouchEnabled": true, "AutoGameRefocus": true,
          "FloatingPanel": {"IsEnabled": false, "KeyboardBinding": null, "IsHiddenOnStart": false},
          "PanelSource": {"X": 100, "Y": 200, "Color": "Red"},
          "FixedCameraConfig": {"Id": 0, "CameraType": 1, "CameraIndex": 1, "Name": "Cockpit Pilot"}
        }
      ],
      "ProfileSetting": {
        "PowerOnRequiredForColdStart": false,
        "IncludeInGamePanels": true,
        "HudBarConfig": {"IsEnabled": true, "HudBarType": 2},
        "RefocusOnDisplay": {"IsEnabled": false, "Monitors": []},
        "NumPadConfig": {"IsEnabled": false}
      },
      "MsfsGameWindowConfig": {"Top": 0, "Left": 0, "Width": 0, "Height": 0},
      "PanelSourceCockpitZoomFactor": 50,
      "IsUsedLegacyCameraSystem": false
    }
  ]
}"#;
        let f: UserProfileFile = serde_json::from_str(json).unwrap();
        assert_eq!(f.profiles.len(), 1);
        let p = &f.profiles[0];
        assert_eq!(p.name, "Cessna 172");
        assert!(p.is_locked);
        assert_eq!(p.panel_configs[0].panel_source.x, Some(100));
        assert_eq!(p.panel_configs[0].panel_source.color.as_deref(), Some("Red"));
        assert_eq!(p.profile_setting.hud_bar_config.hud_bar_type, HudBarType::Pmdg737);
        assert!(p.panel_configs[0].is_floating);
        assert_eq!(p.panel_configs[0].is_pop_out_success(), None);

        let out = serde_json::to_string(&f).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["Profiles"][0]["Id"], "1b4e28ba-2fa1-11d2-883f-0016d3cca427");
        assert_eq!(v["Profiles"][0]["PanelConfigs"][0]["PanelType"], 2);
        assert_eq!(v["Profiles"][0]["ProfileSetting"]["HudBarConfig"]["HudBarType"], 2);
        assert!(v["Profiles"][0].get("IsPoppedOut").is_none());
        assert!(v["Profiles"][0]["PanelConfigs"][0].get("PanelHandle").is_none());
    }

    #[test]
    fn missing_and_null_fields_use_defaults() {
        let json = r#"{"Profiles":[{"Name":null,"PanelConfigs":[{"PanelName":null}]}]}"#;
        let f: UserProfileFile = serde_json::from_str(json).unwrap();
        let p = &f.profiles[0];
        assert_eq!(p.name, "");
        assert!(p.is_used_legacy_camera_system);
        assert_eq!(p.panel_source_cockpit_zoom_factor, 50);
        assert_eq!(p.panel_configs[0].panel_name, "");
        assert!(p.panel_configs[0].auto_game_refocus);
    }

    #[test]
    fn app_settings_round_trip_and_defaults() {
        let f: AppSettingFile = serde_json::from_str("{}").unwrap();
        assert!(f.application_setting.general_setting.always_on_top);
        assert_eq!(f.application_setting.dynamic_lod_setting.targeted_fps, 60);

        let out = serde_json::to_value(&f).unwrap();
        assert!(out["ApplicationSetting"].get("TrackIRSetting").is_some());
        assert!(out["ApplicationSetting"]["TrackIRSetting"]
            .get("AutoDisableTrackIR")
            .is_some());
        assert!(out["ApplicationSetting"]["KeyboardShortcutSetting"]
            .get("StartPopOutKeyBinding")
            .is_none());
    }

    #[test]
    fn legacy_keybinding_is_migrated() {
        let json = r#"{"ApplicationSetting":{"KeyboardShortcutSetting":{"IsEnabled":true,"StartPopOutKeyBinding":"p"}}}"#;
        let mut f: AppSettingFile = serde_json::from_str(json).unwrap();
        f.application_setting.keyboard_shortcut_setting.migrate_legacy();
        assert_eq!(
            f.application_setting
                .keyboard_shortcut_setting
                .pop_out_keyboard_binding
                .as_deref(),
            Some("LeftCtrl|LeftShift|P")
        );
    }

    #[test]
    fn profile_sorting_is_case_insensitive() {
        let mut v = vec![
            UserProfile::new("b"),
            UserProfile::new("A"),
            UserProfile::new("c"),
        ];
        sort_profiles(&mut v);
        let names: Vec<_> = v.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["A", "b", "c"]);
    }

    #[test]
    fn colors_are_assigned_in_order() {
        let mut p = UserProfile::new("x");
        let a = p.add_custom_panel();
        let b = p.add_custom_panel();
        assert_eq!(p.panel(a).unwrap().panel_source.color.as_deref(), Some("LimeGreen"));
        assert_eq!(p.panel(b).unwrap().panel_source.color.as_deref(), Some("Red"));
    }

    #[test]
    fn hud_bar_panel_is_added_and_removed() {
        let mut p = UserProfile::new("x");
        p.profile_setting.hud_bar_config.is_enabled = true;
        p.on_hud_bar_changed();
        p.on_hud_bar_changed();
        assert_eq!(p.panel_configs.len(), 1);
        assert_eq!(p.panel_configs[0].height, 40);
        p.profile_setting.hud_bar_config.is_enabled = false;
        p.on_hud_bar_changed();
        assert!(p.panel_configs.is_empty());
    }

    #[test]
    fn full_screen_and_touch_invariants() {
        let mut c = PanelConfig::default();
        c.always_on_top = true;
        c.hide_titlebar = true;
        c.set_full_screen(true);
        assert!(!c.always_on_top && !c.hide_titlebar);
        c.auto_game_refocus = false;
        c.set_touch_enabled(true);
        assert!(c.auto_game_refocus);
    }

    #[test]
    fn popped_out_false_resets_handles() {
        let mut p = UserProfile::new("x");
        p.panel_configs.push(PanelConfig::default());
        p.panel_configs[0].panel_handle = PanelHandle::Hwnd(5);
        p.set_popped_out(false);
        assert_eq!(p.panel_configs[0].panel_handle, PanelHandle::Unset);
    }
}
