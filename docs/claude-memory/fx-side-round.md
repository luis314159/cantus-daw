---
name: fx-side-round
description: "Second parallel session (2026-11-13) building new effects beside round 40 — Cribrum, Fragor, Lapsus, Vetus, Libra in C:\\projects\\daw-fx, merged only after r40 closes"
metadata:
  node_type: memory
  type: project
  originSessionId: 02467d0a-aeb1-4d1b-bb55-92c52af32cb9
  modified: 2026-11-15T00:30:47.266Z
---

Owner opened a second session (2026-11-13) beside the main round-40 session to add effects
and low-conflict backlog items. He chose all four proposals: **Cribrum** (auto filter,
`AutoFilter`), **Fragor** (distortion/fuzz pedals, `Pedal`), **Lapsus** (Gross Beat-class
time shaper, `TimeShaper`), **Vetus** (lo-fi, `LoFi`) + **Libra** (utility, `Utility`).
Mid-turn he added: **every effect a panel with its own personality, graphics polished and
professional like FL Studio and others** — a design sheet he approves before the full editor.
Later the same day he added **wave 3: a pedalboard** (Tabula, proposed) hosting any of our
units as pedals, add/remove/reorder graphically, **with amp + cabinet (IR) inside**; drawn in
2D with the kit on Fragor's reusable enclosure component, not r40's scene engine (ADR 0126).
Pedals reuse existing units' engines; every NEW engine wave 3 makes (pitch/octaver/whammy,
rotary, amp+cab) is also a standalone Cantus unit with its own face and export (owner).
After the units: interactive "try it" demos on the website (CC0 DI guitar — Carbonthief, in
C:\projects\daw-fx\refs\audio — + presets + the editor's picture per preset, pre-rendered);
in-browser live DSP (WASM AudioWorklet) noted for BACKLOG §6a. A side web agent also fixed
light-mode contrast + animations on branch side/web (owner decides deploy).
Status 2026-11-13 evening: Cribrum and Fragor done and merged into fx/integration (3b6f2eba,
5 785 tests green, clippy clean); Lapsus in Phase B (separate time/volume banks); Vetus+Libra
started from 3b6f2eba; side/web has contrast fixes + motion v3 (scroll story), served locally
from C:\projects\daw-fx\web\web\dist. Integrator list: docs/round-fx/integration-todo.md.
2026-11-14: all five done and merged (fx/integration 60cc1be6); a close-out agent runs the
leftovers (bench, MCP text, README, colourways held to the house rule, site pictures, full
suite, docs/round-fx/integration.md). Next proposed: wave 3 (pedalboard + pitch/rotary/amp)
and the website's interactive demos — owner to choose.
Wave 2 closed (fx/integration d80a2444: Win 5 909 + Linux 6 949 + fuzz 1 208 green; Fragor
colourways Valve/Cream/Steel; Petrol/Walnut nudged). Wave 3 started 2026-11-14: names Tabula
(board), Saltus (pitch), Rota (rotary), Arca (amp+cab); Tabula starts as a guitar rig;
architecture in C:\projects\daw-fx\tabula-design.md; contract agent running (schema 63,
ADRs 0134–0137 provisional). Disk: C: ~51 GB free, WSL vhdx 100 GB (compaction needs owner/
UAC), F: 96 % (r40's).
2026-11-14 night: owner said "vamos a hacer todo" — the whole plan is in
C:\projects\daw-fx\PLAN.md (waves 3, A vocoder/glitch/shimmer, B bus comp/resonance/ducker,
C reference comparator, E repair, D controllers after r40, web demos); full lane vs DSP lane;
≤ ~4 agents (API weekly limit hit once, reset Nov 16 22:00).
Owner prefs this round: share each result as it lands (pictures, numbers, what to hear); dark displays in both themes; fine strokes; colourways selectable per
unit (Cribrum first; mechanism reusable).

2026-11-16: wave 3 done in fx/integration (db58934a): Tabula, Saltus (+ app-wide user
starting preset, ADR 0138), Arca + Rota (+ Phasis lamp vibe). DSP done for Loquax, Fractus,
Caelum, Vinculum, Lenis, Purus, Cedo, Comparatio. Wave A Phase B runs in one worktree
fx/wave-a (the 30 GB target moves between worktrees — C: only fits one full-lane build;
WSL vhdx compaction script compartir\compactar-wsl.ps1 needs owner UAC). Owner approvals
and sounds live in compartir\<unit>\{design,sound,verdict.md}; status log in PLAN.md.

Owner 2026-11-17: when every pending item of the side round is done, merge everything into
master and publish the new website (authorised: merge + private push + site deploy; NOT the
public repo or a release tag — ask). Names approved as proposed; the site must show every new
effect like the others (cards, pictures, detail pages, excerpts) before publishing. Close-out order in C:\projects\daw-fx\PLAN.md.

**Why:** grow the effect family (BACKLOG §3b) without disturbing round 40 (schema 62–70,
ADRs up to 0126, 22 worktrees on F:, which is at 91 %).

**How to apply:** worktrees in `C:\projects\daw-fx\` (integration = `fx/integration` from
master dd4ad68b), branches `fx/*`; schema 62 and ADRs 0130–0133 are provisional, renumbered
when merged after r40 lands on master (as round 41 material); max 2 cargo builders (RAM shared
with r40); wave 1 Cribrum + Fragor, wave 2 Lapsus + Vetus/Libra; never touch F:\projects\cantus
or daw-r40. Docs in `docs/round-fx/`. See [[round-40]], [[cantus-parallel-builders]],
[[feedback-review-cost]].

2026-11-17 later: 17/19 effects done (Purus, Cedo, wave D Tactus/Calculus done; Comparatio step 3,
Vinculum opto excellence, Caelum Sustain running). Issue #1 follow-up (ozersomer): menus at 200 %
fixed on fix/flicker + one-step "route the selected to a new bus" (Ctrl+G, ADR 0150 prov., keymap 18)
being built. Owner asked for multi-monitor support: fx/multimon (ADR 0151 prov., keymap 19), owner
answered the 6 questions (dock back, window bar on, layouts my choice, patched eframe in third_party,
Alt+1..9/F11, per-window scale), emphasis: looks excellent on every screen + stable. Owner approved
integrating everything else in parallel on fx/final (C:\projects\daw-fx\final, from master 477cb6b4)
while multimon builds; multimon lands last, then master + private push + site.
Owner 2026-11-17 (supersedes the order above): publish everything except multimon as soon as it is integrated and tested (master + private push + site); multimon is a second, later merge.
Owner 2026-11-17 (later): multimon joins the same single publication ('todo junto').
