# Round 39 — `fixes`: six items of BACKLOG §8 "Polish"

Branch `r39/fixes` (from `r39/integration` with ux-a). Each item: what was wrong, what
changed, how it is held, what was verified only here.

## 1. A shared helper's crash blamed on no one

**Cause, reproduced.** A Windows fail-fast abort runs no crash handler, and on a loaded
machine the aborted helper takes seconds to end (the system's error reporting runs first).
The gain's job was taken and never finished; the engine went on and sent the neighbour's
next job, which no worker ever took. Both passed the hang limit, and Cantus's hang judgement
(`BridgeMain::idle`) laid the end on whichever slot it looked at first — the neighbour, for
a job that never ran. Beta.12's CI saw `crash_culprit(gain)` false; here, with 24 threads
spinning beside the test, the hundred-abort test failed within three or four rounds (the
neighbour counted a crash) before the fix and passed after it.

**What it was not.** The first suspect — the main thread's 15-ms tick marking `MAIN_AT`
with the neighbour while the gain aborts — never showed: a debug print of every end's
words over a hundred aborts, and over the loaded runs, read `main_at 0` each time, and splitting the tick's mark from the
calls left the loaded test failing as before. That change was taken back.

**Fix.** A job past the hang limit is laid on its plug-in only if a worker took it and has
not finished it (`x.running()`); one never taken is the job it waits behind's to answer
for, judged on that one's slot. With no job of the helper running at all (its workers take
nothing), the helper is ended on nobody (`Sandbox::a_job_runs`).

**Tests.** `cantus-sandbox/tests/session_faults9.rs`:
`a_hundred_aborts_in_a_shared_helper_are_each_the_aborting_plugins_alone` (a hundred
helpers, the gain aborting in `process` beside the VST3 gain; `CANTUS_ABORT_LOAD=n` adds
spinning threads, the condition that shows the race; `CANTUS_ABORTS` another count) and
`a_hang_is_laid_on_the_job_that_runs_never_on_one_waiting_behind_it` (the race held without
load: one processing thread, the gain hanging on it, both slots past the limit at the same
frame, in both insertion orders).

