# ADR 0108 — How the kit draws round 39's sheets: vectors of its own, measured

**Status:** Accepted (round 39, builder *kit*) · **Date:** 2026-11-09 · **Round:** 39 ·
**Extends:** ADR 0024 §1 and §13 (the kit draws in vectors, static richness cached; the frame
budget), ADR 0106 (at rest costs nothing) · **Related:** ADR 0111 (the design system)

## Context

Round 39 redraws the whole window from design sheets (`docs/round39/design/dir-a/` and its
siblings), made as SVG by the design agents and rendered by Edge. The kit had to decide how
to put them on screen (`docs/round39/briefs-phase-b.md`, *kit* 1):

- **A — vector operations of the kit's own**: each part painted by a function into the
  one `Brush` (fills, 1-px edges snapped to the physical grid, two- or three-stop
  gradients), tessellated every frame into the view's single mesh;
- **B — the sheets' SVG, rasterised per scale and cached**: each part (and each state) drawn
  once by `resvg` into the layer atlas at the exact physical size it is shown at, then one
  textured quad a frame.

The owner's rule for the round: **the new look may not cost frames**.

## What was measured

`examples/kit_draw_bench.rs` (release, one core of the i7-12700H, no GPU): 240 Slate keys —
a well, its edge and lip, a glyph — the same keys both ways, at 1× and 1.5×; the warm frame
is the median of 200, the cold frame the first after the atlas is cleared (a start, a theme
switch, a change of scale).

| 240 keys | scale | cold frame | warm frame | vertices | draw calls |
|---|---|---|---|---|---|
| A — vectors of the kit's own | 1× | 3.58 ms | 0.297 ms | 24 960 | 1 |
| B — SVG rasterised, cached quads | 1× | 0.82 ms | 0.010 ms | 960 | 1 |
| A | 1.5× | 2.39 ms | 0.373 ms | 33 600 | 1 |
| B | 1.5× | 1.09 ms | 0.009 ms | 960 | 1 |

(The cold frame of A carries the context's first glyph and text work too; both share the
one atlas, so both stay one draw call.) Per key, B is about 30 times cheaper on a warm frame —
1.2 µs against 0.04 µs — but A's whole bill for 240 keys is 0.3 ms.

`tests/slate_budget.rs` (the whole window at 1920 × 1080, playing, the demo project; counted
in every `cargo test`, timed in release with `--ignored`): beta.13's Cantus Night against
round 39's three families.

| Whole window, 1920 × 1080, playing | p50 | p95 | triangles | allocations | draw calls |
|---|---|---|---|---|---|
| Cantus Night (beta.13's default) | 1.36 ms | 2.01 ms | 48 940 | 314 | 90 |
| **Slate Night** | **1.11 ms** | **1.83 ms** | **41 134** | 318 | 90 |
| Graphite Night | 1.02 ms | 1.91 ms | 39 352 | 318 | 90 |
| Studio Ink Night | 0.99 ms | 1.79 ms | 39 364 | 313 | 90 |

Build plus tessellation, release, best of three interleaved runs of 200 frames. Slate is 18 %
cheaper than beta.13 at the median and 16 % fewer triangles: flat wells replace the raised
keys' bevels, drops and glass. (Four more allocations a frame: the face's first lookups; the
test allows 8.)

## Decision

1. **The kit draws the sheets in vectors of its own (A).** Slate is flat fills, 1-px edges
   and lips and a handful of two-stop gradients (`dir-a/README.md`, *What it costs*): the
   painter's cheapest work, and measured cheaper than beta.13's whole window already. Every
   part stays one function `paint(brush, rect, &Style, State, …)`, every state the same
   function, so a token change, a hover or a scale change costs nothing extra and nothing
   goes stale. Where a part is static, repeated by the hundred and measured to matter — a
   row of keys whole, well and glyph — B is the measured cheaper way (30× per key) and the
   kit may move it to the atlas; until a frame needs the 0.3 ms, the wells stay vectors.
2. **SVG rasterised and cached (B) is kept for what is static and detailed** — the logos and
   the mark (ADR 0024 §12, ADR 0046), and the multi-stop bodies already in the layer atlas
   (knob bodies, lamps; ADR 0103) — never for the chrome. A part goes to the atlas only when
   it is measured cheaper there.
3. **The icon family is drawn per key height, as its designer set each pixel** — 16, 20, 24,
   30 and 36 device pixels, never one drawing scaled. Those drawings are made on the key's
   own pixel grid (straight edges on whole pixels, some set pixel by pixel), so they are
   the one place where B's exactness is the point: each (glyph, size, inks) is rasterised
   once into the layer atlas and drawn as one quad on the device's pixel grid
   (`kit::icons`). A key's well around it stays vectors (A).
4. **The budget is held by tests**: on a warm frame Slate issues no more draw calls and
   tessellates no more triangles than beta.13's Cantus Night, and allocates within 8 of it
   (`slate_costs_no_allocation_no_draw_call_and_no_triangle_more_than_beta_13`); in release,
   its median frame is within 5 % (+0.1 ms) of beta.13's (`slate_timing`, `--ignored`).

## Consequences

- Design sheets are references, not assets: each element is turned into kit code when the
  owner approves its sheet, and checked against it with `shoot`.
- A theme switch re-tessellates nothing it did not draw anyway; the atlas holds only logos,
  marks, knob bodies and lamps, so its pressure does not grow with themes × scales × states.
- What B would have given for free — the designer's exact anti-aliasing of a curve — is
  bought with care in the glyph tables (pixel-grid hinting at small sizes).

## What the tests hold

- `tests/slate_budget.rs` — counted on every run; timed with `--ignored`.
- `tests/frame_budget.rs`, `tests/workspace_budget.rs`, `tests/night_budget.rs` — unchanged
  budgets, run in Slate (the default) from round 39 on.
- `examples/kit_draw_bench.rs` — the comparison above, to re-run when a part is added.
