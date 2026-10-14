---
name: gotcha-boot-path
description: The desktop app never calls CantusApp::new — real start-up is cantus_ui::run → boot::Boot stages → CantusApp::assemble; wiring only in ::new is dead in the shipped app
metadata:
  node_type: memory
  type: project
  originSessionId: 03130613-fa0b-4e4e-ac84-674beeca975e
  modified: 2026-10-14T11:49:23.940Z
---

Round 26 hit this twice (2026-10-14): the release builder's font fallback and the sampler
builder's cache folder / cache limit / start sweep / load-beside-window were wired only in
`CantusApp::new`, which tests use but the shipped app never calls. The real path is
`cantus_ui::run` → `boot::Boot` (stages: fonts, settings, engine…) → `CantusApp::assemble`.

**Why:** tests passed while the feature was dead in the real app; only an adversarial review
reading the start-up path caught it.

**How to apply:** in every brief that adds start-up wiring, tell builders to wire it in the
boot stages / `assemble` and to add a test that fails if it leaves that path (the font
fallback's `the_boot_font_stage_arms_the_fallback` is the model). Reviewers: always ask
"is this reachable from `cantus_ui::run`?". See [[cantus-workflow-habits]].
