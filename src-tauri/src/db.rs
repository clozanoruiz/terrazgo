// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The shell-owned database runner: composes every module's migration steps into
//! the single global version sequence and opens the app database.
//!
//! Errors here are `anyhow` by design — src-tauri *is* the Tauri command
//! boundary, not a reusable library crate (thiserror stays in the crates,
//! anyhow at the boundary). If the core ever becomes a shared crate, promote to `thiserror`.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags};
use rusqlite_migration::{M, Migrations};
use terrazgo_core::settings::IntegrityCheck;

use crate::registry;

/// Migration steps owned by the core: the `terrazgo-core` crate keeps the SQL
/// (farm, plot, country, record_change), exactly as module crates keep theirs.
/// Public so tests can pin the global version count.
pub fn core_migrations() -> Vec<M<'static>> {
    terrazgo_core::migration_set()
}

/// The single global migration sequence: core steps first, then each registered
/// module's steps in registry order. The resulting version numbers are GLOBAL —
/// cue's two migrations are global v1 and v2 today.
///
/// Pre-release, reordering/squashing is allowed and dev databases are recreated;
/// the moment any database holds real data, this composed sequence becomes
/// append-only as a whole: new migrations join at the global tail regardless of
/// which crate owns the SQL (docs/architecture.md → Migrations: one global sequence).
pub fn composed_migrations() -> Migrations<'static> {
    let mut steps = core_migrations();
    for module in registry::registered_modules() {
        steps.extend(module.migrations());
    }
    Migrations::new(steps)
}

/// Open (or create) the app database: WAL mode + foreign keys + the composed
/// global migrations. Mirrors `module_cue::db::open`, which stays library-only.
///
/// Three settings are deliberately left at their defaults, and all three are
/// the kind someone "optimises" later without noticing what they cost:
///
/// * `synchronous` stays FULL. This file is a legal record with a three-year
///   retention duty, not a cache; `NORMAL` is the usual WAL advice and is
///   faster, but it trades away durability across a power cut.
/// * `mmap_size` stays 0. With memory-mapped I/O a stray pointer anywhere in
///   the process can silently corrupt the database file.
/// * `cell_size_check` stays off.
pub fn open_app_db(path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(path)
        .with_context(|| format!("opening database at {}", path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", true)?;
    // Applied to our own database too, not just imported ones: it costs
    // nothing (no views, no triggers) and it means nothing running in SQL can
    // drop this file out of WAL or rewrite its schema.
    terrazgo_core::db::harden(&conn)?;
    composed_migrations()
        .to_latest(&mut conn)
        .context("applying the global migration sequence")?;
    Ok(conn)
}

/// Current global schema version — the SQLite `user_version` pragma, which
/// `rusqlite_migration` maintains as it applies steps.
pub fn schema_version(conn: &Connection) -> Result<usize> {
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    usize::try_from(version).context("negative user_version")
}

// ---------------------------------------------------------------------------
// Corruption checking
// ---------------------------------------------------------------------------

/// How long a corruption verdict stands before the database is checked again.
///
/// Not regulatory and deliberately not a setting. It fails the test the alert
/// lead times pass: a farmer has no basis on which to prefer a fortnight to a
/// week, and would never feel the difference — it paces work nobody asked for,
/// rather than answering a question only they can answer. Weekly because the
/// check is linear in database size (1.6 ms/MB, measured 2026-08-26 over
/// 29-513 MB): free for a smallholder, and 0.8 s for a cooperative-scale book,
/// which is work nobody asked for to repeat on every launch.
const INTEGRITY_CHECK_INTERVAL_DAYS: i64 = 7;

/// Whether the last verdict is old enough to re-check. Never checked, or a
/// timestamp we cannot read, both mean "check now" — the honest answer when we
/// do not know is to look.
fn integrity_check_is_due(previous: Option<&IntegrityCheck>, today: &str) -> bool {
    let Some(previous) = previous else {
        return true;
    };
    let Some(last_day) = previous.at.split('T').next() else {
        return true;
    };
    match terrazgo_core::date::add_days(last_day, INTEGRITY_CHECK_INTERVAL_DAYS) {
        // ISO dates compare lexicographically, which is chronologically.
        Ok(next_due) => next_due.as_str() <= today,
        Err(_) => true,
    }
}

/// `PRAGMA quick_check` on `path`, as a verdict.
///
/// `quick_check` rather than `integrity_check`: it costs about half as much
/// (measured 1.5-2.1x across sizes, converging on 2.1x) and catches structural
/// page damage, which is what a recurring background check is for. The thorough one still runs on
/// every backup, where `VACUUM INTO` reads every page and the copy is verified.
///
/// Any failure to run at all is itself a bad verdict — a database too damaged
/// to answer is not a healthy one.
fn quick_check(path: &Path) -> Result<IntegrityCheck> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| format!("opening {} to check it", path.display()))?;
    terrazgo_core::db::harden(&conn)?;
    let verdict = run_check(&conn, "PRAGMA quick_check", false);
    conn.close().map_err(|(_, err)| err)?;
    Ok(verdict)
}

