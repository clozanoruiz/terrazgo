// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Materialising a merged log: given the image a log row carries, make the
//! table hold it.
//!
//! This is the lower of the two layers in docs/sync.md → "Two layers, and only
//! one of them has policy". **It has no policy.** It does not decide which side
//! wins, what is concurrent, or what order to apply things in — it is handed a
//! row and it writes it. Everything that chooses lives above it.
//!
//! # One applier, not sixty
//!
//! [`Applier::upsert`] builds `INSERT … ON CONFLICT DO UPDATE` from the
//! payload's own JSON keys, checked against `PRAGMA table_info`, rather than a
//! hand-written applier per table. That is sound because **the payload key set
//! IS the column set by deliberate design**: `Machinery.kind` carries
//! `#[serde(rename = "type")]` precisely so the logged payload uses the real
//! column name, and the log's contract has always been that a receiving device
//! must be able to materialise a row from `after` alone. A table that breaks
//! that contract is refused here rather than half-applied.
//!
//! # Why it holds its own statements
//!
//! The SQL varies per table AND per column set, so it must never reach
//! SQLite's prepared-statement cache — that cache is one small LRU keyed by SQL
//! text, shared with the write path, and a single import would evict the
//! statements every write depends on (see [`crate::sql::cached_statement`], and
//! docs/sync.md → "Where the applier's statements live"). Instead each shape's
//! statement is prepared once and held here for as long as the applier lives,
//! which is faster than the cache would be — no per-row lookup — and bounded,
//! because it all dies when the import ends.

use std::collections::HashMap;

use rusqlite::types::{ToSqlOutput, Value as SqlValue};
use rusqlite::{Statement, Transaction};
use serde_json::Value;

use crate::error::{CoreError, Result};

/// A table as the database actually has it, read once per applier.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Shape {
    /// Every column, in `PRAGMA table_info` order.
    columns: Vec<String>,
    /// The primary key's columns, in key order — what `ON CONFLICT` targets and
    /// what `DO UPDATE` must not assign. Usually `id`; a regional extension
    /// keyed by its parent has that column instead.
    key: Vec<String>,
}

/// Writes rows into the materialised tables, one prepared statement per shape.
///
/// Created for a single apply and dropped with it. See the module docs for why
/// it keeps its own statements rather than using the connection's cache.
pub struct Applier<'tx> {
    tx: &'tx Transaction<'tx>,
    shapes: HashMap<String, Shape>,
    statements: HashMap<String, Statement<'tx>>,
}

impl<'tx> Applier<'tx> {
    /// An applier writing inside `tx`.
    ///
    /// The transaction is the caller's: an apply that cannot finish must leave
    /// nothing behind, and a bundle is refused whole.
    ///
    /// **Give it its own scope.** Its statements borrow the transaction, and
    /// `commit()` consumes one, so an applier still alive at the commit is a
    /// borrow error (E0505) rather than a runtime surprise:
    ///
    /// ```ignore
    /// let tx = conn.transaction()?;
    /// {
    ///     let mut applier = Applier::new(&tx);
    ///     applier.upsert("farm", &image)?;
    /// } // its statements are finalised here
    /// tx.commit()?;
    /// ```
    pub fn new(tx: &'tx Transaction<'tx>) -> Self {
        Self {
            tx,
            shapes: HashMap::new(),
            statements: HashMap::new(),
        }
    }

    /// How many distinct shapes this applier has prepared a statement for.
    ///
    /// Exists to be asserted on: that a thousand rows of one shape cost ONE
    /// statement is the property that keeps an import off the shared cache, and
    /// nothing else can observe it (SQLite's tracing reports statements as they
    /// RUN, so a reused statement and a re-prepared one look identical).
    pub fn prepared_shapes(&self) -> usize {
        self.statements.len()
    }

