"""The first analysis's endings measure: the level 0.5 s after the last release (the pedal up)
against the level 0.25-0.05 s before it (L+R, 20 ms windows)."""
import sys, os, numpy as np, soundfile as sf
REL = {'ballad': 4 * 60 / 72 * 4 + 1.0, 'soft_chords': 9.2, 'middle_c': 3.0, 'bass': 14.5, 'ff_chord': 5.0}
for d in sys.argv[1:]:
    out = []
    for ph, tr in REL.items():
        x, sr = sf.read(os.path.join(d, ph + '.wav'), always_2d=True)
        e = (x ** 2).mean(1); h = int(0.02 * sr); k = len(e) // h
        env = 10 * np.log10(e[:k * h].reshape(k, h).mean(1) + 1e-30); t = np.arange(k) * 0.02 + 0.01
        ref = env[(t > tr - 0.25) & (t < tr - 0.05)].mean()
        out.append(f'{ph} +0.5 s {env[(t > tr + 0.47) & (t < tr + 0.53)].mean() - ref:.1f}')
    print(os.path.basename(os.path.normpath(d)), ' | '.join(out))
