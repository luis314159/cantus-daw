# Where Cantus keeps its sample content, and how the app gets it

Research note, 2026-10-05. Scope: rights-free drums, loops and multisampled
instruments, from a ~100 MB core kit up to several GB of optional packs
(VSCO 2 CE, VCSL, Salamander piano, ...). None of it lives in the source repo.
Research only: no code. Every fact carries a source at the end. Anything
marked **not verified** comes from memory or from a second-hand page and must be
re-checked before a decision relies on it.

Caveat on method: the corporate proxy (Zscaler) blocks backblaze.com outright
("FileHost" category), so the Backblaze figures come from search snippets of
Backblaze's own blog and price pages, not from a direct read. Hetzner's page
renders its prices with JavaScript; its figures are from a 2026 third-party
comparison. Everything else below was read from the vendor's own page on the
date given.

---

## 1. Summary (for readers in a hurry)

- **Do not use Git LFS** for content: on GitHub Free it gives 10 GiB storage and
  10 GiB bandwidth per month, then $0.07/GiB storage and $0.0875/GiB transfer, and
  every fork/clone of the content counts against *our* quota. One thousand
  downloads of a 500 MB library would cost about $40/month.
- **Phase 1 (now): GitHub Releases on a separate `cantus-content` repo**, one
  release per pack version, plus a signed `manifest.json` attached to a
  `manifest` release. GitHub states: up to 1000 assets per release, each file
  under 2 GiB, "no limit on the total size of a release, nor bandwidth usage".
  Cost: $0. Risk: the Acceptable Use Policy lets GitHub throttle "significantly
  excessive" bandwidth.
- **Phase 2 (when downloads grow, or as an early mirror): Cloudflare R2** with a
  custom domain. $0.015/GB-month storage, 10 GB-month free, and **no egress fee
  at all**. For our sizes the bill is roughly $0 to $1 a month even at 100k
  downloads of 500 MB a month (only per-request charges, mostly inside the free
  tier).
- **The app** gets a content manager that downloads on demand. It reads a
  manifest signed with minisign/Ed25519, checks a SHA-256 for every file, resumes
  downloads with HTTP Range requests, tries mirrors in order, lets the user put
  the library on any drive, accepts offline zips, sends no telemetry and needs no
  account.
- **Installer budget**: about 100 to 150 MB of core kit inside the installer.
  For comparison: REAPER is 17 MB (no content), MuseScore's MSI is 129 MB (with a
  51 MB SoundFont), Surge XT is 183 MB (factory content included) and FL Studio
  is 1.03 GB. Ableton puts most of its 71 GB Suite content in downloadable Packs.

---

## 2. Hosting options compared

### 2.1 Git LFS on GitHub — reject

| Item | Value (seen 2026-10-05) |
|---|---|
| Free quota, GitHub Free / Pro / Free org | 10 GiB storage + 10 GiB bandwidth per month |
| Free quota, Team / Enterprise Cloud | 250 GiB + 250 GiB |
| Overage | $0.07 per GiB-month storage, $0.0875 per GiB transfer (pricing calculator) |
| Billing model | Pre-paid **data packs have been removed**, replaced by metered billing |
| Over quota with no payment method | Clones still work but only fetch pointer files; over the bandwidth quota "Git LFS support is disabled on your account until the next month" |
| Forks | "Bandwidth and storage usage always count against the repository owner's account. Forking and pulling a repository counts against the parent repository's bandwidth usage." |
| Source archives | If LFS objects are included in source archives, downloads of those archives count as bandwidth |

Why it is wrong for end users: LFS is a developer tool. A musician should not
need `git` + `git-lfs` to get drum samples. Every CI run, fork and contributor
clone burns the same quota the users depend on. When the quota runs out without
a card on file, downloads stop serving content and serve pointer files
instead, which is a silent failure for users. And the source repo would grow
heavy with history. LFS could at most hold *test fixtures*, and those
should stay small enough not to need it.

### 2.2 GitHub Releases assets — phase-1 choice

| Item | Value (seen 2026-10-05) |
|---|---|
| Assets per release | up to 1000 |
| Size per file | "must be under 2 GiB" |
| Total size / bandwidth | "There is no limit on the total size of a release, nor bandwidth usage." (About releases); "We don't limit the total size of the binary files in the release or the bandwidth used to deliver them" (About large files) |
| Policy limit | AUP section 9 "Excessive Bandwidth Use": if usage is "significantly excessive in relation to other users of similar features", GitHub may "suspend your Account, throttle your file hosting, or otherwise limit your activity"; after advance notice it may delete repositories placing undue strain |
| Download statistics | Releases API gives `download_count` per asset |
| URL stability | `https://github.com/<owner>/<repo>/releases/download/<tag>/<file>` stays stable for as long as the tag and asset exist. It redirects to a signed CDN URL, so clients must follow redirects. Re-uploading an asset with the same name keeps the URL. The exact redirect host is **not verified** and must not be hard-coded. |

