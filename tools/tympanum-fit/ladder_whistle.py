"""Gliding tones in a cymbal's tail (round 40, ladder; the owner's "iuuu iuuu" near the end
of the cymbal clips): spectral ridges tracked frame to frame in the late part of a clip.

    python -I ladder_whistle.py <ladder dir> [b9 dir]           (gliding ridges)
    python -I ladder_whistle.py warble <ladder dir> [b9 dir]    (slow swoops of the tail)
    python -I ladder_whistle.py lone <ladder dir> [b9 dir]      (a band standing alone at the end)
    python -I ladder_whistle.py blips <ladder dir> [b9 dir]     (musical noise: short tonal blobs)
    python -I ladder_whistle.py excess <ladder dir> [b9 dir]    (where a tail outgrows the recording)
    python -I ladder_whistle.py swell <ladder dir> [b9 dir]     (a tail that climbs again: the finding)

Per clip, from 35 % of its length to 60 ms before its end: an STFT (4096 points, hop 256);
in each frame the peaks 1-16 kHz standing 8 dB over the 300-Hz running median and within
45 dB of the frame's loudest; peaks linked frame to frame (±2 bins); a ridge of at least
120 ms is a *tone*, and a *glide* when its pitch moves 25 cents or more over its life
(a straight-line fit). Printed: tones and glides per second of tail, the share of the
tail's 1-16 kHz energy the glides carry, and the strongest glides.
"""
import glob
import json
import os
import sys
import numpy as np
import soundfile as sf
from scipy import signal, ndimage

SR = 48000
N, H = 4096, 256


def ridges(x, a, b):
    f, t, S = signal.stft(x[a:b], SR, nperseg=N, noverlap=N - H, boundary=None)
    P = np.abs(S) ** 2
    d = 10 * np.log10(P + 1e-30)
    df = f[1]
    band = (f >= 1000) & (f <= 16000)
    med = ndimage.median_filter(d, size=(int(300 / df) | 1, 1))
    tracks, live = [], []
    for i in range(d.shape[1]):
        col = d[:, i]
        ok = band & (col - med[:, i] >= 8) & (col >= col[band].max() - 45)
        pk, _ = signal.find_peaks(np.where(ok, col, -400))
        nxt = []
        used = set()
        for tr in live:
            j = tr["k"][-1]
            near = [p for p in pk if abs(p - j) <= 2 and p not in used]
            if near:
                p = min(near, key=lambda q: abs(q - j))
                used.add(p)
                # (parabolic interpolation of the peak's bin)
                y0, y1, y2 = col[p - 1], col[p], col[p + 1]
                fr = (p + 0.5 * (y0 - y2) / (y0 - 2 * y1 + y2 + 1e-12)) * df
                tr["k"].append(p)
                tr["f"].append(fr)
                tr["t"].append(t[i])
                tr["e"].append(P[p - 1:p + 2, i].sum())
                nxt.append(tr)
            else:
                tracks.append(tr)
        for p in pk:
            if p not in used:
                nxt.append({"k": [p], "f": [p * df], "t": [t[i]], "e": [P[p - 1:p + 2, i].sum()]})
        live = nxt
    tracks += live
    tot = P[band].sum()
    out = []
    for tr in tracks:
        if tr["t"][-1] - tr["t"][0] < 0.12:
            continue
        tt, ff = np.array(tr["t"]), np.array(tr["f"])
        sl = np.polyfit(tt, ff, 1)[0]
        cents = 1200 * np.log2((ff.mean() + sl * (tt[-1] - tt.mean())) / (ff.mean() + sl * (tt[0] - tt.mean())))
        out.append(dict(hz=float(ff.mean()), dur=float(tt[-1] - tt[0]), cents=float(cents),
                        wob=float(np.std(ff - np.polyval(np.polyfit(tt, ff, 1), tt))),
                        share=float(np.sum(tr["e"]) / tot)))
    return out, (b - a) / SR


