//! Minimal dynamic binding to the SimConnect C API (`SimConnect.dll`).
//!
//! The DLL is loaded at run time (from the executable's folder or `PATH`) so the program
//! starts, and reports a helpful message, even when the SimConnect redistributable is missing.

use libloading::{Library, Symbol};
use std::ffi::{CString, c_char, c_void};

pub type Handle = *mut c_void;
pub type Hresult = i32;

pub const E_FAIL: Hresult = 0x8000_4005_u32 as i32;

pub const DATATYPE_FLOAT64: u32 = 4;
pub const SIMOBJECT_TYPE_USER: u32 = 0;
pub const OBJECT_ID_USER: u32 = 0;
pub const UNUSED: u32 = 0xFFFF_FFFF;
pub const EVENT_FLAG_GROUPID_IS_PRIORITY: u32 = 0x10;
pub const GROUP_PRIORITY_HIGHEST: u32 = 1;

// SIMCONNECT_RECV ids
pub const RECV_EXCEPTION: u32 = 1;
pub const RECV_OPEN: u32 = 2;
pub const RECV_QUIT: u32 = 3;
pub const RECV_EVENT: u32 = 4;
pub const RECV_EVENT_FILENAME: u32 = 6;
pub const RECV_EVENT_FRAME: u32 = 7;
pub const RECV_SIMOBJECT_DATA_BYTYPE: u32 = 9;
pub const RECV_SYSTEM_STATE: u32 = 15;

pub const MAX_PATH: usize = 260;

// SIMCONNECT_EXCEPTION values that are expected during normal operation and ignored.
pub const EXC_NAME_UNRECOGNIZED: u32 = 7;
pub const EXC_UNRECOGNIZED_ID: u32 = 3;
pub const EXC_EVENT_ID_DUPLICATE: u32 = 9;
pub const EXC_DATA_ERROR: u32 = 20;
pub const EXC_ALREADY_CREATED: u32 = 32;

