// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! What the record book is missing — reported, never enforced.
//!
//! The printed book has NO precheck gate on purpose: a farmer must be able to
//! print for an inspection while some registry data is still incomplete (see
//! this crate's module docs). That argues against *blocking*, not against
//! *telling*, and nothing told: `export_precheck` serves the parked SIEX
//! export, so the artifact that actually carries legal weight had no
//! completeness check at all.
//!
//! Everything here is therefore advisory. It reports what a binding field list
//! asks for and the book prints blank, and it never refuses anything.
//!
//! The advisory lives in this crate rather than beside the SIEX precheck
//! because it reads the whole book — core, module-cue and module-fertilisation
//! — and modules may not read each other. Shared presentation belongs in a
//! consumer crate above them (the placement rule of the recordbook extraction).

use crate::error::Result;
use rusqlite::Connection;
use serde::Serialize;

/// A treatment the advisory points at, with enough to render a list row.
#[derive(Debug, Clone, Serialize)]
pub struct TreatmentRef {
    pub treatment_record_id: String,
    pub application_date: String,
    /// `None` for a purely non-chemical actuation, which names no product.
    pub product_name: Option<String>,
}

/// A treated plot whose crop is unknown: Anexo III Parte I B.e asks for the
/// "cultivo, indicando especie y variedad" of every treatment.
#[derive(Debug, Clone, Serialize)]
pub struct TreatedPlotRef {
    pub treatment_record_id: String,
    pub application_date: String,
    pub plot_id: String,
    pub plot_name: String,
}

/// An applicator with no licence number to print in table 1.2.
#[derive(Debug, Clone, Serialize)]
pub struct OperatorRef {
    pub operator_id: String,
    pub full_name: String,
}

/// How RD 1051/2022 art. 4.1's exemption looks on the figures the app holds.
///
/// Deliberately not a verdict of "exempt". The exemption is narrower than a
/// size test — art. 4.1 carves it back for pastures that ARE fertilised and
/// for more than 0,1 ha under glass — and a holding that records nothing is
/// exactly the holding whose fertilising the app cannot see. So the strongest
/// honest statement is "nothing here contradicts the exemption", which is what
/// [`Duty::PossiblyExempt`] says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Duty {
    /// Over a threshold, or carved back by the greenhouse rule.
    Binding,
    /// Under both thresholds on what the app knows, with nothing carving it
    /// back. The farmer still decides — see the type's own note.
    PossiblyExempt,
    /// A plot has no known land use or no area, so the totals cannot be
    /// trusted. Reported as such rather than guessed either way.
    Undetermined,
}

/// A binding section of the book with no records this campaign, and the figures
/// that decide whether that is a gap or an exemption.
#[derive(Debug, Clone, Serialize)]
pub struct SectionGap {
    pub duty: Duty,
    /// Art. 4.1.a's first threshold: permanent crops + arable land, pastures
    /// excluded. 5 ha.
    pub arable_permanent_ha: f64,
    /// Art. 4.1.a's second threshold: 1 ha.
    pub irrigated_ha: f64,
    /// Art. 4.1's carve-back: more than 0,1 ha under cover must be recorded
    /// even by an otherwise exempt holding.
    pub greenhouse_ha: f64,
    /// Why the totals could not be trusted, when they could not.
    pub plots_without_land_use: usize,
    pub plots_without_area: usize,
}

/// Everything the book is missing. Every field is advisory; nothing here
/// prevents a print or an export.
#[derive(Debug, Clone, Serialize)]
pub struct BookAdvisory {
    /// Anexo III Parte I A.1.a-b: the holding's address, and the holder's name
    /// and tax id. Printed blank in a binding section when absent.
    pub farm_missing_fields: Vec<&'static str>,
    /// Anexo III Parte I B.e.
    pub treatments_missing_crop: Vec<TreatedPlotRef>,
    /// Anexo III Parte I B.j ("valoración de la eficacia del tratamiento").
    /// Observed after the application, which is why it is advisory here and
    /// nullable in the schema.
    pub treatments_missing_efficacy: Vec<TreatmentRef>,
    /// Table 1.2's "Nº inscripción ROPO / nº carné", asked of every applicator
    /// a record names (B.d).
    pub operators_missing_licence: Vec<OperatorRef>,
    /// Conditional registers (3.2-3.5) that hold no records AND carry no
    /// stated "APLICA TRATAMIENTO: NO", so the book prints neither a row nor a
    /// tick — the one state of the three that says nothing at all.
    pub registers_undeclared: Vec<String>,
    /// Model section 6, RD 1051/2022 art. 5.d. `None` when the campaign holds
    /// fertilisation records.
    pub fertilisation_absent: Option<SectionGap>,
    /// Model section 8, art. 5.e — the same duty, in the same article.
    pub irrigation_absent: Option<SectionGap>,
}

