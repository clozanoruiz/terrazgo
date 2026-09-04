// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What a call costs the database: statements begun, and rows produced.
//!
//! Both halves are invisible to every other kind of test, because both return
//! the right answer. They are the two shapes the query-scope audit found, and
//! they need different instruments:
//!
//!   * **statements** catch an N+1 — list a season, list a season with four
//!     times the records, and check the count stands still while the rows
//!     multiply. That is the `CatalogueCache` test's shape
//!     (`terrazgo-recordbook`), generalised to anything holding a `Connection`.
//!   * **rows** catch an unbounded result set, which is the failure no index
//!     can fix. A query answering a question about today, reading twenty
//!     campaigns to do it, runs *one* statement — so counting statements says
//!     it is fine. Counting the rows it made SQLite produce says it is not.
//!
//! **Why this is in the testkit at all**, given the core-only rule: it needs
//! `rusqlite` and nothing else. A helper that counts statements knows nothing
//! about which tables exist, so it never becomes the back door between modules
//! the crate docs warn about. The scaled-data builders that feed it *do* need
//! module tables, and they live in each crate's own `tests/common`.
//!
//! The `trace` feature is enabled on this crate's `rusqlite` only. The
//! workspace is on `resolver = "3"`, and this crate is a dev-dependency
//! everywhere, so the feature is absent from every non-test build.

use std::cell::Cell;

use rusqlite::Connection;
use rusqlite::trace::{TraceEvent, TraceEventCodes};

/// What a counted block cost the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QueryCost {
    /// Prepared statements that began running. Grows with the record count
    /// when a list hydrates its children one parent at a time.
    pub statements: usize,
    /// Result rows SQLite produced. Grows with the record count when a query
    /// answering a question about today reads the whole history to do it.
    pub rows: usize,
}

thread_local! {
    /// The tally for this thread, since the last [`query_cost`] call.
    ///
    /// A thread local rather than a field, because SQLite's tracer is a bare
    /// `fn(TraceEvent)` with nowhere to hang captured state. Tests run in
    /// parallel threads and each owns its own connection, so per-thread is also
    /// the correct granularity — two tests counting at once cannot see each
    /// other's work.
    static STATEMENTS: Cell<usize> = const { Cell::new(0) };
    static ROWS: Cell<usize> = const { Cell::new(0) };
}

/// `SQLITE_TRACE_STMT` fires when a prepared statement begins running;
/// `SQLITE_TRACE_ROW` fires once per row it produces. Preparing costs little —
/// running the same shape once per record, or producing rows nobody asked for,
/// are the two defects worth failing a test on.
fn tally(event: TraceEvent<'_>) {
    match event {
        TraceEvent::Stmt(..) => STATEMENTS.with(|n| n.set(n.get() + 1)),
        TraceEvent::Row(..) => ROWS.with(|n| n.set(n.get() + 1)),
        _ => {}
    }
}

