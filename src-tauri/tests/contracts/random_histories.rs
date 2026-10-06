// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Random histories over the whole schema, each ending where a device that
//! replays the whole log from nothing ends (docs/sync.md → How it is tested).
//!
//! `terrazgo-core/tests/random_histories.rs` races sowings, the one register of
//! core's that holds records in a book; core may never name a module's table,
//! so it can see nothing else. Here, in the shell, which sees every crate, a
//! laptop and two phones start from the demo campaign and race what only the
//! whole schema has:
//!
//!   * **every module's records**, recorded, corrected and removed — recorded
//!     from a small pool of days and plots, so two devices recording one
//!     operation write a pair the duplicate rules raise;
//!   * **change sets that write two registers at once** — a soil cover with
//!     its mowing and its grazing, a seed treatment withdrawing its book's "no
//!     treated seed" declaration, a duplicate kept with the copy's removal —
//!     and records pointing into other registers: a fertilisation at its
//!     irrigation, a grazing at its cover, a seed treatment at its sowing, a
//!     plan at its crops;
//!   * **slot-keyed registers**: that declaration, which a book merge moves
//!     from one book's slot to the other's, and a plot's zone check;
//!   * **acts on derived state**: duplicate verdicts — distinct, keep one,
//!     bring back one of a pair removed twice over — and an alert seen;
//!   * the book acts, resolutions, purges and files core's sweep makes, with
//!     the duplicate policies the app passes them (core's sweep passes none).
//!
//! After a full exchange, every device must hold the same tables, review queue
//! and records in a removed book as a fresh device joining from one of them,
//! and the same log. On the way, three things are findings: a refused file, a
//! resolution or purge that fails, and any act failing on a raw database error
//! rather than a refusal with a name — what a person would read as "error".
//!
//! **The seeds are fixed; the clock is not**, as in core's sweep: which version
//! goes live depends on each write's stamp, so one seed can take a different
//! path on another run. The property holds for every path, so a failure is a
//! defect whichever run finds it; its message carries the seed and every act.
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fmt::Debug;
use std::ops::RangeInclusive;
use std::path::Path;

use module_ecoscheme::models::*;
use module_ecoscheme::repository as eco;
use module_fertilisation::models::*;
use module_fertilisation::repository as fert;
use module_phytosanitary::models::*;
use module_phytosanitary::repository as phyto;
use rusqlite::{Connection, OptionalExtension, Params};
use terrazgo_core::bundle;
use terrazgo_core::date::{add_days, now_ms, today_utc};
use terrazgo_core::duplicates::{DuplicatePolicy, Scope};
use terrazgo_core::merge::{RowCaption, heads, resolve};
use terrazgo_core::models::{NewSeason, NewZoneFlag};
use terrazgo_core::repository as repo;
use terrazgo_core::sync::VersionVector;
use terrazgo_lib::duplicates::{DUPLICATE_REGISTERS, register_by_table};

/// The laptop, two phones, and the device that joins at the end.
const DEVICES: [&str; 4] = [
    "0192f3a4-0000-7000-8000-00000000000a",
    "0192f3a4-0000-7000-8000-00000000000b",
    "0192f3a4-0000-7000-8000-00000000000c",
    "0192f3a4-0000-7000-8000-00000000000d",
];

/// Acts per history in the sweep the suite runs.
const STEPS: usize = 50;

/// The days new records fall on: two, a day apart, so two devices recording
/// one operation often write two records the rules compare.
const DAYS: [&str; 2] = ["2026-05-04", "2026-05-05"];

/// What a correction leaves in a record's notes.
const NOTES: [Option<&str>; 3] = [Some("a"), Some("b"), None];

/// What an error says when no refusal with a name answered for it — the
/// database, the log or the aggregate map refusing instead. `Debug` names the
/// variant; a chain of `anyhow` causes prints each cause's `Debug` too.
const RAW: [&str; 4] = ["Sqlite(", "Json(", "ShapeViolation(", "Stamp("];

/// A small random-number generator (xorshift64), so a seed names one sequence
/// of choices on every machine with no dependency for it.
struct Choices(u64);

