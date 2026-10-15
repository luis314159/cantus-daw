# ADR 0052 — The top bar: FL's strip as groups that fold, pattern mode, the metronome's voice and the master's loudness

**Status:** Accepted · **Date:** 2026-10-14 · **Round:** 28 (builder *topbar*) ·
**Extends:** ADR 0010 (keymap: file version 8), ADR 0024 (the kit; §9 the hint panel,
§13 the frame budget), ADR 0025 (recording: the click's settings), ADR 0005 (real time) ·
**Schema:** 26 · **Commands:** `SetPatternMode`

> Numbering: written as 0052 on `r28/topbar`. The builder's account:
> `docs/round28/topbar.md`. BACKLOG §6e.

## Context

The owner's reference is FL Studio's top strip: dense, one group per function. Ours was a
row of hand-placed rects (`views/transport.rs`, 1 176 lines) whose groups were *dropped*
when the window was narrow — and before that, overlapped. The engine's numbers, the MIDI
count and the audio chip sat in the status bar; there was no tap tempo, no editable time
signature, one click sound, no pattern mode, no RAM reading and no loudness on the master.

## Decisions

### 1. The bar is a list of groups laid out by a pure function, in FL's two rows

`views/transport_layout.rs`: thirteen `Group`s (PAT/SONG, transport, tempo, metronome,
position, snap, undo, windows, inputs, performance, meter, device, tools) and the left
block (logo, menus over the project's name and the hint panel). `layout(room, wants)` is a
pure function of the bar's width and each group's least/most width, in fixed arrays (no
allocation):

- **two rows, as FL's strip.** A group with two lines of its own (PAT/SONG, the
  metronome, the clock, CPU/RAM, the meter) is *tall*: a column of the bar's height. The
  others are one row high (22-pt keys on FL's button strips: one cap a strip, which keeps
  the whole window under its triangle budget) and stack two to a column in the owner's order —
  by default the transport over the tempo, the snap over undo, the windows over the
  inputs, the audio chip over the tools. At **1920 px, 100 %, every group is on the
  bar, in English and in Spanish** (`the_default_bar_shows_every_group_at_1920`); the
  audio chip is capped at 200 pt there (a long device name is shortened in the chip, its
  whole text in the hint);
- groups keep the owner's order; a hidden one is left out;
- when they do not fit, the **least important** (a fixed rank: transport 0, position 1,
  tempo 2, mode 3, metronome 4, device 5, meter 6, windows 7, performance 8, snap 9,
  undo 10, inputs 11, tools 12) **folds whole** into the **⋯ menu**; then every folded
  group that fits again, most important first, comes back (a wide fold can free room for
  a narrow group). Nothing is ever shrunk, clipped or overlapped;
- **the menus never fold.** They sit above the hint panel, in its width, and the panel
  keeps at least 300 pt — the final review's rule that every hint reads whole at
  1280 × 720 at 100 % and 200 % (`final_look.rs`). A compact ≡ key would save no width
  while the panel needs its 300 pt, so there is none (review 1 found the first version's
  one could never be reached);
- room to spare goes to the left block (up to 460 pt), then to the meter (up to 280 pt).

Each group's width is **measured** in the interface's language at the context's zoom,
cached per (language, zoom, text scale, groups, chip width, metre — the clock is as wide
as the widest position its metre can show: two beat digits from 10 beats, four tick digits
for a half-note beat): the bar at rest measures nothing; the owner's order is resolved into
a fixed array each frame, so a custom order allocates nothing more than the default.

**The overflow** keeps its open state on the bar (`TopBar`), not in egui's single popup
slot, and is drawn as an area under the popovers it opens: the folded groups' own menus and
popovers (the grid list, the metronome's options, the metre's editor, the zoom steps, the
count-in) open from it and work there (review 1 found them closing it). Esc (when none of
them is open), a click on the window outside it, or the ⋯ key close it; it scrolls inside
the window and hangs under the bar.

**Shown, hidden and order** are the owner's: `Settings::topbar` (`order`, `hidden`,
`meter_lufs`), edited in *Customise top bar…* (⋯, *View*, right-click on the bar): tick,
drag by the grip (the drop line shows where the group lands), *Reset*. Unknown ids are
ignored; a group newer than the file goes in after its default predecessor.

