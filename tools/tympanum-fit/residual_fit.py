"""The hybrid's residual: what a recorded kit holds that model 5 does not (round 40, drums-hyb;
ADR 0124's third amendment).

    python -I residual_fit.py <rec takes.wav> <model takes.wav> <probe.wav> <schedule.txt>
                              <pulsus_measured.bin> <out residual.crs> <out report.txt>

Inputs are `tympanum_hybrid takes` (the recorded kit and the modelled kit, generated, the
same schedule) and `tympanum_hybrid probe` (the engine's own response from each piece's
source to the mix). For every articulation x layer x take:

1. the recording aligned to the model's onset (both through the engine, mono);
2. the model's expected power E|M|^2 per STFT bin, over the layer's four model hits (its
   seeded phases differ each hit: a waveform difference would be wrong at any other seed);
3. the residual = the recording's STFT under the mask sqrt(max(0, 1 - E|M|^2 / |R|^2)):
   played with any hit of the model, the powers add up to the recording's;
4. kept for the piece's length T (30-200 ms: where the residual's share of the recording
   falls below -10 dB for good, the 90th percentile of the piece's hits), faded out over
   its last 40 %;
5. deconvolved to the piece's dry source: its probe's magnitude smoothed to a sixth of an
   octave, divided out (zero phase), and the probe's first arrival taken off — so the
   engine's mics, bleed and room give it back its colour and delay, and follow a mic moved.

The report says, per piece, how much of the recording's energy the model already explains
over T (1 - E_residual / E_recording) and over its first 30 ms.
"""
import os
import sys

import numpy as np
import soundfile as sf
from scipy.ndimage import uniform_filter
from scipy.signal import stft, istft

# `python -I` leaves the script's own folder off the path: the fit's sibling module, by name.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import residual_pack  # noqa: E402

FS = 48000
NFFT = 512
HOP = 128
TMIN, TMAX = 0.030, 0.200


def read_schedule(path):
    slot = None
    rows = []
    for line in open(path, encoding="utf-8"):
        if line.startswith("#"):
            slot = float(line.split("slot ")[1].split(" s")[0])
            continue
        p = line.split()
        if not p:
            continue
        rows.append(dict(i=int(p[0]), name=p[1], key=int(p[2]), open=float(p[3]),
                         layer=int(p[4]), layer_vel=int(p[5]), take=int(p[6]), vel=int(p[7])))
    return slot, rows


def onset(x, look=int(0.3 * FS)):
    """The first sample at -20 dB of the hit's peak (its first 300 ms)."""
    y = np.abs(x[:look])
    pk = y.max()
    if pk <= 0:
        return 0
    return int(np.argmax(y > pk * 0.1))


def shift(x, n):
    """x delayed by n samples (n < 0: earlier), the same length."""
    out = np.zeros_like(x)
    if n >= 0:
        out[n:] = x[:len(x) - n]
    else:
        out[:n] = x[-n:]
    return out


def env(x, frame=240):
    n = len(x) // frame
    return (x[:n * frame].reshape(n, frame) ** 2).mean(axis=1)


def smooth_oct(mag, frac=6.0):
    """|H| smoothed over 1/frac octave."""
    f = np.arange(len(mag))
    out = np.empty_like(mag)
    for k in range(len(mag)):
        if k == 0:
            out[k] = mag[0]
            continue
        lo = max(1, int(k * 2 ** (-0.5 / frac)))
        hi = min(len(mag) - 1, int(np.ceil(k * 2 ** (0.5 / frac))))
        out[k] = np.sqrt(np.mean(mag[lo:hi + 1] ** 2))
    return out


