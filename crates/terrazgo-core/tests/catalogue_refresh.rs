// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The user-triggered catalogue refresh: what it adopts, what it refuses, and
//! — the property that actually matters — that a refusal leaves the stored
//! rows exactly as they were.
//!
//! Why that property and not the refusal message: `reconcile` NEVER deletes
//! (providers baja-date codes instead of removing them, so a code on a
//! years-old record must keep resolving). A bad file adopted once therefore
//! leaves bogus rows in every picker forever, and no later good file can take
//! them out. Validation has to happen before the write, and the only way to
//! prove it did is to check that nothing moved.
//!
//! Every input below is the REAL vendored `MAT_FERTI.csv` with one damage
//! applied to it, at the byte level so the file keeps its Windows-1252
//! encoding (it carries a 0x85 — "…" in cp1252, a control character in true
//! ISO-8859-1).
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use rusqlite::Connection;
use terrazgo_core::catalogue::{self, RefreshOutcome};

/// The catalogue under test: 24 rows, three text columns and the three
/// lifecycle dates — small enough to reason about whole, and a closed list the
/// decree enumerates, so a shrunken copy of it is unambiguously wrong.
const ID: &str = "MAT_FERTI";

fn vendored_bytes(catalogue_id: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("catalogues")
        .join(format!("{catalogue_id}.csv"));
    std::fs::read(&path).unwrap_or_else(|_| panic!("missing vendored file {path:?}"))
}

fn ensured_db() -> Connection {
    let mut conn = terrazgo_core::open_in_memory().unwrap();
    catalogue::ensure_catalogues(&mut conn).unwrap();
    conn
}

