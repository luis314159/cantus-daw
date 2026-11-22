---
name: decision-round41-content
description: "Owner 2026-11-20 approved round 41's effects/instruments (resonators, one-knob enhancer, Fragor pedals in Tabula), microtuning moved to round 42, AppImage dual-name rename in beta.16, zsync later"
metadata:
  node_type: memory
  type: project
  originSessionId: c330c055-9d87-4a1d-9df8-77cfd8e6440e
  modified: 2026-11-20T16:36:33.626Z
---

Owner, 2026-11-20 ("Sí dale si esto nos acerca a la excelencia"), approving the integrator's
recommendations:
- **Round 41's builds (after beta.16):** (1) a resonator instrument — exciter → string/bar/
  plate/membrane/tube (BACKLOG §3e item 6, §6l A2); (2) a one-knob enhancer unit covering
  §6l B9 (Soundgoodizer), B10 (low-end/sub-harmonic) and B12 (exciter by band), null at zero;
  (3) Fragor's five circuits as separate Tabula pedals (§3b, his 2026-11-18 ask), each bit-equal
  to Fragor with that slot alone. A contract commit first.
- **Microtuning** (next-session's round-41 item) moves to round 42.
- **beta.16:** the AppImage renamed per the catalogue bot (`Cantus-x86_64.AppImage`) but the old
  `…-linux-x86_64.AppImage` names still published, the updater trying new then old (beta.15's
  updater looks only for the old names, cantus-update/src/release.rs). zsync update info later.
- beta.15's public release body was corrected (macOS → beta.16, RVR1960 verse) the same day.

**Why:** rounds stay small (2–3 features); performance/excellence first.
**How to apply:** don't start round 41's builders before beta.16 is out; write the contract
(schema/ADR numbers, owners) first. See [[round-41]], [[feedback-work-standards-r36]].
