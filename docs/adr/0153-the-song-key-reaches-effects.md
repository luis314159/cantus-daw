# ADR 0153 — The song's key reaches every effect

**Status:** Accepted (round 41's contract) · **Date:** 2026-11-20 · **Builds on:** ADR 0069
(the song's key, schema 43) · **Amends:** the effect unit's contract (`FxUnit`)

## Context

The project's key (`Project::key`, `Snapshot::key`) reached only the note effects (ADR 0069,
`configure_notefx`). An effect that should follow the song — Nitor's low end tuned to the tonic
(ADR 0151) — had no way to read it; Tonus carries its own key parameters. Gravis's
`set_song_key` existed with nothing in the engine calling it.

## Decision

- **`FxUnit::set_song_key(&mut self, key: Option<scale::Key>)`**, defaulted to do nothing:
  the engine hands each effect unit the snapshot's key **right after its spec**
  (`configure`) — on a strip (`rebuild_chain`), on the master (`install_master`) and when a
  hosted plug-in is restarted in place (`replace_effect`) — and a board hands it to every
  pedal. Allocation-free (a `Copy` value on the audio thread).
- `EffectUnit::set_song_key` forwards to an `Fx` unit; the engine's own kinds (delay, reverb,
  EQ, compressor) do not take it.
- An exported plug-in has no song: a unit that follows the key keeps a parameter for it (Nitor's
  `key`, 0 = the song's).

## Consequences

- Every existing unit is unchanged (the default does nothing): no golden hash moves.
- Gravis's own song-key plumbing (an instrument) is not part of this decision.

## What the tests hold

- `cantus-engine/tests/song_key.rs`: an effect on a strip and one on the master each receive
  the project's key (G, D) or none, after the render's snapshot.
- Every engine and null test unchanged.
