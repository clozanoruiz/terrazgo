// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// What a register's columns are CALLED, for the conflict review (docs/sync.md →
// Conflicts as the person sees them).
//
// The backend compares two versions of a register generically — it works off
// the log's row images and knows nothing about what a column means — so it
// hands back lines naming a table and a column. This is the other half: the
// words. It is the only file in the app that maps schema columns to display
// keys, and it exists because a farmer choosing between two versions of a
// treatment must read "Dosis", not `dose_value`.
//
// Framework-agnostic tier: no Svelte imports (docs/frontend-conventions.md).
// It names i18n KEYS and never resolved text, the settingsTree.js arrangement —
// the caller supplies the resolver.
//
// THREE RULES, because this map is a second description of the schema and the
// hazard is drift:
//
//   * **Keys are reused, never invented.** Every label here is one the
//     register's own form already uses, so the two screens cannot disagree
//     about what a field is called.
//   * **Order is the map's order.** Objects keep their insertion order, so
//     listing a table's columns the way its form does is what makes the
//     comparison read like the record. The backend's order is alphabetical and
//     would open a treatment with its `advisor_id`.
//   * **Coverage is declared and tested.** A register in COVERED whose schema
//     grows a column this map does not name fails syncFields.test.js. A
//     register NOT in COVERED still reviews — its columns fall back to their
//     own names, which is plain rather than broken — and adding it here is how
//     it stops being plain.

/// A column whose value is a catalogue code: `code` is the `tCode` prefix that
/// turns `l_ha` into "l/ha". Without it the screen would print the code.
const coded = (key, code) => ({ key, code });

/// Columns every table carries and every register means the same thing by.
const COMMON = {
  // A version that deleted the register against one that corrected it is the
  // state most worth reviewing, so this is a line like any other.
  deleted_at: "sync.field.deleted_at",
  // Merging two books moves records between them, so two versions can file a
  // record in different books — one of them perhaps a book that is gone.
  season_id: "sync.field.season_id",
};

