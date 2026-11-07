# ADR 0098 — Chorda's second model: voiced against a reference piano

**Status:** Accepted · **Date:** 2026-11-04 · **Extends:** ADR 0059 (Chorda), ADR 0023 (the
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
