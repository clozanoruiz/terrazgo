// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The land a register test runs on: one farm with two plots and its season,
//! and a *second* farm with a plot and a season of its own.
//!
//! Extracted from `module-fertilisation/tests/irrigation.rs` and
//! `module-ecoscheme/tests/grazing.rs`, which held the same sixty lines and
//! differed only in the plot names and areas — so those stay parameters.

use rusqlite::Connection;
use terrazgo_core::models::{NewFarm, NewPlot, NewSeason};
use terrazgo_core::repository as core_repo;

/// The ids a register test needs to write its own rows.
///
/// `other_farm_plot` is not decoration: every register that scopes records to a
/// farm has a rejection test for a plot that belongs to someone else, and the
/// fixture has to be able to hand it one.
pub struct CoreFixture {
    /// The farm's own season. A season belongs to one farm, so this is the
    /// only one a record on `farm_id` may be filed under.
    pub season_id: String,
    pub farm_id: String,
    pub plot_a: String,
    pub plot_b: String,
    /// The second farm. Registers that scope more than plots to a farm —
    /// machinery, premises — hang their own foreign row off this.
    pub other_farm_id: String,
    /// A plot on a different farm — the subject of every `PlotNotOnFarm` test.
    pub other_farm_plot: String,
    /// The second farm's season, same campaign year and label. A test that
    /// writes a record ON the other farm files it here: filing it under
    /// `season_id` is refused by the schema before any rule under test runs.
    pub other_season_id: String,
}

/// One plot to create: the name a test's assertions read back, and its area.
pub struct PlotSpec {
    pub name: String,
    pub area_ha: f64,
}

impl PlotSpec {
    pub fn new(name: &str, area_ha: f64) -> Self {
        Self {
            name: name.into(),
            area_ha,
        }
    }
}

/// What [`farm_with_plots`] should create. Override the fields a test asserts
/// on and leave the rest at their defaults:
///
/// ```
/// use terrazgo_testkit::{FarmWithPlots, PlotSpec, farm_with_plots};
///
/// // A module test would open through its own `open_in_memory()` instead;
/// // the fixture writes core rows, so it runs on either schema.
/// let mut conn = terrazgo_core::open_in_memory().unwrap();
/// let fx = farm_with_plots(
///     &mut conn,
///     FarmWithPlots {
///         farm_name: "Dehesa de Arriba".into(),
///         plot_a: PlotSpec::new("Pasto Alto", 22.0),
///         plot_b: PlotSpec::new("Pasto Bajo", 18.0),
///         ..Default::default()
///     },
/// );
/// assert_ne!(fx.plot_a, fx.other_farm_plot);
/// ```
pub struct FarmWithPlots {
    /// Both farms' season bounds. The defaults span the new year, so the
    /// seasons are named "2025/2026".
    pub season_starts_on: String,
    pub season_ends_on: String,
    pub farm_name: String,
    pub other_farm_name: String,
    pub plot_a: PlotSpec,
    pub plot_b: PlotSpec,
    pub other_farm_plot: PlotSpec,
}

impl Default for FarmWithPlots {
    fn default() -> Self {
        Self {
            season_starts_on: "2025-09-01".into(),
            season_ends_on: "2026-08-31".into(),
            farm_name: "Finca La Vega".into(),
            other_farm_name: "Finca del Vecino".into(),
            plot_a: PlotSpec::new("El Prado", 4.0),
            plot_b: PlotSpec::new("La Loma", 3.0),
            other_farm_plot: PlotSpec::new("Ajena", 2.0),
        }
    }
}

