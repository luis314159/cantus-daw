"""Neon Tide, bars 33–56: the break and the second drop — the clips' content as notes
(bar, start in beats, length in beats, pitch, velocity) or audio envelopes."""
import math

PROG = [(57, 60, 64), (53, 57, 60), (48, 52, 55), (55, 59, 62)]   # Am F C G, one a bar

TRACKS = [
    # name, kind, clips [(start bar, bars, label)]
    ('Kick', 'drum', [(41, 8, 'Kick'), (49, 8, 'Kick')]),
    ('Clap', 'drum', [(33, 8, 'Clap fill'), (41, 8, 'Clap'), (49, 8, 'Clap')]),
    ('Hats', 'drum', [(33, 8, 'Hats soft'), (41, 8, 'Hats'), (49, 8, 'Hats')]),
    ('Percussion', 'audio', [(41, 8, 'Perc loop 124'), (49, 8, 'Perc loop 124')]),
    ('Bass', 'synth', [(41, 8, 'Bass'), (49, 8, 'Bass')]),
    ('Chords', 'synth', [(33, 8, 'Pads'), (41, 8, 'Stabs'), (49, 8, 'Stabs')]),
    ('Pluck', 'synth', [(33, 8, 'Arp'), (41, 8, 'Arp'), (49, 8, 'Arp')]),
    ('Lead', 'synth', [(41, 8, 'Hook'), (49, 8, 'Hook 2')]),
    ('Choir', 'audio', [(33, 8, 'Choir ahh'), (41, 16, 'Choir ahh')]),
]

SECTIONS = [(33, 'Break'), (41, 'Drop'), (49, 'Drop')]


def notes(track, start, bars, label):
    n = []
    for b in range(bars):
        bar = start + b
        ch = PROG[(bar - 1) % 4]
        if track == 'Kick':
            n += [(b, q, 0.25, 36, 1.0) for q in range(4)]
        elif track == 'Clap':
            if label == 'Clap fill':
                if b % 2 == 1:
                    n.append((b, 3, 0.25, 39, 0.8))
                if b == bars - 1:
                    n += [(b, 2 + k * 0.25, 0.2, 39, 0.5 + k * 0.06) for k in range(8)]
            else:
                n += [(b, 1, 0.25, 39, 1.0), (b, 3, 0.25, 39, 1.0)]
        elif track == 'Hats':
            if label == 'Hats soft':
                n += [(b, q + 0.5, 0.2, 42, 0.6) for q in range(4)]
            else:
                for s in range(16):
                    if s % 4 == 2:
                        n.append((b, s * 0.25, 0.4, 46, 0.9))
                    else:
                        n.append((b, s * 0.25, 0.2, 42, 0.55 + 0.3 * (s % 2 == 0)))
        elif track == 'Bass':
            root = ch[0] - 24 + (12 if ch[0] < 50 else 0)
            for k, (q, ln, dp) in enumerate([(0, 0.75, 0), (1, 0.5, 0), (1.5, 0.5, 12), (2.5, 0.5, 0),
                                              (3, 0.5, 7), (3.5, 0.5, 0)]):
                n.append((b, q, ln, root + dp, 0.9))
        elif track == 'Chords':
            if label == 'Pads':
                n += [(b, 0, 4, p, 0.7) for p in ch + (ch[0] + 12,)]
            else:
                for q, ln in ((0, 0.5), (0.75, 0.5), (1.5, 0.75), (2.5, 0.5), (3.25, 0.5)):
                    n += [(b, q, ln, p, 0.85) for p in ch + (ch[0] + 12,)]
        elif track == 'Pluck':
            arp = list(ch) + [ch[0] + 12, ch[1] + 12, ch[0] + 12, ch[2], ch[1]]
            step = 0.5 if label == 'Arp' and start == 33 else 0.25
            k = 0
            q = 0.0
            while q < 4 - 1e-6:
                n.append((b, q, step * 0.8, arp[k % len(arp)] + 12, 0.75))
                k += 1
                q += step
        elif track == 'Lead':
            phrase = [[(0, 1.5, 76), (1.5, 0.5, 74), (2, 1, 72), (3, 1, 74)],
                      [(0, 2.5, 72), (3, 0.5, 69), (3.5, 0.5, 71)],
                      [(0, 1, 72), (1, 1, 76), (2, 1.5, 79), (3.5, 0.5, 77)],
                      [(0, 3, 76), (3, 0.5, 74)]]
            if label == 'Hook 2':
                phrase[3] = [(0, 1, 76), (1, 1, 79), (2, 2, 81)]
            n += [(b, q, ln, p, 0.9) for q, ln, p in phrase[b % 4]]
    return n


def audio_env(track, start, bars, x0, x1, px_per_beat):
    """Samples of a waveform's peak envelope, one per pixel column: (x, amplitude 0..1)."""
    out = []
    for x in range(int(x0), int(x1)):
        beat = (x - x0) / px_per_beat
        if track == 'Percussion':
            pat = [0, 0.75, 1.5, 2.0, 2.75, 3.5]
            ph = beat % 4
            a = 0.08
            for k, p in enumerate(pat):
                d = ph - p
                if d >= 0:
                    a = max(a, (0.95 if k in (0, 3) else 0.6) * math.exp(-d * 7))
            a *= 0.9 + 0.1 * math.sin(x * 1.7)
        else:   # Choir: a breath per chord, swelling into each bar
            bb = beat / 4
            ph = bb % 1
            swell = 0.42 + 0.38 * math.sin(math.pi * min(1, ph * 1.15)) ** 0.8
            if start == 33:
                swell *= 0.55 + 0.45 * (bb / bars)
            a = swell * (0.86 + 0.14 * math.sin(x * 0.9) * math.sin(x * 0.37))
        out.append((x, max(0.04, min(1, a))))
    return out
