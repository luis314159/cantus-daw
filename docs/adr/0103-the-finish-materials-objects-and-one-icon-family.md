# ADR 0103 — The finish: materials, controls as objects, one icon family

*Round 37, the finish builder. Status: accepted for steps 1–3 (first delivery) and for the
second delivery's strips, top bar, colour and content (§4–§7 below); the acceptance
extends it.*

## Context

The owner's side-by-side comparison with FL Studio 24 (2026-11-04,
`research/ui/fl-look-2026-11/notes.md`) went to FL 10 to 2. The losses that were the kit's
own: flat circles for knobs, a beige slab for a fader cap, a hazy halo for an LED, black
boxes for value displays, toolbar icons of mixed weights at ~14 px whose active state was a
block of orange that swallowed them. FL's controls read as *objects*: lit from above, set
in wells, with a specular on what is metal or glass, and one icon language everywhere. The
owner's direction: be inspired, never copy; take qualities, not forms; Cantus's strengths
(label/value units, numbers, meters, cables, themes) stay.

The kit already had one lighting recipe (ADR 0024 §4) — raised and sunken, a lightness
ramp of each part's own colour, scaled by the theme's depth. It could make relief but not
material: every theme's light was the same arithmetic, nothing could be glossy or matte,
and a well had no inner shadow.

## Decision

1. **Material tokens.** Six colours join the depth group, each a colour whose alpha is its
   strength: `depth.bevel` (the light on an object's upper bevel), `depth.bevel_shade`,
   `depth.inner_shadow` (inside a well, or a pressed key), `depth.lip` (the light on a
   well's lower lip), `depth.specular` (the hot highlight on metal and glass) and
   `depth.drop` (the shadow an object casts). **Every preset defines them** — Cantus Night
   as warm metal and glass (a strong specular, deep inner shadows), Cantus Paper and the
   light themes as matte (a white bevel, a soft shadow, a faint specular), the cool dark
   themes between, the two high-contrast themes with firmer shadows. `light::Material`
   gathers them, scaled by the theme's depth (0 flat … 1.5 deep), so a flat theme stays
   flat. **Saved custom themes need no migration:** a theme file stores its tokens by key
   and every key it lacks is derived (`Tokens::derive_from_palette` gives a dark file the
   dark materials and a light file the matte ones), so a theme saved before round 37 opens
   with materials of its kind and saves them from then on (tested).
2. **Controls redrawn as objects**, from those tokens only (`light::object`,
   `light::recess`, `light::inner_shadow`, `light::glass`):
   - **Knob**: a cylinder standing in a round well; the track runs in the well and the
     value arc, from the default, always shows; the cached body has its shadow in the well,
     a skirt lit from above, a domed cap with a bevel and the specular; the pointer casts
     a shadow. The mixer's mini knob is the same object, smaller (a matte cap in a light
     theme), its value arc in a groove of its well.
   - **Fader**: a slot cut into the strip; a cap with a drop shadow, a face lit above and
     turned from the light below an engraved index line, sides turned away, the specular
     across its top. Selected, the index line and the slot below it light in the selection
     colour and the face takes a little of it (it was a lime slab).
   - **LED**: glass in a socket — the socket's lip, a lens lit from inside (a hot core, the
     colour, a darker rim) or dark tinted glass when off, the specular either way, a tight
     halo instead of a haze.
   - **Buttons and toggles** (caps, LED buttons, toolbar keys): raised at rest, sat down
     while held. **A key that is on stays down** with a tint of the highlight across its
     floor and an edge of it round its rim, its glyph in the highlight — *a tint and an
     edge, not a block of orange*. The top bar's joined view keys are drawn by the kit
     (`cap::key_strip`), so they follow.
   - **Value boxes and fields** are recesses; a readout is a recessed display under the
     theme's glass. **The meter's well** has its lip and its inner shadow (still one
     batched mesh, still no feathering). A rack slot (tile) has its bevels.
