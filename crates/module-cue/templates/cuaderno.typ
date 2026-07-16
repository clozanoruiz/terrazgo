// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Cuaderno de explotación imprimible — sections 1 (información general),
// 2.1 (parcelas) and 3.1 (registro de tratamientos) of the official model
// (layout "orientativo"; the binding content list is RD 1311/2012 Anexo III,
// which is why a "Plazo de seguridad" column appears even though the model
// lacks one). Labels are Spanish template content by design — reports are
// per-country documents, not UI i18n (docs/architecture.md → Report engine).
//
// All values in sys.inputs are pre-formatted STRINGS (dates dd/mm/yyyy,
// decimal commas); the template does layout only. Empty string = the blank
// cell an official form would leave for hand-filling.

#set text(font: "Liberation Sans", size: 8pt)
#set page(
  paper: "a4",
  flipped: true,
  margin: (x: 1.2cm, top: 1.6cm, bottom: 1.4cm),
  header: context {
    if counter(page).get().first() > 1 {
      set text(size: 7.5pt)
      grid(
        columns: (1fr, auto),
        [Explotación / Titular: *#sys.inputs.farm.name* — #sys.inputs.farm.owner],
        [CAMPAÑA: *#sys.inputs.campaign*],
      )
      line(length: 100%, stroke: 0.5pt)
    }
  },
  footer: context {
    set text(size: 7pt)
    grid(
      columns: (1fr, auto),
      [Documento generado el #sys.inputs.generated_on],
      [Hoja nº #counter(page).display() de #counter(page).final().first()],
    )
  },
)

// The model's rounded section-title box.
#let section-title(body) = align(
  center,
  rect(radius: 6pt, inset: (x: 18pt, y: 7pt), stroke: 1pt)[
    #text(size: 12pt, weight: "bold")[#body]
  ],
)

#let subsection(body) = block(
  above: 12pt,
  below: 6pt,
  fill: luma(225),
  width: 100%,
  inset: 4pt,
  align(center, text(weight: "bold", size: 8.5pt)[#body]),
)

// Data tables: repeating bold header row, hairline grid. `rows` is an array
// of arrays of content; when empty, one row of blank cells keeps the form
// look (a section with nothing registered still prints as a fillable table).
#let data-table(columns, headers, rows) = {
  let body = if rows.len() == 0 {
    (headers.map(_ => []),)
  } else {
    rows
  }
  table(
    columns: columns,
    stroke: 0.5pt,
    inset: 3.5pt,
    align: left + horizon,
    table.header(..headers.map(h => text(weight: "bold", size: 7.5pt)[#h])),
    ..body.flatten(),
  )
}

// ============================================================ 1. INFORMACIÓN GENERAL

#section-title[1. INFORMACIÓN GENERAL]

#v(4pt)
FECHA DE APERTURA DEL CUADERNO #box(width: 7em, repeat("_")) #h(1fr) CAMPAÑA: *#sys.inputs.campaign*

#subsection[1.1 DATOS GENERALES DE LA EXPLOTACIÓN]
#table(
  columns: (2fr, 1fr, 1fr),
  stroke: 0.5pt,
  inset: 4pt,
  [*Nombre de la explotación:* #sys.inputs.farm.name],
  [*Nombre y apellidos o razón social:* #sys.inputs.farm.owner],
  [*NIF:* #sys.inputs.farm.nif],

  [*Nº Registro de Explotaciones (REA):* #sys.inputs.farm.rea],
  [*Localidad:* #sys.inputs.farm.location],
  [*Provincia:* #sys.inputs.farm.province],
)

#subsection[1.2 PERSONAS O EMPRESAS QUE INTERVIENEN EN EL TRATAMIENTO CON PRODUCTOS FITOSANITARIOS]
#data-table(
  (auto, 2fr, 1fr, 1fr, 1fr),
  ([Nº de orden], [Nombre y apellidos / Empresa de servicios], [NIF], [Nº inscripción ROPO / nº carné], [Tipo de carné]),
  sys.inputs.operators.map(o => (
    align(center)[#o.order], [#o.name], [#o.nif], [#o.licence], [#o.level],
  )),
)

#subsection[1.3 EQUIPOS DE APLICACIÓN DE PRODUCTOS FITOSANITARIOS PROPIOS DE LA EXPLOTACIÓN]
#data-table(
  (auto, 2fr, 1fr, 1fr, 1fr),
  ([Nº de orden], [Descripción del equipo], [Nº inscrip. ROMA], [Nº inscrip. REGANIP], [Fecha de la última inspección]),
  sys.inputs.machinery.map(m => (
    align(center)[#m.order], [#m.description], [#m.roma], [#m.reganip], [#m.last_inspection],
  )),
)

#subsection[1.4 ASESOR, AGRUPACIÓN O ENTIDAD DE ASESORAMIENTO A LA QUE PERTENECE LA EXPLOTACIÓN]
#data-table(
  (2fr, 1fr, 1fr, 1fr),
  ([Nombre o razón social], [NIF], [Nº de identificación], [Tipo de explotación]),
  (),
)

// ============================================================ 2. PARCELAS

#pagebreak()
#section-title[2. IDENTIFICACIÓN DE LAS PARCELAS DE LA EXPLOTACIÓN]

#subsection[2.1 DATOS IDENTIFICATIVOS Y AGRONÓMICOS DE LAS PARCELAS]
#data-table(
  (auto, 1.4fr, auto, auto, auto, auto, auto, auto, auto, auto, 1.2fr, 1fr, auto, auto),
  (
    [Nº DE ORDEN], [Parcela], [Prov.], [Municipio], [Agreg.], [Zona], [Políg.], [Parc.], [Rec.],
    [Superficie cultivada (ha)], [Especie], [Variedad], [Secano / Regadío], [GIP #super[(1)]],
  ),
  sys.inputs.plot_rows.map(p => (
    align(center)[#p.order], [#p.name], [#p.province], [#p.municipality], [#p.aggregate],
    [#p.zone], [#p.polygon], [#p.parcel], [#p.enclosure],
    align(right)[#p.area], [#p.species], [#p.variety], [], align(center)[#p.gip],
  )),
)
#text(size: 6.5pt)[
  #super[(1)] Sistema de asesoramiento en gestión integrada de plagas: (AE) Agricultura
  Ecológica, (PI) Producción Integrada.
]

// ============================================================ 3. TRATAMIENTOS

#pagebreak()
#section-title[3. INFORMACIÓN SOBRE TRATAMIENTOS FITOSANITARIOS]

#subsection[3.1 REGISTRO DE ACTUACIONES FITOSANITARIAS DE LA PARCELA]
#data-table(
  (auto, 1fr, 0.9fr, auto, auto, 1.4fr, auto, auto, 1.2fr, 0.9fr, 0.9fr, 1.1fr, auto, 1.2fr),
  (
    [Id. Parcelas #super[(1)]], [Especie], [Variedad], [Fecha], [Superf. tratada (ha)],
    [Problema fitosanitario], [Aplicador #super[(2)]], [Equipo #super[(3)]],
    [Nombre comercial], [Nº Registro], [Dosis], [Plazo de seguridad #super[(4)]],
    [Eficacia #super[(5)]], [Observaciones],
  ),
  sys.inputs.treatments.map(r => (
    align(center)[#r.plots], [#r.species], [#r.variety], [#r.date], align(right)[#r.surface],
    [#r.problems], align(center)[#r.operator], align(center)[#r.equipment],
    [#r.product], [#r.reg_no], [#r.dose], [#r.phi], align(center)[#r.efficacy], [#r.notes],
  )),
)
#text(size: 6.5pt)[
  #super[(1)] Nº de orden de las parcelas tratadas según la tabla 2.1.
  #super[(2)] Nº de orden según la tabla 1.2.
  #super[(3)] Nº de orden según la tabla 1.3; "Manual" cuando la aplicación no empleó equipo.
  #super[(4)] Días de plazo aplicados y primer día en que la cosecha vuelve a estar permitida.
  #super[(5)] Buena, regular o mala, observada tras la aplicación.
]
