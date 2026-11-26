#!/usr/bin/env python3
"""Real drumming to MIDI (round 42, tympanum-real): a tool, never shipped.

    python -I tools/tympanum-real/transcribe.py templates <hits.jsonl> <templates.npz>
    python -I tools/tympanum-real/transcribe.py run <audio> <templates.npz> <out.csv> [--mid out.mid]
    python -I tools/tympanum-real/transcribe.py score <truth notes.csv> <found.csv> [--offset 0.5]

Partially-fixed NMF drum transcription (Dittmar & Gärtner, DAFx 2014; Wu et al., IEEE/ACM
TASLP 2018's NMF family), written here so nothing NC is used (madmom's and ADTOF's models are
CC BY-NC-SA): a third-octave power spectrogram (2048/256, 31 bands 25 Hz–20 kHz) is
factored as W·H with W started from each class's template — measured off a recorded kit's
hits (`templates`: the first 200 ms's third octaves of every hit of that class at velocity ≥ 64, median) — and allowed to adapt
to the recording (W blended back toward the template each step), H the activations. A hit is
where H rises 6 dB or more within 10 ms, within 32 dB of the class's loudest, 30 ms apart; its
velocity comes from the activation's peak, mapped through the template kit's own level per
velocity (`rms80_db` against `vel` in its measured hits), the class's 97th percentile at that
kit's velocity 112. Classes: kick 36, snare 38, closed hat 42, open hat 46 (cc4 0.9), tom 45,
crash 49, ride 51. `score` gives each class's precision, recall and F (±30 ms) against known
notes — run on a Groove MIDI performance rendered through a recorded kit.
"""
import csv
import json
import struct
import sys

import numpy as np
import soundfile as sf
from scipy import signal

CLASSES = [("kick", 36, None), ("snare", 38, None), ("hat_closed", 42, 0.0),
           ("hat_open", 46, 0.9), ("tom", 45, None), ("crash", 49, None), ("ride", 51, None)]
N, HOP = 2048, 256
SHARE = {"kick": 0.35, "snare": 0.3, "tom": 0.35, "hat_closed": 0.15, "hat_open": 0.2,
         "crash": 0.2, "ride": 0.2}
THIRDS = [25 * 2 ** (k / 3) for k in range(31)]
NB = len(THIRDS)


def bands(fs):
    """Third-octave bands, 25 Hz - 20 kHz (as `features.thirds`, so a kit's measured hits'
    `spec3` are templates as they are)."""
    f = np.fft.rfftfreq(N, 1 / fs)
    M = np.zeros((NB, len(f)))
    for b, c in enumerate(THIRDS):
        m = (f >= c / 2 ** (1 / 6)) & (f < c * 2 ** (1 / 6))
        if not m.any():
            m[np.argmin(np.abs(f - c))] = True
        M[b, m] = 1.0
    return M


def spec(x, fs):
    _, _, Z = signal.stft(x, fs, nperseg=N, noverlap=N - HOP, boundary=None, padded=True)
    return bands(fs) @ (np.abs(Z) ** 2)


def mono(path):
    x, fs = sf.read(path, dtype="float64", always_2d=True)
    return x.mean(axis=1), fs


def cls_of(row):
    k, art, o = row["kind"], row["art"], row["open"]
    if k == "kick":
        return "kick"
    if k == "snare" and art in ("center", "head", "hit", "snare"):
        return "snare"
    if k == "hihat":
        if art in ("pedal", "foot"):
            return None
        return "hat_open" if o >= 0.6 else ("hat_closed" if o <= 0.1 else None)
    if k in ("racktom", "floortom"):
        return "tom"
    if k == "crash":
        return "crash"
    if k == "ride" and art != "bell":
        return "ride"
    return None


def templates(jsonl, out):
    """Each class's template: the median third-octave power spectrum of the first 200 ms of
    the kit's measured hits of that class at velocity 64 or more (`spec3`)."""
    rows = [json.loads(l) for l in open(jsonl, encoding="utf-8")]
    T, levels = {}, {}
    for r in rows:
        c = cls_of(r)
        if not c:
            continue
        levels.setdefault(c, {}).setdefault(r["vel"], []).append(r["rms80_db"])
        if r["vel"] >= 64:
            T.setdefault(c, []).append(10 ** (np.array(r["spec3"]) / 10))
    W = np.stack([np.median(np.array(T[c]), axis=0) for c, _, _ in CLASSES], axis=1)
    W /= W.sum(axis=0, keepdims=True)
    lv = {c: sorted((v, float(np.mean(x))) for v, x in levels[c].items()) for c in levels}
    np.savez(out, W=W, levels=json.dumps(lv), fs=48000)
    print(f"templates: {[c for c, _, _ in CLASSES]} from {jsonl}")


def nmf(V, W0, iters=60, keep=0.5):
    W = W0.copy()
    H = np.full((W.shape[1], V.shape[1]), V.mean() / W.shape[1] + 1e-9)
    eps = 1e-12
    for _ in range(iters):
        WH = W @ H + eps
        H *= (W.T @ (V / WH)) / (W.T.sum(axis=1, keepdims=True) + eps)
        WH = W @ H + eps
        W *= ((V / WH) @ H.T) / (H.sum(axis=1)[None, :] + eps)
        W = keep * W0 + (1 - keep) * W / (W.sum(axis=0, keepdims=True) + eps)
    return W, H