impl BookAdvisory {
    /// Whether the book has nothing outstanding. Callers render the findings;
    /// none of them may block a print.
    pub fn is_clean(&self) -> bool {
        self.farm_missing_fields.is_empty()
            && self.treatments_missing_crop.is_empty()
            && self.treatments_missing_efficacy.is_empty()
            && self.operators_missing_licence.is_empty()
            && self.registers_undeclared.is_empty()
            && self.fertilisation_absent.is_none()
            && self.irrigation_absent.is_none()
    }
}

/// What one plot contributes to the art. 4.1 thresholds.
#[derive(Debug, Clone, Default)]
pub(crate) struct PlotFacts {
    pub area_ha: Option<f64>,
    /// SIGPAC land use of the provider boundary (`TA`, `OV`, `PS`…). `None`
    /// when the plot was never verified against SIGPAC.
    pub land_use: Option<String>,
    /// Any crop on it this campaign states an irrigation system other than
    /// rainfed.
    pub irrigated: bool,
    /// Any crop on it grows under glass or plastic, or SIGPAC calls the plot
    /// itself a greenhouse.
    pub greenhouse: bool,
}

/// SIGPAC uses that are "tierras de cultivo" for art. 4.1.a.
const ARABLE_USES: &[&str] = &["TA", "TH", "IV"];

/// SIGPAC uses that are "cultivos permanentes" for art. 4.1.a, associations
/// included — an olivar-viñedo plot is two permanent crops, not neither.
const PERMANENT_USES: &[&str] = &[
    "CI", "CF", "CS", "CV", "FF", "FL", "FS", "FV", "FY", "OC", "OF", "OP", "OV", "VF", "VI", "VO",
];

/// Whether a SIGPAC use counts toward art. 4.1.a's 5 ha.
///
/// Pastures (`PA`, `PR`, `PS`) do not: the article counts "cultivos permanentes
/// y tierras de cultivo, **excluidos los pastos temporales**", and permanent
/// pasture is neither of those two categories to begin with. Non-agricultural
/// uses (water, roads, buildings, forest, scrub, unproductive) count for
/// nothing either.
///
/// A temporary pasture sown on arable land is counted here, because SIGPAC
/// calls that plot `TA` and nothing in our data says what is growing on it.
/// That errs toward reporting the duty, which is the safe direction for an
/// advisory: it invites a farmer to check a rule that may not apply to them,
/// rather than assuring one who is bound that they are not.
fn counts_toward_threshold(land_use: &str) -> bool {
    let code = land_use.trim().to_ascii_uppercase();
    ARABLE_USES.contains(&code.as_str()) || PERMANENT_USES.contains(&code.as_str())
}

fn is_greenhouse_use(land_use: &str) -> bool {
    land_use.trim().eq_ignore_ascii_case("IV")
}

