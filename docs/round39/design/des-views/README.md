# des-views: the piano roll, channel rack, browser and clip content in Slate (round 39, Phase B)

**The idea.** The other main views in Slate's system (dir-a: a cool grey-blue room, panels
separated by value, one azure accent spent on state, the user's colours as a stripe and a tint,
lit only when selected). Everything is drawn here from a blank sheet. The owner's open item since
round 37 is the piano roll's keyboard, and it is drawn here as an instrument. It has ivory and
ebony keys in both themes and a real piano's widths: an octave's twelve rows are shared by seven
white keys, C-D-E taking 5 rows and F-G-A-B taking 7, so only E|F and B|C fall on row lines. The
black keys are one row high and 0.60 of the keyboard's depth, which makes them about 0.58 of a
white key's width. Every C is named in its own key.

## Why it beats FL Studio and today's Cantus

- **Piano roll.** Today's keys are cream, with glossy black slabs. Here they are calm, and
  lit from the player's side. Each white key is its own piece, with a 1-px gap between keys
  (round-trip with the owner, 2026-11-09: "at 100 % it read flat"). The black keys lift off
  them with a light edge on the top and front and a soft shadow onto the whites. The key under the pointer, the key heard by a
  click and the keys sounding while playing each have their own tint. Notes carry their name,
  a brighter attack column (so repeated notes part) and their velocity as a little brightness.
  The selection is lit with an edge. Muted notes are hollow. Ghost notes come in two kinds:
  the same track's other clip in its own hue, faint, and other tracks in neutral slate. The head
  names which ghosts are shown ("Pads · Bass · Lead"). FL has ghosts but does not say whose they
  are. Outside the edited clip, the grid sits under a veil, and the ruler marks the clip's span
  in its colour. The velocity lane gives one stem a note in the note's colour, with a short
  cap. The selection is lit there too, and chords share one clean cap.
- **Channel rack.** Each step shows its velocity as a level inside the step. Beats are told
  apart by value, not by colour. A kit's pads sit indented under it, joined by a tree line, and
  the kit's own row sums their hits. A melodic channel shows its pattern as a small roll and an
  audio channel shows its waveform, both on the same step grid. A map in the head shows the
  pattern's eight bars, with the page in view lifted. The foot holds "Add a channel" and a drop
  zone for sounds from the browser. Today's rack packs 128 steps into stripes that cannot be
  read; this one shows two bars a page.
- **Browser.** Each pack has a cover made from what it holds (drum rings, a kick's sweep,
  vowels, a keyboard, a square wave, contour lines, an impulse response, a groove). The covers
  are drawn per size, at 14, 40 and 128 px. Every sound row has a thumbnail of its own waveform.
  Tree guides are hairlines. The preview shows the waveform with the part already heard lit, and
  says what the sound is in numbers. In a search, the match is marked in the accent within one
  text run, and results are grouped by kind with filter chips. Cantus units and hosted plug-ins
  are told apart by a filled or outlined mark, with the format written in words. Packs show their
  state: installed, update, getting (with a progress bar and Pause) or get, with the size.
- **Clip content.** Each zoom has a rule (below). MIDI is a pitch contour from far away and
  ranked rows at today's zoom. Closer in, notes are drawn at their true length with an attack
  column. Drums get one row a piece, and the hats sit back so kick and clap carry the beat.
  Audio is loudness from far away and a textured peak envelope at today's zoom. Closer in it is
  drawn as peak and loudness in two tones, with stereo channels apart. Automation is a line over
  a faint fill, with points, then handles, then curve handles and values as the zoom grows.
  Today's MIDI reads as barcode dashes and FL's as thin lines. Here, the selected track's clips
  are lit.

## Measured spec for the kit builder (logical px; 150 % multiplies by 1.5)

Tokens are Slate's (`dir-a/README.md`, `src/base.py` here is a copy of its `d280cd13`). New
tokens are marked **new**.

