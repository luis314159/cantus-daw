"""Build the interface faces round 39's themes bundle (ADR 0108, ADR 0111).

    python tools/fonts/build_faces.py <fonts-dir> <out-dir>

`<fonts-dir>` holds the upstream files as published on github.com/google/fonts:
`barlow-semi-condensed/` (Barlow Semi Condensed 1.408, static TTFs) and `sofia/` (Sofia Sans
4.101, variable TTFs). Needs fontTools (MIT).

What it changes, and why (both families are SIL OFL 1.1 with no Reserved Font Name, so a
modified version may keep its name; the copyright and licence travel unchanged):

- **Tabular figures by default.** egui maps a character to a glyph through `cmap` alone and
  applies no OpenType feature, so the designs' `tnum` would be lost and a counting value
  would shift as its digits change width. Every character the font's `tnum` lookup
  substitutes is mapped straight to its tabular glyph (`zero.tf`, …).
- **Static instances of Sofia Sans** at the weights the Graphite and gold design uses
  (`fontTools.varLib.instancer`): smaller than the four variable files, and the same code
  path as every other face.
- **No hinting** (egui rasterises without it): the `fpgm`, `prep`, `cvt ` programs and the
  glyphs' instructions are dropped. Nothing else is subset: every glyph and every table that
  carries meaning stays.

The output is deterministic for the same inputs and fontTools version.
"""
import os
import sys

from fontTools.ttLib import TTFont
from fontTools.subset import Options, Subsetter
from fontTools.varLib import instancer

DIGITS = "0123456789"

BARLOW = [
    ("ExtraLight", "BarlowSemiCondensed-ExtraLight"),
    ("Light", "BarlowSemiCondensed-Light"),
    ("Regular", "BarlowSemiCondensed-Regular"),
    ("Medium", "BarlowSemiCondensed-Medium"),
    ("SemiBold", "BarlowSemiCondensed-SemiBold"),
]

# (variable source, weight, output name)
SOFIA = [
    ("SofiaSansSemiCondensed[wght].ttf", 500, "SofiaSansSemiCondensed-Medium"),
    ("SofiaSansSemiCondensed[wght].ttf", 600, "SofiaSansSemiCondensed-SemiBold"),
    ("SofiaSansSemiCondensed[wght].ttf", 650, "SofiaSansSemiCondensed-SemiBold650"),
    ("SofiaSansCondensed[wght].ttf", 600, "SofiaSansCondensed-SemiBold"),
    ("SofiaSansExtraCondensed[wght].ttf", 200, "SofiaSansExtraCondensed-ExtraLight"),
    ("SofiaSansExtraCondensed[wght].ttf", 300, "SofiaSansExtraCondensed-Light"),
]


def tnum_map(font):
    """Default glyph → tabular glyph, from every single substitution under `tnum`."""
    if "GSUB" not in font:
        return {}
    gsub = font["GSUB"].table
    out = {}
    for rec in gsub.FeatureList.FeatureRecord:
        if rec.FeatureTag != "tnum":
            continue
        for i in rec.Feature.LookupListIndex:
            lookup = gsub.LookupList.Lookup[i]
            for st in lookup.SubTable:
                if lookup.LookupType == 7:  # extension
                    st = st.ExtSubTable
                mapping = getattr(st, "mapping", None)
                if mapping:
                    for a, b in mapping.items():
                        out.setdefault(a, b)
    return out


def tabular(font):
    """Point every character whose glyph has a tabular twin at that twin. Returns how many
    characters moved; fails if a digit did not."""
    sub = tnum_map(font)
    moved = 0
    for table in font["cmap"].tables:
        if not table.isUnicode():
            continue
        for cp, glyph in list(table.cmap.items()):
            if glyph in sub:
                table.cmap[cp] = sub[glyph]
                moved += 1
    best = font.getBestCmap()
    widths = {font["hmtx"][best[ord(d)]][0] for d in DIGITS}
    if len(widths) != 1:
        raise SystemExit(f"digits are not tabular after the remap: {sorted(widths)}")
    return moved


def unhinted(font):
    opts = Options()
    opts.glyph_names = True
    opts.hinting = False
    opts.layout_features = ["*"]
    opts.name_IDs = ["*"]
    opts.name_languages = ["*"]
    opts.notdef_outline = True
    opts.recalc_bounds = False
    opts.recalc_timestamp = False
    opts.drop_tables = ["DSIG"]
    s = Subsetter(opts)
    s.populate(glyphs=font.getGlyphOrder())
    s.subset(font)


def save(font, out_dir, name):
    path = os.path.join(out_dir, name + ".ttf")
    font.save(path)
    print(f"{name}.ttf  {os.path.getsize(path):>7} bytes")


def main(src, out):
    os.makedirs(out, exist_ok=True)
    for _, stem in BARLOW:
        f = TTFont(os.path.join(src, "barlow-semi-condensed", stem + ".ttf"))
        tabular(f)
        unhinted(f)
        save(f, out, stem)
    for file, wght, stem in SOFIA:
        var = TTFont(os.path.join(src, "sofia", file))
        f = instancer.instantiateVariableFont(var, {"wght": wght}, updateFontNames=False)
        tabular(f)
        unhinted(f)
        save(f, out, stem)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    main(sys.argv[1], sys.argv[2])
