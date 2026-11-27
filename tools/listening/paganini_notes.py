"""The Paganini comparison's notes (round 42): Caprice No. 24's theme (with its first half's
repeat, as played) from the public-domain MIDI on Wikimedia Commons (`Paganini caprice24
theme.mid`, Hyacinth, 2018), aligned to the recording by DTW on the pitch track and moved
onto the recording's onsets; Variation 2 transcribed from the recording (YIN, semitone runs,
octave errors folded, repeated notes split at onsets). Variation 1 (arpeggios over three
octaves, bounced across the strings) is left out: see the report.

Writes, into <work>:
  pasaje-original.wav       the two excerpts joined (theme 0.30–17.40 s, variation 2
                            35.85–53.85 s of take 03), 48 kHz stereo, float
  notes.json                every note: recording time, passage time, key, articulation,
                            velocity, the bow (new / slurred), and its score time
  arcus-alineado.txt        the score for `paganini_render` at the recording's timing,
                            with CC1 following the recording's level
  arcus-partitura.txt       the same notes at the score's tempo (the MIDI's 150 bpm),
                            the dynamic flat at mf, no rubato

    python -I tools/listening/paganini_notes.py <work dir>
"""
import json
import os
import struct
import sys

import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import paganini_common as pc  # noqa: E402

MIDI = os.path.join(pc.REFS, "paganini", "commons", "Paganini_caprice24_theme.mid")
THEME = (0.30, 17.40)
VAR2 = (35.85, 53.85)
JOIN = THEME[1] - VAR2[0]  # var 2's recording time + JOIN = its passage time (- THEME[0])
SCORE_BPM = 150.0          # the MIDI's tempo (400 000 us a quarter)
FRAME = 0.02


def read_midi(path):
    """(notes [(beat, beats, key, vel)], ticks per quarter) of a type-1 file."""
    d = open(path, "rb").read()

    def vlq(b, i):
        v = 0
        while True:
            c = b[i]
            i += 1
            v = (v << 7) | (c & 0x7F)
            if c < 0x80:
                return v, i

    _, ntr, div = struct.unpack(">HHH", d[8:14])
    i, notes = 14, []
    for _ in range(ntr):
        ln = struct.unpack(">I", d[i + 4:i + 8])[0]
        b = d[i + 8:i + 8 + ln]
        i += 8 + ln
        j = t = run = 0
        on = {}
        while j < len(b):
            dt, j = vlq(b, j)
            t += dt
            st = b[j]
            if st == 0xFF:
                ln2, j2 = vlq(b, j + 2)
                j = j2 + ln2
                continue
            if st in (0xF0, 0xF7):
                ln2, j2 = vlq(b, j + 1)
                j = j2 + ln2
                continue
            if st & 0x80:
                run = st
                j += 1
            typ = run & 0xF0
            if typ in (0x80, 0x90, 0xA0, 0xB0, 0xE0):
                a, c = b[j], b[j + 1]
                j += 2
                if typ == 0x90 and c > 0:
                    on[a] = (t, c)
                elif typ in (0x80, 0x90) and a in on:
                    s, v = on.pop(a)
                    notes.append((s / div, (t - s) / div, a, v))
            else:
                j += 1
    return sorted(notes), div


def frames_of(t, k, ap, a, b):
    """The recording's pitch per FRAME over [a, b): the median key of the periodic frames
    (NaN where none)."""
    n = int((b - a) / FRAME)
    out = np.full(n, np.nan)
    idx = ((t - a) / FRAME).astype(int)
    ok = (idx >= 0) & (idx < n) & (ap < 0.3)
    for f in range(n):
        s = ok & (idx == f)
        if s.sum() >= 2:
            out[f] = np.median(k[s])
    return out


