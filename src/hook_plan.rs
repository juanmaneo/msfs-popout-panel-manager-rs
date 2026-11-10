//! Decides which input hooks must be active for the current application state.
//!
//! Upstream starts and stops its hooks from many different event handlers. Here the desired
//! hook set is a pure function of the state which the core thread re-evaluates regularly and
//! applies by diffing, which is easier to reason about (and to test).

use crate::model::{ApplicationSetting, PanelType, UserProfile};
use uuid::Uuid;

// WinEvent constants (kept numeric so this module stays platform independent).
pub const EVENT_SYSTEM_CAPTURESTART: u32 = 0x0008;
pub const EVENT_SYSTEM_CAPTUREEND: u32 = 0x0009;
pub const EVENT_SYSTEM_MOVESIZEEND: u32 = 0x000B;
pub const EVENT_OBJECT_STATECHANGE: u32 = 0x800A;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HookPlan {
    pub keyboard: bool,
    /// Report left clicks (choosing a panel source point).
    pub click_capture: bool,
    pub touch: bool,
    pub win_event: Option<(u32, u32)>,
}

/// Inputs that are not part of the profile/settings themselves.
#[derive(Debug, Clone, Copy, Default)]
pub struct PlanInput {
    /// A panel is waiting for the user to click its source point.
    pub selecting_panel: Option<Uuid>,
    /// A keystroke is being recorded (floating panel or pop out shortcut).
    pub detecting_keystroke: bool,
}

pub fn win_event_range(settings: &ApplicationSetting, profile: &UserProfile) -> Option<(u32, u32)> {
    if !profile.is_popped_out {
        return None;
    }
    let success = |p: &crate::model::PanelConfig| p.is_pop_out_success() == Some(true);

    let required_configuration = (!profile.is_locked
        || settings.pop_out_setting.enable_panel_reset_when_locked)
        && profile.panel_configs.iter().any(success);

    let non_touch_refocus = profile
        .panel_configs
        .iter()
        .any(|p| p.auto_game_refocus && !p.touch_enabled && success(p))
        && !profile.panel_configs.iter().all(|p| p.touch_enabled)
        && settings.refocus_setting.refocus_game_window.is_enabled;

    match (required_configuration, non_touch_refocus) {
        (true, true) => Some((EVENT_SYSTEM_CAPTURESTART, EVENT_OBJECT_STATECHANGE)),
        (true, false) => Some((EVENT_SYSTEM_MOVESIZEEND, EVENT_OBJECT_STATECHANGE)),
        (false, true) => Some((EVENT_SYSTEM_CAPTURESTART, EVENT_SYSTEM_CAPTUREEND)),
        (false, false) => None,
    }
}

pub fn needs_touch_hook(profile: &UserProfile) -> bool {
    if !profile.is_popped_out {
        return false;
    }
    let has_refocus_displays = profile
        .panel_configs
        .iter()
        .any(|p| p.panel_type == PanelType::RefocusDisplay);
    let has_touch_panel = profile
        .panel_configs
        .iter()
        .any(|p| p.touch_enabled && p.is_pop_out_success() == Some(true));
    has_refocus_displays || has_touch_panel
}

