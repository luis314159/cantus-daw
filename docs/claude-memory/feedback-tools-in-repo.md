---
name: feedback-tools-in-repo
description: "Owner 2026-11-20 — every tool made in a session (test runners, A/B checks, release helpers, video pipeline) goes into the repository for future iterations, not the scratchpad"
metadata:
  node_type: memory
  type: feedback
  originSessionId: c330c055-9d87-4a1d-9df8-77cfd8e6440e
  modified: 2026-11-20T19:29:24.247Z
---

The owner (2026-11-20): *"Deja las herramientas hechas en esta sesión para todo lo que hagas
tanto pruebas como para el video en el repo para futuras iteraciones."*

**Why:** the next session (or round) should rerun the same checks without rebuilding them;
scratchpad scripts are lost.
**How to apply:** write helper scripts straight into `tools/` (tools/test, tools/release,
tools/video …) with a header comment on usage, name them in docs/DEVELOPING.md §4, commit them
(explicit paths). Only generated outputs stay outside the repo. Tell subagents the same.
Kept so far: tools/test/suite-windows.sh, ab-compare.sh, disk-watch.sh,
tools/release/release-body.ps1 (branch b16/fragor-cpu, 8431b840). See [[cantus-workflow-habits]].

2026-11-21, again: **every script used to make the video, directly or indirectly**, goes in the repo (tools/video/<area>/ + an end-to-end tools/video/README.md), and the session's own release/merge/WSL tools too (tools/release/gh-api.ps1, watch-run.ps1, tools/merge/keep_both.py, tools/test/timing_tests.py, tools/wsl/guarded.sh — commit 2ce6d2013). A one-off inline check becomes a kept script when it will be needed again.
