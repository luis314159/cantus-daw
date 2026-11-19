# Bitwig Studio — why it is called easy, mechanism by mechanism (2026-11)

*Research only, 2026-11-19, for BACKLOG §6l. Bitwig Studio 6.0 shipped 2026-03-11, 6.1 (the
sampler) in June–August 2026; the online user guide still documents 5.3, so 6.x keys and
gestures come from Bitwig's 6.0 release notes (read whole) and reviews. The look is in
`bitwig-studio.md` beside this file; this note is about use.*

## 1. The interaction layer

- **Inspector + footer.** The Inspector (left) shows whatever was last selected, with group
  editing for many objects (a histogram with Mean, Scale and Chaos; 6.0 stripes values
  that differ). The **footer** shows, for what is under the mouse, its name and value, the
  value after modulation in brackets (`39.5 % [27.1 %]`), and every gesture available
  (CLICK, SHIFT+CLICK, ALT-drag…), updated during a drag. Reviewers: Bitwig works "by drag
  and drop much more than menus and dialogue boxes" (Sound On Sound).
- **Panels by one key:** I Inspector, E Detail Editor, D Device, M Mixer, Alt+B Browser
  (5.x: A Automation Editor; 6.0: A is Automation Mode). Fixed slots: Inspector left,
  access panels right, a secondary panel below. Views Arrange / Mix / Edit (Shift+Tab is
  Edit in 6.0). Shortcuts act on the focused panel (silver outline); in 6.0 tool and grid
  keys act on the editor under the mouse. Panel toggles work in the middle of a drag.
- **Display profiles** (right-click the window header): Single Large/Small, Tablet, four
  dual-screen layouts, Triple.
- **Tablet, touch, pen:** a PLAY view with a multitouch keyboard (sideways micro-pitch,
  vertical timbre, force pressure), a radial gesture menu, one-touch buttons; 6.0: touch
  follows the tool, two-finger tap-drag opens a tool menu.
- **Pop-up browser:** a **+** before the first device and after every device (and in the
  mixer, inspector, modulator slots) opens it with the focus in search. Down to results;
  Tab accepts an autocompleted tag; filters by key (C category/creator, L location, T tags,
  F file kind, D device, V vendor, * favourites, X clear); F1–F9 quick sources; 0 favourite,
  1–9 collections; → a device's presets, ← back; live preview while arrowing, audio
  stretched to the project's tempo; saved searches and smart collections.
- **Tools:** 1 pointer, 2 time selection, 3 pencil, 4 spray, 5 knife, 6 eraser, 7 audition,
  8 step input; a short press switches, **holding switches until release**; right-click
  offers a tool switcher; the pointer is a smart tool. Since 5.2, ←/→ jump to the next point
  of interest (note edges, onsets, fades, loop edges), Shift extends, Alt moves by grid.
- **Universal conventions:** double-click resets, Ctrl+click types a value, Shift+drag
  fine. Shortcuts bound by pressing a key or moving a MIDI control.
- **Defaults:** sample rate and buffer "Automatic"; scripted controllers detected and added;
  Quick Start templates; new clips open in the editor (6.0).

## 2. Power that stays approachable

- **Device chains:** drag a header to move, Ctrl-drag to copy, drop on a device to replace,
  **Shift-drag onto another to make a layer of both**, double-click to collapse. Containers:
  Instrument/FX/Note FX Layer and Selector (round robin possible), Drum Machine (128 chains,
  choke groups), Mid/Side, Stereo Split, Multiband; chains inside devices (Pre, Post, Wet,
  FB — an effect inside a delay's feedback). Conversions by right-click (Layer ↔ Selector,
  Polymer → Poly Grid…).
- **Modulation (the most-cited reason to switch):** every device, plug-ins included, has a
  modulator section; click the modulator's routing button, click a target, drag the depth
  (it may exceed the range); Alt-drag copies a routing, Shift-click bypasses. Transfer
  shapes per routing; any source can scale a depth. **Per-voice** (green, one signal per
  note) or mono (blue), switched by right-click; CLAP plug-ins receive polyphonic
  modulation; voice stacking up to 16 per note. Modulators on a track, group or master
  reach everything below, tempo and (6.0) the project key included. **Remote controls:**
  pages of 8 knobs per preset, device type, track or project, assigned by click,
  followed by controllers, themselves modulation sources. MSEG editors since 5.0.
- **The Grid:** Poly/FX/Note Grid; drop a module on a cable or port and it connects to the
  most fitting port; delete one and the cable heals; replace by dropping on its centre;
  **pre-cords** (gate → envelope, pitch → oscillator) so a blank patch plays; F1 help;
  scopes per port; direct feedback loops removed unless through a long delay. Still called
  a "substantial" learning curve.
