# Releasing Cantus — read this before every release

A release is a tag on `cantus-daw` that `release.yml` builds, tests and publishes for three
systems (`docs/DEVELOPING.md` §5, ADR 0067). This page holds **the lessons every earlier
release paid for**, and the release enforces them: `release.yml`'s first job,
**preflight** (`tools/release/preflight.sh`), refuses a tag unless
`docs/releases/v<version>.preflight.md` ticks every lesson below and names a green
rehearsal of the same code. A lesson added here without a line in the checklist's
template fails the workspace's tests (`xtask`, `package_ci_tests.rs`).

beta.15 (2026-11-19) is why: its run failed six times, one cause at a time, each costing a
new run of two to four hours and a moved tag. Every cause belonged to a kind an earlier
round had already met.

## The procedure

1. **The candidate.** Version in `Cargo.toml` and `web/src/top.rs`, the release notes
   `docs/releases/v<version>.md` (from `TEMPLATE.md`), the round's suite green on Windows
   and in WSL (`tools/wsl/check.sh`). Publish it to `cantus-daw` (`tools/public/publish.sh`).
2. **The rehearsal.** On `cantus-daw`: *Actions → Release → Run workflow* on `master`. It
   runs the release's three jobs exactly — clippy, the **whole** suite (`--no-fail-fast`)
   and every package — and publishes nothing. If any job fails, read **every** failure in
   the run, fix them all in one commit, publish, rehearse again (L7).
3. **The checklist.** Copy `docs/releases/PREFLIGHT.md` to
   `docs/releases/v<version>.preflight.md`, write the green rehearsal's run id and commit,
   and tick each lesson only after checking it for this release. Commit and publish: this
   commit changes **only** files under `docs/releases/` — anything else needs a new
   rehearsal, and preflight refuses the tag.
4. **The tag.** `git tag v<version> && git push origin v<version>` on `cantus-daw`, with
   the owner's OK. Preflight checks the checklist and the rehearsal through the GitHub API,
   and only then do the three systems build.

## The lessons

### L1 Rehearse before the tag

Tag only a commit whose code a hand-run *Release* (`workflow_dispatch`) passed on all three
systems; the tag's commit may differ from it only in `docs/releases/`.
*Paid for:* beta.15 was tagged first and tested after; every failure moved the tag again.
*Check:* the run id in the checklist is green, on `release.yml`, run by hand; preflight
verifies both and the diff.

### L2 The whole suite runs, and fits its job's time limit

Every `cargo test --workspace` in `release.yml` runs with `--no-fail-fast`, so one run
shows every failing binary. Each test job's `timeout-minutes` must hold the **whole**
suite with room to spare: read the rehearsal's step times, and raise a limit that is under
1.5× them in the same commit.
*Paid for:* beta.15's first failure hid the binaries after it; once it was fixed, the full
suite ran past Windows's 120 minutes and was cancelled with nothing failed.
*Check:* the rehearsal's test steps against `timeout-minutes` in `release.yml`.

### L3 Timing tests compare, never measure against a fixed number

