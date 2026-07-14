// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Imported reference catalogues (`catalogue` / `catalogue_code`).
//!
//! Regulatory exports must speak the provider's coded vocabulary — for Spain,
//! the FEGA SIEX "Anexo VII" catalogues (efficacy, justification, crop,
//! phytosanitary problem codes, …). This module vendors a snapshot of the 16
//! treatment-relevant catalogue CSVs in the binary (offline-first: the app
//! must resolve codes from first run, no network) and imports them with
//! [`ensure_catalogues`] at startup.
//!
//! Design (docs/siex-export.md → "Storage design"):
//!   * Generic tables, provider columns verbatim in `attrs` JSON — promote a
//!     catalogue to a typed table only when a real query needs its attributes.
//!   * **Upsert only, never delete.** Providers retire codes by baja date
//!     instead of removing them; a code on an old record must keep resolving.
//!   * Not in `record_change`: each device imports its own copy.
//!   * A vendored snapshot refresh rides an app release; an in-app refresh
//!     over the network is a possible later addition, same parser and upsert.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::date::now_utc_iso;
use crate::error::{CoreError, Result};

/// `catalogue.source` tag for the FEGA SIEX catalogues.
pub const SOURCE_SIEX: &str = "siex";

/// One vendored provider CSV, embedded at compile time.
///
/// The FEGA files share one shape: column 0 is the code, one column is the
/// human-facing label, optional trailing lifecycle-date columns, and whatever
/// sits in between is catalogue-specific attributes.
struct Vendored {
    /// Provider table id (the SIEX idTabla) — also the `catalogue.id`.
    id: &'static str,
    /// Raw CSV bytes, verbatim from the provider (Windows-1252 today; the
    /// decoder also accepts UTF-8 — see [`decode_provider_text`]).
    csv: &'static [u8],
    /// 0-based index of the label column. Usually 1; the hierarchical problem
    /// catalogues and EST_FENOLOGICO put a classification number there and
    /// the real name in column 2.
    label_col: usize,
    /// Attribute headers that qualify the code for upsert identity, for
    /// catalogues that legitimately repeat a code (one row per ámbito, one
    /// row per SIGPAC uso). Empty for everything else: the code alone is the
    /// identity.
    identity_attrs: &'static [&'static str],
}

