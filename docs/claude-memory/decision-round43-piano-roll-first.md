---
name: decision-round43-piano-roll-first
description: Owner 2026-11-25 — adaptive piano roll leads round 43 (guitar engine round 44); expressive musician motion only after both Arcus musicians reach 100 %
metadata:
  node_type: memory
  type: project
  originSessionId: 46310d93-8b9d-4995-a1a6-07dcc8fb61ce
  modified: 2026-11-25T18:17:36.180Z
---

On 2026-11-25 the owner asked when the per-instrument adaptive piano roll would come. It had been planned for round 43b, after a depth round.

The new order, recorded in `docs/PLAN-after-round-42.md` on r42/integration:
- **Round 43** leads with the adaptive piano roll (BACKLOG "An instrument's playing profile"). Beside it run the polish of the Arcus musicians to 100 % and Chorda's sound.
- **Round 44** brings the guitar engine, built on the piano roll.

The owner also approved holding back the expressive musician: body sway and facial expressions driven by phrases, leaps, dynamics and vibrato (BACKLOG "The musician who plays expressively"). It starts only once both musicians look 100 %.

**Why:** the piano roll gives Arcus per-note control (bowing direction, techniques, vibrato), and every later instrument depends on it. Motion looks best on a figure that already looks right standing still.

**How to apply:** when writing round 43's contract and next-session.md, put the piano roll first. Do not start the motion work while the figure polish has open items.

Related: [[round-42]], [[decision-owner-delegates-excellence]].