/// Run `work` and report both its result and what it cost.
///
/// Not re-entrant: the tally is per thread, so a nested call would reset the
/// outer one. There is no reason to nest, and the shape of the API is what
/// stops a stale count leaking into the next assertion.
///
/// ```
/// use terrazgo_testkit::query_cost;
///
/// let mut conn = terrazgo_core::open_in_memory().unwrap();
/// let (seasons, cost) = query_cost(&mut conn, |conn| {
///     terrazgo_core::repository::list_seasons(conn).unwrap()
/// });
/// assert!(seasons.is_empty());
/// assert_eq!(cost.statements, 1, "one list is one statement");
/// assert_eq!(cost.rows, 0, "and an empty table produces no rows");
/// ```
pub fn query_cost<T>(
    conn: &mut Connection,
    work: impl FnOnce(&mut Connection) -> T,
) -> (T, QueryCost) {
    STATEMENTS.with(|n| n.set(0));
    ROWS.with(|n| n.set(0));
    conn.trace_v2(
        TraceEventCodes::SQLITE_TRACE_STMT | TraceEventCodes::SQLITE_TRACE_ROW,
        Some(tally),
    );
    let result = work(conn);
    conn.trace_v2(TraceEventCodes::empty(), None);
    let cost = QueryCost {
        statements: STATEMENTS.with(Cell::get),
        rows: ROWS.with(Cell::get),
    };
    (result, cost)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Five parents, one child each, so a row count is unambiguous.
    fn parents_and_children() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE parent (id INTEGER PRIMARY KEY);
             CREATE TABLE child (parent_id INTEGER);
             INSERT INTO parent (id) VALUES (1), (2), (3), (4), (5);
             INSERT INTO child (parent_id) VALUES (1), (2), (3), (4), (5);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn counts_one_statement_per_execution() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY)")
            .unwrap();

        let (_, cost) = query_cost(&mut conn, |conn| {
            for id in 0..5 {
                conn.execute("INSERT INTO t (id) VALUES (?1)", [id])
                    .unwrap();
            }
        });
        assert_eq!(
            cost.statements, 5,
            "five executions of one prepared shape are five"
        );
    }

    #[test]
    fn a_query_per_parent_is_what_the_statement_count_catches() {
        // The N+1 in miniature: same result either way, five statements against
        // one. Without this helper the two are indistinguishable from a test.
        let mut conn = parents_and_children();

        let (_, per_parent) = query_cost(&mut conn, |conn| {
            for id in 1..=5 {
                conn.query_row(
                    "SELECT COUNT(*) FROM child WHERE parent_id = ?1",
                    [id],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap();
            }
        });

        let (_, hoisted) = query_cost(&mut conn, |conn| {
            let mut stmt = conn
                .prepare("SELECT parent_id, COUNT(*) FROM child GROUP BY parent_id")
                .unwrap();
            stmt.query_map([], |r| r.get::<_, i64>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap();
        });

        assert_eq!(per_parent.statements, 5);
        assert_eq!(hoisted.statements, 1);
    }

    #[test]
    fn reading_rows_nobody_asked_for_is_what_the_row_count_catches() {
        // Both are ONE statement, so the statement count calls them equal. The
        // row count is what separates "answer the question" from "read the
        // table and discard most of it" — the defect an index cannot fix.
        let mut conn = parents_and_children();

        let (_, whole_table) = query_cost(&mut conn, |conn| {
            let mut stmt = conn.prepare("SELECT parent_id FROM child").unwrap();
            stmt.query_map([], |r| r.get::<_, i64>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
                .into_iter()
                .filter(|id| *id == 3)
                .collect::<Vec<_>>()
        });

        let (_, scoped) = query_cost(&mut conn, |conn| {
            let mut stmt = conn
                .prepare("SELECT parent_id FROM child WHERE parent_id = 3")
                .unwrap();
            stmt.query_map([], |r| r.get::<_, i64>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        });

        assert_eq!(whole_table.statements, scoped.statements, "one each");
        assert_eq!((whole_table.rows, scoped.rows), (5, 1));
    }

    #[test]
    fn the_tally_does_not_leak_between_calls() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY)")
            .unwrap();

        let (_, first) = query_cost(&mut conn, |conn| {
            conn.execute("INSERT INTO t (id) VALUES (1)", []).unwrap();
        });
        let (_, second) = query_cost(&mut conn, |conn| {
            conn.execute("INSERT INTO t (id) VALUES (2)", []).unwrap();
        });
        assert_eq!(first.statements, 1);
        assert_eq!(second.statements, 1);
    }

    #[test]
    fn work_done_outside_a_counted_block_is_not_counted() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY)")
            .unwrap();

        let (_, first) = query_cost(&mut conn, |conn| {
            conn.execute("INSERT INTO t (id) VALUES (1)", []).unwrap();
        });
        // The tracer is uninstalled on the way out, so this one is invisible.
        conn.execute("INSERT INTO t (id) VALUES (2)", []).unwrap();

        let (_, second) = query_cost(&mut conn, |conn| {
            conn.query_row("SELECT COUNT(*) FROM t", [], |r| r.get::<_, i64>(0))
                .unwrap()
        });
        assert_eq!(first.statements, 1);
        assert_eq!(second.statements, 1);
    }
}
