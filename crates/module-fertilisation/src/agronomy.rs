// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The arithmetic model section 7.1 is assembled from.
//!
//! The printed plan de abonado table shows, per application, the unidades
//! fertilizantes **aportadas** and **acumuladas** beside the **recomendadas**
//! the plan states. Only the recommendation is stored: the other two are sums
//! over section 6's own records, and a stored copy is a second number that can
//! disagree with the first.
//!
//! A unidad fertilizante is kg/ha of N, of P₂O₅ or of K₂O (the model's footnote
//! 2), so it is the dose expressed in kilograms of material per hectare times
//! the material's richness in that nutrient. The catch is the dose unit: Anexo
//! III C.j allows the material to be dosed by volume, and a cubic metre of
//! slurry only becomes kilograms if the material states a density.

/// A fertiliser dose expressed in kilograms of material per hectare, which is
/// what a unidad fertilizante is computed from.
///
/// `None` when the conversion is not knowable: a dose given by volume (l/ha or
/// m³/ha) needs the material's density, and a material that does not state one
/// cannot be converted. **Not zero, and not a guess** — an assumed density of 1
/// would understate a slurry by whatever the real figure is, in the very number
/// a farmer compares against the recommendation.
pub fn dose_as_kg_per_ha(value: f64, unit_code: &str, density_kg_l: Option<f64>) -> Option<f64> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    match unit_code {
        "kg_ha" => Some(value),
        // One tonne is a thousand kilograms — a definition, not a measurement.
        "t_ha" => Some(value * 1000.0),
        // Density is kg per litre, so litres convert directly and cubic metres
        // are a thousand litres.
        "l_ha" => density_kg_l
            .filter(|d| d.is_finite() && *d > 0.0)
            .map(|d| value * d),
        "m3_ha" => density_kg_l
            .filter(|d| d.is_finite() && *d > 0.0)
            .map(|d| value * 1000.0 * d),
        _ => None,
    }
}

/// The unidades fertilizantes one application supplies of one nutrient: kg/ha
/// of material times its richness, which the label states as a percentage of
/// the material.
///
/// `None` propagates from an unconvertible dose or an unstated richness, and it
/// means "not known" rather than "none" — the two are different claims, and the
/// second would quietly lower a nitrogen total.
pub fn nutrient_units(dose_kg_ha: Option<f64>, richness_percent: Option<f64>) -> Option<f64> {
    let dose = dose_kg_ha?;
    let richness = richness_percent?;
    (dose.is_finite() && richness.is_finite()).then_some(dose * richness / 100.0)
}

/// A running total of unidades fertilizantes down section 7.1's table, for one
/// production unit.
///
/// **An unknown contribution stops the total rather than being skipped.** Once
/// one application's figure cannot be computed, every later accumulated cell is
/// blank: a total that silently omitted a slurry application would read as the
/// nitrogen already applied, and a farmer comparing it against the
/// recommendation would over-fertilise on the strength of it. Blank says "the
/// app cannot tell you" — and stating the material's density fixes it.
#[derive(Debug, Default, Clone)]
pub struct Accumulator {
    total: f64,
    broken: bool,
}

impl Accumulator {
    /// Add one application's contribution and return the total so far, or
    /// `None` once the series has been broken by an unknown.
    pub fn add(&mut self, contribution: Option<f64>) -> Option<f64> {
        match contribution {
            Some(value) if !self.broken => {
                self.total += value;
                Some(self.total)
            }
            Some(_) => None,
            None => {
                self.broken = true;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rounded comparison: these are decimal percentages of decimal doses, so
    /// exact float equality would pin binary noise rather than agronomy.
    fn close(actual: Option<f64>, expected: f64) {
        let actual = actual.expect("a figure was expected");
        assert!(
            (actual - expected).abs() < 1e-9,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn a_dose_already_in_kilograms_needs_no_conversion() {
        close(dose_as_kg_per_ha(250.0, "kg_ha", None), 250.0);
    }

    #[test]
    fn tonnes_convert_without_a_density_because_that_is_a_definition() {
        close(dose_as_kg_per_ha(2.5, "t_ha", None), 2500.0);
    }

    #[test]
    fn volumes_convert_only_when_the_material_states_a_density() {
        // Pig slurry at 1,02 kg/L: 25 m³/ha is 25 500 kg/ha.
        close(dose_as_kg_per_ha(25.0, "m3_ha", Some(1.02)), 25_500.0);
        close(dose_as_kg_per_ha(300.0, "l_ha", Some(1.3)), 390.0);
        // Without one, the app does not know — and must not assume 1,0.
        assert_eq!(dose_as_kg_per_ha(25.0, "m3_ha", None), None);
        assert_eq!(dose_as_kg_per_ha(300.0, "l_ha", None), None);
        // A density that is not a measurement is no better than none.
        assert_eq!(dose_as_kg_per_ha(25.0, "m3_ha", Some(0.0)), None);
        assert_eq!(dose_as_kg_per_ha(25.0, "m3_ha", Some(f64::NAN)), None);
    }

    #[test]
    fn a_unit_this_module_does_not_dose_in_converts_to_nothing() {
        // The dose column is narrowed to the four rates, but a stored value
        // from elsewhere must not be silently treated as kilograms.
        assert_eq!(dose_as_kg_per_ha(250.0, "kg", None), None);
        assert_eq!(dose_as_kg_per_ha(250.0, "m3", None), None);
    }

    #[test]
    fn unidades_fertilizantes_are_the_dose_times_the_richness() {
        // 250 kg/ha of a 27 % nitrogen fertiliser supplies 67,5 UF N/ha —
        // the model's footnote 2 defines a UF as kg/ha of the nutrient.
        close(nutrient_units(Some(250.0), Some(27.0)), 67.5);
        // Pig slurry: 25 m³/ha at 1,02 kg/L and 0,42 % N is 107,1 UF N/ha.
        let dose = dose_as_kg_per_ha(25.0, "m3_ha", Some(1.02));
        close(nutrient_units(dose, Some(0.42)), 107.1);
    }

    #[test]
    fn an_unstated_richness_is_unknown_rather_than_zero() {
        // A label that says nothing about potassium has not said there is none,
        // and a zero here would quietly lower every total below it.
        assert_eq!(nutrient_units(Some(250.0), None), None);
        assert_eq!(nutrient_units(None, Some(27.0)), None);
        // A stated zero, on the other hand, IS a statement and contributes.
        close(nutrient_units(Some(250.0), Some(0.0)), 0.0);
    }

    #[test]
    fn the_running_total_adds_what_is_known() {
        let mut acc = Accumulator::default();
        close(acc.add(Some(67.5)), 67.5);
        close(acc.add(Some(32.5)), 100.0);
        close(acc.add(Some(0.0)), 100.0);
    }

    #[test]
    fn one_unknown_contribution_stops_the_total_for_good() {
        // A slurry application whose density is missing contributes an unknown
        // amount of nitrogen. Skipping it would print a total that reads as
        // "this much has been applied" while being short by that application —
        // and a farmer comparing it against the recommendation would
        // over-fertilise. Blank from there on says the app cannot tell them.
        let mut acc = Accumulator::default();
        close(acc.add(Some(67.5)), 67.5);
        assert_eq!(acc.add(None), None, "the unknown row itself is blank");
        assert_eq!(
            acc.add(Some(20.0)),
            None,
            "and every later row stays blank — the total is no longer known"
        );
    }
}
