# Round 39 — `conv`: Aula, the convolution reverb (ADR 0112, schema 60)

The owner asked for a convolution reverb (2026-11-09). **Aula** (Latin, *the hall*) is a new
effect kind, inserted from the picker's *Space* family, beside Spatium.

## What changed

| Layer | Files (owned) | Shared files (additive lines marked `// r39:conv`) |
|---|---|---|
| DSP | `cantus-dsp/src/aula.rs`, `aula_conv.rs`, `aula_kernel.rs`, `aula_fft.rs`, `aula_shape.rs`, `aula_gen.rs`, tests `aula_tests.rs`, `aula_conv_tests.rs` | `lib.rs` |
| Model | `cantus-core/src/fx/aula.rs` (params, `IrFile`) | `effect.rs` (kind, `EffectSpec::ir`), `command.rs` (`SetEffectIr`), `starting.rs` (*Concert hall*), `freeze_guard.rs`, `record_mode.rs`, `fx/mod.rs`, `lib.rs`, `tests/undo_invariants.rs` (fuzz arm) |
| Project | — | `format.rs` (schema 60), `examples/write_fixture.rs`, `fixtures/v60.cantus`, `dawproject/mod.rs` |
| Engine | `cantus-engine/src/fx/aula.rs`, `aula_store.rs`, `aula_load.rs`, `tests/fx_aula.rs`, `tests/data/aula/` | `fx/mod.rs` (two trait methods with defaults, the factory), `effects.rs` (`MemoryNeed::Aula`, `EffectMemory::Aula`, two dispatch arms), `backend.rs` (files read at compile), `tests/fx_contract.rs`, `tests/rt_memory.rs`, `tests/support/heavy.rs` |
| UI | `views/fx/aula.rs`, `aula_view.rs`, `locales/areas/aula.{en,es}.txt`, `presets/effects/aula.toml`, `examples/shoot_aula.rs`, `tests/aula_e2e.rs` | `views/fx/mod.rs`, `frame.rs`, `host.rs`, `inspector_mini.rs`, `mixer_desk.rs`, `picker_snap.rs`, `picker_catalog.rs`, `preset_data.rs`, `i18n.rs`, `agent_units.rs`, `app.rs`, `samples_drop.rs` (a drop target), `demos/heart/desk.rs` (the showcase's guitar), `kit/skin/tone.rs` (Aula's face) |
| Agents | `cantus-ai/src/tools_aula.rs` (`set_impulse_response`, files class) | `tools_project.rs`, `tools_args.rs`, `tools_mixer.rs` |
| Plug-ins | `cantus-plugins/src/memory.rs` (a builder thread for needs) | `desc.rs`, `instance.rs`, `Cargo.toml`, `build.rs`, `text.rs`, `lib.rs`, `tests/common/mod.rs`, `xtask/src/main.rs` |
| Content | — | `cantus-content/manifest/content.json` (the *OpenAIR Impulse Responses* pack, CC BY 4.0) |
| Docs | `docs/adr/0112-…`, `docs/plugins/aula.md`, this file | manual EN/ES (a section and the list of effects), `web/src/units_data.rs` (the unit card's words) |

## Pictures

`docs/round39/conv/`: a file loaded (York Minster, OpenAIR) at 100 % and 150 %, Cantus's
own large hall (English, Spanish), the cathedral reversed in Paper — `shoot_aula`, with
`CANTUS_AULA_IR=<a WAV>` for the file. With a file playing, the list of Cantus's own reads
*— none: a file is playing* and the file's name is lit (the coordinator's review; spoken the
same to a screen reader, `tests/fx_aula.rs`).

## How it works (ADR 0112 has the whole)

Zero latency: the first 64 taps in direct form; levels of uniform partitions of 64 / 512 /
4 096 / 32 768 (15, 14, 14, as many as needed), level *k* starting at twice its partition so
its work is spread evenly over its slack's 64-sample ticks by a schedule made with the kernel.
The grid counts from the stream's start, so any block split gives the same bits. Kernels are
built off the audio thread through ADR 0022's path (now open to every `FxUnit`), kept in a
pool so the audio thread never frees one, and swapped level by level with the inputs' delay
lines kept (the tail survives a new response).

