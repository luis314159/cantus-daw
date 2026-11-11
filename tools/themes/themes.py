"""The themes round 39 brings back and adds (the owner, 2026-11-10: "Recover all of them,
and add more if you want"), each as a short spec over Slate's ladder (`build.py`).

    python tools/themes/themes.py            # measure every theme, print what moved
    python tools/themes/themes.py --write    # and write crates/cantus-ui/src/theme_*.rs

Each spec: `dark`; the room's `hue`, its saturation `gain` over Slate's and its light `f`
(WCAG's (L + 0.05) × f − 0.05, so the room keeps Slate's ratios between its surfaces); the
text inks' `text_hue` and `text_gain`; the metal's `metal_gain`; the `accent` (its tones
derived as Slate's are); then `set`, the theme's own colours on top. `system` is the
Rust expression of its design system.
"""

import sys

from colour import hx, alpha, mix

N, P = True, False
CLEAR = (0, 0, 0, 0)

SPECS = {}


def spec(id_, **kw):
    kw["id"] = id_
    SPECS[id_] = kw


# ---- restored: round 25's family ---------------------------------------------------------

spec("cantus_night", name="Cantus Night", dark=N, hue=24, gain=1.6, f=0.86, text_hue=34, text_gain=1.9,
     metal_gain=0.7, accent=hx(0xe8925a), system="System::GRAPHITE",
     set=dict(
         selection=hx(0x5cc0aa), selection_tint=alpha(hx(0x5cc0aa), 38),
         readout=hx(0xf2c46e), readout_soft=hx(0xcca66c), playhead=hx(0xf5c860),
         loop_edge=hx(0xf5c860), brand=hx(0xd4622a), brand_bright=hx(0xf5ad3d),
         led=hx(0x74c482), fx_on=hx(0x74c482), danger=hx(0xf0604c), destructive=hx(0xc8503f),
         meter_low=hx(0x4fbfa3), meter_mid=hx(0x9cb273), meter_high=hx(0xeaa543),
         meter_clip=hx(0xff5a3c),
     ))
spec("cantus_paper", name="Cantus Paper", dark=P, hue=152, gain=0.8, f=0.96, text_hue=24,
     text_gain=0.75, metal_gain=0.4, accent=hx(0xa94617), system="System::GRAPHITE",
     set=dict(
         selection=hx(0x1f6f62), selection_tint=alpha(hx(0x1f6f62), 24),
         readout=hx(0x8a3a12), readout_soft=hx(0x7a4a2a), playhead=hx(0xc8561f),
         loop_edge=hx(0xc8561f), brand=hx(0xd4622a), led=hx(0x23855a), fx_on=hx(0x23855a),
         danger=hx(0xb8321f), destructive=hx(0xb8321f),
         meter_low=hx(0x1a8a6e), meter_mid=hx(0x788a1c), meter_high=hx(0xa86e06),
         meter_clip=hx(0xd6322b),
     ))
spec("daylight", name="Daylight", dark=P, hue=214, gain=3.0, f=1.0, text_hue=216, text_gain=1.4,
     metal_gain=1.2, accent=hx(0x0f62e6), veil=160, system="System::GRAPHITE",
     set=dict(
         selection_tint=alpha(hx(0x0f62e6), 16), readout=hx(0x0f55c8), readout_soft=hx(0x3a5f94),
         loop_edge=hx(0x0f62e6), led=hx(0x1e9450), fx_on=hx(0x1e9450), danger=hx(0xcc2f27),
         destructive=hx(0xcc2f27), meter_low=hx(0x1b8a52), meter_mid=hx(0x7f9a1e),
         meter_high=hx(0xa86e06), meter_clip=hx(0xd6322b),
     ))
spec("blue_hour", name="Blue Hour", dark=N, hue=216, gain=2.8, f=0.87, text_hue=214, text_gain=1.5,
     metal_gain=0.9, accent=hx(0x3d9bff), system="System::GRAPHITE",
     set=dict(
         selection_tint=alpha(hx(0x3d9bff), 44), readout=hx(0xa8cdff), readout_soft=hx(0x87a8d4),
         loop_edge=hx(0xa8cdff), brand=hx(0x4c8dff), led=hx(0x3fd17a), fx_on=hx(0x3fd17a),
         danger=hx(0xff6464), destructive=hx(0xc74e4e), meter_low=hx(0x4fd18b),
         meter_mid=hx(0xa7d26c), meter_high=hx(0xffd34d), meter_clip=hx(0xff6b4a),
     ))
