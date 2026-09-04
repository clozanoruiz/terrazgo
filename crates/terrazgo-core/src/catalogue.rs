// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Imported reference catalogues (`catalogue` / `catalogue_code`).
//!
//! Regulatory exports must speak the provider's coded vocabulary — for Spain,
//! the FEGA SIEX "Anexo VII" catalogues (efficacy, justification, crop,
//! phytosanitary problem codes, …). This module vendors a snapshot of the
//! catalogue CSVs the app has a named consumer for in the binary
//! (offline-first: the app must resolve codes from first run, no network) and
//! imports them with [`ensure_catalogues`] on the first launch of each app
//! version.
//!
//! Design (docs/siex-export.md → "Storage design"):
//!   * Generic tables, provider columns verbatim in `attrs` JSON — promote a
//!     catalogue to a typed table only when a real query needs its attributes.
//!   * **Upsert only, never delete.** Providers retire codes by baja date
//!     instead of removing them; a code on an old record must keep resolving.
//!     A row that vanishes from the provider's file unexplained is marked
//!     `absent_since` and leaves the pickers, but is still resolvable.
//!   * Not in `record_change`: each device imports its own copy.
//!   * A vendored snapshot refresh rides an app release; users can also fetch
//!     the provider's current copy with [`refresh_catalogue`], through the
//!     same parser and the same upsert. An adoption outlives every restart and
//!     is replaced by the next app version's own curated snapshot.
//!
//! The provider publishes far more catalogues than we vendor; which ones we
//! carry, and how to enumerate the rest, is docs/maintenance.md §1.

use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::date::{now_utc_iso, today_utc};
use crate::error::{CoreError, Result};

/// `catalogue.source` tag for the FEGA SIEX catalogues.
pub const SOURCE_SIEX: &str = "siex";

/// The app version, stamped on the rows a vendored import writes so startup can
/// tell its own snapshot from a copy the user fetched. This crate inherits
/// `version.workspace = true`, so the workspace version — which is the app's —
/// arrives here with nothing to plumb through from the shell.
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Where the bytes an import is adopting came from.
///
/// The two origins are trusted for different things, which is why this is a
/// parameter rather than a flag on the caller's side: a vendored file is the
/// snapshot this release was tested against and stamps
/// `catalogue.imported_by_version`; a fetched one is the provider's current
/// list and must not, or the next launch would treat the user's refresh as
/// this version's own work and never restore the curated set.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Origin {
    Vendored,
    Fetched,
}

/// One vendored provider CSV, embedded at compile time.
///
/// The FEGA files share one shape: a code column, a human-facing label
/// column, optional trailing lifecycle-date columns, and whatever sits in
/// between is catalogue-specific attributes.
///
/// Which column is which is not guesswork: the provider's own registry
/// (docs/maintenance.md §1) publishes each catalogue's field metadata, where
/// the field named `codigo`/`codigoPadre` gives the code column, `descripcion`
/// the label, and `numCamposClavePrimaria > 1` means the code alone is not
/// unique and [`Vendored::identity_attrs`] is required.
///
/// **Columns are named, never numbered** (2026-08-08). They used to be
/// 0-based indices, which made the importer's correctness depend on an
/// ordering the provider never promised: injecting one leading column into
/// `MAT_FERTI` turned every stored code into the contents of the column beside
/// it, while the row count, the labels and the idempotence check all stayed
/// happy. Resolving by name removes that whole class — an inserted, removed or
/// reordered column is simply found where it now sits — and leaves only
/// renames, which [`Vendored::headers`] pins.
struct Vendored {
    /// Provider table id (the SIEX idTabla) — also the `catalogue.id`.
    id: &'static str,
    /// Raw CSV bytes, verbatim from the provider (Windows-1252 today; the
    /// decoder also accepts UTF-8 — see [`decode_provider_text`]).
    csv: &'static [u8],
    /// Header of the code column — whichever code the export payload carries.
    /// Usually `Código SIEX`, but COMUNIDAD_AUTONOMA is keyed on `Código INE`
    /// (the file leads with the *catastro* code, which SIEX does not want),
    /// and EDIFICACIONES_INSTALACIONES and DETALLE_MATERIAL_FERT lead with
    /// their parent catalogue's code rather than their own.
    code_header: &'static str,
    /// Header of the human-facing label column. The hierarchical problem
    /// catalogues and EST_FENOLOGICO carry a classification number beside the
    /// code, so their label is the third column, not the second.
    label_header: &'static str,
    /// Attribute headers that qualify the code for upsert identity, for
    /// catalogues that legitimately repeat a code (one row per ámbito, one
    /// row per SIGPAC uso, one row per crop cross-reference). Empty for
    /// everything else: the code alone is the identity.
    identity_attrs: &'static [&'static str],
    /// The file's complete header row, in order, as this app version was built
    /// against it — the pinned shape contract, checked by [`validate_shape`]
    /// on every parse.
    ///
    /// It exists for the failures name-based resolution cannot prevent:
    /// **renames**. `parse_vendored` matches `Fecha de alta`,
    /// `Fecha de modificación` and `Fecha de baja` by name in the 40 files
    /// that carry them, and four crates read `attrs` keys by name — so a
    /// renamed column silently loses retirement dates (retired codes stay in
    /// every picker) or turns an attribute read into `None`. FEGA's own
    /// variance is documented: `USOS_AGUA` heads it `Fecha Baja`.
    ///
    /// Updating an entry here is a REVIEW, never a fix: read what the provider
    /// moved, decide whether the app still reads what it thinks it reads, and
    /// only then paste the entry the failing test prints.
    headers: &'static [&'static str],
}

