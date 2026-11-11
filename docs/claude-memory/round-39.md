---
name: round-39
description: "Round 39 (2026-11-08/11) — UX and graphics: Slate the default (ADR 0111), the kit (0108), icon family + 316 track icons, logo #03, Slate Lapis redone distinct, 24 themes, Delay's own window (0115), every view redrawn, ADRs 0109/0110/0113/0114, Aula (0112), memory 237→140 MB private quiet (0107, still lost); closed as v0.1.0-beta.14"
metadata:
  node_type: memory
  type: project
  originSessionId: c2c8879f-7be8-412c-abd3-92a9c35caa70
  modified: 2026-11-11T08:27:01.512Z
---

Round 39 = all UX/graphics (see [[decision-round39-ux-graphics]], [[decision-slate-default]], [[feedback-more-agents-r39]], [[feedback-themes-legible]]). Worktrees under F:\projects\daw-r39\; integration branch r39/integration; website branches r39/web-content, r39/web-struct (merges all web work), r39/web-extra.

Close-out checklist (owner, 2026-11-09), all done 2026-11-11:
- **Reshoot every interface picture on the website and in the README** with the new look, after the last visual merge. The owner flagged the Delay picture; Delay then got its own window (ADR 0115, owner OK'd) and was reshot. web/assets/dance/ is unused (no dancers on the site).
- Website: orange accents became #03 gold; section bands distinct (web-struct 007aa555).
- **Memory claim:** the owner thought we already beat REAPER/FL on RAM. What shrank is the live heap (engine 95.8→8 MB); whole-process private bytes went 237 → 140 MB. Still lost. Never claim WON without a measurement.
- Owner re-confirmed 2026-11-10 21:30: once the new screenshots are in, publish and deploy (push master, publish.sh, tag v0.1.0-beta.14 → Actions release builds + site deploy), only when Win+Linux are green.

**Why:** the owner judges by whole windows and by what the site shows; stale pictures or numbers undercut the round.
**How to apply:** reshoot only after the last visual merge; keep pictures out of builders' branches until then.

**The round's facts (`docs/round39/integration.md`):**
- Phases: A = dir-a/b/c directions + icons + ux-a, ux-b, memory; B = owner chose Slate, approved des-* sheets; builders kit, el-topbar, el-controls, el-mixer, el-cables, el-views, el-arrange; beside them conv (Aula), fixes, logo, lapis, stack, icons-*, web*; close-out lo-arrange, lo-mixer, lo-rest, shots, docs; late: delay, lapis2, themes, fix2.
- Schema 61, keymap 16, DSP_VERSION 6. ADRs 0107–0115. Icons: set 1 rejected, set 2 approved, sets 3–5 in use pending the owner's review.
- **24 themes**: Slate, Slate Lapis (lapis selection, gold readouts, night-blue room; tokens text.readout(_secondary), accent.selection_tint, editor.loop_edge), Graphite and gold, Studio ink, high contrast; the ten older themes back redrawn (FL-derived ones renamed Pewter and Chalk) + Evergreen Night/Paper, Bordeaux, Print. Every theme measured: non-text 3:1, text 4.5:1 (arrangement_legibility.rs, incl. desk stripes).
- fix2: icon picker's click away made strips (fixed); Project::strip_is_free; mixer folds only what does not fit, overflow slot sized to its count.
- Final: integration 67d9c38e — Win 6 781/0/106, engine fuzz 1 145, Linux 6 742/0/106, clippy + fmt clean. Quiet scorecard 2026-11-11 (release 81f88171): empty 0.76 s / 1.5 s / 140 MB / 1.1 %, Beat 0.77 / 1.5 / 143 / 2.0 % vs REAPER 1.76 / 2.0 / 62 and FL 2.49 / 5.8 / 133. Live heap ~25 MB; map the rest with memmap.ps1 next.
- Frame budget: whole 1080p window no more than beta.13; 64-strip window 96 584 → 46 855 triangles; idle_repaint_e2e holds nothing asked at rest.
- Twenty-two open owner decisions in integration.md (right dock, meter in Level, maximise, 1/2 vs ½ bar, Follow at start, browser kinds, pad's own mixer track, section-drag ripple, el-controls' five, icon sets 3–5, wooden faces, Lapis gold playhead + tint strength, theme strengths/Print, vintage round, dancers, Delay tempo sync + plug-in export, legal reading of Slate vs FL). Vintage "oscilloscope" mode idea is in BACKLOG (proposed round 41).
- Local build F:\projects\daw-r39\builds\beta14-81f88171; round-40 prompt docs/next-session.md.
