# Lapsus, the time shaper — the builder's report (fx side round)

Branch `fx/lapsus` (from `fx/integration` c730231c). ADR 0132 (provisional), schema 62 (the
side round's, provisional). Plug-in page `docs/plugins/lapsus.md`.

## Done

- **The model** (`cantus_core::fx::lapsus`): eleven keys, every default a null; **two banks
  as Gross Beat's** (the owner's verdict) — 36 time and 36 volume patterns, chosen apart
  (`tslot`, `vslot`), each by notes of its own range (`tkey` C4, `vkey` C1), switched or held,
  optionally restarted; the banks as content in `EffectSpec::lapsus` (only the slots that
  differ from the factory banks); `Command::SetLapsusPattern` with its exact inverse.
- **The DSP** (`cantus_dsp::lapsus*`): the time curve as a delay read from the unit's own
  history; each lane on its own cycle on the song's samples; nothing before a cycle's start
  read; joins crossfaded (inside a cycle after the jump, at the end before the line);
  distance-aware interpolation (line → Hermite → 32-tap Kaiser sinc) with speed-following
  anti-alias kernels; notes at their samples; lookahead with its latency; exact rest.
- **The engine unit**: the banks loaded slot by slot without allocating; the history by the
  memory path (`MemoryNeed::Lapsus`), swapped in without allocating.
- **The editor** (the v2 sheet as approved): the timeline (time over the cycle's input, the
  level under, the playhead and the slip); points dragged on the grid (Shift free), added and
  removed by double-click, segments bent, the pencil's steps; length; copy and paste; the
  banks behind a Time / Volume switch with their pads drawn and a keyboard with both ranges;
  a pad's right-click restores the factory's pattern. Finer strokes; the dashes under the
  preset field (the vents ornament) removed.
- **The face**: petrol under smoked glass, an amber VFD, ice-blue level (differs from every
  unit's: the family's sibling tests pass).
- **Presets**: seventeen, with styles and roles, levelled to the library (preset_render); **Half time** the starting preset.
- **The exported CLAP/VST3**: bit-identical to the engine's unit; notes through both formats;
  the banks carried in its state and posted to its processor without locks or allocation.
- **Docs**: ADR 0132, `docs/plugins/lapsus.md`, the manual (EN and ES), the site's card, the
  AI's description; the showcase's fills through *Beat repeat*.

## Measured

| What | Measured |
|---|---|
| Interpolation error (32-tap sinc) | −95 dB at 1 kHz, −88 dB at 10 kHz, −87 dB at 18 kHz |
| Aliasing read at 2× (15 kHz tone) | −93 dB against the plain kernel |
| Joins on a stuttered 220 Hz sine (largest second difference) | 0.0019 with 2 ms, 0.058 hard (the sine's own 0.0008) |
| Tape stop end | on the bar's own sample at 60, 120 and 174 BPM (DSP and engine) |
| Render from bar 5 | the whole render's bits, every pattern of both banks |
| At rest | after 4.0 s of silence (the history's length) at defaults and heavy |

## Tests and what each holds

- `cantus-core` `fx::lapsus::tests`, `undo_invariants` (the new command).
- `cantus-project` `schema_62_carries_lapsus_s_bank`.
- `cantus-dsp` `lapsus::tests` (22) and `lapsus::curve::tests` (6).
- `cantus-engine` `fx::lapsus::tests` (5), `fx_lapsus` (9), `fx_contract` (split, rest),
  `rt_memory::lapsus_never_allocates`.
- `cantus-ui` `fx_lapsus` (4, pointer-driven), `lapsus_e2e` (through `CantusApp::assemble`),
  `views::fx::lapsus::edit::tests`, the presets' and skins' suites, `demos`.
- `cantus-plugins` the CLAP/VST3 suites, `notes.rs` (Lapsus added), `ids` (ten ids blessed),
  `lapsus_bank::tests`.

## The finishing suite

`cargo test -p cantus-core -p cantus-dsp -p cantus-engine -p cantus-project -p cantus-plugins
-p cantus-ui -p cantus-ai --no-fail-fast`: 5 767 passed, 4 failed on the first run — the
presets' roles (effects take creative, drums, bus, vocal), a ♯ no bundled face draws (now #),
the slot parameters' learn offers (now on the banks' names) and six presets' levels — each
fixed and its binary run again green. `cargo clippy --workspace --all-targets -- -D warnings`
and `cargo clippy -p cantus-plugins --no-default-features --all-targets -- -D warnings`: clean.
`cargo fmt -p` on every crate touched. Not run: the Linux check in WSL (no push this round).
The frame budget with the window open and audio playing was not measured: other builders'
compilations ran the whole time (BUILDING: never measure beside a build) — for the integrator.

## Shared files touched (each line marked `// fx:lapsus`)

`cantus-core`: `effect.rs` (the field), `command.rs` (the command), `freeze_guard.rs`,
`record_mode.rs`, `starting.rs`, `tests/undo_invariants.rs`. `cantus-engine`: `effects.rs`
(the memory arms), `tests/support/heavy.rs`, `tests/rt_memory.rs`. `cantus-project`:
`examples/write_fixture.rs`, `src/format.rs` (a test), `fixtures/v62.cantus`. `cantus-ui`:
`views/fx/host.rs` (`set_lapsus_pattern`, `touches`), `views/fx/frame.rs` (designed size),
`kit/skin/side.rs` (the face), `agent_units.rs`, `demos/heart/desk.rs`, `tests/demos.rs`,
`tests/preset_audit.rs`. `cantus-plugins`: `lib.rs`, `instance.rs`, `state.rs`, `editor.rs`,
`tests/common/mod.rs`, `tests/notes.rs`, `tests/golden/ids.txt`. `web/src/units_data.rs`
(the card; the array's length 25 → 26 — Cribrum's card makes another, at merge 27). The docs.

## Verified only in emulation

The CLAP and VST3 plug-ins are exercised by the crate's own host emulation (as every unit's
are); no third-party DAW was used. The bank's main-to-audio post in a real host's threads is
covered by the unit test and the CLAP/VST3 suites, not by a DAW.

## Sound examples

`C:\projects\daw-fx\compartir\lapsus\sound\` (drums and bus from Cantus's audition sources,
each with its dry twin) and `sound\guitar\` — the owner's progression on Carbonthief's CC0 DI
guitar: tape stop ¼, tape stop over the bar, half time, the roll build, a gated stutter and
reverse, each level-matched to `00-guitar-di.flac` (RMS within 0.01 dB).

## Left

- Owner-only: listening to the guitar set and the presets' choices; whether *Half time* is
  the right starting preset.
- A decision for later (ADR 0132 Consequences): patterns count 4/4 bars from the song's start,
  so in 3/4 a one-bar pattern does not follow the bars.
- Optional at integration (the integrator's note): Lapsus colourways in Cribrum's
  mechanism, once merged.
