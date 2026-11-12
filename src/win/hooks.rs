//! The hook thread.
//!
//! Low-level keyboard/mouse hooks and WinEvent hooks must be installed from a thread that
//! pumps messages, and their callbacks must return quickly. A dedicated thread owns all of
//! them (plus the panel source overlay windows) and reports what happened as [`HookEvent`]s.
//! It is controlled with [`HookCmd`]s sent through [`HookHandle`].

use super::input::INJECT_TAG;
use super::overlay::{self, OverlaySpec};
use super::touch_hook;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, OnceLock, RwLock};
use uuid::Uuid;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::GetCurrentThreadId;
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_CONTROL, VK_SHIFT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, EVENT_OBJECT_STATECHANGE, EVENT_SYSTEM_CAPTUREEND,
    EVENT_SYSTEM_CAPTURESTART, EVENT_SYSTEM_MOVESIZEEND, GetMessageW, HHOOK, KBDLLHOOKSTRUCT, MSG,
    MSLLHOOKSTRUCT, PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL, WH_MOUSE_LL, WINEVENT_OUTOFCONTEXT,
    WM_APP, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

#[derive(Debug, Clone)]
pub enum HookEvent {
    /// A physical key press/release (events we inject ourselves are filtered out).
    Key {
        vk: u32,
        down: bool,
        ctrl: bool,
        shift: bool,
    },
    /// Left mouse button pressed (only while click capture is on).
    MouseLeftDown {
        x: i32,
        y: i32,
    },
    /// A WinEvent for one of the registered panel windows.
    WinEvent {
        event: u32,
        hwnd: isize,
    },
    OverlayDragStarted(Uuid),
    OverlayMoved {
        id: Uuid,
        x: i32,
        y: i32,
    },
}

#[derive(Debug, Clone)]
pub enum HookCmd {
    Keyboard(bool),
    /// Report left clicks (panel source selection).
    MouseClickCapture(bool),
    /// Run the touch/refocus mouse logic.
    Touch(bool),
    /// `None` removes the WinEvent hook.
    WinEvent(Option<(u32, u32)>),
    ShowOverlay(OverlaySpec),
    CloseOverlay(Uuid),
    CloseAllOverlays,
    Shutdown,
}

type Emitter = Arc<dyn Fn(HookEvent) + Send + Sync>;

static EMITTER: OnceLock<Emitter> = OnceLock::new();

static KB_HOOK: AtomicIsize = AtomicIsize::new(0);
static MOUSE_HOOK: AtomicIsize = AtomicIsize::new(0);
static WIN_HOOK: AtomicIsize = AtomicIsize::new(0);
static CAPTURE_CLICKS: AtomicBool = AtomicBool::new(false);
static TOUCH_ENABLED: AtomicBool = AtomicBool::new(false);
static PANEL_HWNDS: RwLock<Vec<isize>> = RwLock::new(Vec::new());

pub(super) fn emit(e: HookEvent) {
    if let Some(f) = EMITTER.get() {
        f(e);
    }
}

/// Window handles whose WinEvents are interesting (all others are dropped in the callback).
pub fn set_panel_hwnds(v: Vec<isize>) {
    *PANEL_HWNDS.write().unwrap_or_else(|e| e.into_inner()) = v;
}

fn is_panel_hwnd(h: isize) -> bool {
    PANEL_HWNDS
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .contains(&h)
}

// --- hook procedures ---------------------------------------------------------------

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == 0 {
        let k = &*(lparam as *const KBDLLHOOKSTRUCT);
        if k.dwExtraInfo != INJECT_TAG {
            let msg = wparam as u32;
            let down = matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN);
            let up = matches!(msg, WM_KEYUP | WM_SYSKEYUP);
            if down || up {
                let ctrl = GetAsyncKeyState(VK_CONTROL as i32) < 0;
                let shift = GetAsyncKeyState(VK_SHIFT as i32) < 0;
                emit(HookEvent::Key {
                    vk: k.vkCode,
                    down,
                    ctrl,
                    shift,
                });
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == 0 {
        let m = &*(lparam as *const MSLLHOOKSTRUCT);
        let msg = wparam as u32;
        let extra = m.dwExtraInfo;

        if CAPTURE_CLICKS.load(Ordering::Relaxed) && msg == WM_LBUTTONDOWN && extra != INJECT_TAG {
            emit(HookEvent::MouseLeftDown {
                x: m.pt.x,
                y: m.pt.y,
            });
        }

        if TOUCH_ENABLED.load(Ordering::Relaxed)
            && touch_hook::on_mouse_event(msg, m.pt.x, m.pt.y, extra)
        {
            return 1;
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if hwnd.is_null() {
        return;
    }
    match event {
        EVENT_SYSTEM_MOVESIZEEND | EVENT_SYSTEM_CAPTURESTART | EVENT_SYSTEM_CAPTUREEND
            if id_object != 0 =>
        {
            return;
        }
        EVENT_SYSTEM_MOVESIZEEND
        | EVENT_SYSTEM_CAPTURESTART
        | EVENT_SYSTEM_CAPTUREEND
        | EVENT_OBJECT_STATECHANGE => {}
        _ => return,
    }
    let h = hwnd as isize;
    if is_panel_hwnd(h) {
        emit(HookEvent::WinEvent { event, hwnd: h });
    }
}

// --- the thread -----------------------------------------------------------------------

#[derive(Default)]
struct ThreadState {
    overlays: HashMap<Uuid, isize>,
}

fn set_keyboard(on: bool) {
    unsafe {
        let cur = KB_HOOK.load(Ordering::SeqCst);
        if on && cur == 0 {
            let h = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_proc),
                GetModuleHandleW(std::ptr::null()),
                0,
            );
            KB_HOOK.store(h as isize, Ordering::SeqCst);
        } else if !on && cur != 0 {
            UnhookWindowsHookEx(cur as HHOOK);
            KB_HOOK.store(0, Ordering::SeqCst);
        }
    }
}

/// The mouse hook is installed while either click capture or touch handling is requested.
fn update_mouse_hook() {
    let want = CAPTURE_CLICKS.load(Ordering::SeqCst) || TOUCH_ENABLED.load(Ordering::SeqCst);
    unsafe {
        let cur = MOUSE_HOOK.load(Ordering::SeqCst);
        if want && cur == 0 {
            let h = SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(mouse_proc),
                GetModuleHandleW(std::ptr::null()),
                0,
            );
            MOUSE_HOOK.store(h as isize, Ordering::SeqCst);
        } else if !want && cur != 0 {
            UnhookWindowsHookEx(cur as HHOOK);
            MOUSE_HOOK.store(0, Ordering::SeqCst);
        }
    }
}

