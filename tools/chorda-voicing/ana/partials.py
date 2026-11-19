"""Round 40: middle C's partials (levels re the strongest, dB) in a window, per render dir.

    python partials.py <t0> <t1> <dir> ...
"""
import sys
import numpy as np
import soundfile as sf

SR = 48000
F0 = 261.63
B = 3.0e-4


def parts(d, t0, t1, n=12):
    x, _ = sf.read(d + '/middle_c.wav', always_2d=True)
    m = x.mean(1)
    a = np.argmax(np.abs(m) > np.abs(m).max() * 0.003)
    seg = m[a + int(t0 * SR): a + int(t1 * SR)]
    s = np.abs(np.fft.rfft(seg * np.hanning(len(seg)), 8 * len(seg)))
    f = np.fft.rfftfreq(8 * len(seg), 1 / SR)
    out = []
    for k in range(1, n + 1):
        fk = k * F0 * np.sqrt(1 + B * k * k)
        sel = (f > fk * 0.985) & (f < fk * 1.015)
        out.append(20 * np.log10(s[sel].max() + 1e-20))
    out = np.array(out)
    return out - out.max()


def main():
    t0, t1 = float(sys.argv[1]), float(sys.argv[2])
    dirs = sys.argv[3:]
    ref = parts(dirs[0], t0, t1)
    print(f'{"":16}' + ' '.join(f'{k:5d}' for k in range(1, 13)))
    print(f'{dirs[0].split("/")[-1][:16]:16}' + ' '.join(f'{v:5.0f}' for v in ref))
    for d in dirs[1:]:
        p = parts(d, t0, t1)
        print(f'{d.split("/")[-1][:16]:16}' + ' '.join(f'{v - r:+5.0f}' for v, r in zip(p, ref)))


if __name__ == '__main__':
    main()
