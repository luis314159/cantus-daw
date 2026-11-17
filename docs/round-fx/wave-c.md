# The effects side round, wave C — Purus and Comparatio: contract, engine, design sheets v1

Branch `fx/wave-c` (worktree `C:\projects\daw-fx\wave-a`), from `fx/wave-a` finished plus
`fx/purus` and `fx/comparatio` (938bdcfb). Steps 1 and 2 of the full lane; step 3 (editors
finished, presets, export, docs, e2e) waits for the owner's verdict on the sheets.

**Numbers are provisional**: wave B holds schema 65 and ADRs 0142–0144 in parallel
(`fx/wave-b`), so wave C takes **schema 66** and **ADRs 0145 (Purus), 0146 (Comparatio)**.
`migrate_64_to_65` is an **empty step marked "wave B's number, filled at the merge"** so the
chain reaches 66; the integrator replaces it with wave B's own and renumbers everything onto
master's numbers after round 40.

## Step 1 — the contract and the engine

- **Kinds** `EffectKind::Repair` (Purus) and `EffectKind::Reference` (Comparatio), appended
  after wave A's (`ALL` 38 → 40; wave B's three come before them at the merge). Every arm
  appended (core, engine, project/dawproject, UI, AI words and aliases, bench, picker).
- **Purus** (`cantus_core::fx::purus`, 27 keys): every module off by default (a null);
  `csens` stops at `SENSITIVITY_MAX` = 0.7 (and the engine clamps a hand edit);
  `DIRECT_DB_DEFAULT` = +6 dB, one constant equal to the DSP's default (a test). Engine unit:
  latency = the modules on (gentle 50 ms, de-click 767, de-reverb N − 1 = **23.2 ms at
  44.1 kHz**, 21.3 at 48), read live; telemetry with the 48 reduction bands in three pages;
  rest the DSP's except while estimating with a decay learned (it outlives silence).
  De-click shares the editor's declick cores bit-identically (ADR 0096; `purus_tests_shared`
  and the editor's `denoise::tests` green in the DSP suite).
- **Comparatio** (`cantus_core::fx::comparatio`, 12 keys): the mix heard by default. The
  reference is a file by path in `EffectSpec::ir` through `SetEffectIr` (undo-fuzz arm
  added); `comparatio_store` reads it off the audio thread (its own thread live, at once
  offline), analyses the whole file, and the compiled snapshot names it only once it is in
  (`poll_loaded` asks for a compile on arrival). **Only the live engine hands a unit its
  reference** (`EffectMemory::build_live`); renders answer with none, and a render's compile
  strips the reference and sets the mix. A swap or removal while heard fades through the
  mix. ADR 0146 decides the place (an insert meant for the master's last slot) and the
  plug-in's file story for step 3 (path + size + hash in the state, *Load reference…*).
- **Schema 66** (`migrate_65_to_66` writes nothing), `fixtures/v66.cantus` (a Purus with
  de-hum and de-reverb, a Comparatio with a reference by path), its test.
- **Starting presets**: Purus *Mains hum 50 Hz* (followed); Comparatio starts neutral (it
  measures).

### Tests (step 1)

| Suite | Result |
|---|---|
| `cargo test -p cantus-core` | 810 passed |
| `cargo test -p cantus-project` | 194 passed |
| `cargo test -p cantus-engine` (whole) | 1 305 passed, 0 failed |
| `fx_purus` | 6: null, latency per module at 48/44.1 kHz, compensation, the starting de-hum (> 25 dB), live = offline, blocks, fork, cap and constant |
| `fx_comparatio` | 7: null with/without a reference asked for, heard matched and back to the bit, swap through the mix, **no render ever holds the reference** (strip, master, blind X, the render compile), blocks 1/37/64/512/4096 and fork, a WAV through the store (resampled, whole-file loudness, missing file reported), live = offline |
| `fx_contract` | 40 kinds; heavy Purus (estimating) never rests |
| `rt_memory` | `purus_never_allocates`, `comparatio_never_allocates` (the rig answers needs as the live backend does) |

## Step 2 — the design sheets v1

`C:\projects\daw-fx\compartir\purus\design\v1\` and `…\comparatio\design\v1\`, each with
its `index.md` (what every picture shows, the owner's choices). Shot by
`crates/cantus-ui/examples/shoot_fxc.rs` with the clip-aware probe; `tests/fxc_whole.rs`
holds every word whole in both languages, Slate and Paper, 75/100/200 %, every colourway
and design state, the displays' size in both languages, and the objects' controls.

### Tests (step 2) and lints

| Suite | Result |
|---|---|
| `cargo test -p cantus-ui --lib` | 546 passed (the side round's and the families' palette tests included) |
| `fxc_whole` | 4: no word cut or clipped (both units, every design state, EN/ES, Slate/Paper, 75/100/200 %, every colourway at 100 %), the displays keep their size in both languages, Purus's bench and Comparatio's console answer real clicks (each one undo step) |
| `fx_lint`, `kit_lint`, `kit_alloc`, `preset_audit` | green |
| `ai_mcp_boot` | the MCP tools golden regenerated on purpose (the two effect words and aliases) |
| `cantus-dsp` `tests_shared` | the editor's denoise/declick hashes and Comprimo/Terminus's loudness hashes unchanged |
| clippy `-D warnings --all-targets` | clean on cantus-core, -project, -engine, -bench, -ai, -ui |

The colourways were moved once: a first choice collided with Anulus (walnut amber), Vetus,
Libra's stereo, Cribrum and an FM instrument; the hues now in `side_fxc.rs` were found by a
script against every landed sheet plus wave B's values from `fx/wave-b`.

## Verified only in emulation / not yet

- No ear has heard either unit in the app; the sheets' pictures are the real units over
  synthetic demo takes.
- Purus's live spectrogram needs the slot's scope (step 3); the sheet's comes from the
  demo's own analysis. Comparatio's mix tonal curve likewise (the reference's comes from
  the store's whole-file analysis already).
- No CLAP/VST3 export, presets beyond the starting one, e2e through `CantusApp::assemble`,
  manual or site card yet — step 3.
- Linux check (WSL) not run.
