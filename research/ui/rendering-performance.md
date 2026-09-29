# How Cantus draws: vectors by hand in egui 0.36, and the frame budget

*Research report, September 2026. Measured, not estimated: every number below comes
from a scratch harness outside the repository that links `cantus-ui` by path and runs
the real `CantusApp` headless (method in §11). Nothing in the repository was changed.*

The user asked for an interface "as excellent as FL Studio", drawn in hand-made vectors,
that looks excellent and has "incredible performance". FL Studio has drawn its whole
interface with its own vector engine since FL 12 ("100% vectorial … 4, 5 or even 8K
monitors with pin-sharp fidelity", Image-Line, April 2015). Cantus already paints
everything itself through egui's `epaint`. This report answers four questions:

1. What `epaint` 0.36 can and cannot draw for an FL-grade look, and the workarounds.
2. How icons and the user's ten logos should be rendered, tinted and cached, and how to
   show a splash screen.
3. What Cantus's frames cost today, where the time goes, and what budget to hold.
4. How to build a hand-drawn component kit that is themeable (FL-style H/S/L), stateful,
   animated and tested.

---

## 0. The short version

**The renderer is not the problem; the frame is.** At 1920×1080 on one P-core of an
i7-12700H, the whole window costs **3.2 ms** with the 16-strip mixer, but **9.7 ms** with
64 strips — and only about 13 of those 64 strips are on screen. **8.2 ms of that is
layout and shape building, 1.5 ms is tessellation**, and the GPU needs **0.3–1.8 ms**.
The 64-strip frame allocates **28,600 times (5.6 MB)**. The heaviest single item is the
sends menu's target list: **2.8 ms and 8,064 allocations a frame**, computed for every
strip even when no menu is open. A 4K frame (2× scale) costs only +10 % CPU.

**Five decisions** (argued in §10):

1. **Stay on egui 0.36 with `glow`, and draw only through meshes and textures.** No
   paint callbacks. Then the same kit runs unchanged in the plug-in windows, which use
   `egui-baseview` with OpenGL. The GPU is not the limit: 0.3–2.7 ms at 1080p on
   either GPU of this laptop. `wgpu` doubles binary size, and egui#7761 reports that it
   can take 1–7 s to start.
2. **Use a hybrid vector pipeline.** Anything that moves (arcs, meters, curves, text)
   is live epaint geometry, with its feathered anti-aliasing. The rich parts that do
   not move are rasterised from vectors at the exact physical pixel size and cached as
   textures: knob bodies, bevels, soft shadows and glows, icons and logos. That
   rasteriser is `vello_cpu`, which epaint 0.36 already compiles in for its glyphs, or
   `resvg` for SVGs that use filters.
3. **Fix the frame, not the renderer:**
   - skip the layout of strips and rows that are off screen;
   - build lists lazily, only when a menu opens;
   - stop the per-frame clones and strings;
   - batch meters and knob arcs into one mesh each;
   - cache text galleys;
   - repaint only on change, with a single 60 Hz ticker for meters and animation.
4. **Hold a budget: ≤ 4 ms CPU (p50) at 1080p** for a 64-strip mixer with meters
   running, ≤ 1,500 allocations a frame, and no repaints while idle. Enforce it with a
   frame-budget test that checks counters, which do not vary between runs, as well as
   timings.
5. **Build a component kit.** Theme seeds (hue, saturation, lightness, contrast, as in
   FL) derive the tokens. Pure `look` painters draw each state (rest, hover, active,
   focus, disabled). Interaction lives in thin wrappers around them. A golden gallery,
   rendered by `raster.rs` at 1×, 1.5× and 2×, catches any regression.

For the logos:

- **Rendering:** `resvg`, cached per size. `lyon` is ruled out: it cannot do the
  filters, and its meshes are aliased.
- **Tinting:** map roles to tokens while keeping the designed lightness. "Original"
  stays the default for #2.
- **Splash:** the root window boots as a borderless splash and advances the start-up
  in stages. On this machine that start-up is 0.7 s of MIDI, 0.3 s of audio, plus the
  project.
- **Icons:** bake the icons at build time, and embed the `.exe` icon with `winresource`.

---

## 1. Machine and method, in one paragraph

- **Machine:** Intel i7-12700H laptop (6 P-cores and 8 E-cores), Iris Xe plus an RTX
  A2000 Laptop GPU, Windows 11, Rust 1.97.1.
- **Build:** release, with the repository's own profile (`opt-level 3`, `lto = "thin"`,
  4 codegen units).
- **The frame:** the app runs through `eframe::App::ui` with
  `eframe::Frame::_new_kittest()`, at 1920×1080 points, light theme. Meters are fed by
  a real `Processor` from `EngineHandle::manual`, so the transport is playing and the
  meters move.
- **What is timed:** `ctx.run_ui` (layout and shape building, called **build** below)
  and `ctx.tessellate` (called **tess**) are timed separately. Allocations are counted
  by a counting global allocator.
- **Controlling for noise:** the first runs were dominated by scheduler noise, because
  the OS moves the thread between P- and E-cores and the laptop is thermal-limited. So:
  - every case runs in its own process, pinned to one P-core at high priority;
  - each case is 300 frames after 30 warm-up frames;
  - there are 3 rounds, and the report takes the **best of the three medians**. p95 is
    reported alongside.
- **GPU cost:** the tessellated frames were dumped and replayed through `egui-wgpu`
  into an off-screen target on every adapter, timed with GPU timestamp queries.

---

## 2. What a frame costs today

### 2.1 The whole window (build + tessellate, CPU, 1920×1080, best of 3 medians)

The **mixer 64** rows use the welcome project grown to 64 strips, with 64 playing
tracks. Two terms in the tables:

- **Draws** is the number of GPU draw calls (egui primitives).
- **Allocs/frame** is the number of heap allocations the frame makes; KB/frame is how
  much memory they asked for.

**Mixer:**

| View (default layout: transport, arrangement, inspector, status) | Build ms | Tess ms | **Total ms** | p95 total | Shapes | Vertices | Triangles | Draws | Allocs/frame | KB/frame |
|---|---|---|---|---|---|---|---|---|---|---|
| Mixer, 16 strips, idle | 2.20 | 1.52 | **3.72** | 4.7 | 1,673 | 44,880 | 60,858 | 40 | 5,970 | 893 |
| Mixer, 16 strips, playing | 2.19 | 1.05 | **3.24** | 4.2 | 1,676 | 45,234 | 61,329 | 40 | 6,016 | 905 |
| Mixer, **64 strips**, playing | 8.17 | 1.52 | **9.69** | 11.7 | 3,133 | 50,246 | 64,531 | 51 | **28,639** | **5,636** |
| … arrangement hidden | 7.38 | 1.04 | 8.42 | 11.8 | 2,197 | 42,382 | 57,753 | 37 | 26,120 | 4,672 |
| … at 1.5× (2880×1620 px) | 8.72 | 1.60 | 10.32 | 13.1 | 3,133 | 50,276 | 64,602 | 51 | 28,639 | 5,636 |
| … at **2× (4K)** | 9.02 | 1.62 | **10.64** | 13.3 | 3,133 | 50,276 | 64,602 | 51 | 28,639 | 5,636 |
| … feathering off | 9.87 | 1.07 | 10.94 | 16.7 | 3,133 | 31,509 | 24,633 | 51 | 28,639 | 5,636 |

**Piano roll and channel rack:**

| View (default layout: transport, arrangement, inspector, status) | Build ms | Tess ms | **Total ms** | p95 total | Shapes | Vertices | Triangles | Draws | Allocs/frame | KB/frame |
|---|---|---|---|---|---|---|---|---|---|---|
| Piano roll, welcome project | 0.82 | 0.44 | **1.27** | 1.8 | 823 | 19,804 | 25,000 | 41 | 1,435 | 288 |
| Piano roll, 64 tracks | 1.69 | 0.51 | 2.20 | 2.9 | 1,548 | 24,132 | 27,572 | 44 | 3,703 | 1,202 |
| Piano roll, **stress project** (8 × 20,000 notes), idle | 8.52 | 6.15 | **14.67** | 17.6 | 1,220 | 205,570 | 138,635 | 28 | 1,535 | **19,970** |
| Channel rack | 1.16 | 1.72 | **2.89** | 4.5 | 1,467 | 56,422 | 79,061 | 23 | 1,573 | 368 |

