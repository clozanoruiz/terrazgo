// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Catalogue importer tests against the REAL vendored FEGA files
//! (crates/terrazgo-core/catalogues/, fetched per idTabla from
//! https://www11.fega.es/bdcsixwsp/catalogos/{id}). Every expected value below
//! is read off those files, not invented — see docs/maintenance.md §1 for the
//! snapshot's provenance and docs/siex-export.md → "Anexo VII catalogue study"
//! for the per-catalogue shapes.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::collections::HashSet;
use std::path::PathBuf;

use rusqlite::Connection;
use terrazgo_core::catalogue::{self, CatalogueCode};

/// Every vendored SIEX catalogue (idTabla ids). Kept in sync by hand with
/// `catalogue.rs`'s `VENDORED`; `imports_all_vendored_catalogues` fails if the
/// two drift, in either direction.
const VENDORED_IDS: [&str; 48] = [
    "AUTORIZACION_EXCP",
    "BUENAS_PRACTICAS_AMBITOS",
    "COMUNIDAD_AUTONOMA",
    "CULTIVO_USO_SIGPAC",
    "DESTINO_CULTIVO",
    "DEST_COSECHA",
    "DEST_RES_VEG",
    "DETALLE_MATERIAL_FERT",
    "EDIFICACIONES_INSTALACIONES",
    "EFICACIA_TRATAMIENTO",
    "ENFERMEDADES",
    "EST_FENOLOGICO",
    "JUSTIFICACION_ACTUACION",
    "MACRONUTRIENTES",
    "MALAS_HIERBAS",
    "MATERIAL_ANALIZADO",
    "MATERIAL_VEGETAL_REPRODUCCION",
    "MAT_FERTI",
    "MEDIDA_PREVENTIVA_CULTURAL",
    "METALES_PESADOS",
    "METODO_APLICACION_FERTILIZANTE",
    "MUNICIPIO_SIGPAC",
    "MICRONUTRIENTES",
    "ORIGEN_AGUA_RIEGO",
    "PAIS",
    "PLAGAS",
    "PROC_VEGETAL",
    "PRODUCTOS",
    "PROD_VEGETAL",
    "PROVINCIA",
    "REGIMEN_TENENCIA",
    "REGULADORES_CRECIMIENTO",
    "SIST_CULTIVO",
    "SIST_EXPLOTACION",
    "SIST_RIEGO",
    "SUST_ACTIVAS",
    "TIPENERGIA",
    "TIPO_ANALISIS",
    "TIPO_COBERTURA_SUELO",
    "TIPO_FERITILIZACION",
    "TIPO_LABOR",
    "TIPO_MAQUINA_UNE",
    "TIPO_MEDIDA_FITOSANITARIA",
    "TIPO_PRODFITO",
    "TIPO_TRATAMIENTO",
    "TRAT_ESTIERCOLES",
    "UNIDADES_MEDIDA",
    "USO_SIGPAC",
];

/// Data rows of one vendored file, read off disk (the importer reads the same
/// bytes through `include_bytes!`). Windows-1252 decode, matching the importer.
fn file_rows(catalogue_id: &str) -> Vec<Vec<String>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("catalogues")
        .join(format!("{catalogue_id}.csv"));
    let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("missing vendored file {path:?}"));
    let text: String = match std::str::from_utf8(&bytes) {
        Ok(text) => text.to_owned(),
        // The 0x80-0x9F range differs between cp1252 and Latin-1, but no code
        // or identity value in these files uses it — only labels do, and this
        // helper is used for counting, not for label comparison.
        Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
    };
    csv::ReaderBuilder::new()
        .delimiter(b';')
        .from_reader(text.as_bytes())
        .records()
        .map(|r| r.unwrap().iter().map(str::to_string).collect())
        .collect()
}

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
    assert_eq!(
        catalogues,
        VENDORED_IDS.len() as i64,
        "VENDORED_IDS and catalogue.rs's VENDORED have drifted"
    );
    // The snapshot holds 17384 stored rows across the 48 files (17385 data
    // rows less COMUNIDAD_AUTONOMA's code-less placeholder). Codes are only
    // ever added or baja-dated upstream, so a refreshed snapshot may grow
    // this number but must never shrink it.
    let codes: i64 = conn
        .query_row("SELECT COUNT(*) FROM catalogue_code", [], |r| r.get(0))
        .unwrap();
    assert!(codes >= 17384, "expected >= 17384 codes, got {codes}");
}

