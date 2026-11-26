"""Chorda's sound-lab subject (round 42, chorda-c17): the references catalogue from the two
recorded grands in $CANTUS_REFS/piano/content — the Salamander Grand (a Yamaha C5, CC BY 3.0,
Alexander Holm) and the bitKlavier Grand (Princeton University, CC BY 4.0) — each a physical
instrument that weighs one in the band; and the candidate's notes split out of the voicing
lab's single-note renders.

    python -I make_refs.py refs                     # writes references.json beside this file
    python -I make_refs.py candidate <out> [spec]   # renders Chorda's notes (CHORDA_EXE), splits them

A dynamic is the velocity the lab's single notes are played at (pp 30, mf 75, ff 115), and a
reference note is the layer each grand's own SFZ plays at that velocity, so both sides of a cell
are the same keypress.
"""
import json, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
DYNS = {'pp': 30, 'mf': 75, 'ff': 115}
PIANOS = [
    ('salamander', 'piano/content/salamander-grand-piano', 'SalamanderGrandPiano-V3+20200602.sfz',
     'CC-BY-3.0', 'Salamander Grand Piano V3, Alexander Holm (CC BY 3.0)'),
    ('bitklavier', 'piano/content/bitklavier-grand-stage-lid-ortf', 'stage-lid-ortf.sfz',
     'CC-BY-4.0', 'The bitKlavier Grand, Princeton University: Dan Trueman, Andrés Villalta, Matt Wang (CC BY 4.0)'),
]
NAMES = {'C': 0, 'C#': 1, 'D': 2, 'D#': 3, 'E': 4, 'F': 5, 'F#': 6, 'G': 7, 'G#': 8, 'A': 9, 'A#': 10, 'B': 11}


def refs_root():
    return os.environ.get('SOUNDLAB_REFS') or os.environ.get('CANTUS_REFS') or 'F:/projects/cantus-refs'


def regions(sfz_path):
    """(sample, lokey, hikey, lovel, hivel, keycenter) of the note regions (no release/harm)."""
    out, key_of = [], lambda s: None
    for line in open(sfz_path, encoding='utf-8', errors='replace'):
        if '<region>' not in line or 'trigger=release' in line:
            continue
        kv = dict(re.findall(r'(\w+)=(\S+)', line))
        s = kv.get('sample', '')
        base = os.path.basename(s)
        m = re.match(r'([A-G]#?)(\d)v\d+\.flac$', base)
        if not m:
            continue
        note = 12 * (int(m.group(2)) + 1) + NAMES[m.group(1)]
        out.append((s, int(kv.get('lokey', note)), int(kv.get('hikey', note)), int(kv.get('lovel', 1)),
                    int(kv.get('hivel', 127)), int(kv.get('pitch_keycenter', note))))
    return out


def refs():
    root = refs_root()
    cat = []
    for inst, d, sfz, lic, credit in PIANOS:
        for s, lo, hi, lv, hv, kc in regions(os.path.join(root, d, sfz)):
            for dyn, vel in DYNS.items():
                if lv <= vel <= hv:
                    cat.append(dict(path=f'{d}/{s}'.replace('\\', '/'), source=inst, licence=lic, credit=credit,
                                    inst=inst, art='struck', note=kc, dyn=dyn))
    cat.sort(key=lambda r: (r['inst'], r['note'], r['dyn']))
    json.dump(cat, open(os.path.join(HERE, 'references.json'), 'w'), indent=0)
    print(f'{len(cat)} references')


def candidate(out, spec):
    import numpy as np
    import soundfile as sf
    exe = os.environ['CHORDA_EXE']
    os.makedirs(out, exist_ok=True)
    tmp = os.path.join(out, '_lab')
    env = dict(os.environ, CHORDA_PHRASES='notes_pp,notes_mf,notes_ff')
    subprocess.run([exe, 'lab', tmp, 'c', spec], check=True, env=env)
    keys = list(range(21, 109, 3))
    for dyn in DYNS:
        x, sr = sf.read(os.path.join(tmp, 'wav', 'c', f'notes_{dyn}.wav'), dtype='float32', always_2d=True)
        for i, k in enumerate(keys):
            # Only the keys both grands sampled (every minor third from A0).
            seg = x[int(i * 6.0 * sr):int((i * 6.0 + 5.9) * sr)]
            sf.write(os.path.join(out, f'chorda-{k}-{dyn}.wav'), seg, sr, subtype='FLOAT')
    for f in os.listdir(os.path.join(tmp, 'wav', 'c')):
        os.remove(os.path.join(tmp, 'wav', 'c', f))


if __name__ == '__main__':
    if sys.argv[1] == 'refs':
        refs()
    else:
        candidate(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else os.environ.get("CHORDA_SPEC", "base=shipped"))