3. **One icon family** (`kit/glyph.rs`). A glyph is data: a list of ops in units of a
   12-unit grid — lines, loops, outlines, rings, arcs, frames, pills, blocks, discs, tints
   — none of which can carry a width or a radius, so every glyph has **one stroke weight**
   (1.2 units, rounded to whole physical pixels, every short stroke's points on the pixel
   grid so they are crisp at 100–200 %) and **one corner radius** (1 unit). Two tones: the
   ink, and inside closed shapes a tint of it (30 %). **One optical size**: every glyph's
   ink stays inside ±5.7 units and none is smaller than 8 units across. A toolbar key
   gives its glyph 0.72 of its side. The Session and Stage marks, which views drew on
   their own, are in the family now.

## Consequences

- Every view that uses the kit's painters changes look at once (the mixer strips, device
  cards, the inspector, fields, menus, the top bar's keys). Views that call the old
  `light::raised`/`sunken` directly keep their look until they are moved — the old recipe
  stays for them and for the unit faces (ADR 0074), whose skins are their own.
- Six more tokens in the theme editor (Depth group), named in both languages; the
  eyedropper names them on every part that paints with them (`tests/kit_tokens.rs`).
- The kit gallery's golden images were regenerated on purpose for every part whose look
  changed (listed in `docs/round37/finish.md`).
- Cost: an object is three rounded rects (drop, silhouette, face) plus one of stops for its
  bevels; a well three plus its shadow. The frame-budget tests hold (numbers in the round
  report).

## What the tests hold

- `tests/kit_glyphs.rs`: every glyph inside the live area, at least the optical size,
  centred (but the grip and minimise, placed on purpose), its filled shapes convex; each
  glyph's rendered ink within 0.35–2.4 × the family's median (a chevron is the lightest by nature) at 18 pt × 1, 18 pt × 1.5 and
  24 pt × 2; the stroke whole pixels at every scale.
- `tests/kit_tokens.rs`: every part registers exactly the tokens its pixels depend on,
  materials included, in Slate and Paper.
- `kit::tokens` and `theme_family` tests: every preset defines the materials with a
  strength; a theme file without them derives materials of its kind; every pairing the
  kit promises still holds.
- `tests/kit_gallery.rs`: the golden images (re-blessed on purpose).

## Second delivery (after the owner's "¿es lo mejor que puedes hacer?")

4. **Strips are plates** (`light::plate`, `light::chip`): lit, bevelled, tinted whole by
   their colour by default; the meter, lamps and pan knob in one recessed **pod**; the
   chain/output/law boxes are chips. **Controls grow with the width mode**
   (`mixer_layout::grow`: 1.0 / 1.15 / 1.4) and so do the rows that hold them.
5. **The top bar's scale follows the window's width** (`transport::bar_scale`: 1.0 up to
   1366 pt, 1.12 at 1920, 1.2 from 2400) rather than one size for every screen: a laptop
   keeps every group, a desktop gets FL-sized keys. Its wants and its room are computed
   at the same width (`wants_at`, `room`). At 1920 undo and the inputs (and in Spanish the
   system meters) fold into ⋯; from 2560 nothing folds. A key that is **on is lit through
   in the highlight** (a lit dome, a dark glyph) — clearer than a tint and an edge, which
   the owner found weaker than the old block; the transport is a capsule, record is red
   wherever it is drawn.
6. **Track colour with strength**: `safe_track`'s saturation band 0.42–0.72 (FL's is
   ~0.25–0.40), the lightness bands kept so text on colour holds.
7. **Content you can see**: a clip whose name repeats the clip above it at the same bar is
   *quiet* — no title bar, its notes at its full height, a row per pitch in use, centred.

## Third delivery (the blind judge's list; the owner on the fader and the small buttons)

8. **The fader is the user's choice**: `kit::fader::FaderCap { Classic, Metal }`, Classic —
   round 36's fader, long throw and scale — the default, because the owner preferred it;
   the metal cap of the second delivery kept as the alternative. It is an *appearance*
   setting (`Appearance::fader`, settings, not the project: no schema bump; absent reads
   as Classic), applied by `apply_appearance` to one global every fader painter reads, so
   strips, VCAs, master and units switch together and live. `grow` is now
   1.0 / 1.1 / 1.25: the long throw wins over a larger cap.
