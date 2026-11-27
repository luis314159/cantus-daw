---
name: round-42
description: "Round 42 (started 2026-11-22, session cantus-57) — beta.17 candidate on cantus-daw branch rc/0.1.0-beta.17; microtuning (ADRs 0155-0156, schema 80) and Arcus the bowed string + note articulation (ADRs 0157-0158, schema 81); builders tuning, tuning-face, arcus; strings-rec is wave 2"
metadata:
  node_type: memory
  type: project
  originSessionId: 46310d93-8b9d-4995-a1a6-07dcc8fb61ce
  modified: 2026-11-22T23:47:49.620Z
---

Session cantus-57, 2026-11-22. Worktrees in F:\projects\daw-r42\ (research/, logs/, listen/, downloads/).

- Windows suite on master 7eb64c6ec: 8 318 pass, 0 fail (167 ignored). WSL suite on master
  e6e5523b4 started after it (guarded.sh, log F:\projects\daw-r42\logs\wsl-test.log).
- **beta.17**: b17/release (c1ef7c9dc version + notes; a7feaae5f publish.sh PUBLISH_BRANCH=rc/<v>)
  fast-forwarded into master; e6e5523b4 site budget calibrated (lhci_calibrated.mjs: CPU
  slowdown 4×benchmarkIndex/3400 — the Site budget failure 44982501210 was a slow runner,
  index 2786 vs 3466, LCP 2106 vs 1884). Candidate published as **branch rc/0.1.0-beta.17**
  on cantus-daw (public a62e9cfb) so the site (deploys on master push) does not show Fides/Nitor
  before the release; master is published after the release is out. Site budget dispatched on
  rc (run 45084840767); then the rehearsal (release.yml workflow_dispatch on the rc branch),
  preflight, tag with the owner's OK.
- **Round 42 contract** r42/contract 3cd82f6f5 (+ master merge c28fe171a): tuning types,
  SetProjectTuning/SetTrackTuning/SetNoteArt + fuzz, schema 80/81, BowedString stub (not in
  ALL), set_tuning/note_art hooks, ExternalTuning seam; docs/round42/BUILDING.md, briefs.md,
  pitch-map.md; research verified (microtuning-verified.md, strings-verified.md).
- Decided with the owner's mandate: name **Arcus**; the effect (multiband delay) moved to
  round 43; cello + double bass first, violin/viola only if they pass the gate.
- Builders launched 2026-11-22 (agents): tuning, tuning-face (design sheet → owner first;
  MTS-ESP in cantus-rt; picker warm-up leftover), arcus (design sheet + renders → owner;
  Fides bow pitch leftover). strings-rec (wave 2) not launched yet.
- Build slots held during the WSL suite by 3 slot.sh holders waiting on
  F:\projects\daw-r42\HOLD-BUILDS — **delete that file when the WSL suite ends** to free them.
- Social kit (branch social/kit 6d6e8d0e6, worktree daw-r42/social): images in
  F:\projects\video-work\social\, docs/social/{accounts,youtube-launch,setup}.md; owner creates
  the accounts (@cantusdaw, YouTube brand account) and decides bio language, Mastodon server,
  Bluesky handle, channel verification. Merge social/kit into master with the round.

See [[round-41]], [[decision-owner-delegates-excellence]], [[cantus-parallel-builders]].

- **Site LCP** (2026-11-23): cause was the network, not CPU — the home's HTML crossed TCP slow
  start's two round trips (43 737 → 43 822 B gzip = +150 ms) and the picture shared the line
  with fonts/JS/logos. Branch site/lcp (worktree daw-r42/site-lcp, 597f81e90 minified inline CSS
  + JS after paint + Light face late + data: logo; 165c12f27 new gate: home HTML ≤ 42 000 B gzip,
  accepted; 940252660 tools lcp_graph.mjs, same_look.mjs): LCP 2.11 → 1.66 s, same look proven.
  Merge into master AFTER beta.17 is released (not in the rehearsed code). Pre-existing: hero CLS
  0.135 in 1 of 3 throttled runs → BACKLOG. Delete F:\projects\daw-r42\site-lcp-base when done.
- C: nearly full: WSL vhdx 135 GB (25 GB used inside after deleting ~/cantus-ci/target); owner to
  run tools/wsl/compact.ps1 as admin (on r42/contract 0924e082b) before the WSL suite.
