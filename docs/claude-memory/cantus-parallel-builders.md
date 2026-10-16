---
name: cantus-parallel-builders
description: "How to run several builder agents on Cantus in parallel without touching master or the user's unstaged changes: snapshot commit via a temp index, one worktree per builder, file-by-file 3-way merge back"
metadata:
  node_type: memory
  type: feedback
  originSessionId: b0b2654a-292c-4c32-83d9-5788c7201752
  modified: 2026-09-25T20:22:44.518Z
---

Round 16 (effects) ran seven builder agents at once this way, and it worked; round 17
(instruments) reuses it. The user's work in c:\projects\daw is usually **uncommitted and
unstaged**, and commits happen only when the user asks.

1. Snapshot the working tree without moving a branch or the real index:
   `GIT_INDEX_FILE=$(mktemp) git read-tree HEAD; git add -A; git write-tree; git commit-tree <tree> -p HEAD`
   (unset GIT_INDEX_FILE after). Save the base hash to a file — merging back diffs against it.
2. `git worktree add -b <prefix>/<name> /c/projects/daw-<round>/<name> <base>` per builder,
   plus an untracked `.cargo/config.toml` with `debug = 0` for dev/test and `jobs = 4` so
   parallel builds fit on the machine.
3. Merge back into the working tree only, file by file: added → copy; modified → take
   theirs if main still equals the base blob, else `git merge-file main base theirs`.
   A script for it: merge_fx.sh (last copy in the round-17 session scratchpad).
   Run `cargo check --workspace` after each merge — cross-branch API drift happens.
   **Normalise line endings first**: some old checkouts (core.autocrlf) left files CRLF in
   the working tree while blobs are LF, so identical content becomes a whole-file conflict.
   `tr -d '\r' < f > tmp` and compare before merging. `grep $'\r'` does not detect CR
   reliably in Git Bash.
4. Two sessions sharing the tree: split schema numbers and ADR numbers up front. Whoever
   merges second renumbers (migration plus fixture) and the undo-fuzz arms. Announce every
   merge that touches the engine or the format before doing it, and stay out of the tree
   while the other session merges.
   **Strip CR first** (`tr -d '\r'` on the main file before comparing or merging):
   core.autocrlf is true, so some tree files are CRLF with content identical to the base,
   and merge-file turns them into false whole-file conflicts. A new worktree checks out
   CRLF too; re-checkout with `git -c core.autocrlf=false` before merging into it.
4. Scaling that worked in round 17: dispatch lines pre-declared for every builder in a
   contract commit, an integration worktree where everything is merged and tested before
   one final file-by-file merge into the user's tree. Builders can stall (stream watchdog)
   or stop at an account spend limit: resume each with SendMessage, context intact.

**Why:** the user's changes must never be staged, committed or lost; parallel agents in one
tree break each other's builds.

**How to apply:** tell builders to write files in pieces under ~300 lines per tool call (a
600 s stream watchdog killed three whole-file writes), to make a WIP commit early on their
branch, and to declare extra modules with `#[path]` inside their own files so shared
`lib.rs`/`mod.rs` stay untouched. Coordinate with any other live Cantus session first
(ListAgents / SendMessage). See [[cantus-workflow-habits]].

Round 28 (2026-10-14/16) added:
- An agent stopped twice in a row by an API safeguard false positive: launch a **fresh agent
  on the same branch** with a brief of what is done (commits) and what is left; it works.
- **Fix branches share the integration target** (`target-dir` in their untracked
  `.cargo/config.toml`): warm builds, one target on a nearly full disk — but never build in
  integration while they run (cargo's lock).
- A builder can `git add` its untracked `.cargo/config.toml`: check `git ls-tree -r <branch>
  --name-only | grep ^.cargo` before every merge.
- **Review every fix round**, not only the features: second reviews found regressions the
  first-round fixes introduced. Counts per round fell 45 → 20 → 25 → 13 → 6 → 6 (low).
- Reviewers (Workflow) must be read-only (no cargo): RAM belongs to the builders.
- Two parallel branches editing the same exported-plug-in function (notes + transport):
  expect to reconcile semantics by hand, not just text (round 28's HostSeen beats/last_beats).
