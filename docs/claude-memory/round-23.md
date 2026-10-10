---
name: round-23
description: "Round 23 (2026-10-09): beta blockers only — export dialog audit and fixes, theme text bug, CPU tests robust, Ctrl+C/X/V; committed and pushed as master cead440 (2026-10-09)"
metadata:
  node_type: memory
  type: project
  originSessionId: d516c3e1-edb8-4b30-8d8c-1f743ad3dca2
  modified: 2026-10-09T23:45:26.862Z
---

Asked 2026-10-09 (owner, in Spanish): round 23 = only the beta blockers, in order —
(1) every option of the export dialog audited, a test per option, the bar wired to
`render_with_progress`, the dialog in the shoot examples; (2) text that looks strange
after a theme change: reproduce with a pixel test (round trip of themes), fix; (3) the two
CPU tests robust to load; (4) Ctrl+C/X/V in the piano roll and arrangement as Commands
with undo-fuzz arms, strings en/es. Rules: own worktree `r23/beta-hardening`, touch only
crates/ and docs/ (another session builds the landing page in web/), **no commit unless
asked**, tests first, clippy clean, STATUS + claude-memory at the end.

Done (worktree `F:\projects\daw-r23\beta-hardening`, base master 2ad043b, nothing committed):
- Export: 27 audit findings, all confirmed by skeptics. The big one: every list in the
  dialog opened *under* it (it moves itself to the top each frame) — fixed with
  `set_sublayer`. Also: a selection's tail played the next bars (now Stop at the range
  end), latent projects made the mix late/cut (`RenderOptions::compensate_latency`, on for
  exports), stems ignored hosted plug-ins, stems re-export deleted old stems, FLAC failed
  at 192 kHz (now 96 max), keys acted behind the dialog, Loop range added. ADR 0035.
- Theme text: kit's galley cache kept text laid out against egui's old font atlas after a
  dark↔light switch; now watches text options + font faces. Test `theme_roundtrip.rs`.
- CPU tests: best of five windows (bass, Calor, sampler), stress frames use the fastest,
  hot-plug bounds 150 ms.
- Clipboard: built by a workflow builder in `F:\projects\daw-r23\clipboard` (branch
  r23/clipboard, uncommitted), ADR 0036, reviewed (8 findings fixed), then its diff
  applied to r23/beta-hardening (clean). The final branch was reviewed again (13 of 14
  findings real, all fixed, e.g. the stop frame now uses `transport::sample_at`).
- Close: 1 921 tests green (194 binaries, 4 run renamed for UAC), clippy clean, fuzz at
  CANTUS_WORKERS=3 green; STATUS.md updated; BACKLOG left alone (owner's uncommitted edits
  in the main tree). Committed and pushed at the owner's request: master cead440.

Owner to-dos left: try the build with real drivers; the manual clipboard checks (Ctrl+C,
restart, Ctrl+V; Notepad text; two windows). See [[round-22]], [[machine-f-drive-build-quirks]].