spec("ultraviolet", name="Ultraviolet", dark=N, hue=264, gain=2.5, f=0.96, text_hue=258,
     text_gain=2.6, metal_gain=1.0, accent=hx(0xff5ad8), system="System::SLATE",
     set=dict(
         selection=hx(0x33e6ff), focus=hx(0x7cf3ff), selection_tint=alpha(hx(0x33e6ff), 36),
         readout=hx(0x7cf3ff), readout_soft=hx(0x6ccbe0), playhead=hx(0x33e6ff),
         loop_edge=hx(0xff5ad8), brand=hx(0xb26bff), brand_bright=hx(0xe0a8ff),
         led=hx(0x59ff9b), fx_on=hx(0x59ff9b), danger=hx(0xff5a76), destructive=hx(0xcc485e),
         meter_low=hx(0x3ff2b0), meter_mid=hx(0x9fe580), meter_high=hx(0xffd84f),
         meter_clip=hx(0xff4f6d), auto=hx(0xd29bff), modulated=hx(0xff8fd6),
     ))
spec("lagoon", name="Lagoon", dark=N, hue=186, gain=1.7, text_hue=172, text_gain=1.3,
     metal_gain=0.8, accent=hx(0x2fd3b5), system="System::SLATE",
     set=dict(
         selection=hx(0xffb454), focus=hx(0xffd28a), selection_tint=alpha(hx(0xffb454), 34),
         readout=hx(0xffc46b), readout_soft=hx(0xd6ab6c), playhead=hx(0xffb454),
         loop_edge=hx(0xffb454), brand=hx(0x3ec9b0), led=hx(0x5ce08a), fx_on=hx(0x5ce08a),
         danger=hx(0xff6b5b), destructive=hx(0xc45246), meter_low=hx(0x3fd6a8),
         meter_mid=hx(0x9fd47c), meter_high=hx(0xffd24f), meter_clip=hx(0xff6b4a),
     ))

# ---- restored: the FL-derived pair, renamed (no product's name) --------------------------

spec("slate", name="Pewter", dark=N, hue=202, gain=1.05, f=1.15, text_hue=150, text_gain=0.3,
     metal_gain=0.6, accent=hx(0xffaa54), system="System::SLATE",
     set=dict(
         ink=hx(0x1f282d), device_ground=hx(0x262f34), meter_well=hx(0x2b3439),
         # Cards sit darker than the desk, as round 24's Slate had them: the units' and the
         # sections' hues read on them at 4.5:1 without turning pastel.
         deck=hx(0x313c42), deck_title=hx(0x2b353b),
         text=hx(0xf2f4f3), text_dim=hx(0xd3d9de), label=hx(0xd3d9de), menu_text=hx(0xf2f4f3),
         value_pill_text=hx(0xf2f4f3), knob_pointer=hx(0xf2f4f3),
         selection=hx(0xc3ff5a), selection_tint=alpha(hx(0xc3ff5a), 30), playhead=hx(0xb5f060),
         text_select=hx(0x56703a),
         brand=hx(0xf2c14e), brand_bright=hx(0xffd97a), focus=hx(0x7fd4ff),
         led=hx(0x9be34f), fx_on=hx(0xffe34f), danger=hx(0xff8a6a), destructive=hx(0xc94d2a),
         meter_low=hx(0xc4fc55), meter_mid=hx(0xe2ee58), meter_high=hx(0xffdf5a),
         meter_clip=hx(0xff8d3e), auto=hx(0xd3c4fd), modulated=hx(0xf8badb),
     ))
spec("paper", name="Chalk", dark=P, hue=203, gain=1.0, f=0.86, text_hue=203, text_gain=1.0,
     metal_gain=0.6, accent=hx(0x286a0b), system="System::SLATE",
     set=dict(
         selection_tint=alpha(hx(0x286a0b), 24), playhead=hx(0x3f8a00),
         text_select=hx(0xe6f5d6), readout=hx(0x7a5300), readout_soft=hx(0x5b5f22),
         # Cards lighter than the desk (round 24's Paper): the units' accents at 4.5:1.
         deck=hx(0xeaeff2), deck_title=hx(0xe4eaee),
         brand=hx(0x956608), focus=hx(0x0a6cb4), led=hx(0x3f8a00), fx_on=hx(0x3f8a00),
         danger=hx(0xc4321a), destructive=hx(0xc4321a), meter_low=hx(0x4a8a00),
         meter_mid=hx(0x7d8c00), meter_high=hx(0xa36a04), meter_clip=hx(0xd6322b),
     ))

# ---- restored: the first themes, ink and gold and parchment ------------------------------

spec("ink_gold", name="Ink & Gold", dark=N, hue=228, gain=1.8, f=0.87, text_hue=42, text_gain=1.4,
     metal_gain=0.5, accent=hx(0xf2c14e), system="System::STUDIO_INK",
     set=dict(
         selection=hx(0x7fb2ff), selection_tint=alpha(hx(0x7fb2ff), 34), focus=hx(0xffe4a8),
         readout=hx(0xf2c14e), readout_soft=hx(0xc9a55a), loop_edge=hx(0xf2c14e),
         brand=hx(0xd4a24c), led=hx(0x5cb86e), fx_on=hx(0xf2c14e), danger=hx(0xe56464),
         destructive=hx(0xc15353), meter_low=hx(0x5cb86e), meter_mid=hx(0x98ad5d),
         meter_high=hx(0xd4a24c), meter_clip=hx(0xe55d5d),
     ))
