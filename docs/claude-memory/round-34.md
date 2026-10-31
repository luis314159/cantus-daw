---
name: round-34
description: "Round 34 (2026-10-30..31) — AI: MCP server (ADR 0081), 63 tools (0082), listen + mastering (0083–0084), stems HT-Demucs SDR 8.80 (0085, schema 48), audio to MIDI (0086); five builders + docs in F:\\projects\\daw-r34; Win 5 590 + fuzz 985; beta.9"
metadata:
  node_type: memory
  type: project
  originSessionId: 3b24da2c-539b-4b94-8f79-b015bae6f859
  modified: 2026-10-31T07:23:55.420Z
---

Round 34 = docs/next-session.md of round 33 (owner's order 2026-10-28). Item 0 had nothing
(CI + macOS green, 0 issues, no Mac session → beta.9 = Windows + Linux).

- Contract b4568467 on `r34/integration` (F:\projects\daw-r34\integration): `cantus-ai/src/tool.rs`
  (ToolDef/ToolClass/ToolRun, EditPlan, Said, JobDone::Then), registry `cantus-ui/src/ai_tools.rs`.
  Builders mcp, listen, stems at once; tools after mcp d1; transcribe after stems d1; a docs
  builder at the end. `F:\projects\daw-r34\slot.sh` = 3 build slots + 1 WSL slot (mkdir locks).
- Decisions: MCP written by hand (no rmcp/tokio); HTTP 127.0.0.1 off by default, token per client,
  bridge `cantus-mcp` proves the door per connection; secrets in `<settings>/mcp/`; ML in helper
  `cantus-ml` on ONNX Runtime (load-dynamic, downloaded), weights rebuilt from Meta's .th (Cantus
  hosts no weights); helper + VC++ DLLs in `$INSTDIR\ml\` (else they shadow System32 for plug-ins
  and block updates); provenance as a side map `Project::provenance` (schema 48); no DSP change to
  Space/Organum (pre-roll limits → BACKLOG §4b); checkpoint images only of cells in use (80 → 6–40 MB),
  512 MB budget, released after 10 quiet min.
- Reviews: mcp 6 rounds (10/14/15/12/13/10), tools 4 (18/11/10/4), listen 3+, stems 3+, transcribe 1,
  integrated tree 13 seams, final review 3. Every finding fixed with a test.
- Final on 48188b83: Windows 5 590 passed (82 ignored), fuzz 985, clippy + fmt -p clean.
- Machine notes: a TaskStop of a background suite left its cargo running (killing it was denied by
  the permission classifier) — don't stop a long cargo run mid-way; wait it out.

- After the tag, GitHub caught three things the local suites missed (fixed, tag moved to public
  0514b44f with the owner's OK — deleting a remote tag needs the owner's explicit approval here):
  Windows runner's built-in Administrator → SDDL aliases (`LA`) broke a DACL string test; a fork's
  pool `pool_wakes` underflow seen only with `CANTUS_WORKERS=3` on engine lib tests (macOS's "engine
  on the pool" step); two conversions in one process shared `Cache/transcribe/<pid>-1` (parallel
  test threads). **Local final suite must also run `CANTUS_WORKERS=3 cargo test -p cantus-engine --lib`.**

**How to apply:** round 35 = the dancer (docs/next-session.md, [[decision-round35-dancer]]).
Owner-only items in docs/beta-checklist.md (clean Windows without VC++ redist, Mac rows 17+, signing
cantus-mcp.exe/cantus-ml.exe, .mcpb in Claude Desktop, listening to writers and masters, FAT32/exFAT
export, legal review of the model sources). Proposed next AI work: analysis store per track, assistant
panel with the owner's own key (kept outside the repository), an agent benchmark — and a real
Claude-drives-Cantus session after beta.9 only with the owner's OK.
See [[cantus-parallel-builders]], [[feedback-review-cost]], [[feedback-no-leftovers]].