- Rehearsal of beta.17: run 45086843755 on rc c6c00660 (watch output F:\projects\daw-r42\logs\rehearsal-b17.txt).
- 2026-11-23: C: hit 281 MB; owner ran compact.ps1 twice (2nd after fstrim as root): WSL image 135 → 27.4 GB, C: 114 GB free; HOLD-ONE removed (three build slots again).
  temp (→ 4.9 GB). Builders now export TMP/TEMP to F:\projects\daw-r42\tmp\<name>. One build
  slot held (file F:\projects\daw-r42\HOLD-ONE → delete it to go back to three) until the owner
  runs tools/wsl/compact.ps1. Arcus picture moved to a design agent (branch r42/arcus-look,
  worktree daw-r42/arcus-look): 3D cello/bass on Chorda's scene engine (owner: "same engine",
  v2 sheet "muy feo").
- 2026-11-23: owner OK "Sí vamos con beta 17" → tag v0.1.0-beta.17 pushed on public d9c7ee33 (rc
  branch, preflight commit; rehearsal 45086843755 green ×3); release run 45314224993. After it
  publishes: publish master (fast-forward from rc) and merge site/lcp. F: hit 0 bytes (main tree
  target ~50 GB from the Windows suite): deleted target/debug → 52 GB free. Agents hit the weekly
  API limit once and were resumed ("Reanuda todo").
- 2026-11-24: **beta.17 PUBLISHED** (release run 45314224993 green; latest). Public master
  d9c7ee33 then site/lcp merged (private 290b82256) → public 37561254: Site deployed, Site
  budget green. Supply chain audit red since 2026-11-20 (url RUSTSEC-2026-0592) → fixed on
  r42/integration 71d22f037 (url 2.5.9). Asset sizes same as beta.16 (Windows setup 399 MB vs
  dmg 78 MB — BACKLOG). Private origin not pushed yet.
- Round 42 state: integration = contract + tuning + tuning-face + fix-engine + fix-ui +
  strings-rec + url fix (tests: UI 661, dsp/core/project/content/engine 4 189, 1 flake
  inst_stream child under load → fix L3). Arcus (r42/arcus) pass 2 running on the study
  (F:\projects\daw-r42\study\: refdb + bowed-realism.md, 13 sources); arcus-look done (v4
  approved); arcus-upper (violin ×4, viola ×2) waits for owner's pose A/B. ABX page:
  F:\projects\daw-r42\listen\abx-ciego\abx.html (tools/listen/abx.html).
- 2026-11-24 later: private origin pushed (master 290b82256). Owner chose violin/viola **pose B**
  (playing angle, with bow). Arcus pass 4 merged; arcus-upper (violin/viola, pass 4) merged;
  social/kit + study tools (tools/arcus/study, research/instruments-next/bowed-refdb) +
  inst_stream L3 fix + inst_keys ellipsis rule in integration (1da2e8df6). ABX ref bug fixed:
  "grabación" had been the old Arcus render; now strings-rec renders. Mid-round review (3
  finders) → 26 findings; fix waves r42/fix2-arcus + r42/fix2-model (worktrees F:\projects\
  daw-r42\fix2-*), gated on F:\projects\daw-r42\tmp\SUITE-DONE (create it when the Windows
  suite, log F:\projects\daw-r42\logs\suite-win-r42-mid.txt, finishes). `cargo fmt --all` fails
  on Windows (os error 206): run rustfmt per changed file.
- 2026-11-25: merged into r42/integration: fix2-model, fix2-arcus, fix3-suite, tymp-abx, fix4-narrow,
  arcus-demo, **arcus-p5** eee97dd86 (vibrato from 1 976 measured notes, DSP_VERSION 13, preset
  parity <1 LU), **arcus-upper** e3396db94 (holders Woman/Man realistic, left-hand table, Figure
  detail High/Light/Auto, ADR 0159), **arcus-hair** f42f88109 (hair maps + 3×3/2×2 samples on the
  app's bake path, CC0 eyes, sheet v9). Owner shown every preview; open: temple band, flyaways,
  live-hand texture. Full Windows suite: use tools/test/suite-windows.sh (565 test exes ≈ 56 GB
  never fit F: at once — a plain `cargo test --workspace` filled the disk twice). Then WSL, STATUS,
  next-session. Arcus into ALL waits for owner ABX verdicts.