def report(name, x):
    a, b = int(0.35 * len(x)), len(x) - int(0.06 * SR)
    rs, dur = ridges(x, a, b)
    gl = [r for r in rs if abs(r["cents"]) >= 25]
    top = sorted(gl, key=lambda r: -r["share"])[:3]
    print(f"{name:34s} tail {dur:4.1f} s  tones/s {len(rs) / dur:5.1f}  glides/s {len(gl) / dur:5.1f}  "
          f"glide energy {100 * sum(r['share'] for r in gl):5.1f} %  | "
          + "  ".join(f"{r['hz']:6.0f} Hz {r['cents']:+5.0f} c {r['dur']:.2f} s" for r in top), flush=True)
    return dict(tones=len(rs) / dur, glides=len(gl) / dur, energy=sum(r["share"] for r in gl))


def main():
    d = sys.argv[1]
    res = {}
    for key in ["drs-crash-v127", "virt-ride-v80", "virt-hat_open-v127", "drs-hat_closed-v127",
                "virt-crash-v127", "drs-ride-v127", "drs-hat_open-v127", "drs-hat_half-v80"]:
        for r in ["R0", "R1", "R2", "R3", "R3b", "R3o", "R4"]:
            x, _ = sf.read(os.path.join(d, f"{key}-{r}.wav"))
            res[f"{key}-{r}"] = report(f"{key} {r}", x)
        print()
    if len(sys.argv) > 2:
        # model 5 (b9: "-2-nuevo"), model 4 ("-1-antes") and the recording ("-3-grabado"):
        # four hits a file, the last (hardest) taken
        for f in sorted(glob.glob(os.path.join(sys.argv[2], "*-*.flac"))):
            if not any(p in f for p in ("crash", "ride", "hihat-abierto")):
                continue
            x, _ = sf.read(f, always_2d=True)
            m = x.mean(1)
            q = len(m) // 4
            res[os.path.basename(f)] = report(os.path.basename(f)[:-5] + " (4th hit)", m[3 * q:])
    json.dump(res, open(os.path.join(d, "whistle.json"), "w"), indent=1)




def warble(x):
    """The tail's slow swoops: octave bands 1-16 kHz, 20-ms RMS in dB less a quadratic
    trend, band-passed to 0.7-8 Hz — the depth (sd, dB) per band and of the 1-16 kHz
    centroid (Hz); and the dominant swoop rate."""
    a, b = int(0.35 * len(x)), len(x) - int(0.06 * SR)
    d = 240
    fs = SR / d
    sos = signal.butter(2, [0.7, 8.0], btype="band", fs=fs, output="sos")
    out = []
    E = []
    for lo in (1000, 2000, 4000, 8000):
        y = signal.sosfiltfilt(signal.butter(4, [lo, min(2 * lo, 23000)], btype="band", fs=SR, output="sos"), x)
        e = np.sqrt(np.convolve(y * y, np.ones(960) / 960, "same")[a:b:d] + 1e-30)
        E.append(e)
        db = 20 * np.log10(e)
        t = np.arange(len(db))
        r = db - np.polyval(np.polyfit(t, db, 2), t)
        out.append(float(np.std(signal.sosfiltfilt(sos, r))))
    E = np.array(E)
    c = (E ** 2 * np.array([1.5, 3, 6, 12])[:, None]).sum(0) / (E ** 2).sum(0)
    t = np.arange(len(c))
    cr = signal.sosfiltfilt(sos, c - np.polyval(np.polyfit(t, c, 2), t))
    P = np.abs(np.fft.rfft(cr * np.hanning(len(cr)))) ** 2
    f = np.fft.rfftfreq(len(cr), 1 / fs)
    return out, float(np.std(cr) * 1000), float(f[np.argmax(P)])


