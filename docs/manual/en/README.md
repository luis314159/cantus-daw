# Cantus — quick-start manual

*For Cantus 0.1.0-beta.1 on Windows · [Leer en español](../es/README.md)*

Cantus is a free, open-source digital audio workstation: record, compose, mix and master a
song, from the first note to a finished file. This manual gets you there in an evening.
Everything it names is in the program as it is today; where the beta still has gaps,
[chapter 14](#14-known-issues-and-limitations) says so.

![Cantus at start: the arrangement, the mixer below, the inspector at the right](../img/startup.png)

**Contents**

1. [Install](#1-install)
2. [First run: set up your audio](#2-first-run-set-up-your-audio)
3. [The workspace](#3-the-workspace)
4. [Record audio and MIDI](#4-record-audio-and-midi)
5. [Edit: clips, piano roll, automation](#5-edit-clips-piano-roll-automation)
6. [Mix: the mixer, inserts and sends](#6-mix-the-mixer-inserts-and-sends)
7. [Master: the master chain, Fervor, Latitudo, Culmen](#7-master-the-master-chain-fervor-latitudo-culmen)
8. [Export](#8-export)
9. [Sounds: the browser and sound packs](#9-sounds-the-browser-and-sound-packs)
10. [Instruments and effects: the device picker](#10-instruments-and-effects-the-device-picker)
11. [Projects, autosave and settings](#11-projects-autosave-and-settings)
12. [Keyboard shortcuts](#12-keyboard-shortcuts)
13. [Privacy and updates](#13-privacy-and-updates)
14. [Known issues and limitations](#14-known-issues-and-limitations)
15. [Report a bug](#15-report-a-bug)

## 1. Install

You need Windows 11, 64-bit (Windows 10 untested), and about
200 MB of disk.

1. Download `Cantus-0.1.0-beta.1-windows-x64-setup.exe` from the
   [releases page](https://github.com/luis314159/cantus/releases).
2. Run it. It installs for you alone, in `%LOCALAPPDATA%\Programs\Cantus`, and asks for no
   administrator rights. This beta is not code-signed yet, so Windows may show *"Windows
   protected your PC"*: choose *More info ▸ Run anyway*.
3. Choose the components: the program (required), *Open .cantus projects with Cantus*
   (on), a desktop shortcut (off), the CLAP and VST3 plug-ins for other DAWs (on), and
   *FlexASIO driver (downloaded now)* (off — tick it if your sound card has no ASIO driver
   of its own; see [chapter 2](#2-first-run-set-up-your-audio)).
4. The *Privacy* page says what Cantus sends over the network (only a daily question about
   new versions, and the downloads you ask for). *Check for updates once a day
   (recommended)* is ticked; untick it to turn the check off (also later in *Settings ▸
   Updates*).

**Portable copy.** Unzip `Cantus-0.1.0-beta.1-windows-x64-portable.zip` anywhere (a USB
stick works). Because of the `portable.txt` beside `cantus.exe`, that copy keeps its
settings in the `data` folder next to it and writes nothing to your profile.

**Uninstall** from Windows' *Apps*. Your settings, presets and autosaves — and the sample
cache (up to 16 GB, see [chapter 11](#11-projects-autosave-and-settings)) — are removed
only if you tick that box; your projects are never touched.

## 2. First run: set up your audio

The first time Cantus starts, the window **Set up your audio** opens over the music.

![The first-run audio setup](../img/first-run.png)

1. **Choose your interface.** Each audio device has a card: an *Audio interface*, *This
   computer*, *Headphones*… Click the one you play and listen through. Cantus picks a
   driver and a buffer for it: **ASIO** at 128 samples for an interface with its own ASIO
   driver, otherwise **WASAPI** (Windows' own audio) at 256.
2. **Listen.** Click *Test tone*: you should hear a soft tone on outputs 1–2.
3. **Play or speak — the meter should move.** Strum, sing or speak into what is plugged
   in; the input that moves is the one you will record from.
4. **Latency.** Cantus says how long the sound takes from string to speaker, and whether
   that is tight enough to play through. For the most accurate takes, cable output 1 to
   input 1 and click *Measure latency*; Cantus then places every take to the sample.

Click **Record on …** and Cantus adds an audio track on that input, armed and ready:
press `R` to record. *Skip* closes the window; it does not open by itself again (the
*Setup assistant…* button in the audio chip brings it back).

### The audio chip

The chip in the top bar (or in its ⋯ menu when the window is narrow) shows the device,
driver, rate, buffer and latency —
for example `UMC204HD · ASIO · 48 k · 128 · 6.1 ms`. Its lamp is green when the audio
runs, amber after a dropout, red when the device is lost or the driver stopped. **Click
it** to change anything; every change applies at once and is remembered for that
interface:

![The audio chip's popover](../img/audio-popover.png)

- **Interface**, **Driver** (WASAPI or ASIO, with the *ASIO driver* list), **Input
  device**, **Rate**;
- **Buffer · round trip**: 64 to 2048 samples, each with its latency. Smaller is tighter
  for playing; larger is safer for big mixes. If the top bar counts dropouts, take the
  next size up;
- problems appear on top with a button that fixes them (*Restart audio*, *Use this
  computer's audio*, *Run at 48 kHz*…);
- *Test tone*, *Channels & routing…* (name your inputs and outputs, choose a headphone
  cue pair), *Setup assistant…*, and the gear for every setting.

**Which driver?** Use your interface's own ASIO driver if it has one (Behringer, Focusrite,
PreSonus, Steinberg, MOTU, RME…). For a laptop's built-in sound, use WASAPI, or install
**FlexASIO** for lower latency. *Get ASIO…* in the popover opens a window with FlexASIO's
download page, ASIO4ALL's page and the makers' driver pages; after installing a driver,
click *Rescan drivers*. Cantus never installs drivers by itself.

Plugging an interface in offers to switch to it; unplugging one offers your computer's
audio, and Cantus goes back to it when it returns. If a driver stops responding, Cantus
restarts the audio once by itself. You cannot change the device while recording.

## 3. The workspace

Cantus opens on a song — the second drop of the demo *Neon Tide*, looping — so the first
thing you hear is music. Press `Space` to play it.

- **The top bar** (see [below](#the-top-bar)): the menus over the project's name and the
  hint panel, then the transport, the tempo, the metronome, the position, the meters and
  more — one group per function.
- **The arrangement** (centre): tracks and their clips on a timeline.
- **The clip browser** (`Alt+F5`, *View ▸ Clip browser*): every clip of the song as a
  tile, beside the arrangement (see [chapter 5](#clips-in-the-arrangement)).
- **The lower zone**: the Mixer, the Piano roll and the Channel rack as tabs.
- **The inspector** (right, `I`): everything about the selected track, strip or clip.
- **The status bar** (bottom): what is under the pointer, plug-in latency, samples loading,
  the typing keyboard's octave and minimised devices.

Every area can be shown or hidden with its window key or its function key (`F5`
arrangement, `Alt+F5` clip browser, `F6` channel rack, `F7` piano roll, `F9` mixer,
`Alt+F8` browser, `I` inspector). Drag the splitters to resize; right-click a tab to float it in its own
window; save the arrangement of the window under *View ▸ Layout ▸ Save layout…*. The
interface zooms from 75 % to 200 % (`Ctrl+Plus`, `Ctrl+Minus`, `Ctrl+0`). Twelve themes —
Cantus Night (the default), Cantus Paper, Daylight, Blue Hour, high-contrast dark and light
and more, each with a preview — the colours, the logo and the taskbar icon's colour are
under *View ▸ Appearance*.

**Typing a name or a value** works the same everywhere — renaming a track, a clip, a pad
or a strip, a search, a level: the field opens with its words selected, so typing
replaces them; `Enter` or a click elsewhere keeps what you typed; `Esc` puts back what was
there. While you type, no shortcut fires. Names can be in any script — Japanese, Chinese,
Korean, Hindi, Thai, Arabic, Hebrew…: Cantus uses your system's own fonts for them (see
the [known issues](#14-known-issues-and-limitations) about right-to-left scripts).

**Demo songs.** *File ▸ Demo songs…* (`Ctrl+Shift+N`) opens six songs made entirely with
Cantus's own instruments and effects: Neon Tide (house), Morning Light (pop ballad),
Sunday Strut (funk and soul), Paper Lanterns (lo-fi hip-hop), Aurora (cinematic) and
Midnight Static (trap). Open one, play it, take it apart.

### The top bar

The top bar is FL Studio's strip, one group per function, a thin divider between them:

- **The menus and the hint panel**: *File, Edit, Clip, View, Help* over the project's name
  and the hint for whatever is under the pointer. **Click the name** to rename the
  project in place (`Enter` keeps it, `Esc` leaves); the window's title follows it, with
  `•` while there are unsaved changes.
- **PAT / SONG** (`L`): *song mode* plays the arrangement; *pattern mode* plays the clip
  you are editing in the piano roll or the channel rack — with whatever the rack shows
  on the other tracks in that stretch — alone and looped, and the clock counts from the
  pattern's start. Choosing another clip changes what plays. Leaving pattern mode puts
  the playhead back where the song was. Pattern mode does not record (a take belongs to
  the song: switch to song mode first) and export always renders the song. It is saved
  with the project.
- **Transport**: go to start, play, stop, record and its ▾ options, loop.
- **Tempo**: the BPM (drag, wheel, double-click for 120); **TAP** (`Shift+T`): tap at least
  twice — the tempo is the average of your last taps, a pause starts again, and each run
  of taps is one undo step; the **time signature**: click it for the beats (‹ ›), the
  note value and the usual metres.
- **Metronome** (`Ctrl+M`): its switch, the **count-in** (off, 1, 2, 4 bars) and the
  click's **level** (drag, wheel, double-click for 0 dB). Its ▾ holds the **sound**
  (*Classic*, *Woodblock*, *Beep*), the **accent** on the first beat of each bar, and
  **when** it plays (always, while recording, during the count-in). The sound and the
  accent are saved with the song.
- **Position**: bars.beats.ticks and the time, the metre beside them; while recording, the
  take's time.
- **Grid and snap**: the grid and the SNAP lamp of the editor you are working in — the
  arrangement, the piano roll or the channel rack (always on its grid). The same values
  as the editor's own toolbar.
- **Undo** and **redo**, and the **window keys** (browser, arrangement, rack, piano roll,
  mixer, inspector).
- **CPU, RAM, voices, drops**: the audio callback's load (its histogram in the tooltip),
  the memory Cantus uses, the voices sounding and the dropouts counted (the latest one's
  cause in the tooltip, and said on the status line when it happens).
- **Master meter**: *Peak* shows the left and right peaks and holds the highest of the
  last two seconds (click the number to let it go); *LUFS* shows the master's loudness —
  momentary in the top lane, short-term below, with a mark at −14 LUFS (the streaming
  services' level), and underneath the short-term, the integrated loudness since you
  pressed play and the true peak.
- **Inputs**: the typing keyboard (`Ctrl+T`), the **MIDI** lamp (it lights as notes come
  in; the number is the inputs open) and **Armed** (how many tracks will record; click to
  arm the selected one).
- **The audio chip** (see [the audio chip](#the-audio-chip)) and the **tools**: the
  element picker, the appearance page, and the interface's zoom.

**A narrow window** folds the least important groups — whole — into the **⋯** menu at the
right end, where they work as on the bar; the menus fold into one key before the
transport would. A dot on ⋯ says something is folded there (red when it is the audio chip
and the audio needs a look). **Customise top bar…** (in ⋯, in *View*, or right-click the
bar): tick a group to show or hide it, drag it by its grip to move it, *Reset* for the
default. Your arrangement is kept.
## 4. Record audio and MIDI

### An audio track

1. Add an audio track: `Ctrl+Shift+A`, or right-click the **+** above the track headers
   ▸ *Add audio track*. (The first-run setup's *Record on …* makes one for you.)
2. **Arm it**: click the round red lamp on its header (or `Alt+R`). Shift+click arms that
   track alone. An armed track shows its input meter.
3. **Choose its input**: the input chip on the header (*In 1*, *In 1-2*…; shown when the
   track row is tall enough), the inspector's *Recording* section, or the track's menu.
4. **Monitoring**: the speaker key on the header cycles *Off*, *Auto* (you hear the input
   while the track is armed — the default) and *On*. The inspector also has *Trim*
   (−24 to +24 dB before recording).
5. Press **`R`** (or the record key). Every armed track records; press `R` or `Space` to
   stop. The whole pass is one undo step.

Takes are written as 32-bit float WAV files: into the project's `Audio` folder once it is
saved, until then into `%LOCALAPPDATA%\Cantus\recordings\`; saving moves them into the
project. Each take is placed earlier by the measured latency, so it lines up with what you
heard.

### Recording options

The ▾ beside the record key (or right-click it) opens **Recording options**:

![Recording options](../img/record-options.png)

- **Count-in**: off, 1, 2 or 4 bars before the take (`Alt+C`). The count-in clicks
  whether or not the **Metronome** (`Ctrl+M`) is on.
- **Punch**: record only inside a range. *From selection* takes the time selection; `Alt+P`
  uses the selection, else the loop, else two bars from the playhead. The range shows as a
  red brace under the ruler: drag its ends. Everything played is kept in the file; punch
  only sets the clip's window.
- **Loop takes**: with the loop on, each pass becomes a take.
- **Take mode**: *Stack* (old clips move to take lanes — the default), *Replace* (cut what
  the take covers) or *Overdub* (MIDI: add notes to the clip).
- The click — its level, sound, accent and when it plays — is the metronome's, in the
  [top bar](#the-top-bar).

### Takes and comping

Every take is kept on a **take lane** under the track: open them with the small disclosure
at the bottom of the track's colour cell, `Alt+T`, or the track menu's *Show takes*.
**Drag across a take** to comp that stretch into the track; click a take to use all of it.
Right-click a lane for *Use this take*, *Delete unused takes* (which also drops recordings
nothing plays any more), *Explode takes to new tracks* and *Hide takes*.

### MIDI

Arm an **instrument track** (one with no audio input) and press `R`: notes from every MIDI
keyboard Cantus found (the status bar says *MIDI inputs: n*) are recorded into a clip.
With nothing armed, `R` records into the selected track. No keyboard? `Ctrl+T` turns the
**typing keyboard** on: `A`–`L` play notes, `Z`/`X` change octave. While it is on, letter
shortcuts (including `R`) are off — use `Space` or the record key.

## 5. Edit: clips, piano roll, automation

### Quick gestures

As in FL Studio, the arrangement opens things with no hunting through menus:

- **Double-click a track header** (anywhere but its name): its **instrument** opens — the
  instrument's window (Unda, Pulsus II…) or a CLAP/VST3 plug-in's own window. Firmus and
  audio tracks, which have no window, show the inspector instead.
- **Click a track's name** — on the letters themselves (the pointer becomes a text cursor)
  — to **rename** it in place: the name is selected, type the new one, `Enter` keeps it
  (one undo step), `Esc` puts back the old one. A click elsewhere on the header only
  selects the track, and typing with a track selected never renames it. (Mixer strips and
  rack pads rename on a double-click.)
- **Double-click a clip**: the **piano roll** opens on it, the whole clip in view. A step
  pattern the rack shows with nothing lost — every note a short hit on a step, one pitch
  per channel (a drum kit may use all its pads; a melody on a synth opens the piano roll,
  as in FL) — opens in the **channel rack** instead, at the pattern's own grid and
  scrolled to its track. An automation clip opens in its large editor; an audio clip shows
  in the inspector.
- **Open editor** — in the inspector, the mixer's menus or an instrument's frame — brings a
  window that is already open back from the tray (or the taskbar, for a plug-in instrument
  or effect) and to the front; while a plug-in's window is open its key reads *Show*. `F8`
  and the channel rack's channel button do the same when the window is minimised, behind
  another tab or under another window, and close it only when it is already in front.
  (Cantus cannot bring forward a plug-in that floats a window of its own: its key reads
  *Close*.)

### Clips in the arrangement

- **Create**: double-click an empty place on a track (a 4-bar clip), or record.
- **Select**: click; `Ctrl+click` adds; drag on empty space for a marquee; `Esc` clears.
- **Move**: drag (also to another track). `Alt+drag` copies; `Ctrl` while dragging ignores
  the grid. `Left`/`Right` nudge by the grid, `Shift` by a bar.
- **Trim**: drag a clip's edge — its content stays where it is. **Slip**: `Shift+drag`
  slides the content inside the clip.
- **Split** at the playhead: `Ctrl+E`. **Glue**: `Ctrl+J`. **Duplicate**: `Ctrl+D` (press
  again to keep tiling). **Mute**: `0`. **Rename**: `F2`. **Colour**: `Ctrl+Shift+C`.
- **Time selection**: drag in the ruler's lower lane; the top strip is the loop. `Shift+L`
  loops the selection, `Z` zooms to it.
- **Audio clips** have handles for fade in, fade out and gain (double-click one to reset
  it; right-click a fade for its curve) and a menu with *Normalize*, *Reverse*, *Reset clip
  gain* and *Remove fades*.
- **Import audio**: drag WAV, AIFF, FLAC or Ogg files onto the arrangement (they land at
  the pointer), or *File ▸ Import audio…*. A MIDI file adds its tracks.
- **Linked copies**: a paste, `Ctrl+D` or `Alt+drag` makes *the same clip* again — edit one
  and every copy follows (notes, or an audio clip's gain, fades and reverse). A link mark and
  a ×N count show it. *Make unique* (`Alt+U`) takes the selection out of its group;
  *Select linked copies* (`Shift+Alt+L`); *Link selection to this clip* (`Alt+L`); *Link
  to…* (`Alt+N`) picks, by name or track, a linked group or a clip of its own to become.
  In the inspector, a linked clip's *Link* row shows how many copies it has and offers
  *Link to…*; renaming it in the *Name* row renames every copy. Right-click an empty place
  in the arrangement ▸ *Copies are linked* to make independent copies instead.
- **The clip browser** (`Alt+F5`, or *View ▸ Clip browser*) shows every linked group and
  every clip of its own once, as tiles in the clip's colour with a miniature of what it
  holds; type to filter by name or track. **Drag a tile** onto the arrangement to place a
  linked copy there. **Click a tile** to choose it for the **paint tool**: with the
  arrangement focused, `B` (or `2`) takes the tool out — a click places a linked copy, a
  drag paints copies end to end along the track, a right-drag (or `Alt`+drag) erases the
  copies of that clip only; each stroke is one undo step; `Esc` puts the tool away.
  Double-click a tile to select all its copies.

The **grid** (the arrangement's toolbar) starts at one bar; choose finer values for
detail. Scroll with the wheel, `Shift+wheel` for time, `Ctrl+wheel` to zoom.

### The piano roll

Select a MIDI clip and show the piano roll (`F7`, or its tab under the arrangement): it
edits that clip's notes.

![The piano roll](../img/piano-roll.png)

| Tool | Key | What it does |
|---|---|---|
| Draw | `P` or `1` | Click adds a note, drag moves, drag the right edge resizes, right-click deletes |
| Paint | `B` or `2` | Drag across the grid to paint notes |
| Select | `E` or `3` | Drag to select; drag notes to move them |
| Delete | `D` or `4` | Click or drag over notes to delete |
| Mute | `T` or `5` | Click a note to mute it (it stays, silent) |
| Slice | `C` or `6` | Drag a line to split notes |
| Play | `Y` or `7` | Click notes or rows to hear them |

`Up`/`Down` transpose the selection (`Shift`: an octave), `Left`/`Right` nudge it, `Q`
quantizes, `Ctrl+C`/`Ctrl+V` copy and paste. Velocities are stems in the lane under the
grid: drag them, or `Alt+wheel` over a note. A drum kit shows one named row per pad.

The **channel rack** (`F6`) shows every track as a row of steps over the selected clip's
bars: click a step on, right-click it off, drag to paint. A step is an ordinary note.

### Automation

Right-click any knob, fader or effect control ▸ **Create automation clip**, or move a
control and press **`Alt+A`** (automate the last thing you touched). The clip covers the
time selection (else the loop, else four bars) on a lane under its track. On the lane:
drag points, drag the ring between two points to bend the curve, click the line to add a
point, double-click a point to delete it, right-click it for its shape (single or double
curve, hold, stairs, pulse, wave). Double-click the clip for the large editor, with a
drawing tool and articulators (flip, smooth, random, scale, LFO). An automated control
shows a dot.

## 6. Mix: the mixer, inserts and sends

![The mixer](../img/mixer.png)

Each track feeds a **strip** of the mixer (`F9`); strips feed the **master** at the left,
or another strip (a bus). A strip has its level meter, mute and solo, pan, fader, phase
and swap, width, a three-band EQ, its inserts and its sends. Click to select, `Ctrl+click`
to add, `Shift+click` for a run; dragging one fader of a selection moves them all. With
the mixer focused, `M` mutes, `S` solos, `Alt+Left`/`Alt+Right` move the strip.
`Ctrl+wheel` changes the strips' width; at *Extra large* each strip shows a one-line summary
of its chain (click it to open the inspector).

- **Inserts** (up to ten per strip): click an empty slot to choose an effect in the device
  picker, or right-click it for the quick list. On a filled slot, click the lamp to bypass
  it and the name to open its window; right-click to swap, move or remove it. In the
  inspector, each insert has a **Mix** ring (dry/wet).
- **Sends**: select a strip, then look at the *Sends* row of the others — click the ▲ of
  the strip you want to send to (a reverb return, a parallel compressor); a knob then sets
  the send's level. Each send taps **after the fader (POST, the default) or before it
  (PRE)** — click its PRE/POST chip; a PRE send keeps sounding when you pull the fader down.
- **Cables**: each strip has a port. Drag from it to another strip, or click it and then
  the destination (`Esc` cancels): plain makes a send at 0 dB, `Ctrl` makes it the strip's
  only output, `Alt` makes it a sidechain key. The selected strip's cables (or all, or
  none — the switch above the desk, with a filter for which kinds) are drawn in a band
  under the desk; click a cable to select it, drag its end to move it, `Delete` removes it.
  A doubled path is allowed and marked. Every change is one undo step.
- **Routing**: right-click a strip ▸ *Routing* ▸ *Output* or *Send to*, `Shift+F10` for the
  selected strip's route menu, or select several strips and *Create submix from selected*.
  In a cable's or a send's route menu, *Level ▸ Type a level…* takes a level in dB (−∞ to
  0; `Enter` or a click elsewhere sets it, in one undo step; `Esc` cancels).
  **The routing matrix** (`Alt+F9`) shows every source against every destination: click a
  cell to route, drag for the level (`Shift` for fine), `Ctrl+drag` copies down a column.
  Strips and groups move by dragging or with `Alt+Left`/`Alt+Right`; a track header dropped
  on a strip plays into it.
- **Device windows** have *Swap*, minimise (to the tray in the status bar), maximise and
  close. `F8` opens the selected track's instrument (or brings it to the front, or closes
  it when it is already in front), `Esc` closes the device you used last,
  `F12` closes every floating one.

**Solo** follows the routing: a soloed strip is heard with what feeds it and what it feeds.

## 7. Master: the master chain, Fervor, Latitudo, Culmen

The **master** has its own chain of up to ten inserts, run on the whole mix before the
master fader. Click the master's head to see its chain in the inspector (*The whole mix*
→ the inserts → *Audio output*), or right-click its name ▸ *Effects* ▸ *+ Add effect*.

![Culmen on the master](../img/master.png)

The demos master in this order, and it is a good start:

1. **Aequalis** (EQ): a high-pass under the sub, a little air.
2. **Multiplex** or **Comprimo**: gentle glue.
3. **Latitudo** (stereo): the preset *Master: tighten the low end* or *Master: wider top,
   mono bass* keeps the bass in mono and the top wide. Watch its correlation meter: it
   should stay above zero.
4. **Culmen** (maximizer): choose a **target** — *Streaming −14* (−14 LUFS, −1 dBTP),
   *Apple −16*, *CD −9*, *Club −7*, *Broadcast −23* — then play the loudest part of the song
   and click **Learn drive**: Culmen listens, sets the drive, listens again and stops on
   the target, in one undo step. The *Loudness* card shows momentary, short-term and
   integrated loudness and the true peak; the verdict says *On target* or how many LU you
   are over or under. Culmen works in three bands plus the full band, into a true-peak
   brickwall at the ceiling; its *Character* (Clean, Punch, Loud, Bus, Safe) and its ten
   presets cover most masters.

**Fervor** (saturation) adds colour: *Tape*, *Tube*, *Overdrive*, *Fuzz*, *Clip*, *Fold* or a
drawn *Curve*, with oversampling, tone, mix and *Level match*, so that louder never fools
you. On the master, the presets *Master clipper* and *Tube glue on the mid* are the gentle
ones.

## 8. Export

*File ▸ Export audio…* (`Ctrl+Shift+E` in the default keys) opens the export window:
presets on the left, the choices on the right, and a line that says exactly what will be
written.

- **Presets**: *Streaming* (MP3 320 kbps, 44.1 kHz, −14 LUFS, −1 dBTP), *CD* (WAV 16-bit,
  44.1 kHz, shaped dither), *Stems for mastering* (every strip, WAV 24-bit at the session's
  rate, not normalised) and *Podcast* (mono MP3 128 kbps, −16 LUFS, −1 dBTP). Change any
  choice and the *Custom* card lights.
- **What**: the song (from the start to the end of the last clip, plus a tail you choose),
  the time selection, or **stems** — one file per mixer strip (after its fader) or per track
  (before its strip), all starting on the song's first beat, delay compensation included,
  optionally *through the master chain*.
- **Formats**: **WAV** 16-bit, 24-bit or 32-bit float; **FLAC** 16 or 24-bit; **MP3** CBR
  128–320 kbps or VBR V0/V2/V4/V6 (LAME), gapless; **Ogg Vorbis** quality 3–10. Sample rate:
  the session's or 44.1, 48, 88.2, 96, 192 kHz (MP3 uses 44.1 or 48). Mono or stereo.
- **Dither** (off, TPDF, shaped) when writing 16 or 24 bits.
- **Loudness**: off, or a target (−9, −11, −14, −16, −19, −23 LUFS) with a true-peak ceiling
  (−0.1, −0.3, −1, −2 dBTP); Culmen's own limiter holds the ceiling when the gain needs it.
  Stems are never normalised.
- **Tags**: title, artist, album, year, comment, and a *Cantus* encoder credit.

Exports run in the background with a progress bar; *Stop* leaves no partial file. Your last
choices are remembered.

- **MIDI**: *File ▸ Export MIDI…* — the whole song, the selected track or the selected clip,
  as a type 1 file with the tempo, metre and track names.
- **DAWproject**: *File ▸ Export DAWproject…* to move a project to Bitwig, Studio One and
  other DAWs that read it (a report lists anything that could not travel).

## 9. Sounds: the browser and sound packs

The **browser** (`Alt+F8`, the first window key) docks at the left. It lists the sound
packs you installed and *My folders* (click **+** to add any folder of samples).

![The sample browser](../img/browser.png)

- **Click** a sound to hear it; click again to stop. Type in the search field to find
  sounds across every pack and folder.
- **Drag** a sound onto a **Pulsus pad** (the pad plays it), onto **Exemplar**'s key map (a
  zone on that key), onto the **channel rack** (a new sample channel with its own strip),
  or onto the **arrangement** (an audio clip where you drop it). Each drop is one undo step.
- With nothing installed, the browser offers **Start with a drum kit**: *Download* fetches
  *Cantus Starter Drums* (81 one-shots, 33 MB). *Sound packs…* (or *More packs…*) lists
  the others — Karoryfer's kits, VCSL and VSCO 2 percussion, a TX81Z for Exemplar, and the
  **Salamander Grand Piano** (a Yamaha C5, 16 velocity layers) — with their size, licence
  and credits; *Install*, *Update*, *Remove*, *Show the folder*, and *Install from a
  file…* for computers without internet.

Every pack is free. Most are **CC0** (no credit needed); the Salamander Grand Piano is
**CC BY 3.0**: its credit is shown on its page in *Sound packs…* and in *Help ▸ Credits*,
and kept in the pack's folder. Either way, the music you make with them is yours. Packs
are saved in `%LOCALAPPDATA%\Cantus\Content\`; every file is checked against its SHA-256.

**Pack instruments.** An installed pack that brings instruments (the Salamander: with 8
velocity layers, about 120 MB of memory, or all 16, about 185 MB) shows them in the
device picker's *Keys* shelf and on its *Sound packs…* page (*New track with …*); each
plays in Exemplar. Long samples are **streamed from disk**: only their first moments stay
in memory. The first time, Cantus decodes the samples once into its **sample cache** (1 GB
for the Salamander's 8 layers, 2 GB for 16), beside the window, with *Loading samples… n
of m* on the status bar; later loads take a fraction of a second. If the disk cannot keep
up, the status bar counts *Disk late: n*.

**A missing pack.** A project that plays a pack this computer does not have opens a card
that names the pack and its silent tracks: *Install in Sound packs…*, *Use the installed
pack* (when the project points at another copy, say from another computer), or *Find the
folder…* (when files of an installed pack were moved). The rest of the project plays as
usual.

## 10. Instruments and effects: the device picker

The **device picker** shows every instrument and effect — Cantus's own and the CLAP and
VST3 plug-ins installed on your computer — as tiles.

![The device picker](../img/picker.png)

Open it with the **+** above the track headers (a new instrument track), by clicking a
track's instrument chip (swap the instrument), *Swap* on any device window, an empty
insert slot, or *View ▸ Device picker…*. Search by name, maker or kind; filter by shelf
(*Favourites*, *Recent*, a category, or *Cantus / CLAP / VST3*); arrows move, `Enter`
chooses, `Ctrl+D` marks a favourite, `Esc` closes. Installed pack instruments (the
Salamander Grand Piano) are under *Keys*. Swapping an instrument keeps its notes,
clips, mixer strip and effects; swapping an effect keeps its slot, mix and bypass.

| Instrument | What it is |
|---|---|
| Firmus | The simple synth every new track starts with |
| Pulsus II | Drum machine: synthesized drums and sample pads, choke groups |
| Unda | Wavetable synth: three oscillators, unison, two filters, drawn LFOs |
| Calor | Analog polysynth |
| Modulus | FM synth with eight operators; reads DX7 banks |
| Exemplar | Sampler: SFZ, Decent Sampler and SoundFont |
| Gravis | Bass: 808, synth and modelled electric bass |
| Lamina | Electric piano |
| Organum | Tonewheel organ with drawbars and a rotary speaker |

The effects are Aequalis (EQ), Comprimo (compressor), Porta (gate and expander), Sibilus
(de-esser), Multiplex (multiband dynamics), Terminus (limiter), Culmen (maximizer),
Spatium (reverb), Fervor (saturation), Latitudo (stereo), Delay and Reverb.

**Your plug-ins.** Cantus scans the standard folders — `%LOCALAPPDATA%\Programs\Common\CLAP`
and `…\VST3`, `C:\Program Files\Common Files\CLAP` and `…\VST3` — and any folder you add
in *Settings ▸ Plug-ins* (*Scan for plug-ins*, *Rescan everything*). Each file is scanned
in a separate process, so a plug-in that crashes while being scanned is blocked, not
fatal. Cantus's own effects are installed in your CLAP and VST3 folders too, for other
DAWs.

## 11. Projects, autosave and settings

- *File ▸ New project* (`Ctrl+N`), *Open…* (`Ctrl+O`), *Save* (`Ctrl+S`), *Save as…*
  (`Ctrl+Shift+S`). A project is a `.cantus` file; saving keeps the previous one as
  `.cantus.bak`.
- **Audio files** are collected when you save: recordings move into the project's `Audio`
  folder and imported files are copied there, so the folder travels whole. *Collect audio
  and save* does it on demand. A missing file shows **MISSING** on its clip; right-click it
  ▸ *Locate file…*.
- **Autosave** writes a recovery copy every minute when something changed (*Settings ▸
  Saving*, 10 s to 10 min): `<project>.cantus.autosave` beside a saved project, or in
  `%LOCALAPPDATA%\Cantus\autosave\` for an unsaved one. It never overwrites your file.
  Writing it never freezes the window (it runs in the background).
- **Recover your work**: if Cantus closes unexpectedly — or you close it without saving —
  the next start offers what was left: the autosaved changes and any recorded takes that
  are in no saved file. *Restore* brings them back as one undo step (`Ctrl+Z` shows the last
  save); *Discard* deletes nothing. *New* and *Open* ask before replacing unsaved work.

**Settings** (*View ▸ Settings…*): *General* (the verse at start, on or off, and the
Spanish Bible), *Audio* (driver, devices, rate, buffer, latency and the record offset),
*MIDI*, *Appearance* (language — English or Español —, interface size, colours, keyboard
shortcuts), *Saving*, *Sample cache* (its size limit — 16 GB by default; when full, the
least recently used samples go first — and *Clear cache*; it lives in
`%LOCALAPPDATA%\Cantus\cache\samples`, or in a portable copy's `data` folder), *Updates*
and *Plug-ins*.

## 12. Keyboard shortcuts

*Help ▸ Keyboard shortcuts* lists every shortcut and lets you change them, or load the
FL Studio, Ableton Live or Bitwig Studio presets. The defaults:

| Keys | Action |
|---|---|
| `Space` | Play / pause (also stops a recording) |
| `R` | Record |
| `Home` | Go to start |
| `Ctrl+L` · `Ctrl+M` | Loop on/off · metronome on/off |
| `L` · `Shift+T` | Pattern / song mode · tap the tempo |
| `Alt+R` · `Alt+C` · `Alt+P` · `Alt+T` | Arm the selected track · count-in · punch · show takes |
| `Ctrl+Shift+A` | Add an audio track |
| `Ctrl+N` · `Ctrl+O` · `Ctrl+S` · `Ctrl+Shift+S` | New · open · save · save as |
| `Ctrl+Shift+N` | Demo songs |
| `Ctrl+Shift+E` | Export audio |
| `Ctrl+Z` · `Ctrl+Y` | Undo · redo |
| `Ctrl+X` · `Ctrl+C` · `Ctrl+V` · `Ctrl+D` | Cut · copy · paste · duplicate |
| `Ctrl+A` · `Delete` · `Esc` | Select all · delete · clear the selection |
| `Ctrl+E` · `Ctrl+J` · `0` · `F2` | Split clip · glue · mute clip · rename |
| `Left` `Right` (`Shift`: a bar) | Nudge |
| `Up` `Down` (`Shift`: an octave) | Transpose notes |
| `Q` | Quantize |
| `P` `B` `E` `D` `T` `C` `Y` or `1`–`7` | Piano roll tools; in the arrangement, `B` or `2` is the paint tool |
| `Shift+L` · `Z` | Loop the time selection · zoom to it |
| `Alt+A` | Automate the last parameter you moved |
| `F5` `F6` `F7` `F9` · `Alt+F8` · `I` | Arrangement, channel rack, piano roll, mixer · browser · inspector |
| `Alt+F5` | Clip browser |
| `F8` · `F12` | The selected track's instrument · close floating devices |
| `M` · `S` · `Alt+Left`/`Alt+Right` | Mixer: mute · solo · move the strip |
| `Alt+F9` · `Shift+F10` | The routing matrix · the selected strip's route menu |
| `Alt+U` · `Alt+L` · `Shift+Alt+L` · `Alt+N` | Make unique · link the selection to this clip · select linked copies · link to… |
| `Ctrl+T` | Typing keyboard plays notes |
| `Ctrl+Plus` · `Ctrl+Minus` · `Ctrl+0` | Interface zoom |

## 13. Privacy and updates

Cantus has **no telemetry**. An installed or portable copy asks GitHub once a day whether
a newer version is out — an anonymous request that carries no version, machine or user
information — and asks you before downloading anything; *Settings ▸ Updates ▸ Check daily*
turns it off. Sound packs are downloaded only when you click *Install*. Everything else
stays on your computer. The details are in [PRIVACY.md](../../../PRIVACY.md); the licences
of everything Cantus is built from are in
[THIRD-PARTY-NOTICES.md](../../../THIRD-PARTY-NOTICES.md) (also in *Help ▸ About Cantus*).

## 14. Known issues and limitations

This is a beta. What we know about:

- **A plug-in made by someone else that crashes closes Cantus** (plug-ins run inside the
  program; isolating them comes later). Save often.
- **MP3 files cannot be imported** (WAV, AIFF, FLAC and Ogg can).
- **No time-stretch**: audio clips keep their speed when the tempo changes; one tempo and
  one time signature per project. No crossfades or clip gain envelopes.
- The parameters of hosted plug-ins cannot be automated yet; automation is drawn, not
  recorded (no latch or touch). In stems exported *through the master chain*, a hosted
  effect on the master is left out of the stems (the full mix has it).
- Splitting a **looping** clip silences its right half: select the time range you need,
  copy it and paste it instead (the pasted part keeps the loop).
- Automation clips are pasted only within the project they came from, while Cantus stays
  open; a first save or a *Save As* that copies imported audio into the project folder can
  pause the window while the files are copied; Ctrl+V with nothing
  copied does nothing and says nothing.
- **Lamina** plays a little quieter than the other instruments (about 4 dB) at the same
  fader, and the **Salamander Grand Piano** about 5 dB quieter.
- The **Salamander's first load** decodes its samples into the sample cache (1–2 GB on
  disk; about 10 s), beside the window; a note played before its samples
  arrive is silent. Later loads are quick.
- Names in other scripts use your system's fonts (read when first needed, never shipped
  with Cantus). **Arabic and Hebrew show left to right with unjoined letters**, and scripts
  that join or reorder letters (Devanagari, Thai…) show them without that: the toolkit
  does not shape text yet.
- **Audio**: the built-in Realtek ASIO driver delivers no sound on some laptops (use
  WASAPI or FlexASIO); ASIO4ALL can take several seconds to start; many ASIO drivers set
  the buffer in their own control panel; plug an interface in *before* starting Cantus.
  Exclusive-mode WASAPI is not offered.
- **Performance**: dense projects at small buffers can drop out on modest computers; raise
  the buffer if the status bar counts dropouts.
- The audio device cannot be changed while recording; the sample browser has no keyboard navigation yet; projects refer to
  sound-pack samples by their full path.
- **Not in this beta**: macOS and Linux, warping, MIDI learn and control surfaces, AI
  features, several arrangements per project.

## 15. Report a bug

*Help ▸ Report a bug…* opens the bug form on GitHub (you need a free GitHub account). Say
what you did, what you expected and what happened; give your Cantus version (*Help ▸
About Cantus*), Windows version, audio interface, driver, rate and buffer (all in the
audio chip); attach a small project that shows the problem if you can. Tell us about your
audio interface even when it works, with the
[hardware report](https://github.com/luis314159/cantus/issues/new?template=hardware_report.yml),
and send ideas with *Help ▸ Suggest a feature…*. English or Spanish, both are welcome.

*Cantus is made by Luis Ángel Almazán — [lalmazan.com](https://lalmazan.com) — and is free
software under the GNU GPL, version 3 or later.*