def dtw_align(rec, score, rec_on, score_on, start):
    """Steps (1,1), (1,2), (2,1) — the tempo ratio held within 1/2 … 2 — over a cost of
    the pitch distance (octave errors at a small price, an unpitched frame 0.6, a rest
    0.3), plus the onsets: a score note's first frame costs more where the recording has
    no onset, its other frames a little more where it has one (so repeated notes find
    their own strokes). The path starts at recording frame `start` (±2). Returns the
    recording frame of each score frame."""
    S, R = len(score), len(rec)
    c = np.empty((S, R))
    for i, p in enumerate(score):
        if np.isnan(p):
            c[i] = 0.3
            continue
        d = np.abs(rec - p)
        d = np.minimum(d, np.minimum(np.abs(d - 12) + 1.0, np.abs(d - 24) + 1.5))
        row = np.minimum(d, 3.0) / 3.0
        row[np.isnan(rec)] = 0.6
        if score_on[i]:
            row = row + 0.8 * (1.0 - rec_on)
        else:
            row = row + 0.25 * rec_on
        c[i] = row
    inf = 1e18
    D = np.full((S, R), inf)
    lo, hi = max(start - 2, 0), min(start + 3, R)
    D[0, lo:hi] = c[0, lo:hi]
    back = np.zeros((S, R), np.int8)
    for i in range(1, S):
        a = np.full(R, inf)
        a[1:] = D[i - 1, :-1]
        b = np.full(R, inf)
        b[2:] = D[i - 1, :-2]
        cc = np.full(R, inf)
        if i >= 2:
            cc[1:] = D[i - 2, :-1] + c[i - 1, 1:]
        m = np.minimum(np.minimum(a, b), cc)
        back[i] = np.where(m == a, 0, np.where(m == b, 1, 2))
        D[i] = m + c[i]
    j = int(np.argmin(D[-1]))
    path = np.zeros(S, int)
    i = S - 1
    path[i] = j
    while i > 0:
        st = back[i, j]
        if st == 0:
            i, j = i - 1, j - 1
        elif st == 1:
            i, j = i - 1, j - 2
        else:
            path[i - 1] = j - 1
            i, j = i - 2, j - 1
        path[i] = j
    return path


def snap(times, onsets, keys, t, k, ap, win=0.08):
    """Each aligned time moved to the strongest-placed onset within `win` whose pitch just
    after agrees (within 1.5 semitones, octave-folded); kept in order."""
    out = []
    prev = -1.0
    for x, key in zip(times, keys):
        cand = onsets[(onsets > max(x - win, prev + 0.03)) & (onsets < x + win)]
        best = None
        for o in cand:
            s = (t >= o + 0.015) & (t < o + 0.07) & (ap < 0.3)
            if s.sum() < 2:
                continue
            d = abs(np.median(k[s]) - key)
            d = min(d, abs(d - 12))
            if d < 1.5 and (best is None or abs(o - x) < abs(best - x)):
                best = o
        y = best if best is not None else max(x, prev + 0.03)
        out.append(float(y))
        prev = y
    return out


def regularize(ons, beats, min_share=0.5):
    """A snapped onset that leaves its note under `min_share` of the time its score value
    asks at the local tempo is put back where that tempo places it (between its
    neighbours)."""
    t = list(ons)
    for i in range(1, len(t) - 1):
        span = beats[i + 1] - beats[i - 1]
        if span <= 0:
            continue
        local = (t[i + 1] - t[i - 1]) / span
        want = (beats[i] - beats[i - 1]) * local
        if t[i] - t[i - 1] < min_share * want or t[i + 1] - t[i] < min_share * (beats[i + 1] - beats[i]) * local:
            t[i] = t[i - 1] + want
    return t