def warble_main():
    d = sys.argv[2]
    for key in ["drs-crash-v127", "virt-crash-v127", "virt-ride-v80", "drs-ride-v127",
                "virt-hat_open-v127", "drs-hat_open-v127", "drs-hat_half-v80"]:
        for r in ["R0", "R1", "R2", "R3", "R3b", "R3o", "R4"]:
            x, _ = sf.read(os.path.join(d, f"{key}-{r}.wav"))
            w, c, fr = warble(x)
            print(f"{key:20s} {r:4s} swoop depth dB 1/2/4/8k " + " ".join(f"{v:4.2f}" for v in w)
                  + f"   centroid sd {c:6.0f} Hz at {fr:4.1f} Hz", flush=True)
        print()
    if len(sys.argv) > 3:
        for f in sorted(glob.glob(os.path.join(sys.argv[3], "*-*.flac"))):
            if not any(p in f for p in ("crash", "ride", "hihat-abierto")):
                continue
            x, _ = sf.read(f, always_2d=True)
            m = x.mean(1)
            q = len(m) // 4
            w, c, fr = warble(m[3 * q:])
            print(f"{os.path.basename(f)[:-5]:30s} swoop depth dB 1/2/4/8k " + " ".join(f"{v:4.2f}" for v in w)
                  + f"   centroid sd {c:6.0f} Hz at {fr:4.1f} Hz", flush=True)


def lone_band(x):
    """The end of a clip (its last 0.8 s before the fade): the quarter-octave band that
    stands most over its neighbours (dB over the median of the bands within ±1 octave),
    its centre, and how much it grew against the 0.8 s before (dB, re the whole's change)."""
    e = len(x) - int(0.06 * SR)
    seg = lambda a, b: np.abs(np.fft.rfft(x[a:b] * np.hanning(b - a))) ** 2
    f = np.fft.rfftfreq(int(0.8 * SR), 1 / SR)
    c = 250 * 2 ** (np.arange(0, 26) / 4)
    def bands(P):
        return np.array([10 * np.log10(P[(f >= k * 2 ** -0.125) & (f < k * 2 ** 0.125)].sum() + 1e-30) for k in c])
    L1 = bands(seg(e - int(0.8 * SR), e))
    L0 = bands(seg(e - int(1.6 * SR), e - int(0.8 * SR)))
    ex = np.array([L1[i] - np.median(L1[max(0, i - 4):i + 5]) for i in range(len(c))])
    i = int(np.argmax(ex))
    grow = (L1[i] - L0[i]) - (10 * np.log10(np.sum(10 ** (L1 / 10))) - 10 * np.log10(np.sum(10 ** (L0 / 10))))
    return float(ex[i]), float(c[i]), float(grow)


def lone_main():
    d = sys.argv[2]
    for key in ["drs-crash-v127", "virt-crash-v127", "virt-ride-v80", "drs-ride-v127",
                "virt-hat_open-v127", "drs-hat_open-v127"]:
        row = []
        for r in ["R0", "R1", "R2", "R3", "R3b", "R3o", "R4"]:
            x, _ = sf.read(os.path.join(d, f"{key}-{r}.wav"))
            ex, hz, g = lone_band(x)
            row.append(f"{r} {ex:4.1f} dB @{hz / 1000:4.1f}k {g:+4.1f}")
        print(f"{key:20s} " + " | ".join(row), flush=True)
    if len(sys.argv) > 3:
        for f in sorted(glob.glob(os.path.join(sys.argv[3], "*-*.flac"))):
            if not any(p in f for p in ("crash", "ride", "hihat-abierto")):
                continue
            x, _ = sf.read(f, always_2d=True)
            m = x.mean(1)
            q = len(m) // 4
            out = []
            for h in range(4):
                ex, hz, g = lone_band(m[h * q:(h + 1) * q])
                out.append(f"{ex:4.1f} dB @{hz / 1000:4.1f}k {g:+4.1f}")
            print(f"{os.path.basename(f)[:-5]:30s} hits 1-4: " + " | ".join(out), flush=True)


