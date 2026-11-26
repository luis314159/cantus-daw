#!/usr/bin/env python3
"""Real drumming to MIDI (round 42, tympanum-real): a tool, never shipped.

    python -I tools/tympanum-real/transcribe.py templates <hits.jsonl> <hits.wav> <templates.npz>
    python -I tools/tympanum-real/transcribe.py run <audio> <templates.npz> <out.csv> [--mid out.mid]
    python -I tools/tympanum-real/transcribe.py score <truth notes.csv> <found.csv>

Partially-fixed NMF drum transcription (Dittmar & Gärtner, DAFx 2014; Wu et al., IEEE/ACM
TASLP 2018's NMF family), written here so nothing NC is used (madmom's and ADTOF's models are
CC BY-NC-SA): a log-frequency magnitude spectrogram (2048/256, 72 bands 30 Hz–16 kHz) is
factored as W·H with W started from each class's template — measured off a recorded kit's
hits (`templates`: the first 60 ms of every hit of that class, median) — and allowed to adapt
to the recording (W blended back toward the template each step), H the activations. A hit is
a peak of H's rise over an adaptive threshold (its 0.5-s median + δ, 30 ms apart); its
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
N, HOP, NB = 2048, 256, 72


def bands(fs):
    edges = np.geomspace(30, min(16000, fs / 2 * 0.95), NB + 1)
    f = np.fft.rfftfreq(N, 1 / fs)
    M = np.zeros((NB, len(f)))
    for b in range(NB):
        m = (f >= edges[b]) & (f < edges[b + 1])
        if not m.any():
            m[np.argmin(np.abs(f - np.sqrt(edges[b] * edges[b + 1])))] = True
        M[b, m] = 1.0
    return M


def spec(x, fs):
    _, _, Z = signal.stft(x, fs, nperseg=N, noverlap=N - HOP, boundary=None, padded=True)
    return bands(fs) @ np.abs(Z)


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


def templates(jsonl, wav, out):
    x, fs = mono(wav)
    rows = [json.loads(l) for l in open(jsonl, encoding="utf-8")]
    starts = {int(r["start"]): r for r in csv.DictReader(open(wav[:-4] + ".csv", encoding="utf-8"))}
    T, levels = {}, {}
    for r in starts.values():
        c = cls_of({"kind": r["kind"], "art": r["art"], "open": float(r["open"])})
        if not c:
            continue
        a = int(r["start"])
        S = spec(x[a:a + int(0.06 * fs) + N], fs)
        T.setdefault(c, []).append(S.mean(axis=1))
    for r in rows:
        c = cls_of(r)
        if c:
            levels.setdefault(c, {}).setdefault(r["vel"], []).append(r["rms80_db"])
    W = np.stack([np.median(np.array(T[c]), axis=0) for c, _, _ in CLASSES], axis=1)
    W /= W.sum(axis=0, keepdims=True)
    lv = {c: sorted((v, float(np.mean(x))) for v, x in levels[c].items()) for c in levels}
    np.savez(out, W=W, levels=json.dumps(lv), fs=fs)
    print(f"templates: {[c for c, _, _ in CLASSES]} from {wav}")


def nmf(V, W0, iters=40, keep=0.85):
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


def picks(h, fs, delta):
    fr = fs / HOP
    d = np.maximum(0.0, np.diff(h, prepend=h[0]))
    w = int(0.5 * fr) | 1
    thr = signal.medfilt(d, w) + delta * d.max()
    pk, _ = signal.find_peaks(d, height=thr, distance=max(1, int(0.03 * fr)))
    # the activation's own peak within the next 30 ms gives the strength
    amp = [h[p:p + max(2, int(0.03 * fr))].max() for p in pk]
    return pk, np.array(amp)


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


def run(audio, tpl, out, mid=None, delta=0.06):
    x, fs = mono(audio)
    z = np.load(tpl)
    levels = json.loads(str(z["levels"]))
    if int(z["fs"]) != fs:
        x = signal.resample_poly(x, int(z["fs"]), fs)
        fs = int(z["fs"])
    V = spec(x, fs)
    _, H = nmf(V, z["W"])
    notes = []
    for i, (c, key, cc4) in enumerate(CLASSES):
        pk, amp = picks(H[i], fs, delta)
        for p, v in zip(pk, vel_map(levels, c, amp)):
            notes.append((p * HOP / fs, key, int(v), cc4 if cc4 is not None else -1))
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


def score(truth, found, tol=0.03):
    def rd(p):
        r = [l.split(",") for l in open(p, encoding="utf-8").read().splitlines()[1:] if l]
        return [(float(a[0]), int(a[1])) for a in r]
    fold = {36: 36, 38: 38, 40: 38, 37: 38, 42: 42, 44: 42, 46: 46, 49: 49, 51: 51, 53: 51}
    t, f = rd(truth), rd(found)
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
        templates(*sys.argv[2:5])
    elif m == "run":
        mid = sys.argv[sys.argv.index("--mid") + 1] if "--mid" in sys.argv else None
        run(sys.argv[2], sys.argv[3], sys.argv[4], mid)
    elif m == "score":
        score(sys.argv[2], sys.argv[3])
