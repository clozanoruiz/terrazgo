// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The printable cuaderno (PDF): data assembly for `templates/cuaderno.typ`,
//! rendered in-process by the shared report engine (`terrazgo-report`).
//!
//! Unlike the SIEX export there is NO precheck gate: the printed record book
//! shows what exists, and fields the official model asks for but the data
//! lacks print blank — a farmer must be able to print for an inspection
//! even while some registry data is incomplete. Soft-deleted records are
//! audit history and never print.
//!
//! The assembly is where all knowledge lives; the renderers only present it.
//! [`Cuaderno`] holds the book as VALUES (real numbers, ISO dates, lookup
//! codes) and two renderers consume it: the Typst template gets pre-formatted
//! strings because it only does layout, while the spreadsheet gets typed cells
//! because a farmer must be able to sort, filter and sum. Adding a field means
//! touching the assembly once, not each output.
//!
//! The book's LAYOUT is per country (the Spanish official model); its LANGUAGE
//! is per region, because a farmer in a region with a co-official language must
//! be able to hand an inspector the same book in either one. So [`Cuaderno`]
//! holds no prose: every printed word comes from [`labels::Labels`] at render
//! time, and `region` decides which languages a given holding may choose from.
//!
//! Cross-references follow the official model: section 3.1 names
//! operators/equipment/plots by the order numbers of tables 1.2, 1.3 and 2.1,
//! and all four lists are built here from the same records, so a reference can
//! never dangle. The spreadsheet keeps those numbers AND resolves the names
//! beside them — the two documents reconcile row for row, and the sheet is
//! still filterable on its own.

pub mod advisory;
mod collate;
pub mod db;
pub mod error;
pub mod labels;
pub mod region;

use crate::collate::NameCollator;
use error::Result;
use labels::Labels;
use module_cue::crop_groups;
use module_cue::models::TreatmentRecordWithPlots;
use module_cue::repository::{
    list_analysis_records, list_non_field_treatments, list_register_declarations,
    list_seed_treatments, list_treatment_records,
};
use module_cue::siex;
use module_fertilisation::agronomy::{Accumulator, dose_as_kg_per_ha, nutrient_units};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use terrazgo_core::catalogue::CatalogueCode;
use terrazgo_report::{Cell, Column, RenderedPdf, RenderedWorkbook, Sheet, Workbook};

pub use advisory::{BookAdvisory, book_advisory};
pub use db::open_in_memory;
pub use error::{RecordbookError, Result as RecordbookResult};
pub use labels::ReportLanguage;
pub use region::{default_language, languages_for_farm};

const TEMPLATE: &str = include_str!("../templates/cuaderno.typ");

// ---------------------------------------------------------------------------
// The typed book — one assembly, two renderers
// ---------------------------------------------------------------------------

/// The whole record book for one farm+season, as values.
pub struct Cuaderno {
    campaign: String,
    /// ISO `YYYY-MM-DD`.
    generated_on: String,
    farm: FarmInfo,
    operators: Vec<OperatorRow>,
    advisors: Vec<AdvisorRow>,
    machinery: Vec<MachineryRow>,
    plots: Vec<PlotRow>,
    zones: Vec<ZoneRow>,
    treatments: Vec<TreatmentRow>,
    /// Section 3.2 — sowings with supplier-treated seed.
    seed: Vec<SeedRow>,
    /// Sections 3.3, 3.4 and 3.5, in one list. Each printed table filters this
    /// by `subject_kind`, so the three registers share one assembly.
    non_field: Vec<NonFieldRow>,
    /// Section 4 — laboratory analyses.
    analysis: Vec<AnalysisRow>,
    /// Section 5 — what left the holding, and to whom.
    harvest: Vec<HarvestRow>,
    /// Section 6 — the fertilisation register (RD 1051/2022 art. 5.d).
    fertilisation: Vec<FertilisationRow>,
    /// The fertiliser materials section 6 points at. Not a section of the
    /// printed model: it is where Anexo III C.h's eight agronomic values, the
    /// micronutrients and the sludge heavy metals actually live, so the
    /// workbook publishes it as its own tab rather than losing it.
    materials: Vec<MaterialRow>,
    /// Section 7.1 — the plan de abonado table, assembled from section 6's own
    /// records and the plan's recommendation. Nothing here is stored twice.
    plan_rows: Vec<PlanRow>,
    /// Section 8 — the irrigation register (RD 1051/2022 art. 5.e).
    irrigation: Vec<IrrigationRow>,
    /// Section 9.1 — extensive grazing (RD 1048/2022 art. 30.2 ter). The book's
    /// third decree; empty for every holding that claims no ecorrégimen, which
    /// is most of them.
    grazing: Vec<GrazingRow>,
    /// Section 9.2 — siega sostenible, pivoted onto the plot the model's row
    /// actually is (arts. 31 and 31.4.d).
    mowing: Vec<MowingRow>,
    /// The book's "9.6" — the pastos comunales register anexo IV orders and the
    /// printed model gives no page to.
    communal: Vec<CommunalRow>,
    /// Both of the above, unpivoted, for the spreadsheet: one row per
    /// operation, with the duty it evidences as a filterable column.
    operations: Vec<OperationSheetRow>,
    /// Section 9.3 — the five dates art. 45.2 names, gathered per plot from
    /// three tables in three crates.
    flooded: Vec<FloodedRow>,
    /// The sowing register itself. No page of the model prints it; its dates
    /// feed 9.2 and 9.3, and this is the only place it can be read whole.
    sowings: Vec<SowingSheetRow>,
    /// Section 9.4 — the live covers of art. 42, with the three maintenance
    /// columns art. 42.1.c fills from two other registers.
    plant_covers: Vec<CoverRow>,
    /// Section 9.5 — the inert covers of art. 43, which take no maintenance.
    inert_covers: Vec<CoverRow>,
    /// Both of the above for the spreadsheet, with the practice, the kind of
    /// cover and the widths' own annotation date the pages have no column for.
    covers: Vec<CoverSheetRow>,
    /// Which conditional registers the farmer explicitly declared empty.
    declared_empty: Vec<String>,
}

struct FarmInfo {
    name: String,
    owner: String,
    nif: String,
    /// National registry number, printed beside the autonómico `rea`.
    siex: String,
    rea: String,
    location: String,
    /// The province NAME where the stored code resolves against the FEGA
    /// PROVINCIA catalogue, and the stored value verbatim where it does not —
    /// the `problem_code` rule. The column is entered by hand, so it holds
    /// "47" on one holding and "Valladolid" on the next, and a legal document
    /// should print a province either way rather than a bare number.
    province: String,
    address: String,
    postal_code: String,
    phone_fixed: String,
    phone_mobile: String,
    email: String,
    /// Model 1.1's "Fecha de apertura del cuaderno". Empty prints the model's
    /// blank rule, which is the honest rendering of a date nobody stated.
    opened_on: String,
    /// Model 1.1's "titular o representante" block; absent for most farms.
    representative: Option<RepresentativeInfo>,
}

struct RepresentativeInfo {
    name: String,
    nif: String,
    kind: String,
    address: String,
    locality: String,
    /// Free text as the farmer typed it: this is the representative's postal
    /// address, not the coded geography the holding's own province carries.
    province: String,
    postal_code: String,
    phone: String,
    email: String,
}

/// Assemble and render the cuaderno for one farm+season as PDF, in the chosen
/// language (the layout is the same official model either way).
pub fn render_cuaderno(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
    generated_on_iso: &str,
    language: ReportLanguage,
) -> Result<RenderedPdf> {
    let inputs = cuaderno_inputs(conn, season_id, farm_id, generated_on_iso, language)?;
    Ok(terrazgo_report::render_pdf(TEMPLATE, &inputs)?)
}

/// The same book as a spreadsheet: one sheet per section of the official
/// model, with real dates and numbers.
pub fn render_cuaderno_xlsx(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
    generated_on_iso: &str,
    language: ReportLanguage,
) -> Result<RenderedWorkbook> {
    Ok(terrazgo_report::render_xlsx(&cuaderno_workbook(
        conn,
        season_id,
        farm_id,
        generated_on_iso,
        language,
    )?)?)
}

/// The workbook description, public for the same reason as
/// [`cuaderno_inputs`]: tests pin the data contract here rather than by
/// cracking open a rendered .xlsx, so an assertion names the value that was
/// written (`Cell::Date`, `Cell::Number`) instead of a byte pattern.
pub fn cuaderno_workbook(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
    generated_on_iso: &str,
    language: ReportLanguage,
) -> Result<Workbook> {
    let catalogues = CatalogueCache::default();
    Ok(assemble(
        conn,
        &catalogues,
        season_id,
        farm_id,
        generated_on_iso,
        language,
    )?
    .to_workbook(language))
}

/// The template's `sys.inputs`, public so tests can pin the data contract
/// without parsing a PDF. `generated_on_iso` is passed in (not read from the
/// clock) so output is reproducible.
pub fn cuaderno_inputs(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
    generated_on_iso: &str,
    language: ReportLanguage,
) -> Result<Value> {
    let catalogues = CatalogueCache::default();
    Ok(assemble(
        conn,
        &catalogues,
        season_id,
        farm_id,
        generated_on_iso,
        language,
    )?
    .to_typst(language))
}

/// Read the book out of the database. Every query lives here; the renderers
/// touch no connection.
///
/// `catalogues` is one book's worth of label resolution — see
/// [`CatalogueCache`]. It is a parameter rather than a local so the tests can
/// count what a real book costs.
fn assemble(
    conn: &Connection,
    catalogues: &CatalogueCache,
    season_id: &str,
    farm_id: &str,
    generated_on_iso: &str,
    language: ReportLanguage,
) -> Result<Cuaderno> {
    let farm = terrazgo_core::repository::get_farm(conn, farm_id)?;
    let campaign: String =
        conn.query_row("SELECT label FROM season WHERE id = ?1", [season_id], |r| {
            r.get(0)
        })?;

    // Register order: oldest first — a record book reads chronologically.
    let mut records = list_treatment_records(conn, season_id, farm_id)?;
    records.reverse();

    // Model table 2.1, and the plot order numbers every later register
    // cross-references back to it.
    let table_21 = plot_rows(conn, catalogues, season_id, farm_id, language)?;
    let plots = &table_21.index;
    let zones = zone_rows(conn, season_id, farm_id, language)?;
    let advisors = advisor_rows(conn, farm_id, language)?;
    let operators = operator_rows(
        conn,
        &records,
        &terrazgo_core::repository::list_advisors(conn)?,
    );
    let machinery = machinery_rows(conn, &records)?;
    // The stated surface of every crop a treatment row can reference, resolved
    // once: 3.1 and 3.1 bis print it per row, and looking it up inside the row
    // loop costs a query per treated plot.
    //
    // Deliberately NOT `list_crops`, which filters `deleted_at IS NULL`.
    // Deleting a crop is always allowed precisely BECAUSE `treatment_plot`
    // freezes the species and variety and the record book is unaffected
    // (docs/data-model.md) — so a book that blanked the cultivated surface of a
    // deleted crop would break the guarantee that permits the deletion. Only
    // crops stating an area enter the map; the rest are the blank above.
    let mut crop_areas: HashMap<String, f64> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT crop.id, crop.area_ha FROM crop
             JOIN plot ON plot.id = crop.plot_id
             WHERE crop.season_id = ?1 AND plot.farm_id = ?2 AND crop.area_ha IS NOT NULL",
        )?;
        let mut found = stmt.query((season_id, farm_id))?;
        while let Some(row) = found.next()? {
            crop_areas.insert(row.get(0)?, row.get(1)?);
        }
    }
    let treatments = treatment_rows(
        conn,
        catalogues,
        &records,
        plots,
        &operators,
        &machinery,
        &crop_areas,
    )?;
    let seed = seed_rows(conn, season_id, farm_id, plots)?;
    let non_field = non_field_rows(conn, catalogues, season_id, farm_id)?;
    let analysis = analysis_rows(
        conn,
        catalogues,
        season_id,
        farm_id,
        &farm.farm.country_code,
        plots,
    )?;
    let harvest = harvest_rows(conn, season_id, farm_id, plots)?;
    let irrigation = irrigation_rows(conn, season_id, farm_id, plots)?;
    // Read ONCE and used twice: model 9.1's own rows, and model 9.4's Pastoreo
    // column. The two partition the register on `soil_cover_id`, so neither a
    // second read nor a second filter pass is needed.
    let grazing_details =
        module_ecoscheme::repository::list_grazing_records(conn, season_id, farm_id)?;
    let grazing = grazing_rows(
        conn,
        catalogues,
        &farm.farm.country_code,
        plots,
        &grazing_details,
    )?;
    // Read ONCE and used three ways: the sowing register's own tab, model
    // 9.2's "Siembra" column, and model 9.3's "siembra en seco" and
    // "inundación". A second read per page would be three queries for one
    // table (the `crop_areas` rule).
    let sowing_details = terrazgo_core::repository::list_sowing_records(conn, season_id, farm_id)?;
    // Read here rather than inside `operation_rows`, so both registers this
    // section reads arrive the same way and the projection function stays a
    // projection.
    let operation_details =
        module_ecoscheme::repository::list_cultural_operations(conn, season_id, farm_id)?;
    let operations = operation_rows(
        conn,
        catalogues,
        &farm.farm.country_code,
        plots,
        language,
        &operation_details,
        &sowing_details,
    )?;
    let flooded = flooded_rows(plots, &sowing_details, &records, &operations.flooded);
    let sowings = sowing_sheet_rows(&sowing_details, plots, language);
    let cover_details = module_ecoscheme::repository::list_soil_covers(conn, season_id, farm_id)?;
    let covers = cover_rows(
        conn,
        catalogues,
        &farm.farm.country_code,
        plots,
        language,
        CoverSources {
            covers: &cover_details,
            grazings: &grazing_details,
            operations: &operations.cover_maintenance,
        },
    )?;
    let fertilisation = fertilisation_rows(
        conn,
        catalogues,
        season_id,
        farm_id,
        &farm.farm.country_code,
        plots,
    )?;
    let materials = material_rows(conn, catalogues, &farm.farm.country_code, language)?;
    let plan_rows = plan_rows(conn, season_id, farm_id, plots)?;
    let declared_empty = list_register_declarations(conn, farm_id, season_id)?
        .into_iter()
        .map(|d| d.register_code)
        .collect();

    let row = farm.farm;
    let ext = farm.es;
    Ok(Cuaderno {
        campaign,
        generated_on: generated_on_iso.to_string(),
        farm: FarmInfo {
            name: row.name,
            owner: row.owner_name.unwrap_or_default(),
            nif: row.owner_tax_id.unwrap_or_default(),
            siex: ext
                .as_ref()
                .and_then(|e| e.siex_code.clone())
                .unwrap_or_default(),
            rea: ext
                .as_ref()
                .and_then(|e| e.rea_code.clone())
                .unwrap_or_default(),
            location: row.location_text.unwrap_or_default(),
            province: province_name(
                conn,
                catalogues,
                ext.as_ref().and_then(|e| e.province_code.as_deref()),
            ),
            address: row.address.unwrap_or_default(),
            postal_code: row.postal_code.unwrap_or_default(),
            phone_fixed: row.phone_fixed.unwrap_or_default(),
            phone_mobile: row.phone_mobile.unwrap_or_default(),
            email: row.email.unwrap_or_default(),
            opened_on: row.opened_on.map(|d| format_date(&d)).unwrap_or_default(),
            representative: farm.representative.map(|r| RepresentativeInfo {
                name: r.full_name,
                nif: r.tax_id.unwrap_or_default(),
                kind: r.representation_kind.unwrap_or_default(),
                address: r.address.unwrap_or_default(),
                locality: r.locality.unwrap_or_default(),
                province: r.province.unwrap_or_default(),
                postal_code: r.postal_code.unwrap_or_default(),
                phone: r.phone.unwrap_or_default(),
                email: r.email.unwrap_or_default(),
            }),
        },
        operators,
        advisors,
        machinery,
        plots: table_21.rows,
        zones,
        treatments,
        seed,
        non_field,
        analysis,
        harvest,
        fertilisation,
        materials,
        plan_rows,
        irrigation,
        grazing,
        mowing: operations.mowing,
        communal: operations.communal,
        operations: operations.sheet,
        flooded,
        sowings,
        plant_covers: covers.plant,
        inert_covers: covers.inert,
        covers: covers.sheet,
        declared_empty,
    })
}

// ---------------------------------------------------------------------------
// Section 2.1 — parcelas (+ the plot_id → order map 3.1 references)
// ---------------------------------------------------------------------------

struct PlotRows {
    rows: Vec<PlotRow>,
    index: PlotIndex,
}

/// How every other register names a plot: its order number in table 2.1, and
/// its name. The two travel together because each register prints both — the
/// number for the cross-reference the model asks for, the name so the
/// spreadsheet filters on its own — so they are one parameter, not two.
#[derive(Default)]
struct PlotIndex {
    orders: HashMap<String, usize>,
    names: HashMap<String, String>,
    /// The plot's full SIGPAC reference as one colon-joined string, for the
    /// registers whose model column asks for the reference itself rather than
    /// a cross-reference to table 2.1 — section 9.1 is the first. Empty when
    /// the plot carries no reference, so the page prints the plot's name alone
    /// instead of a string of colons.
    references: HashMap<String, String>,
    /// The same reference broken into the parts a register prints as columns of
    /// their own — model 9.2 identifies its row the way table 2.1 does, rather
    /// than by cross-reference. Kept per PLOT and not on `PlotRow`, because
    /// that type carries one row per (plot, crop) while 9.2's row is a plot.
    sigpac: HashMap<String, PlotSigpac>,
    /// Orders those names the way the book's language does. It lives here
    /// because this is the type that knows what a plot is called, and because
    /// every register that prints a list of plot names already takes it — so
    /// the collator reaches them without a new parameter apiece.
    collator: NameCollator,
}

/// The parcel-register facts a register prints in columns of their own, held
/// once per plot. Every part may be blank on its own — a plot the farmer
/// entered by hand carries none of them, and a blank cell is a true statement
/// where an invented one would not be.
#[derive(Clone, Default)]
struct PlotSigpac {
    province: String,
    municipality: String,
    polygon: String,
    parcel: String,
    enclosure: String,
    /// The provider's own surface, never the farmer's `plot.area_ha`: model
    /// 9.2's column says "Superficie SIGPAC", and merging the two is what the
    /// parcel register's whole separation exists to prevent.
    area_ha: Option<f64>,
}

/// One row of model table 2.1: a plot, and one of the crops on it.
struct PlotRow {
    order: usize,
    name: String,
    province: String,
    municipality: String,
    /// The municipality's name, resolved against `MUNICIPIO_SIGPAC` — the model
    /// asks this column for "código y nombre", and the provider returns only
    /// the code. Empty when it cannot be resolved, so the PDF prints the code
    /// on its own rather than an invented name.
    municipality_name: String,
    aggregate: String,
    zone: String,
    polygon: String,
    parcel: String,
    enclosure: String,
    /// SIGPAC land-use code (`TA`, `PA`…) from the provider boundary.
    land_use: String,
    /// Provider-declared surface; never merged with the farmer's own figure.
    sigpac_area_ha: Option<f64>,
    /// Surface under THIS crop. `None` when the plot carries several crops and
    /// the split is not stored yet — a blank the farmer fills, never a guess.
    cultivated_area_ha: Option<f64>,
    species: String,
    variety: String,
    /// Model siglas, already resolved: SEC/ASP/LOC/GRA and AL/M/BP/INV are
    /// the form's own codes, printed identically in every language (the
    /// footnote that expands them is what translates).
    irrigation: &'static str,
    environment: &'static str,
    /// Model sigla (AE/PI), already resolved — a code like the two above.
    gip: &'static str,
}

/// All active plots of the farm, ordered by name; one row per (plot, season
/// crop), repeating the plot's order number — the model groups rows by
/// parcela. A plot without a crop this season still prints (blank species).
fn plot_rows(
    conn: &Connection,
    catalogues: &CatalogueCache,
    season_id: &str,
    farm_id: &str,
    language: ReportLanguage,
) -> Result<PlotRows> {
    let plots = terrazgo_core::repository::list_plots(conn, farm_id)?;
    let crops = terrazgo_core::repository::list_crops(conn, season_id, farm_id)?;
    let sigpac = sigpac_facts(conn, farm_id)?;

    let mut rows = Vec::new();
    let mut index = PlotIndex {
        collator: NameCollator::new(language),
        ..PlotIndex::default()
    };
    for (idx, detail) in plots.iter().enumerate() {
        let order = idx + 1;
        index.orders.insert(detail.plot.id.clone(), order);
        index
            .names
            .insert(detail.plot.id.clone(), detail.plot.name.clone());
        let es = detail.es.as_ref();
        let ref_part = |field: Option<&String>| field.cloned().unwrap_or_default();
        let facts = sigpac.get(&detail.plot.id);
        // Anexo III A.2.c–d: the provider's use code and official surface ride
        // beside the user's own figure and are never merged with it.
        let province = ref_part(es.and_then(|e| e.sigpac_province.as_ref()));
        let municipality = ref_part(es.and_then(|e| e.sigpac_municipality.as_ref()));
        let reference = sigpac_reference(&[
            &province,
            &municipality,
            &ref_part(es.and_then(|e| e.sigpac_aggregate.as_ref())),
            &ref_part(es.and_then(|e| e.sigpac_zone.as_ref())),
            &ref_part(es.and_then(|e| e.sigpac_polygon.as_ref())),
            &ref_part(es.and_then(|e| e.sigpac_parcel.as_ref())),
            &ref_part(es.and_then(|e| e.sigpac_enclosure.as_ref())),
        ]);
        if !reference.is_empty() {
            index.references.insert(detail.plot.id.clone(), reference);
        }
        index.sigpac.insert(
            detail.plot.id.clone(),
            PlotSigpac {
                province: province.clone(),
                municipality: municipality.clone(),
                polygon: ref_part(es.and_then(|e| e.sigpac_polygon.as_ref())),
                parcel: ref_part(es.and_then(|e| e.sigpac_parcel.as_ref())),
                enclosure: ref_part(es.and_then(|e| e.sigpac_enclosure.as_ref())),
                area_ha: facts.and_then(|f| f.official_area_ha),
            },
        );
        let base = PlotRow {
            order,
            name: detail.plot.name.clone(),
            municipality_name: municipality_name(conn, catalogues, &province, &municipality),
            province,
            municipality,
            aggregate: ref_part(es.and_then(|e| e.sigpac_aggregate.as_ref())),
            zone: ref_part(es.and_then(|e| e.sigpac_zone.as_ref())),
            polygon: ref_part(es.and_then(|e| e.sigpac_polygon.as_ref())),
            parcel: ref_part(es.and_then(|e| e.sigpac_parcel.as_ref())),
            enclosure: ref_part(es.and_then(|e| e.sigpac_enclosure.as_ref())),
            land_use: facts.and_then(|f| f.land_use.clone()).unwrap_or_default(),
            sigpac_area_ha: facts.and_then(|f| f.official_area_ha),
            cultivated_area_ha: detail.plot.area_ha,
            species: String::new(),
            variety: String::new(),
            irrigation: "",
            environment: "",
            gip: "",
        };

        // Collated, not left in the repository's order: SQL sorts species names
        // with BINARY collation, which files "Álamo" after "Avena". The screen
        // orders the same names with `Intl.Collator` (src/lib/collate.js), and
        // this crate's collator exists so the two agree.
        let mut plot_crops: Vec<_> = crops
            .iter()
            .filter(|c| c.plot_id == detail.plot.id && c.deleted_at.is_none())
            .collect();
        plot_crops.sort_by(|a, b| index.collator.compare(&a.species_name, &b.species_name));
        if plot_crops.is_empty() {
            rows.push(base);
        } else {
            // "Superficie cultivada" is per crop row. With a single crop the
            // plot's area is the honest answer; with several the split is
            // unknown until `crop.area_ha` exists (docs/cuaderno-print.md →
            // Capture design), so the cell stays empty for hand-filling
            // rather than repeating the whole plot on every row.
            let share_unknown = plot_crops.len() > 1;
            for crop in plot_crops {
                rows.push(PlotRow {
                    species: crop.species_name.clone(),
                    variety: crop.variety.clone().unwrap_or_default(),
                    irrigation: irrigation_abbrev(crop.irrigation_code.as_deref()),
                    environment: environment_abbrev(crop.growing_environment_code.as_deref()),
                    gip: crop_gip_abbrev(
                        crop.gip_system_code.as_deref(),
                        crop.production_system_code.as_deref(),
                    ),
                    // The crop's own surface when stated; otherwise the plot's,
                    // but only while it is the plot's ONLY crop — with several
                    // the split is unknown and the cell stays blank.
                    cultivated_area_ha: crop.area_ha.or(if share_unknown {
                        None
                    } else {
                        base.cultivated_area_ha
                    }),
                    ..clone_plot_row(&base)
                });
            }
        }
    }
    Ok(PlotRows { rows, index })
}

/// `PlotRow` deliberately does not derive `Clone`: the only place a row is
/// duplicated is the per-crop expansion above, and spelling it out keeps that
/// the one place where a plot's identity is copied onto several rows.
fn clone_plot_row(row: &PlotRow) -> PlotRow {
    PlotRow {
        order: row.order,
        name: row.name.clone(),
        province: row.province.clone(),
        municipality: row.municipality.clone(),
        municipality_name: row.municipality_name.clone(),
        aggregate: row.aggregate.clone(),
        zone: row.zone.clone(),
        polygon: row.polygon.clone(),
        parcel: row.parcel.clone(),
        enclosure: row.enclosure.clone(),
        land_use: row.land_use.clone(),
        sigpac_area_ha: row.sigpac_area_ha,
        cultivated_area_ha: row.cultivated_area_ha,
        species: row.species.clone(),
        variety: row.variety.clone(),
        irrigation: row.irrigation,
        environment: row.environment,
        gip: row.gip,
    }
}

/// The model's sigla for an irrigation-system code — 2.1 footnote 3: (SEC)
/// secano, (ASP) aspersión, (LOC) goteo o localizado, (GRA) por gravedad.
/// A code, not prose: the same four letters print in every language.
fn irrigation_abbrev(code: Option<&str>) -> &'static str {
    match code {
        Some("rainfed") => "SEC",
        Some("sprinkler") => "ASP",
        Some("drip") => "LOC",
        Some("gravity") => "GRA",
        _ => "",
    }
}

