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
| `geozero.txt` | `geozero` (the MIT half of its dual licence) | `georust/geozero`, `main/LICENSE-MIT` | 2026-09-04 |
| `rusqlite_migration.txt` | `rusqlite_migration` | `cljoly/rusqlite_migration`, `master/LICENSE.txt` | 2026-09-04 |
| `typst.txt` | `typst`, `typst-layout`, `typst-pdf` | `typst/typst`, `main/LICENSE` | 2026-09-04 |

`terra-draw-maplibre-gl-adapter` names `JamesLMilner/terra-draw` as its own
repository and that repository has the only licence file, so one text covers
both packages.

These files are verbatim copies. Do not reformat, re-wrap or "tidy" them: the
whole reason they are here is to be reproduced exactly as their authors wrote
them.
