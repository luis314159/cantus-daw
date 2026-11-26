"""Round 42 (chorda-c18): spectrograms of chosen keys from notes renders, one row per source, one
column per key — to look at the top keys' mechanics beside the grands'.

    python -I spec_png.py <out.png> <dyn> <keys 102,108> <label>=<notes dir> ... [--ms 400] [--khz 12]
"""
import os, sys
import numpy as np
import soundfile as sf
from scipy import signal
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

SR = 48000
KEYS = list(range(21, 109, 3))


def main():
    a = sys.argv[1:]
    ms, khz = 400, 12.0
    if '--ms' in a:
        i = a.index('--ms'); ms = int(a[i + 1]); del a[i:i + 2]
    if '--khz' in a:
        i = a.index('--khz'); khz = float(a[i + 1]); del a[i:i + 2]
    out, dyn, keys = a[0], a[1], [int(k) for k in a[2].split(',')]
    srcs = [tuple(s.split('=', 1)) for s in a[3:]]
    fig, ax = plt.subplots(len(srcs), len(keys), figsize=(4.2 * len(keys), 2.6 * len(srcs)), squeeze=False)
    for r, (lab, d) in enumerate(srcs):
        x, _ = sf.read(os.path.join(d, f'notes_{dyn}.wav'), dtype='float64', always_2d=True)
        m = x.mean(1)
        for c, k in enumerate(keys):
            a0 = int(KEYS.index(k) * 6 * SR)
            seg = np.abs(m[a0:a0 + int(0.3 * SR)])
            on = a0 + int(np.argmax(seg > seg.max() * 0.05))
            y = m[on - int(0.03 * SR):on + int(ms / 1000 * SR)]
            ref = (m[on:on + int(0.06 * SR)] ** 2).mean()
            f, t, S = signal.spectrogram(y, SR, window='hann', nperseg=512, noverlap=448)
            S = 10 * np.log10(S / ref + 1e-30)
            sel = f <= khz * 1000
            ax[r, c].pcolormesh((t - 0.03) * 1000, f[sel] / 1000, S[sel], vmin=-110, vmax=-40, shading='auto', cmap='magma')
            ax[r, c].set_title(f'{lab} key {k} {dyn}', fontsize=8)
            ax[r, c].tick_params(labelsize=7)
    fig.tight_layout()
    fig.savefig(out, dpi=80)


if __name__ == '__main__':
    main()
