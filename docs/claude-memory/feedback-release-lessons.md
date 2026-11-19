---
name: feedback-release-lessons
description: "beta.15 needed many re-runs and re-tags; the failure classes were known — rehearse the release matrix before tagging, no absolute timing gates, no fs-order or shared-folder assumptions in tests"
metadata:
  node_type: memory
  type: feedback
  originSessionId: efdd6c7f-2125-496d-987a-1bbaa7998edd
  modified: 2026-11-19T21:06:41.358Z
---

beta.15 (2026-11-19) failed on GitHub one cause at a time: curl 56 in deps.sh, DRSKit copy
in ext4 hash order, Chorda CPU gate (absolute share of a core), scene_vector lid drag
(absolute ms on macOS), tympanum_e2e race (every app's start-up clean_orphans deleted a live
install's staging — shared CANTUS_DATA_DIR per test binary; also a real two-window bug),
the Windows suite past the 120-min job limit, and the unauthenticated GitHub API limit
used up by polling. Owner asked whether the lessons were written down and whether it could
have been foreseen: in kind, yes — round 23 already taught "CPU tests best-of-N".

**Why:** each fix meant a new run of about 2 h, and sometimes a moved tag. `cargo test`
without `--no-fail-fast` shows only the first failing binary, so the failures arrived in
series instead of all at once.

**How to apply:**
- Before tagging, rehearse: run release.yml's whole matrix (Windows, Ubuntu 22.04, macOS)
  on the candidate commit with `--no-fail-fast`, and tag only on green.
- Timing tests on CI compare against something timed beside them (a ratio), never an
  absolute ms or % of a core. Tests that time things in one binary take turns (a mutex).
- Tests never rely on read_dir order, and never on a data folder that the binary's other
  tests also boot into without a lock. Watch for a start-up clean that sweeps a shared folder.
- Every CI download retries on every error (`--retry-all-errors`).
- Watch the suite's duration against the job's time limit every round — the full suite's, not one that stops at its first failure (beta.15: Windows passed 2 h once the Chorda gate stopped failing early; limits now 240 Win / 300 Linux, macOS).
- Poll the GitHub API at most once every ~5 min (60 req/h shared per IP).
See [[round-40]], [[feedback-ci-linux]], [[feedback-work-standards-r36]].