/// Table → column → label key (or `coded(...)`), in the order the register's
/// own form asks for them.
export const SYNC_FIELDS = {
  // --- core: the holding -----------------------------------------------
  farm: {
    name: "farm.name",
    owner_name: "farm.owner",
    owner_tax_id: "farm.owner_tax_id",
    location_text: "farm.location",
    address: "farm.address",
    postal_code: "farm.postal_code",
    phone_fixed: "farm.phone_fixed",
    phone_mobile: "farm.phone_mobile",
    email: "farm.email",
    opened_on: "farm.opened_on",
    latitude: "farm.latitude",
    longitude: "farm.longitude",
    country_code: coded("farm.country", "country"),
  },
  farm_es_extension: {
    rega_code: "farm.rega",
    rea_code: "farm.rea",
    siex_code: "farm.siex",
    province_code: "farm.province",
  },
  farm_representative: {
    full_name: "farm.rep_name",
    tax_id: "farm.rep_tax_id",
    representation_kind: "farm.rep_kind",
    address: "farm.rep_address",
    locality: "farm.rep_locality",
    province: "farm.rep_province",
    postal_code: "farm.rep_postal_code",
    phone: "farm.rep_phone",
    email: "farm.rep_email",
  },
  plot: {
    name: "plot.name",
    area_ha: "plot.area",
  },
  plot_es_extension: {
    sigpac_province: "plot.sigpac_province",
    sigpac_municipality: "plot.sigpac_municipality",
    sigpac_aggregate: "plot.sigpac_aggregate",
    sigpac_zone: "plot.sigpac_zone",
    sigpac_polygon: "plot.sigpac_polygon",
    sigpac_parcel: "plot.sigpac_parcel",
    sigpac_enclosure: "plot.sigpac_enclosure",
  },
  season: {
    starts_on: "season.starts",
    ends_on: "season.ends",
    custom_label: "season.custom_label",
    label: "sync.field.season_label",
    status: "sync.field.season_status",
  },
  crop: {
    plot_id: "crop.plot",
    species_name: "crop.species",
    variety: "crop.variety",
    area_ha: "crop.area_ha",
    production_system_code: coded("crop.production_system", "production_system"),
    irrigation_code: coded("crop.irrigation", "irrigation_system"),
    growing_environment_code: coded("crop.growing_environment", "growing_environment"),
    gip_system_code: coded("crop.gip_system", "gip_system"),
    crop_code: "sync.field.crop_code",
    declared_area_ha: "sync.field.declared_area",
    source: "sync.field.source",
    source_campaign: "sync.field.source_campaign",
  },
  plot_water_point: {
    denomination: "water_point.denomination",
    inside_plot: "water_point.inside_plot",
    distance_m: "water_point.distance",
    latitude: "water_point.latitude",
    longitude: "water_point.longitude",
  },

  // --- core: the catalogue ---------------------------------------------
  operator: {
    full_name: "operator.full_name",
    tax_id: "operator.tax_id",
    licence_number: "operator.licence_number",
    licence_level_code: coded("operator.licence_level", "licence_level"),
    licence_expiry_date: "operator.licence_expiry",
  },
  machinery: {
    name: "machinery.name",
    type: "machinery.kind",
    acquired_on: "machinery.acquired_on",
    last_inspection_date: "machinery.last_inspection",
    next_inspection_due_date: "machinery.next_inspection",
  },
  machinery_es_extension: {
    roma_number: "machinery.roma",
    reganip_number: "machinery.reganip",
  },
  premises: {
    name: "premises.name",
    kind_code: coded("premises.kind", "premises_kind"),
    address: "premises.address",
    vehicle_model: "premises.vehicle_model",
    plate: "premises.plate",
    class_code: "premises.class",
    volume_m3: "premises.volume",
    notes: "premises.notes",
  },
  premises_es_extension: {
    cadastral_reference: "premises.cadastral_reference",
    rea_installation_code: "premises.rea_installation_code",
  },
  advisor: {
    name: "advisor.name",
    tax_id: "advisor.tax_id",
    registration_number: "advisor.registration_number",
  },
  farm_advisor: {
    advisor_id: "sync.field.advisor",
    gip_system_code: coded("advisor.gip_system", "gip_system"),
  },
  product: {
    commercial_name: "product.name",
    holder: "product.holder",
    formulation_type_code: coded("product.formulation", "formulation_type"),
    default_phi_days: "product.phi_days",
  },
  fertiliser_material: {
    name: "material.name",
    material_code: "material.kind",
    material_detail_code: "material.detail",
    supplier_name: "material.supplier_name",
    supplier_rega: "material.supplier_rega",
    supplier_tax_id: "material.supplier_tax_id",
    supplier_nima: "material.supplier_nima",
    manure_treatment_code: "material.manure_treatment",
    density_kg_l: "material.density",
    notes: "treatment.notes",
  },

  // --- core: the devices and the people --------------------------------
  user_profile: {
    display_name: "profile.display_name",
    operator_id: "profile.operator_link",
  },
  sync_peer: {
    label: "sync.peer_label",
  },

  // --- the record book: sowing and harvest ------------------------------
  sowing_record: {
    kind_code: coded("sowing.kind", "sowing_kind"),
    sown_on: "sowing.sown_on",
    sowing_end_date: "sowing.sowing_end_date",
    flooded_on: "sowing.flooded_on",
    seed_quantity_kg: "sowing.seed_quantity",
    notes: "treatment.notes",
  },
  sowing_plot: {
    plot_id: "crop.plot",
    crop_id: "sowing.crop",
    crop_name_snapshot: "crop.species",
    variety_snapshot: "crop.variety",
  },
  harvest_record: {
    harvested_on: "harvest.harvested_on",
    product_name: "harvest.product",
    // The form fills the name and the catalogue code from one picker, so it
    // has one label for both; side by side they would read as one field
    // twice, the second printing a bare code. The crop's code is named the
    // same way.
    plant_product_code: "sync.field.product_code",
    quantity_value: "harvest.quantity",
    quantity_unit_code: coded("treatment.total_quantity_unit", "unit"),
    delivery_note_ref: "harvest.delivery_note",
    lot_number: "harvest.lot",
    buyer_name: "harvest.buyer_name",
    buyer_tax_id: "harvest.buyer_tax_id",
    buyer_address: "harvest.buyer_address",
    buyer_registry_number: "harvest.buyer_registry",
    notes: "treatment.notes",
  },
  harvest_plot: {
    plot_id: "crop.plot",
    crop_id: "sowing.crop",
    crop_name_snapshot: "crop.species",
    variety_snapshot: "crop.variety",
  },

  // --- the record book: phytosanitary -----------------------------------
  treatment_record: {
    application_date: "treatment.date",
    application_end_date: "treatment.end_date",
    application_time: "treatment.time",
    drying_date: "treatment.drying_date",
    product_id: "treatment.product",
    product_name_snapshot: "treatment.product",
    dose_value: "treatment.dose",
    dose_unit_code: coded("treatment.unit", "unit"),
    total_quantity_value: "treatment.total_quantity",
    total_quantity_unit_code: coded("treatment.total_quantity_unit", "unit"),
    target_organism: "treatment.target",
    efficacy_code: coded("treatment.efficacy", "efficacy"),
    operator_id: "treatment.operator",
    operator_name_snapshot: "treatment.operator",
    operator_licence_snapshot: "operator.licence_number",
    machinery_id: "treatment.machinery",
    machinery_roma_snapshot: "machinery.roma",
    machinery_reganip_snapshot: "machinery.reganip",
    advisor_id: "treatment.advisor",
    advisor_name_snapshot: "treatment.advisor",
    advisor_registration_snapshot: "advisor.registration_number",
    measure_code: "treatment.measure",
    measure_intensity_value: "treatment.measure_intensity",
    measure_intensity_unit_code: coded("treatment.measure_intensity_unit", "unit"),
    measure_registration_number: "treatment.measure_registration",
    measure_basic_substance_code: "treatment.measure_basic_substance",
    phi_days_used: "treatment.phi_days",
    phi_end_date: "treatment.phi_until",
    authorisation_number_snapshot: "product.auth_number",
    active_substances_snapshot: "product.substances",
    country_code: coded("farm.country", "country"),
    notes: "treatment.notes",
  },
  treatment_plot: {
    plot_id: "crop.plot",
    crop_id: "treatment.crop",
    surface_treated_ha: "treatment.surface",
    growth_stage_code: "treatment.growth_stage",
    crop_name_snapshot: "crop.species",
    variety_snapshot: "crop.variety",
  },
  treatment_problem: {
    problem_code: "treatment.problem",
    reason_category_code: coded("treatment.reason", "reason_category"),
  },
  treatment_justification: {
    justification_code: coded("sync.field.justification", "justification"),
  },
  seed_treatment: {
    sown_on: "sowing.sown_on",
    species_name: "crop.species",
    variety: "crop.variety",
    crop_code: "sync.field.crop_code",
    seed_quantity_kg: "sowing.seed_quantity",
    seed_lot: "harvest.lot",
    treatment_kind_code: "treatment.measure",
    acquired_on: "machinery.acquired_on",
    sowing_record_id: "sync.field.sowing",
    product_name: "treatment.product",
    product_registration_number: "product.auth_number",
    product_active_substance: "product.substances",
    product_id: "treatment.product",
    efficacy_code: coded("treatment.efficacy", "efficacy"),
    notes: "treatment.notes",
  },
  seed_treatment_plot: {
    plot_id: "crop.plot",
    surface_sown_ha: "treatment.surface",
  },
  non_field_treatment: {
    treated_on: "treatment.date",
    subject_kind_code: coded("sync.field.subject_kind", "non_field_subject_kind"),
    subject_description: "sync.field.subject",
    subject_product_code: "harvest.product",
    premises_id: "sync.field.premises",
    treated_quantity_value: "treatment.total_quantity",
    treated_quantity_unit_code: coded("treatment.total_quantity_unit", "unit"),
    product_id: "treatment.product",
    product_name_snapshot: "treatment.product",
    product_quantity_value: "treatment.dose",
    product_quantity_unit_code: coded("treatment.unit", "unit"),
    operator_id: "treatment.operator",
    operator_name_snapshot: "treatment.operator",
    operator_licence_snapshot: "operator.licence_number",
    machinery_id: "treatment.machinery",
    machinery_roma_snapshot: "machinery.roma",
    machinery_reganip_snapshot: "machinery.reganip",
    advisor_id: "treatment.advisor",
    advisor_name_snapshot: "treatment.advisor",
    advisor_registration_snapshot: "advisor.registration_number",
    efficacy_code: coded("treatment.efficacy", "efficacy"),
    authorisation_number_snapshot: "product.auth_number",
    country_code: coded("farm.country", "country"),
    notes: "treatment.notes",
  },
  non_field_treatment_problem: {
    problem_code: "treatment.problem",
    reason_category_code: coded("treatment.reason", "reason_category"),
  },
  non_field_treatment_justification: {
    justification_code: coded("sync.field.justification", "justification"),
  },
  analysis_record: {
    sampled_on: "analysis.sampled_on",
    material_kind_code: coded("analysis.material", "analysis_material"),
    bulletin_number: "analysis.bulletin",
    lab_name: "analysis.lab_name",
    lab_address: "analysis.lab_address",
    lab_tax_id: "analysis.lab_tax_id",
    substances_detected: "analysis.substances",
    soil_ph: "analysis.soil_ph",
    soil_organic_matter_pct: "analysis.soil_organic_matter",
    soil_available_p_mg_kg: "analysis.soil_p",
    soil_available_k_mg_kg: "analysis.soil_k",
    soil_total_n_pct: "analysis.soil_n",
    soil_conductivity_ds_m: "analysis.soil_conductivity",
    soil_sand_pct: "analysis.soil_sand",
    soil_silt_pct: "analysis.soil_silt",
    soil_clay_pct: "analysis.soil_clay",
    notes: "treatment.notes",
  },
  analysis_plot: {
    plot_id: "crop.plot",
    crop_id: "sowing.crop",
    crop_name_snapshot: "crop.species",
    variety_snapshot: "crop.variety",
  },
  analysis_record_type: {
    analysis_type_code: coded("analysis.types", "analysis_type"),
  },
  analysis_substance: {
    substance_code: "analysis.substance",
  },

  // --- the record book: fertilisation and irrigation --------------------
  fertilisation_record: {
    applied_on: "fertilisation.applied_on",
    application_end_date: "fertilisation.end_date",
    fertilisation_type_code: coded("fertilisation.type", "fertilisation_type"),
    application_method_code: coded("fertilisation.method", "application_method"),
    fertiliser_material_id: "fertilisation.material",
    material_name_snapshot: "fertilisation.material",
    material_code_snapshot: "material.kind",
    richness_n_snapshot: "sync.field.richness_n",
    richness_p2o5_snapshot: "sync.field.richness_p2o5",
    richness_k2o_snapshot: "sync.field.richness_k2o",
    dose_value: "fertilisation.dose",
    dose_unit_code: coded("fertilisation.dose_unit", "unit"),
    sludge_application: "fertilisation.sludge",
    machinery_id: "fertilisation.machinery",
    sustainable_input_management: "fertilisation.sustainable_inputs",
    irrigation_record_id: "fertilisation.irrigation_link",
    service_company: "fertilisation.service_company",
    service_regfer_number: "fertilisation.service_regfer",
    delivery_note_ref: "fertilisation.delivery_note",
    yield_estimated_kg_ha: "fertilisation.yield_estimated",
    yield_final_kg_ha: "fertilisation.yield_final",
    notes: "treatment.notes",
  },
  fertilisation_plot: {
    plot_id: "crop.plot",
    crop_id: "treatment.crop",
    fertilised_area_ha: "fertilisation.area",
  },
  // A practice is a country-scoped lookup whose names come with the lookup
  // rather than from the dictionaries, so the code prints as it is stored.
  fertilisation_practice: {
    practice_code: "fertilisation.practices_section",
  },
  fertilisation_plan: {
    drawn_up_on: "plan.drawn_up_on",
    expected_yield_kg_ha: "plan.expected_yield",
    preceding_crop_code: "plan.preceding_crop",
    notes: "treatment.notes",
    tool_generated: "plan.tool_generated",
    needs_n_kg_ha: "plan.needs_n",
    needs_p2o5_kg_ha: "plan.needs_p2o5",
    needs_k2o_kg_ha: "plan.needs_k2o",
  },
  fertilisation_plan_crop: {
    crop_id: "plan.crops",
  },
  irrigation_record: {
    irrigated_on: "irrigation.irrigated_on",
    irrigation_end_date: "irrigation.end_date",
    irrigation_method_code: coded("irrigation.method", "irrigation_method"),
    volume_value: "irrigation.volume",
    volume_unit_code: coded("irrigation.volume_unit", "unit"),
    water_nitric_n_mg_l: "irrigation.nitric_n",
    water_soluble_p2o5_mg_l: "irrigation.soluble_p2o5",
    energy_type_code: "sync.field.energy_type",
    meter_number: "irrigation.meter_number",
    notes: "treatment.notes",
  },
  irrigation_plot: {
    plot_id: "crop.plot",
    crop_id: "treatment.crop",
    irrigated_area_ha: "irrigation.area",
  },
  irrigation_water_origin: {
    origin_code: coded("column.origin", "water_origin"),
  },
  // As `fertilisation_practice`: the code prints as it is stored.
  irrigation_practice: {
    practice_code: "irrigation.practices_section",
  },

  // --- the record book: eco-schemes -------------------------------------
  grazing_record: {
    practice_code: coded("grazing.practice", "eco_practice"),
    started_on: "grazing.started_on",
    ended_on: "grazing.ended_on",
    plot_group_ref: "grazing.plot_group_ref",
    soil_cover_id: "sync.field.soil_cover",
    notes: "treatment.notes",
  },
  grazing_plot: {
    plot_id: "crop.plot",
  },
  grazing_animal: {
    species_code: "grazing.species",
    rega_code: "grazing.rega",
    animal_count: "grazing.animal_count",
  },
  cultural_operation: {
    practice_code: coded("operation.practice", "eco_practice"),
    operation_kind_code: coded("operation.kind", "cultural_operation_kind"),
    performed_on: "operation.performed_on",
    performed_end_date: "operation.performed_end_date",
    activity_description: "operation.activity_description",
    residue_destination_code: "operation.residue_destination",
    soil_cover_id: "sync.field.soil_cover",
    notes: "treatment.notes",
  },
  cultural_operation_plot: {
    plot_id: "crop.plot",
  },
  soil_cover: {
    practice_code: coded("cover.practice", "eco_practice"),
    cover_type_code: "cover.type",
    established_on: "cover.established_on",
    width_m: "cover.width_m",
    free_canopy_width_m: "cover.free_canopy_width_m",
    widths_stated_on: "cover.widths_stated_on",
    notes: "treatment.notes",
  },
  soil_cover_plot: {
    plot_id: "crop.plot",
  },
};

