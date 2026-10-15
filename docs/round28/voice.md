# Round 28 — voice: the pitch base and Tonus

*Builder: voice · branch `r28/voice` (from `ca5b48e`) · ADR 0053 · BACKLOG §3d, the base of §3c.*

## Done

- **The pitch base in `cantus-dsp`** (our own implementation of YIN, pYIN and TD-PSOLA from
  the papers; MPL):
  - `pitch.rs` (+ `pitch_stream.rs`, `pitch_offline.rs`, `pitch_testkit.rs`,
    `pitch_tests.rs`): the **streaming tracker** `PitchTracker` (a frame every 2.7 ms,
    fixed cost per hop, no allocation, exact rest) and the **offline analysis** `analyse`
    (centred YIN, pYIN candidates, a Viterbi pass over 20-cent voiced/unvoiced states, the
    epochs phase-locked by cross-correlation). Voicing from aperiodicity, level and the
    sibilance share, with hysteresis. Lowest f0 55 Hz (why: ADR 0053 §1). A test kit of
    synthetic voices (Rosenberg pulses through formant resonators, vibrato, glides,
    sibilants, breaths, octave-ambiguous tones), public for the next builder's tests.
  - `psola.rs` (+ `psola_offline.rs`, `psola_tests.rs`): the **streaming shifter**
    `Shifter` at a **fixed 128 samples at 48 kHz** (2.67 ms at every rate), asymmetric
    windows, grains on a lattice aligned by a causal cross-correlation, formant shift by
    grain resampling, an exact dry path; the **offline renderer** `render` (ratio and
    formant curves, a time map, the source bit for bit wherever nothing is asked).
- **Tonus** (`EffectKind::AutoTune`, stem `tonus`), complete:
  - DSP `tonus.rs` + `tonus_scale.rs` (+ tests): targets from key + 13 scales with per-note
    *on / removed / bypass*, from the notes held on MIDI, or from the MIDI melody; retune
    speed 0–400 ms, amount, humanise, flex, the singer's vibrato scaled, a created
    vibrato, formants kept or following, throat, transpose, reference A4; MIDI notes
    applied at their own sample.
  - Parameters (`cantus-core/src/fx/tonus.rs`, 26 keys, choices for the plug-ins), engine
    unit with latency, rest, skip, notes and telemetry (`cantus-engine/src/fx/tonus.rs`).
  - The editor (`views/fx/tonus.rs`, `tonus_graph.rs`, `tonus_keys.rs`): the pitch
    display (sung and corrected over note lanes), the **detected note** with cents, target,
    shift and a zero-centred meter; the front row (targets, key, scale, speed, amount); a
    one-octave keyboard (click: target/removed, right-click: bypass; reset); the folded
    settings (*More*); the MIDI track picker in Cantus; nine factory presets (*Natural*,
    *Tight*, *Hard tune*, *Hard tune with a created vibrato*, *Throat up*, *Throat down*,
    *MIDI lead*, *Major key, soft touch*, *An octave down*); the inspector preview; both
    locales; it also runs in the exported plug-ins' windows.
  - **Notes into the exported plug-ins**: a CLAP note port (CLAP and MIDI dialects) and a
    VST3 event bus on every kind that takes notes (Tonus and Porta), delivered at their
    sample — the gap for Porta's MIDI trigger in other DAWs closes too.
  - A **demo project** (`views/fx/tonus_demo.rs`, `examples/tonus_demo.rs`), `shoot_tonus`,
    `docs/plugins/tonus.md`, the manual (EN/ES, §10), ADR 0053.

## Measurements

