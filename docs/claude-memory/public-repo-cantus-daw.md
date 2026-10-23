---
name: public-repo-cantus-daw
description: Since 2026-10-20 the public repo is luis314159/cantus-daw (rewritten history via tools/public/publish.sh); the private luis314159/cantus stays the working repo and deploys the site; never push private refs to the public remote
metadata:
  node_type: memory
  type: project
  originSessionId: 986410ca-b4ef-4425-9ddc-1112fd5eda06
  modified: 2026-10-21T02:18:24.063Z
---

On 2026-10-20 (round 29) the owner created `luis314159/cantus-daw` (public) and asked to
publish. Done: private master 56a9105 → public master c85893c (580 commits, all identities
mailmapped to the GitHub no-reply address, no screenshots, no RVR1960 es.txt, 33.8 MiB),
`check.sh` READY, gitleaks clean. The rewrite is deterministic (two runs gave c85893c).

How it works now (ADR 0049 amended 2026-10-20):
- Work stays in the private `luis314159/cantus` (F:\projects\cantus, origin). It deploys
  cantusdaw.com (site.yml guarded to that repo). CI, release, null hashes, audit, macOS and
  perf workflows are guarded to run only in cantus-daw (free minutes there).
- To publish: `GITLEAKS=F:/projects/daw-r29/pubtools/gl/gitleaks.exe bash
  tools/public/publish.sh F:/projects/daw-r26/private --push` — fresh clone, filter-repo
  (pip-installed 2.47.0 by hash), check.sh, fast-forward-only push. Releases are tags
  pushed to cantus-daw (from the publish clone), never to the private repo.
- NEVER add the public remote to F:\projects\cantus or push private refs there.
- The owner's private folder (identities.mailmap, forbid.txt) is F:\projects\daw-r26\private
  — keep it when cleaning round folders.
- New research screenshots: `git add -f` in the private repo only (ignored by .gitignore).

See [[round-29]], [[decision-round30-platforms]].

2026-10-22: the first public release, **v0.1.0-beta.1**, is out on cantus-daw (lightweight
tag pushed from the publish clone after `publish.sh --push`; the Release workflow tested,
packaged and published it as "latest"). The website's button
(`releases/latest/download/Cantus-windows-x64-setup.exe`, web/src/top.rs; the site is a
WASM app, so the link is not in the HTML) serves the 116 MB unsigned installer. A tag is
never annotated here: the machine's git e-mail is a personal one. publish.sh deletes and
re-clones the publish folder each run; the tag lives on the remote.

2026-10-22 (later): **v0.1.0-beta.2** published (latest) — the kit windows' 0×0 egui area
fix (Settings could not scroll to Plug-ins; also Appearance, Sound packs, Updates, Routing)
and the manual's plug-in section. The first release run failed on a Chorda test race (fault
cleared after the lock was let go); re-running the failed jobs via the REST API
(`POST /actions/runs/<id>/rerun-failed-jobs`, the stored git credential) published it.
Round 30 (another session, cantus-5a, worktrees F:\projects\daw-r30) must release as
beta.3+; it shares the WSL clone ~/cantus-ci — use a separate clone for checks.