Who does this: MuseScore Studio 4.7.5 ships its installers as release assets
(MSI 128.7 MB, DMG 207.8 MB, AppImages ~190 MB), each with a
`checksums.sha256.txt` and `.zsync` files for delta updates. Surge XT 1.3.4
(setup.exe 183.4 MB), LMMS 1.3.0-alpha.2 (Windows exe 129.6 MB) and many more do
the same. Surge XT 1.3.4 assets show thousands of downloads each (10,459 for the Linux plugins-only archive) per
release, with no throttling reported that we found (counts read from the API).

Fit for Cantus: every pack we plan to host is under 2 GiB if it is split
sensibly. VSCO 2 CE at about 3 GB and VCSL at about 5 GB need 2 or 3 archive
parts, or per-instrument sub-packs, which is better UX anyway. It is free and
already where the project lives. The weak points are the AUP clause and the
fact that a GitHub outage or a GitHub block (some countries, some corporate
networks) blocks content. Mirrors cover both.

### 2.3 Separate content repo vs per-pack repos

- **`cantus-content` (one repo, releases per pack)**: the git tree holds only
  small text (pack definitions such as `.sfz`/`.cantuskit` mappings, licence
  files, credits, the manifest source, CI scripts). The audio itself sits only
  in release assets. Tags follow the pattern `pack/<pack-id>/v<semver>`, plus a
  rolling `manifest` release. This keeps the source repo clean and gives one
  place for issues and licence review.
- **Per-pack repos (the sfzinstruments model)**: sfzinstruments.github.io keeps
  one GitHub repo per instrument (about 40 repos: `SalamanderGrandPiano` 731 MB,
  `WilkinsonAudio.NakedDrums` 1.25 GB, `DrumGizmo.DRSKit` 736 MB, ...). They
  commit the **FLAC samples into plain git, without LFS** (`.gitattributes`
  only marks `*.flac binary`), and users download the repo zip. VSCO 2 CE
  (repo 2.3 GB) and VCSL (repo 4.0 GB) by Sam Gossner do the same. This works
  for a community library, but full history and source archives grow without
  limit. It also gives no checksums or versions an app can use, and a
  `codeload` zip of a 4 GB repo can be slow or fail. **Not verified**: the size
  limits on GitHub's on-the-fly archive generation.
- Recommendation: one `cantus-content` repo. **Upstream** packs (VSCO, VCSL,
  Salamander, NakedDrums) are not re-hosted at first. Our manifest points at
  their own URLs (GitHub archives, archive.org) with our SHA-256 values, and
  our own repackaged FLAC/zip goes up as a second mirror. That respects
  attribution and lowers our bandwidth. The catch: upstream URLs can change, so
  our copy is the fallback.

### 2.4 Object storage / CDN (price per month, seen 2026-10-05)

| Provider | Storage | Egress to Internet | Requests | Free tier / minimum |
|---|---|---|---|---|
| **Cloudflare R2** (Standard) | $0.015 / GB-month | **Free** (via S3 API, Workers, r2.dev or custom domain) | Class A $4.50 / M; Class B $0.36 / M | 10 GB-month, 1 M Class A, 10 M Class B free every month |
| **Backblaze B2** | $6.95 / TB-month (raised from $6, effective 1 May 2026; announced March 2026) | Free up to 3x average stored data, then $0.01/GB (**not verified directly**, proxy blocks site); **$0 to CDN partners Cloudflare, Fastly, bunny.net** | API calls free from 1 May 2026 | first 10 GB free (**not verified**) |
| **AWS S3 + CloudFront**, pay-as-you-go | S3 Standard ~$0.023 / GB (**not verified**, the page renders prices with JS) | CloudFront ~$0.085/GB for the first 10 TB in NA/EU (secondary source) | extra | AWS free tier (not verified) |
| **AWS CloudFront flat-rate plans** (since Nov 2025) | includes S3 storage credits | Free: 100 GB/mo; **Pro $15/mo: 50 TB**; Business $200/mo: 50 TB; Premium $1,000/mo | 1 M / 10 M / 125 M / 500 M requests | "no overage charges"; whether bulk large-file distribution fits the plan's intended use: **not verified** |
| **Bunny Storage + Bunny CDN** | $0.01/GB (1 region HDD), $0.02 for 2 regions | Standard network: $0.01/GB EU/NA, $0.03 Asia/Oceania, $0.045 S. America, $0.06 ME/Africa; Volume network: $0.005/GB (first 500 TB) | none for storage API | $1/month minimum; storage→CDN traffic free |
| **DigitalOcean Spaces** | $5/mo includes 250 GiB, then $0.02/GiB | 1 TiB included, then $0.01/GiB | included | $5/month base |
| **Hetzner Object Storage** | €6.49/mo base includes 1 TB storage + 1 TB egress; extra storage ~€8.70/TB-month | extra egress ~€1/TB | S3 API calls free | 3 EU locations (FSN1, NBG1, HEL1); figures from a third-party July 2026 comparison, **not verified** on hetzner.com |

