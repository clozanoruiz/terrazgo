// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The printed cuaderno's labels, per language.
//!
//! The layout is per COUNTRY (the Spanish official model), the language is per
//! REGION: where a co-official language exists, the farmer must be able to hand
//! an inspector the same book in either language. So the template holds no
//! prose at all — every heading, footnote and printed word arrives here as
//! data, and both renderers (Typst and the spreadsheet) read the same struct.
//!
//! A Rust struct rather than a dictionary file: a missing translation is then a
//! COMPILE error, which is stronger than the key-parity contract test the
//! frontend dictionaries need (`src-tauri/tests/i18n_contract.rs`), and serde
//! turns the same struct into the template's `sys.inputs.labels` for free.
//!
//! # What does NOT live here
//!
//! Codes are not prose. The model's own siglas (SEC/ASP/LOC/GRA, AL/M/BP/INV,
//! AE/PI/CP/Atrias/AS/NO), dose-unit symbols (`L/ha`) and the FEGA catalogue
//! labels resolved for section 3.1's "problema fitosanitario" are catalogue
//! payload printed verbatim in every language — the record's legal value is the
//! code, and the footnote that explains the sigla is what translates. So a
//! Catalan book prints Spanish pest names under Catalan headings, deliberately.
//!
//! Adding a language = one `Labels` const here + one arm in [`ReportLanguage`]
//! + its provinces in `super::region`. Nothing else changes.

use serde::Serialize;

/// A language the record book can be printed in.
///
/// Not the same set as the UI locales (`src/i18n.js`): the UI speaks English,
/// which is official nowhere in Spain, and a report language needs the full
/// regulatory vocabulary rather than app chrome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportLanguage {
    /// Castilian — official across the whole state, so always offered.
    Es,
    /// Catalan — co-official in Catalunya and the Illes Balears.
    Ca,
}

impl ReportLanguage {
    /// Every language the app can print today, Castilian first (it is the
    /// fallback everywhere).
    pub const ALL: [ReportLanguage; 2] = [ReportLanguage::Es, ReportLanguage::Ca];

    pub fn code(self) -> &'static str {
        match self {
            ReportLanguage::Es => "es",
            ReportLanguage::Ca => "ca",
        }
    }

    /// The language's name in itself — shown untranslated in the chooser, the
    /// same reasoning `SUPPORTED` in `src/i18n.js` applies. "Castellano", not
    /// "Español": that is what the language is called in Spain, where naming it
    /// otherwise reads as a claim about the co-official ones.
    pub fn native_name(self) -> &'static str {
        match self {
            ReportLanguage::Es => "Castellano",
            ReportLanguage::Ca => "Català",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        ReportLanguage::ALL.into_iter().find(|l| l.code() == code)
    }

    pub fn labels(self) -> &'static Labels {
        match self {
            ReportLanguage::Es => &ES,
            ReportLanguage::Ca => &CA,
        }
    }
}

// ---------------------------------------------------------------------------
// The label set — grouped by model section, mirroring templates/cuaderno.typ
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct Labels {
    pub doc: Doc,
    pub s1: S1,
    pub s12: S12,
    pub s13: S13,
    pub s14: S14,
    pub s21: S21,
    pub s22: S22,
    pub s31: S31,
    pub s31bis: S31Bis,
    pub s32: S32,
    pub s33: S33,
    pub s4: S4,
    pub s5: S5,
    pub s6: S6,
    pub s71: S71,
    pub s8: S8,
    pub annex: Annex,
    pub value: Values,
    /// Spreadsheet-only wording (tab names, the 1.1 label column, and the
    /// columns the sheet adds because it resolves what the PDF cross-references).
    pub sheet: SheetLabels,
}

/// Page furniture: the running header, the footer and the campaign line.
#[derive(Serialize)]
pub struct Doc {
    pub farm_owner: &'static str,
    pub campaign: &'static str,
    pub generated_on: &'static str,
    pub page: &'static str,
    pub page_of: &'static str,
}

/// Section 1 and its 1.1 block, including the representative and signature.
#[derive(Serialize)]
pub struct S1 {
    pub title: &'static str,
    pub opening_date: &'static str,
    pub general_title: &'static str,
    pub owner_name: &'static str,
    pub tax_id: &'static str,
    pub registry_national: &'static str,
    pub registry_regional: &'static str,
    pub address: &'static str,
    pub locality: &'static str,
    pub postal_code: &'static str,
    pub province: &'static str,
    pub phone_fixed: &'static str,
    pub phone_mobile: &'static str,
    pub email: &'static str,
    pub farm_name: &'static str,
    pub representative_title: &'static str,
    pub full_name: &'static str,
    pub representation_kind: &'static str,
    pub phone: &'static str,
    pub signature: &'static str,
    pub date: &'static str,
    pub signature_note: &'static str,
}

#[derive(Serialize)]
pub struct S12 {
    pub title: &'static str,
    pub order: &'static str,
    pub name: &'static str,
    pub tax_id: &'static str,
    pub licence_number: &'static str,
    pub licence_level: &'static str,
    pub advisor: &'static str,
    pub note: &'static str,
}

#[derive(Serialize)]
pub struct S13 {
    pub title: &'static str,
    pub order: &'static str,
    pub description: &'static str,
    pub roma: &'static str,
    pub reganip: &'static str,
    pub acquired_on: &'static str,
    pub last_inspection: &'static str,
}

#[derive(Serialize)]
pub struct S14 {
    pub title: &'static str,
    pub name: &'static str,
    pub tax_id: &'static str,
    pub registration_number: &'static str,
    pub gip: &'static str,
    pub note: &'static str,
}

#[derive(Serialize)]
pub struct S21 {
    pub section_title: &'static str,
    pub title: &'static str,
    pub order: &'static str,
    pub plot: &'static str,
    pub province: &'static str,
    pub municipality: &'static str,
    pub aggregate: &'static str,
    pub zone: &'static str,
    pub polygon: &'static str,
    pub parcel: &'static str,
    pub enclosure: &'static str,
    pub land_use: &'static str,
    pub sigpac_area: &'static str,
    pub cultivated_area: &'static str,
    pub species: &'static str,
    pub variety: &'static str,
    pub irrigation: &'static str,
    pub environment: &'static str,
    pub gip: &'static str,
    pub note_gip: &'static str,
    pub note_area: &'static str,
    pub note_irrigation: &'static str,
    pub note_environment: &'static str,
}

#[derive(Serialize)]
pub struct S22 {
    pub title: &'static str,
    pub order: &'static str,
    pub species: &'static str,
    pub variety: &'static str,
    pub water_point: &'static str,
    pub distance: &'static str,
    /// The model heads this column "Coordenadas UTM" and marks it voluntary.
    /// We print the lat/lon pair we store — what SIGPAC, the geometry store and
    /// the importer all speak — and say so, rather than converting behind the
    /// farmer's back into a projection nothing else in the app uses.
    pub coordinates: &'static str,
    pub denomination: &'static str,
    pub fully: &'static str,
    pub partly: &'static str,
    pub checked: &'static str,
    pub note: &'static str,
}

#[derive(Serialize)]
pub struct S31 {
    pub section_title: &'static str,
    pub title: &'static str,
    pub plots: &'static str,
    pub species: &'static str,
    pub variety: &'static str,
    pub date: &'static str,
    pub surface: &'static str,
    pub problem: &'static str,
    pub operator: &'static str,
    pub equipment: &'static str,
    pub product: &'static str,
    pub registration: &'static str,
    pub dose: &'static str,
    /// Anexo III Parte I B.i — total product used over the whole actuation.
    pub total_quantity: &'static str,
    pub phi: &'static str,
    pub efficacy: &'static str,
    pub notes: &'static str,
    pub note_plots: &'static str,
    pub note_operator: &'static str,
    pub note_equipment: &'static str,
    pub note_phi: &'static str,
    pub note_efficacy: &'static str,
    pub note_date: &'static str,
    pub note_total_quantity: &'static str,
    /// Reglamento (UE) 2023/564's growth stage, which folds into the species
    /// cell because the Spanish model has no column for it — so the page has to
    /// say what the number beside the crop is.
    pub note_growth_stage: &'static str,
}

/// Section 3.1 bis — registro por parcela de los cultivos objeto de
/// asesoramiento. The same actuations as 3.1, cut for the advised ones and
/// showing what 3.1 has no column for: the non-chemical alternative and who
/// advised it.
#[derive(Serialize)]
pub struct S31Bis {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub crop_group: &'static str,
    pub plot_group: &'static str,
    pub problem_group: &'static str,
    pub non_chemical_group: &'static str,
    pub chemical_group: &'static str,
    pub species: &'static str,
    pub variety: &'static str,
    pub plots: &'static str,
    pub crop_surface: &'static str,
    pub treated_surface: &'static str,
    pub problem: &'static str,
    pub justification: &'static str,
    pub measure: &'static str,
    pub intensity: &'static str,
    pub measure_date: &'static str,
    pub product: &'static str,
    pub registration: &'static str,
    pub dose: &'static str,
    pub product_date: &'static str,
    pub efficacy: &'static str,
    pub notes: &'static str,
    pub note_plots: &'static str,
    pub note_intensity: &'static str,
    /// The two sign-off boxes at the foot of the model's page. Hand-signed, so
    /// the book prefills who and their ROPO number and leaves the rest ruled.
    pub validation_interim: &'static str,
    pub validation_final: &'static str,
    pub signature: &'static str,
    pub advisor: &'static str,
    pub ropo: &'static str,
    pub date: &'static str,
    pub season_end_date: &'static str,
}

/// Section 3.2 — uso de semilla tratada.
#[derive(Serialize)]
pub struct S32 {
    pub title: &'static str,
    pub plots: &'static str,
    pub date: &'static str,
    pub species: &'static str,
    pub variety: &'static str,
    pub surface: &'static str,
    pub seed_quantity: &'static str,
    pub seed_lot: &'static str,
    pub product: &'static str,
    pub registration: &'static str,
    pub active_substance: &'static str,
    pub efficacy: &'static str,
    pub notes: &'static str,
    pub note_plots: &'static str,
    pub note_seed_lot: &'static str,
}

/// Sections 3.3, 3.4 and 3.5. One struct: the three tables share every column
/// but the two that name the subject and its measure, so those are per section
/// and the rest are stated once.
#[derive(Serialize)]
pub struct S33 {
    /// The "APLICA TRATAMIENTO: SÍ / NO" line each register is headed with.
    pub applies: &'static str,
    pub title_postharvest: &'static str,
    pub title_storage: &'static str,
    pub title_transport: &'static str,
    pub subject_postharvest: &'static str,
    pub subject_storage: &'static str,
    pub subject_transport: &'static str,
    pub quantity_postharvest: &'static str,
    pub quantity_storage: &'static str,
    pub quantity_transport: &'static str,
    pub date: &'static str,
    pub problem: &'static str,
    pub operator: &'static str,
    pub product: &'static str,
    pub registration: &'static str,
    pub product_quantity: &'static str,
    pub efficacy: &'static str,
    pub notes: &'static str,
    pub note_applies: &'static str,
    pub note_product_quantity: &'static str,
}

/// Section 4 — registro de análisis.
#[derive(Serialize)]
pub struct S4 {
    pub section_title: &'static str,
    pub title: &'static str,
    pub date: &'static str,
    pub material: &'static str,
    pub plots: &'static str,
    pub bulletin: &'static str,
    pub laboratory: &'static str,
    pub substances: &'static str,
    pub note_plots: &'static str,
    /// Why the register carries no result attachment: art. 16.3 obliges keeping
    /// the bulletin, and this book only says where it is.
    pub note_keep: &'static str,
    /// Why soil figures appear in the findings cell: the printed model predates
    /// Anexo III A.3 and has no soil page.
    pub note_soil: &'static str,
}

/// Section 5 — registro de cosecha comercializada.
#[derive(Serialize)]
pub struct S5 {
    pub section_title: &'static str,
    pub title: &'static str,
    pub date: &'static str,
    pub product: &'static str,
    pub quantity: &'static str,
    pub plots: &'static str,
    pub delivery_note: &'static str,
    pub lot: &'static str,
    pub buyer: &'static str,
    pub buyer_tax_id: &'static str,
    pub buyer_address: &'static str,
    pub buyer_registry: &'static str,
    pub note_plots: &'static str,
    pub note_voluntary: &'static str,
}