pub fn is_ignored_exception(code: u32) -> bool {
    matches!(
        code,
        EXC_DATA_ERROR
            | EXC_NAME_UNRECOGNIZED
            | EXC_ALREADY_CREATED
            | EXC_UNRECOGNIZED_ID
            | EXC_EVENT_ID_DUPLICATE
    )
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct RecvHeader {
    pub size: u32,
    pub version: u32,
    pub id: u32,
}

type OpenFn = unsafe extern "system" fn(
    *mut Handle,
    *const c_char,
    *mut c_void,
    u32,
    *mut c_void,
    u32,
) -> Hresult;
type CloseFn = unsafe extern "system" fn(Handle) -> Hresult;
type GetNextDispatchFn =
    unsafe extern "system" fn(Handle, *mut *mut RecvHeader, *mut u32) -> Hresult;
type AddToDataDefinitionFn =
    unsafe extern "system" fn(Handle, u32, *const c_char, *const c_char, u32, f32, u32) -> Hresult;
type ClearDataDefinitionFn = unsafe extern "system" fn(Handle, u32) -> Hresult;
type RequestDataOnSimObjectTypeFn =
    unsafe extern "system" fn(Handle, u32, u32, u32, u32) -> Hresult;
type SetDataOnSimObjectFn =
    unsafe extern "system" fn(Handle, u32, u32, u32, u32, u32, *mut c_void) -> Hresult;
type MapClientEventFn = unsafe extern "system" fn(Handle, u32, *const c_char) -> Hresult;
type TransmitClientEventFn = unsafe extern "system" fn(Handle, u32, u32, u32, u32, u32) -> Hresult;
type SubscribeFn = unsafe extern "system" fn(Handle, u32, *const c_char) -> Hresult;
type UnsubscribeFn = unsafe extern "system" fn(Handle, u32) -> Hresult;
type RequestSystemStateFn = unsafe extern "system" fn(Handle, u32, *const c_char) -> Hresult;

/// Loaded library plus resolved entry points.
pub struct SimConnectLib {
    _lib: Library,
    open: OpenFn,
    close: CloseFn,
    get_next_dispatch: GetNextDispatchFn,
    add_to_data_definition: AddToDataDefinitionFn,
    clear_data_definition: ClearDataDefinitionFn,
    request_data_on_sim_object_type: RequestDataOnSimObjectTypeFn,
    set_data_on_sim_object: SetDataOnSimObjectFn,
    map_client_event_to_sim_event: MapClientEventFn,
    transmit_client_event: TransmitClientEventFn,
    subscribe_to_system_event: SubscribeFn,
    unsubscribe_from_system_event: UnsubscribeFn,
    request_system_state: RequestSystemStateFn,
}

macro_rules! sym {
    ($lib:expr, $name:literal, $ty:ty) => {{
        let s: Symbol<$ty> = unsafe { $lib.get(concat!($name, "\0").as_bytes()) }
            .map_err(|e| format!("{}: {e}", $name))?;
        *s
    }};
}

impl SimConnectLib {
    pub fn load() -> Result<Self, String> {
        let lib = unsafe { Library::new("SimConnect.dll") }.map_err(|e| {
            format!("SimConnect.dll could not be loaded ({e}). Copy it next to the executable.")
        })?;
        Ok(Self {
            open: sym!(lib, "SimConnect_Open", OpenFn),
            close: sym!(lib, "SimConnect_Close", CloseFn),
            get_next_dispatch: sym!(lib, "SimConnect_GetNextDispatch", GetNextDispatchFn),
            add_to_data_definition: sym!(
                lib,
                "SimConnect_AddToDataDefinition",
                AddToDataDefinitionFn
            ),
            clear_data_definition: sym!(
                lib,
                "SimConnect_ClearDataDefinition",
                ClearDataDefinitionFn
            ),
            request_data_on_sim_object_type: sym!(
                lib,
                "SimConnect_RequestDataOnSimObjectType",
                RequestDataOnSimObjectTypeFn
            ),
            set_data_on_sim_object: sym!(
                lib,
                "SimConnect_SetDataOnSimObject",
                SetDataOnSimObjectFn
            ),
            map_client_event_to_sim_event: sym!(
                lib,
                "SimConnect_MapClientEventToSimEvent",
                MapClientEventFn
            ),
            transmit_client_event: sym!(
                lib,
                "SimConnect_TransmitClientEvent",
                TransmitClientEventFn
            ),
            subscribe_to_system_event: sym!(lib, "SimConnect_SubscribeToSystemEvent", SubscribeFn),
            unsubscribe_from_system_event: sym!(
                lib,
                "SimConnect_UnsubscribeFromSystemEvent",
                UnsubscribeFn
            ),
            request_system_state: sym!(lib, "SimConnect_RequestSystemState", RequestSystemStateFn),
            _lib: lib,
        })
    }
}

/// An open connection to the simulator.
pub struct Connection<'a> {
    lib: &'a SimConnectLib,
    handle: Handle,
}

fn check(hr: Hresult) -> Result<(), Hresult> {
    if hr >= 0 { Ok(()) } else { Err(hr) }
}

fn cstr(s: &str) -> CString {
    CString::new(s).unwrap_or_default()
}

/// What `next_dispatch` found.
pub enum Dispatch {
    /// A message; the pointer is valid until the next call.
    Message(*const RecvHeader, u32),
    Empty,
}

