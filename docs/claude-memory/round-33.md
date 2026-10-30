---
name: round-33
description: "Round 33 (2026-10-29..30) — Ableton's core: session view (ADR 0076 schema 46, view/grids ADR 0077 keymap 14), Link (GPL crate cantus-link, ADR 0078), MIDI clock/MTC (ADR 0079), MPE (ADR 0080, schema 47, sandbox protocol 7); all green, squashed to master as beta.8; next = round 34 (AI)"
metadata:
  node_type: memory
  type: project
  originSessionId: d27e5c3d-43a2-403c-a635-cfda238eadd1
  modified: 2026-10-30T10:18:47.151Z
---

Round 33 = docs/next-session.md of round 32 (owner order 2026-10-28). Item 0 had nothing
(macOS green, no issues, Mac session not done → beta.8 = Windows + Linux).

- Licence decision: Link SDK is GPLv2+/proprietary → Link's protocol in Rust in a new
  GPL-3.0-or-later crate `cantus-link`; the MPL engine sees only `cantus-engine/src/sync.rs`.
- Contract `e0853ec1` on r33/integration; builders session, launcher (started from
  session's first delivery), sync, mpe in F:\projects\daw-r33\. Merged in order: session d1
  → session engine 5eba37bd → sync d658ee43 → mpe a851e2ef (schema chained 45→46→47) →
  launcher 1df68be4 → session round 2 ce23e294 → integrated-review seams (session 6582f587).
- Session design: no session code on the audio thread; a planner + conductor play the
  performance as a projection compiled into ordinary snapshots. Lead = 2 device blocks +
  compile p95 + 2 ms; late presses caught up; scene tempos proposed to Link from the audio
  thread via `Snapshot::scene_tempos`.
- Final checks on 6582f587: Windows 5 137 pass (61 ignored), engine fuzz 963
  (`--features fuzz`, CANTUS_WORKERS=3), clippy + fmt -p clean; Linux (WSL) 5 120 pass, clippy clean.
- Causes met: F: filled twice with three builders linking (delete each target on merge);
  a Linux-only palette race between lib tests (light case moved to its own test binary);
  timing tests under load (made median/best-of-N).
- Firewall: tests bind 127.0.0.1 only; the shipped app's first Link enable prompts (expected).

**How to apply:** round 34 = docs/next-session.md (AI: MCP server first, stems, audio to
MIDI, mastering; beta.9); round 35 = the dancer ([[decision-round35-dancer]]). Owner-only
items for round 33 (Launchpad/APC, a real Link app, MIDI clock/MTC hardware, MPE controller,
listening to a performance) are in docs/beta-checklist.md. See [[round-32]],
[[decision-round33-order]], [[cantus-parallel-builders]], [[feedback-review-cost]].
