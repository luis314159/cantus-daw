# Round 31 — the macOS crash-test flake (`r31/mac-flaky`)

**The flake.** The macOS runner (a VM), beta.6's run: `cargo test -p cantus-sandbox --test
session` —
`a_crash_mid_song_keeps_cantus_playing_says_so_once_and_reload_brings_back_what_was_set`
failed at `session.rs:116`: "before the crash (0 late blocks; the bell waits by os_sync)
left: 20478, right: 0". In the first 40 blocks of 256 frames (20 480 samples) of the gain
isolated `Individually`, all but the first frame's two zeros differed from the in-process
−6 dB reference, while the host counted no late block. It passed on retry; it was never
seen on Windows or Linux.

## The cause

The test's `gain_rig` sets the gain's Latency to 37 and then "settles" with a fixed 64 blocks
(`Rig::settle`: 64 blocks and frames, then `until_loaded`, which looks only at loads and
moves). A new latency makes the plug-in ask for a restart. Cantus takes its unit out of the
engine and puts a wire in its place (`PluginHost::begin_restart`). An isolated plug-in's
restart is then a call that its helper answers frames later
(`BridgeMain::restart`, `Call::Restart`). Until the answer is in, every block passes dry,
**and on time**: no job is sent, so none can be late. Here the restart is done two blocks
after it is asked for (~3.5 ms), and the 64 blocks take ~14 ms. On the VM, the helper's two
round trips (the flush that carries the latency, then the restart) took longer than that.
The crash test then played 40 blocks of a wire against the gain: 20 478 samples differed,
with 0 late blocks.

Round 30 found the same race in the isolated-insert and freeze tests ("the latency arrives
with the answer to a later call") and gave them `Rig::until_latency`. The crash test builds
its rig through `gain_rig` too, but it never got that wait.

**Not a user bug.** A restart passes dry in process too: the in-process reference is
restarting for one block. That is the design: a CLAP plug-in is deactivated to be
restarted, and a wire stands in. Isolated, the gap lasts as long as the helper's answer
takes, which is longer on a slow machine or with a plug-in that is slow to activate. Loads
are not affected: a state that asks for a restart is restarted inside the load, before the
first job (`Server::load`). So an isolated plug-in's first blocks are not wrong on a slow
machine. What can be heard is a short dry passage when a plug-in changes its latency while
it plays, and that happens in process as well.

## The evidence

- Measured here (a temporary diagnostic, now removed): in-process, the restart is asked for
  after block 0 and done after block 1; isolated `Individually`, it is asked for after
  block 0 (1.8 ms) and done after block 1 (3.5 ms). The 64 settle blocks take 13.6–15.1 ms.
  The fixed count worked only because the restart usually finishes in the first 2 of the 64
  blocks.
- Forced on Windows: the test gain's new `CANTUS_TEST_GAIN_SLOW_RESTART_MS` makes every
  activation after an instance's first take that long. That stands in for a plug-in slow
  to restart, or a helper whose main thread a busy VM runs late. With 1 000 ms, the new
  test (the crash test's opening: `gain_rig(Individually)`, then 40 blocks against the
  reference) failed 3 runs out of 3 with the macOS signature, to the sample:
  `the gain from the first block (0 late blocks; restarting: true; latency 0) left: 20478
  right: 0`.

## The fix (test rig only; no product code changed)

- `gain_rig` ends with `rig.until_latency(id, 37)`: the restart has been asked for and
  done, the new unit is in, and the latency is reported and compensated.
- `Rig::until_latency` also waits while that plug-in is restarting. `Rig::until_loaded`
  (and so `settle`) also waits while any hosted effect or instrument is restarting
  (`Rig::restarting`). Both now wait for a condition, not for a number of blocks.
- The crash test's message now says whether the plug-in was restarting, and what latency it
  had, as the song started. If it ever fails again on macOS, the log will say whether this
  cause is back.

## The test that holds it

`session::a_restart_its_new_latency_asked_for_is_done_before_the_song_is_compared`: it
compares `gain_rig(Individually)` with a 1 s restart, block by block, against the in-process
reference. Before the fix (commit `65e63191`) it failed every time with 20 478 / 0 late
blocks. After the fix (`43d6ccf6`) it passes. The `session` binary passed 20 runs out of 20
in a loop.

## Suites

- Windows: `cargo test -p cantus-sandbox -p cantus-rt -p cantus-host --no-fail-fast`:
  304 passed, 0 failed, 2 ignored. `cargo clippy -p cantus-sandbox -p cantus-rt -p
  cantus-host -p cantus-test-clap --all-targets -- -D warnings`: clean. `cargo fmt --check`
  on those crates: clean. `--test session` run 20 times in a loop: 20 passed out of 20.
- Linux (WSL, `tools/wsl/check.sh r31/mac-flaky`, its own clone): the whole workspace
  passed 4 690 tests, 0 failed, 56 ignored. Clippy `-D warnings` was clean, and so were the
  plug-ins built without their editor.
- macOS: not run here. The next `macos.yml` run will show whether the `FLAKY` line for
  this test is gone. If it ever fails again, its message now says whether the plug-in was
  restarting as the song started.
