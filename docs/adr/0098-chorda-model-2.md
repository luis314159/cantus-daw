# ADR 0098 — Chorda's second model: voiced against a reference piano

**Status:** Accepted · **Date:** 2026-11-04 (amended 2026-11-14: candidate 16 ships, `DSP_VERSION` 7; 2026-11-18: the technician's pass, `DSP_VERSION` 10) · **Extends:** ADR 0059 (Chorda), ADR 0023 (the
family's rules), ADR 0042 (CC BY content: the reference) · **Schema:** 54

## Context

Chorda I (ADR 0059) passes every number it was built to: partials within half a cent, T60
within −4 %/+1 % of Bensa et al., a continuous half pedal, 64 pedal-down voices in a fifth of
a core. The owner's ear says no: *"no suena bien la verdad"* (2026-10-16), and in round 37
*"sintético, muy sintético, muy falso, nada real, sin cuerpo, muy fino"* — synthetic, fake,
no body, very thin. Measured physics is not yet a beautiful piano.

Round 37 measured it, for the first time, against a recorded grand: the **Salamander Grand
Piano** (a Yamaha C5, Alexander Holm, CC BY 3.0), installed through Cantus's own pack code
and played by Exemplar exactly as *Sound packs…* installs it (all 16 velocity layers). The
tool is `cantus-ui/examples/chorda_compare` (`chorda_kit/`): every phrase rendered through
the real engine, then measured note by note (A0 … C8 every minor third at pp/mf/ff) and band
by band — the third-octave spectrum of the first 50 ms and at 0.5 s (level-free), the knock
(the share of the first 60 ms below 0.7 f₀), the rise, the decay per octave band and early /
late, the stereo image, the release — and phrase by phrase (loudness, long-term spectrum,
correlation). What it found in model 1 is in §*Measured* below; in one line: **too little
below 250 Hz (−8 … −25 dB), far too much above 1 kHz (+9 … +35 dB), almost mono
(correlation 0.7–0.9 against 0.2–0.5), no knock (−45 dB against −21 dB in the middle), an
attack that rises in a millisecond (13 ms on the recording), a middle and treble that die
twice as fast in the first half second**. Thin, synthetic and dry — the owner's words, in
numbers.

## Decisions

### 1. A second model, never a change to the first

`model` (ADR 0059 §7) is 2 for a new instance (`MODEL = 2`, stored by `NativeSpec::new`).
**Schema 54's migration writes `model = 1` into every Chorda saved before** that lacks it
(every fixture since v28 already says it; the migration covers a file that does not), so an
old song plays the first model. Model 1's code path is untouched: every new branch is taken
only when `model ≥ 2`, the new voice fields are neutral (0 or 1) in model 1, and two pinned
hashes prove it — a DSP phrase through every part of model 1 (`tests_hash`) and an old
song's Chorda opened through the app (`chorda_model2_e2e`), both computed on the contract
commit before model 2 existed. `DSP_VERSION` stays 5; no golden moves. `model` is set, never
automated or modulated (`SET_KEYS`): the engine reads no lane or route on it. An old song's
piano moves to model 2 by choice, as one undo step, when one of Chorda's presets is loaded on
it (a preset starts from a new instance).

### 2. What model 2 changes, and why (each a measured gap)

- **The body** (`chorda_body.rs`, `chorda_conv.rs`): instead of model 1's 40 modes, four-line
  network and radiation rise, the bridge force reaches the listener through **two radiation
  impulse responses** — the soundboard, rim and lid at two microphones over the strings — by
  a zero-latency uniformly partitioned convolution (64-sample partitions anchored to the
  stream's grid, the first partition in direct form: any block split gives the same bits).
  Each response is synthesised once, deterministically: a board's dense, irregular modes
  (density, loss and radiation after Conklin 1996 and Berthaut et al. 2003), Rayleigh
  amplitudes and phases hashed per microphone (two microphones decorrelate as real ones
  do), a radiation envelope, and a direct path beside it. It is a filter after the strings,
  never a source of their coefficients (ADR 0059's patent rule holds). It gives the body
  (the low-mid energy), the woody colour, the stereo and the slower rise.
- **The room** (`chorda_room.rs`): early reflections from a tapped line (a pattern per side)
  and an eight-line Householder FDN with a decay per line at low and high frequencies (Jot
  & Chaigne 1991); a new `room` setting is its level (default 45 %). Its states are flushed
  to zero on the grid once its input has been silent longer than its longest path, so the
  instrument still reaches an exact rest.
- **The action** (`chorda_noise.rs`, `chorda_voice.rs`): the hammer's blow (a share of the
  felt's force carried into the frame), the key bottoming on the keybed (two damped modes
  and a burst of felt-filtered noise, growing with the key's speed) and the damper landing
  on release — each a force at the bridge's side, so the body turns it into the board's
  knock. A new `noise` setting scales them (default 100 %).
- **No two strikes alike**: each strike's speed and felt stiffness vary a little, seeded
  from the key and the note's id (never random, never the unit's clock: the same note plays the same on every play, live or offline, from any start — ADR 0023 rule 1;
  offline equals real time).
- **The voicing** (`chorda_voicing.rs`): the felt, the decay by register and its
  frequency-dependent part, the action's levels, the body's and the room's numbers — one
  struct, its default what ships, set by a coordinate search of `chorda_compare`'s score
  against the Salamander (§*Measured*), then checked phrase by phrase.

### 3. Levels: staged with the family, an output ceiling for fortissimo

ADR 0101 stages every instrument at −18.7 ± 4.5 LUFS on a C-major triad at velocity 100
for two seconds (model 1 measured −30.1). A piano's chords peak some 20 dB over their
loudness (the Salamander's fortissimo chord: −21.8 LUFS, −2.4 dBFS at the pack's own gain),
and a triad decays where a synthesiser holds: at that level a fortissimo chord passes full
scale. Model 2 is staged inside the window anyway, and gets an **output ceiling**: unity up
to −4.4 dBFS, a tanh from there toward −0.72 dBFS (memoryless: any block size gives the same
bits). Playing at mezzo-forte never reaches it; fortissimo chords' attacks do — both hands
fortissimo with the pedal down (the engine's headroom test) are bent by a few dB. Each
factory preset carries a gain so it sits within ±2 LU of the keys role's target on the
preset library's phrase (ADR 0101 §2). **The owner's ears judge the ceiling** on loud
playing; a song that needs it gone turns the gain down (the ceiling acts on the output
after the gain).

### 4. The owner's ABX kit

`cargo run --release -p cantus-ui --example chorda_abx -- <dir>` renders the seven listening
phrases (middle C, soft chords, a fast run, a pedalled ballad, a ff chord, bass notes alone,
the top octave) through the real engine for the Salamander, model 1 and model 2, matches
their loudness (BS.1770, one target per phrase, lowered together under a −1 dBFS peak),
writes them as 24-bit WAV and writes **one offline HTML page**: two ABX tests of 16 trials
(X drawn with `crypto.getRandomValues` each trial, A and B in a random order, switching keeps
the place), the score with its one-sided binomial p-value, 14 blind preference questions
(a sign test), answers kept in the browser and offered as JSON, README in English and
Spanish. Only the owner's result closes Chorda (`docs/beta-checklist.md`).

**Kit 5 (round 40, `docs/round40/abx5.md`) is made to be given to other listeners:** file names
and piano tokens drawn at random per kit (kit 4's salted hash left a phrase's files sharing 15
of 16 digits), the answer table veiled, a short form about the listener, one ABX (the candidate
against the Salamander) and every preference pair with two unannounced catch questions, the
second ABX optional, the reveal and the credits only at the end, neutral wording in Spanish and
English, Web Audio switching over http, and the published page's per-listener documents with an
owner-only summary. `chorda_abx <dir> --candidate <spec> [--partials <csv>] [--levels <csv>]`;
every control is checked in Edge, Chrome and Firefox (`chorda_kit/abx_browser_check.mjs`).

## Measured

Every number from `chorda_compare` through the real engine at 48 kHz, against the Salamander
(the full tables, per register and dynamic, and the plots: `docs/round37/piano/compare/`).

| | Salamander | Model 1 | Model 2 |
|---|---|---|---|
| Score (shapes + decay + knock + brightness + evenness + stereo; lower is closer) | — | 64.4 | 33.1 |
| Shape distance, first 50 ms / at 0.5 s (RMS dB, every note) | — | 21.5 / 29.6 | 9.4 / 13.5 |
| **Body** — bass mf, 125 Hz octave at 0.5 s (dB of the note) | −2.8 | −14.7 | −9.0 |
| middle mf, 125 / 250 Hz octaves at 0.5 s | −34 / −25 | −60 / −50 | −44 / −27 |
| middle mf, centroid at 0.5 s | 526 Hz | 998 Hz | 692 Hz |
| ballad: long-term spectrum against the reference, 63 / 125 / 1k / 4k / 8k Hz octaves | 0 | −7 / −9 / +11 / +11 / +26 dB | −8 / −7 / +2 / −5 / +6 dB |
| ballad: centroid | 289 Hz | 748 Hz | 330 Hz |
| **Synthetic** — middle mf, knock (share below 0.7 f₀ in 60 ms) | −22 dB | −45 dB | −19 dB |
| middle mf, rise 10–90 % | 16 ms | 1 ms | 20 ms |
| middle mf, centroid of the first 50 ms | 520 Hz | 1 170 Hz | 797 Hz |
| middle mf, decay 0.1–0.6 s / 1.5 s on | 22 / 5.0 dB/s | 39 / 6.5 | 25 / 4.3 |
| treble mf, decay 0.1–0.6 s | 34 dB/s | 70 | 52 |
| two strikes of C4 at one velocity | (a recording: two takes differ) | identical | differ by 0.09 dB and in detail |
| **Space** — middle mf, stereo correlation / side under mid | 0.10 / −1.1 dB | 0.83 / −9.5 | 0.15 / −1.5 |
| ballad, correlation | 0.30 | 0.71 | 0.18 |
| 64 pedal-down voices, 256 frames, one thread, release (this PC) | — | 19.2 % of a core | 23.0 % |

What is still apart (the next voicing's aim, and the owner's ears' to judge): the bass's
weight below 125 Hz (−9 against −3 dB at the bass's 125 Hz octave), the bass's early decay
(10 dB/s on the recording, 4.2 here), the bass's upper partials (2–4 kHz, −13 dB in the
bass phrase), the
treble's early decay (52 against 34 dB/s) and its release with no damper, the treble's
brightness growing faster with velocity (×2.0 pp → ff against ×1.5).

## Consequences

- New Chorda tracks sound as model 2; old songs keep model 1, to the bit.
- Model 2 costs more (two convolutions and a room after the voices): 23.0 % of a core for
  64 pedal-down voices against model 1's 19.2 %. Its quiet floor is 1e-4 N at the bridge
  (about −115 dBFS at the staged output) where model 1's is 1e-10: its decays are up to four times longer,
  and the family's contract asks for rest within 30 s.
- The reference is one recorded piano: voicing against it moves Chorda toward a Yamaha C5
  recorded close in a room, not toward "a piano" in general. The owner's ears, and the
  blind test of ADR 0059, judge the rest.
- The Salamander is CC BY 3.0: it is downloaded on demand (ADR 0042) and never enters the
  repository; the comparison's renders stay in the builder's downloads folder.

## What the tests hold

| What | Test |
|---|---|
| Model 1 never moves | `cantus-dsp` `chorda::tests_hash::model_1_renders_to_the_bits_it_always_did`; `cantus-ui` `chorda_model2_e2e` (an old song through the app) |
| Old files open as model 1; new is model 2 | `cantus-project/tests/chorda_model.rs` |
| Model 2's family rules: silence, any block size, threads, exact rest | `chorda::tests_model2::*` |
| No two strikes alike in model 2; identical in model 1 | `tests_model2::model_2s_strikes_vary_and_model_1s_do_not` |
| The convolver is a convolution | `tests_model2::the_convolver_is_a_convolution` |
| 64 pedal-down voices under 25 % of a core | `tests_model2::measure_model_2s_sixty_four_pedal_down_voices` (ignored, release) |
| A new Chorda plays model 2 live through the app = offline | `cantus-ui/tests/chorda_model2_e2e.rs` |

## Amended by the review of the first delivery (round 37)

- **Strikes are seeded from the note, never the unit's clock.** Each strike's variation and
  its noises come from the key and the note's id (a clip note's own): the same song plays the
  same bits live after any idle time and offline, and a range render (an export of a range,
  a freeze, a bounce) equals the same span of the whole. Tests:
  `tests_model2::an_idle_unit_plays_as_a_new_one`, the engine's
  `played_after_idling_it_equals_the_offline_render` and
  `a_range_render_equals_the_same_span_of_the_whole`.
- **A model switched on a ringing unit leaves nothing behind**: the side it leaves is
  cleared (no allocation) and both sides keep the stream's grid while the other plays, so
  the unit rests and a strike after the switches equals a new unit's
  (`switching_the_model_mid_tail_leaves_no_frozen_state`).
- **The body's length is at most 48 kHz's frames** (0.3 s at 48 kHz, 0.15 s at 96 kHz): its
  cost per sample is the same at every rate. 64 pedal-down voices, release: 48 kHz model 1
  19.1 % / model 2 23.3 %; 96 kHz 39.8 % / 51.9 %; 192 kHz 78.3 % / 98.0 % of a core — model
  2 costs about a quarter more than model 1 at every rate, as the voices' own cost doubles
  with the rate (the family's 25 % bound is met at 48 kHz by both, at higher rates by
  neither).
- The designer makes the body's responses before the strings' design; a preset that moves an
  old song's Chorda to model 2 says so in the status line (`chorda.now_model2`); `room` and
  `noise` are named *(model 2)* wherever they are listed; the ABX kit's files have opaque
  names, salted per kit.

## Amended by the owner's first ABX: voicing pass two and the perspective (round 37)

The owner's ABX of the first delivery (2026-11-05): ABX 1 (model 2 / Salamander) 15/16, ABX
2 (model 1 / model 2) 15/16, p = 0.0003 each; preferred model 2 over model 1 in 7 of 7 and
the Salamander over model 2 in 7 of 7 — *"muchos medios, algo borroso, un poco nasal por los
medios; el Salamander suena más fuerte y vivo, más bonitos graves y agudos"*. And his idea:
*"¿será configurable — más de cerca, más sala? Si es generado, aprovecharlo."*

### 5. The body's bands are even, its microphones coherent where a pair is

The first body drew each soundboard mode's amplitude (Rayleigh) and phase per microphone at
random. With modes 14–22 Hz apart and a few in a third-octave, one draw left some bands 4–6
dB over or under the radiation (a honk at 1 kHz and 1.6 kHz on every note — the same
response for all of them), and some bands' two sides anti-phase (the mono sum −3 to −4 dB at
0.8, 1.26 and 3.2 kHz: a comb on loudspeakers, nasal and without presence). Now
(`BodyVoicing::even`, `coh_hz`, `eq`): each third-octave's modes carry, per microphone, the
radiation's energy; each band's zero-lag correlation between the sides is held within ±0.1 of
a spaced pair's coherence (`1 / (1 + (f / coh_hz)²)`, 200 Hz: one centre below, two
microphones above) by turning the most contrary modes over; and an octave EQ (63 Hz … 8 kHz)
voices the board against the reference's long-term spectrum. The response is still
synthesised once, deterministically, off the audio thread.

### 6. Perspective is a control (`cantus_dsp::chorda::stage`)

Model 2's microphones, the pair's width, the lid and the room's size are parameters — `persp`
(0 over the hammers … 35 % the player … 1 the audience), `width` (model 1's key spread; in model 2
only the mid/side width of the board and the room — the keys keep their voiced places), `lid` (closed … open), `roomsize` —
each a gain or a shelf between the body and the output that glides (20 ms, the room's
decay coefficients 50 ms) sample by sample: no zipper, any block size, the same bits. Toward
the hammers the direct path rises (24 dB a unit), toward the audience the board's ring (3 dB)
and the room (20 dB) rise and the air takes the top (a first-order shelf from 2.5 kHz,
−10 dB asked at the far end — about −6 dB at 5–10 kHz; +2.5 dB at the hammers); the action's
noises are louder close (8 dB a unit); the whole stage's level is trimmed with the
perspective (−3 dB at the audience, +1.8 dB at the hammers: measured on the ballad and
middle C, the far end was 3 dB louder and the near end 2 dB softer before), so turning the
mics does not turn the volume. The lid:
a shelf from 1.2 kHz to −12 dB closed, the direct path −6 dB, the board −1.5 dB. The room's
size scales its decay times by 2^(3 (size − 0.4)). Width is a mid/side gain, so no setting
changes the mono sum of the board and the room — none can comb-filter it. At the defaults
every gain is exactly 1 and every shelf off: the stage passes the voiced chain bit for bit
(a stored 0.35 reads back as 0.349999994 and is still the default). Model 1 ignores them.

## Amended by voicing pass three: candidate 16 ships (round 40, `DSP_VERSION` 7)

Round 40 built voicing pass three behind knobs neutral at beta.14's values (sixteen
candidates, `docs/round40/chorda.md`). Candidate 16 meets the owner's bar **in measures**
(every key within 10 cents of the Salamander, the last soft chord's D3–A3 fifth 1.18 Hz
against 0.68, round 38's phrase metric 4.49); its blind ABX is published for him. The owner,
2026-11-14: *"Ship to master with Chorda's newest version (if it's the best) — but keep in
the backlog listening and doing the ABX evaluation."* Candidate 16 is the best of the sixteen
by every measure, so it ships.

### 7. The shipped voicing is candidate 16, named and pinned

- `Voicing::default()` is **candidate 16**: `chorda::voicing::c16::SPEC` (the voicing lab's
  spec, verbatim, applied key by key over beta.14's voicing as the lab applies it) with its
  two tables (`c16::PARTIALS`, `c16::LEVELS`), made once. `BodyVoicing::default()` and
  `RoomVoicing::default()` are candidate 16's board and room, so the board responses made
  ahead on the designer's worker (`body::warm`) are the shipped ones. **Beta.14's voicing is
  `Voicing::beta14()`** (with `BodyVoicing::beta14()`, `RoomVoicing::beta14()`): the voicing
  lab reads every spec over it, as every candidate was made, so a candidate's files keep
  their meaning; the listening kit's *Chorda 2 (beta.14)* plays it.
- A new unit builds every part from the shipped voicing as `set_voicing` would (candidate
  16's bass high-pass is fourth order, on the sympathetic strings too), to the bit.
- **The evening is weighed at the voiced perspective.** A key's evening (the voicer's
  needling: `even`) was weighed with the stage's live gains; with candidate 16's evening on,
  moving *Width* re-levelled the outer keys (their mono sum moved by 6.6 % of its peak — §6's
  promise broken). The evening is now the stage's defaults' (every gain 1): the microphones
  never re-level a key. At the defaults this is the same number, so candidate 16's default
  sound did not move (its pin held across the change).
- **A voice's end never depends on where a piece ends.** With the arrival-time difference
  on (candidate 16's `itd_ms` 0.65), a voice that ended inside a long piece let its later
  side's last samples (up to a millisecond, under −115 dBFS) drain, and in a short one
  dropped them: blocks of 1 and 7 differed from 512 (the contract's split test and the
  long phrase below). The voice says where it ended (`Voice::ended`) and its later side is
  dropped from there in every piece.
- A model switched to 1 clears model 2's output roll-off and the sympathetic strings'
  high-pass too (both on since candidate 16; beta.14 had both off, so the review's test never
  saw them frozen).
- **Songs saved before** play candidate 16 if they use model 2, as every `DSP_VERSION` bump
  before it moved the songs it touched (2 to 6; render and freeze caches key on the version,
  so a frozen Chorda track is rendered again); songs on model 1 are untouched, to the bit
  (§1: schema 54 wrote `model = 1` into every older Chorda). No schema bump: nothing in the
  file changes. The 24 presets are unchanged: each still starts from a new instance, so each
  plays candidate 16 with its own controls.

The size of the change (`DSP_VERSION` 6 → 7), measured:

| Render | max \|Δ\| | RMS of Δ | |
|---|---|---|---|
| Every golden hash (`null_tests` `v5`, Pulsus's three kits, the scene bakes, the null corpus) | 0 | 0 | none includes model 2 of Chorda: no golden moved |
| Model 1 (`tests_hash`, an old song through the app) | 0 | 0 | pinned, held |
| Model 2 at beta.14's voicing (`MODEL_2_BETA14`) | 0 | 0 | pinned, held (now by name) |
| Model 2 at its default (the pinned 0.5 s phrase: bass, a loud chord, both pedals, a re-strike, the top) | 1.35 | 0.365 (−8.8 dBFS) | another voicing: the phrase's RMS 0.369 → 0.187 (−5.9 dB), its peak 0.920 → 0.959; new pin `0x717d_64b7_22b9_e71b` |

### What the tests hold (third amendment)

| What | Test |
|---|---|
| Beta.14's voicing renders the contract's bits, named | `chorda::tests_voicing3::model_2_at_beta14s_voicing_renders_to_the_bits_it_did` (`0x663b_f285_d17a_ebdd`) |
| The shipped voicing is candidate 16's files, and a new unit plays it as `set_voicing` of it | `tests_voicing3::the_shipped_voicing_is_candidate_16_and_a_new_unit_plays_it` |
| Model 2 as shipped, pinned | `tests_voicing3::model_2_at_its_shipped_voicing_renders_to_the_bits_it_does` (`0x717d_64b7_22b9_e71b`) |
| Width never moves the outer keys' mono sum, at candidate 16 | `chorda::tests_stage::model_2s_width_never_moves_the_mono_sum_of_the_outer_keys` |
| The perspective moves the room's share (its return against the dry sound) | `tests_stage::the_perspective_moves_from_the_hammers_to_the_audience` |
| A model switched mid-tail leaves nothing frozen, at candidate 16 | `tests_model2::switching_the_model_mid_tail_leaves_no_frozen_state` |
| Any block size over a long random phrase, at candidate 16 | `tests_voicing3::the_shipped_voicing_keeps_every_block_size_over_a_long_phrase`; the engine's `inst_contract` `split::chorda` |
| Its fortissimo meets the ceiling's top, never full scale | `cantus-engine` `inst_chorda::its_levels_leave_headroom_and_a_middle_c_is_heard` |
| A new Chorda through the app plays the lab's candidate 16 | `cantus-ui` `chorda_model2_e2e` (the round-40 phrases against the lab's renders) |

The owner's ear has not judged candidate 16: his blind ABX against the Salamander is in
`docs/BACKLOG-excellence.md` (§ Chorda), and Chorda's sound is not called done before it.

## Amended by the technician's pass: every key in tune (round 40, `DSP_VERSION` 10)

The owner, 2026-11-18: *"En Chorda hay notas desafinadas, detecté Bb2, B2, C3 por lo menos …
checa si no hay más notas desafinadas también."* Every key A0–C8 was measured as a tuner
hears it (`chorda::tunekit`: each partial's power-weighted frequency over 0.1–1.1 s, the
key's pitch its partials weighed by level and the ear's dominance region, each partial's beat
the 3–97 % swing of its level about its decay) against the smooth curve model 2's keyboard
is designed to. Candidate 16 had Bb2 13.3 cents flat, B0 17.9 sharp, ten more keys 4–10 cents
off (E1, A1, A#1, C#2, D2, C3, F#3, G3, C#4, A0), fundamentals up to 4.7 cents off their own
ladders (G2, G#1, F2, B2) and eighteen unisons beating 20–28 dB deep. The Salamander
Grand's beat 12–19 dB. Single strings were exact (within 0.5 cents): the faults were in how a
key's strings sound together. `docs/round40/chorda-tune.md` has the measurements.

### 8. The causes, and what fixes each

1. **The strings' stiffness scatter was not centred on the key** (`uni_b`, 2× in the bass):
   each string's dispersion is drawn apart from its neighbours', and the draw's mean moved
   the whole key's upper partials one way — the ear's pitch of Bb2 −13, B0 +18, A#1 −9.5.
   `uni_center` 1: the strings' detunes (fan and skew) and their stiffness scatter each lose
   their mean, weighed by the strings' shares of the blow (their power); the scatter is held
   as one set (its fallbacks scale every string's alike). The strings still beat against
   each other as drawn; the key sits on the design. A key of one string keeps its design.
2. **The bridge's resonances pulled the strings' in-phase mode** (`bridge`, six band-pass
   modes per key at Q 30): a mode near a partial adds phase to the reflection the strings
   moving together meet (`−(1 − nY)/(1 + nY)`), so that partial moved while the strings'
   opposed modes, which never move the bridge, stayed — the key off its design and its two
   kinds of mode beating each other (G2's fundamental −4.7 cents, 19 dB deep). `bridge.tune`
   1: from the admittance at each of the key's first partials, each mode's pull is computed
   (`KeyStrings::pull_cents`, `Δω = φ/τ`); the key's modes are scaled down until none pulls
   one of the first three partials more than `bridge.pull` (2 cents — most keys keep their
   modes whole, about fifteen are scaled to 0.35–0.9), and every string's fraction gives back the
   pull the ear hears (the partials' pulls weighed as the ear weighs them). A tuner sets
   strings on the bridge they rest on.
3. **A loop's fraction met its clamp** (F4, B4, E5, F5, A5, C#7): the design had left the
   fraction at +0.95, a cent from +0.99, so the unison's sharp string was held flat.
   `keyboard.frac_room` 8: every loop keeps 8 cents of detune each way
   (`design::design_with_room` moves a sample between the delay and the fraction and refines
   the design, its partials as good as before); in `Tuning`, so in the designer's key.
4. **What stays after the three, measured, is set as a tuner sets it.** The board's high
   band and the room take a share of a decaying partial's measured pitch (with the body and
   room off the treble's offsets are nought), and the strings' scatter leaves a cent here
   and there. `Voicing::tune` (`chorda::voicing::tune::TUNE`, A0–E6; −1.2…+1.1 cents) moves
   each key's whole ladder on its keyboard design (`Tuning::tune`, in the designer's key:
   the sympathetic strings move with it). It was measured with the suite's kit and iterated
   (secant steps) until every key A0–E6 sat within 0.1 cent; a fraction per voice could not
   carry it (near −0.95 a fraction moves the fundamental twice as far as the upper partials). Above E6 no table: a
   key's own sympathetic string rings with it undamped and its measured pitch is the board's
   tail's as much as the strings' (±2.2 cents, under the ear's discrimination there).
5. **Sixteen unisons redrawn** (`draw.<key>`, the candidate-13 mechanism): C1, Bb2, D#3,
   F#3, G#3, A#3, C#4, D#4, F4, E5, F#5, E6, B6, E7, F7, A#7. Each takes a draw whose
   deepest beat is under its register's ceiling, whose ladder is straight (partials 1–6
   within 2 cents of its pitch), and whose level stays candidate 16's (within 1 dB over the
   first half second, 2 and 3 dB later). F4 and F7 could not keep their later levels, because
   their old draws' deep beats were troughs. Bb2's draw 3 had left its ladder 5 cents bent.

Each is a voicing knob, neutral at 0 in `Voicing::beta14()` and `Voicing::candidate16()`:
both render to the bits they did (pinned), model 1 too. c16's timbre, levels and bass weight
are untouched (no level, partial or body number moved). What changed is the tuning, the bridge
modes of about fifteen keys and sixteen unisons' draws. Measured on the lab's keys at
mezzo-forte, each key's level over its first half second moved 0.27 dB rms, none more than
1.5 dB. Over 1.25–3.4 s it moved 1.4 dB rms; the largest were the keys whose beats or
fractions were the fault: F4, F7, B4 (its clamp) and A1.

- **Songs saved before** play the retuned voicing if they use model 2, as every
  `DSP_VERSION` bump before it moved the songs it touched (render and freeze caches key on
  the version); model 1 is untouched, to the bit. No schema bump: nothing in the file
  changes. The 24 presets play the retuned voicing with their own controls.

The size of the change (`DSP_VERSION` 9 → 10), measured (`CANTUS_GOLDEN_DUMP=1`):

| Render | max \|Δ\| | RMS of Δ | |
|---|---|---|---|
| Every golden hash (`null_tests` `v5`, Pulsus's kits, the scene bakes, the null corpus) | 0 | 0 | none includes model 2 of Chorda: no golden moved |
| Model 1 (`tests_hash`, an old song through the app) | 0 | 0 | pinned, held |
| Model 2 at beta.14's voicing (`MODEL_2_BETA14`) and at candidate 16 (`MODEL_2_C16`, the old `MODEL_2_SHIPPED`) | 0 | 0 | pinned, held |
| Model 2 at its default (the pinned 0.5 s phrase) | 0.743 | 0.141 (−17.0 dBFS) | retuned: the strings' phases drift apart within the phrase, so the difference is near the signal's own size; the phrase's RMS 0.187 → 0.202 (+0.64 dB); new pin `0x9561_d8f8_a3ce_3a61` |

### What the tests hold (fourth amendment)

| What | Test |
|---|---|
| Every key A0–C8 sounds on the curve: the ear's pitch within 1 cent to E6 (it is 0.2), 2.5 above (1.3); every fundamental within 3 | `chorda::tests_tune_gate::every_key_sounds_on_the_curve` |
| No key ragged against its neighbours (1 cent to E6, 2.5 above) | `tests_tune_gate::neighbouring_keys_lie_on_one_smooth_curve` |
| Octaves (2:1, 4:2) and fifths (3:2) beat as the curve's own: within 0.33 Hz, or 3 cents (3.5 above E6; 4:2 4.5 or 0.5 Hz) | `tests_tune_gate::octaves_and_fifths_beat_as_the_curve_draws_them` |
| No unison beats deeper than a recorded grand's register (10 / 16 / 20 / 22 dB) | `tests_tune_gate::no_unison_beats_deeper_than_a_recorded_grands` |
| The shipped voicing is candidate 16 with the pass; candidate 16 renders its old bits | `tests_voicing3::the_shipped_voicing_is_candidate_16_with_the_technicians_pass_and_a_new_unit_plays_it`, `model_2_at_candidate_16_renders_to_the_bits_it_did` |
| A Chorda added through the app plays Bb2, B2, C3 and B0 on the curve, on the real engine | `cantus-ui` `chorda_tune_e2e` |
| The designer's key tells the table and the room apart | `cantus-engine` `inst::chorda` `model_2_asks_for_its_own_keyboard_and_model_1_never_moves` |

### What it costs, and the CI gate (beta.15's release, 2026-11-19)

Candidate 16 costs more than beta.14's voice: 64 pedal-down voices at 48 kHz, the test
build (`cantus-dsp` at `opt-level` 3), on the owner's machine take **26.5–26.9 %** of one core
against beta.14's 20.7 % and model 1's 17.7–18.0 %. That is over ADR 0059's quarter
core, which `measure_model_2s_sixty_four_pedal_down_voices` (ignored, quiet machine) keeps
asserting: making candidate 16 cheaper without moving a sample is BACKLOG §4b's. The
technician's pass costs nothing per sample (26.9 % before and after it).

The suite's gate was an absolute 60 % of a core; GitHub's Windows runner times this about
2.25 times the owner's machine and read 60.8 %, failing beta.15's release. It is now
`model_2s_sixty_four_pedal_down_voices_cost_at_most_twice_model_1`: both models timed in
turn in 21 ms windows, the best of 600 each, model 2 under twice model 1 (1.48 alone,
1.66–1.71 under the whole parallel `cantus-dsp` suite and under two more copies of it beside
it), and model 2 under real time on any machine.

## Amended by the unison pass: every unison beatless (round 41, `DSP_VERSION` 11 — awaiting the owner's ear)

The owner, 2026-11-20, on beta.15: *"Chorda: some notes sound like some of their strings are
slightly out of tune … E4, C#3, D3, C3, B2, E5. Alone, they sound like one of the strings
inside the key is out of tune."* He decided: no comparison against a recorded piano; measure
each string of each note and adjust it; review all 88 keys. Round 41's first measurement
(`docs/round41/chorda-strings.md`) had found every loop "within tolerance" — of its own
design, which scattered each unison's strings on purpose. `docs/round41/chorda-tune.md` has
every number.

### 9. What a tuner hears is measured, and held

1. **Each unison's modes, read off its sound** (`chorda::modekit`, tests only): a key's
   strings once the hammer has left them are a linear system, so near each partial the
   unison rings as a few damped sinusoids (one per loop, as the bridge couples them); each
   partial heterodyned, low-passed and decimated, fitted by ESPRIT (Roy & Kailath 1989) to a
   thousandth of a hertz, on the voice's own bridge force (action noises and phantom
   partials off: neither moves a string's frequency). Each string alone is read the same way
   with the others muted on the bridge (round 41's felt).
2. **A beat the ear counts** is a pair of modes that swings the partial's level by a
   decibel or more while it sounds within 40 dB of its peak, for at least one whole swell in
   the first five seconds (`most_heard_beat`): slower than one in five seconds, or heard for
   less than a cycle (a fast first stage over a slow second), it is a change of level, not a
   beat.
3. **The bounds** (`chorda::tests_unison`, in the normal suite): on partials 1–3 of every
   unison at velocities 0.3, 0.6 and 0.9, no beat the ear counts; the vertical strings
   alone within half a cent of each other there (a beat every 7 s at C4's fundamental, every
   2.3 s at its third partial — a fraction of a cent, as a fine technician leaves a unison);
   each string alone's pitch (its partials 1–3 as the ear weighs them) within 0.75 cent of its
   key's pitch (the curve and its table), the table within 0.65 cent. The whole key's gate (`tests_tune_gate`, the fourth amendment's) still holds.

### 10. The causes, and what fixes each (`chorda::voicing::unison`, over the technician's)

1. **The stiffness scatter moved the strings' upper partials by tens of cents** (`uni_b` 0.054,
   twice at A0): C#3's third partials stood at −14.7, +2.5 and +10.0 cents, beating 5.7 Hz (E4
   5.1, D3 3.9). Round 41's first measurement held each string to its *designed* partials, so
   this passed. Each key's scatter is now scaled (`uni_b.<key>`, 0.02–0.35).
2. **The designed fan** (±1–2 cents on every partial) beat on the upper partials faster than
   the bridge can lock (its loss is the same on every partial, so it locks strings within a
   number of hertz). `uni_tight.<key>` takes a share of the `unison` control's first cent
   as the technician's: at the default each unison is as wide as it can be with no beat the
   ear counts and its strings alone within half a cent (median 0.66; the control's cents above
   1 detune as before, so the presets keep their character). `uni_pair.<key>` lays a
   three-string unison as two strings alike and one apart (35 keys): one slow mode instead of
   two that beat each other.
3. **The bridge's resonances** pulled the in-phase mode up to 2 cents from the opposed ones,
   and one lying on a partial split it unseen (C2: 0.9 Hz). `bridge.pull_hz` 0.1 holds the
   pull on partials 1–3 in hertz too, read across a ±2 % band.
4. **What the tuned unisons cost**, the slow second stage a detuned unison rang by beating,
   was given back where the bounds allow: `coupling.<key>` (16 keys, 0.2–0.6) takes less of
   the strings' loss through the bridge. Each key's four values were chosen by
   `chorda::tests_unison::unison_optimise` to keep its level closest to `DSP_VERSION` 10's;
   still, over 1.25–3.4 s the keys moved 4.5 dB rms, a third of them 6 dB or more (C3, E5,
   most of G4–A6). That is for the owner's ear.
5. **The technician's table, re-measured** on the tuned unisons (A0–E6 within 0.04 cent),
   folded to ±0.61 cent (it was ±1.21): a string alone and the whole key both land. Above E6
   nought but B6, D#7 (+0.5) and A#7 (−0.5).

Each is a voicing knob, neutral at 0 or 1 in every earlier voicing; `Voicing::technician()`
is `DSP_VERSION` 10's, pinned by name with its old hash. Songs on model 2 play the tuned
unisons (render and freeze caches key on the version); model 1 untouched; no schema bump.

### What the tests hold (fifth amendment)

| What | Test |
|---|---|
| The kit finds close modes to a thousandth of a hertz | `chorda::modekit::tests::the_kit_finds_close_modes_to_a_thousandth_of_a_hertz` |
| No unison beats the ear counts on partials 1–3, three velocities | `chorda::tests_unison::every_unison_is_beatless_on_its_first_three_partials` |
| The strings of each unison alone within half a cent | `tests_unison::the_strings_of_each_unison_alone_agree_within_half_a_cent` |
| Each string alone on its key's pitch; the table within 0.65 cent | `tests_unison::every_string_alone_sounds_its_keys_pitch` |
| The whole key on the curve, octaves and fifths, depth ceilings (the fourth amendment's) | `chorda::tests_tune_gate` |
| The shipped voicing is the technician's with the unison pass; the technician's renders its old bits | `tests_voicing3::the_shipped_voicing_is_the_technicians_with_the_unison_pass_and_a_new_unit_plays_it`, `model_2_at_the_technicians_pass_renders_to_the_bits_it_did` |
| A Chorda added through the app plays the six keys the owner named without a beat | `cantus-ui` `chorda_tune_e2e::a_new_chorda_plays_the_six_keys_the_owner_named_without_a_beat` |

### The size of the change (`DSP_VERSION` 10 → 11)

| Render | max \|Δ\| | RMS of Δ | |
|---|---|---|---|
| Every other unit's goldens, model 1, beta.14's, candidate 16's and the technician's voicings | 0 | 0 | pinned, held |
| Model 2 at its default (the pinned 0.5 s phrase) | 0.393 | 0.0802 (−21.9 dBFS) | the phrase's RMS 0.2015 → 0.2180 (+0.68 dB); new pin `0x2dca_3cab_2f21_fbbc` |

Each key at mezzo-forte against `DSP_VERSION` 10: 0.93 / 4.17 / 4.47 dB rms over 0–0.5 /
0.25–1.25 / 1.25–3.4 s (key by key in `docs/round41/chorda-tune.md`).

## Amended by candidate 17: the action as a grand's case rings it (round 42, `DSP_VERSION` 16 — awaiting the owner's ear)

The owner's blind test of candidate 16 against the Salamander (2026-11-25,
`docs/round42/chorda-abx16-verdict.md`): told apart 16 of 16, the Salamander preferred on five
phrases of seven — middle C's mid "somewhat ugly", fast runs "no mids, very metallic", the top
octave without "the hammers' mechanism", the bass close. `docs/round42/chorda-c17.md` has every
number.

### 11. Two recorded grands, never one

Every measure of this pass stands Chorda against the Salamander *and* the bitKlavier Grand
(Princeton, CC BY 4.0), both through the engine as the picker installs them. A difference both
grands share against Chorda is Chorda's; the grands' own difference is the natural spread, and a
cell inside it counts nothing. Fits use some keys and are judged on others; the sound lab's
subject `chorda-grand` (ADR 0160) judges single notes on its seeded held-out third.

### 12. The constant cue, and what fixes it

1. **Chorda was too clean between its partials** — outside both grands' band in 8 of 8
   held-out groups (the sound lab's `noise`), 10–20 dB under them in 0.02–0.3 s and gone sooner.
   The round-40 knock was a low two-mode click whose noise (`KEY_NOISE`) lay 80 dB under a
   note: nothing in the model sounded between the partials once the strings had the blow.
2. **The knock as the case rings it** (`chorda::noise::Thud`'s second stage and steep band,
   `VoiceSettings::knock_shape`, `Voicing::knock_shape` and `knock_noise_reg`; spec keys
   `knock.noise`, `.tau`, `.lo`, `.hi`, `.tail`, `.tail_tau`, `.t60`, `.noise_vel`,
   `.noise_bass`, `.noise_treble`): the noise scaled by register and by the hammer's speed,
   band-limited at 24 dB an octave above (a case's knock has no hiss) and 12 below, a slower
   second stage, the keybed's modes ringing longer — each neutral at its default
   (`voice::KNOCK_AS_IS`), so every earlier voicing renders its old bits. Fitted against both
   grands' top-key knock (under the fundamental, where no string of the key sounds) and the
   middle's noise between partials, judged on C6–C7 and the bass.
3. **Middle C's nasal mid is its third partial** (785 Hz): 10–13 dB over both grands at every
   key measure, its neighbours B3 and C♯4 too; voiced key by key in the partial table, which a
   spec now reaches cell by cell (`partial<n>.<key>`). Six top keys whose second or third
   partial stood 10–30 dB over both grands likewise.
4. **What the analysis before the blind test added**: the keybed's ring shorter and its noise
   from 90 Hz (a 70 Hz boom on middle C), the dampers' hold after the pedal (the pedalled
   tails 5–10 dB short at 0.5–1 s), the soft strikes' knock law, the damper felt's noise band.
5. An undamped top key thumps as it comes up (`key_return`), between the two grands' levels.

### 13. Colour, a control (`colour`, schema 84)

The owner asked that Chorda reach both colours. `colour` (−1 … 1, 0 Chorda's own voicing and
then not run at all) moves model 2's board sound along the measured difference between the
Salamander's middle phrases and Chorda's (`chorda::colour::SHAPE`, five octave bands
250 Hz – 4 kHz, the median over four phrases); before the lid and the room, as a technician's
voicing would be. A factory preset, *Recorded Grand Colour*, sets it to 1. An older song
reads 0: its sound unchanged by the control.

### What the tests hold (sixth amendment)

| What | Test |
|---|---|
| The knock's shape at its default is the round-40 thud; set and set back, a unit plays as one never set | `tests_voicing4::the_knock_as_is_is_the_round_40_thud` |
| Its noise in its band, its tail ringing on; a reshaped thud keeps its ringing | `tests_voicing4::the_knocks_noise_sits_in_its_band_and_its_tail_rings_on`, `a_thud_reshaped_keeps_its_ringing_and_its_tail_falls` |
| The register laws move their registers alone | `tests_voicing4::the_knocks_register_laws_move_their_registers_alone` |
| Every part on: the same bits at every block size and thread count; an idle unit as a new one; rest | `tests_voicing4::the_pass_gives_the_same_bits_at_every_block_size_and_on_threads`, `an_idle_unit_with_the_pass_plays_as_a_new_one_and_comes_to_rest` |
| Colour 0 is nothing; its stage follows its shape; a coloured unit keeps every block size and rests | `tests_voicing4::the_colour_at_zero_is_nothing_and_its_stage_follows_its_shape`, `a_coloured_unit_keeps_every_block_size_and_comes_to_rest` |
| The shipped voicing is candidate 17 over the unison pass; the unison pass renders its old bits | `tests_voicing3::the_shipped_voicing_is_candidate_17_over_the_unison_pass_and_a_new_unit_plays_it`, `model_2_at_the_unison_pass_renders_to_the_bits_it_did` |
| The tuning kits measure the strings with the action's noises off (the knock is no pitch); every gate holds | `chorda::tests_tune_gate`, `tests_strings`, `tests_unison` |
| A Chorda added through the app knocks as a grand under C8, and its *Colour* knob moves the middle by the shape; undone, the bits return | `cantus-ui` `chorda_c17_e2e` |
| Schema 84 keeps the colour; every older file reads 0 | `cantus-project` `format::tests::schema_84_keeps_chordas_colour` |

### The size of the change (`DSP_VERSION` 15 → 16)

| Render | max \|Δ\| | RMS of Δ | |
|---|---|---|---|
| Every other unit's goldens, model 1, beta.14's, candidate 16's, the technician's and the unison pass's voicings | 0 | 0 | pinned, held |
| Model 2 at its default (the pinned 0.5 s phrase) | 0.142 | 0.0253 (−31.9 dBFS) | the phrase's RMS 0.2180 → 0.2191 (+0.05 dB); new pin `0x9fe3_49b2_b228_bc8f` |

Two cells of the analysis's voicing were held back by the unison gate (ADR 0098's fifth
amendment, `no_unison_beats_deeper_than_a_recorded_grands`): cut 21–24 dB, A♯6's third partial
and D♯7's second left the board's share of each to beat against the strings past the ceiling;
A♯6 is not voiced and D♯7's second partial is cut 15 dB. The tuning kits measure with the
action's noises off since this amendment (the knock moved A0's measured fundamental 4 cents).

## Amended by candidate 18: the top's mechanics dry (round 42, `DSP_VERSION` 18 — awaiting the owner's ear)

The owner's blind test of candidate 17 (2026-11-26, `docs/round42/chorda-abx17-verdict.md`): told
from the Salamander 16 of 16 again, the Salamander preferred on five phrases of seven; the top
phrase back to candidate 16, its knock "a knock with reverb", not the Salamander's "hammer and
mechanism … there are details there"; "something in the very low notes (the first two)"; middle
C "a mid that gives it away … but both sound real". `docs/round42/chorda-c18.md` has every number.

### 14. What made the knock a reverb, and the dry mechanics

1. **Measured under the fundamental at a millisecond** (`probes/c18/mech.py`, `mech2.py`; C7–C8,
   both grands): the grands' top keys burst 5–30 ms after the strike, broadband to about 3 kHz,
   −4 … −6 dB under the note, their highs dying first (1–3 kHz falls 100–120 dB/s from 20 ms; the
   late part 20 dB under the burst). Candidate 17's knock was a 2 ms click and then its slow stage:
   −12 dB at 5–20 ms, falling 63 dB/s, as much energy after 30 ms as before — a decaying noise of
   the same colour throughout, which is what a room sounds like. Neither the room nor the body made
   it (`room=0` moves it under 1 dB): the knock's own second stage did.
2. **The knock gets two parts and a top shape**, each neutral at its default: the first stage's
   rise over a share of its fall (`knock.rise`), the slow stage's own low-pass (`knock.tail_hi`),
   and the top keys' shape from C6 to its values at C7 and above (`Voicing::knock_treble_shape`:
   the fall, the band, the slow stage's share and fall as factors, the rise and the slow stage's
   band as values; `knock.*_treble`). Candidate 18: a 15 ms burst rising over its first quarter,
   its band to 2.7 kHz, the slow stage at three quarters of its share and fall under 1.5 kHz.
3. **The bass's thump before its tone.** A bass string's tone reaches the bridge 10–15 ms after
   the hammer; candidate 17's knock and the hammer's blow stood there alone, loud enough that the
   note began with a thump and its tone 20 ms later (A0–D♯1 rising in 21–34 ms; the grands in
   5–13). `knock_bass` and `blow_bass` scale both at A0 (log to 1 at C4): at 0.3 the bass's rise
   is the grands'. The bass's release "click" the analysis listed is under both grands' recording
   floor in absolute level (−77 … −80 dB re the note, the grands −70 … −86): not a cue.
4. **Middle C's fifth partial** (1.31 kHz) stood 5–10 dB over both grands' span in the phrase's
   sustain: `partial5.60` −10. Its second (523 Hz) too, but cutting it took middle C's stereo
   correlation from 0.49 to 0.30 (the grands 0.52 / 0.86): left.

### What the tests hold (seventh amendment)

| What | Test |
|---|---|
| The new parts are nothing at their defaults; candidate 17 keeps its bits | `tests_voicing5::the_new_knobs_at_their_defaults_are_nothing`, `tests_voicing3::model_2_at_candidate_17_renders_to_the_bits_it_did` |
| The top's shape moves C6 and up alone and is whole at C7 | `tests_voicing5::the_trebles_shape_moves_the_top_alone_and_reaches_its_value_at_c7` |
| The rise takes the first milliseconds and keeps the peak; the slow stage's band takes its highs, rests, and set back is never set | `tests_voicing5::the_clicks_rise_…`, `the_rings_own_band_…` |
| The bass's knock and blow move the bass alone | `tests_voicing5::the_bass_knock_and_blow_scale_the_bass_alone` |
| Every part on: every block size and thread count, idle as new, rest | `tests_voicing4` (its `pass` sets every new part) |
| The shipped voicing is candidate 18 over candidate 17 | `tests_voicing3::the_shipped_voicing_is_candidate_18_over_candidate_17_and_a_new_unit_plays_it` |

### The size of the change (`DSP_VERSION` 16 → 18)

| Render | max \|Δ\| | RMS of Δ | |
|---|---|---|---|
| Every other unit's goldens, model 1, every earlier voicing (beta.14's, candidate 16's, the technician's, the unison pass's, candidate 17's) | 0 | 0 | pinned, held |
| Model 2 at its default (the pinned 0.5 s phrase), against candidate 17 | 0.0870 | 0.01385 (−37.2 dBFS) | the phrase's RMS 0.2191 → 0.2178 (−0.05 dB); new pin `0xb19b_545c_67c3_0ee9`; candidate 17 named and pinned (`MODEL_2_C17`, `0x9fe3_49b2_b228_bc8f`) |