/// Section 6 — registro de fertilización. The second decree's other half
/// (RD 1051/2022 art. 5.d), so its notes cite that one and Anexo III sección C
/// rather than RD 1311/2012 art. 16.
#[derive(Serialize)]
pub struct S6 {
    pub section_title: &'static str,
    pub title: &'static str,
    pub dates: &'static str,
    pub plots: &'static str,
    pub area: &'static str,
    pub crop: &'static str,
    pub material: &'static str,
    pub delivery_note: &'static str,
    pub richness: &'static str,
    pub dose: &'static str,
    /// The model's "Tipo de fertilización (F)/(AF)/(AC)" column. It merges two
    /// separate legal fields, so the printed cell carries the sigla the model
    /// defines AND the forma de aplicación it drops.
    pub kind: &'static str,
    /// Anexo III C.g and C.k, which the model has no column for: who actually
    /// spread it — the holding's machine, or a service company with its REGFER.
    pub applicator: &'static str,
    pub yield_estimated: &'static str,
    pub yield_final: &'static str,
    pub note_plots: &'static str,
    /// Why the sigla can read "F/AC": fertirrigación is a forma de aplicación
    /// (C.f), not a tipo de fertilización (C.c), and the two are independent.
    pub note_kind: &'static str,
    /// The model prints three richness figures where C.h asks for eight; the
    /// rest, the micronutrients and the sludge heavy metals live on the
    /// material's registry entry.
    pub note_richness: &'static str,
    /// C.i / art. 5.g — a sludge application is marked on the material cell,
    /// the model having no box for it.
    pub note_sludge: &'static str,
}

/// Section 7.1 — plan de abonado (RD 1051/2022 art. 4.2, 5.a and 6).
///
/// The table is ASSEMBLED: only the recommendation is stored, everything else
/// being section 6's own records seen again. Its notes say so, because a reader
/// comparing two numbers deserves to know which one the app computed.
#[derive(Serialize)]
pub struct S71 {
    pub section_title: &'static str,
    pub title: &'static str,
    pub plots: &'static str,
    pub crop: &'static str,
    pub date: &'static str,
    pub area: &'static str,
    pub fertiliser: &'static str,
    pub richness: &'static str,
    pub dose: &'static str,
    pub supplied: &'static str,
    pub accumulated: &'static str,
    pub recommended: &'static str,
    pub note_plots: &'static str,
    /// What a unidad fertilizante is — the model's own footnote 2.
    pub note_units: &'static str,
    /// Why two of the three blocks carry no stored number.
    pub note_assembled: &'static str,
    /// Why a cell can be blank: a volume dose with no density behind it cannot
    /// become kilograms, and the total stops rather than understating itself.
    pub note_unknown: &'static str,
    /// What the book does NOT hold: art. 6's plan document itself.
    pub note_document: &'static str,
}

/// Section 8 — registro de riego. The second decree's half of the book
/// (RD 1051/2022 art. 5.e), so its notes cite that one rather than RD 1311/2012.
#[derive(Serialize)]
pub struct S8 {
    pub section_title: &'static str,
    pub title: &'static str,
    pub plots: &'static str,
    pub area: &'static str,
    pub method: &'static str,
    pub dates: &'static str,
    pub volume: &'static str,
    pub cumulative: &'static str,
    pub water_quality: &'static str,
    pub source: &'static str,
    pub note_plots: &'static str,
    /// Why the cumulative column is filled in and not left to the reader: it is
    /// a running sum of this table, never a stored figure.
    pub note_cumulative: &'static str,
    /// Anexo III C.l read together with RD 1051/2022 art. 17.2 — the water's
    /// own nitrogen and phosphorus are recorded only when someone supplies them.
    pub note_water_quality: &'static str,
}

/// The closing page: the documents the holder must keep beside the book.
///
/// A duty, not a register — RD 1311/2012 art. 16.3 obliges keeping what backs
/// the entries for at least three years, and the book has no attachment
/// capability by design (the seam-4 scope decision). So this prints as a plain
/// list, never as tick boxes: boxes would make it look like something the
/// farmer fills in, when it is a reminder of what to file away.
///
/// The seven items are the printed model's, which is WIDER than art. 16.3:
/// `item_containers` (empty-container return receipts) and `item_sale`
/// (harvest delivery notes, food-chain traceability) rest on other duties. The
/// retention sentence therefore cites art. 16.3 for the three years without
/// claiming all seven come from it.
#[derive(Serialize)]
pub struct Annex {
    pub section_title: &'static str,
    pub title: &'static str,
    pub intro: &'static str,
    pub item_invoices: &'static str,
    pub item_contracts: &'static str,
    pub item_inspections: &'static str,
    pub item_containers: &'static str,
    pub item_analyses: &'static str,
    pub item_advice: &'static str,
    pub item_sale: &'static str,
    /// The second decree's own documents (RD 1051/2022). They are NOT in the
    /// printed model, which predates it: art. 6 makes the plan de abonado a
    /// document to keep, art. 5.g the sludge application document issued by
    /// the authorised manager, and art. 13.2 the agronomic-quality document
    /// that must accompany manure supplied by someone else — the last needed
    /// only then, since a holder supplying their own is explicitly exempt.
    pub item_plan: &'static str,
    pub item_sludge: &'static str,
    pub item_manure: &'static str,
    pub retention: &'static str,
}

/// Printed VALUES rather than headings: words the renderers emit for a piece of
/// data. Reached through the accessor methods below, never matched on directly.
#[derive(Serialize)]
pub struct Values {
    pub yes: &'static str,
    pub no: &'static str,
    /// The paper form's cross in 1.2's "Asesor" column.
    pub cross: &'static str,
    /// Model 3.1 footnote 3: an application with no equipment is "Manual" —
    /// a value the model defines, not missing data.
    pub manual: &'static str,
    /// The six IPM justifications of `JUSTIFICACION_ACTUACION`, in the
    /// authority's own wording — model 3.1 bis prints them where the form
    /// leaves a free-text cell.
    /// Intensity units for model 3.1 bis. Unlike a dose unit — `L/ha` reads
    /// the same in every language — a count is a WORD, so these translate
    /// rather than printing as symbols.
    pub unit_traps: &'static str,
    pub unit_traps_ha: &'static str,
    pub unit_diffusers: &'static str,
    pub unit_diffusers_ha: &'static str,
    pub unit_units: &'static str,
    pub unit_units_ha: &'static str,
    pub justification_threshold: &'static str,
    pub justification_monitoring: &'static str,
    pub justification_dss: &'static str,
    pub justification_authority: &'static str,
    pub justification_advisor: &'static str,
    pub justification_alert_device: &'static str,
    pub efficacy_good: &'static str,
    pub efficacy_fair: &'static str,
    pub efficacy_poor: &'static str,
    pub level_basic: &'static str,
    pub level_qualified: &'static str,
    pub level_fumigator: &'static str,
    pub level_pilot: &'static str,
    pub zone_nitrate: &'static str,
    pub zone_phyto: &'static str,
    pub zone_natura: &'static str,
    /// 2.2: the checked negative — "the question was asked and the answer was no".
    pub no_affection: &'static str,
    /// Joins a zone summary to the campaign it was checked in.
    pub campaign_word: &'static str,
    /// 2.2's water half: the same kind of checked negative, stated by the
    /// farmer rather than found by a service — "there is no abstraction point
    /// on or near this plot, and I looked".
    pub no_water_points: &'static str,
    /// Section 4's "Material analizado" — FEGA's own four-value wording, which
    /// separates the standing crop from the produce taken off it; the model's
    /// parenthetical "(vegetal / tierra / agua)" cannot express that.
    pub material_crop: &'static str,
    pub material_harvested_produce: &'static str,
    pub material_soil: &'static str,
    pub material_water: &'static str,
    /// Section 4's kinds of analysis. The printed model has no column for them,
    /// so they ride in the material cell and get their own spreadsheet column.
    pub analysis_residues: &'static str,
    pub analysis_microbiological: &'static str,
    pub analysis_heavy_metals: &'static str,
    pub analysis_nutrients: &'static str,
    pub analysis_soil_parameters: &'static str,
    pub analysis_gmo: &'static str,
    /// Section 3.2's "where was this seed treated" — same situation: no printed
    /// column, so it rides in the product cell.
    pub seed_kind_on_farm: &'static str,
    pub seed_kind_processing_centre: &'static str,
    pub seed_kind_purchased_es: &'static str,
    pub seed_kind_purchased_abroad: &'static str,
    /// Section 8's eight irrigation systems (FEGA SIST_RIEGO, and the model's
    /// own footnote). Distinct from 2.1's four-value plot vocabulary above:
    /// that one characterises the parcel, these describe one watering.
    pub method_surface_gravity: &'static str,
    pub method_sprinkler_fixed: &'static str,
    pub method_sprinkler_mobile: &'static str,
    pub method_micro_sprinkler: &'static str,
    pub method_misting: &'static str,
    pub method_drip: &'static str,
    pub method_hydroponic_open: &'static str,
    pub method_hydroponic_recirculating: &'static str,
    /// Section 8's water sources (FEGA ORIGEN_AGUA_RIEGO).
    pub water_surface: &'static str,
    pub water_groundwater: &'static str,
    pub water_rainwater: &'static str,
    pub water_reclaimed: &'static str,
    pub water_desalinated: &'static str,
    pub water_alternative: &'static str,
    /// Section 6's tipo de fertilización (FEGA TIPO_FERITILIZACION).
    pub fertilisation_base: &'static str,
    pub fertilisation_top: &'static str,
    pub fertilisation_amendment: &'static str,
    /// Section 6's forma de aplicación (FEGA METODO_APLICACION_FERTILIZANTE).
    pub application_broadcast: &'static str,
    pub application_broadcast_buried: &'static str,
    pub application_banded: &'static str,
    pub application_banded_buried: &'static str,
    pub application_fertigation_sprinkler: &'static str,
    pub application_fertigation_localised: &'static str,
    pub application_foliar: &'static str,
    /// What a sludge application is called where it rides in the material cell.
    pub sludge_mark: &'static str,
    /// What the manure received before it was spread (FEGA TRAT_ESTIERCOLES).
    pub manure_none: &'static str,
    pub manure_solid_fraction: &'static str,
    pub manure_liquid_fraction: &'static str,
    pub manure_ndn: &'static str,
    pub manure_composting: &'static str,
    pub manure_anaerobic: &'static str,
    pub manure_solar_drying: &'static str,
    pub manure_stripping: &'static str,
    pub manure_membrane: &'static str,
    /// The three nutrient catalogues, named in the material tab.
    pub nutrient_macro: &'static str,
    pub nutrient_micro: &'static str,
    pub nutrient_heavy_metal: &'static str,
    /// The model's own siglas for the tipo de fertilización column. Codes, not
    /// prose: they print the same in every language, like SEC/ASP/LOC/GRA.
    pub sigla_base: &'static str,
    pub sigla_top: &'static str,
    pub sigla_fertigation: &'static str,
    pub phi_days: &'static str,
    pub phi_until: &'static str,
}

