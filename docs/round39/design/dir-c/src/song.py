"""Neon Tide's second drop (bars 41-56, what Cantus opens on), note for note from
crates/cantus-ui/src/demos/house.rs, so the clips draw the real music."""
import math

BARS = 16
VEL = {"X": 120, "x": 100, "o": 78, "g": 46}

KICK, CLAP, HAT, OPEN, RIDE, RIM, CRASH = 36, 39, 42, 46, 51, 37, 49


def drums(rows, bar):
    out = []
    for pat, note in rows:
        for i, ch in enumerate(pat):
            if ch in VEL:
                out.append((bar * 4 + i * 0.25, 0.2, note, VEL[ch]))
    return out


def kick():
    return [e for i in range(BARS) for e in drums([("X...x...X...x...", KICK)], i)]


def clap():
    return [e for i in range(BARS) for e in drums(
        [("....X.......X..g" if i % 4 == 3 else "....X.......X...", CLAP)], i)]


def hats():
    out = []
    for i in range(BARS):
        out += drums([("..x...x...x...x.", OPEN), ("x...o...x...o..g", HAT),
                      ("o...o...o...o..." if i % 2 == 1 else "................", RIDE)], i)
    return out


def perc():
    out = []
    for i in range(BARS):
        out += drums([("...o..o....o.o.." if i % 2 == 0 else "...o..o......o.o", RIM)], i)
        if i % 8 == 0:
            out += drums([("X...............", CRASH)], i)
    return out


CHART = ["Am", "F", "C", "G"]
ROOT = {"Am": 33, "F": 29, "C": 36, "G": 31}
VOICE4 = {"Am": [57, 60, 64, 69], "F": [57, 60, 65, 69], "C": [60, 64, 67, 72], "G": [59, 62, 67, 71]}
VOICE3 = {"Am": [57, 60, 64], "F": [57, 60, 65], "C": [55, 60, 64], "G": [55, 59, 62]}


def bass():
    out = []
    for i in range(BARS):
        r = ROOT[CHART[i % 4]]
        for at, ln, v, o in [(0.5, .4, 100, 0), (1.5, .4, 92, 0), (2.5, .4, 98, 0),
                             (3.25, .2, 84, 12), (3.5, .4, 94, 0)]:
            out.append((i * 4 + at, ln, r + o, v))
    return out


def chords():
    return [(i * 4, 3.92, p, 92) for i in range(BARS) for p in VOICE4[CHART[i % 4]]]


def choir():
    return [(i * 4, 3.96, p, 72) for i in range(BARS) for p in VOICE3[CHART[i % 4]]]


NOTE = {"C": 0, "D": 2, "E": 4, "F": 5, "G": 7, "A": 9, "B": 11}


def melody(text, vel):
    out, at = [], 0.0
    for tok in text.replace("|", " ").split():
        acc = tok.startswith(">")
        tok = tok.lstrip(">")
        n, d = tok.split(":")
        p = 12 * (int(n[-1]) + 1) + NOTE[n[0]]
        d = float(d)
        out.append((at, d * 0.8, p, vel + (12 if acc else 0)))
        at += d
    return out, at


HOOK = ("A4:.5 C5:.25 E5:.5 A5:.25 G5:.5 E5:.5 C5:.5 D5:.5 E5:.5 "
        "| A4:.5 C5:.25 F5:.5 A5:.25 G5:.5 F5:.5 C5:.5 D5:.5 C5:.5 "
        "| G4:.5 C5:.25 E5:.5 G5:.25 G5:.5 E5:.5 C5:.5 D5:.5 E5:.5 "
        "| G4:.5 B4:.25 D5:.5 G5:.25 A5:.5 G5:.5 D5:.5 B4:.5 D5:.5")


def pluck():
    h, _ = melody(HOOK, 88)
    # four bars per pass (the hook's bars are four beats each after scaling)
    per = 16.0
    total = sum(1 for _ in h)
    out = []
    for k in range(4):
        out += [(at + k * per, d, p, v) for at, d, p, v in h]
    return out


LEAD_A = (">E5:.75 E5:.75 D5:.5 E5:1 C5:1 | >C5:.75 C5:.75 A4:.5 C5:1 D5:1 "
          "| >E5:.75 E5:.75 G5:.5 E5:1 D5:1 | >D5:.75 B4:.75 G4:.5 B4:1 D5:1")
LEAD_B = (">E5:.75 E5:.75 D5:.5 E5:1 A5:1 | >G5:.75 F5:.75 E5:.5 C5:1 D5:1 "
          "| >E5:.75 G5:.75 E5:.5 D5:1 C5:1 | >D5:1.5 B4:.5 D5:2")


def lead():
    a, _ = melody(LEAD_A, 98)
    b, _ = melody(LEAD_B, 100)
    out = []
    for k, part in enumerate([a, b, a, b]):
        out += [(at + 16 * k, d * 0.92 / 0.8, p, v) for at, d, p, v in part]
    return out


def choir_wave(n, seed=7):
    """The frozen choir: held chords ducked by the kick on every beat (house.rs `pump`,
    8 dB), as an envelope of n columns over the 16 bars: (peak, rms) pairs in 0..1."""
    out = []
    rnd = seed
    for i in range(n):
        beat = i / n * BARS * 4
        ph = beat % 1.0
        duck = 1.0 - 0.6 * math.exp(-ph * 5.0) * (1 - math.exp(-ph * 60))
        bar = beat % 4.0
        swell = 0.82 + 0.18 * math.sin(math.pi * min(1.0, bar / 0.6) / 2)
        rnd = (rnd * 1103515245 + 12345) & 0x7FFFFFFF
        jit = (rnd / 0x7FFFFFFF - 0.5) * 0.10
        pk = max(0.05, min(1.0, 0.86 * duck * swell + jit))
        out.append((pk, pk * 0.62))
    return out


TRACKS = [
    # key, name, icon, colour key, notes fn, kind
    ("kick", "Kick", "drum", "kick", kick, "drum"),
    ("clap", "Clap", "drum", "clap", clap, "drum"),
    ("hats", "Hats", "drum", "hats", hats, "drum"),
    ("perc", "Percussion", "drum", "perc", perc, "drum"),
    ("bass", "Bass", "synth", "bass", bass, "keys"),
    ("chords", "Chords", "synth", "chords", chords, "keys"),
    ("pluck", "Pluck", "synth", "pluck", pluck, "keys"),
    ("lead", "Lead", "synth", "lead", lead, "keys"),
    ("choir", "Choir", "voice", "choir", choir, "audio"),
]
