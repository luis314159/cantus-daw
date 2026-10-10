# Round 24 — beta-rest: gain staging, Save off the UI thread, the recovery dialog's shots

Branch: **`r24/beta-rest`** (worktree `daw-r24/beta-rest`, base master cead440), left
uncommitted for the integrator. Decision records: **ADR 0039** (instrument output
calibration, schema 21) and **ADR 0040** (Save off the interface's thread) — both numbers
as written on this branch; renumber at the merge if another branch took them.

The part of `docs/BETA.md` that needs no hardware: *instrument gain staging* (strongly
recommended), *Save off the UI thread* and *the recovery dialog in `examples/shoot*`*
(round 22's list).

## 1. Instrument gain staging (ADR 0039, schema 20 → 21)

### Measured on the base

`instrument_levels.rs::default_patches_staging_report` (round 19's), and a new
`factory_presets_staging_report` (every factory preset of every native kind on the same
material: a C-major triad at velocity 100 for two seconds; a bass its root; at unity,
nothing on the master). The default patches:

| Instrument | Before | After |
|---|---|---|
| Unda (wavetable) | −18.7 LUFS, −10.4 dBFS | unchanged |
| Calor (analog) | −19.2 LUFS, −8.6 dBFS | unchanged |
| Modulus (FM) | −20.0 LUFS, −11.9 dBFS | unchanged |
| Gravis (bass, one note) | −15.7 LUFS, −10.2 dBFS | unchanged |
| Organum (organ) | −18.5 LUFS, −7.8 dBFS | unchanged |
| **Lamina** (electric piano) | **−27.9 LUFS, −14.1 dBFS** | **−22.9 LUFS, −9.1 dBFS** |
| **Exemplar** (nothing loaded: the placeholder sine) | **−27.2 LUFS, −21.6 dBFS** | **−19.2 LUFS, −13.6 dBFS** |
| Pulsus II (kick, snare, hat) | −25.3 LUFS (sparse, gated), −8.8 dBFS | unchanged |

The presets told the rest: every Lamina preset sat with its default (−25 … −29 LUFS; the
two reed presets −37); Exemplar's presets were already balanced by their own `gain`
(round 19) — only its placeholder was quiet, and a sampler must play loaded samples at
their own level. The demos had compensated by hand: +8/+8/+9 dB of channel trim on their
Lamina keys.

**Why Lamina +5 and not +8.7.** At velocity 127 on the headroom test's dense riff Lamina's
*Ballad* preset peaked at −5.6 dBFS — the instrument is that dynamic. +5 dB is the most
the headroom rule allows (*Ballad* now −0.6 dBFS, *Suitcase* −1.2, the default −3.2). At
+5 its default's peak at velocity 100 (−9.1 dBFS) is inside the synths' range (−7.8 …
−11.9) and its loudness 4.2 LU under the others' median (−18.7; 3.95 under the four synths'
−18.95): the rest is an electric piano's decay
against held saws. The reed presets got +5 dB of their own factory `gain` (−37.7 → −27.7
and −36.6 → −26.6 LUFS; −1.6 and −1.0 dBFS on the riff). Exemplar's placeholder took the
full +8 dB: −19.2 LUFS, within 0.5 LU of the others' median, −5.4 dBFS on the riff.

### How old projects keep their sound

A **hidden output trim** (`cantus_core::inst::TRIM_KEY`, `"trim"`, dB) whose default is the
kind's calibration (`InstrumentKind::calibration_db()`: Lamina 5, Exemplar 8, every other
kind has no trim at all). Not in `params()` — no knob, no lane, no modulation target. Lamina
adds it to the gain stage (its last, linear stage); Exemplar adds it to the placeholder
zone's −18 dB only. **Schema 21's migration writes `trim = 0` into every Lamina and
Exemplar saved before**; `gain_db + 0.0` and `−18 + 0.0` are the old numbers to the bit.
Two alternatives were rejected (ADR 0039): changing `gain`'s default (a new Lamina would
read *Gain +5 dB*) and lowering every stored `gain` (not bit-exact in `f32`, clamps,
automation would need rewriting).

User presets (bare `NativeSpec` JSON, no version) are handled the same way: written with
the trim spelled out (`NativeSpec::spell_out_trim`); read without one, a preset is from
before and gets 0 dB (`NativeSpec::read_unversioned`, which also clears a trim equal to
the calibration, so it reads as a new instance). **DAWproject** device states are the same
bare JSON: the export spells the trim out and the import reads with the same rule, so a
`.dawproject` exported before schema 21 comes back at its old level (found in review).
The demos' Lamina trims went to +3/+3/+4 dB.