#[derive(Serialize)]
pub struct SheetLabels {
    pub tab_farm: &'static str,
    pub tab_operators: &'static str,
    pub tab_machinery: &'static str,
    pub tab_advisors: &'static str,
    pub tab_plots: &'static str,
    pub tab_zones: &'static str,
    /// Model 3.1 bis's two extra measurements, as their own sheet columns.
    pub intensity_unit: &'static str,
    pub measure_registration: &'static str,
    pub tab_treatments: &'static str,
    pub tab_seed: &'static str,
    pub tab_postharvest: &'static str,
    pub tab_storage: &'static str,
    pub tab_transport: &'static str,
    pub tab_analysis: &'static str,
    pub tab_harvest: &'static str,
    /// 2.2's water half gets a tab of its own: the section's own row is per
    /// plot, so several points on one plot join into a string there — and a
    /// joined string cannot be sorted, filtered or summed, which is the whole
    /// point of the sheet. Here each point is a row with typed cells.
    pub tab_water_points: &'static str,
    /// Section 8's own tab. The PDF joins a record's water sources into one
    /// cell and prints a running total; here the sources are one filterable
    /// column and every number is a real number.
    pub tab_irrigation: &'static str,
    /// Section 6's own tab, plus the tab holding the material registry the
    /// records point at — the full Anexo III C.h composition has no room in a
    /// register row and every figure there is a number worth filtering.
    pub tab_fertilisation: &'static str,
    /// Section 7.1's own tab, where every UF is a real number.
    pub tab_plan: &'static str,
    /// Anexo III A.3's soil block. Its own tab because the PDF joins nine
    /// figures into one cell — the printed model has no soil page — and a
    /// joined string can be neither compared across campaigns nor charted.
    pub tab_soil: &'static str,
    pub soil_ph: &'static str,
    pub soil_organic_matter: &'static str,
    pub soil_p: &'static str,
    pub soil_k: &'static str,
    pub soil_n: &'static str,
    pub soil_conductivity: &'static str,
    pub soil_sand: &'static str,
    pub soil_silt: &'static str,
    pub soil_clay: &'static str,
    /// 7.1's three blocks, split into nine columns: the PDF joins each block
    /// into one cell because the model does, and a joined string can be
    /// neither compared nor charted.
    pub supplied_n: &'static str,
    pub supplied_p2o5: &'static str,
    pub supplied_k2o: &'static str,
    pub accumulated_n: &'static str,
    pub accumulated_p2o5: &'static str,
    pub accumulated_k2o: &'static str,
    pub recommended_n: &'static str,
    pub recommended_p2o5: &'static str,
    pub recommended_k2o: &'static str,
    pub tab_materials: &'static str,
    /// Columns the sheet adds because the PDF folds them into a neighbouring
    /// cell (the analysis-kinds precedent): the material's coded kind, the two
    /// legal fields the model's single letter merges, the sludge flag, and the
    /// good practices the printed model has no column for at all.
    pub material_kind: &'static str,
    pub fertilisation_type: &'static str,
    pub application_method: &'static str,
    pub sludge: &'static str,
    pub practices: &'static str,
    pub service_company: &'static str,
    pub service_regfer: &'static str,
    pub richness_n: &'static str,
    pub richness_p2o5: &'static str,
    pub richness_k2o: &'static str,
    pub nutrient_group: &'static str,
    pub nutrient: &'static str,
    pub percentage: &'static str,
    pub supplier: &'static str,
    pub supplier_registry: &'static str,
    pub manure_treatment: &'static str,
    pub density: &'static str,
    /// The PDF folds Anexo III C.l's two water-quality figures into one cell,
    /// because the model has no column for either. The sheet gives each its
    /// own, so a reader can filter and compare them — the analysis-kinds
    /// precedent.
    pub water_nitric_n: &'static str,
    pub water_soluble_p2o5: &'static str,
    pub field: &'static str,
    pub value: &'static str,
    pub campaign: &'static str,
    pub generated_on: &'static str,
    pub representative_prefix: &'static str,
    pub registry_regional: &'static str,
    pub operator_name: &'static str,
    pub gip_code: &'static str,
    pub plot_province: &'static str,
    pub plot_municipality: &'static str,
    /// The municipality NAME, its own column here while the PDF folds it
    /// into the code cell: a joined "122 · Íscar" can be read but not
    /// filtered, which is the whole point of the spreadsheet.
    pub plot_municipality_name: &'static str,
    pub plot_aggregate: &'static str,
    pub plot_polygon: &'static str,
    pub plot_parcel: &'static str,
    pub plot_enclosure: &'static str,
    pub sigpac_area: &'static str,
    pub cultivated_area: &'static str,
    pub plot_id: &'static str,
    /// 3.1's cross-reference column. Its own entry rather than the PDF's
    /// header, which the model capitalises differently ("Id. Parcelas").
    pub plot_ids: &'static str,
    pub water_point: &'static str,
    /// The 2.2 tab splits the PDF's single coordinate cell, so each number
    /// sorts on its own and can be fed to a map or a GPS.
    pub latitude: &'static str,
    pub longitude: &'static str,
    pub plots: &'static str,
    /// The sheet spells out what the PDF abbreviates to fit its column.
    pub treated_area: &'static str,
    pub operator_order: &'static str,
    pub operator: &'static str,
    pub equipment_order: &'static str,
    pub equipment: &'static str,
    pub registration: &'static str,
    pub dose_unit: &'static str,
    /// 3.1's interval, split in two so each end sorts and filters on its own.
    pub date_start: &'static str,
    pub date_end: &'static str,
    /// Reglamento (UE) 2023/564's two conditional fields, which the PDF folds
    /// into a neighbouring cell for want of a column in the Spanish model.
    /// "BBCH" is the monograph's own name and stays as it is in every language.
    pub growth_stage: &'static str,
    pub application_time: &'static str,
    pub total_quantity: &'static str,
    pub total_quantity_unit: &'static str,
    pub subject: &'static str,
    pub quantity: &'static str,
    pub quantity_unit: &'static str,
    pub product_quantity: &'static str,
    pub product_quantity_unit: &'static str,
    pub register_applies: &'static str,
    pub seed_quantity: &'static str,
    pub sown_area: &'static str,
    /// Columns the printed model has no room for: the PDF folds each into a
    /// neighbouring cell, the sheet gives them their own so they can be
    /// filtered on.
    pub analysis_types: &'static str,
    pub substances_coded: &'static str,
    pub seed_treatment_kind: &'static str,
    /// 3.3-3.5's advisor (Anexo III B.d), which the printed model has no column
    /// for: the PDF folds the pair into the applicator cell, the sheet splits
    /// them so a book can be filtered by who advised it.
    pub advisor_name: &'static str,
    pub advisor_registration: &'static str,
    /// Section 4's laboratory gets a column per field here; the PDF joins the
    /// three into the model's single "nombre y dirección" cell.
    pub lab_name: &'static str,
    pub lab_address: &'static str,
    pub lab_tax_id: &'static str,
    pub phi_days: &'static str,
    pub harvest_from: &'static str,
}

// ---------------------------------------------------------------------------
// Accessors — a schema code in, the printed word out
// ---------------------------------------------------------------------------

impl Labels {
    /// The model's footnote wording for efficacy (3.1 footnote 5). Blank when
    /// not yet assessed — efficacy is observed AFTER the application.
    pub fn efficacy(&self, code: Option<&str>) -> &'static str {
        match code {
            Some("good") => self.value.efficacy_good,
            Some("fair") => self.value.efficacy_fair,
            Some("poor") => self.value.efficacy_poor,
            _ => "",
        }
    }

    /// The IPM justification behind an actuation, for model 3.1 bis's
    /// "Justificación de la actuación" column. The printed model leaves that
    /// cell free text; we hold the coded `treatment_justification` rows the
    /// SIEX twin requires, so the column prints the resolved words and no
    /// second free-text field is captured for the same fact.
    ///
    /// Unknown codes print themselves — the `material_kind` rule.
    pub fn justification<'a>(&self, code: &'a str) -> &'a str {
        match code {
            "threshold_exceeded" => self.value.justification_threshold,
            "monitoring" => self.value.justification_monitoring,
            "decision_support_system" => self.value.justification_dss,
            "authority_warning" => self.value.justification_authority,
            "advisor_recommendation" => self.value.justification_advisor,
            "alert_device" => self.value.justification_alert_device,
            other => other,
        }
    }

    /// The unit a non-chemical measure's intensity is counted in. A count is
    /// prose, not a symbol, so it lives here and not in `unit_symbol` —
    /// "4 trampes/ha" on a Catalan page, not "4 Trampas/ha". Unknown codes
    /// print themselves (the `material_kind` rule), so a unit added upstream
    /// cannot blank a cell.
    pub fn intensity_unit<'a>(&self, code: &'a str) -> &'a str {
        match code {
            "traps" => self.value.unit_traps,
            "traps_ha" => self.value.unit_traps_ha,
            "diffusers" => self.value.unit_diffusers,
            "diffusers_ha" => self.value.unit_diffusers_ha,
            "units" => self.value.unit_units,
            "units_ha" => self.value.unit_units_ha,
            other => other,
        }
    }

    /// Section 4's "Material analizado". An unknown code prints ITSELF rather
    /// than nothing — the rule `zone` follows, and the reason this returns a
    /// borrowed `&'a str` instead of `&'static str`: with the old signature the
    /// fallback could only ever be `""`, so a fourth material added without an
    /// arm here would have left a blank cell in a legal document.
    pub fn material_kind<'a>(&self, code: &'a str) -> &'a str {
        match code {
            "crop" => self.value.material_crop,
            "harvested_produce" => self.value.material_harvested_produce,
            "soil" => self.value.material_soil,
            "water" => self.value.material_water,
            other => other,
        }
    }

    /// Section 8's "Sistema de riego". Unknown codes print themselves, the
    /// `material_kind` rule — a ninth system added upstream must not blank a
    /// cell in a legal document.
    pub fn irrigation_method<'a>(&self, code: &'a str) -> &'a str {
        match code {
            "surface_gravity" => self.value.method_surface_gravity,
            "sprinkler_fixed" => self.value.method_sprinkler_fixed,
            "sprinkler_mobile" => self.value.method_sprinkler_mobile,
            "micro_sprinkler" => self.value.method_micro_sprinkler,
            "misting" => self.value.method_misting,
            "drip" => self.value.method_drip,
            "hydroponic_open" => self.value.method_hydroponic_open,
            "hydroponic_recirculating" => self.value.method_hydroponic_recirculating,
            other => other,
        }
    }

    /// Section 6's "Tipo de fertilización" (Anexo III C.c). Unknown codes print
    /// themselves, the `material_kind` rule.
    pub fn fertilisation_type<'a>(&self, code: &'a str) -> &'a str {
        match code {
            "base_dressing" => self.value.fertilisation_base,
            "top_dressing" => self.value.fertilisation_top,
            "amendment" => self.value.fertilisation_amendment,
            other => other,
        }
    }

    /// What the manure received before it was spread (FEGA
    /// `TRAT_ESTIERCOLES`) — a lookup this app owns, so it prints as prose.
    /// Unknown codes print themselves, the `material_kind` rule.
    pub fn manure_treatment<'a>(&self, code: &'a str) -> &'a str {
        match code {
            "none" => self.value.manure_none,
            "solid_fraction" => self.value.manure_solid_fraction,
            "liquid_fraction" => self.value.manure_liquid_fraction,
            "ndn_effluent" => self.value.manure_ndn,
            "composting" => self.value.manure_composting,
            "anaerobic_digestion" => self.value.manure_anaerobic,
            "solar_drying" => self.value.manure_solar_drying,
            "stripping" => self.value.manure_stripping,
            "membrane_separation" => self.value.manure_membrane,
            other => other,
        }
    }

    /// Which of the three FEGA nutrient catalogues a composition figure came
    /// from — a group name in the material tab, never printed in the book
    /// itself. Unknown codes print themselves.
    pub fn nutrient_kind<'a>(&self, code: &'a str) -> &'a str {
        match code {
            "macro" => self.value.nutrient_macro,
            "micro" => self.value.nutrient_micro,
            "heavy_metal" => self.value.nutrient_heavy_metal,
            other => other,
        }
    }

    /// Section 6's "Forma de aplicación" (C.f) — the legal field the model's
    /// single "(F)/(AF)/(AC)" letter drops. Unknown codes print themselves.
    pub fn application_method<'a>(&self, code: &'a str) -> &'a str {
        match code {
            "broadcast" => self.value.application_broadcast,
            "broadcast_buried" => self.value.application_broadcast_buried,
            "banded" => self.value.application_banded,
            "banded_buried" => self.value.application_banded_buried,
            "fertigation_sprinkler" => self.value.application_fertigation_sprinkler,
            "fertigation_localised" => self.value.application_fertigation_localised,
            "foliar" => self.value.application_foliar,
            other => other,
        }
    }

    /// The model's sigla for one application — its footnote lists (F)
    /// fertirrigación, (AF) abonado de fondo and (AC) abonado de cobertera as
    /// though they were one list. They are not: F answers C.f and AF/AC answer
    /// C.c, so a fertigated cobertera is honestly "F/AC" and an enmienda has no
    /// sigla at all. Blank when the model defines none.
    pub fn fertilisation_sigla(&self, type_code: &str, fertigation: bool) -> String {
        let base = match type_code {
            "base_dressing" => self.value.sigla_base,
            "top_dressing" => self.value.sigla_top,
            _ => "",
        };
        match (fertigation, base.is_empty()) {
            (false, _) => base.to_string(),
            (true, true) => self.value.sigla_fertigation.to_string(),
            (true, false) => format!("{}/{}", self.value.sigla_fertigation, base),
        }
    }

    /// Section 8's "Procedencia del agua". Unknown codes print themselves.
    pub fn water_origin<'a>(&self, code: &'a str) -> &'a str {
        match code {
            "surface" => self.value.water_surface,
            "groundwater" => self.value.water_groundwater,
            "rainwater" => self.value.water_rainwater,
            "reclaimed" => self.value.water_reclaimed,
            "desalinated" => self.value.water_desalinated,
            "alternative" => self.value.water_alternative,
            other => other,
        }
    }

    /// What the laboratory looked for (model section 4 has no column for it —
    /// it rides in the material cell). Unknown codes print themselves.
    pub fn analysis_type<'a>(&self, code: &'a str) -> &'a str {
        match code {
            "pesticide_residues" => self.value.analysis_residues,
            "microbiological" => self.value.analysis_microbiological,
            "heavy_metals" => self.value.analysis_heavy_metals,
            "nutrients" => self.value.analysis_nutrients,
            "soil_parameters" => self.value.analysis_soil_parameters,
            "gmo_presence" => self.value.analysis_gmo,
            other => other,
        }
    }

    /// Where treated seed was treated (model section 3.2 has no column for it
    /// either). Blank when unstated, because the model does not ask.
    pub fn seed_treatment_kind<'a>(&self, code: Option<&'a str>) -> &'a str {
        match code {
            Some("on_farm") => self.value.seed_kind_on_farm,
            Some("processing_centre") => self.value.seed_kind_processing_centre,
            Some("purchased_es") => self.value.seed_kind_purchased_es,
            Some("purchased_abroad") => self.value.seed_kind_purchased_abroad,
            Some(other) => other,
            None => "",
        }
    }

    /// Carné de aplicador levels (RD 1311/2012 niveles de capacitación).
    pub fn licence_level(&self, code: Option<&str>) -> &'static str {
        match code {
            Some("basic") => self.value.level_basic,
            Some("qualified") => self.value.level_qualified,
            Some("fumigator") => self.value.level_fumigator,
            Some("pilot") => self.value.level_pilot,
            _ => "",
        }
    }

    /// Zone-type display names. An unknown code prints itself: a new zone kind
    /// must never silently vanish from a printed record book.
    pub fn zone<'a>(&self, code: &'a str) -> &'a str {
        match code {
            "nitrate_vulnerable" => self.value.zone_nitrate,
            "phytosanitary_restriction" => self.value.zone_phyto,
            "natura_2000" => self.value.zone_natura,
            other => other,
        }
    }

    /// "21 días (hasta 22/05/2026)" — the days applied AND the first day
    /// harvest is allowed again, because the model's column asks for both.
    pub fn phi_phrase(&self, days: i64, end_date: &str) -> String {
        format!(
            "{days} {} ({} {end_date})",
            self.value.phi_days, self.value.phi_until
        )
    }

    pub fn yes_no(&self, flag: bool) -> &'static str {
        if flag { self.value.yes } else { self.value.no }
    }
}

