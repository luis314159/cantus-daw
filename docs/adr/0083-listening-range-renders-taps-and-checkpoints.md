# ADR 0083 — Listening: range renders from checkpoints, analysis taps, live = offline

**Status:** accepted (round 34) · **Date:** 2026-10-30 · **Extends:** ADR 0005 (real time),
ADR 0021 (stems), ADR 0026 (the master chain), ADR 0031 (the pool), ADR 0052/0056 (the
loudness meter, Speculum), ADR 0081 (the MCP server's tools and jobs)

## Context

BACKLOG §7 ("What an AI needs…", from round 28's demo): an agent mixing in Cantus needs to
*listen and get numbers back in one call* — loudness per section, true peak, the spectrum
against a reference, correlation, clicks at section boundaries, a WAV — per-track analysis
in the mix and a masking matrix rather than soloed guesses, and mix targets solved as one
undo step. The demo's builder took four full three-minute renders to mix one song, and his
first harness diverged from the application by sending automation differently. What
Cantus needed inside: a range render that does not replay the song before it, analysis
taps on every strip of an offline render that change no sample, and "live = offline"
checked on the very path an agent uses.

## Decision

1. **One render path** (`cantus_engine::listen`, MPL). `listen(snapshot, options, units,
   hosted, checkpoints, progress)` sets the processor up exactly as `render` does (units,
   the automation runtime, the planned graph, delay compensation settled before the first
   block, the loop and the click off) and drives it block by block. For a range it gives,
   sample for sample, what `render` gives from the song's start with
   `compensate_latency` — a range that ends early stops and rings out as an export's does.
2. **Analysis taps** are the stems' `StemTap` (ADR 0021): at the two places a signal is
   mixed the processor copies what it is about to add and never writes. Each tapped source
   (a track at its channel, a strip after its fader, the direct sum, the master) is aligned
   by its own lateness, so sample 0 of every ear is the range's first tick, and heard by a
   **Speculum ear** (`cantus_dsp::speculum::ear`, MPL): a column every 10 ms (K-weighted
   energy of the pair and of its mono fold-down, true peak 4× oversampled, sample peak,
   energy, the sums correlation is made of, mid/side energy, the sharpest second
   difference) and a spectrum every 4 096 samples (8 192-point Hann frames, mid and side in
   31 third-octave bands through one FFT). Any range of a render is then read without
   rendering again: integrated loudness and LRA **through the house meter's own
   `ColumnLoudness`** (equal to `LoudnessMeter` to the bit — tested), short-term and
   momentary maxima, peaks, RMS and crest, correlation and its lowest 400 ms window, width,
   mono loss, octave and third-octave spectra.
3. **Checkpoints.** At each section start (the markers, else every eight bars) the render
   keeps a copy of the whole engine paused there: `Processor::fork` (`listen_fork.rs`, a
   child of the processor module) copies every unit's state, the transport, the
   automation's and the modulators' state, the compensation lines, the notes on their way
   to inserts, the master chain and the pass context's snapshot into a processor with its
   own rings and its own pool. Units copy themselves through a new `fork()` on the two unit
   traits (`FxUnit`, `InstrumentUnit`, default `None`); every native effect and instrument
   implements it by `Clone` of its whole state (the DSP structs derive `Clone`). What
   cannot be copied returns `None`: a hosted plug-in (its state is the plug-in's), a frozen
   render's reader and a voice streaming from disk (they share a board with the reader
   thread), a sync port. A checkpoint sits on the automation's control grid (16 samples)
   at or before the section's first sample, so a snapshot installed on it waits for
   nothing; kept checkpoints drop their pool (`Workers::SERIAL`); at most 24 are kept (the
   song and a solo of a dozen sections each — review 2) and at most **512 MB** of them
   (`DEFAULT_CHECKPOINT_BYTES`, internal; each one's size estimated when it is taken,
   `Processor::approx_bytes` — review 3), the least recently used going; a checkpoint of
   the same setup from before an earlier edit is replaced at its place. They are let go
   when another project replaces the open one, and after **ten minutes** with no listen
   (`listening::QUIET`, never waiting for a running one — review 3).
   **A checkpoint keeps only the cells in use** (review 3): `Cells` names the track and
   strip slots the render's lineage was ever given (a slot given and freed stays named);
   the checkpoint is an *image* — a processor with only those cells
   (`Processor::with_cells`) and none of the fresh delay lines, never played — and a
   render resumes from a full processor copied back from it, every other cell fresh, as a
   fresh processor made it in the original too.
4. **When a checkpoint holds** (`listen::same_until`): two snapshots that play the same up
   to its tick leave the engine in the same state there. They are compared with
   everything that only happens at or after the tick taken out — note, controller and
   expression events, audio clips starting there or later, automation pieces beginning
   after it (the piece playing at the tick stays; one beginning exactly on it counts by its
   value there) — and everything else compared whole (a setting, a fader, the routing, the
   tempo map, a modulator), so a difference it cannot place fails the checkpoint, never
   passes it. A changed snapshot is installed on the copy as playback installs an edit
   (the runtime first, then the snapshot and its plan).
5. **What cannot be copied is known before the render where it can be**: a hosted plug-in,
   a frozen track or strip, and a sample streaming from disk anywhere in the project — the
   streamed-sample check is project-wide, so a stream on a track the range does not hear
   still makes the range pre-roll (the trade-off: no per-track reachability analysis, and
   a check that never claims a copy it cannot make). A fork that fails anyway is remembered
   with its reason when the reason lasts (`Refusal::lasting`; a snapshot waiting for the
   grid does not) and the same setup pre-rolls next time; an uncopiable setup never tries a
   fork again, nor takes a checkpoint, even rendered exactly from the top.
6. **Where the engine cannot be copied** the range starts a **pre-roll** before itself
   (8 s by default — a long hall's tail) and says so: `Resumed::Preroll { from, seconds,
   why }`; the tools answer `exact: false` with the reason, and render from the song's start
   when asked `exact: true`.
7. **Live = offline, checked**: a test drives the application's own `EngineHandle`
   (`EngineHandle::manual`: its snapshot, units and automation sent as playback sends
   them, delay compensation answered as the UI answers it, irregular blocks) and compares
   it to the listen an agent hears, to the bit.
8. **The tools** (`cantus-ui/src/ai_tools_listen.rs`, GPL), each a job: `audition`
   (range, solo/mute, master on/off, a genre curve or a reference file inside the project's
   folder; loudness, sections, spectrum against the reference, clicks, a WAV in the
   application's cache, how the render began), `analyse_tracks` (per track at its channel,
   in the mix: loudness and share of the mix, spectrum, crest, peak, loudest three
   seconds), `masking_matrix` (two tracks fight in a band and a frame when both are within
   15 dB of the band's loudest track and within 6 dB of each other; the overlap is the share
   of the band's energy spent so), `set_mix_targets` (the channel's level is linear, so one
   render solves the trims exactly; one `Batch` of `SetTrackGain`, clamped to −96 … +12 dB
   and said). They reach the application through `ListenHost` (`CantusApp` and the tests'
   host), which refuses — with what to do — while samples or a render's plug-ins load
   (`RenderFor::Listen`, a new owner of the plug-in host's render preparation; its
   instances are let go on the UI thread when the listen ends).

## Consequences

- An agent that rewrote a late section hears it again in the time of that section. On the
  showcase song (*With All Your Heart*, 2 min 50 s, dev profile with the DSP at opt-level 3):
  the whole song rendered in 144 s taking its ten checkpoints (round 28 measured 157–217 s
  without them); its last section from the checkpoint at bar 73 rendered 20.7 s of audio in
  14.2 s — **ten times faster**, and identical to the whole render's frames to the bit. The
  test process holding those ten checkpoints stayed under 250 MB of working set.
- **The pre-roll is close, not exact.** Review 1 found it started on the fractional sample
  of its first tick, every event a fraction of a sample off a render from the top; it now
  starts on the whole sample a render from the top reaches there (`SeekFrac`), tested at
  116 BPM with odd pre-rolls to the bit. Measured again on the showcase's last section,
  pre-rolled 2, 4, 8 and 16 s: still 2.0, 2.5, 1.4 and 1.7 dB under the section's peak
  (largest sample difference) — so that was not the cause. Measured unit by unit
  (`preroll_by_unit`, a chord struck on every beat, 3 s of pre-roll; review 2 added the
  instruments): through the phaser, chorus, flanger, tremolo, vibrato and volume shaper
  (their LFOs follow the song's position), the compressor, the limiter and the plain reverb,
  the pre-roll meets the whole render within 129–139 dB; through the delay (its feedback
  still ringing) 62.5 dB; through **Space only 8.3 dB** — its feedback delay network's
  modulation rotors turn from the moment the unit is reset, not from the song. Of the
  instruments, the sampler and Gravis meet it exactly, Unda and Calor within 132–134 dB,
  Lamina 64 dB, Modulus 43 dB and Chorda 36 dB (tails still ringing after 3 s), and
  **Organum not at all (−1.4 dB)** — its rotary's angle is integrated from its reset. The
  showcase's concert hall is a Space. Anchoring that state to song time is a sound change
  (a `DSP_VERSION` bump), not this round's (the integrator's decision); it is on BACKLOG §4b.
  So the tools say `exact: false` with the reason, and take `exact: true` to render
  from the song's start instead (a pre-roll reaching back to the start is that render, and
  answers `from: start`).
- Every native unit must stay copyable: a new unit implements `fork` (a `Clone` derive and
  three lines); one that does not still works, but its project pre-rolls. The fork's
  completeness is held by the compiler and by tests: the tracks' and the strips' state are
  copied by struct literals, and the processor's own fields and its nodes' — copied field
  by field onto a fresh processor — are all named in a destructuring pattern without `..`
  (`every_field_is_accounted_for`, review 1), so a field added to either does not compile
  until the fork says what becomes of it (it found two at once); `a_fork_plays_on_as_the
  _original_does` plus the rich project of `tests/listen.rs` (every native instrument and
  effect, sends, a return, the master chain with the limiter's look-ahead, a fader ride, a
  lane, an LFO) fail on any state left behind. (The pass context's snapshot was: found by
  the first test.)
- Checkpoints cost memory — the state of the units and cells in use. Measured (review 3;
  a whole engine was **75 MB** whatever the song, 1.9 GB for 24): a one-track song's
  checkpoint **5.7 MB**, a 19-minute one-track song's 7.9 MB (24 kept: 190 MB,
  `listen_measure::the_checkpoints_memory_on_a_long_song`); on the demos 19–40 MB — the
  showcase (*heart*, 170 s, 17 tracks) **40 MB**, *house* (124 s) 26 MB
  (`the_checkpoints_memory_on_the_demos`). So the 512 MB budget keeps about a dozen of the
  showcase's. The estimate is within −8 % to +25 % of the working set on every demo; its
  figures per unit kind are measured (`listen::tests::what_each_unit_adds_to_an_image`:
  most inserts 100–250 KB, Space and the maximizer 2 MB, the sampler 4 MB, Chorda 5 MB).
- The taps and the ears are offline only: nothing new runs on the audio thread.

## What the tests hold

`crates/cantus-dsp/src/speculum_ear_tests.rs` (8): the ear's loudness and true peak are the
house meter's to the bit; any block size gives the same columns, spectra and readings; a
sine reads its level in its band and nothing 50 dB near it elsewhere; opposite phase reads
−1 and folds to nothing; independent channels are wide and lose 3 dB in mono; a range reads
its own part; a click is twenty times sharper than the music; NaN is silence.
`crates/cantus-engine/src/listen_prefix.rs` (2), `listen_fork.rs` (1),
`crates/cantus-engine/tests/listen.rs` (9), `tests/rt_memory.rs`'s `a_forked_processor_never_allocates`: the taps change no sample; a section from its
checkpoint is the whole render to the bit (from the section's start, inside it, from an
earlier checkpoint); an edit after the checkpoint plays from it as a whole render of the
edited song does, an edit before it does not use it, a fader change uses none; any block
size; a range that ends early is the export of that range; a hosted unit pre-rolls and says
so; a listen stops when asked; live playback through the application's handle is what a
listen hears. `crates/cantus-ui/src/mastering_tests.rs` and `tests/ai_listen.rs`: the tools
through their `ToolRun`s and through the MCP server's in-process client.
Since review 3, every one of those renders resumes from an image: still to the bit.
`listen_fork.rs`'s `an_image_holds_the_cells_in_use_and_plays_on_from_them_to_the_bit` (one
track cell of 256, the sixteen strips of a new project, the fresh lines left out, under
8 MB; a processor copied back plays on as the original, 400 blocks); `listen.rs`'s
`the_checkpoints_keep_to_their_budget_in_bytes` (a budget of three and a half images keeps
the last three, and one still serves its section); `mastering_tests.rs`'s
`quiet_listening_lets_the_checkpoints_go`.
