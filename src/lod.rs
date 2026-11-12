//! Dynamic LOD: adjusts MSFS' terrain/object level of detail in memory to hold a target FPS
//! (port of `DynamicLodOrchestrator.cs`).
//!
//! The decision logic is independent of the process-memory access, which is abstracted by
//! [`LodMemory`] so it can be unit tested.

use crate::flight_sim::{DynamicLodSimData, to_i32};
use crate::model::DynamicLodSetting;
use std::time::{Duration, Instant};

pub const SIM_MODULE_NAME: &str = "WwiseLibPCx64P.dll";

const OFFSET_MODULE_BASE: i64 = 0x004B_2368;
const OFFSET_POINTER_MAIN: i64 = 0x3D0;
const OFFSET_POINTER_TLOD: i64 = 0xC;
const OFFSET_POINTER_OLOD: i64 = 0xC;
const OFFSET_POINTER_CLOUD_QUALITY: i64 = 0x44;
const OFFSET_POINTER_FG_MODE: i64 = 0x4A;
const OFFSET_POINTER_ANISO_FILTER: i64 = -0x18;
const OFFSET_POINTER_WATER_WAVES: i64 = 0x3C;

/// Raw access to the simulator's memory. Reads return zero on failure.
pub trait LodMemory {
    fn read_i64(&self, addr: i64) -> i64;
    fn read_i32(&self, addr: i64) -> i32;
    fn read_f32(&self, addr: i64) -> f32;
    fn read_u8(&self, addr: i64) -> u8;
    fn write_i32(&self, addr: i64, v: i32);
    fn write_f32(&self, addr: i64, v: f32);
}

#[derive(Debug)]
pub struct DynamicLod {
    active: bool,
    addr_tlod: i64,
    addr_olod: i64,
    addr_cloud: i64,
    addr_fg: i64,
    last_update: Instant,
    cloud_quality_decreased: bool,
}

impl Default for DynamicLod {
    fn default() -> Self {
        Self::new()
    }
}

