---
name: round-22
description: "Round 22 (started 2026-10-07): multicore live pool + beta hardening (build v3, math2, export MP3/FLAC/Ogg, beta-docs, soak); round 21 closed on r21/integration 33f9c73 (not pushed)"
metadata:
  node_type: memory
  type: project
  originSessionId: a47a4c80-267b-403d-a74f-bdab0153945d
  modified: 2026-10-07T13:27:11.503Z
---

Round 21 closed on `r21/integration` @33f9c73: 1 718 tests green, clippy clean; the owner's
test build (ASIO always linked) is C:\projects\daw-r21\builds\cantus.exe. **Not merged into
master nor pushed** — ask the owner (they asked to push round 20 explicitly).
docs/BETA.md (on that branch) is the road to 0.1.0-beta.1: 6 blockers (real hardware,
performance floor, stability, owner's real use, code signing, legal).

Round 22, worktrees C:\projects\daw-r22\<name>, base c8bc9f2 (brief docs/round22/BUILDING.md):
pool (wire sched into the graph: multicore live engine; sole owner of processor*/graph/
sched; never spin inside MMCSS), build (x86-64-v3 + v2 fallback + launcher), math2 (fast
powf, counter dither R10, Aequalis residual; DSP_VERSION 3), export (MP3/FLAC/Ogg,
loudness presets), beta-docs (THIRD-PARTY-NOTICES, PRIVACY, kit About, EN/ES manual, issue
templates, release notes, version 0.1.0-beta.1, no tag), soak (soak harness, crash
recovery). After: round 23 = remaining beta items + owner's hardware/song tests.

Owner's to-dos: test the UMC with the r21 build, apply for SignPath, trademark search.
See [[round-21]], [[feedback-one-build-all-drivers]].

2026-10-07 owner request: **when round 22 ends, push to master** (rounds 21 + 22), and make the
repo self-sufficient for another PC: docs/DEVELOPING.md (prereqs: Rust pinned via
rust-toolchain.toml, MSVC, LLVM for asio-sys bindgen; xtask fetches ASIO SDK + NSIS with
pinned hashes), a repo CLAUDE.md carrying the working conventions now only in this memory,
the test-plugin SOURCES list (hashes/URLs, not binaries), no absolute paths to this PC in
tracked files; then **prove it with a fresh clone into an empty folder** following the doc
until build + tests pass, before pushing. Do not commit third-party screenshots, sample
packs, plug-in binaries or target/.

2026-10-07 (later): the owner asked that a clone carry everything, "screenshots, research etc."
The repo is private (GitHub API 404 unauthenticated), so `research/*/screenshots/` (479 files,
130 MB) were un-ignored and committed on r22/integration (4c5841b); BETA.md says to strip
them from history before the repo is made public. The owner then approved copying this
memory into the repo: `docs/claude-memory/` (README says how to restore it); re-copy it
there at the end of each round, before the push.