### Piano roll (`src/pianoroll.py`, `src/keys.py`)
| Part | Measure |
|---|---|
| Head | 28 high, `surface`, lit top edge. Title Medium 13. Clip menu: a well of 128 × 20 holding the track stripe (3 × 8, r 1), track name Medium 12, `›`, clip name in ink 2. Tools: a joined well of 6 × 26 × 20; the chosen tool is a `raised` pill inset 2. Grid well 54 × 20. Snap key 26 × 20, on. Word toggles ("Ghosts", "Scale"): a well with a 2.5-r lamp (accent when on, ink 3 at 55 % when off) and the word in 11, followed by what they show in ink 3. The selection readout sits at the right: count in ink 1 Medium 11, range in ink 3. |
| Ruler | 24 high, two tiers. Upper tier (y 2–12): the loop band (accent 13 % over `surface`, a 1-px accent start) and the section name in SemiBold 9. Lower tier: bar numbers SemiBold 10 in ink 1 with a 9-px tick; beats `41.2` Regular 9 in ink 3 with a 6-px tick; sixteenths as 3-px ticks. Clip span: 2 px at the foot in the clip's hue (75 % Night, 85 % Paper). Playhead: a 9-px accent pentagon. |
| Keyboard | 72 wide at 12-px rows; 56 at 8; 88 at 18. Drawn on a bed of `sep` **new** (Night `#59636E`, Paper `#A3ADB8`). Each white key is a piece of its own in `ivory` **new** (`#CFD5DB` / `#FBFCFD`): a **1-px gap** under it where the bed shows, its front corners rounded (r 2; 1.5 under 10-px rows), a 1-px lit top `ivory_hi` (`#E2E6EA` / `#FFFFFF`), a 1-px shade at its foot (ivory→`sep` 28 %). Depth: a horizontal gradient over all keys: black 20 % → 7 % over the first 10 % (back), 0 by 55 %, white 10 % at 92 % (Night only), black 6 % at the front edge. Black keys: 0.60 × width, one row high, r 1.5, edge `ebony` (`#161A1F` / `#262C33`), body gradient back→front `ebony` → `ebony_top` (`#262D34` / `#39414A`) at 62 % → `ebony_front` (`#3A434C` / `#58626C`). **A light edge** of 1 px along the upper side and the front end (`ebony_front` + 12 % white Night, + 18 % Paper). **A shadow onto the white keys**: two soft steps, (+0, +0.5, width + 1, row + 0.5) at 26 % / 18 % and (+0, +1, width + 2, row + 1) at 45 % of that. Bed: 1 px `edge` toward the grid. |
| Key labels | Every C in its key, right-aligned 5 from the front, centred on the key: C4 (middle C) SemiBold 10 `#17202A`; other Cs Medium 10 `#4A5664` (Night) or `#6A7684` (Paper). From 16-px rows, every white key is named in Regular 11, quieter. |
| Key states | Pointer: accent 16 % (white keys) or 30 % (black). Heard by a click: accent 55 % / 70 %. Sounding: the track's colour 55 % / 75 %. The note's row gets the accent at 7 % across the grid. |
| Grid | Rows of 12 (8 compact, 18 tall). White-key rows `base`; black-key rows `base`→`gutter` 62 % (Night) or 30 % (Paper). The octave line under each C: `grid_bar`→`ink4` 35 %. E\|F: `base`→`grid_bar` 55 %. Bars: `grid_bar`→`ink4` 45 %. Beats: `grid_bar`. Snap lines: `grid_beat`, only when they are 6 px or more apart. Outside the clip: a `gutter` veil at 55 % (Night) or 42 % (Paper). |
| Notes | The row's full height. A 1-px edge (fill→`gutter` 55 % Night, 35 % Paper), r 2. Fill: `base`→hue at 50 % + 22 % × velocity (Night), or 58 % + 30 % × velocity with the hue darkened 38 % (Paper). The first column is brighter (Night: +35 % white; Paper: −18 %). Name in SemiBold 9, `#0B1A20` (Night) or white (Paper), when the note is 20 wide or more. Selected: Night fill hue + 30 % white, Paper full hue, with a 1-px `ink1` ring outside. Under the pointer: a 1-px ring of the fill + 60 % white. Muted: hollow, a 1-px edge at hue 55 %, fill hue 10 %, its name in ink 3. Ghosts: inset 1 top and bottom, r 2, no edge or name. The track's other clip is drawn in its hue at 20 % (Night) or 22 % (Paper); other tracks in `ink3` at 24 % / 22 %. |
| Velocity lane | 106 high under a 6-px splitter (`gutter`, five 2-px grip dots) and an 8-px horizontal scroll. Head: "Velocity" Medium 11 with a chevron (the lane's menu); `127` and `64` Regular 9 in ink 3 at their lines. Stems: 1 px (2 px when selected) in the note's fill. Cap: 2 high, stem width + 5, r 1; the selection's cap is `ink1`. Muted: a dotted stem and a hollow cap. |
| Status | Under the pointer: what (ink 1), then how (ink 3). |

### Channel rack (`src/rack.py`, `src/rack_panel.py`)
| Part | Measure |
|---|---|
| Row | 30 high (a kit's pad: 26), `surface`, 1-px `base` gap. Stripe 3 px (hue at 72 %; full hue when selected). Selected row: `surface`→hue 10 % (Night) or 12 % (Paper). |
| Controls | Lamp at x 16: r 2.8 in `play` with an 18 % halo, or a 1.2-px ring in `ink4` when off. Pan knob at 36 and volume at 58: r 6.5 with a `kcap` gradient, a 1-px `edge` ring, an arc at r + 2.6 in `ink3` 1.2 (pan from centre), and a 1.4-px `ink1` pointer from 25 % to 85 % of r. Mixer track: a well of 24 × 18, Medium 11 in ink 2. |
| Name plate | x 104, 150 × 20, r 3, `raised` (Night) or `raised`→`surface` 20 % (Paper), a 1-px foot of plate→`edge` 50 %. Selected: plate→hue 22 % / 20 %, the name Medium. A kit's pads are indented 14, with a 1-px tree line at x 110. Kit plate: a chevron, the name Medium, "3 pads" in ink 3 10. Target mark at x 262: 3 × 14, r 1.5, accent when selected, `ink4` at 35 % otherwise. |
| Steps | 18 × 22 at a pitch of 20, +4 between beats, +12 between bars (two bars = 676). A well with a 1-px edge, r 3. Beats 2 and 4 of each bar: `well`→`surface` 75 % (Night) or `well`→`edge` 35 % (Paper). On: the step filled with hue 42 % (the room), then from the floor to 45 % + 55 % × velocity with hue 80 % (86 % selected), a 1-px lit top (+35 % white Night, +25 % Paper). Muted channel: 30 % and 18 %. |
| Melodic / audio rows | One well over the page. Beat lines `well`→`ink4` 30 % (60 % at the bar). Notes 3 high, r 1, pitch-ranked, hue 78 % (95 % selected). Audio: one column a pixel, hue 72 %. |
| Head | Pattern and Show wells (96 and 92 wide, with the label in ink 3 10 and the value Medium/Regular 12). Swing: a knob and a value. The pattern's map: a well of 8 × 22, the page in view a `raised` pill, density as 2-px bars. |
| Foot | "Add a channel" in a 150 × 24 well. Drop zone: dashed 4/3 in `ink4` 90 %, r 6, 56 high, two lines of text in ink 3 / ink 4. |

### Browser (`src/browser.py`, `src/browser_pages.py`, `src/browser_packs.py`, `src/covers.py`)
| Part | Measure |
|---|---|
| Panel | 288 wide. Head 28, then the search well (24 high), then the kinds segmented control (24 high, 50/66/66/82), on `surface`. |
| Rows | 22 high (search results 36: the name, then the place in ink 3 10). Indent 14 a level. Chevron at +5, 2.5, 1.2 px. Icon at +14: a folder (12 × 10, body ink 2→`base` 55 %, closed or open), a pack cover (14, r 3), or a sound's waveform thumbnail (8 columns every other pixel, 10 high, ink 3; accent when selected). Name 12: folders and packs ink 1 (packs Medium), sounds ink 2. Right: count or duration in ink 3 10. Tree guides: 1 px `base`→`ink4` 55 %. Selected: `raised`→accent 10 % with a 2-px accent stripe, a speaker glyph when sounding. Hover: `well_hi`. Section titles: SemiBold 11 in ink 3, Title case; a link at the right in accent. The list fades 8 px into the preview. |
| Search | Focused: a 1-px accent ring; the query in ink 1 with a caret; a clear ×. Match: accent SemiBold inside one `<text>` run (tspan). Chips: 20 high, r 10, accent 10 % fill and 35 % ring. |
| Preview | 118 high, `surface`. Name Medium 13, place ink 3 11, star (outline, `ink3`). Waveform well 38 high: the heard part in accent, the rest `well`→`ink2` 80 %, a 1-px `ink1` position line. Play key on: `well`→`play` 20 %. Loop key. Level: a 2-px slider with a 4-r thumb. Numbers in ink 3 10. |
| Packs | Covers 128, r 8: a diagonal gradient of the pack's hue darkened 30 %→62 %, the motif in hue +30 % white at weight s/40, a 10 % white hairline ring. Installed: a check in `play` on a disc in the corner. Get: an accent-tinted well. Getting: a 4-px bar and a percentage. Name Medium 12, then what · count · licence in ink 3 10. |
| Unit marks | 16, r 4. Cantus units: a filled square (`base`→`ink2` 28 %); hosted plug-ins: a 1-px `ink3` outline. Inside: two keys for an instrument, a filter curve for an effect. The format in words at the right; favourites a `solo` star. |

### Clip content (`src/clipdraw.py`, `src/clips.py`): the rules
| Zoom (px a beat) | MIDI | Drums | Audio | Automation |
|---|---|---|---|---|
| Far (< 6) | A pitch contour: per column, lowest to highest note sounding (+3 px) | The pulse: one 1-px tick per kick or clap, its height the velocity, at 62 % | Loudness: the mean of the envelope under the column, smooth | A 1.2-px line over a 10 % fill |
| Default (6–20) | Rows ranked by pitch (3.5 high). A melody is linear in pitch, 3–6 high (one voice) | One row a piece, 2-px ticks; hats at 62 % | The peak envelope with a texture (62–100 %); a 1-px centre line | Line, fill, 2-r points |
| Near (20–60) | True length; brightness by velocity (62 % + 38 % × v); melody rows 4–7 | Hits at their length; brightness by velocity | Peak at 55 % + loudness (RMS) at +25 % white; stereo channels apart in lanes ≥ 40 | 3-r points with a `base` ring |
| Close (≥ 60) | As near, plus a brighter attack column; melody rows 5–8 | As near | Peak detail per column; "L"/"R" | Plus curve handles (a 5-px diamond mid-segment) and values ("688 Hz") in Medium 9 ink 2 |

Lanes of 44 px and taller get a 13-px name strip (Medium 10). Shorter lanes show a name tag over
the content when the clip is selected, or when it is 80 px wide or more at near and close zooms.
Clip states: at rest `tint_rest`; selected `tint_sel` with a 1-px edge (hue→ink 1 35 %) and its
name; playing `tint_rest` + 6 % with a lit top; muted at 7 % with grey notes and a 1-px hatch
(`ink4` 35 %, every 6 px); a linked copy carries two small rings by its name.

## What it costs to build

- **Keyboard:** flat rects, two gradients per keyboard (white depth, black body) and text.
  The key layout is the rule above, with no bitmap. Cache it per row height and scale, and redraw
  it only when lit keys change.
- **Notes and steps:** rects with one edge, plus text. Cheap.
- **Pack covers:** vector motifs (circles, paths, ellipses). Rasterise each once per size and
  scale, and cache the texture (ADR 0108 decides how).
- **Waveform thumbnails, previews and clip audio:** they need peak and RMS summaries per sound
  and per clip, computed off the audio thread and cached. Per-row thumbnails need a summary of
  each listed file (8 columns), computed lazily as rows scroll into view.
- **Clip content by zoom:** four renderers chosen by pixels a beat, each cached per clip and
  zoom band. The contour and the loudness are cheaper than drawing notes.

## Files

| File | What |
|---|---|
| `pianoroll-{night,paper}-{1x,1.5x}.png` | the piano roll, whole (top bar, head, ruler, keyboard, grid, notes in every state, ghosts, velocity lane, status) |
| `pianoroll-*-4x-keys`, `-notes`, `-muted-ghosts`, `-velocity` | 4× nearest-neighbour cuts of the 1:1 render |
| `keyboard-{night,paper}-{1x,1.5x}.png`, `-4x-default`, `-4x-compact`, `-1.5x-3x-default` | the keyboard at 8, 12 and 18-px rows, its states and its measures |
| `rack-browser-{night,paper}-{1x,1.5x}.png` | the browser (tree, covers, preview) and the channel rack, whole |
| `rack-*-4x-row`, `-steps`, `-rolls`; `browser-*-4x-tree`, `-preview` | cuts |
| `browser-pages-{night,paper}-{1x,1.5x}.png`, `browser-*-4x-search`, `-covers` | search, plug-ins and presets, the Packs page |
| `clips-{night,paper}-{1x,1.5x}.png`, `clips-*-4x-far`, `-default`, `-close`, `-2x-states` | clip content at four zooms and the states |
| `src/` | the generator: `base.py`, `glyphs.py` and `topbar.py` copied from dir-a `d280cd13`; everything else is drawn here. `render_all.sh` rebuilds every PNG with Edge headless (`--disable-lcd-text`) and the comparisons. |
| `research/ui/round39/des-views/compare-*.png` | private: FL Studio, Slate at 150 % and beta.13 (enlarged ×1.5, Lanczos) at the same scale, for the piano roll, the channel rack, the browser (with Bitwig), the clips; the keyboards ×3 beside FL's, beta.13's and Ableton's |

**Not decided here, and left to the owner:** the icon family. The tool, snap and loop glyphs
are dir-a's, until the `icons` agent's new set is approved. Nothing here was checked in the
running app. These are design sheets only.