9. **A target on a mixer strip is at least 24 × 24 pt**, in every width mode and at every
   zoom (`mixer_layout::TARGET`, `keys_height`): phase, swap and mono are kit keys (lit
   through when on, tip and AccessKit name with the state); mute/solo, FX/record, the
   sends' lamp, tap chip, "+" and "+n", the master's polarity key and the desk's handles
   hold the same floor. Rows grow to hold them rather than targets shrinking to fit.
10. **The transport is the bar's whole height** (first group, `Group::Transport` tall;
    PAT / SONG two word keys over the tempo). On a 1920 window undo, the inputs and the
    system meters fold into ⋯ (from 2560 nothing does); at 1280 logical points the area
    keys fold too — the price of a transport the size of FL's. The audio chip's least is
    its short line (name · latency) whole: it never cuts a name.
11. **Displays in IBM Plex Mono Light**, values in Plex Mono Regular (OFL, in the notices).
12. **Notes fold to the pitches in use** in a clip's miniature (contour kept, empty pitches
    left out), so every clip's content is at its full height.
13. **Meters stay one cost at every level**: zones lit by a gradient and a two-tone peak
    hold, no per-segment lines (a 64-strip desk paid 45 000 triangles for them).

What the tests hold, added: `tests/mixer_targets.rs` (the 24-pt floor, no overlaps, three
modes × five zooms); `kit::appearance` (the fader choice absent → Classic, round trip);
`kit_gallery` goldens `fader` and `fader_metal`; `topbar_layout` (what may fold at 1920,
nothing at 2560, the chip's least is its short line).

## Fourth delivery (the second blind judge: 23 of 24; the owner: "lo que sea excelente y mejor para UX y UI")

14. **At 1920 × 1080 at 100 % nothing folds.** The bar has two scales: the groups that
    carry its presence — the transport, PAT / SONG, the tempo and the clock
    (`transport::primary`) — at `bar_scale` (1.12 at 1920), every other group at
    `secondary_scale` (the window's own up to 1920, 1.1 from 2400). The clock's time and
    metre are a line under its digits (a console's display), not a column beside them;
    snap is a key with a magnet (a new glyph in the family, 64 now), not a lit word; the
    hint panel's least is at the window's scale. Undo / redo, the inputs, CPU / RAM /
    voices / dropouts, the area keys and the audio chip are all on the bar with the
    transport at its larger size.
15. **Below 1920 the bar folds in a defined order** (`Group::importance`): never the
    transport, the clock, the tempo, undo / redo or the audio status before the rest;
    first the area keys (each has its F-key), then the system meters, then the
    metronome's options, then sync, the tools, the snap, the inputs. The ⋯ menu shows each
    folded group under its name **with its shortcut** at the band's right.
16. **Unit faces keep their own look in a light theme** (ADR 0074 stands): Aequalis is a
    dark instrument display by design, as hardware is — what had to go was the smoked
    glass's diagonal reflection across a large display (it read as a glare streak over the
    plot); a pane taller than 64 pt now has the glass's darkening and lit lip only.
17. **Use over looks where they trade**, and the looks still have to win: pictograms that
    show the view (a playlist with its playhead, a keyboard with notes, two strips with
    meters and faders, a step sequencer, a folder with a sound) instead of bar and list
    variants; the record arm a dot and the automation's Read a curve, so no two header keys
    say "R"; "+ send" fills the rows left under a strip's sends; the add-strip column a
    key, not an empty column; the fader caps' shadow tight (no halo); on and off steps
    apart at a glance; the zoomed-out arrangement grouped by four bars.

## Fifth delivery (the third judge: 22 of 24; the last items)

18. **From Wide up a strip's meter is the long one beside its fader** (FL's), as tall as
    the fader's travel, with its scale's marks; the pod keeps the held peak's number. The
    meter row's least there is 26 pt and its share of the flexible height half the
    preference's (`mixer_layout::meter_min`, `ratio_for` takes the mode), so the fader gets
    the travel. Compact keeps the pod's meter.
19. **A large display has no reflection band** (`skin::displays::cover` and the glass
    material, panes over 80 / 64 pt): it read as a glare streak over Aequalis's plot.
20. **What a test can see is what it compares**: `final_look`'s pinned-strip test now reads
    each text inside its clip — the failure was a value scrolled away under the pinned
    strip (painted, clipped, never on screen), not two words touching.
21. The status readings (CPU, RAM, voices, dropouts) stand as name over value at the label
    size, hairlines between them, an idle dash quiet; the inputs' and the metronome's names
    at the label size too. The first clip of a vertical run keeps its title (the name is
    always on screen); notes are bars with a pixel's gap, short hits clean blocks. The
    right dock sits after the strips when they all fit.

## Sixth delivery (the fourth judge: 17 of 26)

22. **One face for values**: `Role::Value`, `Hero` and `Display` are Plex Sans Condensed —
    its figures are tabular (every digit 0.54 em, measured), so a number still never shifts
    as it counts, and the transport's "Off", "+0.0 dB" and "124.0 BPM" are one family with
    the words beside them. The mono faces stay registered for the legacy text styles.
23. **A key that is on keeps its glyph**: its face tinted a tenth toward the highlight, a
    rim and a bar of the highlight along its foot, the glyph in the highlight (a step
    darker on a light theme, 3:1 against the face) — not a dark glyph on an orange
    sticker. Glyphs take 0.80 of a key's side (was 0.72).
24. Without audio the system readings are zero in the dim ink, not dashes. The ⋯ key has
    its divider. The menus and the hint are drawn at the bar's scale again (the groups'
    tighter packing pays for it).

