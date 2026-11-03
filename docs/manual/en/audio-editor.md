# The audio editor

*Cantus 0.1.0-beta.11 and later · [Leer en español](../es/audio-editor.md)*

Cut a cough out of a take, silence a pop, fade the ends, bring a take to a level, take the hiss
and the clicks out, remove the dead air between phrases — and hear it in the clip straight away.
The audio editor is the wave editor you open from a clip. (The pitch editor, which corrects
notes without touching the audio, is a different tool: double-click the clip.)

## Open it, leave it

Right-click an audio clip ▸ **Edit in audio editor…** The window shows the clip's whole file.
Close it with ✕: the clip plays what you left. Undo and Redo work as everywhere else, with the
window open or closed; the window follows them. Opening another project closes it.

**Nothing you do here touches the file you started from.** Every edit makes a new file (named
`edit-` and a code from its content, in the project's *Audio* folder) and points the clip at it:
one undo step each, the original still in the project's list of files.

Also in the toolbar: **Open file…** puts a file on a new track and opens it here; **Paste from
file…** pastes a file at the cursor (at another sample rate, or stereo into mono, it is converted
first — so is the clipboard); **To new clip** — or **Alt-drag** the selection out of the window —
makes the selection a clip of its own on a new track, right where it plays now.

## Select to the sample

- **Drag** in the wave or the spectrogram to select. The foot of the window says the selection
  to the frame (`1.23456 s (59259)`). Drag an edge to change it, **Shift**-drag to extend,
  double-click to select everything, click for a cursor. Escape clears the selection.
- **Wheel** zooms about the pointer; **Shift + wheel** (or the bar under the graphs) scrolls.
  *Fit all* and *Zoom to selection* are in the toolbar. Zoomed in far enough, the samples
  themselves are drawn, and the spectrogram is drawn again in detail for what you see.
- **Marker** drops a flag at the cursor; **Region** names the selection; **Loop points** marks the
  selection as the loop. They are saved with the file, in the project, and move with the audio
  when you cut or paste. Click a flag on the ruler to select up to the next one, or a region's bar
  to select it. *Clear marks* removes them (one undo step).
- **Play** auditions the selection, or the whole take (Space starts and stops it).

## Edits

The toolbar: **Cut**, **Copy**, **Paste** (replaces the selection, or goes in at the cursor),
**Delete**, **Trim**, **Silence**, **Invert**, **Reverse**. With nothing selected, Invert and
Reverse work on the whole take. Cut, Copy, Paste and Delete use the keys you know while the
window is open. While an edit is being made, the next one waits ("Wait: the editor is still
working…").

**Smooth joins** (on by default): where pieces meet — a cut, a paste, silence — the edges move to
the nearest zero crossing and the two sides cross over 3 ms, so nothing clicks. Turn it off for a
raw splice exactly where you put it.

The right column:

- **Level.** *Apply gain* (dB); *Normalize* to a **peak**, an **RMS** (both dBFS) or a loudness
  (**LUFS**, integrated; at least 0.4 s) target — of the selection, or the whole take. With **Peak
  ceiling** on, RMS and LUFS stop where the peaks reach the ceiling (the status says what level
  was reached); off, the status warns when peaks pass 0 dBFS.
- **Fades.** Five curves; *Fade in* and *Fade out* over the selection.
- **Noise reduction.** Select a stretch of **only noise**, press **Learn noise** (the selection is
  then cleared); **Reduce noise** cleans the whole take, or a selection you make (which blends
  into the rest over 10 ms). *Amount* 0 % changes nothing; *Reduce* is the most any frequency is
  turned down; *Strength* is how much of the noise's peaks it catches; *Smooth* softens the result.
  For steady noise — hiss, hum, fans.
- **Silence and DC.** *Remove silence* cuts the stretches below *Below* that last longer than
  *Minimum*, leaving *Keep* milliseconds of each. *Remove DC* takes the offset out.
- **Clicks.** **Declick** finds clicks and pops in the selection (or the take) and rebuilds each
  from the audio around it; *Sensitivity* finds fainter ones. Or select a click (up to 20 ms) and
  **Repair selection**.
- **Spectral region.** In **Spectrogram** or **Both**, drag a rectangle over the sound to turn down
  (a whistle, a hum's band), set *Region gain* and **Apply to region**.

An edit that would change nothing says so and makes no file; one that would leave nothing is
refused.

## What the clip keeps

Gain, fades, crossfades and reverse stay. The pitch editor's notes and warp markers stay through
edits that leave the audio's timing alone (gain, normalize, invert, DC, fades, silence, noise,
clicks, region) and are dropped — the status says so — by Reverse, Paste and edits that change the
length. A clip that showed the whole take shows the whole edited take; other clips keep their
place. Linked clips change together.

## Clean up

Each edit keeps a file. **Clean up…** (also in the audio clip's menu: *Remove unused audio…*) lists
the files no clip plays and the editor's files nothing refers to, with their sizes, before removing
anything; the list leaves the project in one undo step. Files you put in the folder yourself are
never deleted, and files Undo could still bring back stay on disk until you reopen the project and
clean up again.

## For agents

`edit_audio`, `denoise_clip` and `remove_unused_audio` do the same, in one undo step each; the
edits run in the background. Positions are seconds in the clip's own file.

## Good to know

- Takes up to 30 minutes open here.
- Recording straight into the editor is not here yet: record in the arrangement and open the take.
