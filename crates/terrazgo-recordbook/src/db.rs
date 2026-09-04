// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The schema a record book needs in order to be assembled at all.
//!
//! The book owns no tables — it is a read model. But it reads core AND every
//! module that contributes a section, so the set of migrations it needs is the
//! same composition the shell performs at startup, and nothing smaller. Before
//! a second module joined, the tests borrowed `module_cue::open_in_memory` and
//! that happened to be enough; it stopped being enough the moment section 8
//! arrived, with a failure ("no such table") that named a missing table rather
//! than the missing composition.
//!
//! So the composition lives here, next to the code whose correctness depends
//! on it. A module joining the book adds one line to [`migrations`] and one
//! dependency — the cost the consumer-crate design was chosen knowing about
//! (docs/architecture.md → the record book).

use crate::error::Result;
use rusqlite::Connection;
use rusqlite_migration::Migrations;

/// Core's migration steps followed by those of every module the book reads, in
/// the shell's registration order. Kept in step with `src-tauri/src/registry.rs`
/// by `the_record_book_composes_the_same_schema_the_shell_does` in that crate's
/// `tests/migration_composition.rs`, which migrates a database each way and
/// compares `sqlite_schema`.
///
/// That test is younger than this comment, which asserted it from 2026-08-07
/// while nothing of the sort existed — a module forgotten here would have shown
/// up as "no such table" in whichever seam first read it. Verified by breaking
/// it when the third module joined.
pub fn migrations() -> Migrations<'static> {
    let mut steps = terrazgo_core::migration_set();
    steps.extend(module_cue::migration_set());
    steps.extend(module_fertilisation::migration_set());
    steps.extend(module_ecoscheme::migration_set());
    Migrations::new(steps)
}

/// An in-memory database carrying the whole book's schema, for this crate's
/// tests and for any caller that needs to render a book against a scratch
/// database. The app opens its database through the shell's composed runner.
pub fn open_in_memory() -> Result<Connection> {
    let mut conn = Connection::open_in_memory()?;
    conn.pragma_update(None, "foreign_keys", true)?;
    // Tests run on the same connection configuration the app does.
    terrazgo_core::db::harden(&conn)?;
    migrations()
        .to_latest(&mut conn)
        .map_err(|e| crate::error::RecordbookError::Internal(e.to_string()))?;
    Ok(conn)
}
