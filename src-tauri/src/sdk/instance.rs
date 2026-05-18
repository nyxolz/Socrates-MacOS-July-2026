//! Roblox `Instance` walk: read names, list children, find a child by name.
//!
//! These functions are the building blocks the ESP loop composes — they all
//! go through the shared `SdkI` so they share the read counters.

use super::process::SdkI;

#[allow(non_camel_case_types)]
pub enum Instance {
    Parent = 0x070,
    ChildrenList = 0x078,
    Name = 0x0B0,
}

/// Read a printable ASCII string at `instance + offset` (the offset points
/// to a `char*`, not the inline bytes).
pub fn read_string(sdk: &SdkI, instance: u64, offset: u64) -> String {
    let data_ptr = sdk.read::<u64>(instance + offset);
    if !sdk.is_valid_ptr(data_ptr) { return String::new(); }
    let mut bytes = [0u8; 128];
    for i in 0..128u64 {
        bytes[i as usize] = sdk.read::<u8>(data_ptr + i);
        if bytes[i as usize] == 0 { break; }
    }
    bytes.iter().take_while(|&&b| b != 0 && b >= 32 && b <= 126)
        .map(|&b| b as char).collect()
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