    /// Write `image` into `table`, inserting it or replacing the row that holds
    /// its key.
    ///
    /// `image` is a log row's `after` (or its `before`, when a change is being
    /// undone): a JSON object whose keys are the table's columns exactly. A key
    /// the table does not have, or a column the image does not carry, is a
    /// [`CoreError::ShapeViolation`] — a row materialised from a partial image
    /// would be wrong in a way nothing downstream could detect.
    pub fn upsert(&mut self, table: &str, image: &Value) -> Result<()> {
        let object = image.as_object().ok_or_else(|| {
            CoreError::ShapeViolation(format!("the image for {table} is not an object"))
        })?;
        let shape = self.shape_of(table)?.clone();

        for column in &shape.columns {
            if !object.contains_key(column) {
                return Err(CoreError::ShapeViolation(format!(
                    "{table}.{column} is missing from the image — a row cannot be \
                     materialised from part of itself"
                )));
            }
        }
        for key in object.keys() {
            if !shape.columns.contains(key) {
                return Err(CoreError::ShapeViolation(format!(
                    "the image for {table} carries {key}, which is not a column of it"
                )));
            }
        }

        let sql = upsert_sql(table, &shape);
        let values = shape
            .columns
            .iter()
            .map(|column| sql_value(table, column, &object[column]))
            .collect::<Result<Vec<_>>>()?;
        self.statement(sql)?
            .execute(rusqlite::params_from_iter(values))?;
        Ok(())
    }

    /// Remove the row `key` names from `table` — how an insert is undone.
    ///
    /// `key` is the value of the table's primary key, which is `id` for every
    /// register and the parent's id for an extension keyed by it. Removing a row
    /// that is not there is not an error: undoing an insert twice, or on a
    /// device that never had the row, must both leave the same state.
    pub fn delete(&mut self, table: &str, key: &str) -> Result<()> {
        let shape = self.shape_of(table)?.clone();
        let [column] = shape.key.as_slice() else {
            return Err(CoreError::ShapeViolation(format!(
                "{table} has a composite primary key, so one value does not name a row"
            )));
        };
        let sql = format!(
            "DELETE FROM {} WHERE {} = ?1",
            quote_ident(table),
            quote_ident(column)
        );
        self.statement(sql)?.execute([key])?;
        Ok(())
    }

    /// The prepared statement for `sql`, preparing it the first time only.
    ///
    /// `entry` rather than `get`-then-`insert` because the statement borrows the
    /// transaction: preparing inside the closure keeps that borrow inside the
    /// map's own lifetime, which two separate lookups would not.
    fn statement(&mut self, sql: String) -> Result<&mut Statement<'tx>> {
        Ok(match self.statements.entry(sql) {
            std::collections::hash_map::Entry::Occupied(held) => held.into_mut(),
            std::collections::hash_map::Entry::Vacant(slot) => {
                let prepared = self.tx.prepare(slot.key())?;
                slot.insert(prepared)
            }
        })
    }

    /// `table`'s real shape, read from the schema once per applier.
    fn shape_of(&mut self, table: &str) -> Result<&Shape> {
        if !self.shapes.contains_key(table) {
            let shape = read_shape(self.tx, table)?;
            self.shapes.insert(table.to_owned(), shape);
        }
        self.shapes
            .get(table)
            .ok_or_else(|| CoreError::ShapeViolation(format!("{table} has no shape")))
    }
}

/// A table's columns and primary key, from `PRAGMA table_info`.
///
/// An unknown table returns no rows, which is what refuses a name that came off
/// a peer's bundle before it can be spliced into any SQL.
fn read_shape(tx: &Transaction, table: &str) -> Result<Shape> {
    let mut stmt = tx.prepare(&format!("PRAGMA table_info({})", quote_ident(table)))?;
    let mut columns = Vec::new();
    // (key position, column) — `pk` is 1-based within the key, 0 when the
    // column is not part of it.
    let mut key: Vec<(i64, String)> = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get("name")?;
        let position: i64 = row.get("pk")?;
        if position > 0 {
            key.push((position, name.clone()));
        }
        columns.push(name);
    }
    if columns.is_empty() {
        return Err(CoreError::ShapeViolation(format!(
            "{table} is not a table in this database"
        )));
    }
    key.sort_by_key(|(position, _)| *position);
    Ok(Shape {
        columns,
        key: key.into_iter().map(|(_, name)| name).collect(),
    })
}

