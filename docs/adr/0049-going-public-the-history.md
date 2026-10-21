# ADR 0049 — Going public: what leaves the history

**Status:** Accepted as a plan — executed only on the day the owner decides (on
2026-10-13 they said: not yet) · **Date:** 2026-10-13 · **Round:** 26 · **Related:** ADR
0030 (installer, updates), ADR 0042 (content licences), ADR 0047 (licences, CLA, marks),
`docs/BETA.md` blocker 6, `docs/beta-checklist.md` §2, `docs/round26/public.md` (the
report, the commands)

## Context

The repository is private on GitHub (`luis314159/cantus`, only `master` pushed). The beta
needs it public: the downloads, the auto-update (it reads the public Releases), SignPath's
free signing for open-source projects and free CI minutes all assume a public repository.
The owner wants the step conservative: **nothing lost, nothing leaked**. The history holds
things that must not be published:

- **479 reference screenshots of other products** under `research/*/screenshots/`
  (FabFilter, FL Studio, Ableton, Bitwig, Logic/Studio One, Kontakt, Pianoteq, Arturia…),
  added in one commit (`4c5841b`, 2026-10-07) at the owner's request while the repository
  was private; ~125 MB of the 155 MB pack. They are third parties' copyrighted images. (77
  of them, `research/ui/screenshots/cantus-audit/`, are pictures of Cantus itself; they
  leave with the rest because the owner's rule names the whole folder.)
- **The Reina-Valera 1960 file** `crates/cantus-ui/locales/verses/es.txt` (281 passages,
  © Sociedades Bíblicas Unidas), in every commit since `b0dce1f` (2026-10-05). Its
  publisher's terms ask written permission of applications and websites whatever the
  number of verses; until it arrives (`verse::RVR1960_PERMITTED = false`) the text must
  not be published, in the tree or in the history.
- **RVR1960 quotations**: the motto, Colossians 3:23, in `web/src/body.rs` and
  `docs/releases/v0.1.0-beta.1.md` (six blobs), and five words of Psalm 33:3 in older
  blobs of `docs/round20/verse.md`.
- **What is private without being secret**: the work address on part of the commits, and
  notes about the machine's setup (details kept privately, outside every repository — a
  committed document must not name what it advises to remove).

The scan (gitleaks 8.30.1 over every commit of every ref, merges included, plus
`tools/public/scan.py` over every distinct blob — text, UTF-16 and the strings of
binaries — every commit and tag message and every identity) found **no secret**.

## Decision

1. **The public repository is a new GitHub repository** that receives a *rewritten*
   history; the private one is never force-pushed, never made public, never deleted — it
   is renamed (`cantus-private`) and archived whole. GitHub keeps unreferenced commits of
   a repository reachable by hash (and in caches, forks and pull request refs), so a
   force-push followed by "make public" can still leak the old objects; a new repository
   starts with only what is pushed to it.
2. **What leaves every commit**, in one `git filter-repo` run on a fresh `--no-local`
   clone of `master` alone (`--single-branch`), with `--invert-paths` and the
   case-insensitive `--path-regex` set that `tools/public/scan_rules.py` also checks
   (`scan.py --filter-repo-args` prints it):
   - `(?i)^research/(.*/)?screenshots/` — every screenshot folder under `research/`, at
     any depth;
   - `(?i)^research/.*\.(png|jpe?g|gif|webp|bmp|tiff?|avif|heic|ico|svg|pdf|psd)$` — any
     image or PDF elsewhere under `research/`;
   - `(?i)^crates/cantus-ui/locales/verses/es\.txt$` (until the permission);
   - `--replace-text` rules that turn the RVR1960 quotations into the public-domain RV1909
     (`tools/public/motto_replacements.py` builds them from the private `es.txt`, outside
     the repository, so no published file carries the RVR1960 wording);
   - `--mailmap <private folder>/identities.mailmap`, if the owner chooses to rewrite old
     identities (the GitHub no-reply address is the suggestion) — **the owner's choice**.
3. **What is published:** `master` and the release tags (`v0.1.0-beta.1`, created on the
   rewritten history, never before). The `r*/` branches are not: they are builders'
   work in progress, superseded by each round's integration on `master` (round 25 was
   applied, not merged, so `r25/integration` holds 54 commits that are not ancestors of
   `master`). Round 26 itself is squashed into one commit on `master`, so this round's
   intermediate report blobs (which named the work address before the review) never reach
   it.
4. **The tree that day** (one commit on top of the rewritten `master`, in the public clone,
   before the first push): `research/**/screenshots/` and `es.txt` go into `.gitignore`
   (the owner keeps them on disk); the `research/*.md` links to the screenshots (inline
   and reference-style) become plain text (`unlink_screenshots.py`); the verse README says
   the RVR1960 file is kept privately. Already in the tree since round 26: the build does
   not need `es.txt` (`include_str!` read it even in a dead match arm; the verse tests read
   it only when it exists, and only `NotFound` while not permitted counts as absent),
   `LICENSE` and the notices say the RVR1960 is kept privately and the RV1909 is the
   Spanish text shown.
5. **The owner's private folder**, outside every repository, holds what must not be
   committed: `forbid.txt` (addresses, domains, names that must appear nowhere),
   `identities.mailmap`, and the note explaining them.
