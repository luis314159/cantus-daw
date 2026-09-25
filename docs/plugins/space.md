# Spatium — the reverb (`EffectKind::Space`)

*Owner: the reverb. Benchmark: FabFilter Pro-R 2 (`research/fabfilter/pro-r.md`). The older
`Reverb` kind (Freeverb, `cantus_dsp::reverb`) is untouched, so old projects render as
they did.*

## What it is

A reverb whose decay time is a **curve you draw in seconds**, on a feedback delay network
whose filters are fitted so the tail really follows it — and an editor that shows the
curve, **what the engine achieves**, and **what was measured** on the audio that went
through it, side by side. Around that: twelve rooms morphed by one Space control, early
reflections you can shape, a post EQ, ducking keyed from any strip, freeze, four styles
(Room, Plate, Vintage, Spring), and a time view of the tail with handles on it.

## Files

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/space.rs` |
| DSP | `crates/cantus-dsp/src/spatium.rs` (processor) and its children: `spatium_fdn.rs` (the late field and the fit), `spatium_model.rs` (rooms, curves, reflections — pure functions shared with the editor), `spatium_spring.rs`, `spatium_measure.rs` (Schroeder T20, echo density, autocorrelation, correlation), `spatium_tests.rs` |
| Engine | `crates/cantus-engine/src/fx/space.rs` (`settings(spec)`, telemetry layout) |
| Editor | `crates/cantus-ui/src/views/fx/space.rs`, `space_curve.rs` (the decay/post-EQ display), `space_tail.rs` (the time view and the measurement) |
| Strings | `crates/cantus-ui/locales/fx/space.{en,es}.txt` |
| Tests | the DSP module's own, `crates/cantus-engine/tests/fx_space.rs`, `crates/cantus-ui/tests/fx_space.rs` |

The DSP's sub-files are declared from `spatium.rs` with `#[path]`, and the editor's the same
way, so no shared `lib.rs` or `mod.rs` changed.

## The engine, and why it is built that way

**Signal flow.** Input → freeze gate → pre-delay (gliding) → early reflections (a tapped
delay, a pattern per room, darker with distance) and, after a gap, four Schroeder
all-passes of input diffusion → the late field → post EQ (Aequalis, wet only) → width (M/S)
→ ducking → mix. The mix law keeps the dry at unity up to 50 % and brings the wet to unity
at 50 %.

**The late field** is a 16-line feedback delay network:

- **Lengths** are distinct primes spread geometrically over the room's span (2.2–135 ms),
  interleaved so neighbours differ.
- **The matrix** is `A = P·H·D`: random signs, the Hadamard transform, a permutation. The
  Sylvester–Hadamard matrix alone is an involution (`A·A = I`): whatever leaves a line
  returns to it two passes later, and a still tail showed it as a repetition of 0.25 at a
  line's period. With signs and a permutation the strongest repetition dropped to 0.10.
- **Modulation** (Character): each line is read through a first-order **all-pass
  interpolator** whose length follows its own slow sine. An all-pass is lossless at every
  frequency, so modulation smears the modes without costing high-frequency decay — the
  fitted curve stays exact. With no modulation and a whole-sample length it is a plain
  delay.
- **Per-frequency decay** (Jot & Chaigne; Prawda, Välimäki & Schlecht, DAFx 2019): each
  line's filter must lose `Aᵢ(f) = mᵢ·γ(f)` dB per pass, `γ = −60 / (fs·T60(f))`. Each
  line's filter is a low shelf, eight octave bells, a 12.5 kHz bell and a 16 kHz shelf plus
  a broadband gain (twelve numbers), fitted on a third-octave grid by least squares through
  an interaction matrix, then refined four times by feeding back the true residual.
  Two findings shaped it:
  1. **Weight by 1/t².** What must be right is the decay *time*, whose error is relative.
     Unweighted, the fit spent its accuracy on the large high-frequency losses and the long
     bass came out 5 % wrong.
  2. **Store sections as `H = 1 + D/A`, poles as distance from `z = 1`.** In direct form a
     44 Hz shelf's `1 + a1 + a2` is ~3·10⁻⁵ at 48 kHz, so one f32 step of `a1` moves its DC
     gain by 0.03 dB — 18 % of what a long line loses per pass there. In the delta form a
     section with little to do has small coefficients, each exact to f32's relative
     precision. The bass error went from 9.7 % to under 1 %.
