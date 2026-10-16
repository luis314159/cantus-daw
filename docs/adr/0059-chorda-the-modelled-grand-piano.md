# ADR 0059 — Chorda: the modelled grand piano

**Status:** Accepted · **Date:** 2026-10-15 · **Extends:** ADR 0023 (native instruments),
ADR 0024 (the kit shell), ADR 0031 (exact rest), ADR 0038 (voices across the pool)

## Context

BACKLOG §3e asks for a grand piano that is played from a physical model, not from
samples — the instrument a free DAW cannot otherwise offer (Pianoteq is the only
commercial one of note) — with a measurable *done when*: every partial within 1 cent of
the stretched tuning on every key; each partial's T60 within −25 % / +40 % of published
measurements (the perceptual tolerance of decay, Tolonen & Järveläinen via Bank et al.
2003); a continuous half pedal; 64 voices with the pedal down inside 25 % of one core at
256-frame blocks; and the family's rules (a default changes no sample, block-size
independence, offline equal to real time). Three patents constrain the structure: no
modal bank whose per-partial coefficients come from a soundboard parameter (Modartt US
7,915,515), no detuned non-speaking delay-loop resonators (Casio US 8,178,772), no loop
at a non-speaking pitch (Yamaha US 10,818,278). The decision to build it this round, and
why it could be finished, is `docs/round28/sound.md` §B, written before any code.

## Decisions

### 1. The keyboard's numbers are published physics, computed, never fitted to a board

`chorda_tables.rs`: per key the first partial, the inharmonicity `B`, the string count
(one to F♯1 — MIDI 30 —, two to F2 — MIDI 41 —, three above), the speaking length, wave speed and
impedance, the decay law `σ_k = b₁ + b₂ (kπ/L)²`, the hammer (mass, `K`, `p`, strike
point) and whether it has a damper (none above E6, MIDI 88).

- **Inharmonicity and tuning:** Rigaud, David & Daudet (DAFx 2011) — `B` as the sum of a
  treble and a bass exponential (`s_T = 9.26e-2, y_T = −13.64`; the bass branch
  `s_B = −0.09, y_B = −6.38` fitted to Bank's measured C2, `B = 1e-4`) — and their aural
  tuning: octaves widened to match partials of order `ρ(m) = (K−1)/2·(1 − erf((m−64)/24))
  + 1`, `K = 4.51`, from A4 outwards. A0 lands at −24 cents, C8 at +35 cents: a Railsback
  curve. *Stretch* scales the octave type's excess, *stiffness* scales `B`.
- **Strings:** Bensa et al. 2003 (C2: L 1.23 m, b₁ 0.25, b₂ 7.5e-5; C4: 0.63, 1.1, 2.7e-4;
  C7: 0.10, 9.17, 2.1e-3), interpolated on a log scale between them; the tension 700 N.
- **Hammers:** Chaigne & Askenfelt 1994 (C2 4.9 g, `K` 4e8, `p` 2.3, struck at 0.12; C4
  2.97 g, 4.5e9, 2.5, 0.12; C7 2.2 g, 1e12, 3.0, 1/16); the felt's memory after Stulov 1995
  (`ε` 0.9, `τ` 10 µs, `Q₀ = K/(1−ε)`), solved against the strings' impedance with four
  Newton steps a sample.

### 2. A string is a waveguide whose dispersion is designed, then measured

`chorda_design.rs`, `chorda_string.rs`: two rails split at the strike point; a linear-phase
three-tap loss filter (its decay follows `σ_k` and never moves a pitch); a first-order
fractional allpass; and the stiffness as **second-order allpass sections** constructed after
Abel, Välimäki & Smith (2010) and refined by Levenberg–Marquardt with an analytic Jacobian
until every partial below 5 kHz (at most 32) lies within a cent of `n F₀ √(1 + B n²)`
(the fundamental weighted ten times, then put exactly in tune by a Newton step on the
fractional delay). The fit is checked to 10 kHz. 1 to 22 sections a key (the bass needs the
most). A keyboard (88 designs and 88 reduced ones for the sympathetic strings) takes about
two seconds on eight threads.

The unisons are detuned by a hashed spread (1 cent by default) and meet at a **resistive
bridge** `Y·Z = tanh(κ·b₁/(2 f₁))`: their in-phase motion drains through it faster than
their out-of-phase motion — Weinreich's two-stage decay (measured: 24.6 dB/s prompt, 8.7
dB/s aftersound at middle C). **Phantom partials** come from the tension's modulation
(`∝ v²` of the strings' arriving waves) through a longitudinal-mode resonator.

### 3. The post-voice stage: sympathetic strings at the keys' own pitches, a multirate board

`chorda_board.rs`, after the voices' lanes are summed in key order (ADR 0038):

- **Sympathetic strings**: every key's own string, at its speaking pitch (a reduced design
  of that key, at most two dispersion sections: the first partial exact, the low ones
  close), driven one way by the bridge's motion (`0.2 · Y · F`, high-passed at 20 Hz: a
  bridge does not move steadily), its damper the key's — lifted by the pedal continuously
  and gripping as firmly as the *dampers* setting says, as a voice's does; the keys above
  E6 ring always. Never a detuned or non-speaking loop (Casio, Yamaha). Its loop is one
  ring for the round trip with **two** inversions — the nut's rigid reflection
  (`arrived = −x`, as a voice's `-nut.read()`) and the bridge's `2·v_b − arrived` — so it
  rings at `k·f₀`; one inversion (review 1 found it in the first version) would have put
  it at `(k − ½)·f₀`, an octave down with odd partials only — close to the non-speaking
  resonator the patents describe. Half the first partial's loss is the bridge's
  reflection `(1 − YZ)/(1 + YZ)` (as in a voice), which keeps the loop under unity at DC;
  the loss filter is set from the loop's own trip time at each partial (its group
  delay), so a sharp dispersion section does not slow a partial's decay. A string is
  silenced as its key's voice takes over and comes back silent when the voice has ended
  (never its stale state replayed); a new keyboard lays a string out only once it is
  silent (never a cut).
