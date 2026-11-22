# Round 41 — `pedals`: Fragor's five circuits as pedals of their own (ADR 0152)

The owner's ask of 2026-11-18 (BACKLOG §3b): the green overdrive, the op-amp distortion, the
silicon distortion, the germanium fuzz and the triangle muff each a stompbox of its own on
Tabula's board, with its own face and knobs, so several drives can sit anywhere in a chain,
between other pedals, beyond Fragor's three slots. Branch `r41/pedals` from `r41/contract`.

## What changed

- **The model** (`cantus-core/src/fx/fragor_pedal.rs`): each pedal's table under **Fragor's
  own keys** — the circuit's switch (`od.on` …), its three knobs (`od.drive` …), `gate`, `in`,
  `mix`, `mono`, `os`, `out` — the same ranges, defaults and words as Fragor's. The null is
  *the circuit out* (the switch defaults to off: a wire, no latency), as Arca's "no amp";
  every board pedal starts from its starting preset, circuit in. `as_fragor` / `from_fragor`
  turn a pedal into the one-slot Fragor it is and back. No schema change (78 holds it).
- **The unit** (`cantus-engine/src/fx/fragor_pedal.rs` and `fragor_pedal_dsp.rs`): Fragor's
  board with one slot, built with only its circuit — **two circuits where Fragor builds
  ten** — every step Fragor's, in Fragor's order. `fragor*.rs` in `cantus-dsp` untouched:
  no Fragor golden moves, `DSP_VERSION` stays 10.
- **The faces**: the five rows in `tabula_faces.rs` hold their circuit's three knobs and the
  case Fragor draws them in; `tabula_board::fragor_circuit` makes every one of Fragor's
  special cases on the board (finish, knobs and their words, name, art, lettering, paint,
  the strip, randomise, the keyboard's knob words) serve the five kinds. A pedal whose
  circuit is out shows its LED dark. On the shelf the whole Fragor is named *Fragor ·
  Distortion pedals*, so it is not mistaken for the green overdrive pedal beside it.
- **The strip under the board** (the five have no window of their own): knobs, Gate, Mix,
  Output, Oversampling (4×), Channels (Mono), Circuit (On) — stepped values in words — and
  *Open its full editor* is offered only for a pedal that has a window.
- **Presets**: 16, three or four each (`presets/effects/fragor*.toml`); twelve made from
  Fragor's own single-circuit presets (the same bits as Fragor's preset on one slot, so
  their fitted levels carry over), four new (Clean boost, Silicon lead, Fuzz under a bass,
  Wall of muff). Each pedal's starting preset is its first. The audit's floor for the five: 3.
- **A factory board**, *Stacked drives* (`tabula_presets::factory`): compressor → green
  overdrive as a boost → phaser → triangle muff → Arca's clean amp and 4×12 → delay → room;
  *Drive* turns both drives, *Echo* the delay.
- `picker_snap::effect_name` has the five; `demo_editor("fragor_od")` … opens Tabula with
  that pedal alone (`shoot_tabula` gained `circuits`, `stacked` and `drives`).
- Words in both locales (`fx/fragor_pedal.*.txt`), the assistant's description of the five
  (`agent_units.rs`), the manual's Tabula section in both languages, ADR 0152 filled, a
  *Ronda 41* item in `docs/beta-checklist.md`.

## How it is tested

