# L · Espectro, the analyser: what the rig draws today, what it would need

**Files:**
- `espectro-sheet.png` / `.html`
- `espectro-moves-night.gif`, `espectro-moves-paper.gif`
- `espectro-window-night.gif`, `espectro-window-paper.gif`
- Design source: `src/espectro.py`

## What it is

A figure made of spectrum bars; its body is the music.

**Each part is a small analyser of segmented bars, lit from the part's bottom by a band:**

| Part | Band |
|---|---|
| Legs | the low frequencies (the kick) |
| Torso | the mids |
| Arms | the snare and claps |
| Head | the highs (the hats) |

**The bars:**
- Bars are 0.020 wide with 0.008 gaps; segments every 0.022.
- Segments light **lapis** to 62 %, **gold** to 86 % and **vermilion** above (the logo's
  colours as an analyser's), with a gold peak-hold cap per bar.
- The unlit segments are the room's well, so the silhouette always shows.

**Face:** eyes and mouth cut through the head's bars.

**Signature, Pico:** every bar drops (a crouch), climbs (a stretch), hits the top on the beat
and breaks into gold sparks that fly and fall.

**In the designs** the spectrum is synthesised from the beat: kick on the beats, snare on 2
and 4, hats on the eighths. **In the app** it reads the real audio.

## What `cantus-dance` draws today

| Espectro needs | Today |
|---|---|
| The parts' shapes | `tube_into`, `blob_into`, `ellipse_into` |
| Sparks | `rig_fx::confetti` (pure functions of age and a seed) |

## What it would need (and its cost, honestly)

1. **The real spectrum on the dance feed.** The companion already measures the master for its
   light visualiser (ADR 0091). The bands are averaged from a short FFT of the master on the
   interface's side, never on the audio thread, and handed to the pose as four numbers.
   - For a video export, the same bands come from the render. The video stays a pure
     function of the song.
2. **Bars without masks.** The design clips the bars to each part with SVG clip paths. In
   tiny-skia a clip mask is a pixmap-sized allocation per part per frame, which is too costly.
   - The rig should compute **each bar's span inside the part analytically** (the vertical
     extent of a tube or an ellipse at that x) and fill plain rectangles.
3. **Batch by colour.** All lapis segments are one path, all gold one, all vermilion one,
   all caps one, all gaps one (the room's colour), and the unlit well one per part.

## Cost

| | Fills |
|---|---|
| About 6 batched fills a part × 6 parts | about 36 |
| The sparks | 1 |
| In all | **about 40 opaque fills** |

But those paths hold many rectangles (a few hundred), so the cost is in path building, not
the fill count. **Measure with `cast_bench` before choosing it.** If it is heavy, use wider
bars (0.03) and fewer segments.

**Contrast:** the silhouette holds by its lit bars at rest. The unlit well alone is 1.3:1 at
night, which is why the level never rests at zero (a floor of 0.15).
