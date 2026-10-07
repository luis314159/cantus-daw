# Round 21 — at rest: the exact predicate, unit by unit

*Builder: at-rest (`r21/at-rest`). ADR 0031 §5.1 and risk 1; `docs/perf/plan.md` Phase 0.*

## What was asked, and what "at rest" means here

`FxUnit::at_rest`, `InstrumentUnit::at_rest` and `EffectUnit::at_rest` (default `false`),
implemented for every native unit by reading its real state, and the tests that make the
graph builder's per-node segmentation and suspension safe.

Reading the units showed that "the state equals a freshly reset unit's" is not one property
but two, and that a third method is needed for either to be exact. The contract, as written
on the traits (`crates/cantus-engine/src/fx/mod.rs`, `inst/mod.rs`):

1. **Silence in, silence out, still at rest.** With silent input (silent key, no note) the
   unit outputs exact zeros (+0.0) and stays at rest.
2. **Suspension is exact.** Not calling `process` for those samples — telling the unit with
   the new **`skip(frames)`** — changes no sample of what follows, *whatever input and
   whatever parameter changes come next*.
3. **As good as new.** It renders exactly what a freshly reset (or new) unit with the same
   configuration renders, once that unit is `skip`ped as far as this one has run — for any
   input, **as long as the parameters stay**.

`skip` exists because several units count samples and the count is in their output's
bits: the delay's write position (its fractional read head is worked out from the absolute
position — skipping 300 samples without moving it changed bits in the first twin test), the
grids that Latitudo, Fervor, Multiplex, Aequalis, Sibilus and the instruments update their
coefficients on, the linear-phase convolvers' block position (a partitioned convolution
rounds differently depending on where its blocks fall), Terminus's and Culmen's 10 ms
columns, Comprimo's 100 ms loudness steps, Lamina's tremolo and Calor's chorus (LFOs that
follow the sample clock, so they *can* rest while they turn). `skip` is O(1) for most and
loops per block for the linear-phase EQ (a design that was due is made where processing
would have made it).

Why (3) is limited to unchanged parameters: Terminus's and Culmen's Match monitor and
Comprimo's compare hold their level through silence; a new unit's is neutral. They reach
the audio only in the monitor modes that use them, so the unit renders like a new one until
a parameter switches that mode on — then the running unit (correctly) remembers. Suspension
(2) is still exact then, because the skipped unit remembers the same.

**Recommendation to the graph builder** (also in `requests.md`): suspend a node whose units
are all at rest and whose gathered input, key and notes are silent; while suspended call
`skip(pass_frames)` on each unit, once per pass; on wake call nothing (a `reset` is exact
too, but only while the parameters have not changed — prefer no reset).

**Zero means +0.0, bit for bit.** A state that holds −0.0 (the last input was −0.0) can
turn the next −0.0 into −0.0 where a new unit gives +0.0 — found by the twin test in
Terminus's DC blocker. Every check compares bits (`cantus_dsp::rest::{z, z64, all_zero}`).
The engine's gathered silence is +0.0 (ADR §1.3), so units that hear it come to rest.

**Cheap.** Long lines are never scanned when the engine asks: a `ZeroRun` beside a line
counts the zeros written into it in a row, updated after each block from the end of what
was written (one comparison while sound passes), or the unit counts zero input frames.
Shorter state (filters, envelopes, a 128-tap history) is compared directly, cheap checks
first. `rest_blocker()` (on `EffectUnit`, `FxUnit` and the big DSP types) names the first
condition that fails — for the tests and for a profiler that wants to say why a node runs.

## The units

Times are the silence the twin tests needed before `at_rest()` held (after their signal,
random length, up to +12 dBFS), with FTZ/DAZ; "—" is a kind that does not rest at that
setting, with the reason.

### Effects (`EffectKind::ALL`, twelve)

