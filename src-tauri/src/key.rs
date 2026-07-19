//! License gate — now brokered through our own Supabase Edge Function.
//!
//! The app NO LONGER talks to keyauth.win directly and no longer embeds the
//! KeyAuth secret. Instead it POSTs `{key, hwid, nonce}` to the `key-validate`
//! Edge Function (HMAC-signed like every other Socrate function call). That
//! function runs the whole KeyAuth exchange server-side and returns a verdict
//! **signed with an Ed25519 private key that never leaves the server**.
//!
//! We verify that signature with an embedded PUBLIC key (harmless to ship — you
//! can't forge with it). This kills the old bypass: redirecting the URL to a
//! fake server no longer works, because a fake server can't produce a valid
//! signature. See `supabase/functions/key-validate/index.ts` for the rationale.

use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, Nonce};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey as XPublicKey, StaticSecret};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager};

use crate::secrets;
use crate::state::AppState;

type HmacSha256 = Hmac<Sha256>;

const APP_VERSION: &str = "1.0";
/// Max age (seconds) we accept for a signed verdict — bounds replay of an old
/// captured "valid" response.
const MAX_VERDICT_AGE: i64 = 120;

#[derive(Serialize, Clone, Default)]
pub struct KeyResult {
    /// Key was accepted by KeyAuth for this machine.
    pub valid: bool,
    /// Unix-millis timestamp the access expires at (0 = no expiry / lifetime).
    pub expires_after: i64,
    /// Set when the network call failed, or a verdict's signature didn't check
    /// out (lets the UI say "try again" rather than "wrong key").
    pub network_error: bool,
    /// This build's version has been disabled server-side — the app must quit
    /// and the user must update.
    pub version_blocked: bool,
    /// The latest version hash (shown in the "new version" message).
    pub latest_version: String,
}

fn network_error() -> KeyResult {
    KeyResult { network_error: true, ..Default::default() }
}

fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Decrypt the offsets blob the server sealed to our ephemeral X25519 key.
/// shared = X25519(client_priv, server_pub); key = SHA256(shared); then
/// AES-256-GCM (ct carries the 16-byte tag). Returns the UTF-8 plaintext
/// (`"<fdmEntry>|<fdmDataModel>"`) or None on any failure.
fn decrypt_offsets(client_priv: [u8; 32], server_pub: [u8; 32], iv: &[u8], ct: &[u8]) -> Option<String> {
    if iv.len() != 12 {
        return None;
    }
    let shared = StaticSecret::from(client_priv).diffie_hellman(&XPublicKey::from(server_pub));
    let key = Sha256::digest(shared.as_bytes());
    let cipher = <Aes256Gcm as aes_gcm::KeyInit>::new_from_slice(&key).ok()?;
    let pt = cipher.decrypt(Nonce::from_slice(iv), ct).ok()?;
    String::from_utf8(pt).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Interop vector produced by the Deno key-validate crypto (@noble x25519 +
    // WebCrypto AES-256-GCM). If this passes, server encrypt ↔ client decrypt
    // agree byte-for-byte.
    #[test]
    fn decrypts_deno_vector() {
        let client_priv: [u8; 32] = [0x01; 32];
        let server_pub: [u8; 32] =
            hex::decode("ce8d3ad1ccb633ec7b70c17814a5c76ecd029685050d344745ba05870e587d59")
                .unwrap()
                .try_into()
                .unwrap();
        let iv = [0x03u8; 12];
        let ct = hex::decode("37ae3c00d2c2242e11d30cd7b1eaa5713a1b932a864eaadae5ac8615").unwrap();
        let pt = decrypt_offsets(client_priv, server_pub, &iv, &ct);
        assert_eq!(pt.as_deref(), Some("125721960|24"));
    }
}

/// `hex(HMAC-SHA256(secret, "{ts}.{body}"))` — same request-signing scheme as
/// `telemetry::sign`, so the Edge Function verifies us identically.
fn sign(ts: &str, body: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secrets::supabase_hmac_secret().as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(ts.as_bytes());
    mac.update(b".");
    mac.update(body.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// `{supabase_url}/functions/v1/key-validate`
fn fn_url() -> String {
    format!("{}/functions/v1/key-validate", secrets::supabase_url())
}

/// Load the embedded Ed25519 public key (64 hex chars → 32 bytes).
fn verifying_key() -> Option<VerifyingKey> {
    let bytes = hex::decode(secrets::keyauth_ed25519_pub()).ok()?;
    let arr: [u8; 32] = bytes.try_into().ok()?;
    VerifyingKey::from_bytes(&arr).ok()
}

/// Validate a KeyAuth license key against this machine's hardware id.
#[tauri::command]
pub fn validate_keyauth_key(token: String) -> KeyResult {
    check_license(token.trim())
}

/// Core validation, shared by the `validate_keyauth_key` command and the
/// background license-watch thread. Talks to the `key-validate` Edge Function
/// and verifies the Ed25519-signed verdict.
pub fn check_license(token: &str) -> KeyResult {
    let token = token.trim();
    if token.is_empty() {
        return KeyResult::default();
    }

    let vk = match verifying_key() {
        Some(v) => v,
        None => return network_error(), // build misconfigured — fail closed
    };

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return network_error(),
    };

    let hwid = crate::telemetry::pc_uuid();
    let nonce = uuid::Uuid::new_v4().simple().to_string();

    // Ephemeral X25519 keypair — the server seals the offsets to `cpub` so they
    // stay unreadable on the wire even if TLS is intercepted. `cpriv` never
    // leaves the process and is thrown away after this call.
    let mut cpriv = [0u8; 32];
    if getrandom::getrandom(&mut cpriv).is_err() {
        return network_error();
    }
    let cpub = XPublicKey::from(&StaticSecret::from(cpriv));
    let cpub_hex = hex::encode(cpub.as_bytes());

    let body = serde_json::json!({
        "key": token,
        "hwid": hwid,
        "nonce": nonce,
        "ver": APP_VERSION,
        // The server returns the arch-specific FakeDataModel offset.
        "arch": crate::security::arch_name(),
        // This binary's version hash — the server flags it if disabled.
        "build": crate::version::VERSION,
        // Ephemeral public key the server encrypts the offsets to.
        "cpub": cpub_hex,
    });
    let raw = match serde_json::to_string(&body) {
        Ok(s) => s,
        Err(_) => return network_error(),
    };
    let ts = now_unix().to_string();
    let sig = sign(&ts, &raw);
    let anon = secrets::supabase_anon_key();

    let resp = match client
        .post(fn_url())
        .header("apikey", &anon)
        .header("Authorization", format!("Bearer {}", anon))
        .header("Content-Type", "application/json")
        .header("x-socrate-ts", &ts)
        .header("x-socrate-sig", &sig)
        .body(raw)
        .send()
    {
        Ok(r) => r,
        Err(_) => return network_error(),
    };
    if !resp.status().is_success() {
        // 4xx/5xx from the function (KeyAuth down, bad request…) → try again,
        // never silently treat as a valid or invalid key.
        return network_error();
    }
    let v: serde_json::Value = match resp.json() {
        Ok(j) => j,
        Err(_) => return network_error(),
    };

    // ─── Verify the signed verdict ───
    let r_valid = v["valid"].as_bool().unwrap_or(false);
    let r_expiry = v["expiry"].as_i64().unwrap_or(0);
    let r_hwid = v["hwid"].as_str().unwrap_or("");
    let r_nonce = v["nonce"].as_str().unwrap_or("");
    let r_iat = v["iat"].as_i64().unwrap_or(0);
    let r_sig = v["sig"].as_str().unwrap_or("");

    // Sealed offsets blob (encrypted to our ephemeral key; empty when invalid).
    let o = &v["offsets"];
    let enc_pub = o["encPub"].as_str().unwrap_or("");
    let iv_hex = o["iv"].as_str().unwrap_or("");
    let ct_hex = o["ct"].as_str().unwrap_or("");

    // Version gate — is this build disabled server-side?
    let r_blocked = v["blocked"].as_bool().unwrap_or(false);
    let r_latest = v["latest"].as_str().unwrap_or("");

    // Rebuild exactly what the server signed and check the Ed25519 signature.
    // The signature covers the sealed blob AND the version gate, so a fake or
    // tampered server can neither forge a "valid" verdict, swap the ciphertext,
    // nor spoof / suppress a version block.
    let msg = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        r_nonce, if r_valid { 1 } else { 0 }, r_expiry, r_hwid, r_iat,
        enc_pub, iv_hex, ct_hex,
        if r_blocked { 1 } else { 0 }, r_latest,
    );
    let sig_bytes = match hex::decode(r_sig).ok().and_then(|b| <[u8; 64]>::try_from(b).ok()) {
        Some(b) => b,
        None => return network_error(),
    };
    if vk.verify(msg.as_bytes(), &Signature::from_bytes(&sig_bytes)).is_err() {
        return network_error(); // signature invalid → fake/tampered server
    }

    // Bind the verdict to THIS request/machine and reject stale replays.
    if r_nonce != nonce || r_hwid != hwid {
        return network_error();
    }
    if (now_unix() - r_iat).abs() > MAX_VERDICT_AGE {
        return network_error();
    }

    // Version gate — this build is disabled server-side. Freeze features and let
    // the watcher show the "new version" message and quit. Takes priority over
    // key validity (a valid key on a dead build must still update).
    if r_blocked {
        crate::security::set_chain_offsets(crate::security::ChainOffsets::default());
        return KeyResult {
            version_blocked: true,
            latest_version: r_latest.to_string(),
            ..Default::default()
        };
    }

    if !r_valid {
        crate::security::set_chain_offsets(crate::security::ChainOffsets::default());
        return KeyResult::default(); // invalid/expired/wrong-hwid key
    }

    // Valid + signature verified → decrypt the offsets sealed to our ephemeral
    // key and install them so the resolve chain (every feature) can work.
    let server_pub = match hex::decode(enc_pub).ok().and_then(|b| <[u8; 32]>::try_from(b).ok()) {
        Some(b) => b,
        None => return network_error(),
    };
    let iv = match hex::decode(iv_hex) { Ok(b) => b, Err(_) => return network_error() };
    let ct = match hex::decode(ct_hex) { Ok(b) => b, Err(_) => return network_error() };
    let plain = match decrypt_offsets(cpriv, server_pub, &iv, &ct) {
        Some(p) => p,
        None => return network_error(), // couldn't decrypt → treat as unreachable
    };
    // Plaintext is "<fdmEntry>|<fdmDataModel>" in decimal.
    let mut parts = plain.split('|');
    let fdm_entry = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let fdm_datamodel = parts.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    crate::security::set_chain_offsets(crate::security::ChainOffsets {
        fdm_entry,
        fdm_datamodel,
    });
    KeyResult {
        valid: true,
        expires_after: r_expiry,
        network_error: false,
        version_blocked: false,
        latest_version: r_latest.to_string(),
    }
}

/// Keyless pre-launch version check. Asks the server whether THIS build hash is
/// disabled, verifies the Ed25519-signed answer, and returns `Some(latest)` if
/// it is (so the caller can show "new version" and quit before anything runs).
/// Fails open — any network / signature / freshness problem returns None so a
/// legitimate offline user can still launch (the 5 s watcher covers them later).
pub fn version_gate_check() -> Option<String> {
    let vk = verifying_key()?;
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(6))
        .build()
        .ok()?;
    let nonce = uuid::Uuid::new_v4().simple().to_string();
    let body = serde_json::json!({
        "vcheck": true,
        "build": crate::version::VERSION,
        "nonce": nonce,
    });
    let raw = serde_json::to_string(&body).ok()?;
    let ts = now_unix().to_string();
    let sig = sign(&ts, &raw);
    let anon = secrets::supabase_anon_key();

    let resp = client
        .post(fn_url())
        .header("apikey", &anon)
        .header("Authorization", format!("Bearer {}", anon))
        .header("Content-Type", "application/json")
        .header("x-socrate-ts", &ts)
        .header("x-socrate-sig", &sig)
        .body(raw)
        .send()
        .ok()?;
    if !resp.status().is_success() {
        return None; // fail open
    }
    let v: serde_json::Value = resp.json().ok()?;

    let blocked = v["blocked"].as_bool().unwrap_or(false);
    let latest = v["latest"].as_str().unwrap_or("");
    let r_nonce = v["nonce"].as_str().unwrap_or("");
    let iat = v["iat"].as_i64().unwrap_or(0);
    let r_sig = v["sig"].as_str().unwrap_or("");

    let msg = format!("vcheck|{}|{}|{}|{}", r_nonce, if blocked { 1 } else { 0 }, latest, iat);
    let sig_bytes = hex::decode(r_sig).ok().and_then(|b| <[u8; 64]>::try_from(b).ok())?;
    if vk.verify(msg.as_bytes(), &Signature::from_bytes(&sig_bytes)).is_err() {
        return None; // fail open on a bad/forged signature
    }
    if r_nonce != nonce || (now_unix() - iat).abs() > MAX_VERDICT_AGE {
        return None;
    }
    if blocked {
        Some(latest.to_string())
    } else {
        None
    }
}

