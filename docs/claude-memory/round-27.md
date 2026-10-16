---
name: round-27
description: "Round 27 (2026-10-14): BACKLOG §6g quick gestures (ADR 0051) — pushed fast-forward as master 445a44c; test build in F:\\projects\\daw-r27\\builds; next-session prompt (round 28) given in chat"
metadata:
  node_type: memory
  type: project
  originSessionId: 03130613-fa0b-4e4e-ac84-674beeca975e
  modified: 2026-10-15T00:21:58.361Z
---

One builder (branch r27/gestures, worktree removed after merge), one adversarial review
(11 findings fixed), verified Windows 2 490 tests + Linux (WSL) green. Pushed 2026-10-14 as
a fast-forward of 8 commits to master 445a44c (no private data in that branch).

Owner's rule for renaming (overrides §6g.2): rename ONLY by a click exactly on the track
name's letters; typing never renames; Esc cancels. Double-click elsewhere on the header
opens the instrument; Open editor / F8 bring the window forward (restored if minimised; F8
closes only what is in front); double-click a notes clip → piano roll, or channel rack when
the rack loses nothing (one pitch per synth channel, kit pads, the clip's own grid). Mixer
strips and rack pads keep double-click rename (owner may want to revisit).

Test build: `F:\projects\daw-r27\builds\` (setup, stable copy, portable). The owner was
still running Cantus from `F:\projects\daw-r25\builds\night\` (folder left in place).

Next session the owner chose BACKLOG 3–6 (top bar §6e, voice §3c/§3d, effects §3b, a CC BY
drum pack / Chorda §3e, comping + modulators §0.2) — things that need neither their
hardware nor other software; a round-28 prompt was given in chat. The DAW-vs-DAW benchmark
needs the owner (other DAWs installed, the UMC, ~2–3 h of their time).

See [[round-26]], [[feedback-no-leftovers]].
