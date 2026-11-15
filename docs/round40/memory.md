# Round 40 — `memory`: the private bytes outside the heap (ADR 0120)

Branch `r40/memory`, worktree `F:\projects\daw-r40\memory`, base `f00ed550` (the round's
contract on master `dd4ad68b`).

**Every number here is a map**, taken on the scorecard's laptop while other builders were
compiling. None is the scorecard's: the integrator measures that on a quiet machine. Nothing
here says Cantus beats REAPER or FL Studio.

## 1. The map

### How it was taken

- **Build.** A release build of master from this worktree (`cargo build --release -p cantus`),
  and beta.14's release as a control (the same within 4 MB).
- **Data and songs.** A fresh data folder each start (the audio set-up done, round 38's plug-in
  cache) and round 38's `empty.cantus` and `beat.cantus`. Mapped 25 s after spawn.
- **`tools/scorecard/memmap.ps1 -Detail`** (new: `MemDetail.cs`). Every byte of the commit
  charge under an owner, read from outside the process:
  - heaps: their maker, their segments and big blocks, their allocated and committed bytes;
  - each thread's stack, named;
  - the drivers' write-combined regions;
  - each module's charged image pages;
  - page tables.

  Its total lands within ~1 MB of `PrivateUsage`.
- **The heap census.** A measurement build (`cargo build --release -p cantus --features
  census`; `cantus-rt/src/census.rs`) gives the live heap by call stack. `memmap.ps1 -Census`
  names the addresses with dbghelp (`CensusNames.cs`).
- **A bare-window probe** (outside the repository, `F:\projects\daw-r40\tmp\memory\gfxprobe`):
  eframe 0.36 with one label, with each renderer, to see each driver's floor.

### What an empty song holds (master, ~140 MB; *Beat* within 2 MB of it)

| Holder | MB |
|---|---:|
| **Process heap**, in all | ~100–105 |
| · Cantus's own live allocations (census) | 25.4 |
| · · the engine: slots and parts 3.6, the backend 3.5, compensation start lines 2.25 | 9.4 |
| · · Exemplar's built-in sounds, warmed at every start | 5.7 |
| · · egui's text (the font atlas's CPU copy 2.0) | 2.9 |
| · · both string tables | 2.2 |
| · · views and kit | 2.0 |
| · · the rest | ~3 |
| · the OpenGL driver's and Windows's heap use (a bare window: 71–79 MB, the same 16 / 9.3 / 4 / 2 MB big blocks) | ~75 |
| The GL driver's write-combined, GPU-visible memory | 27–29 (bare window 24) |
| The GL driver's own heap | 4 |
| Thread stacks (44 threads) and TEBs | 2.4 |
| Writable image pages (`atio6axx.dll` 2.0) | 3.4 |
| Page tables, copy-on-write views | 1.4 |

The window draws with OpenGL on the AMD integrated GPU (`atio6axx.dll`). **A bare eframe
window with one label holds 105–109 MB on this machine.** Only about 33 MB of Cantus's ~140
is above it: its own 25 MB, and its textures' copies in the driver. Textures at rest are
three:
- the kit's layer atlas: 1024², 4 MB;
- egui's font atlas: 16 384 × 32 texels (2 MB), as wide as the driver's largest texture;
- a 30² logo.

On this driver one 1024² texture costs +11 MB of private bytes.

### Measured and rejected