def picks(h, fs, rise_db=6.0, floor_db=32.0):
    """A class's hits: where its activation (power, in dB) rises by `rise_db` or more over the
    next 10 ms, 30 ms apart, and is within `floor_db` of the class's loudest (ghost notes are
    20-35 dB under the accents, so the rise is read in dB, not against the loudest's rise)."""
    fr = fs / HOP
    top = h.max() + 1e-30
    hd = 10 * np.log10(np.maximum(h, top * 1e-6))
    k = max(1, int(round(0.010 * fr)))
    rise = np.zeros_like(hd)
    rise[:-k] = hd[k:] - hd[:-k]
    ok = (rise >= rise_db) & (np.roll(hd, -k) >= 10 * np.log10(top) - floor_db)
    pk, _ = signal.find_peaks(np.where(ok, rise, 0.0), height=rise_db, distance=max(1, int(0.03 * fr)))
    # the strength: the activation's peak within the 30 ms after the rise
    n = max(2, int(0.03 * fr))
    amp = np.array([h[p:p + n].max() for p in pk])
    return pk + k // 2, amp


def vel_map(levels, c, amps):
    tab = levels.get(c)
    if not tab or len(amps) == 0:
        return np.full(len(amps), 90)
    v, l = np.array(tab).T
    a97 = np.percentile(amps, 97)
    l112 = np.interp(112, v, l)
    db = 20 * np.log10(np.maximum(amps, 1e-12) / a97) + l112
    order = np.argsort(l)
    return np.clip(np.round(np.interp(db, l[order], v[order], left=1, right=127)), 1, 127)


def write_mid(path, notes):
    ev = []
    for t, k, v, _ in notes:
        tick = int(round(t * 960))  # 120 BPM, 480 ppq: 960 ticks a second
        ev += [(tick, 0x99, k, int(v)), (tick + 60, 0x89, k, 0)]
    ev.sort()
    trk, last = bytearray(), 0
    for tick, st, k, v in ev:
        d, b = tick - last, []
        last = tick
        b.append(d & 0x7F)
        d >>= 7
        while d:
            b.append(0x80 | (d & 0x7F))
            d >>= 7
        trk += bytes(reversed(b)) + bytes([st, k, v])
    trk += b"\x00\xff\x2f\x00"
    with open(path, "wb") as fh:
        fh.write(b"MThd" + struct.pack(">IHHH", 6, 0, 1, 480))
        fh.write(b"MTrk" + struct.pack(">I", len(trk)) + trk)


def run(audio, tpl, out, mid=None):
    x, fs = mono(audio)
    z = np.load(tpl)
    levels = json.loads(str(z["levels"]))
    if int(z["fs"]) != fs:
        x = signal.resample_poly(x, int(z["fs"]), fs)
        fs = int(z["fs"])
    V = spec(x, fs)
    _, H = nmf(V, z["W"])
    notes = []
    # each class's share of the frame (its activation times its template's energy): a
    # snare's low end lights the kick a little, a kick's the tom; a hit is kept only where
    # its class carries a good share of what sounds then
    E = H * z["W"].sum(axis=0)[:, None]
    share = E / (E.sum(axis=0, keepdims=True) + 1e-30)
    for i, (c, key, cc4) in enumerate(CLASSES):
        pk, amp = picks(H[i], fs)
        n = max(2, int(0.03 * fs / HOP))
        keep = [j for j, p in enumerate(pk) if share[i, p:p + n].max() >= SHARE.get(c, 0.15)]
        pk, amp = pk[keep], amp[keep]
        for p, v in zip(pk, vel_map(levels, c, amp)):
            # a frame's time is its window's centre (the rise is seen as the window reaches it)
            notes.append(((p * HOP + N // 2) / fs, key, int(v), cc4 if cc4 is not None else -1))
    notes.sort()
    hat = 0.0
    with open(out, "w", encoding="utf-8") as fh:
        fh.write("seconds,key,velocity,cc4\n")
        for t, k, v, o in notes:
            hat = o if o >= 0 else hat
            fh.write(f"{t:.4f},{k},{v},{hat:.2f}\n")
    if mid:
        write_mid(mid, notes)
    print(f"{audio}: {len(notes)} hits -> {out}")


def score(truth, found, tol=0.03, offset=0.0):
    def rd(p):
        r = [l.split(",") for l in open(p, encoding="utf-8").read().splitlines()[1:] if l]
        return [(float(a[0]), int(a[1])) for a in r]
    fold = {35: 36, 36: 36, 38: 38, 40: 38, 37: 38, 42: 42, 44: 42, 46: 46, 49: 49, 57: 49, 55: 49,
            52: 49, 51: 51, 53: 51, 59: 51}
    t, f = [(x + offset, k) for x, k in rd(truth)], rd(found)
    keys = sorted({fold.get(k, 45) for _, k in t})
    for k in keys:
        a = sorted(x for x, kk in t if fold.get(kk, 45) == k)
        b = sorted(x for x, kk in f if fold.get(kk, 45) == k)
        used, tp = set(), 0
        for x in a:
            j = np.searchsorted(b, x - tol)
            while j < len(b) and b[j] <= x + tol:
                if j not in used:
                    used.add(j)
                    tp += 1
                    break
                j += 1
        p = tp / len(b) if b else 0.0
        r = tp / len(a) if a else 0.0
        print(f"key {k}: truth {len(a)} found {len(b)}  P {p:.2f} R {r:.2f} F {2 * p * r / (p + r + 1e-9):.2f}")


if __name__ == "__main__":
    m = sys.argv[1]
    if m == "templates":
        templates(sys.argv[2], sys.argv[3])
    elif m == "run":
        mid = sys.argv[sys.argv.index("--mid") + 1] if "--mid" in sys.argv else None
        run(sys.argv[2], sys.argv[3], sys.argv[4], mid)
    elif m == "score":
        off = float(sys.argv[sys.argv.index("--offset") + 1]) if "--offset" in sys.argv else 0.0
        score(sys.argv[2], sys.argv[3], offset=off)
