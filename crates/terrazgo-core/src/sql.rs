// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Query plumbing every repository needs and none of them should own.
//!
//! In core for the reason [`crate::audit`] is: a module may never depend on
//! another module, so anything more than one of them needs has to sit below
//! them all. It carries no domain knowledge — no table of ours is named here —
//! which is what keeps it plumbing rather than a place for logic to collect.

use std::collections::HashMap;

use rusqlite::{Connection, Row, params_from_iter};

/// Parent ids per statement. Comfortably under SQLite's variable limit on every
/// version (999 before 3.32, 32 766 after), so the bound never has to be
/// rechecked against a bundled-version bump.
///
/// It also means a hydrated list costs one statement per 500 records rather
/// than one per record — the difference between a constant and a defect, not
/// between a constant and a smaller constant.
const IDS_PER_STATEMENT: usize = 500;

/// The marker `sql` must carry where its parent ids go.
const IDS: &str = "{ids}";

/// The children of many parents, in one statement per [`IDS_PER_STATEMENT`]
/// parents, grouped by the parent they belong to.
///
/// **This is the shape that replaced a query per record.** Listing a season used
/// to run one child query per register row per child table — 400 records became
/// 1 200 statements — and it returned exactly the right answer while doing it,
/// which is why no correctness test ever failed on it.
///
/// `sql` is the child query, written out at the call site with `{ids}` where its
/// parent ids belong. **The caller writes the whole query on purpose**: several
/// child lists join a lookup or order by something other than their id, and a
/// helper that assembled the SQL from a table name would have silently changed
/// the order rows print in. It also keeps every query greppable and runnable
/// through `EXPLAIN QUERY PLAN` without reassembling it first.
///
/// Order the query by the parent column first, then by whatever the register
/// prints in: rows are grouped as they arrive, so each parent's children keep
/// exactly the order the per-parent query gave them.
///
/// A parent with no children is absent from the map — callers take an empty
/// slice for it, which is the answer the point query gave. No parents at all
/// runs no statement, since `IN ()` is not valid SQL.
///
/// ```
/// use terrazgo_core::sql::children_by_parent;
///
/// let conn = rusqlite::Connection::open_in_memory().unwrap();
/// conn.execute_batch(
///     "CREATE TABLE leaf (id TEXT PRIMARY KEY, branch_id TEXT NOT NULL);
///      INSERT INTO leaf VALUES ('l1', 'b1'), ('l2', 'b1'), ('l3', 'b2');",
/// )
/// .unwrap();
///
/// let ids = vec!["b1".to_string(), "b2".to_string(), "b3".to_string()];
/// let grouped = children_by_parent(
///     &conn,
///     "SELECT * FROM leaf WHERE branch_id IN ({ids}) ORDER BY branch_id, id",
///     &ids,
///     |row| Ok((row.get::<_, String>("id")?, row.get::<_, String>("branch_id")?)),
///     |leaf| leaf.1.clone(),
/// )
/// .unwrap();
///
/// assert_eq!(grouped["b1"].len(), 2);
/// assert_eq!(grouped["b2"].len(), 1);
/// assert!(!grouped.contains_key("b3"), "a childless parent is absent");
/// ```
pub fn children_by_parent<T>(
    conn: &Connection,
    sql: &str,
    parent_ids: &[String],
    map: impl Fn(&Row) -> rusqlite::Result<T>,
    parent_of: impl Fn(&T) -> String,
) -> rusqlite::Result<HashMap<String, Vec<T>>> {
    debug_assert!(sql.contains(IDS), "child query has no {IDS} marker: {sql}");
    let mut grouped: HashMap<String, Vec<T>> = HashMap::new();
    for chunk in parent_ids.chunks(IDS_PER_STATEMENT) {
        let mut stmt = conn.prepare(&sql.replace(IDS, &placeholders(chunk.len())))?;
        let rows = stmt.query_map(params_from_iter(chunk), &map)?;
        for row in rows {
            let row = row?;
            grouped.entry(parent_of(&row)).or_default().push(row);
        }
    }
    Ok(grouped)
}