| Bare window, 10–12 s | MB |
|---|---:|
| glow (Cantus) | 105–109 |
| wgpu → DX12 (loads both GPUs' drivers) | 397 |
| wgpu → Vulkan | 482 |
| wgpu → GL | 133 |
| glow, segment heap (manifest) | 107–109 |
| glow, no accesskit | 105–107 |
| glow, a 24-bit depth buffer | 116–119 (Cantus asks for none) |
| glow, no text | 93–94 |
| glow, font atlas ≤ 2 048 wide | 98–99 |

On Cantus itself the segment heap measured 139 MB against 136. The heap's committed bytes
are within 1 MB of its allocated bytes, so decommit thresholds have nothing to give. The
window's size does not matter. Thread stacks are 2.4 MB in all.

## 2. The levers (ADR 0120 §§3–6)

| Lever | Mapped Δ (empty song) | Where |
|---|---:|---|
| The kit's layer atlas starts at 512² and doubles, between passes, when a layer does not fit (at most once; never shrinks) | −8.2 MB | `kit/cache.rs` (`// r40:memory`, by the integrator's leave) |
| Exemplar's built-in sounds computed when an instrument may be chosen (the picker opens, a device is dragged from the browser), not at every start | −7.8 MB | `app.rs`, `picker.rs`, `views/picker_panel.rs`, `cantus-dsp/src/exemplar_builtin.rs` |
| A language's strings parsed when that language is first read; values borrowed from the bundled text | −3.3 MB | `i18n.rs` (the parsing only) |
| egui told of a texture side no wider than 4 096 (its font atlas follows) | −2.2 MB | `boot.rs` (the shell's input hook) |
| **The four together** | empty 138.2 → **122.2**, *Beat* 139.9 → **123.7** | |

Each was measured with an experimental build whose environment toggled it (three starts
each; the toggles were never committed).

**The final build** of this branch, mapped against master's, alternating starts, three of
each (the machine busy, so these are maps):

| Song | master | this branch | Δ |
|---|---:|---:|---:|
| empty | 136.5 MB (135.1–137.0) | **121.2 MB** (120.7–125.5) | −15.3 |
| *Beat* | 140.4 MB (140.2–140.6) | **123.6 MB** (123.1–125.2) | −16.8 |

In the empty song's map:
- the process heap: 100.1 → 91.7 MB;
- the driver's write-combined and other private memory: 28.2 → 21.3 MB;
- stacks, image pages and page tables: unchanged.

**Not done: the compensation pool's start-up lines** (−2.25 MB). The processor makes them
in `Processor::with_cells`, and hand-driven processors rely on them through
`hold_parts_for`. Both are `processor*.rs`, the `pads` builder's this round. The change is
two lines there, written down in ADR 0120 §7.

## 3. How it is tested

- **`tests/atlas_growth.rs`** (new), on the CPU renderer:
  - a page of 40 layers fills 512², grows exactly once and draws the same pixels as a 1024²
    start;
  - a page of 12 never grows;
  - steady state (60 and 120 frames) never grows, resets or redraws a layer;
  - the assembled application on the demo, mixer at 2 px/pt, draws the same window from
    either start. It fits 512²: 29 layers, no growth.
- **`tests/memory_e2e.rs`**, through `CantusApp::assemble`:
  - an English start on an empty song computes no Exemplar sound and parses no Spanish;
  - the built-ins are the step the picker causes;
  - the bounds were lowered to a tenth above the new measure: empty song and frames
    7.8 → 5.2 MB (≤ 5.8), a start on the demo 59.7 → 56.9 MB (≤ 62.6).
- **A new `memory_e2e` test runs the window as the boot does** (`boot::Shell` around the
  assembled app):
  - with the input reporting 16 384, egui's font atlas stays no wider than
    `boot::TEXTURE_SIDE`;
  - `CantusApp::set_locale(Es)`, the Settings path, shows *Archivo* in the very next frame
    and no *File*.
- **`i18n` unit tests:** every locale has every English key; a new one checks that an escaped
  `\n` still becomes a line break while other values are borrowed.
- **`cargo test -p cantus-ui`, whole:** 2 654 passed. The 2 that failed were in this branch's
  `memory_e2e`: it wrote the process language, which `locale_isolation` forbids, and its
  Spanish check depended on test order. Both were fixed and rerun green, with
  `locale_isolation` and `atlas_growth`.
- **Engine:** `cargo test -p cantus-engine --test rt_memory --test low_latency --test pdc
  --test recording_latency --test slot_parts` green (the engine is untouched).
- **cantus-dsp:** the Exemplar tests are green.
- **Release timing tests not settled here:** `frame_budget` and `skins_budget` with
  `--ignored`, run twice while two other builders ran `cargo test -p cantus-ui` and a
  release build.
  - Green: every mixer and window timing test (16 and 64 strips, wired, hovered bus,
    scroll).
  - Over their p95 limits, with p50s inside the 4 ms budget:
    - `three_editors_frame_time`: p50 2.7–3.8 ms, p95 3.7–6.2;
    - the dancer's own redraw (2.45 ms mean against 1.5; its rasteriser, `cantus-dance`, is
      untouched here);
    - the dancer's whole window: p95 5.1–5.3 ms.
  - In steady state `skins_budget` drew no layer and reset nothing (`layers rasterised 0,
    atlas resets 0` in every case), so the atlas's growth adds no work per frame. The other
    levers are outside the frame path; a string lookup is no dearer.
  - **The integrator should rerun these on a quiet machine** before calling the frame
    budget unchanged.
