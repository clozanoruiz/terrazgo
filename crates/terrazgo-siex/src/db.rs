// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The schema an export needs in order to be built at all.
//!
//! This crate owns no tables — like the record book it is a read model over
//! core and every module. And like the record book it needs the *composed*
//! schema, not any one module's: while `TratamFito` was the only block the
//! tests could borrow `module_cue::open_in_memory` and it happened to be
//! enough, exactly as the book's did before its second section arrived.
//!
//! Composing it here, next to the code whose correctness depends on it, is what
//! turns "no such table" in some future seam into a one-line change now.

use crate::error::{Result, SiexError};
use rusqlite::Connection;
use rusqlite_migration::Migrations;

/// Core's migration steps followed by those of every module the export reads,
/// in the shell's registration order. Kept in step with
/// `src-tauri/src/registry.rs` by a contract test in that crate's
/// `tests/migration_composition.rs`, which migrates a database each way and
/// compares `sqlite_schema` — the same guard the record book gets, added for
/// the same reason.
pub fn migrations() -> Migrations<'static> {
    let mut steps = terrazgo_core::migration_set();
    steps.extend(module_cue::migration_set());
    steps.extend(module_fertilisation::migration_set());
    steps.extend(module_ecoscheme::migration_set());
    Migrations::new(steps)
}

/// An in-memory database carrying the whole schema the export reads, for this
/// crate's tests. The app opens its database through the shell's composed
/// runner.
pub fn open_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", true)?;
    // Tests run on the same connection configuration the app does.
    terrazgo_core::db::harden(&conn)?;
    migrations()
        .to_latest(&mut conn)
        .map_err(|e| SiexError::Internal(e.to_string()))?;
    Ok(conn)
}