## Measured

- **Accuracy:** against a direct convolution in `f64`, lengths 1 … 70 000 taps, mono, stereo
  and true stereo: worst error **3.1 × 10⁻⁷ of the peak (−130 dB)**.
- **Published files:** the eight OpenAIR responses (16-bit, 24-bit at 44.1/48/96 kHz,
  32-bit float mono; two with their `fmt` chunk after others) read to the value against a
  reader written from the RIFF specification, and played raw within −100 dB of their samples
  (`CANTUS_AULA_OPENAIR=…/downloads/conv/openair`, run here; the files are not in the
  repository). Outside-written files in the repository: libsndfile's mono 16 / stereo 24 /
  float, and a four-channel 24-bit file written by CPython's `wave` module.
- **CPU:** see the table below (`fx_aula::what_it_costs`, `--ignored`, this machine, other
  builds running).

| Response | Blocks | Real time | One core | Worst block |
|---|---|---|---|---|
| Cathedral stretched ×1.5 (≈ 11 s, true stereo, 4 levels) | 64 | 20× | 4.9 % | 372 µs of 1 333 µs |
| same | 512 | 23× | 4.4 % | 1 144 µs of 10 667 µs |
| Studio room (true stereo) | 64 | 34× | 2.9 % | 278 µs of 1 333 µs |
| same | 512 | 36× | 2.8 % | 844 µs of 10 667 µs |

Measured in a debug-profile test (DSP and engine at opt-level 3) while three other builds ran
on this machine: the worst blocks are scheduler noise as much as work. The owner's quiet
machine and a release build will read lower; the scorecard's CPU row should be measured there.

## Tests

- `cantus-dsp` (17, `aula*`): direct convolution, block splits at tolerance 0, swap, rest,
  schedule, FFT, shaping, Cantus's own responses, the null.
- `cantus-core`: all pass (the undo fuzz with `SetEffectIr`); `cantus-project`: all pass,
  `v60.cantus` written and every fixture loading.
- `cantus-engine`: the whole suite passes; `fx_aula` (7, two with the published files and the cost run),
  `fx_contract` (split invariance and exact rest for Aula: default, heavy, random),
  `rt_memory::aula_never_allocates`, the unit's own tests.
- `cantus-ai`: all, with `tools_aula` (the tool's refusals and its undo).
- `cantus-plugins`: all (CLAP, VST3, hosted in Cantus, ids blessed for Aula).
- Clippy with `-D warnings` on every crate touched (all targets): clean.
- `cantus-ui`: `aula_e2e` (through `CantusApp::assemble`), `fx_aula` (a file playing is the
  source shown and spoken), the library (473), `agent_tools`, `ai_mcp_boot` (the tools list
  dumped), `demos`, `picker`, `preset_audit`, `fx_lint`, `learn_everywhere`,
  `modulate_everywhere`, `fx_frame`, `library_fixes`, `master_chain`, `clipfx`,
  `legacy_reverb`, `skins_boot`, `skins_budget`. The whole UI suite is the integrator's run.

## Verified only in emulation / needs the owner

- **Ears:** Cantus's fifteen responses (`aula_gen`) are generated, not recorded — rooms,
  halls, chambers, plates and two springs. They are measured (finite, decaying, longer rooms
  longer, true stereo) but how good they *sound* needs the owner's ears, especially the
  springs and the plates; the presets' levels were not fitted with `preset_tool fit`.
- **Real devices:** no ASIO/WASAPI run with Aula on a live strip (the engine's manual handle
  and the offline render only); a live swap's timing (a kernel built on the interface's
  thread, tens of ms for a long true-stereo hall) wants a look on the owner's machine.
- **Licence:** the OpenAIR pages' CC BY 4.0 notice was read on the Internet Archive's copy of
  June 2024 (the live site's certificate had expired; nothing bypassed). The owner may want
  to confirm before the pack is offered.

## Left

The website's card (pictures from `shoot_skins`, three excerpts from `render_unit_sounds`)
and the README's row are done (`96ced5d4`).


- **The presets' levels** (`preset_tool fit`, release) and their audition by ear.
- **No WSL/Linux run** (this round's rule): `cfg(windows)`-free code, but unchecked on Linux.
