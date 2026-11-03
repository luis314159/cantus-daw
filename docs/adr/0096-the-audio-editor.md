# ADR 0096 — The audio editor: destructive edits as new content-addressed files, clean joins, a spectral view, noise and click repair

**Status:** accepted (round 36, editor builder; revised after the integrator's review) ·
**Date:** 2026-11-03 · **Extends:** ADR 0004 (commands), ADR 0025 (audio clips and the pool),
ADR 0031 (offline = real time), ADR 0054 (the pitch editor is the non-destructive one), ADR
0081 (agents) · **Serves:** BACKLOG §3 *Editing* (an Edison-like audio editor) · **Schema 50
in the branch, numbered 52 when round 36 is merged** (§6) · one new `Command`,
`SetAudioSourceMarks`.

> Numbers and the tests: `docs/round36/editor.md`. The manual: `docs/manual/en/audio-editor.md`.

## Context

The pitch editor (ADR 0054) edits a take *without* touching it. A recording also needs the
other kind of editing — Edison's — where the result **is** the audio and everything
downstream (the mixer, a bounce, a freeze, another clip over the same file) just plays it:
cut a cough, silence a pop, fade the ends, normalise, reverse, take the hiss and the clicks
out, remove the dead air. The owner's rule: *"the audio editor cleans a take and returns it to
the clip."*

## Decision

### 1. Opened from a clip, back to the clip; files in and out

*Edit in audio editor…* (the audio clip's context menu) opens a large window over the
workspace (`cantus_ui::editor`, the kit, both themes) on **the clip's current file**, whole, at
the file's own rate and channels. Closing leaves the clip playing what it was left as. The
window follows the project: an undo, a redo or an agent that points the clip at another file
makes it read that file; **another project** (New, Open, a template, a recovery, an undoable
replacement) closes it — `reset_bus_to` closes it at once, and the window also closes when
`CantusApp::epoch` moves. A take over 30 minutes is not opened (32-bit floats in memory).

Beside the clip's take: **Open file…** imports a file onto a new track at the playhead and
opens it here; **Paste from file…** reads a file and pastes it at the cursor, conformed; **To
new clip** (or an Alt-drag of the selection out of the window) writes the selection as a file of
its own and puts it on a new track exactly where the selection plays now, named "‹clip› (part)",
in the import's one undo step. (Dropping onto a chosen track and time would need the
arrangement's geometry under a window that covers it; a new track where the audio already plays
is where it is wanted most often, and moving a clip is one drag afterwards.)

**Pasting across formats.** A clipboard or a file at another rate is converted with the engine's
windowed-sinc resampler (the one every file the engine plays goes through) and to the take's
channels (mono doubled, stereo averaged to mono).

### 2. A destructive edit is a new file and one undo step

An edit never writes into the file it started from. It is made off the interface thread and
written as a **32-bit float WAV named by the SHA-256 of its bytes** in **the project's own edit
folder**, `Audio/<project file's name>/edit-<16 hex>.wav` (one folder per project, as freeze
keeps `Freeze/<name>`: several projects in one folder must never see each other's edits; files an
earlier build wrote in `Audio/` stay where their paths say and are never cleaned), or the
session's scratch folder until the first save, which collects it. The project gets one
`Command::Batch`: `InsertAudioSource` (the new file, `sha256` set), `SetAudioSourceMarks` (the
file's marks moved with the audio, §6), `SetClipAudio` (the clip pointed at it) and, when the
clip showed the whole take and the length changed, `SetClipBounds`. Its undo label is the
edit's own (`cmd.editor.<edit>`), carried with the job and its result — never read from what the
window did meanwhile.

- **One undo step, exact**; the original file stays in the pool and on disk.
- **Content addressing**: the same edit of the same take is one file and one pool entry.
  Written beside then renamed; a half-written `edit-*.wav.part` from a cut session is removed
  when the window opens (no edit is being written then).
- **The folder at commit**: the file goes to the project's edit folder *as it is when the edit
  is committed* — a Save As while the edit was being made moves it there.
- **What the clip keeps.** Gain, fades, crossfades, the gain envelope, reverse and
  `slip_frames` are the region's and stay. The pitch editor's notes and a warp's markers are set
  on source frames: **kept by edits that leave every frame where it was** — gain, normalize,
  invert, DC, fades, silence, noise reduction, spectral attenuation, declick, repair
  (`Edit::keeps_timing`) — and **dropped by a reversal, a paste and every edit that changes the
  length**; the status says when they were dropped. A linked clip shares the edit through
  `SetClipAudio`; other clips over the same file keep the old one.
- **ARA and the recording mode.** A clip an ARA plug-in edits (ADR 0097) plays the plug-in's
  rendering: the window does not open on it ("unbind the ARA plug-in first"), an agent is refused,
  and a new source's region never carries a binding. In the recording mode (ADR 0092) an edit is
  asked of the mode's lock before any work starts and refused in the mode's own words.
- **Busy means held.** While the thread works, an edit, a cut, a paste, *Learn noise* or *To
  new clip* is refused with "Wait: the editor is still working on the last edit" — the
  clipboard is never touched by a cut that cannot be made.
- **No orphans.** A result that comes back for a closed window or another take, a commit the
  project refuses (the clip changed), and an agent's edit that is never made (a declined card,
  another project) remove the file they wrote — when they wrote it and nothing refers to it.

### 3. The operations (`cantus_core::audio_edit`, pure functions)

`apply_with(buf, selection, edit, joins) -> Edited { buf, sel, used, changed, delta_frames,
note }`; no I/O, no randomness. Selections are in frames (the window shows frame numbers; at
32 points per frame the samples are drawn). Cut/copy/paste/delete, trim, silence, insert
silence, fades on the clip fades' five curves, gain, normalize (peak, RMS, LUFS), invert,
reverse, DC remove, remove silence, noise reduction (§4), declick and repair (§5a), spectral
attenuation (§5). No selection means the whole take for the edits that make sense on it;
delete, trim, silence, the region and repair need one and say so. **An edit that would leave
nothing is refused** (`WouldBeEmpty`, checked on every result — *Remove silence* with nothing
kept on a silent take included); an edit that finds nothing to do writes no file and says so.

**Normalize.** To a peak, an RMS or an integrated loudness (BS.1770 gated) by one gain. RMS and
LUFS can push peaks past 0 dBFS: with **Peak ceiling** on (the default, −1 dBFS) the gain stops
where the peak reaches it and the status says the level reached; off, the result says where the
peaks went ("will clip on export"). Loudness of less than 400 ms is "too short to measure".

**§3a — Joins without clicks.** Wherever an edit puts two pieces of audio side by side — a cut,
a trim's new ends, a paste's two seams, inserted or written silence, a reversal's edges, the
cuts of *Remove silence* — the edges first move to the **nearest zero crossing** of the mono mix
(within 5 ms; gain-like edits snap their edges too), then the two sides are **crossfaded over 3
ms on an equal-power curve**. Equal power because the two sides of a cut are different moments
of a take — uncorrelated — where a linear crossfade dips 3 dB. Three milliseconds: longer than a
cycle of everything above 333 Hz, where a click's energy is, and shorter than a consonant's
attack (5–10 ms), so it smooths the seam without smearing a transient beside it; 2 ms leaves a
cut in a low note (under ~150 Hz, less than a third of a cycle) close to a step, and 5 ms starts
to soften a cut placed on a drum hit. Each side fades with its own material — the left with what
followed the cut, the right with what led into it — never with invented samples; the edge of the
file or silence fades from or to zero. The **Smooth joins** switch turns both off for raw,
sample-exact splices (the identity tests use it). A 220 Hz cut mid-waveform: raw step over 0.1,
smooth under 0.03 (the tone's own largest step is 0.0144).

### 4. Noise reduction by profile (`cantus_dsp::denoise`, MPL-2.0)

*Learn noise* keeps the mean power per bin, per channel, of a noise-only selection (at least four
2048-point frames); the selection is then cleared, so *Reduce noise* cleans the whole take unless
something else is selected. The reduction is spectral subtraction (`sensitivity ×` the noise,
square root of what is left over what was there, never below `reduction_db`), smoothed over three
bins and, falling, over time; `amount` 0 does not touch the take (bit-exact). **A selection**
is processed with 4096 frames of real audio either side (so the transform's edge frames see
audio, not silence) and meets the untouched audio through **10 ms linear crossfades inside it**
— linear because the two sides are the same audio less its noise, correlated. Defaults 24 dB,
sensitivity 3, smoothing 50 %. Spectral subtraction is for steady noise (hiss, hum, fans); the
comparison by ear with RX / Audition / ReaFIR is the owner's.

### 5. The spectral view

A log-frequency spectrogram (30 Hz – Nyquist, −80…0 dB, the theme's ground → accent → text) of
the mono mix, drawn at the pixel size it is shown at (capped at the GPU's largest texture side).
A long take's whole picture is coarse (a column per take/6000 frames); **zoomed in, the visible
span and a view's width either side is drawn again at a hop of the view's own resolution** (a
power of two from 64 to 512 frames) off the interface thread, on a grid of view widths so a small
scroll asks nothing new. Dragging on it picks a time-frequency rectangle; *Apply to region*
scales it by a gain with soft edges (one hop in time, 1.5 bins in frequency). **Only the samples
the rectangle can reach are rewritten**: frames centred within one hop of it, `N/2` samples either
side of their centre — `±(N/2 + HOP)` of the selection; every sample past that keeps its bits (a
test checks the first ones outside).

**§5a — Clicks.** *Declick* finds clicks in the selection (or the take): a 32nd-order
autoregressive model per 2048-sample block (Levinson–Durbin on the Welch-windowed
autocorrelation); a sample whose prediction error passes `k` × the block's robust scale (median
absolute error / 0.6745, floored at a thousandth of the block's level) is suspect, `k` from 12
(sensitivity 0) to 4 (sensitivity 1); suspects within 24 samples are one click; anything over 400
samples is not a click and is left. Each click is **rebuilt by least-squares AR interpolation**
(Vaseghi, Godsill & Rayner): the missing samples minimise the model's prediction errors over the
gap and 32 samples past it — a small symmetric positive-definite system solved by Cholesky.
*Repair selection* rebuilds a selected stretch of up to 20 ms the same way. Additions,
multiplications, divisions and `sqrt` only: the same bits everywhere. A click on a five-minute
take is found in the detail view and repaired to within 0.01 of the audio it hid (a test).

### 6. Markers, regions, loop points — saved with the file (schema 50 → 52)

Markers (a flag on the ruler; a click selects to the next), named **regions** (bars at the
ruler's foot; a click selects one) and **loop points** belong to *the file*:
`Project::source_marks` by source id (`SourceMarks { markers, regions, loop_points }`, in the
file's frames), set by the new `SetAudioSourceMarks` (validated in range and in order; its inverse
is the marks before; removing a source removes its marks and its inverse puts them back;
classified in `freeze_guard` as changing no sound; its own stream in the undo fuzz). A destructive
edit carries them to the new file **moved with the audio** (a cut moves those after it up and
collapses those inside onto its start; a paste, a trim, inserted silence and a reversal map them
likewise; *Remove silence* clears them — its many cuts would not leave them meaningful). Load drops
marks of a file not in the pool or out of its range (a hand edit). The schema is 50 in this branch
(49 is the newest merged) and becomes 52 when the integrator merges *mix* (50) and *external* (51).
The loop points are stored for a sampler that plays the file looped; Exemplar keeps its own loop
today, and reading these is BACKLOG §3e's slicer work.

### 7. Cleaning: *Remove unused audio*

Each distinct edit keeps a file, so the project needs a way to let go of them. *Remove unused
audio…* (the editor's toolbar, the audio clip's menu, and the agent tool `remove_unused_audio`)
**first lists what will go, with sizes**: the pool entries no clip, comp window or session clip
plays, and the editor's own files (`edit-*.wav` and `.part`) in the project's audio folder that
**nothing refers to** — not the project, not any command in the undo or redo history, not the
project as saved on disk, **not any other project file in the same folder** (nor their autosaves
and backups), and not an agent's edit waiting to be made. Nothing is tidied or swept while an
edit — the window's or an agent's — is being made, and a half-written `.part` is touched only when
it is older than this run of Cantus (a cut session's). Only the project's own edit folder is
cleaned. The agent tool is a host action of the destructive class: always asked first, with the
sizes, whether it removes pool entries, files, or both. Then the pool entries go in **one undo
step** (`RemoveAudioSource` each), and the unreferenced files are deleted.

*Why the history holds files, rather than the command holding them:* a command cannot hold a file
(`cantus-core` does no I/O, and an inverse that recreated deleted audio would have to keep the
bytes in memory — hundreds of MB for a session of takes). So the files of the entries just removed
stay on disk while the undo history still names them; the dialog says so, and running it again
after reopening the project (history cleared) deletes them. Only files the editor wrote are ever
deleted — never one a person put in the folder — and only in a saved project's folder (the scratch
folder is shared by every unsaved session).

### 8. Threads, keys, audition

Reading, every edit, the profile, the pictures and exports run on the editor's thread
(`cantus-editor`), one at a time. The audio thread is never involved. While the window is open it
keeps Cut, Copy, Paste, Delete, Select all, Clear selection, Space and Stop (a marked hook at the
top of `run_action`). Play auditions the selection through the browser's audition voice.

### 9. Agents

`edit_audio` and `denoise_clip` (class Edit) are **jobs**: the take is read, edited and written on
the job's thread with the same functions the window runs, and the edit is planned back on the
interface thread (`JobDone::Then`) — one undo step, the result naming the file, its hash and
whether pitch and warp were dropped. The file waits, as a pending entry, to see the edit made; a
declined card or another project removes it. `remove_unused_audio` (class Destructive) plans the
pool step and deletes the unreferenced editor files once it is made. Bad values (a zero-length
insert, `lo_hz` ≥ `hi_hz`) are bad arguments.

### 10. Not here, and why

- **Recording into the editor.** Edison records the armed input into its own buffer. Ours would
  either duplicate the recorder (ADR 0025: takes, punch, comping lanes, the latency placement, the
  count-in) inside a modal window, or drive the transport from it while round 36's *record* builder
  owns the recording mode. Neither fits the recorder cleanly. A take recorded in the arrangement is
  one click from this window (*Edit in audio editor…* on it). Recording straight into the editor
  waits for the recording mode to settle; it is in BACKLOG §3 *Editing*.
- **An insert effect rendered into a selection.** It belongs to the clip, not to this window: a
  native effect on a stretch of audio is a clip effect (ADR 0070's clip FX) or bounce in place (ADR
  0064) — both keep the effect editable and undoable as an effect. Rendering it destructively here
  would need the effect's whole interface inside a modal window and would throw its settings away.
  What the editor adds is the destructive tools no effect gives (cuts, repair, region, noise by
  profile).

## Consequences

- A cleaned take is a plain file: it bounces, freezes, exports and survives a lost plug-in. The
  price is disk (a 3-minute stereo take is 66 MB per distinct edit), which *Remove unused audio*
  gives back.
- The edits are pure and the files named by their bytes, so tests hold them exactly.

## What the tests hold

`cantus-dsp` (`denoise_tests`): STFT identity to 2e-6; amount 0 and a 0 dB region bit-exact; hiss
≥ 12 dB quieter with the voice within 1 dB; monotonic; deterministic; refusals; the region's tone
down ≥ 20 dB and another within 0.3 dB; clicks found (and none in a clean take), repaired to within
0.03, nothing else moved; a gap without context refused. `cantus-core` (`audio_edit_tests`,
`audio_edit_tests2`): identity edits bit-exact with raw joins; each edit inside its selection; joins
(a mid-wave cut, pastes, silence, inserted silence, trims) without a step; snapped edges within the
window and reported; an empty result refused; *Remove silence* with nothing kept touching nothing
outside the selection; a denoised selection continuous at its edges and the rest bit-exact; the
region's reach exact; LUFS too short; the ceiling and the clipping note; `keeps_timing`; declick and
repair; the WAV named by its hash. `cantus-core` `undo_invariants`: `SetAudioSourceMarks` set,
refused out of range, cleared and gone with its source. `cantus-project` (`schema50`): the fixture's
marks load, older files have none, bad marks dropped. `cantus-ui` (`tests/editor.rs`): the session,
files and undo; busy held and the label right; warp kept by gain and dropped by reverse; another
project closes it; a Save As moves the file; no orphan from a closed window or a declined agent
edit; agents' bad arguments; marks saved and moved; a paste conformed from 44.1 kHz stereo; files
pasted and opened; a selection to a new clip in one step; *Remove unused audio*; a click on a
five-minute take found in detail and repaired; the window in both themes and at 200 %.
