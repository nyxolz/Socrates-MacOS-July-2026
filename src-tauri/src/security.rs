
use std::ffi::CStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::Duration;

use obfstr::obfstr;
use once_cell::sync::Lazy;

unsafe extern "C" {
    fn _dyld_image_count() -> u32;
    fn _dyld_get_image_name(idx: u32) -> *const libc::c_char;
}

// ─── Frida + hook framework scan ────────────────────────────────────

pub fn frida_check() -> bool {
    let needles: [String; 7] = [
        obfstr!("FridaGadget").to_string(),
        obfstr!("frida-agent").to_string(),
        obfstr!("frida-gum").to_string(),
        obfstr!("CydiaSubstrate").to_string(),
        obfstr!("substitute").to_string(),
        obfstr!("MobileSubstrate").to_string(),
        obfstr!("libsubstitute").to_string(),
    ];
    unsafe {
        let n = _dyld_image_count();
        for i in 0..n {
            let p = _dyld_get_image_name(i);
            if p.is_null() { continue; }
            let s = match CStr::from_ptr(p).to_str() { Ok(s) => s, Err(_) => continue };
            for needle in &needles {
                if s.contains(needle.as_str()) { return true; }
            }
        }
    }
    false
}

// ─── Parent process name check ──────────────────────────────────────

pub fn parent_name() -> String {
    unsafe {
        let ppid = libc::getppid();
        if ppid <= 1 { return String::new(); }
        let mut buf = vec![0u8; 4096];
        let rc = libc::proc_pidpath(ppid, buf.as_mut_ptr() as *mut libc::c_void, buf.len() as u32);
        if rc <= 0 { return String::new(); }
        let s = match std::str::from_utf8(&buf[..rc as usize]) { Ok(s) => s, Err(_) => return String::new() };
        s.rsplit('/').next().unwrap_or("").to_string()
    }
}

pub fn parent_suspicious() -> bool {
    let p = parent_name().to_lowercase();
    if p.is_empty() { return false; }
    let bad: [String; 5] = [
        obfstr!("lldb").to_string(),
        obfstr!("gdb").to_string(),
        obfstr!("dtrace").to_string(),
        obfstr!("frida").to_string(),
        obfstr!("debugserver").to_string(),
    ];
    bad.iter().any(|b| p.contains(b.as_str()))
}

// ─── sysctl P_TRACED — secondary check behind PT_DENY_ATTACH ────────

pub fn is_being_traced() -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
        const CTL_KERN: libc::c_int = 1;
        const KERN_PROC: libc::c_int = 14;
        const KERN_PROC_PID: libc::c_int = 1;
        const P_TRACED: u32 = 0x0000_0800;
        // p_flag offset inside `struct kinfo_proc` on macOS = 32.
        const KINFO_PROC_SIZE: usize = 648;
        const P_FLAG_OFF: usize = 32;

        let mut mib: [libc::c_int; 4] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, libc::getpid()];
        let mut buf = vec![0u8; KINFO_PROC_SIZE];
        let mut size: libc::size_t = buf.len();
        let rc = libc::sysctl(
            mib.as_mut_ptr(), 4,
            buf.as_mut_ptr() as *mut libc::c_void, &mut size,
            std::ptr::null_mut(), 0,
        );
        if rc != 0 || size < P_FLAG_OFF + 4 { return false; }
        let p_flag = u32::from_le_bytes([
            buf[P_FLAG_OFF], buf[P_FLAG_OFF + 1],
            buf[P_FLAG_OFF + 2], buf[P_FLAG_OFF + 3],
        ]);
        (p_flag & P_TRACED) != 0
    }
    #[cfg(not(target_os = "macos"))]
    { false }
}

// ─── Silent exit with random delay ──────────────────────────────────

fn silent_exit() -> ! {
    // Stall briefly so the trigger isn't trivially correlated to the
    // exact check that fired (helps mask which path tripped us).
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    let ms = (seed % 800) + 200;
    thread::sleep(Duration::from_millis(ms));
    std::process::exit(0);
}

// ─── Background watcher ─────────────────────────────────────────────

pub fn start_security_thread() {
    thread::spawn(|| {
        // First check ~1 s after start — give Tauri a chance to spin up
        // its own helper processes (their parent != us, harmless).
        thread::sleep(Duration::from_secs(1));
        loop {
            if frida_check() || parent_suspicious() || is_being_traced() {
                silent_exit();
            }
            thread::sleep(Duration::from_secs(3));
        }
    });
}

// ─── Opaque predicates (always const, look variable to static analysis) ─
//
// IDA Pro / Ghidra try to reason about control flow by computing the
// possible values at each branch. These predicates depend on atomic
// loads of constants the analyser can't prove constant without simulating
// the runtime — most decompilers just give up and mark the branch as
// dependent on external state, which forks the disassembly view.

static OPAQUE_A: AtomicU64 = AtomicU64::new(0xC0DE_F00D_DEAD_BEEF);
static OPAQUE_B: AtomicU64 = AtomicU64::new(0xCAFE_BABE_5A11_5A11);

#[inline(never)]
pub fn always_true() -> bool {
    let a = OPAQUE_A.load(Ordering::Relaxed);
    let b = OPAQUE_B.load(Ordering::Relaxed);
    // (a XOR a) | (b ^ b) is always 0 — always true: `0 == 0`.
    (a ^ a) | (b ^ b) == 0
}

#[inline(never)]
pub fn always_false() -> bool {
    let a = OPAQUE_A.load(Ordering::Relaxed);
    // a multiplied by itself is never -1 (always positive overflow).
    a.wrapping_mul(a).wrapping_add(1) == 0
}