spec("parchment", name="Parchment", dark=P, hue=40, gain=1.9, f=0.90, text_hue=30, text_gain=1.2,
     metal_gain=1.6, accent=hx(0x8a4810), system="System::STUDIO_INK",
     set=dict(
         selection_tint=alpha(hx(0x8a4810), 26), readout=hx(0x6b4506), readout_soft=hx(0x7a5a2a),
         playhead=hx(0xad7810), loop_edge=hx(0xad7810), led=hx(0x2f7d3a), fx_on=hx(0x2f7d3a),
         danger=hx(0xb03333), destructive=hx(0xb03333), meter_low=hx(0x2a7d3e),
         meter_mid=hx(0x7a8a1c), meter_high=hx(0x9a6506), meter_clip=hx(0xc93434),
     ))

# ---- new in round 39 ---------------------------------------------------------------------

spec("evergreen_night", name="Evergreen Night", dark=N, hue=150, gain=1.9, text_hue=46,
     text_gain=1.3, metal_gain=0.5, metal_hue=40, accent=hx(0xd9b45a),
     system="System { cables: CableStyle::Jack, ..System::GRAPHITE }",
     set=dict(
         selection_tint=alpha(hx(0xd9b45a), 36), readout=hx(0xe6c878), readout_soft=hx(0xc2a86c),
         loop_edge=hx(0xd9b45a), led=hx(0x6fd08f), fx_on=hx(0x6fd08f), danger=hx(0xff6a5c),
         destructive=hx(0xc8443f), meter_low=hx(0x62d08c), meter_mid=hx(0xb9cf6a),
         meter_high=hx(0xe6b84d), meter_clip=hx(0xff5d50),
     ))
spec("evergreen_paper", name="Evergreen Paper", dark=P, hue=96, gain=1.25, f=0.92, text_hue=150,
     text_gain=0.9, metal_gain=0.6, metal_hue=60, accent=hx(0x23704a),
     system="System { cables: CableStyle::Jack, ..System::GRAPHITE }",
     set=dict(
         selection_tint=alpha(hx(0x23704a), 24), readout=hx(0x6e5008), readout_soft=hx(0x7a5a04),
         playhead=hx(0x9a6e1c), loop_edge=hx(0x9a6e1c), led=hx(0x1f8a4c), fx_on=hx(0x1f8a4c),
         danger=hx(0xc23a2e), destructive=hx(0xc23a2e), meter_low=hx(0x1f8a4c),
         meter_mid=hx(0x7d8c22), meter_high=hx(0xa06c06), meter_clip=hx(0xd6322b),
     ))
spec("bordeaux", name="Bordeaux", dark=N, hue=345, gain=1.6, f=0.92, text_hue=24, text_gain=0.9,
     metal_gain=0.5, accent=hx(0xeba487), veil=130, system="System::GRAPHITE",
     set=dict(
         selection_tint=alpha(hx(0xeba487), 36), readout=hx(0xedd3a4), readout_soft=hx(0xc9b08a),
         playhead=hx(0xe9c98e), loop_edge=hx(0xe9c98e), led=hx(0x6fd39a), fx_on=hx(0x6fd39a),
         danger=hx(0xff6b6b), destructive=hx(0xc8443f), meter_low=hx(0x62d39a),
         meter_mid=hx(0xc9d36a), meter_high=hx(0xebb64d), meter_clip=hx(0xff5d50),
     ))
spec("print", name="Print", dark=P, hue=0, gain=0.0, f=1.02, text_hue=0, text_gain=0.0,
     metal_gain=0.0, accent=hx(0x1c1c1c),
     system="System { cables: CableStyle::Subway, ..System::STUDIO_INK }",
     set=dict(
         selection_tint=alpha(hx(0x1c1c1c), 20), playhead=hx(0xd8371f), loop_edge=hx(0xd8371f),
         brand=hx(0xd8371f), brand_bright=hx(0xe4573f), led=hx(0x1c1c1c), fx_on=hx(0x1c1c1c),
         danger=hx(0xc8301c), destructive=hx(0xc8301c), meter_low=hx(0x6b6b6b),
         meter_mid=hx(0x444444), meter_high=hx(0x1d1d1d), meter_clip=hx(0xd8371f),
         hue_source=hx(0x3e4f6b), hue_tone=hx(0x2f5552), hue_dynamics=hx(0x5e4a22),
         hue_mod=hx(0x54426e), hue_space=hx(0x6b3a52), hue_output=hx(0x45542c),
         auto=hx(0x5a3fa8), modulated=hx(0x99306f),
     ))

if __name__ == "__main__":
    import report

    report.main(SPECS, "--write" in sys.argv)
