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

### 1. The bar is a list of groups laid out by a pure function

`views/transport_layout.rs`: thirteen `Group`s (PAT/SONG, transport, tempo, metronome,
position, snap, undo, windows, performance, meter, inputs, device, tools) and the left
block (logo, menus over the project's name and the hint panel). `layout(room, wants)` is a
pure function of the bar's width and each group's least/most width, in fixed arrays (no
allocation):

- groups keep the owner's order; a hidden one is left out;
- when they do not fit, the **least important** (a fixed rank: transport 0, position 1,
  tempo 2, mode 3, metronome 4, device 5, meter 6, performance 7, snap 8, undo 9, windows
  10, inputs 11, tools 12) **folds whole** into the **⋯ menu** at the right end, where the
  same drawing code shows it at the same size, usable; nothing is ever shrunk, clipped or
  overlapped;
- the menus fold into one key (≡, each menu a submenu) only when the groups left matter
  more than they do (`MENUS_IMPORTANCE` = 3);
- room to spare goes to the left block (up to 460 pt), then to the meter (up to 280 pt).

The hint panel keeps at least 300 pt: the final review's rule that every hint reads whole
at 1280 × 720, at 100 % and 200 %, holds (`final_look.rs`). Each group's width is
**measured** in the interface's language at the context's zoom, cached per (language,
zoom, text scale, groups, chip width): the bar at rest measures nothing; a warm frame of
the bar allocates **1** time (was 12).

**Shown, hidden and order** are the owner's: `Settings::topbar` (`order`, `hidden`,
`meter_lufs`), edited in *Customise top bar…* (⋯, *View*, right-click on the bar): tick,
drag by the grip, *Reset*. Unknown ids are ignored; a group newer than the file goes in
after its default predecessor.

**Draw calls.** Each group painted its wells, then its words, then the next group's wells:
every switch between the kit's atlas and the font's is a draw call. After drawing, the
bar's shapes are reordered (shapes, then words — the groups never overlap, so no pixel
changes): the whole-window budget scenario holds at 87 draw calls (ceiling 96).

**Proved by** `tests/topbar_layout.rs`: a sweep of 800–3 840 px × zooms 0.75–2.0 × six
group sets (shuffled, some hidden) × EN/ES — 11 172 layouts — asserting no two parts
overlap, none leaves the bar, none is narrower than it needs, every shown group is on the
bar or in ⋯, in the owner's order, and the transport stays at 800 px / 100 %; real frames
at 800/1280/1920/2560 × 75/100/200 % × EN/ES with every text inside the bar; and each
group drawn alone in its own width with every word inside it and none shortened with "…".

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

`Meters::master_loudness` (`momentary`, `short_term`, `integrated` since play started, the
block's `true_peak`), from a `LoudnessMeter` (`cantus_dsp::terminus::loudness`: BS.1770
K-weighting, 10 ms columns, 4× true peak) fed with the mix as it leaves the master strip,
before the click. It only reads `out`: the goldens (`null_tests.rs` `GOLDEN_V5`) are
unchanged. Built with the processor; no allocation (`rt_memory.rs`). **Cost measured:
121 ns per stereo frame, 0.58 % of one core at 48 kHz** (`loud_cost`, release). A 1 kHz
sine at −20 dBFS on both channels reads −20.0 LUFS within 0.1 LU on all three windows.
The meter group's PEAK/LUFS switch is kept in the settings; LUFS mode marks −14 LUFS.

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
- At 1920 px some groups fold by default (in Spanish, the windows keys too); everything is
  one click away in ⋯ and the owner can reorder or hide groups to choose what stays.
- A future group needs: a `Group` variant with id, name, rank; its width and drawing; a
  string pair. The property test covers it automatically.