/// The thorough check, on an already-open connection — what the Settings
/// button runs.
///
/// `integrity_check` rather than `quick_check` because the two are asked for
/// different reasons. The weekly one is a cheap structural screen nobody
/// requested, so it is paced to cost nothing; this one is a person asking, and
/// a person asking is willing to wait. It buys the checks `quick_check` skips:
/// that every index row matches a table row and back, and that UNIQUE, NOT NULL
/// and CHECK constraints actually hold. Roughly twice the cost: measured
/// 2026-08-26 at 3.4 ms/MB against quick_check's 1.6, over 29-513 MB
/// (`src-tauri/tests/quick_check_cost.rs` re-runs it). That is ~20 ms on a
/// smallholder's 6 MB book and 1.7 s on a 513 MB cooperative-scale one —
/// nothing for something a person asked for and is waiting on.
pub(crate) fn integrity_check(conn: &Connection) -> IntegrityCheck {
    run_check(conn, "PRAGMA integrity_check", true)
}

/// Run one of the two check pragmas and stamp the verdict.
///
/// Both answer the single string "ok" when the file is sound and one row per
/// problem otherwise, so reading the first row is enough. **Any failure to run
/// at all is itself a bad verdict** — a database too damaged to answer is not a
/// healthy one, and reporting the error instead would leave the farmer with no
/// verdict rather than a bad one.
fn run_check(conn: &Connection, pragma: &str, thorough: bool) -> IntegrityCheck {
    let ok = conn
        .query_row(pragma, [], |r| r.get::<_, String>(0))
        .map(|verdict| verdict == "ok")
        .unwrap_or(false);
    IntegrityCheck {
        at: terrazgo_core::date::now_utc_iso(),
        ok,
        thorough,
    }
}

/// The database's size as SQLite sees it, in bytes.
///
/// `page_count * page_size` rather than the file's length on disk: in WAL mode
/// pages live in the `-wal` sidecar until a checkpoint, so the file's size is
/// not the honest number at the moment a VACUUM finishes.
pub(crate) fn database_bytes(conn: &Connection) -> Result<i64> {
    let pages: i64 = conn.pragma_query_value(None, "page_count", |r| r.get(0))?;
    let page_size: i64 = conn.pragma_query_value(None, "page_size", |r| r.get(0))?;
    Ok(pages * page_size)
}