/// The vendored SIEX snapshot (fetched 2026-07-14 from
/// `https://www11.fega.es/bdcsixwsp/catalogos/zip/`): every catalogue the
/// `TratamFito` export block codes against, plus CULTIVO_USO_SIGPAC for the
/// declared-crops prefill. Refreshing = replacing the files and re-releasing;
/// the importer reconciles by lifecycle dates.
const VENDORED: [Vendored; 16] = [
    Vendored {
        id: "AUTORIZACION_EXCP",
        csv: include_bytes!("../catalogues/AUTORIZACION_EXCP.csv"),
        label_col: 1,
        identity_attrs: &[],
    },
    Vendored {
        id: "BUENAS_PRACTICAS_AMBITOS",
        csv: include_bytes!("../catalogues/BUENAS_PRACTICAS_AMBITOS.csv"),
        label_col: 1,
        identity_attrs: &["Ámbito"],
    },
    Vendored {
        id: "CULTIVO_USO_SIGPAC",
        csv: include_bytes!("../catalogues/CULTIVO_USO_SIGPAC.csv"),
        label_col: 1,
        identity_attrs: &["Uso SIGPAC"],
    },
    Vendored {
        id: "EFICACIA_TRATAMIENTO",
        csv: include_bytes!("../catalogues/EFICACIA_TRATAMIENTO.csv"),
        label_col: 1,
        identity_attrs: &[],
    },
    Vendored {
        id: "ENFERMEDADES",
        csv: include_bytes!("../catalogues/ENFERMEDADES.csv"),
        label_col: 2,
        identity_attrs: &[],
    },
    Vendored {
        id: "EST_FENOLOGICO",
        csv: include_bytes!("../catalogues/EST_FENOLOGICO.csv"),
        label_col: 2,
        identity_attrs: &[],
    },
    Vendored {
        id: "JUSTIFICACION_ACTUACION",
        csv: include_bytes!("../catalogues/JUSTIFICACION_ACTUACION.csv"),
        label_col: 1,
        identity_attrs: &[],
    },
    Vendored {
        id: "MALAS_HIERBAS",
        csv: include_bytes!("../catalogues/MALAS_HIERBAS.csv"),
        label_col: 2,
        identity_attrs: &[],
    },
    Vendored {
        id: "PLAGAS",
        csv: include_bytes!("../catalogues/PLAGAS.csv"),
        label_col: 2,
        identity_attrs: &[],
    },
    Vendored {
        id: "PRODUCTOS",
        csv: include_bytes!("../catalogues/PRODUCTOS.csv"),
        label_col: 1,
        identity_attrs: &[],
    },
    Vendored {
        id: "REGULADORES_CRECIMIENTO",
        csv: include_bytes!("../catalogues/REGULADORES_CRECIMIENTO.csv"),
        label_col: 2,
        identity_attrs: &[],
    },
    Vendored {
        id: "TIPENERGIA",
        csv: include_bytes!("../catalogues/TIPENERGIA.csv"),
        label_col: 1,
        identity_attrs: &[],
    },
    Vendored {
        id: "TIPO_MAQUINA_UNE",
        csv: include_bytes!("../catalogues/TIPO_MAQUINA_UNE.csv"),
        label_col: 1,
        identity_attrs: &[],
    },
    Vendored {
        id: "TIPO_MEDIDA_FITOSANITARIA",
        csv: include_bytes!("../catalogues/TIPO_MEDIDA_FITOSANITARIA.csv"),
        label_col: 1,
        identity_attrs: &[],
    },
    Vendored {
        id: "TIPO_PRODFITO",
        csv: include_bytes!("../catalogues/TIPO_PRODFITO.csv"),
        label_col: 1,
        identity_attrs: &[],
    },
    Vendored {
        id: "UNIDADES_MEDIDA",
        csv: include_bytes!("../catalogues/UNIDADES_MEDIDA.csv"),
        label_col: 1,
        identity_attrs: &[],
    },
];

/// One catalogue code as stored, for pickers and code→label resolution.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogueCode {
    pub id: i64,
    pub catalogue_id: String,
    pub code: String,
    pub label: String,
    /// The provider's remaining columns, keys verbatim (e.g. `"EPPO cd"`).
    pub attrs: Option<Value>,
    pub added_on: Option<String>,
    pub modified_on: Option<String>,
    pub retired_on: Option<String>,
}

/// Import every vendored catalogue that is missing or older than the vendored
/// snapshot. Idempotent and upsert-only — over-calling is sanctioned (it runs
/// at every startup), and rows never disappear, whatever the snapshot says.
pub fn ensure_catalogues(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    for vendored in &VENDORED {
        let parsed = parse_vendored(vendored)?;
        let stored_update: Option<Option<String>> = tx
            .query_row(
                "SELECT source_updated_at FROM catalogue WHERE id = ?1",
                [vendored.id],
                |r| r.get(0),
            )
            .optional()?;
        // Fast path: already imported at (or past) the vendored snapshot's
        // newest lifecycle date. ISO dates compare correctly as strings.
        // Catalogues without lifecycle dates reconcile every run — cheap, and
        // there is nothing else to compare against.
        if let (Some(Some(stored)), Some(newest)) = (&stored_update, &parsed.newest_date)
            && stored >= newest
        {
            continue;
        }
        reconcile(&tx, vendored, &parsed, stored_update.is_some())?;
    }
    tx.commit()?;
    Ok(())
}

/// The non-retired codes of one catalogue, in file order (providers publish
/// their lists in a deliberate order) — what a UI picker offers.
pub fn active_codes(conn: &Connection, catalogue_id: &str) -> Result<Vec<CatalogueCode>> {
    codes_where(
        conn,
        "catalogue_id = ?1 AND retired_on IS NULL",
        params![catalogue_id],
    )
}

/// Every row of one catalogue regardless of lifecycle state, in file order.
pub fn all_codes(conn: &Connection, catalogue_id: &str) -> Result<Vec<CatalogueCode>> {
    codes_where(conn, "catalogue_id = ?1", params![catalogue_id])
}

