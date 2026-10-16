# Round 28 — fix5: the review of fix4

Branch `r28/fix5` (worktree `F:\projects\daw-r28\fix2`), from `r28/integration` at
`a2a4519`. Every confirmed finding of `reviews/fix4-1.json` (13, deduplicated to eight),
then three the integrator added (Linux clippy on the VST3 test, two tests that failed only
under load). "Seen failing" means the test was run against the old code and failed; where a
test names something new, it was seen failing against a mutation of the new code that
brings the old behaviour back (said per case).

**Schema:** none (32 stays). **Golden hashes / DSP_VERSION:** none moved (no golden render
loops behind a latent insert; the desk's fader mark is the same drawing, moved into the
kit). **Demos:** three changed note lengths (below), every loudness test passes.

## The findings

| Finding(s) | Fixed how | Test |
|---|---|---|
| 2, 5 — a recording anywhere on a track with take lanes was flagged as a comp window, so a comp switch took it away | `record_pass::lay` flags the pass only when a clip on a take lane overlaps its own stretch — what it stacked, the loop's earlier passes, takes already under it. A pass where no take lies is a plain clip, as under schema 31 | `rec_pass::a_pass_recorded_away_from_the_takes_plays_under_every_comp` (takes at beats 2–6 and two comps; a chorus at beat 20 is no window and plays under A and B; a loop at beat 20 still flags its last pass) — seen failing (`[(0, true)]`). ADR 0058's fix4 amendment now says "its stretch over a clip on a take lane"; the manual EN/ES says a recording where no take lies plays under every comp |
| 10 — the Spanish heading read "what became of the comp" | "**Qué forma parte de la composición.**" | — |
| 11 — cutting notes at every chord change left holes where the bass stays (Aurora's sub, Morning Light's left hand on Asus4 → A) | `Laid::held(beat, pitch)`: a note rings on through every change to a chord that still holds its pitch class (bass included, `Chord::holds`), to the chart's end at most (the next section has its own chart); `comp`, `bass` and `arp` use it per note (`Laid::left` is gone) | `parts::tests::a_note_the_next_chord_still_holds_rings_on` (the 3.9-beat sub and the 3.8-beat octaves whole over Asus4 → A; the comped sus fourth still lets go; nothing past the chart) — seen failing; `no_note_rings_into_a_chord_without_it` (was *…into_the_next_chord*) walks every chord a note sounds under |
| 0, 1, 12 — `earlier` folded back through passes never played (an insert later than the loop is long, right after the first wrap) | `fx::Wraps` in `Transport` and `SongPosition::wrap`: the wraps since a start or seek, the samples since the last, the exact lengths of the last 32 passes. `Wraps::before_wrap` walks back through the passes played, no further — before the first wrap the audio is the lead-in, played straight. `earlier` and the processor's per-insert pass cuts both ask it. A wrap forced at a pass's first sample counts too (`Transport::wrap_to`; it used to seek and forget the wraps) | `position_tests::earlier_folds_only_after_a_wrap_…`: one wrap 6.75 (unfolded), two 7.75, three or more 8.75, passes of 24 001 samples moving the boundary by their sample; `transport::tests::the_wraps_are_the_ones_played_at_their_samples` against a real transport played sample by sample (exact over the kept passes, with the tempo changed at every wrap too); `fx_tremor::a_gate_later_than_the_loop_never_hears_passes_not_played` (four Flexus, 960 samples, a 32nd's loop of 703⅛ samples) — fails with the wraps uncapped |
| 4 — the tempo-change limit fix4 left under "Left" | fixed for everything the engine can know exactly: passes are folded by their real lengths, so a tempo change or a fractional loop length between kept passes is exact. What stays is a documented limit in ADR 0057, measured: passes older than the last 32 are counted at the loop's length now — within 1 sample over 205 wraps at a steady tempo, 112 samples at most for a tempo change 60 passes back on a 1/16-beat loop (only an insert later than 32 passes meets it; `pdc::MAX_DELAY` over the shortest pass is unbounded, so no fixed history covers it). fix4.md's "Left" points there | the transport test above prints and bounds both |
| 7 — the late-gate loop test would pass with the fold removed | `fx_tremor::a_late_gate_hears_each_wrap_where_its_audio_wraps`: frame by frame the gate is shut or open as the beat of the audio it hears — the real transport's position `late` frames before — says, in a 4⅓-beat loop entered from beat 0 (more than 100 sounding frames behind a wrap checked); the first entry equals the song played without the loop, to the bit, at irregular blocks and at blocks of 7 | fails with the fold removed (*frame 142 500 hears beat 6.32: open*), and with a fold on the first entry (blocks of 7); the old block-size test passes with the fold removed, as the review said |
| 3 — Chorda with no designer showed "designing 0 %" forever | `chorda::design` → `Ready / Coming(done) / Refused / NoDesigner`; `progress` and `refused` follow it; the caption says `chorda.no_designer` ("the string designer could not start: keeping this one" / "el diseñador de cuerdas no pudo arrancar: se queda esta") | engine `a_tuning_no_designer_can_make_is_not_on_its_way` with `faults::set_spawn_fails` — seen failing (`Some(0.0)`); UI `a_tuning_no_designer_can_make_says_so_never_zero_percent` (the caption's words for each state, EN and ES). ADR 0059 amended |
| 6 — the space-label test was still tautological | the tautological loop is gone from the unit test (it keeps the Spanish label's width against the old 110 pt column); `samples_browser::the_details_labels_are_drawn_whole_and_clear_of_their_values` draws the content window and reads the painted text: every label whole (none elided) and 8 pt clear of its value, the values in one column, in Spanish and English | passes; fails when `details()` goes back to a fixed 110 pt column (*not painted whole*) |
| 8 — the pinned fader's live-mark test compared a function with its body | the desk and the pinned strip draw one mark (`kit::fader::live_mark`); `Live::at` gives each way's point; the probe records which mark each pinned control draws (`automate::probe::note_live`) | unit: the fader's way sits at the desk's cap centre at 0 and 1, the edge tick does not; `modulators::the_pinned_strip_shows_its_modulation_the_desks_way` draws the inspector — fails when the pinned fader is drawn `Live::Edge` |
| 9 — the top bar's grid field named a track under the hint "Grid" | with a Quantise-to track the hint's name is "Quantise to — Pitch editor", its value the whole track name (the field may elide it) | `topbar::the_pitch_editors_track_target_shows_and_a_grid_picked_lets_it_go` extended — seen failing |

## From the integrator

- **Linux clippy on `cantus-plugins/tests/vst3.rs`** (`unnecessary_cast`, u32 → u32 at the
  ProcessContext flags): the file-level `#![allow(clippy::unnecessary_cast)]` with the
  one-line reason, as `src/vst3/plugin.rs` and `cantus-host/src/vst3/mod.rs` have. Every
  other file naming `Steinberg` was checked: `cantus-host/src/vst3/*` sit under the
  module's allow, `cantus-test-plugins/vst3` has a crate-level one, `cantus-plugins`'
  `timer.rs` a local one, `view.rs` and `tests/notes.rs` cast to other widths (u16, i16)
  only. Not run in WSL here (`~/cantus-ci` is the integrator's, busy).
- **`fx_speculum::the_copy_allocates_nothing` under load**: the pool runs a pass in
  parallel only once a worker is available (`Pool::run_pass`, `avail == 0` → serial), and
  under a full run the two workers took longer to start than the 16 warm blocks. The test
  (already `Workers::parallel(2)`) now warms until both workers joined (bounded, 120 s)
  and asserts **every** armed block ran on the pool (30 of 30), so the allocation count
  covers the workers' path for certain.
- **`hosted_instruments::opening_its_editor_from_the_track_opens_the_plugin_window`
  under load** (its last line, the window still open after F8): F8 closes a plug-in's
  window only when it is in front (round 27, ADR 0051); under a full run other test
  binaries' windows (plug-in editors opened at Windows' default place, the z-order tests)
  can lie over its middle, and then F8 rightly brings it forward first. The test presses
  F8 until the window closes (bounded, 60 s) and checks nothing else opened; the first
  assertion now prints the app's status, so a failed open says why. Its plug-in folder
  was already a fresh temporary one per test.

## The runs

RUNS