**Later (abort-blame, after beta.14's CI):** a second cause remained — the main thread's
mark named the neighbour while it wrote the neighbour's answer, so the end had two
candidates and was nobody's. See `abort-blame.md`.

## 2. Chorda model 2's constructor costs 17–20 ms

**Measured** (dsp at opt 3): the body's two responses at a rate not yet made cost 16.3 ms;
everything else in `Chorda::new` 2.5 ms. **Fix.** The unit is made with
`Chorda::new_waiting`: its body takes the responses if the designer has made them and else
waits for them — its lines allocated, its resonance silent — and `follow_design` takes them
when they are made (a lock-free table, no allocation: the audio thread); offline they are
made on the spot. The responses' spectra are shared per rate and voicing (`Prepared`), not
copied per unit. The designer already made the body before the keyboard; `ensure`'s own
design now does too.

**Tests.** `chorda_tests_model2.rs`
`a_waiting_body_takes_its_responses_and_then_sounds_as_one_made_with_them` (to the bit
from the next block); `cantus-ui/tests/chorda_choose_e2e.rs` (assemble, the picker,
Chorda chosen: no body synthesised on the interface's thread, and the piano plays once
designed).

## 3. Chorda's scene baker spawns threads per pass

A `Crew` (a rayon pool, `threads − 1` workers plus the baking thread) is made once per
bake and kept for all its passes: rows, shading, occlusion, lines, every blur. The shading
and occlusion passes write one buffer (`rows1`) — no more two `w·h` dummies. rayon 1.12 was
already in the program. **Bits:** `a_bake_keeps_the_bits_it_had` pins the hashes of eight
bakes measured with round 38's baker (Windows), and `a_bake_is_the_same_on_any_number_of_threads`
holds.

## 4. The `stage_budget` variance

**Cause, found.** The look is one slot for the whole process, and two things wrote it
while the budget's applications ran. Each rig installs Slate (`theme::apply`); each
application, on its first whole-window frame, read the taskbar (`eframe::App::ui` →
`taskbar.poll()`, light on this machine) and on that change applied its **whole
appearance** again — a different look — when only the window icon depends on the taskbar.
Every install also bumped the theme's generation, even of the tokens already installed:
every cache keyed on it went cold and the next frame was drawn as a first frame is (the
dock's tab icons, the bar's measured groups, a knob's layers) — 4 472 triangles fewer here,
the "+4 540" of round 37. With the tests of the binary building rigs on their own threads,
which look a measured frame was drawn in, and whether its caches were warm, depended on
what the other tests had just done: an application against an identical one differed under
a loaded suite. (Not the samples' loading, the first suspect: twins with every source read
in before playing differed the same way.)

**Fix.** The taskbar's poll sets the icon's colours alone (`CantusApp::follow_taskbar`);
the look stays. `kit::tokens::install` changes nothing for the tokens already installed
(their generation aside), so a look applied again as it was (a system-dark poll, an
appearance file read again, another rig) no longer chills every cache. Touched outside my
items: `app.rs` (the taskbar's line and the new helper) and one function of
`kit/tokens.rs` (the `kit` builder's file) — said here for the integrator.

**Test.** `frame_budget.rs` (`stage_budget/frame.rs`)
`two_identical_applications_draw_the_same_triangles`: two applications built the same
way, the taskbar's next read made a change for both (the look stays the rigs' Slate), a
frame each, the same look applied again by another rig (the theme's generation does not
move), then sixty frames interleaved,
equal to the triangle, while another thread applies the look every millisecond. Before the
fixes the application drew its own look (55 322 triangles a frame here) until another rig
put Slate back (50 850 on the next frame, cold). The machine's own readouts (CPU, RAM) are
hidden in the twins: the process's memory differs by a bar. An application's very first
frame draws less than its second (the dock settles a frame late, by design): the twins are
compared from their second frame.

## 5. The arrangement clamps its stored header width in a small window

The width drawn is `header_shown(stored, width)`; only the user's own move of the edge is
stored. `arrangement_rec`'s record controls read the drawn width. The capture scene's
per-frame reset of the width (a workaround for this) is gone. **Test:**
`cantus-ui/tests/arrangement_header_width.rs` (assemble, a 560×420 window, then 1920×1080:
the stored 300 kept and drawn again). Touched for this only: `views/arrangement.rs` (the
header width), `views/arrangement_rec.rs` (one line), `shot_scene.rs` (the reset removed).

## 6. A preview's level without restarting it

`EngineMessage::PreviewGain(f32)` (`// r39:fixes` in `cantus-engine`): the audition's level
ramps over 10 ms to the new one, the sound going on; the level it has changes no sample; at
rest it is the next start's. The browser strip sends it as the level is dragged (it used
to start the sound again on release). **Tests:** `preview.rs` units
(`a_new_level_ramps_in_without_starting_again`, `…_is_the_next_start_s_own`), `rt_memory`
`an_audition_s_level_moved_as_it_plays_never_allocates`, and
`cantus-ui/tests/preview_level_e2e.rs` (assemble, a real processor, the row clicked, the
level clicked at a quarter: the output only goes down to a quarter, never dips, and the
audition is not started again).

## 7. The device picker's subtitle names no strip (left by ux-b)

"Add an effect on —": the subtitle took the strip's own name, empty for most strips. It
now names the strip as the mixer and the inspector do (`inspector::strip_label`: its name,
else the first track feeding it, else "Strip n"), and a strip no longer in the project
says so (`picker.context.strip_gone`, both locales). **Test:**
`cantus-ui/tests/picker_context_e2e.rs` (assemble, the picker opened for an effect on a
track's unnamed strip and on a strip nothing feeds, the window drawn: "on Bass",
"on Strip n", never "on —"; the same in Spanish).

## Verified only here

Everything above ran on the owner's Windows machine; nothing on Linux (no WSL this round).
The loaded-runner condition of item 1 is emulated with spinning threads, not a CI runner.