## Seventh delivery (the fifth judge: 16 of 26)

25. **The comparisons are captures of the real app**, as FL Studio's are: `CANTUS_SHOT_SCENE=hero`
    (`crates/cantus-ui/src/shot_scene.rs`, one marked call in the window's frame, the
    variable read once) sets the release build up as the README's hero picture, and
    `tools/look/shots_of_scene.ps1` captures its client area, maximised, DPI-aware, through
    `PrintWindow` (no other window over it), each theme on a fresh data folder. The CPU
    renderer's shots stay for the README and the views no keystroke reaches alone.
26. **A light theme's key that is on keeps its glyph in the ink** (the rim and the foot bar
    in the highlight say "on"); a dark theme's glyph is the highlight. The area keys have
    distinct silhouettes: the inspector an "i" in its ring, the session four slots one
    playing, the rack a name button over four step keys.
27. **The send band has as many rows as the strip with the most sends** (no spare row):
    "+" follows a strip's last send where it has room; a strip at the most adds from its
    route menu or FROM SELECTED.
28. **A clip's notes are a piano roll in miniature**: each note a bar at its pitch row (the
    rows in use), its true length, 2 px at least each way, a pixel's gap only before a note
    that follows another on its row; dense hits zoomed out merge into the bar before them
    rather than drift.

## Eighth delivery (the sixth judge: 17 of 26)

29. **A toolbar key's glyph is a fine-line pictogram** (`glyph::paint_line`): the family's
    shapes with a stroke three quarters of the family's, rounded to whole physical pixels
    (1 px at 100 %, 2 px at 150 %); every filled shape an outline over a faint tint, only
    dots under two pixels left solid. At rest the ink is the theme's text on a dark theme
    and its label's mid-dark ink on a light one (`cap::rest_tok`, registered per theme);
    a key that is on has the highlight in its glyph and a faint tint in its face — no bar at
    its foot.
30. **The sounds browser shows what a sound is before it is heard**: category tabs (All,
    Drums, Loops, Melodic, FX, read from the names), a waveform thumbnail and the length of
    each audio row, its key and tempo where its name says them (`views/samples_thumbs.rs`:
    WAV files read once on a worker thread and kept for the session; the browser never
    waits on a disk), and a preview strip in its foot: play or stop, the sound's name, its
    position, and the preview's level (a session's choice; a new level starts the sound
    again so it is heard at once).
31. **Clips are calm**: their fill in one band of lightness and saturation with the mixer's
    strip tints; their notes a deep tint of the clip's colour, never black; their header a
    faint gradient.
32. **A light theme's meters sit in light wells with a rim**; their colours are deepened to
    hold 3:1 on the well.

## Ninth delivery (the seventh judge's defects)

33. **The piano roll's keyboard is a piano's**: an octave's seven white keys share its
    twelve rows, C–E three over five rows (5/3 each), F–B four over seven (7/4 each) — so
    B|C and E|F fall on row edges and every other boundary under the black key between,
    the white keys nearly one width as a piano's are; the black keys 60 % of the length,
    each over its own row (`piano_roll_paint::white_rows`, with a unit test against the
    note layout).
