// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! One device's whole log carried to another through the real bundle
//! transport: written, read back from its bytes, and applied.
//!
//! Three test files across three crates had their own copy. A scenario that
//! needs trimmed replies, a refused file or the bytes themselves still builds
//! its own devices — what lives here is only the exchange a test makes when
//! the transport is not what it is about, and the one thing that makes two
//! devices differ without a sync: their catalogues.

use rusqlite::Connection;
use terrazgo_core::bundle::{self, Bundle, ImportSummary};
use terrazgo_core::sync::VersionVector;

/// Everything `from` holds, as the file a device writes when it knows nothing
/// of the one it is writing for. Change sets the receiver already holds are
/// skipped by the import, so sending the whole log again is always safe.
pub fn whole_log(from: &Connection) -> Bundle {
    let device = terrazgo_core::sync::installed_device(from).unwrap();
    let mut bytes = Vec::new();
    bundle::write_bundle(from, &device, &VersionVector::default(), &mut bytes).unwrap();
    bundle::read_bundle(&bytes[..]).unwrap()
}

/// Everything `from` holds, applied to `to`.
///
/// Panics if `to` refuses the file — a test whose subject is a refusal calls
/// `apply_bundle` on [`whole_log`] itself.
pub fn send(from: &Connection, to: &mut Connection) -> ImportSummary {
    bundle::apply_bundle(to, &whole_log(from), terrazgo_core::date::now_ms()).unwrap()
}

/// `to` joins `from`'s sync group and receives everything `from` wrote — a
/// second device after its first sync. `from`'s group is minted if it has
/// none, as an export does.
pub fn join_and_receive(from: &Connection, to: &mut Connection) {
    let group = terrazgo_core::sync::ensure_sync_group(from).unwrap();
    terrazgo_core::sync::join_sync_group(to, &group).unwrap();
    send(from, to);
}

/// `codes` in this device's copy of `catalogue_id`, the catalogue marked as
/// imported if it was not — what a catalogue refresh leaves on one device and
/// on no other, since catalogues never travel. Set by SQL because the refresh
/// itself fetches from the provider.
pub fn hold_codes(conn: &Connection, catalogue_id: &str, codes: &[&str]) {
    conn.execute(
        "INSERT OR IGNORE INTO catalogue (id, source, imported_at)
         VALUES (?1, 'siex', '2026-07-15T00:00:00Z')",
        [catalogue_id],
    )
    .unwrap();
    for code in codes {
        conn.execute(
            "INSERT INTO catalogue_code (catalogue_id, code, label) VALUES (?1, ?2, ?2)",
            [catalogue_id, code],
        )
        .unwrap();
    }
}
