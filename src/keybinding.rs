//! Keyboard binding names and the global key-capture state machine.
//!
//! Bindings are stored exactly like upstream: WPF `System.Windows.Input.Key` names,
//! distinct, sorted, joined with `|` (e.g. `LeftCtrl|LeftShift|P`).

/// Sort + deduplicate key names and join them with `|`.
///
/// Upstream used a culture-sensitive `OrderBy`, which for key names behaves like a
/// case-insensitive ordinal comparison (`LeftCtrl` < `LeftShift` < `LWin` < `P`).
pub fn sort_binding(keys: &[String]) -> String {
    let mut v: Vec<&String> = Vec::new();
    for k in keys {
        if !v.contains(&k) {
            v.push(k);
        }
    }
    v.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| b.cmp(a)) // lower-case first on ties, like culture compare
    });
    v.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("|")
}

/// Human readable form of a binding (`LeftCtrl + LeftShift + P`).
pub fn display_binding(binding: &str) -> String {
    binding.replace('|', " + ")
}

/// Map a Windows virtual key code to the WPF `Key` name used in the data files.
pub fn vk_to_key_name(vk: u32) -> Option<String> {
    let s: &str = match vk {
        0x08 => "Back",
        0x09 => "Tab",
        0x0C => "Clear",
        0x0D => "Return",
        0x10 => "LeftShift", // generic shift: only seen if left/right is not reported
        0x11 => "LeftCtrl",
        0x12 => "LeftAlt",
        0x13 => "Pause",
        0x14 => "Capital",
        0x1B => "Escape",
        0x20 => "Space",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "Left",
        0x26 => "Up",
        0x27 => "Right",
        0x28 => "Down",
        0x29 => "Select",
        0x2A => "Print",
        0x2B => "Execute",
        0x2C => "Snapshot",
        0x2D => "Insert",
        0x2E => "Delete",
        0x2F => "Help",
        0x30..=0x39 => return Some(format!("D{}", vk - 0x30)),
        0x41..=0x5A => return Some(((vk as u8) as char).to_string()),
        0x5B => "LWin",
        0x5C => "RWin",
        0x5D => "Apps",
        0x5F => "Sleep",
        0x60..=0x69 => return Some(format!("NumPad{}", vk - 0x60)),
        0x6A => "Multiply",
        0x6B => "Add",
        0x6C => "Separator",
        0x6D => "Subtract",
        0x6E => "Decimal",
        0x6F => "Divide",
        0x70..=0x87 => return Some(format!("F{}", vk - 0x70 + 1)),
        0x90 => "NumLock",
        0x91 => "Scroll",
        0xA0 => "LeftShift",
        0xA1 => "RightShift",
        0xA2 => "LeftCtrl",
        0xA3 => "RightCtrl",
        0xA4 => "LeftAlt",
        0xA5 => "RightAlt",
        0xBA => "OemSemicolon",
        0xBB => "OemPlus",
        0xBC => "OemComma",
        0xBD => "OemMinus",
        0xBE => "OemPeriod",
        0xBF => "OemQuestion",
        0xC0 => "OemTilde",
        0xDB => "OemOpenBrackets",
        0xDC => "OemPipe",
        0xDD => "OemCloseBrackets",
        0xDE => "OemQuotes",
        0xDF => "Oem8",
        0xE2 => "OemBackslash",
        _ => return None,
    };
    Some(s.to_string())
}

/// Result of feeding a key event into [`KeyCapture`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CaptureResult {
    /// Nothing completed yet.
    None,
    /// A key chord has been released; the sorted binding string is returned.
    Completed(String),
}

/// Reproduces `KeyboardOrchestrator.HandleGlobalKeyboardHookOnKeyboardPressed`:
/// every key-down is collected; on the first key-up the collected chord is finalized.
#[derive(Debug, Default)]
pub struct KeyCapture {
    pressed: Vec<String>,
    capturing: bool,
}

