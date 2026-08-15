// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Offline tests for the declared-crops proposal: the diff and guard rules
//! that decide what the farmer may import from the PAC declaration, and the
//! species picker's land-use narrowing.
//!
//! The declaration fixtures under `tests/fixtures/` are REAL Nube de SIGPAC
//! responses harvested 2026-08-03 (recintos 47/163/0/0/11/40/1 and
//! 47/219/0/0/11/28/2, Valladolid). Codes resolve against the vendored FEGA
//! catalogues, so nothing here reaches the network.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_sigpac::service::{CropProposals, crop_species, propose_crops};
use rusqlite::Connection;
use std::collections::HashSet;
use std::sync::Mutex;
use terrazgo_core::models::{NewCrop, NewFarm, NewPlot, PlotEsFields};
use terrazgo_core::repository::{insert_crop, insert_farm, insert_plot, insert_season};
use terrazgo_geo::db::open_cache_in_memory;

const DECLARED: &[u8] = include_bytes!("fixtures/cultivo-declarado.json");
const DECLARED_EMPTY: &[u8] = include_bytes!("fixtures/cultivo-declarado-empty.json");
const DECLARED_SECONDARY: &[u8] = include_bytes!("fixtures/cultivo-declarado-secondary.json");
const DECLARED_MULTILINE: &[u8] = include_bytes!("fixtures/cultivo-declarado-multiline.json");
const CAMPAIGNS: &[u8] = include_bytes!("fixtures/geopackages-listing.html");

/// The fixture recintos, as the seven reference parts a plot stores.
const VALLADOLID: [&str; 7] = ["47", "163", "0", "0", "11", "40", "1"];
const VALLADOLID_SECONDARY: [&str; 7] = ["47", "219", "0", "0", "11", "28", "2"];
const VALLADOLID_MULTILINE: [&str; 7] = ["47", "219", "0", "0", "10", "91", "3"];

/// The harvested campaign listing names 2026 as current, and the declaration
/// answers for 2025 — the one-campaign-behind case that is the normal path.
const CURRENT: i64 = 2026;
const ANSWERING: i64 = 2025;

struct Fixture {
    app: Connection,
    cache: Mutex<Connection>,
    farm_id: String,
    season_id: String,
    plot_id: String,
}

/// A farm with one plot carrying the fixture reference, and a cache holding
/// the campaign listing plus that recinto's declaration for both campaigns.
fn fixture(parts: [&str; 7], declaration: &[u8]) -> Fixture {
    let mut app = terrazgo_core::db::open_in_memory().unwrap();
    terrazgo_core::catalogue::ensure_catalogues(&mut app).unwrap();

    let farm = insert_farm(
        &mut app,
        NewFarm {
            name: "La Vega".into(),
            owner_name: None,
            owner_tax_id: None,
            country_code: "es".into(),
            es: None,
        },
        None,
    )
    .unwrap();
    let plot = insert_plot(
        &mut app,
        NewPlot {
            farm_id: farm.id.clone(),
            name: "El Páramo".into(),
            area_ha: Some(30.0),
            es: Some(PlotEsFields {
                sigpac_province: Some(parts[0].into()),
                sigpac_municipality: Some(parts[1].into()),
                sigpac_aggregate: Some(parts[2].into()),
                sigpac_zone: Some(parts[3].into()),
                sigpac_polygon: Some(parts[4].into()),
                sigpac_parcel: Some(parts[5].into()),
                sigpac_enclosure: Some(parts[6].into()),
            }),
        },
        None,
    )
    .unwrap();
    let season = insert_season(
        &mut app,
        terrazgo_core::models::NewSeason {
            campaign_year: 2026,
            label: "2025-2026".into(),
            starts_on: None,
            ends_on: None,
        },
        None,
    )
    .unwrap();

    let path = parts.join("/");
    let cache = seeded_cache(&[
        ("sigpac/campaigns".to_string(), CAMPAIGNS),
        (format!("sigpac/cultivos/{CURRENT}/{path}"), DECLARED_EMPTY),
        (format!("sigpac/cultivos/{ANSWERING}/{path}"), declaration),
    ]);

    Fixture {
        app,
        cache,
        farm_id: farm.id,
        season_id: season.id,
        plot_id: plot.id,
    }
}

