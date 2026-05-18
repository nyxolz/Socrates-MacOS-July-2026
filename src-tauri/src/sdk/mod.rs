//! External SDK — attach to the Roblox process, read/write its virtual memory,
//! walk its instance tree, and match byte signatures.
//!
//! Split into three submodules:
//!   - `process`  : attach + raw vm_read / vm_write / pointer chain.
//!   - `instance` : Roblox-specific helpers (Name, GetChildren, FindFirstChild).
//!   - `signature`: pattern scan against the base image (with `??` wildcards).
//!
//! Everything is re-exported here so callers can `use crate::sdk::*` and not
//! care about the file split.

pub mod process;
pub mod instance;
pub mod signature;

pub use process::SdkI;
pub use instance::{
    Instance,
    find_first_child, get_children, name_of, read_string,
};
pub use signature::find_offset_from_token;
