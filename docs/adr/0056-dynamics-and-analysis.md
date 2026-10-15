# ADR 0056 — Dynamics and analysis: Tonsor, Impetus, Discrimen, Speculum; a follower to share; latencies that move; an analyser's own stream

**Status:** Accepted · **Date:** 2026-10-15 · **Extends:** ADR 0017 (the plug-in family), ADR 0018 (the contract, delay compensation), ADR 0020 (the exported plug-ins), ADR 0031 (exact rest)

## Context

The owner asked (BACKLOG §3b) for a clipper (FL's Soft Clipper, StandardCLIP, KClip:
oversampled, true-peak aware, a clip meter, a null at 0 dB), a transient processor (SPL's
Transient Designer: attack and sustain on level, not a threshold; split band; sidechain), a
transient splitter (two parts that sum back to the input, a zero-latency mode), and an
analyser in the class of Wave Candy and SPAN (oscilloscope, spectrum, spectrogram,
vectorscope, correlation, loudness to the standards, resizable, docked or floating). Four
things had to be settled first:

- **Nulls with oversampling.** Oversampling filters are not an identity: a clipper that
  oversamples changes a signal it never clips, so "at 0 dBFS with no overs it is a null"
  fails by construction.
- **A latency that moves.** The oversampling and the look-aheads set a unit's latency, and
  they are parameters — automation (and the leaders' modulators) can switch them while the
  song plays; delay compensation was planned only when a snapshot arrived.
- **Detection without a threshold** must read the same at any level, and must not read a
  steady low tone's ripple as attacks.
- **An analyser needs every frame of its own slot, stereo**, several at once; the existing
  analyser tap carries one effect's mono input/output pairs for the one editor that is open.

## Decisions

**Oversample the residual.** `dynfx_os`: a polyphase Kaiser-sinc interpolator whose phase 0
is a pure delay, and a decimator that band-limits **only what the curve changed**
(`shape(u) − u`), added to the input delayed by the path's latency. A sample the curve leaves
alone comes out bit for bit at 2 … 16×; the filters' ripple touches only the distortion.
(Terminus's clipper does the same at 4×; this is the general form.) The decimator's window ends
at the newest sample's point 0, so the residual lands on the dry line's own sample (review 1:
it was (F − 1)/F of a sample early and an oversampled clip passed its ceiling by 2.26 dB;
now within 0.034 dB). Between two factors above 1× (same latency) only the decimator's
history starts over.

**A true-peak ceiling is a guard, not a hope.** After the curve, the waveform between the
samples is read as the meters read it (`terminus::os::TruePeak`); the gain each sample needs
is held over ±20 samples and smoothed by a ±16 moving mean (so it is no more than any near
sample needs and never steps), aimed 0.1 dB under the ceiling — the detector's measured
accuracy against a 32-point reconstruction. Measured: a plain clip leaves +1.88 dBTP; the
guard −0.045 … −0.072 dBTP.

**A latency is a configuration's, and the engine follows it.** Every unit's latency is a
function of its switches (oversampling, guard, look-ahead), never of a level. The processor
keeps a signature of every insert's and instrument's latency from the last plan and checks
it before each pass (after the pass's automation is applied): when it moved, compensation is
planned again, as an edit would plan it (`Processor::replan_if_latency_moved`). A test
switches Tonsor from 1× to 16× and back by automation with the bass on another strip: the
whole mix is the reference later by the new latency, to the bit, once the lines have
filled. **A line keeps what it holds when its delay changes** (`DelayLine::retime`: the
read point moves — shorter skips ahead, longer waits only the difference), so a switch costs
at most the change, never a dropout of the path's whole delay (review 1). The signature folds
every insert, the master's one by one. This closes the gap for every unit with such a switch (Fervor's oversampling,
Sibilus's look-ahead), not only these.

**The follower is standalone** (`dynfx_follow`): one-pole attack and release on a rectified
level (time constants), peak or RMS, homogeneous — the envelope of `a·x` is `a` times that of
`x`, to the bit for powers of two — and flushed to +0.0 after 64 silent samples once under
−120 dB, so a unit built on it rests exactly. Anything can take it (the leaders' follower
modulator).

**Transients from level differences** (`dynfx_transient`): a 10 ms running maximum first (a
steady 55 Hz tone reads under 0.05 dB), then an attack pair (0.3 ms against *speed*, one
release) and a sustain pair (one fast attack, *length*/8 against *length*); their ratios in
dB are the readings. Ratios, so level-independent (a test holds quiet = loud to the bit).
The split band is a Butterworth SVF's low or high pass, 90° from the rest at the crossover,
so a boost or cut never cancels.

**An exact split** (Discrimen): the larger part by a product, the other by an exact
subtraction (Sterbenz), so transient + body is the input bit for bit for any mask; ±0 keeps
its sign. Zero latency, or a 2 ms look-ahead (a running maximum of the mask over the
samples ahead, the audio delayed by as much): 75 % of an onset's first 0.25 ms in the
transient part at zero latency, 100 % with the look-ahead.

**Meters that lose nothing.** Telemetry is read once a UI frame and carries the last block:
the family keeps peaks for 50 ms of audio (`dynfx_meter::Hold`), running counts (samples,
clips, overs) the editor turns into rates over the last second of audio, and **columns** of
512 samples counted from the stream in a ring of sixteen with a running count, from which
the editor appends every column it has not seen (`new_columns`). A non-finite level is never
kept, and a sample that is not a number is read as silence by the follower, the detector
and every filter or envelope of the units (review 1: one NaN had locked them for good).

**Speculum has its own stream.** Eight scope rings (lock-free, 1 024 chunks of 64 stereo
frames filled across passes, so 65 536 frames whatever the passes) are made when the engine
starts. Each chunk carries its owner (a ring handed to another Speculum drops the last one's;
review 2: a ring whose watch changed — released, handed over, or the same Speculum watched
again — starts afresh in the engine, no chunk or loss of an earlier watch carried on),
the frames a full ring lost before it (the editor shows the gap), and the song's position
**as heard at the slot** — the transport's less the compensation the slot waits
(`Processor::scope_delays`). A master Speculum hears the mix with the chain off or bypassed.
`EngineHandle::watch_scope` gives a ring to a Speculum (asked every UI frame it is
measured); `release_unwatched_scopes` (once a frame) frees the rest; the strip and master
chains copy a watched slot's frames after the insert (eight comparisons for one nobody
watches; no allocation, nothing waits). Measurements run on the editor's thread
(`cantus_dsp::speculum::Analysis`), counted in samples of the stream: loudness is the house
R128 meter, its integrated reading and range worked out every 200 ms of audio, not every
callback. A docked Speculum is measured every frame whether its tab shows or not. In the
exported plug-in, Speculum's tap carries its frames (left, right) and `FxHost::take_stereo`
reads them in both hosts; the song's position travels with them (review 2): the tap counts
the frames it took, the block's end is published with that count under a sequence lock
(`Shared::clock_of_drained`), and the editor dates the newest frame it drained — not the
host's newest block, which could be ahead of the frames or behind them. In the overview of
a short display (a docked tab) the loudness panel takes a column of the whole height and
keeps its rows by rank — momentary, short-term, integrated, the peak and RMS rows, the
gaps, then the true peak, the range and the scale as room allows.

**Speculum is docked through the workspace's own device tabs** (`DeviceWindow`): resizable,
split, torn out; the floating editor docks itself from its ⋯ menu.

## Consequences

- New: `cantus_dsp::{dynfx_follow, dynfx_meter, dynfx_os, dynfx_transient}`, the four DSP
  modules; `cantus_engine::fx::speculum::{ScopeChunk, MAX_SCOPES, …}`,
  `EngineMessage::WatchScopes`, `EngineHandle::{watch_scope, drain_scope,
  release_unwatched_scopes, watched_scopes}`, `Processor::{attach_scopes,
  replan_if_latency_moved}`; `FxHost::take_stereo`, `StereoTake`; `views::fx::{dynfx,
  speculum::{dock, docked, measured}}`; `examples/shoot_dynfx.rs`.
- Speculum's table has no `out` (a pass-through): its never-shipped id line was removed;
  the others keep `out`. Plug-in ids blessed (appended).
- No golden hash moved; `DSP_VERSION` unchanged (new units are new sounds).
- A minimised floating Speculum is not measured while minimised (a docked one always is).

## What the tests hold

DSP (`<stem>_tests.rs`, `dynfx_*`): the curves, the residual null at every factor, the
true-peak ceiling against the reference, aliasing at 1× against 16×, latencies where an
impulse comes out; the follower's times, homogeneity and rest; the detector on onsets,
tails, steady tones and levels; the exact split for any mask; the EBU Tech 3341/3342
vectors, the spectrum's level, slope and hold, the scope on the beat and on a crossing,
correlation, non-finite samples. Engine (`fx_<stem>.rs`): nulls at the defaults through the
whole engine, latency as a later render, block sizes 1 … 4 096, live = offline, a key, an
automated latency keeping the mix in line, and Speculum's stream (every frame, several at
once, released, no allocation). UI (`fx_<stem>.rs`, `speculum_budget.rs`): the words, one
undo step per choice, the drawing against the DSP, no word cut in either language, and
three analysers at once in the real window losing no frame. The family's contract and the
CLAP/VST3 export (bit-identical) for all four.
