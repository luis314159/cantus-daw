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
