---
name: mixer-direction-fl-parity
description: "The user wants Cantus's mixer to follow FL Studio's, then a FabFilter-class plug-in family, pro instruments, a mix recorder and a first-rank UI; what is done and what is next"
metadata:
  node_type: memory
  type: project
  originSessionId: ab6ce7df-a46e-4ebb-a229-b046ae37783d
  modified: 2026-09-24T13:54:38.797Z
---

On 2026-09-20 the user asked for FL Studio's mixer features first (FL manual mixer.htm as
reference), prioritising sends/linking, channel groups, the per-channel effects rack, the
channel EQ and the channel-rack link. On 2026-09-23 they added docks/separators and, as
pending work, FabFilter-class effects, "very pro" instruments with their own UIs, a
recorder inside the mix, and a GUI at the level of a first-rank product. On 2026-09-24
they said to carry on with that plan and to put agents to research every FabFilter Pro
bundle product (with official screenshots) so ours ends up *better*, not a clone.

**Why:** the user builds Cantus as an FL-shaped DAW and judges it against FL and FabFilter
and wants it to feel like a professional commercial product.

**How to apply:** Rounds 13–15 (ADRs 0015–0017) did the FL strip, docks/groups, the house
knob, meter ballistics, the mix recorder, Aequalis (EQ) and Comprimo (compressor with
sidechain from any strip). Round 16 (2026-09-24, ADR 0018) finished the effect family
with one agent per plug-in: EQ, compressor, Terminus limiter, Spatium reverb, Multiplex
multiband, Sibilus de-esser and Porta gate, plus delay compensation. The research is in
`research/fabfilter/` (README indexes it; screenshots are git-ignored). Next, per
docs/STATUS.md: the instruments (a parallel session `daw-b0` started that round), then DAW
interop ([[daw-interop-later]]), then the plug-in gaps STATUS lists.
Check the UI visually with `cargo run -p cantus-ui --example shoot -- <dir>` (software
renderer; the dev machine's screen may be locked). See [[cantus-workflow-habits]].