/// RD 1051/2022 art. 4.1, on the figures the app holds.
///
/// > a) Sobre el total de su superficie de cultivos permanentes y tierras de
/// > cultivo, excluidos los pastos temporales, cuenten con una superficie menor
/// > o igual a 5 hectáreas, siempre y cuando tengan una superficie de regadío
/// > menor o igual a 1 hectárea
///
/// and, for holdings exempt under a):
///
/// > 2.º Invernaderos con superficie total bajo cubierta superior a 0,1 ha.
/// > deberán anotar exclusivamente en el cuaderno de explotación la información
/// > relativa a esas superficies.
///
/// The greenhouse clause is checked BEFORE the thresholds, because it survives
/// the exemption: an otherwise exempt holding with 0,2 ha under plastic still
/// records for that surface, so telling it "possibly exempt" would be wrong.
pub(crate) fn nutrient_duty(plots: &[PlotFacts]) -> SectionGap {
    let mut arable_permanent_ha = 0.0;
    let mut irrigated_ha = 0.0;
    let mut greenhouse_ha = 0.0;
    let mut plots_without_land_use = 0;
    let mut plots_without_area = 0;

    for plot in plots {
        let Some(area) = plot.area_ha else {
            plots_without_area += 1;
            continue;
        };
        let Some(land_use) = plot.land_use.as_deref() else {
            plots_without_land_use += 1;
            continue;
        };
        if counts_toward_threshold(land_use) {
            arable_permanent_ha += area;
        }
        if plot.irrigated {
            irrigated_ha += area;
        }
        if plot.greenhouse || is_greenhouse_use(land_use) {
            greenhouse_ha += area;
        }
    }

    // The carve-back first: it binds a holding the size test would excuse.
    let duty = if greenhouse_ha > 0.1 {
        Duty::Binding
    } else if plots_without_land_use > 0 || plots_without_area > 0 {
        Duty::Undetermined
    } else if arable_permanent_ha <= 5.0 && irrigated_ha <= 1.0 {
        Duty::PossiblyExempt
    } else {
        Duty::Binding
    };

    SectionGap {
        duty,
        arable_permanent_ha,
        irrigated_ha,
        greenhouse_ha,
        plots_without_land_use,
        plots_without_area,
    }
}

/// The four conditional registers, in the order the book prints them. Each
/// prints in three states — rows, a stated "NO", or neither — and only the
/// third is a finding.
const CONDITIONAL_REGISTERS: &[&str] = &[
    "seed_treatment",
    "postharvest",
    "storage_premises",
    "transport",
];

/// Read the whole book and report what it is missing.
pub fn book_advisory(conn: &Connection, season_id: &str, farm_id: &str) -> Result<BookAdvisory> {
    let farm = terrazgo_core::repository::get_farm(conn, farm_id)?;

    // Anexo III Parte I A.1.a-b. The farm NAME is NOT NULL in the schema, so
    // only the three that can actually be blank are checked.
    let mut farm_missing_fields = Vec::new();
    if is_blank(farm.farm.address.as_deref()) {
        farm_missing_fields.push("address");
    }
    if is_blank(farm.farm.owner_name.as_deref()) {
        farm_missing_fields.push("owner_name");
    }
    if is_blank(farm.farm.owner_tax_id.as_deref()) {
        farm_missing_fields.push("owner_tax_id");
    }

    let plots = terrazgo_core::repository::list_plots(conn, farm_id)?;
    let plot_name = |plot_id: &str| {
        plots
            .iter()
            .find(|p| p.plot.id == plot_id)
            .map(|p| p.plot.name.clone())
            .unwrap_or_default()
    };

    let mut treatments_missing_crop = Vec::new();
    let mut treatments_missing_efficacy = Vec::new();
    let mut operator_ids = Vec::new();
    for record in module_cue::repository::list_treatment_records(conn, season_id, farm_id)? {
        let reference = || TreatmentRef {
            treatment_record_id: record.record.id.clone(),
            application_date: record.record.application_date.clone(),
            product_name: record.record.product_name_snapshot.clone(),
        };
        if record.record.efficacy_code.is_none() {
            treatments_missing_efficacy.push(reference());
        }
        if !operator_ids.contains(&record.record.operator_id) {
            operator_ids.push(record.record.operator_id.clone());
        }
        for treated in &record.plots {
            if treated.crop_id.is_none() {
                treatments_missing_crop.push(TreatedPlotRef {
                    treatment_record_id: record.record.id.clone(),
                    application_date: record.record.application_date.clone(),
                    plot_id: treated.plot_id.clone(),
                    plot_name: plot_name(&treated.plot_id),
                });
            }
        }
    }
    for record in module_cue::repository::list_non_field_treatments(conn, season_id, farm_id)? {
        if !operator_ids.contains(&record.record.operator_id) {
            operator_ids.push(record.record.operator_id.clone());
        }
    }

    // The licence is read from the operator registry rather than the record's
    // snapshot: a number added since the treatment was written is the answer to
    // "what will table 1.2 print", which is what this advisory is about.
    let operators = terrazgo_core::repository::list_operators(conn)?;
    let operators_missing_licence = operator_ids
        .iter()
        .filter_map(|id| operators.iter().find(|o| &o.id == id))
        .filter(|operator| is_blank(operator.licence_number.as_deref()))
        .map(|operator| OperatorRef {
            operator_id: operator.id.clone(),
            full_name: operator.full_name.clone(),
        })
        .collect();

    // A register with rows, or with a stated "NO", has answered; silence has
    // not. Seed treatments and the three non-field subjects share one list of
    // register codes (`register_kind` is wider than `non_field_subject_kind`).
    let declared: Vec<String> =
        module_cue::repository::list_register_declarations(conn, farm_id, season_id)?
            .into_iter()
            .map(|d| d.register_code)
            .collect();
    let sowings = module_cue::repository::list_seed_treatments(conn, season_id, farm_id)?;
    let non_field = module_cue::repository::list_non_field_treatments(conn, season_id, farm_id)?;
    let registers_undeclared = CONDITIONAL_REGISTERS
        .iter()
        .filter(|register| !declared.iter().any(|d| d == *register))
        .filter(|register| match **register {
            "seed_treatment" => sowings.is_empty(),
            kind => !non_field
                .iter()
                .any(|record| record.record.subject_kind_code == kind),
        })
        .map(|register| (*register).to_string())
        .collect();

    // Sections 6 and 8: the duty is one article's, so one computation serves
    // both findings.
    let crops = terrazgo_core::repository::list_crops(conn, season_id, farm_id)?;
    let facts = crate::sigpac_facts(conn, farm_id)?;
    let plot_facts: Vec<PlotFacts> = plots
        .iter()
        .map(|detail| {
            let crops_here = crops.iter().filter(|c| c.plot_id == detail.plot.id);
            let mut irrigated = false;
            let mut greenhouse = false;
            for crop in crops_here {
                if crop
                    .irrigation_code
                    .as_deref()
                    .is_some_and(|code| code != "rainfed")
                {
                    irrigated = true;
                }
                if crop.growing_environment_code.as_deref() == Some("greenhouse") {
                    greenhouse = true;
                }
            }
            PlotFacts {
                area_ha: detail.plot.area_ha,
                land_use: facts.get(&detail.plot.id).and_then(|f| f.land_use.clone()),
                irrigated,
                greenhouse,
            }
        })
        .collect();

    let fertilisation_absent =
        module_fertilisation::repository::list_fertilisation_records(conn, season_id, farm_id)?
            .is_empty()
            .then(|| nutrient_duty(&plot_facts));
    let irrigation_absent =
        module_fertilisation::repository::list_irrigation_records(conn, season_id, farm_id)?
            .is_empty()
            .then(|| nutrient_duty(&plot_facts));

    Ok(BookAdvisory {
        farm_missing_fields,
        treatments_missing_crop,
        treatments_missing_efficacy,
        operators_missing_licence,
        registers_undeclared,
        fertilisation_absent,
        irrigation_absent,
    })
}

