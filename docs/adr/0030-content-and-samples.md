# ADR 0030 — Content and samples: a manifest in the repo, packs on demand

**Status:** accepted (round 20, samples builder; the owner rules on the open points below) · **Date:** 2026-10-05

## Context

The owner (2026-10-05): *"Where could we get rights-free WAV files, so users have sound
options — a drum kit, kicks, snares and so on — and where should they be kept? Not in the
repo, I think."* Then: *"Put an agent on the WAVs right away."*

ADR 0001 says bundled samples must be original or CC0. Sample libraries weigh hundreds of
megabytes to gigabytes. A Git repository is the wrong place for them: every clone would
carry every version forever, and Git LFS has bandwidth quotas. Research for this round
(`research/content/samples.md`, `storage.md`, on `r20/research`) reached the same
conclusion and recommends a download-on-demand content manager.

## Decision

**1. The manifest is in the repo; the sounds are not.** `crates/cantus-content/manifest/content.json`
(format 1, about 250 KB) lists every pack. Each entry has:

- an id and a version;
- the name and description in English and Spanish;
- a category;
- the licence: its SPDX id, the URL of its canonical text, and the path of the licence
  file once installed;
- credits: who made it, the upstream page, and the upstream licence file at the version
  taken;
- the date the licence was read;
- the installed size;
- the source.

The manifest is compiled into the binary, so nothing a server sends can change what a
pack is supposed to contain. A newer `manifest.json` saved beside the content wins if its
`updated` date is later. That is a hook for updating the list without a release; nothing
fetches one yet (see the open points).

**2. Two kinds of source.**

- **Files** are individual files at pinned, content-addressed URLs: raw GitHub files at a
  commit SHA, so the bytes can never change under the hash. Each file has its own size
  and SHA-256. This is how a **curated subset** of a large library ships without
  downloading the library. The starter kit is 81 one-shots, 33 MB, from three CC0
  libraries. Downloading all of Virtuosity Drums would have cost 1.2 GB.
- **Zip** is one archive (a GitHub release asset) with its size and SHA-256. It has a
  `strip` prefix, and `include`/`exclude` patterns applied **after** download. Swirly
  Drums keeps its 341 MB of dry samples out of 1.8 GB; the Karoryfer packs leave out
  their GUI images, PDFs and player presets.

**3. Where packs go: `<data dir>/Content/<pack>/`.** On Windows that is
`%LOCALAPPDATA%\Cantus\Content`, the same folder as `settings.json` (`app::data_dir`).
An install works like this:

1. Download into `.downloads/<sha256>.part`. An interrupted download resumes with an HTTP
   `Range` request. A cancelled or failed one keeps its part file for the next try.
2. Check the size and the SHA-256. Wrong bytes are deleted, never kept for a resume.
3. Unpack into `.staging/<pack>/`. Entry names that would leave the folder are refused
   (zip-slip).
4. Require the licence file named by the manifest. A pack without it is not installed.
5. Write a receipt, `cantus-pack.json`, with the version, licence, credits and counts.
6. Rename the folder into place. An update moves the old version to `.trash` first, so a
   pack folder is always either complete or absent.

Removing a pack renames its folder into `.trash` before deleting it. Installs run on
worker threads (`Manager`). The interface polls once a frame and never waits.

**4. The network, behind a corporate proxy.**

- HTTP uses `ureq` 3: blocking, small, MIT/Apache-2.0.
- TLS uses `rustls` with `rustls-platform-verifier`, so certificates are checked against
  the operating system's trust store. A TLS-inspecting proxy (Zscaler here) re-signs with
  a root the administrator installed in Windows, and Cantus trusts it exactly as the
  browser does. There is no switch to turn verification off, and none will be added.
- Proxies set in `HTTPS_PROXY`/`HTTP_PROXY`/`NO_PROXY` are honoured.
- A filter's HTML page is treated as a block, not as the file. That covers a body declared
  `text/html` and bytes that start with `<!doctype`, `<html` or `<head`. Such a page is
  never written into the library. It is retried with a growing wait (15 s up to 2 min),
  because Zscaler holds a file for up to about ten minutes while it scans it.
- Network errors and 5xx/429 answers are retried with backoff. A 404 is not.
- An install gives up after 12 attempts or 12 minutes of waiting.
- **Install from a file…** takes an archive the user already has (offline, or fetched by
  another route) through exactly the same size, hash and licence checks.

The zip side uses `zip` 8 with deflate through `flate2`'s pure-Rust backend
(`miniz_oxide`, already in the tree). `sha2` was already a workspace dependency.

