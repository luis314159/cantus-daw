---
name: round-28
description: "Round 28 (2026-10-14/16): top bar §6e, Tonus + pitch editor, 12 effects, modulators + comping, Chorda + DRSKit, demo song; 7 builders + demo + fix2..fix7; 20 reviews / 454 findings; Win 3 362 + Linux 3 345 green; schema 32, DSP_VERSION 5, keymap 8; pushed to master with the owner's advance permission"
metadata:
  node_type: memory
  type: project
  originSessionId: 0327286f-1cd4-445a-b41e-15821abb34e4
  modified: 2026-10-16T07:50:37.342Z
---

Requested 2026-10-14 (the owner's five items: top bar §6e; voice §3c/§3d; effects §3b;
CC BY drum kit + Chorda §3e/§5; comping + modulators §0.2). Base master 445a44c; contract
a327c53 + BUILDING.md ca5b48e; integration branch r28/integration (worktree
`F:\projects\daw-r28\integration`); final code 1723d2a, then docs/STATUS.

Builders: voice (Tonus, ADR 0053), modfx (0055), topbar (0052), pitch (0054), dynfx (0056),
leaders (modulators 0057, comping 0058), sound (DRSKit amends 0042; Chorda 0059), demo
(*With All Your Heart*, the owner's mid-round ask), fix2…fix7 (review rounds). Schema chain:
25 contract, 26 topbar, 27 pitch, 28 modulators, 29 comping, 30 Exemplar rr order, 31 pitch
`corrected`, 32 `Clip::comp`. DSP_VERSION 5 (Aequalis glide). Keymap 8.

Verified (2026-10-16, 1723d2a): Windows 3 362 passed / 0 failed (294 binaries), engine fuzz
689, clippy + fmt + notices clean; Linux (WSL) 3 345 passed / 0 failed, clippy clean.
Reviews: 20 Workflow reviews, 454 confirmed findings (21 high), all closed. Test build:
`F:\projects\daw-r28\builds\`. Demo renders: `F:\projects\daw-r28\shots\demo\` (−11.3 LUFS,
−1.14 dBTP). The owner gave permission in advance to push when green; history scanned clean
(only the owner's own gmail as author; leaders' accidental .cargo/config.toml had only build
paths, removed in 068aa98) → pushed as a fast-forward.

Owner-only open items: listening (Tonus/pitch on a real voice, effects, Chorda blind test +
legal review, DRSKit, comping/modulators, top bar look, the demo), whether modulation
effects should sound on insert (today Mix/Depth 0 = null), keep/delete
`F:\projects\daw-r28\downloads` (DRSKit zip for real-kit tests). The MCP wishlist from the
demo builder is in BACKLOG §7.

Lessons: see [[cantus-parallel-builders]] (fresh agent past a safeguard false positive;
shared target dir for fix branches; check `git ls-tree` for committed .cargo/; review every
fix round — regressions hide there). C: was ~99 % full: TMP forced to F: via `[env] TMP =
{ value = '…', force = true }` in each worktree's config.

See [[round-27]], [[feedback-no-leftovers]], [[gotcha-boot-path]].
