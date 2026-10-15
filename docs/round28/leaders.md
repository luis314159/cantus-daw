# Round 28 — leaders: modulators on any parameter, then comping (BACKLOG §0.2)

Branch `r28/leaders` from `r28/integration` at `405bd9e` (voice, topbar, modfx, pitch
merged); `r28/integration` merged in again mid-round (dynfx). ADRs **0057** (modulators,
hosted automation) and **0058** (comping). Schemas **28** and **29**.

## Done

### A. Modulators on any parameter, hosted plug-ins included (ADR 0057, schema 28)

- **Hosted parameters are targets** — `AutoTarget::Plugin { effect, id }` and
  `PluginInstrument { track, id }`, normalised (the unit knows the range): automation
  clips, *last tweaked* (including gestures made in the plug-in's own window), recorded
  automation, modulation. Their values reach the plug-in **at their sample offsets inside
  the block** (CLAP `param_value` times, VST3 queue points; CLAP pieces of a long block get
  their own events), never by cutting a pass; a parameter no longer driven goes back to the
  project's value when the engine re-plans (`automation_replanned`).
- **Modulators**: LFO (5 shapes, synced/Hz, phase, note retrigger, unipolar), envelope
  (ADSR from the track's notes — sequenced and live — or from an audio level), follower
  (any strip's or the master's post-fader peak), steps (1–32, glide), random (held/smooth,
  seeded). Each drives up to 32 parameters with bipolar depths; several sum on a target, on
  top of its automation, clamped; one level of nesting (a modulator's *amount*). On tracks,
  strips and the master (`Track/MixerStrip/Master::modulators`), 16 per host. Every edit is
  `InsertModulator` / `RemoveModulator` / `SetModulator` (drags merge), in the fuzz.
- **Determinism**: synced sources and the note envelope are functions of the song position
  (a closed form over note gates); free ones run from the start of playback; followers,
  audio envelopes and live notes are stepped state, restarted at play, carried across
  snapshots, one control point behind so gains ramp.
- **Bake to automation**: per route (panel or *Modulate…*), over the time selection / loop /
  song, one undo step; an audio-following source is traced by an offline render on a worker.
- **Recording automation** (`RecordSettings::record_automation`, on by default, a switch in
  the record options): every parameter moved while a pass records — knob or plug-in window
  — becomes an automation clip at the end of the pass, one undo step; loop passes overwrite.
- **Interface**: the inspector's *Modulators* section for the track, its channel and the
  master (title row with handle, live meter, map, on/off, remove; the movement drawn from the
  engine's own functions; settings; amount; a row per route with depth, bake, disconnect);
  **Modulate…** in every parameter menu (native instruments, effects, the hosted generic
  editor, a hosted instrument's new *Parameters* list in the inspector, the inspector's
  track/Firmus rows, the desk's strip/master menus, the track menu); a modulator's handle
  dropped on any control connects it; **Alt+drag** (or *map*) edits a depth on the control;
  rings by kind, the live dot, a mark; live marks on the desk's pan, width and fader.

### B. Take comping, Logic / Studio One-class (ADR 0058, schema 29)

- **Seam crossfades rendered in the engine**: the right window's `AudioRegion::xfade`
  (default `RecordSettings::comp_xfade_ms` = 10 ms); the snapshot overlaps the two windows,
  centred, and `add_clips` crosses them equal-power over the same samples. Validated in
  `Command::apply` (0…a beat); 0 is the old butt join to the bit.
- **Seams move as a unit**: `Command::SetCompSeam` (refuses what is not a seam), from a
  handle at the foot of the playing lane or a window's edge at a seam; right-click for its
  crossfade (off/5/10/20/50 ms, or for every seam). Each comp window shows its take's number.
- **Swipes keep the comp's edits**: a grown window keeps its pitch edits (the pitch review's
  finding), gain and crossfade.
- **Alternate comps** (`Track::comps`, `active_comp`, `SetTrackComps`): switch (back whole,
  pitch edits included), new (a copy), rename, delete; **flatten**; **flatten and merge**
  (an offline bounce of the comp into one recording, the takes' files kept). The comp's chip
  *Comp A ▾* in the first take lane's header and the lanes' menus.
- **Grouped comping** by mixer group (`comp_grouped`), **quick swipe** on/off.

## Measurements

| What | Measured |
|---|---|
| Modulation, block sizes 1/7/16/37/64/333/1 024/8 192 vs 512 | identical (tolerance 0) |
| Real-time processor at irregular blocks vs offline, every kind | identical (tolerance 0) |
| Render from bar 5 vs bars 5… of a full render (synced LFO, steps, random, note envelope) | identical (tolerance 0) |
| LFO square on an EQ bell's gain (±15 dB) | > 20 dB between halves, every cycle |
| Envelope on Firmus cutoff, brightness at note start vs after decay | > 2× |
| Follower ducking a strip (depth −0.5) | organ < 0.5× under each lead note, recovers between |
| Hosted step sequence (CLAP gain) at blocks 1/7/64/1 000/4 096 | identical; each step lands on its own sample (1e-4) |
| Hosted instrument (CLAP synth) step at 30 000 | lands on frame 30 000 at every block size |
| Bake vs modulated render (LFO, steps, follower via trace) | within 1 % of peak |
| Audio-thread allocations with modulators (every kind, snapshot, live notes, seek, stop, play) | 0 |
| Seam crossfade | L²+R² = 1 ± 1e-5 at every sample, centred, monotone |
| Seams: block sizes 1/37/64/1 000 and live vs offline | identical (tolerance 0) |

## Tests and what each holds

- `cantus-core` `modulator_tests.rs` (16): validity, insert/remove/set inverses, refusals,
  the LFO a function of the song position / from play, note retrigger, steps and glide,
  seeded randoms, note gates, the closed-form envelope agreeing with the stepped one, the
  follower (NaN-proof), compile leaves out what drives nothing, lanes for modulated targets,
  JSON, a dragged modulator one superseding step. `modulator_bake.rs` (4). `comp.rs` (3):
  the seam as a unit and back, refusals, the comps' checks. Fuzz arms 41 (modulators) and
  42 (comps), seeds 40 → 60 so every arm keeps being drawn.
- `cantus-project`: `schema_28_keeps_modulators_and_hosted_automation`,
  `schema_29_keeps_comp_seams_and_alternate_comps`; fixtures `v28`, `v29`.
- `cantus-dsp`: `a_band_raised_from_zero_db_glides_up` (Aequalis fix, below).
- `cantus-engine`: `modulators.rs` (8: null, heard and block-size independent, live = offline,
  bar 5, LFO on an effect, envelope on a filter, follower ducking, bake), `comp_seams.rs`
  (3), `rt_memory::modulators_never_allocate`.
- `cantus-host`: `clap_modulators.rs` (3: offsets at every block size, a clip on a hosted
  parameter and its release, modulation over the project's value and its end),
  `hosted_inst_modulators.rs` (1: a hosted instrument's step on its sample).
- `cantus-ui`: `modulators.rs` (8: steps, depth drags, mapping, handle drop, bake, recorded
  automation and its switch, Alt-drag in the kit, the section drawn in EN/ES, reach through
  `boot::Shell`), `comping.rs` (10: default crossfades, pitch kept, seams by command and by
  hand from the real arrangement, comps back whole, unused takes, flatten, merge, grouped,
  quick swipe), `modulator_record` unit test (loop overwrite).

## Files outside my area (all additive, said here)

- `cantus-core`: `model.rs` (`ModulatorId`, `Track/MixerStrip/Master::modulators`,
  `Track::comps/active_comp`), `command.rs` (5 commands, `supersedes` arms, `validate_clip`
  and `SetClipAudio` crossfade checks, `RemoveAudioSource` sees kept comp windows),
  `snapshot.rs` (`modulators`, seams, `AudioClipSnapshot::{xin, xout}`), `audio.rs`
  (`AudioRegion::xfade`, `RecordSettings::{record_automation, comp_xfade_ms, quick_swipe,
  comp_grouped}`), `time.rs` (`Ticks::is_zero`), `lib.rs`, new `comp.rs`.
- `cantus-engine`: `fx/mod.rs`, `inst/mod.rs` (`automate`, `automation_replanned`,
  `HostedValue`, `NativeUnit::automation`), `effects.rs`, `graph/{strip,strip_run,track,
  master_run}.rs`, `processor_{pass,msg,snap,rec}.rs`, `messages.rs`, `backend.rs`, `lib.rs`.
- `cantus-host`: `unit.rs`, `inst_unit.rs` (mine, additive), `clap/{audio,events,main}.rs`
  (events per piece; capacity 4 096), `vst3/process_objects.rs` (80 points).
- `cantus-dsp`: `aequalis.rs` `is_active` (a fix, with its test).
- `cantus-ui`: `app.rs` (field, popup frame, host tweaks, seam trims), `automation.rs`
  (new targets' names, record hook, hosted start value), `kit/{automate,knob,widgets,
  gallery}.rs`, `views/fx/{controls,host}.rs`, `views/plugins.rs`, `views/inst/frame.rs`,
  `views/inspector{,_track,_hosted}.rs`, `views/mixer_{menu,strip}.rs`,
  `views/arrangement{,_auto}.rs`, `views/record_options.rs`, `views/mod.rs`.
- `cantus-plugins/src/editor.rs`: one line so the merged dynfx + modfx compile (Speculum's
  clock reads the host's optional beats).
- `cantus-project`: `format.rs` (28, 29), `examples/write_fixture.rs`, `midi/import.rs`.

## Left

Nothing that is not the owner's: judging the comp crossfades and the modulators by ear.

## Notes for the integrator

- Schema numbers 28 (modulators) and 29 (comping) as the contract planned.
- The follower is our own (`cantus_core::modulator::eval::Follower`, a few lines: peak per
  control window, one-pole attack/release); dynfx's `dynfx_follow.rs` can replace it later.
- Latency moved by a modulator relies on dynfx's `replan_if_latency_moved` (one mechanism).
- The kit gallery's golden images did not move (the live dot is not in the gallery).
- `processor_pass.rs` calls `auto_hosted` before each pass; `processor_msg.rs`'s
  `sound_live` and `processor_snap.rs`'s `release_all` call the modulators' live-note hooks.
- Shots: `F:\projects\daw-r28\shots\leaders\` (`shoot_modulators`, `shoot_comping`).

## Review 1

45 confirmed findings (duplicates across lenses merged: 33 distinct), 1 HIGH. Every one
fixed; each testable one with a test seen failing first (noted where not).

**The HIGH — Modulate… on any parameter.** Two shared paths so no editor can drift:
`controls::Cell::show_in` for every effect cell (Aequalis's bands and output, Sibilus,
Comprimo's engine and ratio cells, Discrimen and Speculum through `dynfx::param_labelled`,
Saturator's character, Space's size and pre-delay) and `inst::frame::{wire, wired}` for
every instrument knob (Calor/Modulus `bound`, Organum, Unda, Exemplar's cells, zone and
group fields, the macros, the matrix depths — these also had no automation menu before).
The desk's pan, fader and width, strips and master: `mixer::strip::DeskMod` (handle drop,
Alt-drag depth, rings, live dot, the fader mark on the master too). Delay and Reverb (no
editor) are modulated from their inspector rows. A kit probe records every parameter
control drawn; `tests/modulate_everywhere.rs` opens all 25 native effect editors (each
as its demo sets it up) and all 8 instruments in Play/Edit/Matrix and fails on any control
without Modulate… — it caught the instrument macros and matrix depths the review had not
listed.

**Engine and host.** Note envelope: start levels computed per envelope and tempo at
compile (exact for any number of gates). `live_on` let go at wrap/seek/stop and ignored
when ahead. Hosted-only modulators: a control point at least once a pass (grid-aligned) and
live envelopes stepped at hosted grid points (test: a live key on a CLAP gain while
playing; before, the gain never moved). Hosted queue: only changes queued, 8 192 per unit
(128 moving parameters at 16-sample resolution), and every drop on the way — engine queue,
`ParamSync` merge, CLAP list (now 9 216), VST3's 128 queues — counted in
`dropped_events` (tests: 40 moving parameters arrive at every grid point; 136 report
drops). A parameter with no project value swings around the plug-in's own value.

**Core.** Bake includes nested amount modulation (render test within 1 %; before, 25 %)
and finds jumps by their left limit; `needs_trace` when the nesting follows audio. A
follower whose strip is gone hears silence (`ListenAt::Silent`; the panel says so). New
random sources seeded from their id. One `comp::seam_window` test for snapshot, command
and arrangement (no looped seams). A crossfade at a file's edge shifts to where both sides
have audio (engine test: equal power, no step). `InsertTrack` refuses kept-window ids the
project holds. Fuzz: the comp arm builds real seams (and still asks refused ones); modulator
and comp arms count what applied and assert it.

**Comping in the app.** Explode leaves comps; slipped/moved windows stay in their comp;
grouped comping moves seams, crossfades, switches and new comps; a swipe merges several
windows' pitch edits; every-seam crossfade one undo step; *New comp* off at 26 with a
message; the comp menu and the Modulate… list scroll (title, New, Bake stay in view, a
modulator is never offered its own amount); merge waits for loading, renders on a worker,
named in the interface's language, its test reads the bounce back (A left, B right,
equal-power seam).

**UI.** Nesting reachable (the amount is a parameter row); recording from a plug-in window
records every parameter of a frame (`PluginHost::take_tweaks`); hosted instrument rows
cache the plug-in's text by value and allocate nothing per frame; list rows have hover
hints; a pending bake or merge wakes the frame loop; the section test asserts every band,
the master's, and folding.

**DSP_VERSION 5** (commit `ad83fbf`): the Aequalis fix, measured — a 1 kHz bell raised to
+12 dB from 0 dB rendered 0.0 dB before, 12 dB now; no golden hash moved.

Not covered by a failing-first test (said why): the wake-up while a bake or merge is pending
(a repaint request, no observable state in a headless frame); the merge's wait for a pitch
render (the wait is `samples.loading()`, which export already relies on); the hover hint.

Files touched outside my area this review: `cantus-host` `session.rs`,
`session_inst_idle.rs`, `backend.rs` (`ParamOut::lost_in`), `vst3/audio_thread.rs`;
`cantus-dsp/src/lib.rs` (`DSP_VERSION`); `cantus-ui` `eq_editor_panel.rs`,
`fx/{deesser,dynfx,saturator,space}.rs`, `comp_editor.rs`, `inst/{analog_parts,
organ_parts,wavetable_parts,sampler_parts,sampler_cards,sampler_map}.rs`,
`inspector_mini.rs`, `inspector.rs`, `mixer_parts.rs`, `kit/widgets.rs` (`Param::plain`, the
probe note).
