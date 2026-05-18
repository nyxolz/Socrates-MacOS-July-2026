//! Find the live Roblox window rect via CoreGraphics window listing.
//!
//! Filters out fake "minimized" windows (alpha == 0, off-screen parked
//! coords) so the overlay doesn't pop back the moment Roblox is hidden.

use core_foundation::{
    array::{CFArrayGetCount, CFArrayGetValueAtIndex},
    base::{CFTypeRef, TCFType},
    dictionary::{CFDictionaryGetValueIfPresent, CFDictionaryRef},
    number::{kCFNumberFloat64Type, CFNumberGetValue, CFNumberRef},
    string::{CFString, CFStringRef},
};

pub fn find_window_bounds(target_owner: &str) -> Option<(f64, f64, f64, f64)> {
    use core_graphics::display::*;
    unsafe {
        let opts: u32 = (1 << 0) | (1 << 4);
        let info_array = CGWindowListCopyWindowInfo(opts, kCGNullWindowID);
        if info_array.is_null() { return None; }
        let count = CFArrayGetCount(info_array);
        for i in 0..count {
            let dict: CFDictionaryRef = CFArrayGetValueAtIndex(info_array, i) as _;
            if dict.is_null() { continue; }
            let owner = read_dict_string(dict, "kCGWindowOwnerName");
            if owner.as_deref() != Some(target_owner) { continue; }

            // Alpha == 0 → window is minimized/hidden, not just off-screen.
            let alpha = read_dict_number(dict, "kCGWindowAlpha").unwrap_or(1.0);
            if alpha < 0.05 { continue; }
            let bounds = read_dict_bounds(dict);
            if let Some((x, y, w, h)) = bounds {
                // Some apps "fake minimize" by parking at far-negative coords.
                if x < -10000.0 || y < -10000.0 { continue; }
                if w > 200.0 && h > 200.0 { return Some((x, y, w, h)); }
            }
        }
        None
    }
}

unsafe fn read_dict_string(dict: CFDictionaryRef, key: &str) -> Option<String> {
    let cf_key = CFString::new(key);
    let mut value: CFTypeRef = std::ptr::null();
    if CFDictionaryGetValueIfPresent(dict, cf_key.as_concrete_TypeRef() as _, &mut value) == 0 { return None; }
    if value.is_null() { return None; }
    Some(CFString::wrap_under_get_rule(value as CFStringRef).to_string())
}

unsafe fn read_dict_number(dict: CFDictionaryRef, key: &str) -> Option<f64> {
    let cf_key = CFString::new(key);
    let mut value: CFTypeRef = std::ptr::null();
    if CFDictionaryGetValueIfPresent(dict, cf_key.as_concrete_TypeRef() as _, &mut value) == 0 { return None; }
    if value.is_null() { return None; }
    let mut out: f64 = 0.0;
    CFNumberGetValue(value as CFNumberRef, kCFNumberFloat64Type, &mut out as *mut f64 as _);
    Some(out)
}

unsafe fn read_dict_bounds(dict: CFDictionaryRef) -> Option<(f64, f64, f64, f64)> {
    let cf_key = CFString::new("kCGWindowBounds");
    let mut value: CFTypeRef = std::ptr::null();
    if CFDictionaryGetValueIfPresent(dict, cf_key.as_concrete_TypeRef() as _, &mut value) == 0 { return None; }
    if value.is_null() { return None; }
    let bounds_dict = value as CFDictionaryRef;
    Some((
        read_dict_number(bounds_dict, "X")?,
        read_dict_number(bounds_dict, "Y")?,
        read_dict_number(bounds_dict, "Width")?,
        read_dict_number(bounds_dict, "Height")?,
    ))
}
