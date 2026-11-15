# Round 40 — `fix-a`: the mid-round review's engine and DSP findings

Branch `r40/fix-a`, base `91339a2b` (r40/integration with look, memory, ripple, delay and pads
merged). The four findings of `review-mid.md` §fix-a, each reproduced by a failing test first,
fixed at the root, the test then passing. ADR 0118 §1 and ADR 0119 §3, §4 and its Consequences
are amended. `DSP_VERSION` stays 6; no golden hash moved; the sync off, hear off and Before FX
are untouched to the bit (their tests stand).

## A-M1 — Hear the key was recorded into After FX takes

- **Reproduced:** yes. `record_tap::after_fx_a_take_never_holds_the_key_heard` — a voice
  recorded After FX through a compressor keyed from a kick, the key heard: sample 0 of the
  take was the kick's 0.52 where the plain take had 0.
- **Cause:** `strip_run.rs` replaces the strip's pair by the key (or the compressor plays its
  detector's input) at the heard insert, and the recorder's tap copies the pair after the
  insert loop. A stateful chain cannot give both the heard and the unheard signal.
- **Fix:** while a strip's take is being written After FX (`ctx.recording` and the strip
  holds a recorder's tap) no key is heard on that strip: it plays what it records. Other
  strips and Before FX takes hear the key as before. ADR 0118 §1 amended (*Never in a take*,
  with the limit: an insert after the heard one that keeps a memory carries what it heard
  before recording began into the take's first moments).
- **Tests:** `after_fx_a_take_never_holds_the_key_heard` (the take with the key heard from
  before recording = the take with none, to the bit) and
  `before_fx_a_take_is_the_input_while_a_key_is_heard` (the input to the bit, the key heard).

## A-M2 — Delay's `note`/`sync` by automation never asked for its line; follow lates not re-planned

- **Reproduced:** yes, both symptoms, against the base sources.
  `fx_delay_sync::a_note_automated_longer_plays_its_whole_echo_offline_and_in_real_time`
  (60 BPM, a quarter on its 3 s line, a lane to a whole note from beat 4: the echo of beat 8
  came at 11 s, held at the 3 s line, not 12 s), and
  `fx_delay_sync::sync_switched_on_by_automation_behind_a_latent_insert_is_the_same_at_every_block_size`
  (block 1 against 512: max difference 0.455 at sample 99 486).
- **Cause:** lanes configure units between passes; memory was asked for only after a
  snapshot or a supply, and `follow_lates` were rebuilt only by `compensate()`.
- **Fix (root):**
  - the automation runtime (built off the audio thread with the snapshot) works out, for each
    Delay whose `note` or `sync` a lane drives, the note with the longest line the lanes can
    reach while synced (`FxCopy::line_note`: each lane's base and its pieces' ends, or its whole
    range when a modulator drives it); `auto_install` sets it as the unit's floor
    (`EffectUnit::set_line_floor` → `StereoDelay::set_line_floor`) and asks for memory then
    (`request_memory` moved from before `auto_install` to its end). The line exists before the
    song gets there, so offline equals real time. The floor holds with the sync off too (a
    lane switching the sync asks for nothing); a Delay nobody automates is untouched (null);
  - after the automation's configure loop each configured unit asks for what its settings
    need (`msg::ask_memory`), and a unit whose `follows_song` flipped re-plans the passes
    (`scope_delays`) before the pass's end is worked out;
  - the offline render answers `EffectNeed` during the song too (taken at its next block).
- **Tests:** the two above (now green: the echo at 12.0 s, real time with varied blocks equal
  to offline to the bit, the quarter's line given back once; every block size 1 … 4 096 the
  same to the bit).

## A-M3 — `StereoDelay::supply` and `reset` did O(line) work on the audio thread

- **Reproduced:** yes, measured on the base unit (optimised test build): a 48 s line full of
  echoes — `supply` 22.9 ms, `reset` 1.18 ms in one call (a 128-frame callback is 2.7 ms).
  The reproduction was a timing probe, removed; the kept tests count cells.
- **Cause:** `supply` copied the history (2 × 2.3 M cells, two `%` each) and `reset`
  zero-filled a non-silent line, both inside one callback.
- **Fix (`crates/cantus-dsp/src/delay_line.rs`, new):** a line offered while the old one holds
  sound waits beside it (`Move`); each call first copies the frames it will process plus
  `MAINTAIN_CELLS` (32 768) more of the history, oldest first, in `memcpy` runs, and plays on
  the old line meanwhile; when the copy has caught up the lines swap before that call runs
  and the old one waits in `spent`. A reset of a loud line marks the cells not written since
  as stale: a call's reads see them as zeros (a masked, monomorphised copy of the loops; the
  unmasked ones are the instructions they were) and each call clears its frames plus
  `MAINTAIN_CELLS` from the top; skipping clears what it passes. A silent line still swaps
  at once. The engine gives the old line back off the audio thread: `supply_effect` takes a
  waiting one before supplying, `Processor::sweep_spent` runs after each block while a unit is
  busy (`fx_busy`), and the plug-in's memory thread takes it in `deliver`. Per call: at most
  `2 × (frames + 32 768)` cells for each of the two (`StereoDelay::work`).