def blips(x):
    """Musical noise: short isolated tonal blobs in the tail (the last 60 % before the
    fade). STFT 2048/256; a cell is a blip cell when it stands 10 dB over the median of
    its 9 frames x 9 bins neighbourhood (1-16 kHz, within 50 dB of the tail's loudest);
    connected cells lasting 10-80 ms and at most 3 bins wide are one blip. Returns blips
    per second and the share of the tail's 1-16 kHz energy in them."""
    from scipy import ndimage as nd
    a, b = int(0.4 * len(x)), len(x) - int(0.06 * SR)
    f, t, S = signal.stft(x[a:b], SR, nperseg=2048, noverlap=2048 - 256, boundary=None)
    P = np.abs(S) ** 2
    d = 10 * np.log10(P + 1e-30)
    k = (f >= 1000) & (f <= 16000)
    d, P = d[k], P[k]
    med = nd.median_filter(d, size=(9, 9))
    m = (d - med >= 10) & (d >= d.max() - 50)
    lab, n = nd.label(m)
    sl = nd.find_objects(lab)
    cnt, e = 0, 0.0
    for i, s in enumerate(sl):
        dur = (s[1].stop - s[1].start) * 256 / SR
        if 0.01 <= dur <= 0.08 and s[0].stop - s[0].start <= 3:
            cnt += 1
            e += P[lab == i + 1].sum()
    return cnt / ((b - a) / SR), e / P.sum()


def blips_main():
    d = sys.argv[2]
    for key in ["drs-crash-v127", "virt-crash-v127", "virt-ride-v80", "drs-ride-v127",
                "virt-hat_open-v127", "drs-hat_open-v127", "drs-hat_half-v80", "drs-snare-v127"]:
        row = []
        for r in ["R0", "R1", "R2", "R3", "R3b", "R3o", "R4"]:
            x, _ = sf.read(os.path.join(d, f"{key}-{r}.wav"))
            n, e = blips(x)
            row.append(f"{r} {n:5.1f}/s {100 * e:4.1f}%")
        print(f"{key:20s} " + " | ".join(row), flush=True)
    if len(sys.argv) > 3:
        for f in sorted(glob.glob(os.path.join(sys.argv[3], "*-*.flac"))):
            if not any(p in f for p in ("crash", "ride", "hihat-abierto")):
                continue
            x, _ = sf.read(f, always_2d=True)
            m = x.mean(1)
            q = len(m) // 4
            out = [blips(m[h * q:(h + 1) * q]) for h in range(4)]
            print(f"{os.path.basename(f)[:-5]:30s} hits 1-4: " + " | ".join(f"{n:5.1f}/s {100 * e:4.1f}%" for n, e in out), flush=True)


def thirds(x, a, b):
    P = np.abs(np.fft.rfft(x[a:b] * np.hanning(b - a))) ** 2
    f = np.fft.rfftfreq(b - a, 1 / SR)
    c = 1000 * 2 ** (np.arange(-8, 13) / 3)
    return c, np.array([10 * np.log10(P[(f >= k * 2 ** (-1 / 6)) & (f < k * 2 ** (1 / 6))].sum() + 1e-30) for k in c])


def excess_main():
    """Where a tail holds more than the recording's: per 1/3 octave, the level re the
    recording (each normalised to its own 0.2-0.7 s energy), in the last second and the
    second before."""
    d = sys.argv[2]
    rows = [(k, r) for k in ["drs-crash-v127", "virt-ride-v80", "virt-hat_open-v127", "drs-ride-v127", "virt-crash-v127"]
            for r in ["R1", "R3", "R3b", "R3o", "R4"]]
    for key, r in rows:
        x0, _ = sf.read(os.path.join(d, f"{key}-R0.wav"))
        x1, _ = sf.read(os.path.join(d, f"{key}-{r}.wav"))
        print(f"{key:20s} {r:4s} " + excess(x0, x1), flush=True)
    if len(sys.argv) > 3:
        for kit in ("studio", "jazz"):
            for el in ("crash", "ride", "hihat-abierto"):
                y = {}
                for v in ("2-nuevo", "3-grabado"):
                    x, _ = sf.read(os.path.join(sys.argv[3], f"{kit}-{el}-{v}.flac"), always_2d=True)
                    y[v] = x.mean(1)
                q = min(len(y["2-nuevo"]), len(y["3-grabado"])) // 4
                for h in (1, 3):
                    seg = lambda z: z[h * q:(h + 1) * q]
                    print(f"b9 {kit:6s} {el:13s} hit {h + 1} " + excess(seg(y["3-grabado"]), seg(y["2-nuevo"])), flush=True)


