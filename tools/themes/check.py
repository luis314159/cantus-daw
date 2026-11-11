"""Every promise the kit's tests hold a shipped theme to, measured on a token dict:
`Tokens::pairings` (ADR 0024 §14), `theme::tests::every_theme_meets_the_contrast_floor`
(round 25's floor and the recessed keys of round 39), the old palette's legibility test
(`every_shipped_theme_is_legible`), and the materials' character
(`tokens::tests::every_shipped_theme_defines_its_materials_and_their_character`).
"""

from colour import contrast, mix, hsl, lum

WHITE = (255, 255, 255, 255)


def pairings(t):
    v = []
    add = lambda what, fg, bg, need: v.append((what, fg, bg, need))
    for n in ["ink", "panel", "strip", "strip_selected", "panel_raised", "deck", "deck_title", "well"]:
        add("text on " + n, t["text"], t[n], 4.5)
    for n in ["panel", "strip", "deck"]:
        add("text_dim on " + n, t["text_dim"], t[n], 4.5)
    for n in ["strip", "deck", "deck_title"]:
        add("label on " + n, t["label"], t[n], 4.5)
    add("menu text on menu", t["menu_text"], t["menu"], 4.5)
    add("menu text on band", t["menu_text"], t["menu_band"], 4.5)
    add("hover text", t["menu_text_hover"], t["menu_hover"], 4.5)
    add("value pill", t["value_pill_text"], t["value_pill"], 4.5)
    add("text on its selection", t["text"], t["text_select"], 4.5)
    add("selection fill on well", t["text_select"], t["well"], 1.3)
    for f in ["hue_source", "hue_tone", "hue_dynamics", "hue_mod", "hue_space", "hue_output"]:
        for n in ["deck", "deck_title", "ink"]:
            add(f + " on " + n, t[f], t[n], 4.5)
    add("edge on deck", t["edge"], t["deck"], 3.0)
    add("knob track on deck", t["knob_track"], t["deck"], 3.0)
    rim = contrast(t["knob_rim"], t["deck"])
    boundary = t["knob_rim"] if rim > contrast(t["knob_body"], t["deck"]) else t["knob_body"]
    add("knob boundary on deck", boundary, t["deck"], 3.0)
    for n in ["deck", "panel", "strip"]:
        add("auto on " + n, t["auto"], t[n], 4.5)
        add("modulated on " + n, t["modulated"], t[n], 4.5)
    add("primary ink", t["primary_ink"], t["highlight"], 4.5)
    add("destructive words", WHITE, t["destructive"], 4.5)
    for n in ["readout", "readout_soft"]:
        if t[n][3] == 0:
            continue
        for bn in ["toolbar", "well", "panel"]:
            add(n + " on " + bn, t[n], t[bn], 4.5)
    add("focus on panel", t["focus"], t["panel"], 3.0)
    add("focus on ink", t["focus"], t["ink"], 3.0)
    add("selection on strip", t["selection"], t["strip"], 3.0)
    add("selection on panel", t["selection"], t["panel"], 3.0)
    add("placeholder on tile", t["placeholder"], t["panel_raised"], 1.8)
    return v