def main():
    rec_p, mod_p, probe_p, sched_p, blob_p, out_p, rep_p = sys.argv[1:8]
    rec, fs = sf.read(rec_p, dtype="float64")
    mod, fs2 = sf.read(mod_p, dtype="float64")
    probe, fs3 = sf.read(probe_p, dtype="float64")
    assert fs == fs2 == fs3 == FS
    slot, rows = read_schedule(sched_p)
    S = int(round(slot * FS))
    W = int(0.45 * FS)  # the analysis window from the slot's start
    model_hash = residual_pack.fnv1a(open(blob_p, "rb").read())

    # The probe: each articulation's slot, in the schedule's art order.
    arts = []
    for r in rows:
        k = (r["key"], r["open"], r["name"])
        if k not in arts:
            arts.append(k)
    deconv = {}
    for a, (key, op, name) in enumerate(arts):
        h = probe[a * S:a * S + W]
        pk = np.abs(h).max()
        d0 = int(np.argmax(np.abs(h) > 0.5 * pk))
        H = np.abs(np.fft.rfft(h, 2 * W))
        Hs = smooth_oct(H)
        Hs = np.maximum(Hs, Hs.max() * 10 ** (-40 / 20))
        deconv[a] = (d0, Hs, pk)

    # Per hit: the aligned recording, the model.
    hits = {}
    for r in rows:
        s = r["i"] * S
        x = rec[s:s + W].copy()
        m = mod[s:s + W].copy()
        x = shift(x, onset(m) - onset(x))
        hits[r["i"]] = (x, m)

    by_layer = {}
    for r in rows:
        a = arts.index((r["key"], r["open"], r["name"]))
        by_layer.setdefault((a, r["layer"]), []).append(r)

    res = {}
    stats = {}
    excess = {}
    for (a, l), rs in by_layer.items():
        # The model's expected power over the layer's hits.
        pm = None
        for r in rs:
            _, _, Z = stft(hits[r["i"]][1], FS, nperseg=NFFT, noverlap=NFFT - HOP)
            p = np.abs(Z) ** 2
            pm = p if pm is None else pm + p
        pm /= len(rs)
        for r in rs:
            x = hits[r["i"]][0]
            _, _, R = stft(x, FS, nperseg=NFFT, noverlap=NFFT - HOP)
            pr = np.abs(R) ** 2
            # The mask from powers smoothed over 7 bins (650 Hz) and 3 frames (8 ms): one
            # take's own bin-to-bin scatter is not the model's lack (subtracting bin by bin
            # would leave max(0, |R|² − E|M|²) where the two match on average — a third too
            # much); the mask then keeps the recording's own fine structure where it applies.
            prs = uniform_filter(pr, size=(7, 3), mode="nearest")
            pms = uniform_filter(pm, size=(7, 3), mode="nearest")
            mask = np.sqrt(np.clip(1.0 - pms / np.maximum(prs, 1e-30), 0.0, 1.0))
            # Where the model is louder than the recording (a residual cannot take it away).
            over = np.sum(np.clip(pms - prs, 0, None)[:, :int(0.2 * FS / HOP)])
            excess.setdefault(a, []).append(over / max(np.sum(prs[:, :int(0.2 * FS / HOP)]), 1e-30))
            _, y = istft(R * mask, FS, nperseg=NFFT, noverlap=NFFT - HOP)
            y = y[:W]
            if len(y) < W:
                y = np.pad(y, (0, W - len(y)))
            res[r["i"]] = y
            # Where the residual stops mattering: its share of the recording below -10 dB
            # for good (5-ms frames), from the onset.
            o = onset(hits[r["i"]][1])
            ex, ey = env(x[o:]), env(y[o:])
            share = ey / np.maximum(ex, 1e-20)
            loud = np.nonzero(share > 0.1)[0]
            t_end = (loud[-1] + 1) * 240 / FS if len(loud) else TMIN
            # Only up to 300 ms looked at.
            stats[r["i"]] = dict(t=float(np.clip(t_end, TMIN, TMAX)), o=o)

    # Per articulation: its length, the 90th percentile of its hits'.
    t_art = {}
    for a in range(len(arts)):
        ts = [stats[r["i"]]["t"] for r in rows if arts.index((r["key"], r["open"], r["name"])) == a]
        t_art[a] = float(np.ceil(np.percentile(ts, 90) * 100) / 100)

    # Faded, measured, deconvolved to the source.
    out_arts = []
    report = ["# the hybrid's residual: how much the model explains (1 - E_res / E_rec)",
              "# excess: the model's power above the recording's, first 200 ms (what a residual cannot remove)",
              "# piece        T(ms)  explained(T)  explained(30ms)  excess   per layer explained(T)",
              ""]
    take_src = {}
    for a, (key, op, name) in enumerate(arts):
        e_rec = sum(np.sum(hits[r["i"]][0] ** 2) for r in rows
                    if arts.index((r["key"], r["open"], r["name"])) == a)
        e_mod = sum(np.sum(hits[r["i"]][1] ** 2) for r in rows
                    if arts.index((r["key"], r["open"], r["name"])) == a)
        if e_rec < 1e-10 or e_mod < 1e-10:
            # The recorded kit lacks it (or the model does): the model alone plays it.
            report.append("%-12s  no residual (recording %s, model %s)" % (
                name, "silent" if e_rec < 1e-10 else "ok", "silent" if e_mod < 1e-10 else "ok"))
            continue
        T = t_art[a]
        d0, Hs, _ = deconv[a]
        ex_t, ex_30, per_layer = [], [], {}
        for r in rows:
            if arts.index((r["key"], r["open"], r["name"])) != a:
                continue
            o = stats[r["i"]]["o"]
            x = hits[r["i"]][0]
            y = res[r["i"]].copy()
            n_end = o + int(T * FS)
            n_fade = int(0.4 * T * FS)
            w = np.zeros(W)
            w[:n_end] = 1.0
            w[n_end - n_fade:n_end] = 0.5 * (1 + np.cos(np.linspace(0, np.pi, n_fade)))
            y *= w
            er, ey = np.sum(x[o:n_end] ** 2), np.sum(y[o:n_end] ** 2)
            e30r, e30y = np.sum(x[o:o + 1440] ** 2), np.sum(y[o:o + 1440] ** 2)
            ex_t.append(1 - ey / max(er, 1e-20))
            ex_30.append(1 - e30y / max(e30r, 1e-20))
            per_layer.setdefault(r["layer_vel"], []).append(1 - ey / max(er, 1e-20))
            # To the source: |H| divided out, the first arrival taken off.
            Y = np.fft.rfft(y, 2 * W)
            src = np.fft.irfft(Y / Hs, 2 * W)[:W]
            src = shift(src, -d0)
            n = min(W, n_end - d0 + 64)
            src = src[:n]
            # A short fade at the very end (the division's smear).
            src[-64:] *= np.linspace(1, 0, 64)
            take_src[r["i"]] = src
        report.append("%-12s %5.0f  %10.1f %%  %12.1f %%  %5.1f %%  %s" % (
            name, T * 1000, 100 * np.mean(ex_t), 100 * np.mean(ex_30), 100 * np.mean(excess[a]),
            "  ".join("v%d %.0f%%" % (v, 100 * np.mean(e)) for v, e in sorted(per_layer.items()))))
        layers = []
        for l in sorted({r["layer"] for r in rows}):
            rs = [r for r in rows if r["layer"] == l and arts.index((r["key"], r["open"], r["name"])) == a]
            rs.sort(key=lambda r: r["take"])
            layers.append(dict(vel=rs[0]["layer_vel"], rows=rs))
        for li, L in enumerate(layers):
            rho = 0.0
            if li + 1 < len(layers):
                cs = []
                for ra, rb in zip(L["rows"], layers[li + 1]["rows"]):
                    p, q = take_src[ra["i"]], take_src[rb["i"]]
                    n = min(len(p), len(q))
                    den = np.sqrt(np.sum(p[:n] ** 2) * np.sum(q[:n] ** 2))
                    cs.append(np.sum(p[:n] * q[:n]) / den if den > 0 else 0.0)
                rho = float(np.clip(np.mean(cs), -1, 1))
            L["rho"] = rho
            L["takes"] = [take_src[r["i"]].astype(np.float64) for r in L["rows"]]
        out_arts.append(dict(key=key, open=255 if op < 0 else int(round(op * 100)),
                             layers=[dict(vel=L["vel"], rho=L["rho"], takes=L["takes"]) for L in layers]))
    size = residual_pack.write(out_p, FS, model_hash, out_arts)
    report.append("")
    report.append("pack %s: %d bytes, model tables 0x%08X" % (out_p, size, model_hash))
    open(rep_p, "w", encoding="utf-8").write("\n".join(report) + "\n")
    print("\n".join(report))


if __name__ == "__main__":
    main()
