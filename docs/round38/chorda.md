# Round 38 — Chorda: the scene's speed, and voicing pass three (begun)

Branch `r38/chorda`, worktree `F:\projects\daw-r38\chorda`. Written 2026-11-09 by the main
session when the owner moved on to round 39 (UX and graphics). Two pieces of work are here:
one is finished and waits only for the owner's eyes; the other is half done and ships flat,
so it changes no sample.

## 1. The scene no longer pixelates while a control moves (done; the owner confirmed it on 2026-11-09: "ya no se pixelean")

**The report.** The owner: moving *Width*, *Lid* or *Room* pixelates Chorda's picture, and
*Room* flickered. (*Hardness*, *Mics*, *Sympathy* and *Boom* "stuck" because of automation
clips that override the knob; that was not a bug. Round 39 adds a sign on an automated knob.)

**The cause.** A full bake took ~600 ms at 1.5 px/pt. While a control glided, a *preview*
was baked at half resolution with one sample a pixel, then stretched; the full bake then
replaced it with a hard cut. The first fix (antialiased previews at an adaptive resolution
plus a 0.15 s dissolve) was "a bit better, not fixed": the preview was still a smaller
picture stretched.

**The fix: the baker is ~4× faster, to the bit.**

| At 1.5 px/pt, 710×300 pt, 14 threads | before | after |
|---|---:|---:|
| Full bake (ss 2, every effect) | 597 ms | ~140 ms |
| Preview at full resolution (ss 2, plain) | ~400 ms | ~95 ms |
| Key light's shadow map (1 536²) | 130 ms | 12 ms |
| Strings and joints (lines) | 18 ms | 1 ms |

- **Span rasterization** (`chorda_scene_raster.rs`, `span`). Each edge function is linear
  along a row, so a row's inside is an interval. It is widened by two samples and every
  sample still takes the exact test, so the picture is unchanged. The shadow map had tested
  ~60 samples for each one it kept. The lines use the same idea: `|q · nrm|` is linear
  along the row.
- **Rows in bands of 8, taken by whichever thread is free** (`rows1`, `rows3`). A fixed
  split left most threads idle while one finished the piano's rows.
- **The serial passes run on every thread.** These are the blur (rows, then column bands
  walked row by row, each column's running sum in its own order), the mirror composite,
  depth of field, bloom, the resolve and the lines (each band draws every line in order
  over its own rows).
- **Settings:**
  - bake threads: all but two, at most 14;
  - `PREVIEW_MS` 120: the whole picture fits on this machine;
  - the first preview is the whole picture (`PREVIEW_START` 1.0).
- **Unchanged from the first fix:** the antialiased preview and the 0.15 s dissolve
  (two textures, recoloured in place, no allocation).

**Bit-identical.** 24 bakes (three lid/room/width settings × both themes × four qualities and
thread counts) hash the same before and after (`chorda_knobs_measure::hash_the_bakes`).

**Tests:**
- New: `a_bake_is_the_same_on_any_number_of_threads` (1, 2, 5, 14 threads; preview and
  full; three rooms, mirror included; both themes).
- Kept from the first fix: `a_previews_resolution_is_what_the_machine_bakes_in_its_time`
  and `a_landed_picture_dissolves_over_the_one_before_and_then_rests`.
- `chorda_scene_budget` 11 pass, `chorda_scene` 3 pass.
- Clippy `-D warnings` and fmt are clean on cantus-ui.

**Picture:** `F:\projects\daw-r38\shots\preview-compare3.png` (beta.12, the first fix, now, final).

**Test build for the owner:** `F:\projects\daw-r38\builds\chorda-scene2\cantus.exe`.

**ADR:** amend ADR 0104 (the baker) with the span walk, the bands and the parallel passes;
the bits stay as they were.

**Also here:**
- `examples/shoot_skins.rs` bakes Chorda's scene blocking. The site's picture had been an
  empty card; it is already on master as 709ee14c.
- The regenerated unit pictures are the same files as master's 709ee14c.
- Diagnostics, all `#[ignore]`:
  - `tests/chorda_knobs_measure.rs` measures UI frames and audio blocks while each control
    turns, the bake tiers (`T=<threads>` env), the previews and the hashes. It writes to
    `F:/projects/daw-r38/shots`.
  - `tests/chorda_knob_drag.rs` drags each Play knob with the pointer.
  - Either keep them as they are, or drop the paths before merging.

## 2. Voicing pass three (begun — ships flat, changes no sample)

**Why.** Before the owner's third ABX, a spectral comparison of model 2 against the
reference piano (kit 3) showed four defects:
- the bass's low-mids too thick (≈ 500 Hz);
- the middle register nasal near middle C;
- the treble dull;
- *fortissimo* attacks flattened by the body (crest too low).

The stereo image of the treble is also too wide (correlation).

**Done:**
- `crates/cantus-dsp/src/chorda_registers.rs`: **voicing by register**. The keys' bridge
  forces go into three buses (bass/middle/treble, centre keys 34/65/96, a linear crossfade),
  and each bus passes four RBJ peaking bands (125, 500, 1 600, 4 000 Hz, Q 1). The cost is
  fixed; a 0 dB band is skipped; the sympathetic strings get the un-EQ'd mono.
- `Voicing::reg` (default all 0 dB) and the `reg.<bass|mid|treble>.<0-3>` keys in the lab.
- Tests (`chorda_tests_registers.rs`, 5 pass):
  - every key's shares sum to 1 and glide;
  - each band gives ±6 dB at its centre and < 0.5 dB three octaves away;
  - a flat voicing is bit-identical to the engine without registers;
  - a voiced piano is the same at block sizes 1/7/64/100/512 and on 1/3/4 threads;
  - an exact rest.
- The search (`F:\projects\daw-r38\voicing\`: `metric.py`, `search3.py`, logs):
  - the phrase metric (A-weighted third-octave error with a per-band floor weight, crest,
    envelope spread, stereo correlation, plus 0.1 × the round-37 single-note score as a
    brake) went **9.05 → 8.39** (voicing parameters alone, `best3-a.txt`) **→ 7.66** (with
    registers, `best3.txt`);
  - the first, unbraked search overfit (`search3-overfit.log`); the brake and the bounds
    fixed it.

**Left (round 40, unless the owner says otherwise):**
- A velocity-dependent direct path for the *fortissimo* attack.
- The treble's stereo coherence: the correlation got worse, 0.43 → 0.56.
- The nasal register near middle C.
- A second search.
- Ship `best3.txt`'s voicing as the default:
  - a `DSP_VERSION` bump and goldens;
  - the 24 presets re-trimmed to ±2 LU;
  - the full Chorda suite, rt_memory, clippy and fmt;
  - the ADR 0098 third amendment.
- ABX kit 4 for the owner (`chorda_abx`).

**Today the shipped voicing is beta.12's**: `reg` is all zeros, so no sample changes and no
golden moves.