#[test]
fn every_file_row_is_imported_exactly_once() {
    // The guard that catches a WRONG `identity_attrs`, which is otherwise
    // silent: `reconcile` keys existing rows by (code, identity), so if a
    // catalogue repeats a code and we don't say which attribute qualifies it,
    // the HashMap keeps only the last row per code and re-UPDATEs the others
    // onto one id. Row count and MAX(id) both stay put, so the idempotence
    // test passes while labels are thrashed on every run. Comparing against
    // the files is the only thing that notices.
    let conn = ensured_db();
    for id in VENDORED_IDS {
        // COMUNIDAD_AUTONOMA's "Comunidad Desconocida" row carries no INE
        // code and is deliberately skipped by the importer.
        let expected = file_rows(id)
            .iter()
            .filter(|row| !(id == "COMUNIDAD_AUTONOMA" && row[1].trim().is_empty()))
            .count() as i64;
        assert_eq!(
            code_count(&conn, id),
            expected,
            "{id}: imported row count does not match the vendored file"
        );
    }
}

#[test]
fn every_imported_row_has_a_label() {
    // An empty label is a mis-set `label_col`, and it would print as a blank
    // cell in a picker or a report rather than failing loudly.
    // DETALLE_MATERIAL_FERT is why this exists: the provider's own
    // `descripcion` column is blank on its 83 "PERSONALIZADO" rows.
    let conn = ensured_db();
    for id in VENDORED_IDS {
        for row in catalogue::all_codes(&conn, id).unwrap() {
            assert!(
                !row.label.trim().is_empty(),
                "{id} code {} imported with an empty label",
                row.code
            );
        }
    }
}

