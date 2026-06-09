//! Usage telemetry + access control via a Supabase **Edge Function**.
//!
//! The app never touches the database directly and never holds a powerful key.
//! It only POSTs a machine id to the `access` function, which (server-side, with
//! the service_role key) decides whether the machine is allowed, registers new
//! machines, and accumulates usage time. Because the decision is server-side, a
//! blacklist cannot be bypassed by reversing the app — the only embedded secret
//! is the publishable key, which is public by design.
//!
//! Identity is the macOS hardware UUID (`pc_id`). Flow:
//!   * launch   → POST gate      → blocked users get a native dialog + ticket
//!   * every 60s → POST heartbeat → accumulates time, enforces live blacklist
//!   * tamper   → POST tamper    → server flips is_allowed=false permanently

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::secrets;

type HmacSha256 = Hmac<Sha256>;

/// Seconds since the Unix epoch (used for replay-resistant request signing).
fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// `hex(HMAC-SHA256(secret, "{ts}.{body}"))`. Binding the timestamp into the
/// signature lets the server reject stale/replayed requests.
fn sign(ts: &str, body: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secrets::supabase_hmac_secret().as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(ts.as_bytes());
    mac.update(b".");
    mac.update(body.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Where a blacklisted user is sent to request access back. Change to your
/// real support / Discord / ticket URL.
const TICKET_URL: &str = "https://usesocrate.xyz";

// ─── Machine identity ───────────────────────────────────────────────

/// macOS hardware UUID — stable per machine, our `pc_id`.
fn pc_uuid() -> String {
    let out = std::process::Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output();
    if let Ok(o) = out {
        let s = String::from_utf8_lossy(&o.stdout);
        for line in s.lines() {
            if line.contains("IOPlatformUUID") {
                if let Some(eq) = line.find('=') {
                    return line[eq + 1..].trim().trim_matches('"').to_string();
                }
            }
        }
    }
    "unknown".to_string()
}

/// Placeholder name sent for every machine — the real login name is no longer
/// collected.
fn pc_name() -> String {
    "XXX".to_string()
}

/// ISO country code from the macOS locale (e.g. "FR"). No network call.
#[cfg(target_os = "macos")]
fn country_code() -> String {
    use cocoa::base::{id, nil};
    use objc::{class, msg_send, sel, sel_impl};
    unsafe {
        let locale: id = msg_send![class!(NSLocale), currentLocale];
        if locale == nil {
            return "XX".to_string();
        }
        let cc: id = msg_send![locale, countryCode];
        if cc == nil {
            return "XX".to_string();
        }
        let bytes: *const std::os::raw::c_char = msg_send![cc, UTF8String];
        if bytes.is_null() {
            return "XX".to_string();
        }
        std::ffi::CStr::from_ptr(bytes)
            .to_string_lossy()
            .into_owned()
    }
}

#[cfg(not(target_os = "macos"))]
fn country_code() -> String {
    "XX".to_string()
}

// ─── Edge Function transport ────────────────────────────────────────

fn http() -> Option<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(6))
        .build()
        .ok()
}

/// `{url}/functions/v1/quick-function`
fn fn_url() -> String {
    format!("{}/functions/v1/quick-function", secrets::supabase_url())
}

/// POST a JSON body to the access function, return the parsed JSON response.
///
/// The body is serialized once and sent verbatim so the bytes the server hashes
/// match ours exactly. We add `x-socrate-ts` (Unix seconds) and `x-socrate-sig`
/// (HMAC over `ts.body`); the Edge Function rejects anything it can't verify.
fn call(client: &reqwest::blocking::Client, body: serde_json::Value) -> Option<serde_json::Value> {
    let key = secrets::supabase_anon_key();
    let raw = serde_json::to_string(&body).ok()?;
    let ts = now_unix().to_string();
    let sig = sign(&ts, &raw);
    client
        .post(fn_url())
        .header("apikey", &key)
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .header("x-socrate-ts", &ts)
        .header("x-socrate-sig", &sig)
        .body(raw)
        .send()
        .ok()?
        .json()
        .ok()
}

