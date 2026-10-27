---
name: round-31
description: "Round 31 (2026-10-25..27) — daily gaps: automation modes + lanes, note effects/key/grooves, crossfades + clip gain, palette/history/templates/start page, starting presets, cores as engines; beta.6 Windows+Linux; next = §6h part 2"
metadata:
  node_type: memory
  type: project
  originSessionId: fbe99dfa-5aab-438f-9222-84305f1ed822
  modified: 2026-10-27T21:58:11.677Z
---

Round 31 = BACKLOG §0.4 item 3 (the daily gaps) plus round 30's flaky tests. Contract
`697550fa`, integration `r31/integration` (F:\projects\daw-r31\integration),
`docs/round31/integration.md` is the record. Builders automation (ADR 0068, schema 42),
midi (0069, 43), editing (0070+0071, 44), workflow (0072, keymap 11); vehicles (the cores as
engines + SVG cars, model names only, no makers). Six one-agent reviews, 57 findings fixed.
Final suites on 59bc8282 (0.1.0-beta.6): Windows 4 707 passed / 0 failed, fuzz 895, Linux
clippy + tests exit 0. `tools/wsl/check.sh test` now prints a TOTAL line.

Flaky causes found (not waits): test-support scan counted as destroyed; stale state after a
save; liveness from a heartbeat timer (now a progress word); `Server::grow` answered before the
processing thread served; the process-wide locale (tests use `i18n::set_thread_locale`);
an app built from parts read the user's plugins.json (owner had Odin2/Surge XT/Valhalla).

Owner requests added: BACKLOG §6h (per-plug-in looks; hosted native editor vs Cantus view,
default per plug-in — I promised to start part 2 after the round), §7a singing voice
(`research/ai/singing-voice.md`), start page at launch with *Show at start*, cores as cars.
Notation from the piano roll is still BACKLOG §0.2.

**Why:** owner's round-31 prompt; Mac session not done, so beta.6 is Windows + Linux
(CANTUS_RELEASE_MACOS off). Owner pre-approved push, publish, tag when green; Spanish replies.

**How to apply:** round 32's prompt is `docs/next-session.md`. Squash-merge rounds to master
so intermediate PNGs (they were uncompressed, ~120 MB) never enter history. Disk trick: move a
finished builder's target to the next worktree; fix rounds share one target via target-dir.
See [[round-30]], [[cantus-parallel-builders]], [[public-repo-cantus-daw]], [[feedback-review-cost]].