/// Check the app database if its last verdict has expired, and record the
/// result in the device-local settings so `get_status` can report it.
///
/// The verdict lives in `settings.json` rather than in the database, which is
/// the point: a file too corrupt to read still has a readable verdict beside
/// it. It persists, so a throttled launch that skips the check still reports
/// what the last one found.
pub fn run_due_integrity_check(app: &tauri::AppHandle) -> Result<()> {
    use tauri::Manager;
    let Some(state) = app.try_state::<crate::state::AppState>() else {
        return Ok(());
    };
    let Some(settings_state) = app.try_state::<crate::state::SettingsState>() else {
        return Ok(());
    };

    let previous = settings_state
        .settings
        .lock()
        .map_err(|_| anyhow::anyhow!("settings mutex is poisoned"))?
        .last_integrity_check
        .clone();
    if !integrity_check_is_due(previous.as_ref(), &terrazgo_core::date::today_utc()) {
        return Ok(());
    }

    let verdict = quick_check(&state.db_path)?;
    if !verdict.ok {
        eprintln!(
            "WARNING: {} failed its integrity check — restore a backup",
            state.db_path.display()
        );
    }

    // Settings are written file-first, in-memory copy second, the same order
    // every other settings write uses.
    let mut settings = settings_state
        .settings
        .lock()
        .map_err(|_| anyhow::anyhow!("settings mutex is poisoned"))?;
    settings.last_integrity_check = Some(verdict);
    terrazgo_core::settings::save_settings(&settings_state.path, &settings)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked(at: &str, ok: bool) -> IntegrityCheck {
        IntegrityCheck {
            at: at.to_string(),
            ok,
            thorough: false,
        }
    }

    #[test]
    fn a_database_never_checked_is_due() {
        assert!(integrity_check_is_due(None, "2026-08-25"));
    }

    #[test]
    fn a_verdict_stands_for_a_week_then_expires() {
        let last = checked("2026-08-18T09:00:00Z", true);
        assert!(
            !integrity_check_is_due(Some(&last), "2026-08-24"),
            "six days on, the previous verdict still stands"
        );
        assert!(
            integrity_check_is_due(Some(&last), "2026-08-25"),
            "on the seventh day it is due again"
        );
        assert!(integrity_check_is_due(Some(&last), "2026-09-30"));
    }

    #[test]
    fn a_failed_verdict_does_not_re_check_any_sooner() {
        // Deliberate: a damaged database is not re-checked every launch, which
        // would be a weekly-paced check turned into a per-launch one exactly
        // when the database is at its slowest to read.
        let last = checked("2026-08-18T09:00:00Z", false);
        assert!(!integrity_check_is_due(Some(&last), "2026-08-24"));
    }

    #[test]
    fn an_unreadable_timestamp_means_check_now() {
        // Never silently skip on data we cannot interpret.
        assert!(integrity_check_is_due(
            Some(&checked("not a date", true)),
            "2026-08-25"
        ));
        assert!(integrity_check_is_due(
            Some(&checked("", true)),
            "2026-08-25"
        ));
    }

    #[test]
    fn a_healthy_database_passes_its_check() {
        let file = terrazgo_testkit::files::TempFile::reserve("shell-quick-check.db");
        let conn = open_app_db(file.path()).unwrap();
        conn.close().map_err(|(_, e)| e).unwrap();

        let verdict = quick_check(file.path()).unwrap();
        assert!(verdict.ok);
        assert!(verdict.at.ends_with('Z'), "recorded as a UTC instant");
        assert!(
            !verdict.thorough,
            "the weekly check is the quick one and must say so"
        );
    }

    #[test]
    fn a_file_that_is_not_a_database_fails_its_check() {
        let file = terrazgo_testkit::files::TempFile::written("shell-not-a-db.db", b"nope");
        assert!(!quick_check(file.path()).unwrap().ok);
    }

    #[test]
    fn the_manual_check_is_the_thorough_one_and_says_so() {
        // The distinction is what makes the button worth pressing: a verdict
        // that did not record WHICH check produced it would make "checked
        // recently, fine" mean two different things.
        let file = terrazgo_testkit::files::TempFile::reserve("shell-integrity-check.db");
        let conn = open_app_db(file.path()).unwrap();

        let verdict = integrity_check(&conn);
        assert!(verdict.ok);
        assert!(verdict.thorough);
        assert!(verdict.at.ends_with('Z'));
        conn.close().map_err(|(_, e)| e).unwrap();
    }

    #[test]
    fn a_damaged_database_fails_the_thorough_check_rather_than_erroring() {
        // A file too damaged to answer is not a healthy one, and the caller
        // needs a verdict rather than an error — it is what gets recorded and
        // shown, and it is what stops the VACUUM.
        let file = terrazgo_testkit::files::TempFile::written("shell-integrity-bad.db", b"nope");
        let conn = Connection::open(file.path()).unwrap();
        assert!(!integrity_check(&conn).ok);
    }

    #[test]
    fn compacting_reclaims_the_space_a_deletion_left_behind() {
        // What the button's second half is for, and the measurement the report
        // makes: `page_count * page_size`, not the file's length, because in WAL
        // mode the pages are still in the sidecar when VACUUM returns.
        let file = terrazgo_testkit::files::TempFile::reserve("shell-vacuum.db");
        let conn = open_app_db(file.path()).unwrap();
        conn.execute_batch(
            "CREATE TABLE bulk (v TEXT);
             WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 5000)
             INSERT INTO bulk SELECT hex(randomblob(200)) FROM n;",
        )
        .unwrap();
        let full = database_bytes(&conn).unwrap();
        conn.execute_batch("DELETE FROM bulk").unwrap();

        // Deleting rows returns pages to the freelist, not to the filesystem —
        // which is exactly why a compact step exists at all.
        assert_eq!(
            database_bytes(&conn).unwrap(),
            full,
            "a delete alone should not have shrunk the file"
        );

        conn.execute_batch("VACUUM").unwrap();
        assert!(
            database_bytes(&conn).unwrap() < full,
            "VACUUM should have returned the free pages"
        );
        conn.close().map_err(|(_, e)| e).unwrap();
    }
}