| What | Measured |
|---|---|
| Tracker, steady vowels 98–784 Hz at 44.1/48/96 kHz | no gross error; worst under 2 cents (typically under 0.2) |
| Tracker, 220 Hz ± 60 cents vibrato | streaming mean 2.0 cents, offline 2.0; no gross error |
| Tracker, 150 → 600 Hz glide | streaming mean 2.2 cents, offline 2.3 |
| Phrase with a breath and a sibilant | voicing 100 % (away from 40 ms of each change), no gross error |
| Octave-ambiguous tones (110, 150, 330 Hz) | no octave error, either form |
| Offline analysis speed | **0.85–0.96 s per minute** of audio at 48 kHz |
| Shifter latency | 128 samples at 48 kHz (118 at 44.1, 256 at 96): **2.67 ms**, constant |
| Shifted pitch (±3 … ±12 st) | within 2 cents |
| Formants kept at −3 / +3 / +5 st | envelope distance 1.0 / 0.9 / 1.4 dB (moved with the pitch: 9.6 / 7.9 / 10.3 dB) |
| Tonus, speed 0, notes 30–35 cents off | within 0.1 cent of the target |
| Tonus, speed 0, a note with ±40 cents vibrato | mean 2.3, worst 3.8 cents |
| Tonus, 60 ms | note centre within 5 cents; vibrato kept |
| Tonus, a legato note change (a whole tone, the voice sounding) | largest second difference around it 1.02 × a steady voice's at 60 ms, 0.98 × at 0 ms: no click |
| MIDI targets | change on the note's sample; same bits at blocks 1, 37, 64, 512, 4096 (DSP, engine, CLAP, VST3) |
| Rest | 0.35 s of silence |
| Monitoring | the monitored voice 128 samples late (exact), nothing else in the project latent |

## Tests and what each holds

- `cantus-dsp`: `pitch_tests.rs` (12), `psola_tests.rs` (11), `tonus_tests.rs` (14) — the
  figures above; the nulls bit for bit; block-size independence; exact rest; determinism.
- `cantus-engine/tests/fx_tonus.rs` (7): latency; the four nulls through the whole engine
  are the dry mix delayed by exactly 128 (delay compensation included); block-size
  independence while correcting; offline = real time; MIDI targets from another track's
  clip; monitoring late by 128 only, and by the project's largest latency when another
  strip has one. `fx_contract.rs` (split, rest twins) passes at defaults/heavy/random.