fn set_win_event(range: Option<(u32, u32)>) {
    unsafe {
        let cur = WIN_HOOK.swap(0, Ordering::SeqCst);
        if cur != 0 {
            UnhookWinEvent(cur as HWINEVENTHOOK);
        }
        if let Some((min, max)) = range {
            let h = SetWinEventHook(
                min,
                max,
                std::ptr::null_mut(),
                Some(win_event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            WIN_HOOK.store(h as isize, Ordering::SeqCst);
        }
    }
}

/// Returns `false` when the thread should exit.
fn apply(cmd: HookCmd, st: &mut ThreadState) -> bool {
    match cmd {
        HookCmd::Keyboard(on) => set_keyboard(on),
        HookCmd::MouseClickCapture(on) => {
            CAPTURE_CLICKS.store(on, Ordering::SeqCst);
            update_mouse_hook();
        }
        HookCmd::Touch(on) => {
            TOUCH_ENABLED.store(on, Ordering::SeqCst);
            if !on {
                touch_hook::reset();
            }
            update_mouse_hook();
        }
        HookCmd::WinEvent(range) => set_win_event(range),
        HookCmd::ShowOverlay(spec) => {
            if let Some(old) = st.overlays.remove(&spec.id) {
                overlay::destroy(old);
            }
            if let Some(h) = overlay::create(&spec) {
                st.overlays.insert(spec.id, h);
            }
        }
        HookCmd::CloseOverlay(id) => {
            if let Some(h) = st.overlays.remove(&id) {
                overlay::destroy(h);
            }
        }
        HookCmd::CloseAllOverlays => {
            for (_, h) in st.overlays.drain() {
                overlay::destroy(h);
            }
        }
        HookCmd::Shutdown => return false,
    }
    true
}

fn drain(rx: &Receiver<HookCmd>, st: &mut ThreadState) -> bool {
    while let Ok(cmd) = rx.try_recv() {
        if !apply(cmd, st) {
            return false;
        }
    }
    true
}

fn thread_main(rx: Receiver<HookCmd>, thread_id: Arc<AtomicU32>) {
    unsafe {
        let mut msg: MSG = std::mem::zeroed();
        // force creation of the thread's message queue before publishing the thread id
        PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);
        thread_id.store(GetCurrentThreadId(), Ordering::SeqCst);

        let mut st = ThreadState::default();
        let mut running = drain(&rx, &mut st);
        while running {
            let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if r <= 0 {
                break;
            }
            if msg.hwnd.is_null() && msg.message == WM_APP {
                running = drain(&rx, &mut st);
                continue;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        set_keyboard(false);
        CAPTURE_CLICKS.store(false, Ordering::SeqCst);
        TOUCH_ENABLED.store(false, Ordering::SeqCst);
        update_mouse_hook();
        set_win_event(None);
        for (_, h) in st.overlays.drain() {
            overlay::destroy(h);
        }
        thread_id.store(0, Ordering::SeqCst);
    }
}

/// Handle used to control the hook thread from anywhere.
pub struct HookHandle {
    tx: Sender<HookCmd>,
    thread_id: Arc<AtomicU32>,
}

impl HookHandle {
    /// Start the hook thread. `emitter` is called (on the hook thread) for every event and
    /// must not block.
    pub fn spawn(emitter: impl Fn(HookEvent) + Send + Sync + 'static) -> std::io::Result<Self> {
        let _ = EMITTER.set(Arc::new(emitter));
        let (tx, rx) = channel();
        let thread_id = Arc::new(AtomicU32::new(0));
        let tid = thread_id.clone();
        std::thread::Builder::new()
            .name("popm-hooks".into())
            .spawn(move || thread_main(rx, tid))?;
        Ok(Self { tx, thread_id })
    }

    pub fn send(&self, cmd: HookCmd) {
        if self.tx.send(cmd).is_err() {
            return;
        }
        let tid = self.thread_id.load(Ordering::SeqCst);
        if tid != 0 {
            unsafe {
                PostThreadMessageW(tid, WM_APP, 0, 0);
            }
        }
    }
}