- **The soundboard** after Bank et al. 2003: 40 low modes at a quarter of the rate (60 Hz –
  1.5 kHz, loss factor ≈ 3 %), a four-line Hadamard FDN for the high band whose decay
  shortens with frequency, and the direct radiation (a 35 Hz high-pass, a tilt). A filter
  after the strings, never a source of their coefficients (Modartt).
- **Radiation**: a plate below coincidence radiates better as frequency rises; the output
  rises 20 dB between 120 Hz and 1.2 kHz against the bridge's force — what levels the
  keyboard (A0 −10.7, C4 −9.1, C6 −13.6 dBFS peak at full velocity, alone).

### 4. Dampers and pedals

A released key's damper grips with a contact `1 − smoothstep(0.15 … 0.65)` of the pedal
(the half-pedal zone) times the *dampers* setting, gliding over 8 ms: no step between two
models, so re-pedalling leaves no discontinuity. A key struck from rest finds its damper
already lifted (the key's travel lifts it before the hammer lands). A strike takes the
settings of the moment — a new decay or coupling reaches a voice struck again, and the
ringing sympathetic strings. A ringing voice keeps its strings when a new keyboard arrives
(it takes the new one at its next strike from rest). The soft pedal (CC 67) shifts the
next strikes' hammers off one string onto softer felt (*soft pedal* sets how far).

### 5. Every clock counts from the stream, so any split gives the same bits

The voices' control steps (32 samples) count from their strike; the board's quarter-rate
steps and the chunks (64 samples) from the instrument's start. A chunk ends only where the
stream's grid does; the sympathetic dampers learn which keys sound only on that grid; the
sympathetic strings wake on drive *seen*, never on drive looked ahead (a block's end would
cut that look); the board's tail is flushed below 1e-10 only on the grid. `skip` moves every
clock as processing would. (The instrument contract caught the first version's chunks
counted from each block.)

### 6. One designer, nothing waits on the interface thread

One designer thread makes keyboards, one at a time, into a registry that never evicts (128
`OnceLock` entries keyed by A4 to 0.01 Hz, stretch and stiffness to 0.001, and the rate;
Unda's rule: what the audio thread holds is never freed there) and that the audio thread
scans lock-free. It is asked for the reference keyboard at start-up (`assemble`) and after
a device change, at once; and for each track's tuning on every snapshot (`units_for`),
designed only once it has **settled** for 250 ms — a knob being dragged asks for a tuning
a frame, and only the one let go is designed (review 1: the first version designed one a
frame and filled its registry in a drag). A tuning the registry has no room for is
remembered and never designed again that session. A unit never waits: made before its
keyboard exists it is silent, and the editor says *designing the strings* with the
design's progress, until the keyboard arrives; while another tuning is designed it plays
the one it had. An offline render waits for its own, designing it on the spot if the
registry cannot keep it. `a4`, `stretch` and `stiffness` are set, never modulated or
automated: `InstrumentKind::automatable` says so, the engine ignores a lane on them (an
export never waits for a tuning nobody asked for), the knob offers no clip and the app
refuses one with the reason.

### 7. The model is versioned

`model` (1) is stored with every new instance (`NativeSpec::new`); a user preset or a
DAWproject state written without it reads it as 1 (`read_unversioned`). A later model
changes `MODEL` and never a song made with this one.

## Consequences

- A free, sample-less grand piano with an editor that shows the physics — strings at their
  lengths, the half-pedal law, the tuning's stretch, the felt, the ring — in Play / Edit /
  Matrix, ten presets with four macros, both languages, the ballad demo.
- 88 voices (one per key: a repeated note strikes the strings that still ring), every
  delay line allocated when the unit is built, never on the audio thread.
- The first time Chorda plays at a sample rate (or a tuning) it has not designed, it is
  silent for the second or two the design takes, and says so; the window never waits.
- **The owner's, before release** (`docs/beta-checklist.md`): the blind listening test
  (≥ 20 listeners against Pianoteq 9 and a sampled grand) and the legal review of the
  structure against every claim of the patents above.

## What the tests hold

| Done when | Test | Measured |
|---|---|---|
| Every partial within 1 cent | `tests_string::every_key_rings_within_a_cent_at_every_rate_and_tuning` (rendered, phase vocoder: all 88 keys at 44.1, 48 and 96 kHz, at A4 415 Hz with 70 % stretch, and at 150 % stiffness); `tests_design::every_key_is_designed_within_a_cent…` | worst on the sound 0.47 (44.1 kHz), 0.49 (48), 0.49 (96), 0.48 (415 Hz), 0.50 cents (stiffest) |
| Per-partial T60 within −25 %/+40 % of Bensa et al. | `tests_string::a_string_decays_as_bensa_et_al_measured` (C2, C4, C7) | within −4 % / +1 % |
| Half pedal continuous | `tests_piano::half_pedalling_is_continuous`, `re_pedalling_leaves_no_discontinuity` | decay 241 → 41 dB/s across the pedal, largest step 57 dB/s (28 % of the range) |
| 64 pedal-down voices < 25 % of a core at 256 frames | `tests_piano::measure_sixty_four_pedal_down_voices` (ignored, release) | 18.2–19.5 % (best of three, this PC; 19.5 % after review 1) |
| Defaults change no sample; any split; parallel = serial; exact rest | `inst_chorda.rs`, `inst_contract.rs` (split, voices, at_rest, block sizes), `tests_engine::notes_and_pedals_inside_a_block…` | to the bit |
| Offline = real time | `inst_chorda::offline_render_equals_real_time` (reference tuning and a designed one) | to the bit |
| Headroom | `inst_chorda::its_levels_leave_headroom…`, `instrument_levels` (every preset, dense riff) | both hands ff with the pedal −3.3 dBFS; loudest preset −6 dBFS |
| Two-stage decay, una corda, sympathy | `tests_piano::*` | 24.6 / 8.7 dB/s; softer and darker; sympathy −16 dB with the pedal, −35 dB without |
| A sympathetic string at its key's pitch | `tests_engine::a_sympathetic_string_rings_at_its_keys_own_pitch` (C3, C4, C5, E7) | the first two partials 70–145 dB over `f₀/2`; the first within 5 cents |
| No ghost, no cut, the grip, a new decay, a lifted damper | `tests_piano::a_struck_string_leaves_no_ghost…`, `a_new_keyboard_never_cuts…`, `without_dampers_the_strings_ring_in_sympathy`, `a_new_decay_reaches…`, `a_fresh_strike_finds_its_damper_already_lifted` | — |
| Never waits; designs what settles; tuning not automatable | `inst_chorda` (engine): `a_unit_never_waits_for_its_keyboard`, `a_dragged_tuning_designs_only_where_it_settles`, `a_lane_on_the_tuning_is_inert…`; UI: `the_tuning_is_set_never_automated`, `while_the_strings_are_designed_the_editor_says_so`, `inst_chorda_boot` | — |
| The editor | `cantus-ui/tests/inst_chorda.rs` (views, undo, drop, presets, no elided or overlapping text at 75–200 % and in Spanish, kit lint, the boot path) | — |

## Amended by fix3 (round 28: the integration review and sound's second review)

- **A keyboard reaches what rings.** A sympathetic string that a new keyboard finds ringing
  fades its output over 256 samples and then takes the new layout (woken again if the bridge
  drives it) — under continuous playing it never went quiet before and kept the old tuning.
  A key struck while its strings ring for an older keyboard fades them over 128 samples and
  the strike lands on the new strings (a note-off meanwhile lands it released). Never a cut;
  a song whose keyboard never changes renders as before, to the bit.
- **A fresh unit is silent until its own tuning arrives**: a unit that has struck nothing
  drops the reference keyboard when the tuning asked for is not designed yet (it played
  A4 440 meanwhile). Once it has played, a tuning on its way waits behind the one playing.
- **The designer never leaves an export waiting**: a design runs under `catch_unwind` (a
  failure is refused, the designer freed); `ensure` designs on its own thread when the
  designer could not start, refused or failed the design, or has not delivered in two
  minutes. A refused tuning says so in the editor (`chorda.no_room`), never "0 %".
- **One rule for the tuning keys**: `InstrumentKind::modulatable` (automatable and with a
  range) closes every path — the shared knob wiring (no menu, no rings, no inert clip's
  value), the source-chip drop, `connect_modulation`, the matrix's targets, a modulator's
  handle, *New modulator*, an instrument swap's carried connections.
- The bench rig designs a Chorda's keyboard before it builds its units (`inst.chorda.v8`
  measures eight voices); the 1-cent test measures the ranges' corners too (A4 466, 200 %,
  150 % at 44.1 and 96 kHz: 0.488 and 0.497 cents; A4 415, 0 %, 50 % at 44.1 kHz: 0.483).

