---
name: round-36
description: "Round 36 the professional studio — recording mode, VCA/snapshots/cues, external gear + Mackie/HUI, audio editor, ARA 2; schema 53; Win 6 291 + Linux 6 253; beta.11 with macOS preview"
metadata:
  node_type: memory
  type: project
  originSessionId: 2f8b0af2-408b-4947-b282-16b12b3313ac
  modified: 2026-11-04T01:28:05.814Z
---

Round 36 (2026-11-02 → 11-03), BACKLOG §0.4 item 6, contract 7393f1c2, worktrees in
F:\projects\daw-r36\{record,mix,external,editor,ara,showcase}, slot script F:\projects\daw-r36\slot.sh.

- record ADR 0092 (no schema), mix ADR 0093 schema 50, external ADRs 0094/0095 schema 51,
  editor ADR 0096 schema 52, ara ADR 0097 schema 53 (new GPL crate cantus-ara; SDK Apache-2.0 → go).
- Recording mode measured: 64 tracks 224–590 % live → 2–4 %, master baked 0.4–0.9 %, 0 underruns
  32–1024 frames at 64/128 streams. Spanish term in the app: "pre-renderizado" (not "horneado").
- Reviews caught: external instruments never sounding in the app (unit tests green), HUI tables
  wrong with a matching emulation, cross-feature bugs (cues × freeze × mode) only the integrated
  review saw → new rules [[feedback-work-standards-r36]].
- Owner decisions 2026-11-03: macOS ships as an unsigned **preview** in beta.11
  (CANTUS_RELEASE_MACOS=true on cantus-daw); showcase must sell instruments/effects
  [[feedback-showcase-sell]]; ARA slice 1+2 essentials this round, out-of-process later.
- Final checks: Windows 9acf6195 6 291 passed / 89 ignored, fuzz 1 085; Linux 6 253 / 89 (4188f15d),
  clippy+fmt clean on 9acf6195; null hashes 158 items identical. Main tree's full-debug target hit
  100 GB once — build with CARGO_PROFILE_*_DEBUG=0.
- Release: private master 651b4c31 (squash dd2330b5 + gitleaks allowlist + macOS clippy fix + CI
  timeout), cantus-daw 7c66e375 tagged v0.1.0-beta.11 (lightweight; moved twice, first from 1ac7dfad).
  Lessons: macOS-only code (`#[cfg(target_os="macos")]`) is checked only by the Mac runner — run
  `cargo clippy --target aarch64-apple-darwin -p cantus-rt -p cantus-ara -p cantus-engine` before
  tagging; Windows CI time grows each round (r35 101 min, r36 ~123) — ci.yml now 150 min; watch it.
- Owner-only: real interface buffers, hardware synth ping, Mackie/HUI hardware, Celemony letter,
  tools/ara/build-test-plugin.sh (session was denied fetching 3rd-party code), Melodyne trial, Mac.
- Next = docs/next-session.md: round 37 content & instruments + measured scorecard, ≤ 3 builders.