/// `INSERT … ON CONFLICT(key) DO UPDATE SET …` for one shape, binding every
/// column in `PRAGMA table_info` order.
///
/// A table whose every column is part of its key has nothing to assign, and
/// takes `DO NOTHING`: the row that is already there IS the row being applied.
fn upsert_sql(table: &str, shape: &Shape) -> String {
    let columns = shape
        .columns
        .iter()
        .map(|c| quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    let binds = (1..=shape.columns.len())
        .map(|n| format!("?{n}"))
        .collect::<Vec<_>>()
        .join(", ");
    let target = shape
        .key
        .iter()
        .map(|c| quote_ident(c))
        .collect::<Vec<_>>()
        .join(", ");
    let assignments = shape
        .columns
        .iter()
        .filter(|c| !shape.key.contains(c))
        .map(|c| format!("{0} = excluded.{0}", quote_ident(c)))
        .collect::<Vec<_>>()
        .join(", ");
    let resolution = if assignments.is_empty() {
        "DO NOTHING".to_string()
    } else {
        format!("DO UPDATE SET {assignments}")
    };
    format!(
        "INSERT INTO {} ({columns}) VALUES ({binds}) ON CONFLICT({target}) {resolution}",
        quote_ident(table)
    )
}

/// One JSON value as SQLite will store it.
///
/// A nested object or array is refused rather than serialised: no column in
/// this schema holds one, so an image carrying one is a payload that does not
/// describe a row, and guessing would write text into a column that expects a
/// value.
fn sql_value(table: &str, column: &str, value: &Value) -> Result<ToSqlOutput<'static>> {
    let stored = match value {
        Value::Null => SqlValue::Null,
        Value::Bool(flag) => SqlValue::Integer(i64::from(*flag)),
        Value::String(text) => SqlValue::Text(text.clone()),
        Value::Number(number) => match (number.as_i64(), number.as_f64()) {
            (Some(whole), _) => SqlValue::Integer(whole),
            (None, Some(real)) => SqlValue::Real(real),
            (None, None) => {
                return Err(CoreError::ShapeViolation(format!(
                    "{table}.{column} carries a number SQLite cannot store: {number}"
                )));
            }
        },
        Value::Array(_) | Value::Object(_) => {
            return Err(CoreError::ShapeViolation(format!(
                "{table}.{column} carries a nested value, so the image does not describe a row"
            )));
        }
    };
    Ok(ToSqlOutput::Owned(stored))
}