Notes:

- R2's `r2.dev` subdomain is meant for development and is rate-limited.
  Production should use a custom domain on a Cloudflare zone (**not verified**
  this pass; check R2 "public buckets" docs).
- Cloudflare's CDN terms on serving large non-HTML files through the regular
  proxied CDN (as opposed to R2) must be re-read before choosing
  "B2 behind Cloudflare". Content served straight from R2 is the documented,
  supported path (**not verified** this pass).
- Bunny has the best paid CDN price if R2 ever stops being free on egress.

### 2.5 Cost scenarios

Assumptions: 10 GB stored in total (all packs). The "factory library" is 500 MB
and gets 1k, 10k or 100k downloads/month (500 GB, 5 TB, 50 TB of egress). Each
download is ~10 requests (manifest + ranged chunks). USD/EUR, before tax, rounded.

| Option | 1k dl/mo (0.5 TB) | 10k dl/mo (5 TB) | 100k dl/mo (50 TB) |
|---|---|---|---|
| GitHub Releases | $0 | $0 | $0 (AUP risk grows) |
| GitHub LFS (Free plan) | ~$40 | ~$407 | ~$4,070 |
| **Cloudflare R2** | **$0** (inside free tier) | **$0** (100k Class B ops) | **~$0** (1 M Class B ops, inside 10 M free) |
| B2 direct | ~$5 | ~$50 | ~$500 |
| B2 behind Cloudflare/Bunny (partner egress $0) | ~$0.07 + CDN cost | same | same (Bunny CDN would add $250–500) |
| S3 + CloudFront pay-as-you-go | ~$0.25 (inside 1 TB free tier, not verified) | ~$340 | ~$4,000 |
| CloudFront flat-rate Pro | $15 | $15 | $15 (50 TB cap; fit not verified) |
| Bunny Storage + CDN (Volume / Standard EU-NA) | $3 / $6 | $26 / $51 | $251 / $501 |
| DigitalOcean Spaces | $5 | ~$41 | ~$460 |
| Hetzner Object Storage | €6.49 | ~€10.50 | ~€55.50 |

R2 wins on cost at every scale we can foresee. It is the only option where cost
does not grow with popularity, which matters for a free project that cannot
charge its users. Hetzner is the cheapest *metered* option but serves only
from the EU, without a CDN.

### 2.6 Free hosting aimed at open source

| Service | What it offers | Fit |
|---|---|---|
| **SourceForge** | Still offers free project hosting with file releases, "unlimited bandwidth for Open Source projects" and a worldwide mirror network. It can also mirror GitHub release downloads (search snippets of sourceforge.net/create and its blog; the page itself was behind a bot check, **partly verified**) | Good free **mirror** for packs: add as fallback URL. Mirror redirects + ads on its HTML pages, so the app should use direct file URLs only. |
| **Internet Archive (archive.org)** | Free permanent storage. The Salamander Grand Piano original is hosted there. Upload terms and speed limits: **not verified** | Good long-term **archival mirror** and the canonical source for some upstream packs. Slow to download. |
| **Fastly Fast Forward** | "we extend free services to eligible open source projects and nonprofits" (fastly.com/fast-forward) | Apply once Cantus has users. Possible CDN in front of B2/R2. |
| **Cloudflare Project Alexandria** | Recurring annual credits for OSS projects (Pro/Business plans, Workers, more R2 storage); requires a recognised OSS licence and a non-profit basis (Cloudflare blog/landing page, from search) | Apply in phase 2. It covers R2 storage beyond the free 10 GB. |
| **OSU Open Source Lab** | FTP mirror cluster for OSS projects (ftp.osuosl.org, e.g. OSGeo downloads). Projects request it through hosting-request@osuosl.org (mailing-list evidence, 2019) | Possible later for a bigger project. Not for us now. Current acceptance policy **not verified**. |
| **Hugging Face Hub** | Public repos: "Best-effort" free storage for free accounts, abuse mitigations, "please use this resource responsibly" (paid tiers up to 10 TB). Files up to 200 GB, <100k files/repo recommended | Works technically, but its purpose is ML models and datasets. Hosting a DAW's factory library there goes against its stated purpose and may be removed. **Do not use** as primary. |
| **Zenodo** | 50 GB per record, max 100 files (one-time increase to 200 GB). Splitting a dataset into records to dodge the limit is not "fair usage" | Meant for research outputs with DOIs. A **citation/archival copy** of a pack at most. Not a download backend. |