impl DynamicLod {
    pub fn new() -> Self {
        Self {
            active: false,
            addr_tlod: 0,
            addr_olod: 0,
            addr_cloud: 0,
            addr_fg: 0,
            last_update: Instant::now(),
            cloud_quality_decreased: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Resolve the setting addresses from the sim module base address and validate them.
    pub fn attach(
        &mut self,
        mem: &dyn LodMemory,
        module_base: i64,
        now: Instant,
    ) -> Result<(), String> {
        if self.active {
            return Ok(());
        }
        if module_base <= 0 {
            return Err("simulator module not found".into());
        }
        let main = mem.read_i64(module_base + OFFSET_MODULE_BASE) + OFFSET_POINTER_MAIN;
        if main > 0 {
            self.addr_tlod = mem.read_i64(main) + OFFSET_POINTER_TLOD;
            self.addr_olod = self.addr_tlod + OFFSET_POINTER_OLOD;
            self.addr_cloud = self.addr_tlod + OFFSET_POINTER_CLOUD_QUALITY;
            self.addr_fg = self.addr_tlod - OFFSET_POINTER_FG_MODE;

            if !self.memory_boundary_test(mem) {
                return Err("Unable to validate memory space for Dynamic LOD".into());
            }
        }
        self.active = true;
        self.last_update = now;
        self.cloud_quality_decreased = false;
        Ok(())
    }

    /// Optionally restore the configured reset values, then stop adjusting.
    pub fn detach(&mut self, mem: &dyn LodMemory, setting: &DynamicLodSetting) {
        if self.active && setting.reset_enabled {
            mem.write_f32(self.addr_tlod, setting.reset_tlod as f32 / 100.0);
            mem.write_f32(self.addr_olod, setting.reset_olod as f32 / 100.0);
        }
        self.active = false;
    }

    pub fn read_tlod(&self, mem: &dyn LodMemory) -> i32 {
        to_i32((mem.read_f32(self.addr_tlod) * 100.0) as f64)
    }

    pub fn read_olod(&self, mem: &dyn LodMemory) -> i32 {
        to_i32((mem.read_f32(self.addr_olod) * 100.0) as f64)
    }

    pub fn read_cloud_quality_value(&self, mem: &dyn LodMemory) -> i32 {
        mem.read_i32(self.addr_cloud)
    }

    pub fn read_cloud_quality(&self, mem: &dyn LodMemory) -> &'static str {
        cloud_quality_name(self.read_cloud_quality_value(mem))
    }

    /// Frame generation (DLSS 3 / FSR 3) active?
    pub fn read_is_fg(&self, mem: &dyn LodMemory) -> bool {
        mem.read_u8(self.addr_fg) == 1
    }

    fn memory_boundary_test(&self, mem: &dyn LodMemory) -> bool {
        let tlod = self.read_tlod(mem);
        let olod = self.read_olod(mem);
        if !(10..=1000).contains(&tlod) || !(10..=1000).contains(&olod) {
            return false;
        }
        if self.read_cloud_quality(mem) == "N/A" {
            return false;
        }
        let aniso = mem.read_i32(self.addr_tlod + OFFSET_POINTER_ANISO_FILTER);
        if !(1..=16).contains(&aniso) {
            return false;
        }
        matches!(
            mem.read_i32(self.addr_tlod + OFFSET_POINTER_WATER_WAVES),
            128 | 256 | 512
        )
    }

    /// Run one adjustment step. Returns `true` if something was written.
    pub fn update(
        &mut self,
        mem: &dyn LodMemory,
        setting: &DynamicLodSetting,
        sim: &DynamicLodSimData,
        in_cockpit_and_flying: bool,
        now: Instant,
    ) -> bool {
        if !self.active || !in_cockpit_and_flying {
            return false;
        }
        if now.saturating_duration_since(self.last_update) <= Duration::from_secs(3) {
            return false;
        }

        let delta_fps = sim.fps - setting.targeted_fps;
        if (delta_fps.abs() as f64)
            < setting.targeted_fps as f64 * setting.fps_tolerance as f64 / 100.0
        {
            return false;
        }
        self.last_update = now;

        let wrote_tlod = self.set_tlod(mem, setting, sim, delta_fps);
        let wrote_olod = self.set_olod(mem, setting, sim);
        wrote_tlod || wrote_olod
    }

    fn set_tlod(
        &mut self,
        mem: &dyn LodMemory,
        setting: &DynamicLodSetting,
        sim: &DynamicLodSimData,
        delta_fps: i32,
    ) -> bool {
        let new_tlod = compute_tlod(setting, sim, delta_fps);
        if self.read_tlod(mem) == new_tlod {
            return false;
        }

        let near_ground = sim.alt_above_ground <= setting.alt_tlod_base as f64;
        if setting.decrease_cloud_quality && !setting.tlod_min_on_ground && !near_ground {
            if delta_fps < 0
                && new_tlod < setting.cloud_recovery_tlod
                && !self.cloud_quality_decreased
            {
                self.cloud_quality_decreased = true;
                mem.write_i32(self.addr_cloud, self.read_cloud_quality_value(mem) - 1);
                // extra delay for the cloud setting to take effect
                self.last_update += Duration::from_secs(2);
                return true;
            }
            if delta_fps > 0
                && new_tlod >= setting.cloud_recovery_tlod
                && self.cloud_quality_decreased
            {
                self.cloud_quality_decreased = false;
                mem.write_i32(self.addr_cloud, self.read_cloud_quality_value(mem) + 1);
                self.last_update += Duration::from_secs(2);
                return true;
            }
        }

        mem.write_f32(self.addr_tlod, new_tlod as f32 / 100.0);
        true
    }

    fn set_olod(
        &mut self,
        mem: &dyn LodMemory,
        setting: &DynamicLodSetting,
        sim: &DynamicLodSimData,
    ) -> bool {
        let new_olod = compute_olod(setting, sim.alt_above_ground);
        if self.read_olod(mem) == new_olod {
            return false;
        }
        mem.write_f32(self.addr_olod, new_olod as f32 / 100.0);
        true
    }
}

pub fn cloud_quality_name(v: i32) -> &'static str {
    match v {
        0 => "Low",
        1 => "Medium",
        2 => "High",
        3 => "Ultra",
        _ => "N/A",
    }
}

/// Target TLOD (percent) for the given FPS error.
pub fn compute_tlod(setting: &DynamicLodSetting, sim: &DynamicLodSimData, delta_fps: i32) -> i32 {
    let step = 10.max((delta_fps / 2).abs());
    let new_tlod = sim.tlod + delta_fps.signum() * step;
    if (setting.tlod_min_on_ground && sim.alt_above_ground <= setting.alt_tlod_base as f64)
        || new_tlod < setting.tlod_min
    {
        setting.tlod_min
    } else if new_tlod > setting.tlod_max {
        setting.tlod_max
    } else {
        new_tlod
    }
}