- **Launcher and arranger together:** the launcher inside the arranger on the same tracks,
  per-track Stop and *back to arranger*; **hybrid tracks** hold audio and note clips; 6.0
  progress rings with a countdown to the next action.
- **Clip aliases (6.0):** Ctrl+Alt+V paste as alias, Ctrl+Alt+D duplicate as alias, Alt-drag;
  a shared pattern (content, name, colour), per-clip start/length/loop; a pattern chooser in
  the Inspector; *Make Unique*; *Merge Duplicate Patterns* turns copies into aliases. One
  track only; a small icon.
- **Comping:** takes inside the audio clip; click a region to make it active, arrows to
  audition, drag to split; *Fold to Takes*. No MIDI comping.
- **Operators per event:** Chance (dice, seeded), Repeats (2–128 or 1/2–1/128, a curve and a
  velocity end), Occurrence (first / not first, with/without previous, Fill on/off with a
  global Fill button), Recurrence (1–8 cycle, a toggle per pass); spread on velocity and
  expression points, re-drawn each pass.
- **Per-note expression:** velocity, release, chance, gain, pan, timbre, pressure,
  micro-pitch ±24 st; 6.0 **expression overlay** (X; P pitch, G gain; axis range by
  right-click); audio events get gain, pan, pitch ±96 st, formant, with spread; several
  clips edited together; a waveform behind the piano roll; a drum/hybrid editor.
- **Audio in clips:** edges for gain, slide and fades; onsets on import; beat markers
  (double-click, drag to warp); Alt-drag to stretch; knife repeats with Alt; Ctrl+J
  consolidate. **6.1 drop gestures:** one file → Sampler (Alt spectral, Ctrl sliced);
  several → round robin (Alt Drum Machine, Ctrl tuned multisample).
- **Plug-in hosting modes:** within Bitwig, together, by manufacturer, by plug-in, each
  instance; overrides per plug-in; on a crash the device shows a notice while playback goes
  on, with *Reload plug-in*. Plug-in parameter changes in undo since 5.2.
- **Deactivate** a track or device (Alt+A): no CPU or latency, content kept.
- **Projects in tabs**, items dragged between them; one sounds at a time.

## 3. 5.x–6.x workflow additions

5.0 browser rebuilt (sources, quick sources, key filters, smart collections), MSEGs,
pop-out editors, project remotes; 5.2 point-of-interest navigation, plug-in undo, a GPU
renderer; 5.3 master recording, bounce in place (pre-FX/pre/post fader), an audio-device
overhaul (hot switching), Stepwise; 6.0 automation clips (loop, stretch, library, Ctrl+G /
Shift+Ctrl+G), Automation Mode and a flying lane, hold points, a smoothing pencil, a
**project key** (23 scales, snap and quantise to key, note FX following it), a tool
palette, spray/audition/step tools, editor history (10 states), group meta clips,
scrollbars with an overview, a permanent backup of projects from older versions; 6.1 the
sampler (slicing, pitch analysis, Spectral, Fragments), a tuner, Scala files, bounce while
dragging (B).

## 4. What users complain about

No ARA and no pitch correction; no video, MIDI comping, retrospective capture, project
versions or stem separation; swing and groove "underwhelming"; small stock content (about
10 GB against Ableton Suite's ~70 GB); a "basic" piano roll next to FL's (ghost notes,
chord stamps and strum built in); a steep curve in the Grid and modulation (the basic
workflow is called easy); 6.0 aliases one track only.

## 5. Not verified

Default keys for Arrange/Mix (only Shift+Tab for Edit), a default key for "insert after the
selected device", an undo-history panel (none found), pen pressure and tablet hardware
lists, exact 6.1.x dates.

## Sources

- bitwig.com/userguide/latest (5.3): the window footer, the window body, browsers, the
  unified modulation system, advanced device concepts, working with devices, the Grid,
  special connections, tracks, the clip launcher, note and audio events, operators, plug-in
  handling, MIDI controllers, the dashboard, multiple projects, tablet computers.
- downloads.bitwig.com/6.0/Release-Notes-6.0.pdf; /6.1/Release-Notes-6.1.html;
  /5.2/Release-Notes-5.2.html; /5.0/Release-Notes-5.0.html.
- soundonsound.com/reviews/bitwig-studio-6 and /bitwig-studio-4;
  musicradar.com/music-tech/daws/bitwig-studio-6-review; rekkerd.org/review-bitwig-studio-6;
  synthanatomy.com/2026/03/bitwig-studio-6.html; create.routenote.com/blog/bitwig-studio-6-is-out-now.
- KVR threads (Bitwig vs Live, piano roll, feature gaps); audeobox.com (Ableton vs Bitwig).
