# Round 28 — fix7: the review of fix6

Branch `r28/fix7` (worktree `F:\projects\daw-r28\fix2`), from `r28/integration` at
`2611f95`. The six confirmed findings of the review of fix6 are two distinct ones. (The
findings file `reviews/fix6-1.json` was not on disk, so I worked from the brief.) "Seen
failing" means against the old code or against a mutation that brings it back.

**Schema:** none. **Golden hashes / DSP_VERSION:** none moved.

| Finding | Fixed how | Test |
|---|---|---|
| `Transport::wraps()` answered only while a loop lay ahead, so a loop turned off or moved ahead of the playhead just after a wrap hid the wraps still in flight. The heard playhead and late inserts' positions then went straight back from the playhead | `Transport::wraps_played()` keeps the history readable whatever the loop is now. `Processor::heard_position_f64`, `song_position_of` (→ `SongPosition::wrap`) and the processor's per-insert pass cuts all read it. With no loop ahead the cuts are no longer capped by the next wrap. The passes older than the 32 kept are counted at the length of the loop the last wrap came in (`Transport::wrapped_loop`), at the tempo now. A start or a seek still forgets the wraps. No allocation: `Wraps` is `Copy` | `master_chain::the_heard_position_keeps_the_wraps_played_when_the_loop_changes`: 12 000 samples of latency, a loop 8–9 turned off or moved to 12–13, 1 000 samples after a wrap → 8.54 (the old code gave 7.54); 12 000 samples later, straight. `transport::tests::the_wraps_played_outlive_the_loop`: the late insert's `earlier(12 000)` gives the same answer, and a seek or a stop and start forgets the wraps. Both cases of both tests were seen failing. End to end, `fx_tremor::a_late_gate_hears_the_wrap_played_after_the_loop_is_turned_off`: a gate 240 samples late, loop turned off or moved ahead 100 samples after a wrap, renders to the bit what it renders with the loop kept on, at blocks of 1, 7/37/100/1024, 64/333, 512 and 1 024. It fails with the fold taken back to `wraps()` and with the pass cut taken back to `wraps()` |
| Chorda's CPU test said 47 blocks / 12 s, but took 46 blocks (11.78 s) | `div_ceil`: 47 blocks of 256 (12 032 samples) per stretch, asserted, 48 stretches (12.03 s) | `sixty_four_pedal_down_voices_fit_a_quarter_of_a_core`. Release: `measure_sixty_four_pedal_down_voices` gave **17.2, 17.1, 17.4 %** of one core (this PC, three runs). That is now in ADR 0059's table |

ADR 0057 amended (fix7); ADR 0059's table amended.

## The runs

`cargo fmt --all`. `cargo clippy --workspace --all-targets -- -D warnings` is clean on
Windows. `cargo test -p cantus-engine -p cantus-dsp`, in full: 1 374 passed, 0 failed
(fix6 had 1 371; this round adds 3).

## Left

Nothing that is not the owner's.