/// Seeded as fetched TODAY on purpose. The fallback re-asks an EMPTY
/// current-campaign answer stored on an earlier day, so seeding a fixed past
/// date would make every test here depend on the machine having network.
fn seeded_cache(entries: &[(String, &[u8])]) -> Mutex<Connection> {
    let cache = open_cache_in_memory().unwrap();
    for (key, data) in entries {
        cache
            .execute(
                "INSERT INTO resource (key, data, content_type, fetched_at)
                 VALUES (?1, ?2, 'application/json', ?3)",
                rusqlite::params![key, data, today_stamp()],
            )
            .unwrap();
    }
    Mutex::new(cache)
}

/// Today, as the cache writes it.
fn today_stamp() -> String {
    format!("{}T00:00:00Z", terrazgo_core::date::today_utc())
}

fn add_crop(fx: &mut Fixture, species: &str, crop_code: Option<&str>) -> String {
    insert_crop(
        &mut fx.app,
        NewCrop {
            plot_id: fx.plot_id.clone(),
            season_id: fx.season_id.clone(),
            species_name: species.into(),
            variety: None,
            production_system_code: None,
            area_ha: None,
            irrigation_code: None,
            growing_environment_code: None,
            gip_system_code: None,
            sown_on: None,
            crop_code: crop_code.map(str::to_string),
            source: None,
            source_campaign: None,
            declared_area_ha: None,
        },
        None,
    )
    .unwrap()
    .id
}

fn propose(fx: &Fixture, treated: &HashSet<String>) -> CropProposals {
    propose_crops(
        &fx.app,
        &fx.cache,
        &fx.farm_id,
        &fx.season_id,
        treated,
        false,
    )
    .unwrap()
}

/// The plain case: an empty season, a declared crop, one importable row —
/// carrying the campaign that answered, the catalogue's name for the code, and
/// the surface converted out of the service's square metres.
#[test]
fn a_declared_crop_on_an_empty_plot_is_a_plain_insert() {
    let fx = fixture(VALLADOLID, DECLARED);
    let proposals = propose(&fx, &HashSet::new());

    assert_eq!(proposals.current_campaign, CURRENT);
    assert_eq!(proposals.rows.len(), 1);
    let row = &proposals.rows[0];
    assert_eq!(row.kind, "insert");
    // The declaration answered for the PREVIOUS campaign; saying so on every
    // row is what stops last year's declaration becoming this year's record.
    assert_eq!(row.campaign, ANSWERING);
    // PRODUCTOS code 5 = CEBADA (vendored FEGA catalogue).
    assert_eq!(row.crop_code, "5");
    assert_eq!(row.species_name.as_deref(), Some("CEBADA"));
    // parc_supcult 296800 m² = 29,68 ha.
    assert_eq!(row.declared_area_ha, Some(29.68));
    assert!(!row.secondary);
    assert!(row.existing_crop_id.is_none());
}

/// `parc_sistexp` says whether a crop is irrigated, never by which system,
/// while the record book's column is the four-value one (Anexo III A.2.e:
/// "secano o regadío, indicando en su caso el sistema de riego"). So secano
/// prefills and regadío deliberately does not.
#[test]
fn secano_prefills_the_irrigation_code_and_regadio_leaves_it_unset() {
    let secano = fixture(VALLADOLID, DECLARED);
    let rows = propose(&secano, &HashSet::new()).rows;
    assert_eq!(
        rows[0].suggested_irrigation_code.as_deref(),
        Some("rainfed")
    );

    let regadio = fixture(VALLADOLID_SECONDARY, DECLARED_SECONDARY);
    for row in propose(&regadio, &HashSet::new()).rows {
        assert_eq!(
            row.suggested_irrigation_code, None,
            "regadío must not claim a system SIGPAC never stated"
        );
    }
}

/// A line declaring a secondary crop offers it as its own row: a second crop
/// on the same plot, never a correction of the first.
#[test]
fn a_secondary_crop_becomes_its_own_row() {
    let fx = fixture(VALLADOLID_SECONDARY, DECLARED_SECONDARY);
    let rows = propose(&fx, &HashSet::new()).rows;

    assert_eq!(rows.len(), 2);
    // Codes 4 = MAÍZ (main) and 6 = CENTENO (secondary).
    assert_eq!(rows[0].kind, "insert");
    assert_eq!(rows[0].crop_code, "4");
    assert!(!rows[0].secondary);
    assert_eq!(rows[1].kind, "insert_secondary");
    assert_eq!(rows[1].crop_code, "6");
    assert!(rows[1].secondary);
    assert_eq!(rows[1].species_name.as_deref(), Some("CENTENO"));
}

