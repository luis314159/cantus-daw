# ADR 0152 — Fragor's five circuits as pedals of their own on Tabula's board

**Status:** Accepted (round 41; the owner's ask of 2026-11-18, BACKLOG §3b); built by the
`pedals` builder, the faces and new presets awaiting the owner's eye and ear · **Date:** 2026-11-20 · **Builds on:** ADR
0128 (Fragor), ADR 0131 (Tabula) · **Schema:** 78

## Context

Fragor enters a board as one pedal — the whole unit, its three slots (ADR 0131's table). The
owner wants the green overdrive, the op-amp distortion, the silicon distortion, the germanium
fuzz and the triangle muff each a stompbox of its own, so several drives can sit anywhere in a
chain, between other pedals, beyond Fragor's three slots.

## Decision (what the contract fixed)

- **Five effect kinds**, appended to `EffectKind::ALL` after Nitor, in Fragor's model order:
  `FragorOverdrive`, `FragorOpAmp`, `FragorSilicon`, `FragorFuzz`, `FragorMuff`
  (`EffectKind::fragor_circuit` gives 0 … 4).
- **Board-only**: `EffectKind::offered` is false for them (no insert menu, no main picker, no
  agent insert); `pedal_allowed` is true. Tabula's shelves read
  `picker_catalog::board_devices` (every native, the board-only ones included); the main
  picker reads the catalogue without them.
- **Not exported** as plug-ins (Fragor is the plug-in).
- Their core module `fx::fragor_pedal` (`STEMS`, `NAME_KEYS`, the stub's `out`), a stub engine
  unit (`cantus-engine/src/fx/fragor_pedal.rs`, a wire times `out`), five pedal face rows,
  one locale file for the five (`fx/fragor_pedal.*.txt`), five empty preset files (floor 0).
- **Schema 78**: the fixture holds a board of the five, in order.

## What the builder settled (round 41, `pedals`)

- **The keys are Fragor's** (`cantus_core::fx::fragor_pedal`), forever: the circuit's switch
  (`od.on`, `op.on`, `si.on`, `ge.on`, `mf.on`), its three knobs (`od.drive` …), and `gate`,
  `in`, `mix`, `mono`, `os`, `out` — the same ranges, defaults and words as Fragor's, so a
  pedal and a one-slot Fragor read the same values and one of Fragor's single-circuit
  presets is a pedal preset key for key (`as_fragor`, `from_fragor`).
- **The null is the circuit out.** A circuit colours at every knob, so — as Arca's "no amp"
  — the switch defaults to *off*, and off is a wire (true bypass): Fragor with its board
  empty, no latency. A pedal on a board always starts from its starting preset
  (`starting_spec`, ADR 0071), which puts the circuit in. The board's footswitch
  (`Pedal::on`) stays the one a player stamps on, and keeps the board's latency (ADR 0131
  §3); the circuit's switch is structural, as Fragor's slot is (its 10 ms dip, the latency
  following). *Rejected:* the switch on by default with Fragor's footswitch meaning (off =
  the dry delayed by the latency) — the defaults would sound, or would add latency.
- **The unit** (`cantus-engine/src/fx/fragor_pedal.rs`, `fragor_pedal_dsp.rs`): Fragor's
  board with one slot, built with **only its circuit** — two `Pedal`s (one per channel)
  where Fragor builds ten — every step Fragor's in Fragor's order: the gate, the input, the
  oversampling, mono, the dry path delayed for the mix, the output, the 16-sample grid and
  its glides, the dip, the rest after a second of silence, the same telemetry layout as a
  one-slot Fragor. What one circuit never needs is left out (a per-pedal footswitch, a
  reorder). It copies Fragor's control flow rather than calling into it because
  `cantus-dsp/src/fragor*.rs` is nobody's this round; `fragor.rs`, `fragor_dk.rs` and
  `fragor_circuits.rs` are untouched, so no Fragor golden can move, and the equality tests
  fail the day Fragor's board changes and this does not follow.
- **The faces**: each kind's row in `tabula_faces.rs` holds its circuit's three knobs and its
  case's natural shape (`fragor::SHAPES`); the board draws a pedal of these kinds as it
  draws Fragor's own pedals — the circuit's finish (`pedals::FRAGOR`), paint, lettering,
  artwork, name and knob words — through `tabula_board::fragor_circuit` (Fragor's first
  slot for a Fragor, the kind's circuit for the five), which every one of Fragor's special
  cases in `tabula_board.rs`, `tabula.rs` and `tabula_edit.rs` now reads.
- **No window of their own**: the strip under the board shows everything a pedal has — its
  knobs, gate, input, mix, output, oversampling, channels and its circuit — and *Open its
  full editor* is offered only for a pedal with a window. A full editor (Fragor's window on
  one circuit) waits for the owner's design decision.
- **Presets**: three or four each (`presets/effects/fragor*.toml`), the first of each the
  starting preset: twelve made from Fragor's own single-circuit presets (the same bits as
  Fragor's preset on one slot, so their fitted levels carry over) and four new (clean boost,
  silicon lead, fuzz under a bass, wall of muff); the audit's floor for the five is 3. A new
  factory board, **Stacked drives** (`preset.tabula.stacked_drives`): compressor → the green
  overdrive as a boost → a phaser → the triangle muff → Arca's clean amp and 4×12 → delay →
  room, its *Drive* macro turning both drives.
- `picker_snap::effect_name` names the five; `demo_editor("fragor_od")` … pictures a pedal on
  a board of its own (Tabula holding it alone, chosen).

## Consequences

- A pedal costs what Fragor costs with that circuit alone (Fragor already runs only the
  circuits on its board): 0.39–0.49 of a two-pedal Fragor board holding it and another heavy
  circuit, stereo at 4× (measured on a loaded machine, `fx_fragor_pedal::cpu_of_each_pedal`;
  `docs/round41/pedals.md`), half that in mono. Its memory is a fifth of Fragor's (two
  circuits of 40 704 bytes, not ten).
- Several drives on one board cost what each costs, added.
- A song with one of these pedals does not open in beta.16 or older (schema 78).

## What the tests hold

- **Each pedal is Fragor with only its slot filled, to the bit** (`cantus-engine`
  `fx_fragor_pedal`, one test per circuit): defaults, noon, heavy, the structure changing
  mid-note, three drawn settings with seeks and resets; blocks of 1, 64, 1024 and random
  splittings; samples, latency, rest and telemetry, every block; rest and skip as the engine
  does them.
- The tables are Fragor's keys, ranges and words with a null default; a pedal reads as its
  one-slot Fragor; its defaults touch no sample and add no latency (`fragor_pedal::tests`,
  core and engine).
- `fx_contract::a_board_of_one_is_the_unit`, rest and split invariance cover the five at
  their heavy settings; `rt_memory::fragors_circuits_as_pedals_never_allocate`.
- `tabula_faces::every_pedal_has_one_face_of_its_own_keys` and
  `fragors_circuits_wear_fragors_knobs_and_cases`.
- `tabula_presets::the_showcase_board_stacks_two_drives_around_another_pedal`; the preset
  audit's floor of three each.
- **End to end** (`cantus-ui` `pedals_e2e`): through `CantusApp::assemble`, two drives around
  a phaser on a board, heard, equal to the bit to the three inserted in a row with each drive
  a one-slot Fragor, a footswitch, saved and reopened to the bit, undo.
- `schema_78_…` reads the board of five back in order.