/// The registers this map claims to cover completely.
///
/// Read by the contract test: a table listed here whose schema grows a column
/// the map does not name is a failure, not a silent fallback. Every table in
/// SYNC_FIELDS is covered by definition — the list IS its keys — and the
/// constant exists so the test and the app name the same thing.
export const COVERED = Object.keys(SYNC_FIELDS);

/// How to render one column: `{ key, code }`, where `key` is the i18n key for
/// its label and `code` the `tCode` prefix its value needs, if any.
///
/// A column the map does not name falls back to itself. That is deliberately
/// not an error: a register nobody has described here is still reviewable, and
/// a farmer reading `meter_number` can still tell two versions apart.
export function fieldOf(table, column) {
  const entry = SYNC_FIELDS[table]?.[column] ?? COMMON[column];
  if (!entry) return { key: null, code: null, column };
  if (typeof entry === "string") return { key: entry, code: null, column };
  return { key: entry.key, code: entry.code ?? null, column };
}

/// Where a column sits in its register's reading order; unnamed columns sort
/// after the named ones, alphabetically among themselves (the order the
/// backend produced them in).
function columnRank(table, column) {
  const columns = Object.keys(SYNC_FIELDS[table] ?? {});
  const at = columns.indexOf(column);
  return at === -1 ? columns.length : at;
}