// ─── Access control ─────────────────────────────────────────────────

/// Native "you are blacklisted" dialog. Returns true if the user picked
/// "Open a ticket" (so we re-check), false for "Quit".
fn show_blacklist_dialog() -> bool {
    use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
    let res = MessageDialog::new()
        .set_title("Access denied")
        .set_description("! You have been blacklisted.\n\nOpen a ticket if you want your access back.")
        .set_level(MessageLevel::Error)
        .set_buttons(MessageButtons::OkCancelCustom(
            "Open a ticket".into(),
            "Quit".into(),
        ))
        .show();
    matches!(&res, MessageDialogResult::Custom(s) if s == "Open a ticket")
        || matches!(&res, MessageDialogResult::Ok)
}

fn open_ticket() {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(TICKET_URL).spawn();
}

/// Permanently blacklist this machine (server sets is_allowed = false). Called
/// by the security watcher when tampering is detected. Best-effort.
pub fn blacklist_self() {
    let pc = pc_uuid();
    if let Some(client) = http() {
        let _ = call(
            &client,
            serde_json::json!({ "action": "tamper", "pc_id": pc }),
        );
    }
}

/// Mark this machine as no longer using Socrate (server sets is_playing=false).
/// Best-effort beacon sent when the app is closing. Blocking + short, so it can
/// run on an exit path before the process terminates.
pub fn mark_stopped() {
    let pc = pc_uuid();
    if let Some(client) = http() {
        let _ = call(
            &client,
            serde_json::json!({ "action": "stop", "pc_id": pc }),
        );
    }
}

// ─── Entry point ────────────────────────────────────────────────────

/// Synchronous access gate, run during setup BEFORE any overlay/thread.
/// On success spawns the background usage heartbeat and returns true.
/// Returns false only when a blacklisted user chooses to quit — the caller
/// must then exit so nothing of Socrate ever appears.
///
/// Network failure fails OPEN (offline users aren't locked out); the heartbeat
/// keeps re-checking, so a blacklist still lands once connectivity returns.
pub fn gate_and_start() -> bool {
    let client = match http() {
        Some(c) => c,
        None => return true,
    };
    let pc = pc_uuid();

    loop {
        let resp = call(
            &client,
            serde_json::json!({
                "action": "gate",
                "pc_id": pc,
                "pc_name": pc_name(),
                "country": country_code(),
            }),
        );

        match resp {
            // Reachable + explicitly blocked → blacklist dialog.
            Some(ref v) if v["allowed"].as_bool() == Some(false) => {
                if show_blacklist_dialog() {
                    open_ticket();
                    continue; // re-check (lets an un-blacklist take effect live)
                }
                return false;
            }
            // Allowed → resume from stored total and start the heartbeat.
            Some(v) => {
                let total = v["total"].as_u64().unwrap_or(0);
                start_heartbeat(pc, total);
                return true;
            }
            // Unreachable → fail open.
            None => {
                start_heartbeat(pc, 0);
                return true;
            }
        }
    }
}

/// Background worker: every 60 s push usage time and re-check the verdict.
/// A remote blacklist (allowed → false) closes the app immediately.
fn start_heartbeat(pc: String, start_total: u64) {
    std::thread::spawn(move || {
        let client = match http() {
            Some(c) => c,
            None => return,
        };
        let mut total = start_total;
        loop {
            std::thread::sleep(Duration::from_secs(60));
            total = total.saturating_add(60);

            let resp = call(
                &client,
                serde_json::json!({
                    "action": "heartbeat",
                    "pc_id": pc,
                    "seconds": total,
                }),
            );
            if let Some(v) = resp {
                if v["allowed"].as_bool() == Some(false) {
                    mark_stopped(); // best-effort: clear is_playing before quitting
                    std::process::exit(0); // remotely blacklisted mid-session
                }
            }
        }
    });
}
