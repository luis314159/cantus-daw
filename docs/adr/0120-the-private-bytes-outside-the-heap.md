# ADR 0120 — The private bytes outside the heap: mapped, and what Cantus can give back

**Status:** accepted (round 40, the `memory` builder) · **Date:** 2026-11-12 · **Schema:**
none · **Amends:** ADR 0107 (what a start holds), ADR 0024 §1 (the kit's layer atlas), ADR
0007 (how the string tables are read) · **Relates to:** ADR 0102 (the scorecard's method),
`docs/round40/memory.md`

## Context

Memory is the one scorecard row Cantus still loses: 140 MB of private bytes on an empty
song against REAPER's 62, 143 on the *Beat* starter against FL Studio's 133 (quiet
machine, 2026-11-11). ADR 0107 brought the engine's own heap down to what the song needs;
`memory_e2e` counts about 25 MB live at a start. Where the other ~115 MB lives was not
known. This round mapped it before choosing what to change.

## Decision

### 1. The map is a tool, and a measurement build

- **`tools/scorecard/memmap.ps1 -Detail`** (`MemDetail.cs`) puts every byte of the commit
  charge a process's *private bytes* count under an owner, read from outside:
  - committed private regions: each thread's stack (named by its description and start
    module, found through its TEB), the TEBs, each heap's segments (the NT heap's segment
    signature names the heap; `RtlQueryProcessDebugInformation` gives every heap's
    allocated and committed bytes) and its big blocks (walked from the heap's header), and
    the rest by the module whose data points into it;
  - every module's writable (copy-on-write) image pages, charged whether written or not;
  - copy-on-write views and an estimate of the page tables.

  Its total lands within about 1 MB of `GetProcessMemoryInfo`'s `PrivateUsage`.
- **A heap census** for measurement builds (`cantus-rt/src/census.rs`; the `cantus`
  feature `census`, off by default, never in a release): a global allocator over the
  system's that records the call stack (`RtlCaptureStackBackTrace`, no lock, no allocation)
  of every block of 64 KB or more and of one block per 128 KB a thread allocates, and
  writes what is live by stack to `CANTUS_HEAP_CENSUS` every ten seconds;
  `memmap.ps1 -Census` names the addresses from outside with dbghelp. The audio thread
  never allocates (ADR 0005), so the census adds nothing there.
- Maps taken while other work runs on the machine are maps, never the scorecard's numbers
  (ADR 0102).

### 2. What the map found (empty song, release, the scorecard's laptop)

The window draws with OpenGL (glow) on the AMD integrated GPU. **A bare eframe window with
one label holds 105–109 MB there**; Cantus holds about 138–142. The heap census puts
Cantus's own live allocations at 25.4 MB (the engine 9.4, Exemplar's built-in sounds 5.7,
egui's text 2.9, both string tables 2.2, views and kit 2.0, the rest ~3); the process heap
around them, ~75 MB, is the OpenGL driver's and Windows's; the driver also holds 27–29 MB
of write-combined, GPU-visible memory and a 4 MB heap of its own. Thread stacks (44
threads) are 2.4 MB, writable image pages 3.4, page tables 1.4. On this driver **a texture
costs about 2.7 times its size** in private bytes (one 1024² RGBA texture: +11 MB).

Measured and rejected: wgpu instead of glow (DX12 397 MB, Vulkan 482 — both GPUs' drivers
load — GL 133); the segment heap through the manifest (no change, in a bare window or in
Cantus); accesskit off (no change); the heap's decommit thresholds (its committed bytes are
within 1 MB of its allocated bytes). Cantus asks for no depth buffer (one would cost 11 MB).

### 3. The kit's layer atlas grows with what is drawn

The atlas (ADR 0024 §1) was a 1024² texture from the first frame; an idle window's layers
fill a fraction of it. It now **starts at 512² (`kit::cache::ATLAS_START`) and doubles,
between passes, the first time a layer does not fit**, up to 1024² (`ATLAS`). A layer that
does not fit while the atlas can still grow waits one frame — the cache asks for a
repaint, and the next pass begins on the doubled atlas, a new texture every brush takes
at `begin`; its layers are drawn again as they are asked for. At 1024² a full atlas is
cleared, as before. The atlas never shrinks, so steady state never grows or resets it
again (`Stats::atlas_grown` counts the growths). Tests and benches may start it elsewhere
(`KitCache::start_at`).

