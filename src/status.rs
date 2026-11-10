//! Status message log shown in the pop-out progress window
//! (port of `StatusMessageWriter` / `WorkflowStepWithMessage`).

use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusMessageType {
    Info,
    Success,
    Failure,
    Executing,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusMessage {
    pub text: String,
    pub kind: StatusMessageType,
    pub new_line: bool,
}

#[derive(Debug, Default)]
struct Inner {
    messages: Vec<StatusMessage>,
    /// Bumped on every change so the UI can cheaply detect updates.
    version: u64,
    enabled: bool,
}

/// Thread-safe, cheaply clonable message log.
#[derive(Debug, Clone, Default)]
pub struct StatusLog {
    inner: Arc<Mutex<Inner>>,
}

impl StatusLog {
    pub fn new() -> Self {
        Self::default()
    }

    fn with<R>(&self, f: impl FnOnce(&mut Inner) -> R) -> R {
        let mut g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let r = f(&mut g);
        g.version += 1;
        r
    }

    pub fn set_enabled(&self, enabled: bool) {
        self.with(|i| i.enabled = enabled);
    }

    pub fn write(&self, text: &str, kind: StatusMessageType) {
        self.push(text, kind, false);
    }

    pub fn write_line(&self, text: &str, kind: StatusMessageType) {
        self.push(text, kind, true);
    }

    fn push(&self, text: &str, kind: StatusMessageType, new_line: bool) {
        self.with(|i| {
            i.messages.push(StatusMessage {
                text: text.to_string(),
                kind,
                new_line,
            })
        });
    }

    pub fn write_executing(&self) {
        self.push("  (Executing)", StatusMessageType::Executing, false);
    }

    pub fn write_ok(&self) {
        self.push("  (OK)", StatusMessageType::Success, true);
    }

    pub fn write_failure(&self) {
        self.push("  (FAILED)", StatusMessageType::Failure, true);
    }

    pub fn remove_last(&self) {
        self.with(|i| {
            i.messages.pop();
        });
    }

    pub fn clear(&self) {
        self.with(|i| i.messages.clear());
    }

    /// `(version, messages)` snapshot for rendering.
    pub fn snapshot(&self) -> (u64, Vec<StatusMessage>) {
        let g = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        (g.version, g.messages.clone())
    }

    pub fn is_enabled(&self) -> bool {
        self.inner.lock().unwrap_or_else(|e| e.into_inner()).enabled
    }

    /// Run a workflow step, printing `message  (Executing)` and then `(OK)` / `(FAILED)`.
    pub fn step(&self, message: &str, sub_task: bool, f: impl FnOnce() -> bool) -> bool {
        let message = if sub_task {
            format!("          - {message}")
        } else {
            message.to_string()
        };
        self.write(&message, StatusMessageType::Info);
        self.write_executing();
        let ok = f();
        self.remove_last();
        if ok {
            self.write_ok();
        } else {
            self.write_failure();
        }
        ok
    }

    /// A step that cannot fail.
    pub fn step_ok(&self, message: &str, sub_task: bool, f: impl FnOnce()) {
        self.step(message, sub_task, || {
            f();
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_replaces_executing_with_result() {
        let log = StatusLog::new();
        assert!(log.step("Doing", false, || true));
        assert!(!log.step("Failing", true, || false));
        let (_, m) = log.snapshot();
        assert_eq!(m.len(), 4);
        assert_eq!(m[0].text, "Doing");
        assert_eq!(m[1].kind, StatusMessageType::Success);
        assert_eq!(m[2].text, "          - Failing");
        assert_eq!(m[3].kind, StatusMessageType::Failure);
        assert!(m[3].new_line);
    }

    #[test]
    fn version_changes_and_clear() {
        let log = StatusLog::new();
        let (v0, _) = log.snapshot();
        log.write_line("hello", StatusMessageType::Info);
        let (v1, m) = log.snapshot();
        assert!(v1 > v0);
        assert_eq!(m.len(), 1);
        log.clear();
        assert!(log.snapshot().1.is_empty());
    }
}
