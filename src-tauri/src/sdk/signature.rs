//! Byte-pattern signature scanner.
//!
//! Two token formats are supported:
//!   - simple : `"48 8B 05 ?? ?? ?? ?? 48 89 1D"`
//!   - split  : `"48 8B 05 | ?? ?? ?? ?? | 48 89 1D"`
//!
//! In the split form the **middle** part marks the bytes we actually want
//! the offset of (so the returned address points at the value, not the
//! start of the pattern). `??` is a wildcard byte.

use super::process::SdkI;

/// Scan `[base, base + max_offset)` for the given byte pattern and return
/// the offset (relative to `base`) of the matched **value** bytes.
pub fn find_offset_from_token(
    sdk: &SdkI,
    base: u64,
    token: &str,
    max_offset: u64,
) -> Option<u64> {
    let parts: Vec<&str> = token.split('|').collect();

    let mut pattern: Vec<Option<u8>> = Vec::new();
    let mut value_start: usize = 0;

    if parts.len() == 3 {
        let before: Vec<Option<u8>> = parts[0].split_whitespace()
            .map(|s| if s == "??" { None } else { u8::from_str_radix(s, 16).ok() })
            .collect();
        let value: Vec<Option<u8>> = parts[1].split_whitespace()
            .map(|s| if s == "??" { None } else { u8::from_str_radix(s, 16).ok() })
            .collect();
        let after: Vec<Option<u8>> = parts[2].split_whitespace()
            .map(|s| if s == "??" { None } else { u8::from_str_radix(s, 16).ok() })
            .collect();
        value_start = before.len();
        pattern.extend(before);
        pattern.extend(value);
        pattern.extend(after);
    } else {
        pattern = token.split_whitespace()
            .map(|s| if s == "??" { None } else { u8::from_str_radix(s, 16).ok() })
            .collect();
    }

    if pattern.is_empty() { return None; }
    let pattern_len = pattern.len() as u64;
    if max_offset < pattern_len { return None; }

    let buf = sdk.vm_read_bulk(base, max_offset as usize);

    for candidate in 0..buf.len().saturating_sub(pattern.len()) {
        let mut all_match = true;
        for (i, want) in pattern.iter().enumerate() {
            if let Some(b) = want {
                if buf[candidate + i] != *b { all_match = false; break; }
            }
        }
        if all_match {
            return Some((candidate + value_start) as u64);
        }
    }
    None
}
