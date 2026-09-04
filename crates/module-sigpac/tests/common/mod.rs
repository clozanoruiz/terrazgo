// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared plumbing for this crate's integration tests.
//!
//! Unlike the register modules, what these three files were copying is not a
//! fixture of rows but a fixture of *responses*: the harvested Nube de SIGPAC
//! payloads under `tests/fixtures/`, declared by `include_bytes!` in two or
//! three files each, and the geo-cache seeding that serves them offline.
//!
//! `terrazgo-testkit` is not re-exported here: nothing in this crate's tests
//! builds a farm-with-plots fixture — `service.rs` needs a plot carrying a
//! specific SIGPAC reference, which is its own thing.

// Each test binary compiles this whole module and uses a subset of it, so what
// one binary does not touch is not dead code — it is the other binaries' half
// of the shared helper.
#![allow(dead_code)]
// Clippy's `allow-unwrap-in-tests` only covers `#[test]` fns, not shared
// helpers — the file-level allow is the workspace convention.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rusqlite::Connection;
use terrazgo_core::db::Database;
use terrazgo_geo::db::open_cache_in_memory;

// --- the harvested responses ------------------------------------------------
//
// REAL Nube de SIGPAC responses harvested 2026-07-08 (recinto
// 34/10/0/0/604/5021/13, Palencia, and two Valladolid recintos for the
// declarations). No test touches the network; the client is exercised through a
// pre-seeded in-memory geo cache.

pub const RECINFO: &[u8] = include_bytes!("../fixtures/recinfo.geojson");
pub const RECINFO_NOT_FOUND: &[u8] = include_bytes!("../fixtures/recinfo-notfound.geojson");
pub const RECINFO_BY_POINT: &[u8] = include_bytes!("../fixtures/recinfobypoint.geojson");
pub const NITRATOS: &[u8] = include_bytes!("../fixtures/intersection-nitratos.json");
pub const FITOSANITARIOS: &[u8] = include_bytes!("../fixtures/intersection-fitosanitarios.json");
pub const RED_NATURA: &[u8] = include_bytes!("../fixtures/intersection-red-natura.json");
pub const CAMPAIGNS: &[u8] = include_bytes!("../fixtures/geopackages-listing.html");
pub const DECLARED: &[u8] = include_bytes!("../fixtures/cultivo-declarado.json");
pub const DECLARED_EMPTY: &[u8] = include_bytes!("../fixtures/cultivo-declarado-empty.json");
pub const DECLARED_SECONDARY: &[u8] =
    include_bytes!("../fixtures/cultivo-declarado-secondary.json");
pub const DECLARED_MULTILINE: &[u8] =
    include_bytes!("../fixtures/cultivo-declarado-multiline.json");

/// The day the fixtures above were harvested, as the cache writes it.
pub const HARVESTED_AT: &str = "2026-07-08T00:00:00Z";

/// Today, as the cache writes it.
pub fn today_stamp() -> String {
    format!("{}T00:00:00Z", terrazgo_core::date::today_utc())
}

// --- the offline cache ------------------------------------------------------

/// A migrated in-memory app database at core's schema.
pub fn app_db() -> Connection {
    terrazgo_core::db::open_in_memory().unwrap()
}

/// A geo cache pre-seeded with the given entries at an explicit fetch time.
///
/// The time is a parameter rather than a constant because the two callers need
/// different answers from it, and neither is the default: see
/// [`seeded_cache`] and [`seeded_cache_today`].
pub fn seeded_cache_at<K: AsRef<str>>(entries: &[(K, &[u8])], fetched_at: &str) -> Database {
    let cache = open_cache_in_memory().unwrap();
    for (key, data) in entries {
        seed_resource_at(&cache, key.as_ref(), data, fetched_at);
    }
    cache
}

/// Seeded as fetched on the day the fixtures were harvested — the right choice
/// wherever the age of a cached answer is not part of what is being tested.
pub fn seeded_cache<K: AsRef<str>>(entries: &[(K, &[u8])]) -> Database {
    seeded_cache_at(entries, HARVESTED_AT)
}

/// Seeded as fetched TODAY, which is not interchangeable with the above: the
/// declared-crops fallback re-asks an EMPTY current-campaign answer that was
/// stored on an earlier day, so a test about a trusted empty seeded at a fixed
/// past date would silently depend on the machine having network.
pub fn seeded_cache_today<K: AsRef<str>>(entries: &[(K, &[u8])]) -> Database {
    seeded_cache_at(entries, &today_stamp())
}

/// Add one entry to an already-open cache, at the harvest time.
pub fn seed_resource(cache: &Database, key: &str, data: &[u8]) {
    seed_resource_at(cache, key, data, HARVESTED_AT);
}

/// Add one entry to an already-open cache at an explicit fetch time — for the
/// tests where the AGE of a cached answer is what is under test.
pub fn seed_resource_at_time(cache: &Database, key: &str, data: &[u8], fetched_at: &str) {
    seed_resource_at(cache, key, data, fetched_at);
}

fn seed_resource_at(cache: &Database, key: &str, data: &[u8], fetched_at: &str) {
    let guard = cache.lock().unwrap();
    guard
        .conn()
        .unwrap()
        .execute(
            "INSERT INTO resource (key, data, content_type, fetched_at)
             VALUES (?1, ?2, 'application/json', ?3)",
            rusqlite::params![key, data, fetched_at],
        )
        .unwrap();
}
