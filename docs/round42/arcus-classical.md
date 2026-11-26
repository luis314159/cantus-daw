# Round 42 — Arcus's classical sound (the `arcus-classical` builder)

**The owner's request** (2026-11-26): *"Ve arreglando los puntos del clásico"* — the ranked list of
Part 2 of the Arcus analysis (`F:\projects\cantus-work\listen\arcus-analisis\index.html`, *clásico*;
its data `clasico\parte2.json`): violin, viola, cello and double bass, played by Arcus's bowed
string and its player (ADR 0162), measured against recordings by the sound lab (ADR 0160) on the
held-out cells, and the solos' phrases against the HSD quartet's players. ADR 0166 (provisional
number) holds the decision; this file the work and the numbers.

## How it is measured

- `tools/arcus/classical/lab.sh <run> [<bin dir>] [<before>]` — the four instruments' held-out
  notes rendered at Arcus's defaults (the cello and the bass by `arcus_match render`, the violin
  and the viola by `arcus_upper_listen <dir> study`, every fourth key), measured by the lab, judged
  on the held-out cells; `summary.py` puts the pass's rows beside the analysis's run
  (`analisis-clasico`, the base `e63ceddd2`, which this branch's base build reproduced bit for
  bit). The violin's, the viola's and the bass's subjects are the analysis's scratch subjects,
  kept in `tools/arcus/classical/subjects/`; the cello's is the lab's. `INSTS="violin cello"`
  measures a few.
- `ARCUS_SHAPE=name=value,…` hands a shape to try to the renders (`arcus::shape::Knobs::parse`,
  `ArcusSettings::shape_try`, never stored) — one build, many tries. Every value below was chosen
  on the held-out report; the runs are in `$CANTUS_WORK/arcus-classical/runs/c1 … c8`.
- `tools/arcus/classical/phrase.py` — the solos' phrases (`arcus_demo <dir> solos`) by the
  arcus-expr phrase study's note cutter: intonation p90, the long notes' start and end under
  their peak, the vibrato's extent and onset, beside the HSD players' numbers.
- `tools/arcus/classical/clips.py` and `listen_page.py` — the owner's clips (the analysis's
  windows of the solos, found by correlation: 2.16–2.28 s, 19.95 s long) and his page.

## What changed