- `cantus-engine/tests/fx_fragor_pedal.rs` — **each pedal is Fragor with only its slot
  filled, to the bit**: all five, side by side through the plug-in contract, on a guitar's
  phrase with 1.2 s of silence; scenarios: defaults, noon, heavy (gate, +12 dB in, −6 out,
  70 % mix, mono, 8×), the structure changing mid-note (the circuit out and in, 2×, 8×,
  mono, mix and input, a seek, the gate), and three drawn at random with eight changes, a
  seek and a reset each; blocks of 1, 64, 1024 and random splittings; every block's samples,
  latency, rest and its reason and all 64 telemetry numbers equal; the pedal block-size
  independent; and each scenario sounds (or, at the defaults, is the input to the bit).
  A mutation (one glide's tolerance) made it fail; reverted.
- Unit tests: the tables are Fragor's keys and ranges with a null default
  (`fragor_pedal::tests`), a pedal reads as its one-slot Fragor (`settings(...).board() ==
  fragor::settings(as_fragor(...))`), the defaults touch no sample and add no latency, the
  faces wear Fragor's knobs and cases, the showcase board stacks two drives around another
  pedal.
- `cantus-ui/tests/pedals_e2e.rs` — **end to end through `CantusApp::assemble`** and the
  engine's compile: Tabula inserted, emptied and filled from the shelf's path with the green
  overdrive, a phaser and the triangle muff; heard; **equal to the bit** to the three units
  inserted in a row with each drive as a one-slot Fragor; the muff's footswitch heard and
  undone to the bit; saved, opened again — the same project and the same render to the bit;
  a fuzz added to the rig heard and undone to the bit.
- `rt_memory::fragors_circuits_as_pedals_never_allocate` — the five at heavy settings with
  lanes taking the circuit out and in, sweeping the oversampling (circuits rebuilt in place)
  and a knob; play, seek, loop, stop: no allocation.
- The contract's `fx_contract` arms (rest, split invariance, a board of one is the unit) now
  run the real units at their heavy settings (`support/heavy.rs`, the bench's `kernels.rs`).

## Measured

`cargo test --release -p cantus-engine --test fx_fragor_pedal -- --ignored --nocapture`
(`cpu_of_each_pedal`): stereo, 4×, 48 kHz, a guitar's phrase, best of five, 256-sample
blocks. **The machine was loaded** (three other builds running): the ratios hold, the
absolute numbers are high; the integrator measures quiet.

| Circuit | Pedal | Fragor, that slot alone | Fragor, two slots (it and the next) | Pedal / two slots |
|---|---|---|---|---|
| Green overdrive | 5.76 % | 6.26 % | 12.34 % | 0.47 |
| Op-amp distortion | 9.10 % | 8.77 % | 20.99 % | 0.43 |
| Silicon distortion | 12.81 % | 14.65 % | 26.11 % | 0.49 |
| Germanium fuzz | 13.09 % | 12.74 % | 33.62 % | 0.39 |
| Triangle muff | 20.08 % | 20.80 % | 24.41 % (with the overdrive) | 0.82 |

- A pedal costs what Fragor costs with that slot alone, within the noise: Fragor already
  runs only the circuits on its board, so the per-sample work is the same. Against a
  two-pedal Fragor board it is 0.39–0.49 of it when the other circuit is a heavy one (the
  muff beside the cheap overdrive is 0.82), and mono (every factory preset but two) halves
  it again. *About a third* (BACKLOG §3b) holds against Fragor's 0.30 preset budget for its
  heaviest two-pedal board: the heaviest pedal, the muff, 0.20 stereo, 0.10 mono.
- **Memory**: one circuit is 40 704 bytes; Fragor holds ten (407 kB), a pedal two (81 kB) —
  five times less per drive on a board, and a fifth of the circuits rebuilt when its
  oversampling changes on the audio thread.

- **The preset render check** (`cargo test --release -p cantus-ui --test preset_render`,
  loaded machine): every one of the five pedals' 16 presets and *Stacked drives* inside its
  level window (the twelve made from Fragor's within 0.3 LU before any fit; the four new
  fitted by `preset_tool fit`: Clean boost −11 dB, Silicon lead −6.6, Fuzz under a bass
  −1.3, Wall of muff −5.6), no clipping; CPU 0.04–0.10 of a core (*Drum smash*, stereo, at
  0.093–0.10), so the five get a budget line of 0.15 (`preset_measure::budget`). The run's
  other CPU failures (Fragor's *Synth fuzz* 0.31, two of Saltus's at 0.11) are other units'
  under the machine's load (three builds beside it), not this branch's.

## Test counts (Windows, this branch; a loaded machine)

- `cantus-core`: 928 pass (the undo fuzz among them: no new command, it stays passing).
- `cantus-engine`: 1 455 pass; 1 failed — `sync_midi::the_out_thread_sends_on_time`, a
  timing bound (median 3.4 ms against 0.5) while the machine was out of memory with four
  builds; nothing of this branch touches MIDI out — rerun alone: 9 of 9 pass.
- `cantus-bench`: 10 pass.
- `cantus-ui` (the lib and the touched tests: `pedals_e2e`, `preset_audit`, `tabula_e2e`,
  `fragor_e2e`, `agent_tools`, `picker`, `fx_frame`, `fragor_colourways`): 630 pass; the one
  failure, `fx_frame` (the five claimed an editor they do not have), fixed by
  `EffectKind::has_editor` false for them; `fx_frame` 5 of 5 and `pedals_e2e` pass after it.
- Release: `preset_render` (above), `fx_fragor_pedal::cpu_of_each_pedal`.
- Clippy `--all-targets` with `-D warnings` on core, engine, bench and ui: clean. `cargo fmt`
  on core, engine, bench; `rustfmt` on the touched UI files (`cargo fmt -p cantus-ui` fails
  here, os error 206).
- Not run by me: the whole `cantus-ui` suite, `cantus-plugins`, the Linux check (WSL is the
  integrator's).

## Shared files touched

`crates/cantus-core/src/effect.rs` (`has_editor` false for the five), `crates/cantus-core/src/starting.rs` (the five starting presets; Nitor's arm split out
unchanged), `crates/cantus-core/src/fx/tabula_presets.rs` (one board, one test),
`crates/cantus-ui/presets/effects/board.toml` (its tag), `crates/cantus-engine/tests/
{rt_memory.rs, support/heavy.rs}`, `crates/cantus-bench/src/kernels.rs`,
`crates/cantus-ui/tests/preset_audit.rs` (floor 3), `crates/cantus-ui/src/preset_measure.rs` (a budget line), `crates/cantus-ui/src/app.rs`
(`demo_editor`'s pedal names, `demo_pedal_board`), `crates/cantus-ui/src/views/picker_snap.rs`,
`crates/cantus-ui/src/agent_units.rs`, `crates/cantus-ui/examples/shoot_tabula.rs`,
`docs/manual/{en,es}/README.md`, `docs/beta-checklist.md`. Every line marked `r41:pedals`.

## Verified only by measurement or emulation

- Bit-identity with Fragor is proved by tests; how the five *sound* is Fragor's, already
  heard — but the four new presets and the *Stacked drives* board are by my choice of
  values, not by ear: the owner's to judge.
- The pictures are CPU renders (`shoot_tabula`); the owner's screen decides.

## For the owner

- Look at `docs/round41/pedals/` (the five on a board, Slate and Paper, 100 % and 150 %; the
  showcase board; the Distortion shelf).
- Listen: the four new presets and *Stacked drives* (render them with the preset audition).
- Decide: do the five want a window of their own (Fragor's window showing one circuit)?
  Today the strip under the board holds what a player sets.