def floor(t):
    v = []
    add = lambda what, fg, bg, need: v.append((what, fg, bg, need))
    for n in ["meter_low", "meter_high", "meter_clip"]:
        add(n + " on its well", t[n], t["meter_well"], 3.0)
    add("meter_mid on its well", t["meter_mid"], t["meter_well"], 2.0)  # not tested; a guide
    # The meters' amber is also the warning's and the solo's ink (the top bar's device chip,
    # `views::room`): it must read on the bars.
    add("meter_high as a warning on the bar", t["meter_high"], t["toolbar"], 3.0)
    add("playhead on row_white", t["playhead"], t["row_white"], 3.0)
    add("playhead on row_black", t["playhead"], t["row_black"], 3.0)
    add("glyph on highlight", t["highlight_glyph"], t["highlight"], 4.5)
    add("text on header cell", t["text"], t["header_cell"], 4.5)
    add("text on toolbar", t["text"], t["toolbar"], 4.5)
    add("led on strip", t["led"], t["strip"], 3.0)
    add("record on strip", t["danger"], t["strip"], 3.0)
    add("selection on selected strip", t["selection"], t["strip_selected"], 3.0)
    add("pointer on knob", t["knob_pointer"], t["knob_body"], 3.0)
    # The recessed keys (cap::well_fill / well_ink).
    dark = t["dark"]
    hover = mix(t["well"], t["text"], 0.04 if dark else 0.05)
    lit = mix(t["well"], t["highlight"], 0.20 if dark else 0.16)
    add("key glyph at rest", t["text_dim"], t["well"], 4.5)
    add("key glyph hovered", t["text"], hover, 4.5)
    add("lit key glyph", t["highlight"], lit, 3.0)
    add("well against its bar", t["well"], t["toolbar"], 1.05)
    return v


def palette(t, accent, bright):
    """The old palette's test, on `palette_of(t, accent, bright)`."""
    v = []
    add = lambda what, fg, bg, need: v.append((what, fg, bg, need))
    add("P text on canvas", t["text"], t["ink"], 4.5)
    add("P text on panel", t["text"], t["panel"], 4.5)
    add("P text on card", t["text"], t["panel_raised"], 4.5)
    add("P dim on panel", t["text_dim"], t["panel"], 4.5)
    add("P accent on panel", accent, t["panel"], 4.5)
    add("P accent on canvas", accent, t["ink"], 4.5)
    add("P bright on panel", bright, t["panel"], 3.0)
    add("P focus on panel", t["focus"], t["panel"], 3.0)
    add("P focus on canvas", t["focus"], t["ink"], 3.0)
    add("P danger on panel", t["danger"], t["panel"], 4.5)
    add("P black key on white", t["key_black"], t["key_white"], 4.5)
    add("P selection on canvas", t["selection"], t["ink"], 3.0)
    add("P selection on panel", t["selection"], t["panel"], 3.0)
    add("P panel vs canvas", t["panel"], t["ink"], 1.0201)
    return v


def materials(t):
    bad = []
    for m in ["bevel", "bevel_shade", "inner_shadow", "lip", "specular", "drop"]:
        if t[m][3] == 0:
            bad.append(m + " has no strength")
    l = lambda c: hsl(c)[2]
    if not l(t["bevel"]) > l(t["bevel_shade"]):
        bad.append("bevel not lit")
    if not l(t["drop"]) < 0.3:
        bad.append("drop not a shadow")
    if not l(t["inner_shadow"]) < l(t["well"]):
        bad.append("well's edge is lit")
    if not l(t["specular"]) > 0.8:
        bad.append("specular not a highlight")
    return bad


def piano(t):
    """`views::room::Piano::of`: the keys' names on the ivory (C4 at 7:1, a C at 4.5:1)."""
    dark = t["dark"]
    ivory = mix(t["key_white"], t["panel"], 0.07 if dark else 0.33)
    ink = t["ink"] if dark else t["text"]
    return [
        ("piano C4", mix(ivory, ink, 0.92), ivory, 7.0),
        ("piano C", mix(ivory, ink, 0.72 if dark else 0.64), ivory, 4.5),
    ]


def failures(t, accent, bright, high_contrast=False):
    out = []
    for what, fg, bg, need in piano(t):
        got = contrast(fg, bg)
        if got + 1e-3 < need:
            out.append("%s: %.2f < %.1f" % (what, got, need))
    for what, fg, bg, need in pairings(t) + floor(t) + palette(t, accent, bright):
        got = contrast(fg, bg)
        if got + 1e-3 < need:
            out.append("%s: %.2f < %.1f" % (what, got, need))
    out += materials(t)
    return out