1. **The stroke's shape** (`crates/cantus-dsp/src/arcus_shape.rs`, the player's): a contour in dB
   moves the bow's speed (the force riding it: Schelleng's limit and the region are linear in the
   speed, `Bowing::scaled`) and, for 0.6 of it, the dynamic the bowing is read at (the contact
   and the force's place), that share lagging 0.3 s (the tone opens after the level rises). From
   silence it starts at the instrument's swell depth (within 15 dB) over 1.6 × its length (0.2 …
   0.6 s) — the output's amplitude swell is retired with the player on; a bow change starts its
   stroke 6 dB under (a fast passage's 0.3 of it); a slurred note starts 3 dB under where the
   last had sagged; after the rise the stroke sags (6 dB, τ 1.2 s) and breathes (a seeded walk of
   about a dB, 0.45 s steps); the key let go **tapers** the bow (12 dB over 1.5 × `bow.release`)
   and the bow lifts still moving (slowed on the string it had stopped the string: the
   fundamental fell 22 dB in 40 ms). The hair's noise rides the speed.
2. **The free string after the bow** (model 2): a released string the bow has left rings at the
   family's measured pluck T60 and partial fall (`pizz_t60`), never longer than `ring`.
3. **The vibrato's way back** into a note inside a phrase (after a bow change or a slur that is not
   a fast passage's): 0.05 of itself until 0.12 s, full by 0.5 s.
4. **The player's intonation in a passage** (never a note from silence): each note's seeded error
   (≈ 8 cents spread, × 1.6 on a shift) landing with the finger and corrected to 0.6 of itself
   (τ 0.18 s), a semitone step's tendency (±4 cents).
5. **The corner with the force** (model 2's string): the bridge corner of a sustained stroke
   moves with its force against the same bowing's mf force (power 0.8 above, 0.3 below).
6. **The pluck** (model 2): the loop's DC blocker at 1/1024 of the fundamental on a string the bow
   did not leave pushed (at 1/64 its phase lead, matched at the fundamental only, left the
   second and third partials 3.2 and 3.8 cents flat — the "4 cents flat"), and the finger's
   contact shorter with the period (power 0.8 from the lowest open string, at least 0.3 of it).
7. **The pitch tables** measured again with the player on, over a held note's steady part
   (0.6–2.95 s; three passes for the cello and the bass).
8. **A fiddler's bow** (`bow.pair` on — the Fiddle character): the classical shape, taper,
   vibrato's way back, intonation and corner give way to it, so the Fiddle preset's lines keep
   their measured numbers; instead it shuffles (two or three fast separate notes slurred into one
   bow, seeded), turns a fast change in 0.6 of `bow.change`, and widens a long note's vibrato late
   (from 0.3 s, to 2.4 × its depth and 1.12 × its rate) — the coordinator's fiddle items (a), (b).

Tried and not kept: pressing the force nearer Schelleng's limit at ff (`press_ff`, the loud
cello's partials 2–8 further out), the loss exponent with the force (`loss_force`, no change),
a shorter, shallower start (`fresh_db 0.75, fresh_s 1.0`: the cello's `t_steady` back to 0.12 s).

## Measures, before → after (held-out cells; run `c8`, the defaults above)

| row | violin | viola | cello | bass | band |
|---|---|---|---|---|---|
| crest factor, groups in | 0/11 → 8/11 | 3/11 → 11/11 | 6/12 → 10/12 | 6/12 → 10/12 | p10–p90 |
| `t_steady` median, groups in | 0.21 s, 1/8 → 0.78, 1/6 (above) | 0.09, 1/10 → 0.37, 9/10 | 0.13, 3/10 → 0.19, 6/10 | 0.13, 6/10 → 0.35, 7/10 | 0.15–0.85 s |
| release, dB/s (groups in) | 168 (1/11) → 86 (3/11) | 134 (3/11) → 82 (6/11) | 61 (9/12) → 73 (9/12) | 62 (7/12) → 68 (7/12) | ≤ 60–150 |
| room T20 | 0.37 → 0.67 s | 0.49 → 0.72 s | 0.86 → 0.76 | 0.83 → 0.77 | 0.5–5 s |
| attack 0–25 ms re 60–160 ms (in) | 10/11 → 8/11 | 9/11 → 4/11 (over) | 8/12 → 10/12 | 7/12 (under) → 3/12 (over) | |
| noise burst (in) | 9/11 → 11/11 | 8/11 → 8/11 | 9/12 → 9/12 | 8/12 → 6/12 | |
| pizz harmonics 1–20, dB RMS out | 12.8 → 5.9 | 8.0 → 1.3 | 11.2 → 4.4 | 3.9 → 1.8 | 0 |
| pizz pitch, median | −3.86 → −0.24 ct | −3.93 → −0.25 | −3.93 → −0.25 | −4.06 → −0.26 | 0 |
| arco within ±1 ct, player on | 91 → 91 % | 64 → 82 % | 80 → 75 % | 98 → 100 % | the gate |

The solos' phrases (`c7`; interpreted takes; HSD players in the last column):

| row | before | now | players |
|---|---|---|---|
| intonation p90 \|dev\| | 6.7–8.2 ct | 10.7–14.2 ct | 13–16 ct |
| long notes: start / end under the peak | 2.3–3.5 / 3.7–4.5 dB | 3.6–4.9 / 4.5–6.5 dB | 7–14 / 10–35 dB |
| long notes: vibrato onset p75 | 0 s | 0.03–0.09 s | 0.26 s |

## Open

- **A note's end inside a slur** cannot taper before the next note without knowing its length:
  the phrases' ends stay 4.5–6.5 dB under the peak against the players' 10–35. Look-ahead on
  playback (the survey's item 7; the engine's `processor*.rs` is `tuning`'s this round).
- **The loud cello stays dull** in its second to eighth partials (r2/r3 loud 5.8 / 6.6 dB RMS
  out, were 4.9 / 6.9): neither the corner, the press nor the loss exponent moved them; the
  violin's loud r1/r3 did come in (2.2 → 1.4, 4.0 → 2.7).
- The bass's and the viola's first 25 ms are now a little strong against the next 100 ms (were
  weak on the bass); the pizzicato is still cleaner between its harmonics than the recordings
  (their floor and room).
- The pitch gate (±1 ct) holds 75–100 % of the lab's held-out notes, not all (the stroke sags and
  breathes through the steady window; the closed loop trims what it can).
- Not measured here: the Fiddle character's lines with the shuffle and the late vibrato (the
  fiddle lab's `fiddle_measure.py`) — next.