**Draw calls.** Each group painted its wells, then its words, then the next group's wells:
every switch between the kit's atlas and the font's is a draw call. After drawing, the
bar's shapes are reordered (shapes, then words — the groups never overlap, so no pixel
changes); the project name's field is drawn after that, so the panel's words never show
under it (and the panel leaves the name out while it is renamed).

**Proved by** `tests/topbar_layout.rs`: a sweep of 800–3 840 px × zooms 0.75–2.0 × six
group sets (shuffled, some hidden) × EN/ES, asserting that no two parts overlap in the
bar's two rows, none leaves the bar, none is narrower than it needs, every tall group has
the bar's height, every shown group is on the bar or in ⋯, in the owner's order, and the
transport stays at 800 px / 100 %; real frames at 800/1280/1920/2560 × 75/100/200 % ×
EN/ES with every text inside the bar; each group drawn alone in its own width and row with
every word inside it and none shortened with "…"; the clock whole in 4/4, 3/2, 12/8, 32/2
and 7/16 at bar 123; every group shown at 1920 px; the allocations of an owner's order.

### 2. Pattern mode: a project the engine plays, not a mode of the engine

`Project::pattern_mode` (schema 26; not written while false), switched by
`Command::SetPatternMode` (one undo step, as the metronome's switch) and `L` (FL's key).

