# Icons in other DAWs — what makes them elegant (round 39, `icons`, second pass)

*The owner on delivery 1 (2026-11-09): "Vuelve a compararlos con FL y otros DAWs, creo que
lo podemos hacer mejor: más elegantes, bonitos y mejor pensados."* I looked at each family
at its own key height, 1:1 and enlarged, from `research/ui/screenshots/{ableton-live,
bitwig-studio,logic-studio-one,fl-studio}` and `fl-look-2026-11`. These are observations of
*qualities*. No shape is copied.

## Ableton Live 12 (control bar, browser, devices)

- **Strict monoline.** Every line icon uses one weight: 1 px at 100 % and 2 px on Retina.
  Solids are kept for play, stop and record, which are pure filled primitives with no
  detail.
- **Reduction.** Each glyph carries one idea in two to four strokes. Loop is an open
  rectangle whose bottom edge comes back as an arrow. Punch in and out are a step line.
  Draw mode is a pencil on a baseline. Plug-ins are a plug seen from the side. MIDI
  effects are lines with ticks. Nothing is a diagram of a window.
- **Air.** A line glyph fills about 55 % of its key. Transport solids are large (about
  50 %) because they have no key around them.
- **Geometry first.** Corners are square at 1 px. Circles are true circles. A 45° line
  is avoided where an orthogonal one will do.
- **Lesson:** a family reads as one when every glyph uses the same weight and the same
  vocabulary (lines, one filled mass at most). Ours mixed solid blocks, outlines and
  diagrams.

## Bitwig Studio 6 (toolbars, mixer, headers)

- **Bold solids on dark.** White filled glyphs with high contrast: the play triangle, the
  piano (three key stems), the waveform, the scissors. Strokes are 2 px at 1×. Shapes are
  simple silhouettes.
- **Loop** is a small stadium with an arrowhead folded into the stroke. The head is the
  stroke's own end, the same weight, not a separate triangle placed on the line.
- **Lesson:** a silhouette survives small sizes better than an outline. Where a glyph is
  a mass (speaker, plug, gear), fill it. An arrowhead belongs to its stroke.

## Logic Pro (control bar) and Studio One (console, navigation)

- **Logic:** thin strokes with round caps and joins, at SF-Symbols-like proportions.
  Metaphors are symmetric where they can be. The cycle is two opposed curved arrows with
  rotational symmetry, so it balances in its key. The mixer is three fader lines with
  rounded caps at three heights. The metronome is a tapered body with a ticked rod. The
  record has a second "ring" variant for the capture states. Optical sizes are tuned per
  glyph: the record dot is larger than the stop square, which is smaller than the play's
  height.
- **Studio One:** solid white pictograms in a navigation column: mixer, keyboard and
  browser as small filled silhouettes, all of the same weight.
- **Lesson:** symmetry gives calm. Round terminals at 1.5–2 px read as refined; at 1 px
  they read as soft, so they are kept for 24 px and up. Each solid gets its own optical
  size.

## FL Studio 24/2025

- **Two tones and depth.** A light grey ink with a second tone for edges and inner lines
  (the chromatic left and right edges on play and stop, the file's fold). Window keys are
  pictures of their windows (playlist, rack, mixer). They are rich but busy, and the
  owner liked their finish rather than their metaphors.
- **Proportion:** the transport glyph is about 30 % of the key, and toolbar glyphs about
  60 %.
- **Lesson:** the quality is in the finish (one ink, evenly weighted, crisp) more than in
  the drawing. Diagrams of windows only work at FL's large key size.

## What the first Cantus pass got wrong, measured against these

1. **Mixed vocabularies.** Solids, outlines and block diagrams appeared in one row. The
   others keep one vocabulary per family.
2. **Diagrams instead of icons.** Count-in (dots and a marker), follow (a marker and a
   chevron), slice (two boxes and a bar) and the window keys (blocks of clips, pads, keys)
   explained the feature instead of naming it.
3. **Bolted-on arrowheads.** Loop, undo and redo set a separate triangle on the stroke.
4. **Two inks used as decoration** (the skip's ink-2 wake, the start line in ink-2) rather
   than for structure, so the glyphs looked partly disabled.
5. **Size system.** The keys were 1.5× their canvas (24/30/36 at 100 %). Slate's keys
   are 16/20/24 at 100 % (24/30/36 at 150 %), so every glyph must be drawn for those
   heights.

## The second pass, in one line

Monoline, one weight per size: 1 px at keys of 16–24 and 2 px at 30–36. Transport keeps
filled primitives. A glyph that is a mass (speaker, gear, plug) is filled. Each icon
carries one idea, symmetric where it can be, and every arrowhead grows out of its own
stroke. It is drawn for Slate's key heights, at rest in Slate's ink 2, and lit in its
accent (play green, record red).
