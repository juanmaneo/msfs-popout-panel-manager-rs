//! Process lookup and process memory access (used by Dynamic LOD).

use super::from_wide;
use crate::lod::LodMemory;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::Debug::{ReadProcessMemory, WriteProcessMemory};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW, PROCESSENTRY32W,
    Process32FirstW, Process32NextW, TH32CS_SNAPMODULE, TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_VM_OPERATION, PROCESS_VM_READ, PROCESS_VM_WRITE,
};

pub const SIM_PROCESS_NAME: &str = "FlightSimulator.exe";

/// Find the first running process with the given executable name (case-insensitive).
pub fn find_process_id(exe_name: &str) -> Option<u32> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut result = None;
        let mut ok = Process32FirstW(snap, &mut entry);
        while ok != 0 {
            if from_wide(&entry.szExeFile).eq_ignore_ascii_case(exe_name) {
                result = Some(entry.th32ProcessID);
                break;
            }
            ok = Process32NextW(snap, &mut entry);
        }
        CloseHandle(snap);
        result
    }
}

pub fn is_sim_running() -> bool {
    find_process_id(SIM_PROCESS_NAME).is_some()
}

/// Base address of a module loaded in the process.
pub fn module_base(pid: u32, module_name: &str) -> Option<i64> {
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid);
        if snap == INVALID_HANDLE_VALUE {
            return None;
        }
        let mut entry: MODULEENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
        let mut result = None;
        let mut ok = Module32FirstW(snap, &mut entry);
        while ok != 0 {
            if from_wide(&entry.szModule).eq_ignore_ascii_case(module_name) {
                result = Some(entry.modBaseAddr as i64);
                break;
            }
            ok = Module32NextW(snap, &mut entry);
        }
        CloseHandle(snap);
        result
    }
}

/// An open process handle with VM read/write access.
pub struct ProcessMemory {
    handle: usize,
}

impl ProcessMemory {
    pub fn open(pid: u32) -> Option<Self> {
        let h = unsafe {
            OpenProcess(
                PROCESS_VM_OPERATION | PROCESS_VM_READ | PROCESS_VM_WRITE,
                0,
                pid,
            )
        };
        if h.is_null() {
            None
        } else {
            Some(Self { handle: h as usize })
        }
    }

    fn read<const N: usize>(&self, addr: i64) -> [u8; N] {
        let mut buf = [0u8; N];
        if addr > 0 {
            let mut read = 0usize;
            unsafe {
                ReadProcessMemory(
                    self.handle as HANDLE,
                    addr as usize as *const _,
                    buf.as_mut_ptr() as *mut _,
                    N,
                    &mut read,
                );
            }
        }
        buf
    }

    fn write(&self, addr: i64, data: &[u8]) {
        if addr <= 0 {
            return;
        }
        let mut written = 0usize;
        unsafe {
            WriteProcessMemory(
                self.handle as HANDLE,
                addr as usize as *mut _,
                data.as_ptr() as *const _,
                data.len(),
                &mut written,
            );
        }
    }
}

impl Drop for ProcessMemory {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle as HANDLE);
        }
    }
}

impl LodMemory for ProcessMemory {
    fn read_i64(&self, addr: i64) -> i64 {
        i64::from_le_bytes(self.read::<8>(addr))
    }

    fn read_i32(&self, addr: i64) -> i32 {
        i32::from_le_bytes(self.read::<4>(addr))
    }

    fn read_f32(&self, addr: i64) -> f32 {
        f32::from_le_bytes(self.read::<4>(addr))
    }

    fn read_u8(&self, addr: i64) -> u8 {
        self.read::<1>(addr)[0]
    }

    fn write_i32(&self, addr: i64, v: i32) {
        self.write(addr, &v.to_le_bytes());
    }

    fn write_f32(&self, addr: i64, v: f32) {
        self.write(addr, &v.to_le_bytes());
    }
}