impl Choices {
    fn from_seed(seed: u64) -> Self {
        // Spread small seeds across the state; xorshift may never hold 0.
        Choices(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn below(&mut self, bound: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        usize::try_from(self.0 % bound as u64).unwrap()
    }

    /// One of three plots at least, as a bit mask.
    fn plot_mask(&mut self) -> usize {
        1 + self.below(7)
    }
}

/// One simulated device: a database opened the way the app opens one — the
/// composed schema, the whole aggregate map, its own `sync_peer` row — kept
/// in memory.
struct Device {
    conn: Connection,
    id: &'static str,
}

impl Device {
    fn new(id: &'static str) -> Self {
        let conn = terrazgo_lib::db::open_app_db(Path::new(":memory:"), id).unwrap();
        Device { conn, id }
    }

    /// A file of everything `seen` lacks.
    fn file(&self, seen: &VersionVector) -> Vec<u8> {
        let mut bytes = Vec::new();
        bundle::write_bundle(&self.conn, self.id, seen, &mut bytes).unwrap();
        bytes
    }

    fn apply(&mut self, bytes: &[u8]) -> Result<(), String> {
        let parsed = bundle::read_bundle(bytes).map_err(debug)?;
        bundle::apply_bundle(&mut self.conn, &parsed, now_ms())
            .map(|_| ())
            .map_err(debug)
    }

    /// Every synced table, row by row, sorted — devices insert in different
    /// orders. `sync_peer` is left out: the joining device has written its own
    /// row, which the others have not heard of.
    fn tables(&self) -> Vec<(String, Vec<String>)> {
        let names = ids(
            &self.conn,
            "SELECT table_name FROM temp.sync_shape
             WHERE role <> 'local' AND table_name <> 'sync_peer' ORDER BY table_name",
            [],
        );
        names
            .into_iter()
            .map(|table| {
                let mut stmt = self
                    .conn
                    .prepare(&format!("SELECT * FROM {table}"))
                    .unwrap();
                let width = stmt.column_count();
                let mut rows: Vec<String> = stmt
                    .query_map([], |row| {
                        Ok((0..width)
                            .map(|index| format!("{:?}", row.get_ref(index).unwrap()))
                            .collect::<Vec<_>>()
                            .join("|"))
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<_>>()
                    .unwrap();
                rows.sort();
                (table, rows)
            })
            .collect()
    }

    /// The review queue, as each device derives it from its log.
    fn queue(&self) -> Vec<String> {
        ids(
            &self.conn,
            "SELECT root_table || '/' || root_id || ' ' || live_device || '/' || live_seq
                    || ' ' || other_device || '/' || other_seq
             FROM sync_conflict ORDER BY 1",
            [],
        )
    }

    /// The records in a removed book.
    fn strays(&self, captions: &[RowCaption]) -> Vec<String> {
        let mut ids: Vec<String> = repo::list_stray_records(&self.conn, captions)
            .unwrap()
            .into_iter()
            .flat_map(|book| book.records.into_iter().map(|record| record.id))
            .collect();
        ids.sort();
        ids
    }

    /// The log, as a list of change sets and the rows each names.
    fn log(&self) -> Vec<String> {
        ids(
            &self.conn,
            "SELECT origin_device || '/' || origin_seq || ' ' || entity_table || ' ' || entity_id
             FROM record_change ORDER BY 1",
            [],
        )
    }

    /// What every device must agree on.
    fn picture(&self, captions: &[RowCaption]) -> Picture {
        Picture {
            tables: self.tables(),
            queue: self.queue(),
            strays: self.strays(captions),
        }
    }
}

/// What a device holds that another must hold too.
#[derive(Debug, PartialEq, Eq)]
struct Picture {
    tables: Vec<(String, Vec<String>)>,
    queue: Vec<String>,
    strays: Vec<String>,
}

/// Two of the devices at once, one to read and one to write.
fn pair(devices: &mut [Device], from: usize, to: usize) -> (&Device, &mut Device) {
    if from < to {
        let (left, right) = devices.split_at_mut(to);
        (&left[from], &mut right[0])
    } else {
        let (left, right) = devices.split_at_mut(from);
        (&right[0], &mut left[to])
    }
}

/// The first column of every row `sql` returns, as text.
fn ids<P: Params>(conn: &Connection, sql: &str, params: P) -> Vec<String> {
    conn.prepare(sql)
        .unwrap()
        .query_map(params, |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

fn debug<E: Debug>(err: E) -> String {
    format!("{err:?}")
}

/// An `anyhow` error with every cause in its chain, each by its `Debug` — the
/// variant a cause is, which the error's own message does not say.
fn chain(err: anyhow::Error) -> String {
    err.chain()
        .map(|cause| format!("{cause:?}"))
        .collect::<Vec<_>>()
        .join(" ← ")
}

// ---------------------------------------------------------------------------
// The land, and the campaign every history starts from
// ---------------------------------------------------------------------------

/// What every act writes on: the demo's farm, three of its plots, its book
/// and a twin — the same campaign opened on another date, as two devices do —
/// and the product, operator and fertiliser a new record names.
struct Land {
    farm: String,
    plots: [String; 3],
    books: [String; 2],
    product: String,
    operator: String,
    material: String,
}

impl Land {
    /// The plots a bit mask names.
    fn plots(&self, mask: usize) -> Vec<String> {
        (0..3)
            .filter(|bit| mask & (1 << bit) != 0)
            .map(|bit| self.plots[bit].clone())
            .collect()
    }
}

struct Start {
    devices: Vec<Device>,
    group: String,
    land: Land,
}

/// The demo campaign on the laptop, a twin of its book, one of each register
/// the demo does not write — linked as a farmer links them — carried to both
/// phones.
fn start() -> Start {
    let mut devices = vec![
        Device::new(DEVICES[0]),
        Device::new(DEVICES[1]),
        Device::new(DEVICES[2]),
    ];
    let group = terrazgo_core::sync::ensure_sync_group(&devices[0].conn).unwrap();
    for phone in &devices[1..] {
        terrazgo_core::sync::join_sync_group(&phone.conn, &group).unwrap();
    }
    let laptop = &mut devices[0].conn;
    let seeded = module_phytosanitary::demo::seed_demo(laptop).unwrap();
    assert!(seeded.seeded, "the demo seeded nothing");

    let farm = ids(laptop, "SELECT id FROM farm ORDER BY id", []).remove(0);
    let book = ids(laptop, "SELECT id FROM season ORDER BY id", []).remove(0);
    let plots: [String; 3] = ids(
        laptop,
        "SELECT id FROM plot WHERE farm_id = ?1 AND deleted_at IS NULL ORDER BY id LIMIT 3",
        [&farm],
    )
    .try_into()
    .unwrap();
    let (product, operator): (String, String) = laptop
        .query_row(
            "SELECT product_id, operator_id FROM treatment_record
             WHERE product_id IS NOT NULL ORDER BY id LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let twin = repo::insert_season(
        laptop,
        NewSeason {
            farm_id: farm.clone(),
            starts_on: "2025-10-01".into(),
            ends_on: "2026-08-31".into(),
            custom_label: Some("2025/2026 bis".into()),
        },
        None,
    )
    .unwrap()
    .id;
    let material = fert::insert_fertiliser_material(
        laptop,
        NewFertiliserMaterial {
            name: "Purín de porcino".into(),
            material_code: "5".into(),
            material_detail_code: None,
            supplier_name: None,
            supplier_rega: None,
            supplier_tax_id: None,
            supplier_nima: None,
            manure_treatment_code: None,
            density_kg_l: None,
            notes: None,
            nutrients: Vec::new(),
        },
        None,
    )
    .unwrap()
    .material
    .id;
    let land = Land {
        farm,
        plots,
        books: [book.clone(), twin.clone()],
        product,
        operator,
        material,
    };

    // In the demo's book: an irrigation and the fertilisation applied with
    // it, a plan for its crops, and a cover whose grazing is a record of its
    // own and named by another grazing.
    let crop = laptop
        .query_row(
            "SELECT id FROM crop WHERE season_id = ?1 AND plot_id = ?2",
            [&book, &land.plots[0]],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .unwrap();
    let irrigation = fert::insert_irrigation_record(
        laptop,
        new_irrigation(&land, &book, DAYS[0], &land.plots(0b001), crop),
        None,
    )
    .unwrap()
    .record
    .id;
    fert::insert_fertilisation_record(
        laptop,
        new_fertilisation(&land, &book, DAYS[0], &land.plots(0b001), Some(irrigation)),
        None,
    )
    .unwrap();
    let crops = ids(
        laptop,
        "SELECT id FROM crop WHERE season_id = ?1 ORDER BY id LIMIT 2",
        [&book],
    );
    fert::insert_fertilisation_plan(
        laptop,
        NewFertilisationPlan {
            season_id: book.clone(),
            farm_id: land.farm.clone(),
            needs_n_kg_ha: 120.0,
            needs_p2o5_kg_ha: 40.0,
            needs_k2o_kg_ha: 30.0,
            expected_yield_kg_ha: 5000.0,
            preceding_crop_code: None,
            drawn_up_on: "2026-01-10".into(),
            tool_generated: false,
            notes: None,
            crop_ids: crops,
        },
        None,
    )
    .unwrap();
    let cover = eco::insert_soil_cover(
        laptop,
        new_cover(&land, &book, DAYS[0], &land.plots(0b010)),
        None,
    )
    .unwrap()
    .record
    .id;
    eco::insert_grazing_record(
        laptop,
        new_grazing(&land, &book, DAYS[1], &land.plots(0b010), Some(cover)),
        None,
    )
    .unwrap();

    // In the twin: an irrigation, and the declaration that it holds no
    // treated seed — which a seed treatment recorded into it withdraws.
    fert::insert_irrigation_record(
        laptop,
        new_irrigation(&land, &twin, DAYS[1], &land.plots(0b100), None),
        None,
    )
    .unwrap();
    phyto::set_register_declaration(
        laptop,
        &land.farm,
        &twin,
        "seed_treatment",
        "2025-10-01",
        None,
    )
    .unwrap();

    let whole = devices[0].file(&VersionVector::default());
    for phone in &mut devices[1..] {
        phone.apply(&whole).unwrap();
    }
    Start {
        devices,
        group,
        land,
    }
}

// ---------------------------------------------------------------------------
// New records — the same few operations, so two devices can record one twice
// ---------------------------------------------------------------------------

fn new_treatment(land: &Land, book: &str, day: &str) -> NewTreatmentRecord {
    NewTreatmentRecord {
        season_id: book.into(),
        farm_id: land.farm.clone(),
        application_date: day.into(),
        application_end_date: None,
        application_time: None,
        drying_date: None,
        product_id: Some(land.product.clone()),
        country_code: None,
        dose_value: Some(1.0),
        dose_unit_code: Some("l_ha".into()),
        total_quantity_value: None,
        total_quantity_unit_code: None,
        target_organism: None,
        // SIEX ENFERMEDADES 254, Septoriosis — one the demo records too.
        problems: vec![NewTreatmentProblem {
            reason_category_code: "disease".into(),
            problem_code: "254".into(),
        }],
        justifications: vec!["monitoring".into()],
        efficacy_code: None,
        operator_id: land.operator.clone(),
        machinery_id: None,
        advisor_id: None,
        measure_code: None,
        measure_intensity_value: None,
        measure_intensity_unit_code: None,
        measure_registration_number: None,
        measure_basic_substance_code: None,
        phi_days_used: None,
        notes: None,
    }
}

fn new_irrigation(
    land: &Land,
    book: &str,
    day: &str,
    plots: &[String],
    crop: Option<String>,
) -> NewIrrigationRecord {
    NewIrrigationRecord {
        season_id: book.into(),
        farm_id: land.farm.clone(),
        irrigated_on: day.into(),
        irrigation_end_date: None,
        irrigation_method_code: "drip".into(),
        volume_value: 300.0,
        volume_unit_code: "m3_ha".into(),
        water_nitric_n_mg_l: None,
        water_soluble_p2o5_mg_l: None,
        energy_type_code: None,
        meter_number: None,
        notes: None,
        plots: plots
            .iter()
            .map(|plot_id| NewIrrigationPlot {
                plot_id: plot_id.clone(),
                crop_id: crop.clone(),
                irrigated_area_ha: None,
            })
            .collect(),
        water_origins: vec!["groundwater".into()],
        practices: vec![],
    }
}

fn new_fertilisation(
    land: &Land,
    book: &str,
    day: &str,
    plots: &[String],
    irrigation: Option<String>,
) -> NewFertilisationRecord {
    NewFertilisationRecord {
        season_id: book.into(),
        farm_id: land.farm.clone(),
        applied_on: day.into(),
        application_end_date: None,
        fertilisation_type_code: "top_dressing".into(),
        // Applied with an irrigation means through it.
        application_method_code: if irrigation.is_some() {
            "fertigation_localised"
        } else {
            "broadcast"
        }
        .into(),
        dose_value: 250.0,
        dose_unit_code: "kg_ha".into(),
        fertiliser_material_id: land.material.clone(),
        sludge_application: false,
        sustainable_input_management: false,
        machinery_id: None,
        irrigation_record_id: irrigation,
        service_company: None,
        service_regfer_number: None,
        delivery_note_ref: None,
        yield_estimated_kg_ha: None,
        yield_final_kg_ha: None,
        notes: None,
        plots: plots
            .iter()
            .map(|plot_id| NewFertilisationPlot {
                plot_id: plot_id.clone(),
                crop_id: None,
                fertilised_area_ha: None,
            })
            .collect(),
        practices: Vec::new(),
    }
}

/// A head of livestock grazing, as a grazing record or a cover's line names it.
fn sheep() -> GrazingAnimal {
    GrazingAnimal {
        id: String::new(),
        grazing_record_id: String::new(),
        species_code: "03".into(),
        rega_code: "ES071234560001".into(),
        animal_count: 120,
    }
}

/// A live cover, mown and grazed: one change set, three registers.
fn new_cover(land: &Land, book: &str, day: &str, plots: &[String]) -> NewSoilCover {
    NewSoilCover {
        season_id: book.into(),
        farm_id: land.farm.clone(),
        practice_code: "plant_cover".into(),
        cover_type_code: "2".into(),
        established_on: "2026-02-01".into(),
        width_m: None,
        free_canopy_width_m: None,
        widths_stated_on: None,
        notes: None,
        plot_ids: plots.to_vec(),
        maintenance: vec![
            CoverMaintenanceLine {
                id: String::new(),
                kind_code: "mowing".into(),
                performed_on: day.into(),
                performed_end_date: None,
                animals: Vec::new(),
            },
            CoverMaintenanceLine {
                id: String::new(),
                kind_code: GRAZING_MAINTENANCE.into(),
                performed_on: day.into(),
                performed_end_date: None,
                animals: vec![sheep()],
            },
        ],
    }
}

/// A grazing — on a cover when one is named, extensive otherwise.
fn new_grazing(
    land: &Land,
    book: &str,
    day: &str,
    plots: &[String],
    cover: Option<String>,
) -> NewGrazingRecord {
    NewGrazingRecord {
        season_id: book.into(),
        farm_id: land.farm.clone(),
        practice_code: if cover.is_some() {
            "plant_cover"
        } else {
            "extensive_grazing"
        }
        .into(),
        plot_group_ref: None,
        soil_cover_id: cover,
        started_on: day.into(),
        ended_on: Some(day.into()),
        notes: None,
        plot_ids: plots.to_vec(),
        animals: vec![sheep()],
    }
}

fn new_seed_treatment(land: &Land, book: &str, day: &str, plots: &[String]) -> NewSeedTreatment {
    NewSeedTreatment {
        season_id: book.into(),
        farm_id: land.farm.clone(),
        sown_on: day.into(),
        species_name: "trigo blando".into(),
        variety: None,
        crop_code: None,
        seed_quantity_kg: Some(180.0),
        seed_lot: None,
        treatment_kind_code: None,
        acquired_on: None,
        sowing_record_id: None,
        product_name: "Celest".into(),
        product_registration_number: None,
        product_active_substance: None,
        product_id: None,
        efficacy_code: None,
        notes: None,
        plots: plots
            .iter()
            .map(|plot_id| NewSeedTreatmentPlot {
                plot_id: plot_id.clone(),
                surface_sown_ha: 1.0,
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// The acts
// ---------------------------------------------------------------------------

/// An act's line in the history, and what it came to: `Err` holds a refusal.
type Act = (String, Result<(), String>);

/// Record one of six operations into either book.
fn record(conn: &mut Connection, land: &Land, choose: &mut Choices) -> Act {
    let index = choose.below(2);
    let book = &land.books[index];
    let day = DAYS[choose.below(DAYS.len())];
    let plots = land.plots(choose.plot_mask());
    let (what, done) = match choose.below(6) {
        0 => (
            "a treatment",
            phyto::insert_treatment_record(
                conn,
                new_treatment(land, book, day),
                plots
                    .iter()
                    .map(|plot_id| NewTreatmentPlot {
                        plot_id: plot_id.clone(),
                        crop_id: None,
                        surface_treated_ha: 1.0,
                        growth_stage_code: None,
                    })
                    .collect(),
                None,
            )
            .map(|_| ())
            .map_err(debug),
        ),
        1 => (
            "an irrigation",
            fert::insert_irrigation_record(
                conn,
                new_irrigation(land, book, day, &plots, None),
                None,
            )
            .map(|_| ())
            .map_err(debug),
        ),
        2 => {
            // Applied with an irrigation of the same book, when it holds one.
            let irrigation = ids(
                conn,
                "SELECT id FROM irrigation_record WHERE season_id = ?1 ORDER BY id LIMIT 1",
                [book],
            )
            .pop();
            (
                "a fertilisation",
                fert::insert_fertilisation_record(
                    conn,
                    new_fertilisation(land, book, day, &plots, irrigation),
                    None,
                )
                .map(|_| ())
                .map_err(debug),
            )
        }
        3 => (
            "a cover, mown and grazed",
            eco::insert_soil_cover(conn, new_cover(land, book, day, &plots), None)
                .map(|_| ())
                .map_err(debug),
        ),
        4 => (
            "a grazing",
            eco::insert_grazing_record(conn, new_grazing(land, book, day, &plots, None), None)
                .map(|_| ())
                .map_err(debug),
        ),
        _ => (
            "a seed treatment",
            phyto::insert_seed_treatment(conn, new_seed_treatment(land, book, day, &plots), None)
                .map(|_| ())
                .map_err(debug),
        ),
    };
    (format!("records {what} in book {index} on {day}"), done)
}

/// The registers a correction reaches, each with what it changes besides the
/// notes: the plots, and the record it points at in another register.
const CORRECTED: [&str; 7] = [
    "treatment_record",
    "seed_treatment",
    "irrigation_record",
    "fertilisation_record",
    "fertilisation_plan",
    "soil_cover",
    "grazing_record",
];

/// The `pick`th of `candidates`, or none past the last — `pick` drawn before
/// anything was read, so a link is sometimes cleared whatever a book holds.
fn one_or_none(candidates: Vec<String>, pick: usize) -> Option<String> {
    candidates.into_iter().nth(pick)
}

/// Correct a live record — in a live book or a removed one: its notes, the
/// plots it covers, what it points at. A removed record has no form to
/// correct it from.
fn correct(conn: &mut Connection, land: &Land, choose: &mut Choices) -> Option<Act> {
    let table = CORRECTED[choose.below(CORRECTED.len())];
    let note = NOTES[choose.below(NOTES.len())].map(str::to_owned);
    let mask = choose.plot_mask();
    let link = choose.below(4);
    let records = ids(
        conn,
        &format!("SELECT id FROM {table} WHERE deleted_at IS NULL ORDER BY id"),
        [],
    );
    if records.is_empty() {
        return None;
    }
    let id = records[choose.below(records.len())].clone();
    let plots = land.plots(mask);
    // What the record's own book holds of `linked`, for a pointer to it.
    let in_its_book = |conn: &Connection, linked: &str, book: &str| {
        ids(
            conn,
            &format!("SELECT id FROM {linked} WHERE season_id = ?1 ORDER BY id"),
            [book],
        )
    };
    let done = match table {
        "treatment_record" => phyto::get_treatment_record(conn, &id)
            .and_then(|held| {
                let record = held.record;
                let update = UpdateTreatmentRecord {
                    application_date: record.application_date,
                    application_end_date: record.application_end_date,
                    application_time: record.application_time,
                    drying_date: record.drying_date,
                    product_id: record.product_id,
                    dose_value: record.dose_value,
                    dose_unit_code: record.dose_unit_code,
                    total_quantity_value: record.total_quantity_value,
                    total_quantity_unit_code: record.total_quantity_unit_code,
                    target_organism: record.target_organism,
                    problems: held
                        .problems
                        .into_iter()
                        .map(|problem| NewTreatmentProblem {
                            reason_category_code: problem.reason_category_code,
                            problem_code: problem.problem_code,
                        })
                        .collect(),
                    justifications: held
                        .justifications
                        .into_iter()
                        .map(|justification| justification.justification_code)
                        .collect(),
                    operator_id: record.operator_id,
                    machinery_id: record.machinery_id,
                    advisor_id: record.advisor_id,
                    measure_code: record.measure_code,
                    measure_intensity_value: record.measure_intensity_value,
                    measure_intensity_unit_code: record.measure_intensity_unit_code,
                    measure_registration_number: record.measure_registration_number,
                    measure_basic_substance_code: record.measure_basic_substance_code,
                    phi_days_used: record.phi_days_used,
                    notes: note,
                    plots: plots
                        .iter()
                        .map(|plot_id| {
                            let kept = held.plots.iter().find(|plot| &plot.plot_id == plot_id);
                            NewTreatmentPlot {
                                plot_id: plot_id.clone(),
                                crop_id: kept.and_then(|plot| plot.crop_id.clone()),
                                surface_treated_ha: kept
                                    .map_or(1.0, |plot| plot.surface_treated_ha),
                                growth_stage_code: kept
                                    .and_then(|plot| plot.growth_stage_code.clone()),
                            }
                        })
                        .collect(),
                };
                phyto::update_treatment_record(conn, &id, update, None)
            })
            .map(|_| ())
            .map_err(debug),
        "seed_treatment" => phyto::get_seed_treatment(conn, &id)
            .and_then(|held| {
                let record = held.record;
                let sowing =
                    one_or_none(in_its_book(conn, "sowing_record", &record.season_id), link);
                let update = UpdateSeedTreatment {
                    sown_on: record.sown_on,
                    species_name: record.species_name,
                    variety: record.variety,
                    crop_code: record.crop_code,
                    seed_quantity_kg: record.seed_quantity_kg,
                    seed_lot: record.seed_lot,
                    treatment_kind_code: record.treatment_kind_code,
                    acquired_on: record.acquired_on,
                    sowing_record_id: sowing,
                    product_name: record.product_name,
                    product_registration_number: record.product_registration_number,
                    product_active_substance: record.product_active_substance,
                    product_id: record.product_id,
                    notes: note,
                    plots: plots
                        .iter()
                        .map(|plot_id| NewSeedTreatmentPlot {
                            plot_id: plot_id.clone(),
                            surface_sown_ha: 1.0,
                        })
                        .collect(),
                };
                phyto::update_seed_treatment(conn, &id, update, None)
            })
            .map(|_| ())
            .map_err(debug),
        "irrigation_record" => fert::get_irrigation_record(conn, &id)
            .and_then(|held| {
                let record = held.record;
                let update = UpdateIrrigationRecord {
                    id: id.clone(),
                    irrigated_on: record.irrigated_on,
                    irrigation_end_date: record.irrigation_end_date,
                    irrigation_method_code: record.irrigation_method_code,
                    volume_value: record.volume_value,
                    volume_unit_code: record.volume_unit_code,
                    water_nitric_n_mg_l: record.water_nitric_n_mg_l,
                    water_soluble_p2o5_mg_l: record.water_soluble_p2o5_mg_l,
                    energy_type_code: record.energy_type_code,
                    meter_number: record.meter_number,
                    notes: note,
                    plots: plots
                        .iter()
                        .map(|plot_id| {
                            let kept = held.plots.iter().find(|plot| &plot.plot_id == plot_id);
                            NewIrrigationPlot {
                                plot_id: plot_id.clone(),
                                crop_id: kept.and_then(|plot| plot.crop_id.clone()),
                                irrigated_area_ha: kept.and_then(|plot| plot.irrigated_area_ha),
                            }
                        })
                        .collect(),
                    water_origins: held.water_origins,
                    practices: held.practices,
                };
                fert::update_irrigation_record(conn, &id, update, None)
            })
            .map(|_| ())
            .map_err(debug),
        "fertilisation_record" => fert::get_fertilisation_record(conn, &id)
            .and_then(|held| {
                let record = held.record;
                let irrigation = one_or_none(
                    in_its_book(conn, "irrigation_record", &record.season_id),
                    link,
                );
                let update = UpdateFertilisationRecord {
                    id: id.clone(),
                    applied_on: record.applied_on,
                    application_end_date: record.application_end_date,
                    fertilisation_type_code: record.fertilisation_type_code,
                    application_method_code: if irrigation.is_some() {
                        "fertigation_localised".into()
                    } else {
                        record.application_method_code
                    },
                    dose_value: record.dose_value,
                    dose_unit_code: record.dose_unit_code,
                    fertiliser_material_id: record.fertiliser_material_id,
                    sludge_application: record.sludge_application,
                    sustainable_input_management: record.sustainable_input_management,
                    machinery_id: record.machinery_id,
                    irrigation_record_id: irrigation,
                    service_company: record.service_company,
                    service_regfer_number: record.service_regfer_number,
                    delivery_note_ref: record.delivery_note_ref,
                    yield_estimated_kg_ha: record.yield_estimated_kg_ha,
                    yield_final_kg_ha: record.yield_final_kg_ha,
                    notes: note,
                    plots: plots
                        .iter()
                        .map(|plot_id| {
                            let kept = held.plots.iter().find(|plot| &plot.plot_id == plot_id);
                            NewFertilisationPlot {
                                plot_id: plot_id.clone(),
                                crop_id: kept.and_then(|plot| plot.crop_id.clone()),
                                fertilised_area_ha: kept.and_then(|plot| plot.fertilised_area_ha),
                            }
                        })
                        .collect(),
                    practices: held.practices,
                };
                fert::update_fertilisation_record(conn, &id, update, None)
            })
            .map(|_| ())
            .map_err(debug),
        "fertilisation_plan" => fert::get_fertilisation_plan(conn, &id)
            .and_then(|held| {
                let plan = held.plan;
                // The crops of its book the mask names, by position.
                let crops: Vec<String> = in_its_book(conn, "crop", &plan.season_id)
                    .into_iter()
                    .take(3)
                    .enumerate()
                    .filter(|(bit, _)| mask & (1 << bit) != 0)
                    .map(|(_, crop)| crop)
                    .collect();
                let update = UpdateFertilisationPlan {
                    id: id.clone(),
                    needs_n_kg_ha: plan.needs_n_kg_ha,
                    needs_p2o5_kg_ha: plan.needs_p2o5_kg_ha,
                    needs_k2o_kg_ha: plan.needs_k2o_kg_ha,
                    expected_yield_kg_ha: plan.expected_yield_kg_ha,
                    preceding_crop_code: plan.preceding_crop_code,
                    drawn_up_on: plan.drawn_up_on,
                    tool_generated: plan.tool_generated,
                    notes: note,
                    crop_ids: crops,
                };
                fert::update_fertilisation_plan(conn, &id, update, None)
            })
            .map(|_| ())
            .map_err(debug),
        "soil_cover" => eco::get_soil_cover(conn, &id)
            .and_then(|held| {
                let record = held.record;
                // Its maintenance lines the mask keeps, by position, and a
                // new mowing when the link draw says so.
                let mut maintenance: Vec<CoverMaintenanceLine> = held
                    .maintenance
                    .into_iter()
                    .enumerate()
                    .filter(|(bit, _)| mask & (1 << (bit % 3)) != 0)
                    .map(|(_, line)| line)
                    .collect();
                if link == 0 {
                    maintenance.push(CoverMaintenanceLine {
                        id: String::new(),
                        kind_code: "mowing".into(),
                        performed_on: DAYS[1].into(),
                        performed_end_date: None,
                        animals: Vec::new(),
                    });
                }
                let update = UpdateSoilCover {
                    practice_code: record.practice_code,
                    cover_type_code: record.cover_type_code,
                    established_on: record.established_on,
                    width_m: record.width_m,
                    free_canopy_width_m: record.free_canopy_width_m,
                    widths_stated_on: record.widths_stated_on,
                    notes: note,
                    plot_ids: plots.clone(),
                    maintenance,
                };
                eco::update_soil_cover(conn, &id, update, None)
            })
            .map(|_| ())
            .map_err(debug),
        _ => eco::get_grazing_record(conn, &id)
            .and_then(|held| {
                let record = held.record;
                let cover = one_or_none(in_its_book(conn, "soil_cover", &record.season_id), link);
                let update = UpdateGrazingRecord {
                    practice_code: record.practice_code,
                    plot_group_ref: record.plot_group_ref,
                    soil_cover_id: cover,
                    started_on: record.started_on,
                    ended_on: record.ended_on,
                    notes: note,
                    plot_ids: plots.clone(),
                    animals: held.animals,
                };
                eco::update_grazing_record(conn, &id, update, None)
            })
            .map(|_| ())
            .map_err(debug),
    };
    Some((format!("corrects a {table}"), done))
}

/// Remove a live record of any register the book holds, through the
/// register's own delete — what its form's delete button calls.
fn remove(conn: &mut Connection, choose: &mut Choices) -> Option<Act> {
    let register = &DUPLICATE_REGISTERS[choose.below(DUPLICATE_REGISTERS.len())];
    let table = register.policy.table;
    let live = ids(
        conn,
        &format!("SELECT id FROM {table} WHERE deleted_at IS NULL ORDER BY id"),
        [],
    );
    if live.is_empty() {
        return None;
    }
    let id = &live[choose.below(live.len())];
    let done = terrazgo_core::audit::begin(conn, None)
        .map_err(anyhow::Error::from)
        .and_then(|tx| {
            (register.remove)(&tx, id)?;
            tx.commit()?;
            Ok(())
        })
        .map_err(chain);
    Some((format!("removes a {table}"), done))
}

/// State, or withdraw, that a book holds no treated seed — a slot-keyed
/// register whose slot is the book.
fn declare(conn: &mut Connection, land: &Land, choose: &mut Choices) -> Act {
    let index = choose.below(2);
    let book = &land.books[index];
    if choose.below(3) == 0 {
        (
            format!("withdraws book {index}'s declaration"),
            phyto::clear_register_declaration(conn, &land.farm, book, "seed_treatment", None)
                .map_err(debug),
        )
    } else {
        let day = ["2025-10-01", "2025-10-02"][choose.below(2)];
        (
            format!("declares book {index} holds no treated seed, on {day}"),
            phyto::set_register_declaration(conn, &land.farm, book, "seed_treatment", day, None)
                .map(|_| ())
                .map_err(debug),
        )
    }
}

/// Check the first plot against the nitrate zone — a slot-keyed register
/// outside any book, which two devices fill offline as two versions of one
/// register. One plot, so that they do.
fn zone_check(conn: &mut Connection, land: &Land, choose: &mut Choices) -> Act {
    let status = ["inside", "outside"][choose.below(2)];
    let done = repo::replace_zone_flags(
        conn,
        &land.plots[0],
        2026,
        "sigpac",
        vec![NewZoneFlag {
            zone_type_code: "nitrate_vulnerable".into(),
            status: status.into(),
            coverage_pct: None,
            detail: None,
        }],
        None,
    )
    .map(|_| ())
    .map_err(debug);
    (format!("finds the first plot {status} the zone"), done)
}

/// Judge a pair a book's page lists: distinct, or keep either copy; or bring
/// back one of a pair removed twice over. `Err` is a finding: the list a page
/// shows failed to load.
fn judge(
    conn: &mut Connection,
    land: &Land,
    policies: &[DuplicatePolicy],
    captions: &[RowCaption],
    choose: &mut Choices,
) -> Result<Option<Act>, String> {
    let index = choose.below(2);
    let list = repo::list_duplicates(
        conn,
        policies,
        captions,
        Scope::Book {
            season_id: &land.books[index],
        },
    )
    .map_err(|err| format!("book {index}'s duplicates did not load: {err:?}"))?;
    let verdicts = list.suspects.len() * 3;
    let options = verdicts + list.both_removed.len();
    if options == 0 {
        return Ok(None);
    }
    let pick = choose.below(options);
    if pick < verdicts {
        let suspect = &list.suspects[pick / 3];
        let register = register_by_table(suspect.register).unwrap();
        let [first, second] = &suspect.records;
        let act = match pick % 3 {
            0 => (
                format!("judges two {} distinct", suspect.register),
                repo::mark_distinct(conn, &register.policy, &first.id, &second.id, None)
                    .map_err(debug),
            ),
            kept => {
                let (kept, removed) = if kept == 1 {
                    (first, second)
                } else {
                    (second, first)
                };
                (
                    format!("keeps one of two {}", suspect.register),
                    repo::keep_duplicate::<anyhow::Error>(
                        conn,
                        &register.policy,
                        &kept.id,
                        &removed.id,
                        None,
                        register.remove,
                    )
                    .map_err(chain),
                )
            }
        };
        Ok(Some(act))
    } else {
        let removed = &list.both_removed[pick - verdicts];
        let register = register_by_table(removed.register).unwrap();
        let record = &removed.records[choose.below(2)];
        Ok(Some((
            format!(
                "brings back one of two {} removed twice over",
                removed.register
            ),
            repo::restore_removed_duplicate(conn, &register.policy, &record.id, None)
                .map_err(debug),
        )))
    }
}

// ---------------------------------------------------------------------------
// The sweep
// ---------------------------------------------------------------------------

/// One random history, then a full exchange and a device joining. `Err`
/// carries what went wrong and every act that led there.
fn history(seed: u64, steps: usize) -> Result<(), String> {
    let mut choose = Choices::from_seed(seed);
    let Start {
        mut devices,
        group,
        land,
    } = start();
    let policies = terrazgo_lib::duplicates::policies();
    let captions = terrazgo_lib::registry::composed_row_captions();
    let today = today_utc();
    let a_month_on = add_days(&today, repo::REMOVED_BOOK_DAYS + 1).unwrap();
    let mut acts: Vec<String> = Vec::new();
    let failed =
        |what: String, acts: &[String]| format!("seed {seed}: {what}\n{}", acts.join("\n"));

    for _ in 0..steps {
        let who = choose.below(3);
        let conn = &mut devices[who].conn;
        let act: Option<Act> = match choose.below(100) {
            0..=13 => Some(record(conn, &land, &mut choose)),
            14..=29 => correct(conn, &land, &mut choose),
            30..=35 => remove(conn, &mut choose),
            36..=38 => Some(declare(conn, &land, &mut choose)),
            39..=40 => Some(zone_check(conn, &land, &mut choose)),
            41..=50 => judge(conn, &land, &policies, &captions, &mut choose)
                .map_err(|why| failed(format!("{who}: {why}"), &acts))?,
            51..=54 => {
                let index = choose.below(2);
                Some((
                    format!("deletes book {index}"),
                    repo::delete_book(conn, &land.books[index], &policies, None)
                        .map(|_| ())
                        .map_err(debug),
                ))
            }
            55..=57 => {
                let index = choose.below(2);
                let done = repo::restore_book(conn, &land.books[index], &today, None);
                Some((
                    format!(
                        "brings back book {index}: {:?}",
                        done.as_ref().map(|restored| restored.records)
                    ),
                    done.map(|_| ()).map_err(debug),
                ))
            }
            58..=59 => {
                let kept = choose.below(2);
                Some((
                    format!("merges, keeping book {kept}"),
                    repo::merge_books(
                        conn,
                        &land.books[kept],
                        &land.books[1 - kept],
                        &policies,
                        None,
                    )
                    .map(|_| ())
                    .map_err(debug),
                ))
            }
            60..=61 => repo::list_stray_records(conn, &captions)
                .unwrap()
                .first()
                .map(|stray| {
                    let into = stray
                        .suggested
                        .as_ref()
                        .map_or_else(|| land.books[0].clone(), |book| book.id.clone());
                    (
                        "moves records out of a removed book".to_owned(),
                        repo::move_stray_records(conn, &stray.season.id, &into, None)
                            .map(|_| ())
                            .map_err(debug),
                    )
                }),
            // Resolve whatever waits, keeping any of its versions. A
            // resolution has nothing to refuse here, so a failure is a finding.
            62..=69 => {
                let waiting: Vec<(String, String)> = conn
                    .prepare("SELECT DISTINCT root_table, root_id FROM sync_conflict ORDER BY 1, 2")
                    .unwrap()
                    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                    .unwrap()
                    .collect::<rusqlite::Result<_>>()
                    .unwrap();
                if waiting.is_empty() {
                    None
                } else {
                    let (table, id) = &waiting[choose.below(waiting.len())];
                    let versions = heads(conn, table, id).unwrap();
                    let kept = &versions[choose.below(versions.len())];
                    if let Err(err) = resolve(conn, table, id, &kept.device, kept.seq, None) {
                        return Err(failed(
                            format!("{who} could not resolve {table}: {err:?}"),
                            &acts,
                        ));
                    }
                    acts.push(format!(
                        "{who} resolves a {table}, keeping {}/{}",
                        kept.device, kept.seq
                    ));
                    None
                }
            }
            // Erase what is due, as if a month had passed for this device
            // alone — racing restores, resolutions and late writes elsewhere.
            70..=72 => {
                let erased = repo::purge_due(conn, &a_month_on, None)
                    .map_err(|err| failed(format!("{who} could not purge: {err:?}"), &acts))?;
                acts.push(format!("{who} purges: {} erased", erased.registers));
                None
            }
            73 => Some((
                "sees the operator's licence alert".to_owned(),
                repo::acknowledge_alert(
                    conn,
                    module_phytosanitary::alerts::LICENCE_EXPIRY.kind(),
                    &land.operator,
                    Some("2026-08-15"),
                    None,
                )
                .map_err(debug),
            )),
            // A file to another device — whole, or trimmed to what it holds.
            // Nothing in these histories should ever be refused.
            _ => {
                let to = (who + 1 + choose.below(2)) % 3;
                let trimmed = choose.below(2) == 0;
                let (from, receiver) = pair(&mut devices, who, to);
                let seen = if trimmed {
                    bundle::seen_by(&receiver.conn).unwrap()
                } else {
                    VersionVector::default()
                };
                let bytes = from.file(&seen);
                let kind = if trimmed { "trimmed" } else { "whole" };
                if let Err(err) = receiver.apply(&bytes) {
                    return Err(failed(
                        format!("{to} refused {who}'s {kind} file: {err}"),
                        &acts,
                    ));
                }
                acts.push(format!("{who} sends {to} a {kind} file"));
                None
            }
        };
        if let Some((what, done)) = act {
            match done {
                Ok(()) => acts.push(format!("{who} {what}")),
                Err(err) if RAW.iter().any(|raw| err.contains(raw)) => {
                    return Err(failed(
                        format!("{who} {what}: refused by a raw error, not by name: {err}"),
                        &acts,
                    ));
                }
                Err(err) => acts.push(format!("{who} {what}: {err}")),
            }
        }
    }

    // Everything everywhere: two rounds, so what one device learns in the
    // first reaches the others in the second; then twice more with every
    // device purging first, so what is due goes everywhere. Each file is
    // trimmed to what its receiver holds, which carries everything it lacks:
    // whole logs travel in the history above, and here, where every device
    // holds nearly all of them, they would only cost time — writing one is
    // the slowest step there is in a debug build.
    for round in 0..4 {
        if round >= 2 {
            for (who, device) in devices.iter_mut().enumerate() {
                repo::purge_due(&mut device.conn, &a_month_on, None)
                    .map_err(|err| failed(format!("the final purge on {who}: {err:?}"), &acts))?;
            }
        }
        for from in 0..3 {
            for to in (0..3).filter(|to| *to != from) {
                let (sender, receiver) = pair(&mut devices, from, to);
                let bytes = sender.file(&bundle::seen_by(&receiver.conn).unwrap());
                if let Err(err) = receiver.apply(&bytes) {
                    return Err(failed(
                        format!("the final exchange: {to} refused {from}: {err}"),
                        &acts,
                    ));
                }
            }
        }
    }
    let mut joining = Device::new(DEVICES[3]);
    terrazgo_core::sync::join_sync_group(&joining.conn, &group).unwrap();
    joining
        .apply(&devices[0].file(&VersionVector::default()))
        .map_err(|err| failed(format!("a device joining was refused: {err}"), &acts))?;

    let expected = joining.picture(&captions);
    for (index, device) in devices.iter().enumerate() {
        let held = device.picture(&captions);
        if held != expected {
            let mut differences = Vec::new();
            for ((table, rows), (_, want)) in held.tables.iter().zip(&expected.tables) {
                if rows != want {
                    differences.push(format!("{table}:\n  holds {rows:?}\n  fresh {want:?}"));
                }
            }
            if held.queue != expected.queue {
                differences.push(format!(
                    "queue: holds {:?}, fresh {:?}",
                    held.queue, expected.queue
                ));
            }
            if held.strays != expected.strays {
                differences.push(format!(
                    "strays: holds {:?}, fresh {:?}",
                    held.strays, expected.strays
                ));
            }
            return Err(failed(
                format!(
                    "device {index} differs from a fresh replay:\n{}",
                    differences.join("\n")
                ),
                &acts,
            ));
        }
        if device.log() != devices[0].log() {
            return Err(failed(
                format!("device {index} holds a different log"),
                &acts,
            ));
        }
    }
    Ok(())
}

/// Run every seed of `seeds`, and fail naming the first that went wrong and
/// how many did.
fn sweep(seeds: RangeInclusive<u64>, steps: usize) {
    let mut failures: Vec<(u64, String)> = Vec::new();
    for seed in seeds.clone() {
        if let Err(why) = history(seed, steps) {
            failures.push((seed, why));
        }
    }
    if let Some((_, first)) = failures.first() {
        let failed: Vec<u64> = failures.iter().map(|(seed, _)| *seed).collect();
        panic!(
            "{} of {} histories did not converge (seeds {failed:?}); the first:\n{first}",
            failures.len(),
            seeds.count()
        );
    }
}

// Four ranges, so the test runner spreads them across threads.

#[test]
fn whole_schema_histories_converge_seeds_1_to_4() {
    sweep(1..=4, STEPS);
}

#[test]
fn whole_schema_histories_converge_seeds_5_to_8() {
    sweep(5..=8, STEPS);
}

#[test]
fn whole_schema_histories_converge_seeds_9_to_12() {
    sweep(9..=12, STEPS);
}

#[test]
fn whole_schema_histories_converge_seeds_13_to_16() {
    sweep(13..=16, STEPS);
}

// The wide sweep, after a change to the merge layer or to any register, in
// four quarters for the same reason:
// `cargo test -p terrazgo --release --test contracts random_histories -- --ignored`.

#[test]
#[ignore = "a wide sweep, minutes long; run after changing the merge layer or a register"]
fn a_thousand_longer_histories_converge_seeds_1_to_250() {
    sweep(1..=250, 70);
}

#[test]
#[ignore = "a wide sweep, minutes long; run after changing the merge layer or a register"]
fn a_thousand_longer_histories_converge_seeds_251_to_500() {
    sweep(251..=500, 70);
}

#[test]
#[ignore = "a wide sweep, minutes long; run after changing the merge layer or a register"]
fn a_thousand_longer_histories_converge_seeds_501_to_750() {
    sweep(501..=750, 70);
}

#[test]
#[ignore = "a wide sweep, minutes long; run after changing the merge layer or a register"]
fn a_thousand_longer_histories_converge_seeds_751_to_1000() {
    sweep(751..=1_000, 70);
}
