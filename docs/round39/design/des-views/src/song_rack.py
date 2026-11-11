"""des-views — Neon Tide's "Drop" pattern as the channel rack holds it (bars 1-2 of 8)."""
PROG = [(57, 60, 64), (53, 57, 60), (48, 52, 55), (55, 59, 62)]


def _two(bar_hits):
    out = {}
    for b in range(2):
        for s, v in bar_hits.items():
            out[b * 16 + s] = v
    return out


KICK = _two({0: 1.0, 4: 0.92, 8: 0.96, 12: 0.9})
CLAP = _two({4: 1.0, 12: 0.94})
CLAP[30] = 0.55
CLAP[31] = 0.7
CLOSED = _two({s: (0.85 if s % 2 == 0 else 0.52) for s in range(16) if s % 4 != 2})
OPEN = _two({2: 0.9, 6: 0.8, 10: 0.88, 14: 0.8})
RIDE = _two({0: 0.7, 8: 0.6})


def bass():
    n = []
    for b in range(2):
        ch = PROG[b]
        root = ch[0] - 24 + (12 if ch[0] < 50 else 0)
        for q, ln, dp in ((0, 0.75, 0), (1, 0.5, 0), (1.5, 0.5, 12), (2.5, 0.5, 0), (3, 0.5, 7), (3.5, 0.5, 0)):
            n.append((b * 4 + q, ln, root + dp))
    return n


def chords():
    n = []
    for b in range(2):
        ch = PROG[b]
        for q, ln in ((0, 0.5), (0.75, 0.5), (1.5, 0.75), (2.5, 0.5), (3.25, 0.5)):
            n += [(b * 4 + q, ln, p) for p in ch + (ch[0] + 12,)]
    return n


def pluck():
    n = []
    for b in range(2):
        ch = PROG[b]
        arp = list(ch) + [ch[0] + 12, ch[1] + 12, ch[0] + 12, ch[2], ch[1]]
        for k in range(16):
            n.append((b * 4 + k * 0.25, 0.2, arp[k % len(arp)] + 12))
    return n


def lead():
    ph = [[(0, 1.5, 76), (1.5, 0.5, 74), (2, 1, 72), (3, 1, 74)],
          [(0, 2.5, 72), (3, 0.5, 69), (3.5, 0.5, 71)]]
    return [(b * 4 + q, ln, p) for b in range(2) for q, ln, p in ph[b]]


# name, colour (a track's), kind, level, mixer track, pan 0..1, volume 0..1, data, flags
CHANNELS = [
    ('Kick', 'Kick', 'steps', 0, 1, 0.5, 0.78, KICK, ''),
    ('Clap', 'Clap', 'steps', 0, 2, 0.5, 0.70, CLAP, ''),
    ('Hats', 'Hats', 'kit', 0, 3, 0.5, 0.66, None, 'open'),
    ('Closed hat', 'Hats', 'steps', 1, 3, 0.42, 0.60, CLOSED, ''),
    ('Open hat', 'Hats', 'steps', 1, 3, 0.60, 0.55, OPEN, ''),
    ('Ride', 'Hats', 'steps', 1, 3, 0.5, 0.50, RIDE, 'muted'),
    ('Percussion', 'Percussion', 'perc', 0, 4, 0.5, 0.68, None, ''),
    ('Bass', 'Bass', 'roll', 0, 5, 0.5, 0.74, bass(), ''),
    ('Chords', 'Chords', 'roll', 0, 6, 0.5, 0.71, chords(), 'sel'),
    ('Pluck', 'Pluck', 'roll', 0, 7, 0.38, 0.62, pluck(), ''),
    ('Lead', 'Lead', 'roll', 0, 8, 0.5, 0.69, lead(), ''),
    ('Choir', 'Choir', 'pad', 0, 9, 0.5, 0.58, None, ''),
]