/// Every row carrying `code` in a catalogue, retired or not — display
/// resolution for stored records, which may reference codes retired since.
/// More than one row only in the composite-identity catalogues (one row per
/// qualifying attribute value).
pub fn find_code(conn: &Connection, catalogue_id: &str, code: &str) -> Result<Vec<CatalogueCode>> {
    codes_where(
        conn,
        "catalogue_id = ?1 AND code = ?2",
        params![catalogue_id, code],
    )
}

fn codes_where(
    conn: &Connection,
    filter: &str,
    params: &[&dyn rusqlite::ToSql],
) -> Result<Vec<CatalogueCode>> {
    let sql = format!(
        "SELECT id, catalogue_id, code, label, attrs, added_on, modified_on, retired_on
         FROM catalogue_code WHERE {filter} ORDER BY id"
    );
    let mut stmt = conn.prepare(&sql)?;
    // attrs comes out as raw TEXT here; the JSON parse needs its own error
    // channel (serde, not rusqlite), so it happens in a second pass below.
    let raw = stmt
        .query_map(params, |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, Option<String>>(7)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    raw.into_iter()
        .map(
            |(id, catalogue_id, code, label, attrs, added_on, modified_on, retired_on)| {
                let attrs = attrs.as_deref().map(serde_json::from_str).transpose()?;
                Ok(CatalogueCode {
                    id,
                    catalogue_id,
                    code,
                    label,
                    attrs,
                    added_on,
                    modified_on,
                    retired_on,
                })
            },
        )
        .collect()
}

/// One CSV data row, normalised: dates ISO, empty cells dropped from attrs.
struct ParsedCode {
    code: String,
    label: String,
    attrs: Option<Value>,
    added_on: Option<String>,
    modified_on: Option<String>,
    retired_on: Option<String>,
    /// Values of the catalogue's `identity_attrs`, in spec order.
    identity: Vec<String>,
}

struct ParsedCatalogue {
    rows: Vec<ParsedCode>,
    /// Newest lifecycle date across all rows — the snapshot's version stamp.
    newest_date: Option<String>,
}

fn parse_vendored(vendored: &Vendored) -> Result<ParsedCatalogue> {
    let bad = |detail: String| CoreError::Catalogue(format!("{}: {detail}", vendored.id));
    let text = decode_provider_text(vendored.csv);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .from_reader(text.as_bytes());
    let headers = reader.headers().map_err(|e| bad(e.to_string()))?.clone();
    let identity_cols = vendored
        .identity_attrs
        .iter()
        .map(|name| {
            headers
                .iter()
                .position(|h| h == *name)
                .ok_or_else(|| bad(format!("identity column '{name}' missing")))
        })
        .collect::<Result<Vec<usize>>>()?;

    let mut rows = Vec::new();
    let mut newest_date: Option<String> = None;
    for record in reader.records() {
        let record = record.map_err(|e| bad(e.to_string()))?;
        let field = |i: usize| {
            record
                .get(i)
                .ok_or_else(|| bad(format!("missing column {i}")))
        };
        let mut attrs = Map::new();
        let mut added_on = None;
        let mut modified_on = None;
        let mut retired_on = None;
        for (i, header) in headers.iter().enumerate() {
            let value = field(i)?;
            match header {
                "Fecha de alta" => added_on = iso_date(vendored.id, value)?,
                "Fecha de modificación" => modified_on = iso_date(vendored.id, value)?,
                "Fecha de baja" => retired_on = iso_date(vendored.id, value)?,
                _ if i == 0 || i == vendored.label_col => {}
                _ if value.is_empty() => {}
                _ => {
                    attrs.insert(header.to_string(), Value::String(value.to_string()));
                }
            }
        }
        for date in [&added_on, &modified_on, &retired_on].into_iter().flatten() {
            if newest_date.as_deref().is_none_or(|n| date.as_str() > n) {
                newest_date = Some(date.clone());
            }
        }
        rows.push(ParsedCode {
            code: field(0)?.to_string(),
            label: field(vendored.label_col)?.to_string(),
            attrs: (!attrs.is_empty()).then_some(Value::Object(attrs)),
            added_on,
            modified_on,
            retired_on,
            identity: identity_cols
                .iter()
                .map(|&i| field(i).map(str::to_string))
                .collect::<Result<_>>()?,
        });
    }
    Ok(ParsedCatalogue { rows, newest_date })
}

/// Upsert one catalogue: update drifted rows in place (keeping their ids —
/// they may be referenced by the time typed promotions exist), insert new
/// ones, and NEVER delete — rows absent from the snapshot stay untouched.
fn reconcile(
    tx: &Transaction<'_>,
    vendored: &Vendored,
    parsed: &ParsedCatalogue,
    already_imported: bool,
) -> Result<()> {
    let now = now_utc_iso();
    if already_imported {
        tx.execute(
            "UPDATE catalogue SET source = ?2, source_updated_at = ?3, imported_at = ?4 WHERE id = ?1",
            params![vendored.id, SOURCE_SIEX, parsed.newest_date, now],
        )?;
    } else {
        tx.execute(
            "INSERT INTO catalogue (id, source, source_updated_at, imported_at) VALUES (?1, ?2, ?3, ?4)",
            params![vendored.id, SOURCE_SIEX, parsed.newest_date, now],
        )?;
    }

    // Existing rows keyed by identity — the code plus, for the catalogues
    // that repeat codes, the qualifying attribute values.
    struct DbRow {
        id: i64,
        label: String,
        attrs: Option<Value>,
        added_on: Option<String>,
        modified_on: Option<String>,
        retired_on: Option<String>,
    }
    let mut existing: HashMap<(String, Vec<String>), DbRow> = HashMap::new();
    {
        let mut stmt = tx.prepare(
            "SELECT id, code, label, attrs, added_on, modified_on, retired_on
             FROM catalogue_code WHERE catalogue_id = ?1",
        )?;
        let raw = stmt
            .query_map([vendored.id], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Option<String>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (id, code, label, attrs, added_on, modified_on, retired_on) in raw {
            let attrs: Option<Value> = attrs.as_deref().map(serde_json::from_str).transpose()?;
            let identity = vendored
                .identity_attrs
                .iter()
                .map(|name| {
                    attrs
                        .as_ref()
                        .and_then(|a| a.get(*name))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                })
                .collect();
            existing.insert(
                (code, identity),
                DbRow {
                    id,
                    label,
                    attrs,
                    added_on,
                    modified_on,
                    retired_on,
                },
            );
        }
    }

    let mut insert = tx.prepare(
        "INSERT INTO catalogue_code (catalogue_id, code, label, attrs, added_on, modified_on, retired_on)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    let mut update = tx.prepare(
        "UPDATE catalogue_code SET label = ?2, attrs = ?3, added_on = ?4, modified_on = ?5, retired_on = ?6
         WHERE id = ?1",
    )?;
    for row in &parsed.rows {
        let attrs_text = row.attrs.as_ref().map(serde_json::to_string).transpose()?;
        match existing.get(&(row.code.clone(), row.identity.clone())) {
            Some(db)
                if db.label == row.label
                    && db.attrs == row.attrs
                    && db.added_on == row.added_on
                    && db.modified_on == row.modified_on
                    && db.retired_on == row.retired_on => {}
            Some(db) => {
                update.execute(params![
                    db.id,
                    row.label,
                    attrs_text,
                    row.added_on,
                    row.modified_on,
                    row.retired_on
                ])?;
            }
            None => {
                insert.execute(params![
                    vendored.id,
                    row.code,
                    row.label,
                    attrs_text,
                    row.added_on,
                    row.modified_on,
                    row.retired_on
                ])?;
            }
        }
    }
    Ok(())
}

/// Decode a provider CSV to UTF-8, without an encoding crate.
///
/// FEGA documents the files as ISO-8859-1, but the real ones are
/// Windows-1252 — UNIDADES_MEDIDA carries € (byte 0x80), which is an
/// invisible control character in true ISO-8859-1. And a future snapshot
/// could quietly switch to UTF-8, which a legacy 1:1 decode would turn into
/// silent mojibake. So, in order:
///
/// 1. Bytes that parse as UTF-8 are taken as UTF-8 (BOM stripped). This can
///    never misread the legacy files: accented Spanish text in Latin-1 or
///    cp1252 is not accidentally valid UTF-8, because every lone byte
///    ≥ 0x80 is an invalid UTF-8 sequence.
/// 2. Everything else decodes as Windows-1252 — identical to the 1:1
///    Latin-1 map except the 0x80–0x9F range, where cp1252 places
///    printable characters (€, quotes, dashes…).
///
/// If some third encoding ever appears, the imported-text control-character
/// tripwire test fails at the snapshot refresh rather than importing garbage.
fn decode_provider_text(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) => bytes.iter().map(|&b| cp1252_char(b)).collect(),
    }
}