6. **Route (b), a fresh one-commit history**, is the alternative if the owner prefers not
   to publish the history or its dates: the same tree, one orphan commit, the untracked
   files left on disk.
7. **The check is a script, read-only, and strict:** `tools/public/check.sh --forbid FILE
   [repo] master --tags` must say `READY` before the first push and again before the tag.
   gitleaks is mandatory (only an explicit `--no-gitleaks` skips it, and then the run
   cannot say `READY`); a missing forbid list stops the check unless `--no-forbid` is
   explicit; a refused ref or no commit to scan is an error, never a clean result.

## Consequences

- The public history is ~30 MB instead of ~156 MB; every commit from `b0dce1f`
  (2026-10-05) on gets a new hash — 186 of `master`'s 294 on 2026-10-13 — and with the
  mailmap every commit does.
- Commit hashes quoted in documents (58 tokens, 52 in the rewritten range) keep pointing
  at the private history; commit *messages* are translated by filter-repo. The run's
  `.git/filter-repo/commit-map` is kept with the private archive (and may be published as
  `docs/history-map.txt`).
- The owner's working copy moves to the public clone that day: the untracked files
  (`.claude/`, `docs/monetize.md`, the screenshots, `es.txt`) are copied over and listed in
  `.git/info/exclude` or `.gitignore`; the old folder and its worktrees are the archive.
- GitHub Actions is free for public repositories on standard runners.

### When the RVR1960 permission arrives

One commit: put `es.txt` back (and out of `.gitignore`); restore its `include_str!` in
`Translation::source`; delete the compile-time `assert!(!RVR1960_PERMITTED, …)`; set
`RVR1960_PERMITTED = true`; remove the es.txt entry from `FORBIDDEN_PATHS`
(`tools/public/scan_rules.py`) and the es.txt probe and pattern from `check.sh`'s tree
check; delete `rvr1960.fingerprints` and `rvr1960-4.fingerprints`; restore the RVR1960
notice in `LICENSE` and `xtask/notices/head.md` (and regenerate the notices); drop the
es.txt `--path-regex` and the `--replace-text` rules from the plan if it has not run yet.

## What the checks hold

`tools/public/check.sh`, on any clone, read-only — failing on:

- **a forbidden path in any commit** of the refs (`git ls-tree -r` of every commit, the
  three regexes above, case-insensitive);
- **RVR1960 text in any blob**: 6-word shingles (after HTML entities, `\u` escapes,
  hyphenation across lines and escaped newlines are undone; accents, case and punctuation
  folded), salted and hashed, against 1 666 fingerprints of the RVR1960 file that do not
  occur in the RV1909 or the WEB; no Bible text in the repository;
- **a secret**: gitleaks, and the scanner's own patterns (private keys; GitHub, AWS,
  Azure, Slack, Google, Stripe, npm, PyPI, Anthropic, OpenAI, Hugging Face tokens; JWTs;
  Cloudflare and signing assignments) in text, UTF-16 and binary strings, commit and tag
  messages; secret-shaped file names;
- **any forbid-list entry** in a path, blob, commit or tag message, or raw
  author/committer/tagger identity;
- **the tree**: the screenshot rules and `es.txt` ignored and untracked, no inline or
  reference-style link to the screenshots.

Listed for review, never failing: 4-word RVR1960 runs (1 424 fingerprints), low-confidence
assignments, images outside our picture folders, identities, e-mails, local user paths.

Measured in round 26 (2026-10-13): the original history — 480 forbidden paths, 7 RVR1960
blobs, 0 secrets, pack 155.4 MiB; the paths-only rewrite — pack 29.7 MiB, the 6 motto blobs
left; the full plan rehearsed on `r26/integration` squashed onto `master` (paths, the
replacements, the mailmap, the day-of commit) — `READY`.

## Amended 2026-10-20 (round 29): the owner's repositories

The owner created the public repository as **`luis314159/cantus-daw`** and kept the private
`luis314159/cantus` as the **working** repository (not renamed, not archived): it deploys
the website (`site.yml`, its Cloudflare secrets) and keeps everything — the full history,
the screenshots, the RVR1960 file. So:

- **Publishing is a script, not a one-time move.** `tools/public/publish.sh <private-folder>
  [--push]` clones the private `master`, rewrites it with the plan's exact rules (the three
  path regexes, the RV1909 replacements, the mailmap to the GitHub no-reply address), runs
  `check.sh` (which must say `READY`) and pushes to `cantus-daw` only as a fast-forward. The
  rewrite is deterministic, so each run extends the last one; nothing is ever forced, and
  the public repository never holds an object of the private history.
- **The day-of tree changes are in the private `master`** (so no commit exists only in the
  public one): the screenshots and `es.txt` are in `.gitignore` (tracked files stay tracked in
  the private repository; a new screenshot goes in with `git add -f`), the research notes'
  links to the screenshots are plain text, the verses README says where `es.txt` lives.
- **Every public address names `cantus-daw`**: the updater (`cantus-update`'s `REPO`), the
  website's download and links, the plug-ins' and the host's URLs, `PRIVACY.md`, the issue
  templates, the manual, the notices, the installer.
- **Each workflow runs where it belongs**: the website's deploy only in the private
  repository; CI, the release, the null hashes, the audit, macOS and the perf runs only in
  the public one, where GitHub's minutes are free.
- Never add the public remote to the private repository: only `publish.sh`'s clone holds it.