def transcribe(t, k, ap, onsets, a, b):
    """Variation 2 from the pitch track: semitone runs of at least 30 ms, an octave error
    folded back towards its neighbours, a run under 40 ms between two others dropped unless
    it is a semitone neighbour (the turns), runs of one key joined unless an onset parts
    them. Returns [(on, key)]."""
    sel = (t >= a) & (t < b)
    tt, q, aa = t[sel], np.round(k[sel]), ap[sel]
    q = np.where(aa < 0.3, q, -1)
    runs, i = [], 0
    while i < len(q):
        j = i
        while j + 1 < len(q) and q[j + 1] == q[i]:
            j += 1
        if q[i] > 0 and tt[j] - tt[i] >= 0.025:
            runs.append([tt[i], tt[j] + 0.005, int(q[i])])
        i = j + 1
    # octave errors: a run 10+ semitones from both neighbours moves by octaves towards them
    for n in range(len(runs)):
        nb = [runs[m][2] for m in (n - 1, n + 1) if 0 <= m < len(runs)]
        if not nb:
            continue
        ref = float(np.median(nb))
        while runs[n][2] - ref >= 9.5:
            runs[n][2] -= 12
        while ref - runs[n][2] >= 9.5:
            runs[n][2] += 12
    # short runs that are not a neighbour's semitone or tone: out
    keep = []
    for n, r in enumerate(runs):
        dur = r[1] - r[0]
        if dur < 0.04:
            nb = [runs[m][2] for m in (n - 1, n + 1) if 0 <= m < len(runs)]
            if not any(abs(r[2] - x) in (1, 2) for x in nb):
                continue
        keep.append(r)
    # join runs of one key unless an onset parts them
    out = []
    for r in keep:
        if out and out[-1][2] == r[2]:
            between = onsets[(onsets > out[-1][1] - 0.03) & (onsets < r[0] + 0.03)]
            if r[0] - out[-1][1] < 0.06 and len(between) == 0:
                out[-1][1] = r[1]
                continue
        out.append(r)
    # a note starts at the onset just before its run, when one is near
    notes = []
    for r in out:
        o = onsets[(onsets > r[0] - 0.06) & (onsets <= r[0] + 0.01)]
        on = float(o[-1]) if len(o) else float(r[0])
        if notes and on <= notes[-1][0] + 0.03:
            on = float(r[0])
        notes.append((on, r[2]))
    return notes


def shape_notes(notes, te, e, end):
    """Each note's end, articulation and bow from the level: the end where it falls 9 dB
    under its peak before the next onset (else at the next onset); a gap of 30 ms or more is
    a detached stroke (staccato under 0.16 s); joined notes with no dip of 6 dB between
    them are slurred. Adds 'off', 'art', 'new_bow', 'peak_db'."""
    for n, x in enumerate(notes):
        nxt = notes[n + 1]["t"] if n + 1 < len(notes) else end
        s = (te >= x["t"]) & (te < nxt)
        seg_t, seg = te[s], e[s]
        if len(seg) == 0:
            x.update(off=nxt, art="sus", new_bow=True, peak_db=-60.0)
            continue
        p = int(np.argmax(seg))
        pk = float(seg[p])
        low = np.nonzero(seg[p:] < pk - 9.0)[0]
        off = float(seg_t[p + low[0]]) if len(low) else nxt
        x["peak_db"] = pk
        x["off"] = off
        x["gap"] = nxt - off
    for n, x in enumerate(notes):
        prev = notes[n - 1] if n else None
        detached = x.get("gap", 0) >= 0.03
        if detached:
            x["art"] = "stac" if x["off"] - x["t"] < 0.16 else "sus"
        else:
            x["art"] = "sus"
        x["new_bow"] = True
        if prev is not None and prev.get("gap", 1) < 0.03 and x["section"] == "var2":
            # the level between the two: a dip of 6 dB under both peaks is a bow change
            s = (te >= x["t"] - 0.04) & (te < x["t"] + 0.02)
            dip = float(e[s].min()) if s.any() else -99
            if dip > min(prev["peak_db"], x["peak_db"]) - 6.0:
                x["new_bow"] = False
                x["art"] = "leg"
    return notes


def cc_curve(te, e, notes, a, b, shift):
    """CC1 every 20 ms over [a, b) from the level smoothed over 80 ms: the 5th percentile of
    the notes' level maps to 0.2, the 98th to 0.95."""
    from scipy.ndimage import uniform_filter1d
    sel = (te >= a) & (te < b)
    tt, ee = te[sel], uniform_filter1d(e[sel], int(0.08 / np.median(np.diff(te))))
    playing = np.zeros(len(tt), bool)
    for x in notes:
        playing |= (tt >= x["t"]) & (tt < x["off"])
    lo, hi = np.percentile(ee[playing], 5), np.percentile(ee[playing], 98)
    out = []
    for g in np.arange(a, b, 0.02):
        i = np.searchsorted(tt, g)
        if i >= len(tt):
            break
        v = float(np.clip(0.2 + 0.75 * (ee[i] - lo) / (hi - lo), 0.12, 1.0))
        out.append((g + shift, v))
    return out


