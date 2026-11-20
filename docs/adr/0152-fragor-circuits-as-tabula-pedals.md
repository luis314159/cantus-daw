# ADR 0152 — Fragor's five circuits as pedals of their own on Tabula's board

**Status:** Accepted as a contract (round 41; the owner's ask of 2026-11-18, BACKLOG §3b); the
`pedals` builder fills in the units and the faces · **Date:** 2026-11-20 · **Builds on:** ADR
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

## The builder's (to fill in)

Each pedal's unit: Fragor's DSP with only its circuit in the first slot, so a pedal and a
one-slot Fragor give **the same bits** (a test of all five, at defaults and drawn settings);
each pedal's table under Fragor's keys (its circuit's three knobs, its footswitch, `in`,
`out`, `mix`, `gate`, `mono`, `os`); the faces with Fragor's per-circuit art (its shapes,
paints, letterings and motifs); a few presets each; a new factory board in
`tabula_presets::factory()`. See `docs/round41/briefs.md`.

## Consequences

- One circuit costs about a third of a two-pedal Fragor board (BACKLOG §3b).
- A song with one of these pedals does not open in beta.16 or older (schema 78).

## What the tests hold

- `fx_contract::a_board_of_one_is_the_unit` covers the five (pedal-allowed kinds).
- `tabula_faces::every_pedal_has_one_face_of_its_own_keys` holds a face for each.
- `schema_78_…` reads the board of five back in order.