/// One byte of Windows-1252. Only 0x80–0x9F differs from the 1:1 Latin-1
/// map; the table is the WHATWG windows-1252 index (what browsers use for
/// content labelled latin-1), with its five unassigned slots falling
/// through to their C1 code points.
fn cp1252_char(byte: u8) -> char {
    match byte {
        0x80 => '€',
        0x82 => '‚',
        0x83 => 'ƒ',
        0x84 => '„',
        0x85 => '…',
        0x86 => '†',
        0x87 => '‡',
        0x88 => 'ˆ',
        0x89 => '‰',
        0x8A => 'Š',
        0x8B => '‹',
        0x8C => 'Œ',
        0x8E => 'Ž',
        0x91 => '‘',
        0x92 => '’',
        0x93 => '“',
        0x94 => '”',
        0x95 => '•',
        0x96 => '–',
        0x97 => '—',
        0x98 => '˜',
        0x99 => '™',
        0x9A => 'š',
        0x9B => '›',
        0x9C => 'œ',
        0x9E => 'ž',
        0x9F => 'Ÿ',
        _ => char::from(byte),
    }
}

/// A provider `DD/MM/YYYY` cell → ISO `YYYY-MM-DD`; empty cells mean "no
/// date" (e.g. never retired), not an error.
fn iso_date(catalogue_id: &str, field: &str) -> Result<Option<String>> {
    if field.is_empty() {
        return Ok(None);
    }
    let date = jiff::civil::Date::strptime("%d/%m/%Y", field).map_err(|_| {
        CoreError::Catalogue(format!("{catalogue_id}: bad lifecycle date '{field}'"))
    })?;
    Ok(Some(date.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_bytes_decode_as_cp1252() {
        // "Más" in Latin-1/cp1252: 0xE1 is á (identical in both).
        assert_eq!(decode_provider_text(&[b'M', 0xE1, b's']), "Más");
        // 0x80 is € in cp1252 — a control character in true ISO-8859-1; the
        // real UNIDADES_MEDIDA file carries it ("€/ha").
        assert_eq!(decode_provider_text(&[0x80, b'/', b'h', b'a']), "€/ha");
    }

    #[test]
    fn utf8_input_is_taken_as_utf8() {
        // Fallback for a future FEGA encoding switch: already-valid UTF-8
        // must pass through unchanged instead of being double-decoded into
        // mojibake ("fúngicas" → "fÃºngicas").
        assert_eq!(decode_provider_text("fúngicas".as_bytes()), "fúngicas");
        assert_eq!(decode_provider_text("€/ha".as_bytes()), "€/ha");
        // A UTF-8 BOM is stripped, not smuggled into the first header name.
        assert_eq!(decode_provider_text(b"\xEF\xBB\xBFcode"), "code");
        // Pure ASCII is identical under every candidate encoding.
        assert_eq!(decode_provider_text(b"TRIGO BLANDO"), "TRIGO BLANDO");
    }

    #[test]
    fn provider_dates_convert_to_iso() {
        assert_eq!(
            iso_date("X", "01/02/2023").unwrap(),
            Some("2023-02-01".to_string())
        );
        assert_eq!(iso_date("X", "").unwrap(), None);
        // Already-ISO input is malformed for this format and must not pass
        // silently (it would store day/month swapped).
        assert!(matches!(
            iso_date("X", "2023-02-01"),
            Err(CoreError::Catalogue(_))
        ));
        assert!(matches!(
            iso_date("X", "31/13/2023"),
            Err(CoreError::Catalogue(_))
        ));
    }
}