- **The loop gain stays below one**: every line's response is checked on a sixth-octave
  grid and at both ends; any point above −0.0001 dB brings the broadband gain down. A
  request past 40 dB per pass is clamped — and the editor shows the achieved decay next
  to the target, so a clamp is visible.
- **Freeze** cross-fades each line from filtered to raw × 0.99999 (a convex blend of two
  gains below one: stable at every point) and gates the input. The all-pass interpolator's
  steps between whole samples under modulation are not quite lossless (a frozen tail sank
  0.4 dB/s), so a level hold takes the loop's energy 150 ms after it closes and trims the
  gain toward it, at most ±0.2 % per pass.

**Smoothers that land.** An f32 one-pole stalls short of its target once a step is under
half an ulp — near 1.0 that is 7·10⁻⁵ short — so `freeze` never reached 1 and line lengths
stopped ~0.1 sample off their primes. Every glide now snaps when a step no longer moves it
(`fdn::glide`; a test holds both cases).

**Rooms** (`model::ROOMS`) are data: booth, small room, studio, chamber, club, theatre,
church, concert hall, large hall, arena, cathedral, vast. Space interpolates every number
(geometrically for times). **The Space knob moves the decay with it**, keeping the ratio to
the room's natural decay (one gesture, one undo step); the Decay knob moves it alone. The
**decay** parameter is the mid-band RT60 in seconds: `room(f)` is normalised to 1 at 1 kHz.

**Styles**: *Plate* — lines at 0.42× the room's, no reflections, high diffusion, brighter
air. *Vintage* — sparse reflections, 2.6× modulation depth, a brighter top cut at 9 kHz.
*Spring* — a separate tank (`spatium_spring.rs`, after Välimäki, Parker & Abel 2010):
three loops of 36 stretched all-pass sections (`a = −0.62`, `K = fs/8.4 kHz`) that delay the
lows more than the highs — the falling chirp — plus a delay line and the same fitted
absorption, with the chain's own group delay added to each loop's length so the decay
curve holds for springs too.

**Ducking** listens to the slot's sidechain when one is routed (`TAKES_SIDECHAIN`), the
input otherwise: a peak detector, a threshold, a range, attack and release — in dB and
ms, where the benchmark offers one knob.

**Latency**: none. **Allocation**: none after `new`. **Cost**: the absorption runs
section-by-section across all 16 lines (structure of arrays, so it vectorises); the fit
(~0.5 ms) runs only when something that decides the late field changes, not on every
snapshot. Measured in release (`spatium_speed`, ignored by default) with a busy setting —
modulation, a decay band, a post-EQ cut, ducking — on a machine running eight builds at
once: Room 6.3 %, Plate 6.1 %, Vintage 5.6 %, Spring 8.3 % of one core at 48 kHz, the
same at 512 samples per block and at 1. Quality modes (fewer lines) are the obvious next
step if that matters.

## The parameters