| Kind (DSP) | At rest | Defaults | Heavy | What it reads; what keeps it from resting |
|---|---|---|---|---|
| Eq (`Aequalis`) | yes | 0 s | — | all three modes: glides landed, no dynamic movement, every band's biquads and detector empty (the detector's envelope has no flush: it reaches zero under FTZ), auto gain neutral when it is on, natural phase designed and not fading, linear phase settled with its blocks on the clock a new unit's run on. **Heavy does not rest**: a dynamic band's release leaves a residual under 0.0001 dB that it never sets back to 0 (finding 3). An expander band acting on silence, or the auto gain after music, never rests either: a reset starts every band at its static gain and the auto gain at 0 dB. |
| Compressor (`Comprimo`) | yes | 3.9–20 s | 3.3–20 s | reduction, hold and programme memory at zero, key filters, lines and oversamplers empty, the auto gain and compare glides landed or stalled (finding 6) — and the three loudness meters' 3 s windows all silence, so no step can move their targets. Needed the meter flush fix (finding 2). Not with auto gain after music, or listening to the compare. |
| Limiter (`Terminus`) | yes, without dither | 5.7–7.9 s | — (dithers) | every stage, detector, look-ahead window, envelope and line empty, the glides landed, the Match monitor's window all silence and its gain landed or stalled (at unity in Match mode). **Never with dither on** (R10): the noise generator stops on silence but does not go back to its seed. Needed the loudness flush fix (finding 2); its programme memory stops in the subnormals without FTZ and is exactly nothing there (proved in `work_is_nothing`). |
| Space (`Spatium`) | **no** | — | — | its late field's delay lines are swept by free-running rotating phasors, renormalised each step: a new unit's start at fixed phases and would only line up again if the rotation were replayed sample by sample at an unchanged rate. With modulation depth 0 it could rest; not built this round. |
| Multiband (`Multiplex`) | yes | 0 s (a wire) | 3.4–3.7 s | every band at unity with no dynamic gain, in force and all through its look-ahead ring, glides landed, detectors, splits, shelves and the audio ring empty; in linear phase the FFT blocks empty, started with the stream, and the filters designed exactly where the crossovers are. A band with a static gain, or an expander acting on silence, never rests: a reset starts every band at unity. Needed the detector flush fix (finding 2). |
| DeEsser (`Sibilus`) | yes | 0.5–0.7 s | 0.4–0.7 s | band edges landed, no reduction, detectors and the input, band and reduction lines empty. |
| Gate (`Porta`) | MIDI trigger only | — | — | played by its key a gate never rests: a reset gate starts **open** and closes after its hold, one that heard silence is closed — two different states, both right. In MIDI mode both are closed. |
| Saturator (`Fervor`) | yes | 0–0.01 s | 1.2–1.3 s | glides and structure as a reset leaves them, no crossfade, gains still, dry line and (when engaged) every filter, the DC blocker, the hold and the oversampler empty. Never with the sample-and-hold on: its phase follows the history of the rate. |
| Stereo (`Latitudo`) | yes | 0.04–0.09 s | 0.04–0.06 s | every glide, fade and structure where a reset puts them, the ramps still, filters and the Haas line empty. |
| Maximizer (`Culmen`) | yes, without dither | 8.4–8.6 s | 7.3–8.5 s | its three stages at rest, the drive landed, reference lines and input loudness empty, the Match window silent and its gain landed or stalled. Never with dither on (R10). |
| Delay (`StereoDelay`) | yes | ~20 s | — (~11 min) | both lines all zeros, the tone filter empty, time and mix landed. The feedback decays to the 1e-20 flush (0.4 → ~18 s), then the 2 s line clears; 0.9 feedback takes about eleven minutes. |
| Reverb (`Reverb`) | yes | 6.3–7.2 s | — (> 1 min) | every comb, allpass and the pre-delay all zeros and the damping filters empty. |

### Instruments (`InstrumentKind::ALL`, seven, and the slot's two)