// ---------------------------------------------------------------------------
// Castilian — the official model's own wording, transcribed
// ---------------------------------------------------------------------------

static ES: Labels = Labels {
    doc: Doc {
        farm_owner: "Explotación / Titular",
        campaign: "CAMPAÑA",
        generated_on: "Documento generado el",
        page: "Hoja nº",
        page_of: "de",
    },
    s1: S1 {
        title: "1. INFORMACIÓN GENERAL",
        opening_date: "FECHA DE APERTURA DEL CUADERNO",
        general_title: "1.1 DATOS GENERALES DE LA EXPLOTACIÓN",
        owner_name: "Nombre y apellidos o razón social:",
        tax_id: "NIF:",
        registry_national: "Nº Registro de Explotaciones Nacional:",
        registry_regional: "Nº Registro de Explotaciones Autonómico:",
        address: "Dirección:",
        locality: "Localidad:",
        postal_code: "C. Postal:",
        province: "Provincia:",
        phone_fixed: "Teléfono fijo:",
        phone_mobile: "Teléfono móvil:",
        email: "e-mail:",
        farm_name: "Nombre de la explotación:",
        representative_title: "TITULAR O REPRESENTANTE DE LA EXPLOTACIÓN",
        full_name: "Nombre y apellidos:",
        representation_kind: "Tipo de representación:",
        phone: "Teléfono:",
        signature: "Firma del titular o representante de la explotación",
        date: "Fecha:",
        signature_note: "La persona firmante se hace responsable de la veracidad de los datos \
                         consignados en el presente cuaderno de explotación.",
    },
    s12: S12 {
        title: "1.2 PERSONAS O EMPRESAS QUE INTERVIENEN EN EL TRATAMIENTO CON PRODUCTOS \
                FITOSANITARIOS",
        order: "Nº de orden",
        name: "Nombre y apellidos / Empresa de servicios",
        tax_id: "NIF",
        licence_number: "Nº inscripción ROPO / nº carné",
        licence_level: "Tipo de carné",
        advisor: "Asesor",
        note: "Marcado cuando la persona figura además como asesor en el registro de asesores \
               (tabla 1.4); la condición de asesor se inscribe en el ROPO aparte del carné de \
               aplicador.",
    },
    s13: S13 {
        title: "1.3 EQUIPOS DE APLICACIÓN DE PRODUCTOS FITOSANITARIOS PROPIOS DE LA EXPLOTACIÓN",
        order: "Nº de orden",
        description: "Descripción del equipo",
        roma: "Nº inscrip. ROMA",
        reganip: "Nº inscrip. REGANIP",
        acquired_on: "Fecha de adquisición",
        last_inspection: "Fecha de la última inspección",
    },
    s14: S14 {
        title: "1.4 ASESOR, AGRUPACIÓN O ENTIDAD DE ASESORAMIENTO A LA QUE PERTENECE LA \
                EXPLOTACIÓN",
        name: "Nombre o razón social",
        tax_id: "NIF",
        registration_number: "Nº de identificación",
        gip: "Tipo de explotación",
        note: "(AE) Agricultura Ecológica, (PI) Producción Integrada, (CP) Certificación Privada, \
               (Atrias) Agrupación de Tratamiento Integrado en Agricultura, (AS) Asistida de un \
               asesor, (NO) Sin obligación de disponer de asesor en GIP.",
    },
    s21: S21 {
        section_title: "2. IDENTIFICACIÓN DE LAS PARCELAS DE LA EXPLOTACIÓN",
        title: "2.1 DATOS IDENTIFICATIVOS Y AGRONÓMICOS DE LAS PARCELAS",
        order: "Nº de orden",
        plot: "Parcela",
        province: "Prov.",
        municipality: "Mun.",
        aggregate: "Agr.",
        zone: "Zona",
        polygon: "Pol.",
        parcel: "Parc.",
        enclosure: "Rec.",
        land_use: "Uso SIGPAC",
        sigpac_area: "Superf. SIGPAC (ha)",
        cultivated_area: "Superf. cultivada (ha)",
        species: "Especie",
        variety: "Variedad",
        irrigation: "Secano / Regadío",
        environment: "Aire libre o protegido",
        gip: "GIP",
        note_gip: "Sistema de gestión integrada de plagas del cultivo, con las siglas de la tabla \
                   1.4; en blanco cuando no consta.",
        note_area: "Superficie de la parcela dedicada a este cultivo; en blanco cuando la parcela \
                    tiene varios cultivos y el reparto no consta.",
        note_irrigation: "(SEC) secano, (ASP) aspersión, (LOC) goteo o localizado, (GRA) por \
                          gravedad.",
        note_environment: "(AL) aire libre, (M) malla, (BP) cubierta bajo plástico, (INV) \
                           invernadero.",
    },
    s22: S22 {
        title: "2.2 DATOS IDENTIFICATIVOS MEDIOAMBIENTALES DE LAS PARCELAS",
        order: "Id. parcelas",
        species: "Especie",
        variety: "Variedad",
        water_point: "Captación incluida en la parcela (SÍ/NO)",
        distance: "Distancia (m)",
        coordinates: "Coordenadas (lat, lon)",
        denomination: "Denominación",
        fully: "Zonas específicas: totalmente",
        partly: "Parcialmente",
        checked: "Comprobación",
        note: "Resultado de la comprobación automática frente a las capas oficiales de SIGPAC, \
               con la campaña consultada. \"Sin afección\" acredita que la comprobación se hizo y \
               resultó negativa; una celda en blanco indica que la parcela aún no se ha \
               comprobado. En las captaciones de agua para consumo humano, \"Sin captaciones\" \
               acredita igualmente que se comprobó y no las hay.",
    },
    s31: S31 {
        section_title: "3. INFORMACIÓN SOBRE TRATAMIENTOS FITOSANITARIOS",
        title: "3.1 REGISTRO DE ACTUACIONES FITOSANITARIAS DE LA PARCELA",
        plots: "Id. Parcelas",
        species: "Especie",
        variety: "Variedad",
        date: "Intervalo de fechas",
        surface: "Superf. tratada (ha)",
        problem: "Problema fitosanitario",
        operator: "Aplicador",
        equipment: "Equipo",
        product: "Nombre comercial",
        registration: "Nº Registro",
        dose: "Dosis",
        total_quantity: "Cantidad total",
        phi: "Plazo de seguridad",
        efficacy: "Eficacia",
        notes: "Observaciones",
        note_plots: "Nº de orden de las parcelas tratadas según la tabla 2.1.",
        note_operator: "Nº de orden según la tabla 1.2.",
        note_equipment: "Nº de orden según la tabla 1.3; \"Manual\" cuando la aplicación no \
                         empleó equipo.",
        note_phi: "Días de plazo aplicados y primer día en que la cosecha vuelve a estar \
                   permitida.",
        note_efficacy: "Buena, regular o mala, observada tras la aplicación.",
        note_date: "Fecha de la actuación, o intervalo cuando se realizó en varios días; el \
                    plazo de seguridad se cuenta desde el último. La hora de inicio se anota \
                    cuando el uso del producto está restringido a determinadas horas del día \
                    (Reglamento (UE) 2023/564, anexo).",
        note_total_quantity: "Cantidad total de producto empleada en la actuación (kg o l).",
        note_growth_stage: "Estado fenológico del cultivo según la monografía BBCH, anotado \
                            cuando el uso del producto está restringido a determinados estados \
                            (Reglamento (UE) 2023/564, anexo). Se indica junto a la especie \
                            porque el modelo oficial no tiene columna propia.",
    },
    s31bis: S31Bis {
        title: "3.1 bis REGISTRO DE ACTUACIONES FITOSANITARIAS POR PARCELA",
        subtitle: "(SOLAMENTE PARA CULTIVOS Y SUPERFICIES OBJETO DE ASESORAMIENTO)",
        crop_group: "CULTIVO",
        plot_group: "DATOS DE LA PARCELA",
        problem_group: "PLAGA A CONTROLAR",
        non_chemical_group: "ALTERNATIVAS NO QUÍMICAS DE INTERVENCIÓN",
        chemical_group: "ALTERNATIVAS QUÍMICAS DE INTERVENCIÓN",
        species: "Especie",
        variety: "Variedad",
        plots: "Id. Parcelas",
        crop_surface: "Superf. cultivada (ha)",
        treated_surface: "Superf. tratada (ha)",
        problem: "Plaga",
        justification: "Justificación de la actuación",
        measure: "Tipo de medida",
        intensity: "Intensidad de la medida",
        measure_date: "Fecha actuación",
        product: "Nombre comercial / Sustancia activa",
        registration: "Nº Registro",
        dose: "Dosis utilizada",
        product_date: "Fecha actuación",
        efficacy: "Eficacia de la intervención",
        notes: "Observaciones",
        note_plots: "Nº de orden de las parcelas tratadas según la tabla 2.1.",
        note_intensity: "Nº de trampas, nº de difusores, etc.",
        validation_interim: "VALIDACIÓN INTERMEDIA",
        validation_final: "VALIDACIÓN FINAL",
        signature: "Firma",
        advisor: "Asesor",
        ropo: "Nº Inscripción ROPO",
        date: "Fecha",
        season_end_date: "Fecha fin de campaña",
    },
    s32: S32 {
        title: "3.2 REGISTRO DE USO DE SEMILLA TRATADA",
        plots: "Id. Parcelas",
        date: "Fecha de siembra",
        species: "Especie",
        variety: "Variedad",
        surface: "Superf. sembrada (ha)",
        seed_quantity: "Cantidad de semilla (kg)",
        seed_lot: "Nº de lote",
        product: "Nombre comercial",
        registration: "Nº Registro",
        active_substance: "Materia activa",
        efficacy: "Eficacia",
        notes: "Observaciones",
        note_plots: "Nº de orden de las parcelas sembradas según la tabla 2.1.",
        note_seed_lot: "Lote que figura en el envase de la semilla; es lo que permite \
                        rastrear el tratamiento hasta el proveedor.",
    },
    s33: S33 {
        applies: "APLICA TRATAMIENTO",
        title_postharvest: "3.3 REGISTRO DE TRATAMIENTOS POSTCOSECHA",
        title_storage: "3.4 REGISTRO DE TRATAMIENTOS EN LOCALES DE ALMACENAMIENTO",
        title_transport: "3.5 REGISTRO DE TRATAMIENTOS EN MEDIOS DE TRANSPORTE",
        subject_postharvest: "Producto vegetal tratado",
        subject_storage: "Local tratado (tipo y dirección)",
        subject_transport: "Vehículo tratado (tipo, modelo y matrícula)",
        quantity_postharvest: "Cantidad (t)",
        quantity_storage: "Volumen (m³)",
        quantity_transport: "Volumen (m³)",
        date: "Fecha",
        problem: "Problemática fitosanitaria",
        operator: "Aplicador",
        product: "Nombre comercial",
        registration: "Nº Registro",
        product_quantity: "Cantidad utilizada",
        efficacy: "Eficacia",
        notes: "Observaciones",
        note_applies: "Marque NO cuando en la campaña no se haya realizado ningún tratamiento \
                       de este tipo; queda constancia de que se ha comprobado.",
        note_product_quantity: "Cantidad de producto empleada, en kg o l.",
    },
    s4: S4 {
        section_title: "4. REGISTRO DE ANÁLISIS",
        title: "4.1 ANÁLISIS REALIZADOS",
        date: "Fecha",
        material: "Material analizado",
        plots: "Cultivo o cosecha muestreados",
        bulletin: "Nº boletín de análisis",
        laboratory: "Laboratorio (nombre y dirección)",
        substances: "Sustancias activas detectadas",
        note_plots: "Nº de orden de las parcelas de la tabla 2.1.",
        note_keep: "Los boletines de análisis se conservan con el resto de la documentación \
                    de la explotación durante al menos tres años (art. 16.3 del RD 1311/2012); \
                    en este registro consta dónde encontrarlos.",
        note_soil: "El modelo impreso es anterior al apartado A.3 del Anexo III y no tiene página de suelo: los parámetros analizados se recogen junto a los resultados. La hoja de cálculo les dedica una pestaña, con cada valor en su columna.",
    },
    s5: S5 {
        section_title: "5. REGISTRO DE COSECHA COMERCIALIZADA",
        title: "5.1 SALIDAS DE COSECHA",
        date: "Fecha",
        product: "Producto",
        quantity: "Cantidad",
        plots: "Parcelas de origen",
        delivery_note: "Nº albarán o factura",
        lot: "Nº de lote",
        buyer: "Cliente (nombre o razón social)",
        buyer_tax_id: "NIF",
        buyer_address: "Dirección",
        buyer_registry: "Nº RGSEAA",
        note_plots: "Nº de orden de las parcelas de la tabla 2.1.",
        note_voluntary: "El nº de albarán o factura, el nº de lote y el nº de RGSEAA son \
                         voluntarios.",
    },
    s6: S6 {
        section_title: "6. REGISTRO DE FERTILIZACIÓN",
        title: "6.1 APLICACIONES REALIZADAS",
        dates: "Fecha / intervalo",
        plots: "Id. parcelas",
        area: "Sup. (ha)",
        crop: "Cultivo",
        material: "Tipo de abono / producto",
        delivery_note: "Nº de albarán",
        richness: "Riqueza N / P₂O₅ / K₂O (%)",
        dose: "Dosis",
        kind: "Tipo de fertilización",
        applicator: "Maquinaria / empresa de servicios",
        yield_estimated: "Producción estimada (kg/ha)",
        yield_final: "Producción final (kg/ha)",
        note_plots: "Nº de orden de las parcelas de la tabla 2.1.",
        note_kind: "Siglas del modelo: (AF) abonado de fondo, (AC) abonado de cobertera, \
                    (F) fertirrigación. La fertirrigación es una forma de aplicación y no \
                    un tipo de fertilización, de modo que ambas siglas pueden concurrir; \
                    la aplicación de enmiendas no tiene sigla en el modelo.",
        note_richness: "El Anexo III, sección C, apartado h, del RD 1311/2012 pide hasta \
                        ocho valores agronómicos del material; el modelo imprime tres. Los \
                        demás, los micronutrientes y los metales pesados de los lodos \
                        constan en la ficha del material fertilizante.",
        note_sludge: "La aplicación de lodos de depuradora se indica junto al material \
                      (RD 1051/2022, art. 5.g).",
    },
    s71: S71 {
        section_title: "7. FERTILIZACIÓN",
        title: "7.1 PLAN DE ABONADO",
        plots: "Id. parcelas",
        crop: "Cultivo",
        date: "Fecha",
        area: "Sup. fertilizada (ha)",
        fertiliser: "Fertilizante",
        richness: "Riqueza N / P₂O₅ / K₂O (%)",
        dose: "Dosis",
        supplied: "UF aportadas (N / P₂O₅ / K₂O)",
        accumulated: "UF acumuladas (N / P₂O₅ / K₂O)",
        recommended: "UF recomendadas (N / P₂O₅ / K₂O)",
        note_plots: "Nº de orden de las parcelas de la tabla 2.1.",
        note_units: "Una unidad fertilizante (UF) es un kilogramo por hectárea \
                     de N, de P₂O₅ o de K₂O.",
        note_assembled: "Las unidades aportadas y acumuladas se calculan a partir del \
                         registro de fertilización (sección 6): aportadas = dosis × riqueza, \
                         acumuladas = su suma corrida en cada unidad de producción. Solo las \
                         recomendadas proceden del plan (RD 1051/2022, art. 5.a).",
        note_unknown: "Una dosis en volumen solo se convierte en kilogramos si el material \
                       tiene densidad anotada; sin ella la casilla queda en blanco y la \
                       columna acumulada se interrumpe, en lugar de dar un total incompleto.",
        note_document: "El plan en sí —identificación de recintos, datos de suelo, agua \
                        disponible, dosis y momento de aplicación, tipo de material, forma de \
                        aplicación, maquinaria y medidas del anexo V— es un documento que se \
                        conserva junto al cuaderno (RD 1051/2022, art. 6).",
    },
    s8: S8 {
        section_title: "8. RIEGO",
        title: "8.1 REGISTRO DE RIEGO",
        plots: "Id. parcelas",
        area: "Superficie regada (ha)",
        method: "Sistema de riego",
        dates: "Fecha / intervalo de riego",
        volume: "Volumen de riego",
        cumulative: "Volumen acumulado (m³/ha)",
        water_quality: "N nítrico / P₂O₅ soluble en el agua (mg/l)",
        source: "Procedencia del agua",
        note_plots: "Nº de orden de las parcelas de la tabla 2.1.",
        note_cumulative: "El volumen acumulado es la suma corrida de esta misma tabla, \
                          expresada en m³/ha; solo se acumulan los riegos anotados en \
                          esa unidad.",
        note_water_quality: "Contenido de nitrógeno nítrico y de fósforo \
                             (P₂O₅) soluble del agua de riego. Se anota cuando el \
                             organismo de cuenca, la comunidad de regantes u organismo \
                             equivalente facilita el dato; con analíticas propias es \
                             voluntario (RD 1051/2022, art. 17.2).",
    },
    annex: Annex {
        section_title: "DOCUMENTACIÓN A CONSERVAR",
        title: "DOCUMENTACIÓN QUE DEBE CONSERVARSE JUNTO AL CUADERNO",
        intro: "Además de este cuaderno, la persona titular conserva a disposición de la \
                autoridad competente:",
        item_invoices: "Facturas y documentos de adquisición de los productos fitosanitarios \
                        utilizados.",
        item_contracts: "Contratos con las empresas o personas que realizaron los tratamientos, \
                         cuando no los realizó la propia explotación.",
        item_inspections: "Certificados de inspección de los equipos de aplicación.",
        item_containers: "Justificantes de entrega de los envases vacíos en un punto de \
                          recogida autorizado.",
        item_analyses: "Boletines de los análisis de residuos realizados sobre los cultivos y \
                        las producciones y, en su caso, sobre el agua de riego.",
        item_advice: "Documentación del asesoramiento recibido.",
        item_sale: "Albaranes o facturas de venta de la cosecha.",
        item_plan: "Plan de abonado de cada unidad de producción (RD 1051/2022, art. 6).",
        item_sludge: "Documento de aplicación de los lodos expedido por el gestor autorizado (RD 1051/2022, art. 5.g; anexo III de la Orden AAA/1072/2013).",
        item_manure: "Documento con la calidad agronómica de los estiércoles recibidos de terceros (RD 1051/2022, art. 13.2); no es necesario cuando los suministra el propio titular.",
        retention: "Los documentos 1 a 6 se conservan durante al menos tres años desde su \
                    emisión (art. 16.3 del RD 1311/2012). Los justificantes de entrega de \
                    envases vacíos (4) y los documentos de venta de la cosecha (7) responden, \
                    respectivamente, a la obligación de devolución de envases y a la \
                    trazabilidad alimentaria. Los documentos 8 a 10 los exige el RD \
                    1051/2022, que no fija para ellos un plazo propio de conservación.",
    },
    value: Values {
        yes: "SÍ",
        no: "NO",
        cross: "X",
        manual: "Manual",
        unit_traps: "trampas",
        unit_traps_ha: "trampas/ha",
        unit_diffusers: "difusores",
        unit_diffusers_ha: "difusores/ha",
        unit_units: "unidades",
        unit_units_ha: "unidades/ha",
        justification_threshold: "Superación de umbrales",
        justification_monitoring: "Monitorización",
        justification_dss: "Sistema de apoyo a la toma de decisión (DSS)",
        justification_authority: "Aviso por Comunidad Autónoma",
        justification_advisor: "Recomendación de asesor",
        justification_alert_device: "Medidor de alerta fitosanitaria",
        efficacy_good: "Buena",
        efficacy_fair: "Regular",
        efficacy_poor: "Mala",
        level_basic: "Básico",
        level_qualified: "Cualificado",
        level_fumigator: "Fumigador",
        level_pilot: "Piloto",
        zone_nitrate: "Vulnerable a nitratos",
        zone_phyto: "Restricción fitosanitaria",
        zone_natura: "Red Natura 2000",
        no_affection: "Sin afección",
        campaign_word: "campaña",
        no_water_points: "Sin captaciones",
        material_crop: "Cultivo",
        material_harvested_produce: "Producto cosechado",
        material_soil: "Suelo",
        material_water: "Agua de riego",
        analysis_residues: "Residuos de sustancias activas fitosanitarias",
        analysis_microbiological: "Microbiológico",
        analysis_heavy_metals: "Metales pesados",
        analysis_nutrients: "Nutrientes",
        analysis_soil_parameters: "Parámetros del suelo",
        analysis_gmo: "Presencia de OMG",
        seed_kind_on_farm: "Tratada en la explotación",
        seed_kind_processing_centre: "Tratada en un centro de acondicionamiento",
        seed_kind_purchased_es: "Adquirida tratada en España",
        seed_kind_purchased_abroad: "Adquirida tratada fuera de España",
        method_surface_gravity: "Superficie o gravedad",
        method_sprinkler_fixed: "Aspersión fija",
        method_sprinkler_mobile: "Aspersión móvil",
        method_micro_sprinkler: "Microaspersión",
        method_misting: "Nebulización",
        method_drip: "Goteo",
        method_hydroponic_open: "Hidroponía a solución perdida",
        method_hydroponic_recirculating: "Hidroponía con recirculación",
        water_surface: "Superficial",
        water_groundwater: "Subterránea",
        water_rainwater: "Pluvial",
        water_reclaimed: "Regeneración",
        water_desalinated: "Desalinización",
        water_alternative: "Recursos alternativos",
        fertilisation_base: "Abonado de fondo",
        fertilisation_top: "Abonado de cobertera",
        fertilisation_amendment: "Aplicación de enmienda",
        application_broadcast: "Esparcido general",
        application_broadcast_buried: "Esparcido general y enterrado",
        application_banded: "Esparcido localizado",
        application_banded_buried: "Esparcido localizado y enterrado",
        application_fertigation_sprinkler: "Riego por aspersión (fertirrigación)",
        application_fertigation_localised: "Riego localizado (fertirrigación)",
        application_foliar: "Aplicación foliar",
        sludge_mark: "aplicación de lodos",
        manure_none: "Ninguno",
        manure_solid_fraction: "Separación sólido-líquido: fracción sólida",
        manure_liquid_fraction: "Separación sólido-líquido: fracción líquida",
        manure_ndn: "Nitrificación-desnitrificación (NDN)",
        manure_composting: "Compostaje",
        manure_anaerobic: "Digestión anaerobia",
        manure_solar_drying: "Secado solar",
        manure_stripping: "Stripping",
        manure_membrane: "Separación por membranas",
        nutrient_macro: "Macronutrientes",
        nutrient_micro: "Micronutrientes",
        nutrient_heavy_metal: "Metales pesados",
        sigla_base: "AF",
        sigla_top: "AC",
        sigla_fertigation: "F",
        phi_days: "días",
        phi_until: "hasta",
    },
    sheet: SheetLabels {
        tab_farm: "1.1 Explotación",
        tab_operators: "1.2 Personas",
        tab_machinery: "1.3 Equipos",
        tab_advisors: "1.4 Asesoramiento",
        tab_plots: "2.1 Parcelas",
        tab_zones: "2.2 Medioambiental",
        intensity_unit: "Unidad de intensidad",
        measure_registration: "Nº registro medida",
        tab_treatments: "3.1 Tratamientos",
        tab_seed: "3.2 Semilla tratada",
        tab_postharvest: "3.3 Postcosecha",
        tab_storage: "3.4 Locales",
        tab_transport: "3.5 Transporte",
        tab_analysis: "4 Análisis",
        tab_harvest: "5 Cosecha",
        tab_water_points: "2.2 Captaciones",
        tab_irrigation: "8 Riego",
        tab_fertilisation: "6 Fertilización",
        tab_plan: "7.1 Plan de abonado",
        tab_soil: "4 Suelo",
        soil_ph: "pH",
        soil_organic_matter: "Materia orgánica (%)",
        soil_p: "P asimilable (mg/kg)",
        soil_k: "K asimilable (mg/kg)",
        soil_n: "N total (%)",
        soil_conductivity: "Conductividad (dS/m)",
        soil_sand: "Arena (%)",
        soil_silt: "Limo (%)",
        soil_clay: "Arcilla (%)",
        supplied_n: "UF aportadas N",
        supplied_p2o5: "UF aportadas P₂O₅",
        supplied_k2o: "UF aportadas K₂O",
        accumulated_n: "UF acumuladas N",
        accumulated_p2o5: "UF acumuladas P₂O₅",
        accumulated_k2o: "UF acumuladas K₂O",
        recommended_n: "UF recomendadas N",
        recommended_p2o5: "UF recomendadas P₂O₅",
        recommended_k2o: "UF recomendadas K₂O",
        tab_materials: "6 Materiales",
        material_kind: "Tipo de material",
        fertilisation_type: "Tipo de fertilización",
        application_method: "Forma de aplicación",
        sludge: "Aplicación de lodos",
        practices: "Buenas prácticas",
        service_company: "Empresa de servicios",
        service_regfer: "Nº REGFER",
        richness_n: "N total (%)",
        richness_p2o5: "P₂O₅ total (%)",
        richness_k2o: "K₂O (%)",
        nutrient_group: "Grupo",
        nutrient: "Nutriente",
        percentage: "%",
        supplier: "Empresa suministradora",
        supplier_registry: "REGA / NIF / NIMA",
        manure_treatment: "Tratamiento del estiércol",
        density: "Densidad (kg/l)",
        water_nitric_n: "N nítrico en el agua (mg/l)",
        water_soluble_p2o5: "P₂O₅ soluble en el agua (mg/l)",
        field: "Campo",
        value: "Valor",
        campaign: "Campaña",
        generated_on: "Documento generado el",
        representative_prefix: "Representante",
        registry_regional: "Nº Registro de Explotaciones Autonómico (REA)",
        operator_name: "Nombre y apellidos / Empresa",
        gip_code: "Código",
        plot_province: "Provincia",
        plot_municipality: "Municipio",
        plot_municipality_name: "Municipio (nombre)",
        plot_aggregate: "Agregado",
        plot_polygon: "Polígono",
        plot_parcel: "Parcela SIGPAC",
        plot_enclosure: "Recinto",
        sigpac_area: "Superficie SIGPAC (ha)",
        cultivated_area: "Superficie cultivada (ha)",
        plot_id: "Id. parcela",
        plot_ids: "Id. parcelas",
        water_point: "Captación incluida en la parcela",
        latitude: "Latitud",
        longitude: "Longitud",
        plots: "Parcelas",
        treated_area: "Superficie tratada (ha)",
        operator_order: "Nº aplicador",
        operator: "Aplicador",
        equipment_order: "Nº equipo",
        equipment: "Equipo",
        registration: "Nº registro",
        dose_unit: "Unidad de dosis",
        date_start: "Fecha inicio",
        date_end: "Fecha fin",
        growth_stage: "Estado fenológico (BBCH)",
        application_time: "Hora inicio",
        total_quantity: "Cantidad total",
        total_quantity_unit: "Unidad de cantidad",
        subject: "Objeto tratado",
        quantity: "Cantidad tratada",
        quantity_unit: "Unidad",
        product_quantity: "Cantidad utilizada",
        product_quantity_unit: "Unidad utilizada",
        register_applies: "Aplica tratamiento",
        seed_quantity: "Cantidad de semilla (kg)",
        sown_area: "Superficie sembrada (ha)",
        analysis_types: "Tipos de análisis",
        substances_coded: "Sustancias (catálogo)",
        seed_treatment_kind: "Tratamiento de la semilla",
        advisor_name: "Asesor",
        advisor_registration: "Nº ROPO asesor",
        lab_name: "Laboratorio",
        lab_address: "Dirección del laboratorio",
        lab_tax_id: "NIF del laboratorio",
        phi_days: "Plazo de seguridad (días)",
        harvest_from: "Cosecha permitida desde",
    },
};

