// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The composed SIGPAC operations the shell's commands wrap: lookup (by
//! reference or point, with the dedup check), and verify-and-store for an
//! existing plot. Composition lives here, not in commands, so it is testable
//! offline against a pre-seeded cache (docs/architecture.md → Testing strategy #4).

use crate::client;
use crate::models::{DeclaredCampaign, RecintoInfo};
use crate::reference::SigpacRef;
use crate::storage::{self, PlotMatch};
use rusqlite::Connection;
use serde::Serialize;
use std::collections::HashSet;
use std::sync::Mutex;
use terrazgo_core::catalogue;
use terrazgo_core::models::{Crop, GeoFeature, ZoneFlag};
use terrazgo_geo::{GeoError, Result};

/// A recinto looked up for entry/prefill, with the plots that already carry
/// its reference — the UI offers "attach to existing" over duplicating.
#[derive(Debug, Serialize)]
pub struct RecintoLookup {
    pub recinto: RecintoInfo,
    pub matching_plots: Vec<PlotMatch>,
}

/// A verified plot: what SIGPAC said, the `geo_feature` row it was stored as
/// (replacing this source's previous row — history soft-deleted), and the
/// zone-check results. The boundary is the primary outcome: if the zone
/// checks fail AFTER it stored (network flake, campaign listing down),
/// `zone_flags` is `None` and `zone_check_error` says why — the caller
/// surfaces "zones unchecked, retry", never loses the stored boundary.
#[derive(Debug, Serialize)]
pub struct PlotVerification {
    pub recinto: RecintoInfo,
    pub feature: GeoFeature,
    pub zone_flags: Option<Vec<ZoneFlag>>,
    pub zone_check_error: Option<String>,
}

/// Door A while typing: look a reference up for form prefill. Stores nothing
/// (the plot may not exist yet); `Ok(None)` = SIGPAC does not know the ref.
pub fn lookup_reference(
    app: &Connection,
    cache: &Mutex<Connection>,
    parts: &[String],
    refresh: bool,
) -> Result<Option<RecintoLookup>> {
    let parts: [&str; 7] = parts
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| GeoError::Invalid("sigpac_ref_invalid"))?;
    let reference = SigpacRef::from_parts(parts)?;
    lookup(app, cache, |cache| {
        client::recinto_by_reference(cache, &reference, refresh)
    })
}

/// Door B: the recinto under a map click (later: the GPS position).
pub fn lookup_point(
    app: &Connection,
    cache: &Mutex<Connection>,
    lon: f64,
    lat: f64,
) -> Result<Option<RecintoLookup>> {
    lookup(app, cache, |cache| {
        client::recinto_by_point(cache, lon, lat)
    })
}

/// Verify an existing plot against SIGPAC using its stored reference:
/// persist the official boundary, then run the zone checks (folded into
/// verification per the 2026-07-08 decision — one tap covers both).
/// `Ok(None)` = reference unknown to SIGPAC (typo or outdated) — nothing is
/// stored, the plot is untouched.
pub fn verify_plot(
    app: &mut Connection,
    cache: &Mutex<Connection>,
    plot_id: &str,
    refresh: bool,
    actor: Option<&str>,
) -> Result<Option<PlotVerification>> {
    let reference = storage::plot_reference(app, plot_id)?;
    let Some(recinto) = client::recinto_by_reference(cache, &reference, refresh)? else {
        return Ok(None);
    };
    let feature = storage::save_recinto_boundary(app, plot_id, &recinto, actor)?;
    let (zone_flags, zone_check_error) =
        match check_zones(app, cache, plot_id, &reference, refresh, actor) {
            Ok(flags) => (Some(flags), None),
            // The boundary is already stored; a zone failure must not undo that.
            Err(err) => (None, Some(format!("{err}"))),
        };
    Ok(Some(PlotVerification {
        recinto,
        feature,
        zone_flags,
        zone_check_error,
    }))
}

// ---------------------------------------------------------------------------
// Declared crops: what the PAC declaration says grows on this farm's plots
// ---------------------------------------------------------------------------

/// The FEGA catalogue of crop species. `parc_producto` and `cultsecun_producto`
/// are codes in it, and it is vendored, so a declared crop gets a name with no
/// network involved.
const CROP_CATALOGUE: &str = "PRODUCTOS";
/// Crop code ↔ SIGPAC land use, the vendored catalogue that says which crops
/// are plausible on tierra arable, on pasture, and so on.
const CROP_LAND_USE_CATALOGUE: &str = "CULTIVO_USO_SIGPAC";
/// The attribute `CULTIVO_USO_SIGPAC` rows carry the land use under.
const LAND_USE_ATTR: &str = "Uso SIGPAC";
/// `crop.source` for a row that came from a PAC declaration.
pub const CROP_SOURCE_SIGPAC: &str = "sigpac";

