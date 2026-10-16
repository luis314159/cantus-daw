# Round 28 — fix6: the review of fix5, and two tests that failed under load

Branch `r28/fix6` (worktree `F:\projects\daw-r28\fix2`), from `r28/integration` at
`4e5e243`. The six confirmed findings of `reviews/fix5-1.json` (four after
deduplication), then the integrator's two failures from the full Windows run (one also the
only failure of the full WSL run). "Seen failing" means against the old code or, for a
test that names something new, against a mutation that brings the old behaviour back.

**Schema:** none. **Golden hashes / DSP_VERSION:** none moved.

## The review of fix5

| Finding(s) | Fixed how | Test |
|---|---|---|
| 0, 3 — the pinned strip's probe noted the mark asked for, drawn or not | `PinMod::paint` notes a mark in the arms that draw it: the edge tick and the fader's mark when there is a live value, the knob when its dot and rings come from the offer | `modulators::the_pinned_strip_shows_its_modulation_the_desks_way`: gain, pan and width modulated → volume `edge`+`fader`, pan `edge`+`knob`, width `edge`; nothing modulated → no mark; only the width → only its mark. Fails with the fader's arm deleted (and, by its second half, with the modulator removed) |
| 1, 4 — `Transport::wrap_to` had no test | `transport::tests::a_forced_wrap_counts_and_its_stretch_ends_where_the_playhead_was` (played from beat 12 past a loop 8–9: the forced wrap counts, the audio before it is beat 12's; a natural wrap; the loop's end moved behind the playhead at 8½: forced again, the stretch the edit cut ends at 8½); `master_chain::a_forced_wrap_is_heard_where_the_playhead_was` through the processor (a `SetSnapshot` that moves the loop's end behind the playing playhead, 12 000 samples of latency: heard at 8½ less 11 000 samples). Both fail with `wrap_to` back to `seek` | the tests themselves |
| (with 1, 4) — what a forced wrap's stretch *is* | To give that test a right answer, each wrap now records where the playhead was when it came (`Wraps` keeps it beside each kept pass's length, and the lead-in's end): the loop's end plus the transport's sub-sample carry for a natural wrap, the playhead's position for a forced one. A late insert's position, and the heard one, is that end less the samples back. Before, every stretch was taken to end at the loop's end — wrong for a forced wrap (played from past the loop, a loop edited behind the playhead) and a sub-sample off for the others | the two above; `fx_tremor`'s frame-by-frame check below now agrees with the transport to a quarter of a sample |
| A point the review raised and I did not do as asked | A processor test played *from past the loop's end* with a late gated insert cannot tell `wrap_to` from `seek`: before the forced wrap nothing has played (the audio the gate hears is the silence before the start), and the heard position is capped at what has played. The processor-level test therefore forces the wrap mid-play, by moving the loop's end behind the playhead, where the stretch before it was really played | — |
| 2 — the 32nd-note loop test's doc claimed it held pass lengths | It now holds them: beside the gate's frame check, the fold the gate is given (`SongPosition::earlier` at 960 frames, built from a transport played sample by sample) is checked against where the transport was 960 frames before, frame by frame, to a quarter of a sample. A pass counted a sample long fails the test | `fx_tremor::a_gate_later_than_the_loop_never_hears_passes_not_played` — fails with each kept pass stored a sample long |
| 5 — the heard playhead still folded by `rem_euclid` | `Processor::heard_position_f64` asks the same `Wraps::locate`: through the wraps played, to where each stretch ended; unfolded with no wrap since the start (a tempo change on a first entry) | `master_chain::the_heard_position_folds_only_through_the_wraps_played`: a one-beat loop behind 60 000 samples of latency, 6 000 samples after the first wrap 6.75 (the old code said 8.75), after two 7.75, after four 8.75; a tempo change on the first entry into 8–16 heard at 7.6 (the old code said 15.6) — seen failing |

ADR 0057 amended (fix6).

## The two failures under load

- **`cantus-dsp` `chorda::tests_piano::sixty_four_pedal_down_voices_fit_a_quarter_of_a_core`**
  (line 238, Windows full run and the WSL run): the best of three 4-second runs, wall
  time — on a machine running a build and a Linux test suite beside it, every 4-second
  run was preempted past the bound. It is now the best of 48 quarter-second stretches
  (the same 12 s of audio, the same 64 ringing voices): a preempted stretch only takes
  longer, and over short stretches one runs unpreempted. The same rule as round 23's CPU
  tests (`inst_epiano`, `inst_organ`: best of five), finer. Wall time with
  `std::time::Instant`, the same on Linux. The bound (0.6 of a core in a test build; the
  ignored release measure keeps 0.25) is unchanged; it measures 17–19 % here.
- **`cantus-ui` `hosted_instruments::opening_its_editor_from_the_track_opens_the_plugin_window`**
  (line 147, after fix5's 60 s polling): F8 closes a plug-in's window only when it is in
  front (round 27, ADR 0051), and "in front" means no other program's ordinary window is
  over its middle. A test process is never the foreground program, and Windows lets no
  background program put a window over the user's (the foreground lock), so wherever
  another program's window covered that point F8 kept bringing the window forward and
  never closed it. Waiting longer could not change that. In the app the click asking for
  the editor is on Cantus, which has the foreground. I could not reproduce it here: the
  old test passed with a maximised Notepad started beside it, and with the test started
  hidden and minimised (a process's first `ShowWindow` takes its launcher's show
  command). So the cause is reasoned, not shown. The test now makes "in front" the
  window's own state, not the desktop's. It shows the window without taking the focus,
  puts it always on top (over every ordinary window; an always-on-top window over it does
  not count as covering), and handles the thread's window messages as the app's frame
  loop does. It then asserts that the window is in front and that F8 closes it.
  Covered-and-brought-forward stays tested where the test owns the covering window
  (`cantus-host` `clap_window`, `quick_gestures`).
  **Ten runs in a row under load** (the test binary, all four tests, while a
  `cargo build -p cantus-engine -p cantus-dsp --tests` compiled with 4 jobs): 10 of 10.
  The Chorda test, ten times under the same load: 10 of 10 (17.7–18.5 % of a core).

## The runs

`cargo fmt --all`; `cargo clippy --workspace --all-targets -- -D warnings` clean (Windows;
the Linux check is the integrator's). Tests, all passing: cantus-dsp + -engine 1 371;
cantus-ui 1 291 in full.

## Left

Nothing that is not the owner's.