34. **A row of the browser never cuts a tab or a sound's details**: the category tabs wrap
    into balanced rows where the panel is narrow; a sound's thumbnail sits just after the
    widest name in view and its details are right-aligned at the panel's edge, and where a
    row is short the key and tempo give way first, then the thumbnail, then the name — the
    length never (`samples_browser`'s test at 180–440 pt in both languages).
35. **A toolbar pictogram is one ink, lines only**: no tint inside its shapes; a key that
    is on has its whole glyph in the highlight and no rim.
36. **Nothing in the top bar wraps mid-phrase**: a long hint is one line a step smaller, or
    two lines broken at a " · " between its parts, and only failing both wrapped where its
    words fall. A silent master meter reads "−∞ dB" over its scale's ticks.
37. **The judge's crops follow the capture's layout**: the hero scene writes each area's
    rect (`shot-layout.json` in the data folder, copied beside the capture by
    `shots_of_scene.ps1`) and sets the browser's width and the headers' width on every
    frame from the 50th, so no capture's crop lands on a neighbour's columns.
38. **The browser's thumbnails are read by one worker, in pieces, within bounds** (the
    review of deliveries 7–8): one thread, a queue of at most 256 asks served newest first
    (the oldest let go and asked again if their rows return), each file read through a
    seeking reader — the format chunk, then at most 64 KB per column in eight pieces —
    never whole; at most 4 096 thumbnails kept, the least recently shown let go first; a
    kept one's file looked at every 2 s and read again when its size or time changed; the
    worker asks the window for a frame when one lands; no worker, no thumbnails (never a
    row waiting forever). `WAVE_FORMAT_EXTENSIBLE` is read by its sub-format's tag, so
    hound's 24/32-bit int and 32-bit float files (every file Cantus records, renders or
    freezes) and 64-bit float read true (`samples_peaks`' tests on hound's bytes).
    A name's words are parted from their digits and a plural read as its singular for the
    tabs ("Kick01", "808Kick", "Toms").
39. **The mixer's right dock is never pushed past the window's edge**: the whole-strips
    rule gives way to the edge (`mixer_desk::the_right_dock_never_leaves_the_window`).

## Tenth delivery (the eighth judge: 10 of 26 — Paper, the icons, the materials)

40. **A light theme's surfaces stay neutral**: a track's colour is its header's colour cell,
    the mixer strip's number cell and its clips' bodies — not a wash over the header row,
    the strip or its pod (judge 8: "candy", lower text contrast). Header names on Paper
    are in the text ink.