def main(work):
    os.makedirs(work, exist_ok=True)
    x, sr = pc.recording()
    m = x[:, 0]
    cache = os.path.join(work, "track.npz")
    if os.path.exists(cache):
        z = np.load(cache)
        t, k, ap, onsets, ft, fr, te, e = (z[n] for n in ("t", "k", "ap", "on", "ft", "fr",
                                                            "te", "e"))
    else:
        mm = m[: int(60 * sr)]
        t, k, ap = pc.pitch_track(mm, sr)
        onsets, ft, fr = pc.flux_onsets(mm, sr)
        te, e = pc.env_db(mm, sr)
        np.savez(cache, t=t, k=k, ap=ap, on=onsets, ft=ft, fr=fr, te=te, e=e)

    # ---- the theme: the MIDI aligned
    midi, _ = read_midi(MIDI)
    beat = 60.0 / SCORE_BPM
    total = max(b0 + d for b0, d, _, _ in midi)
    S = int(np.ceil(total * beat / FRAME)) + 1
    score = np.full(S, np.nan)
    score_on = np.zeros(S, bool)
    for b0, d, key, _ in midi:
        f0, f1 = int(b0 * beat / FRAME), int((b0 + max(d, 0.25)) * beat / FRAME)
        score[f0:f1] = key
        score_on[f0] = True
    rec = frames_of(t, k, ap, THEME[0], THEME[1])
    # the recording's onset strength per frame: the flux ratio's peak in it, 0 … 1
    rec_on = np.zeros(len(rec))
    for f in range(len(rec)):
        s_ = (ft >= THEME[0] + f * FRAME) & (ft < THEME[0] + (f + 1) * FRAME)
        if s_.any():
            r_ = float(fr[s_].max())
            rec_on[f] = float(np.clip((r_ - 1.0) / 3.0, 0.0, 1.0))
    first = onsets[onsets > THEME[0]][0]
    path = dtw_align(rec, score, rec_on, score_on, int((first - THEME[0]) / FRAME))
    approx = [THEME[0] + path[min(int(b0 * beat / FRAME), S - 1)] * FRAME for b0, _, _, _ in midi]
    keys = [key for _, _, key, _ in midi]
    ons = regularize(snap(approx, onsets, keys, t, k, ap), [b0 for b0, _, _, _ in midi])
    notes = []
    for (b0, d, key, vel), on in zip(midi, ons):
        notes.append(dict(section="theme", t=on, key=int(key), score_beat=b0, score_beats=d,
                          midi_vel=int(vel)))
    # ---- variation 2: transcribed
    v2 = transcribe(t, k, ap, onsets, VAR2[0] + 0.1, VAR2[1] - 0.25)
    for on, key in v2:
        notes.append(dict(section="var2", t=on, key=int(key)))
    th = [n for n in notes if n["section"] == "theme"]
    va = [n for n in notes if n["section"] == "var2"]
    shape_notes(th, te, e, THEME[1] - 0.2)
    shape_notes(va, te, e, VAR2[1] - 0.05)
    # the theme's last note and variation 2's: held to the level's own fall
    # ---- the score's own time for variation 2: its onsets quantized to 16ths of its mean beat
    q16 = float(np.median(np.diff([n["t"] for n in va])))
    q16 = 0.125 if not 0.09 < q16 < 0.16 else q16
    b_start = va[0]["t"]
    pos = 0
    for n, x2 in enumerate(va):
        x2["score_beat"] = pos / 4.0
        nxt = va[n + 1]["t"] if n + 1 < len(va) else x2["off"] + 0.05
        steps = max(1, int(round((nxt - x2["t"]) / q16)))
        x2["score_beats"] = steps / 4.0
        pos += steps
    del b_start
    # passage times
    for x2 in notes:
        sh = -THEME[0] if x2["section"] == "theme" else JOIN - THEME[0]
        x2["p_on"] = x2["t"] + sh
        x2["p_off"] = x2["off"] + sh
        lo, hi = -40.0, -14.0
        x2["vel"] = float(np.clip(0.35 + 0.55 * (x2["peak_db"] - lo) / (hi - lo), 0.25, 0.95))
    # ---- the passage, original
    a0, a1 = int(THEME[0] * sr), int(THEME[1] * sr)
    b0_, b1 = int(VAR2[0] * sr), int(VAR2[1] * sr)
    fade = int(0.05 * sr)
    tail = x[b1 - fade:b1].copy() * np.linspace(1, 0, fade)[:, None]
    orig = np.concatenate([x[a0:a1], x[b0_:b1 - fade], tail])
    pc.audio.write(os.path.join(work, "pasaje-original.wav"), orig, sr, subtype="FLOAT")
    # ---- the scores
    cc = cc_curve(te, e, th, THEME[0], THEME[1], -THEME[0]) + \
        cc_curve(te, e, va, VAR2[0], VAR2[1], JOIN - THEME[0])

    def note_line(on, off, x2):
        return f"note {on:.4f} {off:.4f} {x2['key']} {x2['vel']:.3f} {x2['art']} -"

    lines = ["# Caprice 24, theme + variation 2 — at the recording's timing (paganini_notes.py)"]
    lines += [f"cc {tt:.3f} 0 {v:.3f}" for tt, v in cc]
    for n, x2 in enumerate(notes):
        off = x2["p_off"]
        nxt = notes[n + 1] if n + 1 < len(notes) else None
        if nxt is not None and nxt["art"] == "leg" and nxt["section"] == x2["section"]:
            off = nxt["p_on"] + 0.02  # overlapped: the next note slurs
        lines.append(note_line(x2["p_on"], off, x2))
    open(os.path.join(work, "arcus-alineado.txt"), "w", encoding="utf-8").write("\n".join(lines) + "\n")

    # at the score's tempo: the theme from 0.3 s, variation 2 a bar after it
    lines = ["# Caprice 24, theme + variation 2 — at the score's tempo (150 bpm), mf, no rubato"]
    lines.append("cc 0 0 0.6")
    beat = 60.0 / SCORE_BPM
    t_th = 0.3
    t_va = t_th + (2 * np.ceil(max(n["score_beat"] + n["score_beats"] for n in th) / 2) + 2) * beat
    for n, x2 in enumerate(notes):
        base = t_th if x2["section"] == "theme" else t_va
        on = base + x2["score_beat"] * beat
        if x2["section"] == "theme":
            dur = x2["score_beats"] * beat
            # the recording's articulation kept: a staccato at most its written length
            if x2["art"] == "stac":
                dur = 0.6 * dur
        else:
            dur = x2["score_beats"] * beat
        off = on + dur
        nxt = notes[n + 1] if n + 1 < len(notes) else None
        if nxt is not None and nxt["art"] == "leg" and nxt["section"] == x2["section"]:
            off = base + nxt["score_beat"] * beat + 0.02
        v = dict(x2)
        v["vel"] = 0.6
        lines.append(note_line(on, off, v))
    open(os.path.join(work, "arcus-partitura.txt"), "w", encoding="utf-8").write("\n".join(lines) + "\n")
    json.dump(dict(theme=THEME, var2=VAR2, join=JOIN, score_bpm=SCORE_BPM, notes=notes),
              open(os.path.join(work, "notes.json"), "w"), indent=1)
    print(f"{len(th)} theme notes, {len(va)} variation-2 notes; passage {len(orig) / sr:.2f} s")
    for x2 in notes:
        print(f"{x2['section']:5s} {x2['t']:7.3f} {x2['off'] - x2['t']:5.3f} {pc.name(x2['key']):4s} "
              f"{x2['art']:4s} {'new' if x2['new_bow'] else 'slur'} {x2['peak_db']:6.1f}")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else r"F:\projects\cantus-work\listen\paganini\work")
