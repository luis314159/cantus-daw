"""Studio ink — the tokens of both themes.

Night: the ground is ink (a near-black with a cold breath of blue-green, never brown);
content is light — notes, waveforms, meters and curves glow in the user's hue.
Paper: the ground is paper (a neutral, faintly warm white); content is ink — the same
hues taken deep and printed on pale tints of themselves.
In both, chrome is set in three inks and hairlines, and colour means state.
"""
from common import mix, desat

NIGHT = dict(
    name="Night",
    # surfaces, deepest first
    well="#08090B",      # lanes, meter tracks, graph grounds: where content lives
    base="#0E1013",      # panels: the arrangement's header column, the mixer, inspector
    bar="#13161A",       # the top bar, tab rows, status line
    raised="#1A1E23",    # hover, the selected row/strip, open menus
    field="#20252B",     # fields and pressed keys
    float="#171A1F",     # floating panels (the overflow, menus)
    track="#08090B",     # meter and fader slots, slider tracks
    # lines
    hair="#1D2127",      # between areas: barely there
    line="#2A3038",      # control outlines, menu borders
    grid_beat="#101318",
    grid_bar="#181C22",
    grid_4="#22272E",
    # inks
    ink1="#E7EBEF",      # names, values the user reads
    ink2="#A2ABB5",      # secondary text, glyphs at rest
    ink3="#7A838E",      # captions, units, numbers in passing
    ink4="#4B535D",      # disabled, ghosts
    # state
    accent="#62B4FF",    # on, selected, focus: the one pen
    accent_bg="#13283D", # the ground of a key that is on
    play="#42DD8F",
    rec="#FF5A4E",
    solo="#F2C24F",
    warn="#F2A33A",
    clip="#FF4A3D",
    # meters
    m_lo="#3FD39B", m_mid="#D9E264", m_hi="#FF6B4A",
    shadow="rgba(0,0,0,0.55)",
)

PAPER = dict(
    name="Paper",
    well="#FFFFFF",
    base="#F6F6F4",
    bar="#EEEEEB",
    raised="#E7E7E3",
    field="#FFFFFF",
    float="#FBFBFA",
    track="#E2E2DD",
    hair="#DCDCD7",
    line="#C6C6C0",
    grid_beat="#F3F3F1",
    grid_bar="#E6E6E2",
    grid_4="#D6D6D1",
    ink1="#16191D",
    ink2="#454B53",
    ink3="#6A707A",
    ink4="#A9ADB3",
    accent="#1E64D0",
    accent_bg="#DDE8F8",
    play="#0B8A47",
    rec="#CC2B20",
    solo="#8F6400",
    warn="#B86A00",
    clip="#D92B1E",
    m_lo="#0A8457", m_mid="#857400", m_hi="#C9381C",
    shadow="rgba(40,40,30,0.18)",
)

# The song's colours (cantus-ui demos/song.rs `colors`), as the user chose them.
USER = dict(
    kick="#E06A3A", clap="#E88A3C", hats="#E6B048", perc="#D95D5D",
    bass="#8A5CD6", chords="#4FA3C7", pluck="#5CB86E", lead="#C96F9E",
    choir="#4A8FD8", rise="#6CC8B4", bus="#B05648",
)


def hue(t, c, role, sel=False):
    """The user's colour spent for one role. Muted at rest; lit when selected."""
    night = t["name"] == "Night"
    if night:
        if role == "stripe":
            return c if sel else mix(desat(c, 0.25), t["base"], 0.30)
        if role == "body":
            return mix(c, t["well"], 0.80 if sel else 0.84)
        if role == "edge":
            return mix(c, t["well"], 0.25 if sel else 0.42)
        if role == "note":
            return mix(c, "#FFFFFF", 0.42 if sel else 0.30)
        if role == "note_dim":
            return mix(c, t["well"], 0.35)
        if role == "label":
            return mix(c, "#FFFFFF", 0.55)
        if role == "cap":
            return c if sel else mix(desat(c, 0.2), t["base"], 0.25)
    else:
        if role == "stripe":
            return c if sel else mix(desat(c, 0.15), t["base"], 0.15)
        if role == "body":
            return mix(c, "#FFFFFF", 0.80 if sel else 0.86)
        if role == "edge":
            return mix(c, "#FFFFFF", 0.30 if sel else 0.45)
        if role == "note":
            return mix(desat(c, 0.05), "#101418", 0.30 if sel else 0.36)
        if role == "note_dim":
            return mix(c, "#FFFFFF", 0.45)
        if role == "label":
            return mix(c, "#101418", 0.55)
        if role == "cap":
            return c if sel else mix(c, t["base"], 0.15)
    raise KeyError(role)