/// What the declaration proposes for one plot, and whether the farmer may act
/// on it. Rows are never applied by this crate — they are reviewed, edited and
/// confirmed first, and the writing happens through core's crop repository.
#[derive(Debug, Serialize)]
pub struct CropProposal {
    pub plot_id: String,
    pub plot_name: String,
    /// The campaign whose declaration this row repeats — shown on every row,
    /// because the service runs a campaign behind and a book must never
    /// silently record last year's declaration as this year's crop.
    pub campaign: i64,
    /// `insert` · `insert_secondary` · `update` · `already_recorded` ·
    /// `blocked`. Only the first three are selectable.
    pub kind: &'static str,
    /// The declared code, always kept even when nothing resolves it — the code
    /// is the payload, the name is display metadata.
    pub crop_code: String,
    /// The catalogue's name for the code; `None` = the code resolved to
    /// nothing, so the row is flagged and the farmer names the crop.
    pub species_name: Option<String>,
    pub declared_area_ha: Option<f64>,
    pub suggested_irrigation_code: Option<String>,
    pub secondary: bool,
    pub existing_crop_id: Option<String>,
    pub existing_species_name: Option<String>,
    /// `multi_crop` · `has_treatments` · `multi_line` — why a differing
    /// declaration is shown but cannot be applied.
    pub blocked_reason: Option<&'static str>,
}

/// A plot the proposal could not speak for, named so it is visibly skipped.
#[derive(Debug, Serialize)]
pub struct SkippedPlot {
    pub plot_id: String,
    pub plot_name: String,
}

/// The whole review panel's content.
#[derive(Debug, Serialize)]
pub struct CropProposals {
    /// The campaign asked for first. With the answering campaign on each row,
    /// this is what the UI needs to say "campaigns 2026 and 2025 were checked".
    pub current_campaign: i64,
    pub rows: Vec<CropProposal>,
    pub plots_without_reference: Vec<SkippedPlot>,
    pub plots_without_declaration: Vec<SkippedPlot>,
    /// Plots SIGPAC could not be asked about at all — offline, or a response
    /// that did not parse. Distinct from `plots_without_declaration` on
    /// purpose: one says the service has no crops for the plot, the other says
    /// we do not know, and only the first is a statement about the farm.
    pub plots_unreachable: Vec<SkippedPlot>,
    /// Why they could not be asked, from the first failure. One reason rather
    /// than one per plot: when this happens at all it is almost always the
    /// same cause for every plot (the `PlotVerification::zone_check_error`
    /// shape — keep the outcome, report the failure beside it).
    pub unreachable_reason: Option<String>,
}

/// Build the declared-crops proposal for a farm's season. Read-only against
/// the app database: it resolves codes, diffs against the crops already
/// recorded and returns rows to review. Nothing is written here — accepting a
/// row is a separate, user-confirmed step through core's crop repository.
///
/// `treated_crop_ids` comes from the CUE module via the shell (the two modules
/// never call each other): a crop this season's treatments point at is never
/// proposed for overwriting.
pub fn propose_crops(
    app: &Connection,
    cache: &Mutex<Connection>,
    farm_id: &str,
    season_id: &str,
    treated_crop_ids: &HashSet<String>,
    refresh: bool,
) -> Result<CropProposals> {
    let current_campaign = client::current_campaign(cache, refresh)?;
    let existing = terrazgo_core::repository::list_crops(app, season_id, farm_id)?;

    let mut rows = Vec::new();
    let mut plots_without_reference = Vec::new();
    let mut plots_without_declaration = Vec::new();
    let mut plots_unreachable = Vec::new();
    let mut unreachable_reason = None;

    for plot in storage::farm_plot_references(app, farm_id)? {
        let Some(reference) = plot.reference.as_ref() else {
            plots_without_reference.push(SkippedPlot {
                plot_id: plot.plot_id,
                plot_name: plot.plot_name,
            });
            continue;
        };
        // A plot that cannot be asked about is reported, not fatal. Offline,
        // a single plot with no declaration in either campaign would otherwise
        // take the whole panel down — and a farm with one pasture outside the
        // PAC declaration is the ordinary case, not an edge one.
        let declaration =
            match client::declared_crops_with_fallback(cache, reference, current_campaign, refresh)
            {
                Ok(Some(declaration)) => declaration,
                Ok(None) => {
                    plots_without_declaration.push(SkippedPlot {
                        plot_id: plot.plot_id,
                        plot_name: plot.plot_name,
                    });
                    continue;
                }
                Err(error) => {
                    unreachable_reason.get_or_insert_with(|| format!("{error}"));
                    plots_unreachable.push(SkippedPlot {
                        plot_id: plot.plot_id,
                        plot_name: plot.plot_name,
                    });
                    continue;
                }
            };
        let plot_crops: Vec<&Crop> = existing
            .iter()
            .filter(|crop| crop.plot_id == plot.plot_id)
            .collect();
        rows.extend(diff_plot(
            app,
            &plot,
            &declaration,
            &plot_crops,
            treated_crop_ids,
        )?);
    }

    Ok(CropProposals {
        current_campaign,
        rows,
        plots_without_reference,
        plots_without_declaration,
        plots_unreachable,
        unreachable_reason,
    })
}

