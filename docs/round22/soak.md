# Round 22 — soak: long sessions, crash recovery, files refused cleanly

*Builder **soak**, branch `r22/soak` (base c8bc9f2). BETA.md blocker 3; ADR 0006 (amended),
ADR 0025 §8.*

## What was built

| Piece | Where | What it does |
|---|---|---|
| **Session journal** | `cantus-ui/src/recovery.rs` | Each running Cantus writes `<data>/sessions/<pid>-<ms>.json` (atomically: temp + rename) beside a `.lock` it holds with `File::try_lock` for its whole life — the OS releases it however the process ends, so an unlocked journal is a session that is gone, and a running one (a second Cantus) is never mistaken for a crash. It records the project, whether it is dirty, the last autosave (path, time), and every recording pass whose takes are in no file yet (track → WAV, record/transport start, loop, punch, take mode, tempo, rate, latency). Written only when one of those changes (nothing is formatted on an ordinary frame). |
| **Recovery offer** | `recovery_restore.rs`, `recovery_session.rs`, `views/recovery.rs` | At start (boot, and `CantusApp::new`) the newest gone session with something to give back is offered: *Recover your work* — "Cantus closed unexpectedly while “Song” was open. Changes autosaved 3 min ago. 2 recorded takes that are not in any saved file." — **Restore** / **Discard**. Restore loads the last save, puts the autosave over it as **one undo step** (`ReplaceProject`: Ctrl+Z shows the last save), and lays the takes out exactly as a finished pass is laid out (`record_pass::build`, the loop's wraps recomputed from the tempo, so loop takes land on their lanes); a take whose track is gone comes in on a new track (`import_audio`). Discard deletes no file. Offered once. |
| **WAV repair** | `recovery::repair_wav` | A crash leaves the header at its last flush (≤ 0.5 s old, ADR 0025 §8) while the frames written since are on disk; recovery sets the RIFF and `data` sizes from the file's length, to the last whole frame. |
| **Close with unsaved work** | `recovery_session::close_session` | Before: the window's close wrote a silent autosave **that nothing ever offered back** — closing without saving lost the work in practice. Now a pass in progress is finished (files closed, clips placed), the autosave is written, and the journal stays, marked *closed*, so the next start offers it ("…had changes that were not saved when Cantus closed. They were kept."). |
| **Autosave off the frame** | `cantus-ui/src/autosave.rs` | Found by the soak (below): the autosave froze the window for 0.2–1.8 s (up to 8.9 s with the disk busy) every interval. The frame now takes only the copy; a `cantus-autosave` thread serialises, writes, fsyncs and verifies; the next frames collect the result. One write in flight; the close waits for it; a result for a project replaced meanwhile (an `epoch` bumped by open/new/demo/restore) is ignored. |
| **Autosave fallback, failures said** | `store::autosave`, `autosave.rs` | An autosave that cannot be written beside the project (folder gone, read-only, drive unplugged) goes to the fallback folder named after the file, and the status line says so once. A failing autosave is an error on the status line, not only a log line. |
| **Refusals named** | `store::Problem`, `recovery_text.rs` | `StoreError::problem()` → truncated / damaged / newer (found, supported) / not a project / integrity / I/O kind. *Open* says what is wrong, that **the file and the open project are unchanged**, and names a readable `.bak`/`.autosave` beside it. A project on the command line that cannot be opened now says so on the status line (it only logged, and the demo opened in silence). EN and ES. |
| **No data loss on save** | `store::write_verified` | Saving over a file that is itself damaged no longer pushes the last good save out of `.bak` (the damaged one goes to `.corrupt`); a failed write removes its `.tmp`. A save that fails says where and why ("permission denied", "the disk is full"), and "Nothing was lost — the project is still open and unsaved". |
| **Fast undo after a remove** | `CantusApp::sync_engine` | Found by the soak: an effect removed and put back by undo within one engine round-trip stayed **silent** until the next edit. The app now re-sends the snapshot when an effect of the last snapshot is unknown to the engine (the engine-side cause is reported below). |
| **Open/new during a pass** | `app.rs` | Opening a project, a demo or *New* while recording finished the pass into the *new* project's tracks; it is finished into the project it was recorded into first. |
| `CANTUS_DATA_DIR` | `app::directories_fallback` | Moves every per-user file: the soak and the crash tests never touch the user's folder. |

## The soak harness

`crates/cantus-ui/tests/soak.rs` (+ `tests/soak_actions/`, `tests/harness/`):

```text
CANTUS_SOAK_MINUTES=40 cargo test -p cantus-ui --release --test soak -- --ignored --nocapture
```

The real `CantusApp`, headless: whole frames of a 1600 × 900 window at 60 Hz; a manual
engine whose processor runs on its own thread at the pace of a 256-frame callback, fed a
fake input (a ramp); the demo songs looped and switched every 3 min; every 6th frame a
seeded random edit through the interface's commands and methods — notes (insert/remove),
clips (create/delete/duplicate), automation clips, the mixer (fader, pan, mute), effects
(added, removed, **swapped** among 11 kinds, chains up to 6), device windows (effect and
instrument editors opened/closed, the central view switched), undo/redo storms (10–70
each way), transport (play/stop, seek, loop); a recording pass of 2–5 s every 1.5–2.5 min
on an armed audio track; a save → open round-trip every 45–90 s (asserted equal); the
autosave every 10 s. Every 10 s a row: working set, private bytes, handles, threads
(PowerShell on a thread — the loop never waits for it), frame p50/p99/max, callback
overruns and its slowest block, the engine's overruns and dropped events, effect units
known, undo history, tracks, seconds recorded → `soak.csv`. It fails on a panic, a frame
over 10 s, the callback stalled 5 s, a save/open that does not round-trip, and (≥ 10 min)
private bytes growing more than 96 MB + the recorded audio, handles > 200, threads > 8
after warm-up. `soak_smoke` runs the same script for 20 s in the ordinary suite.
`CANTUS_SOAK_SEED` replays a session.

### Runs (this machine, i7-12700H, while 5 other builders compiled — see the caveat)

Three 40-minute release runs, seed 22 (`target/soak-run{1,2,3}/soak.csv`, 240 rows each):

| Run | Build | Frames | Actions | Private bytes (warm-up → end) | Handles | Threads | Frame p99 (mean of rows) | Frames > 250 ms | Slowest frame | Engine dropped | Result |
|---|---|---:|---:|---|---|---|---:|---:|---:|---:|---|
| 1 | first harness (sampler joined on the loop) | 46 247 | 7 520 | 149 → 178 MB | 231–236 | 7–11 | — ¹ | — | 2.56 s | 79 | pass |
| 2 | sampler fixed, slow frames logged | 96 278 | 15 682 | 161 → 171 MB | 226–231 | 7–10 | 28.4 ms | **260** (195 with no action: every 10 s, the autosave) | **8.88 s** | 60 | pass |
| 3 | **autosave on a thread** | 125 546 | 20 313 | 167 → 193 MB ² | 224–229 | 7–11 | 22.5 ms | **62** | **1.11 s** | 46 | pass |

¹ Run 1's rows are distorted: the loop waited 10–20 s for PowerShell every sample.
² With 40 s of recorded audio held decoded by design (≈ 15 MB, finding E3); between
samples it moves ±15 MB with the demo loaded. Every run: 20–21 recording passes, 31–37
save → open round-trips (all equal), 12–13 demo switches, 155–392 undo storms, ~1 300–1 700
device-window opens/closes, ~800–2 200 effect add/remove/swap — no panic, no hang.

**Curves.** Handles and threads are flat for the whole 40 minutes in every run (a pass adds
the writer thread and one file handle and gives them back). Private bytes rise for the
first ~4 minutes (caches, every effect kind met) and then wander within a ±15 MB band
over a slope that matches the audio recorded; no step at demo switches, opens or undo
storms. Run 3's frames: p50 3–7 ms, p99 12–24 ms in most rows; the 62 frames over 250 ms
are scattered (43 with no action that frame, 4 after a demo switch, 4 after a mixer move),
none periodic — the machine (six builders compiling) is the likeliest cause; under a quiet
machine they should be re-measured. The callback's slowest block was 60–191 ms in every
run — preemption of an ordinary thread on a loaded box, not a measurement of the engine.

**Caveat.** The other builders kept 6–17 `rustc`/`cargo` processes running for the whole
session. The callback thread is an ordinary thread (no MMCSS; that is the backend's), so
its overruns (≈ a third of blocks at 256 frames) measure the machine, not Cantus — the
pool builder's live runs are the performance evidence. What the soak asserts is shape:
no growth, no hang, no panic, no lost save.

## Crash recovery, proved by killing the process

`crates/cantus-ui/tests/crash_recovery.rs`: the test binary spawns itself as a child that
runs the app headless (frames, journal, autosave every second) and prints what it does;
the parent `TerminateProcess`es it (`Child::kill`) and starts a session over the same
data folder, as the next launch would.

- **Mid-edit** (an edit every ~150 ms, autosave every 1 s): 32 edits, killed after edit 32
  at 5.1 s; the journal said *crashed, dirty* and named `song.cantus.autosave`, which held
  edit 27 — **618 ms** before the last edit (bound asserted: the interval + the slowest
  frame + 300 ms; slowest frame 380 ms in debug). The project file was untouched. The next
  launch offered it; *Restore* put edit 27 in place over the last save (dirty; Ctrl+Z →
  the save; Ctrl+Y → back); offered once.
- **Mid-recording** (a live manual engine, 256-frame blocks, a ramp on input 1, the track
  armed, ~3 s recorded): the WAV read back **144 640 frames (3.01 s) from its header**, every
  frame exactly the input frame it should be (the take began at input frame 7 680; a second
  before the kill 108 544 had arrived — the header is within the 0.5 s flush). Repair
  extended it to **148 736** frames (+4 096 already on disk past the last flush), still
  frame-exact. The next launch offered "1 recorded take"; *Restore* put it on *Voice* as a
  clip of that file.

`tests/recovery_flows.rs`: the journal's life (clean exit leaves nothing; unsaved work at
close is offered and restored; Discard keeps the files; a running session is never taken
for a gone one); *Open* refusing a truncated, damaged, newer, foreign and empty file with
the session and the file untouched (and in Spanish); a damaged file with a good `.bak`
names it; a save into a blocked folder and into a folder whose ACL denies writing (Windows,
`icacls`) fails in words and loses nothing; the autosave falls back and says so; which new
WAV is which armed track's; loop wraps; WAV header repair.
`crates/cantus-project/tests/refusals.rs`: a file cut at every byte of its first 256 and
every 211th after is *truncated* and untouched; damaged, non-UTF-8, wrong shape, newer
schema, foreign JSON, missing file, hash mismatch each classified; a save over a damaged
file keeps the good `.bak`; the autosave fallback; a failed save leaves no `.tmp`.
`tests/soak_findings.rs`: the fast-undo effect (fails without the fix — checked), the
autosave off the frame (and a late result for a replaced project ignored), the autosave's
cost per demo.

## Findings

1. **Closing with unsaved changes lost them in practice** (app): the close wrote an
   autosave nobody ever offered. Fixed (journal + offer). Test: `a_clean_exit_…`.
2. **No crash recovery existed at all** (app): autosaves were written, never offered. Fixed.
3. **The autosave froze the UI** 0.2–0.7 s per demo song in release (measured: house
   436 ms, ballad 362, funk 710, lofi 205, ambient 355, trap 536 — 170–600 KB of JSON
   serialised ~4 times, fsynced, read back and parsed), 0.7–1.8 s and up to 8.9 s in the
   soak with the disk busy. Fixed (thread). Test: `the_autosave_is_written_off_the_frame`.
4. **An effect removed then undone quickly stayed silent** until the next edit (handle /
   engine race, below). Mitigated in the app; test fails without it.
5. **Autosave failures were silent** (log only) and an unwritable project folder meant no
   autosave at all. Fixed (fallback + status).
6. **A save over a damaged file destroyed the last good backup.** Fixed (`.corrupt`).
7. **A bad project on the command line was refused silently** (the demo opened instead).
   Fixed (status line).
8. **Opening/replacing the project during a recording** finished the pass into the new
   project's tracks. Fixed.
9. Harness-only: PowerShell sampling must not be joined on the loop (it took 10–20 s under
   load).

No leak was found: handles flat (227–234), threads flat (7, + the writer during a pass),
private bytes flat within ±15 MB after warm-up apart from recorded audio, which stays in
memory by design (finding E3).

## The beta manual's frictions (asked by the integrator mid-round)

| # | Reported | Done | Test |
|---|---|---|---|
| 1 | No recovery prompt — autosaves had to be renamed by hand | The journal + *Recover your work* at start-up (boot's `Stage::Assemble` and `CantusApp::new` call `start_session`), after a crash and after a close with unsaved work | `crash_recovery.rs` (both kills), `recovery_flows.rs` |
| 2 | New and Open do not ask about unsaved changes | `views/unsaved.rs`: *Save changes?* — Save / Don't save / Cancel (Escape = Cancel); the keymap actions and the File menu go through `request_new` / `request_open` (`request_open_path` for a chosen file); a Save that does not save (cancelled, failed) goes no further | `manual_frictions::new_and_open_ask_before_unsaved_work_goes` |
| 3 | *Add audio track* does not arm the new track | It comes armed, monitoring *Auto*, one undo step (`rec_fixes` no longer arms it by hand) | `manual_frictions::a_new_audio_track_is_armed_and_monitoring` |
| 4 | The count-in is silent unless the metronome is on | With the switch off, the snapshots of a counting-in pass are compiled from a copy with the click on in `CountInOnly` mode, so the engine clicks exactly up to the record start and not after; the project is untouched. (The engine's own gate — the click needs `snapshot.metronome` — is the root; a `count_in_clicks` flag there would be cleaner, E6.) | `manual_frictions::a_count_in_clicks_with_the_metronome_off` (renders: clicks during the bar, silence after) |

## For the integrator (engine — not edited)

- **E1. Effect units vs a fast undo** (`backend.rs` `set_snapshot` / `GarbageEffect`). The
  handle keeps `known_effects` until the engine hands a unit back; a remove's snapshot and
  its undo's snapshot both go out first, so the undo's snapshot sends no unit and the
  engine installs the effect without one (`dropped_events += 1`, silence). The app now
  re-sends, but the engine could forget an id the moment it sends a snapshot without it
  (or keep a unit until the snapshot after next). The soak's `engine_dropped` column still
  climbs slowly (≈ 2 per minute of edits) — each one a snapshot that found a unit missing
  for a block or two before the re-send.
- **E2. The pass's file paths** are only known at `finish_recording`. Recovery infers them
  by listing the folder before/after `begin_recording` and matching the recorder's naming
  (`recovery_session::match_takes`, tested). An `EngineHandle::recording_files() ->
  Vec<(TrackId, PathBuf)>` would make it exact.
- **E3. `SampleCache` never evicts** (by design, ADR 0025 §9): every take recorded and every
  sample of every demo opened stays decoded in memory for the session. A 2-hour session
  with an hour of takes would hold ~1.3 GB. Suggest `evict_unused()` dropping entries whose
  `Arc::strong_count == 1` (the audio thread holds none, so freeing on the UI thread is
  safe), called on open/new/demo.
- **E4. The writer's header flush is not `fsync`** (hound's `flush` reaches the OS cache):
  a process crash loses ≤ 0.5 s, a power cut more. A `sync_data` every few seconds on the
  writer thread would cover power loss.
- **E6. The count-in click is gated by the metronome switch** (`processor.rs` click path:
  `snapshot.metronome && click_allowed`). The app works around it (above); the engine
  could click during a count-in regardless of the switch.
- **E5. `finish_recording` waits up to 2 s** for a live engine to return the rings, on the UI
  thread (no stall seen in the soak; noted for the hardware matrix).

## Tests

21 new (+1 ignored soak, +1 ignored child): `cantus-project/tests/refusals.rs` 4;
`cantus-ui/tests/recovery_flows.rs` 8, `crash_recovery.rs` 2, `soak_findings.rs` 3,
`soak.rs` 1 (`soak_smoke`), `manual_frictions.rs` 3. `rec_fixes.rs` and `rec_flows.rs`
adjusted for a new audio track that comes armed. `cargo clippy -p cantus-ui -p
cantus-project --tests` clean; every test of both crates green. The curves of runs 2 and 3
are kept beside this file (`soak-run2.csv`, `soak-run3.csv`).

## Files outside my area

None in the engine. `cantus-ui` and `cantus-project` only, plus `docs/adr/0006` (the
amendment) and this file. `i18n.rs` gained `recovery.{en,es}.txt` (one line each — a likely
trivial merge with other builders adding locale files).

## Left

- Hosted plug-in windows in the soak (the native device windows are exercised; a CLAP from
  `cantus-test-plugins` was not loaded) and a hosted plug-in that crashes stays a known
  limitation (BETA.md).
- The recovery dialog has no screenshot in `examples/shoot*` yet.
- A 2-hour run (BETA.md's bar) — the harness takes `CANTUS_SOAK_MINUTES=120`; three 40-min
  runs were done here.
- Save (Ctrl+S) is still written on the UI thread (an explicit action; 0.2–0.7 s per demo).
