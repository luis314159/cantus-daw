"""r42 arcus-classical — the owner's listening clips: the same window of a new solo render as an
earlier clip (found by cross-correlating the earlier clip's level envelope against the earlier
solo), at the earlier clip's loudness (BS.1770, the sound lab's meter), 48 kHz stereo 16-bit,
30 ms fades.

    python -I tools/arcus/classical/clips.py find <clip.wav> <earlier solo.wav>
    python -I tools/arcus/classical/clips.py cut <out.wav> <solo.wav> <t0> <t1> <lufs>
    python -I tools/arcus/classical/clips.py lufs <wav>

`find` prints the window's start (s) and the clip's loudness; `cut` writes the window.
"""
import sys
from pathlib import Path

import numpy as np
import soundfile as sf

LAB = Path(__file__).resolve().parents[2] / "soundlab"
sys.path.insert(0, str(LAB))
from soundlab import audio, loudness  # noqa: E402


def read48(p):
    x, sr = audio.read(p)
    if x.ndim == 1:
        x = x[:, None]
    if sr != 48000:
        x, sr = audio.resample(x, sr, 48000), 48000
    if x.shape[1] == 1:
        x = np.repeat(x, 2, axis=1)
    return x[:, :2].astype(np.float64)


def env(x, hop=480):
    m = x.mean(axis=1)
    n = len(m) // hop
    return 10 * np.log10(np.mean(m[: n * hop].reshape(n, hop) ** 2, axis=1) + 1e-12)


def find(clip, solo):
    c, s = read48(clip), read48(solo)
    ec, es = env(c), env(s)
    ec -= ec.mean()
    best = (-1e30, 0)
    for k in range(0, len(es) - len(ec) + 1):
        w = es[k:k + len(ec)]
        v = float(np.dot(ec, w - w.mean()))
        if v > best[0]:
            best = (v, k)
    t0 = best[1] * 480 / 48000
    print(f"{t0:.3f} {len(c) / 48000:.3f} {loudness.measure(c, 48000)['integrated_lufs']:.2f}")


def cut(out, solo, t0, t1, lufs):
    x = read48(solo)
    x = x[int(float(t0) * 48000):int(float(t1) * 48000)].copy()
    f = int(0.03 * 48000)
    ramp = np.linspace(0, 1, f)[:, None]
    x[:f] *= ramp
    x[-f:] *= ramp[::-1]
    lv = loudness.measure(x, 48000)["integrated_lufs"]
    g = 10 ** ((float(lufs) - lv) / 20)
    peak = np.max(np.abs(x)) * g
    if peak > 10 ** (-1 / 20):
        g *= 10 ** (-1 / 20) / peak
    sf.write(out, x * g, 48000, subtype="PCM_16")
    print(f"{Path(out).name}: {len(x) / 48000:.2f} s, {lv:.1f} LUFS -> {lv + 20 * np.log10(g):.1f}")


if __name__ == "__main__":
    cmd, args = sys.argv[1], sys.argv[2:]
    if cmd == "find":
        find(*args)
    elif cmd == "cut":
        cut(*args)
    else:
        print(f"{loudness.measure(read48(args[0]), 48000)['integrated_lufs']:.2f}")
