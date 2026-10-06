<!--
SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
SPDX-License-Identifier: AGPL-3.0-or-later
-->

# Licence texts that do not travel with their package

Most libraries ship their licence inside the package: a crate carries
`LICENSE-MIT` / `LICENSE-APACHE` in the registry tarball, an npm package carries
`LICENSE`. `scripts/gen-third-party.mjs` reads those directly, so nothing about
them lives here.

A few do not. They declare a licence in their metadata and publish no text with
it — usually because the file is excluded from the published tarball rather than
missing from the project. **Their notices are still owed**, so the file is taken
from the project's own repository once, by hand, and kept here verbatim.

The generator prefers a package's own shipped file and falls back to this
directory. **A package with neither fails the generator** rather than producing
a panel that quietly attributes nothing — that refusal is the whole point of
this directory existing instead of the gap being papered over.

| File | Covers | Taken from | On |
| --- | --- | --- | --- |
| `terra-draw.txt` | `terra-draw`, `terra-draw-maplibre-gl-adapter` | `JamesLMilner/terra-draw`, `main/LICENSE` | 2026-09-04 |
| `geozero.txt` | `geozero` (the MIT half of its dual licence, the one shown) | `georust/geozero`, `main/LICENSE-MIT` | 2026-09-04 |
| `rusqlite_migration.txt` | `rusqlite_migration` | `cljoly/rusqlite_migration`, `master/LICENSE.txt` | 2026-09-04 |
| `typst.txt` | `typst`, `typst-layout`, `typst-pdf` | `typst/typst`, `main/LICENSE` | 2026-09-04 |
| `sqlite.txt` | SQLite, compiled in through `libsqlite3-sys` | two SQLite texts, in order: the "SQLite Is Public Domain" section of `sqlite.org/copyright.html`, and the header of the `sqlite3.c` amalgamation that `libsqlite3-sys` 0.38.2 bundles | 2026-09-16 |

Every file above was compared with its source on 2026-09-16, and the four taken
on 2026-09-04 were still byte-identical.

**Only the licence a package is shown under is kept.** A dual-licensed package
appears in the About panel under the one option taken (`licenceShown` in
`src/lib/thirdParty.js`: MIT wherever it is offered), so its other half is not
owed. `geozero-apache.txt`, geozero's Apache-2.0 half, was kept here until
2026-09-27 for that reason and removed then.

`terra-draw-maplibre-gl-adapter` names `JamesLMilner/terra-draw` as its own
repository and that repository has the only licence file, so one text covers
both packages.

**`sqlite.txt` is not a fallback.** SQLite is not a package at all: it is
compiled into the binary, with no package directory for the generator to scan,
so `src/lib/thirdParty.js` names this file directly (`licenceFile`) instead of
through the generator's fallback list. It is the one file here joined from two
sources — the public-domain statement and the blessing that stands in the source
code for a legal notice — each copied verbatim; the page's HTML wrapping is not
part of the text, so the first part is re-wrapped. It replaced a copy of
2026-09-04 that missed one sentence of the page and paraphrased the line
introducing the blessing.

A package that ships its own text needs nothing here, even when a copy was once
taken: `jiff` looked as if it published only its MIT half and was given a copy
of its Unlicense, until it turned out to ship both (removed 2026-09-16).

These files are verbatim copies. Do not reformat, re-wrap or "tidy" them: the
whole reason they are here is to be reproduced exactly as their authors wrote
them. Re-compare them with their sources when a package they cover changes
version, and move the date in the table when you do.
