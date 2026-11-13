//! SimConnect client running on its own thread (port of `SimConnector` / `SimConnectProvider`).
//!
//! The thread owns the connection: it (re)connects every two seconds while MSFS is not
//! available, polls the required / HUD bar / dynamic LOD data on timers, and executes the
//! commands it receives. Everything it learns is reported as [`SimEvent`]s.

pub mod ffi;

use crate::model::HudBarType;
use crate::simdefs::*;
use ffi::{Connection, Dispatch, Recv, SimConnectLib};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

const SIM_RATE_SETTLE_MS: u64 = 200;

const APP_NAME: &str = "MSFS Pop Out Panel Manager";
const CONNECT_RETRY: Duration = Duration::from_millis(2000);
const REQUIRED_INTERVAL: Duration = Duration::from_millis(500);
const HUDBAR_INTERVAL: Duration = Duration::from_millis(200);
const LOD_INTERVAL: Duration = Duration::from_millis(300);
const TICK: Duration = Duration::from_millis(20);

#[derive(Debug, Clone)]
pub enum SimCmd {
    SetData(WritableVar, f64),
    Transmit(ActionEvent, u32),
    /// Start (`Some`) or stop (`None`) polling HUD bar data.
    HudBar(Option<HudBarType>),
    /// Start or stop polling dynamic LOD data and frame events.
    DynamicLod(bool),
    Shutdown,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SimEvent {
    Connected,
    Disconnected,
    Required(RequiredData),
    HudBar(Vec<(HudProp, f64)>),
    Lod(LodData),
    Frame(i32),
    /// Path of the active aircraft's `aircraft.cfg` (or similar).
    AircraftPath(String),
    /// A problem worth showing the user (e.g. missing DLL).
    Error(String),
}

/// Command sender for the SimConnect thread.
#[derive(Clone)]
pub struct SimHandle {
    tx: Sender<SimCmd>,
}

impl SimHandle {
    pub fn send(&self, cmd: SimCmd) {
        let _ = self.tx.send(cmd);
    }

    pub fn set_data(&self, var: WritableVar, value: f64) {
        self.send(SimCmd::SetData(var, value));
    }

    pub fn transmit(&self, event: ActionEvent, data: u32) {
        self.send(SimCmd::Transmit(event, data));
    }
}

pub fn spawn(emit: impl Fn(SimEvent) + Send + 'static) -> std::io::Result<SimHandle> {
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("popm-simconnect".into())
        .spawn(move || run(rx, emit))?;
    Ok(SimHandle { tx })
}

/// What the polling loop should do after a received message.
#[derive(Debug, PartialEq)]
enum Translated {
    Event(SimEvent),
    /// The connection is unusable (quit message or a real exception).
    Lost,
    Ignore,
}

fn translate(r: Recv, hud_props: &[HudProp]) -> Translated {
    match r {
        Recv::Quit => Translated::Lost,
        Recv::Exception(code) if ffi::is_ignored_exception(code) => Translated::Ignore,
        Recv::Exception(_) => Translated::Lost,
        Recv::EventFilename { event_id, file } if event_id == EVT_AIRCRAFT_LOADED => {
            Translated::Event(SimEvent::AircraftPath(file))
        }
        Recv::SystemState { request_id, text } if request_id == REQ_SYSTEM_STATE_AIRCRAFT_PATH => {
            Translated::Event(SimEvent::AircraftPath(text))
        }
        Recv::EventFrame { fps } => {
            Translated::Event(SimEvent::Frame(crate::flight_sim::to_i32(fps as f64)))
        }
        Recv::SimObjectData { request_id, values } => match request_id {
            REQ_REQUIRED => RequiredData::from_values(&values)
                .map(|d| Translated::Event(SimEvent::Required(d)))
                .unwrap_or(Translated::Ignore),
            REQ_HUDBAR => {
                let pairs: Vec<(HudProp, f64)> = hud_props
                    .iter()
                    .copied()
                    .zip(values.iter().copied())
                    .collect();
                if pairs.is_empty() {
                    Translated::Ignore
                } else {
                    Translated::Event(SimEvent::HudBar(pairs))
                }
            }
            REQ_DYNAMIC_LOD => LodData::from_values(&values)
                .map(|d| Translated::Event(SimEvent::Lod(d)))
                .unwrap_or(Translated::Ignore),
            _ => Translated::Ignore,
        },
        _ => Translated::Ignore,
    }
}

#[derive(Default)]
struct Desired {
    hud: Option<HudBarType>,
    lod: bool,
}

enum SessionEnd {
    Shutdown,
    Lost,
}

fn run(rx: Receiver<SimCmd>, emit: impl Fn(SimEvent)) {
    let mut lib: Option<SimConnectLib> = None;
    let mut warned = false;
    let mut desired = Desired::default();

    loop {
        if lib.is_none() {
            match SimConnectLib::load() {
                Ok(l) => lib = Some(l),
                Err(e) => {
                    if !warned {
                        warned = true;
                        log::error!("{e}");
                        emit(SimEvent::Error(e));
                    }
                }
            }
        }

        let conn = lib
            .as_ref()
            .and_then(|l| Connection::open(l, APP_NAME).ok());
        if let Some(conn) = conn
            && setup(&conn).is_ok() {
                emit(SimEvent::Connected);
                let end = session(&conn, &rx, &mut desired, &emit);
                emit(SimEvent::Disconnected);
                drop(conn);
                if matches!(end, SessionEnd::Shutdown) {
                    return;
                }
            }

        if idle(&rx, &mut desired, CONNECT_RETRY) {
            return;
        }
    }
}

/// Wait while disconnected, remembering desired state. Returns `true` on shutdown.
fn idle(rx: &Receiver<SimCmd>, desired: &mut Desired, wait: Duration) -> bool {
    let deadline = Instant::now() + wait;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return false;
        }
        match rx.recv_timeout(left) {
            Ok(SimCmd::Shutdown) | Err(RecvTimeoutError::Disconnected) => return true,
            Ok(SimCmd::HudBar(t)) => desired.hud = t,
            Ok(SimCmd::DynamicLod(on)) => desired.lod = on,
            Ok(_) => {} // data writes while disconnected are dropped
            Err(RecvTimeoutError::Timeout) => return false,
        }
    }
}