/// The vendored SIEX snapshot (treatment catalogues fetched 2026-07-14; the
/// rest 2026-08-05, re-checked in full 2026-09-04 — only `PRODUCTOS` had moved,
/// per `GET https://www11.fega.es/bdcsixwsp/catalogos/{id}`).
///
/// Selection rule: **a catalogue is vendored when a named part of the app
/// reads it** — the record book's coded fields, the declared-crops prefill,
/// the geography the export and the report-language offer resolve against,
/// and the vocabularies the Fertilization and Irrigation modules will need.
/// The provider publishes 287 catalogues; carrying all of them would be dead
/// weight in the binary, and carrying only what compiles today is how the
/// four "no catalogue exists" claims of slice 8 happened. docs/maintenance.md
/// §1 has the enumeration recipe and the per-catalogue consumer table.
///
/// Refreshing = replacing the files and re-releasing; the importer detects it
/// by content digest (see [`snapshot_digest`]).
const VENDORED: [Vendored; 49] = [
    Vendored {
        id: "AUTORIZACION_EXCP",
        csv: include_bytes!("../catalogues/AUTORIZACION_EXCP.csv"),
        code_header: "Código SIEX",
        label_header: "Sustancia activa o formulado",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Sustancia activa o formulado",
            "Producto comercial",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "BUENAS_PRACTICAS_AMBITOS",
        csv: include_bytes!("../catalogues/BUENAS_PRACTICAS_AMBITOS.csv"),
        code_header: "Código SIEX",
        label_header: "Buenas prácticas",
        identity_attrs: &["Ámbito"],
        headers: &[
            "Código SIEX",
            "Buenas prácticas",
            "Ámbito",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        // Keyed on `Código INE` (column 1), NOT the catastro code the
        // file leads with: the two codings diverge for 10 of the 17
        // communities (Castilla y León is catastro 08 / INE 07, and INE 07
        // is Castilla-La Mancha in the catastro coding) and SIEX asks for
        // INE. The catastro code rides in attrs.
        id: "COMUNIDAD_AUTONOMA",
        csv: include_bytes!("../catalogues/COMUNIDAD_AUTONOMA.csv"),
        code_header: "Código INE",
        label_header: "Descripción",
        identity_attrs: &[],
        headers: &["Código catastro", "Código INE", "Descripción"],
    },
    Vendored {
        id: "CULTIVO_USO_SIGPAC",
        csv: include_bytes!("../catalogues/CULTIVO_USO_SIGPAC.csv"),
        code_header: "Código",
        label_header: "Cultivo",
        identity_attrs: &["Uso SIGPAC"],
        headers: &[
            "Código",
            "Cultivo",
            "Uso SIGPAC",
            "Descripción",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "DESTINO_CULTIVO",
        csv: include_bytes!("../catalogues/DESTINO_CULTIVO.csv"),
        code_header: "Código SIEX",
        label_header: "Destino del cultivo",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Destino del cultivo",
            "Observaciones",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "DEST_COSECHA",
        csv: include_bytes!("../catalogues/DEST_COSECHA.csv"),
        code_header: "Código SIEX",
        label_header: "Declaración de cosecha / producción",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Declaración de cosecha / producción",
            "Observaciones",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "DEST_RES_VEG",
        csv: include_bytes!("../catalogues/DEST_RES_VEG.csv"),
        code_header: "Código SIEX",
        label_header: "Destino del resto vegetal",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Destino del resto vegetal",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        // Keyed on `C_FERTILIZANTE` (column 2); column 0 is the parent
        // MAT_FERTI code. 40 columns, no lifecycle dates. Labelled from
        // `D_FERTILIZANTE_2` rather than the provider's own `descripcion`
        // column: it is the only name column populated on every row (the 83
        // "PERSONALIZADO" rows leave the others blank).
        id: "DETALLE_MATERIAL_FERT",
        csv: include_bytes!("../catalogues/DETALLE_MATERIAL_FERT.csv"),
        code_header: "C_FERTILIZANTE",
        label_header: "D_FERTILIZANTE_2",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Tipo material fertilizantes según lista SIEX",
            "C_FERTILIZANTE",
            "D_CLASIFICA_NIVEL1",
            "D_CLASIFICA_NIVEL2",
            "Nombre producto",
            "Fabricante",
            "D_FERTILIZANTE_2",
            "N_% TOTAL",
            "N orgánico % (en fertilizantes orgánico-minerales)",
            "N % nítrico",
            "N % amoniacal",
            "N % ureico",
            "P2O5 % total",
            "P2O5 % soluble en agua",
            "P2O5 % soluble en citrato amónico neutro y agua",
            "P_% TOTAL",
            "K2O % total",
            "K2O % soluble en agua",
            "K_% TOTAL",
            "D_GRUPO_CONSUMO",
            "¿Inhibidor de la nitrificación? Si/No",
            "¿Inhibidor de la ureasa? Si/No",
            "Ca (CaO)",
            "Mg (MgO)",
            "S (SO3)",
            "Cadmio (Cd)",
            "Cobre (Cu)",
            "Plomo (Pb)",
            "Níquel (Ni)",
            "Zinc (Zn)",
            "Mercurio (Hg)",
            "Cromo total (Cr)",
            "Boro (B)",
            "Cobalto (Co)",
            "Manganeso (Mn)",
            "Molibdeno (Mo)",
            "Hierro (Fe)",
            "Estado de agregación",
            "% Corg",
        ],
    },
    Vendored {
        // Keyed on `Código SIEX` (column 2), unique across the file;
        // column 0 is the tipología, which repeats.
        id: "EDIFICACIONES_INSTALACIONES",
        csv: include_bytes!("../catalogues/EDIFICACIONES_INSTALACIONES.csv"),
        code_header: "Código SIEX",
        label_header: "Edificación e instalación",
        identity_attrs: &[],
        headers: &[
            "Código",
            "Tipología",
            "Código SIEX",
            "Edificación e instalación",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "EFICACIA_TRATAMIENTO",
        csv: include_bytes!("../catalogues/EFICACIA_TRATAMIENTO.csv"),
        code_header: "Código SIEX",
        label_header: "Eficacia del tratamiento",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Eficacia del tratamiento",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "ENFERMEDADES",
        csv: include_bytes!("../catalogues/ENFERMEDADES.csv"),
        code_header: "Código SIEX",
        label_header: "Categoría",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Código",
            "Categoría",
            "Nombre científico",
            "EPPO cd",
            "Observaciones",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        // The animals a grazing record names (model 9.1's "Especie animal que
        // pasta"; SIEX `Pastoreo.Animales[].Especie`), read by
        // module-ecoscheme's species picker.
        //
        // Carries NO lifecycle columns at all — no alta, modificación or baja —
        // so every row is permanently active, the `TIPO_MAQUINA_UNE` precedent.
        // `Código familia` groups the 198 species into families (bóvidos,
        // porcino, peces…); it rides in `attrs` rather than becoming the code,
        // because the record names a species.
        //
        // `RAZAS` is deliberately NOT vendored beside it: neither model 9.1 nor
        // `Pastoreo.Animales[]` asks for a breed (docs/maintenance.md §1).
        id: "ESPECIE_ANIMAL",
        csv: include_bytes!("../catalogues/ESPECIE_ANIMAL.csv"),
        code_header: "Código SIEX",
        label_header: "Especies animales",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Especies animales",
            "Código familia",
            "Familia",
        ],
    },
    Vendored {
        id: "EST_FENOLOGICO",
        csv: include_bytes!("../catalogues/EST_FENOLOGICO.csv"),
        code_header: "Código SIEX",
        label_header: "Estado fenológico",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Estadio bibliografía",
            "Estado fenológico",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "JUSTIFICACION_ACTUACION",
        csv: include_bytes!("../catalogues/JUSTIFICACION_ACTUACION.csv"),
        code_header: "Código SIEX",
        label_header: "Justificación de la actuación",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Justificación de la actuación",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "MACRONUTRIENTES",
        csv: include_bytes!("../catalogues/MACRONUTRIENTES.csv"),
        code_header: "Código SIEX",
        label_header: "Descripción",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Descripción",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "MALAS_HIERBAS",
        csv: include_bytes!("../catalogues/MALAS_HIERBAS.csv"),
        code_header: "Código SIEX",
        label_header: "Categoría",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Código",
            "Categoría",
            "Nombre científico",
            "EPPO cd",
            "Observaciones",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "MATERIAL_ANALIZADO",
        csv: include_bytes!("../catalogues/MATERIAL_ANALIZADO.csv"),
        code_header: "Código SIEX",
        label_header: "Material analizado",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Material analizado",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "MATERIAL_VEGETAL_REPRODUCCION",
        csv: include_bytes!("../catalogues/MATERIAL_VEGETAL_REPRODUCCION.csv"),
        code_header: "Código del tipo",
        label_header: "Tipo de material vegetal de reproducción",
        identity_attrs: &["Código"],
        headers: &[
            "Código del tipo",
            "Tipo de material vegetal de reproducción",
            "Código",
            "Detalle del tipo",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "MAT_FERTI",
        csv: include_bytes!("../catalogues/MAT_FERTI.csv"),
        code_header: "Código SIEX",
        label_header: "Tipo de material",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Tipo de material",
            "Campos a registrar (información disponible según tipo de producto o material)",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "MEDIDA_PREVENTIVA_CULTURAL",
        csv: include_bytes!("../catalogues/MEDIDA_PREVENTIVA_CULTURAL.csv"),
        code_header: "Código SIEX",
        label_header: "Medida",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Medida",
            "Observaciones",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "METALES_PESADOS",
        csv: include_bytes!("../catalogues/METALES_PESADOS.csv"),
        code_header: "Código SIEX",
        label_header: "Descripción",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Descripción",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "METODO_APLICACION_FERTILIZANTE",
        csv: include_bytes!("../catalogues/METODO_APLICACION_FERTILIZANTE.csv"),
        code_header: "Código SIEX",
        label_header: "Método de fertilización",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Método de fertilización",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        // Municipality codes are unique only WITHIN a province — 001 is
        // Alegría-Dulantzi in Álava and Adalia in Valladolid — so the province
        // qualifies both the upsert identity and every lookup. The file has no
        // lifecycle columns at all, which is why none are pinned below.
        id: "MUNICIPIO_SIGPAC",
        csv: include_bytes!("../catalogues/MUNICIPIO_SIGPAC.csv"),
        code_header: "Código de municipio",
        label_header: "Descripción",
        identity_attrs: &["Código de provincia"],
        headers: &[
            "Código de provincia",
            "Código de municipio",
            "Descripción",
            "Comarca agraria",
        ],
    },
    Vendored {
        id: "MICRONUTRIENTES",
        csv: include_bytes!("../catalogues/MICRONUTRIENTES.csv"),
        code_header: "Código SIEX",
        label_header: "Descripción",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Descripción",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "ORIGEN_AGUA_RIEGO",
        csv: include_bytes!("../catalogues/ORIGEN_AGUA_RIEGO.csv"),
        code_header: "Código SIEX",
        label_header: "Procedencia del agua de riego",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Procedencia del agua de riego",
            "Observaciones",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "PAIS",
        csv: include_bytes!("../catalogues/PAIS.csv"),
        code_header: "Código",
        label_header: "Descripción",
        identity_attrs: &[],
        headers: &["Código", "Descripción"],
    },
    Vendored {
        id: "PLAGAS",
        csv: include_bytes!("../catalogues/PLAGAS.csv"),
        code_header: "Código SIEX",
        label_header: "Categoría",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Código",
            "Categoría",
            "Nombre científico",
            "EPPO cd",
            "Observaciones",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "PROC_VEGETAL",
        csv: include_bytes!("../catalogues/PROC_VEGETAL.csv"),
        code_header: "Código SIEX",
        label_header: "Procedencia del material vegetal",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Procedencia del material vegetal",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "PRODUCTOS",
        csv: include_bytes!("../catalogues/PRODUCTOS.csv"),
        code_header: "Código",
        label_header: "Cultivo",
        identity_attrs: &[],
        headers: &[
            "Código",
            "Cultivo",
            "Latín",
            "EPPO",
            "C. UPOV",
            "Hortícola",
            "Energético",
            "Frutal",
            "Frutos cáscara",
            "Aromáticas, condimentarias o medicinales",
            "Siembra directa",
            "Especies mejorantes",
            "Forestal ciclo corto",
            "Leñosos",
            "Tierras de cultivo",
            "Cultivos permanentes",
            "Pastos permanentes",
            "Forestales",
            "Leguminosas",
            "Zona de no cosechado de cereales, leguminosas y oleaginosas",
            "Cereales",
            "Oleaginosas",
            "Aromáticas para zonas de no cosechado",
            "Hierba u otros forrajes herbáceos",
            "Producción de legumbre y semillas certificadas de legumbre",
            "Producción de resto de leguminosas y semillas certificadas de leguminosas",
            "Cultivos plurianuales",
            "Integrado",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        // The harvested-produce catalogue — NOT the crop catalogue
        // PRODUCTOS. One row per (produce, crop) pair, so the produce code
        // repeats: `Aceitunas` appears for OLIVO and for ACEBUCHE.
        id: "PROD_VEGETAL",
        csv: include_bytes!("../catalogues/PROD_VEGETAL.csv"),
        code_header: "Id",
        label_header: "Producto",
        identity_attrs: &["Código SIEX"],
        headers: &[
            "Id",
            "Código",
            "Producto",
            "Código SIEX",
            "Cultivo SIEX",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "PROVINCIA",
        csv: include_bytes!("../catalogues/PROVINCIA.csv"),
        code_header: "Código",
        label_header: "Descripción",
        identity_attrs: &[],
        headers: &["Código", "Descripción"],
    },
    Vendored {
        id: "REGIMEN_TENENCIA",
        csv: include_bytes!("../catalogues/REGIMEN_TENENCIA.csv"),
        code_header: "Código SIEX",
        label_header: "Régimen de tenencia",
        identity_attrs: &[],
        headers: &["Código SIEX", "Régimen de tenencia"],
    },
    Vendored {
        id: "REGULADORES_CRECIMIENTO",
        csv: include_bytes!("../catalogues/REGULADORES_CRECIMIENTO.csv"),
        code_header: "Código SIEX",
        label_header: "Categoría",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Código",
            "Categoría",
            "Nombre científico / Denominación en inglés",
            "EPPO cd",
            "Observaciones",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "SIST_CULTIVO",
        csv: include_bytes!("../catalogues/SIST_CULTIVO.csv"),
        code_header: "Código SIEX",
        label_header: "Sistema de cultivo",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Sistema de cultivo",
            "Observaciones",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "SIST_EXPLOTACION",
        csv: include_bytes!("../catalogues/SIST_EXPLOTACION.csv"),
        code_header: "Código SIEX",
        label_header: "Sistema de explotación",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Sistema de explotación",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "SIST_RIEGO",
        csv: include_bytes!("../catalogues/SIST_RIEGO.csv"),
        code_header: "Código SIEX",
        label_header: "Sistema de riego",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Sistema de riego",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "SUST_ACTIVAS",
        csv: include_bytes!("../catalogues/SUST_ACTIVAS.csv"),
        code_header: "Código SIEX",
        label_header: "Sustancia",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Código Europeo",
            "Sustancia",
            "Número CAS",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "TIPENERGIA",
        csv: include_bytes!("../catalogues/TIPENERGIA.csv"),
        code_header: "Código",
        label_header: "Descripción",
        identity_attrs: &[],
        headers: &[
            "Código",
            "Descripción",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "TIPO_ANALISIS",
        csv: include_bytes!("../catalogues/TIPO_ANALISIS.csv"),
        code_header: "Código SIEX",
        label_header: "Tipo de análisis ",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Tipo de análisis ",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "TIPO_COBERTURA_SUELO",
        csv: include_bytes!("../catalogues/TIPO_COBERTURA_SUELO.csv"),
        code_header: "Código SIEX",
        label_header: "Tipo de cobertura del suelo",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Tipo de cobertura del suelo",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "TIPO_FERITILIZACION",
        csv: include_bytes!("../catalogues/TIPO_FERITILIZACION.csv"),
        code_header: "Código SIEX",
        label_header: "Tipo de fertilización",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Tipo de fertilización",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "TIPO_LABOR",
        csv: include_bytes!("../catalogues/TIPO_LABOR.csv"),
        code_header: "Código SIEX",
        label_header: "Descripción",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Descripción",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "TIPO_MAQUINA_UNE",
        csv: include_bytes!("../catalogues/TIPO_MAQUINA_UNE.csv"),
        code_header: "Código UNE",
        label_header: "Descripción",
        identity_attrs: &[],
        headers: &["Código UNE", "Descripción"],
    },
    Vendored {
        id: "TIPO_MEDIDA_FITOSANITARIA",
        csv: include_bytes!("../catalogues/TIPO_MEDIDA_FITOSANITARIA.csv"),
        code_header: "Código SIEX",
        label_header: "Tipo de medida fitosanitaria",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Tipo de medida fitosanitaria",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "TIPO_PRODFITO",
        csv: include_bytes!("../catalogues/TIPO_PRODFITO.csv"),
        code_header: "Código SIEX",
        label_header: "Tipo de producto fitosanitario",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Tipo de producto fitosanitario",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "TIPO_TRATAMIENTO",
        csv: include_bytes!("../catalogues/TIPO_TRATAMIENTO.csv"),
        code_header: "Código SIEX",
        label_header: "Descripción",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Descripción",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "TRAT_ESTIERCOLES",
        csv: include_bytes!("../catalogues/TRAT_ESTIERCOLES.csv"),
        code_header: "Código SIEX",
        label_header: "Tratamiento de estiércoles",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Tratamiento de estiércoles",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "UNIDADES_MEDIDA",
        csv: include_bytes!("../catalogues/UNIDADES_MEDIDA.csv"),
        code_header: "Código SIEX",
        label_header: "Unidades de medida",
        identity_attrs: &[],
        headers: &[
            "Código SIEX",
            "Unidades de medida",
            "Fecha de alta",
            "Fecha de modificación",
            "Fecha de baja",
        ],
    },
    Vendored {
        id: "USO_SIGPAC",
        csv: include_bytes!("../catalogues/USO_SIGPAC.csv"),
        code_header: "Uso SIGPAC",
        label_header: "Descripción",
        identity_attrs: &[],
        headers: &["Uso SIGPAC", "Descripción"],
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

/// Import the vendored snapshot on the first run of each app version.
/// Idempotent and upsert-only — over-calling is sanctioned (it runs at every
/// startup), and rows never disappear, whatever the snapshot says.
///
/// The question asked here is "did **this app version** write this catalogue
/// data?", not "have these bytes changed?", and the difference is what makes
/// [`refresh_catalogue`] worth having: a user's fetched copy would otherwise be
/// re-imported over at the very next launch, since its bytes are by definition
/// not the vendored ones.
///
/// A version rather than a digest because the vendored files are curated as a
/// SET — the SIEX mapping bijections and the catalogue suites are green against
/// one exact snapshot — so a device must never end up running one refreshed
/// file mixed with the rest of an older release's set. Compared by equality,
/// not order: a downgrade re-imports too, which is right, because the older
/// binary's curated set is the correct set for the older binary.
pub fn ensure_catalogues(conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;
    for vendored in &VENDORED {
        let imported_by: Option<Option<String>> = tx
            .query_row(
                "SELECT imported_by_version FROM catalogue WHERE id = ?1",
                [vendored.id],
                |r| r.get(0),
            )
            .optional()?;
        // Fast path: this version already put its own snapshot here. Reading a
        // short string beats hashing ~1.6 MB of embedded CSV, and this runs at
        // every startup on every device.
        if imported_by.as_ref().and_then(Option::as_deref) == Some(APP_VERSION) {
            continue;
        }
        // Only now is parsing — and hashing — worth it.
        let already_imported = imported_by.is_some();
        let digest = snapshot_digest(vendored.csv);
        let parsed = parse_vendored(vendored)?;
        reconcile(
            &tx,
            vendored,
            &parsed,
            &digest,
            already_imported,
            Origin::Vendored,
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// The idTablas this app version carries, in file order — what a refresh asks
/// the provider for. The list is the vendored snapshot's, not the provider's:
/// fetching a catalogue we hold no spec for would give us bytes nothing can
/// read.
pub fn vendored_ids() -> Vec<&'static str> {
    VENDORED.iter().map(|v| v.id).collect()
}

/// What one catalogue's stored copy looks like right now — for the Settings
/// panel, and for deciding whether a refresh is worth offering at all.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogueStatus {
    pub id: String,
    /// When this device last adopted a copy; `None` = never imported.
    pub imported_at: Option<String>,
    /// Newest lifecycle date the adopted copy carries — the provider's own
    /// version stamp, where the file has one (several carry no dates at all).
    pub source_updated_at: Option<String>,
    pub codes: i64,
}

/// The stored state of every vendored catalogue, in file order.
pub fn catalogue_status(conn: &Connection) -> Result<Vec<CatalogueStatus>> {
    VENDORED
        .iter()
        .map(|vendored| {
            let stored: Option<(Option<String>, Option<String>)> = conn
                .query_row(
                    "SELECT imported_at, source_updated_at FROM catalogue WHERE id = ?1",
                    [vendored.id],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?;
            let (imported_at, source_updated_at) = stored.unwrap_or((None, None));
            Ok(CatalogueStatus {
                id: vendored.id.to_string(),
                imported_at,
                source_updated_at,
                codes: stored_code_count(conn, vendored.id)?,
            })
        })
        .collect()
}

fn stored_code_count(conn: &Connection, catalogue_id: &str) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM catalogue_code WHERE catalogue_id = ?1",
        [catalogue_id],
        |r| r.get(0),
    )?)
}

/// What a refresh did to one catalogue.
///
/// `Refused` is an OUTCOME, not an error: one unreadable file must never stop
/// the other 46 from updating, so the caller loops and collects.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RefreshOutcome {
    /// Byte-identical to the copy already stored — nothing was parsed, nothing
    /// written.
    Unchanged,
    Updated {
        added: usize,
        corrected: usize,
        /// Stored codes this file no longer carries, so they have left the
        /// pickers (they still resolve on old records). Reported because it is
        /// the one effect of a refresh a farmer would otherwise discover by
        /// missing a choice that used to be there.
        withdrawn: usize,
        /// Columns the file carries that this app version does not read. Safe
        /// (they ride along in `attrs`, unread) but worth saying: they are how
        /// a provider announces a field a future release may want.
        extra_columns: Vec<String>,
    },
    /// The file was left exactly where it was. `reason` is a machine code the
    /// UI renders (`catalogue.refused.<reason>`); `detail` is the technical
    /// specifics — a column name, a row count — shown verbatim beside it.
    Refused {
        reason: &'static str,
        detail: String,
    },
}

/// One catalogue's line in a refresh report.
#[derive(Debug, Clone, Serialize)]
pub struct RefreshReport {
    pub id: String,
    pub outcome: RefreshOutcome,
}

impl RefreshReport {
    /// A refusal raised by the caller rather than by the data — the fetch that
    /// never produced bytes. Constructed here so every refusal in a report,
    /// whatever raised it, carries the same shape.
    pub fn refused(id: &str, reason: &'static str, detail: String) -> Self {
        Self {
            id: id.to_string(),
            outcome: RefreshOutcome::Refused { reason, detail },
        }
    }
}

/// Adopt a freshly fetched copy of one catalogue — validate first, write only
/// after everything has passed.
///
/// The order is the whole design, because [`reconcile`] never deletes: a bad
/// file adopted here would leave bogus rows in a picker forever, and no later
/// good file could take them out. So every check that can refuse runs before
/// the transaction opens, and a refusal leaves the stored rows byte-for-byte
/// as they were.
///
/// The checks, in order:
///
/// 1. **digest** — identical bytes are [`RefreshOutcome::Unchanged`]; nothing
///    is even parsed.
/// 2. **shape**, under [`Shape::Compatible`]: every column the app reads still
///    resolves by name. A rename is an *app-update* event, not a data-update
///    one — the app reads named columns and cannot adapt at runtime — so it
///    refuses and says so; an added column is adopted and reported.
/// 3. **every row has a label** — a blank one prints as an empty cell in a
///    picker or a legal document rather than failing loudly.
/// 4. **no control characters** — the tripwire for the provider changing
///    encoding under us (the files are Windows-1252 while documented as
///    ISO-8859-1).
/// 5. **the row count must not shrink** — FEGA baja-dates codes and never
///    removes them, so a file shorter than what is already stored is a
///    truncated download, not a smaller catalogue.
///
/// `bytes` are the provider's, verbatim; decoding and CSV parsing happen here,
/// through exactly the code path the vendored files take. Network I/O is the
/// caller's — core never fetches anything (docs/architecture.md → offline
/// first).
///
/// **What an adoption is worth over time.** It survives every restart, because
/// [`ensure_catalogues`] skips on the app version and this writes no version
/// stamp. It does *not* survive an app update: the next version's first run
/// re-imports its own curated snapshot over the top, which is the point — the
/// vendored files are tested as a set. What an update restores is every label,
/// attribute and lifecycle date; codes the refresh *added* stay, since the
/// import cannot delete without breaking the promise that a code already
/// written onto a record keeps resolving.
pub fn refresh_catalogue(conn: &mut Connection, id: &str, bytes: &[u8]) -> Result<RefreshReport> {
    let vendored = VENDORED
        .iter()
        .find(|v| v.id == id)
        .ok_or(CoreError::NotFound)?;
    let refused = |reason: &'static str, detail: String| {
        Ok(RefreshReport::refused(vendored.id, reason, detail))
    };

    let digest = snapshot_digest(bytes);
    let stored_digest: Option<Option<String>> = conn
        .query_row(
            "SELECT source_digest FROM catalogue WHERE id = ?1",
            [vendored.id],
            |r| r.get(0),
        )
        .optional()?;
    if stored_digest.as_ref().and_then(Option::as_deref) == Some(digest.as_str()) {
        return Ok(RefreshReport {
            id: vendored.id.to_string(),
            outcome: RefreshOutcome::Unchanged,
        });
    }

    let parsed = match parse_catalogue(vendored, bytes, Shape::Compatible) {
        Ok(parsed) => parsed,
        // Everything the parser rejects — a missing or duplicated column, a
        // malformed record, an unreadable lifecycle date — arrives as
        // `Catalogue`, and all of it means the same thing to a user: this file
        // is not one this version can read. Anything else is a real failure
        // (a poisoned connection, a JSON error) and propagates.
        Err(CoreError::Catalogue(detail)) => {
            // The parser prefixes its messages with the catalogue id, which
            // the report line already carries — drop it rather than print the
            // file's name twice on one line.
            let prefix = format!("{}: ", vendored.id);
            return refused(
                "shape",
                detail.strip_prefix(&prefix).unwrap_or(&detail).to_string(),
            );
        }
        Err(other) => return Err(other),
    };

    if parsed.rows.is_empty() {
        return refused("empty", String::new());
    }
    if let Some(row) = parsed.rows.iter().find(|r| r.label.trim().is_empty()) {
        return refused("label", format!("{} — {}", vendored.label_header, row.code));
    }
    if let Some(detail) = first_control_character(&parsed.rows) {
        return refused("control_characters", detail);
    }
    let stored = stored_code_count(conn, vendored.id)?;
    if (parsed.rows.len() as i64) < stored {
        return refused("shrunk", format!("{} < {stored}", parsed.rows.len()));
    }

    let tx = conn.transaction()?;
    let counts = reconcile(
        &tx,
        vendored,
        &parsed,
        &digest,
        stored_digest.is_some(),
        Origin::Fetched,
    )?;
    tx.commit()?;
    Ok(RefreshReport {
        id: vendored.id.to_string(),
        outcome: RefreshOutcome::Updated {
            added: counts.added,
            corrected: counts.corrected,
            withdrawn: counts.withdrawn,
            extra_columns: parsed.extra_columns,
        },
    })
}

/// The first code + text carrying a C0/C1 control character, if any. Tabs and
/// newlines are legitimate inside quoted CSV cells; everything else in those
/// ranges is an encoding accident (the same rule the vendored-snapshot
/// tripwire test applies).
fn first_control_character(rows: &[ParsedCode]) -> Option<String> {
    let dirty = |text: &str| {
        text.chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    };
    for row in rows {
        if dirty(&row.label) {
            return Some(format!("{} — {:?}", row.code, row.label));
        }
        let values = row.attrs.as_ref().and_then(Value::as_object);
        for (key, value) in values.into_iter().flatten() {
            if value.as_str().is_some_and(dirty) {
                return Some(format!("{} — {key}: {value}", row.code));
            }
        }
    }
    None
}

/// Content fingerprint of the bytes an import adopted — FNV-1a 64, hand-rolled
/// because this needs a change detector, not a cryptographic hash, and the
/// alternative was a dependency.
///
/// It answers one question, for [`refresh_catalogue`] alone: are these the
/// bytes already stored here, so that re-offering them costs nothing? Bytes
/// rather than the file's newest lifecycle date, because a provider correcting
/// a label without moving any date is a real refresh, and several catalogues
/// ship no dates at all. Startup does not consult it — which snapshot a launch
/// should import is a question about the app version, not about bytes
/// (see [`ensure_catalogues`]).
fn snapshot_digest(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The header row a vendored catalogue is pinned to, for consumers that read
/// a file column-wise and must therefore account for every column of it.
///
/// `module-fertilisation`'s composition fill is the one such consumer today:
/// it maps 19 of `DETALLE_MATERIAL_FERT`'s columns and must state why it
/// ignores the rest, so that a provider adding a column cannot pass unnoticed
/// as a nutrient nobody decided about.
pub fn vendored_headers(catalogue_id: &str) -> Option<&'static [&'static str]> {
    VENDORED
        .iter()
        .find(|v| v.id == catalogue_id)
        .map(|v| v.headers)
}

/// The code and label headers a vendored catalogue is read by — the two
/// columns [`vendored_headers`] consumers never have to account for, since
/// they are not attributes.
pub fn vendored_key_headers(catalogue_id: &str) -> Option<(&'static str, &'static str)> {
    VENDORED
        .iter()
        .find(|v| v.id == catalogue_id)
        .map(|v| (v.code_header, v.label_header))
}

/// One offer in a catalogue-backed picker: the code the row stores, and the
/// name the farmer reads. The modules' own catalogue helpers re-export this
/// rather than defining it again.
#[derive(Debug, Clone, Serialize)]
pub struct CataloguePick {
    pub code: String,
    pub name: String,
}

/// Which catalogue names the classes of building a holding can hold. Core's
/// first per-country catalogue map, and it stays country-neutral the way the
/// storage layer does: the mechanism is generic and the Spanish-ness is data.
pub fn premises_class_catalogue(country_code: &str) -> Option<&'static str> {
    match country_code {
        "es" => Some("EDIFICACIONES_INSTALACIONES"),
        _ => None,
    }
}

/// The classes a `premises` row can name (FEGA `EDIFICACIONES_INSTALACIONES`,
/// 109 entries). Empty for a country with no coded list, which is what a picker
/// with nothing to offer means — the column is nullable and the class optional.
///
/// Anexo V marks this class obligatory in the REA "in the event of a treatment
/// in the buildings and installations that entails their identification for the
/// CUE" — this app's exact case, models 3.4 and 3.5.
pub fn premises_classes(conn: &Connection, country_code: &str) -> Result<Vec<CataloguePick>> {
    let Some(catalogue_id) = premises_class_catalogue(country_code) else {
        return Ok(Vec::new());
    };
    Ok(active_codes(conn, catalogue_id)?
        .into_iter()
        .map(|row| CataloguePick {
            code: row.code,
            name: row.label,
        })
        .collect())
}

/// The offerable codes of one catalogue, in file order (providers publish their
/// lists in a deliberate order) — what a UI picker offers.
///
/// Two ways a code stops being offered, deliberately separate because they are
/// different claims: `retired_on` is the authority's own baja date, and
/// `absent_since` is ours, meaning the provider's current file no longer
/// carries the row at all. Neither removes it — [`find_code`] resolves both, so
/// a record that cites one still displays.
pub fn active_codes(conn: &Connection, catalogue_id: &str) -> Result<Vec<CatalogueCode>> {
    codes_where(
        conn,
        "catalogue_id = ?1 AND retired_on IS NULL AND absent_since IS NULL",
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
    /// Headers the file carries that this app version was not built against.
    /// Always empty under [`Shape::Pinned`]; under [`Shape::Compatible`] they
    /// are adopted (they land in `attrs`, unread) and reported.
    extra_columns: Vec<String>,
}

/// How strictly a header row is held to [`Vendored::headers`].
///
/// The two callers want different things, and the difference is the whole
/// reason a refresh can exist at all.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// The complete header row, in order, exactly as pinned. What the
    /// vendored files are held to: a mismatch there means the repository's own
    /// snapshot moved under the app, which is a review, never a runtime event.
    Pinned,
    /// Every column this app version READS is still present, by name and
    /// unambiguously; anything else the file has gained is adopted and
    /// reported. What a fetched file is held to, because a provider appending
    /// a column nobody reads must not leave users unable to update until the
    /// next app release — while a renamed column, which silently loses
    /// retirement dates or turns an attribute read into nothing, still
    /// refuses.
    Compatible,
}

/// Refuse a header row that is not the shape this app version reads.
///
/// Called on EVERY parse, so it guards the vendored files today and anything
/// fetched from the provider later — the rules a test cannot enforce on a
/// user's machine. A vendored file can only fail this if someone edited it
/// without running the suite, which CI prevents.
fn validate_shape(vendored: &Vendored, headers: &csv::StringRecord) -> Result<()> {
    let bad = |detail: String| CoreError::Catalogue(format!("{}: {detail}", vendored.id));
    if headers.len() != vendored.headers.len() {
        return Err(bad(format!(
            "the file has {} columns, not the {} this version was built against — \
             a column was added or removed; review what moved before repinning",
            headers.len(),
            vendored.headers.len()
        )));
    }
    for (index, (actual, pinned)) in headers.iter().zip(vendored.headers).enumerate() {
        if actual != *pinned {
            return Err(bad(format!(
                "column {index} is '{actual}', not the pinned '{pinned}' — a rename can \
                 silently lose lifecycle dates or turn an attribute read into nothing"
            )));
        }
    }
    Ok(())
}

/// The [`Shape::Compatible`] check: every pinned header must still resolve by
/// name, exactly once. Returns whatever the file carries beyond them.
///
/// This is strictly weaker than [`validate_shape`] and deliberately so — it
/// drops "in this order" and "and nothing else", which are properties the app
/// never actually relied on once columns stopped being positions. What it
/// keeps is the property that matters: every name the app reads is still
/// there. A rename fails here as a *missing* pinned header, which is why the
/// tolerance for additions costs nothing.
fn validate_compatible_shape(
    vendored: &Vendored,
    headers: &csv::StringRecord,
) -> Result<Vec<String>> {
    for pinned in vendored.headers {
        column_index(vendored, headers, pinned)?;
    }
    Ok(headers
        .iter()
        .filter(|actual| !vendored.headers.contains(actual))
        .map(str::to_string)
        .collect())
}

/// Resolve one column by NAME, refusing both absence and ambiguity. A file
/// carrying the same header twice would make "the" column meaningless, and
/// picking the first would be a guess.
fn column_index(vendored: &Vendored, headers: &csv::StringRecord, name: &str) -> Result<usize> {
    let bad = |detail: String| CoreError::Catalogue(format!("{}: {detail}", vendored.id));
    let mut found = headers.iter().enumerate().filter(|(_, h)| *h == name);
    let (index, _) = found
        .next()
        .ok_or_else(|| bad(format!("column '{name}' is missing")))?;
    if found.next().is_some() {
        return Err(bad(format!("column '{name}' appears more than once")));
    }
    Ok(index)
}

fn parse_vendored(vendored: &Vendored) -> Result<ParsedCatalogue> {
    parse_catalogue(vendored, vendored.csv, Shape::Pinned)
}

/// Parse provider bytes against a catalogue's spec. Split from
/// [`parse_vendored`] so the same code path serves bytes that did not come
/// from `include_bytes!` — [`refresh_catalogue`] fetches them over the network.
fn parse_catalogue(vendored: &Vendored, bytes: &[u8], shape: Shape) -> Result<ParsedCatalogue> {
    let bad = |detail: String| CoreError::Catalogue(format!("{}: {detail}", vendored.id));
    let text = decode_provider_text(bytes);
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b';')
        .from_reader(text.as_bytes());
    let headers = reader.headers().map_err(|e| bad(e.to_string()))?.clone();
    let extra_columns = match shape {
        Shape::Pinned => {
            validate_shape(vendored, &headers)?;
            Vec::new()
        }
        Shape::Compatible => validate_compatible_shape(vendored, &headers)?,
    };
    let code_col = column_index(vendored, &headers, vendored.code_header)?;
    let label_col = column_index(vendored, &headers, vendored.label_header)?;
    let identity_cols = vendored
        .identity_attrs
        .iter()
        .map(|name| column_index(vendored, &headers, name))
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
                _ if i == code_col || i == label_col => {}
                _ if value.is_empty() => {}
                _ => {
                    attrs.insert(header.to_string(), Value::String(value.to_string()));
                }
            }
        }
        // A blank code is not a row: COMUNIDAD_AUTONOMA's "Comunidad
        // Desconocida" placeholder has no INE code, and storing it would put
        // an unaddressable row in the picker.
        let code = field(code_col)?;
        if code.is_empty() {
            continue;
        }
        for date in [&added_on, &modified_on, &retired_on].into_iter().flatten() {
            if newest_date.as_deref().is_none_or(|n| date.as_str() > n) {
                newest_date = Some(date.clone());
            }
        }
        rows.push(ParsedCode {
            code: code.to_string(),
            label: field(label_col)?.to_string(),
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
    Ok(ParsedCatalogue {
        rows,
        newest_date,
        extra_columns,
    })
}

/// What one [`reconcile`] pass changed — the numbers a refresh reports back to
/// the user, and the only honest way to say "nothing moved" about bytes that
/// differ from the ones already stored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct ReconcileCounts {
    /// Codes the file carries that were not stored yet.
    pub added: usize,
    /// Stored codes whose label, attributes or lifecycle dates moved.
    pub corrected: usize,
    /// Stored codes the provider's current file no longer carries: kept and
    /// still resolvable, but no longer offered. Only a fetched file can do
    /// this — see [`Origin`].
    pub withdrawn: usize,
}

/// Upsert one catalogue: update drifted rows in place (keeping their ids —
/// they may be referenced by the time typed promotions exist), insert new
/// ones, and NEVER delete — rows absent from the snapshot stay untouched.
///
/// That last rule is why [`refresh_catalogue`] validates before calling this:
/// a bad file adopted here leaves permanent bogus rows, and a later good
/// import can repair their labels but can never remove them.
fn reconcile(
    tx: &Transaction<'_>,
    vendored: &Vendored,
    parsed: &ParsedCatalogue,
    digest: &str,
    already_imported: bool,
    origin: Origin,
) -> Result<ReconcileCounts> {
    let now = now_utc_iso();
    // Only a vendored import claims the version stamp. A fetched one leaves
    // whatever is there untouched — including NULL, which correctly tells the
    // next startup that this version's own snapshot has never been imported
    // here.
    let stamp = (origin == Origin::Vendored).then_some(APP_VERSION);
    if already_imported {
        match stamp {
            Some(version) => tx.execute(
                "UPDATE catalogue SET source = ?2, source_updated_at = ?3, source_digest = ?4, imported_at = ?5, imported_by_version = ?6 WHERE id = ?1",
                params![vendored.id, SOURCE_SIEX, parsed.newest_date, digest, now, version],
            )?,
            None => tx.execute(
                "UPDATE catalogue SET source = ?2, source_updated_at = ?3, source_digest = ?4, imported_at = ?5 WHERE id = ?1",
                params![vendored.id, SOURCE_SIEX, parsed.newest_date, digest, now],
            )?,
        };
    } else {
        tx.execute(
            "INSERT INTO catalogue (id, source, source_updated_at, source_digest, imported_at, imported_by_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![vendored.id, SOURCE_SIEX, parsed.newest_date, digest, now, stamp],
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
        absent_since: Option<String>,
    }
    let mut existing: HashMap<(String, Vec<String>), DbRow> = HashMap::new();
    {
        let mut stmt = tx.prepare(
            "SELECT id, code, label, attrs, added_on, modified_on, retired_on, absent_since
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
                    r.get::<_, Option<String>>(7)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (id, code, label, attrs, added_on, modified_on, retired_on, absent_since) in raw {
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
                    absent_since,
                },
            );
        }
    }

    let mut insert = tx.prepare(
        "INSERT INTO catalogue_code (catalogue_id, code, label, attrs, added_on, modified_on, retired_on)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    // A row the file carries is a row the provider still publishes, so this
    // clears `absent_since` as well as writing the provider's own fields —
    // whichever origin the file came from, since presence is presence.
    let mut update = tx.prepare(
        "UPDATE catalogue_code SET label = ?2, attrs = ?3, added_on = ?4, modified_on = ?5, retired_on = ?6, absent_since = NULL
         WHERE id = ?1",
    )?;
    let mut counts = ReconcileCounts::default();
    // Identities the file carried, so what is left of `existing` afterwards is
    // exactly what it did not. Tracked separately rather than by draining
    // `existing`, so a file that repeats one identity still collapses onto the
    // single stored row instead of inserting a duplicate.
    let mut seen: HashSet<(String, Vec<String>)> = HashSet::with_capacity(parsed.rows.len());
    for row in &parsed.rows {
        let attrs_text = row.attrs.as_ref().map(serde_json::to_string).transpose()?;
        let key = (row.code.clone(), row.identity.clone());
        match existing.get(&key) {
            Some(db)
                if db.label == row.label
                    && db.attrs == row.attrs
                    && db.added_on == row.added_on
                    && db.modified_on == row.modified_on
                    && db.retired_on == row.retired_on
                    && db.absent_since.is_none() => {}
            Some(db) => {
                update.execute(params![
                    db.id,
                    row.label,
                    attrs_text,
                    row.added_on,
                    row.modified_on,
                    row.retired_on
                ])?;
                counts.corrected += 1;
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
                counts.added += 1;
            }
        }
        seen.insert(key);
    }

    // Rows the file did not carry. Only a FETCHED file licenses the inference:
    // it is the provider's current list, so a row missing from it is genuinely
    // no longer published. A vendored file proves nothing of the sort — a code
    // can be missing from it merely by being newer than the release — and
    // inferring there would hide every code a refresh had added, at the next
    // app update.
    if origin == Origin::Fetched {
        let today = today_utc();
        let mut withdraw =
            tx.prepare("UPDATE catalogue_code SET absent_since = ?2 WHERE id = ?1")?;
        for (key, db) in &existing {
            // Keep the FIRST date we saw it gone: it records when the row
            // disappeared, and re-stamping it would also report a change on
            // every later refresh to a row that did not change.
            if db.absent_since.is_none() && !seen.contains(key) {
                withdraw.execute(params![db.id, today])?;
                counts.withdrawn += 1;
            }
        }
    }
    Ok(counts)
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

    /// The header row of one vendored file, read exactly the way the importer
    /// reads it — same decoder, same delimiter, same csv parser.
    fn header_of(vendored: &Vendored) -> csv::StringRecord {
        let text = decode_provider_text(vendored.csv);
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b';')
            .from_reader(text.as_bytes());
        reader
            .headers()
            .expect("a vendored file must have a header row")
            .clone()
    }

    /// The pinned entry a file would need, formatted for pasting — so a
    /// legitimate provider change costs a read and a paste rather than
    /// transcription by hand.
    fn pinned_entry(headers: &csv::StringRecord) -> String {
        let mut out = String::from("        headers: &[\n");
        for header in headers {
            out.push_str(&format!("            {header:?},\n"));
        }
        out.push_str("        ],");
        out
    }

    #[test]
    fn every_vendored_file_matches_its_pinned_shape() {
        // The contract the whole snapshot rests on. Name-based resolution
        // already absorbs an inserted, removed or reordered column; what this
        // catches is a RENAME, which nothing else would notice — the importer
        // matches the three lifecycle headers by name in the 40 files that
        // carry them, and four crates read `attrs` keys by name, so a rename
        // silently loses retirement dates or turns a read into nothing.
        //
        // Kept from the experiment that prompted it (2026-08-08): injecting
        // one leading column into MAT_FERTI left all twenty other catalogue
        // guards passing while every stored code became "Nuevo campo".
        for vendored in &VENDORED {
            let headers = header_of(vendored);
            if let Err(err) = validate_shape(vendored, &headers) {
                panic!(
                    "{err}\n\nIf the provider legitimately changed this file, REVIEW what \
                     moved — does the app still read what it thinks it reads? — and only \
                     then paste:\n\n{}\n",
                    pinned_entry(&headers)
                );
            }
            // The named columns must resolve, and unambiguously.
            column_index(vendored, &headers, vendored.code_header).unwrap();
            column_index(vendored, &headers, vendored.label_header).unwrap();
            for attr in vendored.identity_attrs {
                column_index(vendored, &headers, attr).unwrap();
            }
        }
    }

    #[test]
    fn a_shifted_file_still_imports_the_right_codes() {
        // The mechanism fix, stated as behaviour rather than as a guard: with
        // columns resolved by name, a file that gains a leading column still
        // yields the same codes. Under the old index-based spec every code
        // became the contents of the column beside it.
        let vendored = VENDORED
            .iter()
            .find(|v| v.id == "MAT_FERTI")
            .expect("MAT_FERTI is vendored");
        let original = decode_provider_text(vendored.csv);
        let shifted: String = original
            .lines()
            .map(|line| {
                if line.trim().is_empty() {
                    line.to_string()
                } else {
                    format!("\"Nuevo campo\";{line}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        // The pin refuses the new shape, as it must — the app was not built
        // to read it.
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b';')
            .from_reader(shifted.as_bytes());
        let headers = reader.headers().unwrap().clone();
        assert!(validate_shape(vendored, &headers).is_err());

        // But the columns themselves are still found by name, so nothing
        // could silently read the wrong one.
        let code_col = column_index(vendored, &headers, vendored.code_header).unwrap();
        assert_eq!(code_col, 1, "the code column moved right by one");
        let mut records = reader.records();
        let first = records.next().unwrap().unwrap();
        assert_eq!(first.get(code_col), Some("0"), "still MAT_FERTI's own code");
    }

    #[test]
    fn an_ambiguous_column_is_refused_rather_than_guessed() {
        // A file carrying the same header twice makes "the" column meaningless,
        // and taking the first would be a guess.
        let vendored = &VENDORED[0];
        let headers = csv::StringRecord::from(vec!["Código SIEX", "Otra", "Código SIEX"]);
        let err = column_index(vendored, &headers, "Código SIEX").unwrap_err();
        assert!(format!("{err}").contains("more than once"), "{err}");
    }

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