### 2.7 BitTorrent / web seeds

A `.torrent` per pack with a BEP 19 web seed (the GitHub or R2 URL) makes
peers optional. Torrent clients can always fall back to HTTP. Big Linux distros
and archive.org (it auto-generates torrents for items, **not verified** this
pass) use this. For Cantus it is an *optional* extra for the largest packs (VCSL,
VSCO): publish `.torrent` files next to the zips and do not build a BitTorrent
client into the app. Corporate networks block BitTorrent almost everywhere, so
it can never be the only path.

---

## 3. How other products do it

| Product | Installer (seen 2026-10-05) | Content model |
|---|---|---|
| **FL Studio 26.1.7** (Image-Line) | Windows 1.03 GB, macOS 1.10 GB; "at least 4 GB of free disk space" | Big all-in-one installer with factory samples and plugins. The browser's **Content Library** shows online content (samples, presets, themes), some free and some paid: "Where content is associated with a plugin, it will be automatically downloaded to the correct location". The **Sounds (FL Cloud)** tab is a hosted sample library (subscription, account) whose items you add to your library or drag into a project. FLEX preset packs download from inside the plugin (FL Cloud page). |
| **Ableton Live 12** | Basic install: up to 6 GB (Windows), 8 to 10 GB (macOS) | Content in **Packs**: Intro 5+ GB, Standard 38+ GB, Suite 71+ GB; "optionally up to 76 GB" disk for extra sound content. Packs install and update from **Live's browser** (Places → Packs → "Available Packs", a download arrow then "Install", plus an updates list). They also come as a `.alp` file from the user account, installed by double-click or drag-and-drop. If Live was **authorised offline, browser downloads are disabled** and manual `.alp` install is the path. The `.alp` can be deleted after install. |
| **Bitwig Studio** | size **not verified** | In-app **Package Manager** (search snippets of bitwig.com; the in-app flow itself **not verified** against the user guide). Packages are listed with sizes on bitwig.com/sound-content: Orchestral Strings 2.02 GB, Woodwinds 2.84 GB, Brass 3.43 GB, Percussion 3.05 GB, Bösendorfer 1.47 GB, small preset packs 5 to 11 MB. Total library "over 20 GB" for Bitwig 6 (secondary source). |
| **REAPER 7.82** | Windows x64 **17 MB**, Linux 13 MB, macOS universal 29 MB | No sound content at all. **ReaPack** (third-party, open source) is a package manager for scripts, JSFX, extensions, themes, language packs. A repository is one **XML index** listing packages, versions, author, time and per-file source URLs. It is decentralised (users add third-party repo URLs), keeps packages up to date in one click, and **exports installed packages to an archive for offline computers**. It has 1,300+ packages. |
| **MuseScore Studio 4.7.5** | Windows MSI **128.7 MB** (GitHub Releases, with `checksums.sha256.txt` and `.zsync`) | **MS Basic.sf3, 51.3 MB**, ships in the installer (`share/sound` in the repo, next to FluidR3Mono_GM.sf3, 23.7 MB). The high-end **Muse Sounds** come from a separate app, **Muse Hub** (Windows/macOS), which also installs MuseScore itself. Extra SoundFonts/SFZ can be dropped into the user `SoundFonts` folder or dragged onto the window. |
| **Surge XT 1.3.4** (OSS) | win64 setup.exe **183.4 MB**; plugins-only zip 49.1 MB; portable zip 301.8 MB | Factory patches and wavetables bundled in the installer. GitHub Releases, with an `md5sum.txt`. |
| **LMMS 1.3.0-alpha.2** (OSS) | Windows x86_64 exe **129.6 MB**; macOS dmg ~42 to 44 MB; AppImage ~154 MB | Bundled samples and presets in the installer (content list **not verified**). GitHub Releases. |
| **Logic Pro** (Apple) | **not verified** | User guide has "Download additional sound packs" and a "Sound Library settings" page. Logic installs a minimal set and downloads the rest of the Sound Library on demand, and the library can be relocated to an external drive (details **not verified** this pass). |
| **GarageBand** | **not verified** | Same Apple Sound Library mechanism: extra sounds download from inside the app (**not verified**). |
| **Native Instruments Native Access** | **not verified** | Separate installer/download manager app with account login, per-product install and update, user-chosen content folder (from memory, **not verified**). |
| **Arturia Software Center** | **not verified** | Separate account-based manager app for installing/activating instruments and sound banks (**not verified**). |
| **Steinberg Download Assistant / Library Manager** | **not verified** | Download Assistant fetches installers and content. Library Manager registers, moves and removes VST Sound content files, including to another drive (**not verified**). |
| **Ardour** | **not verified** | Ships no sound library. Size **not verified**. |
| **Vital** | **not verified** | Factory presets come with an account download. Preset packs are bought or downloaded in-app (**not verified**). |
| **sfzinstruments.github.io** | n/a | One GitHub repo per instrument, FLAC in plain git, users download the repo. See 2.2/2.3. |