### 4. egui is told of a texture side no wider than 4 096

egui makes its font atlas as wide as the largest texture the driver takes (16 384 here)
and the driver keeps copies of it. The shell lowers the side its input reports
(`boot::cap_texture_side`, `boot::TEXTURE_SIDE` = 4 096) before every pass. Only egui's
reading changes: the painter still takes any texture the driver can, and 4 096 keeps a
panel-sized texture on a 4K screen inside egui's debug-build check.

### 5. Exemplar's built-in sounds are computed when an instrument may be chosen

They were computed beside the window at every start (5.7 MB live, ~8 MB of private bytes
with the heap they leave behind), whether the song had an Exemplar or not.
`CantusApp::warm_builtins` computes them on a thread, once, when the instrument picker
opens or a device is dragged from the browser; a song with an Exemplar computes them where
its unit is made (off the audio thread, as before); `exemplar::builtin::built` says
whether they are.

### 6. A language's strings are parsed when that language is first read

Both string tables were parsed at the first lookup; now each is parsed the first time its
language is read (`i18n::parsed`), and a value borrows the bundled text unless it holds an
escape. An English session never parses Spanish; a Spanish one parses English only for a
key Spanish lacks (none does: the i18n test). Switching language parses the other table in
that frame.

**Amended (the effects side round's final merge).** The table was a hash map of 40-byte
entries, and a hash map's buckets double at a power of two (and it reserves ahead as each
area's strings are added): with the side round's ~2 600 new English keys (13 896 in all,
16 384 buckets holding at most 14 336) it was filled into 32 768 buckets, one allocation of
1.31 MB where 0.66 MB had held round 40's 11 271 (`memory_e2e`'s size histogram; an empty
song and its frames 5.2 → 5.9 MB, nothing else of the start moved). Each table is now one sorted
slice exactly as long as its keys (`i18n::Table`: 40 bytes a key, 0.53 MB for English),
looked up by binary search — about fourteen comparisons of short strings, beside the
`String` every `t` already returns — and it grows with the keys, never by doubling. A key
given twice keeps its last value, as the map's `insert` did
(`i18n::tests::the_table_holds_each_key_once_with_its_last_value`).

### 7. Not done here: the compensation pool's start-up lines

The processor makes its 32 start-up compensation lines (2.25 MB) at construction
(`processor.rs`), and a processor driven by hand relies on them. Starting the
application's pool empty — the backend already sends the lines a snapshot can need
(ADR 0107 §3) — needs `Processor::with_cells` to build `Compensation::with_lines(0)` for
the application's engine and `hold_parts_for` to hold `pdc::lines_needed` for hand-driven
ones. Both are in `processor*.rs` (the `pads` builder's this round); left for when that
file is free.

## Consequences

Mapped on the scorecard's machine with an experimental build that toggled each lever
(three starts each, the machine busy — maps, not the scorecard): an empty song 138.2 MB →
122.2 with §§3–6 together (the atlas −8.2, the built-ins −7.8, one language −3.3, the
texture side −2.2 at 4 096), the *Beat* starter 139.9 → 123.7. The final build's map and
the quiet machine's scorecard are in `docs/round40/memory.md` and the integrator's
measurement. What is left is the OpenGL driver's floor (~105 MB for a bare window here);
REAPER draws with GDI and carries none of it. A CPU-presented window was probed for the
BACKLOG (`docs/round40/memory.md`).

**Tests.** `tests/atlas_growth.rs`: a page that fills 512² grows once and draws the same
pixels as a 1024² start (the CPU renderer), a page that fits never grows, steady state never
grows, resets or redraws a layer, and the assembled application on the demo's mixer at
2 px/pt draws the same window from either start. `tests/memory_e2e.rs`: an English start on
an empty song through `assemble` computes no built-in sound and parses no Spanish; the
shell (`boot::Shell` around an assembled app, as the boot runs it) keeps egui's font atlas
no wider than `TEXTURE_SIDE` when the input reports 16 384; switching to Spanish through
`CantusApp::set_locale` shows Spanish in the next frame. `rt_memory.rs`, the engine's
latency and compensation tests and `skins_budget`'s no-reset rule hold. The release timing
tests (`frame_budget`, `skins_budget --ignored`) went over their p95 limits on the busy
machine with their p50s inside the budget, and are to be rerun quiet
(`docs/round40/memory.md` §3).
