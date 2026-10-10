# ADR 0035 — What an export writes: the range, the latency, the hosted plug-ins, a modal dialog

**Status:** Accepted · **Date:** 2026-10-09 · **Round:** 23 (beta hardening) · **Amends:** ADR 0033 (export), ADR 0021 §3 (stems) · **Keeps:** ADR 0021 §1 (offline = real time)

## Context

The owner found that "several options of the export dialog do not work" (BACKLOG §1, a
beta blocker). Round 23 audited every control from the click to the bytes in the file
(two independent auditors, every finding checked by two skeptics; 27 findings, none
refuted; `docs/round23/beta-hardening.md` lists them). Most were bugs of the dialog —
every list (tail, quality, rate, loudness target, ceiling) opened *under* the dialog and
could not be clicked — but four were about what a file holds, and they needed decisions:

1. **A selection's tail played the music after the selection.** `end` only counted
   frames; the transport kept playing through the tail.
2. **A latent project made the file late and cut its end.** Delay compensation makes the
   master late by the project's total latency L (a limiter's look-ahead). The stems
   trimmed it (ADR 0021 §3); the mix did not, so the mix and its `00 Master` stem
   differed, and with no tail the mix lost its last L samples.
3. **Stems ignored hosted plug-ins**: every hosted effect was a wire in the stems and
   every hosted instrument track silent, where the mix used fresh instances (ADR 0019,
   ADR 0028).
4. **The dialog was not modal for the keyboard**: Esc also cleared the time selection,
   Delete and Ctrl+Z acted on the project behind it, Ctrl+E threw away the choices.

## Decisions

### 1. A range stops at its end; its tail only rings out

When a render has an `end` (a selection, the loop brace), the transport stops exactly at
the range's last frame — the block is split there, which moves no sample because the
processor is block-size independent; the frame is computed with the transport's own
arithmetic (`transport::sample_at`), so a note starting exactly on the end never fires
first, at any tempo and rate — and the tail renders with the transport stopped:
`EngineMessage::Stop` releases every note (`release_all`), so releases, delays and
reverbs ring and nothing new starts. This is what playing the range and stopping at its
end gives, so offline still equals real time. The song (no `end`) is unchanged: it plays
to the end of its last clip and every note to its own end. Stems take the same rule.

### 2. `RenderOptions::compensate_latency`: an export starts at the first tick

A new option, **off by default**: with it, the render runs L frames longer and drops the
first L, so sample 0 is the range's first tick and the file holds the whole range and
tail — as the stems already were. Off, `render` is unchanged: what playing gives, sample
for sample, latency included — what the engine's tests compare (a latent effect is
checked as "the dry render delayed by its latency", and those tests are right about the
processor). The export turns it on (`export_mix`), so a mix equals its `00 Master` stem
to the bit. No golden hash moves (the v5 fixture has no latent effect).

### 3. Stems render through the hosted instances too

`render_stems_to_folder_with` and `render_stems_hosted` take the hosted effects' and
instruments' instances as `render_with_progress` does: an effect with an instance gets it
(matched by id), every other the engine's unit; hosted instruments join the native ones.
The interface loads them for stems as for a mix and frees them when the export ends.
One instance cannot be in two places: the copies of the master chain a stem goes
*through* (`through_master`) build their own units, so a **hosted insert on the master is
a wire there** — documented, not hidden.

### 4. The bar follows the render; a stop stops it

`export_mix` uses `render_with_progress` with `Control::render`: the render's frames are
the bar's first half (`MIX_RENDER_SHARE`), the pulse is gone, and `Control::cancel` sets
the render's own flag, so Stop ends the render at its next block. The status line shows
the phase and the percentage while the dialog is closed.

### 5. Stems are committed together

Every stem is encoded to `name.ext.part` first and all are put in place only when the
last is whole — all or none: what each would replace is moved aside first and put back
if any rename fails (a player holding the last export's stem open), so a stopped or
failed export leaves the folder as it found it — an earlier export's stems included (it
used to delete them). A mix whose extension was put right asks before replacing a file
the save dialog never asked about. Before a stems export replaces files of
the same names, the interface asks (the system's message box, as the save dialog is).

### 6. The dialog owns the keyboard while it is open

`global_shortcuts` returns while the export dialog is open. Esc closes an open list
first, then the dialog; Enter exports (unless a tag is being typed or a list is open);
Ctrl+E no longer reopens over the choices being made.

### 7. Smaller rules the audit fixed

- A **range the project lacks** (no time selection, no loop brace) is not chosen: its
  segment stays unlit and a note says what to do, until the dialog closes. A new
  **Loop** range exports the loop brace (FL's pattern loop, Ableton's and Logic's cycle).
- A **name typed with another format's extension** gets the chosen format's (or one is
  added): the contents and the name agree.
- **FLAC is written at 96 kHz at most** (flacenc's limit): 192 kHz → 96, 176.4 → 88.2, the
  dialog's "written at" note says so; the rate list offers only what the format carries.
- The **summary** says when the loudness target was missed and why (limiter off, the
  +30 dB cap, nothing to measure), how many samples clipped, and a stems export always
  reads as stems. Every error is in the interface's language.
- **Presets** compare only the choices the dialog shows (loudness only when normalising;
  the stems preset includes strips-not-tracks and not-through-the-master).
- *Export audio…* after *Export stems…* opens on the mix; the MP3 modes keep their own
  last choice; the selection note names its last bar (the end is exclusive).
- The **settings file** never holds the Loop range (it is kept as the song: builds out
  before round 23 cannot read it), and `Settings::load` reads a file one field at a time,
  so a field this build cannot read keeps its default instead of resetting them all.
- Enter that confirms a tag does not export; menus left open behind the dialog close
  when it opens; progress does not cover a recent error on the status line; a note held
  on the typing piano is released when the dialog opens.
- The old stems dialog in `views::interop` (unreachable since round 22) is gone.

## Consequences

- An export of a project with a look-ahead limiter on the master starts on time and ends
  complete; mixes and `00 Master` stems null.
- Exporting a selection or the loop gives the part and its ring-out, not the next bar.
- `render`'s default stays the real-time truth; a caller that wants a file aligned to the
  timeline asks for it.

## What the tests hold

- `cantus-engine/tests/render_range.rs`: a selection's tail equals the render of the
  project without the notes after it, to the bit, at three block sizes; stems too; an idle
  limiter (strip or master) leaves a compensated render identical to the dry one, and the
  raw render is still late; the mix equals the master stem.
- `cantus-host/tests/stems_hosted.rs`: with a hosted effect the master stem is the mix to
  the bit (and differs without the instance); a hosted instrument's track stem plays.
- `cantus-export/tests/options.rs`: every format × rate × channels, every depth, every
  MP3 bitrate (each frame header), every VBR level, every Ogg quality, every dither choice
  and every preset, against the file read back by an independent decoder.
- `cantus-export/tests/jobs.rs`: the bar rises through the render and a stop during it
  stops it; a stopped stems re-export keeps the earlier stems.
- `cantus-ui/tests/export_options.rs`: every control clicked through egui's input — every
  list included — lands in the settings; Esc, Enter, Ctrl+E and the shortcuts behind.
- `cantus-ui/tests/export_files.rs`: song, selection, loop, tail, rate, channels, depth,
  loudness, extension, stems summary, progress and errors, through the application.
