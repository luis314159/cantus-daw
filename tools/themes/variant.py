"""A theme's spec as a theme file, to judge a variant on the whole window before it is
written into the code (round 42, `violet-paper`):

    cargo run -p cantus-ui --example shoot_violet -- <dir> --map
    python tools/themes/variant.py <dir> <name> <spec-id> key=value …
    cargo run -p cantus-ui --example shoot_violet -- <dir> arrangement --themes <dir>/<name>.json

`key=value` overrides the spec's numbers (`hue=272 gain=1.8 f=0.97 text_gain=1.2 …`) or,
as `set.<token>=rrggbb[aa]`, one of its own colours; the theme is built and measured as
`themes.py` builds and measures the shipped ones, and written only if it keeps every promise.
"""

import json
import sys
from pathlib import Path

import build
import check
import report
import themes
from colour import hx, to_hex


def hex_of(c):
    return "#" + to_hex(c) + ("" if c[3] == 255 else "%02x" % c[3])


PALETTE = {
    "ink": "ink", "panel": "panel", "panel_raised": "panel_raised", "grid": "grid",
    "grid_strong": "grid_strong", "grid_bar": "grid_bar", "text": "text",
    "text_dim": "text_dim", "focus": "focus", "shadow": "shadow", "selection": "selection",
    "danger": "danger", "ok": "led", "black_key": "key_black", "white_key": "key_white",
    "row_black": "row_black", "row_white": "row_white", "meter_low": "meter_low",
    "meter_high": "meter_high", "meter_clip": "meter_clip",
}


def main(dir_, name, spec_id, overrides):
    dir_ = Path(dir_)
    s = dict(themes.SPECS[spec_id])
    s["set"] = dict(s.get("set", {}))
    s.pop("moved", None)
    for o in overrides:
        k, v = o.split("=", 1)
        if k.startswith("set."):
            v = v.lstrip("#")
            s["set"][k[4:]] = hx(int(v[:6], 16), int(v[6:], 16) if len(v) > 6 else 255)
        elif k == "accent":
            s[k] = hx(int(v.lstrip("#"), 16))
        else:
            s[k] = float(v)
    t = build.make(s)
    acc, bright = report.palette_accents(t)
    bad = check.failures(t, acc, bright)
    for m in s.get("moved", []):
        print("moved", m)
    if bad:
        print("\n".join(bad))
        sys.exit(1)
    fields = dict(json.loads((dir_ / "tokens-map.json").read_text()))
    theme = json.loads((dir_ / "violet_paper.theme.json").read_text(encoding="utf-8"))
    theme["name"] = name
    for f, key in fields.items():
        if f in t and isinstance(t[f], tuple):
            theme["kit"][key] = hex_of(t[f])
    for pf, tf in PALETTE.items():
        theme[pf] = hex_of(t[tf])
    theme["accent"] = hex_of(acc)
    theme["accent_bright"] = hex_of(bright)
    out = dir_ / (name + ".json")
    out.write_text(json.dumps(theme, indent=2), encoding="utf-8")
    print(out, "panel", to_hex(t["panel"]), "bar", to_hex(t["toolbar"]), "well",
          to_hex(t["well"]), "text", to_hex(t["text"]), "accent", to_hex(t["highlight"]))


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4:])
