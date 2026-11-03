---
name: feedback-work-standards-r36
description: "Owner approved 2026-11-03 new working rules after round 36 — end-to-end test per feature, external-source protocol tests, smaller rounds with mid-round integrated review, explicit git staging, no-debug main-tree builds"
metadata:
  node_type: memory
  type: feedback
  originSessionId: 2f8b0af2-408b-4947-b282-16b12b3313ac
  modified: 2026-11-03T18:20:53.511Z
---

After my assessment of round 36 (2026-11-03) the owner said "me parece bien estos cambios en cómo
trabajamos". Now in CLAUDE.md ("How work is done here" and "Parallel builders" 5–6):

- A feature is done only with an **end-to-end test through the real boot/assemble path** (an
  external instrument had dozens of green unit tests and never sounded in the app). See [[gotcha-boot-path]].
- **Protocols/formats tested with bytes from outside sources**, never only our own emulation (HUI
  tables were wrong and the emulation shared the mistake).
- **Rounds of 2–3 features**, first deliveries merged early, **integrated-tree review mid-round**
  (the worst bugs were cross-feature: mix cues × freeze × recording mode).
- **Stage by explicit path**, never `git add -A`/folders — I staged the owner's untracked
  docs/monetize.md in round 36 and had to amend it out.
- Main-tree builds with debug info off; check disk before a workspace build (100 GB target filled F:).
- Real hardware and users outrank more process; reports say what was verified only in emulation.

**Why:** round 36 showed builders deliver with 10–12 real bugs each and per-branch reviews miss
interactions; cost and time grow with round size [[feedback-review-cost]].

**How to apply:** plan round 37's contract with ≤ 3 builders, an e2e test in each brief's "done
when", and an integrated review after first deliveries.
