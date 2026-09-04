#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Re-cuts src/fonts/IBMPlexSansVar-Roman-subset.woff2 from the upstream release.
#
# The app's UI font is IBM Plex Sans, shipped as ONE variable file rather than
# one static file per weight: the app uses 400, 600 and 700 today, and three
# static subsets came to 94 KB against this file's 102 KB — 8 KB for any weight
# we might want later, plus a `wdth` axis (85-100%) no set of static weights
# can offer.
#
# WHICH UPSTREAM, because there are three and only one is right:
#   - @ibm/plex-sans-variable  the vendor's own, current per-family line   <-- this
#   - @ibm/plex                the retired monolith; its variable is v1.001 (2023)
#   - google/fonts             a Google-built fork, v3.201, `wdth` down to 75
# The Google build is a later font version and condenses further, but it is a
# raw file on a branch rather than a released package, and after subsetting the
# two differ by 8 bytes. A pinned package version is worth more than 10 points
# of an axis we do not intend to use.
#
# WHY SUBSET AT ALL: the complete file is 230 KB and most of it is Cyrillic and
# Greek this app will never render. The cut keeps every Latin script in the EU,
# so a future market is a translation and not a font decision.
#
# THE `--layout-features='*'` IS LOAD-BEARING. A subset that drops OpenType
# features still renders text, so the loss is invisible until someone looks for
# a slashed zero and finds a plain one. Google's own subsetting of this face
# drops `zero`, which is how this was discovered. `scripts/check-font.py`
# re-asserts what survived; run it after this.
#
# Needs fonttools (`sudo apt install fonttools`), which is a local tool and not
# a project dependency — the committed .woff2 is what the build consumes.

set -euo pipefail
cd "$(dirname "$0")/.."

PACKAGE="@ibm/plex-sans-variable@0.2.0"
OUT="src/fonts/IBMPlexSansVar-Roman-subset.woff2"

# Latin for every EU market, plus exactly the non-Latin characters the app puts
# on screen. The subscripts are not decoration: the fertilisation register
# prints P₂O₅ and K₂O, and a Latin-only cut drops them silently.
#
#   U+0000-024F  Basic Latin, Latin-1, Latin Extended-A and -B
#   U+0300-036F  combining marks, so `ccmp` can still compose
#   U+2000-206F  general punctuation: – — ' ' " " …
#   U+2070-209F  super/subscripts: m², P₂O₅
#   U+20A0-20BF  currency, for €
#   U+FB00-FB04  the fi/fl ligatures `liga` substitutes into
UNICODES='U+0000-024F,U+0300-036F,U+2000-206F,U+2070-209F,U+20A0-20BF,U+2122,U+2190-2199,U+2212,U+2260-2265,U+2713-2715,U+26A0,U+FB00-FB04'

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "fetching $PACKAGE"
(cd "$WORK" && npm pack "$PACKAGE" >/dev/null && tar xzf ./*.tgz)

SRC="$WORK/package/fonts/complete/woff2/IBM Plex Sans Var-Roman.woff2"
[ -f "$SRC" ] || { echo "upstream layout changed: $SRC not in the package" >&2; exit 1; }

pyftsubset "$SRC" \
  --unicodes="$UNICODES" \
  --layout-features='*' \
  --name-IDs='*' \
  --flavor=woff2 \
  --output-file="$OUT"

# The licence travels with the font, as OFL-1.1 requires. The About panel reads
# this same file through src/lib/thirdParty.js.
cp "$WORK/package/fonts/complete/woff2/license.txt" src/fonts/LICENSE
chmod 644 "$OUT" src/fonts/LICENSE

echo "wrote $OUT ($(stat -c%s "$OUT") bytes, from $(stat -c%s "$SRC") upstream)"
echo "now run: python3 scripts/check-font.py"