fn setup(c: &Connection) -> Result<(), i32> {
    for (evt, name) in [
        (EVT_SIM_START, "SimStart"),
        (EVT_SIM_STOP, "SimStop"),
        (EVT_VIEW, "View"),
        (EVT_AIRCRAFT_LOADED, "AircraftLoaded"),
    ] {
        let _ = c.unsubscribe_from_system_event(evt);
        c.subscribe_to_system_event(evt, name)?;
    }

    for (name, units) in REQUIRED_VARS {
        c.add_to_data_definition(DEF_REQUIRED, name, units)?;
    }
    for w in WritableVar::ALL {
        let (name, units) = w.sim_var();
        c.add_to_data_definition(w.definition_id(), name, units)?;
    }
    for e in ActionEvent::ALL {
        c.map_client_event(e as u32, e.sim_name())?;
    }
    c.request_system_state(REQ_SYSTEM_STATE_AIRCRAFT_PATH, "AircraftLoaded")?;
    Ok(())
}

fn apply_hud(c: &Connection, t: Option<HudBarType>) -> Vec<HudProp> {
    let _ = c.clear_data_definition(DEF_HUDBAR);
    let mut props = Vec::new();
    if let Some(t) = t {
        for (prop, (name, units)) in hud_bar_vars(t) {
            if c.add_to_data_definition(DEF_HUDBAR, name, units).is_ok() {
                props.push(prop);
            }
        }
    }
    props
}

fn apply_lod(c: &Connection, on: bool) {
    let _ = c.unsubscribe_from_system_event(EVT_FRAME);
    if on {
        let _ = c.clear_data_definition(DEF_DYNAMIC_LOD);
        for (name, units) in DYNAMIC_LOD_VARS {
            let _ = c.add_to_data_definition(DEF_DYNAMIC_LOD, name, units);
        }
        let _ = c.subscribe_to_system_event(EVT_FRAME, "Frame");
    }
}

