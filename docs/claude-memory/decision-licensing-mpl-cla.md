---
name: decision-licensing-mpl-cla
description: "Owner chose (2026-10-11) engine crates MPL-2.0, app GPL-3.0-or-later, CLA with free-forever promise, Cantus trademark policy (ADR 0047)"
metadata:
  node_type: memory
  type: project
  originSessionId: d516c3e1-edb8-4b30-8d8c-1f743ad3dca2
  modified: 2026-10-12T03:20:00.508Z
---

The owner wanted modifications to be shared with him mandatorily; told that no open
licence can require sending changes to the author, he accepted (2026-10-11): engine crates
(cantus-core/dsp/engine/project/rt) MPL-2.0 instead of MIT/Apache, the rest GPL-3.0-or-later,
a CLA (licence grant, not assignment) whose steward promises contributions always stay
available under their free licence (keeps a commercial licence possible, BACKLOG §6f), and
TRADEMARKS.md (forks under another name). Website footer/promise updated to "app GPL, engine
MPL". AGPL considered and deferred until Cantus offers network services.

**Why:** copyleft for the engine while keeping it embeddable; ability to grow a team and
relicense; values-compatible (no closing the community's work).

**How to apply:** never copy GPL code into the MPL crates; outside PRs need the CLA box
ticked; CLA.md is a draft — recommend legal review before the first outside merge. See
[[decision-cc-by-content]], [[round-25]].
