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

## 2. Stage 1: the quartet and the solos performed with what Arcus could already do

**What changed** (`crates/cantus-ui/examples/arcus_demo/perform.rs`, `solos.rs`; the score's
new marks in `score.rs`, the parts' in `parts.rs`; `tools/arcus/demo.sh`, `… solos`): a player
turns the written part into the performed one with only what a user enters — each note's
articulation and bow, its velocity, start and length, a per-note MPE bend, the clip's CC1 /
CC11 / CC21 / CC65 lanes, the tempo track:

| | how |
|---|---|
| bowing | one bow per slur; a phrase starts down on a strong beat, up on a pickup; bows alternate; a written `d`/`u` wins |
| attacks | a phrase's first note from the string (velocity 40 + 14·d); a bow change in a phrase détaché by its beat's weight (58 + 10·w + 10·d); `>` 96, `m` martelé 108 (a staccato), `f` fp 108 and CC11 to 0.55 in 140 ms, `z` sforzando 116 and CC11 held 150 ms then eased |
| dynamics | CC1 = the written dynamic + the part's place (melody +0.06, inner −0.03, bass −0.02; the viola's fader +2.5 dB) + each phrase's arc (±0.05 melody, ±0.025 others, peaking at 0.4 of its highest note, falling 1.1 × to its cadence), ÷ CC11's rest (0.85); CC11 = each bow's messa di voce (a long bow from 0.55 of its rest up to its top at 0.55 of the bow, back to 0.64 — melody m 0.45, others 0.30), a phrase's last bow dying 60 %, portato's pulse (−28 % over 120 ms) |
| vibrato (CC21) | continuous through a phrase (60 ms glides between notes' depths), a phrase's long first note plain then growing in after 0.12 s (0.22 when soft), none on spiccato, staccato, short détaché and open strings, 0.7 on short slurred notes, ×1.2 at the climax, ×0.85 in the coda, ×1.1 on the melody, narrowing with a dying release |
| timing | every start σ 3 ms (melody) / 4.5 ms, finger changes σ 2 ms, seeded; a phrase's last note lifts 6 % early before a rest; the tempo track eases 3.5 % into a melody's peak and 6 % over a phrase's last beat and a half, away from the written ritardandos |
| intonation | +6 cents on a note leading up a semitone, −5 on one falling a semitone (melody and countermelody), as a per-note bend |
| slides | CC65 around three chosen shifts in the quartet (the theme's sixth in its second statement, the violin's entry, the viola's peak), two in each solo |

**Measured against the HSD players** (`phrase_study.py quartet`, the same estimator; long notes
≥ 1.2 s, medians):

| | pass 6 | stage 1 | HSD |
|---|---|---|---|
| level peak, share of the note | 0.575 | 0.569 | 0.55 |
| start under the peak, dB | 3.3 | 4.8 | 11.9 |
| end under the peak, dB | 2.8 | 3.7 | 14.2 |
| short notes (0.12–0.3 s) with a vibrato | 0.39 | 0.15 | 0.57 |
| balance (dB over the four's mean): Vl, Vla, Vlc, Cb | +3.6, −6.9, +1.6, +0.5 | +3.8, −5.3, +0.9, +0.2 | Vl1 +1.1, Vla −1.2, Vlc +3.5 |

Read plainly: the shapes moved the right way but the swells stayed shallow. Arcus's whole
pp → ff (CC1 0 → 1) spans 9–14 dB (pass 6's swell renders; recorded libraries 13–18 dB over
their dynamic steps), and much of the players' start and end depth is their joins (a détaché's
dip 7–11 dB, §2.10 of the study) and their attacks, which are the instrument's. The short notes'
vibrato went down (none on fast détaché and spiccato by rule) where the HSD players carry it
through half of them: a candidate for the next tuning by ear.

**Checks:** every demo check passes (the same bytes twice and from the saved project, −16 LUFS
under −1 dBTP, no full-scale sample, each instrument audible where it plays and silent where it
does not, no click at a note's boundary, the hall's tail); the plain build in this tree is byte
for byte pass 6's file. Every solo passes its own (the same bytes, levels, boundaries).

## 3. Stage 2: what the instrument lacked (ADR 0162, `DSP_VERSION` 15, schema 83)

`cantus_dsp::arcus::bow` is the one definition of the bow's state and of a shift (the survey's
ask), and `player` (default 1) / `bow.weight` (default 1) its controls; `player` 0 is pass 6's
Arcus to the bit (`upper_tests::the_player_off_keeps_pass_six_s_bits`).

**Built and kept:**

- the bow along the stick (`x`, frog 0 … tip 1), moved by the stroke over the hair (0.57–0.65
  m), a stroke from silence near the frog down-bow (0.15), near the tip up-bow (0.85), in the
  middle in a fast passage (an inter-onset under 0.22 s), or where the note's part says;
- the arm's weight `1 + 0.15·(0.5 − x)` (±7.5 %) on the attack's force of the cello and the bass
  (measured: without it the bass's soft E2 took 288 ms to settle against Guettler's 214; on the
  violin and the viola the soft attacks settle without it, not reliably with it); along a
  sustained stroke the player **makes up 85 %** of the fall (`bow::COMPENSATE`: pronation and the
  index finger keep a held note even — the coordinator's call after the first held-out
  measure, below), ±1.1 % left, `bow.weight` scaling that uncompensated part;
- a stroke from the string: Guettler's measured acceleration with the force placed before the
  motion, then a heavy attack's force easing into the stroke's over 0.12 s (pp) … 0.05 s (ff);
  from the air (the note's choice, or a staccato's): landing at 0.45 of the speed, the force
  1.12 × the stroke's within 5 ms;
- the fingering: first position, a named string honoured where it reaches (up to an octave over
  its open string), a slur on one string by three semitones or more a shift (7 ms of finger per
  semitone past two, at most 110; the slur's dip as long; the vibrato stopped and growing back);
  no vibrato on an open string in a passage, 0.3 of it in a fast détaché, 0.7 in a fast slur;
- the note's `start` (`string`, `air`), `part` (`frog`, `middle`, `tip`, `whole`) and `string`
  (MusicXML's count, 1 the highest) — `SetNoteArt`, the piano roll's note menu, both locales,
  the undo fuzz's arm, fixture v83.

**Built, measured and left off or weaker** (each at the gate it broke):

| tried | measured | kept |
|---|---|---|
| the player changing bow when it runs out (1.2 cm left) | at the model's speeds the bow lasts 2–4 s: the gates' single 3–5 s notes got a change in their windows — the slur's pitch +2.3 cents at 5 s, the bass's vibrato extent jitter 0.57 (players 0.19–0.36) | off (`CHANGE_M` < 0); waits for look-ahead (a note's length) |
| slowing the bow as it runs out (to 0.4–0.8 ×, nearer the bridge) | the bowed pitch, its table measured at the dynamic's speed, moved 2.2–2.3 cents | off (`SLOW_MIN` 1) |
| the arm's weight ±17 % | the violin's open G at pp held out +4.2 cents | ±7.5 % |
| from the air by default at velocity ≥ 0.8 | the violin's and the viola's attacks at 0.95 settled in 150–415 ms (Guettler 90) | air only when asked or for a staccato |
| the landing's force riding its speed | worse: 3 of 80 lower and 21 of 120 upper attacks over | the stroke's force |
| accelerating only to 0.6 of the speed, the speed's contour after | soft attacks on open strings never settled (the viola's D at 0.15: 500 ms) | the classic acceleration |
| the force's contour 0.2 / 0.08 s, both ways | a bass point held out 1.09 cents, the violin's top at ff, a violin G3 124 ms | 0.12 / 0.05 s, a heavy start only |
| an open string's pitch held stopped on the string below; a slur kept on its string | the slur gate +1.4 cents (the pitch tables are per key in first position) | first position; a named string only |

**The size of the change** (`upper_tests::probe_player_size`, the seven renders behind
`LOWER_HASHES`, player off against on, with the compensation): RMS +0.30, −0.06, +0.01, −0.24 dB
(cello) and −1.93, −1.19, −0.06 dB (bass); the difference +1.2, −4.4, −22.4, −12.7 dB (cello) and
−1.0, −6.4, −21.4 dB (bass) re each render — the bowed ones at the signal's level (the
vibrato's and the strokes' shapes move them).

**Gates** (Windows, this branch): `cantus-dsp` `arcus::` 45 passed, 0 failed, 41 ignored probes
— among them the Guettler attack limits (0 of 80 lower, 0 of 120 upper), the bowed pitch (168 of
168, and 96 of 96 upper, within a cent held out), the tremolo in tune, the vibrato's band and
its partials, block sizes, rest, the pool, the player's own seven; `cantus-project` 79 (schema
83's fixture); `cantus-core` the undo fuzz 22 and `note_art` 3; `cantus-engine` `inst_arcus`,
`arcus_image`, `null_tests` 26, `rt_memory` 67, `strings_rec_art`; `cantus-score`'s round trip;
`cantus-ui` `arcus_level_e2e` (every bowed preset within 1.5 LU in music, the fiddle at −18.8),
`instrument_levels`, `arcus_tuning_art_e2e`, `arcus_pick_e2e`, `arcus_upper_e2e`,
`strings_rec_e2e`, `fix2_model` 8, the note menu (`views::piano_roll::art`) and the i18n keys 4.
The final `cantus-dsp` run (after the compensation and the tremolo's exemptions): `arcus::` 45
passed, 0 failed. Not run: the workspace suite (the rule), the Linux check (WSL is the
integrator's), clippy on `cantus-ui` (only `cantus-dsp`'s).

**The held-out third** (arcus-match's protocol: `tools/arcus/pass4/measure.sh` renders the
held-out notes and the study's estimator reads them, `study_compare.py`; before = pass 6's
`after3.json`; the rows in `$CANTUS_WORK/arcus/expr/heldout/p6_vs_s2.md` and `p6_vs_s3.md`):
over the rows with a distance to the real instruments' band,

| | better | worse | the same (±0.05) |
|---|---|---|---|
| cello, the arm's fall not made up (`s2`) | 7 | 13 | 66 |
| bass, the same | 12 | 24 | 50 |
| **cello, 85 % of the fall made up** (`s3`, the default) | 6 | 12 | 68 |
| **bass, the same** | **17** | **15** | 54 |

- The compensation (the coordinator's call: players pronate so a sustained note stays even)
  turned the bass into a net gain — its soft centroid 29.6 → 9.9 Hz outside, its noise bursts
  soft 18.1 → 16.9 and medium 11.5 → 10.7 dB, its medium release better — against its release
  decays (+0.6 … +0.7 dB/s), its loud burst (+0.6 dB) and its vibrato extent (+0.2 … +0.5 cents)
  worse.
- **The cello still regresses, by tenths**: its noise against its harmonics 3.70 → 4.12 dB
  outside (medium 4.95 → 5.35, soft 3.56 → 3.85), the soft burst 3.17 → 3.56, the medium burst 0
  → 1.00, the medium build order 0 → 0.20, the 7–10 kHz body curve 3.79 → 3.98; better: the loud
  HNR 1.29 → 1.13, octave 2's medium harmonics 1.40 → 1.20, the loud noise 1.40 → 1.07, the
  pluck's thump 10.75 → 10.22. These are the attack's rows, and the arm's fall no longer moves
  them: they come from the stroke's start (the frog's weight on the cello's attack, and the
  heavy start's easing), so the next try is that start on the training two thirds (the bass's
  attack gate is what keeps the frog's weight on the lower instruments).
- `t_steady` barely moved (the cello 0.114 → 0.12 s against the players' 0.24–0.42): the contour
  that keeps every gate is too mild to close it (§5, item 2).

By the rule (no row regresses beyond noise) the cello's rows are reported here, not accepted:
`DSP_VERSION` 15 and schema 83 stand for the integrator's and the owner's decision with them in
view.

**In music** (the stage-1 performance re-rendered on the stage-2 instrument, `etapa2/`, the
same phrase study, with the compensation): the long notes start 5.1 dB and end 4.0 dB under their
peak (stage 1: 4.8, 3.7; HSD 11.9, 14.2), the peak at 0.577 of the note (HSD 0.55); the balance
Vl +3.4, Vla −4.9, Vlc +0.1, Cb +0.2 dB over the four's mean (stage 1: +3.8, −5.3, +0.9, +0.2;
HSD Vl1 +1.1, Vla −1.2) — a small step towards the players. Every demo check
passes (the same bytes twice and from the saved project, levels, audibility, boundaries, the
tail), and every solo's.

## 4. The fiddle: country and bluegrass (the owner, 2026-11-25)

"We should also be able to play country on the violin; it sounds very different from the
classical style." The first non-classical **playing character**.

### 4.1 What fiddlers do differently (the study)

No fiddle recording is in `$CANTUS_REFS` (every set there is classical or a single-note
library), so this part rests on the method books and the style's common teaching, not on a
measurement here. The sources: Stacy Phillips's *Complete Country Fiddler* (Mel Bay), Brian
Wicklund's *American Fiddle Method* (Mel Bay), Mark O'Connor's *Fiddle Method*, and the chop
as Darol Anger and Richard Greene teach it (Anger & Eggleston, *Chops & Grooves*, Berklee
Press). Their content is summarised here from the style's common knowledge; the exact pages
were not checked in this session.

- **Tone and setup**: the bow nearer the bridge and heavier, a grittier sound, short strokes
  in the middle and upper half, little or no vibrato (an occasional one at a phrase's end), open
  strings used freely and let ring; a flatter bridge makes double stops easy.
- **Bowing patterns**: the *Nashville shuffle* (here, after the owner's description: four
  eighths as two slurred then two separate — long, short, short), the *Georgia shuffle* (one,
  then two slurred from the off-beat, then one), *saw strokes* (every note its own bow), the
  accent pushed onto the off-beats (the pulse), bow rocking between strings, the *chop* (a
  percussive, scratchy down-stroke at the frog on beats 2 and 4).
- **Left hand**: slides into notes (often from a semitone or a minor third under), grace notes
  and hammer-ons, turns and rolls, bluesy bent and flatted thirds and sevenths, unisons against
  an open string.
- **Tunings**: cross-tunings (AEAE, GDGD).
- **Feel**: straight or lightly swung eighths, a strong rhythmic drive, accents rather than long
  swells.

### 4.2 What was built

- **The character**: the factory preset *Fiddle (country, bluegrass)*
  (`arcus.preset.violin_fiddle`): contact ×0.82 (nearer the bridge), force ×1.15, bow noise +4
  dB, a quicker attack (×0.7), a 70 ms release, 14 ms bow changes and finger, vibrato depth 0.35
  with a 450 ms delay and more variation, the strings ringing 1.8 s, the body a little drier;
  its level in music −18.8 LUFS (`arcus_level_e2e`, every bowed preset within the spread).
- **The fiddler's rules** (`examples/arcus_demo/fiddle.rs`, all entered as a user enters them):
  the Nashville and Georgia shuffles by section and saw strokes by mark, alternating bows (a bow
  after a rest down on a beat); the pulse (off-beat eighths at velocity 92, on-beats 78–84, a
  CC11 kick on every off-beat, a 3 %-of-a-beat lilt) and a few ms of a player; per-note bends
  for the slides (a semitone under over 70 ms, a minor third under over 100), the hammer-on grace
  (a whole step under for 35 ms) and the blue notes (pushed a fifth of a tone up through the
  note); no vibrato but a little and late on a long note before a rest; short strokes in the
  middle of the bow (the note's part), the chop from the air at the frog (the note's start and
  part, a staccato at velocity 127).
- **Double stops and drones**: a second Arcus violin with the same preset bows the open string
  next to the melody's (A under an E-string note, D under an A-string one), re-bowed where the
  melody's bow changes and in its direction: one bow on two strings as far as two instruments
  can say it.
- **The demo** (`arcus_demo -- <out> fiddle`): *Hoedown en re*, an original tune (a kick-off with
  two chops, an eight-bar A part on the Nashville shuffle, an eight-bar B part higher on the
  Georgia shuffle with double stops, the A part again with double stops; 25 bars at 116 BPM),
  with a honky-tonk piano (Chorda, `chorda.preset.honky`) on 2 and 4, a pizzicato bass (Arcus,
  root and fifth on 1 and 3) and a light kit (Tympanum: closed hi-hat eighths, a soft kick, a
  cross-stick on 2 and 4 — Tympanum has no brushes); and the B part's first four bars on the
  violin alone, classical (the cantabile preset and the quartet's player) then fiddle.

- **The second demo** (the owner after the hoedown: "much livelier, more cowboy, with more
  energy"; `fiddle2.rs`, `arcus_demo -- <out> fiddle`): *Tren de la medianoche*, an original
  breakdown in A at 160 BPM, 44 bars (66 s): a chop kick-off, the A part twice (the Nashville
  shuffle, a flatted seventh, a cross-string arpeggio up to E6), the B part twice (the Georgia
  shuffle, slides into the accented notes; the second time **twin fiddles**, a third Arcus violin
  a diatonic third under), a quiet breakdown with the **train whistle** (a double stop sliding up
  from a minor third under — the mark `x`: the second violin a third under, bending with it) and
  chops, a double-time push of saw-stroke sixteenths, a shout tag with the band's hits; the bass
  root and fifth hard on 1 and 3, the piano stabbing 2 and 4 with fills, a train beat (the
  snare's eighths accented on 2 and 4, the kick on 1 and 3). −16 LUFS, −2.9 dBTP, the same bytes
  twice, the fiddle stems' 950 boundaries at worst 3.8 dB over their music.
- **Checks** (`fiddle::render`): each take exported twice to the same bytes, under −1 dBTP, no
  full-scale sample, and its fiddle's boundaries measured on the fiddle's own stems (in the
  band's mix the cross-stick on 2 and 4 lands on the fiddle's starts): the hoedown 58.7 s, worst
  4.2 dB; the two passages 2.4 and 2.5 dB; the breakdown 3.8 dB.

### 4.3 Not built yet: the design and the steps for a fresh builder

Today the double stops and drones are a second Arcus violin re-bowed with the melody's bow.
That is two instruments, two bodies and two players. These five items are in the order to build
them; each keeps the house rules (null at its default, block sizes, offline = real time, the pool,
rest, the undo fuzz and both locales for anything stored).

**1. One bow on two strings in one Arcus** (the most characteristic):

- *The model.* One bow drives two `BowedString`s at two contacts. Its speed is shared; its force
  is split by the bow's tilt (`tilt` 0 … 1: the share on the lower string), as Schelleng's
  two-string bowing and Guettler's double stops describe it.
- *Where it lives.* A voice gets a second string, `string2: Option<BowedString>` (allocated with
  the voice, never on the audio thread). Its gesture runs once (`gesture()`), and the junction is
  solved per string with its own force: `force·(1 − tilt)` and `force·tilt`. Both strings feed the
  same body; each has its own pitch loop and ear (`ear`, the trim), its own table correction, and
  its own string's impedance `z0`.
- *How a user asks for it.* In `Poly` mode with `poly` ≥ 2, two notes that start together (within
  a tick) on adjacent strings — `bow::finger` must place them so — become one voice with two
  strings. A drone needs no new property: a long note under shorter ones on the next string up or
  down. The note's `string` (schema 83) may say which.
- *The tilt.* It follows each note's CC (or the velocity's balance) with a default of 0.5. The
  "bow rocking" of item 4 moves it.
- *Measure.* The two-string junction against the single string at `tilt` 0 / 1 must give the
  same bits as one string alone (the null). Then the double stop's level against the two notes
  bowed separately, and its pitch: each string within a cent, held out.
- *Demos.* Re-render `fiddle`/`fiddle2` with the double stops in the fiddle's own clip and the
  second Arcus removed.

**2. The chop's scratch past Schelleng's limit:**

- *The technique.* `Articulation::Chop`, appended to the open list (the schema gets a migration
  that writes nothing; every exhaustive match is extended: the roll's mark, MusicXML writes
  nothing, the MCP enum, DAWproject's count, Exemplar maps it to staccato).
- *The DSP.* `NoteTech::Chop`: from the air at the frog (`bow::start_x(Down, Some(Frog))`). The
  force lands in 3 ms at 2.5 × `f_max` (past Schelleng's upper limit, so the motion is raucous by
  design) while the speed rises to only 0.3 × the stroke's. Then the bow stops on the string
  (`Phase::Stop`) at 60 ms and the hair noise rises 12 dB during the contact.
- *Measure.* The chop's noise share (the 2–8 kHz band against its harmonics) over its 60 ms
  against Darol Anger's and Casey Driessen's published chop examples, if a free recording is
  found; otherwise by ear. The Guettler gate excludes `Chop`, like a tremolo's strokes.

**3. Cross-tunings (AEAE, GDGD, and the rest):**

- *The setting.* A per-instrument parameter `tuning.strings` (stepped: standard, AEAE, GDGD, ADAE,
  calico — AEAC#; open strings in semitones from standard per string). It is set and not
  automated, like `model`.
- *The DSP.* `InstrumentSpec::strings` stays the table. A voice reads each string's open key as
  `open + offset[s]`; its tension follows by `T ∝ f²` (so `z0` and the bowing map move with it).
  `bow::finger` sees the new open keys.
- *The pitch.* The bowed-pitch tables are per key, so they hold. The open strings' own offsets
  are measured once per tuning (`pitch-table.sh` at those keys).
- *Measure.* Each tuning's open strings within a cent; the null test (standard to the bit).

**4. Bow rocking** (a fiddler's string crossings on every note, the shuffle's drone pulsing):

- *Needs item 1.* With two strings under one bow, rocking is the tilt moving between them at the
  stroke rate: a per-note `tilt` curve (an MPE-like expression on the note, or a CC), and a player
  rule in the fiddle character that moves the tilt across on each re-bow of a drone pattern.

**5. Unisons against an open string** (a stopped note on the string below beside the open string
at the same pitch, the characteristic shimmer):

- *Needs item 1 and the fingering's string choice.* `bow::finger` puts the stopped note on the
  string below (with item 5's tables per string), and the open string sounds with it under one
  bow. A note property `unison: true` (or the open id `tech.unison` in round 43's profile) asks
  for it. The beating comes from the two strings' own detuning: the stopped one carries the
  vibrato, the open one does not.

Also not built: turns and rolls as techniques (the tunes write them as notes), the swung feel
as a groove (the lilt is in the notes), and a measurement of fiddling. Fiddle recordings under
CC0 or CC BY are still to be found (the Library of Congress's field recordings of old-time
fiddlers are public-domain candidates) and measured as the HSD stems were.

## 5. Next (for the next builder)

In the survey's order (`research/instruments-next/strings-realism-survey.md` §7, §11), each on
`bow`'s state and its definition of a shift:

1. **The blocker: the bowed-pitch tables measured again under the player** (`pitch-table.sh`,
   `upper-pitch.sh`, `trem-table.sh` with `player` 1, then per string). Every change below moved
   the bowed pitch past the 1-cent gate because the tables were measured around pass 6's
   classic attack, speed and first position.
2. **The stroke's contour, longer** (item 3; the open `t_steady`, 0.12 s against the players'
   0.24–0.42): with the tables re-measured, a 0.2–0.3 s force (and speed) contour from the string
   (`bow::SETTLE_F` at 0.2 / 0.08 broke the pitch gate by 0.09 cents and one violin attack), the
   Guettler region measured again with it, then `t_steady` and the noise burst held out.
3. **The bow change when the bow runs out, with look-ahead on playback** (item 7): the engine
   hands a note's length (and the next note's key and start) with its art — the snapshot knows
   both. With it the player *plans* each bow (`bow::CHANGE_M`, `bow::SLOW_MIN`, built and
   measured, off): its speed and where it changes, so a held note lasts in one bow when it can
   and changes where a player would; slurs and shifts start early, the vibrato follows the
   note's length and stops before a shift. Live input keeps 0 ms.
4. **The fingering engine** (item 2): with the tables per string (Iowa's per-string runs),
   `bow::finger`'s hooks (`expressive`, the slur's string) turned on — an open string's pitch held
   stopped below with its vibrato, a slur kept on its string, a phrase solved for its shifts — and
   the open string's longer ring; sul G as the mark it already is.
5. **Transitions by interval** (item 1) and **the portamento's kinds** (item 10): on `glide_to`,
   by the shift and the string the fingering gives.
6. **Playing characters** (item 16): a character as a bundle of the player's numbers (contact,
   force, speed, vibrato's depth, rate, onset, the portamento's tendency, the bowing patterns);
   the fiddle preset and `fiddle.rs`'s rules are the first, to become one morphable setting.
   **A seeded identity per player** (item 6): an intonation tendency, vibrato habits, a bow
   drift, a body a little altered — so a quartet's two violins are two players.
7. **One bow on two strings** (double stops, the fiddle's drones), **the chop** as a technique,
   **cross-tunings**.
8. Stage 1's short-note vibrato (by ear) and the swells' depth through the joins (a détaché's
   dip, item 1).

## 6. For the owner's ears

`F:\projects\cantus-work\listenrcus-expr\escuchar.html` (Spanish): the quartet as pass 6
left it, performed on the pass-6 instrument (stage 1) and on the stage-2 instrument; the solos
plain, stage 1 and stage 2; the fiddle's hoedown and the passage classical against country.
`chat\` holds 16-bit WAV copies of each (under 30 MB: the quartets at 44.1 kHz). The full takes:
`pase6\`, `etapa1\` and `etapa2\` (each quartet's 24-bit export, stems, `.cantus`,
`index.html`), `solos1\`, `solos2\`, `fiddle\` (the hoedown, the breakdown `tren.wav`, the
two passages, each with its project and stems).
