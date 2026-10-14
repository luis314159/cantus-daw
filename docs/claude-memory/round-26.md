---
name: round-26
description: "Round 26 (2026-10-13/14): beta ready minus owner's hardware/paperwork — public-repo prep in a copy, cheaper CI, SignPath drafts, release notes + README pictures, r25 small things, CC BY + Salamander, disk streaming (ADR 0050), link picker/clip browser/paint tool, one kit text field; r26/integration in F:\\projects\\daw-r26\\integration, verified green Windows + Linux"
metadata:
  node_type: memory
  type: project
  originSessionId: 03130613-fa0b-4e4e-ac84-674beeca975e
  modified: 2026-10-14T20:00:38.116Z
---

Started 2026-10-13 from `docs/next-session.md`. Base master 78c8ff5, contract a3b6355.
Six builders (worktrees `F:\projects\daw-r26\<name>`, branches `r26/<name>`): public,
release, mixer, content, linked, and sampler (opened mid-round for Exemplar disk
streaming + pedal noises). Every branch got 1–5 adversarial reviews (lenses + a refuter
per finding) via Workflow; all confirmed findings fixed with a test seen failing.

Final: `r26/integration` (code at e8e0881, then docs/STATUS). Verified 2026-10-14:
Windows 2 462 tests (242 binaries) + fuzz 465 + fork 5, clippy clean; Linux (WSL,
check.sh) clippy clean, 2 448 tests + fuzz 470. Test build (xtask package, ASIO) in
`F:\projects\daw-r26\builds\` (setup, stable-name copy, portable, plugins, SHA256SUMS;
portable reports WASAPI, ASIO). Account: `docs/round26/integration.md`.

New ADRs 0048 (CI budget, stable installer name), 0049 (going public), 0050 (streaming).
Schema 24 (controller-triggered zones), keymap 7 (Alt+N Link to…, Alt+F5 clip browser).

**Squash rule:** r26/public's first commits wrote the owner's work e-mail and a work-hours
analysis into docs (removed since) — **merge round 26 to master as ONE squashed commit**.
Private details kept outside the repo in `F:\projects\daw-r26\private\` (do not delete).

Owner (2026-10-13): not going public yet, "me enojaría mucho perder algo" — real repo
untouched. Owner decisions pending: CLA commercial clause vs SignPath (or Certum), public-day
choices (work e-mail rewrite, new repo, publish docs/claude-memory?). The WSL vhdx (147 GB
on C:, 8 GB used) needs an admin `diskpart compact vdisk`. BACKLOG §6g added (double-click
track/clip, type-to-rename, Open editor raises minimised window).

Cleanup pending owner OK: worktrees in daw-r25 and daw-r26, `public-copy`, `public-tmp`,
`downloads` (Salamander archive 707 MiB), `shots` (5.9 GB).

See [[round-25]], [[feedback-no-leftovers]], [[gotcha-boot-path]], [[cantus-parallel-builders]].
