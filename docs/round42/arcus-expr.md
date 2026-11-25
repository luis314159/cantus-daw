# Round 42 — `arcus-expr`: Arcus played like a human

**Builder:** `arcus-expr` (worktree `daw-r42/arcus-expr`, branch `r42/arcus-expr`, from
`r42/integration` 2ebb24c1c). **The owner's ask (2026-11-25):** "a better-programmed version,
with more expressiveness: different attacks, different vibratos, volume swells … where the bow
comes from, how it attacks, which part of the bow it plays with … so it sounds more natural."

Two stages: **1** — the quartet demo and a solo showcase performed with what Arcus can already
do, entered as a user enters it; **2** — what the instrument itself lacked, so any part sounds
human without programming everything.

## 1. The study: how players shape music, measured

### 1.1 The HSD quartet (Mozart K465, *Andante cantabile*, separated stems, CC BY 4.0)

`tools/arcus/expr/phrase_study.py refs` (`$CANTUS_WORK/arcus/expr/hsd_study.json`): the study's
YIN tracker over each stem, notes cut where the rounded semitone changes for 60 ms or the
sound stops; per note its length, its vibrato's extent and onset, its level envelope; across
stems the balance (calibrated by each stem's 94 dB SPL tone) and the onset asynchrony. The long
take (bars 75–114, 141 s; 108 / 148 / 115 / 260 notes) — medians, [p25, p75]:

| | 0.12–0.3 s | 0.3–0.6 s | 0.6–1.2 s | ≥ 1.2 s |
|---|---|---|---|---|
| notes | 260 | 151 | 130 | 90 |
| vibrato extent, cents p-p | 17.5 [9.5, 30] | 22.8 [12, 42] | 19.0 [10, 39] | 21.1 [15, 32] |
| share with a vibrato (≥ 8 cents) | **0.57** | 0.87 | 0.82 | **0.94** |
| vibrato onset (50 % of its extent), s | 0 | 0 [0, 0.03] | 0 [0, 0.10] | **0 [0, 0.15]** |
| level peak, share of the note | 0.52 | 0.48 | 0.41 | **0.55** [0.34, 0.85] |
| start under the peak, dB | 3.3 | 5.2 | 9.2 | **11.9** [6.3, 17.3] |
| end under the peak, dB | 3.2 | 6.4 | 16.9 | **14.2** [6.5, 19.8] |

- **Long notes swell** (messa di voce): the peak at a little over half the note, 12 dB over
  its first 60 ms and 14 dB over its last; the 0.6–1.2 s notes peak earlier (0.41) and die
  further (17 dB).
- **The vibrato does not wait inside a phrase**: its onset is 0 s at the median at every
  length (the single notes of pass 5 — a quarter in at 0.2–0.25 s — are notes from silence).
  Short notes carry one about half the time (the line's vibrato goes on through them).
- **Balance** (0.5 s windows where all four play, 173 windows): Vl1 +1.1 dB [−1.8, +4.3]
  over the four's mean, Vl2 −2.7, Vla −1.2, Vlc +3.5 (Mozart gives the cello the tune in
  part of this excerpt): the melody sits **2–4 dB** over the inner parts.
- **Asynchrony** (each Vl1 onset against the nearest other onset within 80 ms): 23–41 ms median
  |Δt|, σ 31–42 ms. The stems were recorded one by one over a guide track, and the onsets come
  from a pitch track on a 5 ms grid with a two-period window, so these are an upper bound; the
  ensemble literature (Rasch 1979: 30–50 ms rms in trios) is of the same order. The player here
  stays far under it on purpose (σ 3–4.5 ms): "a few ms, never sloppy".

### 1.2 The reference study's measures used

- **Vibrato** (pass 5, `bowed-refdb/vibrato.json`, 3 102 single notes and Philharmonia's
  phrases): 5.0–5.7 Hz, 15–44 cents p-p, wider loud (×1.4–1.9) and high; onset a quarter in at
  0.2–0.25 s from silence (later soft); slurred notes 5.8 Hz, 31 cents, the vibrato carried
  through the slur.
- **Attacks and swells** (`bowed-realism.md` §2.5): real notes swell 0.1–0.45 s at soft and
  medium dynamics, 0.03–0.08 s loud; `t_steady` 0.28–0.51 s.
- **Legato and bow changes** (§2.10): a finger change 20–35 ms with a 3 dB dip; a détaché's
  dip 7–11 dB; martelé 9–10 dB.
- **The gesture** (§3.4): bow force dominates the brightness; players move force and contact
  point more than speed; force and speed per note are contours (Maestre 2010), not steps.

### 1.3 The literature for what the stems cannot say

- **Bowing:** down-bows on strong beats, up-bows on pickups (the "rule of the down-bow", Galamian,
  *Principles of Violin Playing and Teaching*, 1962); a slur is one bow.
- **Expressive intonation:** players raise leading tones and other rising tendency tones and
  lower falling ones by a few to about 10 cents in melodic contexts (Fyk 1995; Kanno 2003) —
  used here at +6 / −5 cents, on the melodic parts only.
- **Portamento:** a slide on a position shift, part of the romantic style and used sparingly in
  modern playing (Leech-Wilkinson 2009, *The Changing Sound of Music*, ch. 4).
- **Timing:** a phrase's end slows, its peak is lengthened (Todd 1992's phrase arch; Repp 1992).