impl<'a> Connection<'a> {
    pub fn open(lib: &'a SimConnectLib, name: &str) -> Result<Self, Hresult> {
        let mut handle: Handle = std::ptr::null_mut();
        let n = cstr(name);
        let hr = unsafe {
            (lib.open)(
                &mut handle,
                n.as_ptr(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                0,
            )
        };
        check(hr)?;
        if handle.is_null() {
            return Err(E_FAIL);
        }
        Ok(Self { lib, handle })
    }

    pub fn next_dispatch(&self) -> Result<Dispatch, Hresult> {
        let mut data: *mut RecvHeader = std::ptr::null_mut();
        let mut size = 0u32;
        let hr = unsafe { (self.lib.get_next_dispatch)(self.handle, &mut data, &mut size) };
        if hr == 0 && !data.is_null() {
            Ok(Dispatch::Message(data, size))
        } else if hr == 0 || hr == E_FAIL {
            // an empty queue is reported as E_FAIL by some SimConnect versions
            Ok(Dispatch::Empty)
        } else {
            Err(hr)
        }
    }

    pub fn add_to_data_definition(
        &self,
        define_id: u32,
        name: &str,
        units: &str,
    ) -> Result<(), Hresult> {
        let (n, u) = (cstr(name), cstr(units));
        check(unsafe {
            (self.lib.add_to_data_definition)(
                self.handle,
                define_id,
                n.as_ptr(),
                u.as_ptr(),
                DATATYPE_FLOAT64,
                0.0,
                UNUSED,
            )
        })
    }

    pub fn clear_data_definition(&self, define_id: u32) -> Result<(), Hresult> {
        check(unsafe { (self.lib.clear_data_definition)(self.handle, define_id) })
    }

    pub fn request_data_on_user(&self, request_id: u32, define_id: u32) -> Result<(), Hresult> {
        check(unsafe {
            (self.lib.request_data_on_sim_object_type)(
                self.handle,
                request_id,
                define_id,
                0,
                SIMOBJECT_TYPE_USER,
            )
        })
    }

    pub fn set_data_f64(&self, define_id: u32, value: f64) -> Result<(), Hresult> {
        let mut v = value;
        check(unsafe {
            (self.lib.set_data_on_sim_object)(
                self.handle,
                define_id,
                OBJECT_ID_USER,
                0,
                1,
                std::mem::size_of::<f64>() as u32,
                &mut v as *mut f64 as *mut c_void,
            )
        })
    }

    pub fn map_client_event(&self, event_id: u32, name: &str) -> Result<(), Hresult> {
        let n = cstr(name);
        check(unsafe {
            (self.lib.map_client_event_to_sim_event)(self.handle, event_id, n.as_ptr())
        })
    }

    pub fn transmit_client_event(&self, event_id: u32, data: u32) -> Result<(), Hresult> {
        check(unsafe {
            (self.lib.transmit_client_event)(
                self.handle,
                OBJECT_ID_USER,
                event_id,
                data,
                GROUP_PRIORITY_HIGHEST,
                EVENT_FLAG_GROUPID_IS_PRIORITY,
            )
        })
    }

    pub fn subscribe_to_system_event(&self, event_id: u32, name: &str) -> Result<(), Hresult> {
        let n = cstr(name);
        check(unsafe { (self.lib.subscribe_to_system_event)(self.handle, event_id, n.as_ptr()) })
    }

    pub fn unsubscribe_from_system_event(&self, event_id: u32) -> Result<(), Hresult> {
        check(unsafe { (self.lib.unsubscribe_from_system_event)(self.handle, event_id) })
    }

    pub fn request_system_state(&self, request_id: u32, state: &str) -> Result<(), Hresult> {
        let s = cstr(state);
        check(unsafe { (self.lib.request_system_state)(self.handle, request_id, s.as_ptr()) })
    }
}

impl Drop for Connection<'_> {
    fn drop(&mut self) {
        unsafe {
            (self.lib.close)(self.handle);
        }
    }
}

// --- parsing of received messages ---------------------------------------------------------

/// A decoded SimConnect message of interest.
#[derive(Debug, Clone, PartialEq)]
pub enum Recv {
    Open,
    Quit,
    Exception(u32),
    Event { event_id: u32 },
    EventFilename { event_id: u32, file: String },
    EventFrame { fps: f32 },
    SimObjectData { request_id: u32, values: Vec<f64> },
    SystemState { request_id: u32, text: String },
    Other,
}