/// Target OLOD (percent), interpolated linearly between the two altitude bounds.
pub fn compute_olod(setting: &DynamicLodSetting, alt_above_ground: f64) -> i32 {
    if alt_above_ground < setting.alt_olod_base as f64 {
        setting.olod_base
    } else if alt_above_ground > setting.alt_olod_top as f64 {
        setting.olod_top
    } else {
        let base = setting.olod_base as f64;
        let top = setting.olod_top as f64;
        let span = (setting.alt_olod_top - setting.alt_olod_base) as f64;
        to_i32(base - (base - top) * (alt_above_ground - setting.alt_olod_base as f64) / span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Default)]
    struct FakeMem {
        bytes: RefCell<HashMap<i64, Vec<u8>>>,
    }

    impl FakeMem {
        fn set_i64(&self, a: i64, v: i64) {
            self.bytes.borrow_mut().insert(a, v.to_le_bytes().to_vec());
        }
        fn set_i32(&self, a: i64, v: i32) {
            self.bytes.borrow_mut().insert(a, v.to_le_bytes().to_vec());
        }
        fn set_f32(&self, a: i64, v: f32) {
            self.bytes.borrow_mut().insert(a, v.to_le_bytes().to_vec());
        }
        fn get(&self, a: i64, n: usize) -> Vec<u8> {
            self.bytes
                .borrow()
                .get(&a)
                .cloned()
                .unwrap_or_else(|| vec![0; n])
        }
    }

    impl LodMemory for FakeMem {
        fn read_i64(&self, a: i64) -> i64 {
            i64::from_le_bytes(self.get(a, 8)[..8].try_into().unwrap())
        }
        fn read_i32(&self, a: i64) -> i32 {
            i32::from_le_bytes(self.get(a, 4)[..4].try_into().unwrap())
        }
        fn read_f32(&self, a: i64) -> f32 {
            f32::from_le_bytes(self.get(a, 4)[..4].try_into().unwrap())
        }
        fn read_u8(&self, a: i64) -> u8 {
            self.get(a, 1)[0]
        }
        fn write_i32(&self, a: i64, v: i32) {
            self.set_i32(a, v)
        }
        fn write_f32(&self, a: i64, v: f32) {
            self.set_f32(a, v)
        }
    }

    const BASE: i64 = 0x1_0000_0000;

    fn setup() -> (FakeMem, i64) {
        let mem = FakeMem::default();
        let main_ptr = BASE + OFFSET_MODULE_BASE;
        let main = 0x2000_0000;
        mem.set_i64(main_ptr, main - OFFSET_POINTER_MAIN);
        let tlod_base = 0x3000_0000;
        mem.set_i64(main, tlod_base);
        let tlod = tlod_base + OFFSET_POINTER_TLOD;
        mem.set_f32(tlod, 1.0);
        mem.set_f32(tlod + OFFSET_POINTER_OLOD, 2.0);
        mem.set_i32(tlod + OFFSET_POINTER_CLOUD_QUALITY, 2);
        mem.set_i32(tlod + OFFSET_POINTER_ANISO_FILTER, 8);
        mem.set_i32(tlod + OFFSET_POINTER_WATER_WAVES, 256);
        (mem, tlod)
    }

    #[test]
    fn attach_validates_memory() {
        let (mem, tlod) = setup();
        let mut lod = DynamicLod::new();
        lod.attach(&mem, BASE, Instant::now()).unwrap();
        assert!(lod.is_active());
        assert_eq!(lod.read_tlod(&mem), 100);
        assert_eq!(lod.read_olod(&mem), 200);
        assert_eq!(lod.read_cloud_quality(&mem), "High");
        assert!(!lod.read_is_fg(&mem));

        // corrupt water waves -> validation fails
        mem.set_i32(tlod + OFFSET_POINTER_WATER_WAVES, 7);
        let mut lod2 = DynamicLod::new();
        assert!(lod2.attach(&mem, BASE, Instant::now()).is_err());
        assert!(!lod2.is_active());
    }

    #[test]
    fn attach_fails_without_module() {
        let (mem, _) = setup();
        let mut lod = DynamicLod::new();
        assert!(lod.attach(&mem, 0, Instant::now()).is_err());
        assert!(lod.attach(&mem, -1, Instant::now()).is_err());
    }

    #[test]
    fn tlod_steps_toward_target_fps() {
        let s = DynamicLodSetting {
            tlod_min_on_ground: false,
            ..DynamicLodSetting::default()
        };
        let mut sim = DynamicLodSimData {
            tlod: 100,
            alt_above_ground: 5000.0,
            ..Default::default()
        };
        // fps 20 over target: step = max(10, 10) = 10 -> up
        assert_eq!(compute_tlod(&s, &sim, 20), 110);
        // fps 40 under target: step = 20 -> down
        assert_eq!(compute_tlod(&s, &sim, -40), 80);
        // clamped to min / max
        sim.tlod = 55;
        assert_eq!(compute_tlod(&s, &sim, -40), s.tlod_min);
        sim.tlod = 195;
        assert_eq!(compute_tlod(&s, &sim, 40), s.tlod_max);
    }

    #[test]
    fn tlod_is_minimum_near_ground_when_enabled() {
        let s = DynamicLodSetting::default(); // tlod_min_on_ground = true
        let sim = DynamicLodSimData {
            tlod: 150,
            alt_above_ground: 500.0,
            ..Default::default()
        };
        assert_eq!(compute_tlod(&s, &sim, 30), s.tlod_min);
    }

    #[test]
    fn olod_interpolates_between_altitudes() {
        let s = DynamicLodSetting::default(); // base 200 @1000ft, top 50 @10000ft
        assert_eq!(compute_olod(&s, 0.0), 200);
        assert_eq!(compute_olod(&s, 20000.0), 50);
        assert_eq!(compute_olod(&s, 5500.0), 125);
    }

    #[test]
    fn update_respects_interval_and_tolerance() {
        let (mem, _) = setup();
        let mut lod = DynamicLod::new();
        let t0 = Instant::now();
        lod.attach(&mem, BASE, t0).unwrap();
        let s = DynamicLodSetting {
            tlod_min_on_ground: false,
            ..DynamicLodSetting::default()
        };
        let sim = DynamicLodSimData {
            tlod: 100,
            olod: 200,
            alt_above_ground: 5500.0,
            fps: 30, // 30 under target 60
            ..Default::default()
        };

        // too soon after attach
        assert!(!lod.update(&mem, &s, &sim, true, t0 + Duration::from_secs(1)));
        // not in cockpit
        assert!(!lod.update(&mem, &s, &sim, false, t0 + Duration::from_secs(5)));
        // within tolerance
        let mut near = sim.clone();
        near.fps = 61;
        assert!(!lod.update(&mem, &s, &near, true, t0 + Duration::from_secs(5)));

        assert!(lod.update(&mem, &s, &sim, true, t0 + Duration::from_secs(5)));
        assert_eq!(lod.read_tlod(&mem), 85); // step = max(10, 15) = 15 -> 100 - 15
        assert_eq!(lod.read_olod(&mem), 125);

        // a second update right away is throttled
        assert!(!lod.update(&mem, &s, &sim, true, t0 + Duration::from_secs(6)));
    }

    #[test]
    fn cloud_quality_decreases_then_recovers() {
        let (mem, _) = setup();
        let mut lod = DynamicLod::new();
        let t0 = Instant::now();
        lod.attach(&mem, BASE, t0).unwrap();
        let s = DynamicLodSetting {
            tlod_min_on_ground: false,
            decrease_cloud_quality: true,
            cloud_recovery_tlod: 100,
            ..DynamicLodSetting::default()
        };
        let mut sim = DynamicLodSimData {
            tlod: 100,
            olod: 200,
            alt_above_ground: 5500.0,
            fps: 30,
            ..Default::default()
        };
        assert!(lod.update(&mem, &s, &sim, true, t0 + Duration::from_secs(5)));
        assert_eq!(lod.read_cloud_quality_value(&mem), 1); // High -> Medium

        // fps recovers: new tlod >= recovery -> quality restored
        sim.tlod = 100;
        sim.fps = 90;
        assert!(lod.update(&mem, &s, &sim, true, t0 + Duration::from_secs(20)));
        assert_eq!(lod.read_cloud_quality_value(&mem), 2);
    }

    #[test]
    fn detach_restores_reset_values() {
        let (mem, _) = setup();
        let mut lod = DynamicLod::new();
        lod.attach(&mem, BASE, Instant::now()).unwrap();
        let s = DynamicLodSetting {
            reset_enabled: true,
            reset_tlod: 120,
            reset_olod: 80,
            ..DynamicLodSetting::default()
        };
        lod.detach(&mem, &s);
        assert_eq!(lod.read_tlod(&mem), 120);
        assert_eq!(lod.read_olod(&mem), 80);
        assert!(!lod.is_active());
    }
}