/// A crop the plot already records is shown, but as a statement of fact rather
/// than an offer — matched either by the catalogue code it carries or, when it
/// has none, by its name.
#[test]
fn an_already_recorded_crop_matches_by_code_and_by_name() {
    let mut by_code = fixture(VALLADOLID, DECLARED);
    let crop_id = add_crop(&mut by_code, "cebada de invierno", Some("5"));
    let rows = propose(&by_code, &HashSet::new()).rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, "already_recorded");
    assert_eq!(rows[0].existing_crop_id.as_deref(), Some(crop_id.as_str()));

    // No code stored: the crop's own name is compared, trimmed and case-folded.
    let mut by_name = fixture(VALLADOLID, DECLARED);
    add_crop(&mut by_name, "  Cebada  ", None);
    let rows = propose(&by_name, &HashSet::new()).rows;
    assert_eq!(rows[0].kind, "already_recorded");
}

/// The guarded update: the plot's single crop says one thing, the declaration
/// another, and nothing has been applied to it yet — so restating it is safe
/// and offered.
#[test]
fn a_single_untreated_differing_crop_is_offered_as_an_update() {
    let mut fx = fixture(VALLADOLID, DECLARED);
    let crop_id = add_crop(&mut fx, "trigo blando", Some("1"));

    let rows = propose(&fx, &HashSet::new()).rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, "update");
    assert_eq!(rows[0].existing_crop_id.as_deref(), Some(crop_id.as_str()));
    assert_eq!(
        rows[0].existing_species_name.as_deref(),
        Some("trigo blando")
    );
    assert_eq!(rows[0].species_name.as_deref(), Some("CEBADA"));
}

/// Three ways an update is refused, each shown so the farmer sees the
/// discrepancy instead of the row silently vanishing.
#[test]
fn differing_crops_are_blocked_when_the_target_is_ambiguous_or_treated() {
    // Treated: past records keep their own frozen snapshot, but section 2.1
    // would then state a crop the treatment beside it contradicts.
    let mut treated_case = fixture(VALLADOLID, DECLARED);
    let crop_id = add_crop(&mut treated_case, "trigo blando", Some("1"));
    let treated = HashSet::from([crop_id.clone()]);
    let rows = propose(&treated_case, &treated).rows;
    assert_eq!(rows[0].kind, "blocked");
    assert_eq!(rows[0].blocked_reason, Some("has_treatments"));
    assert_eq!(rows[0].existing_crop_id.as_deref(), Some(crop_id.as_str()));

    // Several crops on the plot: no way to say which one the declaration
    // means, so the farmer decides by hand.
    let mut multi_crop = fixture(VALLADOLID, DECLARED);
    add_crop(&mut multi_crop, "trigo blando", Some("1"));
    add_crop(&mut multi_crop, "veza", Some("52"));
    let rows = propose(&multi_crop, &HashSet::new()).rows;
    assert_eq!(rows[0].kind, "blocked");
    assert_eq!(rows[0].blocked_reason, Some("multi_crop"));

    // Two main declaration lines against one recorded crop: the same
    // ambiguity from the other side — there is no saying which line the
    // single crop was meant to be.
    let mut multi_line = fixture(VALLADOLID_MULTILINE, DECLARED_MULTILINE);
    add_crop(&mut multi_line, "trigo blando", Some("1"));
    let rows = propose(&multi_line, &HashSet::new()).rows;
    assert_eq!(rows.len(), 2);
    for row in &rows {
        assert_eq!(row.kind, "blocked");
        assert_eq!(row.blocked_reason, Some("multi_line"));
    }
}

/// A recinto declared in several lines is a real and common shape: this one
/// splits the same crop into an irrigated and a rainfed part. On an empty plot
/// both become their own rows, because that is what the declaration says grows
/// there — and only the rainfed part gets an irrigation code.
#[test]
fn several_declaration_lines_on_an_empty_plot_are_several_rows() {
    let fx = fixture(VALLADOLID_MULTILINE, DECLARED_MULTILINE);
    let rows = propose(&fx, &HashSet::new()).rows;

    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|row| row.kind == "insert"));
    // Both lines declare code 178; they differ in system and surface.
    assert!(rows.iter().all(|row| row.crop_code == "178"));
    assert_eq!(rows[0].declared_area_ha, Some(5.43));
    assert_eq!(rows[0].suggested_irrigation_code, None); // "R"
    assert_eq!(rows[1].declared_area_ha, Some(1.12));
    assert_eq!(
        rows[1].suggested_irrigation_code.as_deref(),
        Some("rainfed")
    );
}