/// One declaration entry before it is matched: main or secondary crop of a line.
struct Entry {
    code: String,
    species_name: Option<String>,
    area_ha: Option<f64>,
    irrigation_code: Option<String>,
    secondary: bool,
}

/// Diff one plot's declaration against the crops it already carries.
///
/// The rule, in order: an entry the plot already records is `already_recorded`;
/// an entry on a plot with no crops is a plain `insert`; an entry that differs
/// from the plot's single, untreated crop is an `update` — but only when the
/// declaration has one main line, because two lines give no way to say which
/// one the single crop should become. Everything else is shown and blocked, so
/// the farmer sees the discrepancy and fixes it by hand.
fn diff_plot(
    app: &Connection,
    plot: &storage::FarmPlotRef,
    declaration: &DeclaredCampaign,
    plot_crops: &[&Crop],
    treated_crop_ids: &HashSet<String>,
) -> Result<Vec<CropProposal>> {
    let mut entries = Vec::new();
    for line in &declaration.lines {
        let area_ha = line.cultivated_area_ha();
        // "S" is secano. "R" says the crop is irrigated but not by which
        // system, and the record book's column is the four-value one
        // (Anexo III A.2.e: "secano o regadío, indicando en su caso el sistema
        // de riego"), so naming a system here would be inventing a fact.
        let irrigation_code = match line.exploitation_system() {
            Some("S") => Some("rainfed".to_string()),
            _ => None,
        };
        for (code, secondary) in [
            (line.product_code(), false),
            (line.secondary_product_code(), true),
        ] {
            let Some(code) = code else { continue };
            let code = code.to_string();
            entries.push(Entry {
                species_name: resolve_species(app, &code)?,
                code,
                area_ha,
                irrigation_code: irrigation_code.clone(),
                secondary,
            });
        }
    }

    let main_lines = entries.iter().filter(|entry| !entry.secondary).count();
    let mut matched: HashSet<&str> = HashSet::new();
    let mut rows = Vec::new();

    for entry in &entries {
        let existing = plot_crops
            .iter()
            .find(|crop| !matched.contains(crop.id.as_str()) && matches_declared(crop, entry));
        if let Some(crop) = existing {
            matched.insert(crop.id.as_str());
            rows.push(
                proposal(plot, declaration, entry, "already_recorded", None).with_existing(crop),
            );
            continue;
        }

        if entry.secondary {
            rows.push(proposal(plot, declaration, entry, "insert_secondary", None));
            continue;
        }

        match plot_crops {
            [] => rows.push(proposal(plot, declaration, entry, "insert", None)),
            [only] if treated_crop_ids.contains(&only.id) => rows.push(
                proposal(plot, declaration, entry, "blocked", Some("has_treatments"))
                    .with_existing(only),
            ),
            [only] if main_lines == 1 => {
                rows.push(proposal(plot, declaration, entry, "update", None).with_existing(only))
            }
            [only] => rows.push(
                proposal(plot, declaration, entry, "blocked", Some("multi_line"))
                    .with_existing(only),
            ),
            _ => rows.push(proposal(
                plot,
                declaration,
                entry,
                "blocked",
                Some("multi_crop"),
            )),
        }
    }
    Ok(rows)
}

/// Whether a recorded crop is the one this declaration entry describes: by
/// catalogue code when the crop carries one, otherwise by name. A crop whose
/// code the catalogue could not resolve has no name to compare, so it never
/// matches by name — guessing there would silently hide a real difference.
fn matches_declared(crop: &Crop, entry: &Entry) -> bool {
    match crop.crop_code.as_deref() {
        Some(code) => code == entry.code,
        None => entry
            .species_name
            .as_deref()
            .is_some_and(|name| normalize(name) == normalize(&crop.species_name)),
    }
}

fn normalize(name: &str) -> String {
    name.trim().to_uppercase()
}

/// The catalogue's name for a declared code, or `None` when nothing resolves
/// it — the code is kept regardless, so an unknown crop is still importable
/// with a name the farmer types.
fn resolve_species(app: &Connection, code: &str) -> Result<Option<String>> {
    let rows = catalogue::find_code(app, CROP_CATALOGUE, code)?;
    Ok(rows.into_iter().next().map(|row| row.label))
}

