# Fractus, the glitch sequencer and grain cloud — the builder's report (wave A, DSP)

Branch `fx/fractus` (from `fx/integration` d3b51a09). **DSP only**: no `EffectKind` yet (a
later contract adds it), no schema, no ADR number. The name is provisional — one constant,
`cantus_dsp::fractus::NAME`.

## What it is, and how it differs from Lapsus

Lapsus is one **time curve** (and a volume curve) per cycle — Gross Beat's job. Fractus is a
**sequence of events**: ten modules, each painted on its own lane of a step sequencer synced
to the song (Effectrix's and Turnado's job, Stutter Edit's build, Beat Repeat's repeats), with
a **granular cloud** beside it that can also be painted in (Portal's and Granulator's job), and
a **performance mode** where held notes play the modules.

| Lane | Module | What it does |
|---|---|---|
| 0 | Repeat | the event's first slice plays live and is *captured*; every repeat after it replays the capture, `pitch_st` higher and `decay_db` lower each time, gated by `duty`; with `ramp` 1–3 the event is cut into parts that each halve the slice (the stutter build) |
| 1 | Reverse | each slice of the event plays the slice before it backwards (slice: the grid's step, or a note value) |
| 2 | Tape stop | the speed falls from 1 to 0 over `time` (or the event), the level with it; or *Start*: up from still |
| 3 | Shuffle | each slice plays a slice up to `range` back (rolled), some backwards (`reverse` chance) |
| 4 | Pitch jump | a four-tap delay-line shifter, ±24 st, window 20–100 ms |
| 5 | Crush | bits (1–24) and a sample-and-hold rate, the hold counted from the event's start |
| 6 | Filter sweep | low/high/band-pass (TPT SVF), cut-off swept from → to over the event in octaves, bent by `curve` |
| 7 | Gate | pulses of a note value, open for `duty`, shut to `depth`; rate 0 = the whole event shut |
| 8 | Throw | the event's signal sent into a synced feedback delay (ping-pong, a low-pass in the loop); the echoes ring after the event |
| 9 | Cloud | the cloud replaces the chain for the event (the cloud's level runs beside the chain at any time) |

## The DSP API the engine unit will call

`crates/cantus-dsp/src/fractus.rs` (and `fractus_{seq,read,fx,cloud,run}.rs`):

```rust
let mut u = Fractus::new(sample_rate);        // allocates (8.6 MB at 48 kHz): off the audio thread
u.set(&FractusSettings { .. });               // every knob; no allocation; call per snapshot
u.set_pattern(&Pattern) / u.set_step(Module, k, Step);   // the lanes (copied in place)
u.set_transport(&Transport { playing, tempo_bpm, samples });  // once per pass, from SongPosition
u.note(offset, Note::On(pitch, vel) | Note::Off(pitch) | Note::AllOff);  // before process
u.process(&mut l, &mut r);                    // in place, any block size
u.at_rest() / u.rest_blocker() / u.skip(frames);
u.latency() == 0; u.reach(tempo); u.memory_bytes(); u.reset();
u.telemetry(&mut [f32; 64]);                  // the family header + `fractus::t::*`
```

- **Settings**: `step` (quarter notes, `STEP_BEATS`: 1/4, 1/8, 1/8T, 1/16, 1/16T, 1/32,
  1/64), `bars` 1–4 (up to 256 steps), `swing` 0–1 (odd steps late by up to half a step),
  `xfade_ms` 0.5–10 (joins, default 2), `trigger` (`Sequence`, `Notes`, `Both`), `quantise`
  (a note waits for the next line), `key` (lane 0's note, default 48 = C3; lanes on the keys
  above), `seed`, `mix`, `out_db`, and one settings struct per module (`repeat`, `reverse`,
  `tape`, `shuffle`, `pitch`, `crush`, `filter`, `gate`, `throw`, `cloud`). Module sizes are
  note values in quarter notes; `note_beats(index)` maps the 14 offered values
  (`NOTE_BEATS`, 1/2 … 1/128) for the core's parameters.
- **Steps**: `Step { kind: Off | Start | Hold, amount, prob }` — a block is a start and the
  holds after it; `amount` is the module's mix for that event, `prob` the chance it plays.
  `Pattern::block(module, from, len, amount, prob)` paints one.
- **Notes**: `TAKES_NOTES` can be true; the engine feeds note-ons/offs with their offsets.
  Velocity is the amount. AllOff on stop/seek/wrap.
- **Telemetry** (for the panel): `t::STEP`, `STEPS`, `PHASE` (the playhead), `ACTIVE` (a bit
  per sounding lane), `NOTES` (lanes a held note plays), `READ` (the read module playing),
  `GRAINS`, `FROZEN`, and up to 16 grains as (position 0…1 of 2 s back, pitch −1…1) pairs
  from `t::GRAIN0` — enough to draw the cloud live.

## How it holds the family's rules

- **Null at the defaults**: nothing painted, cloud level 0 → the output is the input, to the
  bit, through the fast path *and* the full per-sample path (`process_by_ticks`).
- **Sync**: a step's line is `round((k·step + swing) · 60/tempo · fs)` from the song's start —
  the engine's own expression for a beat's sample. Events, rolls, grain onsets: all functions
  of the song sample and the seed. Stopped, the steps rest and only notes play.
- **Joins**: an event's effect starts on its line and is gone by the next one; the end fades
  over the last `xfade` *before* the line, towards whatever plays after it; a jump inside an
  event (each repeat, each slice) fades the old read out, carrying on at its speed, while the
  new one fades in (equal-gain raised cosine) — Lapsus's join, generalised.
- **Reads**: Lapsus's `History` reader (whole sample exact, line → Hermite → 32-tap Kaiser
  sinc by distance, anti-aliased by speed). Repeats read their own **capture** (written until
  the slice is held, plus 64 samples of pad for the sinc's taps), so a repeat held for minutes
  replays its first slice.
- **The cloud**: its own 4 s buffer, counted in the song's samples (still while frozen), with
  two octave copies (half and quarter rate, 63-tap Kaiser half-band, −80 dB, zero-phase); a
  grain transposed up reads the copy whose band, sped up, stays under Nyquist (`level_of`).
  Grain number `⌊so·density/fs⌋`; its place in the pool is that number modulo 128, so the
  grains sum in one order whatever came before; every draw a hash of (seed, number, purpose).
- **No allocation** on the audio path: every buffer is made in `new`; kernels, windows and the
  half-band taps are built there once (OnceLock).
- **Latency 0.**

## Measured (cantus-dsp tests, 26 of them; numbers from `--nocapture`)

| What | Measured |
|---|---|
| Null | bit-exact at the defaults (and with mix 0.4, swing 0.7, 4 bars of 1/64 empty), both paths |
| Timing | every module's output differs from the input only inside its block, at 60, 120 and 174 BPM, straight and swung; gate, crush, filter and pitch differ from the block's **first sample** and are back to the input by **the next line's sample** |
| Repeat accuracy | each repeat bit-identical to the captured slice × its decay gain after the join, at 120 and 174 BPM; repeat lines within 0.5 sample of the ideal (rounded once, never accumulated); a repeat held 11 s still replays its first slice to the bit; the ramp's four parts replay slices of 1/8, 1/16, 1/32, 1/64 to the bit |
| Reverse, shuffle | the slice before, backwards, to the bit; shuffled slices bit-identical to slices 0–3 back, every distance rolled |
| Joins (spectral splatter above 4 kHz, peak, on a 233 Hz sine; the same splices made hard beside) | repeat 1/32 **−70.9 dB** (hard −14.4), reverse 1/16 **−63.7 dB** (hard −3.0), shuffle 1/16 **−61.7 dB** |
| Render from bar 5 | bit-equal to the whole render from there, **at once** for repeats, gates, crushes, filter sweeps and tape stops (`reach` = 0), at 60/120/174 BPM; with reverse, shuffle, pitch and the cloud, bit-equal after `reach` (0.36 s at 174 BPM) — and different before it, as it must be |
| Block sizes | 1, 37, 64, 512 (and 61, 333): the same bits, with every lane, the cloud, swing, the ramp, pitched and decaying repeats, reversed shuffles, notes |
| Rolls | probability 0.5 on 128 steps: 30–70 % play; the same at every block size and from bar 5; another seed rolls another song |
| Notes | a note acts on its own sample at every block size; released, gone `xfade` later; quantised, on the next line |
| Tape | stop: under 0.02 by its line at 60/120/174 BPM, the next bar dry to the bit; start: silent at first, at speed (400 Hz ± 4) after a beat |
| Pitch jump | the strongest line within half the tap rate of the interval (+12: 631 Hz for 622; +7: 470 for 466; −12: 151 for 155.5) — the classic delay-line glitch pitch, not a tuner's |
| Filter sweep | 3 kHz through at the start (> 0.45), gone (< 0.02) at the end of a 16 k → 200 Hz sweep |
| Throw | echoes on the dotted eighth to the sample, ×0.5 each at feedback 0.5; flushed to zeros (−160 dB floor) |
| Cloud | seeded (same bits at 512 and 61, another seed differs); frozen: 440 Hz −20.8 dB vs the new input's 1 500 Hz −162.8 dB; live: the reverse |
| Pitched grains (Welch, worst stray tone ≥ 500 Hz, 600 Hz from the wanted) | 1 k +12: −134.6 dB · 9 k +7: −58.2 · 5 k +19: −49.4 · 3 k +24: −116.4 · 9 k +12: −134.6 · 2 k −12: −85.2 |
| Rest | heavy settings (every lane, the cloud at 80 grains/s, a throw) at rest 10.4 s into a silence (the 8 s history, then the echoes); a twin that skips plays the same bits when the sound returns |
| CPU (release, one core, best of three, **while other builders compiled**: an upper bound) | a wire 0.19 %; the stutter build 0.79 %; the cloud at its defaults beside the chain 0.91 %; **100 grains at once** (200/s × 500 ms, ±24 st) **9.4 %** |
| Memory | 8.6 MB at 48 kHz (history 8 s, capture 2 s, cloud 4 s + octaves, throw 2 s, pitch window) |

## Sounds — `C:\projects\daw-fx\compartir\fractus\sound\`

`cargo run -p cantus-dsp --release --example fractus_sounds -- <guitar wav dir> <out dir>`.
48 kHz 24-bit WAV (cantus-dsp has no FLAC writer; the engine phase can write FLAC through
`cantus-export` as the siblings did). Each beside its dry twin (`00-…`, −6 dBFS peak), level
matched by RMS. Sources: **drums** played by Pulsus (model 2) in the example — a two-bar loop
at 120 BPM; **guitar** — Carbonthief's CC0 DI strums, the owner's progression (Am – F – C –
G, 90 BPM, eight bars); **pad** — band-limited saws synthesized in the example. README there
credits them and lists the cost.

1. `01-drums-stutter-build` — reversed last beat, a gated and shuffled bar with a throw, then
   the build (1/4 → 1/32, rising a quarter tone a repeat, a high-pass opening) into the drop.
2. `02-drums-shuffle` — sixteenths re-ordered from up to four back, some reversed, crushed
   steps, a tape stop closing each two bars; probability 0.9.
3. `03-drums-glitch-performance` — played from notes, quantised: repeat, reverse, 1/32 gate,
   tape stop, a fifth up, a throw, a shuffle, a crushed repeat, a closing sweep.
4. `04-guitar-sequenced` — reversed beat with a throw, an octave up, a falling 1/32 stutter
   under a closing low-pass.
5. `05-guitar-cloud` — a cloud beside the guitar in fifths and octaves, 40 % backwards.
6. `06-pad-frozen-cloud` — only the cloud; frozen bars 3–6 (the chords move on, the cloud
   holds Am and F), thawed at bar 7.
7. `07-pad-pitched-cloud` — percussive grains an octave or two up (or a fifth) over the pad.

**What the owner should listen for**: the joins (no clicks at any repeat or slice — compare
with the dry); the build's acceleration landing exactly on bar 5; the shuffle's slices
staying in time; the frozen cloud holding the earlier chords while the pad moves; the
pitched cloud's brightness (grains just above an octave read the half-rate copy, so very
high content is gently duller there — by design, no aliasing); the pitch jump's character
(the classic delay-line warble, deliberately not a clean shifter).

## Tests and what each holds

`crates/cantus-dsp/src/fractus_tests.rs`: the null (both paths); each module inside its steps
at three tempos with swing; bar 5 (reachless at once, everything after `reach`); block sizes;
repeats to the bit and their lines; reverse to the bit; shuffle's slices; tape stop's end and
the dry bar after; the gate's pulses; joins against hard cuts.
`fractus_tests2.rs`: notes at their sample at four block sizes; quantised notes; a held
repeat beyond the history; the rolls (count, block size, bar 5, seed); rest and the skip
twin; the cloud's seed and telemetry; freeze; pitched grains' interval and strays; scales;
the throw's echoes and flush; memory; the pitch jump; the filter sweep; the ramp; tape
start; stopped transport.

## Shared files touched

`crates/cantus-dsp/src/lib.rs`: one block, `pub mod fractus; // fx-a:fractus` (in a group of
its own so rustfmt does not sort it into wave 3's). Everything else is new:
`fractus.rs`, `fractus_seq.rs`, `fractus_read.rs`, `fractus_fx.rs`, `fractus_cloud.rs`,
`fractus_run.rs`, `fractus_tests.rs`, `fractus_tests2.rs`, `examples/fractus_sounds.rs`.

## Verified only in emulation

Everything here is the DSP alone: no engine, no host, no UI. The CPU numbers were taken while
other builders compiled on the machine. The sounds are not yet heard by the owner.

## What the later phases need

- **Core**: `EffectKind` (the next contract), the parameters (≈ 60: the globals, each
  module's, the cloud's — note values as indices through `note_beats`, the step through
  `STEP_BEATS`), keep `out`; the pattern as content in `EffectSpec` (10 × 256 steps: kind,
  amount, probability — only the lanes that differ from empty), a `Command` to paint a block
  and to clear a lane with exact inverses (undo fuzz arms), a schema bump and fixture.
- **Engine unit**: `Fractus::new` off the audio thread (or the memory path, as Lapsus's
  history, if 8.6 MB per instance at 48 kHz — 34 MB at 192 kHz — is too much to build at
  insert); `SongPosition` → `Transport`; notes with offsets (`TAKES_NOTES`); AllOff on stop,
  seek and loop wrap; `latency() = 0`; `at_rest`/`skip`; `rt_memory`; the fx contract table
  (block 1/37/64/512 at tolerance 0, split, rest twin); `fx_fractus.rs` with the bar-5 and
  three-tempo tests through the engine; the heavy arm for the bench.
- **A panel with its own personality**: the lanes as the display — ten rows of steps painted
  with the pointer (start, drag to hold, height = amount, a probability stroke), the playhead
  from `t::PHASE`, lanes lighting from `t::ACTIVE`, a keyboard strip for the performance
  keys, the cloud drawn live from the grain telemetry (position × pitch), freeze as a large
  switch; fine strokes, a dark display in both themes, every label whole (no abbreviations),
  English and Spanish.
- **Presets** (ten or more, a starting preset that sounds at once; the defaults stay a null):
  stutter build, half-bar shuffle, tape-stop ends, gated sixteenths, throw on the fours,
  frozen pad, shimmer cloud, lo-fi fills, performance kit …
- **Export** (CLAP/VST3 bit-identical, notes through both formats, the pattern in its state),
  **docs** (`docs/plugins/fractus.md`, the manual EN/ES, the AI's description, the site's
  card), and **an end-to-end test** through `CantusApp::assemble`.
- **Open DSP choices for the owner's ears**: whether the pitch jump keeps its classic
  delay-line warble or gets a phase-aligned (WSOLA-style) splice; whether the grains need a
  finer interpolator than Hermite at the octave copies' top band (measured above).

## Design sheet v2 (the owner's verdict on v1: "more elaborate")

`C:\projects\daw-fx\compartir\fractus\design\v2\`. The display became the sequencer itself,
drawn in vectors from `kit::skin::sequencers` (a new palette; no texture tile, no layer-atlas
work, so `kit_alloc` is untouched):

- `views/fx/fractus_art.rs` — the machined chassis (a finish per colourway: black anodised
  and brushed, white powder coat with its speckle, violet lacquer with chrome lips; hex
  socket screws), the windows cut in it (chamfer, gasket, glass), the step pads lit from the
  floor up to the block's amount (the pad's whole height tinted and ruled: 100 %), square
  LEDs, the seven-segment digits, the keys (ivory and ebony, pressed and lit).
- `views/fx/fractus_display.rs` — the chassis's three windows: the lanes (pads per step, the
  beats grouped, the playhead's column, the amount and chance in words, a held note's key
  tagged on its lane, a LED per step, the counter), the keys that play the lanes; a lane's
  name opens the page of cards its module is set on. `View` keeps the last sixteen readings
  for the grains' trails.
- `views/fx/fractus_scope.rs` — the grain cloud on a scope's graticule (10 × 8 divisions,
  the centre lines ticked): each grain drawn as its window (`cwin`'s shape) as long as its
  size, its pitch written where there is room, its trail (matched by pitch and the way a
  grain at that pitch moves, never farther than it reads in its life); the spread's band and
  the scale's degrees; the position and its spray; the size's bracket; frozen frosted with
  "now" stopped and the chassis's Frozen lamp lit.
- `fractus.rs` — the cards in four pages (`PAGES`: Sequencer, Slicing, Colour, Grain cloud),
  each one row at 1340 wide in both languages (`fxa_cells::pack`); a strip of page keys
  (lamp, name, a dot per lane: filled when painted, lit while sounding); the window
  1340 × 720 (v1: 1400 × 1000). The demo: the blocks at different amounts, a dozen grains
  over the last second, two keys held (E3, F3) when notes play the lanes (`DEMO_KEYS`),
  sixteen readings 20 ms apart for the trails (`demo_series`).
- Shared, additive: `kit::cell::paint` grows a value pill into its cell when its words would
  be cut ("Fast attack", a scale's name — values that fit keep the pill); `fxa_cells` sizes a
  choice's cell by its words as the pill sets them (monospaced). `fxa_whole` probes the
  three other pages (keys held, frozen, the longest window name).
- Pitch jump's default stays **Classic warble** (the owner's verdict).

## Phase B (3), to the finish (2026-11-17): v2 approved

- **Red on arrival, fixed properly**: `learn_everywhere` named 16 of Fractus's stepped keys
  drawn without their offer. Fifteen stand on the three pages the demo does not open; the
  sixteenth, the ping-pong switch `wpp`, had **no control at all** — it is on the throw's card
  now. The first page's test excuses the other pages' keys by looking them up in `PAGES`
  (`fractus_page`), and `every_fractus_page_offers_its_stepped_parameters` opens each page and
  finds every stepped key on it offered — and every stepped key on some page.
- **The lanes painted by the pointer** (`fractus_paint.rs`): press and drag a block, its
  height the amount (5 % steps, what it covers is its own); Shift-drag a chance stroke;
  right-click erases; the Frozen lamp is a switch. Each gesture one undo step
  (`FxHost::set_fractus_lane`; Cantus merges `SetFractusLane` by the gesture, the plug-in
  posts the lanes). Page keys and lane names open pages (a view).
- **Presets** eighteen (`glitch.toml`), their lanes among their values (`"repeat.12" = 4`,
  `.amount`, `.chance`: `fx::fractus::to_flat`/`pattern_of_flat`), loaded with the values in
  one step everywhere a preset loads (header, browser, a host's load, A/B — now a flat map,
  `views::fx::preset_map` — a user's saved preset), compared by the drift mark, painted in the
  audition (notes presets on the source's last line, its first key on the line's lowest
  note). Fitted: −1.9 … +6.5 dB, nine as written (the starting stutter among them, so its
  spec is unchanged). Roles drums, bus, vocal, creative.
- **Export** `Cantus Fractus`: lanes in the state, posted like Lapsus's banks (the post made
  generic: `ContentPost<T>`), a fresh instance on the stutter, the DSP built at activation,
  the steps on the host's transport (CLAP and VST3, bit-identical to the DSP), notes playing
  lanes (CLAP both dialects, VST3), the Direct path, ids appended.
- **AI**: `set_effect` paints lanes by name (`lanes`), the words name them and the presets;
  MCP golden regenerated.
- **Site** card (EFFECTS 36, presets 18, big 1800 × 967, thumb 640 × 344 — pictures to make
  at the integration). **Docs**: ADR 0140 phase B, `docs/plugins/fractus.md`, manual EN/ES.
- **Tabula**: not a pedal — `SetFractusLane` has no pedal address (nothing could be painted on
  a board) and a board hears no notes (ADR 0140).

### Measured

- The e2e (120 BPM, through `CantusApp::assemble`): the stutter's first changed frame
  **75 000** = beat 3 + 1/32 exactly; live (blocks 256/333/512/17/1024) = offline to the bit;
  a painted gate and a muted track's note both first change frame **25 500** (beat 2's line +
  the 1/32 gate's open half); edits undone to the bit; defaults the dry master to the bit.
- The plug-ins: at 90 BPM from beat 2.5, the first repeat at frame **20 000** exactly through
  CLAP; CLAP and VST3 bit-identical to the DSP at blocks 1, 7, 512, 4096 and 5/333/64.

### Shared files touched (marked `fxa:fractus`)

Core: `fx/tabula.rs` (comment). UI: `views/fx/host.rs` (`set_fractus_lane`, `touches`,
the default `load`, `save_preset`, `copy_ab`), `views/fx/mod.rs` (`apply_fx_params`,
`toggle_ab`, `preset_map`), `views/fx/frame.rs` (`modified`), `preset_audition.rs`,
`preset_data.rs` (`check`), `agent_units.rs`, `examples/preset_tool.rs` (the header), tests
`learn_everywhere.rs`, `preset_audit.rs`, golden `mcp-tools-list.json`, locales
`areas/ai.{en,es}.txt`. AI: `tools_mixer.rs`, `tools_words.rs`, tests `tools_music.rs`.
Plug-ins: `Cargo.toml`, `build.rs`, `src/desc.rs`, `src/instance.rs`, `src/editor.rs`,
`src/state.rs`, `src/lapsus_bank.rs` (generic), tests `common/mod.rs`, `clap.rs`, `notes.rs`,
`transport.rs`, `vst3.rs`, `golden/ids.txt`, new `fractus.rs`. xtask: `main.rs`. Web:
`units_data.rs`.

### Verified only in emulation

- No glitch was played live on a monitor; the e2e renders, the engine driven block by block
  and the DSP's measures are the evidence. The exported plug-in only in our own CLAP/VST3
  harness, not in a third-party DAW.
- The presets' sound is not yet heard by the owner; *Cloud to freeze* needs Frozen thrown by
  hand (a preset frozen from its first sample would hold an empty buffer).