/// A code the catalogue cannot name is still importable: the code is the
/// payload and the name is display metadata, so the row keeps the first and
/// leaves the second for the farmer.
#[test]
fn an_unresolvable_code_keeps_the_code_and_blanks_the_name() {
    let fx = fixture(VALLADOLID, DECLARED);
    // A database without catalogues is exactly the "code resolves to nothing"
    // case, and reaches it without inventing a fake declaration.
    fx.app.execute("DELETE FROM catalogue_code", []).unwrap();

    let rows = propose(&fx, &HashSet::new()).rows;
    assert_eq!(rows[0].crop_code, "5");
    assert_eq!(rows[0].species_name, None);
    assert_eq!(rows[0].kind, "insert");
}

/// A plot without a usable SIGPAC reference, and a plot SIGPAC has nothing for,
/// are two different answers — and both are named rather than dropped, so the
/// farmer can see which of their plots the import could not speak for.
#[test]
fn plots_without_a_reference_or_a_declaration_are_reported_not_dropped() {
    let mut fx = fixture(VALLADOLID, DECLARED);
    let bare = insert_plot(
        &mut fx.app,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "Sin referencia".into(),
            area_ha: Some(1.0),
            es: None,
        },
        None,
    )
    .unwrap();

    // A second referenced plot whose declaration is empty in both campaigns.
    let undeclared_parts = ["47", "163", "0", "0", "11", "41", "1"];
    let undeclared = insert_plot(
        &mut fx.app,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "Sin declaración".into(),
            area_ha: Some(1.0),
            es: Some(PlotEsFields {
                sigpac_province: Some(undeclared_parts[0].into()),
                sigpac_municipality: Some(undeclared_parts[1].into()),
                sigpac_aggregate: Some(undeclared_parts[2].into()),
                sigpac_zone: Some(undeclared_parts[3].into()),
                sigpac_polygon: Some(undeclared_parts[4].into()),
                sigpac_parcel: Some(undeclared_parts[5].into()),
                sigpac_enclosure: Some(undeclared_parts[6].into()),
            }),
        },
        None,
    )
    .unwrap();
    let path = undeclared_parts.join("/");
    {
        let cache = fx.cache.lock().unwrap();
        for campaign in [CURRENT, ANSWERING] {
            cache
                .execute(
                    "INSERT INTO resource (key, data, content_type, fetched_at)
                     VALUES (?1, ?2, 'application/json', ?3)",
                    rusqlite::params![
                        format!("sigpac/cultivos/{campaign}/{path}"),
                        DECLARED_EMPTY,
                        today_stamp()
                    ],
                )
                .unwrap();
        }
    }

    let proposals = propose(&fx, &HashSet::new());
    assert_eq!(proposals.rows.len(), 1, "only the declared plot proposes");
    assert_eq!(proposals.plots_without_reference.len(), 1);
    assert_eq!(proposals.plots_without_reference[0].plot_id, bare.id);
    assert_eq!(proposals.plots_without_declaration.len(), 1);
    assert_eq!(
        proposals.plots_without_declaration[0].plot_id,
        undeclared.id
    );
}

/// A plot SIGPAC cannot be asked about is reported beside the others, and the
/// rest of the panel still works.
///
/// This is not hypothetical: offline, a plot with no declaration in EITHER
/// campaign fails, because the current campaign's cached empty is deliberately
/// not trusted and the failure to re-ask is not evidence of anything. A farm
/// with one pasture outside the PAC declaration is the ordinary case, so
/// aborting the whole proposal over it would make the feature useless offline
/// exactly where it is most needed.
///
/// The failure is provoked with an unreadable cached response rather than by
/// cutting the network: same code path, and no test in this crate may depend
/// on the machine's connectivity.
#[test]
fn a_plot_that_cannot_be_asked_about_is_reported_and_does_not_abort_the_rest() {
    let mut fx = fixture(VALLADOLID, DECLARED);
    let broken_parts = ["47", "163", "0", "0", "11", "42", "1"];
    let broken = insert_plot(
        &mut fx.app,
        NewPlot {
            farm_id: fx.farm_id.clone(),
            name: "Sin respuesta".into(),
            area_ha: Some(1.0),
            es: Some(PlotEsFields {
                sigpac_province: Some(broken_parts[0].into()),
                sigpac_municipality: Some(broken_parts[1].into()),
                sigpac_aggregate: Some(broken_parts[2].into()),
                sigpac_zone: Some(broken_parts[3].into()),
                sigpac_polygon: Some(broken_parts[4].into()),
                sigpac_parcel: Some(broken_parts[5].into()),
                sigpac_enclosure: Some(broken_parts[6].into()),
            }),
        },
        None,
    )
    .unwrap();
    let path = broken_parts.join("/");
    {
        let cache = fx.cache.lock().unwrap();
        cache
            .execute(
                "INSERT INTO resource (key, data, content_type, fetched_at)
                 VALUES (?1, ?2, 'application/json', ?3)",
                rusqlite::params![
                    format!("sigpac/cultivos/{CURRENT}/{path}"),
                    b"{\"not\":\"a feature collection\"}".as_slice(),
                    today_stamp()
                ],
            )
            .unwrap();
    }

    let proposals = propose(&fx, &HashSet::new());

    assert_eq!(
        proposals.rows.len(),
        1,
        "the plot that DID answer still proposes"
    );
    assert_eq!(proposals.plots_unreachable.len(), 1);
    assert_eq!(proposals.plots_unreachable[0].plot_id, broken.id);
    assert!(
        proposals.unreachable_reason.is_some(),
        "the farmer is told why, not just which"
    );
    // Not knowing and knowing there is nothing are different answers, so the
    // unreachable plot must not land in the "no declaration" list.
    assert!(
        proposals
            .plots_without_declaration
            .iter()
            .all(|plot| plot.plot_id != broken.id)
    );
}

