// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later

// Diccionario español, por áreas. Las claves son idénticas en todos los
// idiomas y ninguna puede repetirse entre archivos: i18n.js los fusiona.

export default {
  // Los ecorrégimenes cuyas anotaciones exige el RD 1048/2022 (sección 9 del
  // modelo). La sigla (P1, P2…) es como el agricultor los conoce desde la
  // solicitud única, así que acompaña al nombre en el selector.
  "eco_practice.extensive_grazing": "Pastoreo extensivo (P1)",
  "eco_practice.sustainable_mowing": "Siega sostenible e islas de biodiversidad (P2)",
  "eco_practice.flooded_biodiversity": "Espacios de biodiversidad en cultivos bajo agua (P5)",
  "eco_practice.plant_cover": "Cubiertas vegetales en cultivos leñosos (P6)",
  "eco_practice.inert_cover": "Cubiertas inertes de restos de poda (P7)",
  // El anexo IV no lleva sigla: es una obligación de los pastos comunales, y
  // el modelo impreso no le dedica ninguna página.
  "eco_practice.communal_pasture": "Pastos comunales (anexo IV)",

  // Labores culturales (catálogo TIPO_LABOR). El catálogo funde siega y
  // desbroce en un solo código; el modelo 9.4 los imprime en dos columnas, así
  // que aquí son dos.
  "cultural_operation_kind.no_tillage": "Sin laboreo",
  "cultural_operation_kind.tillage": "Laboreo",
  "cultural_operation_kind.levelling": "Nivelación en cultivos bajo agua",
  "cultural_operation_kind.ridging": "Caballones y tablas en cultivos bajo agua",
  "cultural_operation_kind.weeding": "Escarda",
  "cultural_operation_kind.mowing": "Siega",
  "cultural_operation_kind.brush_cutting": "Desbroce",
  "cultural_operation_kind.drainage": "Mantenimiento del drenaje",
  "cultural_operation_kind.pruning": "Poda",
  "cultural_operation_kind.thinning": "Aclareo",
  "cultural_operation_kind.staking": "Entutorado",
  "cultural_operation_kind.grafting": "Injerto",
  "cultural_operation_kind.pruning_removal": "Eliminación de restos de poda",
  "cultural_operation_kind.green_pruning": "Poda en verde",
  "cultural_operation_kind.rolling": "Rulado",

  // Ecorrégimenes — sección 9 del modelo (RD 1048/2022).
  "book.tab_ecoschemes": "Ecorrégimenes",
  "grazing.title": "9.1 Pastoreo extensivo",
  "grazing.intro":
    "Anote las fechas de pastoreo cuando difieran de las declaradas en la solicitud única, en el plazo de un mes desde la fecha de FIN del pastoreo (RD 1048/2022, art. 30.2 ter).",
  "grazing.new": "Nuevo pastoreo",
  "grazing.practice": "Ecorrégimen que se justifica",
  "grazing.started_on": "Fecha de inicio",
  "grazing.ended_on": "Fecha de fin",
  "grazing.ended_on_hint": "Déjela en blanco mientras el ganado siga pastando.",
  "grazing.ongoing": "en curso",
  "grazing.plot_group_ref": "Id. del grupo de parcelas",
  "grazing.plot_group_ref_hint":
    "Solo si la parcela o el grupo está a más de 10 km de la instalación ganadera principal.",
  "grazing.plots_section": "Parcelas pastadas",
  "grazing.animals_section": "Animales",
  "grazing.animals_hint":
    "Una línea por especie y explotación ganadera de procedencia: el REGA de los animales de terceros es el de su titular, no el suyo.",
  "grazing.species": "Especie animal",
  "grazing.rega": "REGA",
  "grazing.animal_count": "Nº de animales",
  "grazing.add_animals": "Añadir animales",
  "grazing.animal_detail": "{count} × {species} ({rega})",
  "grazing.delete_confirm": "¿Eliminar este registro de pastoreo?",
  // Subpestañas de la sección 9: cada registro del ecorrégimen es una página
  // del modelo, y la sección entera cabe en una sola pestaña del cuaderno.
  "ecoscheme.tab_grazing": "9.1 Pastoreo",
  "ecoscheme.tab_operations": "9.2 Labores",
  "ecoscheme.tab_covers": "9.4/9.5 Cubiertas",

  // 9.2 siega sostenible + «9.6» pastos comunales (RD 1048/2022 arts. 31,
  // 31.4.d y anexo IV). Un solo registro detrás de dos páginas impresas.
  "operation.title": "9.2 Labores y actividades de mantenimiento",
  "operation.intro":
    "Anote la fecha y la actividad realizada en el plazo de un mes (RD 1048/2022, art. 31). El anexo IV exige lo mismo en cada parcela de pasto comunal; el modelo impreso no le dedica ninguna página, así que el cuaderno la imprime como 9.6.",
  "operation.new": "Nueva labor",
  "operation.practice": "Ecorrégimen que se justifica",
  "operation.practice_hint":
    "Decide en qué página se imprime: la 9.2 para la siega sostenible, la 9.6 para los pastos comunales.",
  "operation.kind": "Labor realizada",
  "operation.performed_on": "Fecha",
  "operation.performed_end_date": "Fecha fin",
  "operation.performed_end_date_hint": "Solo si la labor se prolongó varios días.",
  "operation.activity_description": "Descripción de la actividad",
  "operation.activity_description_hint":
    "El modelo pide la fecha y la actividad; úsela para lo que el código no nombra.",
  "operation.residue_destination": "Destino del resto vegetal",
  "operation.residue_destination_hint":
    "No se imprime en el cuaderno. La trituración de restos de poda depositados sobre el terreno es la que acredita una cubierta inerte (art. 43).",
  "operation.plots_section": "Parcelas",
  "operation.delete_confirm": "¿Eliminar esta labor?",

  // 9.4 y 9.5 cubiertas (RD 1048/2022 arts. 42 y 43). Un solo registro detrás
  // de dos páginas impresas, como 9.2 y «9.6».
  "cover.title": "9.4 y 9.5 Cubiertas del suelo",
  "cover.intro":
    "El art. 42 son TRES anotaciones con tres plazos distintos: la fecha de establecimiento (un mes), las dos anchuras (antes de que acabe el periodo de cubierta viva) y el mantenimiento realizado. El art. 43 pide lo mismo salvo el mantenimiento, y la cubierta inerte no puede establecerse después del 15 de abril.",
  "cover.new": "Nueva cubierta",
  "cover.practice": "Ecorrégimen que se justifica",
  "cover.practice_hint":
    "Decide en qué página se imprime: la 9.4 para las cubiertas vegetales, la 9.5 para las inertes de restos de poda.",
  "cover.type": "Tipo de cobertura",
  "cover.type_hint":
    "No se imprime en el cuaderno: el art. 42.1.a anota la fecha, no cuál de las dos era. Se guarda para el intercambio SIEX.",
  "cover.established_on": "Fecha de establecimiento",
  "cover.established_on_hint": "Art. 42.1.a / 43.1.a: anótela en el plazo de un mes.",
  "cover.widths_section": "Anchuras (art. 42.1.e / 43.1.b)",
  "cover.widths_hint":
    "Las tres van juntas o ninguna: son una sola anotación, con un plazo propio y posterior al de la fecha de establecimiento. Déjelas en blanco hasta que las mida.",
  "cover.width_m": "Anchura de la cubierta (m)",
  "cover.free_canopy_width_m": "Anchura libre proyección copa (m)",
  "cover.widths_stated_on": "Fecha de la medición",
  "cover.plots_section": "Parcelas con cubierta",
  "cover.maintenance_section": "Mantenimiento",
  "cover.maintenance_hint":
    "Art. 42.1.c: las tres columnas del modelo 9.4. Cada línea se guarda en su propio registro —la siega y el desbroce como labor, el pastoreo como pastoreo— y hereda de la cubierta las parcelas y el ecorrégimen.",
  "cover.maintenance_kind": "Mantenimiento",
  "cover.maintenance_date": "Fecha",
  "cover.add_maintenance": "Añadir mantenimiento",
  "cover.maintenance_grazing": "Pastoreo",
  "cover.maintenance_animals_hint":
    "Un pastoreo necesita sus grupos de animales, se anote donde se anote.",
  "cover.no_maintenance": "Una cubierta inerte no lleva mantenimiento (art. 43).",
  "cover.widths_pending": "Anchuras sin medir",
  "cover.delete_confirm":
    "¿Eliminar esta cubierta? Se eliminará también el mantenimiento anotado sobre ella.",
};
