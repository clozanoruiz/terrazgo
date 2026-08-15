// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// English dictionary, by area. The key set is identical in every locale and
// no key may appear in two files: i18n.js merges them.

export default {
  // Command-boundary errors (CommandError codes → error.<code>). "internal"
  // deliberately has no error.<code> entry: the raw developer message is shown,
  // preceded by the internal_intro line so regular users get some orientation.
  "error.internal_intro": "An internal error occurred:",
  "error.not_found": "Record not found.",
  "error.invalid.empty_name": "The name must not be empty.",
  "error.invalid.operator_not_found": "The selected operator no longer exists.",
  "error.invalid.empty_authorisation_number": "The registration number must not be empty.",
  "error.invalid.no_problems": "Add at least one problem treated (pest, disease…).",
  "error.invalid.no_justifications": "Add at least one justification for the treatment.",
  "error.invalid.end_date_before_start": "The end date cannot precede the start date.",
  "error.invalid.application_time": "Enter the time as HH:MM (for example 20:30).",
  "error.invalid.growth_stage_unknown":
    "The selected growth stage is not in the official catalogue.",
  "error.invalid.invalid_total_quantity":
    "Enter the total quantity and its unit (kg or l), with a value greater than zero.",
  "error.invalid.unknown_problem_code": "The selected problem is not in the official catalogue.",
  "error.invalid.export_precheck_failed":
    "The export is blocked: some data required by the official format is still missing.",
  "error.invalid.export_code_unmappable":
    "A stored code could not be converted to the official catalogues.",
  "error.invalid.missing_exceptional_substance":
    "An exceptional authorisation must name its substance (official catalogue code).",
  "error.invalid.unknown_substance_code":
    "The substance is not in the official exceptional-authorisations catalogue.",
  "error.invalid.nonpositive_area": "The area must be greater than zero.",
  "error.invalid.season_in_use":
    "A season with crops or treatments recorded cannot be deleted. Remove its contents first.",
  "error.invalid.missing_distance":
    "State the distance to the plot: it is required when the abstraction point lies outside it.",
  "error.invalid.water_point_distance_inside":
    'An abstraction point inside the plot carries no distance. Untick "inside" or clear the distance.',
  "error.invalid.water_point_coordinates_invalid":
    "Coordinates must be given in full (latitude and longitude) and within the globe's limits.",
  "error.invalid.plot_has_water_points":
    "This plot has abstraction points recorded, so it cannot be declared free of them. Delete them first.",
  "error.invalid.report_language_unknown": "That language is not available for the record book.",
  "error.invalid.cache_cap_too_small": "The space for offline maps is too small (minimum 64 MB).",
  "error.invalid_date": 'Invalid date "{date}" (expected YYYY-MM-DD).',
  "error.authorisation_missing": 'Product {product_id} is not authorised in "{country}".',
  "error.country_mismatch": 'Country "{provided}" does not match the farm\'s country ("{farm}").',
  "error.plot_not_on_farm": "Plot {plot_id} is not on farm {farm_id}.",
  "error.invalid.backup_invalid": "The selected file is not a valid Terrazgo backup.",
  "error.invalid.backup_newer_schema":
    "This backup was created by a newer version of Terrazgo — update the app first.",
  "error.missing_phi_days":
    "No pre-harvest interval available: the product has no default and none was supplied.",
  "error.geo_http": "The map service answered with an error (HTTP {status}).",
  "error.geo_offline": "No network connection — showing cached map data only. ({reason})",
  "error.invalid.geometry_invalid":
    "The geometry is not a valid boundary (a closed polygon with valid coordinates).",
  "error.invalid.geo_subject_missing": "The geometry is not attached to a plot or a farm.",
  "error.invalid.geo_subject_ambiguous": "The geometry cannot belong to two things at once.",
  "error.invalid.boundary_file_unsupported":
    "Unsupported file — use GeoJSON or GeoPackage (.gpkg).",
  "error.invalid.boundary_file_empty": "The file contains no usable boundaries (polygons).",
  "error.invalid.boundary_file_too_large":
    "The file has too many features — use a smaller extract (e.g. one municipality).",
  "error.invalid.gpkg_unsupported_srs":
    "The GeoPackage uses a projected coordinate system this version cannot read yet.",
  "error.invalid.tilejson_invalid": "The map service returned an unusable tile index.",
  "error.invalid.style_unsupported":
    "The base-map style changed upstream in a way Terrazgo does not recognise yet.",
  "error.invalid.sigpac_ref_invalid":
    "The SIGPAC reference is incomplete or not numeric — check the seven parts.",
  "error.invalid.sigpac_response_invalid": "SIGPAC answered in an unexpected format.",
  "error.invalid.sigpac_ref_missing":
    "The plot has no complete SIGPAC reference — fill in the seven parts first.",
  "error.invalid.zone_status_invalid": "Internal zone-check result was not usable.",
  "error.invalid.quantity_unit_mismatch":
    "The unit does not match what was treated: tonnes for plant produce, m\u00b3 for premises and vehicles.",
  "error.invalid.invalid_product_quantity":
    "Enter the product quantity and its unit (kg or l), with a value greater than zero.",
  "error.invalid.empty_subject": "State what was treated.",
  "error.invalid.unknown_subject_kind": "Unknown register type.",
  "error.invalid.register_has_rows":
    "It cannot be declared empty: the register already holds entries.",
  "error.invalid.empty_product_name": "Enter the product name from the label.",
  "error.invalid.no_plots": "Add at least one plot.",
  "error.invalid.invalid_seed_quantity": "The seed quantity must be greater than zero.",
  "error.invalid.unknown_seed_treatment_kind": "Choose a treatment from the list.",
  "error.invalid.unknown_analysis_material": "Choose the material analysed.",
  "error.invalid.unknown_analysis_type": "Choose a kind of analysis from the list.",
  "error.invalid.empty_buyer_name": "Enter the buyer's name or company name.",

  // Section 4 — soil parameters (Anexo III A.3).
  "error.invalid.invalid_soil_ph": "pH runs from 0 to 14.",
  "error.invalid.invalid_soil_percentage": "Percentages run from 0 to 100.",
  "error.invalid.invalid_soil_value": "The value cannot be negative.",
  "error.invalid.invalid_soil_texture":
    "Sand, silt and clay are fractions of one soil: they must add up to 100 %.",
  "error.invalid.invalid_harvest_quantity":
    "Enter the quantity and its unit (kg or t), or leave both blank.",
  "error.invalid.plot_not_on_farm": "The chosen plot does not belong to this holding.",

  // Section 8 — the irrigation register (RD 1051/2022 art. 5.e).
  "error.invalid.invalid_date_interval": "The end date cannot be earlier than the start date.",
  "error.invalid.invalid_irrigation_volume": "Enter an irrigation volume greater than zero.",
  "error.invalid.invalid_volume_unit": "An irrigation volume is measured in m\u00b3/ha or m\u00b3.",
  "error.invalid.invalid_water_quality": "A content in the irrigation water cannot be negative.",
  "error.invalid.unknown_irrigation_method": "Choose an irrigation system from the list.",
  "error.invalid.unknown_water_origin": "Choose a water source from the list.",

  // Section 6 — the fertilisation register (RD 1051/2022 art. 5.d).
  "error.invalid.empty_material_code": "Choose the kind of fertiliser material.",
  "error.invalid.unknown_material_code": "Choose a material kind from the list.",
  "error.invalid.unknown_manure_treatment": "Choose a manure treatment from the list.",
  "error.invalid.unknown_nutrient_kind": "Choose macronutrient, micronutrient or heavy metal.",
  "error.invalid.empty_nutrient_code": "Choose the nutrient from the list.",
  "error.invalid.invalid_percentage": "A richness must be between 0 and 100 %.",
  "error.invalid.supplier_id_conflict":
    "State only one of the three: the supplier's REGA, tax id or NIMA.",
  "error.invalid.invalid_density": "The density must be greater than zero.",
  "error.invalid.invalid_dose": "Enter a dose greater than zero.",
  "error.invalid.invalid_dose_unit": "A fertiliser dose is measured per hectare.",
  "error.invalid.invalid_yield": "A yield cannot be negative.",
  "error.invalid.unknown_fertilisation_type": "Choose a fertilisation type from the list.",
  "error.invalid.unknown_application_method": "Choose an application method from the list.",
  "error.invalid.machinery_not_on_farm": "The chosen machinery does not belong to this holding.",
  "error.invalid.empty_practice_code": "Choose a good practice from the list.",

  // Section 7.1 — the plan de abonado (RD 1051/2022 art. 4.2, 5.a and 6).
  "error.invalid.invalid_nutrient_need": "A requirement cannot be negative.",
  "error.invalid.invalid_expected_yield": "Enter an expected yield greater than zero.",
  "error.invalid.crop_not_in_this_book": "The chosen crop is not from this holding and campaign.",
  "error.invalid.crop_already_planned":
    "That crop is already covered by another fertilisation plan.",
  "error.invalid.no_crops": "Name at least one crop of the production unit.",
  "error.invalid.treatment_without_actuation":
    "Name a plant-protection product, a non-chemical measure, or both.",
  "error.invalid.dose_without_product":
    "You entered a dose with no product. Choose the product or clear the dose.",
  "error.invalid.product_without_dose": "Enter the dose of the chosen product.",
  "error.invalid.unknown_measure_code": "That measure is not in the official catalogue.",
  "error.invalid.invalid_intensity":
    "The intensity needs its unit (traps, diffusers…) and must be greater than zero.",
};
