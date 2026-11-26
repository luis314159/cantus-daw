"""transcribe.py - r42 fiddle: a reference phrase as notes, for the comparison clips (the
owner's ear only, never shipped): the study's tracker (fiddle_lib: YIN, the band-flux onsets)
turns `t0 … t1` of a recording into notes (start, length, key, velocity), and the excerpt is
saved beside them, plain and band-limited as a 78 is (200-4000 Hz), with our render to come
through `fiddle_lab -- <dir> notes=<tsv>`.

    python -I tools/arcus/fiddle/transcribe.py <recording> <t0> <t1> <out-dir> <name> [semitones]

Writes `<out-dir>/<name>.tsv` (start s, length s, MIDI key, velocity 0-1; from 0) and
`<out-dir>/<name>-ref.wav` (the excerpt, 48 kHz mono). `semitones` moves the notes (a 78
playing sharp or flat of its key: the tracker's tuning offset is printed).
"""
import sys
from pathlib import Path

import numpy as np
import soundfile as sf
from scipy.signal import resample_poly

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fiddle_lib as fl  # noqa: E402


def main(a):
    src, t0, t1, out, name = a[0], float(a[1]), float(a[2]), Path(a[3]), a[4]
    shift = float(a[5]) if len(a) > 5 else 0.0
    out.mkdir(parents=True, exist_ok=True)
    x, sr = fl.load(src, t0, t1)
    tr = fl.Track(x, sr)
    ons, _ = fl.onsets(tr)
    ons = list(ons) + [len(x) / sr]
    notes = []
    for i in range(len(ons) - 1):
        a0, a1 = ons[i], ons[i + 1]
        k = (tr.t >= a0 + 0.01) & (tr.t < a1 - 0.005) & tr.voiced
        if k.sum() < 3:
            continue
        m = float(np.median(tr.midi[k])) - tr.tuning + shift
        key = int(round(m))
        # the note sounds while it is loud: until the level falls 20 dB under its peak
        lv = tr.lvl[(tr.t >= a0) & (tr.t < a1)]
        pk = float(lv.max())
        vel = float(np.clip(0.55 + (pk - tr.peak + 12) / 40.0, 0.35, 0.95))
        notes.append((a0, a1 - a0, key, vel))
    # one key on: a repeated key joined only when no onset parted them (they are)
    with open(out / f"{name}.tsv", "w", encoding="utf-8") as f:
        for a0, d, k, v in notes:
            f.write(f"{a0:.4f}\t{d:.4f}\t{k}\t{v:.3f}\n")
    y = resample_poly(x, 48000, sr) if sr != 48000 else x
    y = 0.3 * y / max(1e-9, float(np.sqrt(np.mean(y ** 2)))) * 0.33
    sf.write(str(out / f"{name}-ref.wav"), np.clip(y, -1, 1), 48000, subtype="PCM_24")
    print(f"{name}: {len(notes)} notes, tuning offset {tr.tuning:+.2f} semitones, "
          f"keys {min(n[2] for n in notes)}-{max(n[2] for n in notes)}")


if __name__ == "__main__":
    main(sys.argv[1:])