// ---------------------------------------------------------------------------
// Catalan — same form, same legal vocabulary, Catalan wording
// ---------------------------------------------------------------------------

static CA: Labels = Labels {
    doc: Doc {
        farm_owner: "Explotació / Titular",
        campaign: "CAMPANYA",
        generated_on: "Document generat el",
        page: "Full núm.",
        page_of: "de",
    },
    s1: S1 {
        title: "1. INFORMACIÓ GENERAL",
        opening_date: "DATA D'OBERTURA DEL QUADERN",
        general_title: "1.1 DADES GENERALS DE L'EXPLOTACIÓ",
        owner_name: "Nom i cognoms o raó social:",
        tax_id: "NIF:",
        registry_national: "Núm. Registre d'Explotacions Nacional:",
        registry_regional: "Núm. Registre d'Explotacions Autonòmic:",
        address: "Adreça:",
        locality: "Localitat:",
        postal_code: "C. Postal:",
        province: "Província:",
        phone_fixed: "Telèfon fix:",
        phone_mobile: "Telèfon mòbil:",
        email: "e-mail:",
        farm_name: "Nom de l'explotació:",
        representative_title: "TITULAR O REPRESENTANT DE L'EXPLOTACIÓ",
        full_name: "Nom i cognoms:",
        representation_kind: "Tipus de representació:",
        phone: "Telèfon:",
        signature: "Signatura del titular o representant de l'explotació",
        date: "Data:",
        signature_note: "La persona signant es fa responsable de la veracitat de les dades \
                         consignades en aquest quadern d'explotació.",
    },
    s12: S12 {
        title: "1.2 PERSONES O EMPRESES QUE INTERVENEN EN EL TRACTAMENT AMB PRODUCTES \
                FITOSANITARIS",
        order: "Núm. d'ordre",
        name: "Nom i cognoms / Empresa de serveis",
        tax_id: "NIF",
        licence_number: "Núm. inscripció ROPO / núm. carnet",
        licence_level: "Tipus de carnet",
        advisor: "Assessor",
        note: "Marcat quan la persona figura també com a assessor en el registre d'assessors \
               (taula 1.4); la condició d'assessor s'inscriu al ROPO a part del carnet \
               d'aplicador.",
    },
    s13: S13 {
        title: "1.3 EQUIPS D'APLICACIÓ DE PRODUCTES FITOSANITARIS PROPIS DE L'EXPLOTACIÓ",
        order: "Núm. d'ordre",
        description: "Descripció de l'equip",
        roma: "Núm. inscrip. ROMA",
        reganip: "Núm. inscrip. REGANIP",
        acquired_on: "Data d'adquisició",
        last_inspection: "Data de l'última inspecció",
    },
    s14: S14 {
        title: "1.4 ASSESSOR, AGRUPACIÓ O ENTITAT D'ASSESSORAMENT A LA QUAL PERTANY \
                L'EXPLOTACIÓ",
        name: "Nom o raó social",
        tax_id: "NIF",
        registration_number: "Núm. d'identificació",
        gip: "Tipus d'explotació",
        note: "(AE) Agricultura Ecològica, (PI) Producció Integrada, (CP) Certificació Privada, \
               (Atrias) Agrupació de Tractament Integrat en Agricultura, (AS) Assistida d'un \
               assessor, (NO) Sense obligació de disposar d'assessor en GIP.",
    },
    s21: S21 {
        section_title: "2. IDENTIFICACIÓ DE LES PARCEL·LES DE L'EXPLOTACIÓ",
        title: "2.1 DADES IDENTIFICATIVES I AGRONÒMIQUES DE LES PARCEL·LES",
        order: "Núm. d'ordre",
        plot: "Parcel·la",
        province: "Prov.",
        municipality: "Mun.",
        aggregate: "Agr.",
        zone: "Zona",
        polygon: "Pol.",
        parcel: "Parc.",
        enclosure: "Rec.",
        land_use: "Ús SIGPAC",
        sigpac_area: "Superf. SIGPAC (ha)",
        cultivated_area: "Superf. cultivada (ha)",
        species: "Espècie",
        variety: "Varietat",
        irrigation: "Secà / Regadiu",
        environment: "Aire lliure o protegit",
        gip: "GIP",
        note_gip: "Sistema de gestió integrada de plagues del cultiu, amb les sigles de la taula \
                   1.4; en blanc quan no hi consta.",
        note_area: "Superfície de la parcel·la dedicada a aquest cultiu; en blanc quan la \
                    parcel·la té diversos cultius i el repartiment no hi consta.",
        note_irrigation: "(SEC) secà, (ASP) aspersió, (LOC) degoteig o localitzat, (GRA) per \
                          gravetat.",
        note_environment: "(AL) aire lliure, (M) malla, (BP) coberta sota plàstic, (INV) \
                           hivernacle.",
    },
    s22: S22 {
        title: "2.2 DADES IDENTIFICATIVES MEDIAMBIENTALS DE LES PARCEL·LES",
        order: "Id. parcel·les",
        species: "Espècie",
        variety: "Varietat",
        water_point: "Captació inclosa a la parcel·la (SÍ/NO)",
        distance: "Distància (m)",
        coordinates: "Coordenades (lat, lon)",
        denomination: "Denominació",
        fully: "Zones específiques: totalment",
        partly: "Parcialment",
        checked: "Comprovació",
        note: "Resultat de la comprovació automàtica contra les capes oficials del SIGPAC, amb la \
               campanya consultada. \"Sense afectació\" acredita que la comprovació es va fer i \
               va resultar negativa; una cel·la en blanc indica que la parcel·la encara no s'ha \
               comprovat. En les captacions d'aigua per a consum humà, \"Sense captacions\" \
               acredita igualment que es va comprovar i no n'hi ha.",
    },
    s31: S31 {
        section_title: "3. INFORMACIÓ SOBRE TRACTAMENTS FITOSANITARIS",
        title: "3.1 REGISTRE D'ACTUACIONS FITOSANITÀRIES DE LA PARCEL·LA",
        plots: "Id. Parcel·les",
        species: "Espècie",
        variety: "Varietat",
        date: "Interval de dates",
        surface: "Superf. tractada (ha)",
        problem: "Problema fitosanitari",
        operator: "Aplicador",
        equipment: "Equip",
        product: "Nom comercial",
        registration: "Núm. Registre",
        dose: "Dosi",
        total_quantity: "Quantitat total",
        phi: "Termini de seguretat",
        efficacy: "Eficàcia",
        notes: "Observacions",
        note_plots: "Núm. d'ordre de les parcel·les tractades segons la taula 2.1.",
        note_operator: "Núm. d'ordre segons la taula 1.2.",
        note_equipment: "Núm. d'ordre segons la taula 1.3; \"Manual\" quan l'aplicació no va \
                         emprar cap equip.",
        note_phi: "Dies de termini aplicats i primer dia en què la collita torna a estar permesa.",
        note_efficacy: "Bona, regular o dolenta, observada després de l'aplicació.",
        note_date: "Data de l'actuació, o interval quan es va fer en diversos dies; el termini \
                    de seguretat es compta des de l'últim. L'hora d'inici s'anota quan l'ús del \
                    producte està restringit a determinades hores del dia (Reglament (UE) \
                    2023/564, annex).",
        note_total_quantity: "Quantitat total de producte emprada en l'actuació (kg o l).",
        note_growth_stage: "Estat fenològic del cultiu segons la monografia BBCH, anotat quan \
                            l'ús del producte està restringit a determinats estats (Reglament \
                            (UE) 2023/564, annex). S'indica al costat de l'espècie perquè el \
                            model oficial no té columna pròpia.",
    },
    s31bis: S31Bis {
        title: "3.1 bis REGISTRE D'ACTUACIONS FITOSANITÀRIES PER PARCEL·LA",
        subtitle: "(NOMÉS PER A CULTIUS I SUPERFÍCIES OBJECTE D'ASSESSORAMENT)",
        crop_group: "CULTIU",
        plot_group: "DADES DE LA PARCEL·LA",
        problem_group: "PLAGA A CONTROLAR",
        non_chemical_group: "ALTERNATIVES NO QUÍMIQUES D'INTERVENCIÓ",
        chemical_group: "ALTERNATIVES QUÍMIQUES D'INTERVENCIÓ",
        species: "Espècie",
        variety: "Varietat",
        plots: "Id. parcel·les",
        crop_surface: "Superf. cultivada (ha)",
        treated_surface: "Superf. tractada (ha)",
        problem: "Plaga",
        justification: "Justificació de l'actuació",
        measure: "Tipus de mesura",
        intensity: "Intensitat de la mesura",
        measure_date: "Data actuació",
        product: "Nom comercial / Substància activa",
        registration: "Núm. registre",
        dose: "Dosi utilitzada",
        product_date: "Data actuació",
        efficacy: "Eficàcia de la intervenció",
        notes: "Observacions",
        note_plots: "Núm. d'ordre de les parcel·les tractades segons la taula 2.1.",
        note_intensity: "Nre. de trampes, nre. de difusors, etc.",
        validation_interim: "VALIDACIÓ INTERMÈDIA",
        validation_final: "VALIDACIÓ FINAL",
        signature: "Signatura",
        advisor: "Assessor",
        ropo: "Núm. inscripció ROPO",
        date: "Data",
        season_end_date: "Data fi de campanya",
    },
    s32: S32 {
        title: "3.2 REGISTRE D'ÚS DE LLAVOR TRACTADA",
        plots: "Id. Parcel·les",
        date: "Data de sembra",
        species: "Espècie",
        variety: "Varietat",
        surface: "Superf. sembrada (ha)",
        seed_quantity: "Quantitat de llavor (kg)",
        seed_lot: "Núm. de lot",
        product: "Nom comercial",
        registration: "Núm. Registre",
        active_substance: "Matèria activa",
        efficacy: "Eficàcia",
        notes: "Observacions",
        note_plots: "Núm. d'ordre de les parcel·les sembrades segons la taula 2.1.",
        note_seed_lot: "Lot que consta a l'envas de la llavor; és el que permet \
                        rastrejar el tractament fins al proveïdor.",
    },
    s33: S33 {
        applies: "APLICA TRACTAMENT",
        title_postharvest: "3.3 REGISTRE DE TRACTAMENTS POSTCOLLITA",
        title_storage: "3.4 REGISTRE DE TRACTAMENTS EN LOCALS D'EMMAGATZEMATGE",
        title_transport: "3.5 REGISTRE DE TRACTAMENTS EN MITJANS DE TRANSPORT",
        subject_postharvest: "Producte vegetal tractat",
        subject_storage: "Local tractat (tipus i adreça)",
        subject_transport: "Vehicle tractat (tipus, model i matrícula)",
        quantity_postharvest: "Quantitat (t)",
        quantity_storage: "Volum (m³)",
        quantity_transport: "Volum (m³)",
        date: "Data",
        problem: "Problemàtica fitosanitària",
        operator: "Aplicador",
        product: "Nom comercial",
        registration: "Núm. Registre",
        product_quantity: "Quantitat utilitzada",
        efficacy: "Eficàcia",
        notes: "Observacions",
        note_applies: "Marqueu NO quan durant la campanya no s'hagi fet cap tractament \
                       d'aquest tipus; queda constància que s'ha comprovat.",
        note_product_quantity: "Quantitat de producte emprada, en kg o l.",
    },
    s4: S4 {
        section_title: "4. REGISTRE D'ANÀLISIS",
        title: "4.1 ANÀLISIS REALITZADES",
        date: "Data",
        material: "Material analitzat",
        plots: "Cultiu o collita mostrejats",
        bulletin: "Núm. butlletí d'anàlisi",
        laboratory: "Laboratori (nom i adreça)",
        substances: "Substàncies actives detectades",
        note_plots: "Núm. d'ordre de les parcel·les de la taula 2.1.",
        note_keep: "Els butlletins d'anàlisi es conserven amb la resta de documentació \
                    de l'explotació durant almenys tres anys (art. 16.3 del RD 1311/2012); \
                    en aquest registre consta on trobar-los.",
        note_soil: "El model imprès és anterior a l'apartat A.3 de l'annex III i no té pàgina de sòl: els paràmetres analitzats es recullen al costat dels resultats. El full de càlcul els dedica una pestanya, amb cada valor a la seva columna.",
    },
    s5: S5 {
        section_title: "5. REGISTRE DE COLLITA COMERCIALITZADA",
        title: "5.1 SORTIDES DE COLLITA",
        date: "Data",
        product: "Producte",
        quantity: "Quantitat",
        plots: "Parcel·les d'origen",
        delivery_note: "Núm. albarà o factura",
        lot: "Núm. de lot",
        buyer: "Client (nom o raó social)",
        buyer_tax_id: "NIF",
        buyer_address: "Adreça",
        buyer_registry: "Núm. RGSEAA",
        note_plots: "Núm. d'ordre de les parcel·les de la taula 2.1.",
        note_voluntary: "El núm. d'albarà o factura, el núm. de lot i el núm. de RGSEAA són \
                         voluntaris.",
    },
    s6: S6 {
        section_title: "6. REGISTRE DE FERTILITZACIÓ",
        title: "6.1 APLICACIONS REALITZADES",
        dates: "Data / interval",
        plots: "Id. parcel·les",
        area: "Sup. (ha)",
        crop: "Cultiu",
        material: "Tipus d'adob / producte",
        delivery_note: "Núm. d'albarà",
        richness: "Riquesa N / P₂O₅ / K₂O (%)",
        dose: "Dosi",
        kind: "Tipus de fertilització",
        applicator: "Maquinària / empresa de serveis",
        yield_estimated: "Producció estimada (kg/ha)",
        yield_final: "Producció final (kg/ha)",
        note_plots: "Núm. d'ordre de les parcel·les de la taula 2.1.",
        note_kind: "Sigles del model: (AF) adobat de fons, (AC) adobat de cobertora, \
                    (F) fertirrigació. La fertirrigació és una forma d'aplicació i no pas \
                    un tipus de fertilització, de manera que totes dues sigles poden \
                    concórrer; l'aplicació d'esmenes no té sigla al model.",
        note_richness: "L'annex III, secció C, apartat h, del RD 1311/2012 demana fins a \
                        vuit valors agronòmics del material; el model n'imprimeix tres. La \
                        resta, els micronutrients i els metalls pesants dels llots consten \
                        a la fitxa del material fertilitzant.",
        note_sludge: "L'aplicació de llots de depuradora s'indica al costat del material \
                      (RD 1051/2022, art. 5.g).",
    },
    s71: S71 {
        section_title: "7. FERTILITZACIÓ",
        title: "7.1 PLA D'ADOBAT",
        plots: "Id. parcel·les",
        crop: "Cultiu",
        date: "Data",
        area: "Sup. fertilitzada (ha)",
        fertiliser: "Fertilitzant",
        richness: "Riquesa N / P₂O₅ / K₂O (%)",
        dose: "Dosi",
        supplied: "UF aportades (N / P₂O₅ / K₂O)",
        accumulated: "UF acumulades (N / P₂O₅ / K₂O)",
        recommended: "UF recomanades (N / P₂O₅ / K₂O)",
        note_plots: "Núm. d'ordre de les parcel·les de la taula 2.1.",
        note_units: "Una unitat fertilitzant (UF) és un quilogram per hectàrea \
                     de N, de P₂O₅ o de K₂O.",
        note_assembled: "Les unitats aportades i acumulades es calculen a partir del \
                         registre de fertilització (secció 6): aportades = dosi × riquesa, \
                         acumulades = la seva suma correguda en cada unitat de producció. \
                         Només les recomanades provenen del pla (RD 1051/2022, art. 5.a).",
        note_unknown: "Una dosi en volum només es converteix en quilograms si el material \
                       té densitat anotada; sense ella la casella queda en blanc i la \
                       columna acumulada s'interromp, en comptes de donar un total incomplet.",
        note_document: "El pla mateix —identificació de recintes, dades de sòl, aigua \
                        disponible, dosis i moment d'aplicació, tipus de material, forma \
                        d'aplicació, maquinària i mesures de l'annex V— és un document que es \
                        conserva al costat del quadern (RD 1051/2022, art. 6).",
    },
    s8: S8 {
        section_title: "8. REG",
        title: "8.1 REGISTRE DE REG",
        plots: "Id. parcel·les",
        area: "Superfície regada (ha)",
        method: "Sistema de reg",
        dates: "Data / interval de reg",
        volume: "Volum de reg",
        cumulative: "Volum acumulat (m³/ha)",
        water_quality: "N nítric / P₂O₅ soluble a l'aigua (mg/l)",
        source: "Procedència de l'aigua",
        note_plots: "Núm. d'ordre de les parcel·les de la taula 2.1.",
        note_cumulative: "El volum acumulat és la suma corrent d'aquesta mateixa taula, \
                          expressada en m³/ha; només s'acumulen els regs anotats en \
                          aquesta unitat.",
        note_water_quality: "Contingut de nitrogen nítric i de fòsfor \
                             (P₂O₅) soluble de l'aigua de reg. S'anota quan \
                             l'organisme de conca, la comunitat de regants o un organisme \
                             equivalent facilita la dada; amb analítiques pròpies \
                             és voluntari (RD 1051/2022, art. 17.2).",
    },
    annex: Annex {
        section_title: "DOCUMENTACIÓ A CONSERVAR",
        title: "DOCUMENTACIÓ QUE S'HA DE CONSERVAR JUNTAMENT AMB EL QUADERN",
        intro: "A més d'aquest quadern, la persona titular conserva a disposició de \
                l'autoritat competent:",
        item_invoices: "Factures i documents d'adquisició dels productes fitosanitaris \
                        utilitzats.",
        item_contracts: "Contractes amb les empreses o persones que van fer els tractaments, \
                         quan no els va fer la mateixa explotació.",
        item_inspections: "Certificats d'inspecció dels equips d'aplicació.",
        item_containers: "Justificants de lliurament dels envasos buits en un punt de recollida \
                          autoritzat.",
        item_analyses: "Butlletins de les anàlisis de residus fetes sobre els cultius i les \
                        produccions i, si escau, sobre l'aigua de reg.",
        item_advice: "Documentació de l'assessorament rebut.",
        item_sale: "Albarans o factures de venda de la collita.",
        item_plan: "Pla d'adobat de cada unitat de producció (RD 1051/2022, art. 6).",
        item_sludge: "Document d'aplicació dels llots expedit pel gestor autoritzat (RD 1051/2022, art. 5.g; annex III de l'Ordre AAA/1072/2013).",
        item_manure: "Document amb la qualitat agronòmica dels fems rebuts de tercers (RD 1051/2022, art. 13.2); no cal quan els subministra el mateix titular.",
        retention: "Els documents 1 a 6 es conserven durant almenys tres anys des de la seva \
                    emissió (art. 16.3 del RD 1311/2012). Els justificants de lliurament \
                    d'envasos buits (4) i els documents de venda de la collita (7) responen, \
                    respectivament, a l'obligació de retorn d'envasos i a la traçabilitat \
                    alimentària. Els documents 8 a 10 els exigeix el RD 1051/2022, que no \
                    fixa per a ells un termini propi de conservació.",
    },
    value: Values {
        yes: "SÍ",
        no: "NO",
        cross: "X",
        manual: "Manual",
        unit_traps: "trampes",
        unit_traps_ha: "trampes/ha",
        unit_diffusers: "difusors",
        unit_diffusers_ha: "difusors/ha",
        unit_units: "unitats",
        unit_units_ha: "unitats/ha",
        justification_threshold: "Superació de llindars",
        justification_monitoring: "Monitoratge",
        justification_dss: "Sistema de suport a la presa de decisió (DSS)",
        justification_authority: "Avís per Comunitat Autònoma",
        justification_advisor: "Recomanació d'assessor",
        justification_alert_device: "Mesurador d'alerta fitosanitària",
        efficacy_good: "Bona",
        efficacy_fair: "Regular",
        efficacy_poor: "Dolenta",
        level_basic: "Bàsic",
        level_qualified: "Qualificat",
        level_fumigator: "Fumigador",
        level_pilot: "Pilot",
        zone_nitrate: "Vulnerable als nitrats",
        zone_phyto: "Restricció fitosanitària",
        zone_natura: "Xarxa Natura 2000",
        no_affection: "Sense afectació",
        campaign_word: "campanya",
        no_water_points: "Sense captacions",
        material_crop: "Cultiu",
        material_harvested_produce: "Producte collit",
        material_soil: "Sòl",
        material_water: "Aigua de reg",
        analysis_residues: "Residus de substàncies actives fitosanitàries",
        analysis_microbiological: "Microbiològic",
        analysis_heavy_metals: "Metalls pesants",
        analysis_nutrients: "Nutrients",
        analysis_soil_parameters: "Paràmetres del sòl",
        analysis_gmo: "Presència d'OMG",
        seed_kind_on_farm: "Tractada a l'explotació",
        seed_kind_processing_centre: "Tractada en un centre de condicionament",
        seed_kind_purchased_es: "Adquirida tractada a Espanya",
        seed_kind_purchased_abroad: "Adquirida tractada fora d'Espanya",
        method_surface_gravity: "Superfície o gravetat",
        method_sprinkler_fixed: "Aspersió fixa",
        method_sprinkler_mobile: "Aspersió mòbil",
        method_micro_sprinkler: "Microaspersió",
        method_misting: "Nebulització",
        method_drip: "Degoteig",
        method_hydroponic_open: "Hidroponia a solució perduda",
        method_hydroponic_recirculating: "Hidroponia amb recirculació",
        water_surface: "Superficial",
        water_groundwater: "Subterrània",
        water_rainwater: "Pluvial",
        water_reclaimed: "Regeneració",
        water_desalinated: "Dessalinització",
        water_alternative: "Recursos alternatius",
        fertilisation_base: "Adobat de fons",
        fertilisation_top: "Adobat de cobertora",
        fertilisation_amendment: "Aplicació d'esmena",
        application_broadcast: "Escampat general",
        application_broadcast_buried: "Escampat general i enterrat",
        application_banded: "Escampat localitzat",
        application_banded_buried: "Escampat localitzat i enterrat",
        application_fertigation_sprinkler: "Reg per aspersió (fertirrigació)",
        application_fertigation_localised: "Reg localitzat (fertirrigació)",
        application_foliar: "Aplicació foliar",
        sludge_mark: "aplicació de llots",
        manure_none: "Cap",
        manure_solid_fraction: "Separació sòlid-líquid: fracció sòlida",
        manure_liquid_fraction: "Separació sòlid-líquid: fracció líquida",
        manure_ndn: "Nitrificació-desnitrificació (NDN)",
        manure_composting: "Compostatge",
        manure_anaerobic: "Digestió anaeròbia",
        manure_solar_drying: "Assecatge solar",
        manure_stripping: "Stripping",
        manure_membrane: "Separació per membranes",
        nutrient_macro: "Macronutrients",
        nutrient_micro: "Micronutrients",
        nutrient_heavy_metal: "Metalls pesants",
        sigla_base: "AF",
        sigla_top: "AC",
        sigla_fertigation: "F",
        phi_days: "dies",
        phi_until: "fins al",
    },
    sheet: SheetLabels {
        tab_farm: "1.1 Explotació",
        tab_operators: "1.2 Persones",
        tab_machinery: "1.3 Equips",
        tab_advisors: "1.4 Assessorament",
        tab_plots: "2.1 Parcel·les",
        tab_zones: "2.2 Mediambiental",
        intensity_unit: "Unitat d'intensitat",
        measure_registration: "Núm. registre mesura",
        tab_treatments: "3.1 Tractaments",
        tab_seed: "3.2 Llavor tractada",
        tab_postharvest: "3.3 Postcollita",
        tab_storage: "3.4 Locals",
        tab_transport: "3.5 Transport",
        tab_analysis: "4 Anàlisis",
        tab_harvest: "5 Collita",
        tab_water_points: "2.2 Captacions",
        tab_irrigation: "8 Reg",
        tab_fertilisation: "6 Fertilització",
        tab_plan: "7.1 Pla d'adobat",
        tab_soil: "4 Sòl",
        soil_ph: "pH",
        soil_organic_matter: "Matèria orgànica (%)",
        soil_p: "P assimilable (mg/kg)",
        soil_k: "K assimilable (mg/kg)",
        soil_n: "N total (%)",
        soil_conductivity: "Conductivitat (dS/m)",
        soil_sand: "Sorra (%)",
        soil_silt: "Llim (%)",
        soil_clay: "Argila (%)",
        supplied_n: "UF aportades N",
        supplied_p2o5: "UF aportades P₂O₅",
        supplied_k2o: "UF aportades K₂O",
        accumulated_n: "UF acumulades N",
        accumulated_p2o5: "UF acumulades P₂O₅",
        accumulated_k2o: "UF acumulades K₂O",
        recommended_n: "UF recomanades N",
        recommended_p2o5: "UF recomanades P₂O₅",
        recommended_k2o: "UF recomanades K₂O",
        tab_materials: "6 Materials",
        material_kind: "Tipus de material",
        fertilisation_type: "Tipus de fertilització",
        application_method: "Forma d'aplicació",
        sludge: "Aplicació de llots",
        practices: "Bones pràctiques",
        service_company: "Empresa de serveis",
        service_regfer: "Núm. REGFER",
        richness_n: "N total (%)",
        richness_p2o5: "P₂O₅ total (%)",
        richness_k2o: "K₂O (%)",
        nutrient_group: "Grup",
        nutrient: "Nutrient",
        percentage: "%",
        supplier: "Empresa subministradora",
        supplier_registry: "REGA / NIF / NIMA",
        manure_treatment: "Tractament dels fems",
        density: "Densitat (kg/l)",
        water_nitric_n: "N nítric a l'aigua (mg/l)",
        water_soluble_p2o5: "P₂O₅ soluble a l'aigua (mg/l)",
        field: "Camp",
        value: "Valor",
        campaign: "Campanya",
        generated_on: "Document generat el",
        representative_prefix: "Representant",
        registry_regional: "Núm. Registre d'Explotacions Autonòmic (REA)",
        operator_name: "Nom i cognoms / Empresa",
        gip_code: "Codi",
        plot_province: "Província",
        plot_municipality: "Municipi",
        plot_municipality_name: "Municipi (nom)",
        plot_aggregate: "Agregat",
        plot_polygon: "Polígon",
        plot_parcel: "Parcel·la SIGPAC",
        plot_enclosure: "Recinte",
        sigpac_area: "Superfície SIGPAC (ha)",
        cultivated_area: "Superfície cultivada (ha)",
        plot_id: "Id. parcel·la",
        plot_ids: "Id. parcel·les",
        water_point: "Captació inclosa a la parcel·la",
        latitude: "Latitud",
        longitude: "Longitud",
        plots: "Parcel·les",
        treated_area: "Superfície tractada (ha)",
        operator_order: "Núm. aplicador",
        operator: "Aplicador",
        equipment_order: "Núm. equip",
        equipment: "Equip",
        registration: "Núm. registre",
        dose_unit: "Unitat de dosi",
        date_start: "Data d'inici",
        date_end: "Data de fi",
        growth_stage: "Estat fenològic (BBCH)",
        application_time: "Hora d'inici",
        total_quantity: "Quantitat total",
        total_quantity_unit: "Unitat de quantitat",
        subject: "Objecte tractat",
        quantity: "Quantitat tractada",
        quantity_unit: "Unitat",
        product_quantity: "Quantitat utilitzada",
        product_quantity_unit: "Unitat utilitzada",
        register_applies: "Aplica tractament",
        seed_quantity: "Quantitat de llavor (kg)",
        sown_area: "Superfície sembrada (ha)",
        analysis_types: "Tipus d'anàlisi",
        substances_coded: "Substàncies (catàleg)",
        seed_treatment_kind: "Tractament de la llavor",
        advisor_name: "Assessor",
        advisor_registration: "Nº ROPO assessor",
        lab_name: "Laboratori",
        lab_address: "Adreça del laboratori",
        lab_tax_id: "NIF del laboratori",
        phi_days: "Termini de seguretat (dies)",
        harvest_from: "Collita permesa des de",
    },
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_codes_round_trip() {
        for language in ReportLanguage::ALL {
            assert_eq!(ReportLanguage::from_code(language.code()), Some(language));
        }
        assert_eq!(ReportLanguage::from_code("gl"), None);
        assert_eq!(ReportLanguage::from_code(""), None);
    }

    #[test]
    fn castilian_is_named_as_it_is_named_in_spain() {
        // "Español" would make a claim about the co-official languages, which
        // are Spanish too; the state's own term is "castellano" (CE art. 3.1).
        assert_eq!(ReportLanguage::Es.native_name(), "Castellano");
        assert_eq!(ReportLanguage::Ca.native_name(), "Català");
    }

    #[test]
    fn efficacy_follows_the_models_footnote_wording() {
        // Andalucía model, 3.1 footnote 5: "indicar buena, regular o mala".
        let es = ReportLanguage::Es.labels();
        assert_eq!(es.efficacy(Some("good")), "Buena");
        assert_eq!(es.efficacy(Some("poor")), "Mala");
        assert_eq!(es.efficacy(None), "");
        let ca = ReportLanguage::Ca.labels();
        assert_eq!(ca.efficacy(Some("good")), "Bona");
        assert_eq!(ca.efficacy(Some("poor")), "Dolenta");
        assert_eq!(ca.efficacy(None), "");
    }

    /// The compiler catches a MISSING annex field; it cannot catch one left as
    /// an empty string, which would print a silently short list on a page whose
    /// whole purpose is to enumerate a legal duty.
    #[test]
    fn every_language_states_the_whole_conservation_duty() {
        for language in ReportLanguage::ALL {
            let a = &language.labels().annex;
            for (name, value) in [
                ("section_title", a.section_title),
                ("title", a.title),
                ("intro", a.intro),
                ("item_invoices", a.item_invoices),
                ("item_contracts", a.item_contracts),
                ("item_inspections", a.item_inspections),
                ("item_containers", a.item_containers),
                ("item_analyses", a.item_analyses),
                ("item_advice", a.item_advice),
                ("item_sale", a.item_sale),
                ("retention", a.retention),
            ] {
                assert!(
                    !value.trim().is_empty(),
                    "annex.{name} is blank in {}",
                    language.code()
                );
            }
            // The three-year period is the duty's operative number (art. 16.3);
            // a retention sentence that lost it would say nothing enforceable.
            assert!(
                a.retention.contains("tres anys") || a.retention.contains("tres años"),
                "the retention line must state the three years in {}",
                language.code()
            );
            assert!(a.retention.contains("16.3"), "cite the article");
        }
    }

    #[test]
    fn an_unknown_zone_code_prints_itself_rather_than_disappearing() {
        let labels = ReportLanguage::Ca.labels();
        assert_eq!(labels.zone("nitrate_vulnerable"), "Vulnerable als nitrats");
        assert_eq!(labels.zone("flood_plain"), "flood_plain");
    }

    #[test]
    fn the_phi_phrase_carries_both_the_days_and_the_first_allowed_day() {
        assert_eq!(
            ReportLanguage::Es.labels().phi_phrase(21, "22/05/2026"),
            "21 días (hasta 22/05/2026)"
        );
        assert_eq!(
            ReportLanguage::Ca.labels().phi_phrase(21, "22/05/2026"),
            "21 dies (fins al 22/05/2026)"
        );
    }

    /// Excel refuses tab names over 31 characters; the engine truncates, but a
    /// truncated tab name in a legal document is a defect, not a repair.
    #[test]
    fn every_sheet_tab_name_fits_excels_limit() {
        for language in ReportLanguage::ALL {
            let sheet = &language.labels().sheet;
            for name in [
                sheet.tab_farm,
                sheet.tab_operators,
                sheet.tab_machinery,
                sheet.tab_advisors,
                sheet.tab_plots,
                sheet.tab_zones,
                sheet.tab_treatments,
                sheet.tab_seed,
                sheet.tab_postharvest,
                sheet.tab_storage,
                sheet.tab_transport,
                sheet.tab_analysis,
                sheet.tab_harvest,
                sheet.tab_water_points,
            ] {
                assert!(
                    name.chars().count() <= 31,
                    "tab '{name}' ({}) is longer than Excel allows",
                    language.code()
                );
            }
        }
    }
}
