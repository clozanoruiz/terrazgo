// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionari català, per àrees. Les claus són idèntiques en tots els idiomes
// i cap pot repetir-se entre fitxers: i18n.js els fusiona.

export default {
  // Els ecorègims les anotacions dels quals exigeix el RD 1048/2022 (secció 9
  // del model). La sigla (P1, P2…) és com el pagès els coneix des de la
  // sol·licitud única, així que acompanya el nom al selector.
  "eco_practice.extensive_grazing": "Pasturatge extensiu (P1)",
  "eco_practice.sustainable_mowing": "Sega sostenible i illes de biodiversitat (P2)",
  "eco_practice.flooded_biodiversity": "Espais de biodiversitat en cultius sota aigua (P5)",
  "eco_practice.plant_cover": "Cobertes vegetals en cultius llenyosos (P6)",
  "eco_practice.inert_cover": "Cobertes inertes de restes de poda (P7)",
  // L'annex IV no porta sigla: és una obligació de les pastures comunals, i el
  // model imprès no li dedica cap pàgina.
  "eco_practice.communal_pasture": "Pastures comunals (annex IV)",

  // Labors culturals (catàleg TIPO_LABOR). El catàleg fon la sega i
  // l'esbrossada en un sol codi; el model 9.4 les imprimeix en dues columnes,
  // així que aquí són dues.
  "cultural_operation_kind.no_tillage": "Sense laboreig",
  "cultural_operation_kind.tillage": "Laboreig",
  "cultural_operation_kind.levelling": "Anivellament en cultius sota aigua",
  "cultural_operation_kind.ridging": "Cavallons i taules en cultius sota aigua",
  "cultural_operation_kind.weeding": "Eixarcolada",
  "cultural_operation_kind.mowing": "Sega",
  "cultural_operation_kind.brush_cutting": "Esbrossada",
  "cultural_operation_kind.drainage": "Manteniment del drenatge",
  "cultural_operation_kind.pruning": "Poda",
  "cultural_operation_kind.thinning": "Aclarida",
  "cultural_operation_kind.staking": "Entutorat",
  "cultural_operation_kind.grafting": "Empelt",
  "cultural_operation_kind.pruning_removal": "Eliminació de restes de poda",
  "cultural_operation_kind.green_pruning": "Poda en verd",
  "cultural_operation_kind.rolling": "Corronament",

  // Ecorègims — secció 9 del model (RD 1048/2022).
  "book.tab_ecoschemes": "Ecorègims",
  "grazing.title": "9.1 Pasturatge extensiu",
  "grazing.intro":
    "Anoteu les dates de pasturatge quan difereixin de les declarades a la sol·licitud única, en el termini d'un mes des de la data de FI del pasturatge (RD 1048/2022, art. 30.2 ter).",
  "grazing.new": "Nou pasturatge",
  "grazing.practice": "Ecorègim que es justifica",
  "grazing.started_on": "Data d'inici",
  "grazing.ended_on": "Data de fi",
  "grazing.ended_on_hint": "Deixeu-la en blanc mentre el bestiar continuï pasturant.",
  "grazing.ongoing": "en curs",
  "grazing.plot_group_ref": "Id. del grup de parcel·les",
  "grazing.plot_group_ref_hint":
    "Només si la parcel·la o el grup és a més de 10 km de la instal·lació ramadera principal.",
  "grazing.plots_section": "Parcel·les pasturades",
  "grazing.animals_section": "Animals",
  "grazing.animals_hint":
    "Una línia per espècie i explotació ramadera de procedència: el REGA dels animals de tercers és el del seu titular, no el vostre.",
  "grazing.species": "Espècie animal",
  "grazing.rega": "REGA",
  "grazing.animal_count": "Nre. d'animals",
  "grazing.add_animals": "Afegeix animals",
  "grazing.animal_detail": "{count} × {species} ({rega})",
  "grazing.delete_confirm": "Voleu eliminar aquest registre de pasturatge?",
  // Subpestanyes de la secció 9: cada registre de l'ecorègim és una pàgina del
  // model, i tota la secció cap en una sola pestanya del quadern.
  "ecoscheme.tab_grazing": "9.1 Pasturatge",
  "ecoscheme.tab_operations": "9.2 Feines",
  "ecoscheme.tab_covers": "9.4/9.5 Cobertes",

  // 9.2 sega sostenible + «9.6» pastures comunals (RD 1048/2022 arts. 31,
  // 31.4.d i annex IV). Un sol registre darrere de dues pàgines impreses.
  "operation.title": "9.2 Feines i activitats de manteniment",
  "operation.intro":
    "Anoteu la data i l'activitat realitzada en el termini d'un mes (RD 1048/2022, art. 31). L'annex IV exigeix el mateix a cada parcel·la de pastura comunal; el model imprès no li dedica cap pàgina, així que el quadern la imprimeix com a 9.6.",
  "operation.new": "Nova feina",
  "operation.practice": "Ecorègim que es justifica",
  "operation.practice_hint":
    "Decideix a quina pàgina s'imprimeix: la 9.2 per a la sega sostenible, la 9.6 per a les pastures comunals.",
  "operation.kind": "Feina realitzada",
  "operation.performed_on": "Data",
  "operation.performed_end_date": "Data fi",
  "operation.performed_end_date_hint": "Només si la feina s'ha allargat uns quants dies.",
  "operation.activity_description": "Descripció de l'activitat",
  "operation.activity_description_hint":
    "El model demana la data i l'activitat; feu-la servir per al que el codi no anomena.",
  "operation.residue_destination": "Destinació de la resta vegetal",
  "operation.residue_destination_hint":
    "No s'imprimeix al quadern. La trituració de restes de poda dipositades sobre el terreny és la que acredita una coberta inerta (art. 43).",
  "operation.plots_section": "Parcel·les",
  "operation.delete_confirm": "Voleu eliminar aquesta feina?",

  // 9.4 i 9.5 cobertes del sòl (RD 1048/2022 arts. 42 i 43). Un sol registre
  // darrere de dues pàgines impreses, com 9.2 i «9.6».
  "cover.title": "9.4 i 9.5 Cobertes del sòl",
  "cover.intro":
    "L'art. 42 són TRES anotacions amb tres terminis diferents: la data d'establiment (un mes), les dues amplades (abans que acabi el període de coberta viva) i el manteniment realitzat. L'art. 43 demana el mateix llevat del manteniment, i la coberta inerta no es pot establir després del 15 d'abril.",
  "cover.new": "Nova coberta",
  "cover.practice": "Ecorègim que es justifica",
  "cover.practice_hint":
    "Decideix en quina pàgina s'imprimeix: la 9.4 per a les cobertes vegetals, la 9.5 per a les inerts de restes de poda.",
  "cover.type": "Tipus de cobertura",
  "cover.type_hint":
    "No s'imprimeix al quadern: l'art. 42.1.a anota la data, no quina de les dues era. Es desa per a l'intercanvi SIEX.",
  "cover.established_on": "Data d'establiment",
  "cover.established_on_hint": "Art. 42.1.a / 43.1.a: anoteu-la en el termini d'un mes.",
  "cover.widths_section": "Amplades (art. 42.1.e / 43.1.b)",
  "cover.widths_hint":
    "Les tres van juntes o cap: són una sola anotació, amb un termini propi i posterior al de la data d'establiment. Deixeu-les en blanc fins que les mesureu.",
  "cover.width_m": "Amplada de la coberta (m)",
  "cover.free_canopy_width_m": "Amplada lliure projecció capçada (m)",
  "cover.widths_stated_on": "Data del mesurament",
  "cover.plots_section": "Parcel·les amb coberta",
  "cover.maintenance_section": "Manteniment",
  "cover.maintenance_hint":
    "Art. 42.1.c: les tres columnes del model 9.4. Cada línia es desa al seu propi registre —la sega i l'esbrossada com a feina, el pasturatge com a pasturatge— i hereta de la coberta les parcel·les i l'ecorègim.",
  "cover.maintenance_kind": "Manteniment",
  "cover.maintenance_date": "Data",
  "cover.add_maintenance": "Afegir manteniment",
  "cover.maintenance_grazing": "Pasturatge",
  "cover.maintenance_animals_hint":
    "Un pasturatge necessita els seus grups d'animals, s'anoti on s'anoti.",
  "cover.no_maintenance": "Una coberta inerta no porta manteniment (art. 43).",
  "cover.widths_pending": "Amplades sense mesurar",
  "cover.delete_confirm":
    "Voleu eliminar aquesta coberta? També s'eliminarà el manteniment anotat sobre ella.",
};
