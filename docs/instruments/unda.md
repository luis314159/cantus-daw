# Unda — the wavetable synth (`InstrumentKind::Wavetable`)

*Owner: Unda's builder (round 17, ADR 0023). Benchmark: Serum 2, Pigments 7, Phase Plant,
Vital (`research/instruments/wavetable.md`). Firmus is untouched and renders bit-identically;
Unda is a new kind.*

## What it is

Three wavetable oscillators, each with up to sixteen unison lanes and two warp slots, a sub
and a noise source, two filters that run in series, in parallel or anywhere between, four
envelopes, eight LFOs with drawable shapes, the shared eight macros and 32-slot matrix, and
ten factory tables generated in code. Tables import from WAV (Serum's `clm `, u-he, Surge,
`smpl` and `cue ` markers, or none) and from Surge's `.wt`. Everything is deterministic:
the same notes give the same samples at every block size, live or bounced.

## Files

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/inst/wavetable.rs` |
| DSP | `crates/cantus-dsp/src/unda.rs` (settings, targets) and its children: `unda_table.rs` (a table and its mip levels), `unda_factory.rs` (the factory tables), `unda_osc.rs` (unison, warps, reads), `unda_filter.rs` (the ladder, the SVF slot, the response the editor draws), `unda_mod.rs` (envelopes, LFOs, the seeded hash), `unda_voice.rs` + `unda_voice_run.rs` (a voice), `unda_synth.rs` (voices, grid, decimator), `unda_decim.rs` (the IIR half-band), `unda_tests.rs` |
| Engine | `crates/cantus-engine/src/inst/wavetable.rs` (`settings(spec, assets)`, the table registry, telemetry) |
| Editor | `crates/cantus-ui/src/views/inst/wavetable.rs` and `wavetable_{osc,filter,mod,import,presets}.rs` |
| Strings | `crates/cantus-ui/locales/inst/wavetable.{en,es}.txt` |
| Tests | the DSP module's own, `crates/cantus-engine/tests/inst_wavetable.rs`, `crates/cantus-ui/tests/inst_wavetable.rs` |

One shared file changed, in its own `SHARED:` commit: `cantus-engine/src/inst/mod.rs` gained
`prepare_assets`, called at the top of `units_for` (see *Imported tables*).

## The engine, and why it is built that way

**Signal flow, per voice, at the oversampled rate** (2× by default): oscillators 1–3, sub,
noise → each to F1, F2, both, or out → F1 → (1 − routing) into F2 → out = F2 + routing·F1 +
direct → the amplifier (envelope 1 × velocity × gain). The voices are summed *at the
oversampled rate* and **one decimator per instrument** brings the sum back (decimation is
linear, so summing first is exact and costs one filter instead of one per voice).

**Tables.** A frame is one cycle of 2048 samples (harmonics 1…1023), up to 256 frames.
Each frame is stored as eleven octave mip levels made by truncating its spectrum and one
inverse FFT per level, so every level is exactly band-limited. The level is chosen per lane
on the control grid from its pitch — the lowest whose top harmonic stays under the
oversampled Nyquist — and crossfaded into the next over the top quarter octave, so a
glide never clicks. The position crossfades between adjacent frames.

- *What the research got wrong, and the fix.* The report proposed 4-point cubic reads on
  levels stored 2–16× oversampled. Measured, that aliased at −59 dB for a saw at A1 and
  −87 dB at C7 at 2×: a read's images sit around multiples of the table's own rate and
  fold back from multiples of the oversampled rate — into the audio band, whatever the
  decimator does. So the less-oversampled levels (0–6) are read with a **6-point, 5th-order
  Lagrange** interpolator (images 30–50 dB lower at equal oversampling), levels 7 and up
  (≥ 32×) with the Hermite cubic, and levels 0 and 1 are stored at 8192 and 4096 samples
  (8×). Memory: ~77 KB a frame, 20 MB for a full 256-frame table.
- *Phases* are u32 accumulators: exact, wrap for free, 11 µHz of resolution. Linear FM
  moves the increment itself, through zero.

**Unison.** Up to 16 lanes per oscillator. Detune in cents to the outermost lane, spread by
a law: *Super* (Szabó's measured JP-8000 spacing — ±0.11, ±0.063, ±0.020 of the outer
detune for seven voices — as a curve), linear, power (clustered), or seeded random. Blend is
equal-power (0 the centre lane(s) alone, ½ all equal, 1 the sides alone) and the total is
held at one lane's power (uncorrelated lanes add in power, so 1/√N). Lanes are panned in
mirrored pairs to opposite sides, alternating, so a stack is wide and its mono sum loses at
most 3 dB. Stacks add octaves, fifths or chords across the lanes; *table spread* moves each
lane's position. At one lane every unison setting is inert (tested bit-exact).

**Warps**, two per oscillator, amount 0 skipped entirely (so the identity by construction):
Sync (1–16×), Bend ±, Asym ±, PWM, Mirror, Flip, Quantize, Fold, Soft clip, Hard clip, and
FM (linear through zero), PM, AM, Ring from oscillator 1–3, the sub, the noise or itself.
Phase warps apply in slot order before the read, output warps after. An oscillator that is
off still runs when another one's FM reads it (a silent modulator, as in 2-operator FM).

**Engines (P1): Partes and Grana.** Each oscillator can run another engine
(`osc{n}.engine`), with the same pitch, level, pan, route, filters and modulation:

- **Partes** (additive): a bank of up to 256 sine partials, each a recursive rotator
  (four multiplies a sample), renormalised on the control grid. Its spectrum *is* the
  current table frame (so at stretch 0 it plays the table, phases included — a test holds
  the correlation above 0.99), shaped by a tilt (dB/oct), an odd/even balance and a
  **stretch** that moves partial *k* to `k^(1+s)` × the fundamental: bells and stretched
  pianos no table can hold. Partials above 18 kHz fade out and above 20 kHz are never
  computed, so the cost follows the note, not the knob. Rotation factors are recomputed
  only when the pitch or stretch moves.
- **Grana** (granular): windowed grains (Hann, Gauss, Tukey, Expodec) of the imported
  sample, or — with no file — of the oscillator's own table, played at the note's pitch.
  Density (grains/s, ±25 % seeded jitter), length, spray, pitch jitter, reverse
  probability and pan spread; the level is scaled by the overlap. Grains are scheduled on
  the sample clock from the note and every draw is a hash of (note, grain number,
  purpose), so the cloud is identical at every block size (tested). A fixed pool of 32
  per voice and oscillator; a spawn is skipped when it is full (deterministic,
  click-free). The table source reads mip-mapped levels; a sample is read with the cubic
  interpolator (transposing a sample up aliases — see *Not done*).

Warps and unison belong to the wavetable engine (the panel hides them for the others).

**Filters.** The shared SVF (`cantus_dsp::filter`) in low 12/24, band, high 12/24 and notch;
and Unda's own **zero-delay-feedback ladder** (Zavalishin ch. 5): four trapezoidal
one-poles, the loop solved exactly for its linear part, a Padé `tanh` on the loop's input
with unity small-signal gain (drive moves where it bends, not the level), resonance to
self-oscillation at 1, the pass band held up by `1 + k/2`, and low 24/12, band and high 24
by mode mixing. Coefficients on the control grid. Key tracking, a stereo cutoff offset, a
mix. The editor draws the transfer function the discretisation realises
(`filter::magnitude_db`), and a test holds it within 0.5 dB of the measured response.

**Modulation.** Every 16 base samples counted from the start of the stream (and once at
each note-on), each voice evaluates its sources — envelopes, LFOs, velocity, release
velocity, key, a seeded random, the three MPE axes, and the global macros, wheel,
aftertouch, bend, expression and sustain — through the shared matrix into 85 targets
(`unda::target`). A route's depth is a fraction of its target's range, in octaves for the
log ones (cutoff, rates, times). Pitch and levels ramp per sample in between. A route at
depth 0 or switched off is never evaluated.

**Envelopes** are DAHDSR with curved attack, decay and release (`(e^{kt} − 1)/(e^k − 1)`,
kept by a multiplication per sample, no `exp`), exact at the segment ends: a release lands
on exactly 0, so a finished voice is exactly silent. **LFOs**: sine, triangle, saws,
square, sample & hold, smooth random, or a drawn shape (up to 16 points, stored as text in
`lfo{n}.shape`); restart per note, free running, or one shot; delay, rise, smoothing.

**Determinism.** Every random number is a SplitMix64 hash of (patch seed, note id, key,
purpose, lane) — start phases, the random detune law, drift, S&H, noise. Nothing reads a
clock or a global counter. The decimator's tail is gated on the grid (never at a block
edge), so silence arrives at the same sample at every block size.

**Imported tables.** `osc{n}.table` is an asset key: the engine decodes the WAV before the
snapshot is sent. Building a table's mip levels allocates and takes milliseconds, so it
cannot happen in `configure` on the audio thread; it happens in `prepare`, which
`inst::units_for` calls off the audio thread before every snapshot (the backend and the
offline renderer both pass through it), and the table goes into a process-wide registry
keyed by the decoded audio and the frame size. `configure` only looks it up — lock-free
(`OnceLock` slots), no allocation — so the table is there before the first note and a
bounce equals playback. Like the sample cache, the registry never evicts, so the audio
thread never frees a table. A table missing from the registry, or a file that cannot be
read, is silence.

**Latency**: none reported (the IIR half-band's group delay is about two base samples at
low frequencies). **Allocation**: none after `new` on the audio thread.

## The parameters

Sparse; keys are forever. Units in brackets.

| Group | Keys |
|---|---|
| Global | `gain` [dB −48…12], `quality` [1×/2×/4×], `flt.routing` [serial 0 … parallel 1] |
| Voice | `voice.poly` [1–32], `voice.mode` [poly/mono/legato], `voice.prio`, `voice.glide` [s], `voice.glide_mode` [legato/always], `voice.bend` [st], `voice.vel` [%], `voice.drift` [% of ±5 ct, ±⅙ oct], `voice.seed` |
| Partes / Grana `osc{1,2,3}.` | `engine` [wavetable/Partes/Grana]; `partials` [1–256], `tilt` [dB/oct], `oddeven` [−1 odd … +1 even], `stretch` [±10 %]; `density` [grains/s], `glen` [s], `spray` [s], `gpitch` [ct], `greverse` [%], `gwin` |
| Oscillator `osc{1,2,3}.` | `on`, `wt` (factory table), `frame` [samples per cycle of an import], `pos` [0…1, shown as frames], `oct`, `semi` [st], `fine` [ct], `pitch` [st, the modulation target], `phase` [°], `rand` [%], `level` [dB], `pan`, `route`, `uni` [1–16], `detune` [ct], `ucurve`, `blend`, `width`, `stack`, `spread`, `warp{1,2}`, `warp{1,2}.amt` [%], `warp{1,2}.src`; text `table` (a file), `name` |
| Sub / noise | `sub.on`, `sub.wave`, `sub.oct`, `sub.level`, `sub.pan`, `sub.route`; `noise.on`, `noise.color` [−1 darker … +1 brighter], `noise.level`, `noise.pan`, `noise.width`, `noise.route` |
| Filter `flt{1,2}.` | `on`, `type`, `cutoff` [Hz], `res` [%], `drive` [dB], `key` [% of 1 oct/oct], `stereo` [oct], `mix` |
| Envelope `env{1–4}.` | `delay`, `a`, `h`, `d`, `r` [s], `s` [%], `ac`, `dc`, `rc` [curve] |
| LFO `lfo{1–8}.` | `wave`, `rate` [Hz], `mode`, `phase` [°], `delay`, `rise` [s], `smooth`; text `shape` |

## The editor

The Edit view has three pages. `CANTUS_SHOOT_VIEW=edit|filters|modulation|lfo|matrix
cargo run -p cantus-ui --example shoot -- <dir> 1 wavetable` photographs each (the default
is the Play view), with the Glass Choir preset and a playing engine's telemetry.

- **Oscillators page**: three oscillator cards (power, the table menu with ‹ ›, import, the table
  as a stack of frames — the playing one gold and filled — or, with the 2D chip, the current
  cycle with its warps drawn live; drag up and down on it to scrub), tuning, route and warp
  chips, the unison law in a gear chip that prints it, and ten knobs; the engine menu
  (wavetable, Partes — its partials drawn as bars where they sound — or Grana — the source
  with the position and the spray's reach); sub and noise.
- **Filters page**: the two filters with large curves (drag across for the cutoff, up for
  the resonance), the reach of their modulation as a band and the newest voice's
  modulated curve as a gold ghost; the serial ↔ parallel knob between them.
- **Modulation page**: ENV 1–4 and LFO 1–8 tabs with the number of targets each drives; the
  envelope drawn with its curves and three handles (A, D·S, R); the LFO drawn (and drawable);
  each source's live value as a gold tick; the voice card with the lanes running.
- **Play view**: oscillator 1's table large and alive, and the nine knobs a preset is
  played with; the shared macro bar and keyboard below.
- Unda's hue is a teal (darker on light themes); gold is "now". Every value prints in its
  unit; knobs carry modulation rings and accept dropped sources.

**Presets** (15): Glass Choir, Hyper Saw, Reese Bass, Pluck Keys, Formant Talker, Sync
Scream, Digital Bells, Drawbar Organ, Sub Drop, Warp Pad, PWM Strings, Noise Sweep,
Stretched Bells (Partes), Grain Cloud (Grana), FM Bass — each with tags, its macros named
and connected, and a loud chord under 0 dBFS (tested).

## What the tests hold

- **DSP** (`unda_tests.rs`): the decimator passes 100 Hz–18 kHz within 0.1 dB and rejects
  28–47 kHz below −96 dB (analytically −152 dB at 28 kHz); a table keeps its peak at 1 and
  no level holds a harmonic above its top; audio becomes a table frame by frame with its
  levels kept; the init patch plays and is then exactly silent; every block size (1, 7, 64,
  512, 1000) gives the same samples for a patch with unison, a warp and the ladder, and for
  one with Partes and Grana; an envelope's segments land within two samples of their times
  and its release on exactly 0; the filter curve the editor draws is within 0.5 dB of the
  measured response for eight types; unison holds its level within ±1 dB from 1 to 16
  lanes and its mono sum within 3 dB; Partes at stretch 0 correlates with the table above
  0.99 and stretch moves the second partial off the harmonic.
- **Engine** (`inst_wavetable.rs`): a busy patch (two oscillators in unison with warps, the
  sub, noise, both filters, three routes, glide, drift) is identical at every block size;
  two renders are identical and another seed changes them; **every parameter set
  explicitly to its default changes no sample**; inert settings (a warp mode at amount 0,
  unison settings at one lane, a route at depth 0, a muted route, a filter at mix 0) are
  bit-identical to their absence; pitch across keys 24–105; the alias floor at A1, C7 and
  C8 at 1×, 2× and 4×; the init patch's headroom; a macro at 0 moves nothing and at 1 moves
  the cutoff; an imported WAV plays what it holds and a missing one is silence; Grana on an
  imported sample with Partes beside it is identical at every block size; CPU against
  Firmus measured in the same run.
- **Editor** (`crates/cantus-ui/tests/inst_wavetable.rs`): it opens in Play, Edit (all
  three pages, envelope and LFO) and Matrix and paints real numbers; the Partes and Grana
  panels show their own pictures and knobs; the demo is alive; a drag is
  one undo step, including the filter's two-value drag; every preset names only real keys,
  connects only modulatable targets, names every macro it uses, and applies exactly; every
  preset's loud chord stays under 0 dBFS; a WAV imports in one undo step with its frame
  size; every format (`clm `, `uhWT`, `srge`, `smpl`, `cue `, plain, single cycle) says its
  frame size and bad files fail with a named reason; Surge `.wt` (float, 16-bit, 15-bit)
  becomes a WAV the engine reads, and a `.wt` sample is refused; every English string has
  its Spanish twin.
- The family's contract (`inst_contract.rs`) holds with Unda making sound.

## Measured

Printed by `cargo test -p cantus-engine --test inst_wavetable -- --nocapture` (and the
presets' peaks by the editor's tests), on this shared machine with about a dozen builds
running.

- **Pitch**: worst error 0.00024 cents over keys 24–105 (a sine, zero crossings over 1 s).
- **Aliasing** — energy between the harmonics below 20 kHz relative to the fundamental,
  a full saw (1023 harmonics), Kaiser-windowed 65 536-point FFT:

  | Quality | A1 (55 Hz) | C7 (2093 Hz) | C8 (4186 Hz) |
  |---|---|---|---|
  | 1× | −100.6 dB | −96.5 dB | −102.3 dB |
  | 2× (default) | −102.5 dB | −102.8 dB | −96.8 dB |
  | 4× | −102.2 dB | −102.9 dB | −103.2 dB |

  The research's criteria were ≤ −96 dB at 2× and ≤ −80 dB at 1×; the first version (the
  research's 4-point reads on 2–16× levels) measured −59 dB at A1 and −87 dB at C7.
- **Decimator**: −152 dB at 28 kHz (analytic), below −96 dB measured from 28 to 47 kHz;
  the pass band within 0.1 dB to 18 kHz at 96 kHz.
- **Headroom**: one init note peaks at −17.2 dBFS, an eight-note init chord at velocity
  127 at −6.1 dBFS; the presets' loud chords from −6.5 to −39 dBFS.
- **CPU per voice**, best of three renders, relative to Firmus measured in the same run
  (Firmus read 3.6 % of a core per voice at that moment, about ten times its cost on an
  idle machine — so the absolute numbers here are inflated tenfold): init 0.9× Firmus;
  supersaw (7 lanes, ladder) 3.4×; pad (2 × 7 lanes, SVF, an LFO on the position) 7.8×;
  worst (3 × 16 lanes, two ladders, 4×) 15×. The single-threaded DSP bench
  (`unda_speed`, release) read, in ns per voice-sample: init 172, seven lanes 690, the
  ladder 393, 1× 100 — against Firmus's 168 in the same run. A lane-sample costs about
  45 ns there (roughly 10 ns on an idle core).
- **Memory**: a table frame ≈ 77 KB (mips) + 8 KB (partials); the ten factory tables
  and the sub's (250 frames) ≈ 21 MB once per process; an imported 256-frame table ≈ 22 MB, once per
  distinct file and frame size.

## Not done

- **A wavetable editor** (draw tools, the FFT bar editor, frame strip, recipes, morph
  modes, export to WAV/`.wt`) — P0 item 6 in the research. Tables come from the factory
  or from import; the stored recipe and one-undo-history table editing (P1.1) wait for it.
- **Band-limited hard sync** (a minBLEP at the reset): Sync relies on oversampling, so a
  sync sweep aliases audibly at 2× (4× helps). Waveshaping warps have no ADAA either.
- **Comb, formant, diode, MS-20 filters**; the SVF is the shared one, so its resonance
  stops short of self-oscillation (the ladder does self-oscillate).
- **Tempo sync** (LFO rates in note values, envelope times in beats): the instrument
  contract gives a unit no tempo or transport. For the same reason a free-running LFO's
  phase counts from the unit's last reset, not from the transport, so a bounce that
  starts mid-song differs from the same bars of a full bounce while a free LFO is routed.
- **Chaos LFOs** (Lorenz, Rössler), per-LFO bipolar/unipolar switch, the LFO's own curve
  editor with curvature per segment (the drawn shape is straight lines between points).
- **Import of `.vitaltable`**, folders of single cycles, `sound.txt` overrides, and the
  resample-a-clip-into-a-table workflow (P1.4).
- **Aliasing indicator and measured CPU in the header** (P1.5/P1.6): the lanes running
  are reported; time per block is not measured on the audio thread.
- **Grana**: a transposed-up sample aliases (no mip-mapped source yet); the density modes
  "sync" and "count", grain start quick-fill and a loop mode are not there.
- **Partes**: resynthesis from a file or a clip, a spectral filter, blur and pluck.
- **SIMD across unison lanes**: the lane loop is scalar; the research's CPU targets
  (≤ 0.05 % of a core for an init voice) are not met on this machine (see *Measured*).
- Stereo files use the left channel only; the right is not offered to another oscillator.
