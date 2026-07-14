// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Catalogue importer tests against the REAL vendored FEGA files
//! (crates/terrazgo-core/catalogues/, snapshot fetched 2026-07-14 from
//! https://www11.fega.es/bdcsixwsp/catalogos/zip/). Every expected value below
//! is read off those files, not invented — see docs/siex-export.md → "Anexo VII
//! catalogue study" for the per-catalogue shapes.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use rusqlite::Connection;
use terrazgo_core::catalogue::{self, CatalogueCode};

/// The 16 vendored SIEX catalogues (idTabla ids), one per TratamFito-relevant
/// coded field plus the crop↔SIGPAC-uso relation for the declared-crops prefill.
const VENDORED_IDS: [&str; 16] = [
    "AUTORIZACION_EXCP",
    "BUENAS_PRACTICAS_AMBITOS",
    "CULTIVO_USO_SIGPAC",
    "EFICACIA_TRATAMIENTO",
    "ENFERMEDADES",
    "EST_FENOLOGICO",
    "JUSTIFICACION_ACTUACION",
    "MALAS_HIERBAS",
    "PLAGAS",
    "PRODUCTOS",
    "REGULADORES_CRECIMIENTO",
    "TIPENERGIA",
    "TIPO_MAQUINA_UNE",
    "TIPO_MEDIDA_FITOSANITARIA",
    "TIPO_PRODFITO",
    "UNIDADES_MEDIDA",
];

fn ensured_db() -> Connection {
    let mut conn = terrazgo_core::open_in_memory().unwrap();
    catalogue::ensure_catalogues(&mut conn).unwrap();
    conn
}

fn one(conn: &Connection, catalogue_id: &str, code: &str) -> CatalogueCode {
    let mut found = catalogue::find_code(conn, catalogue_id, code).unwrap();
    assert_eq!(
        found.len(),
        1,
        "expected exactly one {catalogue_id} row for code {code}"
    );
    found.remove(0)
}

fn code_count(conn: &Connection, catalogue_id: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM catalogue_code WHERE catalogue_id = ?1",
        [catalogue_id],
        |r| r.get(0),
    )
    .unwrap()
}