Patterns worth copying:

1. **Small core in the installer, the rest on demand.** Even the giants do
   this (Ableton: 6 GB install vs 76 GB of optional content; MuseScore: 51 MB
   SoundFont in, Muse Sounds out).
2. **Downloads in the browser, where users already look** (Ableton Packs,
   FL Content Library), with an updates list. A separate hub app (Muse Hub,
   Native Access) is heavier and needs accounts. We do not want that.
3. **A file you can carry** for offline machines (Ableton `.alp`, ReaPack
   archive export).
4. **A text index** that anyone can host (ReaPack XML) instead of a central
   server. It is decentralised, mirrorable and cheap.

---

## 4. Size budgets

| What | Budget | Contents |
|---|---|---|
| Installer (app + core kit) | **≤ 200 MB total**, core kit **≤ 100–120 MB** (FLAC) | 1 or 2 drum kits (one-shots, ~300 samples), a basic GM-style multisample set at reduced velocity layers (like MS Basic at 51 MB), a dozen loops, demo songs that use **only** the core kit. Every demo must open offline. |
| "Factory library" (first-run prompt, optional) | ~300–600 MB | More kits, loops by genre, a mid-size piano (Osiris, CC0; a trimmed Salamander only if the owner admits CC-BY), bass, EP, strings. |
| Optional packs | each ≤ 2 GiB per file (GitHub limit), split by instrument family | VSCO 2 CE (~3 GB, split by section), VCSL (~5 GB, split), full Salamander (~730 MB), NakedDrums (~1.25 GB), ... |

Benchmark: REAPER 17 MB < LMMS 130 MB ≈ MuseScore 129 MB < Surge XT 183 MB <
**Cantus ≤ 200 MB** < FL Studio 1.03 GB < Ableton basic 6 GB.
A 200 MB installer still downloads in under a minute on a typical broadband
line, and it stays under GitHub's 2 GiB asset limit by a wide margin.

**Formats.** Store samples as **FLAC** (lossless). For acoustic instrument
samples FLAC is commonly cited at about 50 to 60 % of WAV size (**not
verified** on our own material; measure it on the core kit before committing
to budgets). Distribute packs as **`.zip` with "stored" FLAC entries**
(zip deflate gains almost nothing on FLAC, and stored entries allow
range-reads of single files). Text files (`.sfz`, mappings, JSON) can be
deflated. zstd brings nothing extra for already-compressed FLAC. Keep it for a
possible single-file "pack bundle" of text and small files. WAV is used only
for cached decoded data on the user's disk, if the sampler needs it at all.

---

## 5. Content manager design (download on demand)

### 5.1 User flow

1. **First run**: a dialog says "Cantus comes with a small core kit. Download the
   factory library (≈ 450 MB) now?" It offers *Download*, *Later* and
   *Choose folder…*. Skipping is fine: everything shipped in the installer
   works offline. The prompt can be reopened from the Content window.
2. **Content window** (also a "Packs" node in the browser, as in Ableton). Each
   pack shows name, author, licence badge, size on disk / download size,
   installed version, available version, and the buttons *Install*, *Update*,
   *Remove*, *Show in browser*. It also has an *Updates available (n)* filter.
   Nothing updates silently.
3. **Before install**: a licence and credits sheet (licence name + SPDX id,
   attribution text, source URL). The user confirms once per pack. Credits are
   also collected in Help → Credits.
4. **During download**: per-pack progress, pause/resume, cancel. Several packs
   download in a queue, 2 at a time.
5. **Offline / air-gapped**: *Install from file…* accepts the pack zip (the same
   asset the app would download) plus, optionally, `manifest.json` +
   `manifest.json.minisig`. Drag-and-drop onto the window works too. If the
   manifest is missing, the zip carries its own `pack.json` with hashes. The
   app verifies the hashes and marks the pack "unsigned (installed from file)"
   unless a signed manifest is present.
6. **Library location**: default `%LOCALAPPDATA%\Cantus\Library` (Windows),
   changeable to any drive. *Move library…* copies, verifies, then switches.
   Several library roots are allowed (e.g. a read-only network share plus a
   user folder).

### 5.2 Manifest (signed index, ReaPack-like but JSON)

One `manifest.json` lists all packs. It is published as a release asset of
`cantus-content` (tag `manifest`) **and** on every mirror, with a detached
`manifest.json.minisig`. Sketch of the fields (the design, not a frozen schema):

