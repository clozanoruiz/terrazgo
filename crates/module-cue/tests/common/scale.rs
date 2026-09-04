// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! A holding with years of history behind it.
//!
//! Every other fixture in this directory builds the two or three rows one
//! behaviour needs. This one builds the shape the query-scope audit measured
//! on — one farm, many plots, many campaigns, hundreds of treatments each —
//! because two classes of defect are invisible at three rows and obvious at
//! four thousand: a query whose result set grows with the history, and a
//! per-record child query.
//!
//! It goes through the real repository rather than raw `INSERT`s. That is
//! slower, and it is the point: the `record_change` rows, the junctions and
//! the derived `phi_end_date` all have to be there, or the thing being measured
//! is not the thing that ships.
//!
//! Lives here rather than in `terrazgo-testkit` because it needs this module's
//! schema, which is the testkit's stated boundary. The other register-owning
//! crates carry their own.

// One binary uses a subset of this; that is what shared means, not dead code.
#![allow(dead_code)]
// Test code may unwrap (clippy.toml exempts tests); the workspace lint only
// auto-allows #[test] fns, so file-level for the shared fixtures/helpers too.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use module_cue::models::*;
use module_cue::repository as repo;
use rusqlite::Connection;

use super::treatment::{add_es_authorisation, base_fixture, sample_treatment};

/// The dimensions of the synthetic holding. Defaults are the audit's
/// smallest data point — ten campaigns of a 120-plot farm — so a caller that
/// only wants "a lot of history" needs no arithmetic.
pub struct Scale {
    pub seasons: usize,
    pub plots: usize,
    pub records_per_season: usize,
    /// Treated plots per record. A multi-plot treatment is the normal case and
    /// it is what makes `treatment_plot` outgrow `treatment_record`.
    pub plots_per_record: usize,
}

impl Default for Scale {
    fn default() -> Self {
        Self {
            seasons: 10,
            plots: 120,
            records_per_season: 400,
            plots_per_record: 2,
        }
    }
}

impl Scale {
    /// Small enough to run inside an ordinary test, large enough that a
    /// per-record query is unmistakable in the count.
    pub fn small() -> Self {
        Self {
            seasons: 1,
            plots: 8,
            records_per_season: 4,
            plots_per_record: 2,
        }
    }

    /// `small`, with four times the records and the same everything else — the
    /// second half of a counting test: the rows multiply, the statements must
    /// not.
    pub fn small_times_four() -> Self {
        Self {
            records_per_season: 16,
            ..Self::small()
        }
    }

    pub fn total_records(&self) -> usize {
        self.seasons * self.records_per_season
    }
}

/// What a scaled build hands back: the ids a test needs to ask a question of
/// it. Seasons are newest last, matching the order they were created in.
pub struct ScaledFarm {
    pub farm_id: String,
    pub season_ids: Vec<String>,
    pub plot_ids: Vec<String>,
}

impl ScaledFarm {
    /// The season a "list one campaign" query should be pointed at.
    pub fn latest_season(&self) -> &str {
        self.season_ids.last().expect("at least one season")
    }
}

/// Build the holding. Application dates walk forward through each campaign so
/// PHI windows land at different points, and the LAST campaign's records reach
/// up to `2026-06-30` — so a `today` around then sees a handful of open windows
/// against a long closed history, which is the real ratio the map's tint and
/// the alert refresh face.
pub fn scaled_farm(conn: &mut Connection, scale: &Scale) -> ScaledFarm {
    let fx = base_fixture(conn);
    add_es_authorisation(conn, &fx.product_id);

    let plot_ids: Vec<String> = (0..scale.plots)
        .map(|i| {
            repo::insert_plot(
                conn,
                NewPlot {
                    farm_id: fx.farm_id.clone(),
                    name: format!("Parcela {i}"),
                    area_ha: Some(2.5),
                    es: None,
                },
                None,
            )
            .unwrap()
            .id
        })
        .collect();

    // The fixture's own season is campaign 2026 and is used as the last one, so
    // the earlier campaigns count backwards from it.
    let mut season_ids: Vec<String> = (0..scale.seasons.saturating_sub(1))
        .map(|i| {
            let year = 2026 - (scale.seasons as i64 - 1) + i as i64;
            repo::insert_season(
                conn,
                NewSeason {
                    campaign_year: year,
                    label: year.to_string(),
                    starts_on: None,
                    ends_on: None,
                },
                None,
            )
            .unwrap()
            .id
        })
        .collect();
    season_ids.push(fx.season_id.clone());

    // One running counter across the whole build, so consecutive CAMPAIGNS
    // treat different plots rather than the same handful every year. Without
    // that, "the plots treated recently" and "the plots treated ever" are the
    // same set and no test can tell a scoped query from an unscoped one.
    let mut slot = 0usize;
    for (s, season_id) in season_ids.iter().enumerate() {
        let year = 2026 - (scale.seasons as i64 - 1) + s as i64;
        for r in 0..scale.records_per_season {
            let mut new = sample_treatment(&fx, None, Some(21));
            new.season_id = season_id.clone();
            new.application_date = walking_date(year, r, scale.records_per_season);
            let plots = (0..scale.plots_per_record)
                .map(|_| {
                    let plot_id = plot_ids[slot % plot_ids.len()].clone();
                    slot += 1;
                    NewTreatmentPlot {
                        plot_id,
                        crop_id: None,
                        surface_treated_ha: 1.0,
                        growth_stage_code: None,
                    }
                })
                .collect();
            repo::insert_treatment_record(conn, new, plots, None).unwrap();
        }
    }

    ScaledFarm {
        farm_id: fx.farm_id,
        season_ids,
        plot_ids,
    }
}

/// Spread `count` applications across the growing season, March to June, so
/// that consecutive records do not share a date and the last of a campaign is
/// the most recent thing that happened.
fn walking_date(year: i64, index: usize, count: usize) -> String {
    let day_span = 120; // 1 March to 28 June
    let offset = if count <= 1 {
        day_span
    } else {
        index * day_span / count
    };
    let (month, day) = match offset {
        0..=30 => (3, offset + 1),
        31..=60 => (4, offset - 30),
        61..=90 => (5, offset - 60),
        _ => (6, (offset - 90).min(30)),
    };
    format!("{year:04}-{month:02}-{day:02}")
}
