---
name: round-30
description: Round 30 (2026-10-22) — three systems; integrated, 0.1.0-beta.4; Win 4 469 + Linux 4 451 green; 20 review findings fixed; macOS proven only on cantus-daw's runner; ran lean (owner short on tokens)
metadata:
  type: project
---

Round 30 = Cantus on Windows, Linux and macOS (see [[decision-round30-platforms]]).
Contract `8153666` on `r30/integration` (worktree F:\projects\daw-r30\integration),
`docs/round30/BUILDING.md`. Builders: linux (ADR 0065), macos (0066, also
docs/mac-session.md in Spanish), ship (0067: release.yml for 3 OSes, updater per OS, website
cards). Packaging interface: packaging/linux/build-appimage.sh, packaging/macos/build-dmg.sh.

**Why lean:** the owner said 2026-10-22 they have few tokens left — few agents, one small
reviewer per branch (not 6), no integrator maps.

**How to apply:** macOS can only be proven on cantus-daw's runner (no gh CLI on this
machine; use the GitHub REST API by curl to read runs). Publish when Windows+Linux are green,
then fix macOS-only failures, tag only when the three are green. See [[public-repo-cantus-daw]].

2026-10-22: the round-29 session released **v0.1.0-beta.2 (Windows only)** from master (scroll fix). Round 30's three-system release must be **0.1.0-beta.4** (owner 2026-10-22: the other session releases beta.3 with the updater TLS fix); merge master into r30/integration before the release and fix DEVELOPING.md's example version.

**Result (2026-10-22):** r30/integration has linux, macos, ship (+ the owner's 3 frictions,
assigned to ship) and master merged; 4 reviews (1 agent each) → 20 findings fixed. Version
0.1.0-beta.4. Windows 4 469 + fuzz 851; Linux (WSL) 4 451. Flaky under load: session_faults5
(fixed with a dialog marker) and bridge's hang test (late_blocks==1; 8/8 alone, left as is).
Lessons: master's tools/wsl/check.sh lacked libclang settings — use the branch's copy;
deleting ~/cantus-ci/target under a running check kills it; C: (WSL vhdx) fell to 2.8 GB.
Web hero-meta still says "Windows 11" — change it after the three-system release exists.
