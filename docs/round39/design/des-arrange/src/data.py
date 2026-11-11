"""des-mixer — the desk the sheets show: Neon Tide's mix at bar 44 of the second drop.

Each strip is a dict; the drawing reads only what it needs. Levels are dB. Meter levels
(`lv`) are the two channels' RMS-ish bar tops in dB while playing; `pk` the peak holds.
"""
import base

base.HUES.update({
    'Vox lead': '#d98a5c', 'Vox double (left)': '#c99a6a', 'Guitar DI': '#c7b05a',
    'Room mics': '#8fa0b0', 'Sub drop FX': '#7d86e0', 'Riser': '#b47ae0', 'Strings': '#5aa7c9',
    'Piano': '#7fb2c0', 'Brass stabs': '#d9a14a', 'Shaker': '#c9c06a', 'Snare top': '#e07560',
    'Tom fills': '#d8865a', 'Drums': '#9aa6b2', 'Synths': '#8e9ccc', 'Pads': '#6fb0c0',
    'Batería': '#9aa6b2', 'Voz principal': '#d98a5c', 'Coros dobles (izquierda)': '#c99a6a',
    'Bajo': '#9a80ee', 'Acordes': '#3fc0c8', 'Retardo': '#93b5a3', 'Reverberación': '#86a9c8',
    'Pad': '#6c93ea', 'Strip 11': '#9aa6b2',
})


def S(num, name, db, pan=0.0, fx=(), out='Master', lv=None, **kw):
    d = dict(num=num, name=name, db=db, pan=pan, fx=list(fx), out=out, lv=lv, kind='track',
             sends=[], eq=(0, 0, 0), width=1.0)
    d.update(kw)
    return d


# fx entries: (short name, kind, on)
STRIPS = [
    S(1, 'Kick', -4.0, 0.0, [('Aequalis', 'EQ', True), ('Comprimo', 'Comp', True)], 'Drum bus', (-9, -9.5),
      eq=(2.5, -3, 0), src='Kick'),
    S(2, 'Clap', -6.5, 0.08, [('Aequalis', 'EQ', True)], 'Drum bus', (-15, -14), eq=(0, 0, 1.5),
      sends=[('Reverb', -18.0, True)]),
    S(3, 'Hats', -9.0, -0.22, [('Aequalis', 'EQ', True)], 'Drum bus', (-19, -18.5), eq=(-4, 0, 1)),
    S(4, 'Percussion', -7.5, 0.28, [('Comprimo', 'Comp', True)], 'Drum bus', (-17, -16)),
    S(5, 'Bass', -5.0, 0.0, [('Comprimo', 'Comp', True), ('Aequalis', 'EQ', True)], 'Master', (-11, -11),
      eq=(1.5, 0, -2), sidechain='Kick', width=0.0),
    S(6, 'Chords', -3.5, 0.0, [('Comprimo', 'Comp', True), ('Aequalis', 'EQ', True), ('Saturo', 'Drive', False)],
      'Master', (-12, -13), eq=(2, -2.5, 1.6), sends=[('Reverb', -16.0, True), ('Delay', -22.0, True)]),
    S(7, 'Pluck', -8.0, -0.34, [('Morae', 'Delay', True)], 'Master', (-16, -15.5),
      sends=[('Reverb', -14.0, True), ('Delay', -18.0, False)]),
    S(8, 'Lead', -6.0, 0.16, [('Aequalis', 'EQ', True), ('Chorus', 'Chorus', True)], 'Master', (-12.5, -12),
      eq=(-2, 1.5, 2), sends=[('Reverb', -20.0, True), ('Delay', -16.0, True)]),
    S(9, 'Choir', -10.0, 0.0, [('Aequalis', 'EQ', True), ('Spatium', 'Space', True)], 'Master', (-20, -19),
      eq=(-3, 0, 1), sends=[('Reverb', -10.0, True)]),
    S(10, 'Drum bus', -2.0, 0.0, [('Comprimo', 'Comp', True), ('Saturo', 'Drive', True), ('Limes', 'Limit', True)],
      'Master', (-7, -7.5), kind='bus', receives=['Kick', 'Clap', 'Hats', 'Percussion']),
    S(11, None, 0.0, kind='empty'),
    S(12, None, 0.0, kind='empty'),
    S(13, None, 0.0, kind='empty'),
]

RETURNS = [
    S(14, 'Reverb', -8.0, 0.0, [('Spatium', 'Space', True), ('Aequalis', 'EQ', True)], 'Master', (-22, -21),
      kind='return', receives=['Clap', 'Chords', 'Pluck', 'Lead', 'Choir'], eq=(-6, 0, -2)),
    S(15, 'Delay', -12.0, 0.0, [('Morae', 'Echo', True)], 'Master', (-25, -26),
      kind='return', receives=['Chords', 'Pluck', 'Lead']),
]

VCA = S(16, 'Drums', -1.5, kind='vca', follows=['Kick', 'Clap', 'Hats', 'Percussion'])

MASTER = S(0, 'Master', 0.0, 0.0, [('Aequalis', 'EQ', True), ('Limes', 'Limit', True)], 'Out 1–2', (-6.5, -6.8),
           kind='master', lufs=-9.4, tp=-1.0)

SEL = 'Chords'


def playing(s, on=True):
    """The strip as the meters see it while playing (idle: no meter level at all)."""
    d = dict(s)
    if not on:
        d['lv'] = None
    return d


# the overflow desk: 24 strips with long names (an orchestral-pop session)
LONG = [
    ('Kick', 'Drum bus'), ('Snare top', 'Drum bus'), ('Tom fills', 'Drum bus'), ('Room mics', 'Drum bus'),
    ('Shaker', 'Drum bus'), ('Bass', 'Master'), ('Guitar DI', 'Master'), ('Piano', 'Master'),
    ('Strings', 'Master'), ('Brass stabs', 'Master'), ('Pads', 'Master'), ('Chords', 'Master'),
    ('Lead', 'Master'), ('Vox lead', 'Master'), ('Vox double (left)', 'Master'), ('Choir', 'Master'),
    ('Riser', 'Master'), ('Sub drop FX', 'Master'), ('Pluck', 'Master'), ('Hats', 'Drum bus'),
    ('Clap', 'Drum bus'), ('Percussion', 'Drum bus'), ('Synths', 'Master'), ('Drums', 'Master'),
]
