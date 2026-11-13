# Cribrum — the auto filter

*Effect kind `AutoFilter`. References: FL Studio's Love Philter and Fruity Filter, Ableton's
Auto Filter, FabFilter Volcano, Soundtoys FilterFreak (ADR 0130, provisional number).*

*Cribrum* is Latin for a sieve. Cribrum is two analog-modelled filters, in series or in
parallel, **moved in time with the song** by three sources summed: an **LFO** (on the song
from 1/64 to 8 bars, dotted and triplet, or free in Hz; seven waves), an **envelope
follower** (on the input or a sidechain) and a **drawn curve** (sixteen points, the
automation editor's segment shapes). Each source has its own depth onto the cutoff (in
octaves), the resonance and the vowel.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/cribrum.rs` (the globals and sixteen curve points, sparse) |
| DSP | `crates/cantus-dsp/src/cribrum.rs`, `cribrum_filter.rs` (the models in their modes, the vowels), `cribrum_mod.rs` (the waves, the follower's map, the curve's smoothing), `cribrum_response.rs` (what the editor draws); tests in `cribrum_tests.rs` |
| Engine glue | `crates/cantus-engine/src/fx/cribrum.rs` |
| Editor | `crates/cantus-ui/src/views/fx/cribrum.rs`, `cribrum_display.rs` (what passes), `cribrum_move.rs` (the movement card), `cribrum_curve.rs` (the curve on the lane) |
| Face | `crates/cantus-ui/src/kit/skin/side.rs` (`CRIBRUM`) |
| Strings | `crates/cantus-ui/locales/fx/cribrum.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/autofilter.toml` (sixteen; *Bar sweep* is the starting preset) |
| Tests | `crates/cantus-engine/tests/fx_cribrum.rs`, `rt_memory::cribrum_never_allocates`, the family's contract; `crates/cantus-ui/tests/cribrum_e2e.rs`; the CLAP/VST3 tests over every descriptor |
| Exported | `Cantus Cribrum` (CLAP `com.github.lalmazan.cantus.cribrum`); its curve points grouped as *Curve point 1 … 16* for hosts |

## What the references do well, and what people complain about

- **Love Philter** is the most flexible (eight filter units, envelopes per parameter, an XY
  pad) — and the hardest to read: its envelopes hide behind tabs, nothing shows what the
  filters do to the sound.
- **Volcano** draws its filters beautifully over the spectrum and moves them with any of its
  modulators; its modulation is powerful but takes a drag-and-drop to set up.
- **Ableton's Auto Filter** is quick (one filter, an LFO, an envelope, the response drawn)
  but has one filter, and its sync is the host's only.
- **FilterFreak** sounds analog and has a rhythm editor, but no picture of the response.
- What users ask: filters that **sound analog** (resonance that sings in tune), movement **on
  the song's grid** that never drifts or clicks, two filters, a vowel, a picture that shows
  what is heard, presets that sound at once.

Cribrum's filters are the synths' own zero-delay-feedback models (the same solver, so the
same curve and the same self-oscillation, ±5 cents measured); its LFO and curve are
functions of the song position (a render from bar 5 is bar 5 of the whole render, to the
bit); a step of the curve is smoothed as a function of the song too (it never clicks, and it
costs nothing at rest); the display draws the music **sieved** — what passes lit, what is held
back dim — under both responses as they move.

## Parameters (keys are forever)

The mix starts at 0 and the oversampling off, so Cribrum inserted at its defaults returns its
input to the bit with no latency. An insert from the picker loads *Bar sweep* (ADR 0071).

| Key | Range | Default | Meaning |
|---|---|---|---|
| `mix` | 0 … 100 % | 0 | dry/wet (straight crossfade; the dry delayed to the oversampling's latency) |
| `out` | −24 … +24 dB | 0 | output gain |
| `route` | serial / parallel | serial | A then B, or their mean |
| `spread` | 0 … 2 oct | 0 | the left cutoff below, the right above |
| `os` | 1× / 2× / 4× | 1× | oversampling of the filters (minimum phase: a sample or two of latency, reported) |
| `a.model`, `b.model` | SVF, ladder, OTA cascade, diode ladder, Sallen-Key | SVF | the synths' five models |
| `a.mode`, `b.mode` | low, band, high, notch, peak, vowel | low | where the model gives it (below) |
| `a.slope`, `b.slope` | 12 / 24 dB/oct | 12 | |
| `a.cut`, `b.cut` | 20 Hz … 20 kHz | 1 kHz | cutoff; in vowel mode the formants' size (1 kHz: a tenor's) |
| `a.res`, `b.res` | 0 … 100 % | 25 % | resonance; every model sings from 95 % |
| `a.drive`, `b.drive` | 0 … 24 dB | 0 | into the model's nonlinearities, half given back |
| `a.vowel`, `b.vowel` | A … U | A | the vowel, in vowel mode |
| `b.on` | off / on | off | the second filter |
| `b.track` | −100 … 100 % | 100 % | how much of the movement B takes (negative: the other way) |
| `lfo.sync`, `lfo.div`, `lfo.rate` | on the song 1/64 … 8/1, or 0.02 … 20 Hz | synced, 1/4 | |
| `lfo.shape` | sine, triangle, square, saw up, saw down, sample & hold, drift | sine | |
| `lfo.edge` | 0 … 100 % | 30 % | the soft edge of the stepped waves |
| `lfo.phase`, `lfo.stereo` | 0 … 360°, 0 … 180° | 0 | where it starts; the right channel ahead |
| `env.key` | input / sidechain | input | what the follower hears |
| `env.att`, `env.rel` | 0.1 … 500 ms, 1 … 2 000 ms | 5, 150 ms | |
| `env.gain` | −24 … +24 dB | 0 | sensitivity; the movement is the level from −48 to 0 dBFS |
| `cur.div` | 1/64 … 8/1 | 1/1 | the curve's length, on the song |
| `cur.smooth` | 0 … 50 ms | 2 ms | a Hann window over the curve, centred: a step becomes a slope |
| `{lfo,env,cur}.cut` | −6 … +6 oct | 0 | the source's depth onto the cutoffs |
| `{lfo,env,cur}.res` | −100 … +100 % | 0 | …onto the resonances |
| `{lfo,env,cur}.vowel` | −4 … +4 | 0 | …onto the vowels |
| `c{n}.on/x/y/t/s` | | c0, c1: a fall each cycle | the curve's points (n = 0 … 15) |

### What each model gives

| Model | Low | Band | High | Notch | Peak | 24 dB/oct |
|---|---|---|---|---|---|---|
| SVF (state-variable) | ✓ | ✓ | ✓ | ✓ | ✓ | low and high (a second, Butterworth stage) |
| Ladder, OTA cascade | ✓ | ✓ | ✓ | ✓ | band | ✓ (the stages mixed, as the Xpander did) |
| Diode ladder | ✓ | — | — | — | — | ✓ |
| Sallen-Key | ✓ | — | — | — | — | — |

A response a model does not give is greyed in the editor; set by automation or a preset, the
model plays its low-pass (and the display draws that).

## The editor

- **What passes** (the display): the spectrum as grains, lit up to what comes out, with a cap
  where the input reached; the response of both filters as they move (as set, dashed);
  a handle per filter. **Drag a handle** across for the cutoff, up and down for the
  resonance (Shift: finer); **double-click** puts both back. The chosen filter's handle sits
  on a **rail**: how far its movement can take it, the stretch it has moved now drawn solid.
  In vowel mode the vowels are named across the top, the one sounding lit.
- **Movement**: the LFO, the follower and the curve as keys (each with its depth onto the
  cutoff and a live bar); the chosen one drawn on the lane in time with the song, its own
  controls in one group and its depths (cutoff, resonance, vowel) in the other, with the
  cutoff's reach in Hz. The curve is drawn on the lane as Fluctus's is: drag a point (Shift:
  off the grid), drag a segment to bend it, double-click to add or remove a point; the
  segment shape of the chosen point beside it.
- **Filter A / B**: one card, A or B chosen on its keys or by a click on a handle; model,
  slope, the responses as keys with their curves; cutoff (a size up), resonance, drive, vowel,
  and for B its *Follows* and its on/off lamp.
- **Output**: routing, oversampling, spread, mix (a size up), output.

## Measured

- Each model's low-pass is the synths' filter to the bit; the response drawn and the response
  heard agree within 0.1 dB over every model, mode, slope and resonance.
- Self-oscillation at full resonance, 110 Hz … 2.5 kHz, at 1× and 2×: within ±4.5 cents
  (the OTA cascade's −4.5 the worst; the SVF and the ladder within 0.2).
- A driven ladder (24 dB of drive, a 5 kHz sine at −1 dBFS): the aliasing floor is −16.7 dB at
  1×, −23.2 dB at 2×, −34.2 dB at 4×.
- The vowels on pink-ish noise sit within ±2 dB of the dry level.
- At rest after sound in about 1.5 s at its heaviest settings; exactly as new then.

## In another DAW

`Cantus Cribrum` (CLAP and VST3) is the same unit and the same editor; its sync follows the
host's transport. Its sixteen curve points are grouped *Curve point 1 … 16*.
