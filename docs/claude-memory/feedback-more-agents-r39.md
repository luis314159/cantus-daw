---
name: feedback-more-agents-r39
description: Owner 2026-11-09 — the visual work is "muy muy importante": several agents on each aspect, and more agents on the pending items; overrides the round-39 prompt's three-agent cap (not the review cap)
metadata:
  type: feedback
---

On 2026-11-09, after beta.13 was tagged, the owner said: "esto es muy muy importante pon varios agentes a trabajar en cada aspecto a mejorar", "Pon más agentes a hacer los pendientes", and "Todo esto cuando termines con las pruebas de performance claro, para no afectarlas".

**Why:** the look is the owner's top priority, and they want throughput.

**How to apply:**
- Round 39 runs several design agents in parallel (one per whole-window direction, and an icon family), plus UX and pending-item builders.
- The machine still limits cargo builders to about three at once (31 GB RAM). Design agents run no cargo, so they don't count.
- Never run agents while a local performance measurement runs.
- Reviews stay at about 6 agents or fewer ([[feedback-review-cost]]).
- The owner still accepts the work, on whole windows ([[decision-round39-ux-graphics]]).