/// An identifier as SQL, quoted so that a name from a peer's bundle cannot end
/// the statement it is spliced into. Table names are checked against the schema
/// before they get here and column names against the table's own, so this is
/// the second lock rather than the first.
pub(crate) fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;

    /// Two shapes: a register keyed by `id`, and an extension keyed by its
    /// parent — the two key rules the applier has to get right.
    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE beast (
                 id     TEXT PRIMARY KEY,
                 name   TEXT NOT NULL,
                 legs   INTEGER,
                 weight REAL,
                 note   TEXT,
                 -- A flag, which serde writes as a JSON boolean and SQLite has
                 -- no type for: `plot_water_point.inside_plot` and every
                 -- `Option<bool>` on a fertilisation register arrive this way.
                 housed INTEGER NOT NULL DEFAULT 0
             );
             CREATE TABLE beast_es_extension (
                 beast_id TEXT PRIMARY KEY REFERENCES beast(id),
                 rega     TEXT
             );
             CREATE TABLE pairing (
                 left_id  TEXT NOT NULL,
                 right_id TEXT NOT NULL,
                 PRIMARY KEY (left_id, right_id)
             );",
        )
        .unwrap();
        conn
    }

    fn beast(id: &str, name: &str) -> Value {
        json!({
            "id": id, "name": name, "legs": 4, "weight": 1.5, "note": null,
            "housed": false
        })
    }

    fn one_beast(conn: &Connection, id: &str) -> (String, Option<i64>, Option<String>) {
        conn.query_row(
            "SELECT name, legs, note FROM beast WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap()
    }

    #[test]
    fn an_image_of_a_row_nobody_holds_inserts_it() {
        let mut conn = db();
        let tx = conn.transaction().unwrap();
        Applier::new(&tx)
            .upsert("beast", &beast("b1", "Lucera"))
            .unwrap();
        assert_eq!(one_beast(&tx, "b1").0, "Lucera");
    }

    #[test]
    fn an_image_over_an_existing_row_replaces_every_column() {
        let mut conn = db();
        let tx = conn.transaction().unwrap();
        let mut applier = Applier::new(&tx);
        applier.upsert("beast", &beast("b1", "Lucera")).unwrap();

        let changed = json!({
            "id": "b1", "name": "Morena", "legs": 3, "weight": 2.0, "note": "cojea",
            "housed": false
        });
        applier.upsert("beast", &changed).unwrap();

        assert_eq!(
            one_beast(&tx, "b1"),
            ("Morena".into(), Some(3), Some("cojea".into()))
        );
        let rows: i64 = tx
            .query_row("SELECT COUNT(*) FROM beast", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1, "replaced in place, not inserted beside itself");
    }

    #[test]
    fn a_null_in_the_image_becomes_a_null_in_the_row() {
        // Not "leave what was there": the image is the whole row, so a field
        // the farmer cleared has to clear the column.
        let mut conn = db();
        let tx = conn.transaction().unwrap();
        let mut applier = Applier::new(&tx);
        let mut image = beast("b1", "Lucera");
        image["note"] = json!("una nota");
        applier.upsert("beast", &image).unwrap();

        image["note"] = Value::Null;
        applier.upsert("beast", &image).unwrap();
        assert_eq!(one_beast(&tx, "b1").2, None);
    }

    #[test]
    fn a_boolean_becomes_the_integer_the_column_holds() {
        // A flag is a JSON boolean in the payload and an INTEGER in the
        // schema, so the applier is the one place that translates — and a
        // `bool` read back out of a column holding 'true' would fail at the
        // repository, on the receiving device, far from here.
        let mut conn = db();
        let tx = conn.transaction().unwrap();
        let mut applier = Applier::new(&tx);
        let mut image = beast("b1", "Lucera");
        image["housed"] = json!(true);
        applier.upsert("beast", &image).unwrap();

        let (flag, kind): (bool, String) = tx
            .query_row(
                "SELECT housed, typeof(housed) FROM beast WHERE id = 'b1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(flag);
        assert_eq!(kind, "integer", "never the text 'true'");

        image["housed"] = json!(false);
        applier.upsert("beast", &image).unwrap();
        let cleared: bool = tx
            .query_row("SELECT housed FROM beast WHERE id = 'b1'", [], |r| r.get(0))
            .unwrap();
        assert!(!cleared);
    }

    #[test]
    fn an_extension_is_keyed_by_its_parent_not_by_an_id() {
        let mut conn = db();
        let tx = conn.transaction().unwrap();
        let mut applier = Applier::new(&tx);
        applier.upsert("beast", &beast("b1", "Lucera")).unwrap();

        let image = json!({ "beast_id": "b1", "rega": "ES123" });
        applier.upsert("beast_es_extension", &image).unwrap();
        applier
            .upsert(
                "beast_es_extension",
                &json!({ "beast_id": "b1", "rega": "ES999" }),
            )
            .unwrap();

        let rega: String = tx
            .query_row("SELECT rega FROM beast_es_extension", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rega, "ES999");
    }

    #[test]
    fn a_table_that_is_all_key_keeps_the_row_it_has() {
        let mut conn = db();
        let tx = conn.transaction().unwrap();
        let mut applier = Applier::new(&tx);
        let image = json!({ "left_id": "a", "right_id": "b" });
        applier.upsert("pairing", &image).unwrap();
        applier.upsert("pairing", &image).unwrap();

        let rows: i64 = tx
            .query_row("SELECT COUNT(*) FROM pairing", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }

    // --- the payload↔column contract ---------------------------------------

    #[test]
    fn a_column_the_table_does_not_have_is_refused() {
        let mut conn = db();
        let tx = conn.transaction().unwrap();
        let mut image = beast("b1", "Lucera");
        image["favourite_colour"] = json!("azul");
        let refused = Applier::new(&tx).upsert("beast", &image);
        assert!(matches!(refused, Err(CoreError::ShapeViolation(_))));
    }

    #[test]
    fn an_image_missing_a_column_is_refused() {
        let mut conn = db();
        let tx = conn.transaction().unwrap();
        let mut image = beast("b1", "Lucera");
        image.as_object_mut().unwrap().remove("weight");
        let refused = Applier::new(&tx).upsert("beast", &image);
        assert!(matches!(refused, Err(CoreError::ShapeViolation(_))));
    }

    #[test]
    fn a_table_this_database_does_not_have_is_refused() {
        let mut conn = db();
        let tx = conn.transaction().unwrap();
        let refused = Applier::new(&tx).upsert("dragon", &json!({ "id": "d1" }));
        assert!(matches!(refused, Err(CoreError::ShapeViolation(_))));
    }

    #[test]
    fn an_image_that_is_not_an_object_is_refused() {
        let mut conn = db();
        let tx = conn.transaction().unwrap();
        let refused = Applier::new(&tx).upsert("beast", &json!("b1"));
        assert!(matches!(refused, Err(CoreError::ShapeViolation(_))));
    }

    #[test]
    fn a_nested_value_is_refused_rather_than_flattened() {
        let mut conn = db();
        let tx = conn.transaction().unwrap();
        let mut image = beast("b1", "Lucera");
        image["note"] = json!({ "text": "una nota" });
        let refused = Applier::new(&tx).upsert("beast", &image);
        assert!(matches!(refused, Err(CoreError::ShapeViolation(_))));
    }

    // --- deleting ----------------------------------------------------------

    #[test]
    fn deleting_removes_the_row_the_key_names() {
        let mut conn = db();
        let tx = conn.transaction().unwrap();
        let mut applier = Applier::new(&tx);
        applier.upsert("beast", &beast("b1", "Lucera")).unwrap();
        applier.upsert("beast", &beast("b2", "Morena")).unwrap();
        applier.delete("beast", "b1").unwrap();

        let left: Vec<String> = tx
            .prepare("SELECT id FROM beast ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(left, ["b2"]);
    }

    #[test]
    fn deleting_a_row_that_is_not_there_is_not_an_error() {
        // Undoing an insert twice, or on a device that never held the row, has
        // to reach the same state as doing it once.
        let mut conn = db();
        let tx = conn.transaction().unwrap();
        let mut applier = Applier::new(&tx);
        applier.upsert("beast", &beast("b1", "Lucera")).unwrap();
        applier.delete("beast", "b1").unwrap();
        applier.delete("beast", "b1").unwrap();
    }

    // --- statements --------------------------------------------------------

    #[test]
    fn many_rows_of_one_shape_cost_one_statement() {
        // The property that keeps an import off the connection's shared cache
        // (docs/sync.md → Where the applier's statements live).
        let mut conn = db();
        let tx = conn.transaction().unwrap();
        let mut applier = Applier::new(&tx);
        for n in 0..50 {
            applier
                .upsert("beast", &beast(&format!("b{n}"), "Lucera"))
                .unwrap();
        }
        assert_eq!(applier.prepared_shapes(), 1);

        applier
            .upsert(
                "beast_es_extension",
                &json!({ "beast_id": "b1", "rega": "ES1" }),
            )
            .unwrap();
        applier.delete("beast", "b0").unwrap();
        assert_eq!(applier.prepared_shapes(), 3, "a shape each, and one delete");
    }
}