The **current pattern** is Cantus's pattern region (the channel rack's, round 17): the
window of the clip being edited — its start, one period if it repeats — and on each track
the note clip the rack shows in it. In the common case that is the clip alone; when the
rack shows a bass under the drums, pattern mode plays both, as FL plays the whole pattern
whichever channel the piano roll shows. `cantus_core::pattern::audition` builds the
project the engine is handed: every other clip removed (audio, takes and automation clips
are the song's), the loop set to the window and on. The engine is unchanged: it plays a
snapshot compiled from that project, so pattern mode is **sample-accurate by
construction** — the engine test renders three loops of a pattern and compares them, to
the bit, with the pattern laid end to end as a song.

The application (`views/transport_pattern.rs`) follows the mode — by key, click or undo —
moving the playhead into the pattern and back to where the song was, recompiling when the
edited clip changes. **Recording is refused in pattern mode** with the way out on the
status line (a take belongs to the song; loop recording into a moving window would place
takes nobody could find). **Export renders the song**, always: pattern mode is a way of
listening. The clock counts from the pattern's start; its metre is lit.

Review 1 closed what that left open: the playhead **stays in the pattern** (Home, a second
Stop, a click on the ruler outside it go to its top — the engine wraps only from inside the
loop, so outside it played silence); the pattern is the **last note clip chosen** —
selecting a track, an audio clip or nothing keeps it, as FL keeps its pattern; pattern
mode **cannot be switched on mid-take** (refused with a hint; an undo or redo that brings it
back ends the take first); and the state **starts afresh with another project** (it is
keyed on the app's project epoch).

### 3. The metronome's voice and accent (schema 26)

`RecordSettings::click_sound` (`Classic`, `Wood`, `Beep`) and `click_accent`, both
skipped when at their old values so an older-equivalent project saves byte-for-byte as
before; the migration writes nothing. `cantus_dsp::click`: Classic is the old sine burst —
a test holds it **to the bit** against a copy of the old code; Wood is two damped modes
(≈1 : 1.6) whose upper dies first; Beep a soft-edged square with a 1 ms rise and a flat
top. Without the accent the downbeat is an ordinary beat (tested per voice, in the engine).
The metronome group shows the switch, the count-in and the level; its ▾ holds the voice,
the accent, when it plays, the level and the count-in. The *Click* rows left the record
options (no duplicate).

### 4. Tap tempo

`views/transport_tap.rs`: the tempo is 60 over the mean interval of the last eight taps;
a pause longer than twice the run's interval (at most 3 s) ends the run; rounded to 0.1,
kept in 20–999 BPM. A run merges its tempo changes into **one undo step**
(`MergeKey "tap-tempo"`, the gesture closed when the run ends). Key: `Shift+T` (FL has
none; Shift keeps it off the tools' `T`).

### 5. The time signature, editable

A well on the bar opens an editor: the beats with ‹ › (1–32), the note value (2, 4, 8,
16), the usual metres in one click — `Command::SetTimeSignature`, which existed unused.

### 6. CPU, RAM, voices, dropouts — moved from the status bar

RAM is the process's working set, read through a safe function in **`cantus-rt`**
(`usage::working_set_bytes`: Windows `K32GetProcessMemoryInfo`, the only `unsafe`, with
its SAFETY line; Linux `VmRSS` from `/proc/self/status`, no `unsafe`; elsewhere `None`, a
dash) about once a second on the interface's thread, never the audio thread. Numbers are
capped in width ("999 MB" → "1.0 GB", "999+"). A new dropout is said once on the status
line with its blame (round 20's text); the count and the tooltip stay in the group. The
engine readout, the MIDI count and the audio chip **left** the status bar.

### 7. The master's loudness, fed from the engine

`Meters::master_loudness` (`momentary`, `short_term`, `integrated` of the pass, the
callback's `true_peak`), from a `LoudnessMeter` (`cantus_dsp::terminus::loudness`: BS.1770
K-weighting, 10 ms columns, 4× true peak) fed with the mix as it leaves the master strip,
before the click. It only reads a scratch copy of `out`: the goldens (`null_tests.rs`
`GOLDEN_V5`) are unchanged. Review 1 tightened it:

- a **mono** output is weighed as one channel (`process_mono`), not counted twice;
- a sample that is **not finite** is metered as silence, so a NaN never poisons the filters
  (the readings come back with the music);
- the **integrated** reading is worked out only when a 100 ms gating block was added while
  playing (not every callback: two scans of 8 000 bins), starts again on Play and holds
  after Stop — it is the pass's;
- the **true peak** the interface reads is the highest of every callback drained since the
  last frame (`EngineHandle::poll`, as the compressor's peak), and the meter holds the
  highest since Play until the readout is clicked;
- the histograms are **pre-faulted** with the processor (ADR 0031 R11).

Built with the processor; no allocation (`rt_memory.rs`). **Cost measured: 2.5 µs a
32-frame callback, 5.1 µs at 64, 20.4 µs at 256 — 80 ns per stereo frame at every size,
0.38 % of one core at 48 kHz** (`loud_cost`, release). A 1 kHz sine at −20 dBFS on both
channels reads −20.0 LUFS within 0.1 LU on all three windows (−23.0 on a mono output). The
meter group's PEAK/LUFS switch is kept in the settings; LUFS mode marks −14 LUFS.

### 8. The project's name and the window title

The name in the hint panel is renamed in place with the kit's field
(`Command::SetProjectName`, which existed unused). The window title is "Name • — Cantus":
set when the splash becomes the main window (`boot::become_main_window`, from
`title_of(&app)` — the boot path, tested on its source) and kept current by
`title::sync` in `CantusApp::frame`.

### 9. Inputs, device, tools

The typing keyboard; a **MIDI lamp** lit 150 ms after any event (`MidiSender::received`, a
relaxed atomic counter beside the existing mutex — the audio thread never touches it) with
the inputs open; **Armed** (how many tracks will record; a click arms the selected one).
The audio chip; when it is folded and the audio needs a look, the ⋯ key's dot takes its
colour. The tools: the element picker, the appearance page, the interface's zoom as a
menu of its steps.

### 10. Keys (keymap file version 8)

`TogglePatternMode` = `L` (bare: the typing piano takes it while it plays; the bar's
switch is always there), `TapTempo` = `Shift+T`; older files get them where free.

## Consequences

- New strings live in `locales/areas/topbar.{en,es}.txt`.
- The bar's look is judged in `examples/shoot_topbar.rs` (widths 800–2560, zooms 75–200 %,
  EN/ES, Night/Paper, the overflow open, the metronome options, the metre editor, the
  customise panel, LUFS and pattern mode).
- At 1920 px and 100 % the default bar shows every group in both languages. Below that,
  groups fold by rank (at 1280 px: the inputs, CPU/RAM, the meter, the snap, undo and the
  tools) and work from ⋯; the owner can reorder or hide groups to choose what stays. A
  real device's name can make the chip wider than the no-device text — it is capped at
  200 pt, so it never pushes another group off.
- A future group needs: a `Group` variant with id, name, rank and height (tall or a row);
  its width and drawing; a string pair. The property test covers it automatically.