fn is_blank(value: Option<&str>) -> bool {
    value.unwrap_or("").trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Thresholds are quoted from RD 1051/2022 art. 4.1.a; the greenhouse
    /// carve-back from the same article's closing paragraph.
    fn plot(area: f64, land_use: &str) -> PlotFacts {
        PlotFacts {
            area_ha: Some(area),
            land_use: Some(land_use.into()),
            irrigated: false,
            greenhouse: false,
        }
    }

    #[test]
    fn under_both_thresholds_nothing_contradicts_the_exemption() {
        // "menor o igual a 5 hectáreas ... menor o igual a 1 hectárea": the
        // boundary itself is exempt, so exactly 5 and exactly 1 pass.
        let mut irrigated = plot(1.0, "TA");
        irrigated.irrigated = true;
        let gap = nutrient_duty(&[plot(4.0, "OV"), irrigated]);
        assert_eq!(gap.duty, Duty::PossiblyExempt);
        assert_eq!(gap.arable_permanent_ha, 5.0);
        assert_eq!(gap.irrigated_ha, 1.0);
    }

    #[test]
    fn over_the_surface_threshold_the_duty_binds() {
        let gap = nutrient_duty(&[plot(5.5, "TA")]);
        assert_eq!(gap.duty, Duty::Binding);
        assert_eq!(gap.arable_permanent_ha, 5.5);
    }

    #[test]
    fn over_the_irrigated_threshold_the_duty_binds() {
        // Small enough on surface, too much of it irrigated: art. 4.1.a joins
        // the two conditions with "siempre y cuando".
        let mut irrigated = plot(2.0, "TA");
        irrigated.irrigated = true;
        let gap = nutrient_duty(&[irrigated, plot(1.0, "VI")]);
        assert_eq!(gap.duty, Duty::Binding);
        assert_eq!(gap.irrigated_ha, 2.0);
    }

    #[test]
    fn pastures_do_not_count_toward_the_surface() {
        // "cultivos permanentes y tierras de cultivo, excluidos los pastos
        // temporales" — permanent pasture is in neither category either.
        let gap = nutrient_duty(&[plot(40.0, "PS"), plot(12.0, "PA"), plot(3.0, "TA")]);
        assert_eq!(gap.arable_permanent_ha, 3.0);
        assert_eq!(gap.duty, Duty::PossiblyExempt);
    }

    #[test]
    fn non_agricultural_uses_count_for_nothing() {
        let gap = nutrient_duty(&[plot(30.0, "FO"), plot(9.0, "MT"), plot(2.0, "AG")]);
        assert_eq!(gap.arable_permanent_ha, 0.0);
        assert_eq!(gap.duty, Duty::PossiblyExempt);
    }

    #[test]
    fn a_greenhouse_over_the_carve_back_binds_an_otherwise_exempt_holding() {
        // "Invernaderos con superficie total bajo cubierta superior a 0,1 ha.
        // deberán anotar exclusivamente ... la información relativa a esas
        // superficies" — it survives the exemption, so it is checked first.
        let mut under_glass = plot(0.2, "TA");
        under_glass.greenhouse = true;
        let gap = nutrient_duty(&[under_glass]);
        assert_eq!(gap.duty, Duty::Binding);
        assert_eq!(gap.greenhouse_ha, 0.2);
    }

    #[test]
    fn a_greenhouse_at_the_carve_back_does_not_bind() {
        // "superior a 0,1 ha" — 0,1 itself is not over it.
        let mut under_glass = plot(0.1, "TA");
        under_glass.greenhouse = true;
        let gap = nutrient_duty(&[under_glass]);
        assert_eq!(gap.duty, Duty::PossiblyExempt);
    }

    #[test]
    fn the_sigpac_greenhouse_use_counts_as_one() {
        // A plot SIGPAC itself calls "invernaderos y cultivos bajo plástico"
        // needs no crop to state it.
        let gap = nutrient_duty(&[plot(0.5, "IV")]);
        assert_eq!(gap.duty, Duty::Binding);
        assert_eq!(gap.greenhouse_ha, 0.5);
        // It is cultivated land as well, so it counts toward the 5 ha too.
        assert_eq!(gap.arable_permanent_ha, 0.5);
    }

    #[test]
    fn an_unknown_land_use_leaves_the_question_open() {
        // Never verified against SIGPAC: the totals cannot be trusted, so the
        // advisory says so instead of excusing a holding it cannot measure.
        let unknown = PlotFacts {
            area_ha: Some(1.0),
            land_use: None,
            ..PlotFacts::default()
        };
        let gap = nutrient_duty(&[plot(2.0, "TA"), unknown]);
        assert_eq!(gap.duty, Duty::Undetermined);
        assert_eq!(gap.plots_without_land_use, 1);
    }

    #[test]
    fn a_plot_without_an_area_leaves_the_question_open() {
        let no_area = PlotFacts {
            area_ha: None,
            land_use: Some("TA".into()),
            ..PlotFacts::default()
        };
        let gap = nutrient_duty(&[plot(2.0, "TA"), no_area]);
        assert_eq!(gap.duty, Duty::Undetermined);
        assert_eq!(gap.plots_without_area, 1);
    }

    #[test]
    fn a_greenhouse_binds_even_when_the_rest_is_unmeasurable() {
        // The carve-back is its own duty: it does not wait for the totals.
        let mut under_glass = plot(0.4, "TA");
        under_glass.greenhouse = true;
        let unknown = PlotFacts {
            area_ha: Some(30.0),
            land_use: None,
            ..PlotFacts::default()
        };
        assert_eq!(nutrient_duty(&[under_glass, unknown]).duty, Duty::Binding);
    }

    #[test]
    fn a_holding_with_no_plots_at_all_says_nothing_it_cannot_know() {
        // No plots, no figures: every total is zero and the thresholds are
        // trivially met. That is the honest reading — there is nothing here
        // that contradicts the exemption.
        let gap = nutrient_duty(&[]);
        assert_eq!(gap.duty, Duty::PossiblyExempt);
        assert_eq!(gap.arable_permanent_ha, 0.0);
    }
}
