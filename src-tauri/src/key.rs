//! Work.ink key-system gate.
//!
//! The frontend collects a token (the user gets it by completing the steps on
//! a work.ink link) and calls `validate_workink_key`. We verify it server-side
//! here — doing the HTTP from Rust instead of the webview avoids the browser
//! CORS wall on the work.ink API and keeps the check off the page's JS.
//!
//! `deleteToken=1` makes the token single-use: work.ink deletes it the moment
//! we validate it, so a leaked token can't be replayed. On success we hand the
//! frontend the `expiresAfter` timestamp; the frontend stores it and trusts the
//! local clock until it lapses, then asks for a fresh key.

use serde::Serialize;
use std::time::Duration;

#[derive(Serialize, Clone, Default)]
pub struct KeyResult {
    /// Token was accepted by work.ink.
    pub valid: bool,
    /// Unix-millis timestamp the access expires at (0 when unknown/invalid).
    pub expires_after: i64,
    /// Set when the network call itself failed (lets the UI say "try again"
    /// rather than "wrong key").
    pub network_error: bool,
}

/// Validate a work.ink token. Single-use (`deleteToken=1`).
#[tauri::command]
pub fn validate_workink_key(token: String) -> KeyResult {
    let token = token.trim();
    if token.is_empty() {
        return KeyResult::default();
    }

    let url = format!(
        "https://work.ink/_api/v2/token/isValid/{}?deleteToken=1",
        token
    );

    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            return KeyResult {
                network_error: true,
                ..Default::default()
            }
        }
    };

    let resp = match client.get(&url).send().and_then(|r| r.error_for_status()) {
        Ok(r) => r,
        Err(_) => {
            return KeyResult {
                network_error: true,
                ..Default::default()
            }
        }
    };

    match resp.json::<serde_json::Value>() {
        Ok(v) => {
            let valid = v["valid"].as_bool().unwrap_or(false);
            let expires_after = v["info"]["expiresAfter"].as_i64().unwrap_or(0);
            KeyResult {
                valid,
                expires_after,
                network_error: false,
            }
        }
        Err(_) => KeyResult {
            network_error: true,
            ..Default::default()
        },
    }
}
