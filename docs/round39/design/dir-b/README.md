# Round 39 · direction `dir-b` — Graphite and gold

## The idea

A neutral graphite workspace (every grey has R = G = B + 1 at most: neither the brown-black
that made Night read muddy, nor FL's blue slate) lit by Cantus's own metal, taken from the
logo. **Gold** comes from its playhead and means *on, selected, focus, the playhead, the
loop*. **Teal** comes from its patina and means *signal, healthy, an effect on*. **Ember**
comes from its wave and means *warning*; red is kept for record and clip. Nothing else is
coloured. The user's track colours are muted in clip bodies, strip tints and headers, and lit
only in a 3-px stripe and in the clip's content, so the music shows and the chrome rests.
The type is one humanist family in three widths (Sofia Sans, OFL): a large, thin Extra
Condensed clock as the window's focal point, Condensed names that fit a 62-px strip whole,
and a Semi Condensed body. Keys are dark wells: a 1-px rim, a light lip at the bottom, no
gradient and no shadow. Glyphs sit about a third of the key (transport) or half (tools), and
are drawn in ink-2 with a 1-px ink-1 edge where light falls from above. That is one engraved
family, drawn again for each pixel size.

## Why it beats FL, and today's Cantus

- **Calmer than FL at rest, richer when playing.** No −∞ anywhere: a quiet meter is a ghost
  scale; the master remembers the last pass's peak, dimmed, with LUFS as a dash. Playing
  lights the meters teal to gold to red in the fader's own column, with peak lines and
  readouts.
- **Words where words help, drawing where drawing is clearer.** There is no label column, no
  "Slot 3…10", no capital section bars. Inserts are listed only when they exist, with one
  *Add an effect* row and "8 free". Each strip shows its inserts ("Comp · EQ"), its output
  only when it is not the master ("→ Drums"), and its pan value only when off centre. The
  EQ is drawn as a curve, with the selected band in gold.
- **Clip content reads from across the room.** Notes at their pitch, chords as blocks, drums
  as rhythm rows, audio as filled waveforms, all in the clip's own hue. Each clip's name
  appears once, in a name band.
- **A focal point.** The clock comes first, then the selected track, which carries one gold
  thread through the arrangement header, the clip, the mixer strip and the inspector.
- **Better than FL where FL is weak:** every value has its unit, contrast is measured (WCAG
  ratios on the system page), the hint line is never empty, and Paper is a finished light
  theme, not a hue shift.
- **Clearly Cantus**, not an FL copy: a different base, different accents, a different face,
  a different key construction and a different icon grammar. No FL shape, layout or colour
  value is reproduced.

## The overflow panel, designed again

There are no boxes inside boxes. Each section has a title in sentence case with a hairline
above it, and the controls sit on a two-column grid (labels on the left, controls aligned
on the right).

- **Metronome:** its key (gold when on), the sound as a field, Accent as a toggle, Count-in as
  a segmented control, Level as a slider with "−6.0 dB". The shortcut is a small keycap at
  the right of the title.
- **Windows:** seven keys, each with its own shortcut under it (Alt+F8, F5, F6, F7, F9, Tab,
  I). Keys are lit gold only when that window is open, never orange at rest.
- **System:** CPU and memory as large thin numbers with a 30-second graph beside each, then
  voices (with the peak) and dropouts (with a teal "healthy" dot).
- **Customise top bar…:** a quiet action at the foot, beside "or right-click the bar".

## What it costs to build

- **Fonts:** Sofia Sans v4.101 (four variable TTFs, about 345 KB each; OFL 1.1, © 2019 The
  Sofia Sans Project Authors; https://github.com/lettersoup/Sofia-Sans via
  https://github.com/google/fonts/tree/main/ofl/sofiasans*). The kit needs either a static
  instance per weight used (200, 300, 500, 600, 650) or variable-font support in its text
  path, plus tabular figures (`tnum`). The kit builder adds the notice (`xtask notices`).
  The files are at `F:\projects\daw-r39\fonts\sofia\` (not in the repository).
- **Cheap for the kit:** flat fills, 1-px rims and lips, one gradient (the meters), and a
  soft shadow only under popovers and fader caps. There are no per-frame effects, so the
  frame budget is unchanged.
- **Real work:**
  - Every glyph drawn per pixel size, with snapping and small-size variants. The generator
    holds the rules (`src/glyphs*.py`), but the kit needs these drawings, not one scaled
    path.
  - A text measurer for the three widths.
  - Clip-content painters (pitch rows, drum rows, filled waveforms).
  - The idle-meter and "last peak" states.
  - The overflow panel's layout.

## How it is made

Everything is drawn by `src/*.py` into SVG in physical pixels, snapped to the grid at each
scale, and rendered with Edge headless. To rebuild:

- `python window.py`: the windows.
- `python parts_sheet.py`: the parts sheets.
- `python system.py`: the system page.
- `python compare.py`: the private comparison sheet.

Text widths come from the fonts' own tables (`src/metrics.py`), so labels are placed by
measurement. The 4× views are the real pixels magnified, not vectors.

## Files

| File | What |
|---|---|
| `window-night-1x.png`, `window-paper-1x.png` | the whole window at rest, 1280 × 720 |
| `window-night-1.5x.png`, `window-paper-1.5x.png` | the same at 1920 × 1080, drawn for 1.5× |
| `window-night-playing-1x.png`, `window-paper-playing-1x.png` | the window playing: meters, peaks, LUFS, pause lit |
| `overflow-{night,paper}-{1x,1.5x}.png` | the top bar's overflow panel open |
| `parts-night.png`, `parts-paper.png` | keys at 16/20/24 px and 150 % (24/30/36), states, knob, fader + meter, strip, tab, field, segmented, toggle, scrollbar, menu + submenu; 1:1 and 4× |
| `system.png` | tokens with hex values and contrast, type scale, spacing, radii, lines, density, button states, both themes |
| `src/` | the generator (no Rust) |
| `research/ui/round39/dir-b/compare-fl.png` | private: beside FL's playlist and mixer and beta.13, whole windows and same-scale crops |

## Open — for the owner's eyes

- Whether gold for "on" plus teal for "signal" reads clearly enough on Paper, where gold is
  a deep ochre (#7C5307) to keep 4.5:1.
- Whether the track headers' second line (the instrument) helps or crowds.
- The empty strips (11–13) are drawn as quiet "+" places. Hiding them is the alternative.
- Only these views are designed: the main window, the overflow panel and the parts. The
  piano roll, channel rack, browser, editors and dialogs follow from this system once it is
  chosen.
