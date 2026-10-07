---
name: cantus-workflow-habits
description: "How work is done in the Cantus repo — tests before wiring, an ADR per structural decision, STATUS.md handoff, schema bump with fixture; plus a tooling quirk of this machine's Bash tool"
metadata: 
  node_type: memory
  type: feedback
  originSessionId: ab6ce7df-a46e-4ebb-a229-b046ae37783d
  modified: 2026-09-24T14:57:46.286Z
---

Working conventions observed in the Cantus DAW repo (c:\projects\daw), inferred from
docs/STATUS.md and the ADRs and followed in round 13:

- Null tests go in *before* the engine wiring; "a control at its default touches no
  sample" must hold to the bit; block-size independence is asserted at tolerance 0.
- Every structural decision gets an ADR (docs/adr/NNNN-*.md) with Context / Decision /
  Consequences and a "what the tests hold" list; STATUS.md is the handoff and must be
  updated (header line with test count, "next session", the table, a round section).
- A model change is a schema bump in cantus-project (SCHEMA_VERSION, a migrate_N_to_N+1
  even if it writes nothing, `cargo run -p cantus-project --example write_fixture` to
  write fixtures/vN.cantus, and the fixture writer extended to use the new fields).
- New commands go into the undo fuzz (crates/cantus-core/tests/undo_invariants.rs) and
  get `cmd.*` keys in both locales (en.txt, es.txt); the i18n test requires every English
  key in every locale.

**Why:** the repo's own rules; breaking them fails tests or leaves the next session blind.

**How to apply:** follow the list above for any feature. Tooling: in this machine's Bash
tool a heredoc delimited by `EOF` fails to parse (the harness uses it); use another
delimiter such as `PYEOF`, or write scripts with the Write tool and run them. Multi-line
Rust edits are most reliable as small Python exact-replace scripts (patchlib pattern);
run `cargo fmt` before anchoring on formatted code, and double the backslash of Rust
unicode escapes inside Python string literals. To *see* the UI, render it:
`cargo run -p cantus-ui --example shoot -- <dir>` writes PNGs through `cantus_ui::raster`
(a CPU renderer). Screen capture failed once because the workstation was locked, and a
full-screen capture can grab the user's private windows — delete any such capture at once.

For several builder agents at once, see [[cantus-parallel-builders]]. Give each area its
own locale file (`locales/areas/<area>.{en,es}.txt`, loaded by `i18n.rs::sources()`) so
parallel branches never append to the same file.
