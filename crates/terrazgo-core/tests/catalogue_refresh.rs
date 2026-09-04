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

mod common;

use common::db_with_catalogues;

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

/// Everything about one catalogue that a refresh could possibly move: the rows
/// themselves and the header row's import bookkeeping. Compared as text
/// because the point is "not one thing changed", not any single field.
fn snapshot(conn: &Connection, catalogue_id: &str) -> String {
    let rows = catalogue::all_codes(conn, catalogue_id).unwrap();
    let header: (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = conn
        .query_row(
            "SELECT source_digest, source_updated_at, imported_at, imported_by_version
             FROM catalogue WHERE id = ?1",
            [catalogue_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    format!("{header:?}\n{rows:?}")
}

/// The app version stamped on a catalogue, or `None` when only a fetched copy
/// has ever been adopted.
fn version_stamp(conn: &Connection, catalogue_id: &str) -> Option<String> {
    conn.query_row(
        "SELECT imported_by_version FROM catalogue WHERE id = ?1",
        [catalogue_id],
        |r| r.get(0),
    )
    .unwrap()
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
    let mut conn = db_with_catalogues();
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
    let mut conn = db_with_catalogues();
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

    let mut conn = db_with_catalogues();
    let report = catalogue::refresh_catalogue(&mut conn, ID, &bytes).unwrap();
    match report.outcome {
        RefreshOutcome::Updated {
            added,
            corrected,
            withdrawn,
            extra_columns,
        } => {
            assert_eq!(extra_columns, vec!["Nuevo campo".to_string()]);
            assert_eq!(added, 0, "a shifted file adds no codes");
            // Every row gains the unread column in `attrs`, which is a real
            // (harmless) change to the stored row.
            assert_eq!(corrected, 24);
            assert_eq!(withdrawn, 0, "every code is still in the file");
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

    let mut conn = db_with_catalogues();
    let report = catalogue::refresh_catalogue(&mut conn, ID, &bytes).unwrap();
    match report.outcome {
        RefreshOutcome::Updated {
            added,
            corrected,
            withdrawn,
            extra_columns,
        } => {
            assert_eq!((added, corrected), (1, 1));
            assert_eq!(withdrawn, 0, "the file still carries every stored code");
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
fn a_refresh_survives_the_next_startup() {
    // The whole point of the button. `ensure_catalogues` runs at every launch
    // and re-imports the binary's snapshot; it must not treat a copy the user
    // fetched as its own work to redo, or an adoption would last exactly one
    // session — corrections silently reverting while added codes stayed.
    let mut bytes = replace_once(&vendored_bytes(ID), b"\"Otros\"", b"\"Otros materiales\"");
    bytes.extend_from_slice(b"\"99\";\"Material nuevo\";\"Ninguno\";01/07/2026;;\n");

    let mut conn = db_with_catalogues();
    catalogue::refresh_catalogue(&mut conn, ID, &bytes).unwrap();
    let adopted = snapshot(&conn, ID);

    catalogue::ensure_catalogues(&mut conn).unwrap();

    assert_eq!(
        snapshot(&conn, ID),
        adopted,
        "a startup after a refresh must leave the adopted copy alone"
    );
    assert_eq!(
        catalogue::find_code(&conn, ID, "0").unwrap()[0].label,
        "Otros materiales",
        "the correction reverted at the next launch"
    );
    assert_eq!(catalogue::find_code(&conn, ID, "99").unwrap().len(), 1);
}

#[test]
fn a_refresh_does_not_claim_to_be_this_versions_own_snapshot() {
    // The mechanism behind the test above, stated on its own because it is the
    // one field that decides it: a fetched copy leaves the version stamp
    // exactly as it found it. (Here the startup import has already run, so the
    // stamp is this version's and must survive unchanged.)
    let mut conn = db_with_catalogues();
    let before = version_stamp(&conn, ID);
    assert!(before.is_some(), "the startup import stamped no version");

    let bytes = replace_once(&vendored_bytes(ID), b"\"Otros\"", b"\"Otros materiales\"");
    catalogue::refresh_catalogue(&mut conn, ID, &bytes).unwrap();

    assert_eq!(version_stamp(&conn, ID), before);
}

#[test]
fn a_new_app_version_restores_the_curated_snapshot() {
    // The other half of the bargain: an adoption survives restarts but not an
    // app update, because the vendored files are curated as a SET and a device
    // must not run one refreshed file mixed with an older release's rest.
    //
    // What an update restores is every label, attribute and date — NOT row
    // membership. A code the refresh added stays, because the import cannot
    // delete without breaking the promise that a code already written onto a
    // record keeps resolving. The residue is asserted, not wished away.
    let mut bytes = replace_once(&vendored_bytes(ID), b"\"Otros\"", b"\"Otros materiales\"");
    bytes.extend_from_slice(b"\"99\";\"Material nuevo\";\"Ninguno\";01/07/2026;;\n");

    let mut conn = db_with_catalogues();
    catalogue::refresh_catalogue(&mut conn, ID, &bytes).unwrap();
    conn.execute(
        "UPDATE catalogue SET imported_by_version = 'older' WHERE id = ?1",
        [ID],
    )
    .unwrap();

    catalogue::ensure_catalogues(&mut conn).unwrap();

    assert_eq!(
        catalogue::find_code(&conn, ID, "0").unwrap()[0].label,
        "Otros",
        "the curated label was not restored"
    );
    assert_eq!(
        catalogue::find_code(&conn, ID, "99").unwrap().len(),
        1,
        "a code the refresh added must survive the update"
    );
}

#[test]
fn a_refresh_before_any_startup_import_leaves_the_curated_copy_to_come() {
    // Core does not get to assume the shell's call order. A refresh against a
    // catalogue never imported inserts the row itself, and must leave the
    // version stamp NULL — which is what tells the next startup that this
    // version's own reviewed snapshot has still never been applied here.
    let mut conn = terrazgo_core::open_in_memory().unwrap();
    let report = catalogue::refresh_catalogue(&mut conn, ID, &vendored_bytes(ID)).unwrap();
    assert!(matches!(report.outcome, RefreshOutcome::Updated { .. }));
    assert_eq!(version_stamp(&conn, ID), None);

    catalogue::ensure_catalogues(&mut conn).unwrap();
    assert!(version_stamp(&conn, ID).is_some());
}

#[test]
fn a_corrected_label_is_adopted_even_when_no_lifecycle_date_moved() {
    // Why the refresh's skip compares BYTES and not the file's newest date:
    // the provider correcting a label without touching any date is a real
    // refresh shape, and several catalogues (USO_SIGPAC, PROVINCIA,
    // TIPO_MAQUINA_UNE, …) ship no dates to compare in the first place.
    for id in [ID, "USO_SIGPAC"] {
        let mut conn = db_with_catalogues();
        let original = catalogue::all_codes(&conn, id).unwrap().remove(0);
        let needle = format!("\"{}\"", original.label).into_bytes();
        let bytes = replace_once(&vendored_bytes(id), &needle, b"\"Reworded\"");

        let report = catalogue::refresh_catalogue(&mut conn, id, &bytes).unwrap();

        match report.outcome {
            RefreshOutcome::Updated { corrected, .. } => assert_eq!(corrected, 1, "{id}"),
            other => panic!("{id}: expected an update, got {other:?}"),
        }
    }
}

/// A file where code 0 has been replaced by a code the app has never seen —
/// the only shape in which a dropped row reaches the importer at all, since
/// the replacement keeps the row count past the shrink check.
fn file_without_code_zero() -> Vec<u8> {
    replace_once(
        &vendored_bytes(ID),
        b"\"0\";\"Otros\"",
        b"\"100\";\"Sustituto\"",
    )
}

fn absent_since(conn: &Connection, catalogue_id: &str, code: &str) -> Option<String> {
    conn.query_row(
        "SELECT absent_since FROM catalogue_code WHERE catalogue_id = ?1 AND code = ?2",
        [catalogue_id, code],
        |r| r.get(0),
    )
    .unwrap()
}

fn is_offered(conn: &Connection, catalogue_id: &str, code: &str) -> bool {
    catalogue::active_codes(conn, catalogue_id)
        .unwrap()
        .iter()
        .any(|row| row.code == code)
}

#[test]
fn a_refresh_never_removes_a_code_the_new_file_dropped() {
    // Stated as behaviour rather than left implicit in `reconcile`: the app
    // must keep resolving a code that appears on a years-old record even if
    // the provider stops publishing it.
    let mut conn = db_with_catalogues();
    catalogue::refresh_catalogue(&mut conn, ID, &file_without_code_zero()).unwrap();
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
fn a_code_the_provider_stopped_shipping_leaves_the_picker() {
    // Keeping the row is right; going on OFFERING it is not. A provider
    // retires a code with a baja date, so a row that simply vanishes is
    // unexplained — the app records that it can no longer see it and stops
    // offering it, while a record that already cites it still resolves.
    let mut conn = db_with_catalogues();
    assert!(is_offered(&conn, ID, "0"));

    let report = catalogue::refresh_catalogue(&mut conn, ID, &file_without_code_zero()).unwrap();

    match report.outcome {
        RefreshOutcome::Updated { withdrawn, .. } => assert_eq!(withdrawn, 1),
        other => panic!("expected an update, got {other:?}"),
    }
    assert!(
        !is_offered(&conn, ID, "0"),
        "a vanished code is still offered"
    );
    assert!(absent_since(&conn, ID, "0").is_some());
    assert_eq!(
        catalogue::find_code(&conn, ID, "0").unwrap()[0].label,
        "Otros",
        "the row must still resolve for records that cite it"
    );
}

#[test]
fn a_code_that_comes_back_is_offered_again() {
    // The drop was a provider glitch. Presence in the current file is presence,
    // so the mark clears and the code returns to the picker.
    //
    // The restoring file keeps the replacement row as well: by now the store
    // holds both, and the shrink check counts every stored row, marked absent
    // or not — so a file carrying only the original 24 would be refused as a
    // truncated download rather than adopted.
    let mut conn = db_with_catalogues();
    catalogue::refresh_catalogue(&mut conn, ID, &file_without_code_zero()).unwrap();
    assert!(!is_offered(&conn, ID, "0"));

    let mut bytes = file_without_code_zero();
    bytes.extend_from_slice(b"\"0\";\"Otros\";\"Ninguno\";;;\n");
    catalogue::refresh_catalogue(&mut conn, ID, &bytes).unwrap();

    assert!(
        is_offered(&conn, ID, "0"),
        "a restored code is still hidden"
    );
    assert_eq!(absent_since(&conn, ID, "0"), None);
}

#[test]
fn a_code_that_comes_back_baja_dated_stays_out_of_the_picker() {
    // The provider fixing its own mistake properly: the row returns, carrying
    // the baja date that should have been there all along. Our mark clears —
    // we can see it again — and theirs takes over, so the code stays out of
    // the picker for a reason the authority actually stated. The two columns
    // are independent precisely so this transition needs no special case.
    let mut conn = db_with_catalogues();
    catalogue::refresh_catalogue(&mut conn, ID, &file_without_code_zero()).unwrap();

    let mut bytes = file_without_code_zero();
    bytes.extend_from_slice(b"\"0\";\"Otros\";\"Ninguno\";;;30/06/2026\n");
    catalogue::refresh_catalogue(&mut conn, ID, &bytes).unwrap();

    assert_eq!(absent_since(&conn, ID, "0"), None, "our mark should clear");
    let row = catalogue::find_code(&conn, ID, "0").unwrap().remove(0);
    assert_eq!(row.retired_on.as_deref(), Some("2026-06-30"));
    assert!(!is_offered(&conn, ID, "0"));
}

#[test]
fn a_still_missing_code_keeps_the_date_it_first_went_missing() {
    // `absent_since` records when the row disappeared, so a later refresh that
    // still lacks it must not re-stamp — and must not report a change to a row
    // that did not change.
    let mut conn = db_with_catalogues();
    catalogue::refresh_catalogue(&mut conn, ID, &file_without_code_zero()).unwrap();
    let first = absent_since(&conn, ID, "0");
    conn.execute(
        "UPDATE catalogue_code SET absent_since = '2020-01-01' WHERE catalogue_id = ?1 AND code = '0'",
        [ID],
    )
    .unwrap();

    // A different file, still without code 0, so the digest check cannot skip.
    let mut bytes = file_without_code_zero();
    bytes.extend_from_slice(b"\"101\";\"Otro sustituto\";\"Ninguno\";;;\n");
    let report = catalogue::refresh_catalogue(&mut conn, ID, &bytes).unwrap();

    assert!(first.is_some());
    assert_eq!(
        absent_since(&conn, ID, "0").as_deref(),
        Some("2020-01-01"),
        "the first-seen-missing date was overwritten"
    );
    match report.outcome {
        RefreshOutcome::Updated { withdrawn, .. } => {
            assert_eq!(
                withdrawn, 0,
                "an already-marked code is not withdrawn twice"
            );
        }
        other => panic!("expected an update, got {other:?}"),
    }
}

#[test]
fn a_vendored_import_never_marks_a_refreshed_in_code_absent() {
    // The one that stops this mechanism eating the other. A vendored file is
    // NOT the provider's current list — a code can be missing from it merely
    // by being newer than the release — so startup may not infer absence. If
    // it did, every code a refresh had added would vanish from the pickers at
    // the next app update.
    let mut bytes = vendored_bytes(ID);
    bytes.extend_from_slice(b"\"99\";\"Material nuevo\";\"Ninguno\";01/07/2026;;\n");
    let mut conn = db_with_catalogues();
    catalogue::refresh_catalogue(&mut conn, ID, &bytes).unwrap();
    assert!(is_offered(&conn, ID, "99"));

    conn.execute(
        "UPDATE catalogue SET imported_by_version = 'older' WHERE id = ?1",
        [ID],
    )
    .unwrap();
    catalogue::ensure_catalogues(&mut conn).unwrap();

    assert_eq!(absent_since(&conn, ID, "99"), None);
    assert!(
        is_offered(&conn, ID, "99"),
        "an app update hid a code the user had refreshed in"
    );
}

#[test]
fn an_unknown_catalogue_is_not_found() {
    // Nothing may be fetched that the app holds no spec for: the bytes would
    // have no column names to resolve against.
    let mut conn = db_with_catalogues();
    assert!(matches!(
        catalogue::refresh_catalogue(&mut conn, "NO_SUCH_TABLE", b"x"),
        Err(terrazgo_core::CoreError::NotFound)
    ));
}

#[test]
fn status_reports_every_vendored_catalogue() {
    let conn = db_with_catalogues();
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
    let mut conn = db_with_catalogues();
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
    let mut conn = db_with_catalogues();
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