/// Insert the two farms, a season for each, and the three plots, and return
/// their ids.
///
/// Writes core rows only, so it runs on any connection whose schema starts with
/// core's migrations — which is every module's `open_in_memory()`. The audit
/// actor is `None`: a fixture is not a user, and a test that cares about the
/// actor stamp is testing its own write, not this one.
pub fn farm_with_plots(conn: &mut Connection, spec: FarmWithPlots) -> CoreFixture {
    let farm = |conn: &mut Connection, name: String| {
        core_repo::insert_farm(
            conn,
            NewFarm {
                name,
                owner_name: None,
                owner_tax_id: None,
                country_code: "es".into(),
                es: None,
                ..NewFarm::default()
            },
            None,
        )
        .unwrap()
        .id
    };
    let farm_id = farm(conn, spec.farm_name);
    let other_farm_id = farm(conn, spec.other_farm_name);

    let season = |conn: &mut Connection, farm_id: &str| {
        core_repo::insert_season(
            conn,
            NewSeason {
                farm_id: farm_id.to_string(),
                starts_on: spec.season_starts_on.clone(),
                ends_on: spec.season_ends_on.clone(),
                custom_label: None,
            },
            None,
        )
        .unwrap()
        .id
    };
    let season_id = season(conn, &farm_id);
    let other_season_id = season(conn, &other_farm_id);

    let plot = |conn: &mut Connection, farm_id: &str, spec: PlotSpec| {
        core_repo::insert_plot(
            conn,
            NewPlot {
                farm_id: farm_id.to_string(),
                name: spec.name,
                area_ha: Some(spec.area_ha),
                es: None,
            },
            None,
        )
        .unwrap()
        .id
    };
    let plot_a = plot(conn, &farm_id, spec.plot_a);
    let plot_b = plot(conn, &farm_id, spec.plot_b);
    let other_farm_plot = plot(conn, &other_farm_id, spec.other_farm_plot);

    CoreFixture {
        season_id,
        farm_id,
        plot_a,
        plot_b,
        other_farm_id,
        other_farm_plot,
        other_season_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two properties every consumer relies on: both plots sit on the farm
    /// the fixture returns, and `other_farm_plot` does not. A fixture that got
    /// this wrong would turn every `PlotNotOnFarm` test green for the wrong
    /// reason.
    #[test]
    fn plots_a_and_b_are_on_the_returned_farm_and_the_other_one_is_not() {
        let mut conn = terrazgo_core::open_in_memory().unwrap();
        let fx = farm_with_plots(&mut conn, FarmWithPlots::default());

        let farm_of = |plot_id: &str| {
            conn.query_row("SELECT farm_id FROM plot WHERE id = ?1", [plot_id], |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
        };
        assert_eq!(farm_of(&fx.plot_a), fx.farm_id);
        assert_eq!(farm_of(&fx.plot_b), fx.farm_id);
        assert_ne!(farm_of(&fx.other_farm_plot), fx.farm_id);
    }

    /// The same guard for seasons: each farm's season is its own, and the
    /// other farm's plot and season agree. A fixture that crossed them would
    /// turn every cross-farm test into a test of the schema's key instead.
    #[test]
    fn each_farm_has_its_own_season() {
        let mut conn = terrazgo_core::open_in_memory().unwrap();
        let fx = farm_with_plots(&mut conn, FarmWithPlots::default());

        let farm_of_season = |season_id: &str| {
            conn.query_row(
                "SELECT farm_id FROM season WHERE id = ?1",
                [season_id],
                |r| r.get::<_, String>(0),
            )
            .unwrap()
        };
        assert_eq!(farm_of_season(&fx.season_id), fx.farm_id);
        assert_eq!(farm_of_season(&fx.other_season_id), fx.other_farm_id);
    }

    #[test]
    fn names_and_areas_come_from_the_spec() {
        let mut conn = terrazgo_core::open_in_memory().unwrap();
        let fx = farm_with_plots(
            &mut conn,
            FarmWithPlots {
                farm_name: "Dehesa de Arriba".into(),
                plot_a: PlotSpec::new("Pasto Alto", 22.0),
                ..Default::default()
            },
        );

        let (name, area) = conn
            .query_row(
                "SELECT name, area_ha FROM plot WHERE id = ?1",
                [&fx.plot_a],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?)),
            )
            .unwrap();
        assert_eq!(name, "Pasto Alto");
        assert_eq!(area, 22.0);
    }
}
