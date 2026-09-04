#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Carlos Lozano Ruiz
# SPDX-License-Identifier: AGPL-3.0-or-later

"""Checks the committed UI font against what the frontend actually renders.

Two things can go wrong with a subset font, and neither one breaks the build:
a character falls back to a system face, or an OpenType feature is silently
missing. Both look almost right, which is why they need asserting rather than
eyeballing — the subsetting that produced this file drops `zero` if
`--layout-features` is not passed, and a slashed zero that quietly became a
plain one is exactly the ambiguity the font was chosen to remove.

So this reads the font that is committed, not the one the script would build,
and fails on:

  * a missing variation axis or OpenType feature,
  * any character the frontend can put on screen that the font cannot draw.

Run it after scripts/subset-font.sh, and whenever a dictionary gains a
character. Needs fonttools (`sudo apt install fonttools`).
"""

import pathlib
import re
import sys
import unicodedata

from fontTools.ttLib import TTFont

ROOT = pathlib.Path(__file__).resolve().parent.parent
FONT = ROOT / "src" / "fonts" / "IBMPlexSansVar-Roman-subset.woff2"

# wdth is the reason this is one variable file rather than three static ones.
REQUIRED_AXES = {"wght": (100.0, 700.0), "wdth": (85.0, 100.0)}

# `liga` the fi/fl substitutions; `subs`/`sups` the subscripts the fertilisation
# register prints; `frac` the fraction forms. kern lives in GPOS, and there is
# no `calt` or `tnum` in this face at all — Plex's digits are tabular by
# construction, which is why it was chosen.
#
# `zero` is required here even though the app does not switch it on: it is the
# CANARY. Google's subsetting of this exact face keeps `tnum` and drops `zero`,
# so it is the feature that proves the cut preserved layout features at all,
# and a subset that lost it has probably lost others.
REQUIRED_GSUB = {"zero", "liga", "subs", "sups", "frac", "ccmp", "locl"}
REQUIRED_GPOS = {"kern", "mark"}

# Comments are stripped before scanning: collate.js documents its case folding
# with Greek sigmas that no user ever sees, and a checker that cannot tell a
# comment from a label would demand Greek in the font.
COMMENT = re.compile(
    r"<!--.*?-->|/\*.*?\*/|(?<![:'\"])//[^\n]*",
    re.DOTALL,
)


def rendered_characters():
    """Every non-ASCII character the frontend can put on screen."""
    found = {}
    for path in sorted((ROOT / "src").rglob("*")):
        if path.suffix not in {".js", ".svelte"} or path.name.endswith(".test.js"):
            continue
        text = COMMENT.sub(" ", path.read_text(encoding="utf-8"))
        for ch in text:
            if ord(ch) > 0x7E and ch not in found:
                found[ch] = path.relative_to(ROOT)
    return found


def main():
    if not FONT.exists():
        sys.exit(f"missing {FONT.relative_to(ROOT)} — run scripts/subset-font.sh")

    font = TTFont(FONT)
    problems = []

    axes = {a.axisTag: (a.minValue, a.maxValue) for a in font["fvar"].axes}
    for tag, want in REQUIRED_AXES.items():
        if axes.get(tag) != want:
            problems.append(f"axis {tag}: expected {want}, found {axes.get(tag)}")

    for table, required in (("GSUB", REQUIRED_GSUB), ("GPOS", REQUIRED_GPOS)):
        have = {r.FeatureTag for r in font[table].table.FeatureList.FeatureRecord}
        for tag in sorted(required - have):
            problems.append(f"{table} feature {tag!r} is missing from the subset")

    cmap = font.getBestCmap()
    for ch, where in sorted(rendered_characters().items()):
        if ord(ch) not in cmap:
            name = unicodedata.name(ch, "?")
            problems.append(f"U+{ord(ch):04X} {ch!r} ({name}) — {where}")

    print(f"{FONT.relative_to(ROOT)}: {FONT.stat().st_size:,} bytes")
    print(f"  axes     {', '.join(f'{t} {lo:g}-{hi:g}' for t, (lo, hi) in sorted(axes.items()))}")
    print(f"  glyphs   {len(font.getGlyphOrder()):,}   codepoints {len(cmap):,}")

    if problems:
        print(f"\n{len(problems)} problem(s):", file=sys.stderr)
        for p in problems:
            print(f"  {p}", file=sys.stderr)
        sys.exit(1)
    print("  every character the frontend renders is in the font")


if __name__ == "__main__":
    main()