**5. Default content is CC0 only.** *(Amended by ADR 0042, 2026-10-10: the owner accepted
CC BY 3.0/4.0 with the credit shown; NC, ND and SA stay out.)* Every pack in the manifest is `CC0-1.0`, and a test
fails otherwise. CC-BY material (Salamander Grand, Naked Drums, the DrumGizmo kits, and
Dustyroom's Fake Acoustic Drum Kit, whose page now says CC BY 4.0) stays out until the
owner rules on it. So does anything whose licence texts disagree: Karoryfer's *The Hat
with the Phat* is CC0 in its repository but "GPL 3" on its shop page.

**6. The browser** is a dock pane (`Pane::Browser`, at the left as in FL, Alt+F8, and the
first of the toolbar's window keys). It lists the installed packs, then the user's own
folders, as a tree, and searches an index that is built on a worker thread.

- **A click auditions the sample.** A decoder thread decodes the file. The engine's
  `Preview` voice plays it once, after the master strip, like the click: the master fader
  does not turn it down and the recorder never hears it. The sample arrives in an `Arc`,
  and the voice hands back whatever it lets go of as `EngineEvent::GarbagePreview`, so
  nothing is allocated or freed on the audio thread (`tests/preview_rt.rs`).
- **A drag lands where the views say it can.** Each view that can take a sample publishes
  a target while it draws: a Pulsus pad, Exemplar's key map, the channel rack, the
  arrangement. A release is resolved at the start of the next frame against everything
  the previous frame drew, and the target drawn last (the one on top) wins.
  - On a pad, the pad plays the file.
  - On Exemplar's keys, the file becomes a zone on the key under the pointer.
  - On the rack, the file becomes a new sample channel (a one-pad Pulsus with its own
    strip). On a one-pad channel it replaces that channel's sample.
  - On the arrangement, the file becomes an audio clip, through round 19's import.

  Each of these is one undo step.
- **A one-click starter.** While nothing is installed, the browser offers the recommended
  pack.
- **Sound packs…** lists every pack with its sizes, licence, credits and state. It
  installs, updates, removes (after a confirmation), cancels, shows the folder, and
  installs from a file.

## Trade-offs, written down

- **Upstream URLs vs our own mirror.** The manifest points at the authors' own hosting
  (GitHub release assets and pinned raw files). That costs nothing and keeps provenance
  obvious. The price is that an author can delete a release; the hash then makes the
  install fail cleanly rather than install something else. The research recommends
  GitHub Releases on a `cantus-content` repository as the primary host, with Cloudflare R2
  later. A mirror is an extra URL per entry, which a format 2 can add (`mirrors`).
- **Many small requests vs one archive.** The starter is 81 requests. They resume one by
  one, and a quarantined file holds up only itself. One archive would be a single request
  but would have to be built and hosted by us.
- **Subset before or after download.** File lists take the subset before downloading:
  small, but the list is long and lives in the manifest. Zip filters take it after: the
  manifest stays short, but the whole archive is downloaded. The starter uses the first;
  the large libraries use the second.
- **FLAC and WAV.** Some packs are FLAC (lossless, about half the size). Every Cantus path
  decodes WAV, AIFF, FLAC and Ogg, so the browser lists all four.
- **Signing.** Integrity now comes from SHA-256 in a manifest compiled into the binary.
  A manifest fetched from a server would need a signature (minisign, Ed25519) before it
  could be trusted. That is deferred with the remote manifest itself.
- **References from projects.** A pad or a zone stores the file's absolute path under
  `Content/`, as for any other file. A portable `content://<pack>/<path>` reference that
  survives a moved library is a schema change. It is left for the round that also brings
  "missing content: download it" on project load.

## Consequences

- No sample lives in the repository. CI never downloads.
- The default tests run without a network: `file://` fixtures and a loopback HTTP server
  cover install, a corrupted file, a proxy page, resume, cancel and removal. One ignored
  test, `CANTUS_CONTENT_NET=1 cargo test -p cantus-content -- --ignored`, downloads the
  starter for real and checks every hash.
- Adding a pack is a manifest edit made with a script that downloads and hashes. Sizes
  and hashes are never typed by hand (`docs/round20/samples.md`).

## Open points for the owner

1. **CC-BY:** may Cantus offer CC-BY packs as opt-in downloads, with the licence shown and
   a credits view? The candidates are Salamander Grand, Naked Drums and the DrumGizmo
   kits.
2. **Hosting:** keep pointing at upstream, or mirror every pack on a `cantus-content`
   GitHub repository (and sign its manifest)?
3. **Electronic drums:** there is no clean CC0 source of 808/909-style one-shots beyond
   the small AFR kicks pack. The research recommends rendering Pulsus presets to WAV and
   releasing them as CC0. Rendering them on the user's machine as a "pack" needs no
   hosting at all.
4. **First run:** should the browser be visible in the default layout, and should first
   run offer the starter kit before the user opens it?