Sparse (a new reverb stores nothing). Keys are forever: `style`, `space`, `decay` (s),
`predelay` (ms), `sync` (the note value it was set from; the editor computes the ms from
the project's tempo — the engine only sees ms), `distance`, `character`, `bright`,
`width` (0–1.2), `mix`, `freeze`, `er` (dB offset, −30 = off), `er_size`, `duck` (range),
`duck_thr`, `duck_att`, `duck_rel`, `deq_on`, `peq_on`, `peq_auto`; decay bands
`d0…d5.{used,on,shape,freq,mult,q}` (shape: bell, low shelf, high shelf, notch; `mult`
×0.25…×4); post-EQ bands `e0…e5.{used,on,shape,freq,gain,q,slope,place}` (bell, shelves,
cuts 6–96 dB/oct; stereo/L/R/M/S).

## The editor

- **Top row**: style, freeze, and the macros (Space large, as the hero; Decay, Pre-delay
  with its note-value menu and a "follow the tempo" button when the tempo moved, Distance,
  Character, Brightness, Width, Mix). Every value is printed under its knob, at rest.
- **The curve display**: decay time against frequency on a seconds axis (a tenth to five
  times the decay, 1-2-5 labels), the post EQ on its own dB axis in another hue and dashed.
  Drawn so they can be told apart without colour: the curve asked for (thick, filled
  against the curve before the drawn bands), the curve before the bands (dashed), what the
  engine achieves (dotted), what was measured (hollow squares). Decay bands are numbered
  circles, post-EQ bands lettered squares. Double-click makes a band on the curve chosen
  above (shelves at the edges, a notch at the floor, cuts at the far edges of the EQ),
  drag moves it (Shift fine), wheel sets its width (a cut's slope), Alt-click bypasses,
  right-click gives its menu, Delete removes it. Each curve has its own bypass.
- **The octave table** under it: 63 Hz … 8 kHz, three rows in seconds — curve, engine,
  measured.
- **The tail view**: the pre-delay (shaded), each early reflection as a stem, the late
  field's build-up and its decay at 1 kHz (solid) and at 125 Hz / 8 kHz (dashed), the
  measured envelope of the last event (filled), and four diamond handles — pre-delay,
  the reflections' window (size), their level, and the decay (the −30 dB point).
  A **Spectrum** mode shows the last tail measured as a spectrogram (40 bands, 10 ms
  hop, the device hue's strength as level, one hue so it reads in both themes and in
  grey) with the curve's −30 dB (solid) and −60 dB (dashed) lines per frequency drawn
  over it: if the engine does what it is told, the colour fades along the dashed line.
- **Cards**: the selected band's numbers (the decay multiplier shown with the seconds it
  gives), early reflections, ducking (key picker, four knobs, a gain-reduction history),
  output (wet meters with scale ticks, a correlation bar, a mono warning when width > 100 %
  makes the correlation negative).
- **The measurement** runs on the UI thread from the engine's analyser pairs: the dry part
  is subtracted (the mix law and the slot's mix are known), an event is found in the input,
  and when its tail has fallen 35 dB (or four seconds passed) it is measured exactly as
  the DSP tests measure: octave bands, Schroeder's backward integration, T20.

## What the tests hold (numbers printed with `--nocapture`)

| Test | Holds |
|---|---|
| `fdn::every_line_decays_at_the_target_rate` | every line's decay within 4 % of the target, 50 Hz–16 kHz, five rooms/brightnesses (worst 3.3 %, 5 kHz on the darkest) — under the 5 % JND of ISO 3382-1 |
| `fdn::the_loop_gain_stays_below_one` | an extreme request (30 s, ×4 highs, a deep notch): every line < 0 dB at 2000 frequencies |
| `the_measured_decay_follows_the_curve` | impulse responses measured by octave-band Schroeder T20 (4 source/receiver pairs, energy-averaged) vs. the same measurement of an ideal decay with the target curve: 5 settings × 7 octaves, **worst 3.7 %**, most under 2 % |
| `the_tail_is_dense_and_does_not_repeat` | normalised echo density ≥ 0.9 mean (measured 1.00–1.01, lowest 20 ms window 0.92); strongest autocorrelation at 2–250 ms < 0.2 (measured 0.072 default, 0.099 without modulation, 0.048 small room with a long decay) |
| `the_sides_are_decorrelated_and_width_narrows_them` | L/R correlation of a mono impulse 0.021 at width 1; 1.000 at width 0 |
| `freeze_holds_the_tail_and_shuts_the_door` | frozen level +0.16 dB after 5 s; input after freezing changes the output by < 10⁻⁶ |
| `ducking_follows_the_key` | 12 dB range → −11.94 dB while the key is loud, back within 1 dB after release |
| `the_wet_level_is_calibrated` | steady pink noise at a 2 s decay: every style within ±1 dB of its designed level |
| `mix_zero_is_a_wire_to_the_bit` | all styles, frozen or not, keyed, busy settings: bit-exact |
| `the_block_size_changes_nothing` | blocks 1, 7, 64, 1000, 4096 vs 512, every style, keyed: tolerance 0 |
| `two_instances_agree`, `silence_comes_back_to_exact_zero` | determinism; after a burst, exact zeros (no denormal tail) |
| engine `fx_space` | mix 0 through the whole processor = no reverb, to the bit (busy settings, frozen, keyed); block sizes 1/7/64/1000/4096 at tolerance 0 in the engine; parameter defaults = DSP defaults; keyed from the bass strip, the render is bit-identical to an unducked one until the bass comes in, then the lead's reverb drops 13.7 dB; telemetry's achieved decay within 4 % |
| UI `fx_space` | the editor paints its displays and numbers; the demo's tail, measured from the analyser pairs by the editor, is within 15 % of what an octave band reads on the ideal curve at 250 Hz–4 kHz; the spectrum view has a tail that fades by more than 20 dB and names its lines; every factory preset names real parameters; a band made and removed is one undo step each (both locales' keys are held by the app's i18n test) |

## Feature matrix against Pro-R 2

| Feature | Pro-R 2 | Spatium | |
|---|---|---|---|
| Space macro morphing room models, decay follows | ✓ (≈12 unnamed models) | ✓ 12 **named** models, decay follows (ratio kept) | have, better (named) |
| Decay | % of the model's | **seconds** (mid RT60), and Space keeps the ratio | better |
| Decay rate EQ | 6 bands, % axis | 6 bands, **seconds axis**, per-curve bypass | better |
| Achieved vs target decay | — | engine's fitted decay drawn and tabled per octave | better |
| Measured decay | decorative ghosts | **measured T20 per octave** on the real tail | better |
| Time axis | — | tail view: pre-delay, reflections, build-up, decay per band, measured envelope, 4 handles | better |
| Early reflections | not adjustable | level and size, drawn and draggable | better |
| Distance, Character, Brightness, Width, Mix | ✓ | ✓ (width 0–120 %) | have |
| Pre-delay with sync | ✓ 4 values + offset | ✓ 11 values incl. dotted and triplet, shows the ms, follows tempo changes on request | have |
| Post EQ | 6 bands, cuts to 96 dB/oct, placement, auto gain | ✓ same (Aequalis), per-curve bypass, shows the compensation | have |
| Ducking | one knob, own input | range, threshold, attack, release, **key from any strip**, GR history | better |
| Freeze | ✓ | ✓ with a level hold | have |
| Styles | Modern, Vintage, Plate | Room, Plate, Vintage, **Spring** | better (spring) |
| Thickness | ✓ | — | missing: time; density comes from diffusion and the matrix |
| Auto Gate | ✓ | — | missing: the family's gate (Porta) can follow the reverb |
| IR import | ✓ | — | out of scope this round |
| Surround / Atmos | ✓ | — | stereo only (the host is stereo) |
| Waterfall | decorative ghosts drawn from the settings | **measured** spectrogram of the real tail with the curve's −30/−60 dB lines over it | better |
| Lock mix, presets by size, favourites | ✓ | factory presets (8), host presets, host A/B and undo | have (lock missing) |
| Legibility | low contrast | theme tokens, both themes, shapes as well as colours | better |

## Decisions worth knowing

- **Decay is stored in seconds**, not as a rate: automation of `decay` means seconds. The
  Space knob keeps the ratio to the room's natural decay (Pro-R's behaviour) by writing
  both parameters in one gesture; automating `space` alone morphs the room and leaves the
  decay where it is.
- **The drawn curve's bands store multipliers** (×0.25…×4), so the curve keeps its shape
  when the decay moves; the display converts to seconds.
- **Pre-delay sync lives in the editor**: `sync` records the note value, the engine only
  sees milliseconds, and a tempo change is shown ("Follow 128 BPM") rather than applied
  behind the user's back (that would be an unrequested undo step).
- **Detail at rest**: the cards (band, early reflections, ducking, output) are always
  visible — the family rule "state is visible at rest" won over the research's "no more
  than ten knobs on the default surface"; the eight macros stay the largest controls.

## At merge

- Nothing outside the owned files changed (the DSP's and the editor's extra files are
  children declared with `#[path]` from `spatium.rs` and `views/fx/space.rs`).
- The engine's delay compensation is not needed (latency 0).
- `EffectKind::Space.params()` is a `OnceLock` like the EQ's; its first call is on the UI
  thread (`EffectSpec::new`), before the audio thread reads it.
