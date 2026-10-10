---
name: round-24
description: "Round 24 (2026-10-09/10): gain staging, save off UI thread, linked clips (ADR 0037), perf wave 2 voice split (ADR 0038), soak journal fix; r24/integration uncommitted in F:\\projects\\daw-r23\\beta-hardening"
metadata:
  node_type: memory
  type: project
  originSessionId: d516c3e1-edb8-4b30-8d8c-1f743ad3dca2
  modified: 2026-10-10T14:37:26.551Z
---

Owner 2026-10-09: "commit and push round 23, give me a test build, then continue the
backlog". Round 23 pushed as master cead440; test build `F:\projects\daw-r23\builds\cantus.exe`.
The website session then committed 59ab765 + a9871e2 to master (ADR 0034 is the website's).

Round 24 = three builders (worktrees `F:\projects\daw-r24\{beta-rest,linked,perf2}`, base
cead440, uncommitted, targets deleted) in one workflow (build → 2 reviewers → fix; 29
findings all fixed), integrated as **`r24/integration`** in the worktree
`F:\projects\daw-r23\beta-hardening` on master a9871e2 — **uncommitted**, ask the owner.
- beta-rest: instrument calibration trim (ADR 0039, schema 21), save off UI thread
  (ADR 0040), shoot_recovery.
- linked: linked clips §6d.2 (ADR 0037, schema 22 after renumbering, keymap v4, cantus-clip/2).
- perf2: voices of Calor/Unda/Exemplar on pool threads, bit-exact (ADR 0038); demos worst
  window ≤ 32 % at 256 frames (blocker 2 asks < 70 %); the 10-min live device run not done.
- Soak on master (machine loaded) failed at 65 min: 18.5 s frame = crash journal fsync on
  the UI thread → journal writer thread (recovery.rs). Exposed a record-start race (stale
  "stopped" report ends a pass) — beta-rest's fix kept.
- Integration: 1 999 then 2 000 tests green, fuzz 396, clippy clean.
  Docs: docs/round24/integration.md; STATUS script ready at the scratchpad
  (edit_status24.py, takes the soak result as argv[2]) — applied after the soak.
- The 2-hour soak on r24/integration PASSED (quiet machine, started detached via
  PowerShell Start-Process so a session restart cannot kill it): no frame > 250 ms.
- The §6d.3 study (research/mixer-routing/notes.md, docs/round25/mixer-routing-design.md,
  11 owner questions) found an inexact undo: InsertStripSend added. Final: 2 000 tests green
  (201 binaries), fuzz 396, clippy clean; STATUS updated.

Next: §6d.3 mixer cabling (research first), §6d.4 white/blue theme + logo, the owner's
hand checks. See [[round-23]], [[machine-f-drive-build-quirks]].