```json
{
  "schema": 1,
  "generated": "2026-10-05T00:00:00Z",
  "sequence": 42,
  "mirrors": ["https://github.com/<org>/cantus-content/releases/download/",
              "https://content.<project-domain>/",
              "https://downloads.sourceforge.net/project/cantus-content/"],
  "packs": [{
    "id": "cantus.core-drums", "version": "1.2.0", "min_app": "0.20.0",
    "title": {"en": "Core Drums", "es": "Batería básica"},
    "licence": "CC0-1.0", "attribution": "...", "source_url": "...",
    "size_download": 41234567, "size_installed": 41234567,
    "archives": [{"path": "pack/cantus.core-drums/v1.2.0/core-drums-1.2.0.zip",
                  "size": 41234567, "sha256": "..."}],
    "files": [{"path": "kick/kick-01.flac", "size": 123456, "sha256": "..."}],
    "replaces": [], "deps": []
  }]
}
```

- `sequence` only increases. The client refuses a manifest with a lower
  sequence than the last one it accepted (rollback protection).
- Mirror URLs are *bases*. The relative `path` is the same everywhere, so any
  mirror can serve any file and the hash decides whether it is good. (GitHub
  release URLs are `<base><tag>/<file>`, so the path layout has to map onto
  tag + file name. The CI must generate both from one table.)
- Titles carry both locales (en/es), following the project's both-locales rule.

### 5.3 Integrity and signing

- **Ed25519 via minisign**: the maintainers keep the secret key offline (or in
  a CI secret with a protected environment). The public key is compiled into
  the app, with room for a second key for rotation. The app verifies
  `manifest.json.minisig` before parsing anything else.
- **SHA-256 for every archive and every file**: archives are checked after
  download. Files are checked after extraction, and again by an optional
  "Verify library" command.
- Downloads go to `*.part` files in the library's `.staging` folder. The
  install is an atomic rename of the pack directory, so a crash never leaves a
  half-installed pack.

### 5.4 Network behaviour

- **Resumable downloads**: HTTP `Range: bytes=N-` against the `.part` length,
  checked with `If-Range` / ETag. That GitHub's release CDN honours ranges is
  **not verified**; test it. R2 supports ranges.
- **Mirrors and fallback**: the client tries mirrors in manifest order, moves
  to the next on a timeout, a 4xx/5xx, a hash mismatch or an HTML
  interstitial, and remembers the fastest mirror for the session.
