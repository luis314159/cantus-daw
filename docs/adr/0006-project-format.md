# ADR 0006 — Project format: versioned JSON, atomic writes

**Status:** Accepted · **Date:** 2026-09-07

- **JSON** (`serde_json`, pretty-printed, stable key order): human-diffable, git-friendly,
  readable by every tool and every AI model without a custom parser. RON was considered and
  rejected for tooling reasons only.
- Top-level `schema_version: u32`. `cantus-project` holds an ordered list of migrations
  `N → N+1`; loading any older version runs the chain. A test loads every historical fixture
  in `crates/cantus-project/fixtures/`.
- **Atomic save:** write `name.cantus.tmp`, fsync, rename over the target. The previous file
  is renamed to `name.cantus.bak` first and only deleted after the new file is re-read and
  its SHA-256 (stored in the `integrity` field) verifies.
- Musical positions are stored in **ticks** (960 PPQ), never seconds, so tempo changes never
  shift notes.
- Absolute paths are forbidden; assets are referenced relative to the project folder.

## Amendment (round 22, *soak*): recovery is offered, and refusals are named

- **Session journal.** Each running Cantus keeps `<data>/sessions/<pid>-<ms>.json` beside a
  `.lock` it holds locked for its lifetime (the OS frees the lock however the process ends).
  The journal names the open project, whether it has unsaved changes, the last autosave,
  and the recording passes whose takes are in no file yet (track, WAV path, where the pass
  started, loop/punch, tempo, latency). A clean exit deletes it; a close with unsaved work
  keeps it (marked `closed`) after the last-defence autosave; a crash leaves it unlocked.
- **The offer.** At start, the newest unlocked journal with something to give back is
  offered (*Recover your work*: Restore / Discard). *Restore* loads the last save, puts the
  autosave over it as **one undo step** (`ReplaceProject`), and lays each recovered take out
  as the finished pass would have been (`record_pass::build`, loop wraps recomputed), its
  WAV header first repaired to the last whole frame on disk. *Discard* deletes no file.
- **Autosave fallback.** An autosave that cannot be written beside the project goes to the
  fallback folder (named after the file) and the status line says so once; a failing
  autosave is reported, never only logged.
- **Refusals.** `StoreError::problem()` classifies a failed load (truncated, damaged, newer
  schema, not a project, integrity, I/O); the interface words each and says the open project
  and the file are unchanged, naming a readable `.bak`/`.autosave` beside it. A save never
  moves a damaged file into `.bak` (it goes to `.corrupt`), so the last good backup survives.
- `CANTUS_DATA_DIR` moves every per-user file (tests, the soak harness).
