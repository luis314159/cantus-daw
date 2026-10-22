---
name: feedback-review-cost
description: "Owner 2026-10-20: stop running reviews with many agents (a 77-agent review was very expensive in tokens); keep each review workflow small (~6 agents: few lenses, batch verification)"
metadata:
  node_type: memory
  type: feedback
  originSessionId: 986410ca-b4ef-4425-9ddc-1112fd5eda06
  modified: 2026-10-20T21:02:08.114Z
---

The owner, round 29 (2026-10-20): "Ya no corras tareas con tantos agentes (como la de 77
agentes, fue muy caro en tokens)".

**Why:** the per-finding refuter pattern (7 lenses + one verifier per finding) spawned
55–139 agents per review; the token cost was too high.

**How to apply:** a review workflow has at most ~6 agents: 3 finders over the delta with
merged lenses (correctness + real time; reachability + tests; interface + words), each
returning only concrete findings with file:line evidence, then ONE batch verifier per finder
that tries to refute all of its findings in a single pass. No per-finding agents. Prefer
reviewing only the fix round's delta and regressions. See [[cantus-parallel-builders]],
[[round-29]].
