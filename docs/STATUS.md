# Cantus — status and handoff

**Updated:** 2026-09-10 (round 12: Pulsus and a channel rack across every track, samples per pad, time selection, track shape) · **Phase:** 0/1 complete, Phase 2 (MIDI) and parts of 4 (mixer) and 8 (effects) done · **Tests:** 221 green + 1 real-device test (`--ignored`), clippy clean · **Code:** ~30 500 lines of Rust

Read this first, then `docs/adr/`, then `idea.md` for the long-term map.

## The next session: buses, then what hangs off the selection, then swing

Round 12 built both blocks the last handoff asked for — the kit, the rack, the time
selection, the track shape — and then, on the user's direction, reshaped the rack into
FL's: every track a channel, a drum per track, samples per pad, a groove on start-up.
That leaves three threads, in this order:

**0. Buses and sends — the routing the channel rack now visibly lacks.** Every drum is its
own track and its own mixer strip, and the rack prints the strip number on each row; what
it cannot do yet is send the kick and the snare to *one* strip, or a group of tracks to a
bus with a compressor on it. This is roadmap item 6 pulled forward, because the rack made
the hole visible. The shape: a bus is a `Track` with no instrument and no clips;
`Track.output: Output::{Master, Bus(TrackId)}`; the processor renders tracks in dependency
order (a bus after everything routed to it), which is a topological sort done once per
snapshot on the UI thread and stored in the snapshot as a play order; the mixer shows
buses as strips with a different header; the rack's strip number becomes a picker. Cycles
are refused by the command. Track groups in the arrangement are the **same tree** (the
playlist plan's decision 3) — do not build a second one.

**1. What hangs off the time selection.** It exists, it is visible as a column across every
track, and today two things use it: loop-to-selection (`Shift+L`) and zoom-to-selection
(`Z`). The playlist plan's step 5 is the rest, and every one of them is now a small job
instead of a large one: **insert / delete space** (ripple every clip after the selection),
**quantize clip starts**, **select the clips inside the selection**, **render the
selection**. Do insert/delete space first — it is how a song's structure actually gets
changed, and it exercises the selection as an *edit* anchor rather than a view one.
`CantusApp::time_selection` is the field; `clips.rs` is where the commands go, as free
functions over a `Project` so they are tested without a window.

**2. Swing as a clip property** — decision 4 of the drums plan, still open. A `Clip.swing`
field applied when the snapshot is compiled (`compile_events` is the one place the
timeline mapping is written down), *not* baked into note positions, so it is adjustable
afterwards and undoable by degree. That is a schema bump (8), a migration writing `0`, and
a control in the rack's toolbar. Small, and cheap now; expensive after people have
projects with grooves in them.

Then, in the order the playlist plan gives: **clip chrome and the header map** (the visual
block — a title bar per clip, the track's content as a miniature behind its name), and
**markers and time signatures**.

The two plans below are kept as written, with each item marked as it landed, so the
reasoning stays next to the result.

## What exists

| Area | State |
|---|---|
| Workspace, licenses, ADRs 0001–0013 | done |
| `cantus-core`: time, model, commands, bus, snapshots, effects model, clip windows, pan law / width, **pads and lanes**, **track height and icon** | done; 32 tests incl. 40-seed undo/redo invariant fuzz (now covering pad parameters, pad names and both instrument variants) |
| `cantus-dsp`: PolyBLEP osc, ADSR, SVF, smoother, click, Firmus, **Pulsus (kick, snare, hi-hat, sample pads, choke groups)**, stereo delay, reverb, null-test harness | done; 33 tests, fourteen of them the kit's (block-size independence, determinism, choke, headroom, exact silence, sample playback and retune) |
| `cantus-engine`: transport, processor with insert chains, width / pan law / polarity, **two instruments per slot, stereo instrument path, sample cache**, hardware MIDI in, recording feedback, cpal backend, offline render | done; 16 null/behaviour tests (five for a kit) + 5 effect/MIDI tests + 6 unit tests + 1 real-device test |
| `cantus-project`: versioned JSON (**schema 8**), SHA-256, migrations 0→1→…→8, atomic save, autosave, recovery, fixture corpus | done; 9 tests; fixtures `v1`–`v8.cantus` must always load |
| `cantus-ai`: `AiProvider`, tool catalogue (**+ add_effect**), permissions, command bridge | done (no live provider yet); 3 tests |
| `cantus-ui`: transport, arrangement (clip editing, **time selection, per-track height, icons, reorder by drag**), piano roll (**lanes from the instrument**, 7 mouse tools, clipboard, nudge/transpose, slice, mute), **channel rack across every track**, mixer view, inspector (**channel strip, instrument picker, pad editor with samples**, effects rack), widgets (fader, bar), keyboard piano, MIDI recording, design system, named layouts, editable keymap, settings dialog, **welcome groove**, i18n EN/ES | done; 117 tests (headless egui frames, edit/undo flows, clip and step gestures through published geometry, kit lanes as painted, save→open→export, recording take, effects delivery, fader taper, type scale / spacing grid / bundled fonts / theme upgrade) |

## Verified by running (Windows 11, Realtek WASAPI 48 kHz)

- Window opens maximized (or `--window W H`), audio device opens, MIDI ports are enumerated
  (`MIDI inputs: n` in the status bar; none were attached on the dev machine).
- Offline render is bit-identical to real-time processing for block sizes 1…8192, with
  and without effect chains.
- Real-device test: 200 blocks, 0 overruns, max callback 194 µs at 480-frame blocks.
- Fixture `v2.cantus` opens with a Delay on the Keys track; the effects rack edits it live.
- Round 12, fixture `v7.cantus`: the Drums track opens in the piano roll with seven named
  lanes filling the grid and the kick at the bottom; `--rack --track 1` opens the channel
  rack with the fixture's kick and closed-hat steps lit, the muted pad shown muted, and the
  inspector on *Bombo · Kick*; the header row is 40 px with the drum, keys and strings
  icons and the taller Drums / shorter Bass rows the fixture stores; the inspector's channel
  strip shows the Keys track's −4.5 dB and R25. Checked in English and Spanish.

## The design system (round 5)

`cantus-ui/src/design.rs` owns **form**; `theme.rs` owns **colour** (ADR 0009). The split
matters: colour is user data in `theme.json`, form is a product decision in code.

- **Typography.** IBM Plex Sans + IBM Plex Mono are bundled in
  `crates/cantus-ui/assets/fonts/` (SIL OFL 1.1, licence file alongside them; the faces are
  shipped unmodified because "IBM Plex" is a Reserved Font Name). Views ask for named text
  styles — `design::caption/strong/mono_small/readout/readout_small` — never a raw size.
  **Every number in the UI is monospaced** (`design::paint::ruler` for canvas text) so bar
  counters, dB values and timers do not shift horizontally as they count.
- **The 4-px grid.** `design::space::{S1,S2,S3,S4,S6,S8,ROW,ROW_DENSE}` plus `snap()`. A
  unit test fails if the installed spacing lands off the grid.
- **Elevation.** `design::Elevation::{Canvas,Panel,Card}` returns the `egui::Frame` for a
  surface. Panels are separated from the canvas by a hairline pulled flush with the panel
  edge (a −1 px outer margin), not by a drop shadow — shadows smear the dense grids. Only
  windows and menus cast one (`design::overlay_shadow`).
- **Focus rings.** egui returns the *active* visuals for a focused widget, so keyboard
  focus was invisible. `design::paint_focus_ring` runs once at the end of every frame,
  reads the focused id from egui memory and strokes a ring on a foreground layer — one
  implementation covering every widget in the app, with no call-site changes.
- **Interaction states.** `icons::button` paints rest / hover / press distinctly, and
  reserves the accent fill for *state* (playing, recording, tool selected), never for a
  press. Track colour swatches and mixer strip headers follow the same rule.

Two layout bugs the pass exposed and fixed: the transport and the status bar both let
left-hand content overrun the right-hand readouts at 1200 px. Both now nest a
`left_to_right` layout inside a `right_to_left` one, so the readouts claim their width
first and the flexible content truncates. The Loop / Metronome / Keys text toggles became
icon buttons, which is what freed the space.

## Round 12: Pulsus and the channel rack, the time selection, the shape of a track

Two blocks, in the order the last handoff asked for them. The decisions that turned out to
be architectural are in ADR 0013; this is what was built and what was learned.

### Block A — a channel rack of our own

**1. The piano roll asks the instrument.** `InstrumentSpec::lanes()` returns a `Lanes` —
`Chromatic` for Firmus, `Pads(&[DrumPad])` for a kit — and `lane_name(pitch)` answers
what a pitch is called. The piano roll builds an `Axis` from it every frame (lane → pitch,
pitch → lane, lane → name), plus a row for any pitch the clip holds that the kit has no pad
for, named as a keyboard would name it, so changing a track's instrument can never make
music invisible. The plan had asked for `Option<&[Pitch]>`; the reason it is `Lanes`
instead is written on the type and in ADR 0013 — a slice of pitches is one of the three
mappings an editor needs, and returning it would have put the other two in every caller.

On a kit the rows are the pads, kick at the bottom, **sized to fill the grid** (up to 44
px): seven pads at keyboard height were a strip along the top of an empty canvas.
Vertical moves are in **rows**, and the octave keys move four pads. A drum's note has a
fixed 1/16 length.

**2. Pulsus.** `cantus-dsp/src/pulsus.rs`: kick (sine with a fast pitch envelope, a
high-passed noise click, soft saturation), snare (two tuned bodies a fifth-and-a-bit
apart, band-passed noise with its **own** decay), hi-hat (the 808's six squares as ratios
of a base, band-pass then high-pass), and **choke groups** with a 4 ms fade rather than a
hard cut. Seven pads on the GM map: kick, rim, snare, closed hat, low tom, open hat, high
tom — the toms and the rim are the snare shape retuned, which is a preset and not a fourth
voice, and the kit says so. Everything is one-shot and parameters are captured at the
note-on, which is what makes the output block-size independent without a single smoother.

The null tests went in **before** the engine wiring, as the standing rule says, and two of
them were checked against deliberately broken code: disabling the choke fails exactly the
choke test, and resetting a filter per block fails exactly the block-size test. Two things
the tests then found in the *design*:

- **The kit was too hot.** The kick alone hit 0 dBFS at velocity 127, so a busy bar arrived
  at the fader already clipped. `voice_level` now puts the loudest pad around −6 dBFS — the
  same place Firmus's default gain puts it — and a headroom test holds the line.
- **The saturator was a hidden gain stage.** Peak-preserving normalisation lifts the body
  rather than lowering the top, which is right for a driven kick, but at `k = 1 + 7·drive`
  the default lifted the tail by 10 dB. It is `1 + 3·drive` now, defaulting to 0.2 (about
  +4 dB), with the reason on the function.

The engine holds **both** engines per track slot, preallocated, and switches with a flag
(ADR 0005: no allocation on the audio thread when a track changes instrument). Pulsus
renders **stereo** because its pads are panned; Firmus stays mono and is copied. A muted
pad is resolved in `Snapshot::compile`, like a muted track. `SetInstrumentParam` now
addresses an `InstrumentParam` — `Firmus(_)` or `Pulsus { pad, field }` — so a third
instrument will not need a third command; `SetPadName` renames a pad in the kit. Schema
**6** is the variant's arrival; the migration writes nothing and says why.

**3. The channel rack** (`views/channel_rack.rs`, `F6`). Pads down the side — mute, name
(double-click to rename), level and pan bars — and a step grid across, editing the
selected clip. **A step is a note**: switching one on or off is `InsertNotes` /
`RemoveNotes`, and there is a test that round-trips a project through both views and
asserts it is byte-identical. The steps follow the clip: a repeating clip shows one period.
Left-click on, right-click off, drag to paint with the mode decided at the press, Alt+wheel
for velocity — the same gesture as the piano roll's, because it is the same edit. Velocity
is the height of the lit part of a step, so the graph editor's first column is always on
screen. The pads × steps grid goes into **one `Mesh` with a level-of-detail rule** from
the first line: below four pixels the empty cells stop being drawn (the column shading
behind them says the same thing), below seven the lit steps become plain quads. The test
asserts the *shape count*, not a frame time, so it stays true on a loaded machine.

The rack uses the same `Axis` as the piano roll, which means it is a step view of *any*
clip: on a synth the rows are the pitches the clip uses.

The inspector shows the kit as a row of pad buttons and the selected pad's parameters —
only the fields its voice has, with the ranges its voice needs (a hi-hat's tone lives
around 9 kHz, a snare's around 2 kHz). The choke group is a named list, not a slider: it
is an identity, not a quantity.

### Block B — the playlist

**1. Time selection.** `CantusApp::time_selection: Option<(Ticks, Ticks)>`, a range across
every track. It lives on the app, beside `selected_clips`, and not in the project: a
selection is not part of the piece. The **ruler is two lanes** now — an 8-px loop lane on
top where dragging sets the loop brace (which is what a bare ruler drag used to do), and
the selection lane below where dragging selects time. Reaper splits the ruler the same way
and for the same reason: putting the new gesture on the old one would have cost the loop
its direct manipulation, and putting it behind FL's `Ctrl+drag` would have hidden the
thing everything else hangs off. A click in the selection lane still seeks, and clears the
selection. The selection is drawn as a translucent column across every track, brighter in
the ruler where the edges are grabbed, and its length in bars is read out in the toolbar.

What hangs off it today: `loop-to-selection` (`Shift+L`; sets the region **and** enables
looping, in one undo step) and `zoom-to-selection` (`Z`, Live's key; a view change, so not
a command). Live's `Ctrl+L` was not taken because it is the loop *switch* here and a switch
has to stay reachable when nothing is selected. `Esc` in the arrangement clears both
selections.

**2. Track shape.** `Track.height` (a multiple of the base row, in the **project** — it
does not change what the piece renders as, so ADR 0012 does not force it there, but a
height that did not travel with the file would have to be set again on every machine) and
`Track.icon: Option<TrackIcon>`, where `None` means *derive from the instrument* — a kit
gets a drum, a synth a waveform — so it keeps following the instrument until somebody
chooses. Schema **7**.

The header lost its fader, pan bar and meter to the **inspector's channel strip**, where
the fader has a scale and the pan has a readout. What is left is what a header is for:
colour, icon, name, M/S and a hairline meter. The base row went from 64 px to **40**, and
three tracks that used to fill the panel now leave room for four more. Drag the divider
under a header to resize the track; drag the header itself to reorder (`MoveTrack`, one
undo step); the context menu has height presets, *give every track this height*, and the
icon picker. `Ctrl+↑/↓` change track. `rows_of` is the one function that says where each
row is, and both the headers and the timeline ask it; `TimelineGeometry` publishes the
rows, so a gesture test asks rather than multiplies.

Six painted track icons (`icons::paint_track_icon`): drums, keys, strings, vocal, synth,
effects. The first drums glyph was a circle with a diagonal through it — which at 14 px
is the polarity Ø already on every channel strip. It is a drum with two crossed sticks now.
An icon must not be another icon's twin.

### Three defects found by running, and what they were about

**1. The key labels were painted in the canvas colour.** `paint_keys` wrote `C4` in
`theme::p().ink`, which is the *canvas* — dark on Ink & Gold, where it worked, and white on
Paper, where it had been invisible since round 10. The kit's lane names inherited the same
line and showed up only on every second row, which is what made it visible. It is
`theme::on_light_key` now: whichever of `ink` and `text` is darker, legible on a light key
in any theme. The test that asserted `"C4"` was *painted* had been passing throughout;
text that is painted invisibly is the round-11 lesson in a new shape.

**2. Every divider drag was a reorder.** The header rows were allocated flush, so the
divider line belonged to two rows' hit rectangles and egui gave the press to the later
one — for which the press was not on *its* grip. The interactive rect is shifted down by
the grip now: a row owns the divider *below* it. The gesture test caught this before it
shipped, which is what publishing the geometry is for.

**3. A pan test that measured the wrong thing.** The first stereo test panned the hi-hat
and summed `|x|` over the whole render; the kick dominates that sum, and the hat moved it
by 0.03 %. True, and no assertion can rest on it. The test pans the kick now, and its
companion asserts that with every pad centred the two channels are *bit-identical* —
which is the "a control at its default touches nothing" rule, again.

Also: a flaky guardrail did what round 9 said it would. `stress_project_renders_within_budget`
failed once at 266 ms against a 250 ms ceiling while another project was compiling, and
passed alone. It was left as it is; the note in round 9 already says what it means.

### Round 12, second half: the rack becomes FL's, on the user's direction

After the first half landed, the direction came back: *the channel rack should map several
channels and send them to different mixer channels or group them, use samples or generate
sounds, and open with a kick, snare and hi-hat that sound incredible.* Decision 1 of the
drums plan — one kit on one track, always — was what stood in the way, and it was wrong
about what FL's rack is for. Revised in ADR 0013, and this is what changed:

- **A channel is a track.** The rack lists every track, one row each, with the track's own
  mute, fader and pan (`SetTrackMute` / `SetTrackGain` / `SetTrackPan` — the mixer's
  commands) and the number of the strip it lands on. A kit with several pads expands into
  indented pad rows with the pad's own controls. A drum that needs its own strip is a
  track with a one-pad kit (`PulsusParams::single`); the whole kit on one strip is a
  track with seven. Both are the same type and the same rows.
- **The rack edits a pattern region** — the selected clip's window — on every track. A row
  shows what its track's clip plays inside it; a step on a channel with no clip there
  creates one, the pattern's size and repeat, in the same undo step. The rack **fits the
  pattern to its width** by default and only scrolls once you zoom. Clips stay the unit of
  reuse: still one timeline model. A melodic track is one row whose step adds the clip's
  lowest note, or middle C.
- **Samples per pad.** `PadVoice::Sample` and `DrumPad::sample`, schema **8**. A pad plays
  a WAV — any rate, mono or stereo, resampled and retuned by the pad's tune, choked and
  muted like the others — in the same kit as synthesized pads. `cantus_engine::samples::
  SampleCache` decodes with `hound` and **attaches** the audio to the snapshot
  (`TrackSnapshot::samples`) before it is sent, so real-time and offline render see one
  buffer; the cache never evicts, so an `Arc` dropped on the audio thread is never the
  last. A missing file makes the pad silent and the inspector say so; fixture `v8` points
  at a file that does not exist on purpose. Null tests: block-size independence and
  realtime-equals-offline through a real WAV in a tempdir; tuning up an octave halves the
  length; a stereo file keeps its channels apart; a missing file is exact silence.
- **Any track can become a kit** (`SetInstrument`, an *Instrument* combo in the inspector),
  **pads can be added and removed** (`InsertPad` / `RemovePad`, *+ Pad* and *Remove pad*),
  and *Load…* on a pad picks a WAV (`SetPadSample`, which also switches the voice, in one
  undo step). All four ran through the undo fuzz, which caught the first `PadField::Voice`
  resetting the decay alongside the voice — undo restored one and not the other. A
  parameter set changes exactly one field now.
- **The app opens on a groove.** `welcome_project()`: keys, kick, snare and hi-hat as four
  tracks — the drums each their own strip — playing two bars in a four-bar loop, pad names
  in the interface language. `starter_project()` stays as the tests' fixture, so a change
  to the groove cannot break a test about undo.
- **The voices.** Kick: a second, much faster pitch stage (the beater's knock, gone in
  8 ms) on top of the slow fall. Snare: a small pitch drop at the hit, two noise paths (a
  band-pass for the wires, a high-pass an octave below it for the crack), noise mix
  brighter with velocity. Hi-hat: two milliseconds of stick noise before the metal. The
  stick noise exposed a real non-locality — noise was seeded from a *global* hit counter,
  so adding a kick changed how the next hat sounded; the `kick under hats` residue test
  caught it and the seed is per pad now. I cannot listen; `cargo run -p cantus-ui
  --example render_kit -- out/` renders each drum and the groove to WAV, and those files
  went to the user for judgement. **Whether they sound incredible is not established** —
  the numbers (kick fundamental 47 Hz, ~110 ms to −20 dB; snare centroid brought down from
  10 kHz to ~6; hats −10 dB under the kick) say plausible, and only ears say good.

One mistake to own: mid-way, a `git checkout` on the test file restored the *staged*
round-11 copy and threw away the session's test additions. Every patch had been a script
in the scratchpad, so they were replayed in order and the count came back; the habit of
patching through scripts is what made that a ten-minute loss and not a day.

### What is not done, and why

- **Buses and sends** — the "send several channels to one mixer channel" half of the ask.
  Each drum has its own strip today; grouping them is the next block, designed above.
- **Swing** (decision 4 of the drums plan) — not started; it is schema 9. See *The next
  session*.
- **What hangs off the selection beyond loop and zoom** — insert/delete space, ripple,
  quantize starts, render selection — is step 5 of the playlist plan and the next pull.
- **The rack has no horizontal scrollbar**; it scrolls with the wheel and Ctrl+wheel zooms
  the cells. A mini-map scrollbar is step 6 of the playlist plan and the rack should share
  it.
- **Track groups** are still one tree with the mixer's buses, still not started.

## Round 11: a fader of our own, clip editing, and the pan law

Three blocks, in the order the last handoff asked for them.

### A console fader (and a bipolar bar) — `cantus-ui/src/widgets/`

egui paints a slider's groove and its cap from the same `inactive.bg_fill`, so at rest the
two could only differ by an outline — the limit the round-10 review ran into. `Fader` is
ours: a **recessed groove**, a **raised cap with a read line**, a **dB scale** beside it and
a **unity datum** that crosses the whole lane, so "is this track above or below unity" is
answerable at a glance across twenty strips.

- **The taper is the interesting part.** Linear dB spends half the travel on −60…−30, a
  range nobody mixes in; linear amplitude crushes the useful range into the top centimetre.
  Cantus's fader is **6 dB per 10 % of travel above −24 dB, with unity at 80 %**, and
  steepens below that so silence is reachable without spending the fader on it. Silence is
  a real position (`MIN_GAIN_DB`), not a clamp: pulled all the way down, a fader is off.
  `db_at` and `pos_of` are exact inverses (there is a test over a thousand positions), or
  the cap would jump under the pointer on the frame after a drag.
- **Recessed, in every theme.** A slot is darker than its surface whether the theme is
  dark or light, so `theme::recess` takes whichever of the canvas and the strong grid line
  is actually darker — `ink` on a dark theme, `grid_strong` on a light one — and falls back
  to darkening the surface if a hand-edited theme leaves neither. The mixer's meters use it
  too; on Paper they used to be white slabs.
- **States are edges, never fills.** Rest, hover and grabbed differ by the cap's outline,
  with the accent reserved for the grabbed/focused state — the design-system rule that a
  light theme's "darker fill = pressed" gets exactly backwards.
- `Bar` is the same argument for bipolar controls: egui's slider fills from its left end,
  so a pan at dead centre looks like a pan at 3 %. `Bar` fills **from its detent outwards**,
  which makes the direction the reading and the length the amount, and it snaps to the
  detent within a pixel so centre is reachable by hand.
- Both are used by the mixer, the inspector and the arrangement track headers, and both are
  keyboard-operable.

**Widgets that own the arrow keys.** A focused fader moves with the arrows and so does the
note selection, and `global_shortcuts` runs before any panel is laid out, so it cannot see
that a widget is about to take the key. `widgets::claim_arrow_keys` /`arrow_keys_claimed`
close that: `Keymap::pressed` now takes `arrows_taken` beside `letters_taken`, and suppresses
by *the key pressed*, never by the action's identity — so rebinding Save to `ArrowUp` gets
the same rule the shipped nudge does. Without it, nudging a fader also transposed every
selected note. One egui detail the test pinned down: a widget's focus filter is only
installed on the frame *after* it gains focus, so an arrow pressed in the same frame as the
Tab moves the focus instead of the control.

### Clip editing — schema 4, `cantus-ui/src/clips.rs`

**The decision the last handoff said to make first: a clip is a window onto its content.**
`Clip` gained `offset` (which content tick sits at the left edge) and `content_length` (the
repeat period), plus `muted` and `color`. Notes now live in the clip's own content timeline,
and everything else falls out of that:

- **Splitting moves no notes at all.** Both halves carry the whole note list and differ
  only in `start`/`offset`. Which is why **split then glue gives back the clip, note for
  note** — there is a test that asserts exactly that, on the notes and on the snapshot the
  engine would play.
- **Trimming the left edge** moves `start` and `offset` together, so nothing slides. It is
  also lossless in both directions: notes outside the window are kept, as they already were.
- **Slipping** moves `offset` alone.
- Recording into a clip that has to grow leftwards now moves the offset instead of shifting
  every existing note, which is the same operation as a left-trim.

**Split shares note ids; copying allocates new ones.** Split has to share them, or glueing
the halves would not recognise them as the same notes. A *copy* must not, or glueing a clip
to its own duplicate would see every note as one it already had and throw the lot away.
That asymmetry is deliberate and both directions have a test.

**Repeat is opt-in, and that is a decision.** `content_length: None` — the default and what
every migrated clip gets — is a clip that plays its content once and reveals more of it as
it grows. `Some(period)` repeats. The two are different intents and must not be conflated:
dragging the right edge of a clip you shrank has to give back the notes you hid, and
dragging the right edge of a *looping* clip has to repeat it. Live draws the same
distinction with its Loop switch. The period is a brace around the content, not a modulus:
content outside it is not heard, rather than being piled onto the first bar.

**Drags preview and commit once.** The old code issued a merged `SetClipBounds` every frame
of a drag, which could not express a cross-track move at all and left undo depending on
gesture merging. Now the clips are *drawn* where they would land and one batch is committed
on release — one undo step whatever the pointer did on the way. Move, copy, trim and slip
are one code path.

**Modifiers in the arrangement are not the piano roll's**, and deliberately: there is no
pitch axis here, so Shift and Alt are free. **Alt+drag copies**, **Shift+drag slips**, Ctrl
ignores the grid, Ctrl+click extends the selection (egui reports a click only when the
pointer did not travel, so Ctrl+click and Ctrl+drag never meet). Right-click opens the
clip's menu instead of deleting it outright — deleting the wrong clip with no warning is a
hard thing to undo your way out of.

**Notes and clips share their shortcuts**, and which one runs is decided by `EditFocus` —
the half of the window you last edited in. Cut, copy, paste, duplicate, delete, select-all
and the arrow keys all exist twice over; deciding by "whatever you touched last" is a rule a
musician can predict without being told, and it is not decided by which selection is
non-empty, because both usually are.

New actions: `split-clip` (`Ctrl+E`), `glue-clips` (`Ctrl+J`), `toggle-clip-mute` (`0`),
`toggle-clip-loop` (unbound by default; it is in the clip's menu). WAV export moved to
`Ctrl+Shift+E` in **every** preset, where the Live preset already had it: splitting is
something a musician does a hundred times an hour and exporting a WAV a handful of times a
day, so where the two collide it is the export that moves.

Also: the arrangement's note miniatures now go into **one mesh for the whole view** with a
size check, per the ADR 0011 convention. They were one filled rect per note per clip.

### Pan law, width, mono and polarity — schema 5, ADR 0012

**The law is a field of `Project`,** not a preference. It changes the level of every panned
track in a piece, so two people opening the same file have to hear the same mix. It is
undoable like anything else and it is shown on the master strip, where somebody mixing
looks for it.

Four laws, named by how much a **centred** track is attenuated (the Logic/Pro Tools
convention); all four agree at the extremes:

| Law | Centre | Constant | For |
|---|---|---|---|
| `0` | 0 dB | neither | what Cantus did before — every migrated project gets this |
| `-3` | −3.01 dB | power | **the default**: music on two speakers |
| `-4.5` | −4.52 dB | the geometric mean | the hedge (Pro Tools's default) |
| `-6` | −6.02 dB | amplitude | anything that will be summed to mono |

**`0 dB` is a compatibility guarantee.** `PanLaw::ZeroDb::gains` is the old expression
verbatim, and a test sweeps two thousand pan positions asserting the two agree *exactly*.
Seeding old projects with the new default would have been one line and the worst kind of
bug: one you only hear, in a finished mix, months later.

**The master's pan does not take the law.** A pan law compensates for placing a *mono
source* in a stereo field; the master is already stereo, so the −6 dB law would take 6 dB
off every centred mix for nothing. Its pan is the balance formulation. The null test found
this — a centred mix came out 12 dB down instead of 6, because the law was applied twice.

**Mono is width zero**, not a switch: one value, one control, and no way for a boolean and a
knob to disagree. The mixer still shows a *Mono* button because that is what people look
for, but it is a shortcut to the end stop. Width sits after the inserts and before the pan,
which is a console strip's order — narrowing a stereo reverb has to happen before the
channel is placed. Polarity is an input-stage switch, before the inserts, as on a console;
on the master it is the last thing before the output.

**Null tests.** `width_at_unity_and_polarity_off_change_nothing` is the standing rule made
explicit: a control at its default must not touch a sample, so `apply_width` returns early
at unity rather than computing a mid/side round trip that is only *algebraically* the
identity. `a_track_against_its_inverted_copy_is_exactly_silence` is the classic one — a
track summed with a polarity-inverted copy of itself cancels to zero, not to a small number
— with the flip removed at the end so the test cannot pass by nothing playing.

### Three defects the round-11 review found, and what they were really about

All three shipped green. Worth writing down, because each one says something about what
the tests were and were not asserting.

**1. Resizing a clip moved it instead.** The drag hit-tested where the pointer *is*, but
egui only reports a drag once the pointer has travelled a few pixels — by which time it has
left the 8-px trim zone at the clip's edge, so every resize was read as a move. The fix is
to hit-test the **press origin**, which is exactly what the piano roll learned in round 3
and what its own note about the bug says. The arrangement was written without reading it.

**2. Moving a clip left its notes behind.** `move_to` was one line —
`set_window(track, clip, start, clip.length)` — and `set_window` is *trim* semantics: it
moves `offset` with `start` so the content stays anchored where it sounds. That is right for
an edge and precisely backwards for a move, so a dragged clip slid out from under its own
music. The two operations are opposites on the same two fields and now say so in their doc
comments.

The point is not the one-line fix. **The whole test suite passed with this bug in it**,
because the clip tests asserted on `start` and on the note list — and a clip that has slid
away from its notes still has exactly the `start` you asked for, and the note list of a
clip *never* changes when it moves, since it is the window that moves. The clip unit tests
in `clips.rs` had it right and asserted on `Snapshot::compile` — what the engine would
actually play — and the gesture tests in `app_flows.rs` did not. They do now, and the new
one was checked against the broken code before the fix went in.

**3. Clip editing was invisible.** It existed only on a right-click and on shortcuts
nobody had been told about, and the right-click menu closed the moment the pointer left the
clip — it was drawn inside `if let Some(hit)`, and a context menu appears *next to* the
pointer, so reaching for it took the pointer off its own target. Now: a **Clip** menu in
the menu bar, the context menu opened on the clip that was under the pointer when the
button went down and drawn every frame after, every entry showing the key bound to it
(`Keymap::display_for`, so the menu teaches the shortcut), and the status bar's hint
following `EditFocus` instead of always describing the piano roll.

The **Edit** menu was calling `cut_selection`/`paste_clipboard` directly, so it was
permanently about notes however the arrangement was being used. It goes through
`run_action` now — which is ADR 0010's rule, stated there and not followed here.

### A toolbar for clips, and a key for every one of them

Asked for after the fixes above: the operations to hand, as icons, the way the piano roll's
tools are — because a menu entry and a shortcut are not the same as a row of buttons above
the thing they act on.

`views::arrangement::toolbar` mirrors the piano roll's row exactly: title, icon buttons,
then the grid picker and the snap toggle, then a count on the right. Two new icons (`Glue`,
`Duplicate`); split reuses `Slice`, because it is the same idea applied to a clip and two
glyphs for one operation would be worse than one shared. `active` still means *state* —
mute and repeat light up because the clip is muted or repeating — never a press.

- **The colour button shows the colour**, not an icon of one. A monochrome glyph could only
  say "colour"; the swatch says *which*, and a diagonal bar says "the track's".
- **The grid is the same list the piano roll offers** (`views::GRIDS`), with the same names.
  Two pickers that disagreed about what "1/8" is called would be a small cruelty. The
  arrangement defaults to a bar; the piano roll to 1/16.
- Every tooltip carries the key bound **right now** (`Keymap::display_for`), so the toolbar
  teaches the shortcut instead of competing with it.

**Every clip action now has a key**, and a test asserts it for the whole `Clips` category
rather than for the ones that had one the day it was written: split `Ctrl+E`, glue
`Ctrl+J`, duplicate `Ctrl+B`/`Ctrl+D`, mute `0`, repeat `Ctrl+Shift+L`, colour
`Ctrl+Shift+C`, rename `F2`, delete `Del`.

Two of those are new operations, not just new keys:

- **Rename**, which `SetClipName` had supported since round 3 with no UI at all. `F2` opens
  an editor **on the clip itself** — the name is a label on a rectangle, and typing it in a
  dialog means looking away from the thing you are naming. Enter and clicking away commit;
  Escape and an empty name leave it alone, because a clip with no name is worse than the
  one it had.
- **Colour cycling**, which steps the selection through the palette and off the end back to
  the track's colour — so one key both sets a colour and clears it, and there is no state
  you can reach and not leave.

**`TimelineGeometry`.** The view now records where it drew the timeline each frame, and the
drag tests ask it instead of recomputing the layout from constants. They had mirrored it,
and adding this toolbar broke all four of them — a test that recomputes the layout is a
test that breaks the day the layout changes, which is the day it is least useful.

### What is not done, and why

- **Per-pad mixer routing** for a drum kit waits for bus tracks — see the drums plan.
- **The arrangement is roughly a third of FL's playlist**, and the gap is measured item by
  item in the plan above rather than left as an impression.
- **Repeat by dragging the right edge past the content** is not bound to a bare drag. That
  gesture already means "give me back the notes I shrank away", which is a promise Cantus
  makes and Live makes too. Repeat is the clip menu's *Repeat the content* and the
  `toggle-clip-loop` action.
- **`Ctrl+A` selects notes or clips by focus**, but there is no separate "select all clips
  on this track" yet.
- The **velocity lane at the widest zoom** still overlaps into a solid block (round 9).

## Planned: the arrangement, measured against FL Studio's playlist (requested 2026-09-10)

Read against [FL's playlist manual][fl-playlist] and its own screenshots. The point is not
to end up looking like FL — it is to know exactly what is missing and to decide, for each
thing, whether to take it, take it differently, or not take it at all. An unexamined "we
should have that too" is how a DAW ends up with two of everything.

[fl-playlist]: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/playlist.htm

### Where we stand

The arrangement today: one clip type (notes), tracks of one fixed height, a bar ruler with
a loop region, a clip toolbar, and the round-11 clip editing — split, glue, duplicate,
trim, slip, repeat, mute, colour, name, marquee and `Ctrl+click` selection, drag between
tracks, Alt-drag to copy. That is a working arrangement and roughly Live's Session-less
core. It is perhaps a third of what FL's playlist does.

### The gap

**Verdicts:** *take* — do it as FL does. *differently* — the need is real, the mechanism
should not be copied. *not this* — a deliberate no, with the reason. *blocked* — wanted,
but something else has to exist first.

#### Clip types

| FL has | We have | Verdict |
|---|---|---|
| **Audio clips** — waveform, fades/gain envelope, time-stretch, beat-slicing, zero-cross slicing, reverse | nothing | **take**, and it is the single biggest hole. Needs an audio-file model, a sample cache, and waveform drawing (a `Mesh` with LOD from line one) |
| **Automation clips** — control-point envelopes, step/slide modes, articulator tools | nothing | **take** — already roadmap step 3. FL's *articulators* (flip, scale, normalise, smooth, "create sequence") are worth stealing wholesale; almost nobody else has them |
| Stereo / combined / **spectral** view per audio clip | — | **take, later**. A spectral view in the arrangement is rare and genuinely useful; it is also a lot of DSP for a view |
| Pattern clips carrying **notes *and* automation** in one clip | notes only | **differently** — with automation lanes on tracks, a clip carrying both is a modelling choice we should make once, deliberately |
| **Stem extraction / Deverb** (source separation) | — | **not this**, not for a long time. It is a machine-learning product inside a DAW; it belongs after the AI layer exists, if ever |

#### Tracks

| FL has | We have | Verdict |
|---|---|---|
| **Per-track height**, dragged on the divider, presets 33–200 %, lock to size | ~~one global height~~ **done (round 12)**: divider, four presets, *give every track this height*, stored in the project | **take**. This is small and it is felt every session |
| **Track groups** (drag to nest), group solo/mute, auto-colour a group | none | **take** — and it should be the same tree the mixer's buses use, not a second grouping |
| **Track icons** beside the name | **done (round 12)**: six painted icons, derived from the instrument until one is chosen | **take**. An icon is read faster than a name at 40 px |
| **Track modes** (unassigned / instrument / audio) linking track ↔ channel ↔ mixer track | every track is an instrument track | **differently.** FL needs modes because its playlist is *not* the instrument list; ours is. A track is an instrument or (later) an audio track, and that follows from what is on it |
| **Auto-name / auto-name clips** from the instrument | manual names | **take** — a new clip should be called what the track plays, not "Clip" |
| **Reorder tracks** by drag or Shift+wheel | **done (round 12)**: drag the header | **take**. The command is already there and reversible; it needs a drag handle |
| **Track selector** strip, `Ctrl+↑/↓` to change the selected track | **done (round 12)**: `Ctrl+↑/↓` | **take** |
| **Performance settings** per track (launch modes, trigger columns) | — | **blocked** on a session/clip-launch view, which is a whole feature (idea.md §5.4) |

#### Time: markers, signatures, arrangements

| FL has | We have | Verdict |
|---|---|---|
| **Named time markers** in the ruler | loop region only | **take.** Markers are how a song becomes navigable, and they cost a `Vec<Marker>` and a schema bump |
| Marker **actions**: Start, Loop, Marker loop, Marker skip, Marker pause, Start/Stop recording | — | **take the first three, differently.** Loop/skip/pause turn the ruler into a little program, which is powerful and unreadable. A marker should say *where* something is; a arrangement-level structure should say what plays. Take Start and Loop; leave the branching to a proper arranger track (idea.md §5.4) |
| **Time-signature changes** at markers, per region | one signature per project | **take.** It changes the ruler, the grid, the bar numbers and `ticks_per_bar` everywhere — the reason to do it early rather than late |
| **Multiple named arrangements** sharing channels and mixer | one | **take, differently.** The need is real (an edit, a radio version). FL's caveat — "be careful not to delete Channels used by other Arrangements" — is a warning about a model that lets you break the thing you are not looking at. Ours should make the shared part explicit rather than a footnote |
| **Insert / delete space**, slice-and-insert (`Ctrl+Ins`, `Ctrl+Del`, `Ctrl+Alt+Ins`) | — | **take.** This is ripple editing and it is how a song's structure actually gets changed |

#### Selection and editing

| FL has | We have | Verdict |
|---|---|---|
| **Timeline selection**: `Ctrl+drag` in the ruler selects a time range across all tracks | **done (round 12)**, on a bare drag in the ruler's lower lane; the loop brace moved to its own lane on top | **take.** It is the anchor for insert/delete space, loop-to-selection, render-selection, quantize and consolidate. Nearly every entry below depends on it |
| **Eight tools** (draw, paint, delete, mute, slip, slice, select, zoom) as modes | modifiers and a toolbar | **differently.** The piano roll has tools because a note is drawn; a clip is mostly moved. Add the two that are modes rather than gestures — **slice** and **mute** — and leave the rest as direct manipulation. Eight letter keys to memorise is FL's tax, not its feature |
| **Magic lasso** (loop the pointer around clips, no click) | marquee | **take.** It is genuinely faster and it is one of the few things nobody else copied |
| **Clip groups** (`Shift+G` / `Alt+G`) — move and delete together | — | **take** |
| **Make unique** (detach a clip from its shared source) | n/a — clips own their notes | **decision, see below** |
| **Ripple edit** on a clip edge (`Shift+S`) | — | **take** once timeline selection exists |
| **Swap clip source**, transpose clips, "select all similar" | — | **take** — cheap, and each saves a repetitive job |
| **Quantize clip start times** (`Ctrl+Q`) | note quantize only | **take** |
| Snap: Main / Line / **Cell** / None / steps / beats / bar / **Events** | grid list + snap toggle | **take Cell and Events.** "Cell" snaps to the grid cell rather than the line, which is what you want when placing; "Events" snaps to other clips' edges, which is what you want when butting two clips together |

#### Navigation

| FL has | We have | Verdict |
|---|---|---|
| **Zoom & scroll bar** doubling as a **mini playlist preview** of the whole song | plain scrolling | **take.** It is the single best orientation device in FL's playlist, and it is cheap once clips are already drawn as a mesh |
| Zoom presets `Shift+1..5`, zoom to selection, `Ctrl+RMB` zoom, middle-mouse pan | zoom to selection **done (round 12**, `Z`); the rest not yet | **take** |
| **Vertical zoom** / resize all tracks | — | **take** with per-track height |
| **Center on playhead** (`Shift+0`), follow playback | — | **take** |
| **Picker panel** — every clip in the project, filterable, drag into the playlist | — | **take, differently**, and only after the source decision below: without shared sources a picker is a list of things to copy, which is much less useful |

#### Rendering and performance

| FL has | We have | Verdict |
|---|---|---|
| **Consolidate** a track or selection to audio, muting the source (freeze) | — | **blocked** on audio clips; then **take**. It is how a CPU-heavy project stays playable |
| **Export all tracks / track groups** as stems | whole-mix WAV only | **take.** The render path exists and is already proven bit-identical; stems are a loop over it with solo applied |
| **Performance mode** — trigger and loop tracks and clips live | — | **not this, here.** Live triggering is a *session view* (idea.md §5.4), and bolting it onto the linear playlist is exactly the "two DAWs in one" trap. It should be its own view sharing the same clips |

### The three decisions to make first

Like the content offset was for clip editing: get these wrong and everything above has to
be redone.

**1. Does a clip own its notes, or reference a shared source?** In FL a Pattern Clip is an
*instance* of a pattern; twenty copies of a drum loop are twenty views of one thing, and
editing any of them edits all. "Make unique" is what detaches one. In Cantus a clip owns
its notes, so twenty copies are twenty independent note lists.

FL's model is the productive one for the music FL is used for, and the picker panel, "select
all similar", "make unique" and "swap source" all exist only because of it. But it also
means every edit is an action at a distance, which is why *make unique* has to be so
reachable.

The recommendation is to add it as an **opt-in link, not the default**: `Clip.source:
Option<PatternId>`, where `None` is exactly today's clip and `Some` means the notes live in
a project-level pool. Independent by default, shared when you ask; "make unique" writes the
pool's notes back into the clip. That keeps every existing project and every existing test
meaning what it meant, and it is a strict addition to the model rather than a replacement.
It is also a schema bump, a change to `Snapshot::compile`, and a change to every command
that edits notes — so it is the *first* thing to do, not a later one.

**2. Timeline selection, before anything that depends on it.** A time range across all
tracks is the anchor for insert/delete space, ripple, loop-to-selection, quantize starts,
render selection, consolidate, and zoom-to-selection. Building any of those first means
building it twice.

**3. Per-track height and track groups are the same feature as mixer buses.** A "group" in
the arrangement and a bus in the mixer must be one tree, decided once. Two grouping systems
that nearly agree is a bug that takes a year to surface and a rewrite to fix.

### The interface gap, specifically

The honest summary: **we are ahead of FL on discipline and well behind it on density and
richness.** Both halves of that are worth stating precisely, because "make it more polished"
is not a task and this list is.

**What FL's playlist does better than ours, and why it works:**

1. **Every clip has a title bar.** A darker strip across the top with a type icon and the
   source name, separated from the content. It does three jobs at once: says what kind of
   clip this is, gives a grab handle that is not the content, and keeps the name legible
   over a busy waveform. Ours paints the name straight onto the body, so it competes with
   the notes underneath.
2. **The clip body *is* the preview.** Notes, automation curves as filled areas, waveforms,
   spectra — a clip is recognisable without reading it. Ours draws a note miniature and
   nothing else, because there is nothing else yet.
3. **The track header is a map, not a label.** FL draws a miniature of the whole track's
   content behind the track name. You can find the chorus without scrolling.
4. **Colour identity is total.** The track's colour tints the header, the clip title bar,
   the body and the content, so you navigate a forty-track song by colour alone. Ours tints
   the clip and a 4-px stripe on the header.
5. **Density.** FL fits a name, mute/solo, a menu and a content preview into about 40 px of
   track height. Ours uses 64 px for a name, M/S, a fader, a pan bar and a meter — which is
   a *mixer strip* in a track header, and it is why only four tracks fit on screen.
6. **The scrollbar is a mini-map** of the entire song. It is the best orientation device in
   the playlist and it costs almost nothing once clips are already drawn as a mesh.
7. **Time markers are chips in the ruler** with names, and a time selection is a translucent
   column across every track. Both make the song's structure visible rather than remembered.

**What we already do better, and should not give up to get the above:**

- **Type.** Bundled IBM Plex with a named scale, and every number monospaced so nothing
  jitters as it counts (ADR 0009). FL's playlist sets everything in one small proportional
  face; its bar numbers shift as they scroll.
- **Colour is the user's**, editable to 22 tokens, with a test that every shipped theme
  clears WCAG AA. FL's is fixed per theme and several of its greys fail contrast outright.
- **Keyboard focus is visible** on every widget in the app, from one implementation. FL's
  playlist shows none at all.
- **The accent means state**, never decoration. FL colours everything, which is precisely
  why nothing in it stands out.
- **Every menu entry shows its shortcut**, so the menu teaches the keyboard.

**What "superior" would concretely mean.** Not FL's look — FL's *information*, at our
standard of craft:

| | |
|---|---|
| **Clip chrome** | Title bar with a type icon and a grab area; the name stays readable down to a 12-px-wide clip, and disappears rather than smearing below that |
| **Body as preview** | Every clip type draws its content, at every zoom, through a `Mesh` with an LOD rule (ADR 0011). FL degrades to mush when zoomed out; we already have the machinery not to |
| **Header as map** | A miniature of the track's content behind the name — and the mixer-strip controls moved out of the header into the inspector where they belong, which is what buys the density back |
| **Mini-map scrollbar** | The whole song in the scroll bar, with markers on it |
| **Structure visible** | Marker chips in the ruler; a time selection as a column across all tracks |
| **Motion** | State changes animate — a clip landing, a selection forming, a marker being dragged — and nothing else does. Never at the cost of a frame; the budget is the ADR 0011 one |
| **Every zoom readable** | The one thing no DAW does well. A 300-bar view should still tell you where the chorus is |

### Order of work

Grouped so that each block is shippable and nothing is built twice. The drums block
(previous section) is independent of all of this and can go before, after or between.

1. ~~**Timeline selection**~~ **done (round 12)** — with loop-to-selection and
   zoom-to-selection hanging off it; the rest of what hangs off it is step 5.
2. ~~**Track shape**~~ **done (round 12)** except auto-naming clips from the instrument,
   which is small and still open. The header is 40 px and the fader/pan/meter are in the
   inspector.
3. **Clip chrome and the header map** — the visual block above. No model changes, and it is
   the one that will make the arrangement *feel* like a finished product.
4. **Markers and time signatures**: named markers, Start and Loop actions, per-region time
   signature. A schema bump, and it touches the ruler, the grid and `ticks_per_bar`
   everywhere — which is the argument for doing it before there are many projects.
5. **Structural editing**: insert/delete space, ripple, clip groups, magic lasso, snap to
   cell and to events, quantize clip starts, swap source, select-all-similar.
6. **Navigation**: mini-map scrollbar, zoom presets, zoom to selection, centre on playhead,
   follow playback, middle-mouse pan.
7. **Shared clip sources** (decision 1) and then the **picker panel**, which is only worth
   much once sources are shared.
8. **Audio clips** — the biggest single block, and the one that needs a file model, a sample
   cache, waveform drawing and stretch. Then **consolidate/freeze** and **stem export**,
   which are cheap once audio clips exist and the render path is already proven.
9. **Automation clips and lanes** (roadmap step 3), with FL's articulator tools — flip,
   scale, normalise, smooth, create-sequence — which are nearly free once envelopes exist
   and which almost no other DAW offers.
10. **Multiple arrangements**, once there is enough in a project for a second version of it
    to be worth having.

**Deliberately not on this list:** performance mode (it is a session view, and it belongs in
its own view sharing the same clips — not bolted onto the linear timeline), stem separation
and de-verb (a machine-learning product inside a DAW), and FL's eight-tool model for the
arrangement (two of the eight are modes; the rest are gestures we already have).

## Planned: drums, a channel rack, and a piano roll that knows what a pad is (requested 2026-09-10)

**The ask.** A drum kit with a *good* kick, snare and hi-hat; a way to program it in the
piano roll with the keys adapted or renamed for percussion; and a **channel rack** like FL
Studio's ([its manual][fl-rack] is the reference for what that means).

[fl-rack]: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/channelrack.htm

### The decisions to make first

These are the equivalents of the content offset in the clip work: get them wrong and every
later piece has to be redone.

**1. A kit is one instrument on one track, and pads are pitches.** ~~Not one track per drum.~~
*Revised the same day (round 12, second half; ADR 0013): a channel is a track, so a drum
that needs its own strip is a track with a one-pad kit, and the rack lists every track.
Per-drum routing is therefore real today up to one strip per drum; grouping several into
one strip is buses, next.*
A kit is one instrument the way a piano is; a pad is a note on it. Everything already
built then applies unchanged — one clip, one note list, the same commands, the same undo,
the same snapshot. FL's model, where every drum is its own channel with its own mixer
track, buys per-drum inserts, and that is worth having — but it needs **bus tracks and
sends**, which are already next-but-one in the roadmap. So: pads carry their own level, pan
and choke inside the kit now, and per-pad *routing* arrives with the buses rather than
being faked before them.

**2. The step sequencer is a second editor for the same clip, not a second data model.**
A step is a note: pad pitch, position `i × step`, a short length, a velocity. Toggling a
step is `InsertNotes`/`RemoveNotes` like everything else, so it undoes, it saves, and it is
the same thing the piano roll is editing — switch views mid-edit and nothing is lost or
converted. FL keeps them as two representations that need converting between; we do not
have to, and should not.

**3. A "pattern" in Cantus is a clip.** FL's Channel Rack is a *global* pattern bank: every
channel shares one pattern, and patterns are placed in the playlist. That is a second
timeline model beside the one Cantus has, and running both is how a DAW becomes two DAWs.
Clips already do what patterns are for — duplicate, repeat (`content_length`, round 11),
colour, name — so the channel rack edits **the selected clip**, and reuse stays clips.
This is the one place where we deliberately do not copy FL, and it should be said out loud
rather than discovered.

**4. Swing is a clip property, applied when the snapshot is compiled.** Not baked into note
positions. FL's swing is a playback-time offset of the even steps and is adjustable
afterwards; if we wrote it into the notes it would be destructive and un-un-doable by
degree. That makes it a `Clip` field and therefore a schema bump — cheap now, expensive
after people have projects with grooves in them.

**5. Lane names come from the instrument.** The piano roll asks the track's instrument what
a pitch is called and which pitches are worth showing, instead of assuming a keyboard.
`InstrumentSpec` grows `lane_name(pitch) -> Option<&str>` and `lanes() -> Option<&[Pitch]>`;
`pitch_name` stays the fallback. That is what makes drum mode a property of the *kit*
rather than a mode the user has to remember to switch on, and it is what a sampler with
named zones will want later too.

### Pulsus — the kit

Named like Firmus: *pulsus* is the Latin for a beat, a striking. **Synthesized, not
sampled**, and that is a decision rather than a shortcut:

- ADR 0001 requires bundled samples to be original or CC0, so every sampled kit is a
  licensing question before it is a musical one. Synthesis has none.
- A synthesized drum is *tunable*: the kick's pitch envelope, the snare's body, the hat's
  decay are parameters, not a file you replace. That is the difference between a kit and
  a folder of wavs.
- It costs kilobytes, and the sampler (roadmap) is the right home for sampled kits anyway.

| Voice | Design | The parameters that matter |
|---|---|---|
| **Kick** | Sine body with a fast downward pitch envelope, plus a short click transient, plus soft saturation | start pitch (~110 Hz), end pitch (~45 Hz), pitch decay (~40 ms), amp decay, click level, drive |
| **Snare** | Two tuned bodies (≈180 Hz and ≈330 Hz) plus band-passed noise, decaying separately | body pitch, body/noise mix, body decay, noise decay, noise tone (band-pass centre) |
| **Hi-hat** | Six detuned square oscillators through a band-pass then a high-pass — the 808 method, which is what makes a hat metallic rather than just bright | tone (band-pass centre), closed decay, open decay, HP cutoff |

**Choke groups** are not a detail to leave for later: an open hat must be cut by a closed
hat, or the kit does not sound like a kit. A pad names a choke group; a pad triggering
silences any voice in the same group. It is a handful of lines in the voice allocator and
it is the difference between "hi-hats" and "two unrelated noises".

**The standing rule applies before any of it lands:** a null test per voice — block-size
independence and offline-equals-realtime — the same as every other DSP module.

### The channel rack

A panel listing the kit's pads, one row each, editing the selected clip. Following FL's
layout where it is right, which is most of it:

| Element | FL's | Ours |
|---|---|---|
| Mute | per channel | per pad |
| Pan / volume knobs | per channel, pre-mixer | per pad, inside the kit |
| Channel button | opens the plugin | opens the pad in the inspector |
| **Step grid** | 16th notes, left-click on, right-click off | the same, on the selected clip's notes |
| Steps per pattern | up to 512 | the clip's length ÷ the grid, so it follows the clip |
| **Graph editor** (`Ctrl+K`) | velocity, pan, pitch, shift, … per step | velocity first; the rest as the note model grows |
| Global + channel swing | both must be > 0 | one swing per clip (decision 4) — two controls that must agree is a trap |
| Mixer routing | per channel | one strip per track today (the rack shows the number); several channels into one strip needs buses — next |
| Channel groups/filters | named groups, Page Up/Down | later, and only if a kit ever gets big enough to need it |

The grid is dense — pads × steps, redrawn every frame — so it goes into a **`Mesh` with a
level-of-detail rule** from the first line, per ADR 0011. Not after it is slow.

### Drum mode in the piano roll

With decision 5 in place this is small, and it is what "adapting the keys" means:

- The key strip shows **pad names** — Kick, Snare, Closed hat — instead of C1/D1/F#1, and
  the black/white key colouring gives way to one lane per pad.
- Only the pitches the kit actually uses get a lane, so a 12-pad kit is 12 rows and not
  128 with 12 in use.
- Notes default to a fixed short length, because a drum has no note-off worth aiming at.
- A pad's name is editable, and travels in the kit rather than in the project, so a kit
  renamed once is renamed everywhere it is used.

The velocity lane, the tools, the clipboard, quantize and everything else keep working
unchanged — which is the whole reason for making a pad a pitch rather than a new kind of
thing.

### Order of work

1. ~~**`InstrumentSpec::lane_name` / `lanes`**~~ **done (round 12)** — as `Lanes`, not
   `Option<&[Pitch]>`; ADR 0013 says why.
2. ~~**Pulsus**~~ **done (round 12)**, schema 6. Seven pads: the three voices plus toms and
   a rim as presets of the snare shape.
3. ~~**The channel rack panel**~~ **done (round 12)**, `F6`.
4. **Swing** as a clip property — **still open**, and the next schema bump (8).
5. Per-pad mixer routing, when bus tracks arrive.

More voices — toms, clap, rim, ride, crash — are the same shapes with different numbers
once the three above are right, and are not worth planning separately.

## Appearance (round 10): white by default, and yours to change

**The default is now light.** `theme::paper_palette()` — white canvas, light grey panels,
ink text, the gold accent darkened until it is readable on white. Ink & Gold stays as the
dark preset, and Parchment as the warm light one; *Settings → Theme* switches between them.
An existing `theme.json` is left alone, so anyone already on a theme keeps it.

**Every colour is legible, and a test says so.** `every_shipped_theme_is_legible` computes
WCAG contrast for each preset: 4.5:1 for text and the accent, 3:1 for the focus ring, the
bright accent and the selection. Writing it immediately found two real defects in the
*existing* Parchment theme — its gold managed 2.88:1 against the panel, so every heading
was decorative rather than readable, and its dim text sat on exactly 4.50:1. Both fixed.

**The whole palette is editable in the app**, not just in the file. *Settings → Colours*
opens a window with a picker for each of the 22 tokens, grouped, applied as you drag and
written through to `theme.json` — which stays the source of truth and what `ThemeWatcher`
reloads. `Theme::tokens_mut` is what the dialog iterates, and a test asserts it exposes
every hex field the struct serializes, so a token cannot be added and silently left
unreachable.

**Interface size** is a setting too (`settings.ui_scale`, 70–200 %), applied through
egui's zoom factor so the spacing grid scales with the type instead of only the text
growing. Clamped, NaN included, because it comes from a file people edit.

Two defects the white ground exposed, both fixed:

- The piano roll dimmed everything past the end of the clip with a **black scrim**. On ink
  it was invisible; on white it was a heavy grey slab covering half the view. It now paints
  the panel colour — one step of the elevation scale, right in any theme.
- **Selected notes were painted white**, which was my own regression from the mesh
  batching: unmistakable on ink, invisible on paper. Selection now uses the palette's
  `selection` colour with a `text`-coloured outline, and the contrast test covers it.

**Faders**: egui defaults to a chunky rectangle on a thick rail — the "old Windows" look.
They are now a slim cap (`HandleShape::Rect { aspect_ratio: 0.38 }`) on a 4-px groove with
a defined edge. **Known limit:** egui paints a slider's groove and its cap from the same
`inactive.bg_fill`, so at rest they can only differ by that edge. A cap with its own fill,
a centre line and dB ticks needs a widget of our own — the next thing to do for the mixer.

## Piano roll performance (round 9) — and why we stayed on egui

Prompted by "would Slint be better?", ADR 0011. The honest answer needed a number, and the
number did not exist: the stress test ran at a zoom where culling left **139 notes** on
screen, so ADR 0003's "60 fps with 20 000 visible notes" was never being checked.

Measured properly, with tessellation included:

| | Frame | Triangles | Per note |
|---|---|---|---|
| Before | **74.7 ms** (13 fps) | 416 772 | 20.8 |
| After | **6.9 ms** (146 fps) | 96 772 | 4.8 |

Two causes, both ours. Notes were drawn as rounded rectangles with a stroked outline at
every zoom, and epaint anti-aliases every filled shape into a feathered ring — about twenty
triangles for a rectangle. About forty lines fixed it: `NoteLod` decides what a note is
worth at its size on screen, and anything below the thresholds joins one `egui::Mesh` at
two triangles each. Tessellation fell from 60.4 ms to 2.9 ms. At editing zoom nothing
changed visually — verified by screenshot.

**Convention this establishes:** anything drawn once per note, clip, sample or automation
point goes into a `Mesh` and gets a level-of-detail rule. Not an optimisation to remember —
a rule, because the convenience painters cost ~10x per item.

**What the speed bought:** the piano roll's zoom-out limit was 4 px/beat — about 80 bars —
and it was there because drawing more was ruinous. It is now 0.5 px/beat
(`MIN_PX_PER_BEAT`), so a 1 300-px grid shows some 650 bars and a whole piece fits on
screen. That needed the ruler to thin its bar numbers out, which it now does by doubling
(every bar, every 2, every 4 …) so the numbers shown stay ones a musician counts in, never
every 7th; a test asserts it. Verified by running: the 20 000-note stress project at the
widest zoom draws 6 540 notes in a 6.8 ms frame in a *debug* build, ruler legible, zero
audio overruns.

Known refinement: at that zoom the velocity lane's per-note bars overlap into a solid
block. Truthful about density, but it could show the peak per column instead.

Note on verification: driving the running app with synthetic clicks is unreliable on
Windows — twice now a stray event has landed in the app during a screenshot (once dragging
the autosave slider, once switching theme and selecting every note). When a capture
disagrees with the code, check `settings.json` / `theme.json` before believing the
screenshot, and prefer the headless render tests, which assert on the text actually
painted.

Also fixed: `looping_wraps_and_keeps_playing` asserted `stats().overruns == 0`, which
compares wall clock against the block budget. On a loaded machine a 1 000-block run
exceeds it and the test fails, in an otherwise fully deterministic null test. Real-time
behaviour belongs in `tests/hardware.rs`; a flaky guardrail teaches people to re-run
instead of read.

## Settings (round 8)

`cantus-ui/src/settings.rs` and the *View → Settings…* dialog. Preferences — the ones a
dialog can hold in a handful of rows — live in `settings.json` beside the other user files:
audio device, sample rate, buffer size, language, autosave interval. The theme deliberately
stays in `theme.json`: it is a whole document the user edits by hand and the app reloads
live, not a row in a dialog.

- **Audio changes are applied on *Apply*, never as you drag.** Reopening the device on
  every frame of a combo box would stutter the audio.
- **A settings change can never leave the user in silence.** `EngineHandle::start` falls
  back to the system default when the saved device is not there (unplugged since last
  time), and if nothing can be opened at all, the engine already running is left alone.
  Either way the status line names the device actually in use, so a fallback is visible
  rather than mysterious — there is a test for exactly that.
- **The buffer size is shown in milliseconds** next to the frame count, because that is
  what a musician is actually choosing between.
- **The autosave interval is clamped on the way out** (10 s–10 min): a `settings.json`
  hand-edited to `0` must not turn autosave into a per-frame write of the whole project.
- **Language and theme moved out of the *View* menu** into Settings. Two places to change
  one thing is how they drift apart.
- A test asserts that *opening* the dialog changes nothing and writes no file. A
  preferences window that rewrites preferences by being looked at is a nasty bug, and every
  widget in it is bound to a value that could report "changed" on its first frame.
- `--settings` and `--shortcuts` open those dialogs at startup, next to `--mixer`. They
  exist so that looking at a view does not depend on driving menus, which synthetic clicks
  do not do reliably on Windows.

## The keymap (round 7)

`cantus-ui/src/keymap.rs` and ADR 0010. An `Action` is a thing the app can do, a `Binding`
is a key plus exactly those modifiers, and `keymap.toml` joins them — next to the theme and
the layouts, and written on first run so it is there to be found. *Help → Shortcuts* is now
a real editor: pick a preset, click a shortcut to clear it, click *Add* and press a key to
bind it, with conflicts flagged in red.

- **`CantusApp::run_action` is the single dispatch.** Shortcuts, menus and any future
  script go through one `match`, so a binding cannot reach something a menu cannot. The old
  `if cmd && z` chain in `global_shortcuts` is gone.
- **Modifiers match exactly**, or `Ctrl+Z` would fire on `Ctrl+Shift+Z`.
- **Presets are conflict-free by construction**: a preset is a shared spine plus its own
  bindings, and a later binding takes the shortcut from whoever held it. Tests assert that
  no shipped preset conflicts with itself and that none leaves an essential action unbound
  — that is how the Live preset's `Ctrl+E` (split) colliding with WAV export was caught
  before it shipped; export moved to `Ctrl+Shift+E` there.
- **FL / Live / Bitwig presets are approximations**, following each program's conventions
  for the actions Cantus has. The dialog says so. Only the Cantus preset is authoritative —
  claiming parity we have not verified would be worse than saying nothing.
- **A damaged file never stops the app**: unknown ids and unreadable shortcuts are logged
  and skipped, and something that is not TOML falls back to the default map.
- Views no longer read keys. The piano-roll tooltips ask `Keymap::display_for`, so they
  show whatever is bound today instead of a typed-in "P / 1". `Tool::shortcut()` and
  `Tool::key()` are gone; `Tool::action()` replaces them.
- Mouse and wheel modifiers are not bindings and are not in the file; they are listed
  below the scrolling list — not inside it, where a newcomer would never scroll far enough
  to find them.
- `cantus --shortcuts` opens the editor at startup, next to the existing `--mixer`. Both
  exist so a look at a view does not depend on driving menus.

## Named layouts (round 6)

`cantus-ui/src/layout.rs`. A `Layout` is the shape of the window — the inspector's width,
the arrangement's height, which of the two are showing, and which view fills the centre.
`Layouts` holds the current one plus the user's named ones, in
`%LOCALAPPDATA%/Cantus/layouts.json`. *View → Layout* switches between them, toggles the
panels, saves the current shape under a name, deletes one, or resets to the built-in shape.

Two things about egui 0.36 shaped the design, both found by experiment and both recorded in
tests so a future upgrade tells us if they change:

- **`Panel::default_size` cannot impose a size** once a panel has stored state, and eframe
  is built here **without its `persistence` feature**, so egui does not carry panel sizes
  across restarts at all. The layouts file is what makes sizes survive: the app captures the
  layout on exit and forces it on the first frame.
- **Applying a layout uses `Panel::exact_size` for exactly one frame** (`Layouts::forced` /
  `end_frame`). `exact_size` pins the size range to a point, so it wins whether or not the
  panel has state; the frame after, the panel is resizable again from there. There is no
  setter for `PanelState`, and forging egui's internal state would break the first time it
  changed shape.

A bare `egui::Context` does not keep panel sizes between frames the way eframe's does, so
the tests assert what a *forced* frame produces and leave "a dragged size stays put" to be
checked by running the app. That is noted at the top of the module so the next person does
not read the gap as an oversight.

## Piano roll tools (round 3)

Bug fixed: drags hit-test the **press origin**, not the current pointer, so resizing from a
note's right edge no longer turns into a marquee. Tools use FL's letters — `P` Draw, `B`
Paint, `E` Select, `D` Delete, `T` Mute, `C` Slice, `Y` Play — and also `1`–`7`. The typing
keyboard piano starts **off** (as in FL) because it claims the letter keys; `Ctrl+T` or the
*Keys* toggle in the transport turns it on, and then only the number keys switch tools. Modifiers follow FL:
Shift locks pitch, Ctrl locks time, Alt bypasses snap, right-click deletes, Ctrl+drag
selects, Alt+wheel edits velocity. Shortcuts: Ctrl+B/Ctrl+D duplicate, Ctrl+C/X/V, arrows
nudge (Shift: bar) and transpose (Shift: octave), Esc clears, Ctrl+T toggles the typing
piano. `Note.muted` is schema 3 (fixture `v3.cantus`). All operations live on `CantusApp`
(`slice_at`, `toggle_mute_notes`, `duplicate_selection`, …) so they are unit-tested without
a window.

## How the new pieces fit

- **Effects** (`cantus_core::effect`): a generic `kind + key→f32` parameter model. The
  audio thread never allocates: `EngineHandle::set_snapshot` builds `EffectUnit`s on the UI
  thread for ids the engine has not seen and sends `AddEffect` before the snapshot; units
  dropped by a snapshot come back as `GarbageEffect`. `Processor::rebuild_chain` re-orders
  a track's chain with pre-sized Vecs only.
- **Recording** (`cantus_ui::recording::Recorder`): the engine stamps live notes
  (`EngineEvent::NoteInput`) with the transport tick while `SetRecording(true)`; the UI
  pairs on/off, handles loop wrap and retriggers, and commits each take as **one undo step**.
  `CommandBus` gained *composing* gesture merges for accumulating gestures
  (`Command::supersedes` decides replace-vs-compose).
- **Hardware MIDI** (`cantus_engine::midi`): every input port is opened at start; callbacks
  push into an SPSC ring the audio thread drains each block. The selected track is the
  live track (`SetLiveTrack`).
- Timing note: live notes are stamped at block start (≤ 10 ms at 480 frames). Sub-block
  timestamps from `midir`/cpal are a later refinement.

## Design & personalization plan (requested 2026-09-08)

Goal: a look that says "best DAW in the world" and a UI the user can make their own.
Ordered plan, each step shippable on its own:

1. ~~**Theme tokens as data**~~ **done (round 4)**: `theme::Theme` is serialized to
   `%LOCALAPPDATA%/Cantus/theme.json` (created from "Ink & Gold" on first run), reloaded
   live by `ThemeWatcher`; presets Ink & Gold (dark) and Parchment (light) under
   *View → Theme*, plus "Open theme file". Views read colours via `theme::p()` (a copied
   `Palette`), never constants. `icons.rs` paints all toolbar/transport icons as shapes;
   the piano roll tools are icon buttons with a tooltip "Name (letter / number)" and the
   long hint moved to the status bar.
2. ~~**Design pass**~~ **done (round 5)**: see "The design system" above — bundled IBM
   Plex Sans/Mono, the 4-px grid, `Elevation`, a global focus ring, distinct hover/active
   states, and three more painted icons (Loop, Metronome, Keyboard). ADR 0009 records the
   typography and licensing decision.
3. ~~**Layout personalization**~~ **done (round 6)** for panels: named layouts, saved panel
   sizes, panel visibility and *Reset layout* — see "Named layouts" above. Still open from
   this step: per-view zoom presets and configurable clip colours (track colours are
   already editable in the inspector).
4. ~~**Keyboard map file**~~ **done (round 7)** — see "The keymap" above. The editor is a
   self-contained window today; the settings dialog will host it rather than redefine it.
5. ~~**Settings dialog**~~ **done (round 8)** — see "Settings" above. It hosts the keymap
   editor rather than redefining it.
6. **Piano roll polish** — scale highlighting + snap-to-scale, ghost notes, chord stamps,
   note properties popup, glue/legato, strum/arpeggiate, magic lasso.

## Known gaps / next steps (in order)

1. **Buses and sends**: route several tracks to one strip; the same tree as track groups.
   See *The next session*.
2. **What hangs off the time selection**: insert/delete space, ripple, quantize clip
   starts, select clips in the selection, render the selection.
3. **Swing as a clip property** (schema 9).
4. The rest of the playlist plan, in the order it gives: clip chrome and the header map,
   markers and time signatures, the remaining structural editing, navigation (a mini-map
   scrollbar the channel rack should share), shared clip sources and the picker panel,
   audio clips, automation clips, multiple arrangements.
4. Piano roll polish, the last of the design plan: scale highlighting and snap-to-scale,
   ghost notes from other clips, chord stamps, a note properties popup, glue/legato, strum,
   arpeggiate.
5. **Automation lanes** (parameter curves, sample-accurate) and a **tempo track**.
6. **Sends/buses**: the mixer has strips but no routing yet; add bus tracks + sends.
7. More native devices: EQ, compressor, chorus; a sampler instrument.
8. **CLAP host** (`cantus-plugin-host`, GPL) with out-of-process sandboxing (Phase 3).
9. **AI providers** (Anthropic/OpenAI/Ollama over `reqwest`), disclosure dialog, MCP server.
10. Recording polish: count-in, punch in/out, overdub vs replace, sub-block MIDI timestamps.
11. Accessibility pass (semantic nodes for the painted views).
12. `docs/adr/0001`: trademark search for the name before any public release.
13. Velocity lane at the widest zoom: per-note bars overlap into a solid block. Showing the
    peak per column would say more.

## Conventions

- Every mutation is a `Command`; never touch `Project` fields directly from the UI.
- Audio-thread code lives only in `cantus-engine::processor` and `cantus-dsp`; both are
  `#![forbid(unsafe_code)]` and must not allocate after construction.
- Add a null test before adding a DSP module; effects must be block-size independent.
- New UI strings go in `crates/cantus-ui/locales/en.txt` **and** `es.txt`; a unit test
  fails otherwise. The bundled fonts lack many symbol glyphs: paint icons or use words.
- Spacing comes from `design::space`, type from the named text styles, surfaces from
  `design::Elevation` — never a bare `12.0` or `.size(15.0)` at a call site. Numbers are
  monospaced.
- egui **panics** on a text style it cannot resolve, so any `egui::Context` that renders a
  view must have had `design::install_style` called on it (the test harness has a `ctx()`
  helper that does what `CantusApp::new` does).
- A new colour token goes in `Palette`, in `Theme` with `#[serde(default)]`, **and** in
  `Theme::fill_missing`, so older `theme.json` files keep loading and get the new token
  written back for the user to edit.
- No view reads keys. A view that wants a shortcut adds a `keymap::Action` and a
  `run_action` arm; a view that wants to *show* one asks `Keymap::display_for`. Adding an
  action also means an `action.<id>` string in both locales.
- A **widget** that moves with the arrow keys while focused calls
  `widgets::claim_arrow_keys`, or the same press also nudges the note selection. Widget
  keys are not shortcuts and do not belong in `keymap.toml`.
- **A drag hit-tests the press origin, not the pointer.** egui reports a drag only after
  the pointer has travelled, so by then it has left whatever narrow zone it was aimed at.
- **A menu item runs a `keymap::Action`**, never the method behind it: the same entry means
  different things in the arrangement and the piano roll, and ADR 0010 says a menu and a
  shortcut must reach the same code. `views::arrangement::action_item` also shows the
  binding, which is how a shortcut gets discovered.
- **A test of a clip edit asserts on `Snapshot::compile`** — what the engine would play —
  and not on the clip's fields. A clip is a window: its notes do not move when it does, so
  a note list that looks unchanged proves nothing.
- **A control at its default must not touch a sample.** Skip the maths, do not rely on it
  being algebraically the identity — `(l+r)/2 + (l−r)/2` is not bit-identical to `l`.
- Anything that changes **what a project renders as** belongs in the project file, not in
  `settings.json` (ADR 0012). Preferences follow the machine; a mix must not.
- The **arrangement** and the **piano roll** have different mouse modifiers on purpose, and
  the shortcuts dialog lists both. `EditFocus` — the half of the window last edited in —
  decides whether a shared shortcut acts on clips or on notes.
- **Anything drawn once per note, clip, step, sample or automation point goes into a
  `Mesh` with a level-of-detail rule** (ADR 0011). The convenience painters cost ~10x per
  item. This is a rule at the first line, not an optimisation after it is slow — the
  channel rack's pad × step grid is the next thing it applies to.
- **A view that is tested by pointer gesture publishes its geometry**
  (`arrangement::TimelineGeometry`) and the test asks it where things are. A test that
  mirrors the layout breaks the day the layout changes, which is the day it is least
  useful.
- Anything a test writes to disk must go through a `tempfile::tempdir()`. `CantusApp`'s
  `layouts_path`, `keymap_path` and `settings_path` are public so tests can redirect them;
  never let a test write to the real `%LOCALAPPDATA%/Cantus`.
- The locale is process-wide and tests run in parallel: a test must not leave it changed,
  or it races every test that asserts an English string.
- Bump `SCHEMA_VERSION`, append a migration and regenerate a fixture
  (`cargo run -p cantus-project --example write_fixture`) whenever `Project` changes shape.
- **An editor's vertical axis comes from the instrument** (`InstrumentSpec::lanes`, ADR
  0013), through `views::piano_roll::Axis`, which also gives a row to any pitch the clip
  holds that the instrument has no lane for. A view must never assume a keyboard, and
  every vertical move — drag, transpose — goes through `Axis::moved`, in rows.
- **A step is a note.** The channel rack issues `InsertNotes` / `RemoveNotes` and holds no
  data of its own. A second editor for a clip must not be a second data model.
- **A pad's mute, like a track's, is resolved in `Snapshot::compile`.** The audio thread
  is handed events with the pad already gone; it never consults a kit per note.
- **A snapshot that will be *played* is made by `EngineHandle::compile`**, never by
  `Snapshot::compile` alone: the engine attaches the sample files. Tests that only look at
  events may compile directly.
- **The sample cache never evicts.** That is what makes every `Arc` the audio thread
  holds safe to drop there. Do not add an eviction without adding a garbage path.
- **Noise is seeded per pad**, never from a global counter: editing one pad must not
  change another's render. The `kick under hats` residue test is the guard.
- **A parameter set changes exactly one field.** The command's inverse restores that
  field; anything else it touched is not undone. The fuzz catches this, but only after.
- **The channel rack is a view over every track.** A "channel" is a `Track`; a new
  per-channel feature (routing, sends) goes on `Track` and the rack shows it.
- **One instrument per track, both engines per slot.** `TrackSlot` holds a `Firmus` and a
  `Pulsus` preallocated and switches with a flag; a new instrument adds a field, never a
  `Box<dyn>` (ADR 0005). An instrument that pans inside itself renders stereo; everything
  from the polarity switch on is one signal path.
- A parameter of *any* instrument is addressed by `InstrumentParam`, never by a
  per-instrument command. Adding an instrument means a variant there, a `set_param` arm,
  and a fixture that uses it.
- **Text on a light key is `theme::on_light_key`**, never `ink` (the canvas colour, white
  on Paper) and never `text` (light on Ink & Gold). A test that asserts text was painted
  does not assert it was visible.
- **Rows come from `rows_of`.** Track heights vary; anything that needs to know where row
  *n* is asks `TimelineGeometry::rows`, never multiplies by a height.
- **A row's interactive rect owns the grip below it**, shifted down by the grip's width,
  or a press on the divider goes to the row underneath and means something else.
- `--rack` and `--track N` open the channel rack and select a track at startup, next to
  `--mixer`; synthetic input to the running app is unreliable here, and these exist so a
  view of a particular track can be looked at without it.
- Screenshots on this machine: PrintWindow from a DPI-aware process, enumerating the
  process's windows and taking the largest visible one — `MainWindowHandle` points at an
  18x18 helper window and gives a blank capture. Pick the newest `cantus` process, and
  never kill instances you did not start: if one holds the exe and blocks a build, **rename
  the exe** (Windows allows renaming a running binary) rather than killing it.
