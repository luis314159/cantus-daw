"""des-views — Neon Tide, the whole song (64 bars), for the clip-content sheet: every clip as
notes (T in beats from bar 1, length, pitch, velocity), audio envelopes and automation."""
import math

PROG = [(57, 60, 64), (53, 57, 60), (48, 52, 55), (55, 59, 62)]   # Am F C G, one a bar

SECTIONS = [(1, 'Intro'), (9, 'Verse'), (25, 'Build'), (33, 'Break'), (41, 'Drop'), (49, 'Drop'), (57, 'Outro')]

LANES = [
    # name, colour (track), kind, height, clips [(start bar, bars, label)]
    ('Lead', 'Lead', 'melody', 44, [(25, 8, 'Hook'), (41, 8, 'Hook'), (49, 8, 'Hook 2')]),
    ('Chords', 'Chords', 'chords', 30, [(1, 8, 'Pads'), (9, 16, 'Pads'), (25, 8, 'Stabs'), (33, 8, 'Pads'),
                                        (41, 8, 'Stabs'), (49, 8, 'Stabs'), (57, 8, 'Pads')]),
    ('Drums', 'Kick', 'drums', 30, [(9, 16, 'Beat'), (25, 8, 'Build'), (41, 16, 'Beat'), (57, 4, 'Beat')]),
    ('Percussion', 'Percussion', 'audio', 30, [(25, 8, 'Perc loop 124'), (41, 8, 'Perc loop 124'), (49, 8, 'Perc loop 124')]),
    ('Choir', 'Choir', 'audio2', 60, [(1, 8, 'Choir ahh'), (33, 8, 'Choir ahh'), (41, 16, 'Choir ahh'), (57, 8, 'Choir ahh')]),
    ('Cutoff', 'Chords', 'auto', 44, [(25, 8, 'Cutoff rise'), (41, 16, 'Cutoff')]),
]


def notes(kind, start, bars, label):
    """[(T, length, pitch, velocity)] with T in beats from the clip's start."""
    n = []
    for b in range(bars):
        bar = start + b
        ch = PROG[(bar - 1) % 4]
        o = b * 4
        if kind == 'melody':
            phrase = [[(0, 1.5, 76), (1.5, 0.5, 74), (2, 1, 72), (3, 1, 74)],
                      [(0, 2.5, 72), (3, 0.5, 69), (3.5, 0.5, 71)],
                      [(0, 1, 72), (1, 1, 76), (2, 1.5, 79), (3.5, 0.5, 77)],
                      [(0, 3, 76), (3, 0.5, 74)]][b % 4]
            if label == 'Hook 2' and b % 4 == 3:
                phrase = [(0, 1, 76), (1, 1, 79), (2, 2, 81)]
            n += [(o + q, ln, p, 0.78 + 0.18 * ((k + b) % 3 == 0)) for k, (q, ln, p) in enumerate(phrase)]
        elif kind == 'chords':
            if label == 'Pads':
                n += [(o, 4, p, 0.7) for p in ch + (ch[0] + 12,)]
            else:
                for k, (q, ln) in enumerate(((0, 0.5), (0.75, 0.5), (1.5, 0.75), (2.5, 0.5), (3.25, 0.5))):
                    n += [(o + q, ln, p, 0.95 if k == 0 else 0.8) for p in ch + (ch[0] + 12,)]
        elif kind == 'drums':
            build = label == 'Build'
            for q in range(4):
                n.append((o + q, 0.25, 36, 1.0))
            if build:
                step = 1 if b < 4 else 0.5 if b < 6 else 0.25
                q = 0.0
                while q < 4:
                    n.append((o + q, 0.2, 39, 0.5 + 0.06 * b))
                    q += step
            else:
                n += [(o + 1, 0.25, 39, 1.0), (o + 3, 0.25, 39, 0.96)]
                for s in range(16):
                    if s % 4 == 2:
                        n.append((o + s * 0.25, 0.4, 46, 0.9))
                    elif s % 2 == 0:
                        n.append((o + s * 0.25, 0.2, 42, 0.8))
                    else:
                        n.append((o + s * 0.25, 0.2, 42, 0.5))
    return n


def audio(kind, start, beat):
    """The peak (0..1) and the RMS (0..1) at `beat` beats into a clip."""
    if kind == 'audio':
        ph = beat % 4
        a = 0.06
        for k, p in enumerate((0, 0.75, 1.5, 2.0, 2.75, 3.5)):
            d = ph - p
            if d >= 0:
                a = max(a, (0.95 if k in (0, 3) else 0.62) * math.exp(-d * 7))
        return a, a * 0.55
    bb = beat / 4
    ph = bb % 1
    sw = 0.58 + 0.34 * math.sin(math.pi * min(1, ph * 1.15)) ** 0.8
    if start == 1:
        sw *= 0.35 + 0.65 * min(1, bb / 8)
    return sw, sw * 0.62


def sample(kind, start, beat, ch=0):
    """A waveform's sample at `beat` (for the closest zoom): a sum of partials under the
    envelope — enough to look like sound, deterministic."""
    pk, _ = audio(kind, start, beat)
    s = beat * 2.07 * 60      # seconds-ish
    if kind == 'audio':
        v = math.sin(s * 23.1) * 0.6 + math.sin(s * 57.7 + ch) * 0.3 + math.sin(s * 131.9) * 0.1
    else:
        v = math.sin(s * 11.0 + ch * 0.4) * 0.55 + math.sin(s * 16.5) * 0.3 + math.sin(s * 22.0 + 1) * 0.15
    return pk * v


def auto_points(label, bars):
    """Automation: (beat, value 0..1, tension -1..1 toward the next point)."""
    if label == 'Cutoff rise':
        return [(0, 0.18, 0.6), (bars * 4 - 2, 0.92, 0), (bars * 4, 0.92, 0)]
    pts = []
    for b in range(0, bars, 4):
        pts += [(b * 4, 0.35, -0.5), (b * 4 + 6, 0.72, 0.4), (b * 4 + 12, 0.52, 0)]
    pts.append((bars * 4, 0.40, 0))
    return pts


def auto_value(pts, beat):
    for (b0, v0, k), (b1, v1, _) in zip(pts, pts[1:]):
        if b0 <= beat <= b1:
            f = (beat - b0) / max(1e-6, b1 - b0)
            if k > 0:
                f = f ** (1 + 2 * k)
            elif k < 0:
                f = 1 - (1 - f) ** (1 - 2 * k)
            return v0 + (v1 - v0) * f
    return pts[-1][1]