/// Re-validation cadence for the live license watcher.
const WATCH_INTERVAL: Duration = Duration::from_secs(5);

/// Background thread that re-checks the active license every 5 s so a key
/// deleted / blacklisted / hwid-reset on the dashboard boots the user out of
/// Socrate live, without a restart.
///
/// The token is pushed by the frontend via `set_license_token`. We only act on
/// an *explicit* invalid verdict (`valid == false && !network_error`): a
/// transient network failure must never log a legitimate user out. On a real
/// revocation we flip the authoritative `key_unlocked` kill switch (instantly
/// freezing every feature thread), drop the stored token, and emit
/// `license_revoked` so the UI bounces back to the key screen.
pub fn start_license_watch_thread(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(WATCH_INTERVAL);
        let state = app.state::<AppState>();

        // Nothing to do until a key has actually been activated.
        if !state.key_unlocked.load(Ordering::Relaxed) {
            continue;
        }
        let token = match state.license_token.lock().unwrap().clone() {
            Some(t) if !t.trim().is_empty() => t,
            _ => continue,
        };

        let res = check_license(&token);

        // Version gate — this build was disabled server-side. Freeze features,
        // tell the user which version to update to, then quit Socrate.
        if res.version_blocked {
            state.key_unlocked.store(false, Ordering::Relaxed);
            show_version_dialog(&res.latest_version);
            std::process::exit(0);
        }

        // Only an authoritative "invalid" logs out; offline/rate-limited
        // (network_error) fails open so a legit user isn't kicked.
        if !res.valid && !res.network_error {
            state.key_unlocked.store(false, Ordering::Relaxed);
            *state.license_token.lock().unwrap() = None;
            let _ = app.emit("license_revoked", ());
        }
    });
}

/// Called by the frontend when a validation on the key screen / first launch
/// comes back version-blocked (the watcher covers the mid-session case).
#[tauri::command]
pub fn report_version_blocked(latest: String) {
    show_version_dialog(&latest);
    std::process::exit(0);
}

/// Blocking "you must update" dialog (rfd → NSAlert on macOS), shown just
/// before the process exits. Mirrors the blacklist dialog in `telemetry`.
fn show_version_dialog(latest: &str) {
    use rfd::{MessageButtons, MessageDialog, MessageLevel};
    let hash = if latest.is_empty() { "unknown" } else { latest };
    MessageDialog::new()
        .set_title("Socrate")
        .set_description(format!("Socrate new version : {hash}\n\nThis version has been disabled. Please download the latest build."))
        .set_level(MessageLevel::Warning)
        .set_buttons(MessageButtons::Ok)
        .show();
}
