//! Pure geometry/state rules for touch handling and game refocus
//! (ported from `TouchEventManager.cs` and `GameRefocusManager.cs`).

use crate::model::{MonitorInfo, PanelConfig, PanelHandle, PanelType, UserProfile};

/// Height of a pop out window's title bar.
pub const PANEL_MENUBAR_HEIGHT: i32 = 31;

/// Windows marks mouse events synthesized from touch input with this signature in `dwExtraInfo`.
pub const TOUCH_FLAG: u32 = 0xFF51_5700;

pub fn is_touch_event(extra_info: usize) -> bool {
    (extra_info as u32 & TOUCH_FLAG) == TOUCH_FLAG
}

/// Immutable per-panel data needed by the low-level mouse hook.
#[derive(Debug, Clone, PartialEq)]
pub struct TouchPanel {
    pub panel_type: PanelType,
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
    pub hide_titlebar: bool,
    pub full_screen: bool,
    pub full_screen_monitor: Option<MonitorInfo>,
    pub auto_game_refocus: bool,
}

pub fn within_window(p: &TouchPanel, x: i32, y: i32) -> bool {
    x > p.left
        && x < p.left + p.width
        && y > p.top
            + if p.hide_titlebar {
                5
            } else {
                PANEL_MENUBAR_HEIGHT
            }
        && y < p.top + p.height
}

pub fn within_full_screen(p: &TouchPanel, x: i32, y: i32) -> bool {
    match &p.full_screen_monitor {
        Some(m) => x > m.x && x < m.x + m.width && y > m.y && y < m.y + m.height,
        None => false,
    }
}

/// First touch-enabled panel that contains the point.
pub fn find_touch_panel(panels: &[TouchPanel], x: i32, y: i32) -> Option<&TouchPanel> {
    panels
        .iter()
        .find(|p| (p.full_screen && within_full_screen(p, x, y)) || within_window(p, x, y))
}

/// Touch-enabled panels of the profile that popped out successfully (plus refocus displays).
pub fn collect_touch_panels(profile: &UserProfile) -> Vec<TouchPanel> {
    profile
        .panel_configs
        .iter()
        .filter(|p| p.touch_enabled && p.panel_handle != PanelHandle::Unset)
        .filter(|p| p.panel_handle != PanelHandle::Failed)
        .map(to_touch_panel)
        .collect()
}

pub fn to_touch_panel(p: &PanelConfig) -> TouchPanel {
    TouchPanel {
        panel_type: p.panel_type,
        left: p.left,
        top: p.top,
        width: p.width,
        height: p.height,
        hide_titlebar: p.hide_titlebar,
        full_screen: p.full_screen,
        full_screen_monitor: p.full_screen_monitor.clone(),
        auto_game_refocus: p.auto_game_refocus,
    }
}

/// `GameRefocusManager.HandleMouseDownEvent`: ignore presses on the title bar or the borders.
pub fn is_refocus_press_allowed(p: &PanelConfig, cursor_x: i32, cursor_y: i32) -> bool {
    if cursor_y - p.top < if p.hide_titlebar { 5 } else { 50 } {
        return false;
    }
    if p.top + p.height - cursor_y < 15 {
        return false;
    }
    if cursor_x - p.left < 15 {
        return false;
    }
    if p.left + p.width - cursor_x < 15 {
        return false;
    }
    true
}

/// `GameRefocusManager.HandleMouseUpEvent`: releases over the title bar do not refocus.
pub fn is_refocus_release_allowed(p: &PanelConfig, cursor_y: i32) -> bool {
    cursor_y - p.top > if p.hide_titlebar { 5 } else { 31 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel() -> TouchPanel {
        TouchPanel {
            panel_type: PanelType::CustomPopout,
            left: 100,
            top: 100,
            width: 400,
            height: 300,
            hide_titlebar: false,
            full_screen: false,
            full_screen_monitor: None,
            auto_game_refocus: true,
        }
    }

    #[test]
    fn touch_signature() {
        assert!(is_touch_event(0xFF51_5700));
        assert!(is_touch_event(0xFF51_5781));
        assert!(!is_touch_event(0));
        assert!(!is_touch_event(0x1234_5678));
    }

    #[test]
    fn window_hit_test_excludes_title_bar() {
        let p = panel();
        assert!(within_window(&p, 300, 300));
        assert!(!within_window(&p, 300, 110)); // title bar
        assert!(!within_window(&p, 90, 300));
        assert!(!within_window(&p, 300, 400));
        let mut h = p.clone();
        h.hide_titlebar = true;
        assert!(within_window(&h, 300, 110));
        assert!(!within_window(&h, 300, 103));
    }

    #[test]
    fn full_screen_uses_monitor_bounds() {
        let mut p = panel();
        p.full_screen = true;
        p.full_screen_monitor = Some(MonitorInfo {
            x: 1920,
            y: 0,
            width: 1920,
            height: 1080,
            ..MonitorInfo::default()
        });
        let panels = vec![p];
        assert!(find_touch_panel(&panels, 2500, 500).is_some());
        assert!(find_touch_panel(&panels, 300, 300).is_some()); // still inside the window rect
        assert!(find_touch_panel(&panels, 5000, 5000).is_none());
    }

    #[test]
    fn refocus_rules() {
        let mut c = PanelConfig::default();
        c.left = 100;
        c.top = 100;
        c.width = 400;
        c.height = 300;
        assert!(is_refocus_press_allowed(&c, 300, 300));
        assert!(!is_refocus_press_allowed(&c, 300, 120)); // title bar
        assert!(!is_refocus_press_allowed(&c, 105, 300)); // left border
        assert!(!is_refocus_press_allowed(&c, 495, 300)); // right border
        assert!(!is_refocus_press_allowed(&c, 300, 395)); // bottom border
        assert!(is_refocus_release_allowed(&c, 300));
        assert!(!is_refocus_release_allowed(&c, 120));
    }

    #[test]
    fn collects_only_popped_out_touch_panels() {
        let mut p = UserProfile::new("x");
        let mut a = PanelConfig::default();
        a.touch_enabled = true;
        a.panel_handle = PanelHandle::Hwnd(5);
        let mut b = PanelConfig::default();
        b.touch_enabled = true; // not popped out
        let mut c = PanelConfig::default();
        c.touch_enabled = true;
        c.panel_handle = PanelHandle::Failed;
        let mut d = PanelConfig::default();
        d.panel_handle = PanelHandle::Hwnd(6); // touch disabled
        p.panel_configs = vec![a, b, c, d];
        assert_eq!(collect_touch_panels(&p).len(), 1);
    }
}