- **Corporate proxies (Zscaler and the like)**:
  - honour `HTTPS_PROXY` / `HTTP_PROXY` / `NO_PROXY`. reqwest reads these env
    vars. How it picks up the Windows system proxy (WinINET/PAC) is **not
    verified**; expose a manual proxy field as a fallback;
  - trust the **OS certificate store** (Zscaler re-signs TLS with a corporate
    root installed in the Windows store). Use `rustls` + `rustls-native-certs`
    or a platform verifier, or `native-tls` (SChannel). A bundled webpki root
    list alone **fails** behind Zscaler;
  - **detect interstitials**: if a binary download comes back with
    `Content-Type: text/html`, or its first bytes are `<!DOCTYPE`/`<html`, or
    its length does not match the manifest, treat it as a blocked response.
    Say so in plain words ("Your network blocked the download; try *Install
    from file*") and never write it into the library;
  - never disable certificate checks, and offer no "ignore TLS errors" switch.
- **Privacy**: no account, no telemetry, no unique IDs. The only requests are
  the manifest GET and file GETs, with a plain `User-Agent: Cantus/<version>`.
  The manifest check runs at start-up at most once a day and can be turned off
  ("Check for content updates" setting). Hosts (GitHub, Cloudflare) will see
  IP addresses in their logs; the privacy note must say so.

### 5.5 How projects reference content

- In the project file a sample reference is `content://<pack-id>/<path>` plus
  `pack_version` and the file's `sha256`. User samples keep absolute and
  relative paths as today.
- On load: resolve the reference against installed packs (any version with the
  same file hash counts as a match). If it is missing, the **Missing content**
  dialog lists the packs needed (with sizes, *Download all*, *Locate…*,
  *Skip/silence*). Locate searches by file name + hash in a folder the user
  picks, which fits manually copied libraries.
- Pack updates must never change the audio behind an existing path silently. A
  changed sample gets a new path or a new pack major version, and old versions
  stay downloadable from the release history, so old projects still sound
  the same.
- **Deduplication**: files with equal SHA-256 across packs or versions are
  stored once (hard links where the file system allows, else a
  content-addressed `objects/` store with per-pack views). Optional; phase 2.
- Adding `content://` references is a **project schema bump** with a fixture
  and migration, following the project's habit.

### 5.6 Rust crates (licences read from the crates.io API, 2026-10-05)

| Need | Crate | Licence |
|---|---|---|
| HTTP client, proxies, ranges | `reqwest` | MIT OR Apache-2.0 |
| OS trust store for rustls | `rustls-native-certs` | Apache-2.0 OR ISC OR MIT |
| OS TLS (SChannel / Security.framework / OpenSSL) | `native-tls` | MIT OR Apache-2.0 |
| SHA-256 | `sha2` | MIT OR Apache-2.0 |
| minisign signature check | `minisign-verify` | MIT |
| raw Ed25519 (alternative) | `ed25519-dalek` | BSD-3-Clause |
| zip extraction | `zip` | MIT |
| FLAC decoding | `claxon` | Apache-2.0 |
| FLAC decoding (alternative) | `symphonia-bundle-flac` | MPL-2.0 |
| deflate | `flate2` | MIT OR Apache-2.0 |
| zstd | `zstd` | API's first licence field read BSD-3-Clause; **re-check** (the crate is commonly listed as MIT) |

All of these are compatible with GPL-3.0-or-later (MPL-2.0 is compatible at
file level). `rustls-platform-verifier`, which uses the OS verifier directly,
is worth evaluating; its licence was **not verified**.

---

## 6. Recommended design

**Primary: GitHub Releases on `cantus-content` + signed manifest + in-app
content manager. First mirror once traffic grows: Cloudflare R2 on a custom
domain. Free archival mirror: SourceForge file releases (and archive.org for
upstream packs).**

### Phase 1 — now (cost $0)

- Core kit (≤ 120 MB FLAC) inside the installer. Demo songs use only it.
- New repo `cantus-content`: pack sources (mappings, licences, credits, build
  script) and CI that builds the zips, computes SHA-256, writes
  `manifest.json`, signs it with minisign and uploads the assets to releases
  (`pack/<id>/v<x.y.z>`, rolling `manifest`).
- App: content manager per section 5 with the GitHub mirror only, plus
  *Install from file*.
- Upstream packs (VSCO 2 CE, VCSL, Salamander): manifest entries list the
  upstream URL first and our repackaged copy second.

### Phase 2 — when monthly downloads pass ~5k, or GitHub sends a warning (cost ~$0–5/month)

- Add R2 (bucket + custom domain) as mirror #1, filled by the same CI. GitHub
  stays mirror #2. Apply to Cloudflare Project Alexandria and Fastly Fast
  Forward.
- Add SourceForge file releases as mirror #3 (free, OSS-friendly).
- Deduplicated store; optional `.torrent` + web seed for packs > 1 GB.

### Phase 3 — only if R2's terms or prices change

- Move to Bunny (Volume network, ~$0.005/GB) or B2 behind a free-egress CDN
  partner. The mirror list sits in the signed manifest, so this is a data
  change, not an app release.

### Estimated monthly cost (500 MB factory library, 10 GB stored)

| Downloads/month | Phase 1 (GitHub only) | Phase 2 (R2 + GitHub + SF) | Phase 3 worst case (Bunny Volume) |
|---|---|---|---|
| 1k | $0 | $0 (+ domain, ~$10–15/year) | ~$3 |
| 10k | $0 (AUP risk) | ~$0–1 | ~$26 |
| 100k | not advisable alone | ~$0–5 (requests beyond the free tier only if downloads are heavily chunked) | ~$251 |

### Risks

| Risk | Mitigation |
|---|---|
| GitHub throttles under the AUP "excessive bandwidth" clause | R2 as mirror #1 from phase 2; mirrors are data in the manifest, not code |
| R2's free egress changes | Signed mirror list; Bunny/B2 ready as plan B |
| Upstream URL rot (VSCO, VCSL, archive.org) | Our repackaged copy as a second URL; the hash pins the exact bytes |
| Corporate proxy blocks GitHub/R2 or rewrites TLS | OS trust store, proxy env vars + manual field, interstitial detection, *Install from file* |
| Signing key compromised or lost | Two public keys embedded; key kept offline; `sequence` rollback check |
| Licence problem in a pack | Licence review in `cantus-content` PRs; licence + credits sheet before install; host only what ADR 0001 allows (today: original or CC0; CC-BY only if the owner amends it, see samples.md §1 and README) and log every file's provenance (samples.md §8) |
| Projects depend on content that changed | Immutable paths per version, old versions kept, hash-based relocation |
| Installer creep | CI check that fails when the core kit passes its budget |
| 2 GiB per-asset limit | Split big libraries per instrument family; CI enforces the limit |

---

## Sources (all accessed 2026-10-05)

- GitHub Docs, Git LFS billing: https://docs.github.com/en/billing/concepts/product-billing/git-lfs
- GitHub pricing calculator (LFS $0.07/GiB storage, $0.0875/GiB transfer): https://github.com/pricing/calculator
- GitHub Docs, Product and SKU names: https://docs.github.com/en/billing/reference/product-and-sku-names
- GitHub Docs, About releases (1000 assets, 2 GiB, no total/bandwidth limit): https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases
- GitHub Docs, About large files on GitHub: https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-large-files-on-github
- GitHub Acceptable Use Policies, section 9: https://docs.github.com/en/site-policy/acceptable-use-policies/github-acceptable-use-policies
- Cloudflare R2 pricing (page "last updated Oct 1, 2026"): https://developers.cloudflare.com/r2/pricing/
- Cloudflare Project Alexandria (search snippets): https://www.cloudflare.com/lp/project-alexandria and https://blog.cloudflare.com/expanding-our-support-for-oss-projects-with-project-alexandria/
- Backblaze pricing update, $6.95/TB and free API calls from 1 May 2026 (search snippets; site blocked by the proxy): https://backblaze.com/blog/backblaze-pricing-and-product-updates/ and https://noise.getoto.net/2026/03/17/backblaze-pricing-and-product-updates/
- Backblaze CDN partners (search snippets): https://www.backblaze.com/cloud-storage/solutions/cdn
- B2 3x free egress (secondary): https://infratally.com/articles/s3-alternatives-object-storage/
- AWS CloudFront pricing, flat-rate plans: https://aws.amazon.com/cloudfront/pricing/
- CloudFront pay-as-you-go rate (secondary): https://www.costbench.com/software/cdn-edge/aws-cloudfront/
- AWS S3 pricing (prices rendered by JS, not read): https://aws.amazon.com/s3/pricing/
- Bunny Storage pricing: https://bunny.net/pricing/storage/
- Bunny CDN pricing: https://bunny.net/pricing/cdn/
- DigitalOcean Spaces pricing: https://www.digitalocean.com/pricing/spaces-object-storage
- Hetzner Object Storage FAQ: https://www.hetzner.com/storage/object-storage/ ; prices from https://sliplane.io/blog/cheap-object-storage-providers-europe
- Hugging Face storage limits: https://huggingface.co/docs/hub/storage-limits
- Zenodo size limits (search snippets): https://support.zenodo.org/help/en-gb/1-upload-deposit/80-what-are-the-size-limitations-of-zenodo and https://about.zenodo.org/policies
- SourceForge publishing (search snippets; page behind a bot check): https://sourceforge.net/create/ and https://sourceforge.net/blog/?p=8359
- Fastly Fast Forward: https://www.fastly.com/fast-forward
- OSUOSL mirroring (mailing list, 2019): https://lists.osgeo.org/pipermail/sac/2019-June/011233.html
- MuseScore repo `share/sound` (GitHub API): https://api.github.com/repos/musescore/MuseScore/contents/share/sound
- MuseScore latest release (GitHub API): https://api.github.com/repos/musescore/MuseScore/releases/latest
- MuseScore handbook, SoundFonts and Muse Sounds: https://musescore.org/en/handbook/soundfonts and https://handbook.musescore.org/fr/sound-and-playback/installing-muse-sounds
- Surge XT releases (GitHub API): https://api.github.com/repos/surge-synthesizer/releases-xt/releases/latest
- LMMS releases (GitHub API): https://api.github.com/repos/LMMS/lmms/releases
- sfzinstruments org repos (GitHub API): https://api.github.com/orgs/sfzinstruments/repos
- Salamander Grand Piano (sfzinstruments): https://github.com/sfzinstruments/SalamanderGrandPiano
- VSCO 2 CE and VCSL (GitHub API): https://api.github.com/repos/sgossner/VSCO-2-CE and https://api.github.com/repos/sgossner/VCSL
- FL Studio download page (26.1.7, 1.03 GB): https://www.image-line.com/fl-studio/download
- FL Studio manual, browser content / FL Cloud Sounds: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/browser_content.htm
- FL Cloud Sounds: https://www.image-line.com/fl-cloud/sounds
- Ableton, compare editions: https://www.ableton.com/en/live/compare-editions/
- Ableton, Installing Live Packs: https://help.ableton.com/hc/en-us/articles/115001930644
- Bitwig sound content: https://www.bitwig.com/sound-content/
- REAPER download: https://www.reaper.fm/download.php
- ReaPack: https://reapack.com/ ; index format: https://github.com/cfillion/reapack/wiki/Index-Format
- Logic Pro user guide: https://support.apple.com/guide/logicpro/download-additional-sounds-lgcpd1a0f2a4/mac
- crates.io API (licences), `https://crates.io/api/v1/crates/<name>` for: reqwest, rustls-native-certs, native-tls, sha2, minisign-verify, ed25519-dalek, zip, claxon, flate2, zstd, symphonia-bundle-flac
