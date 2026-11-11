"""The machinery that makes a theme of round 39's design system from a short spec: Slate's
own ladder (`theme_slate.rs`, read from the source) turned to a theme's hue, colour and
light, its text inks turned to theme's ink, its accent's tones derived as Slate derives
its own, then the theme's own colours on top — and every promise of the kit measured
(`check.py`) before anything is written.
"""

import re
from pathlib import Path

from colour import hx, to_hex, mix, alpha, turn, ensure_all, contrast, lum

ROOT = Path(__file__).resolve().parents[2]
SLATE = ROOT / "crates/cantus-ui/src/theme_slate.rs"

WHITE = (255, 255, 255, 255)
BLACK = (0, 0, 0, 255)

# The room: every surface, well and line of Slate's ladder (as Slate Lapis turns them).
ROOM = [
    "ink", "well", "panel", "toolbar", "panel_raised", "panel_light", "grid", "grid_strong",
    "grid_bar", "edge", "placeholder", "meter_well", "strip", "strip_selected", "header_cell",
    "device_ground", "deck", "deck_title", "value_pill", "step_odd", "step_even", "row_white",
    "row_black", "menu", "menu_band", "disabled", "inner_shadow", "lip", "knob_body",
]
# The metal and the keys: caps, rims, the keyboard.
METAL = [
    "cap", "knob_rim", "knob_track", "fcap_hi", "fcap_mid", "fcap_lo", "cap_hi", "cap_mid",
    "cap_lo", "key_white", "key_black",
]
# The text inks.
TEXT = ["text", "text_dim", "label", "knob_pointer", "menu_text", "value_pill_text", "menu_text_hover"]
HUES = ["hue_source", "hue_tone", "hue_dynamics", "hue_mod", "hue_space", "hue_output"]


def parse_slate():
    """Slate Night's and Paper's tokens, in their field order, from the source."""
    src = SLATE.read_text(encoding="utf-8")
    out = []
    for body in re.findall(r"Tokens \{(.*?)\n    \}\n", src, re.S):
        t = {}
        for line in body.splitlines():
            m = re.match(r"\s*(\w+): (.*?),(\s*//.*)?$", line)
            if not m:
                continue
            k, v = m.group(1), m.group(2)
            if v.startswith("hex(0x"):
                t[k] = hx(int(v[6:12], 16))
            elif v.startswith("Color32::from_rgba_unmultiplied_const("):
                nums = [int(x, 0) for x in v[v.index("(") + 1 : v.rindex(")")].split(",")]
                t[k] = tuple(nums)
            elif v == "Color32::TRANSPARENT":
                t[k] = (0, 0, 0, 0)
            elif v in ("true", "false"):
                t[k] = v == "true"
            else:
                t[k] = v
        out.append(t)
    return out[0], out[1]


SLATE_NIGHT, SLATE_PAPER = parse_slate()
FIELDS = list(SLATE_NIGHT.keys())


def tones(t, accent):
    """The accent's tones, as Slate (and Slate Lapis) derive their own."""
    if t["dark"]:
        bright = mix(WHITE, accent, 0.70)
        glyph = mix(BLACK, accent, 0.143)
        t.update(
            brand=accent, brand_bright=bright, highlight=accent,
            highlight_band=mix(WHITE, accent, 0.587), highlight_glyph=glyph,
            selection=accent, focus=accent, step_odd_lit=accent, step_even_lit=bright,
            menu_hover=mix(t["menu"], accent, 0.30), primary_ink=glyph,
            text_select=mix(BLACK, accent, 0.35),
        )
    else:
        half = mix(WHITE, accent, 0.5)
        t.update(
            brand=accent, brand_bright=mix(WHITE, accent, 0.84), highlight=accent,
            highlight_band=half, highlight_glyph=WHITE, selection=accent, focus=accent,
            step_odd_lit=accent, step_even_lit=mix(WHITE, accent, 0.78), menu_hover=accent,
            primary_ink=WHITE, text_select=half,
        )