## Amended by fix4 (round 28: the review of fix3)

- **A fresh unit waits silent only for a tuning that can still arrive.** One whose tuning
  the session refuses (the registry full, a design failed) or that no designer can make
  (its thread could not start) keeps the reference keyboard and plays — what the editor's
  "keeping this one" says; one that dropped it while the tuning was on its way takes it
  back when the tuning is refused. The audio thread asks `can_arrive` — lock-free flags and
  a fixed list of failed designs, written under the designer's lock — never the designer's
  mutex.
- The designer's fault tests run one at a time, reset their faults even when they fail,
  and prove the path they name (the spawn failures are counted); a keyboard landing
  mid-phrase (the sympathetic fade, a ringing key re-struck and let go inside its fade)
  gives the same bits at blocks of 1, 37, 64 and 512 and on threads.

## Amended by fix5 (round 28: the review of fix4)

- **The editor says when no designer can make a tuning.** `chorda::design` says where a
  tuning's design stands — `Ready`, `Coming(done)`, `Refused` (the registry full, its
  design failed: `chorda.no_room`) or `NoDesigner` (the thread could not start:
  `chorda.no_designer`, "the string designer could not start: keeping this one") — and
  `progress` is `None` for the last two, so the editor never reads "designing: 0 %" for a
  session while the unit plays the keyboard it has. Tested with the spawn fault
  (`a_tuning_no_designer_can_make_is_not_on_its_way`, seen failing on `Some(0.0)`) and the
  caption's words in both languages.
