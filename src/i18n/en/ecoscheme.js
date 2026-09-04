// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// English dictionary, by area. The key set is identical in every locale and
// no key may appear in two files: i18n.js merges them.

export default {
  // The eco-schemes whose annotations RD 1048/2022 requires (model section 9).
  // The P1/P2 tags are how a farmer knows them from the solicitud única, so
  // they ride along in the selector.
  "eco_practice.extensive_grazing": "Extensive grazing (P1)",
  "eco_practice.sustainable_mowing": "Sustainable mowing and biodiversity islands (P2)",
  "eco_practice.flooded_biodiversity": "Biodiversity spaces on flooded crops (P5)",
  "eco_practice.plant_cover": "Plant covers on woody crops (P6)",
  "eco_practice.inert_cover": "Inert covers of pruning residue (P7)",
  // Anexo IV carries no tag: it is a communal-pasture duty, and the printed
  // model gives it no page at all.
  "eco_practice.communal_pasture": "Communal pastures (anexo IV)",

  // Cultural operations (FEGA TIPO_LABOR). The catalogue folds mowing and
  // brush cutting into one code; model 9.4 prints them as two columns, so here
  // they are two.
  "cultural_operation_kind.no_tillage": "No tillage",
  "cultural_operation_kind.tillage": "Tillage",
  "cultural_operation_kind.levelling": "Levelling (flooded crops)",
  "cultural_operation_kind.ridging": "Ridges and beds (flooded crops)",
  "cultural_operation_kind.weeding": "Mechanical weeding",
  "cultural_operation_kind.mowing": "Mowing",
  "cultural_operation_kind.brush_cutting": "Brush cutting",
  "cultural_operation_kind.drainage": "Drainage maintenance",
  "cultural_operation_kind.pruning": "Pruning",
  "cultural_operation_kind.thinning": "Thinning",
  "cultural_operation_kind.staking": "Staking",
  "cultural_operation_kind.grafting": "Grafting",
  "cultural_operation_kind.pruning_removal": "Pruning residue removal",
  "cultural_operation_kind.green_pruning": "Green pruning",
  "cultural_operation_kind.rolling": "Rolling",

  // Eco-schemes — model section 9 (RD 1048/2022).
  "book.tab_ecoschemes": "Eco-schemes",
  "grazing.title": "9.1 Extensive grazing",
  "grazing.intro":
    "Record the grazing dates when they differ from those declared in the solicitud única, within one month of the END of grazing (RD 1048/2022, art. 30.2 ter).",
  "grazing.new": "New grazing",
  "grazing.practice": "Eco-scheme evidenced",
  "grazing.started_on": "Start date",
  "grazing.ended_on": "End date",
  "grazing.ended_on_hint": "Leave blank while the animals are still grazing.",
  "grazing.ongoing": "ongoing",
  "grazing.plot_group_ref": "Plot group id",
  "grazing.plot_group_ref_hint":
    "Only when the plot or group lies more than 10 km from the main livestock installation.",
  "grazing.plots_section": "Plots grazed",
  "grazing.animals_section": "Animals",
  "grazing.animals_hint":
    "One line per species and holding of origin: third-party animals carry their owner's REGA, not yours.",
  "grazing.species": "Animal species",
  "grazing.rega": "REGA",
  "grazing.animal_count": "Head count",
  "grazing.add_animals": "Add animals",
  "grazing.animal_detail": "{count} × {species} ({rega})",
  "grazing.delete_confirm": "Delete this grazing record?",
  // Section 9's sub-tabs: each eco-scheme register is a page of the model, and
  // the whole section fits in one tab of the record book.
  "ecoscheme.tab_grazing": "9.1 Grazing",
  "ecoscheme.tab_operations": "9.2 Operations",
  "ecoscheme.tab_covers": "9.4/9.5 Covers",

  // 9.2 sustainable mowing + "9.6" communal pastures (RD 1048/2022 arts. 31,
  // 31.4.d and annex IV). One register behind two printed pages.
  "operation.title": "9.2 Operations and maintenance activities",
  "operation.intro":
    "Record the date and the activity within one month (RD 1048/2022, art. 31). Annex IV asks the same of each communal pasture plot; the printed model gives it no page, so this book prints it as 9.6.",
  "operation.new": "New operation",
  "operation.practice": "Eco-scheme evidenced",
  "operation.practice_hint":
    "It decides which page the row prints on: 9.2 for sustainable mowing, 9.6 for communal pastures.",
  "operation.kind": "Operation",
  "operation.performed_on": "Date",
  "operation.performed_end_date": "End date",
  "operation.performed_end_date_hint": "Only if the work ran over several days.",
  "operation.activity_description": "Activity description",
  "operation.activity_description_hint":
    "The model asks for the date and the activity; use this for what the code does not name.",
  "operation.residue_destination": "Plant residue destination",
  "operation.residue_destination_hint":
    "Not printed in the book. Triturated pruning residue left on the ground is what evidences an inert cover (art. 43).",
  "operation.plots_section": "Plots",
  "operation.delete_confirm": "Delete this operation?",

  // 9.4 and 9.5 soil covers (RD 1048/2022 arts. 42 and 43). One register
  // behind two printed pages, as 9.2 and "9.6" are.
  "cover.title": "9.4 and 9.5 Soil covers",
  "cover.intro":
    "Art. 42 is THREE annotations on three different deadlines: the establishment date (one month), the two widths (before the live-cover period ends) and the maintenance performed. Art. 43 asks the same minus the maintenance, and an inert cover may not be established later than 15 April.",
  "cover.new": "New cover",
  "cover.practice": "Eco-scheme evidenced",
  "cover.practice_hint":
    "Decides which page it prints on: 9.4 for plant covers, 9.5 for inert covers of pruning residue.",
  "cover.type": "Kind of cover",
  "cover.type_hint":
    "Not printed in the book: art. 42.1.a annotates the date, not which of the two it was. Stored for the SIEX exchange.",
  "cover.established_on": "Establishment date",
  "cover.established_on_hint": "Art. 42.1.a / 43.1.a: record it within one month.",
  "cover.widths_section": "Widths (art. 42.1.e / 43.1.b)",
  "cover.widths_hint":
    "All three together or none: they are a single annotation, on a deadline of their own and later than the establishment date's. Leave them blank until you measure.",
  "cover.width_m": "Cover width (m)",
  "cover.free_canopy_width_m": "Free canopy width (m)",
  "cover.widths_stated_on": "Date measured",
  "cover.plots_section": "Plots under cover",
  "cover.maintenance_section": "Maintenance",
  "cover.maintenance_hint":
    "Art. 42.1.c: model 9.4's three columns. Each line is stored in the register that owns it — mowing and brush cutting as an operation, grazing as a grazing — and inherits the cover's plots and eco-scheme.",
  "cover.maintenance_kind": "Maintenance",
  "cover.maintenance_date": "Date",
  "cover.add_maintenance": "Add maintenance",
  "cover.maintenance_grazing": "Grazing",
  "cover.maintenance_animals_hint": "A grazing needs its animal groups, wherever it is recorded.",
  "cover.no_maintenance": "An inert cover carries no maintenance (art. 43).",
  "cover.widths_pending": "Widths not measured",
  "cover.delete_confirm":
    "Delete this cover? The maintenance recorded against it will be deleted too.",
};
