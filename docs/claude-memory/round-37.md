---
name: round-37
description: "Round 37 content, instruments, look, scene, characters + first measured scorecard — closing as v0.1.0-beta.12 (2026-11-06); judge 9 Cantus 20/26; ticker 60 Hz fix; REAPER portable installed"
metadata:
  type: project
---

Round 37 started 2026-11-04 from docs/next-session.md (BACKLOG §0.4 item 7 + §0.1).
Worktrees F:\projects\daw-r37\{integration,piano,drums,library}, slot script
F:\projects\daw-r37\slot.sh. ADRs 0098 Chorda 2, 0099 Sectio, 0100 Pulsus III, 0101 library,
0102 scorecard; schemas 54 Chorda, 55 Sectio, 56 Pulsus III, 57 gain staging.

- Item 0 had nothing: no cantus-daw issues, macOS runs 7 329 passed, no FLAKY.
- Scorecard: rows in docs/scorecard/rows.md; research outputs in F:\projects\daw-r37\scorecard-research\;
  cross-DAW start-up harness tools/scorecard/startup.ps1. FL Studio 24.1.2 build 4394 installed
  (shows a "pirated version" banner; the owner 2026-11-04: use it anyway, their licence issue to
  check later — label results by version/build, never call it the trial). REAPER not installed.
- Owner asked (2026-11-04) to study FL's look, inspired never copied: research/ui/fl-look-2026-11/notes.md
  (screenshots local only, gitignored), BACKLOG §8 entry. See [[ui-quality-bar]].
- Chorda owner verdict: synthetic, fake, no body, thin [[decision-effects-sound-on-insert]].
- 2026-11-04 the owner: "mejoremos cantus donde le falte… todo a mano… superior a FL, sin sentirse copia".
  Fourth builder **finish** (ADR 0103, worktree daw-r37/finish, branch r37/finish) owns
  crates/cantus-ui/src/kit/** + drawing of mixer/arrangement/top bar/rack/piano keys. Acceptance:
  tools/look/compare_fl.py's 13 crops each win or tie (owner judges). Builds still capped at 3 slots.
- 2026-11-06: owner chose to build Chorda's piano scene THIS round (hand-drawn, live with preset/lid/mics/room; type+finish per preset, honest to the sound; schema bump if a look field is stored). Builder "scene" r37/scene in F:\projects\daw-r37\scene from 6f965af3. Show the owner the image sheet before calling it done.
- Fresh ABX kit 3 (integrated Chorda) at F:\projects\daw-r37\abx-kit3 — kit 2 is stale.

- Closing (2026-11-06): r37/integration with all seven builders merged (piano, drums, library,
  fixes, finish x10, scene, chars); schema 59, version 0.1.0-beta.12. Suites before the last
  fixes: Windows 6 524 pass / 95 ignored, fuzz 1 111, Linux 6 484 + 1 (rate-limit test fixed).
- Blind judges: 17 → 14 → 10 → **20/26** after finish's tenth (FL kept toolbar icons, mixer,
  browser; FL crops were softer). See [[ui-quality-bar]].
- Scorecard measured (docs/scorecard/README.md, ADR 0102): window 1.31 s vs FL 2.82 / REAPER
  1.94; Cantus loses quiet-after-window (6–42 s), memory (267 vs 133/62 MB), CPU at rest (29 %
  vs 6.2/0.3: engine on the demo + dancer 12 Hz full-window redraw). REAPER 7.85 portable eval
  at F:\projects\daw-r37\reaper (installed by me at the owner's ask, 2026-11-06).
- Found while measuring: egui subtracts predicted_dt from request_repaint_after, so the
  ticker's 17 ms was 0.3 ms (144 fps on the owner's 144 Hz screen); fixed in workspace_ticker
  finish(); four views (first-run setup, routing meters, VU, instrument editor) moved to the
  ticker. Measure Cantus with CANTUS_DATA_DIR seeded {"audio_setup":{"setup_done":true}}.
- Cantus packs' CC0 dedication is covered by ADR 0042 ("original Cantus content stays CC0").
- 2026-11-07: owner asked for 26/26; finish delivered 11–13 (insert panel, browser table with pack
  covers + sound card, song summary in the top bar, two-tone glyphs, layout-file fix). Two judges
  each: 20/18, 22/19, 20/22; toolbar icons + piano keys left for round 38 (owner chose to release
  after the 13th). r37/integration 1772bc72 + docs. See [[ui-quality-bar]].
- **Released 2026-11-08 07:58 UTC** as v0.1.0-beta.12 (latest; Win setup/portable/plug-ins,
  AppImage, macOS dmg). Private master 25b565a7, public cantus-daw master ae3e51b0; the tag was
  moved once (from ccd87c85, owner approved) after the Release's Linux job was cut at 120 min.
  Lessons: the Release Linux job now needs 180 min (suite 68 min + release + AppImage v2/v3);
  the Release's Windows test took 1 h 51 and package 1 h 23 — tag-to-download is ~3 h 15.
  CI flakes fixed: ipc_process slow-first (hold flag + kill on drop), macOS dense-install
  bound 3 ms in a virtual Mac, Chorda Unit::new bound 500 ms.