No golden hash moved (`null_tests` green, `DSP_VERSION` stays 3): no unit renders a given
spec differently; a spec that stores nothing now means something louder, which is what the
schema bump says. The engine edits are four lines in `inst/epiano.rs` and two in
`inst/sampler.rs` (output level only — the voice loops are perf2's).

### Tests (`crates/cantus-ui/tests/gain_staging.rs`, new: 5 tests and 1 ignored fixture writer)

- **the null test**: `tests/data/v20-quiet-instruments.cantus` was written by the base
  build (schema 20) — a Lamina at default, a Lamina with gain −6 dB and a macro → gain
  modulation route, an empty Exemplar, an Exemplar with a built-in zone — and its render
  hashed then (`OLD_RENDER`, FNV-1a over the bits, 240 000 samples). Opened now it is
  migrated from 20, every Lamina/Exemplar holds `trim = 0`, and it renders **to the same
  hash**; saved at 21 and reopened, the same. With the migration disabled the test fails.
- a new instance is louder by exactly its calibration (peak within 0.05 dB, loudness
  within 0.1 LU) and an old one is not; the other five kinds have no trim;
- an Exemplar with anything loaded is bit-identical with or without a trim;
- a user preset written before reads at 0 dB, one written now names its trim;
- the three demos with Lamina, rebuilt the old way (trim 0, channel +5 dB), match the new
  build through their whole mix and master chain within −71.6 / −70.9 / −74.2 dB of the
  peak (float rounding magnified by Culmen's and Terminus's gain decisions; held at −60).
- `instrument_levels.rs`: the headroom test now also plays every kind's **default patch**
  (it only played presets); nothing passes 0 dBFS. The two reports are ignored tests.
- `cantus-project/tests/dawproject.rs`: a device state without a trim (what the build
  before wrote) imports at trim 0; one exported now comes back as the same spec.
- `cantus-core`: the trim is hidden and only recalibrated kinds accept it, and the
  unversioned-file rules round-trip;
  `cantus-project`: the 20 → 21 step (writes 0 where missing, keeps a trim already there,
  leaves other kinds, refuses params that are not a map); `fixtures/v21.cantus` (a Lamina
  made now, an Exemplar with its trim written down) — every older fixture still loads.

## 2. Save off the interface's thread (ADR 0040)

`save_job.rs` (new), the twin of round 22's `autosave.rs`. The frame takes what must be
taken on it — audio collected into the folder, hosted plug-ins' state — as the owned copy
`project_for_save` already made; a thread serialises, writes atomically, keeps the `.bak`,
`fsync`s and verifies (`cantus_project::save`, unchanged); the next frames collect it.

**Measured** (`save_async.rs::measure_the_copy_against_the_write`, ignored; best of five,
`test` profile on the shared, loaded machine):

| Project | File | Copy (frame) | Serialise | Whole save (was in the frame) | Frame now |
|---|---|---|---|---|---|
| house | 531 KiB | 0.17 ms | 18.4 ms | 51 ms | 0.25 ms |
| funk | 584 KiB | 0.14 ms | 19.3 ms | 49 ms | 0.24 ms |
| ballad | 358 KiB | 0.10 ms | 10.2 ms | 40 ms | 0.18 ms |
| lofi / ambient / trap | 166–391 KiB | 0.07–0.10 ms | 4–11 ms | 25–37 ms | 0.18–0.37 ms |
| 64 tracks × 4 000 notes | 54 MB | 3.6 ms | 2 321 ms | **3 823 ms** | **2.1 ms** |

So the copy, not the serialised bytes, is what the frame takes.

Rules: one save at a time, a second one **queued** (last asked wins; the same file is not
retried after a failure) — with the project's epoch, so a queued save is dropped, never
written, when the project is replaced meanwhile; *Save* while one is in flight goes where
it goes (`save_target`: an untitled project is not asked for a name twice); the crash
journal lets go only of the recording passes finished when the copy was taken (the
autosave's job too); `CommandBus::mark_saved_at(revision)` marks the revision the copy
was taken at, so an edit made during the write keeps the project dirty; the status line
says *Saving “x.cantus”…* then *Saved …* or the failure in the interface's language
(round 22's `recovery.save_failed`); New, Open, a demo, a DAWproject import, a recovery,
the *Save changes?* question and the close wait for a save in flight (it may make the
question moot) — the undo of a replaced project does not wait, the epoch rule holds it;
the question's **Save** and the demo picker's carry out their errand once the save has
landed — never after a failure, and asked again when the project was edited while it was
written — and the close drops it. The picker's *Save first* closes the picker while the
save is written. `save_to_path` keeps returning once the
file is written (the soak, scripts and tests use it); the interface calls `save`,
`save_as`, `start_save`. The repaint ticker wakes every 100 ms while a save is in flight.
New strings: `locales/areas/save.{en,es}.txt`.

**Tests** (`crates/cantus-ui/tests/save_async.rs`, new; a test holds the save's thread
before it writes with `Saving::hold`, so "in flight" is a state, not a race): a save
returns at once and nothing is on disk until released, then the file lands whole and
verified and the project is clean; an edit during the save keeps it dirty and the file
holds the earlier name; a failed save says so in English and Spanish, stays dirty and
unpointed; a second save is queued and takes the later edits; New, Open and the close
wait (the close leaves no journal); the question's *Save* opens the other file only once
the save has landed, and not at all if it failed. Run against the old synchronous save
first: four of six failed (the other two pass trivially when a save is synchronous).
Four more after the review, each run first with its fix taken out (all four failed): a
queued save is never written with a project that replaced the one it was asked for (an
import, then the undo of one); a second *Save* of an untitled project goes where its first
went; the picker's *Save first* closes it, opens the demo once saved and asks again after
an edit made meanwhile; a recording pass (a real one, through `harness::LiveEngine`) that
ends during a save stays in the journal, on disk too, until the next save holds it.
`manual_frictions.rs` now waits for the save after answering *Save* (the open follows it).

## 3. The recovery dialog in the shoot examples

`examples/shoot_recovery.rs` (new): the whole window with *Recover your work* over it, in
both themes and both languages, for three offers — a crash with an autosave 12 minutes old,
three takes and a long project path; a close with unsaved changes in a never-saved
project; a crash before any autosave. `cargo run -p cantus-ui --example shoot_recovery --
<dir> [scale]` → 12 PNGs `recovery-<kind>-<theme>-<lang>.png`. Looked at, and fixed:

- **a gap of ~90 px between the text and the buttons**: `interop::form::buttons` gave its
  right-to-left layout the rest of the window's height, and egui centred the buttons in
  it. The row is now one button high — this also tightens *Save changes?* and the MIDI
  export dialog, which use the same helper;
- **the title said twice** (the window's title and a band with the same words): the band
  now names what is offered, *What the session left* / *Lo que dejó la sesión*.

Nothing overlaps at 1400 × 820 in either language; the long path wraps inside the dialog.

## 4. The adversarial review, and what came of it

Two reviewers read the branch; nine findings (two of them reported twice). Each was checked
against the code; every one held, and each that a test can hold got one that failed
before its fix.

| Finding | Verdict | What was done |
|---|---|---|
| A queued save runs after the project is replaced (an import, the undo of one, a recovery) and writes the new project over the old file | real (major) | `Saving::queued` carries the epoch (`save_job::Queued`); a stale one is dropped, and so is the errand waiting on it. `import_dawproject` and `restore_recovery` now wait for a save in flight, as `replace_project` and `open` did; undo/redo do not (the frame stays free) and the epoch rule holds them. Test: `a_queued_save_never_writes_a_project_that_replaced_the_one_it_was_asked_for` (import part fails without the wait, undo part without the epoch). |
| A landing save drops recording passes that finished during it from the crash journal (reported twice) | real (major) | `journal_finished_passes()` is taken with the copy (save and autosave jobs; the close's synchronous autosave too); `journal_persisted` removes only those. Test: `a_recording_pass_that_ends_during_a_save_stays_in_the_journal`, a real pass through `harness::LiveEngine`. |
| `.dawproject` exported before schema 21 comes back 5 / 8 dB louder | real (minor) | The export spells the trim out; the import reads a native state with `NativeSpec::read_unversioned` (no trim → 0 dB). The user-preset helpers moved into `cantus-core` so both share one rule. Test in `cantus-project/tests/dawproject.rs`. |
| A second Save of an untitled project in flight opens Save As again | real (minor) | `save()` uses `save_target()`: the queued or in-flight save's file for this epoch, else the project's. Test: `a_second_save_of_an_untitled_project_goes_where_its_first_save_went` (checks the target before calling `save()`, so a regression fails instead of opening a dialog). |
| The demo picker's Cancel during a *Save first* save does not cancel; a second press queues another save; an edit during the save drops the errand silently | real (minor) | *Save first* (`views::demos::save_first`) closes the picker once the save is in flight; `save_then` does not start a second save while one serves an errand; an errand whose save landed on an edited project asks the question again (`unsaved_prompt`). Test: `the_demo_pickers_save_first_closes_it_and_opens_the_demo_once_saved`. |
| The report and ADR overstate (no save blocks the window; timings for an overlay nobody reads; 6 tests; the median) | real (minor) | ADR 0040 now says a save never blocks the frame by itself and names what waits; the timings' doc says the measurement prints them (that is the only reader); gain_staging is 5 tests + 1 ignored; the medians are −18.7 (five kinds) and −18.95 (four synths) — ADR 0039 and this report corrected. |
| “will be saved again” for a Save As to a new file | real (minor) | `save.queued` reworded without “again” / “otra vez”. The last request still wins; the message names the file that will be written. |

**Found while writing the journal test, and fixed: a pass could end on the frame right
after *Record*.** `poll_recording` closes the pass when the handle reads the transport
stopped and blocks have been processed; the handle learns of *Play* only from the engine's
next position event, so a frame that ran before the callback had taken the *Play* (a
buffer longer than a frame, a loaded machine) read the old *stopped* and ended the pass —
for an audio pass the empty take was then thrown away. It made round 22's
`crash_recovery::killed_mid_recording…` flaky under load (1 run in 5 here; its child
printed *recording* then *Ready* on the next frame) and could hit a real session with a
large ASIO buffer. `record_session::Live` now has `transport_seen`: the pass is closed by a
stopped transport only once the engine has said it runs. Test (deterministic, a processor
driven by hand): `rec_flows::a_pass_is_not_closed_by_a_play_the_engine_has_not_taken_yet`
— failed before (the pass ended on the frame before the first block), passes now, and the
transport stopping on its own still closes the pass. `crash_recovery` then passed 5 runs
in 5. This is a small edit outside the brief, in `record_session.rs` (`Live`, and the stop
condition in `poll_recording`).

## Tests and checks at the close

After the review fixes (the numbers before them were 1 910 + 24):

- Whole crates touched: `cantus-core`, `cantus-engine`, `cantus-project`, `cantus-ui` —
  all inside the workspace run.
- `cargo test --workspace --no-fail-fast`: **1 915 passed, 1 failed, 31 ignored** in 192
  test binaries that ran. The one failure is `cantus-engine/tests/pool.rs::a_parked_pool_is_woken_once_per_worker_after_silence_and_not_while_busy`,
  a wall-clock test of the worker pool's wakes (a 19.7 µs block after a 400 ns gap), with
  the machine at 85 % CPU from the other builders; this branch does not touch the pool, and
  it passed on two reruns alone (3 / 3). The four binaries Windows will not start by name
  (`audio_setup`, `audio_setup_first`, `audio_setup_flows`, `cantus_update` — os error 740)
  were run copied to neutral names from their crate folders: 4 + 4 + 6 + 10 = 24 passed.
  In all **1 939 passed in the run, plus the pool test on rerun; 0 failures of this branch**.
- The first workspace run after the fixes also showed `crash_recovery::killed_mid_recording…`
  failing (1 in 5): that was the record-start race (§4), fixed; 5 / 5 since.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- rustfmt on the files changed whose base was formatted, and on the new files (via stdin
  where a test file declares `mod harness;`/`mod support;`, so their children are not
  touched); `recovery_session.rs`, `autosave.rs`, `lib.rs` and `manual_frictions.rs` are
  not fmt-clean on the base and were left as they were. `cargo fmt --all` not run.

## Left

- **Collecting audio** into the project's folder still copies files on the interface's
  thread before a save starts (only when a source is outside the folder: a first save, a
  *Save As* elsewhere). Moving it means applying the copy's path commands after the
  thread; a later round.
- **An *upgrade level* action** for migrated Laminas/Exemplars (they keep trim 0 and sit
  5/8 dB under new ones) — reachable today through `SetInstrumentParam`; worth a button if
  users notice.
- Lamina's loudness at velocity 100 is still 3.7 LU under the synths' median: closing it
  needs a narrower dynamic range (a sound-design change to the presets' `dynamics`), not a
  level — the owner's ear should decide.
- The shoot example is not a test; the dialog's geometry is not asserted.
- `form::buttons` (one button high now) also changed *Save changes?* and the MIDI export
  dialog; only the recovery shots were looked at.
- The autosave's journal fix has no held-thread test of its own (the autosave has no
  `hold`); it shares the save's code path (`journal_finished_passes` / `journal_persisted`).
- The record-start fix wants the owner's ear on real hardware too: record with a large
  ASIO buffer (2048) and check the pass starts and stops as it should.