- `cantus-plugins/tests/notes.rs` (7) and `clap/notes.rs` unit tests (4): the ports/bus on
  exactly the note-taking kinds (VST3's refused elsewhere); CLAP (both dialects) and VST3
  notes = the DSP's bits at any block size, for Tonus and Porta; floods of notes lose no
  note-off; CLAP's key −1 releases only the note id or channel it names.
  `clap.rs`/`vst3.rs`: bit-identical to the DSP; ids blessed.
- `cantus-ui/tests/fx_tonus.rs` (12): painting, columns once through the wrap, the keyboard's
  clicks (one undo each), *More*, presets, preview (*Hard* for every source), the demo
  project opens and plays its first note on A3, a transposed output in view, Speed and
  Amount explained on hover, *Reset notes* named in the history, the MIDI messages whole in
  both languages.

## Files outside my area (additive)

- `cantus-dsp/src/lib.rs`: two lines (`pub mod pitch; pub mod psola;`).
- `cantus-engine/tests/support/heavy.rs`, `cantus-bench/src/kernels.rs`: my heavy line.
- `cantus-plugins`: `src/instance.rs` (`NoteIn`, `push_note`, `process_with_notes`; `process`
  delegates), `src/clap.rs` (a notes buffer, `notes::read`, the process call, `mod notes`),
  `src/clap/ext.rs` (the note-ports extension), `src/clap/notes.rs` (new),
  `src/vst3/plugin.rs` (the event bus, `read_notes`, the process call), `src/desc.rs`
  (Tonus's description), `tests/common/mod.rs` (settings arm, `Direct::Tonus`),
  `tests/clap.rs` (automation key `throat`), `tests/golden/ids.txt` (`out` removed, 26
  blessed), `tests/notes.rs` (new).
- `cantus-ui/locales/areas/export.{en,es}.txt`: `export.bus.notes`.
- `cantus-ui/tests/fx_lint.rs`: my three new view files.
- `docs/manual/{en,es}/README.md`: a Tonus subsection at the end of §10.
- New examples: `cantus-ui/examples/shoot_tonus.rs`, `tonus_demo.rs`.

## The demo, for the owner's ears

`cargo run -p cantus-ui --example tonus_demo -- <folder>` writes `tonus-demo.cantus` and
`voice-line.wav` there (one is already at `F:\projects\daw-r28\shots\voice\demo\`). Open it
with *File ▸ Open*: *Voice · natural* plays; solo *Voice · hard tune* or *Voice · MIDI
melody* to compare (track 4 holds the melody the last one sings). For a real voice: arm
*Your voice* (input 1, Tonus natural on its strip) and sing with monitoring on, or drop a
recorded vocal on it. Shots: `F:\projects\daw-r28\shots\voice\` (front, *More*, MIDI; Night
and Paper; EN/ES; 75/100/200 %; the preview).

## Left

- **Owner only:** judging Tonus on a real recorded voice by ear (the demo's *Your voice*
  track), and the blind comparison against Auto-Tune / Newtone of BACKLOG §3b.

## Notes for the integrator

- **Conflicts to expect:** `modfx` also edits `cantus-plugins/src/clap.rs` and
  `src/vst3/plugin.rs` (`plugin_process` / `process`, for the host transport). Both changes
  are additive around the same call: keep the transport lines and call
  `process_with_notes(…, events, notes)`.
- **What a singer hears:** delay compensation aligns every strip to the project's largest
  latency, the monitored input included (a linear-phase EQ elsewhere makes the voice 4 608
  samples late — `monitoring_waits_for_the_projects_largest_latency` documents it). That is
  the engine's rule, older than Tonus; BACKLOG §4c's recording mode answers it. Worth a
  line in the BACKLOG ("monitored inputs: constrain delay compensation while armed") if
  the owner wants it sooner.
- For **pitch** (wave 2): ADR 0053 §4 is the API; `cantus_dsp::pitch::testkit` makes
  synthetic takes with known errors.
- No golden hash moved; `DSP_VERSION` unchanged; no schema change; no new command; no new
  crate.
- The Linux check in WSL was not run from this worktree (one cargo at a time on a shared
  machine); nothing I added is behind `cfg(windows)`.

## Review 1

The review (`reviews/voice-1.json`) confirmed 20 findings; all are fixed, each with a test
seen failing first unless said otherwise. Commits `79b5632` … `32a7c2c`.

**The base (DSP)**
- *The streaming octave guard locked a lower octave* (pitch_stream.rs) — one-sided now: up
  an octave, a shorter lag found as a first dip under the threshold wins unless the old dip
  is deeper by more than 0.02; down an octave the old test stays. Test: legato leaps
  220 → 440 Hz and back, no gross error after the step.
- *Viterbi log-probabilities never renormalised* (pitch_offline.rs) — the frame's maximum is
  subtracted after each frame (same path, deterministic). Test: 4 000 unvoiced frames stay
  near zero (about −33 000 before).
- *`f0_at` voiced half a hop early at onsets* — the arm checks `frac >= 0.5`, symmetric with
  offsets. Test on both edges.
- *The offline first grain out of phase* (psola_offline.rs) — the first grain of an engaged
  stretch reads the take where the output is, and the second, on an epoch, is placed at the
  same offset, so the fade-in is no comb. Test: at a ratio of 1 the fade-in ahead of a
  change is the take.
- *A released correction never reached zero* (tonus.rs, two findings) — the glide lands on
  its target within 1e-6 semitone, so the shifter disengages and the output is the exact
  delayed input again. Test: release a held MIDI note mid-phrase, bit for bit after.
- *Note events dropped past 128 per call* (tonus.rs, two findings) and *the wrapper's 1 024
  notes per block against Porta's 64 and Tonus's 128* (instance.rs) — Tonus and Porta now
  queue `NOTE_CAPACITY` (1 024) notes, checked at compile time in `tests/notes.rs`; past
  that the oldest queued notes act at once (timing lost, never a note). Porta's per-sample
  due check became one comparison and its compaction in place (it copied its whole queue
  every sample while a note waited). Tests: 600 On/Off pairs leave Tonus holding only the
  note still down; `MAX_TRIGGERS + 256` offs at once release Porta; through CLAP (both
  dialects) and VST3, 128 note-offs at once, and 64 notes and an all-notes-off at once,
  close Porta's gate, bit for bit with its DSP.
- *The no-click test never changed note while voiced* — `a_legato_note_change_makes_no_click`
  jumps the MIDI target a whole tone inside one voiced stretch, against a steady voice's
  baseline: 1.02 × at 60 ms, 0.98 × at 0 ms (it passed as written: there was no click).
  The 1.6 × claim on the old test is gone from this report and the plug-in page.

**Engine and plug-ins**
- *"It is corrected" passed on latency alone* — the engine test compares with the dry mix
  moved by Tonus's latency (seen failing with Tonus bypassed); the demo test measures the
  first note, sung 28 cents sharp, landing on A3 within 8 cents (seen failing on the dry
  render, which reads 57.28).
- *A CLAP off/choke with key −1 was all-notes-off* — `clap/notes.rs` keeps `Held` (the
  channels holding each key, the key of each note id): key −1 with a note id releases that
  note, with a channel that channel's keys, only all three −1 everything. A MIDI
  all-notes-off (CC 120/123) is its channel's, as MIDI defines it. Unit tests and a Porta
  test through the plug-in.
- *No test of the VST3 event bus on other kinds, nor of Porta's notes* — every kind:
  `getBusCount(kEvent, kInput)` is 1 exactly on the note-taking kinds, `getBusInfo` refuses
  the others; the test found `activateBus` accepting an event bus on every kind — it
  refuses now. Porta (MIDI trigger) through CLAP (both dialects) and VST3 at several block
  sizes, bit for bit with its DSP.

**Editor**
- *The pitch display pinned the output to the edge on a transposition* — it centres between
  the sung and output means and takes more lanes (up to 33) when they are far apart; a
  pitch outside the lanes is not drawn but marked with an arrow at that edge. Test at
  −12, −24 and +24 (failing at −24 before). It also found that an editor before any
  telemetry centred on note 0 (zeros read as a target C−1): the header names no target
  until the unit has run.
- *Speed and Amount explanations never shown* (two findings) — on hover the hint is the
  explanation with the value. `tonus.out` deleted; `fx.tonus.note.0` kept (the plug-ins'
  choices use it).
- *Reset notes recorded as "preset"* — `cmd.tonus_reset_notes` (Reset notes / Restablecer
  notas).
- *The preview said "0 ms" for a MIDI source at speed 0* — one wording for both branches,
  the editor's pill (Hard, or the time as the knob shows it).
- *Spanish host sentence in an abbreviated caps caption* — both MIDI messages are body
  lines; the test measures them within the 236-pt column in both languages and finds the
  message painted as written. Spanish: "Las notas llegan por el MIDI del anfitrión.",
  "Elige la pista cuyas notas sigue Tonus."
- *Spanish wording* — Humanizar, Tolerancia, Profundidad, Transposición (short forms
  Humaniz., Toler., Prof., Transp. where the knob is narrow), matching the *More* hint;
  English's transposition knob reads Transpose, no longer the Pitch card's name. Shot again
  (`shots\voice\review1\`, now with an octave-down state).

**Files outside my area (review 1, additive or minimal):** `cantus-dsp/src/porta.rs` (the
queue: capacity, in-place compaction, one due check per sample, the oldest acting early
when full — no change below capacity, no golden hash moved) and `porta_tests.rs` (one
test); `cantus-plugins/src/clap.rs` (the `Held` field, cleared on activate and reset),
`src/vst3/plugin.rs` (`activateBus`), `tests/notes.rs`. No public API of `pitch*.rs` or
`psola*.rs` changed; `tonus::MAX_EVENTS` and `porta::MAX_TRIGGERS` became public constants.