41. **Meters are dark recessed wells in both themes**: Paper's well is the night stage's
    (#1d1a18) with its levels, rimmed in the edge's dark.
42. **No dark island on Paper**: a light theme's selected segment is a raised light tile
    with its word in the ink (it was knob-dark); a lit key is the highlight's band with its
    glyph ink (7:1), its lit tokens registered per theme (`cap::lit_toks`).
43. **Toolbar and transport keys draw the family's glyphs**, solid where the family is
    solid (play, stop, record, the folder, the faders' caps) at its stroke (2 px at 100 %):
    the fine-line pictograms of §29/§35 read as wireframe. One ink per state.
44. **The fader cap is one soft fall of light** with its grip line and a soft shadow two
    points down (the two-tone half-lit cap read as a grey band).
45. **Cables are drawn as cables**: a darker jacket under the coloured core and a sheen
    along its top, lit or related; the faint ones stay hairlines.
46. **Clips are a muted tone of the track's colour** (saturation 0.22–0.38 on Paper,
    0.28–0.46 on Night) and their notes a muted tone of the clip; Night's white keys a
    neutral near-white (#eeebe6, was a cream); the octave and note labels a size up.

## Eleventh delivery (the ninth judge: 20 of 26 — keys, mixer, browser)

47. **The sounds browser's lower half is the chosen sound** (`views/samples_detail.rs`): a
    card under the list with its name, its format line ("WAV · 48 kHz · Stereo · 2.6 s"),
    a large waveform (a click plays it) and its tags as chips (key, tempo, the families it
    is in); a row's thumbnail is a smooth envelope (anti-aliased outline over a tint of
    it, 48 columns) and its key and tempo are chips beside its length.
48. **The mixer has an insert panel** (`views/mixer_inserts.rs`, FL's): the selected
    strip's ten slots at the desk's right — number, the effect's whole name, its on lamp;
    empty slots named as places; a click opens the insert's editor or, on an empty slot,
    the inspector. On from the desk's ▾ menu (*Insert panel*); saved per layout
    (`DeskPrefs::inserts`, off in desks saved before it).
49. **The mixer's materials**: no empty inset — an empty chain is its "+" alone until the
    pointer is on it; the fader cap slimmer (14 × 20 pt, was 16 × 25); on a light theme
    the strips stand two points apart in a gutter a clear step darker than they are.
50. **One lit look in both themes**: a lit key's glyph in the highlight on a face warmed by
    it (Paper's highlight a step deeper to hold 3:1); the playlist's glyph is clips on
    three lanes, staggered.
51. **The judge's pairs are 1:1 pixels on both sides** (`judge_real.py`): no zoom, so
    neither side is softened or made blocky by scaling.

## Twelfth delivery (two judges: 20 and 18 of 26)

52. **A browser's sound rows are a table**: the thumbnail, the key chip, the tempo and the
    length each in its own column, each the widest of the rows in view, so every value
    sits at one place in every row; the panel's width (not the row's) drops the thumbnail,
    then the key, for every row alike; a name drops the tempo its column shows ("Gravis -
    Acid 303 - 120 BPM" → "Gravis - Acid 303"). The detail card is compact (84 pt: the
    name and its chips on one line, the format, the waveform).
53. **A pack has a cover**: our own drawing in the kit — a square of a section hue lit
    from above, its motif from the pack's id (a loop for loops, a hit for one-shots, bars
    for the rest).
54. **The hero shows Cantus's own packs**: `CANTUS_SHOT_PACKS` names a folder holding
    their archives (`shots_of_scene.ps1 -Packs`); the scene installs them from there (each
    checked against the manifest's SHA-256) and opens the loops at their bass folder;
    without it the synthesized folder stays. No sample is in the repository (ADR 0030).
    The capture script moves the pointer off the window first.
55. **The toolbar's area glyphs are filled shapes of one weight**: the folder, the
    playlist's clips, the session's slots, the piano roll's keyboard and notes, the step
    grid, the faders — solid, a tint of the ink for what is unlit, larger in the key
    (0.86 of its side).
56. **The hint panel says the song while nothing is hovered**: "11 tracks · 14 strips · 64
    bars · 2:08" under its name (made again only when those change).
57. **Paper's meter wells are a slate well** (#3e4346), not black; the levels keep 3:1.

## Thirteenth delivery (two judges: 22 and 19 of 26 — the keys and the browser)

58. **Every sound row in a pack carries its pack's cover** at its left (a waveform icon
    in a folder of the user's); the covers are small compositions — a sky lit from the
    top, a ground of a second hue, a disc standing on it with its shadow, the pack's motif
    in the light on the disc, a sheen. The hero's browser is 366 pt (458 px at 125 %, as
    wide as FL's panel) so a row holds its cover, its whole name, its waveform, its tempo
    and its length; the table's thumbnail is 48 pt and its gaps 6 pt.
    *Round 39 (lo-rest): one mark per row type — a pack's row carries its cover, a folder
    its glyph, a sound its own waveform (a plain audio glyph while it is read), never its
    pack's cover; a pack's sounds showed covers and waveforms side by side while they were
    read (`samples_browser::row_mark`).*
59. **An area key's glyph is a small two-tone drawing**: its body in the key's ink (framed
    clips on lanes, a folder with its flap, a keyboard with notes at two pitches, faders
    with caps at different heights, framed steps, framed slots) and one accent in the
    highlight (the playhead and its flag, the sound in the folder, the note being played,
    the middle cap, the lit steps, the slot playing); a key that is on draws both in the
    highlight (`glyph::paint_accent`, `glyph::accent_ops`).
60. **The hero's layout file is the last frame's**: written on every frame whose layout
    differs from the last one written (it was every fourth frame, and a Night capture kept
    a transient layout with the browser half the window).