- **Clippy** `--all-targets` with `RUSTFLAGS=-D warnings` is clean on cantus-ui, cantus-dsp,
  cantus-rt and cantus, and again with `--features census`.
- **Linux:** not checked here (builders run no WSL). The census is `cfg(windows)`, its
  feature off by default; everything else is platform-neutral.

## 4. Verified only here

All of it ran on one Windows laptop: AMD Radeon 680M iGPU drawing, RTX 4060 idle, AMD
OpenGL driver 31.0.14001. Other drivers have other floors. NVIDIA's OpenGL driver, Intel's,
Linux's Mesa and macOS were not mapped. The levers are driver-independent (smaller
textures, fewer allocations); their size is not.

## 5. A CPU-presented window (for the BACKLOG; the integrator's probe 4)

A winit window presented from the CPU: a 32-bit DIB section the window's size, drawn with
tiny-skia and blitted with GDI (softbuffer's Windows path; built with `windows-sys` because
softbuffer is not in the offline registry). The probe is
`F:\projects\daw-r40\tmp\memory\cpuprobe`. The scene is synthetic, close to a busy DAW
view:
- a panel fill;
- 2 400 rounded rectangles;
- 400 anti-aliased circles;
- 600 hairlines;
- no text.

It draws single-threaded. Two runs, 100 timed frames each after 20 unmeasured:

| | Private bytes at idle | Draw p50 · p95 | Blit p50 · p95 |
|---|---:|---:|---:|
| 1920 × 1080, full redraw | **11 MB** (7.9 of it the frame buffer) | 37 ms · 47 ms | 0.7 ms · 0.9 ms |
| a 320 × 200 damaged rect | (same) | 3.7 ms · 5.5 ms | 0.15 ms · 0.2 ms |

Against a GL window's 105–109 MB on this machine, a CPU-presented one costs about a tenth.
But a full redraw at 1080p is 2–3 frames at 60 Hz single-threaded. A CPU renderer would
need:
- damage tracking (most frames repaint a meter or a playhead);
- tiling across cores;
- text from a glyph cache.

What the GPU does for free would become Cantus's own code and CPU time. Its own frame
budget (4 ms) would need about ten cores' worth of full redraws, or damage of a few percent
of the screen. Recorded in BACKLOG §4b.

## 6. What is left

- The OpenGL driver's floor: ~105 MB for a bare window on this machine. Only another way of
  presenting can move it (§5).
- The compensation pool's start-up lines: −2.25 MB, needs `processor*.rs` (ADR 0120 §7).
- Exemplar's built-ins once warmed (5.7 MB) and both string tables once switched stay for the
  session.
- The release timing tests' verdict on a quiet machine (§3).

## 7. For the owner

Nothing to look at or listen to: the window draws the same pixels (tested). Two items in
`docs/beta-checklist.md` (*Memoria, ronda 40*): choosing Exemplar right after opening the
picker should show no pause, and switching language should change the whole window at once.
The memory row itself is the integrator's to measure on a quiet machine.
