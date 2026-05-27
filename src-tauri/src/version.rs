//! Compile-time version stamp. `builder_sender.sh` rewrites the literal on
//! the `VERSION` line just before `cargo build`, so each released binary
//! carries a unique 10-char hash that the Settings tab compares against
//! the value published at `https://usesocrate.xyz/status.json`.
//!
//! Format of the line is load-bearing — the sed in builder_sender.sh
//! matches `pub const VERSION: &str = "..."` exactly. Don't change the
//! shape without updating the script.

pub const VERSION: &str = "Cdrxq3W2sZ";