| Kind (DSP) | At rest | Defaults | Heavy (32 voices) | What it reads; what keeps it from resting |
|---|---|---|---|---|
| Wavetable (`Unda`) | yes | 0 s | 0–0.07 s | no voice, no note held, the decimator empty, wheels and pedals home; not with a free-running LFO in use (its phase follows the history of its rate), not with glide on every note and a last pitch to glide from. |
| Analog (`Calor`) | yes | 0.9–1.0 s | 0.9–1.1 s | no voice or ghost, the tail let go (decimator, chorus, DC block), wheels home; the chorus sweeps on the sample clock, so it rests while it turns. Not after a live note (live notes rotate the cards and are seeded by their count), not with glide on and a last pitch. |
| FM (`Modulus`) | yes | 0.05–0.2 s | 0.03–0.2 s | no voice, no note held, wheels home; with glide on, no last pitch. |
| Sampler (`Exemplar`) | yes | 0.4–0.55 s | 0.5–0.6 s | no layer, no key held, pedals home; not with a keyswitch in force (a new unit has none) or after a live note (live round robin counts). |
| Bass (`Gravis` + its host unit) | yes | see test run | see test run | idle (Gravis's own predicate: every voice ended, every stage after it empty, every glide landed), no note held, the legato window shut, no bend; not the String engine (it picks the next string from where the hand last was), not Free phase (the oscillator carries its phase), not glide-always with a last pitch, not with a route reading the LFO, velocity, key or random (the modulation tick would move the settings, or remember the last note). |
| Electric piano (`Lamina`) | yes | 0.04–0.23 s | 0.05–0.13 s | asleep, every voice free, pedal up, wheels home, glides landed; the tremolo is either silent or where a new unit's would be after as many samples (it runs on the sample clock). |
| Organ (`Organum`) | **no** | — | — | its tone wheels, vibrato scanner and rotor turn for ever, and their phases reach the output; a new unit's start at zero. The wheels and the scanner already skip exactly (`Generator::skip`, `Scanner::skip`) so it could rest with them advanced by `skip`; the rotor's speed history (slow/fast with acceleration) would have to be tracked too. Not this round. |
| Firmus (the slot's synth) | yes (`Firmus::at_rest`) | — | — | no voice, both glides landed. For the graph builder: the processor holds Firmus directly; nothing counts samples. |
| Pulsus (the slot's kit) | **no predicate** | — | — | a hit counter seeds live hits and the hi-hat's chick (and every hit in Pulsus I); a new kit's starts at zero, so after its first hit the kit differs from a new one. Suspension alone would be exact for Pulsus II with no voice — a weaker predicate than this round's. |

## Tests

- `tests/support/heavy.rs`: the bench's heavy settings (copied from
  `crates/cantus-bench/src/kernels.rs`), its signal, settings drawn at random over every
  parameter's range, a small RNG, and a test-only FTZ/DAZ guard (`Flush`).
- `tests/fx_contract.rs`, `tests/inst_contract.rs`, per kind (one test each, so they run in
  parallel):
  - **split invariance** of the unit alone: blocks of 1, 7, 4096 and two random split
    sequences (blocks 1…4096, weighted to short) against blocks of 512, bit for bit, at the
    defaults, the heavy settings and six random settings, keyed and not, with notes for the
    kinds that hear them; instruments with notes, chokes, controllers and per-note
    expressions at their samples, and the bench's 32-voice chord;
  - **the twin test**: feed (random length, random splits), run silent until `at_rest()`;
    then a twin that keeps processing zeros (exact +0.0 out, still at rest), a twin that is
    skipped (`skip`), a twin that is reset and a new one (both skipped as far as the first
    has run) must render the same bits for what comes next, each at its own random splits;
    a third trial moves the parameters at the wake (the skipped twin must still agree).
    Trials run without and with FTZ/DAZ (as the engine's `process` will run); rest is
    required, where the kind can rest, with FTZ;
  - the project-level test at the heavy settings at blocks 1, 64, 1000 and 4096.
- `cantus-dsp::rest`'s own tests: `ZeroRun` against a cell-by-cell reference at every block
  size, including blocks longer than the ring.

## Findings

1. **Split invariance holds beyond the defaults.** Every effect kind (keyed and not, notes
   for the gate) and every instrument kind (notes, chokes, controllers, per-note
   expressions, the 32-voice chord) renders the same bits at blocks of 1, 7 and 4096 and at
   random splits as at 512 — at the defaults, the bench's heavy settings and six settings
   drawn over every parameter's range per kind. **No unit was found block-size independent
   only at its defaults.** The project-level test at the heavy settings holds at 1, 64, 1000
   and 4096 too.
2. **Flushes that hover.** Three filters flushed their two state variables one at a time;
   after some decays one kicks the other and the pair hovers at the threshold for ever (the
   comment in Porta's sections names exactly this). They never came back to zero, so their
   units could never be at rest: Comprimo's loudness meters, Terminus's (and Culmen's)
   K-weighting, and Multiplex's band detectors. Each now flushes both at once, in its own
   commit with the reason. All three are measurement paths: what they change is under
   1e-30 (meters) or 1e-20 (detectors, whose followers flush below 1e-12), far under an ulp
   of anything they are compared with; the only place it can be heard is Multiplex's Key
   listen tail under 1e-20. The null corpus and the `v5.cantus` hash do not move (below).
   The same one-at-a-time flush remains in audio-path filters (Aequalis's biquads, the
   crossover sections Multiplex and Latitudo share); there it would move tail bits, so it
   is left — those units came to rest in every test, and if one ever hovers its `at_rest`
   simply stays false.
3. **Aequalis's dynamic bands keep a residual.** A band's dynamic gain is updated only when
   it moves by more than 0.0001 dB, so its release ends a hair from 0 and stays there: a
   dynamic EQ (the heavy setting) never rests. Landing on exactly 0 when the target is 0
   is one line, but it moves bits; it waits for a `DSP_VERSION` bump (the math builder's).
4. **−0.0 is not +0.0.** The first twin test found Terminus's DC blocker holding −0.0 from
   a −0.0 input and turning the next −0.0 into −0.0 where a new unit gives +0.0. Every zero
   check compares bits now.
5. **Counters are in the bits.** The delay's read head takes its fraction from the absolute
   write position (`write as f32 − delay`), so the position of an all-zero line matters:
   skipping 300 samples without moving it changed bits. The same holds for every grid a
   unit updates on and for the linear-phase convolvers' block position. Hence `skip`.
6. **Glides that stall.** Terminus's and Culmen's Match gain glides by 2·10⁻⁵ of the gap per
   sample and stops a few tenths of a percent short of its target when the step falls under
   half an ulp; Comprimo's compare does the same. A stalled glide never moves again, so it
   counts as still (`cantus_dsp::rest::glide_still` replays the step and checks).
7. **Resets that kept counting.** The bass unit's modulation grid and Modulus's clock were
   not zeroed by `reset`, so a reset unit was not a new one. They are now (the only change
   of behaviour: after a panic or an instrument switch the grid restarts at 0, as a new
   unit's does; no render resets mid-way).
8. **What only the editor reads does not count**: meters, telemetry columns, scopes, the
   gain-reduction readouts, event statistics — skipping them changes what an editor would
   draw for the skipped time, never a sample.

## Bits

Three commits change DSP behaviour on purpose, each alone and with its reason — the three
flushes of finding 2 (`ca3bfc4` Comprimo's meters, `556b925` Terminus's loudness sections, `de5540e`
Multiplex's detectors) — and two lines of the instruments' commit (`1437010`) make a reset
bass unit and a reset Modulus restart their clocks (finding 7). `null_tests.rs`, the `v5.cantus` hash and every
test in `crates/cantus-engine/tests` and `crates/cantus-dsp` pass unchanged on this branch
(see the report). Everything else is additive: new methods, new counters beside the state
(`ZeroRun`, quiet and unity counts, clocks) that no output reads.