- **Tests:** `delay::tests::a_long_line_moves_in_a_bounded_stretch_per_call` (24 s → 48 s line
  after 27 s of sound: every 128-frame call within the bound, 35 + calls, output equal to the
  unit with the 48 s line from the start, the 24 s line given back) and
  `a_reset_clears_a_long_line_a_stretch_per_call` (the reset itself touches nothing; at rest at
  once; skipped and played on, equal to a new unit to the bit); `a_line_that_grows_mid_song`
  now also takes the old line from `take_spent`; `rt_memory::a_synced_delay_never_allocates`
  (a loud Delay synced by a snapshot, moved in, automated to 4/1, a panic, a seek, a stop —
  no allocation or free on the audio thread).

## A-L1 — sync switched on took its target from a stale tempo

- **Reproduced:** yes. `delay::tests::sync_switched_on_crossfades_once_to_the_tempo_it_is_told`:
  free at a song of 90 BPM (never told — the unit does not follow the song while free), then
  synced and told 90 before the next pass: the crossfade went to 24 000 samples (a quarter at
  the stale 120), the 90-BPM 32 000 waiting behind it.
- **Cause:** `set()` turning the sync on crossfaded at once to the note at `self.tempo`,
  which nobody updates while the sync is off.
- **Fix:** turning the sync on arms the head (`arm`): the first `set_tempo` after it (the
  engine tells the pass's tempo before the pass) crossfades once, to the note at that tempo;
  a unit never told one goes at its next `process` on the tempo it has. `at_rest` is false
  while armed. ADR 0119 §3 amended.
- **Tests:** the one above (one crossfade, straight to 32 000, settled after 50 ms; never told,
  it goes at the next pass).

## What was run (Windows, this worktree)

- `cargo test -p cantus-dsp`: green (lib 839 passed, 14 ignored; golden hashes unmoved,
  `DSP_VERSION` 6).
- `cargo test --no-fail-fast -p cantus-engine -p cantus-plugins`: 1 244 passed, 0 failed
  (130 test binaries; `rt_memory`, `fx_delay_sync`, `record_tap`, `hear_key`, the null and
  golden tests, Delay's plug-in equivalence and `transport.rs` among them). A first run had
  stopped at `inst_stream::the_suite_leaves_no_cache_folder_behind` (its child run failed
  while two other builders' suites loaded the machine); run again alone it passed, 5 of 5 —
  nothing here touches streaming.
- cantus-ui: `delay_sync_e2e` (1), `delay_window_e2e` (7), `taps_e2e` (1), `pad_track_e2e`
  (2): green.
- `RUSTFLAGS=-D warnings cargo clippy --all-targets -p cantus-dsp -p cantus-engine -p
  cantus-plugins`: clean.
- `cargo fmt -p` on the three crates; it also reformatted `cantus-engine/src/backend.rs` and
  `tests/pdc_lazy.rs`, which this branch does not touch (already unformatted on the base):
  left as they were, for the integrator.

## Verified only in emulation

Everything here is engine and DSP behaviour, held by tests on the processor driven by hand
and on offline renders; no device was involved. The owner may want to listen to a synced
Delay automated from a quarter to a whole note in a song, and to *Hear the key* switching
itself off on a strip while it records After FX (the inspector does not say so yet — a `look`
or `fix-b` follow-up if the owner wants a word there).