**Plug-in editors, open over the 16-strip mixer:**

| View (default layout: transport, arrangement, inspector, status) | Build ms | Tess ms | **Total ms** | p95 total | Shapes | Vertices | Triangles | Draws | Allocs/frame | KB/frame |
|---|---|---|---|---|---|---|---|---|---|---|
| EQ editor | 3.56 | 2.22 | **5.78** | 6.8 | 2,583 | 68,558 | 93,024 | 46 | 6,995 | 1,582 |
| Compressor editor | 3.77 | 2.34 | 6.11 | 7.3 | 2,287 | 73,680 | 98,204 | 50 | 7,778 | 1,629 |
| Analog synth editor | 3.31 | 2.19 | 5.50 | 6.6 | 2,550 | 70,380 | 95,850 | 69 | 7,382 | 1,288 |
| Wavetable editor | 3.80 | 2.55 | 6.34 | 8.2 | 2,050 | 74,732 | 103,676 | 52 | 6,974 | 1,303 |

Every case ran exactly **one layout pass per frame**: no `request_discard`
multi-passing. The dark theme costs the same as the light one.

### 2.2 Each view alone, inside a central panel (1920×1080)

| View | Build ms | Tess ms | Total ms | Allocs/frame |
|---|---|---|---|---|
| Mixer, 16 strips | 2.07 | 0.92 | 2.98 | 4,810 |
| **Mixer, 64 strips** | **7.23** | 0.86 | **8.09** | **25,165** |
| Arrangement, welcome project | 0.13 | 0.08 | 0.21 | 374 |
| Arrangement, 64 tracks | 1.02 | 0.31 | 1.33 | 2,542 |
| **Arrangement, stress project** | **5.66** | 3.86 | **9.51** | 532 allocations, but **18,408 KB/frame** |
| Piano roll | 0.11 | 0.14 | 0.25 | 194 |
| Channel rack | 0.24 | 0.96 | 1.20 | 323 |
| Inspector | 0.35 | 0.15 | 0.50 | 658 |
| Transport | 0.09 | 0.03 | 0.12 | 220 |
| EQ editor window | 0.69 | 0.29 | 0.98 | 797 |
| Compressor editor window | 1.10 | 0.45 | 1.55 | 1,574 |
| Analog synth editor window | 0.92 | 0.57 | 1.48 | 1,489 |

### 2.3 GPU cost (egui-wgpu replay of the same frames, timestamp queries, median of 200–400 frames)

These are GPU pass times in ms, as a range over two runs. GPU clocks on this laptop
move a lot between runs, so a range is honest.

| Frame | Iris Xe, Vulkan | Iris Xe, OpenGL | Iris Xe, DX12 | RTX A2000, Vulkan | RTX A2000, DX12 |
|---|---|---|---|---|---|
| Mixer 16, 1080p | 0.40–1.42 | 0.82–1.34 | 1.47–1.72 | 0.28–1.13 | 0.88–1.10 |
| Mixer 64, 1080p | 0.47–1.47 | 1.05–1.39 | 1.35–1.79 | 0.30–1.21 | 1.10–1.32 |
| **Mixer 64, 4K** | 1.47–4.84 | 4.76 | 4.69–5.36 | 1.03–3.88 | 2.66–2.86 |
| EQ editor over mixer, 1080p | 2.18–2.19 | 1.41–2.10 | 2.33–2.71 | 0.65–0.71 | 0.74–0.90 |
| Channel rack, 1080p | 0.75–1.99 | 1.86 | 1.72–1.96 | 0.38–1.35 | 1.04–1.44 |

What the GPU numbers say:

- **The GPU is not where the time goes.** The work is **fill-bound**: 50,000 vertices
  is nothing to a GPU, and 4K costs 3–4× what 1080p does, which is the pixel count.
  The upload is about 1.8 MB a frame (50k vertices × 20 B plus 64k triangles × 12 B).
- **So GPU cost is driven by overdraw.** Large translucent layers, big blurred
  shadows, and full-panel fills stacked on top of each other are what cost. Vertex
  count hardly matters.
- **At 4K on an integrated GPU the frame needs 5 ms.** That is fine for 60 Hz, but it
  is a real share of a 144 Hz budget (6.9 ms).

### 2.4 How much of the CPU is spent while nothing happens

`app.rs:3869` asks for a repaint every **100 ms when idle** and every **16 ms while
playing**. On top of that:

- `mixer.rs:215` repaints continuously while any meter is still falling.
- Every open editor calls `ctx.request_repaint()` unconditionally (`eq_editor.rs:1024`,
  `comp_editor.rs:846`, `fx/mod.rs:452`, `inst/frame.rs:136`). With vsync, that means
  **repainting at the monitor's rate**: 60 frames a second on this panel, 144 on a
  gaming monitor.

With the costs measured above, the cost per core looks like this:

| Situation | Frames per second | CPU (one core) |
|---|---|---|
| Idle, 16-strip mixer | 10 | 3.7 ms each, about 4 % |
| Idle, stress project | 10 | 14.7 ms each, about 15 % |
| An editor open at 60 Hz | 60 | 6 ms each, about 35 % |
| An editor open at 144 Hz | 144 | 6 ms each, about 85 % |

---

## 3. Hot spots, ranked by what fixing them buys

1. **The mixer lays out every strip, visible or not.**
   - `views/mixer.rs:213` runs `strip()` for all of them inside a horizontal
     `ScrollArea`.
   - egui's tessellator culls what is outside the clip rectangle, so the tessellation
     of the 64-strip view stays flat (0.86 ms). The build does not: each extra strip
     costs **≈ 0.107 ms and ≈ 420 allocations**, even though only about 13 strips are
     on screen at 1920 px.
   - **Fix:** allocate each strip's rect, and draw its body only if
     `ui.is_rect_visible(rect)`, the way `NoteLod` did for the piano roll (ADR 0011).
   - **Expected result:** the 64-strip mixer lands near the 16-strip mixer's
     **≈ 3 ms**.
2. **The sends "+" menu builds its list every frame, for every strip.**
   - `mixer.rs:1149-1160` computes the `targets` list outside the `menu_button`
     closure. That is, for each strip, `routing_would_cycle` over every other strip.
   - `routing_would_cycle` is itself a graph walk whose `downstream()` scans all
     strips and allocates three `Vec`s (`cantus-core/src/model.rs:988-1022`). That
     makes it **O(N³)**.
   - **Measured at 64 strips: 2.83 ms and 8,064 allocations a frame. That is 39 % of
     the mixer's build time**, spent on a list nobody is looking at.
   - **Fix:** compute it inside the closure, as `output_picker` already does.
3. **Per-frame clones and strings.** Examples:
   - `strip(id).cloned()` for each strip (`mixer.rs:778`, which clones its effects,
     sends and name);
   - `fed_by: Vec<String>`;
   - tooltips formatted eagerly on every hover-able widget
     (`on_hover_text(tf(...))`);
   - `t()` returning a cloned `String` (`i18n.rs:171`; measured at **1.35 µs per
     `t` + `tf` pair**);
   - the arrangement cloning **the whole `Track`, clips and notes included**, for every
     row, every frame (`arrangement.rs:455`). On the stress project that is
     **18 MB copied a frame**.

   **Fix:**
   - borrow instead of cloning;
   - use `&'static str` or `Arc<str>` lookups for translations;
   - build tooltips lazily (`on_hover_ui(|ui| …)`, which only runs on hover).
4. **Continuous repaint** from the editors, and the 10 fps idle tick (§2.4).
   - **Fix:** use one repaint policy (§7.1).
