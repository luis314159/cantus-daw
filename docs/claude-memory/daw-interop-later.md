---
name: daw-interop-later
description: "The user wants Cantus compatible with the market's DAWs and plug-in standards; on 2026-09-24 plug-in standards (host VST3/CLAP, export ours) joined the instruments round"
metadata:
  node_type: memory
  type: project
  originSessionId: ab6ce7df-a46e-4ebb-a229-b046ae37783d
  modified: 2026-09-25T17:43:49.275Z
---

On 2026-09-24, while the plug-in agents were running, the user said Cantus should be
compatible with the DAWs on the market, and asked to look at it "al final" (at the end).
Later the same day, starting the instruments round (round 17), they made the plug-in half
explicit: "nuestro sistema debe ser compatible con los estándar y nuestros vsts e
instrumentos deben ser conectables como el estándar" — host third-party plug-ins and make
Cantus's own instruments/effects loadable in other DAWs.

**Why:** a DAW that cannot exchange work with Ableton, FL, Logic, Pro Tools, Reaper,
Bitwig or Cubase is hard to adopt; the user wants Cantus usable alongside them.

The user also told the effects session (2026-09-24) what they want:
- keep our effects **native** inside Cantus for performance;
- ship a **separate CLAP/VST3 build** of them "with the best performance possible";
- make Cantus **host** standard plug-ins.

The instruments belong to session `daw-b0`, which hosts instruments on tracks by
building on `cantus-host` rather than writing a second host.

**How to apply:** VST3 has been MIT since SDK 3.8 (Oct 2025). CLAP is primary per
ADR 0001, whose GPL reasoning should be revisited. Research for the instruments side
is in `research/instruments/standards.md`.

Round 17 on the effects side ran five builders on `r17/*` branches:
- `host`: CLAP plus the host core, ADR 0019. The contract is in
  `crates/cantus-host/src/backend.rs`.
- `vst3`: the VST3 backend.
- `export`: the `cantus-plugins` crate, ADR 0020.
- `engine`: the round-16 follow-ups, ADR 0022.
- `interop`: MIDI files, stems and DAWproject, ADR 0021.

**State on 2026-09-25:** all five branches are merged into the main working tree, which is
still uncommitted. 689 tests pass and clippy is clean. Schema 14 is the external
plug-in slot and 15 is `EffectSlot::midi_from`, so daw-b0's native-instrument migration
becomes 16.

`crates/cantus-plugins/tests/hosted_in_cantus.rs` proves the round trip: our CLAP build,
loaded by Cantus's own host, is bit-identical to the native units for all seven effects.
Build the plug-ins with `cargo run -p xtask -- bundle [--v3]`.

Still "later":
- AAF/OMF;
- audio in DAWproject;
- sync (MIDI clock, Ableton Link);
- crash isolation for running plug-ins;
- the other six editors inside plug-in windows.

ADRs 0023 and up go to daw-b0. The two sessions announce schema bumps and merges to
each other. See [[mixer-direction-fl-parity]] and [[cantus-parallel-builders]].
