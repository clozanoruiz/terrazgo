// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Reference data, fetched once per session instead of once per view.
//
// **Only lists that cannot change while the app runs.** Everything here is a
// lookup table shipped in the binary or a vendored catalogue: units, coded
// vocabularies, the model's closed lists. User data — farms, plots, operators,
// advisors, products, materials — is deliberately NOT here, even though it is
// fetched by several views: the app itself edits those, so caching them would
// buy one invalidation rule per mutating command, which is the trade
// docs/stack-choices.md §3 rejects for records.
//
// Records are never cached at all. Refetch-on-mount is the correct default for
// a legal document: what is on screen came from the database now.
//
// Measured before building (2026-08-13, real IPC on the demo book): the record
// book's cold mount fired 30 invokes, 21 of them these lists, each 1-2 ms and
// 3-10 rows. So the win is NOT speed — it is that a view stops fetching and
// re-drilling twenty props two levels deep to reach a form.
//
// ONE invalidation rule, and its owner already exists: the catalogue refresh in
// Settings, which is the only thing in the app that can change these rows.

import { invoke } from "./backend.js";

/// The command behind each list. Argument-free by construction: a list that
/// needs a country or a category is per-farm reference data, not session-wide,
/// and stays with the view that knows the argument.
const COMMANDS = {
  countries: "list_countries",
  units: "list_units",
  quantityUnits: "list_quantity_units",
  intensityUnits: "list_intensity_units",
  volumeUnits: "list_irrigation_volume_units",
  doseUnits: "list_fertiliser_dose_units",
  reasons: "list_reason_categories",
  justifications: "list_justifications",
  efficacies: "list_efficacies",
  subjectKinds: "list_non_field_subject_kinds",
  premisesKinds: "list_premises_kinds",
  sowingKinds: "list_sowing_kinds",
  seedTreatmentKinds: "list_seed_treatment_kinds",
  analysisMaterials: "list_analysis_materials",
  analysisTypes: "list_analysis_types",
  formulationTypes: "list_formulation_types",
  authorisationKinds: "list_authorisation_kinds",
  productionSystems: "list_production_systems",
  irrigationSystems: "list_irrigation_systems",
  growingEnvironments: "list_growing_environments",
  gipSystems: "list_gip_systems",
  licenceLevels: "list_licence_levels",
  irrigationMethods: "list_irrigation_methods",
  waterOrigins: "list_water_origins",
  fertilisationTypes: "list_fertilisation_types",
  applicationMethods: "list_application_methods",
  manureTreatments: "list_manure_treatments",
  nutrientKinds: "list_nutrient_kinds",
  ecoPractices: "list_eco_practices",
  culturalOperationKinds: "list_cultural_operation_kinds",
  // NOT here: list_fertiliser_material_kinds, list_problem_codes,
  // list_growth_stages, list_crop_species, list_plant_products,
  // list_premises_classes and
  // list_substance_codes all take a country (or a category), so they are
  // per-holding reference data rather than session-wide. Caching them would
  // need a key, and the view already knows the argument.
};

/// Every list, empty until loaded. Components read these directly and re-render
/// when they arrive, so a view can render its frame before the data lands —
/// which is what an empty array means here, never "the list is empty".
export const lookups = $state(Object.fromEntries(Object.keys(COMMANDS).map((k) => [k, []])));

/// Resolves when the lists are in `lookups`. Concurrent callers share one load
/// — the record book mounts six children at once and must not fetch six times.
let inFlight = null;

export function loadLookups() {
  if (!inFlight) {
    const entries = Object.entries(COMMANDS);
    inFlight = Promise.all(entries.map(([, command]) => invoke(command)))
      .then((results) => {
        // A block body, not a concise one: assigning into a `$state` proxy from
        // an expression-bodied arrow returns the right-hand side rather than the
        // stored value, which Svelte flags as `assignment_value_stale` once per
        // key. forEach discards the result either way, so this is only about
        // not emitting thirty warnings on every startup.
        entries.forEach(([name], index) => {
          lookups[name] = results[index];
        });
      })
      .catch((error) => {
        // A failed load must not be remembered as done: the next view retries.
        // The error itself surfaces through the caller's own run() wrapper.
        inFlight = null;
        throw error;
      });
  }
  return inFlight;
}

/// Re-read everything after the catalogue refresh in Settings — the only thing
/// that can change these lists while the app is running.
export function invalidateLookups() {
  inFlight = null;
  return loadLookups();
}