impl KeyCapture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn key_down(&mut self, name: String) {
        self.capturing = true;
        self.pressed.push(name);
    }

    pub fn key_up(&mut self) -> CaptureResult {
        if !self.capturing {
            return CaptureResult::None;
        }
        self.capturing = false;
        let binding = sort_binding(&self.pressed);
        self.pressed.clear();
        CaptureResult::Completed(binding)
    }
}

/// Arrow-key driven move/resize of a panel (`PopOutPanelConfigCardViewModel.HandleKeyUpEvent`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveResizeAction {
    Top(i32),
    Left(i32),
    Height(i32),
    Width(i32),
}

/// `vk` must be an arrow key (0x25..=0x28).
/// Plain = move by 10, Shift = move by 1, Ctrl = resize by 10, Shift+Ctrl = resize by 1.
pub fn move_resize_action(vk: u32, ctrl: bool, shift: bool) -> Option<MoveResizeAction> {
    let sign = match vk {
        0x25 | 0x26 => -1, // left, up
        0x27 | 0x28 => 1,  // right, down
        _ => return None,
    };
    let step = if shift { 1 } else { 10 } * sign;
    let vertical = matches!(vk, 0x26 | 0x28);
    Some(match (ctrl, vertical) {
        (false, true) => MoveResizeAction::Top(step),
        (false, false) => MoveResizeAction::Left(step),
        (true, true) => MoveResizeAction::Height(step),
        (true, false) => MoveResizeAction::Width(step),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn sorts_case_insensitively_and_dedups() {
        assert_eq!(
            sort_binding(&s(&["P", "LeftShift", "LeftCtrl", "P"])),
            "LeftCtrl|LeftShift|P"
        );
        assert_eq!(sort_binding(&s(&["LWin", "LeftCtrl"])), "LeftCtrl|LWin");
        assert_eq!(sort_binding(&s(&["D1", "A"])), "A|D1");
    }

    #[test]
    fn vk_names() {
        assert_eq!(vk_to_key_name(0x50).as_deref(), Some("P"));
        assert_eq!(vk_to_key_name(0x31).as_deref(), Some("D1"));
        assert_eq!(vk_to_key_name(0x62).as_deref(), Some("NumPad2"));
        assert_eq!(vk_to_key_name(0x70).as_deref(), Some("F1"));
        assert_eq!(vk_to_key_name(0x87).as_deref(), Some("F24"));
        assert_eq!(vk_to_key_name(0xA2).as_deref(), Some("LeftCtrl"));
        assert_eq!(vk_to_key_name(0xFF), None);
    }

    #[test]
    fn capture_completes_on_first_key_up() {
        let mut c = KeyCapture::new();
        assert_eq!(c.key_up(), CaptureResult::None);
        c.key_down("LeftCtrl".into());
        c.key_down("LeftShift".into());
        c.key_down("P".into());
        assert_eq!(
            c.key_up(),
            CaptureResult::Completed("LeftCtrl|LeftShift|P".into())
        );
        // later key-ups without key-down do nothing
        assert_eq!(c.key_up(), CaptureResult::None);
    }

    #[test]
    fn auto_repeat_is_deduplicated() {
        let mut c = KeyCapture::new();
        for _ in 0..5 {
            c.key_down("A".into());
        }
        assert_eq!(c.key_up(), CaptureResult::Completed("A".into()));
    }

    #[test]
    fn move_resize_mapping() {
        use MoveResizeAction::*;
        assert_eq!(move_resize_action(0x26, false, false), Some(Top(-10)));
        assert_eq!(move_resize_action(0x28, false, true), Some(Top(1)));
        assert_eq!(move_resize_action(0x25, false, false), Some(Left(-10)));
        assert_eq!(move_resize_action(0x27, false, true), Some(Left(1)));
        assert_eq!(move_resize_action(0x26, true, false), Some(Height(-10)));
        assert_eq!(move_resize_action(0x27, true, true), Some(Width(1)));
        assert_eq!(move_resize_action(0x41, false, false), None);
    }
}