pub fn compute(
    settings: &ApplicationSetting,
    profile: Option<&UserProfile>,
    input: PlanInput,
) -> HookPlan {
    let mut plan = HookPlan {
        keyboard: settings.keyboard_shortcut_setting.is_enabled
            || settings.keyboard_shortcut_setting.is_detecting_keystroke
            || input.detecting_keystroke,
        click_capture: input.selecting_panel.is_some(),
        touch: false,
        win_event: None,
    };
    if let Some(p) = profile {
        plan.keyboard |= p.has_floating_panel() || !p.current_move_resize_panel_id.is_nil();
        plan.touch = needs_touch_hook(p);
        plan.win_event = win_event_range(settings, p);
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{PanelConfig, PanelHandle};

    fn popped_out_profile() -> UserProfile {
        let mut p = UserProfile::new("x");
        let mut a = PanelConfig::default();
        a.panel_handle = PanelHandle::Hwnd(10);
        p.panel_configs.push(a);
        p.is_popped_out = true;
        p
    }

    fn settings() -> ApplicationSetting {
        let mut s = ApplicationSetting::default();
        s.keyboard_shortcut_setting.is_enabled = false;
        s
    }

    #[test]
    fn nothing_when_not_popped_out() {
        let mut p = popped_out_profile();
        p.is_popped_out = false;
        let plan = compute(&settings(), Some(&p), PlanInput::default());
        assert_eq!(plan, HookPlan::default());
    }

    #[test]
    fn auto_refocus_panel_uses_wide_range() {
        let p = popped_out_profile(); // auto_game_refocus defaults to true, no touch
        let plan = compute(&settings(), Some(&p), PlanInput::default());
        assert_eq!(
            plan.win_event,
            Some((EVENT_SYSTEM_CAPTURESTART, EVENT_OBJECT_STATECHANGE))
        );
        assert!(!plan.touch);
    }

    #[test]
    fn without_refocus_only_configuration_events() {
        let mut p = popped_out_profile();
        p.panel_configs[0].auto_game_refocus = false;
        let plan = compute(&settings(), Some(&p), PlanInput::default());
        assert_eq!(
            plan.win_event,
            Some((EVENT_SYSTEM_MOVESIZEEND, EVENT_OBJECT_STATECHANGE))
        );
    }

    #[test]
    fn locked_profile_without_reset_only_needs_refocus_events() {
        let mut p = popped_out_profile();
        p.is_locked = true;
        let mut s = settings();
        s.pop_out_setting.enable_panel_reset_when_locked = false;
        let plan = compute(&s, Some(&p), PlanInput::default());
        assert_eq!(
            plan.win_event,
            Some((EVENT_SYSTEM_CAPTURESTART, EVENT_SYSTEM_CAPTUREEND))
        );

        p.panel_configs[0].auto_game_refocus = false;
        assert_eq!(compute(&s, Some(&p), PlanInput::default()).win_event, None);
    }

    #[test]
    fn all_touch_panels_disable_refocus_logic() {
        let mut p = popped_out_profile();
        p.panel_configs[0].touch_enabled = true;
        let plan = compute(&settings(), Some(&p), PlanInput::default());
        assert_eq!(
            plan.win_event,
            Some((EVENT_SYSTEM_MOVESIZEEND, EVENT_OBJECT_STATECHANGE))
        );
        assert!(plan.touch);
    }

    #[test]
    fn refocus_setting_disabled_removes_refocus_events() {
        let p = popped_out_profile();
        let mut s = settings();
        s.refocus_setting.refocus_game_window.is_enabled = false;
        let plan = compute(&s, Some(&p), PlanInput::default());
        assert_eq!(
            plan.win_event,
            Some((EVENT_SYSTEM_MOVESIZEEND, EVENT_OBJECT_STATECHANGE))
        );
    }

    #[test]
    fn refocus_display_panels_need_touch_hook() {
        let mut p = UserProfile::new("x");
        let mut d = PanelConfig::new(PanelType::RefocusDisplay, "DISPLAY1");
        d.panel_handle = PanelHandle::Hwnd(1);
        p.panel_configs.push(d);
        p.is_popped_out = true;
        assert!(needs_touch_hook(&p));
    }

    #[test]
    fn keyboard_hook_requirements() {
        let mut s = settings();
        let mut p = UserProfile::new("x");
        assert!(!compute(&s, Some(&p), PlanInput::default()).keyboard);

        s.keyboard_shortcut_setting.is_enabled = true;
        assert!(compute(&s, Some(&p), PlanInput::default()).keyboard);
        s.keyboard_shortcut_setting.is_enabled = false;

        let mut c = PanelConfig::default();
        c.floating_panel.is_enabled = true;
        p.panel_configs.push(c);
        assert!(compute(&s, Some(&p), PlanInput::default()).keyboard);
        p.panel_configs[0].floating_panel.is_enabled = false;

        p.current_move_resize_panel_id = Uuid::new_v4();
        assert!(compute(&s, Some(&p), PlanInput::default()).keyboard);
        p.current_move_resize_panel_id = Uuid::nil();

        let input = PlanInput {
            detecting_keystroke: true,
            ..PlanInput::default()
        };
        assert!(compute(&s, Some(&p), input).keyboard);
    }

    #[test]
    fn click_capture_follows_selection() {
        let input = PlanInput {
            selecting_panel: Some(Uuid::new_v4()),
            ..PlanInput::default()
        };
        assert!(compute(&settings(), None, input).click_capture);
        assert!(!compute(&settings(), None, PlanInput::default()).click_capture);
    }
}
