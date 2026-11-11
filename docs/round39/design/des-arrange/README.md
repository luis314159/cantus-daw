# des-arrange: the arrangement in Slate, whole (round 39, phase B)

**The idea.** The arrangement is the room where the song is, so the song is what lights up.
The chrome around it (headers, ruler, lanes) is quiet value-separated slate. At rest a header
is a name and one line under it: the instrument, or the input, or "Frozen". Its keys are
letters in ink 3, with no wells, until the pointer arrives. The user's colour is a 3-px stripe.
The song's structure is above the tracks: a ruler, then named sections with their length and
markers, then (on demand) tempo and metre. The canvas carries the music: des-views' clip
content, at a default track height of 44, so every clip has its name strip and readable notes.
The one selected track is lit across header, lane, clips, mixer strip and inspector. That is
the window's focal point. The playhead and the loop are the only accent lines in the canvas.

## Why this convinces where today's does not

Today's arrangement (`kit/window-song125-slate_night-1920x1080.png`, and dir-a's phase A
window) fails on these points:

- **Every header shouts.** Each has a saturated number slab, a coloured underline, and four
  framed keys (automation, arm, M, S), all at the same weight. Nine tracks make 36 boxes.
  Here the keys are letters until they are needed, and only an engaged key lights. Muted is
  neutral, solo amber, arm red, lanes violet.
- **A track does not say what it is.** Today a row is a name and a glyph. Here line 2 says
  "Pulsus · 909 kit", "Gravis · Sub saw", "Audio · Perc loop 124", the armed input with its
  meter, or "Frozen". A 64-px row adds the level, and a 96-px row adds the inserts.
- **No structure.** Today's ruler has bar numbers and one "Drop" flag inside a clip. Here,
  sections are a lane of named spans with their length ("Drop 8 bars"). The playing section
  is in ink 1. There are markers, and there are tempo and metre lanes when you want them. The
  song map under the canvas is also the scrollbar: every track's clips at 1 px a track, the
  view as a frame, the playhead and the loop in the accent.
- **Content as barcode** (§6j item 7). Today's rows are 26 px of squares. At 44 px each clip
  gets des-views' 13-px name strip and its content rule, so MIDI reads as rows and audio as an
  envelope.
- **No focal point** (§6j item 8). Today every lane and header weighs the same. Here the
  selected track takes a 5 % lane tint and lit clips, and its header takes a tint. Everything
  else stays at rest.
- **Missing whole features:** folders, automation lanes with their headers, time-range
  selection, the clip open in the editor, drag and resize feedback, and an empty-song state.
  None of these was designed before. All are here.

**Against FL Studio** (`research/ui/round39/des-arrange/compare-*.png`). FL's playlist is calm
because its clips are flat and its headers are tinted tiles. Slate is as calm in the chrome.
The difference is that the user's colours never fill a header; they stay a stripe and a tint.
Slate says more than FL does:

- the instrument and its input in the header;
- named sections with lengths;
- a song map;
- automation lanes under their track, with the value and the mode;
- the editor's mark on a clip;
- an empty song that teaches.

No FL shape is used: FL's track tiles, its pattern blocks and its title-bar row are not here.

## The measured spec (logical px; even numbers, so 125 % and 150 % land on whole or half pixels)

Tokens are Slate's (`dir-a/README.md`). Clip content, clip states and the zoom bands are
**des-views'** (`des-views/README.md` § Clip content), unchanged. The only additions are a
`lit` state (the selected track's clips: `tint_sel`, no edge) and a `dim` state (silenced by a
solo: tint × 0.6, notes at 50 %). Glyphs here are placeholders in the icons agent's grammar.

### Layout