/// Everything about one catalogue that a refresh could possibly move: the rows
/// themselves and the header row's import bookkeeping. Compared as text
/// because the point is "not one thing changed", not any single field.
fn snapshot(conn: &Connection, catalogue_id: &str) -> String {
    let rows = catalogue::all_codes(conn, catalogue_id).unwrap();
    let header: (Option<String>, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT source_digest, source_updated_at, imported_at FROM catalogue WHERE id = ?1",
            [catalogue_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    format!("{header:?}\n{rows:?}")
}

fn replace_once(bytes: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    let at = bytes
        .windows(needle.len())
        .position(|w| w == needle)
        .unwrap_or_else(|| {
            panic!(
                "{} not found in the vendored file",
                String::from_utf8_lossy(needle)
            )
        });
    let mut out = bytes[..at].to_vec();
    out.extend_from_slice(replacement);
    out.extend_from_slice(&bytes[at + needle.len()..]);
    out
}

/// Apply `edit` to every non-empty line of the file — the shape of every
/// column-level damage (adding one, duplicating one).
fn per_line(bytes: &[u8], mut edit: impl FnMut(usize, &[u8]) -> Vec<u8>) -> Vec<u8> {
    let mut out = Vec::new();
    for (index, line) in bytes.split(|&b| b == b'\n').enumerate() {
        if !out.is_empty() {
            out.push(b'\n');
        }
        if line.is_empty() {
            continue;
        }
        out.extend_from_slice(&edit(index, line));
    }
    out
}

/// Refuse with this machine code, and leave everything untouched.
#[track_caller]
fn refuses(bytes: &[u8], reason: &str) {
    let mut conn = ensured_db();
    let before = snapshot(&conn, ID);
    let report = catalogue::refresh_catalogue(&mut conn, ID, bytes).unwrap();
    match report.outcome {
        RefreshOutcome::Refused { reason: got, .. } => assert_eq!(got, reason),
        other => panic!("expected a {reason} refusal, got {other:?}"),
    }
    assert_eq!(
        snapshot(&conn, ID),
        before,
        "a refused refresh must leave the stored catalogue byte-for-byte as it was — \
         reconcile cannot undo an adoption"
    );
}

#[test]
fn identical_bytes_are_unchanged_and_nothing_is_parsed() {
    let mut conn = ensured_db();
    let before = snapshot(&conn, ID);
    let report = catalogue::refresh_catalogue(&mut conn, ID, &vendored_bytes(ID)).unwrap();
    assert!(matches!(report.outcome, RefreshOutcome::Unchanged));
    // Not even `imported_at` moves: the digest matched, so there was nothing
    // to adopt and saying "updated just now" would be a false statement.
    assert_eq!(snapshot(&conn, ID), before);
}

#[test]
fn a_new_column_is_adopted_and_reported() {
    // The tolerance half of the two-tier rule, and the reason it is safe:
    // columns resolve by NAME, so a file that gains one still yields the same
    // codes and labels. Refusing this would leave users unable to update at
    // all until the next app release, over a column nothing reads.
    let bytes = per_line(&vendored_bytes(ID), |index, line| {
        let mut out: Vec<u8> = if index == 0 {
            b"\"Nuevo campo\";".to_vec()
        } else {
            b"\"x\";".to_vec()
        };
        out.extend_from_slice(line);
        out
    });

    let mut conn = ensured_db();
    let report = catalogue::refresh_catalogue(&mut conn, ID, &bytes).unwrap();
    match report.outcome {
        RefreshOutcome::Updated {
            added,
            corrected,
            extra_columns,
        } => {
            assert_eq!(extra_columns, vec!["Nuevo campo".to_string()]);
            assert_eq!(added, 0, "a shifted file adds no codes");
            // Every row gains the unread column in `attrs`, which is a real
            // (harmless) change to the stored row.
            assert_eq!(corrected, 24);
        }
        other => panic!("expected an update, got {other:?}"),
    }
    // The point of the whole mechanism: code 0 is still MAT_FERTI's own code
    // with its own label, not the contents of the column beside it.
    let row = catalogue::find_code(&conn, ID, "0").unwrap().remove(0);
    assert_eq!(row.label, "Otros");
}

#[test]
fn a_renamed_lifecycle_column_is_refused() {
    // The failure name-based resolution cannot prevent, and the one with the
    // quietest consequence: the parser matches `Fecha de baja` by name, so a
    // rename loses every retirement date and leaves retired codes in every
    // picker forever. FEGA's own variance is real — USOS_AGUA heads it
    // `Fecha Baja`.
    refuses(
        &replace_once(&vendored_bytes(ID), b"Fecha de baja", b"Fecha Baja"),
        "shape",
    );
}

#[test]
fn a_renamed_label_column_is_refused() {
    refuses(
        &replace_once(
            &vendored_bytes(ID),
            b"\"Tipo de material\"",
            b"\"Material\"",
        ),
        "shape",
    );
}

#[test]
fn a_duplicated_header_is_refused_rather_than_guessed() {
    // "The" column stops meaning anything, and taking the first would be a
    // guess about which one the codes are in.
    let bytes = per_line(&vendored_bytes(ID), |index, line| {
        let mut out = line.to_vec();
        // \xF3 is "ó" in Windows-1252 — the header has to be spelled the way
        // the file spells it, or the duplicate would not be a duplicate.
        out.extend_from_slice(if index == 0 {
            b";\"C\xF3digo SIEX\"".as_slice()
        } else {
            b";\"9\"".as_slice()
        });
        out
    });
    refuses(&bytes, "shape");
}

#[test]
fn a_truncated_download_is_refused() {
    // Codes are baja-dated, never removed, so a file with fewer rows than the
    // copy already stored did not arrive whole. Adopting it would be silent:
    // reconcile never deletes, so the app would look fine and simply have
    // stopped hearing about the rows that went missing.
    let mut bytes = Vec::new();
    for line in vendored_bytes(ID).split(|&b| b == b'\n').take(3) {
        bytes.extend_from_slice(line);
        bytes.push(b'\n');
    }
    refuses(&bytes, "shrunk");
}

#[test]
fn a_blank_label_is_refused() {
    // An empty label prints as an empty cell in a picker or a legal document
    // rather than failing loudly — the DETALLE_MATERIAL_FERT lesson.
    refuses(
        &replace_once(&vendored_bytes(ID), b"\"Otros\"", b"\"\""),
        "label",
    );
}

#[test]
fn control_characters_are_refused() {
    // The tripwire for the provider changing encoding under us: the symptom is
    // C0/C1 controls smuggled into labels and attributes.
    refuses(
        &replace_once(&vendored_bytes(ID), b"\"Otros\"", b"\"Ot\x07ros\""),
        "control_characters",
    );
}

#[test]
fn a_header_only_file_is_refused() {
    let bytes = vendored_bytes(ID);
    let header: Vec<u8> = bytes.split(|&b| b == b'\n').next().unwrap().to_vec();
    refuses(&header, "empty");
}

#[test]
fn a_genuine_update_adds_and_corrects() {
    // What a real provider refresh looks like: one label reworded, one code
    // added. Both are counted, and neither disturbs the other 23 rows.
    let mut bytes = replace_once(&vendored_bytes(ID), b"\"Otros\"", b"\"Otros materiales\"");
    bytes.extend_from_slice(b"\"99\";\"Material nuevo\";\"Ninguno\";01/07/2026;;\n");

    let mut conn = ensured_db();
    let report = catalogue::refresh_catalogue(&mut conn, ID, &bytes).unwrap();
    match report.outcome {
        RefreshOutcome::Updated {
            added,
            corrected,
            extra_columns,
        } => {
            assert_eq!((added, corrected), (1, 1));
            assert!(extra_columns.is_empty());
        }
        other => panic!("expected an update, got {other:?}"),
    }
    assert_eq!(
        catalogue::find_code(&conn, ID, "0").unwrap()[0].label,
        "Otros materiales"
    );
    let added = catalogue::find_code(&conn, ID, "99").unwrap().remove(0);
    assert_eq!(added.label, "Material nuevo");
    assert_eq!(added.added_on.as_deref(), Some("2026-07-01"));

    // Re-offering the same bytes now costs nothing at all.
    let again = catalogue::refresh_catalogue(&mut conn, ID, &bytes).unwrap();
    assert!(matches!(again.outcome, RefreshOutcome::Unchanged));
}

#[test]
fn a_refresh_never_removes_a_code_the_new_file_dropped() {
    // Stated as behaviour rather than left implicit in `reconcile`: the app
    // must keep resolving a code that appears on a years-old record even if
    // the provider stops publishing it. (The refresh only ever sees such a
    // file when it also has enough OTHER rows to clear the shrink check — here
    // the replacement row keeps the count.)
    let bytes = replace_once(
        &vendored_bytes(ID),
        b"\"0\";\"Otros\"",
        b"\"100\";\"Sustituto\"",
    );
    let mut conn = ensured_db();
    catalogue::refresh_catalogue(&mut conn, ID, &bytes).unwrap();
    assert_eq!(
        catalogue::find_code(&conn, ID, "0").unwrap()[0].label,
        "Otros"
    );
    assert_eq!(
        catalogue::find_code(&conn, ID, "100").unwrap()[0].label,
        "Sustituto"
    );
}

#[test]
fn an_unknown_catalogue_is_not_found() {
    // Nothing may be fetched that the app holds no spec for: the bytes would
    // have no column names to resolve against.
    let mut conn = ensured_db();
    assert!(matches!(
        catalogue::refresh_catalogue(&mut conn, "NO_SUCH_TABLE", b"x"),
        Err(terrazgo_core::CoreError::NotFound)
    ));
}

#[test]
fn status_reports_every_vendored_catalogue() {
    let conn = ensured_db();
    let status = catalogue::catalogue_status(&conn).unwrap();
    // Against `vendored_ids()`, never a literal: the count is not the property,
    // and `tests/catalogue.rs` already pins the SET by listing every id. A
    // magic number here would only mean one more place to bump when a
    // catalogue joins, which is how two of these tests broke on the last one.
    assert_eq!(status.len(), catalogue::vendored_ids().len());
    for row in &status {
        assert!(row.imported_at.is_some(), "{} was not imported", row.id);
        assert!(row.codes > 0, "{} stored no codes", row.id);
    }
    // Several vendored files carry no lifecycle dates at all, so a missing
    // provider stamp is normal and must not read as "never imported".
    let undated = status
        .iter()
        .find(|s| s.id == "USO_SIGPAC")
        .expect("USO_SIGPAC is vendored");
    assert!(undated.source_updated_at.is_none());
}

#[test]
fn status_reports_a_catalogue_that_was_never_imported() {
    // A fresh database before `ensure_catalogues` runs: the panel must be able
    // to say "never" rather than fail to render.
    let conn = terrazgo_core::open_in_memory().unwrap();
    let status = catalogue::catalogue_status(&conn).unwrap();
    assert_eq!(status.len(), catalogue::vendored_ids().len());
    assert!(
        status
            .iter()
            .all(|s| s.imported_at.is_none() && s.codes == 0)
    );
}

#[test]
fn a_refusal_does_not_repeat_the_catalogue_name_in_its_detail() {
    // The report line already names the file in bold; the parser prefixes its
    // own messages with the id, so without the strip the panel reads
    // "MAT_FERTI — MAT_FERTI: column … is missing".
    let mut conn = ensured_db();
    let bytes = replace_once(&vendored_bytes(ID), b"Fecha de baja", b"Fecha Baja");
    let report = catalogue::refresh_catalogue(&mut conn, ID, &bytes).unwrap();
    match report.outcome {
        RefreshOutcome::Refused { detail, .. } => {
            assert!(!detail.contains(ID), "{detail}");
            assert!(detail.contains("Fecha de baja"), "{detail}");
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// Every vendored file, fed back to the refresh path as its own bytes, must
/// report `Unchanged` and move nothing.
///
/// The other tests here reason about one representative catalogue; this one is
/// breadth, and it is the guard a NEW vendored file trips first. A wrong
/// `code_header`, `label_header` or `identity_attrs` still imports at startup —
/// `ensure_catalogues` and `refresh_catalogue` share the parser, so both would
/// agree on the same wrong answer — but a shape that cannot be re-derived, or
/// an identity that collapses rows, shows up here as a digest that does not
/// match or a snapshot that moved.
#[test]
fn every_vendored_file_refreshes_to_unchanged_against_itself() {
    let mut conn = ensured_db();
    for id in catalogue::vendored_ids() {
        let before = snapshot(&conn, id);
        let report = catalogue::refresh_catalogue(&mut conn, id, &vendored_bytes(id)).unwrap();
        assert!(
            matches!(report.outcome, RefreshOutcome::Unchanged),
            "{id} did not report Unchanged: {:?}",
            report.outcome
        );
        assert_eq!(snapshot(&conn, id), before, "{id} moved");
    }
}