#[test]
fn efficacy_codes_match_the_fega_file() {
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

/// Reglamento (UE) 2023/564 art. 1.3 requires crop names to follow EPPO codes
/// and puts the correspondence on the Member State. `PRODUCTOS` publishes an
/// EPPO column, so the code looks derivable from `crop.crop_code` — and this
/// pins how far that actually goes, because it is **not** every crop.
///
/// Measured 2026-08-12 against the vendored snapshot: **151 of the 1023 active
/// rows carry no EPPO code**, and the gap is structural rather than an
/// omission. EPPO codes a plant taxon, and a large part of this catalogue is
/// not one: `BARBECHO TRADICIONAL` and `BARBECHO MEDIOAMBIENTAL` are fallow,
/// `PASTOS PERMANENTES DE 5 O MÁS AÑOS` is a land use, `FLORES` is a generic
/// group, and `TRANQUILLÓN` is a wheat-rye mixture with two taxa and so no
/// single code. Those rows can never acquire one — which is consistent with the
/// EU annex heading its column "Crop or situation/**land use**".
///
/// So nothing may derive an EPPO code and present the result as complete: the
/// derivation must carry the gap rather than invent a code or drop the row.
///
/// The numbers are exact on purpose. A refresh that moves either one should
/// make somebody look at what FEGA changed, which is the same discipline as the
/// row-count guard — re-measure, then update these figures and their date.
#[test]
fn eppo_coverage_of_the_crop_catalogue_is_incomplete() {
    let conn = ensured_db();
    let active = terrazgo_core::catalogue::active_codes(&conn, "PRODUCTOS").unwrap();
    assert_eq!(active.len(), 1023, "active PRODUCTOS rows");

    let without_eppo = active
        .iter()
        .filter(|row| {
            row.attrs
                .as_ref()
                .and_then(|attrs| attrs.get("EPPO"))
                .and_then(|v| v.as_str())
                .is_none_or(str::is_empty)
        })
        .count();
    assert_eq!(
        without_eppo, 151,
        "active PRODUCTOS rows with no EPPO code — the EU annex's crop-name \
         correspondence is not derivable for these"
    );

    // Named examples, so a change of shape stays distinguishable from a change
    // of coverage. A mixture names two taxa; fallow names none.
    for (code, label) in [("12", "TRANQUILLÓN"), ("20", "BARBECHO TRADICIONAL")] {
        let row = one(&conn, "PRODUCTOS", code);
        assert_eq!(row.label, label);
        assert!(
            row.attrs
                .as_ref()
                .and_then(|attrs| attrs.get("EPPO"))
                .is_none(),
            "{label} has no EPPO code, and an empty provider cell is omitted \
             rather than stored as \"\""
        );
    }
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
    // MATERIAL_VEGETAL_REPRODUCCION repeats its tipo code once per detalle.
    let semilla = catalogue::find_code(&conn, "MATERIAL_VEGETAL_REPRODUCCION", "1").unwrap();
    assert_eq!(semilla.len(), 11);
    assert!(semilla.iter().all(|r| r.label == "Semilla"));
}

#[test]
fn plant_product_is_not_the_crop_catalogue() {
    // PROD_VEGETAL is the HARVESTED-PRODUCE catalogue that
    // `ComercializacionVD.ProductoVegetal` and
    // `TratamientosPostCosecha.ProductoVegetal` code against — a different
    // list from PRODUCTOS, which codes the crop. The file states the relation
    // itself: produce 1 "Aceitunas" comes from crops 101 OLIVO and 363
    // ACEBUCHE, so the produce code repeats once per crop.
    let conn = ensured_db();
    let aceitunas = catalogue::find_code(&conn, "PROD_VEGETAL", "1").unwrap();
    assert_eq!(aceitunas.len(), 2);
    assert!(aceitunas.iter().all(|r| r.label == "Aceitunas"));
    let mut crops: Vec<&str> = aceitunas
        .iter()
        .map(|r| r.attrs.as_ref().unwrap()["Cultivo SIEX"].as_str().unwrap())
        .collect();
    crops.sort_unstable();
    assert_eq!(crops, ["ACEBUCHE", "OLIVO"]);
    // The crop catalogue answers a different question for the same word.
    assert_eq!(one(&conn, "PRODUCTOS", "101").label, "OLIVO");
    // 208 distinct produce codes behind 692 rows — a picker must dedupe.
    let distinct: HashSet<String> = catalogue::active_codes(&conn, "PROD_VEGETAL")
        .unwrap()
        .into_iter()
        .map(|r| r.code)
        .collect();
    assert_eq!(distinct.len(), 208);
}

#[test]
fn comunidad_autonoma_is_keyed_by_its_ine_code() {
    // The file leads with the CATASTRO code, but SIEX `CAExplotacion` wants
    // INE ("según codificacion INE"), and the two disagree for 10 of the 17
    // communities. Keying on column 0 would resolve INE 07 to Castilla-La
    // Mancha — a wrong region, silently, on a regulatory export.
    let conn = ensured_db();
    let cyl = one(&conn, "COMUNIDAD_AUTONOMA", "07");
    assert_eq!(cyl.label, "Comunidad Autónoma de Castilla y León");
    assert_eq!(cyl.attrs.unwrap()["Código catastro"], "08");
    // 18 rows in the file, but the code-less "Comunidad Desconocida"
    // placeholder is not a community and is not stored.
    assert_eq!(code_count(&conn, "COMUNIDAD_AUTONOMA"), 17);
}

#[test]
fn the_analysis_and_seed_catalogues_carry_what_slice_8_believed_missing() {
    // Seams 2-4 recorded these as "no catalogue in the vendored FEGA set".
    // They exist; the claim was about our snapshot, not about FEGA.
    let conn = ensured_db();
    let materials: Vec<(String, String)> = catalogue::active_codes(&conn, "MATERIAL_ANALIZADO")
        .unwrap()
        .into_iter()
        .map(|r| (r.code, r.label))
        .collect();
    // Four values, not the printed model's three: FEGA separates the standing
    // crop from the harvested produce.
    assert_eq!(
        materials,
        [
            ("1".into(), "Cultivo".to_string()),
            ("2".into(), "Producto cosechado".into()),
            ("3".into(), "Suelo".into()),
            ("4".into(), "Agua de riego".into()),
        ]
    );
    assert_eq!(
        one(&conn, "TIPO_ANALISIS", "5").label,
        "Parámetros del Suelo"
    );
    // `UsoSemillaTratada.Tratamiento` — note the codes start at 2.
    let seed: Vec<String> = catalogue::active_codes(&conn, "TIPO_TRATAMIENTO")
        .unwrap()
        .into_iter()
        .map(|r| r.code)
        .collect();
    assert_eq!(seed, ["2", "3", "4", "5"]);
    // `Analitica.TiposSustancias[]`, and the CAS number that makes it the
    // cross-country key a future non-Spanish export would match on.
    let acefato = one(&conn, "SUST_ACTIVAS", "1");
    assert_eq!(acefato.label, "ACEFATO");
    let attrs = acefato.attrs.unwrap();
    assert_eq!(attrs["Número CAS"], "30560-19-1");
    assert_eq!(attrs["Código Europeo"], "1049");
}

#[test]
fn buildings_are_keyed_by_their_siex_code_not_their_tipologia() {
    // EDIFICACIONES_INSTALACIONES leads with the tipología (9 values, each
    // repeating); the row's own code is `Código SIEX` in column 2.
    let conn = ensured_db();
    let row = one(&conn, "EDIFICACIONES_INSTALACIONES", "1");
    assert_eq!(row.label, "Abrevadero y abastecimiento de agua");
    assert_eq!(
        row.attrs.unwrap()["Tipología"],
        "Naves y obras de edificación de entidad constructiva"
    );
    assert_eq!(code_count(&conn, "EDIFICACIONES_INSTALACIONES"), 109);
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
    // Drift: a tampered label, and a stale digest so the fast-path skip does
    // not mask the reconcile (as it would on a real snapshot refresh).
    conn.execute(
        "UPDATE catalogue_code SET label = 'Tampered' WHERE catalogue_id = 'EFICACIA_TRATAMIENTO' AND code = '1'",
        [],
    )
    .unwrap();
    conn.execute(
        "UPDATE catalogue SET source_digest = 'stale' WHERE id = 'EFICACIA_TRATAMIENTO'",
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
fn skips_catalogues_whose_bytes_have_not_changed() {
    // Fast path: the stored digest matches the vendored file, so nothing is
    // parsed or written — imported_at proves it. This must hold for EVERY
    // catalogue, including the ones with no lifecycle dates: under the old
    // date-based fast path those reconciled on every single startup.
    let mut conn = terrazgo_core::open_in_memory().unwrap();
    catalogue::ensure_catalogues(&mut conn).unwrap();
    conn.execute("UPDATE catalogue SET imported_at = 'sentinel'", [])
        .unwrap();
    catalogue::ensure_catalogues(&mut conn).unwrap();
    let reimported: Vec<String> = conn
        .prepare("SELECT id FROM catalogue WHERE imported_at <> 'sentinel' ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    assert!(
        reimported.is_empty(),
        "up-to-date catalogues were reimported: {reimported:?}"
    );
}

#[test]
fn a_changed_snapshot_is_detected_even_with_no_lifecycle_dates() {
    // The reason the fast path hashes bytes instead of comparing dates. Two
    // real refresh shapes the date comparison could not see: a provider that
    // corrects a label without touching any date, and a catalogue that ships
    // no dates at all (USO_SIGPAC, PROVINCIA, TIPO_MAQUINA_UNE, …).
    let mut conn = terrazgo_core::open_in_memory().unwrap();
    catalogue::ensure_catalogues(&mut conn).unwrap();
    for id in ["EFICACIA_TRATAMIENTO", "USO_SIGPAC"] {
        let stored: Option<String> = conn
            .query_row(
                "SELECT source_digest FROM catalogue WHERE id = ?1",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(stored.is_some(), "{id} stored no digest");
        conn.execute(
            "UPDATE catalogue_code SET label = 'Tampered' WHERE catalogue_id = ?1",
            [id],
        )
        .unwrap();
        conn.execute(
            "UPDATE catalogue SET source_digest = 'stale' WHERE id = ?1",
            [id],
        )
        .unwrap();
    }
    catalogue::ensure_catalogues(&mut conn).unwrap();
    assert_eq!(one(&conn, "EFICACIA_TRATAMIENTO", "1").label, "Buena");
    assert_eq!(one(&conn, "USO_SIGPAC", "TA").label, "TIERRAS ARABLES");
}