5. **Knob tessellation.**
   - Today's `Knob` draws two 48-segment arcs as stroked paths, plus two circles and
     two texts. That is **≈ 490 vertices and 50–60 µs of tessellation per knob**.
   - 256 knobs cost 15.6 ms (§5.1). The 20–40 knobs of an instrument panel cost
     1.5–2.5 ms at 60 frames a second.
   - **Fix:** use a texture for the knob body, fewer arc segments (one per ≈ 3 px of
     arc), and batch the arcs (§5.1).
6. **The channel rack's cells** tessellate to 79,000 triangles (1.7 ms). Rounded,
   feathered rectangles cost about 4 µs each (§5.4).
   - **Fix:** batch the cells into one pre-coloured mesh, the way the note LOD does.
   - **Expected result:** a 2.5× cheaper tessellation step, as measured for batched
     rounded rectangles in §5.4.

What is **not** a hot spot:

- **Feathering** (anti-aliasing): turning it off saves 0.45 ms out of 9.7, and costs
  the look.
- **Theme:** dark and light cost the same.
- **Scale:** 2× costs +10 %.
- **Font rasterisation:** glyphs are cached in the atlas; the text *layout* cache is
  what text costs, and it is cheap (§5.2).
- **Multi-pass layout:** always exactly 1 pass.

---

## 4. What epaint 0.36 can do for a hand-vectorized look

epaint 0.36 is a triangle tessellator with 1-physical-pixel "feathering" for
anti-aliasing. Colours are premultiplied and live in gamma space. It draws one textured
triangle list per clip rectangle. For glyphs, **epaint 0.36 already embeds a proper 2D
rasteriser, `vello_cpu`, with `kurbo`**. That rasteriser matters for the workarounds
below.

### 4.1 Capability matrix

