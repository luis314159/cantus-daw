"""Slate's tokens (ADR 0111, `crates/cantus-ui/src/theme_slate.rs`) and the logo's gold,
as the dancer sheets use them."""

NIGHT = {
    "name": "Night",
    "gutter": "#14191E", "well": "#1A1F25", "base": "#1D2329", "surface": "#262D34",
    "raised": "#2B3239", "strip_sel": "#2F373F", "cap": "#434C56", "edge": "#11151A",
    "grid": "#21282E", "grid_bar": "#323B44", "sep": "#59636E",
    "ink1": "#E3E8ED", "ink2": "#B4BDC6", "ink3": "#909CA8", "ink4": "#47515B",
    "accent": "#5CAFE9",                      # Slate's state accent (azure)
    "gold": "#E2B85A", "gold_deep": "#C9A55A", # logo #03 / Lapis readouts and playhead
    "verm": "#F0784A",                        # the logo's vermilion bars (Night tone)
    "play": "#5DCE8A", "rec": "#EE5C5A", "solo": "#EBB64D",
    "shadow": "#0B0E11",
    "skin": "#C8977A", "skin_shade": "#A97B62",
}

PAPER = {
    "name": "Paper",
    "gutter": "#C5CDD5", "well": "#D6DDE3", "base": "#F4F6F8", "surface": "#E4E9EE",
    "raised": "#FBFCFD", "strip_sel": "#DDE3E8", "cap": "#FBFCFD", "edge": "#B9C3CC",
    "grid": "#E9EDF1", "grid_bar": "#C8D0D8", "sep": "#A3ADB8",
    "ink1": "#17202A", "ink2": "#38444F", "ink3": "#556170", "ink4": "#A9B3BD",
    "accent": "#1B64B2",
    "gold": "#9A6E1C", "gold_deep": "#7A5A04",
    "verm": "#D2541A",
    "play": "#1F8F55", "rec": "#D33B3B", "solo": "#A86E06",
    "shadow": "#8C98A4",
    "skin": "#C8977A", "skin_shade": "#A97B62",
}

THEMES = {"night": NIGHT, "paper": PAPER}


def luminance(hexc):
    h = hexc.lstrip("#")
    r, g, b = (int(h[i:i + 2], 16) / 255 for i in (0, 2, 4))

    def ch(c):
        return c / 12.92 if c <= 0.03928 else ((c + 0.055) / 1.055) ** 2.4
    return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b)


def contrast(a, b):
    la, lb = luminance(a), luminance(b)
    hi, lo = max(la, lb), min(la, lb)
    return (hi + 0.05) / (lo + 0.05)


def mix(a, b, t):
    pa = [int(a.lstrip("#")[i:i + 2], 16) for i in (0, 2, 4)]
    pb = [int(b.lstrip("#")[i:i + 2], 16) for i in (0, 2, 4)]
    return "#" + "".join(f"{round(x + (y - x) * t):02X}" for x, y in zip(pa, pb))