| Part | Measure |
|---|---|
| Panel | under the panel's tool row (dir-a's toolbar, 28) |
| Top lanes | ruler 24, sections and markers 20, tempo 40 (optional) |
| Rows | the rows follow the top lanes |
| Song map | 14, the panel's foot |
| Header column | narrow 136, normal **200**, wide 280; the user drags it from 120 to 360. The splitter is value only (`surface→gutter 80 %`, 1 px); under the pointer it is a 2-px accent line with three grip dots and a tooltip, "Headers 200 · double-click: fit the names" |
| Track heights | small 24, compact 32, **normal 44** (the default), large 64, tall 96. A drag gives any height from 24 to 240, and Alt applies it to every track. A folder's own row is 30. An automation lane is 36 (its default). The "Add track" row is 28 |
| Whole window, 1536 × 864 (125 %) | top bar 44, seam 2, tool row 28, arrangement, docked mixer at 36 % of the space under the bar (des-mixer's Wide desk, which fits first), status 20, inspector 246 at the right |
| Maximised | the docks fold to one 28-px row of their tabs ("Mixer F9 · Piano roll F7 · Channel rack F6", and "Show the docks ⌃") |

### The track header (`src/arr_head.py`)

| Part | Measure |
|---|---|
| Body | `surface`, a 1-px foot `surface→gutter 75 %`. A folder's child sits a step back (`surface→base 30 %`) |
| Body, selected | `surface→hue` at `head_sel` (Night 13 %, Paper 16 %) |
| Body, under the pointer | `+ ink1 3.5 %` |
| Stripe | 3 px at x 0, full height, the hue at `stripe_rest` (72 % Night, 70 % Paper). Whole hue when selected; `ink4` when muted |
| Stripe, folder child | the folder's stripe continues at x 0 (× 0.8), and the child's own colour is a 2-px bar at its indent, inset 4 top and 5 bottom |
| Armed | a 1-px `rec` (70 %) line inside the stripe |
| Number | Regular 10, right-aligned at x 24 (+ indent). Ink 3, or ink 2 when selected. A folder shows its chevron (3, 1.3 px, ink 2) in its place |
| Name | x 32 + 12 × depth. Regular 12 ink 1; Medium when selected and for folders. Muted or silenced: ink 3. Truncated with "…" before the keys |
| Line 1 baseline | y + 18. Rows under 40 are a single line, centred (baseline h/2 + 4) |
| Line 2 baseline | y + 33: the kind glyph (10 × 8, ink 2 / ink 3) at x 37, then 11 Regular ink 3 (ink 2 when selected) at x 47: "Instrument · preset" or "Audio · file" |
| Line 2, armed | the input in ink 2 and a 48 × 3 input meter (`play` fill) |
| Line 2, frozen | a "Frozen" tag (10 SemiBold `accent` on `accent` 18 %, 13 high, r 3), then what was frozen in ink 3 |
| Line 2, wide header | the level in ink 3 at the right |
| Line 3 (h ≥ 60) | at y + 50: a level slider (2-px groove `edge` 60 %, fill ink 3 75 %, an 8-px thumb `raised` with an `edge` ring) and the value, Regular 11 ink 2, right-aligned. The wide header adds a 13-px pan knob and its value |
| Line 4 (h ≥ 88) | at y + 74: the inserts, in words, 10 ink 3 |
| Keys | joined, each 18 × 16, right-aligned 4 before the meter. Normal and wide: M S ● A. Narrow and folders: M S |
| Keys at rest | M and S as SemiBold 10 ink 3; arm as a 3.2-r dot (ink 3 at 70 %); A as the envelope glyph in ink 4 (in `auto` 75 % when the track has hidden lanes) |
| Keys under the pointer | one recessed well (`well`, `edge` dividers inset 3), glyphs ink 2, and "more" (three 1.1-r dots) 13 px to the left of the keys |
| Keys on | M is an `ink2` pill (inset 2, r 2) with the letter in `base`. S is a `solo` pill. Arm is a 3.8-r `rec` dot on `well→rec 24 %`. A is the `auto` glyph on `well→auto 24 %` |
| Key, silenced by a solo | M is outlined, 1 px `solo` 70 % |
| Meter | 3 × (h − 11) at the right edge, 4 in. At rest it is a groove (`surface→edge 55 %`); playing, `meter_lo`, with `meter_mid` above 72 % |
| Automation lane header | `surface→base 45 %`. The parent's stripe continues (× 0.8; × 1.25 when selected). A violet hanger line (1 px, `auto` 35 %) runs from x 22. Then the envelope glyph in `auto`, the parameter in Medium 11 ink 1, the owner in 10 ink 3, the value at the playhead in Regular 11 ink 2 (right-aligned), and a mode chip (34 × 14 well, SemiBold 9): Read ink 2, Touch and Latch `auto`, Write `rec`. Under the pointer, a close × appears |
| Add track | 28 high, `surface→base 55 %`: a plus (4, 1.2) at x 22 and "Add track" 11 ink 3. Under the pointer: a `well_hi` well inset 8/5, ink 1, and its key ("Ctrl T", 9 Medium ink 3). Its menu offers Instrument track, Audio track, Drum kit, Folder, From a preset ▸, From a file…, Return, Automation lane |
| Track menu | right-click or "more". It holds Rename, a colour row (11 swatches r 5.5, the chosen one ringed in ink 1), Instrument ▸, Height ▸ (the five steps and "Fit the window"), Automation ▸, Insert above or below, Duplicate, Put in a new folder, Freeze, Show in the mixer, and Delete (`rec`) |
| Menu style | 24-px rows, 9-px separators, r 6, `raised` with one shadow |

### Folder

- **Open:** a 30-px row, Medium name, M S only. Its lane shows the group's span as one quiet
  body (the folder's hue at 10 %, r 3, a lit top at 30 %). Inside the body, each child's clips
  are a 5-px bar in the child's hue at 55 %. The children are indented 12 and sit a step back.
- **Closed:** a 44-px row with line 2 ("Folder · 2 tracks → Drum bus"). The child bars grow to
  8 px.

### Ruler and top lanes (`src/arr_ruler.py`)

| Part | Measure |
|---|---|
| Corner | `surface`: a 20 × 18 well with a plus, "N tracks" 11 ink 3, and the lanes key (three bars) at the right |
| Corner, lane headers | under the ruler row, each top lane's header: "Sections" 11 ink 3; "Tempo" Medium 11 ink 2 with "124.00" Regular 13 ink 1 and "BPM" SemiBold 9 ink 3; "Metre 4/4" and the range "60–200" in 10 ink 3 |
| Ruler | `surface`, 24 high |
| Bar labels | every bar from 26 px a bar, then every 2, 4, 8, 16. Bar tick 1 px from y + 8 (ink 3); bars without a label get 7-px ticks (ink 4). Numbers Medium 11 at x + 5, baseline y + 17, ink 2; the playhead's bar in ink 1 |
| Beat and sixteenth ticks | beat ticks 4 px (ink 4) from 6 px a beat. From 40 px a beat, 2-px sixteenth ticks, and beats are named "41.2" in Regular 9 ink 3 |
| Loop band | the top 5 px: `accent` 32 % (Night) or 24 % (Paper), r 1.5. The ends are 2 × 7 `accent` bars. On, the looped bars keep `accent` 6 % under the numbers. Off, the band is `ink4` 45 % with ink 3 ends. An end under the pointer becomes a 4-px grip + `#fff` 25 % |
| Time selection | `ink1` 8 % from y + 7 with 1-px ink 3 edges |
| Playhead | a 9 × 7.5 `accent` cap at the ruler's foot (y + 17 to y + 24.5) and a 1-px `accent` line through every lane. Where play started is shown as the same cap, hollow (`surface` with a 1-px ink 2 ring). The pointer over the ruler shows a 1-px ink 3 line and a tooltip ("44.3.1 · 1:22.06") |
| Metre change | a 13-high `raised` chip (SemiBold 9 ink 2) after the bar number, when the ruler has 36 px a bar or more. When the tempo lane is shown, the chip sits on that lane's foot instead |
| Sections lane | 20 high, `surface→base 35 %`; every other section `+ ink1 3.5 %` |
| Section start | a 1-px line (ink 3 for the playing section, ink 3 at 70 % otherwise) |
| Section name | 11 at x + 6, baseline 14: Medium ink 1 for the playing section, Regular ink 2 otherwise. Its length ("8 bars", 10 ink 3) follows when there is room at 20 px a bar or more. A section that began before the view keeps its name at the left edge |
| Section under the pointer | `ink1` 8 %, r 3, and an end grip |
| Section selected | `accent` 20 % with a 1-px `accent` ring. Dragging it moves its bars on every track; Ctrl copies, Shift inserts |
| Marker | a 1-px ink 2 line, a 6 × 7 pennant and the name in 10 ink 2. Below 20 px a bar, the pennant only |
| Tempo lane | 40 high, `surface→base 55 %`. A 1.5-px ink 2 line over ink 2 at 7 %. Points r 2.5 in a `base` ring of r 3.5; values in Medium 10 ink 2. The lane zooms to the range the song uses |

### Canvas (`src/arr_canvas.py`)

| Part | Measure |
|---|---|
| Grid | 4-bar groups alternate `base` / `lane2` (16-bar groups below 8 px a bar) |
| Grid lines | bars `grid_bar` (every bar from 8 px a bar); beats `grid_beat` from 6 px a beat; sixteenths `base→grid_beat 55 %` from 40 px a beat. A section's start is `grid_bar→ink4 60 %` |
| Rows | each row's foot is 1 px `base→gutter 55 %`. The selected track's lane takes `base→hue 5 %`; an armed lane takes `rec` at 4 % |
| Clips | inset 1 top and 2 bottom, 1 px apart, r 3; content by des-views' bands (far < 6 px a beat, default 6–20, near 20–60, close ≥ 60). A clip that began before the view keeps its name at the canvas's left edge |
| Clip states | rest; `lit` (the selected track); `sel` (lit, a 1-px edge `hue→ink1 35 %`); `play` (+6 %, a lit top); `mute` (grey, hatched); `dim` (silenced by a solo) |
| Frozen clip | its render's envelope (`audio2`) and a snowflake (`accent` 85 %) at the top right |
| Clip open in the piano roll | a 2-px `accent` foot and, after the name, the editor's mark: 16 × 11 `accent` 30 %, with three notes in `accent` |
| Time-range selection | `ink1` at 7 % (Night) or 6 % (Paper) over the chosen rows, 1-px ink 3 edges, and a tooltip "43.1 – 46.1 · 3 bars · 3 tracks" |
| Moving a clip | the slot it left is a 1-px ink 3 dashed outline (4/3). The clip rides in `sel` with a shadow (`#000` 35 % / 12 %, +3 px). The snap line is 1-px `accent` through every row. Tooltip: "Stabs → 44.1 · +3 bars · snapped · Alt: free" |
| Resize | the edge under the pointer lights 4 px (`hue + 20 % white`); the cursor is ↔; tooltip "Length 8 bars · Alt: stretch". A 9-px corner notch at the top right loops the clip |
| Audio | the fade corner: a 7-px `ink1` square; the fade curve 1.2 px `ink1` over a `base` 60 % veil. The gain line is 1 px ink 2 at 60 % with its dB value in SemiBold 9 |
| Automation lane | `base→gutter 28 %`. Line 1.5 px `auto` over `auto` at 10 % / 12 %. Points r 2.5 (r 3 nearer) in a lane-coloured ring. Selected points are filled `ink1` with a 1.2-px `auto` ring. The segment under the pointer shows a 6-px diamond (its curve) and a tooltip "Curve +0.40". A lane with no points draws a 1-px line at its value; Write draws the line being written in `rec` over `rec` 6 % |
| Below the last track | the grid at 55 % and, from 90 px of room, "Drop a sound, a loop or a MIDI file here for a new track" (12 ink 3, centred, haloed in `base`) |
| After the song's end | `gutter` 35 % and a 1-px ink 4 line |
| Song map | 14 high, `surface`. Each track is a line of (14 − 6)/n px where it has clips: the hue at 55 %, or 90 % for the selected track. The view is a frame (1-px ink 3, `ink1` 6 %, r 3). The playhead is a 1-px `accent` line and the loop a 1-px `accent` top. The header column under it reads "64 bars · 2:03" in SemiBold 9 ink 3 |
| Vertical scroll | a 3-px `ink4` thumb at the right edge, only when the rows overflow |

### The empty song

The header column holds the add-track row under the pointer and "No tracks yet". The canvas
holds:

- "Start your song", Medium 17 ink 1;
- one sentence in 12 ink 3;
- three starter wells of 236 × 64, r 6: Instrument track (Ctrl T), Drum kit (Ctrl K) and
  Audio track (Ctrl Shift T). Each has a glyph, a Medium 13 title, a line in 11 ink 3 and an
  outlined keycap. The first is ringed in `accent` as the default;
- a dashed drop zone (5/4, ink 4, r 8), "Drop sounds here — each becomes a track at bar 1";
- "Or open a demo song: Neon Tide" in `accent`.

## What it costs to build

- Everything is rects, 1-px lines, text and a few paths, with no gradients except the pan
  knob's cap (already in the kit).
- Clip content is des-views' cost, cached per clip and zoom band. The song map is one line
  per track, redrawn only when clips change.
- Line 2 needs each track's instrument and preset name, or its input, which the model
  already has.
- **New in the model, to check:** sections as spans with a length and markers, if today's
  model has only markers; a folder's open/closed state; the per-lane automation mode chip.
  If any of these needs storing, it is a schema bump for the integrator.
- Behaviours to wire (builder `ux-b` / views):
  - keys that gather into a well under the pointer;
  - the header splitter and the row-foot resize, with their tooltips;
  - the hollow cap showing where play started;
  - a section drag that moves bars on every track;
  - the empty-song starters.

## For the owner to judge

1. The whole window at **125 %**, `window-night-1.25x.png` (your screen's default), beside
   FL in `research/ui/round39/des-arrange/compare-arrangement.png`. Is the focal point clear?
   Is it calm enough at rest?
2. **Headers without wells at rest** (`headers-*-1x.png`, `x4/headers-*-4x-states.png`):
   letters and a dot until the pointer arrives. This is the calmest choice. The alternative
   is a faint well always shown.
3. **The default height of 44 with line 2** (the instrument), against FL's taller tiles and
   Ableton's thin rows.
4. **The song map as the scrollbar.**
5. **Sections as a lane of spans with their lengths**, above the tracks; tempo and metre are
   one click away (the corner's lanes menu).
6. The maximised arrangement (`max-*.png`), with the docks folded to one row of tabs.

## Files

| File | What |
|---|---|
| `window-{night,paper}-1.25x.png` | **the whole window**, 1536 × 864 logical at 125 % (1920 × 1080 physical), idle at bar 41: the top bar, the arrangement, the mixer docked below, the inspector |
| `window-{night,paper}-1x.png` | the same layout on 1920 × 1080 at 100 % |
| `window-{night,paper}-playing-1.25x.png` | playing at 44.3: the header meters, the clips under the playhead a step brighter, the mixer's meters |
| `max-{night,paper}-{1x,1.25x}.png` | **the arrangement maximised**: tempo lane, a folder, two automation lanes (Read, Touch), muted Lead, frozen Choir, armed Vox with its input, the clip open in the piano roll, the docks folded |
| `headers-{night,paper}-{1x,1.25x}.png` | **the headers sheet**: ten states, the keys, the folder (open and closed), widths, heights, resizing, add track and its menu, the track's menu with a submenu |
| `ruler-{night,paper}-{1x,1.25x}.png` | **the ruler and top lanes**: the whole stack, the ruler at four zooms, the loop (on, off, an end under the pointer), the playhead (stopped, playing with its start, the pointer), sections and markers, the lanes menu |
| `canvas-{night,paper}-{1x,1.25x}.png` | **the canvas**: the grid at four zooms with clips, selection and the edited clip, a time range, moving, resizing, looping, fades and gain, automation lanes in three modes, the empty song |
| `x4/*-4x-*.png` | 1:1 cuts enlarged ×4 (nearest-neighbour): states, mute/solo, arm/frozen/lanes, keys, folder, corner and lanes, loop and playhead, selection, automation, the window's corner |
| `research/ui/round39/des-arrange/compare-arrangement.png` | private: FL's playlist (as captured), Slate Night and Paper at 125 %, maximised, and today's kit build at 125 %, all 1920 wide |
| `…/compare-arrangement-half.png` | private: the same with Ableton Live and Bitwig at half size |
| `…/compare-headers.png`, `compare-headers-2x.png` | private: the headers at their physical pixels, then ×2: FL, Slate, today |
| `src/` | the generator. `arr_geo.py` (measures, the view), `arr_head.py`, `arr_ruler.py`, `arr_canvas.py`, `arr.py`, `aglyphs.py`, `song_arr.py`, `win.py` and `sheet_*.py` are drawn here. `base.py`, `glyphs.py`, `topbar*.py`, `toolbar2.py` and `logo.py` are dir-a's (`d3dad46a`). `desk.py`, `strip.py`, `parts.py`, `mglyphs.py`, `insp*.py`, `data.py` and `dwin.py` are des-mixer's (`e94bba38`; `insp.py` names the instrument from data). `clipdraw.py` and `song_full.py` are des-views' (`8de7881d`), with the `lit`/`dim` states added. `render_all.sh` rebuilds every PNG with Edge headless (`--disable-lcd-text`); then run `zoom.py .` and `compare.py . <private dir>` from the design folder |

These are design sheets only: nothing was checked in the running app. The glyphs (kind marks,
the automation envelope, folder, snowflake, cursors) are placeholders until the icons agent's
family is approved.
