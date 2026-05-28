//! Mach-based external SDK: PID lookup, task port, vm_read / vm_write,
//! pointer-chain resolution. Same shape as Cherry's `sdk::SdkI`.

use std::sync::atomic::{AtomicU64, Ordering};

unsafe extern "C" {
    fn task_for_pid(target_tport: u32, pid: i32, t: *mut u32) -> i32;
    fn mach_task_self() -> u32;
    fn vm_read_overwrite(
        target_task: u32,
        address: u64,
        size: u64,
        data: u64,
        out_size: *mut u64,
    ) -> i32;
    fn vm_write(target_task: u32, address: u64, data: u64, size: u32) -> i32;
}

fn find_pid(process_name: &str) -> Option<i32> {
    let output = std::process::Command::new("pgrep")
        .arg("-x").arg(process_name)
        .output().ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse::<i32>().ok()
}

/// Walk `vmmap -wide` looking for the `__TEXT` segment of `module_name`
/// and return its base address.
///
/// On Rosetta (Intel binary on ARM macOS) Roblox typically exposes a single
/// `__TEXT` line, so first-match is enough. On **native Intel macOS** the
/// binary often has multiple `__TEXT` segments (hardened-runtime split),
/// and the first one listed by `vmmap` isn't necessarily the actual load
/// address — picking the wrong one yields a `base` that's offset by a few
/// MB, and `read(base + entry)` lands in garbage that still passes range
/// checks. The lowest address among the matching `__TEXT` segments is
/// reliably the binary's true load address on both arches.
fn find_base(pid: i32, module_name: &str) -> Option<u64> {
    let output = std::process::Command::new("vmmap")
        .arg("-wide").arg(pid.to_string())
        .output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lowest: Option<u64> = None;
    for line in stdout.lines() {
        if !(line.contains(module_name) && line.contains("__TEXT")) { continue; }
        for part in line.split_whitespace() {
            if let Some(addr_str) = part.split('-').next() {
                if let Ok(addr) = u64::from_str_radix(addr_str, 16) {
                    if addr > 0 {
                        lowest = Some(match lowest { Some(l) => l.min(addr), None => addr });
                    }
                }
            }
        }
    }
    lowest
}

pub struct SdkI {
    pub task: u32,
    pub pid: i32,
    pub base: u64,
    pub process_name: String,
    pub read_ok: AtomicU64,
    pub read_fail: AtomicU64,
}

impl SdkI {
    /// Attach. Caller must hold the `get-task-allow` entitlement or be root.
    pub fn new(process_name: &str) -> Result<Self, String> {
        let pid = find_pid(process_name)
            .ok_or_else(|| format!("Process '{}' not found", process_name))?;

        let mut task: u32 = 0;
        let kr = unsafe { task_for_pid(mach_task_self(), pid, &mut task) };
        if kr != 0 { return Err(format!("task_for_pid fail (code {}).", kr)); }
        if task == 0 { return Err("Task port is null".to_string()); }

        let base = find_base(pid, process_name)
            .ok_or_else(|| format!("Base of '{}' not found via vmmap", process_name))?;

        Ok(Self {
            task,
            pid,
            base,
            process_name: process_name.to_string(),
            read_ok: AtomicU64::new(0),
            read_fail: AtomicU64::new(0),
        })
    }

    /// `(ok, fail)` counter pair. Useful to detect "attached but all reads
    /// silently fail" (Roblox re-signed itself with hardened runtime).
    pub fn read_stats(&self) -> (u64, u64) {
        (self.read_ok.load(Ordering::Relaxed), self.read_fail.load(Ordering::Relaxed))
    }

    pub fn rebase(&self, offset: u64) -> u64 { self.base + offset }

    pub fn read<T: Copy + Default>(&self, address: u64) -> T {
        let size = std::mem::size_of::<T>() as u64;
        let mut out = T::default();
        let mut out_size: u64 = 0;
        let kr = unsafe {
            vm_read_overwrite(self.task, address, size, &mut out as *mut T as u64, &mut out_size)
        };
        if kr == 0 && out_size == size {
            self.read_ok.fetch_add(1, Ordering::Relaxed);
        } else {
            self.read_fail.fetch_add(1, Ordering::Relaxed);
        }
        out
    }

    pub fn write<T: Copy>(&self, address: u64, value: T) {
        let size = std::mem::size_of::<T>() as u32;
        let mut val = value;
        unsafe {
            vm_write(self.task, address, &mut val as *mut T as u64, size);
        }
    }

    /// Pointer chain. `sdk.resolve(base, &[0xA0, 0x8, 0x100, 0x184])` follows
    /// the first three offsets through `read::<u64>` and adds the last one.
    pub fn resolve(&self, base: u64, offsets: &[u64]) -> u64 {
        let mut addr = base;
        let last = offsets.len() - 1;
        for (i, &off) in offsets.iter().enumerate() {
            if i == last { addr += off; } else { addr = self.read::<u64>(addr + off); }
        }
        addr
    }

    /// Range check for "this looks like a real Roblox heap pointer".
    ///
    /// ARM macOS (and Intel-via-Rosetta) packs the Roblox heap inside the
    /// `(4 GB, 32 GB)` window — the tight range catches obvious garbage.
    /// Native Intel macOS allocates much higher, typically `0x7F…` —
    /// canonical 48-bit user space — so we widen the ceiling for x86_64.
    /// Tightening to require 8-byte alignment removes most low-bit noise
    /// that the old narrow range was incidentally filtering out.
    #[cfg(target_arch = "aarch64")]
    pub fn is_valid_ptr(&self, addr: u64) -> bool {
        addr > 0x100000000 && addr < 0x800000000
    }
    #[cfg(target_arch = "x86_64")]
    pub fn is_valid_ptr(&self, addr: u64) -> bool {
        addr > 0x100000000 && addr < 0x0000_8000_0000_0000
    }

    /// Raw bulk read — used by the signature scanner.
    pub(crate) fn vm_read_bulk(&self, address: u64, size: usize) -> Vec<u8> {
        let mut buf = vec![0u8; size];
        let mut got = 0u64;
        unsafe {
            vm_read_overwrite(self.task, address, size as u64, buf.as_mut_ptr() as u64, &mut got);
        }
        buf.truncate(got as usize);
        buf
    }
}
