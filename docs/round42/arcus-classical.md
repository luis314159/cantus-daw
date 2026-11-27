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
8. **A fiddler** (`shape::fiddler`: the Fiddle character's steel set, `steel` — not `bow.pair`, which
   a classical double stop or a ringing crossing needs with the classical player kept): the
   classical shape, taper, way back, intonation and corner give way to him; instead it **shuffles** (two or three fast separate
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

**Bow on two strings, classical** (the Paganini builder's two findings): the fiddler was first keyed
to `bow.pair`, so a classical double stop lost the classical player and took the shuffle; it is
keyed to the steel set now, and `tests_classical::a_slurred_crossing_on_one_bow_keeps_every_note`
holds a slurred string-crossing passage with the pair on, classical and fiddler, every note within
12 dB of the passage's median (the Var. 2 render's −90 dB notes were not reproduced by this
passage; the Var. 2 render itself is the Paganini builder's to check on this build). The player's
semitone tendency does not yet reach a double stop's second string (`arcus_voice_pair.rs`, the
fiddle builder's file): open.

## Pass 2 (`p4`): the look-ahead, the violin's body, the noise, the fiddler's switch

The owner: *"sigue mejorando para que podamos sonar más natural, como esa grabación de Paganini"*
(he found the last *"sonando mucho mejor"*). ADR 0166's *Pass 2* holds the decision.

1. **The playback's look-ahead** (the engine's: `InstrumentUnit::note_len`,
   `processor::look_ahead`; the tuning builder's `processor*.rs` work was merged and done): each
   clip note's length and the next note's distance from its end, in frames, handed before its
   note-on — read from the snapshot's sorted events through the tempo map (≤ 256 events, no
   allocation, no latency, the same offline and in real time and at any block size).
   `paganini_render` hands the same from its score. Arcus's shape eases a followed note before
   its end (`Knobs::end_*`: 12 dB on a note of 1 s or more, 3 dB at 0.15 s or less, over half its
   length up to 0.6 s; none before a rest — the key's taper carries it), a bow change after it
   starts from half of where it ended, a slurred or re-bowed note's rise fits in 0.4 of its
   length.
2. **The violin's body** (model 2): the air mode's vented-box roll-off (fourth-order
   Butterworth at 260 Hz, `InstrumentSpec::air_hz`), the bridge corner × 2
   (`upper::VIOLIN_CORNER_K`, sharpening with the dynamic from mp), the match's correction
   re-set between the close references and the Paganini hall recording, the output +0.6 dB,
   the upper pitch table measured again (left after its pass ≤ 0.28 ct on the violin).
3. **The noise between the harmonics**: the classical hair noise +5 dB (violin, cello), +3
   (viola) over each match, on the held stroke (faded in over 0.15 s after the attack); a
   fiddler's bow keeps its scratch.
4. **The cello**: the +5 dB of hair noise over the whole stroke moved `t_steady` 3/11 → 5/11
   groups in, but it held two soft attacks past Guettler's limit; on the held stroke only it is
   3/11 again. Its loud partials 2–8 stay dull — see *Open*.
5. **Double stops**: the second string carries its own intonation (`Second::into`: the error and
   the semitone step's tendency from the note that string last held) and keeps it when left
   ringing at a crossing (`swap_roles`).
6. **The fiddler's switch** `player.fiddle` (schema 87, the coordinator's number; 86 is
   Tympanum's model 6 — a placeholder step here): `shape::fiddler` reads it, not the steel set;
   the Fiddle character sets it; a schema 85–86 Arcus with steel migrates to `player.fiddle` = its
   steel. A fiddler's violin keeps the Fiddle character's body (`VIOLIN_FIDDLE_EQ`, corner × 1,
   the second-order corner), so ADR 0164's measured lines keep their numbers.

### How it was measured

- **Phrases**: `arcus_demo solos` through the engine (so with the look-ahead),
  `phrase.py` (HSD) and `strings_hq_phrases.py measure` (Musopen + HSD) — `$CANTUS_WORK/
  arcus-classical/phr.sh <run> <demo.exe>`.
- **Paganini**: the scores of `$CANTUS_WORK/listen/paganini/work` rendered by this branch's
  `paganini_render` (with the look-ahead; `ARCUS_SHAPE`, `ARCUS_EQ` to try), mixed and measured
  by `paganini_mix.py` and `paganini_compare.py` in a work copy (`$CANTUS_WORK/arcus-classical/
  pagtry.sh`, `pagsum.py`: the note shapes by length and bow), and two probes: `py/lowprobe.py`
  (150–400 Hz under the loud high notes re their partials; the low notes' H1 re H2–4; the 1–16 kHz
  third-octaves) and `py/drop.py` (Variation 2's notes more than 30 dB under its median).
- **Single notes**: `lab.sh` on the strings-hq subjects; `py/h1.py` the partials 1–8 by register
  and dynamic per reference set (the lab's band is wide toward the dull side because Iowa 2012's
  violins' partials 2–8 read 15–40 dB under the others').

### Measures, pass 1 (`c11`) → pass 2 (`p4`, the session's final build)

**Phrases** (the solos through the engine, the look-ahead on):

| | violin | viola | cello | bass | players |
|---|---|---|---|---|---|
| long notes' end under the peak (`phrase.py`, HSD) | 3.7 → 12.4 dB | 3.6 → 12.1 | 3.9 → 12.0 | 5.0 → 12.4 | 10.6 · 14.5 · 35.4 · 15.3 |
| long notes' start under the peak (`phrase.py`) | 4.9 → 10.5 dB | 3.3 → 10.5 | 3.8 → 8.5 | 4.0 → 7.2 | 13.6 · 7.3 · 12.8 · 10.7 |
| start under the peak (`strings_hq_phrases`, Musopen + HSD) | 4.9 → 10.5 dB | 3.3 → 10.5 | 3.8 → 8.5 | — | 9.0 · 8.4 · 9.7 |
| intonation \|dev\| p50 (`strings_hq_phrases`) | 5.2 → 4.7 ct | 5.5 → 5.5 | 4.5 → 4.2 | — | 5.1 · 4.6 · 7.0 |

The first try at the knobs' defaults of the code (`end_db` 7, the first `d1` build) gave ends of
8.1–9.5 dB; 12 brings them into the players' range on every instrument and the Paganini long
notes to 12–13 dB (16.6). Deeper (`end_db` 16–20) moved the Paganini long notes little more
(12–17 dB) and lowered the short notes' peaks.

**Paganini** (the aligned take in the hall, one string; `paganini_compare.py` / the probes):

| | original | actual | pass 1 | pass 2 |
|---|---|---|---|---|
| start under the peak (notes ≥ 0.15 s) | 5.3 dB | 7.6 | 6.9 | 8.4 |
| end under the peak (≥ 0.15 s / ≥ 0.3 s) | 9.1 / 16.6 dB | 8.2 / 16.9 | 5.9 / 9.0 | 7.7 / 13.1 |
| rise to −3 dB | 32 ms | 45 | 45 | 48 (33 in `p3`, before the corner followed the dynamic) |
| notes' peaks p5–p95 | 14.0 dB | 16.2 | 17.9 | 17.3 |
| 200 Hz third-octave re the original | 0 | +13.9 | +15.7 | −2.3 |
| 2 / 2.5 / 3.15 kHz | 0 | −6.9 / −2.5 / −3.8 | −7.1 / −2.4 / −4.7 | −3.7 / +1.5 / −3.1 |
| 10 / 12.7 kHz | 0 | −12.7 / −10.8 | −12.7 / −10.4 | −7.4 / −6.0 |
| 150–400 Hz under the loud high notes, re their partials | −39.6 dB | +7.7 | +8.9 | +6.9 (+0.2…+2 in the tries with 250 Hz at +4…+6; five notes) |
| Var. 2 on two strings: notes > 30 dB under the median | 0 | 0 | 9 of 16 bars to −90 dB | 0 of 137 |
| Var. 2: the upper string's fall / balance / interval | 9.7 dB / +6.9 dB / −9.2 ct | 5.0 / +2.5 / −1.5 | 5.2 / +3.1 / +2.6 | 5.6 / +0.6 / +2.1 |

By length (pass 2, new bow): notes under 0.15 s start 6.7 / end 2.3 dB (the recording's 3.5 /
5.5), 0.3–0.6 s 5.8 / 13.1 (5.8 / 16.5), over 0.6 s 13.2 / 16.7 (8.4 / 27.6). A short note's end
does not follow the bow's contour at that length (10 dB planned over 66 ms measured 2.9): the bow
speed's 30 ms and the force's 60 ms smoothing and the string's own response hold it.

**Single notes** (lab, held-out cells, `c11` → `p3`; the notes are rendered without the
look-ahead, so only the body and the noise moved them):

| | violin | viola | cello | bass |
|---|---|---|---|---|
| harmonics 1–20 in band | 78 → 85 % | 82 → 82 % | 88 → 88 % | 88 % |
| HNR bands in band | 61 → 79 % | 45 → 60 % | 63 → 69 % | 67 % |
| noise re harmonics, groups in | 6 → 10/12 | 9 → 9/12 | 6 → 8/12 | 5/12 |
| crest factor | 10 → 10/12 | 11 → 9/12 | 12/12 | 12/12 |
| `t_steady` | 9 → 8/10 (0.31 → 0.38 s) | 4 → 5/12 (0.15 → 0.20 s) | 3/11 (0.12 s; 5/11 while the noise also rode the attack) | 8/12 |
| arco within ±1 ct | 72 → 65 % | 78 → 74 % | 80 → 84 % | 97 % |

The violin's partials by reference set (`partials.py`, ff, re the strongest): G3–C♯4 H1 −6.8 →
−9.6 dB (Iowa −17.8, TinySOL −13.1, VSCO −9.3); A4–D♭5 H3–H5 −21.9 / −23.3 / −24.4 → −19.6 / −20.2
/ −21.5 (Iowa −11.5 / −9.1 / −7.2). The brighter correction tried (`v2b`: 1.6–4 kHz at 0, 10 kHz
+4) put 92 % of the violin's cells in band but the hall recording's 2.5–8 kHz 6–8 dB over the
original: the default sits between them.

**The size of the change** (`LOWER_HASHES`, `PASS6_HASHES`): on the pinned cello and bass renders
only the cello's hair noise moved them (`tests_classical::probe_pass2_size`) — see below.

## Open

- **The loud cello stays dull** in its partials 2–8 on C4–C6 (8–20 dB under Iowa, TinySOL and
  Karoryfer at ff); the corner is not it (the cello's is 6 kHz, its partials there are under
  2 kHz): the string's loss in the upper partials is the next lever. Its `t_steady` 5/11 in.
- **The pizzicato is still too clean** (HNR 40–51 dB, no cell in band on any instrument): a
  finger's noise and the plucked string's aperiodicity are next; the bowed tone's noise is in
  band on 60–73 % of the cells.
- **Var. 2's two strings** (the target 9.7 dB / +6.9 / −9 ct): the upper string still falls too
  little and the interval is wide; the second string now carries its intonation (the spread is
  the player's), but the narrowing a player gives a leading note in a double stop is not modelled.
- **Short notes' ends** inside a fast passage stay 2–3 dB under their peak (the recording's 5).
- **The violin's top**: between the close references (brighter) and the hall recording; the
  viola's body did not get the air mode (no measured viola admittance or hall recording here).
- **The violin's single-note pitch gate**: within ±1 ct 72 → 65 % with pass 2's body (the upper
  pitch table measured again; its own held-out gate passes); the Paganini attack's rise 48 ms
  (33 in `p3`, the recording's 32) since the corner follows the dynamic.
- **Tests that fail on this branch, not from pass 2**: `format::schema_84_keeps_chordas_colour`
  (this branch's `v85.cantus` is the fiddle builder's, without Chorda's colour; integration's
  has it), `arcus_level_e2e` (the Fiddle and violin tremolo presets 2.7–3.1 LU over the cello's
  portamento against `PARITY_LU` 1.5; the Fiddle preset, whose renders pass 2 leaves to the
  bit, alone sits at −18.6 LUFS against the family's −20.6 … −21.5),
  `arcus_fiddle_e2e::the_bite_raises_the_noise…` (bite 0 → 1/3: −27.5 → −27.6 dB; the Fiddle
  preset's renders are bit-identical to pass 1's, `fiddle_lab`).
- Live playing has no look-ahead (a live slur's end cannot ease).
- Verified only in the lab and by measures: the owner's ears decide.
