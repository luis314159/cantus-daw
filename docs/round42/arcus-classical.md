# Round 42 — Arcus's classical sound (the `arcus-classical` builder)

**The owner's request** (2026-11-26): *"Ve arreglando los puntos del clásico"*, then *"sigue
mejorando para que podamos sonar más natural, como esa grabación de Paganini"* — the ranked list of
Part 2 of the Arcus analysis (`F:\projects\cantus-work\listen\arcus-analisis\index.html`,
*clásico*), then the strings-hq references' re-measure (`docs/round42/strings-refs.md`) and the
Paganini comparison (`F:\projects\cantus-work\listen\paganini`). ADR 0166 (provisional number)
holds the decision; this file the work and the numbers. `DSP_VERSION` 20.

## How it is measured

- `tools/arcus/classical/lab.sh <run> [<bin dir>] [<before>]` — the four instruments' held-out
  notes rendered at Arcus's defaults by `arcus_match render <dir> violin viola cello bass` and
  judged by the sound lab (ADR 0160) on the **strings-hq subjects** (`tools/soundlab/subjects/
  arcus-*`, r42/strings-refs merged); `summary.py` puts the pass's rows beside `hq1`
  (r42/integration's Arcus on the same subjects, the refs builder's run, copied with its cache
  into this pass's own `$CANTUS_WORK/arcus-classical/soundlab`). `INSTS=…` measures a few;
  `ARCUS_SHAPE=name=value,…` tries a shape (`shape::Knobs::parse`, `ArcusSettings::shape_try`,
  never stored). The first runs (`c1 … c8`) used the analysis's scratch subjects (kept in
  `tools/arcus/classical/subjects/`, the script's earlier version in git history).
- `tools/arcus/classical/phrase.py` — the solos' phrases (`arcus_demo <dir> solos`) by the
  arcus-expr phrase study's cutter, beside the HSD players; the strings-refs builder's
  `adapters/strings_hq_phrases.py measure` beside the Musopen + HSD players.
- `tools/arcus/classical/clips.py`, `listen_page.py` — the owner's clips and page.

## What changed (DSP_VERSION 20)

1. **The stroke's shape** (`crates/cantus-dsp/src/arcus_shape.rs`, the player's): a contour in dB
   moves the bow's speed (the force riding it: Schelleng's limit and the region are linear in the
   speed, `Bowing::scaled`) and, for 0.6 of it (at most 0.3 of the dynamic), the dynamic the
   bowing is read at, that share lagging 0.3 s. From silence it starts at the instrument's swell
   depth — within 15 dB (the bass 8), shallower when loud (× 1 − 0.8 d), less under C3 (none at
   E1) — over 1.6 × the swell's length (0.2 … 0.6 s); **what the bow may not carry stays the
   output's swell it was** (pass 4's rise). A fresh note in a fast passage bites at once. A
   slurred note starts 3 dB under where the last one had sagged. The stroke holds still 0.3 s, then
   sags (6 dB, τ 1.2 s) and breathes (a seeded walk of about a dB, 0.45 s steps). The key let go
   **tapers** the bow (12 dB over 1.5 × `bow.release`, in proportion to how long the note was held
   up to 0.8 s) and the bow lifts still moving. The hair's noise rides the speed.
2. **The free string after the bow** (model 2): the family's pluck T60 and partial fall, ≤ `ring`.
3. **The vibrato's way back** into a note inside a phrase (0.05 of itself to 0.12 s, full by 0.5 s);
   **the viola's and the cello's vibrato × 0.8** (the HQ phrases: violas 21 cents, cellos 10–25;
   Arcus's 28–31).
4. **The player's intonation in a passage** (never a note from silence): a seeded error (≈ 8 cents,
   × 1.6 on a shift) landing with the finger, corrected to 0.6 of itself (τ 0.18 s); a semitone
   step's tendency ±4 cents.
5. **The corner with the force** on the violin and the viola (power 0.8 above mf, 0.3 under; a
   sustained stroke's own force). On the cello it moved none of the loud partials; on the bass it
   held the E string's attack past Guettler's limit — off there.
6. **The pluck** (model 2): the loop's DC blocker at 1/1024 of the fundamental on a string the bow
   did not leave pushed (at 1/64 its phase lead, matched at the fundamental only, left the second
   and third partials 3.2 and 3.8 cents flat), the finger's contact shorter with the period
   (power 0.8, at least 0.5 of `pizz.soft`).
7. **The pitch tables**: the upper strings' measured again with the player on (0.6–2.95 s); the
   bass's rows measured again (pass 6's window, three passes); the cello's rows pass 6's (they
   keep the pitch and the attack gates with the shape; measured again they cost the open A's pp
   attack its gate).
8. **A fiddler's bow** (`bow.pair` on — the Fiddle character): the classical shape, taper, way
   back, intonation and corner give way to it; instead it **shuffles** (two or three fast separate
   notes slurred into a bow, seeded), turns a fast change in 0.6 of `bow.change`, and widens a long
   note's vibrato late (from 0.3 s, 2.4 × depth, 1.12 × rate) — the coordinator's fiddle items.

Tried, measured, not kept: the force at ff nearer Schelleng's limit (`press_ff`, the loud cello
further out), the loss exponent with the force (`loss_force`, no change), the force riding the
speed through the shape (`RIDE`, the low strings' attacks later), deeper starts (15 dB on the
bass, 12 on the cello's low strings: Guettler's gate), a bow change's own 3–9 dB dip (the viola's
change ticks over the references').

## Measures, before → after (strings-hq subjects, held-out cells; `hq1` → `c11`)

