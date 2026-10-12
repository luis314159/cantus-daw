---
name: round-25
description: "Round 25 (2026-10-10): mixer cabling §6d.3 (ADRs 0041, 0043-0045, schema 23, keymap v6, click mode), theme family + Cantus Night default + recolourable taskbar mark (ADR 0046), CC BY (ADR 0042); r25/integration in F:\\projects\\daw-r23\\beta-hardening"
metadata:
  node_type: memory
  type: project
  originSessionId: d516c3e1-edb8-4b30-8d8c-1f743ad3dca2
  modified: 2026-10-10T23:33:38.377Z
---

Round 24 pushed as master a89edf4 (owner: "Si, has push"). Round 25 = §6d.3 mixer cabling
with the owner's 12 answers (docs/round25/mixer-routing-design.md §10) + mid-round request
for more themes, a website-matching default, the website favicon as taskbar icon with a
user-changeable colour, in-app logo still configurable.

- Branch **r25/integration** in worktree `F:\projects\daw-r23\beta-hardening`: contract
  57f3ccc, merges of r25/{interaction,matrix,desk} (worktrees `F:\projects\daw-r25\`),
  Delete-on-cable fix 1226d3b, fixer's 30 commits to 11dd5f7 (27 review findings + click
  mode, owner answer 2, which the first build had missed), then merge of r25/look.
- ADRs: 0041 routing, 0042 CC BY content, 0043 cable band, 0044 desk gestures (§10 click
  mode), 0045 pre-fader engine + matrix, 0046 themes + mark. Schema 23 (Send::tap). Keymap v6.
- Look: 12 themes (Cantus Night default, Cantus Paper, Daylight = §6d.4 white/blue,
  Blue Hour, Ultraviolet, Lagoon, High Contrast ×2, + 4 old); taskbar icon recoloured live
  via WM_SETICON (cantus-launch/src/taskicon.rs). Pinned-button behaviour unchecked.
- Pictures: `F:\projects\daw-r23\shots-r25\{final,look}`.
- Pending owner: pan-law centre gain 0.99999994 — (a) leave (recommended) or (b) fix with
  DSP_VERSION bump; permission to push; hand checks (STATUS "The next session").
- Owner's push rule: commit/push only when asked; ask after the test build.

See [[round-24]], [[decision-cc-by-content]], [[ui-quality-bar]], [[machine-f-drive-build-quirks]].