A test that times something compares it with a reference timed beside it, in the same
test (a ratio: model 2 against model 1), or takes the best of several tries over a bound
far above a quiet machine's; never an absolute share of a core or a fixed millisecond
budget alone. The timing tests of one binary take turns (one mutex), since a binary's
tests run in parallel. A runner is several times slower than the owner's PC, and noisy.
*Paid for:* round 23 (CPU tests best-of-5), beta.15 (Chorda's gate at 60 % on Windows;
`scene_vector`'s lid drag at 2.4 s on macOS).
*Check:* the round's new tests with `Instant` or `elapsed`: each compares, or has the
margin and turns.

### L4 No test depends on the order of a folder's listing

`read_dir` gives NTFS's alphabetical order on Windows and ext4's hash order on Linux (and
another one in WSL than on a runner). Sort what is listed, and create a destination folder
before copying into it.
*Paid for:* beta.15 (DRSKit's copy met the kit's XML before its folder on Ubuntu 22.04).
*Check:* the round's new `read_dir` walks in tests and in the code they reach.

### L5 Tests that share a folder or the process leave each other's work alone

A test binary's tests run in parallel in one process. They share one `CANTUS_DATA_DIR`:
whatever an app sweeps at start-up (caches, staging, orphans) must leave a live process's
work alone — a lock, as `install::clear_all_staging` and `user_pack::clean_orphans` take;
what races between two tests races between two Cantus windows. And they share every
process-wide value: the theme (`theme::apply` installs its palette and the kit's tokens for
every thread), the locale's fallback, global registries. A test that reads what another
may set takes turns (one mutex per binary) until the value is per context.
*Paid for:* beta.15 (`tympanum_e2e`: a test's start-up deleted another's install mid-build;
`instruments_scale_e2e`: a test's app installed Slate Lapis while another measured Chorda's
knobs in Slate, about one run in five on a loaded four-core runner).
*Check:* the round's new start-up clean-ups, the folders their tests share, and the new
tests that read a process-wide value another test of their binary sets.

### L6 Every download in CI retries on every error, and is checked

`curl --retry 5 --retry-all-errors --retry-delay 5` (plain `--retry` takes a reset
connection, curl 56, as final), and a pinned SHA-256 where the file is a tool or a
library. Through the corporate proxy, verify what arrived before trusting it.
*Paid for:* beta.15's AppImage job (archive.ubuntu.com reset the connection).
*Check:* the round's new `curl`/`wget`/`Invoke-WebRequest` in workflows and scripts.

### L7 One failed run, one fix of everything in it

When a run fails, read every job's failures before fixing anything; fix them together, in
one commit, and rehearse once. Move a published tag only with the owner's OK, once per
batch of fixes.
*Paid for:* beta.15 (five fixes, five runs, the tag moved four times).
*Check:* the checklist names the rehearsal that passed; the earlier failed ones need not be
listed.

### L8 Watch runs politely

The GitHub API without a token allows 60 requests an hour per address, shared by every
session on the machine: one session watches a run, at most once every five minutes, and
the others ask it. Job logs need a sign-in; annotations and step lists do not.
*Paid for:* beta.15 (a second watcher used up the hour, and the first one went blind).
*Check:* one watcher is named before the tag.

## How the suite runs — the practice to keep

Excellence here means two things at once: **nothing reaches users that the whole suite has
not passed on its exact code**, and **nobody waits longer than the work needs**. The rules:

1. **A release runs the whole suite once, on its exact code, and never less.** The
   rehearsal (L1) is that run. The tagged run differs from it only in `docs/releases/`, so
   it may reuse the rehearsal's result instead of running the suite again — same code to
   the byte, same result — and builds and packages only. Nothing else is ever skipped:
   tests chosen by "what changed" miss exactly what beta.15 failed on (tests racing each
   other, a loaded runner, a file system's order, the network — none in the changed code).
2. **Every test in its own process, and the suite split across runners.** With
   `cargo-nextest`, one process per test removes the races of L5 by construction (the
   theme, the data folder, any global), and partitions run in parallel jobs: the Windows
   suite's three hours become about one on three runners. Each partition holds the whole
   suite's rules (`--no-fail-fast`, the time limit with room, L2).
3. **A flaky test is a bug.** It is never retried in silence: a retry that passes is
   reported as flaky, the cause is found (as L3–L5's were) and fixed, or the test is
   quarantined with an issue and a date — never left to pass by luck.
4. **On a branch, fast feedback first; before master, everything.** A push may run the
   crates its change touches first (their dependents included, by `cargo metadata`), so
   its author hears in minutes; nothing merges to master without the whole suite green on
   Windows and Linux.
5. **Disk and load before every suite** (`CLAUDE.md`): `df -h` first — beta.15's local
   check failed to link with 73 MB left on F: — and never two suites at once on one
   machine, or timing tests fail for the load, not for the code.

These are the practice; building 1 and 2 (the reuse in `release.yml`, nextest and its
partitions in `ci.yml` and `release.yml`) is in `docs/BACKLOG-excellence.md` §8.
