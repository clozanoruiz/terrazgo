// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Terrazgo network seam: the one HTTP agent the app owns, its TLS trust
//! policy, and the offline diagnosis every caller shares.
//!
//! Deliberately thin. This crate knows nothing about caching, tiles, sources,
//! catalogues or user data — it answers "fetch these bytes, or say why not".
//! Everything above it decides what to ask for and what to do with the answer:
//! `terrazgo-geo` wraps it in cache-through semantics for the map, the shell
//! calls it directly for the catalogue refresh (which does not want caching —
//! catalogue rows land in the app database, not in `geo-cache.db`).
//!
//! **`terrazgo-core` and the modules never depend on this crate.** Core having
//! no HTTP crate anywhere in its tree is the build-enforced form of the
//! offline-first rule; a module inheriting network access through core would
//! dissolve the single-seam discipline (docs/architecture.md → "The network
//! seam").
//!
//! It exists because a second in-app consumer became real (2026-08-09, the
//! user-triggered catalogue refresh). Until then the same ~100 lines lived in
//! `terrazgo-geo::fetch`, which was correct while the map was the only thing
//! that fetched.

use std::sync::OnceLock;
use std::time::Duration;

use thiserror::Error;
use ureq::tls::{RootCerts, TlsConfig};

// Android-only TLS bootstrap for the platform verifier; private — its one
// entry point is called from `http_get`.
#[cfg(target_os = "android")]
mod android;

/// Crate-local result alias so signatures stay short.
pub type Result<T> = std::result::Result<T, NetError>;

/// Why a fetch did not produce bytes. Two cases, because callers treat them
/// differently: an HTTP status is the server answering (a 404 can even be
/// meaningful — the map caches empty vector tiles from one), while `Offline`
/// is the network itself failing and the app is expected to degrade.
#[derive(Debug, Error)]
pub enum NetError {
    /// The upstream service answered with a non-success HTTP status.
    #[error("upstream returned HTTP {status}")]
    Http { status: u16 },

    /// The network itself failed (DNS, connect, TLS, timeout, a truncated
    /// body) — the offline case. The string carries the underlying reason so
    /// the UI can show more than "offline" (field diagnosis, 2026-07-09).
    #[error("network unavailable: {0}")]
    Offline(String),
}

/// A fetched payload plus the content type it arrived with.
pub struct Fetched {
    pub data: Vec<u8>,
    pub content_type: String,
}

/// The process-wide agent. One instance so connection pooling and the TLS
/// configuration are shared by every consumer.
fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            // Trust what the platform trusts (the OS certificate store), like
            // a browser does. ureq's default — pinned Mozilla roots — rejects
            // the re-signed certificates of antivirus/proxy HTTPS
            // interception, common on consumer Windows (field bug 2026-07-09:
            // UnknownIssuer in the app while every browser on the machine
            // connected fine).
            .tls_config(
                TlsConfig::builder()
                    .root_certs(RootCerts::PlatformVerifier)
                    .build(),
            )
            // Identify politely to the public services we fetch from.
            .user_agent("Terrazgo/0.1 (offline-first farm app)")
            .build()
            .into()
    })
}

/// GET one URL into memory. `fallback_content_type` is used when the response
/// carries no `content-type` header of its own.
pub fn http_get(url: &str, fallback_content_type: &str) -> Result<Fetched> {
    // Android: the platform verifier PANICS inside the request if it never
    // got its JNI handles (silent blank map, 2026-07-17 field test) — hand
    // them over before ureq can need them. A failure surfaces through the
    // normal offline diagnosis instead of a dead tokio worker.
    #[cfg(target_os = "android")]
    android::ensure_platform_verifier().map_err(NetError::Offline)?;
    let mut response = agent().get(url).call().map_err(|err| match err {
        ureq::Error::StatusCode(status) => NetError::Http { status },
        other => NetError::Offline(other.to_string()),
    })?;
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or(fallback_content_type)
        .to_string();
    let data = response
        .body_mut()
        .read_to_vec()
        .map_err(|e| NetError::Offline(e.to_string()))?;
    Ok(Fetched { data, content_type })
}
