"""The Paganini comparison's notes (round 42): Caprice No. 24's theme (with its first half's
repeat, as played) from the public-domain MIDI on Wikimedia Commons (`Paganini caprice24
theme.mid`, Hyacinth, 2018) and Variation 2 from the public-domain score on Wikimedia Commons
(`Caprice-24-a4.pdf`, David Chan's engraving for the Mutopia project, PD-old; its notes,
fingerings and slurs entered by hand in `VAR2_BARS`), each aligned to the recording by DTW
on the pitch track and moved onto the recording's onsets. Variation 1 (arpeggios over three
octaves, bounced across the strings) is left out: see the report.

Variation 2's first beat in every bar alternates two strings: an open string (fingered 0)
or a stopped note (1) with its lower neighbour on the string below (fingered 4), so the
upper string rings on under the lower note. `VAR2_BARS` gives those notes their strings, and
the variation is played with one bow on two strings (`set pair 1`, ADR 0164). `crossings()`
measures, bar by bar, how loud the upper string still sounds under the lower note.

Writes, into <work>:
  pasaje-original.wav       the two excerpts joined (theme 0.30-17.40 s, variation 2
                            35.85-53.85 s of take 03), 48 kHz stereo, float
  notes.json                every note: recording time, passage time, key, string,
                            articulation, velocity, the bow (new / slurred), its score time;
                            and the crossings measured in the recording
  arcus-{alineado,partitura}-{tema,var2}.txt, arcus-alineado-var2-1cuerda.txt
                            the scores for `paganini_render`: at the recording's timing
                            (CC1 following the recording's level) or at the score's tempo
                            (150 bpm, mf, no rubato); the theme one string a note, variation 2
                            with `pair` as asked

    python -I tools/listening/paganini_notes.py <work dir> [pair for variation 2, default 1]
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

# Variation 2 as the public-domain score writes it (Commons `Caprice-24-a4.pdf`, page 1):
# per bar, beat 1's four 16ths with their strings (0 G ... 3 E; the score's fingerings 0 0 or
# 1 4 put the 1st and 3rd on the upper string, the 2nd and 4th on the one below; None where
# it marks none), the grace note before beat 2, and beat 2's notes (16ths, or (key, beats)).
# Every bar is one slur (bar 4: the slur ends on its E5, the E4 is its own up-bow). Bars 1-4
# are repeated.
_A, _E = (2, 1, 2, 1), (3, 2, 3, 2)
VAR2_BARS = [
    ([69, 68, 69, 68], _A, 70, [69, 71, 72, 74]),                 # 1  open A over G#
    ([76, 75, 76, 74], _E, 77, [76, 74, 72, 71]),                 # 2  open E over D#, D
    ([69, 68, 69, 68], _A, 70, [69, 71, 72, 74]),                 # 3
    ([76, 75, 76, 74], _E, 77, [(76, 0.5), (64, 0.5)]),           # 4  E5, then E4 up-bow
    ([81, 80, 81, 79], _E, 82, [81, 79, 77, 76]),                 # 5  1 4: A5 (E) over G#5 (A)
    ([74, 73, 74, 72], None, 75, [74, 76, 77, 74]),               # 6  no fingering
    ([79, 78, 79, 77], _E, 80, [79, 77, 76, 74]),                 # 7  1 4
    ([72, 71, 72, 71], None, 73, [72, 74, 76, 72]),               # 8  no fingering
    ([77, 76, 77, 76], _E, 79, [77, 74, 72, 71]),                 # 9  1 4
    ([76, 75, 76, 74], _E, 77, [76, 72, 71, 69]),                 # 10 0 0
    ([71, 70, 71, 69], _A, 72, [71, 77, 76, 68]),                 # 11 1 4: B4 (A) over A#4 (D)
    ([69, 68, 69, 68], _A, 71, [(69, 0.5)]),                      # 12 0 0, the A an 8th
]
VAR2_FORM = [0, 1, 2, 3, 0, 1, 2, 3] + list(range(4, 12))
GRACE_BEATS = 0.1


def var2_score():
    """Variation 2's notes as played (the repeat written out): dicts with beat, beats, key,
    string, bar, src_bar, new_bow, grace, cross (its beat-1 notes on two strings), cross_lo
    (a lower note of such a pair)."""
    out, bar_at = [], 0.0
    for n_bar, b in enumerate(VAR2_FORM):
        beat1, strings, grace, beat2 = VAR2_BARS[b]
        for i, key in enumerate(beat1):
            out.append(dict(beat=bar_at + 0.25 * i, beats=0.25, key=key,
                            string=None if strings is None else strings[i], bar=n_bar + 1,
                            src_bar=b + 1, new_bow=i == 0, grace=False,
                            cross=strings is not None, cross_lo=i % 2 == 1))
        out[-1]["beats"] = 0.25 - GRACE_BEATS
        out.append(dict(beat=bar_at + 1.0 - GRACE_BEATS, beats=GRACE_BEATS, key=grace,
                        string=None, bar=n_bar + 1, src_bar=b + 1, new_bow=False, grace=True,
                        cross=False, cross_lo=False))
        at = bar_at + 1.0
        for x in beat2:
            key, d = (x, 0.25) if isinstance(x, int) else x
            out.append(dict(beat=at, beats=d, key=key, string=None, bar=n_bar + 1,
                            src_bar=b + 1, new_bow=(b == 3 and key == 64), grace=False,
                            cross=False, cross_lo=False))
            at += d
        bar_at += 2.0
    return out


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


def align(score, region, t, k, ap, onsets, ft, fr):
    """The score's notes [(beat, beats, key)] aligned to the recording over `region`: DTW,
    then each onset snapped and regularized. Returns the onset times."""
    beat = 60.0 / SCORE_BPM
    total = max(b0 + d for b0, d, _ in score)
    S = int(np.ceil(total * beat / FRAME)) + 1
    sc = np.full(S, np.nan)
    sc_on = np.zeros(S, bool)
    for b0, d, key in score:
        f0, f1 = int(b0 * beat / FRAME), int((b0 + max(d, 0.25)) * beat / FRAME)
        sc[f0:f1] = key
        sc_on[f0] = True
    rec = frames_of(t, k, ap, region[0], region[1])
    rec_on = np.zeros(len(rec))
    for f in range(len(rec)):
        s_ = (ft >= region[0] + f * FRAME) & (ft < region[0] + (f + 1) * FRAME)
        if s_.any():
            rec_on[f] = float(np.clip((float(fr[s_].max()) - 1.0) / 3.0, 0.0, 1.0))
    cand = onsets[(onsets > region[0]) & (onsets < region[1])]
    st = int((cand[0] - region[0]) / FRAME)
    for o in cand:  # the first onset with a pitch after it: the section's first note
        sel = (t >= o + 0.02) & (t < o + 0.08) & (ap < 0.3)
        if sel.sum() >= 3:
            st = int((o - region[0]) / FRAME)
            break
    path = dtw_align(rec, sc, rec_on, sc_on, st)
    approx = [region[0] + path[min(int(b0 * beat / FRAME), S - 1)] * FRAME for b0, _, _ in score]
    return regularize(snap(approx, onsets, [kk for _, _, kk in score], t, k, ap),
                      [b0 for b0, _, _ in score])


def crossings(m, sr, notes, key_t="t"):
    """Variation 2's first beats, bar by bar (`paganini_common.crossing_beat`): how far the
    upper note rings on under its lower neighbour, their balance there and their interval.
    `key_t` names the notes' time in `m` ("t" the recording's, "p_on" the passage's). Bars 6
    and 8 of the score carry no fingering: measured too."""
    out = []
    for bar in sorted({n["bar"] for n in notes if n.get("section") == "var2"}):
        bn = [n for n in notes if n.get("section") == "var2" and n["bar"] == bar]
        g = [n for n in bn if n["grace"]][0]
        a, b = bn[0][key_t], g[key_t]
        r = pc.crossing_beat(m, sr, a, b, bn[0]["key"], bn[1]["key"])
        if r is None:
            continue
        r.update(bar=bar, src_bar=bn[0]["src_bar"], at=a, hi=bn[0]["key"], lo=bn[1]["key"],
                 marked=bn[0]["cross"])
        out.append(r)
    return out


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


def main(work, pair=1.0):
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
    ons = align([(b0, d, key) for b0, d, key, _ in midi], THEME, t, k, ap, onsets, ft, fr)
    th = [dict(section="theme", t=on, key=int(key), score_beat=b0, score_beats=d,
               midi_vel=int(vel), string=None, grace=False)
          for (b0, d, key, vel), on in zip(midi, ons)]
    shape_notes(th, te, e, THEME[1] - 0.2)

    # ---- variation 2: the score aligned (the grace notes placed after, 50 ms before their
    # main note)
    sc = var2_score()
    main_ = [n for n in sc if not n["grace"]]
    ons = align([(n["beat"], n["beats"], n["key"]) for n in main_], VAR2, t, k, ap, onsets, ft, fr)
    for n, on in zip(main_, ons):
        n["t"] = on
    for i, n in enumerate(sc):
        if n["grace"]:
            prev, nxt = sc[i - 1]["t"], sc[i + 1]["t"]
            n["t"] = max(nxt - 0.05, prev + 0.6 * (nxt - prev))
    va = []
    for n in sc:
        va.append(dict(section="var2", t=n["t"], key=n["key"], score_beat=n["beat"],
                       score_beats=n["beats"], string=n["string"], grace=n["grace"],
                       bar=n["bar"], src_bar=n["src_bar"], cross=n["cross"],
                       cross_lo=n["cross_lo"], score_new_bow=n["new_bow"]))
    shape_notes(va, te, e, VAR2[1] - 0.05)
    # the score's bows: every bar one slur, bar 4's E4 its own bow
    for n in va:
        n["new_bow"] = n["score_new_bow"]
        n["art"] = "sus" if n["new_bow"] else "leg"
    # the crossings as the recording has them
    cross_orig = crossings(m, sr, va)

    notes = th + va
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
    def note_line(on, off, x2):
        st = "-" if x2.get("string") is None else str(x2["string"])
        return f"note {on:.4f} {off:.4f} {x2['key']} {x2['vel']:.3f} {x2['art']} - {st}"

    def write(name, head, part, cc, timing):
        lines = [f"# Caprice 24, {name} (paganini_notes.py)"] + head
        lines += [f"cc {tt:.3f} 0 {v:.3f}" for tt, v in cc]
        for n, x2 in enumerate(part):
            on, off = timing(n, x2)
            lines.append(note_line(on, off, x2))
        open(os.path.join(work, name + ".txt"), "w", encoding="utf-8").write("\n".join(lines) + "\n")

    def aligned(part):
        def f(n, x2):
            off = x2["p_off"]
            nxt = part[n + 1] if n + 1 < len(part) else None
            if nxt is not None and nxt["art"] == "leg":
                off = nxt["p_on"] + 0.02  # overlapped: the next note slurs
            return x2["p_on"], off
        return f

    beat = 60.0 / SCORE_BPM
    t_th = 0.3
    t_va = t_th + (2 * np.ceil(max(n["score_beat"] + n["score_beats"] for n in th) / 2) + 2) * beat

    def scored(part, base):
        def f(n, x2):
            on = base + x2["score_beat"] * beat
            dur = x2["score_beats"] * beat
            if x2["art"] == "stac":
                dur = 0.6 * dur
            off = on + dur
            nxt = part[n + 1] if n + 1 < len(part) else None
            if nxt is not None and nxt["art"] == "leg":
                off = base + nxt["score_beat"] * beat + 0.02
            return on, off
        return f

    cc_th = cc_curve(te, e, th, THEME[0], THEME[1], -THEME[0])
    cc_va = cc_curve(te, e, va, VAR2[0], VAR2[1], JOIN - THEME[0])
    pair_head = [f"set pair {pair:g}"]
    write("arcus-alineado-tema", [], th, cc_th, aligned(th))
    write("arcus-alineado-var2", pair_head, va, cc_va, aligned(va))
    # the control: the same variation 2 on one string at a time (no pair)
    write("arcus-alineado-var2-1cuerda", ["set pair 0"], va, cc_va, aligned(va))
    flat = [dict(v, vel=0.6) for v in th]
    write("arcus-partitura-tema", [], flat, [(0.0, 0.6)], scored(flat, t_th))
    flat = [dict(v, vel=0.6) for v in va]
    write("arcus-partitura-var2", pair_head, flat, [(0.0, 0.6)], scored(flat, t_va))
    json.dump(dict(theme=THEME, var2=VAR2, join=JOIN, score_bpm=SCORE_BPM, pair=pair,
                   notes=notes, crossings_original=cross_orig),
              open(os.path.join(work, "notes.json"), "w", encoding="utf-8"), indent=1)
    print(f"{len(th)} theme notes, {len(va)} variation-2 notes; passage {len(orig) / sr:.2f} s")
    print("variation 2, the crossings in the recording (upper string under the lower note):")
    for c in cross_orig:
        print(f"  bar {c['bar']:2d} (score {c['src_bar']:2d}{'' if c['marked'] else ', unmarked'}) "
              f"{pc.name(c['hi'])} over {pc.name(c['lo'])}: falls {c['ring_db']:5.1f} dB, "
              f"balance {c['balance_db']:+6.1f} dB, interval {c['interval_cents']:+6.1f} ct")
    for x2 in va:
        print(f"var2 bar {x2['bar']:2d} {x2['t']:7.3f} {pc.name(x2['key']):4s} {x2['art']:4s} "
              f"{'g' if x2['grace'] else ' '} {x2['string']}")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else r"F:\projects\cantus-work\listen\paganini\work",
         float(sys.argv[2]) if len(sys.argv) > 2 else 1.0)