fn session(
    c: &Connection,
    rx: &Receiver<SimCmd>,
    desired: &mut Desired,
    emit: &impl Fn(SimEvent),
) -> SessionEnd {
    let mut hud_props = apply_hud(c, desired.hud);
    apply_lod(c, desired.lod);

    let now = Instant::now();
    let mut last_required = now - REQUIRED_INTERVAL;
    let mut last_hud = now;
    let mut last_lod = now;

    loop {
        match rx.recv_timeout(TICK) {
            Ok(SimCmd::Shutdown) | Err(RecvTimeoutError::Disconnected) => {
                return SessionEnd::Shutdown;
            }
            Ok(SimCmd::SetData(var, v)) => {
                if let Err(hr) = c.set_data_f64(var.definition_id(), v) {
                    log::error!("SimConnect: unable to set {var:?}: 0x{hr:08X}");
                }
            }
            Ok(SimCmd::Transmit(evt, data)) => {
                if let Err(hr) = c.transmit_client_event(evt as u32, data) {
                    log::error!(
                        "SimConnect: unable to transmit {}: 0x{hr:08X}",
                        evt.sim_name()
                    );
                }
                // Upstream waits after each sim rate change so rapid clicks cannot overshoot
                // before the new rate is read back.
                if matches!(evt, ActionEvent::SimRateIncr | ActionEvent::SimRateDecr) {
                    std::thread::sleep(Duration::from_millis(SIM_RATE_SETTLE_MS));
                }
            }
            Ok(SimCmd::HudBar(t)) => {
                desired.hud = t;
                hud_props = apply_hud(c, t);
                last_hud = Instant::now();
            }
            Ok(SimCmd::DynamicLod(on)) => {
                desired.lod = on;
                apply_lod(c, on);
                last_lod = Instant::now();
            }
            Err(RecvTimeoutError::Timeout) => {}
        }

        // drain everything the simulator sent
        loop {
            match c.next_dispatch() {
                Ok(Dispatch::Message(ptr, size)) => {
                    let bytes =
                        unsafe { std::slice::from_raw_parts(ptr as *const u8, size as usize) };
                    match translate(ffi::parse_recv(bytes), &hud_props) {
                        Translated::Event(e) => emit(e),
                        Translated::Lost => return SessionEnd::Lost,
                        Translated::Ignore => {}
                    }
                }
                Ok(Dispatch::Empty) => break,
                Err(hr) => {
                    log::error!("SimConnect: receive failed: 0x{hr:08X}");
                    return SessionEnd::Lost;
                }
            }
        }

        let now = Instant::now();
        if now.duration_since(last_required) >= REQUIRED_INTERVAL {
            last_required = now;
            if c.request_data_on_user(REQ_REQUIRED, DEF_REQUIRED).is_err() {
                return SessionEnd::Lost;
            }
        }
        if desired.hud.is_some()
            && !hud_props.is_empty()
            && now.duration_since(last_hud) >= HUDBAR_INTERVAL
        {
            last_hud = now;
            if c.request_data_on_user(REQ_HUDBAR, DEF_HUDBAR).is_err() {
                return SessionEnd::Lost;
            }
        }
        if desired.lod && now.duration_since(last_lod) >= LOD_INTERVAL {
            last_lod = now;
            if c.request_data_on_user(REQ_DYNAMIC_LOD, DEF_DYNAMIC_LOD)
                .is_err()
            {
                return SessionEnd::Lost;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_required_data() {
        let vals: Vec<f64> = (0..10).map(|i| i as f64).collect();
        match translate(
            Recv::SimObjectData {
                request_id: REQ_REQUIRED,
                values: vals,
            },
            &[],
        ) {
            Translated::Event(SimEvent::Required(d)) => assert_eq!(d.camera_state, 4.0),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(
            translate(
                Recv::SimObjectData {
                    request_id: REQ_REQUIRED,
                    values: vec![1.0]
                },
                &[]
            ),
            Translated::Ignore
        );
    }

    #[test]
    fn translates_hud_bar_data_by_definition_order() {
        let props = [HudProp::SimRate, HudProp::Flap];
        match translate(
            Recv::SimObjectData {
                request_id: REQ_HUDBAR,
                values: vec![2.0, 5.0],
            },
            &props,
        ) {
            Translated::Event(SimEvent::HudBar(v)) => {
                assert_eq!(v, vec![(HudProp::SimRate, 2.0), (HudProp::Flap, 5.0)]);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn exceptions_and_quit() {
        assert_eq!(translate(Recv::Quit, &[]), Translated::Lost);
        assert_eq!(
            translate(Recv::Exception(ffi::EXC_DATA_ERROR), &[]),
            Translated::Ignore
        );
        assert_eq!(translate(Recv::Exception(1), &[]), Translated::Lost);
    }

    #[test]
    fn aircraft_and_frame_events() {
        assert_eq!(
            translate(
                Recv::EventFilename {
                    event_id: EVT_AIRCRAFT_LOADED,
                    file: "a\\b\\c".into()
                },
                &[]
            ),
            Translated::Event(SimEvent::AircraftPath("a\\b\\c".into()))
        );
        assert_eq!(
            translate(
                Recv::EventFilename {
                    event_id: EVT_VIEW,
                    file: "x".into()
                },
                &[]
            ),
            Translated::Ignore
        );
        assert_eq!(
            translate(
                Recv::SystemState {
                    request_id: REQ_SYSTEM_STATE_AIRCRAFT_PATH,
                    text: "p".into()
                },
                &[]
            ),
            Translated::Event(SimEvent::AircraftPath("p".into()))
        );
        assert_eq!(
            translate(Recv::EventFrame { fps: 59.6 }, &[]),
            Translated::Event(SimEvent::Frame(60))
        );
    }

    #[test]
    fn idle_remembers_desired_state_and_stops_on_shutdown() {
        let (tx, rx) = channel();
        let mut d = Desired::default();
        tx.send(SimCmd::HudBar(Some(HudBarType::Pmdg737))).unwrap();
        tx.send(SimCmd::DynamicLod(true)).unwrap();
        assert!(!idle(&rx, &mut d, Duration::from_millis(10)));
        assert_eq!(d.hud, Some(HudBarType::Pmdg737));
        assert!(d.lod);
        tx.send(SimCmd::Shutdown).unwrap();
        assert!(idle(&rx, &mut d, Duration::from_millis(10)));
    }
}