| row (groups in) | violin | viola | cello | bass |
|---|---|---|---|---|
| crest factor | 5/12 → 10/12 (7.6 → 9.8 dB) | 7/12 → 11/12 | 4/12 → 12/12 | 8/12 → 12/12 |
| `t_steady` | 7/10 → 9/10 (0.23 → 0.31 s) | 1/12 → 4/12 (0.11 → 0.15) | 5/11 → 3/11 (0.12 → 0.12) | 6/12 → 8/12 (0.13 → 0.15) |
| rise 10–90 % | 12/12 → 11/12 | 12/12 → 12/12 | 12/12 → 12/12 | 11/12 → 10/12 |
| noise burst | 12/12 → 12/12 | 12/12 → 12/12 | 12/12 → 12/12 | 10/12 → 10/12 |
| 0–25 ms re 60–160 ms | 10/12 → 11/12 | 12/12 → 12/12 | 9/12 → 9/12 | 7/12 → 7/12 |
| release (dB/s) | 2/12 → 6/12 (148 → 74) | 4/12 → 8/12 (129 → 71) | 9/12 → 9/12 | 9/12 → 8/12 |
| room T20 | 4/12 → 8/12 | 9/12 → 11/12 | 9/12 → 8/12 | 10/12 → 9/12 |
| vibrato extent (median) | 24.6 → 23.7 ct | 28.1 → 23.1 | 27.0 → 21.8 | 17.4 → 16.7 |
| harmonics 1–20 (RMS out) | 2.0 → 1.4 dB | 2.1 → 1.9 | 1.2 → 1.4 | 1.2 → 0.9 |
| HNR by band (in) | 70 → 61 % | 56 → 45 % | 70 → 63 % | 73 → 67 % |
| pizz harmonics (RMS out) | 8.5 → 9.1 dB | 7.0 → 7.2 | 5.9 → 2.7 | 6.8 → 4.3 |
| pizz pitch (median) | −3.19 → −0.20 ct | −3.80 → −0.24 | −3.93 → −0.25 | −4.06 → −0.25 |
| arco within ±1 ct | 65 → 72 % | 74 → 78 % | 80 → 80 % | 97 → 97 % |

On the analysis's scratch subjects (`analisis-clasico` → `c8`, before the gate's limits): crest
in 0/11 → 8/11, 3/11 → 11/11, 6/12 → 10/12, 6/12 → 10/12; `t_steady` violin 0.21 → 0.78 s,
viola 0.09 → 0.37, cello 0.13 → 0.19, bass 0.13 → 0.35; the release 168 → 86 and 134 → 82 dB/s.
**The data changed a conclusion:** the deep starts that moved `t_steady` most broke Guettler's
attack gate on the cello's and the bass's low strings; with the gate kept, `t_steady` improved on
the violin and the bass only, not on the cello.

## Phrases (`c11`, the solos' interpreted takes)

By the strings-refs builder's tool on the Musopen + HSD players (`strings_hq_phrases.py measure`):

| | intonation \|dev\| p50 | vibrato p-p (long notes) | start under the peak |
|---|---|---|---|
| violin: integration → now (players) | 1.3 → 5.2 ct (5.1) | 21 → 19.5 ct (22) | 2.8 → 4.9 dB (9.0) |
| viola | 2.8 → 5.5 ct (4.6) | 28 → 22.0 ct (21) | 3.5 → 3.3 dB (8.4) |
| cello | 2.5 → 4.5 ct (7.0) | 31 → 24.1 ct (13; Musopen 25, HSD 10) | 3.5 → 3.8 dB (9.7) |

By this pass's `phrase.py` against HSD (p90 \|dev\| / long notes' start and end under the peak):
violin 8.2 → 9.1 ct, 2.8 / 3.7 → 4.9 / 3.7 dB; viola 7.6 → 12.7, 3.5 / 4.5 → 3.3 / 3.6; cello 6.7 → 12.6,
3.5 / 4.0 → 3.8 / 3.9; bass 7.1 → 10.9, 2.3 / 4.3 → 4.0 / 5.0 (players 13–16 ct, 7–14 / 10–35 dB).
The intonation and the vibrato now sit with the players; the notes' shapes inside a phrase do not
(the gate-bounded starts, the slur's end without look-ahead). The c7 checkpoint's deeper starts
(4.5–6.5 dB ends) came from a bow change's dip that rang the viola's change ticks over its
references and from fresh starts the gate refused.

**The Fiddle character** (`fiddle_lab`, `fiddle_scratch.py`, the final build): the noise between
the harmonics 4–8 kHz −39.2 / −40.4 dB (agudas / rápidas), YIN d' 0.01, raucous share 0.05–0.06,
long notes with vibrato 0.67 / 0.33 of them (the analysis's fiddle: 0.02; the references
0.04–0.30). The classical shape, taper, way back, intonation and corner do not touch it.

## Open

- **A note's end inside a slur** cannot taper before the next note without knowing its length
  (look-ahead on playback; the engine's `processor*.rs` is `tuning`'s this round).
- **The cello's spectrum settles too fast** (`t_steady` 0.12 s against 0.18–0.67): the bow may not
  start low enough on its low strings within Guettler's gate; the tone's own opening (the corner,
  the contact) is the next lever.
- **The loud cello stays dull** in its partials 2–8; the violin's top above 2 kHz and its air above
  10 kHz are 3–13 dB low on the Paganini caprice; a broad 150–400 Hz thump under loud high violin
  notes and a strong low fundamental (the violin's body under its air mode) — the body's low
  radiation is the next thing to look at (not touched here).
- **Too clean**: between the harmonics (HNR bands, 2–4 dB) and the pizzicato above all (its HNR
  22–33 dB over the recordings); the violin's and viola's plucks rise in 2–4 ms against ≥ 4.
- The pitch gate (±1 ct) holds 72–97 % of the held-out notes with the player on.
- Verified only in the lab and by measures: the owner's ears decide.