def excess(x0, x1):
    n = min(len(x0), len(x1))
    e = n - int(0.06 * SR)
    on0 = int(np.argmax(np.abs(x0) > 0.05 * np.abs(x0).max()))
    a, b = on0 + int(0.2 * SR), on0 + int(0.7 * SR)
    c, r0 = thirds(x0, a, b)
    _, r1 = thirds(x1, a, b)
    g = 10 * np.log10(np.sum(10 ** (r0 / 10))) - 10 * np.log10(np.sum(10 ** (r1 / 10)))
    out = []
    for lo, hi in ((e - 2 * SR, e - SR), (e - SR, e)):
        _, l0 = thirds(x0, lo, hi)
        _, l1 = thirds(x1, lo, hi)
        dd = l1 + g - l0
        top = np.argsort(dd)[::-1][:3]
        out.append(" ".join(f"{c[i] / 1000:4.1f}k{dd[i]:+4.0f}" for i in top))
    return "  second-last s: " + out[0] + "   last s: " + out[1]


def swell(x, on):
    """A tail that stops decaying: per octave band (125 Hz - 16 kHz), its 100-ms level
    after 1 s, the most it climbs over its own running minimum (dB). A free decay never
    climbs; a recording's beating climbs 1-3 dB."""
    out = []
    e = len(x) - int(0.06 * SR)
    for lo in (125, 250, 500, 1000, 2000, 4000, 8000):
        y = signal.sosfiltfilt(signal.butter(4, [lo, min(2 * lo, 23000)], btype="band", fs=SR, output="sos"), x)
        k = int(0.1 * SR)
        L = [10 * np.log10(np.mean(y[i:i + k] ** 2) + 1e-30) for i in range(on + SR, e - k, k // 2)]
        L = np.array(L)
        out.append(float(np.max(L - np.minimum.accumulate(L))) if len(L) else 0.0)
    return out


def swell_main():
    d = sys.argv[2]
    info = json.load(open(os.path.join(d, "ladder.json")))
    print("octave bands 125 250 500 1k 2k 4k 8k: the late climb (dB)")
    for key in ["drs-crash-v127", "virt-crash-v127", "drs-ride-v127", "virt-ride-v80", "drs-ride_bell-v127",
                "drs-hat_open-v127", "virt-hat_open-v127", "drs-hat_half-v127"]:
        for r in ["R0", "R1", "R3", "R3b", "R3o", "R4"]:
            x, _ = sf.read(os.path.join(d, f"{key}-{r}.wav"))
            s = swell(x, info[key]["onset"])
            print(f"{key:20s} {r:4s} " + " ".join(f"{v:5.1f}" for v in s) + f"   max {max(s):5.1f}", flush=True)
    if len(sys.argv) > 3:
        for f in sorted(glob.glob(os.path.join(sys.argv[3], "*-*.flac"))):
            if not any(p in f for p in ("crash", "ride", "hihat-abierto")):
                continue
            x, _ = sf.read(f, always_2d=True)
            m = x.mean(1)
            q = len(m) // 4
            mx = []
            for h in range(4):
                seg = m[h * q:(h + 1) * q]
                on = int(np.argmax(np.abs(seg) > 0.05 * np.abs(seg).max()))
                mx.append(max(swell(seg, on)))
            print(f"{os.path.basename(f)[:-5]:30s} max climb per hit: " + " ".join(f"{v:5.1f}" for v in mx), flush=True)


if __name__ == "__main__":
    if sys.argv[1] == "warble":
        warble_main()
    elif sys.argv[1] == "lone":
        lone_main()
    elif sys.argv[1] == "blips":
        blips_main()
    elif sys.argv[1] == "excess":
        excess_main()
    elif sys.argv[1] == "swell":
        swell_main()
    else:
        main()
