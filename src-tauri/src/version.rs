//! Compile-time version stamp. `builder_sender.sh` rewrites the literal on
//! the `VERSION` line just before `cargo build`, so each released binary
//! carries a unique 10-char hash that the Settings tab compares against
//! the value published at `STATUS_URL`.
//!
//! Format of both lines is load-bearing — the sed in builder_sender.sh
//! matches `pub const VERSION: &str = "..."` and
//! `pub const STATUS_URL: &str = "..."` exactly. Don't change the shape
//! without updating the script.
//!
//! `STATUS_URL` is swapped between the production endpoint and the prov
//! GitHub raw URL depending on the deploy target chosen at build time.

pub const VERSION: &str = "NULkTX3wEX";
pub const STATUS_URL: &str = "https://raw.githubusercontent.com/pleglou26-oss/not_default_socrate_repo/refs/heads/main/status.json";