#[test]
fn imports_all_vendored_catalogues() {
    let conn = ensured_db();
    for id in VENDORED_IDS {
        let source: String = conn
            .query_row("SELECT source FROM catalogue WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap_or_else(|_| panic!("catalogue {id} was not imported"));
        assert_eq!(source, "siex");
        assert!(code_count(&conn, id) > 0, "{id} imported no codes");
    }
    let catalogues: i64 = conn
        .query_row("SELECT COUNT(*) FROM catalogue", [], |r| r.get(0))
        .unwrap();
    assert_eq!(catalogues, 16);
    // The 2026-07-14 snapshot holds 5999 rows across the 16 files. Codes are
    // only ever added or baja-dated upstream, so a refreshed snapshot may grow
    // this number but must never shrink it.
    let codes: i64 = conn
        .query_row("SELECT COUNT(*) FROM catalogue_code", [], |r| r.get(0))
        .unwrap();
    assert!(codes >= 5999, "expected >= 5999 codes, got {codes}");
}

#[test]
fn eficacia_codes_match_the_fega_file() {
    // EFICACIA_TRATAMIENTO is the smallest catalogue: 1 Buena / 2 Regular /
    // 3 Mala, all active — pinned in full against the vendored file.
    let conn = ensured_db();
    let codes = catalogue::active_codes(&conn, "EFICACIA_TRATAMIENTO").unwrap();
    let pairs: Vec<(&str, &str)> = codes
        .iter()
        .map(|c| (c.code.as_str(), c.label.as_str()))
        .collect();
    assert_eq!(pairs, [("1", "Buena"), ("2", "Regular"), ("3", "Mala")]);
}

#[test]
fn legacy_encoded_labels_decode_to_utf8() {
    // FEGA documents the CSVs as ISO-8859-1, but the real files are
    // Windows-1252: accented labels must arrive as real UTF-8, and the €
    // signs in UNIDADES_MEDIDA (0x80 — a control char in true ISO-8859-1,
    // '€' only in cp1252) must survive as €.
    let conn = ensured_db();
    assert_eq!(one(&conn, "TIPENERGIA", "1").label, "ELÉCTRICA");
    assert_eq!(
        one(&conn, "ENFERMEDADES", "1").label,
        "Enfermedades fúngicas"
    );
    assert_eq!(one(&conn, "PLAGAS", "1").label, "Artrópodos");
    assert_eq!(one(&conn, "UNIDADES_MEDIDA", "45").label, "€/ha");
    assert_eq!(one(&conn, "UNIDADES_MEDIDA", "53").label, "€");
}

#[test]
fn no_imported_text_carries_control_characters() {
    // Encoding-drift tripwire: if a future snapshot changes encoding in a way
    // the UTF-8-first fallback mishandles (e.g. some third legacy code page),
    // the symptom is C0/C1 control characters smuggled into labels or attrs.
    // Catch it at the next snapshot refresh instead of importing garbage.
    // Newlines/tabs are legitimate inside quoted notes columns; anything
    // else in the control ranges is an encoding accident.
    fn clean(text: &str) -> bool {
        !text
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    }
    let conn = ensured_db();
    for id in VENDORED_IDS {
        for row in catalogue::all_codes(&conn, id).unwrap() {
            assert!(
                clean(&row.label),
                "control character in {id} code {}: {:?}",
                row.code,
                row.label
            );
            if let Some(attrs) = &row.attrs {
                for (key, value) in attrs.as_object().unwrap() {
                    let value = value.as_str().unwrap();
                    assert!(
                        clean(value),
                        "control character in {id} code {} attr {key}: {value:?}",
                        row.code
                    );
                }
            }
        }
    }
}

#[test]
fn problem_catalogues_use_category_as_label_and_keep_attrs() {
    // ENFERMEDADES row 7 in the vendored file: código SIEX 7, hierarchical
    // nº 8.5.1, categoría "Albugo spp.", EPPO 1ALBUG, empty observaciones.
    // The human-facing name is the categoría column; the rest rides in attrs.
    let conn = ensured_db();
    let row = one(&conn, "ENFERMEDADES", "7");
    assert_eq!(row.label, "Albugo spp.");
    let attrs = row.attrs.expect("hierarchical catalogues carry attrs");
    assert_eq!(attrs["Código"], "8.5.1");
    assert_eq!(attrs["Nombre científico"], "Albugo spp.");
    assert_eq!(attrs["EPPO cd"], "1ALBUG");
    // Empty provider cells are omitted, not stored as "".
    assert!(attrs.get("Observaciones").is_none());
}

#[test]
fn crop_catalogue_keeps_attribute_columns() {
    // PRODUCTOS code 1 = TRIGO BLANDO (Triticum aestivum, EPPO TRZAX); the
    // ~25 SI/NO classification columns stay verbatim in attrs for the future
    // prefill/validation queries.
    let conn = ensured_db();
    let wheat = one(&conn, "PRODUCTOS", "1");
    assert_eq!(wheat.label, "TRIGO BLANDO");
    let attrs = wheat.attrs.unwrap();
    assert_eq!(attrs["Latín"], "Triticum aestivum");
    assert_eq!(attrs["EPPO"], "TRZAX");
    assert_eq!(attrs["Cereales"], "SI");
    assert_eq!(attrs["Frutal"], "NO");
}

#[test]
fn lifecycle_dates_are_stored_iso() {
    // ENFERMEDADES code 1: alta and modificación 03/07/2024 in the file,
    // stored as ISO YYYY-MM-DD per the schema conventions; no baja.
    let conn = ensured_db();
    let row = one(&conn, "ENFERMEDADES", "1");
    assert_eq!(row.added_on.as_deref(), Some("2024-07-03"));
    assert_eq!(row.modified_on.as_deref(), Some("2024-07-03"));
    assert_eq!(row.retired_on, None);
}

#[test]
fn retired_codes_stay_resolvable_but_leave_the_picker() {
    // AUTORIZACION_EXCP code 1 is baja-dated 11/11/2025 in the vendored file:
    // a real retired code. Old records must still resolve it; pickers must not
    // offer it.
    let conn = ensured_db();
    let row = one(&conn, "AUTORIZACION_EXCP", "1");
    assert_eq!(row.retired_on.as_deref(), Some("2025-11-11"));
    let active = catalogue::active_codes(&conn, "AUTORIZACION_EXCP").unwrap();
    assert!(!active.iter().any(|c| c.code == "1"));
    assert!(!active.is_empty());
}

#[test]
fn composite_identity_catalogues_keep_every_row_per_code() {
    let conn = ensured_db();
    // BUENAS_PRACTICAS_AMBITOS repeats code 0 ("No realiza buenas prácticas")
    // once per ámbito — Fertilización / Riego / Fitosanitario in the snapshot.
    let rows = catalogue::find_code(&conn, "BUENAS_PRACTICAS_AMBITOS", "0").unwrap();
    assert_eq!(rows.len(), 3);
    let mut ambitos: Vec<String> = rows
        .iter()
        .map(|r| r.attrs.as_ref().unwrap()["Ámbito"].as_str().unwrap().into())
        .collect();
    ambitos.sort();
    assert_eq!(ambitos, ["Fertilización", "Fitosanitario", "Riego"]);
    // CULTIVO_USO_SIGPAC relates one crop code to several SIGPAC usos.
    let wheat_usos = catalogue::find_code(&conn, "CULTIVO_USO_SIGPAC", "1").unwrap();
    assert_eq!(wheat_usos.len(), 4);
    assert!(wheat_usos.iter().all(|r| r.label == "TRIGO BLANDO"));
}

#[test]
fn machinery_catalogue_has_string_codes_and_no_lifecycle() {
    // TIPO_MAQUINA_UNE is the odd one out: string codes, no date columns.
    let conn = ensured_db();
    let row = one(&conn, "TIPO_MAQUINA_UNE", "0000000_88");
    assert_eq!(row.label, "Máquinas sin clasificar");
    assert_eq!(row.added_on, None);
    assert_eq!(row.retired_on, None);
    let updated: Option<String> = conn
        .query_row(
            "SELECT source_updated_at FROM catalogue WHERE id = 'TIPO_MAQUINA_UNE'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(updated, None);
}

#[test]
fn ensure_is_idempotent() {
    let mut conn = terrazgo_core::open_in_memory().unwrap();
    catalogue::ensure_catalogues(&mut conn).unwrap();
    let count_first: i64 = conn
        .query_row("SELECT COUNT(*) FROM catalogue_code", [], |r| r.get(0))
        .unwrap();
    let max_id_first: i64 = conn
        .query_row("SELECT MAX(id) FROM catalogue_code", [], |r| r.get(0))
        .unwrap();
    catalogue::ensure_catalogues(&mut conn).unwrap();
    let count_second: i64 = conn
        .query_row("SELECT COUNT(*) FROM catalogue_code", [], |r| r.get(0))
        .unwrap();
    let max_id_second: i64 = conn
        .query_row("SELECT MAX(id) FROM catalogue_code", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        count_first, count_second,
        "re-running ensure duplicated rows"
    );
    assert_eq!(
        max_id_first, max_id_second,
        "re-running ensure re-inserted rows"
    );
}

#[test]
fn upsert_never_deletes_and_repairs_drift() {
    // THE storage invariant (docs/siex-export.md): imports only ever upsert.
    // A row the snapshot no longer carries must survive; a drifted label must
    // be repaired in place, keeping its row id.
    let mut conn = terrazgo_core::open_in_memory().unwrap();
    catalogue::ensure_catalogues(&mut conn).unwrap();
    let original_id = one(&conn, "EFICACIA_TRATAMIENTO", "1").id;
    // A code the vendored file does not contain (as if a stage-2 refresh had
    // imported a newer snapshot carrying it).
    conn.execute(
        "INSERT INTO catalogue_code (catalogue_id, code, label) VALUES ('EFICACIA_TRATAMIENTO', '999', 'Not in the snapshot')",
        [],
    )
    .unwrap();
    // Drift: a tampered label, and a stale catalogue stamp so the fast-path
    // skip does not mask the reconcile.
    conn.execute(
        "UPDATE catalogue_code SET label = 'Tampered' WHERE catalogue_id = 'EFICACIA_TRATAMIENTO' AND code = '1'",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE catalogue SET source_updated_at = '2000-01-01' WHERE id = 'EFICACIA_TRATAMIENTO'",
        [],
    )
    .unwrap();

    catalogue::ensure_catalogues(&mut conn).unwrap();

    let repaired = one(&conn, "EFICACIA_TRATAMIENTO", "1");
    assert_eq!(repaired.label, "Buena", "drifted label was not repaired");
    assert_eq!(
        repaired.id, original_id,
        "repair must update in place, not re-insert"
    );
    let survivor = one(&conn, "EFICACIA_TRATAMIENTO", "999");
    assert_eq!(survivor.label, "Not in the snapshot");
}

#[test]
fn skips_catalogues_already_at_the_vendored_snapshot() {
    // Fast path: when the stored source_updated_at is at least the vendored
    // snapshot's, the catalogue is not touched — imported_at proves it.
    let mut conn = terrazgo_core::open_in_memory().unwrap();
    catalogue::ensure_catalogues(&mut conn).unwrap();
    conn.execute(
        "UPDATE catalogue SET imported_at = 'sentinel' WHERE id = 'EFICACIA_TRATAMIENTO'",
        [],
    )
    .unwrap();
    catalogue::ensure_catalogues(&mut conn).unwrap();
    let imported_at: String = conn
        .query_row(
            "SELECT imported_at FROM catalogue WHERE id = 'EFICACIA_TRATAMIENTO'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        imported_at, "sentinel",
        "an up-to-date catalogue was reimported"
    );
}