| Need | What epaint gives | Quality (checked in the rendered sheets, §4.3) | Cost (measured) | Workaround when it falls short |
|---|---|---|---|---|
| **Linear gradient** | `Mesh` with per-vertex colours. Interpolation is exact for linear ramps. `glow` dithers by default (`NativeOptions::dithering = true`), so there is no banding. | Clean | Batched mesh ≈ free (64 gradient meters in 0.04 ms) | — |
| **Gradient inside a rounded, anti-aliased shape** | Not directly | Clean, if you tessellate a white `RectShape` with `Tessellator` and then recolour each vertex by `y` (keeping the feather alpha) | 7.9 µs per rect if re-tessellated every frame; ≈ 1.5 µs once cached as a mesh | Cache the recoloured mesh (§7.3) |
| **Radial gradient** | Triangle fan with centre and rim colours | OK for large, soft fields; faceted on small bright highlights | Cheap | `vello_cpu` radial gradient into a texture (knob caps, LEDs) |
| **Gradient stroke** | `PathStroke::new_uv(width, closure)` (`ColorMode::UV`) | Clean (a green-to-red arc in the sheet) | Same as a plain stroke | — |
| **Anti-aliasing** | 1-px feather on every path, rect, circle and line, at any scale. Text uses the glyph atlas. **Meshes you build yourself are not anti-aliased.** MSAA is off (`multisampling: 0`). | Crisp edges at 1×, 1.5× and 2× | Feathering roughly doubles triangles: 64.5k → 24.6k when off | Give your own meshes a 1-px feather ring, or draw their edge with an epaint stroke on top |
| **Drop shadow / soft edge** | `Shadow::as_shape` and `RectShape::with_blur_width`. This is a *linear* ramp, not a Gaussian. | Good up to about 12 px. Large blur on small rects is clamped and shows **diagonal creases at the corners**, visible in the sheet's glow. | 64 cards with a 12-px shadow: +0.42 ms (≈ 6.6 µs each) | Real Gaussian: `vello_cpu::RenderContext::fill_blurred_rounded_rect` into a texture, cached per size |
| **Glow** | Same blur rect, tinted | Creases at the corners, as above | — | `vello_cpu` blurred texture, drawn additively-looking over a dark surface |
| **Bevels and highlights** | Layered strokes: a 1-px light line inside the top edge, a dark outline, a gradient body | Convincing (the "MASTER" button in the sheet) | 3 shapes | — |
| **Per-corner radii** | `CornerRadius { nw, ne, sw, se }`, **whole points** (`u8`) | Clean | Rounded rect ≈ 4 µs (40 vertices) | Fractional radii need your own path |
| **Stroke kinds** | `StrokeKind::Inside / Middle / Outside` for rects. Paths have a stroke kind. No cap or join options: caps are butt, joins are miter. `Shape::dashed_line` and `dotted_line` exist. | 1-pt strokes are crisp at 1× and 2×, **soft at 1.5×** (§6) | — | Round caps: add a circle per end. Hairlines: `Stroke::new(1.0 / ppp, …)` |
| **Concave fills** (letterforms, logos, icons) | `PathShape` fills **convex polygons only** | — | — | `vello_cpu` or `resvg` texture (anti-aliased), or `lyon` (aliased, §5.6) |
| **Clipping** | Axis-aligned rectangles only | — | — | Masks go through textures |
| **Blend modes** | Premultiplied "over" only. No add, multiply or screen. | — | — | Bake into textures |
| **Rotation** | `RectShape::angle`, `TextShape::with_angle` (the mixer's group names already use it) | Clean | — | — |

### 4.2 Text in 0.36 (and what it changes)

Between egui 0.34 and 0.36:

- egui **replaced `ab_glyph` with `skrifa` + `vello_cpu` and enabled hinting** (#7694);
- added a **font-variations API** (#7859);
- moved **shaping to `harfrust`** (#8031), which gives GPOS kerning and ligatures;
- added **sub-pixel binning** and **configurable hinting** (#8072, #8262);
- added `extra_text_line_spacing` (#8040).

For Cantus that means:

- **Hinting works on Cantus's fonts.** The bundled IBM Plex TTFs ship TrueType
  instructions (their `fpgm`, `prep` and `cvt` tables were checked), so epaint hints
  them with the TrueType interpreter. The default is `SmoothHinting { light: false,
  symmetric_rendering: true, preserve_linear_metrics: true }`. Crops at 1× (zoomed
  3×) show sharp stems at 10.5–13 px. `FontTweak::hinting_target` can try the "light"
  mode per face.
- **Weights.**
  - `FontId` has no weight. A weight is a *family*, which is what Cantus does today
    with `family_strong()`. To get Regular, Medium and SemiBold, add faces as named
    families:
    - `sans` 400, `sans-medium` 500, `sans-semibold` 600;
    - `mono` 400, `mono-medium` 500.
  - Each static Plex face is about 200 KB. Alternatively, ship **one variable face**
    and choose the weight per run: `RichText::variation("wght", 500.0)`,
    `TextFormat::coords`, or `FontTweak::coords` per family.
  - Variable TTFs are usually unhinted, so epaint's auto-hinter takes over. That is
    acceptable at 2×, softer at 1×.
  - **Recommendation:** static faces for 400, 500 and 600, because they are hinted.
- **Letter spacing.** Use `RichText::extra_letter_spacing(pt)` or
  `TextFormat::extra_letter_spacing`, which is applied at cluster boundaries. Caps
  labels ("SENDS · INSERTS · MASTER") read best at **+0.6 pt** for 10.5 px semibold;
  +1.2 is too loose (compare the sheet).
- **Tabular figures.** epaint shapes with `harfrust::ShapeOptions::new()` and exposes
  **no OpenType feature switches** (`epaint/src/text/text_layout.rs:1433`). `tnum`,
  `ss01`, `zero` and the rest cannot be turned on.
  - This does not matter for Plex Sans: **its default figures are tabular**. All ten
    digits advance 7.8125 px at 13 px, and the GSUB table has no `tnum` feature to
    switch. Proportional digits therefore line up in columns anyway.
  - For stylistic sets (for example Plex's slashed zero), freeze the feature into a
    derived font at build time (fontTools `pyftfeatfreeze`; the result stays under
    OFL) and register it as its own family.
- **Hot-path cost.**
  - `painter.text("…")` with a static string: **1.4 µs** a label (a lookup in the
    galley cache and 2 allocations).
  - A cached `Arc<Galley>` through `painter.galley`: **0.39 µs**, 3.6× cheaper.
  - `format!` of a changing value, then `text`: **2.0 µs**.
  - `ui.label`: about 3.3 µs.
  - The kit should lay out labels once per (text, font, colour) and keep the galleys.
    Readouts that change can stay `painter.text`, as long as they are not formatted
    when they did not change.

### 4.3 The quality sheet (1×, 1.5×, 2×)

`micro sheet` renders the same composition three times through `cantus_ui::raster`:

- rounded rects with per-corner radii, in three stroke kinds;
- recoloured gradient buttons (light, and a dark bevelled "MASTER");
- shadows at 4, 12 and 24 px blur, and a glow;
- today's `Knob` beside a knob whose body is a `vello_cpu` texture (radial-gradient
  cap, Gaussian drop shadow, rim);
- a gradient stroke;
- three icons rasterised from SVG path data by `vello_cpu`, beside `icons.rs`'s
  painted icons;
- a type specimen.

What it shows:

- **Text is crisp at every scale**; 10.5 px Plex Sans is still legible at 1×.
- **The textured knob is the one that looks like FL.** The plain one looks like a
  diagram.
- **A 1-pt border is soft at 1.5×.** Measured on the pixels, it becomes one full-colour
  pixel plus one at about 50 %.
- **The epaint glow has corner creases.** Use a texture for any blur above about 12 px.
- **`vello_cpu` icons are cleanly anti-aliased at 16 px.** The painted icons are too,
  but they are hand-coded geometry.

A small caveat about the tool: `raster.rs` samples textures with nearest-neighbour, and
its edge test is inclusive on every edge (no top-left rule), so pixels on a shared
triangle edge can be blended twice. One gradient rect showed a one-pixel seam at 2×
that a GPU would not draw. Good enough for review; see §9.4 for making it good enough for golden
tests.

---

## 5. Measured techniques (256 knobs, 2,000 labels, 1,000 buttons, 64 meters…)

All at 1920×1080, 1×, one P-core, medians of 200 frames.

### 5.1 Knobs: why the static body belongs in a texture

| 256 knobs, values moving | Build ms | Tess ms | **Total ms** | Vertices | Triangles |
|---|---|---|---|---|---|
| Today's `Knob` widget | 3.61 | 12.04 | **15.64** | 125,892 | 177,678 |
| The same painting, no widget | 3.23 | 15.17 | 18.39 | 125,852 | 177,602 |
| Static part cached as one `Arc<Mesh>` in local coordinates, copied with an offset | 4.42 | 10.25 | 14.67 | 125,852 | 177,602 |
| **Body as a `vello_cpu` texture** (gradient, shadow, rim), one batched quad each | **1.29** | **2.75** | **4.05** | 26,012 | 27,842 |

- Rasterising the knob body costs **0.72 ms once per (size, theme)**.
- Caching epaint geometry barely helps: the copy costs nearly what tessellating costs,
  and the moving arc is the rest.
- The texture removes about 100,000 vertices and makes the knob look better.
- What remains is the value arc, drawn with 48 segments per 270°. Two further steps:
  - generate the arc with about **one segment per 3 physical pixels** (a 36-px knob
    needs about 20, not 48);
  - write it straight into one batched mesh with a hand-rolled feather ring, instead
    of one `Shape::line` per knob.

### 5.2 Text

See §4.2: **1.4 µs** per `painter.text`, **0.39 µs** per cached galley, and **2.0 µs**
when formatting a changing number.

### 5.3 Widgets

| 1,000 items | Build ms | Tess ms | Total ms |
|---|---|---|---|
| `ui.button("Mute")` in a `Grid` | 6.39 | 7.64 | **14.02** (131k triangles) |
| `ui.interact` + one rounded rect + one text | 2.33 | 6.28 | **8.62** |

An egui `Button` costs about **14 µs**. A mixer of 64 strips with 8 buttons each is
already 7 ms. The kit's switches (mute, solo, FX and the like) should be painted
directly on an `interact` rect, and batched.

### 5.4 Depth and fills

| Item | Build ms | Tess ms | Total ms | Vertices |
|---|---|---|---|---|
| 64 cards, flat | 0.04 | 0.27 | 0.30 | 2,568 |
| 64 cards with a 12-px `Shadow` | 0.06 | 0.67 | 0.73 | 7,176 |
| 1,000 flat rounded rects (4-pt radius) | 0.30 | 3.76 | 4.06 | 40,008 |
| 1,000 gradient rounded rects, tessellated and recoloured **every frame** | 7.86 | 1.49 | 9.34 | 40,008 |
| (the same as a cached mesh: only its copy is paid) | — | ≈ 1.5 | ≈ 1.5 | 40,008 |

Two things follow:

- **Rounded rectangles cost about 4 µs each to tessellate.** The same triangles copied
  from a cached mesh cost about 1.5 µs, which is 2.5× cheaper. That is the win for the
  channel rack's cells and the note grid.
- **Shadows are affordable**, for the few floating surfaces that should have one.

### 5.5 Meters

| 64 meters | Build ms | Tess ms | **Total ms** | Vertices |
|---|---|---|---|---|
| As today: 8 shapes each (well, bar, hold, 4 ticks, border) | 0.16 | 0.99 | **1.15** | 11,760 |
| **One batched mesh**: well plus a two-segment gradient bar (green→amber→red) | 0.03 | 0.01 | **0.04** | 592 |

That is **30× cheaper, and it looks richer** (FL's meters are gradients). The scale
ticks and the border do not change, so they belong to the static layer.

### 5.6 Icons and logos: rasterise, don't tessellate

- **Rasterising with `vello_cpu`** (SVG `d` strings parsed by `kurbo::BezPath::from_svg`,
  then filled or stroked): **42–51 µs per icon** at 16–48 px, parsing included.
  - A full set of 60 icons × 3 sizes × 3 scales takes **26 ms**, so do it lazily, per
    icon and per pixel size, on first use.
  - It adds **no new crate**: `vello_cpu` and `kurbo` are already compiled into epaint
    0.36. They only need declaring, at the same versions.
- **Tessellating with `lyon`** (only possible for flat art): the flat logos give
  188–602 triangles in 43–157 µs at 256 px.
  - The result is **aliased**. At 32 px the edges are visibly stair-stepped next to
    `resvg`'s (see `lyon_vs_resvg.png` in the harness output).
  - It needs MSAA or a hand-made feather, and cannot do filters or gradients.

---

## 6. UI scale (global zoom, as in FL)

Cantus already has a zoom: `settings.ui_scale`, 0.7–2.0, applied with
`ctx.set_zoom_factor` (`app.rs:231`, `app.rs:527`). `pixels_per_point` is the zoom
times the monitor's native scale. FL offers "Main GUI scaling" (system, main, or a
custom %), separate pop-up and toolbar scaling, and "Thick lines", which applies from
200 % up (FL manual, *System Settings > General*).

**What scale costs.** Build and tessellation are in points, so the CPU barely moves:
+7 % at 1.5× and +10 % at 2×. GPU fill grows with the pixel count: 4K costs 3–4× what
1080p does (§2.3).

**What stays crisp.**

- **Text:** re-rasterised at the physical size, with hinting.
- **Rects:** snapped to pixels (`round_rects_to_pixels`).
- **Axis-aligned lines:** snapped (`round_line_segments_to_pixels`).
- **Text positions:** snapped (`round_text_to_pixels`).
- **Circles and arcs:** fine at any scale. Their segment count depends on the radius in
  points, which is fine up to 2×.

**What breaks at fractional scales (1.25×, 1.5×, 1.75×):**

- **Hairlines.** A 1-pt border at 1.5× is 1.5 px: one full pixel plus a half-coverage
  pixel. That is the soft look (measured, §4.3).
  - Decide it per token: `hairline = 1 physical pixel` (`Stroke::new(1.0 / ppp, c)`).
  - Round other widths to whole pixels: `(w * ppp).round().max(1.0) / ppp`.
  - Adopt FL's "thick lines" rule: at ≥ 2× a hairline becomes 2 px.
- **Your own meshes and paths are not snapped.** egui rounds UI coordinates to 1/32 pt
  (`GUI_ROUNDING`), not to pixels. Kit painters must snap the rects they build:
  `rect.round_to_pixels(ppp)`.
- **Textures sized in points.** Every cached raster (icons, knob bodies, logos) must be
  **keyed by its physical pixel size** and rebuilt when `pixels_per_point` changes.
  Otherwise it is resampled and blurs.
- **egui's own Ctrl +/−/0 zoom is still on.** `Options::zoom_with_keyboard` defaults to
  true and Cantus never turns it off, so the keys change the zoom behind the settings
  dialog's back. The value is not saved. Either turn it off, or route those keys to
  `apply_ui_scale`.
- **At 0.7× the hit targets get small.** A 20-pt dense row becomes 14 px. Keep 0.7×
  for very large screens only.

**Recommendation.** Offer 100, 125, 150, 175 and 200 % plus a custom value, as FL does.
Make hairlines one physical pixel below 200 % and two from 200 %.

---

## 7. Performance techniques and the policy to adopt

### 7.1 Repaint only on change, with one ticker

- Replace the scattered `request_repaint()` calls with **one ticker in `CantusApp`**:
  views say *why* they need the next frame (meters, animation, playhead, plug-in
  pump), and the app requests `ctx.request_repaint_after(next_deadline)` once.
- Meters and playhead: **≤ 60 Hz regardless of the monitor**. Offer FL's choice: full,
  ½ or ¼ of the monitor rate, which FL calls *Ultrasmooth*, *Smooth* and *Less smooth*
  under "Animation refresh rate".
- **Idle means no frames.**
  - Input events already wake eframe.
  - The theme watcher and autosave can use a 1-s tick that only repaints when
    something changed.
  - Plug-in pumping (CLAP `request_callback`) should wake the loop through
    `ctx.request_repaint()` from the host thread, not by polling at 30 Hz.
- An editor repaints when its analyser or meter data changed, and never
  unconditionally.
- When the window is unfocused and the transport is stopped, drop meters to 15 Hz.
- egui's `animate_bool_with_time` and `animate_value_with_time` request their own
  repaints only while they are moving. Use them for hover and press fades.

### 7.2 Meters at 60 fps, only for visible strips

- A meter frame is a *whole* frame: immediate mode cannot repaint a region. So the
  frame must be cheap, which is §3's work.
- Cull strips that are off screen (§3, item 1).
- Draw all meters as **one mesh per dock** (§5.5).
- Keep the ballistics (`MixerView::ballistic`) per strip in a flat `Vec`, as now.

### 7.3 Cache static layers

Three levels, in increasing cost to build and decreasing cost per frame:

1. **Cached galleys.** `Arc<Galley>` for labels and titles, keyed by (text, font,
   colour, wrap width).
2. **Cached meshes.** `Arc<Mesh>` in *local* coordinates, for geometry that does not
   change and does not need a texture:
   - scale ticks;
   - panel frames;
   - rack cell outlines;
   - recoloured gradient buttons.

   Copy them into one per-view batch mesh with an offset. Tessellating is then just a
   memory copy. Note egui's own comment: re-using last frame's tessellation wholesale is
   *not* worth it, because comparing shapes costs half as much as tessellating them
   (`egui/src/context.rs:2863`). Cache at the application level.
3. **Cached textures.** `vello_cpu` or `resvg` rasters for rich static art:
   - knob bodies;
   - bevel plates;
   - large soft shadows and glows;
   - icons;
   - logos.

   Key them by (asset, size in physical px, state, theme generation). Pack small ones
   into one atlas texture, so they share a draw call. Build them lazily; they are
   rebuilt only on a theme or scale change.

### 7.4 No per-frame allocation or formatting

- Borrow the project; never clone a `Track`, `MixerStrip` or `Vec<String>` to draw it.
- Translations as `&'static str` (intern the tables at load time). `tf` should write
  into a reused `String` buffer.
- Tooltips lazily: `on_hover_ui(|ui| …)`, whose closure only runs when the tooltip
  shows. `on_hover_text(tf(…))` formats its string every frame.
- Readouts: format only when the value changed. Keep a small
  `(last_value, Arc<Galley>)` cache per readout.
- Batch meshes that are reused keep their capacity:
  `mesh.clear()` + `reserve_*`.
- Target: **≤ 1,500 allocations a frame** in steady state (today 6,000–28,600).

### 7.5 `glow` or `wgpu` in eframe 0.36

eframe made `wgpu` its default in 0.34. Cantus explicitly compiles `glow`, and should
keep it:

- **The plug-in editors already run egui on OpenGL.** They use `egui-baseview` with
  `features = ["opengl"]` (`crates/cantus-plugins/Cargo.toml`). One renderer means one
  kit, with no paint-callback code paths.
- **The GPU work is small either way** (§2.3).
- **egui#7761 measured `wgpu` against `glow`:**
  - release build time 42 s against 20 s, and binary size 27.7 MB against 13.9 MB;
  - RAM 90–142 MB against 31 MB, and VRAM 215–265 MB against 31 MB;
  - start-up "anywhere from 1 to about 7 seconds" against glow's 100–300 ms;
  - about 3 frames of input latency against about 2.

Revisit only if Cantus needs compute shaders, for example GPU spectrograms, or has to
leave OpenGL on macOS.

### 7.6 vsync and frame pacing

- `glow` swaps with vsync on (`GlowConfiguration::vsync`, default `true`). Keep it: a
  DAW must not tear, and waiting for vsync costs no CPU.
- Frame pacing then comes from the ticker. With `request_repaint_after(16 ms)`, a 60 Hz
  display presents every vblank. A 144 Hz display presents every 2–3 vblanks, which is
  fine for meters.
- Never call `request_repaint()` in a loop: on a 144 Hz display it multiplies the CPU
  by 2.4 for nothing.
- eframe's `frame.info().cpu_usage` gives the CPU time per frame, excluding the vsync
  wait. Feed the existing `show_stats` overlay from it.

### 7.7 Measuring GPU cost

- **Offline, repeatable: the replay harness used here.**
  - Dump `ClippedPrimitive`s and the font atlas from a headless frame.
  - Replay them through `egui-wgpu` with `Features::TIMESTAMP_QUERY` on every adapter.
  - About 200 lines of code; it stays outside the app.
  - The GL shader egui uses is the same trivial textured-triangle shader on either
    backend, so the numbers carry over to `glow`.
- **In the app:**
  - OpenGL timer queries (`GL_TIME_ELAPSED` through `glow`) around the paint, shown in
    the stats overlay;
  - or RenderDoc, Intel GPA or Nsight captures of the running app;
  - PresentMon for present-to-present pacing on Windows.
- **Budget it as fill.** Count the overdraw of full-panel fills. Avoid stacking
  translucent full-screen layers, and big blurs.

---

## 8. The budget and how to hold it

### 8.1 Proposed budget

On one P-core of a 2022 mobile i7 (this machine), release build, 1920×1080, 1×:

| Scenario | Today | **Budget, CPU p50 / p95** | Also hold |
|---|---|---|---|
| Mixer, 64 strips, meters running, arrangement and inspector shown | 9.7 ms | **≤ 4 ms / ≤ 6 ms** | ≤ 1,500 allocations, ≤ 256 KB, ≤ 120k triangles, ≤ 64 draw calls |
| Mixer, 16 strips, playing | 3.2 ms | ≤ 2.5 ms / ≤ 4 ms | ≤ 1,000 allocations |
| An effect or instrument editor over the mixer | 5.5–6.3 ms | ≤ 4 ms / ≤ 6 ms | the editor window alone ≤ 1.5 ms |
| Piano roll or arrangement, stress project (8 × 20k notes) | 14.7 ms | ≤ 8 ms / ≤ 11 ms | **no copy of notes a frame** (≤ 256 KB) |
| 4K (2×), mixer 64 | 10.6 ms | ≤ 4.5 ms | GPU ≤ 6 ms on Iris Xe-class |
| Idle, nothing changing | 10 frames/s | **0 frames/s** | — |
| Meters, playhead, animation | monitor rate (editors) | **≤ 60 Hz**, user choice of 60 or 30 | — |
| GPU pass, 1080p | 0.3–2.7 ms | ≤ 2 ms on Iris Xe-class | fill-bound: watch overdraw |

This leaves more than 10 ms of a 60 Hz frame free for plug-in pumping, file work and
the audio thread's neighbours. The shipped app will run on slower machines than this
one; a 4-ms budget here is about 8 ms on a 2016-class laptop.

### 8.2 The test that holds it

Add a `crates/cantus-ui/tests/frame_budget.rs` in the style of today's
`stress_piano_roll_frame_cost`:

- **Harness.** Build `CantusApp::from_parts(project, EngineHandle::manual(48_000))`,
  keep the `Processor` and process 800 frames of audio before each UI frame, so the
  meters and playhead move. Drive the full window with `eframe::App::ui(&mut app, root,
  &mut eframe::Frame::_new_kittest())`, exactly as §11 does.
- **Scenarios:**
  - `mixer64`: the welcome project grown to 64 strips and tracks;
  - `mixer16`;
  - `editor-eq`;
  - `stress-pianoroll`.
- **Deterministic assertions, which run in every `cargo test`:**
  - allocations a frame, from a counting `#[global_allocator]` in that test binary;
  - bytes allocated a frame;
  - strips laid out (add a counter, like `piano_roll_notes_drawn`);
  - triangles and draw calls;
  - that the idle frame requests no repaint:
    `viewport_output[..].repaint_delay == Duration::MAX`.

  These are stable across machines, so they catch regressions without flakiness.
- **Timing assertions** (`#[ignore]`, run in the release CI job): the median of 200
  frames of build plus tessellate ≤ 2× the budget, a generous ceiling for CI noise.
  Print the p50 and p95.
- **Trend.** Add a `benches/ui_frame.rs` with the workspace's `criterion`, for the same
  scenarios.

---

## 9. The component kit

### 9.1 Architecture

```
cantus-ui/src/kit/
  seeds.rs    ThemeSeeds  (FL-style: hue, saturation, lightness, contrast, text, accent,
                           selection, highlight, mute/option LEDs, meter bands)
  tokens.rs   Tokens      (derived, Copy, one per frame): surfaces, lines, text, accents,
                           per-state deltas, radii, hairline in px, fonts, motion times
  look/       pure painters: fn knob(p, rect, &KnobLook, state, value, &Tokens, &mut Batch)
              knob, fader, button, toggle, tab, meter, strip_header, panel_frame,
              scrollbar, value_field, led, section_header
  widget/     thin interaction wrappers → Response + State, then call the look
  batch.rs    per-view batched meshes (solid and atlas-textured), cleared and reused
  cache.rs    LayerCache: galleys, local-space meshes, textures, keyed by
              (asset, size_px, state, theme_gen, ppp); rasterised with vello_cpu/resvg
  anim.rs     hover, press and toggle fades through ctx.animate_*; value smoothing
  gallery.rs  every component × state × theme × scale, for tests and design review
```

The rules:

- **Looks are pure.** Given a rect, a state, a value and the tokens, a look draws the
  same thing. No input, no `ui`. That makes each state testable without simulating the
  mouse, and it lets the plug-in windows reuse the same painters.
- **Widgets own interaction** and translate it into a `State`. The existing `Knob`,
  `Fader` and `Bar` keep their behaviour (drag, shift for fine control, double-click to
  reset, arrow keys, right-click to type a value), which lives in a shared helper.
- **Static parts come from the cache; moving parts go into the per-view batch.** One
  `Shape::Mesh` for solids and one for textured atlas quads per view, plus text.
- **Everything lands on the pixel grid.** Painters snap their rects to physical pixels
  and use the hairline token.

### 9.2 Theme tokens with FL-style H/S/L

FL's theme is a handful of global knobs plus specific pickers:

- global: *Hue* ("overall color tint of the UI"), *Saturation*, *Brightness*,
  *Contrast*, *Text*;
- pickers: *Selection*, *Highlight*, *Mute* and *Option* LEDs, step colours (odd and
  even), meters per dB band, waveform colours;
- a light-mode switch and A/B comparison (FL manual, *Theme Settings*).

Cantus has 22 hex tokens (`theme.rs`), a WCAG test over them, and a live-reloaded
`theme.json`. Keep all of that. Add a layer in front:

1. **`ThemeSeeds`** hold the global knobs and the pickers.
2. **`derive(seeds)`** produces the neutral ramp: `ink`, `panel`, `panel_raised`, the
   three grid levels and the text colours.
   - Take them from one neutral hue and saturation, with lightness steps scaled by
     *contrast*.
   - Do this in **OKLCH** rather than HSL, so equal steps look equal and a hue change
     does not change perceived brightness.
3. **Accents** come from their own seeds.
4. **Hex tokens in `theme.json` become optional overrides** on top of the derived
   palette. Every existing theme file keeps working.
5. **The WCAG test becomes a property test.** For a grid of seed values, the derived
   palette must pass. Where it would not, `derive` clamps lightness until it does
   (FL's "High visibility" in effect, always on).
6. **A theme generation counter** is bumped on every apply. Caches key on it.
7. **Track colours stay user data.** The kit only *tints* surfaces with them (the
   existing `theme::tint`), so the accent stays free to mean *state*.

### 9.3 States and animation

| State | Visual delta (from tokens) | Motion |
|---|---|---|
| Rest | base | — |
| Hover | +1 lightness step on the surface; the edge goes from `grid_bar` to `text_dim` | fade 90–120 ms |
| Active (pressed or dragging) | accent edge, pressed surface −1 step, the value text brightens | press 60–80 ms, release 150 ms |
| On (toggle, LED) | accent fill or LED glow (a cached glow texture) | 120 ms |
| Focus (keyboard) | 2-px focus ring outside, as `design::paint_focus_ring` does today | none |
| Disabled | 40 % opacity for text and fill, no hover | — |
| Selected (strip, clip) | selection token on the edge, tinted fill | 120 ms |

- Use `ctx.animate_bool_with_time(id, hovered, 0.1)` and `animate_value_with_time`;
  egui repaints only while they run.
- Value smoothing for knobs driven by automation is FL's "Smoothing": one exponential
  step per frame, repainting until it settles.
- Respect a "reduce motion" setting. FL has "Animations: *Don't distract me* …
  *Entertain me*".

### 9.4 Testing it with the software renderer

- A `kit_gallery` example, plus a test that renders the gallery for
  {Paper, Ink & Gold} × {1, 1.5, 2} × every state, through `raster::shoot`.
- **Golden images** in `tests/golden/kit/` (small PNGs, one per component per theme).
  Compare with a tolerance: the maximum per-channel difference, and the count of
  pixels that differ. An intended change updates the goldens with an env var, and the
  review shows before and after, the way round 14 asked for.
- **Structural assertions**, independent of the pixels:
  - the contrast of each label against its fill, computed from the tokens;
  - a focus ring is present for the focused state (as `design.rs` already tests);
  - hairlines are exactly one device pixel wide at 1.5×, found by scanning a row of
    pixels, as this report did;
  - no text overflows its cell, using galley width against the rect. That catches the
    audit's "letters run into each other".
- **Make `raster.rs` exact enough for goldens:** bilinear texture sampling, and a
  top-left fill rule to remove the seam seen at 2×. It stays a reviewer, not the app's
  renderer.

---

## 10. The user's ten logos: rendering, tinting, splash and icons

The SVGs are in the session scratchpad, `icons/01.svg … 10.svg`.

- **The detailed ones (01–04, 09)** use `feDropShadow` and a glow built from
  `feGaussianBlur` and `feMerge`, linear and radial gradients, gradient strokes, and a
  `<pattern>` (03).
- **The minimal ones (05–08, 10)** are flat fills and round-capped strokes.

### 10.1 Rendering: `resvg`, cached per size

Timings: `usvg::Tree::from_str`, then `resvg::render` into a `tiny_skia::Pixmap`; one
P-core, medians; resvg 0.45.1.

| Logo | SVG bytes | Parse | 32 px | 256 px | 1024 px |
|---|---|---|---|---|---|
| 01 Neuma sobre tetragrama | 4,145 | 0.19 ms | 0.67 ms | 15.8 ms | 298 ms |
| **02 C de onda con playhead-cruz (default)** | 5,064 | 0.38 ms | **1.52 ms** | **35.7 ms** | **526 ms** |
| 03 Capitular iluminada | 3,784 | 0.29 ms | 1.00 ms | 28.9 ms | 395 ms |
| 04 Rosetón como perilla | 10,143 | 0.70 ms | 1.38 ms | 47.3 ms | 506 ms |
| 05 Neuma minimalista | 791 | 0.04 ms | 0.02 ms | 0.12 ms | 1.3 ms |
| 06 Onda minimalista | 1,028 | 0.07 ms | 0.08 ms | 0.28 ms | 1.8 ms |
| 07 Capitular minimalista | 602 | 0.03 ms | 0.03 ms | 0.17 ms | 1.6 ms |
| 08 Rosetón minimalista | 2,478 | 0.10 ms | 0.07 ms | 0.49 ms | 3.7 ms |
| 09 Onda verdín | 5,060 | 0.35 ms | 1.41 ms | 30.9 ms | 515 ms |
| 10 Onda lapislázuli y plata | 1,028 | 0.08 ms | 0.10 ms | 0.36 ms | 2.6 ms |

- **The filters cost 100–300× more than the geometry.** The glow's `stdDeviation="10"`
  and the drop shadow's 22 at 1024 px dominate.
- **So the logo is never rendered on the UI thread at a large size.**
  - Render per (logo, tint, physical px) on a worker thread.
  - Upload it as an egui texture when ready.
  - Keep the previous texture until then.
  - Cache the PNG on disk, keyed by a hash of (SVG, tint, px), so the next launch
    loads it instantly.
- It still counts as "drawn natively in vectors": `resvg` is a vector renderer run at
  the exact device size. That is how the font atlas works too, and it is what FL's
  crispness at 4K means. A scale change re-renders at the new pixel size.
- **`lyon` is ruled out:**
  - it cannot do filters, gradient stops or patterns, so the detailed logos are out
    entirely;
  - for the flat ones it works (188–602 triangles, 43–157 µs at 256 px), but the mesh
    is **aliased**. At 32 px the stair-steps are plain next to `resvg`.
  - it would add a second path for no gain.
- **Small sizes need a small master.** At 16–24 px, #2's filtered wave becomes a blur,
  and even the minimal wave's 11 spokes run together (`small_sizes.png`). Standard icon
  practice applies: draw a simplified 16/24/32-px master on the pixel grid, for example
  #06's geometry with fewer, thicker spokes. Use it for the title bar, the taskbar and
  the small `.ico` entries. Keep #2 as designed from 48 px up.
- **Licences, all compatible with GPL-3.0-or-later:**

  | Crate | Licence |
  |---|---|
  | `resvg`, `usvg` | Apache-2.0 OR MIT (0.45 and later; MPL-2.0 before) |
  | `tiny-skia` | BSD-3-Clause |
  | `lyon_tessellation` | MIT OR Apache-2.0 |
  | `vello_cpu`, `kurbo` | Apache-2.0 OR MIT |
  | `winresource` | MIT |

  `resvg` with `default-features = false` (no text, no raster images) adds about 19
  small crates. The latest release is 0.48.1 (August 2026); use that.

### 10.2 Theme tinting: map roles to tokens, keep the designed lightness

Both methods were implemented on #2 and rendered at 256 px against five targets: Paper,
Ink & Gold, Parchment, and a blue and a green accent (`tints.png`). The whole remap,
parse and render costs 33–46 ms at 256 px, almost all of it the render.

**Method A: map roles to tokens.**

- How it works:
  - Each designed colour is assigned a role:
    - **tile** `#4A2213 #22110A #0D0705`;
    - **patina** `#3E8E7E`;
    - **wave** `#FFD98A #E07A2E #9C300C`;
    - **playhead** `#FFF6D6 #E9B64C`;
    - **neutral** (the white gloss and black shadow).
  - Each role takes **hue and saturation from a token**:
    - tile ← the accent's hue at low saturation;
    - wave ← the accent;
    - playhead ← the bright accent;
    - patina ← the selection or "ok" token.
  - Each role keeps **the original stop's lightness**.
- Result:
  - The gradients keep their depth.
  - The playhead stays the brightest element, and the patina follows a token.
  - In blue and green themes it reads as the same logo in the theme's colours.

**Method B: one hue rotation**, from #E07A2E to the accent, keeping lightness.

- It needs one parameter and no table.
- But it moves every colour. The teal patina, chosen to *contrast*, becomes red in the
  blue theme and purple in the green one.
- The three shipped themes all share a gold accent, so B and A barely differ there.
  Only an off-gold accent shows the flaw.

**Recommendation: method A.**

- Declare the roles **in the SVG**, as `data-role="wave"` on gradient stops and fills.
  Keep a table in code as a fallback.
- Apply them by rewriting colour values in the SVG *text* before `usvg` parses it.
  usvg 0.45's `Tree` exposes no public way to change paints. The harness does exactly
  this, in about 40 lines.
- The setting is `logo: 1..10` and `logo_tint: Original | Theme`. **The default is `2`
  + `Original`**, #2 exactly as designed.
- Re-render on the worker when the theme generation changes.

### 10.3 The splash while Cantus loads, like FL's

**What the start-up does today**, measured in the harness with the calls
`CantusApp::new` makes:

| Step | Time |
|---|---|
| `MidiInputs::open_all` (with **zero** MIDI ports) | **701 ms** |
| `EngineHandle::start` (default device) | **283 ms** |
| Listing the output devices (the settings dialog) | 106 ms |
| `CantusApp::from_parts` (plug-in host with an empty database) | 5 ms |
| Fonts, style and the first frame | 1.6 ms |

Project loading and re-instantiating hosted plug-ins grow with the project, and come on
top. eframe also needs 100–300 ms to create the window and GL context (egui#7761's
glow figure).

**Recommended design: one window that starts as the splash.**

1. **`NativeOptions`:**
   - `viewport = ViewportBuilder::default()`:
     - `.with_inner_size([520., 320.])`;
     - `.with_decorations(false)`;
     - `.with_resizable(false)`;
     - `.with_icon(icon)`;
     - `.with_title("Cantus")`;
   - `centered: true`.

   Keep it **opaque**. Paint the splash as a full-bleed card. A transparent surface
   must be asked for when the window is created (its GL configuration), and it would
   then stay on the main window. `ViewportCommand::Transparent` exists, but switching
   transparency after creation depends on the platform. Transparent windows also cost
   compositing.
2. **The app becomes `enum Shell { Booting(Boot), Running(CantusApp) }`**, and
   `eframe::App::ui` dispatches on it.
   - `Boot::ui` paints the baked splash texture, the version and a progress line
     ("Opening audio…", "Scanning MIDI…", "Loading song…").
   - It advances **one stage per frame**.
3. **The stages:**
   - **Stages that can move off the UI thread** go to a `std::thread` with a channel:
     - the MIDI scan (it only needs the `MidiSender`, which is a queue producer);
     - `theme.json`, `settings.json`, `keymap.toml` and the plug-in database;
     - parsing the project file and decoding its samples.
   - **Stages that must stay on the main thread** run one per frame, with the splash
     repainted between them:
     - the audio stream: `EngineHandle` holds a `cpal::Stream`; WASAPI's is `Send`, but
       creating it on the main thread keeps this portable;
     - the plug-in windows' owner handle (`frame.window_handle()`);
     - the fonts.

   Windows only flags a window "not responding" after 5 s. No stage comes close.
4. **When everything is in, turn the window into the main window**, all in the same
   frame:
   - build `CantusApp`;
   - send `ViewportCommand::Decorations(true)` and `Resizable(true)`;
   - send `InnerSize` and `OuterPosition` from the saved layout, or `Maximized(true)`;
   - switch the shell to `Running`.

**The alternative**, a hidden root window (`with_visible(false)`) plus a splash child
viewport (`show_viewport_immediate`) that is dropped when the root becomes visible,
also works with glow's multi-viewport support. It costs a second GL context and risks a
flash between the two windows. The single morphing window is simpler.

**The splash's picture must be instant.** #2 at 512 px costs 131 ms to parse and render
(measured). So:

- **bake the default splash image at build time**, 1× and 2× PNG, `include_bytes!`;
- decode it with the `image` crate eframe already links;
- if the user chose another logo or tint, the disk cache from the last run supplies
  it. If it is missing, show the default and render the chosen one in the background.

### 10.4 Window, taskbar and `.exe` icons

- **Window and taskbar:**
  `ViewportBuilder::with_icon(Arc::new(eframe::icon_data::from_png_bytes(include_bytes!("…/app-256.png"))?))`.
  winit sets the Win32 big and small icons from it. Better: pass the small master for
  the 16/32-px look.
- **`.exe` icon (Explorer, pinned shortcuts, Alt-Tab):**
  - `winresource` (MIT, 0.1.31) in a new `crates/cantus-app/build.rs`, under
    `[target.'cfg(windows)'.build-dependencies]`;
  - `winresource::WindowsResource::new().set_icon("assets/app.ico").compile()`;
  - the `.ico` holds 16, 20, 24, 32, 40, 48, 64 and 256 px. The small ones come from
    the small master, the large ones from #2.
- **Generate, then commit:** an `xtask icons` subcommand (resvg + the `ico` crate) writes
  `assets/app.ico`, `app-{16…256}.png` and the splash PNGs from the SVGs. The outputs
  are committed, so a normal build needs neither resvg nor the SVGs. macOS `.icns` and
  the Linux `.desktop` PNGs come from the same task later.

---

## 11. Decisions, ranked, and what not to do

1. **Stay on egui 0.36 + `glow`, meshes and textures only.**
   - Why: the GPU cost is 0.3–2.7 ms at 1080p; the plug-in windows are OpenGL; `wgpu`
     costs start-up, memory and binary size (egui#7761).
   - *Not:* rewriting the renderer, or chasing `wgpu` for a GPU that is idle.
2. **Hybrid vector pipeline.**
   - Live epaint geometry for what moves.
   - Static richness rasterised per physical pixel size with `vello_cpu` (no new crate)
     or `resvg` (SVG with filters), cached per theme generation and scale.
   - It is the only way to get FL's depth (gradient caps, true soft shadows, glows,
     bevels) at 4K without tessellating it every frame. It is also *cheaper*: the
     knobs measured 4.0 ms against 15.6 ms.
   - *Not:* bitmaps drawn by hand at one size (they blur at 1.5×), or `lyon` meshes
     (aliased, no filters).
3. **Fix the frame before anything else.**
   - Cull off-screen strips and rows.
   - Build the sends list lazily (2.8 ms saved).
   - No per-frame clones (18 MB a frame on the stress project).
   - Batched meters (30× cheaper).
   - Cached galleys (3.6× cheaper).
   - One repaint ticker: 0 idle frames, ≤ 60 Hz meters.
   - Expected result: the 64-strip mixer goes from 9.7 ms to about 3 ms.
4. **Budget ≤ 4 ms p50 / 6 ms p95 CPU at 1080p** for the 64-strip mixer with meters,
   ≤ 1,500 allocations, 0 idle repaints, GPU ≤ 2 ms on Iris Xe-class. Held by
   `tests/frame_budget.rs`: deterministic counters always, timing in release CI.
5. **The component kit:** seeds (H/S/L/contrast, in OKLCH), then tokens, then pure
   looks, then widgets, with the cache and the batch. Hairlines one physical pixel.
   Proven by a golden gallery at 1×, 1.5× and 2×.

The logos follow from 2 and 5:

- `resvg`, cached per size, rendered on a worker;
- role-mapped tinting with **Original #2 as the default**;
- a single window that boots as the splash, with baked splash images;
- `xtask icons` generating the `.ico` and PNGs, and `winresource` for the `.exe`.

*Not to do:*

- **egui `Button`s for dense, repeated controls.** They cost 14 µs each; paint the
  kit's own switches instead.
- **Per-frame `format!` and `t()`.**
- **Unconditional `request_repaint()`.**
- **epaint blur above about 12 px.** It creases at the corners.
- **Point-sized texture caches.**
- **Relying on egui's built-in Ctrl +/− zoom.**

---

## Appendix A. Reproducing the measurements

The harness lives outside the repository, in the session scratchpad:
`perf_harness/` and `gpu_harness/`. It uses path dependencies on `crates/cantus-ui`,
`cantus-engine` and `cantus-core`, a copy of the repository's `Cargo.lock`, the same
release profile, and its own target directory. `LOCALAPPDATA` points at a scratch
folder, so no user data is read or written. The parts worth turning into
`tests/frame_budget.rs` and `benches/ui_frame.rs`:

```rust
// Full window, headless, meters driven by a real processor.
let (engine, mut processor) = cantus_engine::EngineHandle::manual(48_000.0);
let mut app = CantusApp::from_parts(Some(project), engine, false);
app.central = CentralView::Mixer;
app.sync_engine();
app.toggle_play();
let mut frame = eframe::Frame::_new_kittest(); // public (doc-hidden) in eframe 0.36
let mut audio = vec![0f32; 800 * 2];           // one 60-Hz frame at 48 kHz
for _ in 0..(30 + 300) {
    processor.process(&mut audio, 2);
    let t0 = Instant::now();
    let mut out = ctx.run_ui(input.clone(), |root| eframe::App::ui(&mut app, root, &mut frame));
    let t1 = Instant::now();
    let prims = ctx.tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
    let t2 = Instant::now();
    out.textures_delta.clear();
    // record t1-t0 (build), t2-t1 (tess), triangles, prims.len(),
    // out.platform_output.num_completed_passes, allocation counter deltas
}
```

- **Grown projects:** push `make_strip("")` up to 64, then 64 tracks with a 16-note
  clip each.
- **Stress project:** `cantus_ui::stress_project()`.
- **Noise control:** pin to one P-core
  (`Process.ProcessorAffinity = 0x4`, `PriorityClass = High`), one process per case,
  best of 3 medians.
- **Output files:**
  - `perf_harness/frames_summary.txt`, the tables in §2;
  - `micro_*.txt`, §5;
  - `logos.txt`, §10;
  - `gpu_harness/gpu*.txt`, §2.3;
  - images in `perf_harness/sheets/` and `perf_harness/logos_out/`.

## Appendix B. Sources

- Image-Line, *Announcing FL Studio 12* (22 April 2015): "FL Studio is now 100%
  vectorial…" —
  https://www.image-line.com/fl-studio-news/announcing-fl-studio-12
- FL Studio manual, *System Settings – General*: GUI scaling, animation refresh rate,
  "Thick lines", smoothing —
  https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/envsettings_general.htm
- FL Studio manual, *Theme Settings*: Hue, Saturation, Brightness, Contrast, Text,
  Selection, Highlight, Mute and Option LEDs, meters —
  https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/envsettings_themes.htm
- egui changelog, 0.34–0.36 (skrifa + vello_cpu, hinting #7694, font variations #7859,
  harfrust #8031, subpixel binning #8072, hinting target #8262,
  `extra_text_line_spacing` #8040) —
  https://github.com/emilk/egui/blob/main/CHANGELOG.md
- egui#7761, *WGPU backend transition: performance, issues and suggestions* —
  https://github.com/emilk/egui/issues/7761
- crates.io: `resvg` (licence history), `lyon_tessellation`, `winresource`.
- Source read locally (egui 0.36.1, epaint 0.36.1, eframe 0.36.1, vello_cpu 0.1.0,
  cpal 0.18.2), cited by file and line in the text.
- The companion reports in this folder (`fl-studio.md`, `cantus-audit.md`) cover what
  FL looks like; this one covers how to draw it.