def make(spec):
    """A theme's tokens from its spec (see `themes.py`)."""
    dark = spec["dark"]
    base = SLATE_NIGHT if dark else SLATE_PAPER
    t = dict(base)
    hue, gain, f = spec["hue"], spec.get("gain", 1.0), spec.get("f", 1.0)
    for k in ROOM:
        t[k] = turn(base[k], hue, gain, f)
    mh = spec.get("metal_hue", hue)
    for k in METAL:
        t[k] = turn(base[k], mh, spec.get("metal_gain", 0.6), spec.get("metal_f", f))
    th = spec.get("text_hue", hue)
    tf = spec.get("text_f", 1.0 if dark else f)
    for k in TEXT:
        t[k] = turn(base[k], th, spec.get("text_gain", 1.0), tf)
    tones(t, spec["accent"])
    t["track_veil"] = alpha(t["panel"], spec.get("veil", 150))
    for k, v in spec.get("set", {}).items():
        t[k] = v
    t["system"] = spec["system"]
    fix(t, spec)
    return t


def fix(t, spec):
    """Hold every promise the least that holds it (the kit's own `floor()`, written into
    the theme rather than applied at run time); record what moved."""
    moved = spec.setdefault("moved", [])

    def hold(k, bgs, need):
        c = ensure_all(t[k], [t[b] if isinstance(b, str) else b for b in bgs], need)
        if c[:3] != t[k][:3]:
            moved.append("%s %s -> %s" % (k, to_hex(t[k]), to_hex(c)))
            t[k] = c[:3] + (t[k][3],)

    on_text = ["panel", "strip", "strip_selected", "panel_raised", "deck", "deck_title", "well",
               "ink", "header_cell", "toolbar", "text_select"]
    hold("text", on_text, 4.5)
    lit_hover = mix(t["well"], t["text"], 0.04 if t["dark"] else 0.05)
    if contrast(t["text"], lit_hover) < 4.5:
        hold("text", on_text + [lit_hover], 4.5)
    hold("text_dim", ["panel", "strip", "deck", "well"], 4.5)
    hold("label", ["strip", "deck", "deck_title"], 4.5)
    hold("menu_text", ["menu", "menu_band"], 4.5)
    hold("menu_text_hover", ["menu_hover"], 4.5)
    hold("value_pill_text", ["value_pill"], 4.5)
    hold("knob_pointer", ["knob_body"], 3.0)
    hold("edge", ["deck"], 3.0)
    hold("knob_track", ["deck"], 3.0)
    if max(contrast(t["knob_rim"], t["deck"]), contrast(t["knob_body"], t["deck"])) < 3.0:
        hold("knob_rim", ["deck"], 3.0)
    for k in ["auto", "modulated"]:
        hold(k, ["deck", "panel", "strip"], 4.5)
    for k in HUES:
        hold(k, ["deck", "deck_title", "ink"], 4.5)
    hold("primary_ink", ["highlight"], 4.5)
    hold("highlight_glyph", ["highlight"], 4.5)
    for k in ["readout", "readout_soft"]:
        if t[k][3]:
            hold(k, ["toolbar", "well", "panel"], 4.5)
    hold("focus", ["panel", "ink"], 3.0)
    hold("selection", ["strip", "panel", "strip_selected", "ink"], 3.0)
    hold("playhead", ["row_white", "row_black"], 3.0)
    for k in ["meter_low", "meter_high", "meter_clip"]:
        hold(k, ["meter_well"], 3.0)
    hold("led", ["strip"], 3.0)
    hold("danger", ["strip", "panel"], 4.5)
    if contrast(WHITE, t["destructive"]) < 4.5:
        hold("destructive", [WHITE], 4.5)
    lit = mix(t["well"], t["highlight"], 0.20 if t["dark"] else 0.16)
    if contrast(t["highlight"], lit) < 3.0:
        moved.append("lit key glyph %.2f" % contrast(t["highlight"], lit))
    hold("text_select", ["text"], 4.5)


def rs_colour(c):
    if c[3] == 0:
        return "Color32::TRANSPARENT"
    if c[3] == 255:
        return "hex(0x%s)" % to_hex(c)
    return "alpha(0x%s, %d)" % (to_hex(c), c[3])