/// `?1, ?2, …` for `count` bound parameters. Numbered rather than bare `?` so a
/// malformed clause fails at prepare time instead of binding the wrong slot.
fn placeholders(count: usize) -> String {
    (1..=count)
        .map(|n| format!("?{n}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE leaf (id TEXT PRIMARY KEY, branch_id TEXT NOT NULL);")
            .unwrap();
        conn
    }

    fn insert(conn: &Connection, id: &str, branch: &str) {
        conn.execute("INSERT INTO leaf VALUES (?1, ?2)", [id, branch])
            .unwrap();
    }

    fn leaves(
        conn: &Connection,
        ids: &[String],
    ) -> rusqlite::Result<HashMap<String, Vec<(String, String)>>> {
        children_by_parent(
            conn,
            "SELECT * FROM leaf WHERE branch_id IN ({ids}) ORDER BY branch_id, id",
            ids,
            |row| {
                Ok((
                    row.get::<_, String>("id")?,
                    row.get::<_, String>("branch_id")?,
                ))
            },
            |leaf| leaf.1.clone(),
        )
    }

    #[test]
    fn placeholders_are_numbered_from_one() {
        assert_eq!(placeholders(1), "?1");
        assert_eq!(placeholders(3), "?1, ?2, ?3");
        assert_eq!(placeholders(0), "");
    }

    #[test]
    fn the_callers_own_ordering_survives_the_hoist() {
        // The reason the caller writes the query: several registers print their
        // children in an order that is not their id, and a helper that composed
        // the SQL itself would have changed what the page shows.
        let conn = tree();
        insert(&conn, "l1", "b1");
        insert(&conn, "l2", "b1");
        insert(&conn, "l3", "b1");

        let grouped = children_by_parent(
            &conn,
            "SELECT * FROM leaf WHERE branch_id IN ({ids}) ORDER BY branch_id, id DESC",
            &["b1".to_string()],
            |row| row.get::<_, String>("id"),
            |_| "b1".to_string(),
        )
        .unwrap();
        assert_eq!(grouped["b1"], ["l3", "l2", "l1"]);
    }

    #[test]
    fn children_keep_their_id_order_within_a_parent() {
        let conn = tree();
        insert(&conn, "l3", "b1");
        insert(&conn, "l1", "b1");
        insert(&conn, "l2", "b1");

        let grouped = leaves(&conn, &["b1".to_string()]).unwrap();
        let order: Vec<&str> = grouped["b1"].iter().map(|l| l.0.as_str()).collect();
        assert_eq!(order, ["l1", "l2", "l3"]);
    }

    #[test]
    fn no_parents_runs_no_statement_and_returns_nothing() {
        // The empty case matters: `IN ()` is a syntax error, so it must never
        // be built. A season with no records is an ordinary state.
        let conn = tree();
        assert!(leaves(&conn, &[]).unwrap().is_empty());
    }

    #[test]
    fn more_parents_than_fit_one_statement_are_chunked_without_loss() {
        let conn = tree();
        let ids: Vec<String> = (0..IDS_PER_STATEMENT + 7)
            .map(|n| format!("b{n:05}"))
            .collect();
        for (n, id) in ids.iter().enumerate() {
            insert(&conn, &format!("l{n:05}"), id);
        }

        let grouped = leaves(&conn, &ids).unwrap();
        assert_eq!(grouped.len(), ids.len(), "every parent found its child");
    }

    #[test]
    fn a_parent_not_asked_about_is_not_returned() {
        let conn = tree();
        insert(&conn, "l1", "b1");
        insert(&conn, "l2", "b2");

        let grouped = leaves(&conn, &["b1".to_string()]).unwrap();
        assert_eq!(grouped.len(), 1);
        assert!(!grouped.contains_key("b2"));
    }
}