/// The model's sigla for a growing-environment code — 2.1 footnote 4: (AL)
/// aire libre, (M) malla, (BP) cubierta bajo plástico, (INV) invernadero.
fn environment_abbrev(code: Option<&str>) -> &'static str {
    match code {
        Some("open_air") => "AL",
        Some("mesh") => "M",
        Some("plastic_cover") => "BP",
        Some("greenhouse") => "INV",
        _ => "",
    }
}

/// What a provider lookup recorded for a plot: the SIGPAC land-use code and
/// the official surface.
struct SigpacFacts {
    land_use: Option<String>,
    official_area_ha: Option<f64>,
}

/// Provider-fetched boundary facts per plot. Attributes ride `geo_feature`
/// as source-tagged JSON, so the key is read here rather than through
/// module-sigpac — modules never depend on each other, and this document is
/// the Spanish cuaderno, where a `source = 'sigpac'` row means exactly this.
/// A plot never verified simply has no entry and prints blank.
pub(crate) fn sigpac_facts(
    conn: &Connection,
    farm_id: &str,
) -> Result<HashMap<String, SigpacFacts>> {
    let features = terrazgo_core::repository::list_geo_features_for_farm(conn, farm_id)?;
    let mut facts = HashMap::new();
    for feature in features {
        let (Some(plot_id), "boundary", "sigpac") = (
            feature.plot_id,
            feature.role.as_str(),
            feature.source.as_str(),
        ) else {
            continue;
        };
        let land_use = feature
            .properties
            .as_deref()
            .and_then(|json| serde_json::from_str::<Value>(json).ok())
            .and_then(|props| {
                props
                    .get("uso_sigpac")
                    .and_then(Value::as_str)
                    .map(String::from)
            });
        facts.insert(
            plot_id,
            SigpacFacts {
                land_use,
                official_area_ha: feature.official_area_ha,
            },
        );
    }
    Ok(facts)
}

// ---------------------------------------------------------------------------
// Section 2.2 — datos medioambientales (zones half)
// ---------------------------------------------------------------------------

/// One row per plot: the crops on it, the abstraction points for human
/// consumption near it (Anexo III A.1.f–g) and what the zone check found.
///
/// BOTH halves distinguish a stated negative from silence, for the same reason
/// and by different means. The zone flags store `status='outside'` when a
/// provider check came back clear; the water half stores a farmer's declaration
/// that a plot has none. Either way "checked, and nothing" is proof the question
/// was asked, which a blank cell cannot express — and a plot nobody has looked
/// at prints blank, because silence is not the same claim.
struct ZoneRow {
    order: usize,
    /// Only the 2.2 Captaciones tab prints this: the PDF's 2.2 table
    /// cross-references plots by order number, as the model does.
    plot_name: String,
    species: String,
    variety: String,
    /// The plot's water points, in capture order. Several on one plot join
    /// positionally across the four printed cells, so the columns read across.
    water: Vec<WaterCell>,
    /// `Some(date)` = the farmer stated this plot has no abstraction point.
    /// Only consulted when `water` is empty — a record withdraws the
    /// declaration as it lands, so the two cannot both be present.
    water_declared_on: Option<String>,
    /// `None` = the plot was never checked. `Some(false)` is a real negative:
    /// checked, and not affected. Only the first is a blank cell.
    fully: Option<bool>,
    partly: Option<bool>,
    /// What the check found, as values — `None` when the plot was never
    /// checked. The wording ("Sin afección — campaña 2026") is a renderer's
    /// job, so the same finding can print in either language.
    check: Option<ZoneCheckSummary>,
}

/// One abstraction point as section 2.2 prints it, kept as values so the sheet
/// can type them and the PDF can join them.
struct WaterCell {
    denomination: String,
    inside_plot: bool,
    /// Metres, present exactly when the point lies outside the plot.
    distance_m: Option<f64>,
    coordinates: Option<(f64, f64)>,
}

