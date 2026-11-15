"""Two renders of `full_kit_dump` (cantus-engine, ignored) compared sample by sample: a
saving that must not be heard (drums-gen 3) differs by less than -80 dB re the render's peak.

    python -I dump_diff.py <before dir> <after dir> [kit …]

Prints, per kit, the largest sample difference and the difference's RMS, both in dB re the
before render's peak, and the worst 50-ms window's difference re that window's own RMS.
"""
import sys
import numpy as np


def load(p):
    return np.fromfile(p, dtype="<f4").astype(np.float64).reshape(-1, 2)


def main():
    a, b = sys.argv[1], sys.argv[2]
    kits = sys.argv[3:] or ["studio", "jazz"]
    worst = -400.0
    for k in kits:
        x, y = load(f"{a}/{k}.f32"), load(f"{b}/{k}.f32")
        n = min(len(x), len(y))
        x, y = x[:n], y[:n]
        d = y - x
        pk = np.abs(x).max()
        mx = 20 * np.log10(np.abs(d).max() / pk + 1e-30)
        rms = 20 * np.log10(np.sqrt(np.mean(d * d)) / pk + 1e-30)
        w = 2400
        m = n // w
        ex = np.sqrt(np.mean(x[:m * w].reshape(m, w, 2) ** 2, axis=(1, 2))) + 1e-12
        ed = np.sqrt(np.mean(d[:m * w].reshape(m, w, 2) ** 2, axis=(1, 2)))
        loud = ex > pk * 1e-3
        win = 20 * np.log10((ed[loud] / ex[loud]).max() + 1e-30) if loud.any() else -400
        print(f"{k:7} peak {20 * np.log10(pk):6.1f} dBFS  max diff {mx:7.1f} dB  rms diff {rms:7.1f} dB"
              f"  worst 50 ms (re its rms) {win:7.1f} dB")
        worst = max(worst, mx)
    print("PASS" if worst < -80 else "FAIL", f"(worst {worst:.1f} dB re peak; limit -80)")


if __name__ == "__main__":
    main()