/// Proposals are a reading, not a write: nothing may reach the app database
/// until the farmer confirms a row.
#[test]
fn proposing_writes_nothing() {
    let fx = fixture(VALLADOLID, DECLARED);
    let count = |conn: &Connection, table: &str| -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    };
    let before = (
        count(&fx.app, "crop"),
        count(&fx.app, "record_change"),
        count(&fx.app, "geo_feature"),
    );

    propose(&fx, &HashSet::new());

    assert_eq!(
        before,
        (
            count(&fx.app, "crop"),
            count(&fx.app, "record_change"),
            count(&fx.app, "geo_feature"),
        )
    );
}

// --- the species picker ----------------------------------------------------

/// With a verified plot the picker narrows to what the land use plausibly
/// grows, so the farmer scrolls past hundreds of crops instead of a thousand.
#[test]
fn species_are_narrowed_by_the_plots_verified_land_use() {
    let fx = fixture(VALLADOLID, DECLARED);
    let all = crop_species(&fx.app, None).unwrap();
    assert!(all.land_use.is_none());
    assert!(all.options.len() > 1000, "the full catalogue is offered");

    store_land_use(&fx, "TA");
    let filtered = crop_species(&fx.app, Some(&fx.plot_id)).unwrap();
    assert_eq!(filtered.land_use.as_deref(), Some("TA"));
    assert!(filtered.options.len() < all.options.len());
    // CULTIVO_USO_SIGPAC pairs code 1 (TRIGO BLANDO) with uso TA, tierras
    // arables — the vendored catalogue's own row.
    assert!(
        filtered
            .options
            .iter()
            .any(|option| option.code == "1" && option.name == "TRIGO BLANDO")
    );
}

/// The narrowing is a convenience, so it steps aside whenever it cannot be
/// trusted — an unverified plot, or a land use nothing is listed for. A filter
/// that hides every option is worse than no filter at all.
#[test]
fn the_picker_falls_back_to_every_species_when_it_cannot_narrow() {
    let fx = fixture(VALLADOLID, DECLARED);
    let all = crop_species(&fx.app, None).unwrap().options.len();

    // Never verified: no stored boundary, so no land use to filter by.
    let unverified = crop_species(&fx.app, Some(&fx.plot_id)).unwrap();
    assert!(unverified.land_use.is_none());
    assert_eq!(unverified.options.len(), all);

    store_land_use(&fx, "NO-SUCH-USE");
    let empty_match = crop_species(&fx.app, Some(&fx.plot_id)).unwrap();
    assert!(empty_match.land_use.is_none());
    assert_eq!(empty_match.options.len(), all);
}

/// The land use lives in the provider boundary's properties, exactly as
/// `verify_plot` stores it.
fn store_land_use(fx: &Fixture, land_use: &str) {
    fx.app
        .execute("DELETE FROM geo_feature WHERE plot_id = ?1", [&fx.plot_id])
        .unwrap();
    fx.app
        .execute(
            "INSERT INTO geo_feature
               (id, plot_id, role, geometry, source, properties, created_at, updated_at)
             VALUES ('feat-1', ?1, 'boundary', '{\"type\":\"Point\",\"coordinates\":[0,0]}',
                     'sigpac', ?2, '2026-08-03T00:00:00Z', '2026-08-03T00:00:00Z')",
            rusqlite::params![&fx.plot_id, format!("{{\"uso_sigpac\":\"{land_use}\"}}")],
        )
        .unwrap();
}