impl ZoneRow {
    /// The four water cells of section 2.2, in printed order: included in the
    /// plot, distance, coordinates, denomination.
    ///
    /// Several points on one plot join positionally — blanks are KEPT, unlike
    /// the species join above, so the four columns can be read across as one
    /// point per position. A declared-empty plot states so in the denomination
    /// cell alone: writing "NO" in the first column would assert that a point
    /// exists somewhere outside the plot, which is the opposite claim.
    fn water_cells(&self, labels: &Labels) -> [String; 4] {
        if self.water.is_empty() {
            let denomination = match &self.water_declared_on {
                Some(date) => format!("{} — {}", labels.value.no_water_points, format_date(date)),
                None => String::new(),
            };
            return [String::new(), String::new(), String::new(), denomination];
        }
        // Positions must line up across the four columns, so an empty slot is
        // kept — but printed as a dash rather than as nothing, because a cell
        // reading "; 240" looks like a stray separator when it actually says
        // "the first point states no distance, the second is at 240 m". With a
        // single point there is no position to hold and a blank is unambiguous.
        let several = self.water.len() > 1;
        let join = move |values: Vec<String>| {
            values
                .into_iter()
                .map(|value| match (value.is_empty(), several) {
                    (true, true) => "—".to_string(),
                    _ => value,
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        [
            join(
                self.water
                    .iter()
                    .map(|w| labels.yes_no(w.inside_plot).to_string())
                    .collect(),
            ),
            join(
                self.water
                    .iter()
                    .map(|w| w.distance_m.map(format_number).unwrap_or_default())
                    .collect(),
            ),
            join(
                self.water
                    .iter()
                    .map(|w| w.coordinates.map(format_coordinates).unwrap_or_default())
                    .collect(),
            ),
            join(self.water.iter().map(|w| w.denomination.clone()).collect()),
        ]
    }
}

/// The zone check for one plot: which campaign answered, and which zones the
/// plot falls inside (with the share covered, when the service reported one).
/// An empty `affecting` list is the checked NEGATIVE — the proof that the
/// question was asked, which a blank cell cannot express.
struct ZoneCheckSummary {
    campaign: i64,
    /// `(zone_type_code, coverage_pct)`, unordered — the renderer sorts by the
    /// printed label, which differs per language.
    affecting: Vec<(String, Option<f64>)>,
}

impl ZoneCheckSummary {
    /// "Vulnerable a nitratos (50 %); Red Natura 2000 — campaña 2026", or the
    /// negative when nothing affects the plot.
    fn render(&self, labels: &Labels, collator: &NameCollator) -> String {
        let body = if self.affecting.is_empty() {
            labels.value.no_affection.to_string()
        } else {
            let mut named: Vec<String> = self
                .affecting
                .iter()
                .map(|(code, pct)| {
                    let share = pct
                        .map(|p| format!(" ({} %)", format_number(p)))
                        .unwrap_or_default();
                    format!("{}{share}", labels.zone(code))
                })
                .collect();
            // These are TRANSLATED zone names, so they must be ordered by the
            // book's own language — sorting Catalan prose with a Castilian
            // collator would be a quieter version of the same defect.
            collator.sort_owned(&mut named);
            named.join("; ")
        };
        format!("{body} — {} {}", labels.value.campaign_word, self.campaign)
    }
}

fn zone_rows(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
    language: ReportLanguage,
) -> Result<Vec<ZoneRow>> {
    // §2.1's species/variety cells join positionally, so the crops are sorted
    // as structs — sorting the two strings separately would pair a species with
    // another crop's variety. Collated for the same reason as the plot rows.
    let collator = NameCollator::new(language);
    let plots = terrazgo_core::repository::list_plots(conn, farm_id)?;
    let crops = terrazgo_core::repository::list_crops(conn, season_id, farm_id)?;
    let flags = terrazgo_core::repository::list_zone_flags_for_farm(conn, farm_id)?;
    let water = terrazgo_core::repository::list_water_points(conn, farm_id)?;
    let declared = terrazgo_core::repository::list_water_declarations(conn, farm_id)?;

    let mut rows = Vec::new();
    for (idx, detail) in plots.iter().enumerate() {
        let mut plot_crops: Vec<_> = crops
            .iter()
            .filter(|c| c.plot_id == detail.plot.id && c.deleted_at.is_none())
            .collect();
        plot_crops.sort_by(|a, b| collator.compare(&a.species_name, &b.species_name));
        let join = |values: Vec<String>| {
            let kept: Vec<_> = values.into_iter().filter(|v| !v.is_empty()).collect();
            kept.join("; ")
        };

        // Latest campaign wins per (plot, zone kind) — the same candidate rule
        // the alert engine uses; older campaigns stay as history.
        let mut latest: HashMap<&str, &terrazgo_core::models::ZoneFlag> = HashMap::new();
        for flag in flags.iter().filter(|f| f.plot_id == detail.plot.id) {
            latest
                .entry(flag.zone_type_code.as_str())
                .and_modify(|kept| {
                    if flag.campaign > kept.campaign {
                        *kept = flag;
                    }
                })
                .or_insert(flag);
        }

        let (mut fully, mut partly, mut check) = (None, None, None);
        if !latest.is_empty() {
            let campaign = latest
                .values()
                .map(|f| f.campaign)
                .max()
                .unwrap_or_default();
            let affecting: Vec<(String, Option<f64>)> = latest
                .values()
                .filter(|f| f.status == "inside")
                .map(|f| (f.zone_type_code.clone(), f.coverage_pct))
                .collect();

            if affecting.is_empty() {
                (fully, partly) = (Some(false), Some(false));
            } else {
                // Total only when every affecting zone covers the whole plot;
                // an unknown percentage counts as partial, never as total.
                let total = affecting
                    .iter()
                    .all(|(_, pct)| pct.is_some_and(|p| p >= 99.5));
                (fully, partly) = (Some(total), Some(!total));
            }
            check = Some(ZoneCheckSummary {
                campaign,
                affecting,
            });
        }

        let water_cells: Vec<WaterCell> = water
            .iter()
            .filter(|p| p.plot_id == detail.plot.id)
            .map(|p| WaterCell {
                denomination: p.denomination.clone(),
                inside_plot: p.inside_plot,
                distance_m: p.distance_m,
                coordinates: p.latitude.zip(p.longitude),
            })
            .collect();
        // Only meaningful with no points: recording one withdraws the
        // declaration, so the repository never lets both stand.
        let water_declared_on = declared
            .iter()
            .find(|d| d.plot_id == detail.plot.id)
            .map(|d| d.declared_on.clone());

        rows.push(ZoneRow {
            order: idx + 1,
            plot_name: detail.plot.name.clone(),
            species: join(plot_crops.iter().map(|c| c.species_name.clone()).collect()),
            variety: join(
                plot_crops
                    .iter()
                    .map(|c| c.variety.clone().unwrap_or_default())
                    .collect(),
            ),
            water: water_cells,
            water_declared_on,
            fully,
            partly,
            check,
        });
    }
    Ok(rows)
}

/// The model's sigla for a GIP framework code (RD 1311/2012 art. 10-11),
/// printed in 1.4 ("tipo de explotación") and again per row in 2.1. Named
/// `_abbrev` and NOT `gip_code`, which would collide with the schema's
/// `gip_system_code`, a different value ("organic", not "AE").
fn gip_abbrev(code: Option<&str>) -> &'static str {
    match code {
        Some("organic") => "AE",
        Some("integrated_production") => "PI",
        Some("private_certification") => "CP",
        Some("atria") => "Atrias",
        Some("advisor_assisted") => "AS",
        Some("not_required") => "NO",
        _ => "",
    }
}

/// The GIP sigla for one crop row, ready to print. A crop that states its framework is
/// believed; otherwise the production system still implies two of them
/// (ecológico → AE, producción integrada → PI), which keeps the column
/// printing for books entered before the field existed. Conventional farming
/// implies nothing — it prints blank rather than claiming "NO", because "sin
/// obligación de asesor" is a declaration, not a default.
fn crop_gip_abbrev(gip_system: Option<&str>, production_system: Option<&str>) -> &'static str {
    match gip_system {
        Some(code) => gip_abbrev(Some(code)),
        None => match production_system {
            Some("organic") => "AE",
            Some("integrated") => "PI",
            _ => "",
        },
    }
}

// ---------------------------------------------------------------------------
// Section 1.4 — asesor, agrupación o entidad de asesoramiento
// ---------------------------------------------------------------------------

/// One row of model table 1.4: an advisory relationship of this holding.
struct AdvisorRow {
    name: String,
    tax_id: String,
    /// The model's "Nº de identificación" (ROPO inscription in Spain).
    registration_number: String,
    /// Model sigla of the framework the relationship runs under (AE/PI/CP/
    /// Atrias/AS/NO), already resolved.
    gip: &'static str,
    /// The framework's code, for the spreadsheet's own filtering.
    gip_code: String,
}

fn advisor_rows(
    conn: &Connection,
    farm_id: &str,
    language: ReportLanguage,
) -> Result<Vec<AdvisorRow>> {
    let mut links = terrazgo_core::repository::list_farm_advisors(conn, farm_id)?;
    // Collated for the same reason as §2.1: the repository returns BINARY
    // order, and the registry screen sorts the same names with Intl.Collator.
    let collator = NameCollator::new(language);
    links.sort_by(|a, b| collator.compare(&a.advisor.name, &b.advisor.name));
    Ok(links
        .into_iter()
        .map(|detail| AdvisorRow {
            name: detail.advisor.name,
            tax_id: detail.advisor.tax_id.unwrap_or_default(),
            registration_number: detail.advisor.registration_number.unwrap_or_default(),
            gip: gip_abbrev(detail.link.gip_system_code.as_deref()),
            gip_code: detail.link.gip_system_code.unwrap_or_default(),
        })
        .collect())
}

/// Tax ids compared for identity, not for display: case and the separators
/// people type differently (spaces, dots, hyphens) must not decide whether a
/// person is recognised as an advisor.
fn normalise_tax_id(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

// ---------------------------------------------------------------------------
// Sections 1.2 / 1.3 — who and what applied the treatments
// ---------------------------------------------------------------------------

struct OperatorRow {
    operator_id: String,
    order: usize,
    name: String,
    /// Model 1.2's NIF column; read from the operator row (never snapshotted —
    /// an identity number is not a value a past record freezes).
    tax_id: String,
    licence: String,
    level: Option<String>,
    /// Model 1.2's separate "Asesor" cross. Advising is not a carné level:
    /// ROPO registers applicators and advisors as different conditions, so the
    /// cross is a match against the advisor registry by NIF, never a value
    /// read off the operator's licence.
    is_advisor: bool,
}

/// Operators as the records name them — identity is the FK, display values
/// are the record snapshots (the legal values), latest record wins when an
/// operator was edited between treatments. Order = first appearance in the
/// chronological register. The carné level is not snapshotted (only the
/// number is), so it reads from the operator row, blank if gone.
///
/// `advisors` is the whole active advisor registry, not just this farm's
/// links: the "Asesor" cross states what the person IS, and someone the app
/// knows as an advisor is one whichever holding they were advising.
fn operator_rows(
    conn: &Connection,
    records: &[TreatmentRecordWithPlots],
    advisors: &[terrazgo_core::models::Advisor],
) -> Vec<OperatorRow> {
    let advisor_tax_ids: Vec<String> = advisors
        .iter()
        .filter_map(|a| a.tax_id.as_deref())
        .map(normalise_tax_id)
        .filter(|id| !id.is_empty())
        .collect();
    let mut rows: Vec<OperatorRow> = Vec::new();
    for rec in records {
        let record = &rec.record;
        match rows
            .iter_mut()
            .find(|o| o.operator_id == record.operator_id)
        {
            Some(row) => {
                row.name = record.operator_name_snapshot.clone();
                row.licence = record.operator_licence_snapshot.clone().unwrap_or_default();
            }
            None => {
                let current: Option<(Option<String>, Option<String>)> = conn
                    .query_row(
                        "SELECT licence_level_code, tax_id FROM operator WHERE id = ?1",
                        [&record.operator_id],
                        |r| Ok((r.get(0)?, r.get(1)?)),
                    )
                    .ok();
                let (level, tax_id) = current.unwrap_or((None, None));
                let tax_id = tax_id.unwrap_or_default();
                let normalised = normalise_tax_id(&tax_id);
                rows.push(OperatorRow {
                    operator_id: record.operator_id.clone(),
                    order: rows.len() + 1,
                    name: record.operator_name_snapshot.clone(),
                    // No NIF, no claim: an empty id must not match an advisor
                    // whose own id is missing.
                    is_advisor: !normalised.is_empty() && advisor_tax_ids.contains(&normalised),
                    tax_id,
                    licence: record.operator_licence_snapshot.clone().unwrap_or_default(),
                    level,
                });
            }
        }
    }
    rows
}

struct MachineryRow {
    machinery_id: String,
    order: usize,
    description: String,
    roma: String,
    reganip: String,
    /// ISO `YYYY-MM-DD`; formatted per output. Anexo III A.1.h accepts either
    /// date, so both print.
    acquired_on: Option<String>,
    last_inspection: Option<String>,
}

/// Equipment as the records name it: registry numbers from the snapshots,
/// description and inspection date from the current row when it still
/// exists (a deleted machine keeps printing through its snapshots).
fn machinery_rows(
    conn: &Connection,
    records: &[TreatmentRecordWithPlots],
) -> Result<Vec<MachineryRow>> {
    let mut rows: Vec<MachineryRow> = Vec::new();
    for rec in records {
        let record = &rec.record;
        let Some(machinery_id) = &record.machinery_id else {
            continue; // manual application — no 1.3 entry
        };
        if rows.iter().any(|m| &m.machinery_id == machinery_id) {
            continue;
        }
        let current: Option<(String, Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT name, acquired_on, last_inspection_date FROM machinery WHERE id = ?1",
                [machinery_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .ok();
        let (description, acquired_on, last_inspection) = match current {
            Some((name, acquired, inspection)) => (name, acquired, inspection),
            None => (String::new(), None, None),
        };
        rows.push(MachineryRow {
            machinery_id: machinery_id.clone(),
            order: rows.len() + 1,
            description,
            acquired_on,
            roma: record.machinery_roma_snapshot.clone().unwrap_or_default(),
            reganip: record
                .machinery_reganip_snapshot
                .clone()
                .unwrap_or_default(),
            last_inspection,
        });
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Section 3.1 — the register
// ---------------------------------------------------------------------------

/// One row of model table 3.1.
struct TreatmentRow {
    /// Order numbers from table 2.1, ascending.
    plot_orders: Vec<usize>,
    /// The same plots by name — the spreadsheet resolves what the PDF only
    /// cross-references, so a sheet is filterable without a lookup table.
    plot_names: String,
    species: String,
    variety: String,
    /// ISO `YYYY-MM-DD`; the first day of the actuation.
    date: String,
    /// Last day, when the actuation ran over several (Anexo III Parte I B lets
    /// the date be an interval). `None` = a single-day treatment, which prints
    /// as one date rather than a range of one.
    end_date: Option<String>,
    /// Start hour as local wall-clock `HH:MM` (Reglamento (UE) 2023/564's
    /// annex, "where relevant"). No Spanish form has a column for it, so it
    /// folds into the date cell. `None` prints nothing.
    time: Option<String>,
    /// Model 9.3's "fecha de seca": the day the flooded field was dried so this
    /// treatment could be applied (RD 1048/2022 art. 45.2). It rides in the
    /// treatments tab because that is the register that owns the fact — the
    /// field is dried in order to spray. `None` = not a flooded crop.
    drying_date: Option<String>,
    /// The treated crop's growth stages, resolved (the stored code is not the
    /// BBCH number). The annex places the stage inside the "Crop or
    /// situation/land use" column, so the PDF folds the BBCH number into the
    /// species cell while the sheet takes the full names.
    ///
    /// A list, because the stage belongs to `treatment_plot` and one printed
    /// row can span several plots of the same species and variety: when they
    /// disagree, every stage is carried. Printing the first plot's would assert
    /// something false about the others, and blanking it would throw away what
    /// was recorded.
    growth_stages: Vec<module_cue::catalogue::GrowthStage>,
    surface_ha: f64,
    /// The crop's own surface (model 3.1 bis has a "Superf. cultivada" column
    /// beside the treated one). `None` when the crop states none — blank, not
    /// the plot's area, which slice 3 established is a different figure.
    crop_area_ha: Option<f64>,
    problems: String,
    operator_order: Option<usize>,
    operator_name: String,
    /// `None` = manual application, which is a value in itself (model 3.1
    /// footnote 3), not missing data.
    equipment_order: Option<usize>,
    equipment_name: String,
    /// The chemical half, absent for a purely non-chemical actuation. The
    /// register's own title is "actuaciones fitosanitarias", not product
    /// applications, so a measure taken instead of a spray belongs on this
    /// page with these cells blank — leaving it out would make the binding
    /// register an incomplete account of what was done.
    product: String,
    reg_no: String,
    dose_value: Option<f64>,
    dose_unit_code: Option<String>,
    /// Total product used over the whole actuation (Anexo III Parte I B.i).
    /// `None` prints blank — a total is a measurement, and zero would be a
    /// statement the farmer never made.
    total_quantity_value: Option<f64>,
    total_quantity_unit_code: Option<String>,
    /// Derived from the product, so both are absent with it: a non-chemical
    /// measure imposes no plazo de seguridad, and printing "0 días" would
    /// assert a waiting period that was calculated rather than one that does
    /// not exist.
    phi_days: Option<i64>,
    /// ISO; the first day harvest is allowed again.
    phi_end_date: Option<String>,
    efficacy_code: Option<String>,
    notes: String,
    /// The advisor who directed the actuation (Anexo III Parte I B.d) and the
    /// non-chemical measure taken — what model table 3.1 bis prints, and what
    /// decides whether a row appears there at all.
    advisor_name: Option<String>,
    advisor_registration: Option<String>,
    measure_code: Option<String>,
    measure_intensity_value: Option<f64>,
    measure_intensity_unit_code: Option<String>,
    measure_registration_number: Option<String>,
    /// The measure's official label, resolved against
    /// `TIPO_MEDIDA_FITOSANITARIA`; an unresolvable code prints itself.
    measure_label: String,
    /// The coded IPM justifications. Kept as CODES, worded by the renderer
    /// like `efficacy_code`: the assembly is language-neutral and prose is the
    /// language's job (the report-language rule).
    justification_codes: Vec<String>,
}

impl TreatmentRow {
    /// Whether this row belongs on model page 3.1 bis, which is headed
    /// "solamente para cultivos y superficies objeto de asesoramiento".
    ///
    /// Derived from the record rather than from a flag on the crop: a row
    /// carrying an advisor or a non-chemical measure IS an advised
    /// intervention, and nothing has to be kept in sync for that to stay true.
    /// A crop whose treatments name no advisor simply contributes nothing —
    /// which is a true statement about what was recorded, not a gap.
    fn is_advised(&self) -> bool {
        self.advisor_name.is_some() || self.measure_code.is_some()
    }

    /// What a register cell prints for the growth stage: "BBCH 5", or "BBCH 4 /
    /// 5" when the row's plots were at different ones. Blank when none was
    /// stated, so `join_detail` leaves the species cell alone.
    ///
    /// "BBCH" is the monograph's own name — a proper noun like the registry
    /// names SIGPAC and ROMA — so it is not in `Labels` and does not translate.
    fn bbch_stages(&self) -> String {
        if self.growth_stages.is_empty() {
            return String::new();
        }
        let stages: Vec<&str> = self
            .growth_stages
            .iter()
            .map(|stage| stage.bbch.as_str())
            .collect();
        format!("BBCH {}", stages.join(" / "))
    }

    /// What the spreadsheet's own column holds: FEGA's full wording, where
    /// there is room for it and a reader can filter on a name.
    fn stage_names(&self) -> String {
        self.growth_stages
            .iter()
            .map(|stage| stage.label.as_str())
            .collect::<Vec<_>>()
            .join(" / ")
    }
}

impl Cuaderno {
    /// The advisor to prefill into 3.1 bis's two validation boxes: the one
    /// named by the page's own rows, and only when every row that names an
    /// advisor names the same one. Returns (name, registration number).
    ///
    /// The comparison is over the PRINTED PAIR — name and registration number
    /// together — rather than over `advisor_id`, because the boxes have to
    /// agree with the table printed directly above them, and that table prints
    /// snapshots frozen at write time. An advisor whose registry entry is
    /// corrected mid-season leaves rows carrying one name and two different
    /// ROPO numbers: comparing ids would prefill one of them against a
    /// signature nobody gave, which is the book asserting what it cannot know
    /// (the 1.1 signature-box rule). Comparing names alone would print one
    /// person's name beside another's number.
    ///
    /// A row carrying only a non-chemical measure nominates nobody, so it
    /// neither confirms nor contradicts and does not blank the boxes — the page
    /// still names exactly one advisor, and the human signing it is the one
    /// asserting the page is theirs.
    fn advised_advisor(&self) -> Option<(String, String)> {
        let mut named = self
            .treatments
            .iter()
            .filter(|t| t.is_advised())
            .filter_map(|t| {
                t.advisor_name
                    .as_deref()
                    .map(|name| (name, t.advisor_registration.as_deref().unwrap_or_default()))
            });
        let first = named.next()?;
        if named.any(|pair| pair != first) {
            return None;
        }
        Some((first.0.to_string(), first.1.to_string()))
    }
}

/// The growth stages a printed row's plots were at, resolved once and kept in
/// both of their renderings.
///
/// One row can span several treated plots (they share a species and variety,
/// which is what groups them), and Reglamento (UE) 2023/564 attaches the stage
/// to the crop rather than to the record — so an actuation running over two
/// days can legitimately have caught them at different stages. Every distinct
/// stage is listed, in the order the plots come in: taking the first would
/// state something false about the rest, and blanking the cell would discard a
/// value the farmer recorded.
fn growth_stages_of(
    conn: &Connection,
    catalogues: &CatalogueCache,
    group: &[&module_cue::models::TreatmentPlot],
) -> Vec<module_cue::catalogue::GrowthStage> {
    let mut stages: Vec<module_cue::catalogue::GrowthStage> = Vec::new();
    for plot in group {
        let Some(code) = plot.growth_stage_code.as_deref() else {
            continue;
        };
        let rows = catalogues.find(conn, module_cue::catalogue::GROWTH_STAGE_CATALOGUE, code);
        let stage = module_cue::catalogue::growth_stage_from(rows.first(), code);
        if !stage.label.is_empty() && !stages.contains(&stage) {
            stages.push(stage);
        }
    }
    stages
}

/// One printed row per (record, crop-snapshot group) — the same split the
/// SIEX export applies, so the paper register and the electronic one always
/// carry the same line items.
fn treatment_rows(
    conn: &Connection,
    catalogues: &CatalogueCache,
    records: &[TreatmentRecordWithPlots],
    plots: &PlotIndex,
    operators: &[OperatorRow],
    machinery: &[MachineryRow],
    crop_areas: &HashMap<String, f64>,
) -> Result<Vec<TreatmentRow>> {
    let mut rows = Vec::new();
    for rec in records {
        let record = &rec.record;
        let problems = problem_labels(conn, catalogues, rec)?;
        let justification_codes: Vec<String> = rec
            .justifications
            .iter()
            .map(|j| j.justification_code.clone())
            .collect();
        let operator = operators
            .iter()
            .find(|o| o.operator_id == record.operator_id);
        let equipment = record
            .machinery_id
            .as_ref()
            .and_then(|id| machinery.iter().find(|m| &m.machinery_id == id));

        for (_, group) in crop_groups(&rec.plots) {
            let mut order_refs: Vec<usize> = group
                .iter()
                .filter_map(|p| plots.orders.get(&p.plot_id).copied())
                .collect();
            order_refs.sort_unstable();
            let mut names: Vec<&str> = group
                .iter()
                .filter_map(|p| plots.names.get(&p.plot_id).map(String::as_str))
                .collect();
            plots.collator.sort(&mut names);
            let surface: f64 = group.iter().map(|p| p.surface_treated_ha).sum();
            let first = group.first();
            // "Superf. cultivada" is the crop's own stated area (slice 5's
            // `crop.area_ha`), summed when the group spans several plots. A
            // crop absent from the map states none — which collapses the two
            // absences that mean the same thing here (no crop linked, and a
            // crop whose own area is NULL) into one blank cell. Never the plot
            // area, which is a different figure.
            let mut crop_area: Option<f64> = None;
            for p in &group {
                if let Some(area) = p.crop_id.as_ref().and_then(|id| crop_areas.get(id)) {
                    crop_area = Some(crop_area.unwrap_or(0.0) + area);
                }
            }
            rows.push(TreatmentRow {
                plot_orders: order_refs,
                plot_names: names.join(", "),
                species: first
                    .and_then(|p| p.crop_name_snapshot.clone())
                    .unwrap_or_default(),
                variety: first
                    .and_then(|p| p.variety_snapshot.clone())
                    .unwrap_or_default(),
                date: record.application_date.clone(),
                end_date: record.application_end_date.clone(),
                time: record.application_time.clone(),
                drying_date: record.drying_date.clone(),
                growth_stages: growth_stages_of(conn, catalogues, &group),
                surface_ha: surface,
                crop_area_ha: crop_area,
                problems: problems.clone(),
                operator_order: operator.map(|o| o.order),
                operator_name: record.operator_name_snapshot.clone(),
                equipment_order: equipment.map(|m| m.order),
                equipment_name: equipment.map(|m| m.description.clone()).unwrap_or_default(),
                product: record.product_name_snapshot.clone().unwrap_or_default(),
                reg_no: record
                    .authorisation_number_snapshot
                    .clone()
                    .unwrap_or_default(),
                dose_value: record.dose_value,
                dose_unit_code: record.dose_unit_code.clone(),
                total_quantity_value: record.total_quantity_value,
                total_quantity_unit_code: record.total_quantity_unit_code.clone(),
                phi_days: record.phi_days_used,
                phi_end_date: record.phi_end_date.clone(),
                efficacy_code: record.efficacy_code.clone(),
                notes: record.notes.clone().unwrap_or_default(),
                advisor_name: record.advisor_name_snapshot.clone(),
                advisor_registration: record.advisor_registration_snapshot.clone(),
                measure_code: record.measure_code.clone(),
                measure_intensity_value: record.measure_intensity_value,
                measure_intensity_unit_code: record.measure_intensity_unit_code.clone(),
                measure_registration_number: record.measure_registration_number.clone(),
                measure_label: match &record.measure_code {
                    Some(code) => catalogue_label(
                        conn,
                        catalogues,
                        Some("TIPO_MEDIDA_FITOSANITARIA"),
                        code,
                        None,
                    ),
                    None => String::new(),
                },
                justification_codes: justification_codes.clone(),
            });
        }
    }
    Ok(rows)
}

/// Problem codes resolved to their official Spanish catalogue labels,
/// joined "; ". A code the imported catalogues cannot resolve (or a test
/// database without catalogues) prints the code itself — the record's legal
/// payload is the code, the label is display sugar.
fn problem_labels(
    conn: &Connection,
    catalogues: &CatalogueCache,
    rec: &TreatmentRecordWithPlots,
) -> Result<String> {
    let mut labels = Vec::new();
    for problem in &rec.problems {
        let label = catalogue_label(
            conn,
            catalogues,
            siex::problem_catalogue(&rec.record.country_code, &problem.reason_category_code),
            &problem.problem_code,
            None,
        );
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    Ok(labels.join("; "))
}

// ---------------------------------------------------------------------------
// Sections 3.3 / 3.4 / 3.5 — postcosecha, locales, medios de transporte
//
// One assembly for three printed tables: the sections differ only in what was
// treated and what it is measured in, so each table filters this list by
// `subject_kind` rather than reading its own query.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Section 3.2 — uso de semilla tratada
//
// A sowing, not an application: the product travels as the text printed on the
// sack, and the plots come from the model (the SIEX twin carries none).
// ---------------------------------------------------------------------------

struct SeedRow {
    /// Order numbers from table 2.1, ascending — the model's "Id. parcelas".
    plot_orders: Vec<usize>,
    /// The same plots by name, so the sheet filters without a lookup table.
    plot_names: String,
    /// ISO `YYYY-MM-DD`.
    date: String,
    species: String,
    variety: String,
    /// Total surface sown across the listed plots.
    surface_ha: f64,
    /// Kilograms of seed. `None` prints blank.
    seed_quantity_kg: Option<f64>,
    seed_lot: String,
    /// Where the seed was treated. `None` prints nothing: the model has no such
    /// column, so an unstated kind is not a gap in the form.
    treatment_kind_code: Option<String>,
    product: String,
    reg_no: String,
    active_substance: String,
    efficacy_code: Option<String>,
    notes: String,
}

fn seed_rows(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
    plots: &PlotIndex,
) -> Result<Vec<SeedRow>> {
    let mut rows = Vec::new();
    for detail in list_seed_treatments(conn, season_id, farm_id)? {
        let record = detail.record;
        let (orders, names) =
            plot_cross_reference(detail.plots.iter().map(|p| p.plot_id.as_str()), plots);
        rows.push(SeedRow {
            plot_orders: orders,
            plot_names: names,
            date: record.sown_on,
            species: record.species_name,
            variety: record.variety.unwrap_or_default(),
            surface_ha: detail.plots.iter().map(|p| p.surface_sown_ha).sum(),
            seed_quantity_kg: record.seed_quantity_kg,
            seed_lot: record.seed_lot.unwrap_or_default(),
            treatment_kind_code: record.treatment_kind_code,
            product: record.product_name,
            reg_no: record.product_registration_number.unwrap_or_default(),
            active_substance: record.product_active_substance.unwrap_or_default(),
            efficacy_code: record.efficacy_code,
            notes: record.notes.unwrap_or_default(),
        });
    }
    Ok(rows)
}

/// The three non-field registers, in the order the model prints them.
const NON_FIELD_KINDS: [&str; 3] = ["postharvest", "storage_premises", "transport"];

struct NonFieldRow {
    /// `postharvest` | `storage_premises` | `transport`.
    subject_kind: String,
    /// ISO `YYYY-MM-DD`.
    date: String,
    /// The produce, the premises, or the vehicle — in each section's own terms.
    subject: String,
    /// How much of it: tonnes for produce, cubic metres for premises and
    /// vehicles. `None` prints blank; the form leaves the cell hand-fillable.
    quantity_value: Option<f64>,
    quantity_unit_code: Option<String>,
    problems: String,
    product: String,
    reg_no: String,
    /// Product used, in kilograms or litres.
    product_quantity_value: Option<f64>,
    product_quantity_unit_code: Option<String>,
    /// Named outright rather than by an order number: unlike section 3.1, the
    /// printed 3.3/3.4/3.5 tables carry no cross-reference column.
    operator_name: String,
    /// The advisor, when there was one. Anexo III B.d names them in the same
    /// sentence as the applicator — "identificación del aplicador y, en su
    /// caso, del asesor" — and B reaches these registers through B.b, so the
    /// printed cell keeps the pair together and the sheet splits them into
    /// columns of their own.
    advisor_name: String,
    advisor_registration: String,
    efficacy_code: Option<String>,
    notes: String,
}

fn non_field_rows(
    conn: &Connection,
    catalogues: &CatalogueCache,
    season_id: &str,
    farm_id: &str,
) -> Result<Vec<NonFieldRow>> {
    let mut rows = Vec::new();
    for detail in list_non_field_treatments(conn, season_id, farm_id)? {
        let record = &detail.record;
        let problems = non_field_problem_labels(conn, catalogues, &detail)?;
        rows.push(NonFieldRow {
            subject_kind: record.subject_kind_code.clone(),
            date: record.treated_on.clone(),
            subject: record.subject_description.clone(),
            quantity_value: record.treated_quantity_value,
            quantity_unit_code: record.treated_quantity_unit_code.clone(),
            problems,
            product: record.product_name_snapshot.clone(),
            reg_no: record
                .authorisation_number_snapshot
                .clone()
                .unwrap_or_default(),
            product_quantity_value: record.product_quantity_value,
            product_quantity_unit_code: record.product_quantity_unit_code.clone(),
            operator_name: record.operator_name_snapshot.clone(),
            advisor_name: record.advisor_name_snapshot.clone().unwrap_or_default(),
            advisor_registration: record
                .advisor_registration_snapshot
                .clone()
                .unwrap_or_default(),
            efficacy_code: record.efficacy_code.clone(),
            notes: record.notes.clone().unwrap_or_default(),
        });
    }
    Ok(rows)
}

/// The same catalogue resolution section 3.1 does: the stored code is the
/// regulatory payload, the label is display sugar, and an unresolvable code
/// prints itself.
fn non_field_problem_labels(
    conn: &Connection,
    catalogues: &CatalogueCache,
    detail: &module_cue::models::NonFieldTreatmentDetail,
) -> Result<String> {
    let mut labels = Vec::new();
    for problem in &detail.problems {
        let label = catalogue_label(
            conn,
            catalogues,
            siex::problem_catalogue(&detail.record.country_code, &problem.reason_category_code),
            &problem.problem_code,
            None,
        );
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    Ok(labels.join("; "))
}

// ---------------------------------------------------------------------------
// Section 4 — registro de análisis
//
// Metadata only: what was analysed, by whom, and under which bulletin number
// the result can be found. The bulletin itself stays in the farmer's folder.
// ---------------------------------------------------------------------------

struct AnalysisRow {
    /// Order numbers from table 2.1, ascending — the model's cross-reference.
    plot_orders: Vec<usize>,
    /// The same plots by name, so the sheet filters without a lookup table.
    plot_names: String,
    /// ISO `YYYY-MM-DD`.
    date: String,
    /// Schema code; worded at render time, like every other coded value.
    material_kind: String,
    /// Schema codes for what the laboratory looked for, in catalogue order.
    analysis_types: Vec<String>,
    bulletin: String,
    lab_name: String,
    lab_address: String,
    lab_tax_id: String,
    /// The coded findings, already resolved against the FEGA catalogue — an
    /// unresolvable code stays as itself, the `problem_code` rule.
    substances_coded: String,
    /// The farmer's own wording, which the coded list cannot always replace:
    /// SUST_ACTIVAS codes phytosanitary actives only.
    substances: String,
    /// Anexo III A.3, when the bulletin carried soil figures. The printed
    /// model predates A.3 and has no soil page, so these ride in the findings
    /// cell — and take a workbook tab of their own, where nine figures can be
    /// compared instead of read (the analysis-kinds precedent).
    soil: module_cue::models::SoilParameters,
    notes: String,
}

fn analysis_rows(
    conn: &Connection,
    catalogues: &CatalogueCache,
    season_id: &str,
    farm_id: &str,
    country_code: &str,
    plots: &PlotIndex,
) -> Result<Vec<AnalysisRow>> {
    let mut rows = Vec::new();
    for detail in list_analysis_records(conn, season_id, farm_id)? {
        let substances_coded = substance_labels(conn, catalogues, country_code, &detail)?;
        let record = detail.record;
        let (orders, names) =
            plot_cross_reference(detail.plots.iter().map(|p| p.plot_id.as_str()), plots);
        rows.push(AnalysisRow {
            plot_orders: orders,
            plot_names: names,
            date: record.sampled_on,
            material_kind: record.material_kind_code,
            analysis_types: detail
                .types
                .iter()
                .map(|t| t.analysis_type_code.clone())
                .collect(),
            bulletin: record.bulletin_number.unwrap_or_default(),
            lab_name: record.lab_name.unwrap_or_default(),
            lab_address: record.lab_address.unwrap_or_default(),
            lab_tax_id: record.lab_tax_id.unwrap_or_default(),
            substances_coded,
            substances: record.substances_detected.unwrap_or_default(),
            soil: record.soil,
            notes: record.notes.unwrap_or_default(),
        });
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Section 5 — registro de cosecha comercializada
//
// The one register core owns: what leaves the holding is whole-farm data, so
// the rows come from `terrazgo_core`, not from this module's repository.
// ---------------------------------------------------------------------------

struct HarvestRow {
    plot_orders: Vec<usize>,
    plot_names: String,
    /// ISO `YYYY-MM-DD`.
    date: String,
    product: String,
    /// `None` prints blank — an unstated quantity is unknown, never zero.
    quantity: Option<f64>,
    /// `kg` or `t`; a symbol, not prose, so it does not translate.
    quantity_unit: Option<String>,
    delivery_note: String,
    lot: String,
    buyer: String,
    buyer_tax_id: String,
    buyer_address: String,
    buyer_registry: String,
    notes: String,
}

fn harvest_rows(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
    plots: &PlotIndex,
) -> Result<Vec<HarvestRow>> {
    let mut rows = Vec::new();
    for detail in terrazgo_core::repository::list_harvest_records(conn, season_id, farm_id)? {
        let record = detail.record;
        let (orders, names) =
            plot_cross_reference(detail.plots.iter().map(|p| p.plot_id.as_str()), plots);
        rows.push(HarvestRow {
            plot_orders: orders,
            plot_names: names,
            date: record.harvested_on,
            product: record.product_name,
            quantity: record.quantity_value,
            quantity_unit: record.quantity_unit_code,
            delivery_note: record.delivery_note_ref.unwrap_or_default(),
            lot: record.lot_number.unwrap_or_default(),
            buyer: record.buyer_name,
            buyer_tax_id: record.buyer_tax_id.unwrap_or_default(),
            buyer_address: record.buyer_address.unwrap_or_default(),
            buyer_registry: record.buyer_registry_number.unwrap_or_default(),
            notes: record.notes.unwrap_or_default(),
        });
    }
    Ok(rows)
}

/// Model section 6 — one fertiliser application.
struct FertilisationRow {
    plot_orders: Vec<usize>,
    plot_names: String,
    /// Total surface fertilised across this record's plots; `None` when no plot
    /// stated one, which the model allows.
    area_ha: Option<f64>,
    /// The crops on the fertilised plots, as the model's "Cultivo" column.
    crops: String,
    date: String,
    end_date: Option<String>,
    /// The name frozen on the record, never the registry's current one.
    material_name: String,
    /// Anexo III C.d's coded kind, resolved against `MAT_FERTI`; the code
    /// itself when the vendored snapshot cannot resolve it (the `problem_code`
    /// rule).
    material_kind: String,
    /// C.i / art. 5.g. The model has no box for it, so it rides in the material
    /// cell and takes a column of its own in the sheet.
    sludge: bool,
    /// The twin's `GestionSostInsu`; no printed cell carries it.
    sustainable_inputs: bool,
    delivery_note: String,
    /// The three richness figures the model prints; each `None` stays blank,
    /// never zero.
    richness_n: Option<f64>,
    richness_p2o5: Option<f64>,
    richness_k2o: Option<f64>,
    dose: f64,
    /// A symbol, not prose, so it does not translate.
    dose_unit: String,
    /// Lookup codes, resolved by the labels at render time — two separate legal
    /// fields (C.c and C.f) that the model's single letter merges.
    type_code: String,
    method_code: String,
    /// Whether the method is one of the two fertigation entries, which is what
    /// the model's "(F)" actually asks.
    fertigation: bool,
    /// C.g and C.k: the holding's own machine, or the service company that
    /// spread it with its REGFER number.
    machinery: String,
    service_company: String,
    service_regfer: String,
    yield_estimated: Option<f64>,
    yield_final: Option<f64>,
    /// `BUENAS_PRACTICAS_AMBITOS` labels. Sheet only: the model has no column,
    /// and these are whole sentences that no register cell could carry.
    practices: String,
    notes: String,
}

/// One entry of the fertiliser material registry, with the full composition.
struct MaterialRow {
    name: String,
    kind: String,
    detail: String,
    supplier: String,
    /// Whichever one of C.e's three mutually exclusive registry numbers is set.
    supplier_registry: String,
    manure_treatment_code: String,
    density_kg_l: Option<f64>,
    /// One line per composition figure: the group, the resolved nutrient name
    /// and the percentage.
    nutrients: Vec<(String, String, f64)>,
}

fn fertilisation_rows(
    conn: &Connection,
    catalogues: &CatalogueCache,
    season_id: &str,
    farm_id: &str,
    country_code: &str,
    plots: &PlotIndex,
) -> Result<Vec<FertilisationRow>> {
    let crops: HashMap<String, String> =
        terrazgo_core::repository::list_crops(conn, season_id, farm_id)?
            .into_iter()
            .map(|crop| {
                let label = match crop.variety {
                    Some(variety) if !variety.is_empty() => {
                        format!("{} — {}", crop.species_name, variety)
                    }
                    _ => crop.species_name,
                };
                (crop.id, label)
            })
            .collect();
    let fertigation_methods = fertigation_method_codes(conn)?;
    // Every machine a row can name, resolved once rather than per row. No
    // `deleted_at` filter and no farm filter, for the same reason the crop areas
    // in `assemble` have none: a record naming a machine deleted since must go
    // on printing the name it printed before.
    let machinery_names: HashMap<String, String> = {
        let mut stmt = conn.prepare("SELECT id, name FROM machinery")?;
        let mut found = stmt.query([])?;
        let mut names = HashMap::new();
        while let Some(row) = found.next()? {
            names.insert(row.get(0)?, row.get(1)?);
        }
        names
    };

    let mut rows = Vec::new();
    for detail in
        module_fertilisation::repository::list_fertilisation_records(conn, season_id, farm_id)?
    {
        let record = detail.record;
        let (orders, names) =
            plot_cross_reference(detail.plots.iter().map(|p| p.plot_id.as_str()), plots);
        // Sum only what was stated: a record where no plot carries a surface
        // prints blank rather than 0, which would claim nothing was fertilised.
        let stated: Vec<f64> = detail
            .plots
            .iter()
            .filter_map(|p| p.fertilised_area_ha)
            .collect();
        let area_ha = (!stated.is_empty()).then(|| stated.iter().sum());

        let mut crop_labels: Vec<String> = Vec::new();
        for plot in &detail.plots {
            if let Some(label) = plot.crop_id.as_ref().and_then(|id| crops.get(id))
                && !crop_labels.contains(label)
            {
                crop_labels.push(label.clone());
            }
        }

        let machinery = record
            .machinery_id
            .as_ref()
            .and_then(|id| machinery_names.get(id).cloned())
            .unwrap_or_default();

        rows.push(FertilisationRow {
            plot_orders: orders,
            plot_names: names,
            area_ha,
            crops: crop_labels.join("; "),
            date: record.applied_on,
            end_date: record.application_end_date,
            material_name: record.material_name_snapshot,
            material_kind: catalogue_label(
                conn,
                catalogues,
                module_fertilisation::siex::fertiliser_material_catalogue(country_code),
                &record.material_code_snapshot,
                None,
            ),
            sludge: record.sludge_application,
            sustainable_inputs: record.sustainable_input_management,
            delivery_note: record.delivery_note_ref.unwrap_or_default(),
            richness_n: record.richness_n_snapshot,
            richness_p2o5: record.richness_p2o5_snapshot,
            richness_k2o: record.richness_k2o_snapshot,
            dose: record.dose_value,
            dose_unit: record.dose_unit_code,
            fertigation: fertigation_methods.contains(&record.application_method_code),
            type_code: record.fertilisation_type_code,
            method_code: record.application_method_code,
            machinery,
            service_company: record.service_company.unwrap_or_default(),
            service_regfer: record.service_regfer_number.unwrap_or_default(),
            yield_estimated: record.yield_estimated_kg_ha,
            yield_final: record.yield_final_kg_ha,
            practices: detail
                .practices
                .iter()
                .map(|code| {
                    catalogue_label(
                        conn,
                        catalogues,
                        module_fertilisation::siex::good_practice_catalogue(country_code),
                        code,
                        Some((
                            module_fertilisation::siex::GOOD_PRACTICE_SCOPE_KEY,
                            module_fertilisation::siex::FERTILISATION_SCOPE,
                        )),
                    )
                })
                .collect::<Vec<_>>()
                .join("; "),
            notes: record.notes.unwrap_or_default(),
        });
    }
    Ok(rows)
}

/// Which application methods count as fertigation. Read from the lookup rather
/// than matched on the code, because the column exists precisely so a reader
/// does not have to know which of the seven they are.
fn fertigation_method_codes(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt = conn.prepare("SELECT code FROM application_method WHERE is_fertigation = 1")?;
    let codes = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(codes)
}

fn material_rows(
    conn: &Connection,
    catalogues: &CatalogueCache,
    country_code: &str,
    language: ReportLanguage,
) -> Result<Vec<MaterialRow>> {
    let mut rows = Vec::new();
    for detail in module_fertilisation::repository::list_fertiliser_materials(conn)? {
        let material = detail.material;
        let supplier_registry = [
            &material.supplier_rega,
            &material.supplier_tax_id,
            &material.supplier_nima,
        ]
        .into_iter()
        .flatten()
        .next()
        .cloned()
        .unwrap_or_default();

        let nutrients = detail
            .nutrients
            .into_iter()
            .map(|n| {
                let name = catalogue_label(
                    conn,
                    catalogues,
                    module_fertilisation::siex::nutrient_catalogue(country_code, &n.kind_code),
                    &n.nutrient_code,
                    None,
                );
                (n.kind_code, name, n.percentage)
            })
            .collect();

        rows.push(MaterialRow {
            name: material.name,
            kind: catalogue_label(
                conn,
                catalogues,
                module_fertilisation::siex::fertiliser_material_catalogue(country_code),
                &material.material_code,
                None,
            ),
            detail: catalogue_label(
                conn,
                catalogues,
                module_fertilisation::siex::fertiliser_detail_catalogue(country_code),
                material.material_detail_code.as_deref().unwrap_or_default(),
                None,
            ),
            supplier: material.supplier_name.unwrap_or_default(),
            supplier_registry,
            manure_treatment_code: material.manure_treatment_code.unwrap_or_default(),
            density_kg_l: material.density_kg_l,
            nutrients,
        });
    }
    // Collated for the same reason as §2.1: the repository returns BINARY
    // order, and the registry screen sorts the same names with Intl.Collator.
    let collator = NameCollator::new(language);
    rows.sort_by(|a, b| collator.compare(&a.name, &b.name));

    Ok(rows)
}

/// A provider catalogue's label for one code, with the rule every coded field
/// in this book follows: an unresolvable code prints ITSELF rather than
/// vanishing, because the code is the regulatory payload and the label is
/// display sugar (the vendored snapshot rides app releases, a registry does
/// not wait for one).
///
/// `qualifier` picks a row in the catalogues that repeat a code per attribute —
/// `BUENAS_PRACTICAS_AMBITOS` holds three vocabularies keyed by ámbito, and the
/// same integer means a different practice in each.
/// The holding's province as a NAME, for model 1.1's "Provincia" cell.
///
/// `farm_es_extension.province_code` is entered by hand, so it arrives as the
/// farmer typed it — "47", "7", " 07 " or "Valladolid". Anything that reads as
/// an INE province number is resolved against the FEGA PROVINCIA catalogue
/// (whose codes are zero-padded), and everything else prints verbatim: the
/// catalogue-label rule applied to a cell where a bare number would be a poor
/// answer to a form asking for a province.
fn province_name(conn: &Connection, catalogues: &CatalogueCache, stored: Option<&str>) -> String {
    let Some(stored) = stored.map(str::trim).filter(|s| !s.is_empty()) else {
        return String::new();
    };
    let Ok(number) = stored.parse::<u8>() else {
        return stored.to_string();
    };
    if number == 0 {
        return stored.to_string();
    }
    let padded = format!("{number:02}");
    let label = catalogue_label(conn, catalogues, Some("PROVINCIA"), &padded, None);
    // catalogue_label falls back to the code it was given. Printing that would
    // silently rewrite "7" as "07", so an unresolved code prints what the
    // farmer actually typed.
    if label == padded {
        stored.to_string()
    } else {
        label
    }
}

/// The plot's término municipal as a NAME, for model 2.1's "Término municipal
/// (código y nombre)" column. Empty when it cannot be resolved, so the caller
/// prints the code alone rather than an invented name.
///
/// **The province is not optional context, it is part of the key.** Municipality
/// codes are unique only within a province — 001 is Alegría-Dulantzi in Álava
/// and Adalia in Valladolid — so a plot whose province is missing gets no name
/// at all, never the first of 52 candidates.
///
/// Both parts arrive as stored: the SIGPAC reference parses them as numbers
/// (`municipio: 10`) while a hand-typed one may carry padding or spaces, and
/// the catalogue zero-pads to two and three digits. Anything that does not read
/// as a number yields no name, which is the same conservatism `province_name`
/// applies one column to the left.
fn municipality_name(
    conn: &Connection,
    catalogues: &CatalogueCache,
    province: &str,
    municipality: &str,
) -> String {
    let (Ok(province), Ok(code)) = (
        province.trim().parse::<u8>(),
        municipality.trim().parse::<u32>(),
    ) else {
        return String::new();
    };
    if province == 0 || code == 0 {
        return String::new();
    }
    let padded = format!("{code:03}");
    let label = catalogue_label(
        conn,
        catalogues,
        Some("MUNICIPIO_SIGPAC"),
        &padded,
        Some(("Código de provincia", &format!("{province:02}"))),
    );
    // catalogue_label falls back to the code it was given; that is the caller's
    // job here, and printing it would duplicate the code beside itself.
    if label == padded {
        String::new()
    } else {
        label
    }
}

fn catalogue_label(
    conn: &Connection,
    catalogues: &CatalogueCache,
    catalogue: Option<&str>,
    code: &str,
    qualifier: Option<(&str, &str)>,
) -> String {
    if code.is_empty() {
        return String::new();
    }
    catalogue
        .and_then(|catalogue| {
            catalogues
                .find(conn, catalogue, code)
                .into_iter()
                .find(|row| match qualifier {
                    None => true,
                    Some((key, want)) => {
                        row.attrs
                            .as_ref()
                            .and_then(|attrs| attrs.get(key))
                            .and_then(Value::as_str)
                            == Some(want)
                    }
                })
                .map(|row| row.label)
        })
        .unwrap_or_else(|| code.to_string())
}

/// Catalogue rows resolved once per book instead of once per row.
///
/// Assembly asks the same few questions over and over — the measure of every
/// treatment row, the término municipal of every plot row, the growth stage of
/// every treated plot — and each ask used to be its own point query. The number
/// of queries a book issues is now bounded by the DISTINCT codes it prints
/// rather than by how many rows it has, which is the shape the scale rule asks
/// report assembly to keep (`docs/architecture.md` → "The report engine").
///
/// **Memoised per code, not preloaded per catalogue**, which is the one design
/// choice worth stating. Preloading is the obvious fix while the vocabularies
/// are small — `EST_FENOLOGICO` has ten rows and `TIPO_MEDIDA_FITOSANITARIA`
/// fourteen — but the book also resolves `MUNICIPIO_SIGPAC` (8 434 rows) and
/// `DETALLE_MATERIAL_FERT` (1 243) while naming a handful of towns and
/// materials, and reading a whole catalogue to resolve three of its rows costs
/// more than the queries it saves. Memoising is bounded either way, so one
/// mechanism serves every catalogue and no call site has to pick.
///
/// A code that resolves to NOTHING is remembered as such: a book written
/// against a catalogue this installation never imported must not re-ask for
/// every row.
#[derive(Default)]
struct CatalogueCache {
    memo: RefCell<Memo>,
}

/// What the cache remembers: the rows behind each (catalogue, code) it has been
/// asked for, and the two counts that say whether it is doing its job.
#[derive(Default)]
struct Memo {
    rows: HashMap<(String, String), Vec<CatalogueCode>>,
    /// Reads that reached the database.
    queries: usize,
    /// Asks served, hit or miss. The gap against `queries` is what this type is
    /// for, so the tests pin it.
    lookups: usize,
}

impl CatalogueCache {
    /// Every row carrying `code` in `catalogue`, from the memo or, the first
    /// time it is asked for, from the database.
    ///
    /// A failed read yields no rows, exactly as the point query it replaces
    /// did: the code is the record's legal payload and the label is display
    /// sugar, so a book must render even with no catalogues imported at all.
    fn find(&self, conn: &Connection, catalogue: &str, code: &str) -> Vec<CatalogueCode> {
        let key = (catalogue.to_string(), code.to_string());
        {
            let mut memo = self.memo.borrow_mut();
            memo.lookups += 1;
            if let Some(rows) = memo.rows.get(&key) {
                return rows.clone();
            }
        }
        // Borrow released: nothing is held across the read.
        let rows = terrazgo_core::catalogue::find_code(conn, catalogue, code).unwrap_or_default();
        let mut memo = self.memo.borrow_mut();
        memo.queries += 1;
        memo.rows.insert(key, rows.clone());
        rows
    }
}

/// One line of model table 7.1: an application of section 6, seen again with
/// the unidades fertilizantes it supplied, the running total for its production
/// unit, and what the plan recommends for that unit.
///
/// **Only the recommendation is stored.** Everything else is section 6's own
/// record: capturing the aportadas and acumuladas as well would let one book
/// state two different totals for one campaign.
struct PlanRow {
    plot_orders: Vec<usize>,
    plot_names: String,
    crops: String,
    date: String,
    area_ha: Option<f64>,
    material_name: String,
    richness_n: Option<f64>,
    richness_p2o5: Option<f64>,
    richness_k2o: Option<f64>,
    dose: f64,
    dose_unit: String,
    /// UF supplied by this application — `None` when the dose is a volume and
    /// the material states no density, or when the richness is unstated.
    supplied: Nutrients,
    /// The running total down this production unit, blank from the first
    /// unknown onwards (see `agronomy::Accumulator`).
    accumulated: Nutrients,
    /// What the plan recommends for this unit; blank when it has no plan yet.
    recommended: Nutrients,
}

/// The three unidades fertilizantes the plan de abonado speaks in, each of them
/// kg/ha (the model's footnote 2). `None` is "not known", never zero.
#[derive(Default, Clone, Copy)]
struct Nutrients {
    n: Option<f64>,
    p2o5: Option<f64>,
    k2o: Option<f64>,
}

fn plan_rows(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
    plots: &PlotIndex,
) -> Result<Vec<PlanRow>> {
    // What each crop's plan recommends, so a row can carry it without the
    // reader having to look the plan up in another table.
    let mut recommended: HashMap<String, Nutrients> = HashMap::new();
    for detail in
        module_fertilisation::repository::list_fertilisation_plans(conn, season_id, farm_id)?
    {
        let needs = Nutrients {
            n: Some(detail.plan.needs_n_kg_ha),
            p2o5: Some(detail.plan.needs_p2o5_kg_ha),
            k2o: Some(detail.plan.needs_k2o_kg_ha),
        };
        for crop_id in detail.crop_ids {
            recommended.insert(crop_id, needs);
        }
    }

    let crops: HashMap<String, String> =
        terrazgo_core::repository::list_crops(conn, season_id, farm_id)?
            .into_iter()
            .map(|crop| {
                let label = match crop.variety {
                    Some(variety) if !variety.is_empty() => {
                        format!("{} — {}", crop.species_name, variety)
                    }
                    _ => crop.species_name,
                };
                (crop.id, label)
            })
            .collect();

    // A material's density is what turns a volume dose into kilograms, and it
    // lives on the registry row rather than on the record.
    let densities: HashMap<String, Option<f64>> =
        module_fertilisation::repository::list_fertiliser_materials(conn)?
            .into_iter()
            .map(|d| (d.material.id, d.material.density_kg_l))
            .collect();

    // One running total per production unit. The unit a row belongs to is its
    // crop where it has one, and its plot otherwise — a book that summed every
    // application of a holding into one column would answer a question nobody
    // asked.
    let mut totals: HashMap<String, [Accumulator; 3]> = HashMap::new();
    let mut rows = Vec::new();
    for detail in
        module_fertilisation::repository::list_fertilisation_records(conn, season_id, farm_id)?
    {
        let record = &detail.record;
        let density = densities
            .get(&record.fertiliser_material_id)
            .copied()
            .flatten();
        let dose_kg_ha = dose_as_kg_per_ha(record.dose_value, &record.dose_unit_code, density);
        let supplied = Nutrients {
            n: nutrient_units(dose_kg_ha, record.richness_n_snapshot),
            p2o5: nutrient_units(dose_kg_ha, record.richness_p2o5_snapshot),
            k2o: nutrient_units(dose_kg_ha, record.richness_k2o_snapshot),
        };

        // The model prints one row per parcela, so an application over three
        // plots is three lines — each accumulating into its own unit.
        for plot in &detail.plots {
            let unit_key = plot.crop_id.clone().unwrap_or_else(|| plot.plot_id.clone());
            let accumulator = totals.entry(unit_key.clone()).or_default();
            let accumulated = Nutrients {
                n: accumulator[0].add(supplied.n),
                p2o5: accumulator[1].add(supplied.p2o5),
                k2o: accumulator[2].add(supplied.k2o),
            };
            let (orders, names) =
                plot_cross_reference(std::iter::once(plot.plot_id.as_str()), plots);
            rows.push(PlanRow {
                plot_orders: orders,
                plot_names: names,
                crops: plot
                    .crop_id
                    .as_ref()
                    .and_then(|id| crops.get(id))
                    .cloned()
                    .unwrap_or_default(),
                date: record.applied_on.clone(),
                area_ha: plot.fertilised_area_ha,
                material_name: record.material_name_snapshot.clone(),
                richness_n: record.richness_n_snapshot,
                richness_p2o5: record.richness_p2o5_snapshot,
                richness_k2o: record.richness_k2o_snapshot,
                dose: record.dose_value,
                dose_unit: record.dose_unit_code.clone(),
                supplied,
                accumulated,
                recommended: plot
                    .crop_id
                    .as_ref()
                    .and_then(|id| recommended.get(id))
                    .copied()
                    .unwrap_or_default(),
            });
        }
    }
    Ok(rows)
}

/// Model section 8 — one irrigation, with the running total the form asks for.
struct IrrigationRow {
    plot_orders: Vec<usize>,
    plot_names: String,
    /// Total surface watered across this record's plots; `None` when no plot
    /// stated one, which the model allows.
    area_ha: Option<f64>,
    /// Lookup code, resolved to prose by the labels at render time.
    method_code: String,
    /// ISO `YYYY-MM-DD`.
    date: String,
    /// Interval end, `None` for a single day.
    end_date: Option<String>,
    volume: f64,
    /// `m3_ha` or `m3`; a symbol, not prose, so it does not translate.
    volume_unit: String,
    /// The running sum of `volume` down this table, in m³/ha. `None` for a
    /// record measured in absolute cubic metres — see `irrigation_rows`.
    cumulative_m3_ha: Option<f64>,
    /// Anexo III C.l, conditional under art. 17.2; blank when not supplied.
    water_nitric_n: Option<f64>,
    water_soluble_p2o5: Option<f64>,
    /// `water_origin` codes, resolved by the labels at render time.
    origin_codes: Vec<String>,
    notes: String,
}

fn irrigation_rows(
    conn: &Connection,
    season_id: &str,
    farm_id: &str,
    plots: &PlotIndex,
) -> Result<Vec<IrrigationRow>> {
    let mut rows: Vec<IrrigationRow> = Vec::new();
    // The model's "Volumen acumulado" column. Derived here and never stored:
    // it is a sum over the rows above it, and a stored copy is a second number
    // that can disagree with the first.
    //
    // Only m³/ha records accumulate. A meter reading in absolute m³ measures a
    // different thing, and adding it to a per-hectare series would produce a
    // total that is true of no field — so those rows print a blank cumulative
    // cell and leave the running total untouched. The footnote says so.
    let mut running = 0.0_f64;
    for detail in
        module_fertilisation::repository::list_irrigation_records(conn, season_id, farm_id)?
    {
        let record = detail.record;
        let (orders, names) =
            plot_cross_reference(detail.plots.iter().map(|p| p.plot_id.as_str()), plots);
        // Sum only what was stated: a record where no plot carries a surface
        // prints blank rather than 0, which would claim nothing was watered.
        let stated: Vec<f64> = detail
            .plots
            .iter()
            .filter_map(|p| p.irrigated_area_ha)
            .collect();
        let area_ha = (!stated.is_empty()).then(|| stated.iter().sum());

        let cumulative_m3_ha = if record.volume_unit_code == "m3_ha" {
            running += record.volume_value;
            Some(running)
        } else {
            None
        };

        rows.push(IrrigationRow {
            plot_orders: orders,
            plot_names: names,
            area_ha,
            method_code: record.irrigation_method_code,
            date: record.irrigated_on,
            end_date: record.irrigation_end_date,
            volume: record.volume_value,
            volume_unit: record.volume_unit_code,
            cumulative_m3_ha,
            water_nitric_n: record.water_nitric_n_mg_l,
            water_soluble_p2o5: record.water_soluble_p2o5_mg_l,
            origin_codes: detail.water_origins,
            notes: record.notes.unwrap_or_default(),
        });
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Section 9.1 — pastoreo extensivo (RD 1048/2022 art. 30.2 ter)
// ---------------------------------------------------------------------------

/// Model 9.1 — one grazing, on one line per animal group.
///
/// The model's last three columns (especie, REGA, nº animales) describe ONE
/// group of animals, while the dates and the plots describe the grazing. A
/// grazing that moved sheep and goats onto the same pasture therefore prints
/// two lines that repeat the dates — the same shape section 2.2's water points
/// use, and the reason the row carries its own animal fields rather than a
/// list.
struct GrazingRow {
    /// Model 9.1 column 1, filled by the farmer when the plots lie more than
    /// 10 km from the main livestock installation.
    group_ref: String,
    /// Column 2 asks for the reference, not the cross-reference every other
    /// register uses — so this is the SIGPAC reference where the plot carries
    /// one, and the plot's name where it does not.
    plot_reference: String,
    /// The cross-reference to table 2.1, kept beside the reference because the
    /// spreadsheet filters on it and because a plot with no SIGPAC reference
    /// still has an order number.
    plot_orders: Vec<usize>,
    plot_names: String,
    /// ISO `YYYY-MM-DD`; both renderers format it their own way — the PDF as
    /// dd/mm/yyyy prose, the sheet as a real date cell that sorts.
    started_on: String,
    /// Empty while the animals are still grazing. The annotation deadline runs
    /// from this date, so a blank here is "not finished", never "not recorded".
    ended_on: String,
    /// The species NAME, resolved against `ESPECIE_ANIMAL` — a catalogue label,
    /// so it prints verbatim in every language.
    species: String,
    rega: String,
    animal_count: i64,
    notes: String,
}

/// Model 9.1's rows.
///
/// **A grazing that maintains a cover does not print here.** Art. 42.1.c counts
/// pastoreo as one of three ways a live cover is maintained, and model 9.4
/// prints it as a column of its own, so the two pages partition the register on
/// `soil_cover_id`: without one it is the P1 duty this page records, with one it
/// is the P6 maintenance 9.4 records. Printing it on both would show a cover
/// grazing as if it were extensive grazing, on a document an inspector reads.
fn grazing_rows(
    conn: &Connection,
    catalogues: &CatalogueCache,
    country_code: &str,
    plots: &PlotIndex,
    details: &[module_ecoscheme::models::GrazingRecordDetail],
) -> Result<Vec<GrazingRow>> {
    let species_catalogue = module_ecoscheme::siex::animal_species_catalogue(country_code);
    let mut rows = Vec::new();
    for detail in details
        .iter()
        .filter(|detail| detail.record.soil_cover_id.is_none())
    {
        let record = &detail.record;
        let (orders, names) =
            plot_cross_reference(detail.plots.iter().map(|p| p.plot_id.as_str()), plots);
        // Several plots grazed together are what the model's own column calls a
        // "grupo de parcelas", so their references join in one cell.
        let mut references: Vec<&str> = detail
            .plots
            .iter()
            .filter_map(|p| plots.references.get(&p.plot_id).map(String::as_str))
            .collect();
        references.sort_unstable();
        let plot_reference = if references.is_empty() {
            names.clone()
        } else {
            references.join(" · ")
        };

        for animal in &detail.animals {
            rows.push(GrazingRow {
                group_ref: record.plot_group_ref.clone().unwrap_or_default(),
                plot_reference: plot_reference.clone(),
                plot_orders: orders.clone(),
                plot_names: names.clone(),
                started_on: record.started_on.clone(),
                ended_on: record.ended_on.clone().unwrap_or_default(),
                species: catalogue_label(
                    conn,
                    catalogues,
                    species_catalogue,
                    &animal.species_code,
                    None,
                ),
                rega: animal.rega_code.clone(),
                animal_count: animal.animal_count,
                notes: record.notes.clone().unwrap_or_default(),
            });
        }
    }
    Ok(rows)
}

// ---------------------------------------------------------------------------
// Section 9.2 + the book's "9.6" — cultural operations (RD 1048/2022 arts. 31,
// 31.4.d and anexo IV)
// ---------------------------------------------------------------------------

/// One dated activity as a printed cell fragment.
///
/// Model 9.2's cells accumulate dates rather than holding one: footnote (1)
/// allows two cuts a year, and nothing stops a farmer tilling twice. So each
/// column is a list, and the label rides along only where the model's own
/// footnote (4) asks for the activity as well as the date.
struct DatedActivity {
    /// ISO `YYYY-MM-DD`; the renderers format their own way.
    performed_on: String,
    /// Empty for a single day's work — the register distinguishes the two, so
    /// the cell must not invent an interval.
    performed_end_date: String,
    /// Empty in the columns whose heading already names the activity.
    label: String,
}

/// Model 9.2 — one row per PLOT, which is what the model's own row is: it
/// carries the SIGPAC parts and the surface in columns of their own, the way
/// table 2.1 does, and then accumulates dates by activity.
///
/// So this page is a PIVOT of the register rather than a listing of it. The
/// spreadsheet unfolds it back to one row per operation, where filtering and
/// sorting are the point.
struct MowingRow {
    /// Table 2.1's order number — the model's "Id. de parcelas" column.
    order: usize,
    sigpac: PlotSigpac,
    mowing: Vec<DatedActivity>,
    tillage: Vec<DatedActivity>,
    /// Model 9.2's "Siembra" column. **Empty until seam 3**: `TIPO_LABOR`
    /// publishes no siembra code, so this module's owned vocabulary has none
    /// either, and a sowing is its own register — `sowing_record` in core,
    /// which seam 3 builds and wires in here.
    sowing: Vec<DatedActivity>,
    maintenance: Vec<DatedActivity>,
}

/// The book's "9.6" — the register anexo IV orders and the model has no page
/// for. One row per operation, the shape every other register in the book uses,
/// because there is no official form to follow here.
struct CommunalRow {
    plot_orders: Vec<usize>,
    plot_names: String,
    performed_on: String,
    performed_end_date: String,
    /// The kind as prose, plus the free description where one was given.
    activity: String,
}

/// One operation as the spreadsheet wants it: unpivoted, with the duty it
/// evidences and the residue destination — two facts no printed page carries,
/// the first because the PDF answers it by which page a row is on, the second
/// because it is the twin's field and not the model's.
struct OperationSheetRow {
    /// Owned rather than borrowed from `Labels`, because the accessors fall
    /// back to printing an unknown code ITSELF — so the string may come from
    /// the record rather than from the label table.
    practice: String,
    kind: String,
    performed_on: String,
    performed_end_date: String,
    plot_orders: Vec<usize>,
    plot_names: String,
    activity_description: String,
    residue_destination: String,
    notes: String,
}

/// What [`operation_rows`] returns: the same register read once and projected
/// four ways.
struct OperationRows {
    mowing: Vec<MowingRow>,
    communal: Vec<CommunalRow>,
    sheet: Vec<OperationSheetRow>,
    /// Art. 45.2's nivelación and caballones dates, per plot — model 9.3's two
    /// added columns. Collected in this pass rather than re-read, because the
    /// rows live in the same table as 9.2's.
    flooded: HashMap<String, FloodedOperations>,
    /// Art. 42.1.c's maintenance, keyed by the cover it maintained — model
    /// 9.4's Siega and Desbrozado columns.
    ///
    /// Collected here, in the ONE pass over the register, rather than queried
    /// per printed cover row: the book's reads must not grow with the rows it
    /// prints (the `crop_areas` rule), and its own test would catch a
    /// per-row query.
    cover_maintenance: HashMap<String, CoverMaintenance>,
}

/// The two of art. 45.2's five dates that are cultural operations.
#[derive(Default)]
struct FloodedOperations {
    levelling: Vec<DatedActivity>,
    ridging: Vec<DatedActivity>,
}

/// The two of model 9.4's three maintenance columns that are cultural
/// operations.
///
/// The third, Pastoreo, is a grazing record, so [`cover_rows`] keys it out of
/// the grazing slice instead — the same shape as `FloodedOperations`, which
/// carries the two of art. 45.2's five dates that are operations. Model 9.5 has
/// no maintenance columns at all: art. 43 asks for none.
#[derive(Default)]
struct CoverMaintenance {
    mowing: Vec<DatedActivity>,
    brush_cutting: Vec<DatedActivity>,
}

/// Reads the cultural-operation register ONCE and projects it onto the two
/// pages and the tab that show it.
///
/// The split is `practice_code`, which is the whole reason the module holds one
/// table where the model prints several pages: art. 31's mowing and anexo IV's
/// comunal maintenance are the same act recorded against different duties. The
/// practices seams 3 and 4 print (`flooded_biodiversity`, `plant_cover`,
/// `inert_cover`) are readable here already and are deliberately left off both
/// pages until those seams give them one — they still reach the spreadsheet, so
/// nothing captured is invisible in the meantime.
fn operation_rows(
    conn: &Connection,
    catalogues: &CatalogueCache,
    country_code: &str,
    plots: &PlotIndex,
    language: ReportLanguage,
    details: &[module_ecoscheme::models::CulturalOperationDetail],
    sowings: &[terrazgo_core::models::SowingRecordDetail],
) -> Result<OperationRows> {
    let labels = language.labels();
    let residue_catalogue = module_ecoscheme::siex::residue_destination_catalogue(country_code);

    // 9.2's pivot, keyed by plot. A BTreeMap on the table-2.1 order number so
    // the page reads down the parcel list the way section 2.1 does, without a
    // sort afterwards.
    let mut pivot: BTreeMap<usize, MowingRow> = BTreeMap::new();
    let mut communal = Vec::new();
    let mut sheet = Vec::new();
    let mut flooded: HashMap<String, FloodedOperations> = HashMap::new();
    let mut cover_maintenance: HashMap<String, CoverMaintenance> = HashMap::new();

    for detail in details {
        let record = &detail.record;
        let (orders, names) =
            plot_cross_reference(detail.plots.iter().map(|p| p.plot_id.as_str()), plots);
        let kind = labels.operation_kind(&record.operation_kind_code);
        let description = record.activity_description.clone().unwrap_or_default();
        let end = record.performed_end_date.clone().unwrap_or_default();

        sheet.push(OperationSheetRow {
            practice: labels.eco_practice(&record.practice_code).to_string(),
            kind: kind.to_string(),
            performed_on: record.performed_on.clone(),
            performed_end_date: end.clone(),
            plot_orders: orders.clone(),
            plot_names: names.clone(),
            activity_description: description.clone(),
            residue_destination: catalogue_label(
                conn,
                catalogues,
                residue_catalogue,
                record
                    .residue_destination_code
                    .as_deref()
                    .unwrap_or_default(),
                None,
            ),
            notes: record.notes.clone().unwrap_or_default(),
        });

        match record.practice_code.as_str() {
            "sustainable_mowing" => {
                for plot in &detail.plots {
                    let Some(&order) = plots.orders.get(&plot.plot_id) else {
                        continue;
                    };
                    let row = pivot.entry(order).or_insert_with(|| MowingRow {
                        order,
                        sigpac: plots.sigpac.get(&plot.plot_id).cloned().unwrap_or_default(),
                        mowing: Vec::new(),
                        tillage: Vec::new(),
                        sowing: Vec::new(),
                        maintenance: Vec::new(),
                    });
                    // Only the "otras actividades" column names what was done —
                    // the other two are headed by their activity, so repeating
                    // it in the cell would print "Siega" under "Siega".
                    let named =
                        !matches!(record.operation_kind_code.as_str(), "mowing" | "tillage");
                    let entry = DatedActivity {
                        performed_on: record.performed_on.clone(),
                        performed_end_date: end.clone(),
                        label: if named {
                            activity_text(kind, &description)
                        } else {
                            String::new()
                        },
                    };
                    // Which of the model's four activity columns takes the
                    // date. `no_tillage` deliberately does NOT go under
                    // "Laboreo": a date there states that the ground was
                    // worked, which is the opposite of what the record says —
                    // so it joins "otras actividades", where its name prints
                    // beside the date and the cell reads true.
                    match record.operation_kind_code.as_str() {
                        "mowing" => row.mowing.push(entry),
                        "tillage" => row.tillage.push(entry),
                        _ => row.maintenance.push(entry),
                    }
                }
            }
            "communal_pasture" => communal.push(CommunalRow {
                plot_orders: orders,
                plot_names: names,
                performed_on: record.performed_on.clone(),
                performed_end_date: end,
                activity: activity_text(kind, &description),
            }),
            // Art. 45.2's nivelación and construcción de caballones — two of
            // the five dates it names, and two the printed model has no column
            // for. They are gathered per plot here and printed by 9.3, which
            // adds the columns.
            "flooded_biodiversity" => {
                for plot in &detail.plots {
                    let entry = flooded.entry(plot.plot_id.clone()).or_default();
                    let dated = DatedActivity {
                        performed_on: record.performed_on.clone(),
                        performed_end_date: end.clone(),
                        label: String::new(),
                    };
                    match record.operation_kind_code.as_str() {
                        "levelling" => entry.levelling.push(dated),
                        "ridging" => entry.ridging.push(dated),
                        // Any other work on a flooded plot is still recorded
                        // and still reaches the operations tab; art. 45.2 names
                        // five dates and this page prints exactly those.
                        _ => {}
                    }
                }
            }
            // Art. 42.1.c — the maintenance performed ON a cover, which model
            // 9.4 prints as its Siega and Desbrozado columns. Keyed by the
            // cover rather than by the plot: the model's row here is the
            // cover, not the parcel, because one cover carries one
            // establishment date and one pair of widths however many plots it
            // was established over.
            //
            // An operation filed against a cover practice with NO cover named
            // is still a real record and still reaches the operations tab — it
            // is the poda whose residue became a P7 cover, most often — but it
            // has no cell on 9.4, which prints the maintenance OF a cover.
            "plant_cover" | "inert_cover" => {
                let Some(cover_id) = record.soil_cover_id.as_deref() else {
                    continue;
                };
                let entry = cover_maintenance.entry(cover_id.to_string()).or_default();
                let dated = DatedActivity {
                    performed_on: record.performed_on.clone(),
                    performed_end_date: end.clone(),
                    label: String::new(),
                };
                match record.operation_kind_code.as_str() {
                    "mowing" => entry.mowing.push(dated),
                    "brush_cutting" => entry.brush_cutting.push(dated),
                    // The register accepts only these two plus a grazing as
                    // maintenance, so nothing else can arrive here — and if it
                    // ever did, the model has no column for it.
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // Model 9.2's "Siembra" column. It is fed from the SOWING register rather
    // than from a cultural-operation kind, because `TIPO_LABOR` publishes no
    // siembra code and this module's owned vocabulary therefore has none: a
    // sowing is its own register, in core. Only the plots already on this page
    // gain a date — a sowing on a plot that recorded no P2 activity is not
    // evidence of sustainable mowing.
    for detail in sowings {
        for plot in &detail.plots {
            let Some(&order) = plots.orders.get(&plot.plot_id) else {
                continue;
            };
            if let Some(row) = pivot.get_mut(&order) {
                row.sowing.push(DatedActivity {
                    performed_on: detail.record.sown_on.clone(),
                    performed_end_date: detail.record.sowing_end_date.clone().unwrap_or_default(),
                    label: String::new(),
                });
            }
        }
    }

    Ok(OperationRows {
        mowing: pivot.into_values().collect(),
        communal,
        sheet,
        flooded,
        cover_maintenance,
    })
}

// ---------------------------------------------------------------------------
// Sections 9.4 and 9.5 — cubiertas (RD 1048/2022 arts. 42 and 43)
// ---------------------------------------------------------------------------

/// One row of model 9.4 or 9.5 — and here the model's row is the COVER, not the
/// plot.
///
/// That is the difference from 9.2 and 9.3, which pivot onto the parcel: a
/// cover has one establishment date and one pair of widths however many plots
/// it was established over, so there is nothing to accumulate per plot and the
/// register's own row is already the printed one. The plots ride in the "Id.
/// Parcelas" cell as cross-references, the way the book's "9.6" does it.
///
/// The maintenance columns belong to 9.4 alone; 9.5 leaves them unread, because
/// art. 43 asks for no maintenance of an inert cover.
struct CoverRow {
    /// Table 2.1's order numbers — the model's "Id. Parcelas" column.
    plot_orders: Vec<usize>,
    plot_names: String,
    established_on: String,
    /// Blank until art. 42.1.e's separate annotation is made. A blank cell is
    /// the honest statement that the widths are not stated yet, and the
    /// advisory is what says so out loud.
    width_m: String,
    free_canopy_width_m: String,
    mowing: Vec<DatedActivity>,
    brush_cutting: Vec<DatedActivity>,
    grazing: Vec<DatedActivity>,
}

/// The covers tab of the workbook: one row per cover, carrying what neither
/// printed page has a column for — the practice, the kind of cover and the date
/// the widths were stated.
struct CoverSheetRow {
    practice: String,
    cover_type: String,
    established_on: String,
    width_m: Option<f64>,
    free_canopy_width_m: Option<f64>,
    widths_stated_on: String,
    plot_orders: Vec<usize>,
    plot_names: String,
    maintenance: String,
    notes: String,
}

struct CoverRows {
    /// Model 9.4 — the live covers of art. 42.
    plant: Vec<CoverRow>,
    /// Model 9.5 — the inert covers of art. 43.
    inert: Vec<CoverRow>,
    sheet: Vec<CoverSheetRow>,
}

/// The three registers these two pages are assembled from, already read.
///
/// Grouped because art. 42 splits ONE duty across three tables — the cover
/// carries the establishment date and the widths, the operations carry the
/// siegas and desbroces, the grazings carry the pastoreos — and passing them
/// separately said less about them than naming what they are together.
struct CoverSources<'a> {
    covers: &'a [module_ecoscheme::models::SoilCoverDetail],
    grazings: &'a [module_ecoscheme::models::GrazingRecordDetail],
    /// Keyed by `soil_cover_id`, gathered by [`operation_rows`] in its single
    /// pass so nothing here queries per printed row.
    operations: &'a HashMap<String, CoverMaintenance>,
}

/// Projects the cover register onto its two pages and its tab.
///
/// The maintenance arrives already gathered: `operation_rows` collected the
/// siegas and desbroces in its single pass over the operation register, and the
/// grazings are picked out of the grazing slice `assemble` read once. Nothing
/// here queries per printed row.
fn cover_rows(
    conn: &Connection,
    catalogues: &CatalogueCache,
    country_code: &str,
    plots: &PlotIndex,
    language: ReportLanguage,
    sources: CoverSources<'_>,
) -> Result<CoverRows> {
    let labels = language.labels();
    let cover_catalogue = module_ecoscheme::siex::cover_type_catalogue(country_code);

    // Model 9.4's third maintenance column. A grazing is a register of its own,
    // so it is not in `sources.operations` — it is keyed out of the grazing
    // slice here.
    let mut grazed: HashMap<&str, Vec<DatedActivity>> = HashMap::new();
    for detail in sources.grazings {
        let Some(cover_id) = detail.record.soil_cover_id.as_deref() else {
            continue;
        };
        grazed.entry(cover_id).or_default().push(DatedActivity {
            performed_on: detail.record.started_on.clone(),
            performed_end_date: detail.record.ended_on.clone().unwrap_or_default(),
            label: String::new(),
        });
    }

    let mut plant = Vec::new();
    let mut inert = Vec::new();
    let mut sheet = Vec::new();

    for detail in sources.covers {
        let record = &detail.record;
        let (orders, names) =
            plot_cross_reference(detail.plots.iter().map(|p| p.plot_id.as_str()), plots);

        let maintenance = sources.operations.get(&record.id);
        let mut mowing = maintenance
            .map(|m| clone_dates(&m.mowing))
            .unwrap_or_default();
        let mut brush_cutting = maintenance
            .map(|m| clone_dates(&m.brush_cutting))
            .unwrap_or_default();
        let mut grazing = grazed
            .get(record.id.as_str())
            .map(|dates| clone_dates(dates))
            .unwrap_or_default();
        for column in [&mut mowing, &mut brush_cutting, &mut grazing] {
            column.sort_by(|a, b| a.performed_on.cmp(&b.performed_on));
        }

        sheet.push(CoverSheetRow {
            practice: labels.eco_practice(&record.practice_code).to_string(),
            cover_type: catalogue_label(
                conn,
                catalogues,
                cover_catalogue,
                &record.cover_type_code,
                None,
            ),
            established_on: record.established_on.clone(),
            width_m: record.width_m,
            free_canopy_width_m: record.free_canopy_width_m,
            widths_stated_on: record.widths_stated_on.clone().unwrap_or_default(),
            plot_orders: orders.clone(),
            plot_names: names.clone(),
            maintenance: format_cover_maintenance(labels, &mowing, &brush_cutting, &grazing),
            notes: record.notes.clone().unwrap_or_default(),
        });

        let row = CoverRow {
            plot_orders: orders,
            plot_names: names,
            established_on: record.established_on.clone(),
            // Blank rather than a zero: art. 42.1.e's annotation has a deadline
            // of its own and simply has not been made yet.
            width_m: record.width_m.map(format_number).unwrap_or_default(),
            free_canopy_width_m: record
                .free_canopy_width_m
                .map(format_number)
                .unwrap_or_default(),
            mowing,
            brush_cutting,
            grazing,
        };
        if record.practice_code == "inert_cover" {
            inert.push(row);
        } else {
            plant.push(row);
        }
    }

    Ok(CoverRows {
        plant,
        inert,
        sheet,
    })
}

/// The workbook's single maintenance cell, where the printed page has three
/// columns: each activity named, so one column can be filtered and read.
fn format_cover_maintenance(
    labels: &Labels,
    mowing: &[DatedActivity],
    brush_cutting: &[DatedActivity],
    grazing: &[DatedActivity],
) -> String {
    [
        (labels.operation_kind("mowing"), mowing),
        (labels.operation_kind("brush_cutting"), brush_cutting),
        (labels.s9.s94.grazing, grazing),
    ]
    .iter()
    .filter(|(_, entries)| !entries.is_empty())
    .map(|(name, entries)| format!("{name}: {}", format_activities(entries)))
    .collect::<Vec<_>>()
    .join(" · ")
}

// ---------------------------------------------------------------------------
// Section 9.3 — espacios de biodiversidad en cultivos bajo agua
// (RD 1048/2022 art. 45.2)
// ---------------------------------------------------------------------------

/// One row per plot, carrying the FIVE dates art. 45.2 names.
///
/// The model prints three of them. A book following the form would not satisfy
/// the article, so nivelación and construcción de caballones get columns of
/// their own — placed where the article names them, which happens to leave the
/// model's own three in their original relative order.
///
/// The five come from three tables in three crates: the sowing and flooding
/// dates from core's `sowing_record`, the drying date from module-cue's
/// `treatment_record`, and the levelling and ridging dates from
/// module-ecoscheme's `cultural_operation`. Only this crate can read all three
/// — it is the consumer above the modules, and modules may not read each other.
struct FloodedRow {
    /// Table 2.1's order number — the model's "Id. Parcelas" column.
    order: usize,
    levelling: Vec<DatedActivity>,
    sowing: Vec<DatedActivity>,
    flooding: Vec<DatedActivity>,
    drying: Vec<DatedActivity>,
    ridging: Vec<DatedActivity>,
}

/// Gathers art. 45.2's five dates per plot.
///
/// **Which plots appear is the one judgement here.** A plot enters the page
/// when it carries evidence of being a *cultivo bajo agua*: a sowing that was
/// flooded, a cultural operation recorded against `flooded_biodiversity`, or a
/// treatment that dried the field. Once a plot is in, EVERY sowing on it prints
/// its date — which is what keeps a dry sowing visible in the month before the
/// flooding is annotated, since `flooded_on` is filled by a later correction.
///
/// A sowing with no flooding date is not, on its own, evidence of a flooded
/// crop: every wheat sowing on the holding would otherwise land on this page.
fn flooded_rows(
    plots: &PlotIndex,
    sowings: &[terrazgo_core::models::SowingRecordDetail],
    treatments: &[module_cue::models::TreatmentRecordWithPlots],
    operations: &HashMap<String, FloodedOperations>,
) -> Vec<FloodedRow> {
    // The plots that are known to grow a crop under water.
    let mut flooded_plots: HashSet<&str> = operations.keys().map(String::as_str).collect();
    for detail in sowings {
        if detail.record.flooded_on.is_some() {
            flooded_plots.extend(detail.plots.iter().map(|p| p.plot_id.as_str()));
        }
    }
    for detail in treatments {
        if detail.record.drying_date.is_some() {
            flooded_plots.extend(detail.plots.iter().map(|p| p.plot_id.as_str()));
        }
    }

    // Keyed on the table-2.1 order number, so the page reads down the parcel
    // list the way section 2.1 does.
    let mut rows: BTreeMap<usize, FloodedRow> = BTreeMap::new();
    let row_for = |rows: &mut BTreeMap<usize, FloodedRow>, plot_id: &str| -> Option<usize> {
        if !flooded_plots.contains(plot_id) {
            return None;
        }
        let order = *plots.orders.get(plot_id)?;
        rows.entry(order).or_insert_with(|| FloodedRow {
            order,
            levelling: Vec::new(),
            sowing: Vec::new(),
            flooding: Vec::new(),
            drying: Vec::new(),
            ridging: Vec::new(),
        });
        Some(order)
    };

    for detail in sowings {
        for plot in &detail.plots {
            let Some(order) = row_for(&mut rows, &plot.plot_id) else {
                continue;
            };
            let Some(row) = rows.get_mut(&order) else {
                continue;
            };
            row.sowing.push(DatedActivity {
                performed_on: detail.record.sown_on.clone(),
                performed_end_date: detail.record.sowing_end_date.clone().unwrap_or_default(),
                label: String::new(),
            });
            if let Some(flooded_on) = &detail.record.flooded_on {
                row.flooding.push(DatedActivity {
                    performed_on: flooded_on.clone(),
                    performed_end_date: String::new(),
                    label: String::new(),
                });
            }
        }
    }

    for detail in treatments {
        let Some(drying) = &detail.record.drying_date else {
            continue;
        };
        for plot in &detail.plots {
            let Some(order) = row_for(&mut rows, &plot.plot_id) else {
                continue;
            };
            let Some(row) = rows.get_mut(&order) else {
                continue;
            };
            row.drying.push(DatedActivity {
                performed_on: drying.clone(),
                performed_end_date: String::new(),
                label: String::new(),
            });
        }
    }

    for (plot_id, dates) in operations {
        let Some(order) = row_for(&mut rows, plot_id) else {
            continue;
        };
        let Some(row) = rows.get_mut(&order) else {
            continue;
        };
        row.levelling.extend(clone_dates(&dates.levelling));
        row.ridging.extend(clone_dates(&dates.ridging));
    }

    // Each column reads chronologically, whatever order the tables came back
    // in — several sowings or two secas on one plot are a list, like 9.2's cuts.
    for row in rows.values_mut() {
        for column in [
            &mut row.levelling,
            &mut row.sowing,
            &mut row.flooding,
            &mut row.drying,
            &mut row.ridging,
        ] {
            column.sort_by(|a, b| a.performed_on.cmp(&b.performed_on));
        }
    }
    rows.into_values().collect()
}

fn clone_dates(entries: &[DatedActivity]) -> Vec<DatedActivity> {
    entries
        .iter()
        .map(|e| DatedActivity {
            performed_on: e.performed_on.clone(),
            performed_end_date: e.performed_end_date.clone(),
            label: e.label.clone(),
        })
        .collect()
}

/// The sowing register as the spreadsheet carries it — one row per sowing.
///
/// No page of the printed model shows this register: its dates appear in 9.2's
/// "Siembra" column and in 9.3's, and `seed_quantity_kg` appears nowhere at
/// all. So this tab is where it can be read whole, the `tab_materials`
/// precedent.
struct SowingSheetRow {
    /// Sown or planted, rendered in the book's language. Owned because the
    /// accessor falls back to the code itself for a value added upstream, the
    /// `analysis_type` rule.
    kind: String,
    sown_on: String,
    sowing_end_date: String,
    flooded_on: String,
    seed_quantity_kg: Option<f64>,
    plot_orders: Vec<usize>,
    plot_names: String,
    crops: String,
    notes: String,
}

fn sowing_sheet_rows(
    sowings: &[terrazgo_core::models::SowingRecordDetail],
    plots: &PlotIndex,
    language: ReportLanguage,
) -> Vec<SowingSheetRow> {
    let labels = language.labels();
    sowings
        .iter()
        .map(|detail| {
            let (orders, names) =
                plot_cross_reference(detail.plots.iter().map(|p| p.plot_id.as_str()), plots);
            // The frozen crop, not today's: a rename must not rewrite what the
            // book said was sown.
            let mut crops: Vec<&str> = detail
                .plots
                .iter()
                .filter_map(|p| p.crop_name_snapshot.as_deref())
                .collect();
            crops.sort_unstable();
            crops.dedup();
            SowingSheetRow {
                kind: labels.sowing_kind(&detail.record.kind_code).to_string(),
                sown_on: detail.record.sown_on.clone(),
                sowing_end_date: detail.record.sowing_end_date.clone().unwrap_or_default(),
                flooded_on: detail.record.flooded_on.clone().unwrap_or_default(),
                seed_quantity_kg: detail.record.seed_quantity_kg,
                plot_orders: orders,
                plot_names: names,
                crops: crops.join("; "),
                notes: detail.record.notes.clone().unwrap_or_default(),
            }
        })
        .collect()
}

/// Model 9.2 footnote (4) and the "9.6" activity column: the date carries the
/// activity too. The coded kind answers most of it; the free description is
/// what anexo III.B's open-ended list needs, and it appends rather than
/// replacing so a reader always sees which kind was recorded.
fn activity_text(kind: &str, description: &str) -> String {
    if description.is_empty() {
        kind.to_string()
    } else {
        format!("{kind} — {description}")
    }
}

/// Anexo III A.3's figures as one printed cell — the model predates A.3 and has
/// no soil page, so they ride beside the findings. Only what the bulletin
/// reported appears, each with the unit its field is named for, so a reader can
/// see which figure is missing rather than reading a zero that was never
/// measured.
fn soil_cell(soil: &module_cue::models::SoilParameters) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut push = |label: &str, value: Option<f64>, unit: &str| {
        if let Some(value) = value {
            parts.push(format!("{label} {}{unit}", format_number(value)));
        }
    };
    push("pH", soil.ph, "");
    push("M.O.", soil.organic_matter_pct, " %");
    push("P", soil.available_p_mg_kg, " mg/kg");
    push("K", soil.available_k_mg_kg, " mg/kg");
    push("N", soil.total_n_pct, " %");
    push("CE", soil.conductivity_ds_m, " dS/m");
    // Texture reads as one figure of three, the way a bulletin states it.
    if let (Some(sand), Some(silt), Some(clay)) = (soil.sand_pct, soil.silt_pct, soil.clay_pct) {
        parts.push(format!(
            "{} {} / {} / {} %",
            "Text.",
            format_number(sand),
            format_number(silt),
            format_number(clay)
        ));
    }
    parts.join(" · ")
}

/// The model prints each unidades-fertilizantes block as one cell of three
/// figures. An unknown one is a dash rather than a blank, so a reader can see
/// WHICH of the three is missing — and never a zero, which would be a claim.
fn nutrient_cell(values: Nutrients) -> String {
    [values.n, values.p2o5, values.k2o]
        .into_iter()
        .map(|value| value.map(format_number).unwrap_or_else(|| "—".to_string()))
        .collect::<Vec<_>>()
        .join(" / ")
}

/// The material columns repeated on every composition line of the 6-Materiales
/// tab, so each row stands on its own when the sheet is filtered.
fn material_prefix(m: &MaterialRow, labels: &Labels) -> Vec<Cell> {
    vec![
        Cell::text(m.name.as_str()),
        Cell::text(m.kind.as_str()),
        Cell::text(m.detail.as_str()),
        Cell::text(m.supplier.as_str()),
        Cell::text(m.supplier_registry.as_str()),
        Cell::text(labels.manure_treatment(&m.manure_treatment_code)),
        Cell::number(m.density_kg_l),
    ]
}

/// The model's "Riqueza N/P/K" cell. Each figure is stated with its own symbol
/// so a reader can tell which is missing, and an unstated one contributes
/// nothing at all — a printed "0" would claim the material contains none.
fn richness_cell(r: &FertilisationRow) -> String {
    [
        ("N", r.richness_n),
        ("P₂O₅", r.richness_p2o5),
        ("K₂O", r.richness_k2o),
    ]
    .into_iter()
    .filter_map(|(symbol, value)| value.map(|v| format!("{symbol} {}", format_number(v))))
    .collect::<Vec<_>>()
    .join(" / ")
}

/// Fold a value the model has no column for into the neighbouring cell that
/// does. Either side may be blank — a book must never print a stray separator
/// where nothing was recorded.
fn join_detail(main: &str, detail: &str) -> String {
    match (main.is_empty(), detail.is_empty()) {
        (_, true) => main.to_string(),
        (true, false) => detail.to_string(),
        (false, false) => format!("{main} · {detail}"),
    }
}

/// The coded substances an analysis reported, resolved for display exactly as
/// section 3.1 resolves problems: the stored code is the regulatory payload, the
/// catalogue label is display sugar, and a code the vendored snapshot cannot
/// resolve prints itself rather than vanishing.
fn substance_labels(
    conn: &Connection,
    catalogues: &CatalogueCache,
    country_code: &str,
    detail: &module_cue::models::AnalysisRecordDetail,
) -> Result<String> {
    let catalogue = siex::substance_catalogue(country_code);
    let mut labels: Vec<String> = Vec::new();
    for substance in &detail.substances {
        let label = catalogue_label(conn, catalogues, catalogue, &substance.substance_code, None);
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    Ok(labels.join("; "))
}

/// Section 4's "Laboratorio (nombre y dirección)" — one printed cell for what
/// the schema keeps as three fields, skipping whatever the farmer left blank.
fn lab_line(row: &AnalysisRow) -> String {
    let parts = [
        row.lab_name.as_str(),
        row.lab_address.as_str(),
        row.lab_tax_id.as_str(),
    ];
    parts
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" — ")
}

/// Resolve a register's plots into what the two documents each need: the PDF
/// cross-references table 2.1 by order number, the sheet spells the names out
/// so it can be filtered on its own. Both sorted, so the cell reads the same
/// however the junction rows happen to come back.
/// The seven parts of a SIGPAC reference as the one string the visor and every
/// official form print, `provincia:municipio:agregado:zona:polígono:parcela:recinto`.
///
/// A reference is only meaningful whole, so a plot missing any part gets an
/// empty string rather than a partial one with gaps — the caller then prints
/// the plot's name, which is a true statement, instead of "47::0:0:12::" which
/// looks like a reference and is not one.
fn sigpac_reference(parts: &[&str; 7]) -> String {
    if parts.iter().any(|part| part.trim().is_empty()) {
        return String::new();
    }
    parts.join(":")
}

fn plot_cross_reference<'a>(
    plot_ids: impl Iterator<Item = &'a str>,
    plots: &PlotIndex,
) -> (Vec<usize>, String) {
    let ids: Vec<&str> = plot_ids.collect();
    let mut orders: Vec<usize> = ids
        .iter()
        .filter_map(|id| plots.orders.get(*id).copied())
        .collect();
    orders.sort_unstable();
    let mut names: Vec<&str> = ids
        .iter()
        .filter_map(|id| plots.names.get(*id).map(String::as_str))
        .collect();
    plots.collator.sort(&mut names);
    (orders, names.join(", "))
}

/// How a conditional register's "APLICA TRATAMIENTO" boxes print.
///
/// SÍ is derived — rows exist. NO is a stored declaration, never inferred from
/// emptiness: an unfilled register looks exactly like one with nothing to
/// declare, and only the second is evidence the farmer checked. Neither leaves
/// both boxes empty, which is the honest reading of a register nobody has
/// touched yet.
fn register_answer(rows: usize, declared_empty: bool) -> Option<bool> {
    if rows > 0 {
        Some(true)
    } else if declared_empty {
        Some(false)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Renderer 1 — the Typst template's `sys.inputs`
//
// Everything here is a pre-formatted string, labels included: the template does
// layout and holds no prose, so one template serves every language.
// ---------------------------------------------------------------------------

impl Cuaderno {
    /// Takes the LANGUAGE rather than its labels, so the labels and the
    /// collator that orders them are derived from one source and cannot
    /// disagree about which language the book is being printed in.
    fn to_typst(&self, language: ReportLanguage) -> Value {
        let labels = language.labels();
        let collator = NameCollator::new(language);
        // Sections 3.3/3.4/3.5 come out as one entry per register. Assembled
        // before the json! block because a bare array literal would be read as
        // a JSON array rather than a Rust expression.
        let non_field: Vec<Value> = NON_FIELD_KINDS
            .iter()
            .map(|kind| {
                let rows: Vec<&NonFieldRow> = self
                    .non_field
                    .iter()
                    .filter(|r| r.subject_kind == *kind)
                    .collect();
                let answer =
                    register_answer(rows.len(), self.declared_empty.iter().any(|c| c == kind));
                json!({
                    "kind": kind,
                    // Ticked boxes, not words: the model prints two of them.
                    "applies_yes": if answer == Some(true) { labels.value.cross } else { "" },
                    "applies_no": if answer == Some(false) { labels.value.cross } else { "" },
                    "rows": rows.iter().map(|r| json!({
                        "date": format_date(&r.date),
                        "subject": r.subject,
                        "quantity": amount(r.quantity_value, r.quantity_unit_code.as_deref()),
                        "problems": r.problems,
                        // B.d asks for the applicator "y, en su caso, del
                        // asesor", so an advised actuation prints both in the
                        // one cell the model gives it — with the ROPO number,
                        // which is what identifies an advisor.
                        "operator": join_detail(
                            &r.operator_name,
                            &match (r.advisor_name.is_empty(), r.advisor_registration.is_empty()) {
                                (true, _) => String::new(),
                                (false, true) => r.advisor_name.clone(),
                                (false, false) => {
                                    format!("{} ({})", r.advisor_name, r.advisor_registration)
                                }
                            },
                        ),
                        "product": r.product,
                        "reg_no": r.reg_no,
                        "product_quantity": amount(
                            r.product_quantity_value,
                            r.product_quantity_unit_code.as_deref(),
                        ),
                        "efficacy": labels.efficacy(r.efficacy_code.as_deref()),
                        "notes": r.notes,
                    })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let yes_no = |flag: Option<bool>| match flag {
            Some(flag) => labels.yes_no(flag),
            None => "",
        };
        json!({
            "labels": labels,
            "campaign": self.campaign,
            "generated_on": format_date(&self.generated_on),
            "farm": {
                "name": self.farm.name,
                "owner": self.farm.owner,
                "nif": self.farm.nif,
                "siex": self.farm.siex,
                "rea": self.farm.rea,
                "location": self.farm.location,
                "province": self.farm.province,
                "address": self.farm.address,
                "postal_code": self.farm.postal_code,
                "phone_fixed": self.farm.phone_fixed,
                "phone_mobile": self.farm.phone_mobile,
                "email": self.farm.email,
                "opened_on": self.farm.opened_on,
            },
            // Absent representative prints an empty block — the model keeps
            // the rows so the form stays hand-fillable.
            "representative": self.farm.representative.as_ref().map_or_else(
                || json!({"name": "", "nif": "", "kind": "", "address": "",
                          "locality": "", "province": "", "postal_code": "",
                          "phone": "", "email": ""}),
                |r| json!({
                    "name": r.name, "nif": r.nif, "kind": r.kind, "address": r.address,
                    "locality": r.locality, "province": r.province,
                    "postal_code": r.postal_code, "phone": r.phone, "email": r.email,
                }),
            ),
            "operators": self.operators.iter().map(|o| json!({
                "order": o.order.to_string(),
                "name": o.name,
                "nif": o.tax_id,
                "licence": o.licence,
                "level": labels.licence_level(o.level.as_deref()),
                // The model prints a cross, not a word.
                "advisor": if o.is_advisor { labels.value.cross } else { "" },
            })).collect::<Vec<_>>(),
            "advisors": self.advisors.iter().map(|a| json!({
                "name": a.name,
                "nif": a.tax_id,
                "registration_number": a.registration_number,
                "gip": a.gip,
            })).collect::<Vec<_>>(),
            "machinery": self.machinery.iter().map(|m| json!({
                "order": m.order.to_string(),
                "description": m.description,
                "roma": m.roma,
                "reganip": m.reganip,
                "acquired_on": m.acquired_on.as_deref().map(format_date).unwrap_or_default(),
                "last_inspection": m.last_inspection.as_deref().map(format_date).unwrap_or_default(),
            })).collect::<Vec<_>>(),
            "plot_rows": self.plots.iter().map(|p| json!({
                "order": p.order.to_string(),
                "name": p.name,
                "province": p.province,
                // "Término municipal (código y nombre)" is one column in the
                // model, so the two ride together here; the workbook keeps them
                // apart, where a name can be filtered on (the 2.2 precedent).
                "municipality": join_detail(&p.municipality, &p.municipality_name),
                "aggregate": p.aggregate,
                "zone": p.zone,
                "polygon": p.polygon,
                "parcel": p.parcel,
                "enclosure": p.enclosure,
                "land_use": p.land_use,
                "sigpac_area": p.sigpac_area_ha.map(format_number).unwrap_or_default(),
                "area": p.cultivated_area_ha.map(format_number).unwrap_or_default(),
                "species": p.species,
                "variety": p.variety,
                "irrigation": p.irrigation,
                "environment": p.environment,
                "gip": p.gip,
            })).collect::<Vec<_>>(),
            "zone_rows": self.zones.iter().map(|z| {
                let [water_point, distance, coordinates, denomination] = z.water_cells(labels);
                json!({
                    "order": z.order.to_string(),
                    "species": z.species,
                    "variety": z.variety,
                    "water_point": water_point,
                    "distance": distance,
                    "coordinates": coordinates,
                    "denomination": denomination,
                    "fully": yes_no(z.fully),
                    "partly": yes_no(z.partly),
                    "checked": z.check.as_ref().map(|c| c.render(labels, &collator)).unwrap_or_default(),
                })
            }).collect::<Vec<_>>(),
            "treatments": self.treatments.iter().map(|t| json!({
                "plots": t.plot_orders.iter().map(usize::to_string).collect::<Vec<_>>().join(", "),
                // Reglamento (UE) 2023/564's annex puts the BBCH growth stage
                // inside its "Crop or situation/land use" column, and the
                // Spanish model has no column for it — so it folds in here,
                // where the annex says it belongs, with the page's footnote
                // saying what the number is.
                //
                // The NUMBER, not FEGA's wording: the annex asks for the stage
                // "in line with the BBCH monograph" and the monograph's
                // identifier is the number, while the catalogue's labels are
                // whole sentences ("Desarrollo de las partes vegetativas
                // cosechables de la planta o de órganos vegetativos de
                // propagación/ embuchamiento") that would wrap a 15-column
                // landscape register to fourteen lines per row. This is the
                // division the model's own siglas already use — the code
                // prints, the footnote expands it — and the spreadsheet keeps
                // the full name, where a sentence costs nothing.
                "species": join_detail(&t.species, &t.bbch_stages()),
                "variety": t.variety,
                // One date, or the interval Anexo III Parte I B allows — and
                // the annex's start hour beside it, in the same column it heads
                // "Date and where relevant start time (hour)".
                "date": join_detail(
                    &format_date_interval(&t.date, t.end_date.as_deref()),
                    t.time.as_deref().unwrap_or_default(),
                ),
                "surface": format_number(t.surface_ha),
                "problems": t.problems,
                "operator": t.operator_order.map(|o| o.to_string()).unwrap_or_default(),
                // The model's footnote 3: "Manual" when no equipment applied.
                "equipment": t.equipment_order
                    .map(|o| o.to_string())
                    .unwrap_or_else(|| labels.value.manual.to_string()),
                "product": t.product,
                "reg_no": t.reg_no,
                // Blank for a purely non-chemical actuation: there is no dose
                // to state, and a zero would read as one that was measured.
                "dose": match (t.dose_value, t.dose_unit_code.as_deref()) {
                    (Some(value), Some(code)) => {
                        format!("{} {}", format_number(value), unit_symbol(code))
                    }
                    _ => String::new(),
                },
                // Anexo III B.i. Blank when unstated: a total is a measurement.
                "total_quantity": match (t.total_quantity_value, t.total_quantity_unit_code.as_deref()) {
                    (Some(value), Some(code)) => {
                        format!("{} {}", format_number(value), unit_symbol(code))
                    }
                    _ => String::new(),
                },
                // No product, no plazo de seguridad — the cell stays empty
                // rather than claiming a waiting period of zero days.
                "phi": match (t.phi_days, t.phi_end_date.as_deref()) {
                    (Some(days), Some(end)) => labels.phi_phrase(days, &format_date(end)),
                    _ => String::new(),
                },
                "efficacy": labels.efficacy(t.efficacy_code.as_deref()),
                "notes": t.notes,
            })).collect::<Vec<_>>(),
            // 3.1 bis — the advised cut of the very same actuations. Not a
            // second register: Anexo III Parte I B has one list (a-k) covering
            // every treatment, and B.d puts the advisor on it. This page shows
            // the advised rows with the two columns 3.1 has no room for.
            //
            // Reglamento (UE) 2023/564's two conditional fields are deliberately
            // NOT folded in here. They are a duty on the treatment REGISTER, and
            // this page is art. 10-11 GIP compliance as the model renders it —
            // a different column set by design, which already leaves out the
            // total quantity, the plazo and the notes. Every row on it appears in
            // 3.1 above with both fields, so nothing is lost by omitting them.
            "advised": self.treatments.iter().filter(|t| t.is_advised()).map(|t| json!({
                "plots": t.plot_orders.iter().map(usize::to_string).collect::<Vec<_>>().join(", "),
                "species": t.species,
                "variety": t.variety,
                // Blank when the crop states no area of its own: `unwrap_or_default`
                // here would print "0", and a cultivated surface of zero hectares
                // is a statement the farmer never made (slice 3's rule, and the
                // reason this is not filled from the plot's area either).
                "crop_surface": match t.crop_area_ha {
                    Some(area) => format_number(area),
                    None => String::new(),
                },
                "treated_surface": format_number(t.surface_ha),
                "problems": t.problems,
                "justification": t.justification_codes.iter()
                    .map(|c| labels.justification(c))
                    .collect::<Vec<_>>()
                    .join("; "),
                "measure": t.measure_label,
                // "Nº de trampas, nº de difusores, etc."; blank when the
                // measure was recorded without one.
                "intensity": match (t.measure_intensity_value, t.measure_intensity_unit_code.as_deref()) {
                    (Some(value), Some(code)) => {
                        format!(
                            "{} {}",
                            format_number(value),
                            labels.intensity_unit(code, value)
                        )
                    }
                    _ => String::new(),
                },
                // The model gives each alternative its own date column, and a
                // record carries one date: it prints under whichever half of
                // the row it actually describes.
                "measure_date": if t.measure_code.is_some() { format_date(&t.date) } else { String::new() },
                "product": t.product,
                "reg_no": t.reg_no,
                "dose": match (t.dose_value, t.dose_unit_code.as_deref()) {
                    (Some(value), Some(code)) => {
                        format!("{} {}", format_number(value), unit_symbol(code))
                    }
                    _ => String::new(),
                },
                "product_date": if t.product.is_empty() { String::new() } else { format_date(&t.date) },
                "efficacy": labels.efficacy(t.efficacy_code.as_deref()),
                "notes": t.notes,
            })).collect::<Vec<_>>(),
            // The sign-off boxes are hand-signed (the 1.1 signature-box rule),
            // so the book fills in only what it knows for certain: the advisor
            // named on the page's own rows, and only when they all name the
            // same one. Two advisors on one page and the lines stay ruled —
            // guessing which of them signs the interim validation would put a
            // name against a signature nobody gave.
            "advised_advisor": self.advised_advisor().map(|(name, _)| name).unwrap_or_default(),
            "advised_ropo": self.advised_advisor().map(|(_, ropo)| ropo).unwrap_or_default(),
            "seed": self.seed.iter().map(|r| json!({
                "plots": r.plot_orders.iter().map(usize::to_string).collect::<Vec<_>>().join(", "),
                "date": format_date(&r.date),
                "species": r.species,
                "variety": r.variety,
                "surface": format_number(r.surface_ha),
                // Blank rather than zero: an unstated quantity is unknown.
                "seed_quantity": r.seed_quantity_kg.map(format_number).unwrap_or_default(),
                "seed_lot": r.seed_lot,
                // The model prints no column for where the seed was treated, so
                // it rides in the product cell — the book still says everything
                // the record knows, without inventing a column the form lacks.
                "product": join_detail(
                    &r.product,
                    labels.seed_treatment_kind(r.treatment_kind_code.as_deref()),
                ),
                "reg_no": r.reg_no,
                "active_substance": r.active_substance,
                "efficacy": labels.efficacy(r.efficacy_code.as_deref()),
                "notes": r.notes,
            })).collect::<Vec<_>>(),
            "seed_applies_yes": if !self.seed.is_empty() { labels.value.cross } else { "" },
            "seed_applies_no": if self.seed.is_empty()
                && self.declared_empty.iter().any(|c| c == "seed_treatment")
            {
                labels.value.cross
            } else {
                ""
            },
            // Sections 3.3/3.4/3.5, built above: one entry per register, each
            // carrying its own SÍ/NO answer and only its own rows.
            "non_field": non_field,
            "analysis": self.analysis.iter().map(|r| {
                // The model prints one "Laboratorio (nombre y dirección)" cell.
                // Assembled before the macro: `json!` reads a leading `[` as a
                // JSON array, not as a Rust slice expression.
                let laboratory = lab_line(r);
                // Same folding as 3.2: the kinds of analysis ride in the
                // material cell, and the coded findings join the farmer's own
                // wording in the one "sustancias detectadas" cell the model has.
                let material = join_detail(
                    labels.material_kind(&r.material_kind),
                    &r.analysis_types
                        .iter()
                        .map(|code| labels.analysis_type(code))
                        .collect::<Vec<_>>()
                        .join(", "),
                );
                // A.3's soil figures have no column in the model, so they fold
                // in beside the findings — the analysis-kinds precedent, and
                // blank-safe on both sides.
                let substances = join_detail(
                    &join_detail(&r.substances_coded, &r.substances),
                    &soil_cell(&r.soil),
                );
                json!({
                    "plots": r.plot_orders.iter().map(usize::to_string)
                        .collect::<Vec<_>>().join(", "),
                    "date": format_date(&r.date),
                    "material": material,
                    "bulletin": r.bulletin,
                    "laboratory": laboratory,
                    "substances": substances,
                    "notes": r.notes,
                })
            }).collect::<Vec<_>>(),
            "harvest": self.harvest.iter().map(|r| json!({
                "plots": r.plot_orders.iter().map(usize::to_string).collect::<Vec<_>>().join(", "),
                "date": format_date(&r.date),
                "product": r.product,
                // Blank rather than zero: an unstated quantity is unknown.
                "quantity": amount(r.quantity, r.quantity_unit.as_deref()),
                "delivery_note": r.delivery_note,
                "lot": r.lot,
                "buyer": r.buyer,
                "buyer_tax_id": r.buyer_tax_id,
                "buyer_address": r.buyer_address,
                "buyer_registry": r.buyer_registry,
                "notes": r.notes,
            })).collect::<Vec<_>>(),
            "fertilisation": self.fertilisation.iter().map(|r| json!({
                "plots": r.plot_orders.iter().map(usize::to_string).collect::<Vec<_>>().join(", "),
                // Blank rather than zero: no plot stated a surface.
                "area": r.area_ha.map(format_number).unwrap_or_default(),
                "crops": r.crops,
                "dates": format_date_interval(&r.date, r.end_date.as_deref()),
                // C.d's coded kind and the sludge flag have no column of their
                // own; both ride here and take one in the sheet.
                "material": join_detail(
                    &join_detail(&r.material_name, &r.material_kind),
                    if r.sludge { labels.value.sludge_mark } else { "" },
                ),
                "delivery_note": r.delivery_note,
                "richness": richness_cell(r),
                "dose": format!("{} {}", format_number(r.dose), unit_symbol(&r.dose_unit)),
                // The model's single "(F)/(AF)/(AC)" letter merges two legal
                // fields, so the cell carries the sigla AND the forma de
                // aplicación the letter drops.
                "kind": join_detail(
                    // Parenthesised, the way the model's own footnote writes
                    // them; blank when the model defines no sigla.
                    &match labels.fertilisation_sigla(&r.type_code, r.fertigation) {
                        sigla if sigla.is_empty() => String::new(),
                        sigla => format!("({sigla})"),
                    },
                    &join_detail(
                        labels.fertilisation_type(&r.type_code),
                        labels.application_method(&r.method_code),
                    ),
                ),
                // C.g's machine or C.k's service company with its REGFER.
                "applicator": join_detail(
                    &r.machinery,
                    &join_detail(&r.service_company, &r.service_regfer),
                ),
                "yield_estimated": r.yield_estimated.map(format_number).unwrap_or_default(),
                "yield_final": r.yield_final.map(format_number).unwrap_or_default(),
                "notes": r.notes,
            })).collect::<Vec<_>>(),
            "plan_rows": self.plan_rows.iter().map(|r| json!({
                "plots": r.plot_orders.iter().map(usize::to_string).collect::<Vec<_>>().join(", "),
                "crop": r.crops,
                "date": format_date(&r.date),
                "area": r.area_ha.map(format_number).unwrap_or_default(),
                "fertiliser": r.material_name,
                "richness": nutrient_cell(Nutrients {
                    n: r.richness_n, p2o5: r.richness_p2o5, k2o: r.richness_k2o,
                }),
                "dose": format!("{} {}", format_number(r.dose), unit_symbol(&r.dose_unit)),
                // Computed from section 6, never stored — the footnote says so.
                "supplied": nutrient_cell(r.supplied),
                "accumulated": nutrient_cell(r.accumulated),
                // The only stored block of this table.
                "recommended": nutrient_cell(r.recommended),
            })).collect::<Vec<_>>(),
            "irrigation": self.irrigation.iter().map(|r| json!({
                "plots": r.plot_orders.iter().map(usize::to_string).collect::<Vec<_>>().join(", "),
                // Blank rather than zero: no plot stated a surface.
                "area": r.area_ha.map(format_number).unwrap_or_default(),
                "method": labels.irrigation_method(&r.method_code),
                "dates": format_date_interval(&r.date, r.end_date.as_deref()),
                "volume": format!("{} {}", format_number(r.volume), unit_symbol(&r.volume_unit)),
                // Only m3/ha records accumulate; the rest print blank and the
                // footnote says why.
                "cumulative": r.cumulative_m3_ha.map(format_number).unwrap_or_default(),
                // Anexo III C.l's two figures fold into one printed cell, the
                // model having no column for either; the sheet splits them.
                "water_quality": join_detail(
                    &r.water_nitric_n.map(format_number).unwrap_or_default(),
                    &r.water_soluble_p2o5.map(format_number).unwrap_or_default(),
                ),
                "source": r.origin_codes.iter()
                    .map(|code| labels.water_origin(code))
                    .collect::<Vec<_>>().join("; "),
                "notes": r.notes,
            })).collect::<Vec<_>>(),
            "grazing": self.grazing.iter().map(|r| json!({
                "group_ref": r.group_ref,
                "plot_reference": r.plot_reference,
                "started_on": format_date(&r.started_on),
                // Blank while the animals are still out: the deadline runs from
                // this date, so an empty cell says "not finished", and the
                // footnote says exactly that.
                "ended_on": if r.ended_on.is_empty() { String::new() } else { format_date(&r.ended_on) },
                // A catalogue label, so it prints verbatim in every language.
                "species": r.species,
                "rega": r.rega,
                "animal_count": r.animal_count.to_string(),
            })).collect::<Vec<_>>(),
            "mowing": self.mowing.iter().map(|r| json!({
                "order": r.order.to_string(),
                "province": r.sigpac.province,
                "municipality": r.sigpac.municipality,
                "polygon": r.sigpac.polygon,
                "parcel": r.sigpac.parcel,
                "enclosure": r.sigpac.enclosure,
                // The provider's surface, blank when unknown — never the
                // farmer's own figure under a heading that says SIGPAC.
                "sigpac_area": r.sigpac.area_ha.map(format_number).unwrap_or_default(),
                "mowing": format_activities(&r.mowing),
                "tillage": format_activities(&r.tillage),
                "sowing": format_activities(&r.sowing),
                "maintenance": format_activities(&r.maintenance),
            })).collect::<Vec<_>>(),
            "communal": self.communal.iter().map(|r| json!({
                "plot_ids": r.plot_orders.iter().map(usize::to_string)
                    .collect::<Vec<_>>().join(", "),
                "plots": r.plot_names,
                "performed_on": format_date(&r.performed_on),
                "performed_end_date": if r.performed_end_date.is_empty() {
                    String::new()
                } else {
                    format_date(&r.performed_end_date)
                },
                "activity": r.activity,
            })).collect::<Vec<_>>(),
            "flooded": self.flooded.iter().map(|r| json!({
                "order": r.order.to_string(),
                // Art. 45.2's own order. The model's three columns keep their
                // relative places; the two it lacks sit where the article
                // names them.
                "levelling": format_activities(&r.levelling),
                "sowing": format_activities(&r.sowing),
                "flooding": format_activities(&r.flooding),
                "drying": format_activities(&r.drying),
                "ridging": format_activities(&r.ridging),
            })).collect::<Vec<_>>(),
            // Models 9.4 and 9.5. One projection function feeds both, and the
            // 9.5 rows simply leave the three maintenance cells unread — art.
            // 43 asks for no maintenance of an inert cover.
            "plant_covers": self.plant_covers.iter().map(cover_json).collect::<Vec<_>>(),
            "inert_covers": self.inert_covers.iter().map(cover_json).collect::<Vec<_>>(),
        })
    }
}

/// One printed cover row, for either page.
///
/// The widths print blank rather than as a zero when art. 42.1.e's separate
/// annotation has not been made yet: a blank cell says "not stated", which is
/// what is true, while "0" would say the cover has no width.
fn cover_json(r: &CoverRow) -> serde_json::Value {
    json!({
        "plot_ids": r.plot_orders.iter().map(usize::to_string).collect::<Vec<_>>().join(", "),
        "plots": r.plot_names,
        "established_on": format_date(&r.established_on),
        "width": r.width_m,
        "free_canopy_width": r.free_canopy_width_m,
        "mowing": format_activities(&r.mowing),
        "brush_cutting": format_activities(&r.brush_cutting),
        "grazing": format_activities(&r.grazing),
    })
}

// ---------------------------------------------------------------------------
// Renderer 2 — the spreadsheet (typed cells: real dates, real numbers)
// ---------------------------------------------------------------------------

/// Sheet names carry the model's own section numbers, so a reader moving
/// between the PDF and the workbook lands in the same place. Excel caps tab
/// names at 31 characters — the engine truncates, these already fit.
impl Cuaderno {
    /// Takes the LANGUAGE, for the same reason as [`Cuaderno::to_typst`].
    fn to_workbook(&self, language: ReportLanguage) -> Workbook {
        let labels = language.labels();
        let collator = NameCollator::new(language);
        let mut book = Workbook::new();
        book.push(self.sheet_farm(labels));
        book.push(self.sheet_operators(labels));
        book.push(self.sheet_machinery(labels));
        book.push(self.sheet_advisors(labels));
        book.push(self.sheet_plots(labels));
        book.push(self.sheet_zones(labels, &collator));
        book.push(self.sheet_water_points(labels));
        book.push(self.sheet_treatments(labels));
        book.push(self.sheet_seed(labels));
        for kind in NON_FIELD_KINDS {
            book.push(self.sheet_non_field(labels, kind));
        }
        book.push(self.sheet_analysis(labels));
        book.push(self.sheet_soil(labels));
        book.push(self.sheet_harvest(labels));
        book.push(self.sheet_fertilisation(labels));
        book.push(self.sheet_materials(labels));
        book.push(self.sheet_plan(labels));
        book.push(self.sheet_irrigation(labels));
        book.push(self.sheet_grazing(labels));
        book.push(self.sheet_cultural_operations(labels));
        book.push(self.sheet_covers(labels));
        book.push(self.sheet_sowing(labels));
        book
    }

    /// 1.1 as a label/value sheet rather than one wide row: the general data
    /// is a form block, and reading it down a column beats scrolling sideways.
    fn sheet_farm(&self, labels: &Labels) -> Sheet {
        // The 1.1 labels drop the form's trailing colon here: a spreadsheet
        // column header is not a form field caption.
        let field = |label: &str| label.trim_end_matches(':').to_string();
        let mut sheet = Sheet::new(
            labels.sheet.tab_farm,
            vec![
                Column::new(labels.sheet.field, 34.0),
                Column::new(labels.sheet.value, 46.0),
            ],
        );
        let pairs = [
            (labels.sheet.campaign.to_string(), self.campaign.as_str()),
            (field(labels.s1.farm_name), self.farm.name.as_str()),
            (field(labels.s1.owner_name), self.farm.owner.as_str()),
            (field(labels.s1.tax_id), self.farm.nif.as_str()),
            (field(labels.s1.registry_national), self.farm.siex.as_str()),
            (
                labels.sheet.registry_regional.to_string(),
                self.farm.rea.as_str(),
            ),
            (field(labels.s1.address), self.farm.address.as_str()),
            (field(labels.s1.locality), self.farm.location.as_str()),
            (field(labels.s1.postal_code), self.farm.postal_code.as_str()),
            (field(labels.s1.province), self.farm.province.as_str()),
            (field(labels.s1.phone_fixed), self.farm.phone_fixed.as_str()),
            (
                field(labels.s1.phone_mobile),
                self.farm.phone_mobile.as_str(),
            ),
            (field(labels.s1.email), self.farm.email.as_str()),
        ];
        for (label, value) in pairs {
            sheet.push(vec![Cell::text(label.as_str()), Cell::text(value)]);
        }
        if let Some(rep) = &self.farm.representative {
            let prefix = labels.sheet.representative_prefix;
            for (label, value) in [
                (field(labels.s1.full_name), rep.name.as_str()),
                (field(labels.s1.tax_id), rep.nif.as_str()),
                (field(labels.s1.representation_kind), rep.kind.as_str()),
                (field(labels.s1.address), rep.address.as_str()),
                (field(labels.s1.locality), rep.locality.as_str()),
                (field(labels.s1.postal_code), rep.postal_code.as_str()),
                (field(labels.s1.phone), rep.phone.as_str()),
                (field(labels.s1.email), rep.email.as_str()),
            ] {
                sheet.push(vec![
                    Cell::text(format!("{prefix}: {label}").as_str()),
                    Cell::text(value),
                ]);
            }
        }
        sheet.push(vec![
            Cell::text(labels.sheet.generated_on),
            Cell::date(Some(&self.generated_on)),
        ]);
        sheet
    }

    fn sheet_operators(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_operators,
            vec![
                Column::new(labels.s12.order, 12.0),
                Column::new(labels.sheet.operator_name, 32.0),
                Column::new(labels.s12.tax_id, 14.0),
                Column::new(labels.s12.licence_number, 26.0),
                Column::new(labels.s12.licence_level, 16.0),
                Column::new(labels.s12.advisor, 10.0),
            ],
        );
        for o in &self.operators {
            sheet.push(vec![
                Cell::Number(o.order as f64),
                Cell::text(o.name.as_str()),
                Cell::text(o.tax_id.as_str()),
                Cell::text(o.licence.as_str()),
                Cell::text(labels.licence_level(o.level.as_deref())),
                // "SÍ"/"NO" rather than the form's cross: a filterable column
                // needs both answers spelled out, and the cross is a paper
                // convention (the PDF keeps it).
                Cell::text(labels.yes_no(o.is_advisor)),
            ]);
        }
        sheet
    }

    fn sheet_advisors(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_advisors,
            vec![
                Column::new(labels.s14.name, 34.0),
                Column::new(labels.s14.tax_id, 14.0),
                Column::new(labels.s14.registration_number, 22.0),
                Column::new(labels.s14.gip, 20.0),
                Column::new(labels.sheet.gip_code, 22.0),
            ],
        );
        for a in &self.advisors {
            sheet.push(vec![
                Cell::text(a.name.as_str()),
                Cell::text(a.tax_id.as_str()),
                Cell::text(a.registration_number.as_str()),
                Cell::text(a.gip),
                Cell::text(a.gip_code.as_str()),
            ]);
        }
        sheet
    }

    fn sheet_machinery(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_machinery,
            vec![
                Column::new(labels.s13.order, 12.0),
                Column::new(labels.s13.description, 32.0),
                Column::new(labels.s13.roma, 18.0),
                Column::new(labels.s13.reganip, 20.0),
                Column::new(labels.s13.acquired_on, 21.0),
                Column::new(labels.s13.last_inspection, 26.0),
            ],
        );
        for m in &self.machinery {
            sheet.push(vec![
                Cell::Number(m.order as f64),
                Cell::text(m.description.as_str()),
                Cell::text(m.roma.as_str()),
                Cell::text(m.reganip.as_str()),
                Cell::date(m.acquired_on.as_deref()),
                Cell::date(m.last_inspection.as_deref()),
            ]);
        }
        sheet
    }

    fn sheet_plots(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_plots,
            vec![
                Column::new(labels.s21.order, 12.0),
                Column::new(labels.s21.plot, 20.0),
                Column::new(labels.sheet.plot_province, 11.0),
                Column::new(labels.sheet.plot_municipality, 11.0),
                Column::new(labels.sheet.plot_municipality_name, 24.0),
                Column::new(labels.sheet.plot_aggregate, 10.0),
                Column::new(labels.s21.zone, 8.0),
                Column::new(labels.sheet.plot_polygon, 10.0),
                Column::new(labels.sheet.plot_parcel, 15.0),
                Column::new(labels.sheet.plot_enclosure, 9.0),
                Column::new(labels.s21.land_use, 12.0),
                Column::new(labels.sheet.sigpac_area, 20.0),
                Column::new(labels.sheet.cultivated_area, 22.0),
                Column::new(labels.s21.species, 18.0),
                Column::new(labels.s21.variety, 16.0),
                Column::new(labels.s21.irrigation, 17.0),
                Column::new(labels.s21.environment, 21.0),
                Column::new(labels.s21.gip, 8.0),
            ],
        );
        for p in &self.plots {
            sheet.push(vec![
                Cell::Number(p.order as f64),
                Cell::text(p.name.as_str()),
                Cell::text(p.province.as_str()),
                Cell::text(p.municipality.as_str()),
                Cell::text(p.municipality_name.as_str()),
                Cell::text(p.aggregate.as_str()),
                Cell::text(p.zone.as_str()),
                Cell::text(p.polygon.as_str()),
                Cell::text(p.parcel.as_str()),
                Cell::text(p.enclosure.as_str()),
                Cell::text(p.land_use.as_str()),
                Cell::number(p.sigpac_area_ha),
                Cell::number(p.cultivated_area_ha),
                Cell::text(p.species.as_str()),
                Cell::text(p.variety.as_str()),
                Cell::text(p.irrigation),
                Cell::text(p.environment),
                Cell::text(p.gip),
            ]);
        }
        sheet
    }

    fn sheet_zones(&self, labels: &Labels, collator: &NameCollator) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_zones,
            vec![
                Column::new(labels.sheet.plot_id, 12.0),
                Column::new(labels.s22.species, 18.0),
                Column::new(labels.s22.variety, 16.0),
                Column::new(labels.sheet.water_point, 30.0),
                Column::new(labels.s22.distance, 14.0),
                Column::new(labels.s22.coordinates, 22.0),
                Column::new(labels.s22.denomination, 26.0),
                Column::new(labels.s22.fully, 28.0),
                Column::new(labels.s22.partly, 14.0),
                Column::new(labels.s22.checked, 42.0),
            ],
        );
        let yes_no = |flag: Option<bool>| match flag {
            Some(flag) => Cell::Text(labels.yes_no(flag).into()),
            None => Cell::Empty,
        };
        for z in &self.zones {
            // The section's row is per plot, so several points join into one
            // string here; the 2.2 Captaciones tab is where each becomes a row
            // with typed cells. Keeping both means the two documents reconcile
            // row for row AND the numbers stay summable.
            let [water_point, distance, coordinates, denomination] = z.water_cells(labels);
            sheet.push(vec![
                Cell::Number(z.order as f64),
                Cell::text(z.species.as_str()),
                Cell::text(z.variety.as_str()),
                Cell::text(water_point.as_str()),
                Cell::text(distance.as_str()),
                Cell::text(coordinates.as_str()),
                Cell::text(denomination.as_str()),
                yes_no(z.fully),
                yes_no(z.partly),
                Cell::text(
                    z.check
                        .as_ref()
                        .map(|c| c.render(labels, collator))
                        .unwrap_or_default()
                        .as_str(),
                ),
            ]);
        }
        sheet
    }

    /// 2.2's water half, one row per abstraction point instead of per plot.
    ///
    /// The section's own tab has to join several points on one plot into a
    /// string, and a joined string cannot be sorted, filtered or summed — which
    /// is the whole reason the book also exports as a spreadsheet. Here each
    /// point is a row and its distance and coordinates are real numbers.
    ///
    /// A plot declared free of abstraction points writes a single row carrying
    /// that statement: with no points to hang it on, the declaration IS the
    /// content (the same call the declared-empty registers make in 3.3–3.5).
    fn sheet_water_points(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_water_points,
            vec![
                Column::new(labels.sheet.plot_id, 12.0),
                Column::new(labels.s21.plot, 24.0),
                Column::new(labels.s22.denomination, 26.0),
                Column::new(labels.sheet.water_point, 30.0),
                Column::new(labels.s22.distance, 14.0),
                Column::new(labels.sheet.latitude, 14.0),
                Column::new(labels.sheet.longitude, 14.0),
            ],
        );
        for z in &self.zones {
            for point in &z.water {
                let (latitude, longitude) = match point.coordinates {
                    Some((lat, lon)) => (Cell::Number(lat), Cell::Number(lon)),
                    None => (Cell::Empty, Cell::Empty),
                };
                sheet.push(vec![
                    Cell::Number(z.order as f64),
                    Cell::text(z.plot_name.as_str()),
                    Cell::text(point.denomination.as_str()),
                    Cell::Text(labels.yes_no(point.inside_plot).into()),
                    // Blank, never zero: a point inside the plot has no
                    // distance to state, and a zero would be a measurement.
                    point.distance_m.map(Cell::Number).unwrap_or(Cell::Empty),
                    latitude,
                    longitude,
                ]);
            }
            if z.water.is_empty()
                && let Some(declared_on) = &z.water_declared_on
            {
                sheet.push(vec![
                    Cell::Number(z.order as f64),
                    Cell::text(z.plot_name.as_str()),
                    Cell::Text(format!(
                        "{} — {}",
                        labels.value.no_water_points,
                        format_date(declared_on)
                    )),
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                ]);
            }
        }
        sheet
    }

    fn sheet_treatments(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_treatments,
            vec![
                Column::new(labels.sheet.plot_ids, 13.0),
                Column::new(labels.sheet.plots, 24.0),
                Column::new(labels.s31.species, 18.0),
                Column::new(labels.s31.variety, 16.0),
                // Reglamento (UE) 2023/564's two conditional fields. The PDF
                // folds each into the cell the annex places it in, because the
                // Spanish model has no column for either; here they get columns
                // of their own, because a value joined into a neighbour cannot
                // be filtered — the total_quantity precedent below.
                Column::new(labels.sheet.growth_stage, 30.0),
                // The PDF prints one "intervalo de fechas" cell; the sheet
                // splits it, because a range in one cell cannot be sorted or
                // filtered — which is the whole point of the sheet.
                Column::new(labels.sheet.date_start, 12.0),
                Column::new(labels.sheet.date_end, 12.0),
                Column::new(labels.sheet.application_time, 12.0),
                Column::new(labels.sheet.drying_date, 14.0),
                Column::new(labels.sheet.treated_area, 21.0),
                Column::new(labels.s31.problem, 30.0),
                Column::new(labels.sheet.operator_order, 13.0),
                Column::new(labels.sheet.operator, 24.0),
                Column::new(labels.sheet.equipment_order, 11.0),
                Column::new(labels.sheet.equipment, 22.0),
                Column::new(labels.s31.product, 24.0),
                Column::new(labels.sheet.registration, 14.0),
                Column::new(labels.s31.dose, 10.0),
                Column::new(labels.sheet.dose_unit, 16.0),
                Column::new(labels.sheet.total_quantity, 16.0),
                Column::new(labels.sheet.total_quantity_unit, 18.0),
                Column::new(labels.sheet.phi_days, 23.0),
                Column::new(labels.sheet.harvest_from, 23.0),
                Column::new(labels.s31.efficacy, 12.0),
                Column::new(labels.s31.notes, 34.0),
                // Model 3.1 bis's own columns. They ride on THIS tab rather
                // than a tab of their own because 3.1 bis is a view of these
                // very rows, not a second register: duplicating every advised
                // actuation onto another sheet would make one event two rows,
                // and filtering on a non-empty Asesor gives the same page back
                // — which is what a spreadsheet is for.
                Column::new(labels.s31bis.justification, 34.0),
                Column::new(labels.s31bis.advisor, 24.0),
                Column::new(labels.s31bis.ropo, 18.0),
                Column::new(labels.s31bis.measure, 30.0),
                Column::new(labels.s31bis.intensity, 20.0),
                Column::new(labels.sheet.intensity_unit, 20.0),
                Column::new(labels.sheet.measure_registration, 22.0),
            ],
        );
        for t in &self.treatments {
            sheet.push(vec![
                Cell::text(
                    t.plot_orders
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Cell::text(t.plot_names.as_str()),
                Cell::text(t.species.as_str()),
                Cell::text(t.variety.as_str()),
                Cell::text(t.stage_names()),
                Cell::date(Some(&t.date)),
                // Blank rather than repeating the start: an empty cell says
                // "one day", a repeated date would invent an interval.
                Cell::date(t.end_date.as_deref()),
                // Text, not a time-typed cell: this is a local wall-clock hour
                // with no date and no zone, and Excel's time type would anchor
                // it to a serial day the record never stated.
                Cell::text(t.time.as_deref().unwrap_or_default()),
                // A real date cell, unlike the hour above: this one IS a day.
                Cell::date(t.drying_date.as_deref()),
                Cell::Number(t.surface_ha),
                Cell::text(t.problems.as_str()),
                Cell::number(t.operator_order.map(|o| o as f64)),
                Cell::text(t.operator_name.as_str()),
                Cell::number(t.equipment_order.map(|o| o as f64)),
                // Manual application is a value, not a gap (model footnote 3).
                Cell::text(if t.equipment_order.is_none() {
                    labels.value.manual
                } else {
                    t.equipment_name.as_str()
                }),
                Cell::text(t.product.as_str()),
                Cell::text(t.reg_no.as_str()),
                Cell::number(t.dose_value),
                Cell::text(
                    t.dose_unit_code
                        .as_deref()
                        .map(unit_symbol)
                        .unwrap_or_default(),
                ),
                // Value and unit apart, like dose: "9 L" in one cell is not
                // summable, and summing product used is what a sheet is for.
                Cell::number(t.total_quantity_value),
                Cell::text(
                    t.total_quantity_unit_code
                        .as_deref()
                        .map(unit_symbol)
                        .unwrap_or_default(),
                ),
                Cell::number(t.phi_days.map(|d| d as f64)),
                Cell::date(t.phi_end_date.as_deref()),
                Cell::text(labels.efficacy(t.efficacy_code.as_deref())),
                Cell::text(t.notes.as_str()),
                Cell::text(
                    t.justification_codes
                        .iter()
                        .map(|c| labels.justification(c))
                        .collect::<Vec<_>>()
                        .join("; "),
                ),
                Cell::text(t.advisor_name.as_deref().unwrap_or_default()),
                Cell::text(t.advisor_registration.as_deref().unwrap_or_default()),
                Cell::text(t.measure_label.as_str()),
                // Value and unit apart, so a column of intensities can be
                // sorted — the dose rule.
                Cell::number(t.measure_intensity_value),
                // The unit still agrees with the value in its neighbouring
                // cell; with no value recorded it falls to the plural, which
                // is the form a bare unit column reads best in.
                Cell::text(
                    t.measure_intensity_unit_code
                        .as_deref()
                        .map(|c| {
                            labels.intensity_unit(c, t.measure_intensity_value.unwrap_or_default())
                        })
                        .unwrap_or_default(),
                ),
                Cell::text(t.measure_registration_number.as_deref().unwrap_or_default()),
            ]);
        }
        sheet
    }
}

impl Cuaderno {
    /// Section 3.2. Carries the register's SÍ/NO answer in the same column the
    /// non-field tabs use, so the four conditional registers read alike.
    fn sheet_seed(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_seed,
            vec![
                Column::new(labels.sheet.register_applies, 18.0),
                Column::new(labels.sheet.plot_ids, 13.0),
                Column::new(labels.sheet.plots, 24.0),
                Column::new(labels.s32.date, 14.0),
                Column::new(labels.s32.species, 18.0),
                Column::new(labels.s32.variety, 16.0),
                Column::new(labels.sheet.sown_area, 22.0),
                Column::new(labels.sheet.seed_quantity, 22.0),
                Column::new(labels.s32.seed_lot, 16.0),
                Column::new(labels.sheet.seed_treatment_kind, 30.0),
                Column::new(labels.s32.product, 24.0),
                Column::new(labels.sheet.registration, 14.0),
                Column::new(labels.s32.active_substance, 26.0),
                Column::new(labels.s32.efficacy, 12.0),
                Column::new(labels.s32.notes, 34.0),
            ],
        );
        let answer = register_answer(
            self.seed.len(),
            self.declared_empty.iter().any(|c| c == "seed_treatment"),
        );
        let applies = Cell::text(answer.map(|a| labels.yes_no(a)).unwrap_or_default());
        if self.seed.is_empty() {
            if answer == Some(false) {
                let mut row = vec![applies];
                row.extend(std::iter::repeat_n(Cell::Empty, 14));
                sheet.push(row);
            }
            return sheet;
        }
        for r in &self.seed {
            sheet.push(vec![
                applies.clone(),
                Cell::text(
                    r.plot_orders
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Cell::text(r.plot_names.as_str()),
                Cell::date(Some(&r.date)),
                Cell::text(r.species.as_str()),
                Cell::text(r.variety.as_str()),
                Cell::Number(r.surface_ha),
                Cell::number(r.seed_quantity_kg),
                Cell::text(r.seed_lot.as_str()),
                // Its own column here, where the PDF folds it into the product
                // cell: a sheet is filtered per field.
                Cell::text(labels.seed_treatment_kind(r.treatment_kind_code.as_deref())),
                Cell::text(r.product.as_str()),
                Cell::text(r.reg_no.as_str()),
                Cell::text(r.active_substance.as_str()),
                Cell::text(labels.efficacy(r.efficacy_code.as_deref())),
                Cell::text(r.notes.as_str()),
            ]);
        }
        sheet
    }

    /// One tab per non-field register. The "APLICA TRATAMIENTO" answer is a
    /// column rather than a heading, because a sheet has no headings — and it
    /// stays SÍ/NO/blank, so a register nobody has touched is distinguishable
    /// from one declared empty.
    fn sheet_non_field(&self, labels: &Labels, kind: &str) -> Sheet {
        let (tab, subject, quantity) = match kind {
            "postharvest" => (
                labels.sheet.tab_postharvest,
                labels.s33.subject_postharvest,
                labels.s33.quantity_postharvest,
            ),
            "storage_premises" => (
                labels.sheet.tab_storage,
                labels.s33.subject_storage,
                labels.s33.quantity_storage,
            ),
            _ => (
                labels.sheet.tab_transport,
                labels.s33.subject_transport,
                labels.s33.quantity_transport,
            ),
        };
        let mut sheet = Sheet::new(
            tab,
            vec![
                Column::new(labels.sheet.register_applies, 18.0),
                Column::new(labels.s33.date, 12.0),
                Column::new(subject, 34.0),
                Column::new(quantity, 16.0),
                Column::new(labels.sheet.quantity_unit, 10.0),
                Column::new(labels.s33.problem, 30.0),
                Column::new(labels.s33.operator, 24.0),
                // The PDF joins these into the applicator cell the model gives
                // it; a sheet is filtered per field, so here they are their own.
                Column::new(labels.sheet.advisor_name, 24.0),
                Column::new(labels.sheet.advisor_registration, 16.0),
                Column::new(labels.s33.product, 24.0),
                Column::new(labels.sheet.registration, 14.0),
                Column::new(labels.sheet.product_quantity, 18.0),
                Column::new(labels.sheet.product_quantity_unit, 18.0),
                Column::new(labels.s33.efficacy, 12.0),
                Column::new(labels.s33.notes, 34.0),
            ],
        );
        let rows: Vec<&NonFieldRow> = self
            .non_field
            .iter()
            .filter(|r| r.subject_kind == kind)
            .collect();
        let answer = register_answer(rows.len(), self.declared_empty.iter().any(|c| c == kind));
        // SÍ / NO / blank — a register nobody has touched is not a "no".
        let applies = Cell::text(answer.map(|a| labels.yes_no(a)).unwrap_or_default());
        // A register declared empty still gets its answer on the sheet, even
        // with no rows to attach it to — that declaration IS the content.
        if rows.is_empty() {
            if answer == Some(false) {
                let mut row = vec![applies];
                row.extend(std::iter::repeat_n(Cell::Empty, 14));
                sheet.push(row);
            }
            return sheet;
        }
        for r in rows {
            sheet.push(vec![
                applies.clone(),
                Cell::date(Some(&r.date)),
                Cell::text(r.subject.as_str()),
                Cell::number(r.quantity_value),
                Cell::text(unit_symbol_opt(r.quantity_unit_code.as_deref())),
                Cell::text(r.problems.as_str()),
                Cell::text(r.operator_name.as_str()),
                Cell::text(r.advisor_name.as_str()),
                Cell::text(r.advisor_registration.as_str()),
                Cell::text(r.product.as_str()),
                Cell::text(r.reg_no.as_str()),
                Cell::number(r.product_quantity_value),
                Cell::text(unit_symbol_opt(r.product_quantity_unit_code.as_deref())),
                Cell::text(labels.efficacy(r.efficacy_code.as_deref())),
                Cell::text(r.notes.as_str()),
            ]);
        }
        sheet
    }

    /// Section 4. The laboratory's three fields get a column each here, where
    /// the PDF joins them into the model's single "nombre y dirección" cell —
    /// a sheet is filtered per field, a form is read per line.
    fn sheet_analysis(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_analysis,
            vec![
                Column::new(labels.s4.date, 14.0),
                Column::new(labels.s4.material, 16.0),
                Column::new(labels.sheet.analysis_types, 34.0),
                Column::new(labels.sheet.plot_ids, 13.0),
                Column::new(labels.sheet.plots, 24.0),
                Column::new(labels.s4.bulletin, 18.0),
                Column::new(labels.sheet.lab_name, 30.0),
                Column::new(labels.sheet.lab_address, 30.0),
                Column::new(labels.sheet.lab_tax_id, 14.0),
                Column::new(labels.sheet.substances_coded, 34.0),
                Column::new(labels.s4.substances, 34.0),
                Column::new(labels.s31.notes, 30.0),
            ],
        );
        for r in &self.analysis {
            sheet.push(vec![
                Cell::date(Some(&r.date)),
                Cell::text(labels.material_kind(&r.material_kind)),
                Cell::text(
                    r.analysis_types
                        .iter()
                        .map(|code| labels.analysis_type(code))
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Cell::text(
                    r.plot_orders
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Cell::text(r.plot_names.as_str()),
                Cell::text(r.bulletin.as_str()),
                Cell::text(r.lab_name.as_str()),
                Cell::text(r.lab_address.as_str()),
                Cell::text(r.lab_tax_id.as_str()),
                Cell::text(r.substances_coded.as_str()),
                Cell::text(r.substances.as_str()),
                Cell::text(r.notes.as_str()),
            ]);
        }
        sheet
    }

    /// Section 5. Quantity and unit are separate columns, like every other
    /// amount in the book: "42,5 t" in one cell is not summable.
    fn sheet_harvest(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_harvest,
            vec![
                Column::new(labels.s5.date, 14.0),
                Column::new(labels.s5.product, 22.0),
                Column::new(labels.s5.quantity, 12.0),
                Column::new(labels.sheet.quantity_unit, 10.0),
                Column::new(labels.sheet.plot_ids, 13.0),
                Column::new(labels.sheet.plots, 24.0),
                Column::new(labels.s5.delivery_note, 18.0),
                Column::new(labels.s5.lot, 14.0),
                Column::new(labels.s5.buyer, 28.0),
                Column::new(labels.s5.buyer_tax_id, 14.0),
                Column::new(labels.s5.buyer_address, 30.0),
                Column::new(labels.s5.buyer_registry, 16.0),
                Column::new(labels.s31.notes, 30.0),
            ],
        );
        for r in &self.harvest {
            sheet.push(vec![
                Cell::date(Some(&r.date)),
                Cell::text(r.product.as_str()),
                // Blank stays blank: a spreadsheet would add a zero up.
                Cell::number(r.quantity),
                Cell::text(unit_symbol_opt(r.quantity_unit.as_deref())),
                Cell::text(
                    r.plot_orders
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Cell::text(r.plot_names.as_str()),
                Cell::text(r.delivery_note.as_str()),
                Cell::text(r.lot.as_str()),
                Cell::text(r.buyer.as_str()),
                Cell::text(r.buyer_tax_id.as_str()),
                Cell::text(r.buyer_address.as_str()),
                Cell::text(r.buyer_registry.as_str()),
                Cell::text(r.notes.as_str()),
            ]);
        }
        sheet
    }
    /// Section 8. The PDF joins a record's water sources into one cell and
    /// prints a running total; here the sources are one filterable column and
    /// every number is a real number the reader can sum for themselves.
    /// Section 6. Everything the PDF folds into a neighbouring cell gets its
    /// own column here — the coded material kind, the two legal fields the
    /// model's single letter merges, the sludge flag, the service company, the
    /// three richness figures, and the good practices the printed model has no
    /// column for at all.
    fn sheet_fertilisation(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_fertilisation,
            vec![
                Column::new(labels.s6.dates, 20.0),
                Column::new(labels.sheet.plot_ids, 13.0),
                Column::new(labels.sheet.plots, 24.0),
                Column::new(labels.s6.area, 12.0),
                Column::new(labels.s6.crop, 22.0),
                Column::new(labels.s6.material, 26.0),
                Column::new(labels.sheet.material_kind, 24.0),
                Column::new(labels.sheet.sludge, 14.0),
                Column::new(labels.sheet.sustainable_inputs, 26.0),
                Column::new(labels.sheet.richness_n, 12.0),
                Column::new(labels.sheet.richness_p2o5, 14.0),
                Column::new(labels.sheet.richness_k2o, 12.0),
                Column::new(labels.s6.dose, 10.0),
                Column::new(labels.sheet.quantity_unit, 10.0),
                Column::new(labels.sheet.fertilisation_type, 22.0),
                Column::new(labels.sheet.application_method, 26.0),
                Column::new(labels.s6.delivery_note, 16.0),
                Column::new(labels.s13.description, 22.0),
                Column::new(labels.sheet.service_company, 24.0),
                Column::new(labels.sheet.service_regfer, 14.0),
                Column::new(labels.s6.yield_estimated, 20.0),
                Column::new(labels.s6.yield_final, 18.0),
                Column::new(labels.sheet.practices, 40.0),
                Column::new(labels.s31.notes, 30.0),
            ],
        );
        for r in &self.fertilisation {
            sheet.push(vec![
                Cell::date(Some(&r.date)),
                Cell::text(
                    r.plot_orders
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Cell::text(r.plot_names.as_str()),
                // Blank, never zero: an unstated surface is unknown, and a
                // spreadsheet would add a zero up.
                Cell::number(r.area_ha),
                Cell::text(r.crops.as_str()),
                Cell::text(r.material_name.as_str()),
                Cell::text(r.material_kind.as_str()),
                Cell::text(if r.sludge {
                    labels.value.yes
                } else {
                    labels.value.no
                }),
                Cell::text(if r.sustainable_inputs {
                    labels.value.yes
                } else {
                    labels.value.no
                }),
                Cell::number(r.richness_n),
                Cell::number(r.richness_p2o5),
                Cell::number(r.richness_k2o),
                Cell::Number(r.dose),
                Cell::text(unit_symbol(&r.dose_unit)),
                Cell::text(labels.fertilisation_type(&r.type_code)),
                Cell::text(labels.application_method(&r.method_code)),
                Cell::text(r.delivery_note.as_str()),
                Cell::text(r.machinery.as_str()),
                Cell::text(r.service_company.as_str()),
                Cell::text(r.service_regfer.as_str()),
                Cell::number(r.yield_estimated),
                Cell::number(r.yield_final),
                Cell::text(r.practices.as_str()),
                Cell::text(r.notes.as_str()),
            ]);
        }
        sheet
    }

    /// The material registry, one row per composition figure.
    ///
    /// Not a section of the printed model, and that is exactly why it is here:
    /// Anexo III C.h asks for eight agronomic values per material, C.i adds the
    /// sludge heavy metals and a label may declare micronutrients on top. A
    /// register row has no room for any of that, and every one of those figures
    /// is a real number worth filtering and comparing.
    fn sheet_materials(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_materials,
            vec![
                Column::new(labels.s6.material, 26.0),
                Column::new(labels.sheet.material_kind, 26.0),
                Column::new(labels.s31.product, 30.0),
                Column::new(labels.sheet.supplier, 24.0),
                Column::new(labels.sheet.supplier_registry, 18.0),
                Column::new(labels.sheet.manure_treatment, 26.0),
                Column::new(labels.sheet.density, 14.0),
                Column::new(labels.sheet.nutrient_group, 16.0),
                Column::new(labels.sheet.nutrient, 30.0),
                Column::new(labels.sheet.percentage, 10.0),
            ],
        );
        for m in &self.materials {
            // A material with no composition still gets a row: it exists, and a
            // sheet that hid it would misreport the registry.
            if m.nutrients.is_empty() {
                sheet.push(material_prefix(m, labels));
                continue;
            }
            for (kind, nutrient, percentage) in &m.nutrients {
                let mut row = material_prefix(m, labels);
                row.push(Cell::text(labels.nutrient_kind(kind)));
                row.push(Cell::text(nutrient.as_str()));
                row.push(Cell::Number(*percentage));
                sheet.push(row);
            }
        }
        sheet
    }

    /// Section 7.1. The PDF joins each block's three figures into one cell,
    /// the model printing them that way; here every unidad fertilizante is a
    /// number of its own, so applied and recommended can actually be compared.
    /// Anexo III A.3's soil block, one row per bulletin that carried one.
    ///
    /// Its own tab because the PDF joins nine figures into one cell (the model
    /// having no soil page at all) and a joined string can be neither compared
    /// across campaigns nor charted — which is exactly what soil data is for.
    fn sheet_soil(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_soil,
            vec![
                Column::new(labels.s4.date, 14.0),
                Column::new(labels.sheet.plot_ids, 13.0),
                Column::new(labels.sheet.plots, 24.0),
                Column::new(labels.s4.bulletin, 18.0),
                Column::new(labels.sheet.soil_ph, 10.0),
                Column::new(labels.sheet.soil_organic_matter, 20.0),
                Column::new(labels.sheet.soil_p, 20.0),
                Column::new(labels.sheet.soil_k, 20.0),
                Column::new(labels.sheet.soil_n, 14.0),
                Column::new(labels.sheet.soil_conductivity, 20.0),
                Column::new(labels.sheet.soil_sand, 14.0),
                Column::new(labels.sheet.soil_silt, 14.0),
                Column::new(labels.sheet.soil_clay, 14.0),
            ],
        );
        for r in &self.analysis {
            // A residue bulletin has no soil figures; a row of nine blanks
            // would say something it does not.
            if r.soil.is_empty() {
                continue;
            }
            sheet.push(vec![
                Cell::date(Some(&r.date)),
                Cell::text(
                    r.plot_orders
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Cell::text(r.plot_names.as_str()),
                Cell::text(r.bulletin.as_str()),
                Cell::number(r.soil.ph),
                Cell::number(r.soil.organic_matter_pct),
                Cell::number(r.soil.available_p_mg_kg),
                Cell::number(r.soil.available_k_mg_kg),
                Cell::number(r.soil.total_n_pct),
                Cell::number(r.soil.conductivity_ds_m),
                Cell::number(r.soil.sand_pct),
                Cell::number(r.soil.silt_pct),
                Cell::number(r.soil.clay_pct),
            ]);
        }
        sheet
    }

    fn sheet_plan(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_plan,
            vec![
                Column::new(labels.s71.date, 14.0),
                Column::new(labels.sheet.plot_ids, 13.0),
                Column::new(labels.sheet.plots, 24.0),
                Column::new(labels.s71.crop, 22.0),
                Column::new(labels.s71.area, 18.0),
                Column::new(labels.s71.fertiliser, 26.0),
                Column::new(labels.s71.dose, 10.0),
                Column::new(labels.sheet.quantity_unit, 10.0),
                Column::new(labels.sheet.supplied_n, 14.0),
                Column::new(labels.sheet.supplied_p2o5, 16.0),
                Column::new(labels.sheet.supplied_k2o, 14.0),
                Column::new(labels.sheet.accumulated_n, 16.0),
                Column::new(labels.sheet.accumulated_p2o5, 18.0),
                Column::new(labels.sheet.accumulated_k2o, 16.0),
                Column::new(labels.sheet.recommended_n, 16.0),
                Column::new(labels.sheet.recommended_p2o5, 18.0),
                Column::new(labels.sheet.recommended_k2o, 16.0),
            ],
        );
        for r in &self.plan_rows {
            sheet.push(vec![
                Cell::date(Some(&r.date)),
                Cell::text(
                    r.plot_orders
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Cell::text(r.plot_names.as_str()),
                Cell::text(r.crops.as_str()),
                Cell::number(r.area_ha),
                Cell::text(r.material_name.as_str()),
                Cell::Number(r.dose),
                Cell::text(unit_symbol(&r.dose_unit)),
                // Blank, never zero: an unknown contribution is not "none".
                Cell::number(r.supplied.n),
                Cell::number(r.supplied.p2o5),
                Cell::number(r.supplied.k2o),
                Cell::number(r.accumulated.n),
                Cell::number(r.accumulated.p2o5),
                Cell::number(r.accumulated.k2o),
                Cell::number(r.recommended.n),
                Cell::number(r.recommended.p2o5),
                Cell::number(r.recommended.k2o),
            ]);
        }
        sheet
    }

    fn sheet_irrigation(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_irrigation,
            vec![
                Column::new(labels.s8.dates, 20.0),
                Column::new(labels.sheet.plot_ids, 13.0),
                Column::new(labels.sheet.plots, 24.0),
                Column::new(labels.s8.area, 16.0),
                Column::new(labels.s8.method, 22.0),
                Column::new(labels.s8.volume, 14.0),
                Column::new(labels.sheet.quantity_unit, 10.0),
                Column::new(labels.s8.cumulative, 20.0),
                Column::new(labels.sheet.water_nitric_n, 16.0),
                Column::new(labels.sheet.water_soluble_p2o5, 18.0),
                Column::new(labels.s8.source, 26.0),
                Column::new(labels.s31.notes, 30.0),
            ],
        );
        for r in &self.irrigation {
            sheet.push(vec![
                Cell::date(Some(&r.date)),
                Cell::text(
                    r.plot_orders
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Cell::text(r.plot_names.as_str()),
                // Blank, never zero: no plot stating a surface is unknown,
                // and a spreadsheet would add a zero up.
                Cell::number(r.area_ha),
                Cell::text(labels.irrigation_method(&r.method_code)),
                Cell::Number(r.volume),
                Cell::text(unit_symbol(&r.volume_unit)),
                Cell::number(r.cumulative_m3_ha),
                Cell::number(r.water_nitric_n),
                Cell::number(r.water_soluble_p2o5),
                Cell::text(
                    r.origin_codes
                        .iter()
                        .map(|code| labels.water_origin(code))
                        .collect::<Vec<_>>()
                        .join("; "),
                ),
                Cell::text(r.notes.as_str()),
            ]);
        }
        sheet
    }
}

impl Cuaderno {
    /// Model 9.1 as a sheet. Where the PDF joins several plots' references into
    /// one cell, the workbook also carries the table-2.1 order numbers and the
    /// plot names, so a reader can filter by plot the way every other tab does.
    fn sheet_grazing(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_grazing,
            vec![
                Column::new(labels.s9.s91.started_on, 20.0),
                Column::new(labels.s9.s91.ended_on, 20.0),
                Column::new(labels.s9.s91.group_ref, 20.0),
                Column::new(labels.s9.s91.plot_reference, 30.0),
                Column::new(labels.sheet.plot_ids, 13.0),
                Column::new(labels.sheet.plots, 24.0),
                Column::new(labels.s9.s91.species, 22.0),
                Column::new(labels.s9.s91.rega, 18.0),
                Column::new(labels.s9.s91.animal_count, 16.0),
                Column::new(labels.s31.notes, 30.0),
            ],
        );
        for r in &self.grazing {
            sheet.push(vec![
                Cell::date(Some(&r.started_on)),
                // Blank while grazing continues — a spreadsheet must not read
                // an open record as one that ended today.
                Cell::date((!r.ended_on.is_empty()).then_some(r.ended_on.as_str())),
                Cell::text(r.group_ref.as_str()),
                Cell::text(r.plot_reference.as_str()),
                Cell::text(
                    r.plot_orders
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Cell::text(r.plot_names.as_str()),
                Cell::text(r.species.as_str()),
                Cell::text(r.rega.as_str()),
                Cell::Number(r.animal_count as f64),
                Cell::text(r.notes.as_str()),
            ]);
        }
        sheet
    }

    /// The sowing register, which no page of the printed model shows.
    ///
    /// Its dates reach the book through model 9.2's "Siembra" column and model
    /// 9.3's, and `seed_quantity_kg` — captured only because the SIEX twin
    /// requires `Cantidad` — reaches it nowhere at all. So this tab is where
    /// the register can be read whole, the `tab_materials` precedent.
    fn sheet_sowing(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_sowing,
            vec![
                Column::new(labels.sheet.sowing_kind, 12.0),
                Column::new(labels.sheet.date_start, 14.0),
                Column::new(labels.sheet.date_end, 14.0),
                Column::new(labels.s9.s93.flooding, 20.0),
                Column::new(labels.sheet.plot_ids, 13.0),
                Column::new(labels.sheet.plots, 24.0),
                Column::new(labels.s31.species, 22.0),
                Column::new(labels.sheet.seed_quantity, 18.0),
                Column::new(labels.s31.notes, 30.0),
            ],
        );
        for r in &self.sowings {
            sheet.push(vec![
                Cell::text(r.kind.as_str()),
                Cell::date(Some(&r.sown_on)),
                Cell::date((!r.sowing_end_date.is_empty()).then_some(r.sowing_end_date.as_str())),
                // Blank while the field is dry — and for every crop that is
                // never flooded, which is most of them.
                Cell::date((!r.flooded_on.is_empty()).then_some(r.flooded_on.as_str())),
                Cell::text(
                    r.plot_orders
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Cell::text(r.plot_names.as_str()),
                Cell::text(r.crops.as_str()),
                match r.seed_quantity_kg {
                    Some(kg) => Cell::Number(kg),
                    None => Cell::text(""),
                },
                Cell::text(r.notes.as_str()),
            ]);
        }
        sheet
    }

    /// Sections 9.2 and "9.6" as one tab, unpivoted: one row per operation,
    /// with the duty it evidences in a column of its own.
    ///
    /// The PDF answers "which duty" by which page a row is printed on, and
    /// pivots 9.2 onto the plot because that is what the model's row is. Here
    /// both choices reverse — a spreadsheet exists to be filtered and sorted,
    /// and a pivoted cell holding two dates can be read but not sorted, which
    /// is the whole point of the second renderer.
    ///
    /// It carries rows the printed pages do not: the cover and flooded-crop
    /// practices are captured before seams 3 and 4 give them a page, and
    /// `residue_destination` is the twin's field, which no page shows at all.
    /// Nothing recorded is invisible in this tab.
    fn sheet_cultural_operations(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_cultural_operations,
            vec![
                Column::new(labels.s9.s96.performed_on, 14.0),
                Column::new(labels.s9.s96.performed_end_date, 14.0),
                Column::new(labels.sheet.eco_practice, 34.0),
                Column::new(labels.s9.s96.activity, 24.0),
                Column::new(labels.sheet.plot_ids, 13.0),
                Column::new(labels.sheet.plots, 24.0),
                Column::new(labels.s9.s92.maintenance, 30.0),
                Column::new(labels.sheet.residue_destination, 34.0),
                Column::new(labels.s31.notes, 30.0),
            ],
        );
        for r in &self.operations {
            sheet.push(vec![
                Cell::date(Some(&r.performed_on)),
                // Blank for a single day's work: a spreadsheet must not read a
                // one-day operation as an interval that ended the same day.
                Cell::date(
                    (!r.performed_end_date.is_empty()).then_some(r.performed_end_date.as_str()),
                ),
                Cell::text(r.practice.as_str()),
                Cell::text(r.kind.as_str()),
                Cell::text(
                    r.plot_orders
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Cell::text(r.plot_names.as_str()),
                Cell::text(r.activity_description.as_str()),
                Cell::text(r.residue_destination.as_str()),
                Cell::text(r.notes.as_str()),
            ]);
        }
        sheet
    }

    /// Models 9.4 and 9.5 as one tab, the practice telling the two apart.
    ///
    /// One tab rather than two because it is one register, and because the
    /// columns are identical — 9.5 simply leaves the maintenance cell empty. It
    /// also carries three things neither printed page has a column for: the
    /// practice, the kind of cover (`TIPO_COBERTURA_SUELO`, which the twin
    /// sends) and the date the widths were stated, which is what separates
    /// "measured in June" from "never measured".
    fn sheet_covers(&self, labels: &Labels) -> Sheet {
        let mut sheet = Sheet::new(
            labels.sheet.tab_covers,
            vec![
                Column::new(labels.s9.s94.established_on, 18.0),
                Column::new(labels.sheet.eco_practice, 34.0),
                Column::new(labels.s9.s94.plot_ids, 13.0),
                Column::new(labels.sheet.plots, 24.0),
                Column::new(labels.sheet.cover_type, 34.0),
                Column::new(labels.s9.s94.width, 20.0),
                Column::new(labels.s9.s94.free_canopy_width, 24.0),
                Column::new(labels.sheet.widths_stated_on, 18.0),
                Column::new(labels.s9.s94.maintenance, 40.0),
                Column::new(labels.s31.notes, 30.0),
            ],
        );
        for r in &self.covers {
            sheet.push(vec![
                Cell::date(Some(&r.established_on)),
                Cell::text(r.practice.as_str()),
                Cell::text(
                    r.plot_orders
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                Cell::text(r.plot_names.as_str()),
                Cell::text(r.cover_type.as_str()),
                // Blank rather than 0 when art. 42.1.e's annotation has not
                // been made: a spreadsheet sums its columns, and a zero width
                // averaged in would understate every cover on the holding.
                Cell::number(r.width_m),
                Cell::number(r.free_canopy_width_m),
                Cell::date((!r.widths_stated_on.is_empty()).then_some(r.widths_stated_on.as_str())),
                Cell::text(r.maintenance.as_str()),
                Cell::text(r.notes.as_str()),
            ]);
        }
        sheet
    }
}

// ---------------------------------------------------------------------------
// Display formatting
//
// Shared by every language the book prints in: dd/mm/yyyy dates and the
// decimal comma are the convention in Castilian and Catalan alike. Give these
// a `Labels` argument the day a language needs different ones.
// ---------------------------------------------------------------------------

/// ISO date → dd/mm/yyyy; anything unparseable passes through verbatim (a
/// printout must never lose data over a malformed historical value).
fn format_date(iso: &str) -> String {
    siex::date_to_siex(iso).unwrap_or_else(|| iso.to_string())
}

/// The model's 3.1 "intervalo de fechas" column: one date, or two joined by an
/// en dash. An interval that ends on its start day is the same statement as a
/// single day and prints as one, so the book never says "01/05 – 01/05".
fn format_date_interval(start: &str, end: Option<&str>) -> String {
    match end {
        Some(end) if end != start => format!("{} – {}", format_date(start), format_date(end)),
        _ => format_date(start),
    }
}

/// One model-9.2 cell: every date recorded for that activity on that plot,
/// joined — footnote (1) allows two cuts a year, and the column is a list by
/// design. The activity's name rides along only in the "otras actividades"
/// column, whose footnote (4) asks for the date **and** what was done.
///
/// Separated by the same middle dot the reference cells use, so a reader who
/// has met one join in this book has met them all.
fn format_activities(entries: &[DatedActivity]) -> String {
    entries
        .iter()
        .map(|entry| {
            let dates = format_date_interval(
                &entry.performed_on,
                (!entry.performed_end_date.is_empty()).then_some(entry.performed_end_date.as_str()),
            );
            if entry.label.is_empty() {
                dates
            } else {
                format!("{dates} {}", entry.label)
            }
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Decimal-comma number, up to 4 decimals, trailing zeros trimmed
/// ("1,5", "2", "0,0375").
///
/// Four decimals is an APP convention, not a regulatory one — no decree states
/// a precision. It is simply enough for every register whose units already
/// scale (a dose is written in g/ha rather than kg/ha), and the frontend holds
/// itself to the same precision, so screen and book never show a different
/// FIGURE. The separator may differ: this book prints in the holding's language
/// and the app renders in the reader's, so an English reader sees "1234.5"
/// against this "1234,5". Same digits, each in its own convention.
///
/// What it may never do is print a nonzero measurement as "0": a farmer who
/// recorded 0,00003 wrote a figure, and a zero is a statement they never made
/// (the rule `amount` follows for blanks). So a value that would round away
/// widens until its first significant digit shows, rather than being rounded
/// into a falsehood. Coordinates are the other exception and have their own
/// formatter at five decimals.
fn format_number(value: f64) -> String {
    let trimmed = |text: String| text.trim_end_matches('0').trim_end_matches('.').to_string();
    let mut out = trimmed(format!("{value:.4}"));
    if value != 0.0 && out.trim_start_matches('-') == "0" {
        // 12 is where f64 stops being trustworthy for the values this book
        // carries; a figure needing more than that is noise, not a measurement.
        for precision in 5..=12 {
            out = trimmed(format!("{:.*}", precision, value));
            if out.trim_start_matches('-') != "0" {
                break;
            }
        }
    }
    out.replace('.', ",")
}

/// A voluntary lat/lon pair for section 2.2, WGS84 decimal degrees.
///
/// Five decimals is about a metre, which is what locating a wellhead needs and
/// what `format_number`'s four would not give. Separated by " / " rather than a
/// comma because the numbers themselves carry a decimal comma in both printed
/// languages — "41,65234, -4,72891" reads as four numbers.
fn format_coordinates((latitude, longitude): (f64, f64)) -> String {
    let one = |value: f64| format!("{value:.5}").replace('.', ",");
    format!("{} / {}", one(latitude), one(longitude))
}

/// A measured amount with its unit ("120 t", "3 kg"), or blank when unstated —
/// the official form leaves the cell to be filled by hand, and a zero would be
/// a statement the farmer never made.
fn amount(value: Option<f64>, unit_code: Option<&str>) -> String {
    match (value, unit_code) {
        (Some(value), Some(code)) => format!("{} {}", format_number(value), unit_symbol(code)),
        _ => String::new(),
    }
}

/// `unit_symbol` over an optional code, for cells that may carry neither.
fn unit_symbol_opt(code: Option<&str>) -> &'static str {
    code.map(unit_symbol).unwrap_or_default()
}

/// The printable symbol for a unit code, exposed so a test can assert that
/// EVERY seeded unit has one. The fallback below is silent by necessity (a
/// book must render whatever it finds), which makes an unmapped unit invisible
/// until someone reads the PDF — so the guard belongs in the suite.
pub fn unit_display_symbol(code: &str) -> &'static str {
    unit_symbol(code)
}

/// Dose-unit display symbol — a symbol, not prose: `L/ha` reads the same in
/// every language (the same closed list the UI's `unit.*` i18n keys cover).
fn unit_symbol(code: &str) -> &'static str {
    match code {
        "l_ha" => "L/ha",
        "kg_ha" => "kg/ha",
        "ml_ha" => "ml/ha",
        "g_ha" => "g/ha",
        "ml_hl" => "ml/hl",
        "g_hl" => "g/hl",
        "g_l" => "g/L",
        "ml_l" => "ml/L",
        "pct" => "%",
        // Fertiliser and irrigation rates (Anexo III C.j and C.l). Missing
        // arms here print a bare number in a legal document, which is why the
        // fallback below is the one thing in this function worth distrusting.
        "m3_ha" => "m³/ha",
        "t_ha" => "t/ha",
        // Amounts (Anexo III B.i's total, and the non-field registers' subject).
        "kg" => "kg",
        "l" => "L",
        "t" => "t",
        "m3" => "m³",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// A book's worth of catalogue reads: what the cache did, from the outside.
    fn counts(catalogues: &CatalogueCache) -> (usize, usize) {
        let memo = catalogues.memo.borrow();
        (memo.lookups, memo.queries)
    }

    #[test]
    fn a_catalogue_code_is_read_once_however_many_rows_ask_for_it() {
        // The register shape this exists for: one measure code down a column of
        // treatments, one término municipal down a column of plots. The reads
        // must follow the vocabulary, not the row count.
        let mut conn = open_in_memory().unwrap();
        terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
        let catalogues = CatalogueCache::default();

        for _ in 0..50 {
            assert_eq!(
                catalogue_label(&conn, &catalogues, Some("PROVINCIA"), "47", None),
                "Valladolid"
            );
        }
        assert_eq!(counts(&catalogues), (50, 1));

        // A second code is a second read, and only one.
        for _ in 0..10 {
            catalogue_label(&conn, &catalogues, Some("PROVINCIA"), "08", None);
        }
        assert_eq!(counts(&catalogues), (60, 2));
    }

    #[test]
    fn a_code_that_resolves_to_nothing_is_not_asked_for_twice() {
        // The vendored snapshot rides app releases, so a book CAN be full of
        // codes this installation cannot resolve — they print themselves. That
        // must cost one read, not one per row.
        let conn = open_in_memory().unwrap();
        let catalogues = CatalogueCache::default();
        for _ in 0..20 {
            // No catalogues imported at all: the harshest version of the case.
            assert_eq!(
                catalogue_label(&conn, &catalogues, Some("PROVINCIA"), "47", None),
                "47"
            );
        }
        assert_eq!(counts(&catalogues), (20, 1));
    }

    /// The demo campaign, with the season and farm its treatments belong to.
    fn demo_book() -> (Connection, String, String) {
        let mut conn = open_in_memory().unwrap();
        terrazgo_core::catalogue::ensure_catalogues(&mut conn).unwrap();
        assert!(module_cue::demo::seed_demo(&mut conn).unwrap().seeded);
        let (season_id, farm_id) = conn
            .query_row(
                "SELECT season_id, farm_id FROM treatment_record LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        (conn, season_id, farm_id)
    }

    /// Every treatment again under a fresh id, junctions and all: more rows
    /// printing the same vocabulary, which is the growth this book has to
    /// survive. Raw SQL because the point is to multiply rows, not to exercise
    /// the repositories.
    fn duplicate_treatments(conn: &Connection, copies: usize) {
        for n in 0..copies {
            let s = format!("-copy{n}");
            conn.execute_batch(&format!(
                "CREATE TEMP TABLE c_rec AS
                     SELECT * FROM treatment_record WHERE id NOT LIKE '%-copy%';
                 UPDATE c_rec SET id = id || '{s}';
                 INSERT INTO treatment_record SELECT * FROM c_rec;
                 CREATE TEMP TABLE c_plot AS
                     SELECT * FROM treatment_plot WHERE treatment_record_id NOT LIKE '%-copy%';
                 UPDATE c_plot SET id = id || '{s}', treatment_record_id = treatment_record_id || '{s}';
                 INSERT INTO treatment_plot SELECT * FROM c_plot;
                 CREATE TEMP TABLE c_prob AS
                     SELECT * FROM treatment_problem WHERE treatment_record_id NOT LIKE '%-copy%';
                 UPDATE c_prob SET id = id || '{s}', treatment_record_id = treatment_record_id || '{s}';
                 INSERT INTO treatment_problem SELECT * FROM c_prob;
                 DROP TABLE c_rec; DROP TABLE c_plot; DROP TABLE c_prob;"
            ))
            .unwrap();
        }
    }

    #[test]
    fn the_reads_a_book_makes_do_not_grow_with_the_rows_it_prints() {
        // The invariant the hoist exists for, and the only honest way to state
        // it: assemble the same book twice, the second time with four times the
        // treatments, and watch the asks multiply while the reads stand still.
        // Before the hoist the two numbers were the same one.
        let (conn, season_id, farm_id) = demo_book();

        let small = CatalogueCache::default();
        assemble(
            &conn,
            &small,
            &season_id,
            &farm_id,
            "2026-08-13",
            ReportLanguage::Es,
        )
        .unwrap();
        let (asked_small, read_small) = counts(&small);

        duplicate_treatments(&conn, 3);
        let large = CatalogueCache::default();
        assemble(
            &conn,
            &large,
            &season_id,
            &farm_id,
            "2026-08-13",
            ReportLanguage::Es,
        )
        .unwrap();
        let (asked_large, read_large) = counts(&large);

        assert!(
            asked_large > asked_small,
            "the copies must actually reach the register: {asked_small} asks either way"
        );
        assert_eq!(
            read_large, read_small,
            "four times the rows, same vocabulary: {read_large} reads against {read_small}"
        );
        assert!(
            read_small < asked_small,
            "even the demo book asks more often than it reads"
        );
    }

    /// These vectors are MIRRORED by `formatNumber` in `src/i18n.js`, which
    /// renders the same figures on screen under
    /// `{ maximumFractionDigits: 4, useGrouping: false }` — the standard
    /// `collate.js` holds against `collate.rs`, and for the same reason: a
    /// farmer reads one figure here and the same figure there. Change a case
    /// in either place and change it in both.
    ///
    /// The two four-digit values are the ones that would drift first: they are
    /// where a thousands separator would appear if either side grew one.
    #[test]
    fn numbers_render_with_decimal_comma_and_no_trailing_zeros() {
        assert_eq!(format_number(1.5), "1,5");
        assert_eq!(format_number(2.0), "2");
        assert_eq!(format_number(0.0375), "0,0375");
        assert_eq!(format_number(12.25), "12,25");
        assert_eq!(format_number(1234.5), "1234,5");
        assert_eq!(format_number(12000.0), "12000");
    }

    /// A measurement smaller than the fourth decimal must still print as
    /// itself. Rounding it to "0" would put a figure in the book that the
    /// farmer never wrote — the same falsehood `amount` avoids by leaving a
    /// blank when a value is unstated.
    #[test]
    fn a_nonzero_measurement_never_prints_as_zero() {
        assert_eq!(format_number(0.00003), "0,00003");
        assert_eq!(format_number(-0.00003), "-0,00003");
        assert_eq!(format_number(0.0000001), "0,0000001");
        // Zero itself is still zero.
        assert_eq!(format_number(0.0), "0");
        // And the ordinary path is untouched.
        assert_eq!(format_number(1.23456), "1,2346");
    }

    #[test]
    fn the_models_siglas_are_codes_and_print_the_same_in_every_language() {
        // GIP siglas per the model's 1.4 footnote: AE, PI, CP, Atrias, AS, NO.
        assert_eq!(gip_abbrev(Some("organic")), "AE");
        assert_eq!(gip_abbrev(Some("integrated_production")), "PI");
        assert_eq!(gip_abbrev(Some("private_certification")), "CP");
        assert_eq!(gip_abbrev(Some("atria")), "Atrias");
        assert_eq!(gip_abbrev(Some("advisor_assisted")), "AS");
        assert_eq!(gip_abbrev(Some("not_required")), "NO");
        assert_eq!(gip_abbrev(None), "");
        // 2.1 footnotes 3 and 4.
        assert_eq!(irrigation_abbrev(Some("rainfed")), "SEC");
        assert_eq!(irrigation_abbrev(Some("drip")), "LOC");
        assert_eq!(environment_abbrev(Some("greenhouse")), "INV");
        assert_eq!(environment_abbrev(None), "");
    }

    #[test]
    fn crop_gip_prefers_the_stated_framework_over_the_production_system() {
        // Stated wins, even against a production system implying another sigla.
        assert_eq!(crop_gip_abbrev(Some("atria"), Some("organic")), "Atrias");
        // Unstated falls back to what the production system already implies.
        assert_eq!(crop_gip_abbrev(None, Some("organic")), "AE");
        assert_eq!(crop_gip_abbrev(None, Some("integrated")), "PI");
        // Conventional implies nothing: blank, never "NO" — "sin obligación de
        // asesor" is a declaration the farmer makes, not a default.
        assert_eq!(crop_gip_abbrev(None, Some("conventional")), "");
        assert_eq!(crop_gip_abbrev(None, None), "");
    }

    /// The 2.2 summary is assembled as values and worded at render time, so the
    /// same finding reads correctly in either language.
    #[test]
    fn the_zone_summary_words_the_same_finding_in_each_language() {
        let checked_negative = ZoneCheckSummary {
            campaign: 2026,
            affecting: vec![],
        };
        assert_eq!(
            checked_negative.render(
                ReportLanguage::Es.labels(),
                &NameCollator::new(ReportLanguage::Es)
            ),
            "Sin afección — campaña 2026"
        );
        assert_eq!(
            checked_negative.render(
                ReportLanguage::Ca.labels(),
                &NameCollator::new(ReportLanguage::Ca)
            ),
            "Sense afectació — campanya 2026"
        );

        let affected = ZoneCheckSummary {
            campaign: 2026,
            affecting: vec![
                ("natura_2000".into(), None),
                ("nitrate_vulnerable".into(), Some(50.0)),
            ],
        };
        // Sorted by the PRINTED label, so each language reads alphabetically.
        assert_eq!(
            affected.render(
                ReportLanguage::Es.labels(),
                &NameCollator::new(ReportLanguage::Es)
            ),
            "Red Natura 2000; Vulnerable a nitratos (50 %) — campaña 2026"
        );
        assert_eq!(
            affected.render(
                ReportLanguage::Ca.labels(),
                &NameCollator::new(ReportLanguage::Ca)
            ),
            "Vulnerable als nitrats (50 %); Xarxa Natura 2000 — campanya 2026"
        );
    }

    #[test]
    fn tax_ids_match_across_the_separators_people_type() {
        // Same NIF, three ways of writing it.
        assert_eq!(normalise_tax_id("12345678z"), "12345678Z");
        assert_eq!(normalise_tax_id(" 12.345.678-Z "), "12345678Z");
        assert_eq!(normalise_tax_id(""), "");
    }

    #[test]
    fn every_seeded_dose_unit_has_a_display_symbol() {
        for code in [
            "l_ha", "kg_ha", "ml_ha", "g_ha", "ml_hl", "g_hl", "g_l", "ml_l", "pct",
        ] {
            assert!(!unit_symbol(code).is_empty(), "unit '{code}' prints blank");
        }
    }
}
