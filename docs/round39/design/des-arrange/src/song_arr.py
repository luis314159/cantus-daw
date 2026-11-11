"""des-arrange — Neon Tide as the arrangement shows it: the tracks (a folder and its children,
instruments, audio, an automation lane), their clips and the song's top lanes.

The clip content is des-views' (clipdraw.py, song_full.py); this module only adds the parts the
arrangement needs that the clip sheet did not have (the bass, the arp, the drum fill) by giving
song_full one notes() that dispatches on the clip's label first.
"""
import song_full as S
import base

base.HUES.update({
    'Drum group': '#e08a5a',     # the folder's own colour (the user's)
    'Kit': '#e8794f', 'Fills': '#e6a54a', 'Vox': '#d98a5c', 'Strings': '#5aa7c9',
    'Keys': '#7fb2c0', 'Riser': '#b47ae0', 'Sub': '#7d86e0', 'Guitar': '#c7b05a',
})

_orig_notes = S.notes
PROG = S.PROG


def notes(kind, start, bars, label):
    n = []
    if label == 'Bass':
        for b in range(bars):
            ch = PROG[(start + b - 1) % 4]
            root = ch[0] - 24 + (12 if ch[0] < 50 else 0)
            for q, ln, dp in ((0, 0.75, 0), (1, 0.5, 0), (1.5, 0.5, 12), (2.5, 0.5, 0), (3, 0.5, 7), (3.5, 0.5, 0)):
                n.append((b * 4 + q, ln, root + dp, 0.9 if q in (0, 1.5) else 0.75))
        return n
    if label.startswith('Arp'):
        for b in range(bars):
            ch = PROG[(start + b - 1) % 4]
            arp = list(ch) + [ch[0] + 12, ch[1] + 12, ch[0] + 12, ch[2], ch[1]]
            step = 0.5 if label == 'Arp soft' else 0.25
            k, q = 0, 0.0
            while q < 4 - 1e-6:
                n.append((b * 4 + q, step * 0.8, arp[k % len(arp)] + 12, 0.7 + 0.2 * (k % 4 == 0)))
                k += 1
                q += step
        return n
    if label == 'Fill':
        o = (bars - 1) * 4
        for k in range(8):
            n.append((o + 2 + k * 0.25, 0.2, 39 if k % 2 == 0 else 41, 0.55 + k * 0.05))
        n.append((0, 0.25, 36, 1.0))
        return n
    if label == 'Take 3':
        return [(0, 1, 60, 0.8)]
    return _orig_notes(kind, start, bars, label)


S.notes = notes


def T(name, kind, inst, h=44, clips=(), **kw):
    d = dict(name=name, hue=kw.pop('hue', name), kind=kind, inst=inst, h=h, clips=list(clips), lanes=[],
             depth=0, mute=False, solo=False, arm=False, frozen=False, sel=False, auto=False, implicit=False)
    d.update(kw)
    return d


def A(param, owner, value, clips, h=36, **kw):
    d = dict(param=param, owner=owner, value=value, clips=list(clips), h=h, mode='Read')
    d.update(kw)
    return d


SECTIONS = [(1, 'Intro'), (9, 'Verse'), (25, 'Build'), (33, 'Break'), (41, 'Drop'), (49, 'Drop 2'), (57, 'Outro'),
            (65, None)]
MARKERS = [(45, 'Vox in'), (53, 'Lift')]
TEMPO = [(1, 120.0), (25, 120.0), (33, 124.0), (64, 124.0)]     # bar, bpm (ramps between points)
SIGS = [(1, '4/4'), (56, '2/4'), (57, '4/4')]


def song(view='docked'):
    """The tracks of the window. `view` = 'docked' (the mixer below) or 'max' (maximised)."""
    cut = A('Cutoff', 'Calor', '688 Hz', [(25, 8, 'Cutoff rise'), (41, 16, 'Cutoff')], h=36 if view == 'docked' else 44)
    drums = T('Drums', 'folder', 'Folder · 2 tracks → Drum bus', 30, open=True, hue='Drum group')
    kit = T('Kit', 'drums', 'Pulsus · 909 kit', 44, [(9, 16, 'Beat'), (25, 8, 'Build'), (41, 16, 'Beat'), (57, 4, 'Beat')],
            depth=1, parent='Drums', parent_hue='Drum group')
    perc = T('Percussion', 'audio', 'Audio · Perc loop 124', 44,
             [(17, 8, 'Perc loop 124'), (25, 8, 'Perc loop 124'), (41, 8, 'Perc loop 124'), (49, 8, 'Perc loop 124')],
             depth=1, parent='Drums', parent_hue='Drum group')
    bass = T('Bass', 'melody', 'Gravis · Sub saw', 44, [(9, 16, 'Bass'), (41, 16, 'Bass'), (57, 4, 'Bass')])
    chords = T('Chords', 'chords', 'Calor · Glass pad', 44,
               [(1, 8, 'Pads'), (9, 16, 'Pads'), (25, 8, 'Stabs'), (33, 8, 'Pads'), (41, 8, 'Stabs'), (49, 8, 'Stabs'), (57, 8, 'Pads')],
               sel=True, auto=True)
    chords['lanes'] = [cut]
    pluck = T('Pluck', 'melody', 'Unda · Glass pluck', 44, [(9, 8, 'Arp soft'), (17, 8, 'Arp soft'), (33, 8, 'Arp soft'),
                                                                  (41, 8, 'Arp'), (49, 8, 'Arp')])
    lead = T('Lead', 'melody', 'Modulus · Bell lead', 44, [(25, 8, 'Hook'), (41, 8, 'Hook'), (49, 8, 'Hook 2')])
    choir = T('Choir', 'audio2', 'Audio · stereo', 44, [(1, 8, 'Choir ahh'), (33, 8, 'Choir ahh'), (41, 16, 'Choir ahh'),
                                                               (57, 8, 'Choir ahh')])
    tracks = [drums, kit, perc, bass, chords, pluck, lead, choir]
    if view == 'max':
        kit['h'] = 52
        perc['h'] = 44
        lead['mute'] = True
        choir['frozen'] = True
        choir['inst'] = 'Frozen · Spatium, Aequalis'
        chords['lanes'] = [cut, A('Reverb send', 'Mixer', '−16.0 dB', [(41, 16, 'Send')], h=36, mode='Touch')]
        vox = T('Vox', 'audio', 'In 1 · Mic (mono)', 64, [(49, 4, 'Take 3')], arm=True, input='In 1 · Mic',
                hue='Vox')
        tracks.append(vox)
    return tracks


def auto_points(label, bars):
    if label == 'Send':
        return [(0, 0.55, 0), (bars * 4 - 16, 0.55, 0.5), (bars * 4 - 4, 0.85, 0), (bars * 4, 0.85, 0)]
    return _orig_auto(label, bars)


_orig_auto = S.auto_points
S.auto_points = auto_points