fn rd_u32(buf: &[u8], off: usize) -> Option<u32> {
    buf.get(off..off + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn rd_f32(buf: &[u8], off: usize) -> Option<f32> {
    rd_u32(buf, off).map(f32::from_bits)
}

fn rd_cstr(buf: &[u8], off: usize, max: usize) -> String {
    let end = (off + max).min(buf.len());
    let slice = buf.get(off..end).unwrap_or(&[]);
    let n = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    // paths may contain non-ASCII characters in the system code page; keep it lossless enough
    String::from_utf8_lossy(&slice[..n]).into_owned()
}

/// Decode a raw message buffer (as handed out by `GetNextDispatch`).
pub fn parse_recv(buf: &[u8]) -> Recv {
    let Some(id) = rd_u32(buf, 8) else {
        return Recv::Other;
    };
    match id {
        RECV_OPEN => Recv::Open,
        RECV_QUIT => Recv::Quit,
        RECV_EXCEPTION => Recv::Exception(rd_u32(buf, 12).unwrap_or(0)),
        RECV_EVENT => Recv::Event {
            event_id: rd_u32(buf, 16).unwrap_or(u32::MAX),
        },
        RECV_EVENT_FILENAME => Recv::EventFilename {
            event_id: rd_u32(buf, 16).unwrap_or(u32::MAX),
            file: rd_cstr(buf, 24, MAX_PATH),
        },
        RECV_EVENT_FRAME => Recv::EventFrame {
            fps: rd_f32(buf, 24).unwrap_or(0.0),
        },
        RECV_SIMOBJECT_DATA_BYTYPE => {
            let request_id = rd_u32(buf, 12).unwrap_or(u32::MAX);
            let count = rd_u32(buf, 36).unwrap_or(0) as usize;
            let mut values = Vec::with_capacity(count);
            for i in 0..count {
                let off = 40 + i * 8;
                match buf.get(off..off + 8) {
                    Some(b) => values.push(f64::from_le_bytes([
                        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
                    ])),
                    None => break,
                }
            }
            Recv::SimObjectData { request_id, values }
        }
        RECV_SYSTEM_STATE => Recv::SystemState {
            request_id: rd_u32(buf, 12).unwrap_or(u32::MAX),
            text: rd_cstr(buf, 24, MAX_PATH),
        },
        _ => Recv::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(id: u32, total: usize) -> Vec<u8> {
        let mut b = vec![0u8; total];
        b[0..4].copy_from_slice(&(total as u32).to_le_bytes());
        b[4..8].copy_from_slice(&1u32.to_le_bytes());
        b[8..12].copy_from_slice(&id.to_le_bytes());
        b
    }

    #[test]
    fn parses_simple_messages() {
        assert_eq!(parse_recv(&header(RECV_OPEN, 64)), Recv::Open);
        assert_eq!(parse_recv(&header(RECV_QUIT, 12)), Recv::Quit);
        assert_eq!(parse_recv(&header(99, 12)), Recv::Other);
        assert_eq!(parse_recv(&[1, 2]), Recv::Other);
    }

    #[test]
    fn parses_exception_and_event() {
        let mut b = header(RECV_EXCEPTION, 24);
        b[12..16].copy_from_slice(&7u32.to_le_bytes());
        assert_eq!(parse_recv(&b), Recv::Exception(7));
        assert!(is_ignored_exception(7));
        assert!(!is_ignored_exception(1));

        let mut e = header(RECV_EVENT, 24);
        e[16..20].copy_from_slice(&3u32.to_le_bytes());
        assert_eq!(parse_recv(&e), Recv::Event { event_id: 3 });
    }

    #[test]
    fn parses_sim_object_data() {
        let mut b = header(RECV_SIMOBJECT_DATA_BYTYPE, 40 + 16);
        b[12..16].copy_from_slice(&2u32.to_le_bytes()); // request id
        b[36..40].copy_from_slice(&2u32.to_le_bytes()); // define count
        b[40..48].copy_from_slice(&1.5f64.to_le_bytes());
        b[48..56].copy_from_slice(&(-2.0f64).to_le_bytes());
        assert_eq!(
            parse_recv(&b),
            Recv::SimObjectData {
                request_id: 2,
                values: vec![1.5, -2.0]
            }
        );
    }

    #[test]
    fn truncated_data_does_not_panic() {
        let mut b = header(RECV_SIMOBJECT_DATA_BYTYPE, 44);
        b[36..40].copy_from_slice(&10u32.to_le_bytes());
        match parse_recv(&b) {
            Recv::SimObjectData { values, .. } => assert!(values.is_empty()),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn parses_strings_and_frame() {
        let mut s = header(RECV_SYSTEM_STATE, 24 + MAX_PATH);
        s[12..16].copy_from_slice(&0u32.to_le_bytes());
        let path = br"C:\Packages\Some_Plane\aircraft.cfg";
        s[24..24 + path.len()].copy_from_slice(path);
        assert_eq!(
            parse_recv(&s),
            Recv::SystemState {
                request_id: 0,
                text: String::from_utf8_lossy(path).into_owned()
            }
        );

        let mut f = header(RECV_EVENT_FRAME, 32);
        f[24..28].copy_from_slice(&59.5f32.to_le_bytes());
        assert_eq!(parse_recv(&f), Recv::EventFrame { fps: 59.5 });
    }
}
