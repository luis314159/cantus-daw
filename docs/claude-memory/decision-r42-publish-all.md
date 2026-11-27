---
name: decision-r42-publish-all
description: "Owner 2026-11-26 asked me to publish everything once round 42 is done — beta.18 release, public repo, site (plug-ins pages)"
metadata:
  node_type: memory
  type: project
  originSessionId: 46310d93-8b9d-4995-a1a6-07dcc8fb61ce
  modified: 2026-11-27T04:49:34.803Z
---

On 2026-11-26 the owner wrote: "En cuanto termines publica todo please." The context was the site's new Plug-ins pages (r42/site-plugins) and round 42's close.

**How to apply:** when round 42 is finished, publish. "Finished" means: every branch merged into r42/integration, the Windows suite and the WSL Linux check green, and STATUS written. Publishing means:
1. merge to private master and push;
2. release 0.1.0-beta.18 by docs/RELEASING.md: a hand-run rehearsal of release.yml on the three OSes that is green, then the preflight file, then the tag;
3. publish the public repo with tools/public/publish.sh;
4. deploy the site with the Plug-ins section live.

The release notes must carry the manual-update paragraph for Windows users on beta.16 and beta.17 (drafted in docs/round42/fix-update-dim.md).

This authorises this one round's publication, not later ones. If a gate fails, fix it or report it; never skip it.

Related: [[round-42]], [[feedback-release-lessons]], [[public-repo-cantus-daw]].
