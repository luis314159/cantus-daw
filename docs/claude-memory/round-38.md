---
name: round-38
description: "Round 38 (2026-11-08/09) — at rest costs nothing (ADR 0106), dancer off at start, Chorda scene 4x faster; closed as v0.1.0-beta.13 on 2026-11-09 with a quiet scorecard; next = round 39 UX and graphics"
metadata:
  type: project
---

Round 38 was run in this session after beta.12 (2026-11-08). The owner then asked to move to round 39 (UX and graphics). The round is **not closed**: its two branches are unmerged and beta.13 is not out. The prompt for the next session is `docs/next-session.md` in F:\projects\cantus, uncommitted on master. Its step 0 closes round 38.

- **r38/scorecard** (F:\projects\daw-r38\scorecard, up to 264716fb, written by an agent):
  - What changed: the mixer rests in silence, bit-exact; repaints go through the ticker; Chorda is designed when the picker opens; idle threads sleep; the dancer is off at start (e2e); ADR 0106 plus amendments; tools/scorecard/profile.ps1.
  - At rest: Cantus went from 30 % of a core to about 2 %. Memory is still lost: 237 MB against FL 133 and REAPER 62, because the engine preallocates (fix proposed in an ADR). The site still shows round-37 numbers.
  - Re-measure on a quiet machine before publishing: the agent's REAPER run (5 %) had a build running beside it.
- **r38/chorda** (F:\projects\daw-r38\chorda, 7f9d2a38, written by me):
  - The scene's baker is about 4x faster, bit-identical: span rasterization, bands of rows taken by free threads, parallel passes. Previews now render at full resolution with a 0.15 s dissolve. The owner had found the first fix "un poco mejor pero no arreglado". The owner confirmed it on 2026-11-09 with builds\chorda-scene2: "ya no se pixelean".
  - Voicing by register: in the code but flat. Voicing pass three is half done (metric 9.05 → 7.66) and is deferred to round 40; the report is docs/round38/chorda.md.
  - BACKLOG §6j holds the round-39 plan, the button study, the owner's crops (research/ui/owner-crops-2026-11-08/) and the top-bar overflow panel ("horrenda").
- C: has only 16 GB free; the WSL image lives there.

See [[decision-round39-ux-graphics]], [[ui-quality-bar]], [[round-37]].

**Closed 2026-11-09 as v0.1.0-beta.13** (this session): r38/integration (F:\projects\daw-r38\integration) merged both branches with no conflict; ADR 0104 amended; two read-only reviewers found the dancer frozen under live MIDI, an import re-check on two clocks, the dancer editor at 60 Hz, the endpoint watch re-armed late — all fixed with tests (ADR 0106 "found by the review"). Suites: Windows 6 554, fuzz 1 106, Linux 6 515 (clone ~/ci-r38-integ = r37's warm one renamed; fits C:). Quiet scorecard (owner closed all, WSL shut down): Cantus empty 0.74 s / 1.5 s / 237 MB / 1.6 % vs REAPER 1.76 / 2.0 / 62 / 2.3–3.7 %; Beat 0.76 / 1.6 / 239 / 1.8–2.0 % vs FL 2.49 / 5.8 / 133 / 6.6 %. REAPER at rest no longer 0.3 % (unknown why; both published). Gotchas: `cargo fmt --all` on Windows fails (os error 206, too long) — use `cargo fmt -p` per package or WSL; the kit lint matches `Color32::from_rgb` inside `from_rgba_premultiplied`. Next: round 39 (docs/next-session.md).
