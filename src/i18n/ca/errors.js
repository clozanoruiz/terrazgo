// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionari català, per àrees. Les claus són idèntiques en tots els idiomes
// i cap pot repetir-se entre fitxers: i18n.js els fusiona.

export default {
  // Errors del límit de comandes (codis de CommandError → error.<code>).
  // "internal" no té entrada error.<code> a propòsit: es mostra el missatge
  // tècnic, precedit d'internal_intro per orientar l'usuari normal.
  "error.internal_intro": "S'ha produït un error intern:",
  "error.not_found": "El registre no existeix.",
  "error.invalid.unknown_link": "Aquest enllaç no està disponible en aquesta versió.",
  "error.invalid.empty_name": "El nom no pot estar buit.",
  "error.invalid.operator_not_found": "L'operador seleccionat ja no existeix.",
  "error.invalid.empty_authorisation_number": "El número de registre no pot estar buit.",
  "error.invalid.no_problems": "Indiqueu com a mínim una problemàtica (plaga, malaltia…).",
  "error.invalid.no_justifications": "Indiqueu com a mínim una justificació de l'actuació.",
  "error.invalid.end_date_before_start": "La data de fi no pot ser anterior a la d'inici.",
  "error.invalid.application_time": "Indiqueu l'hora en format HH:MM (per exemple 20:30).",
  "error.invalid.growth_stage_unknown": "L'estat fenològic seleccionat no és al catàleg oficial.",
  "error.invalid.invalid_total_quantity":
    "Indiqueu la quantitat total i la seva unitat (kg o l), amb un valor més gran que zero.",
  "error.invalid.unknown_problem_code": "La problemàtica seleccionada no és al catàleg oficial.",
  "error.invalid.export_precheck_failed":
    "L'exportació està bloquejada: falten dades que exigeix el format oficial.",
  "error.invalid.export_code_unmappable":
    "Un codi desat no s'ha pogut convertir als catàlegs oficials.",
  "error.invalid.missing_exceptional_substance":
    "Una autorització excepcional ha d'indicar la seva substància (codi del catàleg oficial).",
  "error.invalid.unknown_substance_code":
    "La substància indicada no és al catàleg oficial d'autoritzacions excepcionals.",
  "error.invalid.nonpositive_area": "La superfície ha de ser més gran que zero.",
  "error.invalid.season_in_use":
    "No es pot suprimir una campanya amb cultius o tractaments registrats. Elimineu-ne abans el contingut.",
  "error.invalid.missing_distance":
    "Indiqueu la distància a la parcel·la: és obligatòria quan la captació queda fora.",
  "error.invalid.water_point_distance_inside":
    "Una captació inclosa a la parcel·la no porta distància. Desmarqueu «inclosa» o esborreu la distància.",
  "error.invalid.water_point_coordinates_invalid":
    "Les coordenades s'han d'indicar completes (latitud i longitud) i dins dels límits del globus.",
  "error.invalid.plot_has_water_points":
    "Aquesta parcel·la té captacions registrades, així que no es pot declarar sense captacions. Suprimiu-les abans.",
  "error.invalid.report_language_unknown": "Aquest idioma no està disponible per al quadern.",
  "error.invalid.cache_cap_too_small":
    "L'espai per als mapes sense connexió és massa petit (mínim 64 MB).",
  "error.invalid.lead_days_out_of_range": "L'antelació de l'avís ha de ser d'entre 1 i 400 dies.",
  "error.invalid.phi_horizon_out_of_range":
    "Els dies per mostrar les parcel·les tractades han de ser entre 7 i 730.",
  "error.invalid_date": "Data no vàlida «{date}» (s'espera AAAA-MM-DD).",
  "error.authorisation_missing": "El producte {product_id} no està autoritzat a «{country}».",
  "error.country_mismatch": "El país «{provided}» no coincideix amb el de l'explotació («{farm}»).",
  "error.plot_not_on_farm": "La parcel·la {plot_id} no pertany a l'explotació {farm_id}.",
  "error.invalid.backup_invalid":
    "El fitxer seleccionat no és una còpia de seguretat vàlida de Terrazgo.",
  "error.invalid.backup_newer_schema":
    "Aquesta còpia s'ha creat amb una versió més recent de Terrazgo; actualitzeu abans l'aplicació.",
  "error.missing_phi_days":
    "No hi ha termini de seguretat disponible: el producte no té valor per defecte i no se n'ha indicat cap.",
  "error.geo_http": "El servei de mapes ha respost amb un error (HTTP {status}).",
  "error.geo_offline":
    "Sense connexió — només es mostren les dades de mapa de la memòria cau. ({reason})",
  "error.invalid.geometry_invalid":
    "La geometria no és un contorn vàlid (un polígon tancat amb coordenades vàlides).",
  "error.invalid.geo_subject_missing":
    "La geometria no està associada a cap parcel·la ni explotació.",
  "error.invalid.geo_subject_ambiguous": "La geometria no pot pertànyer a dos elements alhora.",
  "error.invalid.boundary_file_unsupported":
    "Fitxer no compatible — useu GeoJSON o GeoPackage (.gpkg).",
  "error.invalid.boundary_file_empty": "El fitxer no conté contorns utilitzables (polígons).",
  "error.invalid.boundary_file_too_large":
    "El fitxer té massa elements — useu un extracte més petit (p. ex., un municipi).",
  "error.invalid.gpkg_unsupported_srs":
    "El GeoPackage usa un sistema de coordenades projectat que aquesta versió encara no pot llegir.",
  "error.invalid.tilejson_invalid":
    "El servei de mapes ha retornat un índex de tessel·les inservible.",
  "error.invalid.style_unsupported":
    "L'estil del mapa base ha canviat al servei d'una manera que Terrazgo encara no reconeix.",
  "error.invalid.sigpac_ref_invalid":
    "La referència SIGPAC és incompleta o no és numèrica — reviseu les set parts.",
  "error.invalid.sigpac_response_invalid": "SIGPAC ha respost en un format inesperat.",
  "error.invalid.sigpac_ref_missing":
    "La parcel·la no té una referència SIGPAC completa — ompliu abans les set parts.",
  "error.invalid.zone_status_invalid":
    "El resultat intern de la comprovació de zones no era utilitzable.",
  "error.invalid.quantity_unit_mismatch":
    "La unitat no correspon al que s'ha tractat: tones per a producte vegetal, m\u00b3 per a locals i vehicles.",
  "error.invalid.invalid_product_quantity":
    "Indiqueu la quantitat de producte i la seva unitat (kg o l), amb un valor m\u00e9s gran que zero.",
  "error.invalid.empty_subject": "Indiqueu qu\u00e8 s'ha tractat.",
  "error.invalid.unknown_subject_kind": "Tipus de registre desconegut.",
  "error.invalid.register_has_rows":
    "No es pot declarar sense tractaments: el registre ja t\u00e9 anotacions.",
  "error.invalid.empty_product_name": "Indiqueu el nom del producte de l'etiqueta.",
  "error.invalid.no_plots": "Indiqueu com a m\u00ednim una parcel\u00b7la.",
  "error.invalid.invalid_seed_quantity": "La quantitat de llavor ha de ser m\u00e9s gran que zero.",
  "error.invalid.unknown_seed_treatment_kind": "Trieu un tractament de la llista.",
  "error.invalid.unknown_analysis_material": "Trieu el material analitzat.",
  "error.invalid.unknown_analysis_type": "Trieu un tipus d'an\u00e0lisi de la llista.",
  "error.invalid.empty_buyer_name": "Indiqueu el nom o la ra\u00f3 social del client.",

  // Secció 4 — paràmetres del sòl (annex III A.3).
  "error.invalid.invalid_soil_ph": "El pH va de 0 a 14.",
  "error.invalid.invalid_soil_percentage": "Els percentatges van de 0 a 100.",
  "error.invalid.invalid_soil_value": "El valor no pot ser negatiu.",
  "error.invalid.invalid_soil_texture":
    "Sorra, llim i argila són fraccions d'un mateix sòl: han de sumar 100 %.",
  "error.invalid.invalid_harvest_quantity":
    "Indiqueu la quantitat i la seva unitat (kg o t), o deixeu-les totes dues en blanc.",
  "error.invalid.plot_not_on_farm":
    "La parcel\u00b7la triada no pertany a aquesta explotaci\u00f3.",
  "error.invalid.sowing_not_on_farm":
    "La sembra triada \u00e9s d'una altra explotaci\u00f3 o d'una altra campanya.",
  "error.invalid.irrigation_not_on_farm":
    "El reg triat \u00e9s d'una altra explotaci\u00f3 o d'una altra campanya.",
  "error.invalid.link_needs_fertigation":
    "Nom\u00e9s una fertirrigaci\u00f3 es pot enlla\u00e7ar amb un reg. Trieu fertirrigaci\u00f3 per aspersi\u00f3 o localitzada com a forma d'aplicaci\u00f3.",

  // Secci\u00f3 8 \u2014 el registre de reg (RD 1051/2022 art. 5.e).
  "error.invalid.invalid_date_interval": "La data final no pot ser anterior a la inicial.",
  "error.invalid.invalid_irrigation_volume": "Indiqueu un volum de reg m\u00e9s gran que zero.",
  "error.invalid.invalid_volume_unit": "El volum de reg es mesura en m\u00b3/ha o en m\u00b3.",
  "error.invalid.invalid_water_quality": "El contingut a l'aigua de reg no pot ser negatiu.",
  "error.invalid.unknown_irrigation_method": "Trieu un sistema de reg de la llista.",
  "error.invalid.unknown_water_origin": "Trieu una proced\u00e8ncia de l'aigua de la llista.",

  // Secció 6 — el registre de fertilització (RD 1051/2022 art. 5.d).
  "error.invalid.empty_material_code": "Trieu el tipus de material fertilitzant.",
  "error.invalid.unknown_material_code": "Trieu un tipus de material de la llista.",
  "error.invalid.unknown_manure_treatment": "Trieu un tractament dels fems de la llista.",
  "error.invalid.unknown_nutrient_kind": "Trieu macronutrient, micronutrient o metall pesant.",
  "error.invalid.empty_nutrient_code": "Trieu el nutrient de la llista.",
  "error.invalid.invalid_percentage": "La riquesa ha d'estar entre 0 i 100 %.",
  "error.invalid.supplier_id_conflict":
    "Indiqueu només un dels tres: REGA, NIF o NIMA de l'empresa subministradora.",
  "error.invalid.invalid_density": "La densitat ha de ser més gran que zero.",
  "error.invalid.invalid_dose": "Indiqueu una dosi més gran que zero.",
  "error.invalid.invalid_dose_unit": "La dosi de fertilitzant es mesura per hectàrea.",
  "error.invalid.invalid_yield": "La producció no pot ser negativa.",
  "error.invalid.unknown_fertilisation_type": "Trieu un tipus de fertilització de la llista.",
  "error.invalid.unknown_application_method": "Trieu una forma d'aplicació de la llista.",
  "error.invalid.machinery_not_on_farm": "La maquinària triada no pertany a aquesta explotació.",
  "error.invalid.empty_practice_code": "Trieu una bona pràctica de la llista.",
  "error.invalid.practices_contradict_none":
    "«No fa bones pràctiques» no es pot marcar juntament amb altres pràctiques.",

  // Secció 7.1 — el pla d'adobat (RD 1051/2022 art. 4.2, 5.a i 6).
  "error.invalid.invalid_nutrient_need": "Les necessitats no poden ser negatives.",
  "error.invalid.invalid_expected_yield": "Indiqueu un rendiment esperat més gran que zero.",
  "error.invalid.crop_not_in_this_book": "El cultiu triat no és d'aquesta explotació i campanya.",
  "error.invalid.crop_already_planned": "Aquest cultiu ja és inclòs en un altre pla d'adobat.",
  "error.invalid.no_crops": "Indiqueu almenys un cultiu de la unitat de producció.",
  "error.invalid.treatment_without_actuation":
    "Indiqueu un producte fitosanitari, una mesura no química, o tots dos.",
  "error.invalid.dose_without_product":
    "Heu indicat una dosi sense producte. Trieu el producte o esborreu la dosi.",
  "error.invalid.product_without_dose": "Indiqueu la dosi del producte triat.",
  "error.invalid.unknown_measure_code": "La mesura indicada no figura al catàleg oficial.",
  "error.invalid.invalid_intensity":
    "La intensitat s'ha d'indicar amb la seva unitat (trampes, difusors…) i ser més gran que zero.",

  // Ecorègims — 9.1 pasturatge extensiu (RD 1048/2022 art. 30.2 ter).
  "error.invalid.practice_not_grazing":
    "Aquest ecorègim no es justifica amb un pasturatge. Trieu pasturatge extensiu, sega sostenible, pastures comunals o coberta vegetal.",
  "error.invalid.no_animals": "Indiqueu almenys un grup d'animals.",
  "error.invalid.incomplete_animal_line":
    "Cada grup d'animals necessita espècie i REGA de l'explotació ramadera.",
  "error.invalid.nonpositive_volume": "El volum ha de ser més gran que zero.",
  "error.invalid.premises_kind_mismatch":
    "Aquest registre no correspon a aquest apartat: els locals van al 3.4 i els mitjans de transport al 3.5.",
  "error.invalid.premises_not_on_farm": "El local o vehicle pertany a una altra explotació.",
  "error.invalid.premises_kind_in_use":
    "No es pot canviar el tipus mentre hi hagi tractaments que l'anomenen a l'altre apartat. Corregiu o suprimiu aquests tractaments primer.",
  "error.invalid.premises_on_produce_record":
    "Un tractament postcollita tracta producte vegetal, no un local ni un vehicle.",
  "error.invalid.nonpositive_animal_count": "El nombre d'animals ha de ser més gran que zero.",

  // 9.2 sega sostenible i "9.6" pastures comunals (RD 1048/2022 art. 31 i annex IV).
  "error.invalid.practice_not_operation":
    "El pasturatge extensiu s'anota al registre 9.1, no com a feina: la seva obligació són les dates de pasturatge.",
  // Registre de sembra (RD 1048/2022 art. 45.2, cultius sota aigua).
  "error.invalid.flooded_before_sown":
    "La data d'inundació no pot ser anterior a la de sembra: el registre és de sembra en sec, primer la llavor i després l'aigua.",

  // 9.4 i 9.5 cobertes (RD 1048/2022 arts. 42 i 43).
  "error.invalid.practice_not_cover":
    "Aquest ecorègim no estableix cap coberta. Aquí només s'anoten les cobertes vegetals (P6) i les inerts de restes de poda (P7).",
  "error.invalid.incomplete_widths":
    "L'amplada de la coberta, l'amplada lliure de projecció de capçada i la data en què es van mesurar són una sola anotació: indiqueu-les juntes o deixeu-les totes tres en blanc.",
  "error.invalid.nonpositive_width": "Les amplades han de ser més grans que zero.",
  "error.invalid.not_a_maintenance_kind":
    "El model 9.4 només recull sega, esbrossada i pasturatge com a manteniment de la coberta. La resta de feines s'anoten al registre 9.2.",
  "error.invalid.maintenance_on_an_inert_cover":
    "L'art. 43 no exigeix anotar el manteniment d'una coberta inerta: només la data d'establiment i les dues amplades.",
  "error.invalid.animals_on_a_non_grazing_line":
    "Només el pasturatge porta grups d'animals; traieu-los de la línia de sega o esbrossada.",
  "error.invalid.cover_not_found": "La coberta indicada ja no existeix.",
  "error.invalid.cover_on_another_farm": "La coberta indicada pertany a una altra explotació.",
  "error.invalid.cover_practice_mismatch":
    "El manteniment s'ha d'anotar sota el mateix ecorègim que la coberta que manté.",
};
