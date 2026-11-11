# Studio ink — direction C, round 39

**The idea.** The chrome is ink and the music is light. In Night, every surface the user
does not play with is a near-black with a cold breath of blue-green (never brown), set in
three inks and one-pixel hairlines, so it nearly vanishes; what glows is the content: the
notes, hits and waveforms in their clips, the meters, the EQ curve. Each takes the user's own
hue, lifted toward white on a dim body of the same hue. In Paper the metaphor turns over:
the ground is paper and the content is printed in ink, the same hues taken deep on pale
tints of themselves. In both themes colour means state and nothing else (on, playing,
recording, solo, clipping). One pen, a calm azure, marks what is on or selected. The type is
technical and condensed: IBM Plex Sans Condensed, with engraved capital captions only for
units, a large ExtraLight clock, and Plex Mono for every value that changes. Glyphs take
about a third of their key and are drawn separately for each pixel size.

**Why it beats FL Studio and today's Cantus.**
- *Focal point.* The arrangement's content is the brightest thing on screen and the chrome
  the darkest, so the eye goes to the music. FL spreads the same mid-blue slate everywhere.
  Today's Cantus fills headers, strips and clips with saturated slabs (§6j items 1, 8).
- *The music reads from across the room.* The clips draw *Neon Tide*'s real notes (from
  `demos/house.rs`). You can see the kick's four on the floor, the clap on 2 and 4, the hats
  as two pads, the bass jumping its octave, the chords as stacked voicings, the hook, the
  lead, and the choir as a frozen waveform pumped by the kick. FL's clips are tinted bars,
  and ours were barcode dashes (§6j item 7).
- *Calm at rest, alive when playing.* At rest there is no "−∞" and no black slab. A meter at
  rest is a thin slot. The top bar keeps the last play's loudness in ink-3 ("last −9.4
  LUFS"). A centred pan says so by its detent tick, and only an offset is written. Playing
  lights the meters and each header's 2 px level (`window-*-playing-100.png`) (§6j item 5).
- *Words where they help, drawing where it is clearer.* There is no label column and no
  "Slot 3…10". Section heads are sentence-case words on a hairline. The only route drawn in
  the mixer is the one that is not the default: a bracket from the four drum strips into the
  bus. Settings that are rarely changed fold to one line that still states its value
  (§6j item 3).
- *Keys are wells, not glossy caps.* Keys are flat, the glyph is light grey with a second
  tone, and the glyph is drawn per size: 16/20/24 px and their 150 % (24/30/36). There is one
  gradient in the whole kit, the faint top light on a fader cap and a knob body, which makes
  them read as objects (§6j item 4, the button study).
- *Better than FL, not like it.* Numbers sit where they are read: gains in Mono under every
  fader, pan offsets, and the inspector in label/value units. FL shows almost none. The top
  bar's system meters live in the overflow as small graphs with their numbers.

**The overflow panel** (`window-*-overflow-*.png`), drawn again from a blank sheet. It is one
floating sheet with three groups and no boxes inside it:
- *Metronome* is its own lit key, with the shortcut on the right (Ctrl+M).
- Count-in, Level, Sound and Accent sit on one label edge and one value edge, as segmented
  controls, a slider with its dB value, and a switch.
- *Windows* is seven keys in one row, lit when that window is open, with each shortcut
  centred under its own key (Alt+F8, F5, F6, F7, F9, F8, I).
- *Performance* shows CPU, memory, voices and dropouts as small graphs of the last minute,
  each with its number.
- *Customise the top bar…* is a quiet last line with the count of folded groups.

**What it costs to build** (for the kit builder, ADR 0108):
- Mostly flat fills and 1-physical-pixel hairlines, which egui draws cheaply.
- Snapping to the physical pixel grid is needed at every scale. The sheets do it:
  `Svg.sn`, and lines drawn as filled rects.
- Glyphs are drawn per size. That means a table per glyph and size (as in
  `src/glyphs.py`), not one path scaled.
- One vertical gradient on caps and knob bodies, a mesh of 2 colours.
- Clip content needs the clip's notes per lane at draw time. It should be cached per clip
  and zoom, because drawing every hat hit each frame would cost frames. The frozen
  waveform is an envelope per column, from the freeze cache.
- Two fonts must be added and their notices recorded: IBM Plex Sans Condensed and IBM Plex
  Mono, both OFL. Tabular figures are needed.
- The overflow panel's shadow is a soft blur, which needs a pre-rendered nine-slice texture
  or egui's shadow. Nothing else uses a blur.

**Not done or open.**
- The Cantus mark in the top bar is a stand-in tile. The owner's logo goes there unchanged.
- The glyphs here are the direction's. The `icons` agent owns the family, and these should
  give way to it.
- Only the main window and the overflow panel are drawn. The piano roll, channel rack,
  browser, editors and dialogs follow from the kit once the owner chooses.

## Fonts

| Family | Version | Licence | Source |
|---|---|---|---|
| IBM Plex Sans Condensed (ExtraLight, Light, Regular, Medium, SemiBold) | 1.3 | SIL OFL 1.1, © IBM Corp., Reserved Font Name "Plex" | https://github.com/google/fonts/tree/main/ofl/ibmplexsanscondensed (IBM/plex) |
| IBM Plex Mono (Light, Regular, Medium) | 2.3 | SIL OFL 1.1 | https://github.com/google/fonts/tree/main/ofl/ibmplexmono |

Downloaded to `F:\projects\daw-r39\fonts\ibm-plex\` (each file checked: TrueType `00010000`
header, with `OFL.txt` beside them). Nothing is shipped until the kit builder adds them with
their notice.

## Files

| File | What |
|---|---|
| `window-night-100.png`, `window-night-150.png` | the main window, Night, 1280×720 and 1920×1080 |
| `window-paper-100.png`, `window-paper-150.png` | the main window, Paper |
| `window-night-overflow-100/150.png`, `window-paper-overflow-100/150.png` | the same with the top bar's overflow panel open |
| `window-night-playing-100.png`, `window-paper-playing-100.png` | playing: meters, levels, the playhead, the clock moving |
| `parts-night-100.png`, `parts-paper-100.png` | the parts sheet at 1:1. Keys at 16/20/24 px and 150 % (rest, then on); knob rest/hover/dragged/automated; fader and meter silent/playing/clipping; the mixer strip; tab, fields (rest/hover/focus/disabled), search, segmented control, toggle off/on, scroll bar rest/hover/dragged, a menu with its colour submenu. Every part at 100 % and 150 % |
| `parts-*-keys-4x.png`, `parts-*-controls-4x.png` | the same sheet magnified 4× pixel for pixel, so exactly what would ship is judged |
| `system.png` | the system: every token with hex and contrast against its surface, both themes; the user's colour by role; a key's five states; the type scale; grid, corners, lines, density |
| `src/*.py` | the generator: `python -I src/window.py`, `src/parts.py`, `src/system.py`, `src/compare.py` (Edge headless renders, see BUILDING.md) |
| `html/*.html` | the rendered sources (SVG in HTML; fonts loaded from `F:\projects\daw-r39\fonts`) |
| `research/ui/round39/dir-c/compare.png` (private) | Studio ink Night and Paper at 150 % beside FL's playlist and mixer and Cantus beta.13, each at its physical pixels; `compare-overview.png` is the same at a third |
| `research/ui/round39/dir-c/compare-detail.png` (private) | transport, mixer strips and clips cut at the same physical scale, magnified 2× |
