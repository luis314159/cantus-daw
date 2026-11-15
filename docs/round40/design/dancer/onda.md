# E · Onda, the oscilloscope (chosen): what the rig draws today, what it would need

The owner chose O1 (*"el osciloscopio está chido"*). The rejected ribbon is in
`variants/onda-ribbon/`; O2, O3, the comparison GIFs and still, and `onda-variants.md` are in
`variants/onda/`.

**Files:**
- `onda-sheet.png` / `.html`
- `onda-moves-night.gif`, `onda-moves-paper.gif`
- `onda-window-night.gif`, `onda-window-paper.gif`
- Design source: `src/onda_o1.py` (`anim.OUTNAME` maps it to the plain `onda-*` names)

## What it is

A figure drawn by an oscilloscope's phosphor trace, with no body at all.
- **The limbs:** nine lines (two legs, the spine, the shoulders, two arms, the hips), each
  rippled by the waveform: wavelength 0.075, amplitude 0.006 + 0.016·level, ×1.6 when square.
- **The head:** a Lissajous loop (x = r·sin(3a + δ), y = r·sin(2a), r 0.07), δ turning with
  the beat, two bright dwell points for eyes.
- **Colour:** amber phosphor, the cast's gold.
  - At night: a halo (3.4× the line, 22 %), the line (`#F2C766`), a core (0.35×, `#FFF4D6`).
  - On paper: a dark-amber line alone (`#9C711C`, 4.0:1).
- **The beam and afterglow:** a bright beam dot runs along the whole trace once every 1.25
  beats. A faint afterglow (the skeleton 0.06 beat ago, 16 %) shows at night only.
- **Moves:** it dances the shared groove and step.
- **Signature, Square:** the ripples snap square and the poses go angular (pop and lock),
  then a saw (a lightning-bolt figure), then a sine.

## What `cantus-dance` draws today

| Onda needs | Today |
|---|---|
| The skeleton | the human rig with two-bone IK (shared with Síncopa, Voz, Glissando) |
| Lines | `ribbon_into`: but one ribbon per segment would be far too many fills (below) |
| Pop and lock | `Style::snap` (shared) |

## How to draw the trace cheaply

The trace is about **3 000 points a frame**: nine lines of 200–400 samples each (one sample
every 0.008 units), plus the 140-point Lissajous head. Drawn naively it would be thousands of
strokes. Instead:

1. **Generate the points once per frame** into one reused buffer.
   - Walk each limb's polyline at a fixed arc length (0.008 units, about 1.6 px at a 200-px
     figure).
   - Offset each sample along the limb's normal by `amp · wave(s / λ − beats · 2 + phase)`.
   - Taper the ends to zero over 0.03.
   - It is pure arithmetic on the pose, with no allocation after the first frame (keep the
     buffer in the character).
2. **Tessellate the whole trace once into one mesh per width.**
   - The trace is a set of open polylines. Build a triangle strip for each: two vertices per
     sample, offset by ±w/2 along the smoothed normal, with round joins only where the turn
     passes about 60°.
   - Append every limb's strip into **one** path, the head's loop included.
   - In tiny-skia, push every strip into one `PathBuilder` (the rig's pooled builder) and
     fill it once with the non-zero rule. That is **one fill for all nine limbs**.
3. **The glow is two or three batched widths of the same centreline, never a blur.**
   - At night: the halo strip (3.4× wide, the gold at 22 %, the only translucent fill), the
     line strip (1×) and the core strip (0.35×, opaque).
   - That is three fills in all, whatever the point count.
   - On paper: one fill (the line).
   - A Gaussian blur would cost several milliseconds on tiny-skia on the processor: never.
4. **The beam dot, the eyes and the afterglow** are a handful of circles and one more
   low-alpha strip at night: about 4 fills.

**Estimated:** about 8 fills a frame (3 glow layers, 1 afterglow, 4 dots), with about 3 000
points tessellated into about 6 000 vertices. The fill count is tiny; the cost is the point
generation and the path building.

## What to benchmark first (before anything else in this character)

1. **`cast_bench` with Onda at the 600-pixel stage**, the cast's measure (budget 1.5 ms), at
   the square phase. That is the most samples, because the steep edges need extra joins.
2. **The same at the floating size at 150 %** (a 295-px figure): the halo's translucent area
   is what scales with size.
3. **Point generation alone** (no fill): it should be well under 0.1 ms. If it isn't, cache
   the per-limb arc lengths and use a 64-entry sine table (the deterministic
   `cantus_dsp::math` already provides one).
4. **If the halo fill dominates:**
   - first try a narrower halo (2.6×);
   - then drop the halo below 120 px (the picker tile and 48 px draw the line alone);
   - never a blur.
5. **Allocation:** `tests/alloc.rs` must stay green; the sample buffer and builder are
   reused, so only tiny-skia's fills allocate.

## Tests to keep

- The ripple is a pure function of the beat (30 = 144 fps at the same times).
- Nothing is drawn outside `bounds()`: the ripple's amplitude and the square's ×1.6 included.
- The amber never turns with the hue.
- Chroma key: amber and gold are far from every key.
- The Lissajous and the eyes differ by mood at a 300-pixel stage.
