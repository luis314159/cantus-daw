---
name: round-29
description: "Round 29 (started 2026-10-16): competitive beta — warp/time-stretch, MIDI learn, out-of-process plug-ins, tempo track + markers, freeze/bounce/low-latency monitoring; contract 7fde16b on r29/integration; builders in F:\\projects\\daw-r29"
metadata:
  node_type: memory
  type: project
  originSessionId: 986410ca-b4ef-4425-9ddc-1112fd5eda06
  modified: 2026-10-16T21:53:17.262Z
---

Requested 2026-10-16 (the owner's six items; item 6 "frictions" — none given at the start).
The owner gave permission in advance to push to master when everything is green (one commit
per round if a branch had private data). Reply to the owner in Spanish.

Base master e82ff86. Contract `7fde16b` on `r29/integration` (worktree
`F:\projects\daw-r29\integration`): `cantus_core::tempo::TempoMap` API with constant bodies
(bit-identical to Ticks::to_seconds), locale areas tempo/warp/controllers/isolation/freeze,
undo fuzz below(48) with arms 43–47 reserved, `docs/round29/BUILDING.md` (rules, owners,
briefs). Integrator's code maps (outside the repo) in `F:\projects\daw-r29\maps\*.md`.

Builders: wave 1 **tempo** (schema 33, ADR 0060), **warp** (34, 0061; part B after tempo's
engine merges), **isolation** (36, 0063); wave 2 **controllers** (35, 0062) and **freeze**
(37, 0064), both from integration after tempo merges. Tempo writes `ENGINE READY <commit>`
at the top of `docs/round29/tempo.md` when its model+engine are green.

Config per worktree: `F:\projects\daw-r29\cargo-config.toml` copied untracked (jobs 3,
TMP on F:). Shots in `F:\projects\daw-r29\shots\<builder>\`, test build in `…\builds\`.

See [[round-28]], [[cantus-parallel-builders]], [[feedback-no-leftovers]], [[gotcha-boot-path]].

Progress (2026-10-19): tempo finished (3b023df) and isolation finished (5148288), both
merged into r29/integration (0933835; the merge carried TransportInfo's new tempo fields
over the bridge, PROTOCOL 2; schema chain 33 tempo, 34/35 placeholders, 36 isolation).
Their targets deleted. Wave 2 controllers + freeze started from 0933835; warp told to merge
integration again. API limits (session and weekly) cut all agents several times: resume
each with SendMessage, context intact. Reviews saved in F:\projects\daw-r29\reviews\.

2026-10-20: all five builders delivered and merged into r29/integration (bbcc062): tempo
3b023df, isolation 5148288 (+ merge 0933835), warp 5c17c80, controllers bef6f62, freeze
bbcc062. Schema chain 33 tempo, 34 warp, 35 controllers, 36 isolation, 37 freeze; keymap 9.
freeze added `freeze_guard::touches` (every Command must be classified or the build fails).
Reviews (F:\projects\daw-r29\reviews\): tempo-2 67, isolation-2 53, warp-1 76,
controllers-1 57 confirmed; each sent back to its builder (resumed with SendMessage) as fix
work on its own branch; max three compiling, the rest queued. Freeze review running.

2026-10-21: first fix rounds of all five merged (integration 6e74f7e). Second-round reviews:
tempo-3 56 (the content-origin rule for unwarped clips slides recordings on tempo edits —
tempo told to hold BOTH invariants: tempo edits never move a clip's start frame, and
split/trim/comp/loops exact), isolation-3 50, controllers-2 55, freeze-2 58; warp fix review
running. Counts per round stay ~50 (mostly lows) — keep cycling. Review scripts derived per
branch in the session's workflows/scripts (r29-review-*.js, r29-review-fixes-*.js).

2026-10-20: master moved to 56a9105 (going public as cantus-daw, the site, Google
verification) while round 29 is still on r29/integration (based on e82ff86). At the round's
end: merge master into r29/integration, then re-run the slug replacement
(luis314159/cantus → luis314159/cantus-daw, outside docs/round*, docs/claude-memory and ADR
0049) on anything the builders added, push origin master, then publish.sh --push.

2026-10-21 (evening): final short fix round merged (freeze 4cf1da2, isolation bf8f92d),
master merged (2d653e5), docs brought to round 29 (BETA, beta-checklist, manual §14,
STATUS, integration.md, next-session.md = round 30 prompt). **Owner's priority: the first
Windows release downloadable from the website, without problems.** The public repo's first
CI (c85893c, round-28 code) failed three ways: cargo audit (rustls/webpki/zip advisories →
rustls 0.23.46, zip 9 in ac0307e), Linux pool fuzz timing (busy() forgives 1 in 50), Windows
frame_budget hovered-bus 71 vs 36 (not reproducible locally; forgives one frame). Read CI
logs with the stored git credential (`git credential fill`, never printed) + the REST API
(no gh CLI on this machine). Before tagging v0.1.0-beta.1: publish master, watch CI on
cantus-daw, fix flakies, then tag from the publish clone.

2026-10-22: round closed. Final short fix round merged (controllers cfc4db1, tempo 4d38234),
CI robustness + supply chain (ac0307e, dbf8b38, 630fa6d). Final: Windows 4 422 passed
(364 binaries) + engine fuzz 849; Linux (WSL) 4 339 + clippy clean; fmt, notices clean.
Lesson: never run the Windows and Linux suites at once on this machine — wall-clock tests
(sandbox activate poll, controller message age, Chorda CPU share) fail under that load.
Release: lightweight tag v0.1.0-beta.1 on cantus-daw from the publish clone (an annotated
tag would carry the machine's personal git e-mail into the public repo).
