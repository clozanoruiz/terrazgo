// SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// Cuaderno de explotación imprimible — sections 1 (información general),
// 2.1 (parcelas) and 3.1 (registro de tratamientos) of the official model
// (layout "orientativo"; the binding content list is RD 1311/2012 Anexo III,
// which is why a "Plazo de seguridad" column appears even though the model
// lacks one).
//
// This template holds NO prose. The layout is per country, the language is per
// region (a co-official language must be printable where it is official), so
// every heading, footnote and printed word arrives in sys.inputs.labels from
// the assembly's Labels struct — one template serves every language.
//
// All values in sys.inputs are pre-formatted STRINGS (dates dd/mm/yyyy,
// decimal commas); the template does layout only. Empty string = the blank
// cell an official form would leave for hand-filling.

#let L = sys.inputs.labels

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
        [#L.doc.farm_owner: *#sys.inputs.farm.name* — #sys.inputs.farm.owner],
        [#L.doc.campaign: *#sys.inputs.campaign*],
      )
      line(length: 100%, stroke: 0.5pt)
    }
  },
  footer: context {
    set text(size: 7pt)
    grid(
      columns: (1fr, auto),
      [#L.doc.generated_on #sys.inputs.generated_on],
      [#L.doc.page #counter(page).display() #L.doc.page_of #counter(page).final().first()],
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
// of arrays of content.
//
// An EMPTY register prints `blank_rows` ruled lines instead: no register in
// this book is obliged to hold content, so an empty one is a normal book and
// should print as a usable paper form — a single blank line reads as a
// rendering accident. A register that DOES hold records prints exactly its
// records and no trailing blanks, so the book stays a faithful image of what
// the app holds (and section 3.1 does not grow pages of ruled lines).
//
// `blank_rows: 0` closes the table completely. The conditional registers pass
// it when "APLICA TRATAMIENTO: NO" is ticked: there the tick IS the content,
// and inviting empty lines underneath would solicit writing that contradicts
// the statement directly above them.
//
// `size` scales the whole table. Section 2.1 carries seventeen columns (the
// full SIGPAC reference, both surfaces and four coded attributes), which do not
// fit landscape A4 at the default 8pt — the alternative would be collapsing the
// seven reference parts into one cell, and the model prints them separately.
#let data-table(columns, headers, rows, size: 8pt, blank_rows: 6) = {
  let body = if rows.len() == 0 {
    range(blank_rows).map(_ => headers.map(_ => []))
  } else {
    rows
  }
  set text(size: size)
  table(
    columns: columns,
    stroke: 0.5pt,
    inset: 3pt,
    align: left + horizon,
    table.header(..headers.map(h => text(weight: "bold", size: size * 0.95)[#h])),
    ..body.flatten(),
  )
}

// The "APLICA TRATAMIENTO: SÍ / NO" line heading each conditional register.
// Two boxes, one of which may be ticked: neither ticked is the honest look of
// a register nobody has filled in yet, and is different from a stated "no".
#let applies-line(reg) = block(below: 4pt)[
  #text(size: 8pt, weight: "bold")[#L.s33.applies #super[(1)]:]
  #box(stroke: 0.5pt, width: 10pt, height: 10pt, inset: 1pt, align(center)[#reg.applies_yes])
  #text(size: 8pt)[#L.value.yes]
  #h(6pt)
  #box(stroke: 0.5pt, width: 10pt, height: 10pt, inset: 1pt, align(center)[#reg.applies_no])
  #text(size: 8pt)[#L.value.no]
]

// How many ruled lines an empty conditional register offers. A ticked "NO" is
// a statement that there is nothing to record, so the table closes; a register
// nobody has touched still gets a form to fill in.
#let blank-rows-for(applies_no) = if applies_no != "" { 0 } else { 6 }

// Sections 3.3, 3.4 and 3.5 are one table with two per-section headings: what
// was treated and what it is measured in.
#let non-field-register(reg, title, subject, quantity) = {
  subsection[#title]
  applies-line(reg)
  data-table(
    (auto, 1.6fr, 0.8fr, 1.4fr, 1fr, 1.1fr, 0.8fr, 0.9fr, auto, 1.2fr),
    (
      [#L.s33.date], [#subject], [#quantity], [#L.s33.problem], [#L.s33.operator],
      [#L.s33.product], [#L.s33.registration], [#L.s33.product_quantity #super[(2)]],
      [#L.s33.efficacy], [#L.s33.notes],
    ),
    reg.rows.map(r => (
      [#r.date], [#r.subject], align(right)[#r.quantity], [#r.problems], [#r.operator],
      [#r.product], [#r.reg_no], align(right)[#r.product_quantity],
      align(center)[#r.efficacy], [#r.notes],
    )),
    size: 7.5pt,
    blank_rows: blank-rows-for(reg.applies_no),
  )
  text(size: 6.5pt)[
    #super[(1)] #L.s33.note_applies
    #super[(2)] #L.s33.note_product_quantity
  ]
}

// ============================================================ 1. INFORMACIÓN GENERAL

#section-title[#L.s1.title]

#v(4pt)
// A stated opening date prints; an unstated one keeps the model's ruled line,
// which is a form field the farmer can still fill in by hand.
#L.s1.opening_date #if sys.inputs.farm.opened_on != "" [ *#sys.inputs.farm.opened_on* ] else [
  #box(width: 7em, repeat("_"))
] #h(1fr) #L.doc.campaign: *#sys.inputs.campaign*

#subsection[#L.s1.general_title]
#table(
  columns: (2.4fr, 1fr, 1fr, 1fr),
  stroke: 0.5pt,
  inset: 4pt,
  table.cell(colspan: 3)[*#L.s1.owner_name* #sys.inputs.farm.owner],
  [*#L.s1.tax_id* #sys.inputs.farm.nif],

  table.cell(colspan: 2)[
    *#L.s1.registry_national* #sys.inputs.farm.siex
  ],
  table.cell(colspan: 2)[
    *#L.s1.registry_regional* #sys.inputs.farm.rea
  ],

  [*#L.s1.address* #sys.inputs.farm.address],
  [*#L.s1.locality* #sys.inputs.farm.location],
  [*#L.s1.postal_code* #sys.inputs.farm.postal_code],
  [*#L.s1.province* #sys.inputs.farm.province],

  [*#L.s1.phone_fixed* #sys.inputs.farm.phone_fixed],
  [*#L.s1.phone_mobile* #sys.inputs.farm.phone_mobile],
  table.cell(colspan: 2)[*#L.s1.email* #sys.inputs.farm.email],

  table.cell(colspan: 4)[*#L.s1.farm_name* #sys.inputs.farm.name],
)

#v(2pt)
#align(center, text(weight: "bold", size: 8.5pt)[#L.s1.representative_title])
#table(
  columns: (2.4fr, 1fr, 1fr, 1fr),
  stroke: 0.5pt,
  inset: 4pt,
  table.cell(colspan: 3)[*#L.s1.full_name* #sys.inputs.representative.name],
  [*#L.s1.tax_id* #sys.inputs.representative.nif],

  [*#L.s1.address* #sys.inputs.representative.address],
  [*#L.s1.locality* #sys.inputs.representative.locality],
  [*#L.s1.postal_code* #sys.inputs.representative.postal_code],
  [*#L.s1.province* #sys.inputs.representative.province],

  table.cell(colspan: 2)[*#L.s1.representation_kind* #sys.inputs.representative.kind],
  [*#L.s1.phone* #sys.inputs.representative.phone],
  [*#L.s1.email* #sys.inputs.representative.email],
)

// The model's signature box: the signatory answers for the data's veracity, so
// it is deliberately hand-signed and never pre-filled.
#v(6pt)
#align(
  right,
  block(width: 45%, stroke: 0.5pt, inset: 8pt)[
    #set text(size: 7.5pt)
    #L.s1.signature #super[(1)]
    #v(28pt)
    #L.s1.date
  ],
)
#text(size: 6.5pt)[
  #super[(1)] #L.s1.signature_note
]

#subsection[#L.s12.title]
#data-table(
  (auto, 2fr, 1fr, 1fr, 1fr, auto),
  (
    [#L.s12.order], [#L.s12.name], [#L.s12.tax_id],
    [#L.s12.licence_number], [#L.s12.licence_level], [#L.s12.advisor #super[(1)]],
  ),
  sys.inputs.operators.map(o => (
    align(center)[#o.order], [#o.name], [#o.nif], [#o.licence], [#o.level],
    align(center)[#o.advisor],
  )),
)
#text(size: 6.5pt)[
  #super[(1)] #L.s12.note
]

#subsection[#L.s13.title]
#data-table(
  (auto, 2fr, 1fr, 1fr, 1fr, 1fr),
  (
    [#L.s13.order], [#L.s13.description], [#L.s13.roma], [#L.s13.reganip],
    [#L.s13.acquired_on], [#L.s13.last_inspection],
  ),
  sys.inputs.machinery.map(m => (
    align(center)[#m.order], [#m.description], [#m.roma], [#m.reganip],
    [#m.acquired_on], [#m.last_inspection],
  )),
)

#subsection[#L.s14.title]
#data-table(
  (2fr, 1fr, 1fr, 1fr),
  (
    [#L.s14.name], [#L.s14.tax_id], [#L.s14.registration_number],
    [#L.s14.gip #super[(1)]],
  ),
  sys.inputs.advisors.map(a => (
    [#a.name], [#a.nif], [#a.registration_number], align(center)[#a.gip],
  )),
)
#text(size: 6.5pt)[
  #super[(1)] #L.s14.note
]

// ============================================================ 2. PARCELAS

#pagebreak()
#section-title[#L.s21.section_title]

#subsection[#L.s21.title]
#data-table(
  size: 6.5pt,
  (
    // The municipality carries "código y nombre" in one cell, so it takes a
    // share and wraps; leaving it `auto` would let a long name (up to 67
    // characters upstream) push the whole 17-column table out of the page.
    auto, 1.4fr, auto, 1.3fr, auto, auto, auto, auto, auto, auto, auto, auto, 1.3fr, 1.1fr,
    auto, auto, auto,
  ),
  (
    [#L.s21.order], [#L.s21.plot], [#L.s21.province], [#L.s21.municipality], [#L.s21.aggregate],
    [#L.s21.zone], [#L.s21.polygon], [#L.s21.parcel], [#L.s21.enclosure],
    [#L.s21.land_use], [#L.s21.sigpac_area], [#L.s21.cultivated_area #super[(2)]],
    [#L.s21.species], [#L.s21.variety],
    [#L.s21.irrigation #super[(3)]], [#L.s21.environment #super[(4)]],
    [#L.s21.gip #super[(1)]],
  ),
  sys.inputs.plot_rows.map(p => (
    align(center)[#p.order], [#p.name], [#p.province], [#p.municipality], [#p.aggregate],
    [#p.zone], [#p.polygon], [#p.parcel], [#p.enclosure],
    align(center)[#p.land_use], align(right)[#p.sigpac_area],
    align(right)[#p.area], [#p.species], [#p.variety],
    align(center)[#p.irrigation], align(center)[#p.environment], align(center)[#p.gip],
  )),
)
#text(size: 6.5pt)[
  #super[(1)] #L.s21.note_gip \
  #super[(2)] #L.s21.note_area \
  #super[(3)] #L.s21.note_irrigation \
  #super[(4)] #L.s21.note_environment
]

#subsection[#L.s22.title]
#data-table(
  (auto, 1.2fr, 1fr, auto, auto, 1fr, 1fr, auto, auto, 1.6fr),
  (
    [#L.s22.order], [#L.s22.species], [#L.s22.variety],
    [#L.s22.water_point], [#L.s22.distance], [#L.s22.coordinates],
    [#L.s22.denomination], [#L.s22.fully], [#L.s22.partly],
    [#L.s22.checked #super[(1)]],
  ),
  sys.inputs.zone_rows.map(z => (
    align(center)[#z.order], [#z.species], [#z.variety],
    align(center)[#z.water_point], align(right)[#z.distance], [#z.coordinates],
    [#z.denomination],
    align(center)[#z.fully], align(center)[#z.partly], [#z.checked],
  )),
)
#text(size: 6.5pt)[
  #super[(1)] #L.s22.note
]

// ============================================================ 3. TRATAMIENTOS

#pagebreak()
#section-title[#L.s31.section_title]

#subsection[#L.s31.title]
#data-table(
  (
    // Especie takes a share from Variedad, which holds one short name, because
    // it now carries the BBCH stage folded in behind the crop.
    auto, 1.2fr, 0.7fr, 1.1fr, auto, 1.4fr, auto, auto, 1.2fr, 0.9fr,
    0.9fr, 0.9fr, 1.1fr, auto, 1.2fr,
  ),
  (
    [#L.s31.plots #super[(1)]], [#L.s31.species #super[(2)]], [#L.s31.variety],
    [#L.s31.date #super[(3)]], [#L.s31.surface], [#L.s31.problem],
    [#L.s31.operator #super[(4)]], [#L.s31.equipment #super[(5)]], [#L.s31.product],
    [#L.s31.registration], [#L.s31.dose], [#L.s31.total_quantity #super[(6)]],
    [#L.s31.phi #super[(7)]], [#L.s31.efficacy #super[(8)]], [#L.s31.notes],
  ),
  sys.inputs.treatments.map(r => (
    align(center)[#r.plots], [#r.species], [#r.variety], [#r.date], align(right)[#r.surface],
    [#r.problems], align(center)[#r.operator], align(center)[#r.equipment],
    [#r.product], [#r.reg_no], [#r.dose], align(right)[#r.total_quantity],
    [#r.phi], align(center)[#r.efficacy], [#r.notes],
  )),
  size: 7pt,
)
#text(size: 6.5pt)[
  #super[(1)] #L.s31.note_plots
  #super[(2)] #L.s31.note_growth_stage
  #super[(3)] #L.s31.note_date
  #super[(4)] #L.s31.note_operator
  #super[(5)] #L.s31.note_equipment
  #super[(6)] #L.s31.note_total_quantity
  #super[(7)] #L.s31.note_phi
  #super[(8)] #L.s31.note_efficacy
]

// ------------------------------------------------------------- 3.1 bis ASESORADOS
// The same actuations as 3.1, cut for the advised ones. Anexo III Parte I B is
// ONE list covering every treatment and B.d puts the advisor on it, so this is
// a second VIEW rather than a second register — it shows the two things 3.1 has
// no column for: the non-chemical alternative and its intensity.
//
// The page prints even when empty, like every other register: an advised
// holding that recorded nothing here gets a usable paper form.
#pagebreak()
#subsection[#L.s31bis.title]
#align(center)[#text(size: 7.5pt, style: "italic")[#L.s31bis.subtitle]]
// Widths tuned 2026-08-10 against the rendered page: the two surface columns
// and the two date columns wrap their headers instead of reserving a line's
// width, which buys the room the intensity and observations columns needed —
// the latter was clipping its own heading.
#data-table(
  (
    0.9fr, 0.8fr, auto, 0.7fr, 0.7fr, 1fr, 1fr,
    1fr, 1fr, 0.85fr, 1.1fr, 0.8fr, 0.8fr, 0.85fr, auto, 1.3fr,
  ),
  (
    [#L.s31bis.species], [#L.s31bis.variety], [#L.s31bis.plots #super[(1)]],
    [#L.s31bis.crop_surface], [#L.s31bis.treated_surface],
    [#L.s31bis.problem], [#L.s31bis.justification],
    [#L.s31bis.measure], [#L.s31bis.intensity #super[(2)]], [#L.s31bis.measure_date],
    [#L.s31bis.product], [#L.s31bis.registration], [#L.s31bis.dose],
    [#L.s31bis.product_date],
    [#L.s31bis.efficacy], [#L.s31bis.notes],
  ),
  sys.inputs.advised.map(r => (
    [#r.species], [#r.variety], align(center)[#r.plots],
    align(right)[#r.crop_surface], align(right)[#r.treated_surface],
    [#r.problems], [#r.justification],
    [#r.measure], align(right)[#r.intensity], [#r.measure_date],
    [#r.product], [#r.reg_no], [#r.dose], [#r.product_date],
    align(center)[#r.efficacy], [#r.notes],
  )),
  size: 6.5pt,
)
#text(size: 6.5pt)[
  #super[(1)] #L.s31bis.note_plots
  #super[(2)] #L.s31bis.note_intensity
]

// The two sign-off boxes at the foot of the model's page. They ask for a
// handwritten Firma, so the book prints ruled lines and prefills only what it
// knows: the advisor named by the rows above. Nothing here is captured — the
// signature is the advisor's act, not a field the farmer fills in.
// Tall enough for the four lines it holds: the box was clipping its own date
// label, which is the one line a hand has to write on.
#let validation-box(heading, date_label) = block(
  stroke: 0.5pt,
  inset: 6pt,
  width: 100%,
  height: 96pt,
)[
  #text(size: 8pt, weight: "bold")[#heading]
  #v(2pt)
  #text(size: 7.5pt)[#L.s31bis.signature:]
  #v(14pt)
  #text(size: 7.5pt)[#L.s31bis.advisor: #sys.inputs.advised_advisor]
  #linebreak()
  #text(size: 7.5pt)[#L.s31bis.ropo: #sys.inputs.advised_ropo]
  #linebreak()
  #text(size: 7.5pt)[#date_label: ]
]
#v(6pt)
#grid(
  columns: (1fr, 1fr),
  gutter: 8pt,
  validation-box(L.s31bis.validation_interim, L.s31bis.date),
  validation-box(L.s31bis.validation_final, L.s31bis.season_end_date),
)

// ---------------------------------------------------------- 3.2 SEMILLA TRATADA
// A sowing, not an application: no applicator or equipment column, and the
// product is the text printed on the sack.
#pagebreak()
#subsection[#L.s32.title]
#block(below: 4pt)[
  #text(size: 8pt, weight: "bold")[#L.s33.applies #super[(1)]:]
  #box(
    stroke: 0.5pt,
    width: 10pt,
    height: 10pt,
    inset: 1pt,
    align(center)[#sys.inputs.seed_applies_yes],
  )
  #text(size: 8pt)[#L.value.yes]
  #h(6pt)
  #box(
    stroke: 0.5pt,
    width: 10pt,
    height: 10pt,
    inset: 1pt,
    align(center)[#sys.inputs.seed_applies_no],
  )
  #text(size: 8pt)[#L.value.no]
]
#data-table(
  (auto, auto, 1fr, 0.9fr, auto, auto, 0.9fr, 1.1fr, 0.8fr, 1.2fr, auto, 1.2fr),
  (
    [#L.s32.plots #super[(2)]], [#L.s32.date], [#L.s32.species], [#L.s32.variety],
    [#L.s32.surface], [#L.s32.seed_quantity], [#L.s32.seed_lot #super[(3)]],
    [#L.s32.product], [#L.s32.registration], [#L.s32.active_substance],
    [#L.s32.efficacy], [#L.s32.notes],
  ),
  sys.inputs.seed.map(r => (
    align(center)[#r.plots], [#r.date], [#r.species], [#r.variety],
    align(right)[#r.surface], align(right)[#r.seed_quantity], [#r.seed_lot],
    [#r.product], [#r.reg_no], [#r.active_substance],
    align(center)[#r.efficacy], [#r.notes],
  )),
  size: 7.5pt,
  blank_rows: blank-rows-for(sys.inputs.seed_applies_no),
)
#text(size: 6.5pt)[
  #super[(1)] #L.s33.note_applies
  #super[(2)] #L.s32.note_plots
  #super[(3)] #L.s32.note_seed_lot
]

// The three conditional registers share one layout; only the subject and its
// measure change per section. sys.inputs.non_field arrives in model order.
#non-field-register(
  sys.inputs.non_field.at(0),
  L.s33.title_postharvest,
  L.s33.subject_postharvest,
  L.s33.quantity_postharvest,
)
#non-field-register(
  sys.inputs.non_field.at(1),
  L.s33.title_storage,
  L.s33.subject_storage,
  L.s33.quantity_storage,
)
#non-field-register(
  sys.inputs.non_field.at(2),
  L.s33.title_transport,
  L.s33.subject_transport,
  L.s33.quantity_transport,
)

// ================================================================ 4. ANÁLISIS
// Metadata only: the register says an analysis exists and where its bulletin
// is. Keeping the bulletin itself is an art. 16.3 duty, not a column.
#pagebreak()
#section-title[#L.s4.section_title]

#subsection[#L.s4.title]
#data-table(
  (auto, auto, auto, 1fr, 1.6fr, 1.6fr, 1.2fr),
  (
    [#L.s4.date], [#L.s4.material], [#L.s4.plots #super[(1)]], [#L.s4.bulletin],
    [#L.s4.laboratory], [#L.s4.substances #super[(3)]], [#L.s31.notes],
  ),
  sys.inputs.analysis.map(r => (
    [#r.date], align(center)[#r.material], align(center)[#r.plots], [#r.bulletin],
    [#r.laboratory], [#r.substances], [#r.notes],
  )),
  size: 7.5pt,
)
#text(size: 6.5pt)[
  #super[(1)] #L.s4.note_plots
  #super[(2)] #L.s4.note_keep
  #super[(3)] #L.s4.note_soil
]

// ================================================================= 5. COSECHA
#pagebreak()
#section-title[#L.s5.section_title]

#subsection[#L.s5.title]
#data-table(
  (auto, 0.9fr, auto, auto, 1.1fr, 0.8fr, 1.3fr, 0.8fr, 1.2fr, 0.9fr),
  (
    [#L.s5.date], [#L.s5.product], [#L.s5.quantity], [#L.s5.plots #super[(1)]],
    [#L.s5.delivery_note #super[(2)]], [#L.s5.lot #super[(2)]], [#L.s5.buyer],
    [#L.s5.buyer_tax_id], [#L.s5.buyer_address], [#L.s5.buyer_registry #super[(2)]],
  ),
  sys.inputs.harvest.map(r => (
    [#r.date], [#r.product], align(right)[#r.quantity], align(center)[#r.plots],
    [#r.delivery_note], [#r.lot], [#r.buyer], [#r.buyer_tax_id],
    [#r.buyer_address], [#r.buyer_registry],
  )),
  size: 7.5pt,
)
#text(size: 6.5pt)[
  #super[(1)] #L.s5.note_plots
  #super[(2)] #L.s5.note_voluntary
]

// ========================================================== 6. FERTILIZACIÓN
//
// The record book's SECOND decree, the half that names the section: RD
// 1051/2022 art. 5.d, binding since 1 January 2026 and recorded within one
// month of each operation. The binding field list is RD 1311/2012 Anexo III
// Parte I sección C, which is WIDER than this table — C.h asks for eight
// agronomic values where the model prints three, C.i adds heavy metals for
// sludge, and C.k names a third machinery registry (REGFER).
//
// Two departures from the printed model, both because the model is orientativo
// while sección C binds:
//
//   * the "Tipo de fertilización (F)/(AF)/(AC)" cell carries the sigla AND the
//     forma de aplicación, because that footnote merges two separate legal
//     fields (C.c and C.f) and fertirrigación belongs to the second.
//   * an "applicator" column exists at all, because C.k requires the service
//     company and its REGFER number and the model has nowhere to put them.
//
// What has no column here at all: the material's full composition (it lives on
// the material's own registry entry, and the workbook publishes it) and the
// good practices the SIEX twin requires (whole sentences, no register cell
// could carry them — they take a spreadsheet column instead).
#pagebreak()
#section-title[#L.s6.section_title]

#subsection[#L.s6.title]
#data-table(
  (auto, auto, auto, 0.9fr, 1.3fr, auto, 0.9fr, auto, 1.1fr, 1fr, auto, auto, 1fr),
  (
    [#L.s6.dates], [#L.s6.plots #super[(1)]], [#L.s6.area], [#L.s6.crop],
    [#L.s6.material #super[(4)]], [#L.s6.delivery_note],
    [#L.s6.richness #super[(3)]], [#L.s6.dose], [#L.s6.kind #super[(2)]],
    [#L.s6.applicator], [#L.s6.yield_estimated], [#L.s6.yield_final],
    [#L.s31.notes],
  ),
  sys.inputs.fertilisation.map(r => (
    [#r.dates], align(center)[#r.plots], align(right)[#r.area], [#r.crops],
    [#r.material], [#r.delivery_note], [#r.richness], align(right)[#r.dose],
    [#r.kind], [#r.applicator], align(right)[#r.yield_estimated],
    align(right)[#r.yield_final], [#r.notes],
  )),
  size: 7pt,
)
#text(size: 6.5pt)[
  #super[(1)] #L.s6.note_plots
  #super[(2)] #L.s6.note_kind
  #super[(3)] #L.s6.note_richness
  #super[(4)] #L.s6.note_sludge
]

// ========================================================= 7.1 PLAN DE ABONADO
//
// RD 1051/2022 art. 4.2 requires a plan per production unit from 1 September
// 2026 (1 January 2026 for irrigated units sown 1 March–30 June). Art. 6 says
// what the plan DOCUMENT must contain — recintos, soil parameters, available
// water, the recommended dose of each nutrient with its moment, material, form
// of application and machinery, and the anexo V emission measures — and that
// document is kept beside the book, not printed here. Art. 5.a says what the
// BOOK carries, which is the recommendation this table's last block shows.
//
// The rest of the table is ASSEMBLED from section 6's own records: aportadas =
// dose × riqueza, acumuladas = their running sum per production unit. Storing
// them would let one book state two different totals for one campaign.
#pagebreak()
#section-title[#L.s71.section_title]

#subsection[#L.s71.title]
#data-table(
  (auto, 0.9fr, auto, auto, 1.1fr, 1fr, auto, 1.1fr, 1.1fr, 1.1fr),
  (
    [#L.s71.date], [#L.s71.crop], [#L.s71.plots #super[(1)]], [#L.s71.area],
    [#L.s71.fertiliser], [#L.s71.richness], [#L.s71.dose],
    [#L.s71.supplied #super[(2)(3)]], [#L.s71.accumulated #super[(2)(3)]],
    [#L.s71.recommended #super[(2)]],
  ),
  sys.inputs.plan_rows.map(r => (
    [#r.date], [#r.crop], align(center)[#r.plots], align(right)[#r.area],
    [#r.fertiliser], [#r.richness], align(right)[#r.dose],
    align(right)[#r.supplied], align(right)[#r.accumulated],
    align(right)[#r.recommended],
  )),
  size: 7.5pt,
)
#text(size: 6.5pt)[
  #super[(1)] #L.s71.note_plots
  #super[(2)] #L.s71.note_units #L.s71.note_assembled
  #super[(3)] #L.s71.note_unknown
]
#block(above: 10pt, stroke: 0.5pt, inset: 6pt, width: 100%)[
  #text(size: 7.5pt)[#L.s71.note_document]
]

// ================================================================== 8. RIEGO
//
// The record book's SECOND decree. RD 1051/2022 art. 5.e puts the doses and
// dates of irrigation inside the same cuaderno duty as fertilisation, binding
// since 1 January 2026; the binding field list is RD 1311/2012 Anexo III
// Parte I sección C, letters a, b and l.
//
// Two columns the model prints are computed, never stored: the accumulated
// volume is a running sum of this table, and a stored copy could disagree with
// the rows above it. The water-quality cell is the opposite case — the model
// has no column for C.l's nitrogen and phosphorus at all, so it rides beside
// the volume and gets its own two columns in the spreadsheet.
#pagebreak()
#section-title[#L.s8.section_title]

#subsection[#L.s8.title]
#data-table(
  (auto, auto, 1.1fr, 1.1fr, auto, auto, auto, 1fr, 1.2fr),
  (
    [#L.s8.plots #super[(1)]], [#L.s8.area], [#L.s8.method], [#L.s8.dates],
    [#L.s8.volume], [#L.s8.cumulative #super[(2)]],
    [#L.s8.water_quality #super[(3)]], [#L.s8.source], [#L.s31.notes],
  ),
  sys.inputs.irrigation.map(r => (
    align(center)[#r.plots], align(right)[#r.area], [#r.method], [#r.dates],
    align(right)[#r.volume], align(right)[#r.cumulative],
    align(right)[#r.water_quality], [#r.source], [#r.notes],
  )),
  size: 7.5pt,
)
#text(size: 6.5pt)[
  #super[(1)] #L.s8.note_plots
  #super[(2)] #L.s8.note_cumulative
  #super[(3)] #L.s8.note_water_quality
]

// ========================================= DOCUMENTACIÓN A CONSERVAR (anexo)
//
// A duty, not a register: art. 16.3 obliges keeping what backs the entries for
// at least three years, and this book holds no attachments by design. So the
// page is a plain list — tick boxes would make it look like something to fill
// in, when it is a reminder of what to file away.
#pagebreak()
#section-title[#L.annex.section_title]

#subsection[#L.annex.title]
#block(above: 10pt, below: 8pt)[#L.annex.intro]
#enum(
  spacing: 8pt,
  [#L.annex.item_invoices],
  [#L.annex.item_contracts],
  [#L.annex.item_inspections],
  [#L.annex.item_containers],
  [#L.annex.item_analyses],
  [#L.annex.item_advice],
  [#L.annex.item_sale],
  // The second decree's own documents. They are not in the printed model,
  // which predates RD 1051/2022 — but a book that lists what to keep and
  // omits the plan de abonado would mislead the holding it belongs to.
  [#L.annex.item_plan],
  [#L.annex.item_sludge],
  [#L.annex.item_manure],
)
#block(above: 14pt, stroke: 0.5pt, inset: 6pt, width: 100%)[
  #text(weight: "bold")[#L.annex.retention]
]