fn proposal(
    plot: &storage::FarmPlotRef,
    declaration: &DeclaredCampaign,
    entry: &Entry,
    kind: &'static str,
    blocked_reason: Option<&'static str>,
) -> CropProposal {
    CropProposal {
        plot_id: plot.plot_id.clone(),
        plot_name: plot.plot_name.clone(),
        campaign: declaration.campaign,
        kind,
        crop_code: entry.code.clone(),
        species_name: entry.species_name.clone(),
        declared_area_ha: entry.area_ha,
        suggested_irrigation_code: entry.irrigation_code.clone(),
        secondary: entry.secondary,
        existing_crop_id: None,
        existing_species_name: None,
        blocked_reason,
    }
}

impl CropProposal {
    fn with_existing(mut self, crop: &Crop) -> Self {
        self.existing_crop_id = Some(crop.id.clone());
        self.existing_species_name = Some(crop.species_name.clone());
        self
    }
}

/// One crop the species picker offers.
#[derive(Debug, Serialize)]
pub struct SpeciesOption {
    pub code: String,
    pub name: String,
}

/// The picker's options, and whether a land use narrowed them.
#[derive(Debug, Serialize)]
pub struct SpeciesCatalogue {
    /// The SIGPAC land use the list was filtered by; `None` = unfiltered, so
    /// the UI shows no filter chip and there is nothing to "show all" from.
    pub land_use: Option<String>,
    pub options: Vec<SpeciesOption>,
}

/// The crop species the manual form offers, from the vendored FEGA catalogue.
///
/// Given a plot, the list is narrowed to the crops the `CULTIVO_USO_SIGPAC`
/// catalogue considers plausible on that plot's verified land use — a farmer
/// typing on tierra arable should not scroll past olive groves. The narrowing
/// is a convenience and degrades to the full list whenever it cannot be
/// trusted: no plot, no verified boundary, no land use, or a land use that
/// matches nothing. A filter that hides everything is worse than no filter.
pub fn crop_species(app: &Connection, plot_id: Option<&str>) -> Result<SpeciesCatalogue> {
    let all: Vec<SpeciesOption> = catalogue::active_codes(app, CROP_CATALOGUE)?
        .into_iter()
        .map(|row| SpeciesOption {
            code: row.code,
            name: row.label,
        })
        .collect();

    let land_use = match plot_id {
        Some(plot_id) => storage::plot_land_use(app, plot_id)?,
        None => None,
    };
    let Some(land_use) = land_use else {
        return Ok(SpeciesCatalogue {
            land_use: None,
            options: all,
        });
    };

    let plausible: HashSet<String> = catalogue::active_codes(app, CROP_LAND_USE_CATALOGUE)?
        .into_iter()
        .filter(|row| {
            row.attrs
                .as_ref()
                .and_then(|attrs| attrs.get(LAND_USE_ATTR))
                .and_then(serde_json::Value::as_str)
                .is_some_and(|use_code| use_code.trim() == land_use)
        })
        .map(|row| row.code)
        .collect();
    let filtered: Vec<SpeciesOption> = all
        .iter()
        .filter(|option| plausible.contains(&option.code))
        .map(|option| SpeciesOption {
            code: option.code.clone(),
            name: option.name.clone(),
        })
        .collect();

    if filtered.is_empty() {
        return Ok(SpeciesCatalogue {
            land_use: None,
            options: all,
        });
    }
    Ok(SpeciesCatalogue {
        land_use: Some(land_use),
        options: filtered,
    })
}

/// The three zone-layer checks for one recinto, stored replace-within-campaign.
fn check_zones(
    app: &mut Connection,
    cache: &Mutex<Connection>,
    plot_id: &str,
    reference: &SigpacRef,
    refresh: bool,
    actor: Option<&str>,
) -> Result<Vec<ZoneFlag>> {
    let campaign = client::current_campaign(cache, refresh)?;
    let mut results = Vec::with_capacity(client::ZONE_LAYERS.len());
    for (zone_type_code, layer) in client::ZONE_LAYERS {
        let intersection = client::zone_intersection(cache, reference, layer, refresh)?;
        results.push((*zone_type_code, intersection));
    }
    storage::save_zone_checks(app, plot_id, campaign, results, actor)
}

/// Shared lookup shape: fetch, then attach the dedup matches.
fn lookup<F>(app: &Connection, cache: &Mutex<Connection>, fetch: F) -> Result<Option<RecintoLookup>>
where
    F: FnOnce(&Mutex<Connection>) -> Result<Option<RecintoInfo>>,
{
    let Some(recinto) = fetch(cache)? else {
        return Ok(None);
    };
    let matching_plots = storage::find_plots_with_reference(app, &recinto.reference)?;
    Ok(Some(RecintoLookup {
        recinto,
        matching_plots,
    }))
}
