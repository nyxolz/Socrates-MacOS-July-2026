//! Roblox `Instance` walk: read names, list children, find a child by name.
//!
//! These functions are the building blocks the ESP loop composes — they all
//! go through the shared `SdkI` so they share the read counters.

use super::process::SdkI;

#[allow(non_camel_case_types)]
pub enum Instance {
    Parent = 0x068,
    ChildrenList = 0x070,
    Name = 0x098,
}

/// Read up to `max` printable ASCII bytes from `addr`, stopping at NUL or
/// the first non-printable byte. Shared by both arch implementations of
/// `read_string` below.
fn read_cstr(sdk: &SdkI, addr: u64, max: u64) -> String {
    // One bulk read instead of up to `max` individual 1-byte syscalls — this
    // runs per-child in every by-name tree walk (Explorer, resolve_camera's
    // fallback path), so on a large Workspace it used to be the dominant cost.
    let bytes = sdk.vm_read_bulk(addr, max as usize);
    bytes.iter().take_while(|&&b| b != 0 && (32..=126).contains(&b))
        .map(|&b| b as char).collect()
}

/// ARM Roblox: the string field stores a raw `char*` at offset 0. One read
/// gives the heap pointer, then we walk it as a C-string.
#[cfg(target_arch = "aarch64")]
pub fn read_string(sdk: &SdkI, instance: u64, offset: u64) -> String {
    let data_ptr = sdk.read::<u64>(instance + offset);
    if !sdk.is_valid_ptr(data_ptr) { return String::new(); }
    read_cstr(sdk, data_ptr, 128)
}

/// Intel Roblox: the field holds a **pointer** to a libc++ `std::string`
/// object (NOT the string inline). Two reads are needed:
///   1. `strobj = read u64 at instance+offset` — the std::string's address.
///   2. Decode the 24-byte std::string at `strobj`. The low bit of byte 0
///      picks the layout:
///        • `byte0 & 1 == 0` → **SSO**: `size = byte0 >> 1`, characters
///          live inline starting at `strobj + 1` (up to 22 bytes).
///        • `byte0 & 1 == 1` → **long form**: `size = u64 at strobj+8`,
///          heap `char*` = `u64 at strobj+16`.
///
/// Decoding the std::string at `instance+offset` directly (skipping step 1)
/// is the trap that produced empty names — that address holds the *pointer*
/// to the string struct, not the struct itself.
#[cfg(target_arch = "x86_64")]
pub fn read_string(sdk: &SdkI, instance: u64, offset: u64) -> String {
    let strobj = sdk.read::<u64>(instance + offset);
    if !sdk.is_valid_ptr(strobj) { return String::new(); }

    let byte0 = sdk.read::<u8>(strobj);
    if byte0 & 1 == 0 {
        let size = (byte0 >> 1) as u64;
        if size == 0 || size > 22 { return String::new(); }
        read_cstr(sdk, strobj + 1, size)
    } else {
        let size = sdk.read::<u64>(strobj + 8);
        if size == 0 || size > 4096 { return String::new(); }
        let data_ptr = sdk.read::<u64>(strobj + 16);
        if !sdk.is_valid_ptr(data_ptr) { return String::new(); }
        read_cstr(sdk, data_ptr, size.min(256))
    }
}

pub fn name_of(sdk: &SdkI, instance: u64) -> String {
    read_string(sdk, instance, Instance::Name as u64)
}

/// All direct children of `instance`. Stops at 2000 to keep DataModel sane.
pub fn get_children(sdk: &SdkI, instance: u64) -> Vec<u64> {
    let container = sdk.read::<u64>(instance + Instance::ChildrenList as u64);
    if !sdk.is_valid_ptr(container) { return Vec::new(); }
    let array_begin = sdk.read::<u64>(container);
    let array_end   = sdk.read::<u64>(container + 8);
    if !sdk.is_valid_ptr(array_begin) || array_end <= array_begin { return Vec::new(); }
    let count = ((array_end - array_begin) / 8).min(2000);

    let mut children = Vec::new();
    for i in 0..count {
        let child = sdk.read::<u64>(array_begin + i * 8);
        if !sdk.is_valid_ptr(child) { continue; }
        if sdk.read::<u64>(child + Instance::Parent as u64) != instance { continue; }
        children.push(child);
    }
    children
}

/// First direct child whose `Name` equals `target_name`. Returns 0 if not found.
pub fn find_first_child(sdk: &SdkI, instance: u64, target_name: &str) -> u64 {
    for child in get_children(sdk, instance) {
        if name_of(sdk, child) == target_name {
            return child;
        }
    }
    0
}
