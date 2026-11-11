# ADR 0112 — Aula, the convolution reverb: zero latency, exact, swapped in whole

**Status:** accepted (round 39, the `conv` builder) · **Date:** 2026-11-09 · **Schema:** 60
(an effect's optional `ir`, and the new kind) · **Relates to:** ADR 0004 (commands), ADR 0005
(the audio thread), ADR 0018 (null defaults), ADR 0020 (our plug-ins outside Cantus), ADR 0022
(memory an effect needs, built off the audio thread), ADR 0030 and 0042 (content on demand, CC
BY credited), ADR 0031 §5.1 (at rest), ADR 0071 (effects that sound on insert), ADR 0101
(presets by style and role)

## Context

The owner asked for a convolution reverb (2026-11-09); seven of the eight market leaders have
one (Logic's Space Designer, Altiverb in Pro Tools' world, Cubase's REVerence, Ableton's
Hybrid Reverb, FL Studio's Fruity Convolver, REAPER's ReaVerb, Studio One's Open Air). Spatium
(ADR 0058's algorithmic reverb) makes rooms; a convolution reverb *is* a room — a recording of
how a real space answers a click — and is how a mix is put in a real church, a real plate, a
real spring tank. The bar, as for everything in Cantus: no added latency, the same bits at any
buffer size, offline equal to real time, nothing allocated on the audio thread, and responses
that load without a click.

## Decision

1. **A new kind, `EffectKind::Aula`** (Latin *aula*, the hall), its own parameter table
   (`cantus_core::fx::aula`): `ir` (Cantus's own response, stepped), `predelay`, `stretch`,
   `length`, `attack`, `decay`, `gate`, `low`, `high`, `damp`, `width`, `reverse`, `mix`,
   `level`. Every default leaves the sound as it is (mix 0; the shaping controls at the values
   that leave the response as recorded); an insert loads the starting preset *Concert hall*, a
   quarter wet (ADR 0071).
2. **A response from a file is not a number**: `EffectSpec::ir: Option<Box<IrFile>>` — the
   path as the project stores it and, for a pack's file, the pack's id (its credit is shown,
   ADR 0042). `None` for every other kind and not written, so every other effect's JSON is
   what it was. Set by `Command::SetEffectIr` (its inverse the old value; refused for another
   kind or an empty path; an undo-fuzz arm). **Schema 60**, a migration that writes nothing.
3. **The convolver** (`cantus_dsp::aula_conv`, `aula_kernel`, `aula_fft`):
   - the first 64 taps in **direct form**; then **levels of uniform partitions** of 64, 512,
     4 096 and 32 768 samples (15, 14, 14 partitions, the last level as many as the response
     needs), overlap-save with a frequency-domain delay line of the *inputs'* spectra;
   - **level *k* starts at twice its partition**, so the result of its block is needed one
     partition after the block completes: its work — the gather, the forward stages, the
     products, the inverse stages, the add into the output ring — is cut into pieces of at
     most 1 024 elements and **spread evenly over the 64-sample ticks of that slack** by a
     schedule made with the kernel. Level 0 has no slack and runs whole at each tick. **Zero
     added latency, a constant cost per block;**
   - **the grid counts from the stream's start**, never from a host block: at each 64-sample
     boundary the levels run their tick's pieces, so any split of the host's blocks gives the
     same bits (tested at tolerance 0 with blocks of 1, 7, 512, 4 096 and random);
   - both inputs share one complex transform (left real, right imaginary) and both outputs one
     inverse; a radix-2 transform in `f32` whose twiddles come from `cantus_dsp::math`;
   - formats: **mono** (one response, each side its own input), **stereo** (left to left, right
     to right), **true stereo** (four: L→L, L→R, R→L, R→R — the channel order Waves, Altiverb
     and ReaVerb write).
4. **What changes the response is built off the audio thread** (ADR 0022's path, now open to
   every unit behind the contract: `FxUnit::memory_need` and `FxUnit::supply`). The unit's
   `configure` computes, allocation-free, *what* to build — the source's key (Cantus's index,
   or an FNV-1a of the file's stored path) and the shape's bits — and the engine asks for it
   (`MemoryNeed::Aula`); `EffectMemory::build` makes the kernel (`cantus_engine::fx::aula_store`)
   from the store's copy of the source: resampled and stretched in one windowed-sinc pass, the
   gate, length, decay, damping, the cuts, reverse, attack, loudness set once from the
   stretched response. An offline render builds it at once (`EffectUnit::new`), so a render
   hears exactly the response the project names; live, the previous response plays until the
   new one arrives. *(Integration of round 39.)* The request (84 B by value) does not ride in
   the event, which ADR 0107 holds to 64 B: each Aula unit owns a mailbox made with it
   (`cantus_engine::need_mailbox`, a sequence lock of atomics), the audio thread writes the
   request there and sends `NeedTicket::Aula` — a clone of the mailbox's `Arc`, a count and
   never a free on the audio thread — and the builder reads the latest request written
   (`rt_memory` plays a request through it allocation-free; `an_event_is_small` holds the size).
5. **Swapped in whole, the tail kept.** A kernel is taken at the next boundary by the head and
   level 0 and by every other level at the start of its next period (each period computed with
   one response); the delay lines hold the inputs, so what was played rings on through the new
   response. A kernel that needs more room than the unit's state comes with a new, empty state
   (a longer response); the room a response gets — level 2 whole, then the largest level in
   eights — holds its stretches, so turning *Stretch* never reallocates.
6. **The audio thread never frees.** Every kernel a unit holds has a sibling in the store's
   pool (`Arc`), so a unit letting one go is a decrement; a kernel no unit holds is freed on
   the next build (keeping six spare for undo). States and the boxes a memory travels in go
   back to the interface's thread (`EngineEvent::GarbageMemory`). `rt_memory.rs` plays a
   shape change, a longer response and its new state, a seek and a stop: no allocation.
7. **Files are read where every snapshot is compiled** (`EngineHandle::compile`:
   `aula_store::prepare`), live and offline, resolving a relative path beside the project;
   decoded once and kept (the sample cache's rule). A missing or unreadable file is a status
   in the editor and silence in the reverb, never a failure.
8. **Cantus's own responses** (`cantus_dsp::aula_gen`): fifteen — rooms, chambers, halls,
   plates, springs — made at the engine's rate from seeded noise in three bands with their own
   RT60, early reflections, a build-up; springs from a loop of a delay, eighty all-pass
   sections and a low-pass. Deterministic, nothing downloaded. **Free responses on demand**: the
   *OpenAIR Impulse Responses* pack (eight spaces from the University of York's OpenAIR
   library, CC BY 4.0, read on its archived pages on 2026-11-09), never in the repository; its
   credit is in the manifest and in the editor.
9. **The plug-in** (`Cantus Aula`, CLAP and VST3, ADR 0020): Cantus's own responses by `ir`; a
   builder thread of its own answers the unit's needs through lock-free rings
   (`cantus_plugins::memory`), since a plug-in has no interface thread to lend. Files stay the
   DAW's: a plug-in's state holds numbers.

## Consequences

- An effect behind the contract can now ask for memory built off the audio thread; the EQ's
  linear phase keeps its own path.
- Every snapshot compile reads the files Aula names (once each); the interface's thread builds
  kernels (tens of milliseconds for a long true-stereo hall), pooled by what they are made from.
- A live engine plays the previous response for the time a build takes; an offline render never
  does.
- The plug-in cannot play a file; noted in its description and the manual.

## What the tests hold

- `cantus-dsp` (`aula_conv_tests.rs`, `aula_tests.rs`, `aula_fft.rs`, `aula_gen.rs`): a direct
  convolution in `f64` for lengths 1 … 70 000 in all three formats (worst 3.1 × 10⁻⁷ of the
  peak); the same bits for blocks of 1, 7, 4 096 and random splits, the whole effect included
  with settings moving; a swapped kernel becomes the new response's convolution of the whole
  input; rest after the settling time, +0.0 out, and as good as new; each level's schedule
  covers its work once and no tick holds more than about twice its share; the FFT against a
  direct DFT and done in pieces to the same bits; the null at mix 0 whatever else is set; a
  unit impulse is a wire and the pre-delay delays it exactly; the shaping controls; Cantus's
  responses finite, true stereo and the same bits twice.
- `cantus-engine` (`fx_aula.rs`): files written by libsndfile (mono 16-bit, stereo 24-bit,
  32-bit float) and by CPython's `wave` (four channels, 24-bit) read to the value against a
  reader written from the RIFF/WAVE specification; a true-stereo file's four paths heard where
  they belong; the null through the engine's unit; a need asked once, built, supplied, the box
  handed back; a file read where a snapshot is compiled and its tail heard; a missing file a
  status. With `CANTUS_AULA_OPENAIR` naming the pack's files: read to the value and played
  back raw as recorded. `fx_contract.rs`: block-size independence and exact rest at its
  defaults, heavy and random settings. `rt_memory.rs`: no allocation.
- `cantus-ui` (`aula_e2e.rs`): through `CantusApp::assemble` and the engine's compile, Aula
  inserted on its starting preset, a WAV loaded as the editor loads it, an offline render with
  the file's tail in its colours, undone in one step to the hall's bits.