// ─── Offset obfuscation ─────────────────────────────────────────────
//
// The 4-step resolve chain (FakeDataModel → DataModel → Workspace →
// Camera) is the easiest grep target for a reverser. We hide the literal
// values by XOR'ing them at compile time with a runtime-loaded key. The
// key itself is computed via a small loop the optimiser can fold but the
// raw constant never appears as a 64-bit immediate.

static RT_KEY: Lazy<u64> = Lazy::new(|| {
    let mut k: u64 = 0x6c1b_3b81_1234_5678;
    for i in 0..7 {
        k = k.wrapping_mul(0x100000001b3);
        k = k ^ (k >> 17);
        k = k.wrapping_add((i as u64) * 0xA5A5_A5A5);
    }
    k
});

/// The actual key the const XOR encodings below were baked against.
const KEY_USED_AT_COMPILE: u64 = compute_key();

const fn compute_key() -> u64 {
    let mut k: u64 = 0x6c1b_3b81_1234_5678;
    let mut i: u64 = 0;
    while i < 7 {
        k = k.wrapping_mul(0x100000001b3);
        k = k ^ (k >> 17);
        k = k.wrapping_add(i * 0xA5A5_A5A5);
        i += 1;
    }
    k
}

// ARM and Intel Roblox builds expose two different `base + entry` offsets,
// AND two different chains to reach the DataModel:
//   • ARM  : `read(base + 0x61E8760)` → FakeDataModel*, then `read(+0x18)` → DataModel.
//   • Intel: `read(base + 0x61E8768)` is already the DataModel — no second deref.
// All offsets are XOR-obfuscated; the active pair is selected at compile time
// via `#[cfg(target_arch = …)]` below.
#[allow(dead_code)]
const ENC_ENTRY_FAKE_DMARM:   u64 = 0x6024748u64 ^ KEY_USED_AT_COMPILE;
#[allow(dead_code)]
const ENC_ENTRY_FAKE_DMINTEL: u64 = 0x61E8768u64 ^ KEY_USED_AT_COMPILE; // Fakedatamodel change at update
#[allow(dead_code)]
const ENC_FDM_DATAMODELARM:   u64 = 0x18u64      ^ KEY_USED_AT_COMPILE;
const ENC_DM_WORKSPACE:       u64 = 0x168u64     ^ KEY_USED_AT_COMPILE;
const ENC_WS_CAMERA:          u64 = 0x460u64     ^ KEY_USED_AT_COMPILE;

#[cfg(target_arch = "aarch64")]
#[inline(never)]
fn off_entry_fake_dm_arm()   -> u64 { ENC_ENTRY_FAKE_DMARM   ^ *RT_KEY }
#[cfg(target_arch = "x86_64")]
#[inline(never)]
fn off_entry_fake_dm_intel() -> u64 { ENC_ENTRY_FAKE_DMINTEL ^ *RT_KEY }
#[cfg(target_arch = "aarch64")]
#[inline(never)]
fn off_fdm_datamodel_arm()   -> u64 { ENC_FDM_DATAMODELARM   ^ *RT_KEY }
#[inline(never)]
pub fn off_dm_workspace()    -> u64 { ENC_DM_WORKSPACE       ^ *RT_KEY }
#[inline(never)]
pub fn off_ws_camera()       -> u64 { ENC_WS_CAMERA          ^ *RT_KEY }

/// Display string for the active build target (consumed by the menu).
pub const fn arch_name() -> &'static str {
    if cfg!(target_arch = "aarch64") { "ARM" }
    else if cfg!(target_arch = "x86_64") { "Intel" }
    else { "Unknown" }
}

// ─── Public chain resolvers using obfuscated offsets ────────────────
//
// Use these everywhere instead of duplicating the chain with raw enum
// constants — keeps the literal `0x5F498F0` / `0x18` / `0x168` / `0x460`
// out of any function body in the binary.

use crate::sdk::SdkI;

/// ARM build: `base + 0x61E8760` points at a FakeDataModel struct whose
/// `DataModel` field is at +0x18 — two reads to reach the DataModel.
#[cfg(target_arch = "aarch64")]
#[inline(never)]
pub fn resolve_data_model(sdk: &SdkI) -> Option<u64> {
    let fdm = sdk.read::<u64>(sdk.base + off_entry_fake_dm_arm());
    if !sdk.is_valid_ptr(fdm) { return None; }
    let dm = sdk.read::<u64>(fdm + off_fdm_datamodel_arm());
    if sdk.is_valid_ptr(dm) { Some(dm) } else { None }
}

/// Intel build: `base + 0x61E8768` is *already* the DataModel pointer — no
/// FakeDataModel wrapper, single read.
#[cfg(target_arch = "x86_64")]
#[inline(never)]
pub fn resolve_data_model(sdk: &SdkI) -> Option<u64> {
    let dm = sdk.read::<u64>(sdk.base + off_entry_fake_dm_intel());
    if sdk.is_valid_ptr(dm) { Some(dm) } else { None }
}

#[inline(never)]
pub fn resolve_workspace(sdk: &SdkI) -> Option<u64> {
    let dm = resolve_data_model(sdk)?;
    let ws = sdk.read::<u64>(dm + off_dm_workspace());
    if sdk.is_valid_ptr(ws) { Some(ws) } else { None }
}

#[inline(never)]
pub fn resolve_camera(sdk: &SdkI) -> Option<u64> {
    let ws = resolve_workspace(sdk)?;
    let cam = sdk.read::<u64>(ws + off_ws_camera());
    if sdk.is_valid_ptr(cam) { Some(cam) } else { None }
}