/// The review's lines in the order a person reads the record: the register's
/// own fields first in form order, then each child row, grouped.
///
/// The backend orders by table and id, which is stable but alphabetical — it
/// would open a treatment with `advisor_id` and put the dose two thirds down.
/// Sorting here rather than there is the same split as the labels: the schema
/// is the backend's, the reading order is the screen's.
export function orderedLines(lines) {
  return [...lines]
    .map((line, at) => ({ line, at }))
    .sort((left, right) => {
      const rootFirst = Number(right.line.root) - Number(left.line.root);
      if (rootFirst) return rootFirst;
      if (left.line.table !== right.line.table) {
        return left.line.table.localeCompare(right.line.table);
      }
      if (left.line.entity_id !== right.line.entity_id) {
        return left.line.entity_id.localeCompare(right.line.entity_id);
      }
      const byColumn =
        columnRank(left.line.table, left.line.column) -
        columnRank(right.line.table, right.line.column);
      return byColumn || left.at - right.at;
    })
    .map(({ line }) => line);
}

/// Two records' lines as a person reads them side by side: in reading order,
/// each with the text every side prints (`render(line)` → one string per side)
/// and whether that text differs.
///
/// Built for the duplicate review, which shows every stated field rather than
/// only the differing ones, and so meets two things the conflict review never
/// does:
///
///   * **A line that repeats the one before it is dropped.** A reference and
///     the name frozen beside it — `product_id` and `product_name_snapshot` —
///     both read "Producto: Decis". Dropped only when it is the same row, the
///     same label and the same text on every side, so a product renamed since
///     the record froze its name still shows both.
///   * **"Differs" is judged on the text, not on the stored value.** Two phones
///     that each added one product from the catalogue hold two product rows, so
///     the two records name two ids that print the same name. That is exactly
///     the case the rules are written around, and marking it as a difference
///     would point the person at the one thing that does not matter.
export function readableLines(lines, render) {
  const readable = [];
  for (const line of orderedLines(lines)) {
    const shown = render(line);
    const previous = readable.at(-1);
    const key = fieldOf(line.table, line.column).key;
    const repeats =
      previous &&
      key !== null &&
      !startsRow(line, previous.line) &&
      fieldOf(previous.line.table, previous.line.column).key === key &&
      shown.every((text, side) => text === previous.shown[side]);
    if (repeats) continue;
    readable.push({ line, shown, differs: shown.some((text) => text !== shown[0]) });
  }
  return readable;
}

/// Whether two consecutive lines are about different rows — what the screen
/// draws a separator on, so a child row reads as a block rather than as more
/// fields of the register.
export function startsRow(line, previous) {
  return !previous || previous.table !== line.table || previous.entity_id !== line.entity_id;
}
