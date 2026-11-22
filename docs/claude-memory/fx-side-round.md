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

2026-11-18 (close): fx/final merged to master 1b049d6c (ff from 477cb6b4), pushed private; published to cantus-daw ef9d43c2; site deployed green (Tympanum card, Photographic Chorda/Tympanum, logo 03); owner asked for the release: v0.1.0-beta.15 tagged on cantus-daw. Windows 8 141 (8aaa71be), Linux 8 103 (e245bbe1); schema 77, keymap 19, DSP_VERSION 9. Next: cantus-ui tests leak ~6 GB into %TEMP% (cantus-record-mode-ui, cantus-freeze-ui) — fix; Caelum shimmer creep; Tabula: Fragor's 5 circuits as pedals (BACKLOG §3b); owner live-guitar test in beta-checklist.

2026-11-18 (late): beta.15's first tag failed (fmt os-error-206, then Linux tests); the local build had an input-lag bug (eframe patch painted after every event: 48 events/s) — fixed 98bcc3ff, owner confirmed. Owner then authorised: "Deja las escuchas pendientes y publica cuando acabes" — publish beta.15 (merge, push, publish.sh, move tag v0.1.0-beta.15) once fx/sounds + cantus-6f's r40/chorda-tune, r40/tools, r40/dancer-show, r40/lapis-default are green; listening approvals stay pending for after release. tools/perf/input_flood.ps1 measures UI input rate.
2026-11-18 (later, relayed by cantus-6f): owner — "Hay que esperar a que chorda esté bien para publicar": beta.15 does NOT ship without r40/chorda-tune (no known-issue fallback). Merge the other branches as they come; tag/publish only after Chorda's head is in and green.
2026-11-19 (release): everything merged — chorda-tune c2b79b61 (DSP 10), lapis-default ae3a3a1e (appearance.json v3), Arca plate labels 4.5:1 a0a2b6a8, Try it pictures in Lapis 97b7cf16 (render_tryit uses default_theme()/default_light_theme()), manual rebuilt 45734ad4 (web/build-manual.ps1 must run WITHOUT CARGO_TARGET_DIR — it reads target\manual and wipes web/assets/manual first), reshoot 88ef5a70 (563 pictures), notes fc5f379f. Windows 8 159 / Linux 8 117 green. Private master fc5f379f, public bfef258e, tag v0.1.0-beta.15 moved to bfef258e (release run 44522696511), site deployed from that push. Pending after release: owner's listening (Chorda before/after in F:\projects\daw-r40\compartir\chorda-tune\, effects in F:\projects\daw-fx-d\sounds-review\), AppImage PR #12206 `/retest`, cleanup (ci/release-fix, ci/beta15-final, b14src worktree, F:\projects\daw-fx-d builds).
2026-11-19 (evening): release run 44572307865 (tag on 378adacb) — Windows test job CANCELLED at its 120-min limit (not a failure: with the Chorda gate fixed the full suite runs >2 h; last seen in hosted_in_cantus's nested plugin build). Fix ea2abd50 raised release.yml limits (Windows 240, Linux/macOS 300). Private master ea2abd50, public 89bea52f (includes scene_vector fix), tag v0.1.0-beta.15 moved to 89bea52f; new run watched.
Pending after beta.15 publishes: publish r40/tymp-race (8adaf6b7, already in private master) to public; then merge cantus-6f's r40/release-lessons (now 5b221315 on f478c551; cantus-6f re-runs xtask first: docs/RELEASING.md, preflight job that requires docs/releases/v<ver>.preflight.md + a green rehearsal run; --no-fail-fast; CLAUDE.md edits) — NEVER before beta.15 is out or before any beta.15 re-tag.
2026-11-19 (night): run 44595352714 on 89bea52f — macOS green; Windows failed on the tympanum staging race (fixed 8adaf6b7), Linux on instruments_scale_e2e (process-wide theme race; fixed r40/scale-turns 7e053734, tests take turns). Owner chose to ship with a known issue (effect ids restart per project → an effect never sent after opening another song; in the notes, BACKLOG §1 first item). Private master 3a41a64f, public 91ce953e, tag moved there. Still pending after publish: merge r40/release-lessons 5b221315 (xtask green).
2026-11-20: run 44665357470 on 91ce953e — Windows + Linux green; macOS failed on session_faults5 (fault set by parameter not yet in the helper before ask_states). Fix: Rig::arrived in cantus-sandbox tests/support, used at 7 rig sites (2096a88f, a1f04c40). Private master a1f04c40, public 19fcc455, tag moved there. cantus-6f now on r41/effect-ids (keep r41/* out of master until beta.15 is published).
2026-11-20 (closed): beta.15 PUBLISHED 14:23Z for Windows + Linux only (CANTUS_RELEASE_MACOS set false, re-run failed jobs of run 44688188317, then set back to true). macOS held: Fragor preset.fragor.synth_fuzz really over budget (release 0.35–0.38 vs 0.25; other Fragor presets 0.05–0.15) — fix first in beta.16 with macOS. After release, merged to private master 9d504fa6: r41/effect-ids (ADR 0149), r41/chorda-strings, r40/release-lessons (preflight + rehearsal now required for every tag). Cleanup done: all my worktrees in C:\projects\daw-fx and F:\projects\daw-fx-d removed (branches kept; kept compartir, PLAN.md, refs, sounds-review, takes, content). C: 26 GB free, F: 167 GB. daw-r40/* and daw-r41/* belong to cantus-6f.
