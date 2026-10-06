# Round 20 — samples: rights-free sounds, a content manager, a sample browser

**Builder:** samples (`r20/samples`). **ADR:** 0030 (content and samples).

The owner asked where Cantus could get rights-free WAV files (a drum kit, kicks, snares and
so on) and where they should be kept, "not in the repo, I think", and then asked for it to
be built at once.

## What was built

1. **`crates/cantus-content`**, a new crate (GPL-3.0-or-later) that holds:
   - the manifest, `manifest/content.json` (format 1, CC0 packs only, about 250 KB), and
     its parser and validation (`manifest.rs`);
   - the download routine (`fetch.rs`):
     - `https://` through `ureq` 3 with `rustls`, checked by the operating system's
       certificate verifier, so the Zscaler root is trusted the way the browser trusts
       it and nothing is bypassed;
     - `file://` for offline installs and tests;
     - resume with HTTP `Range`;
     - a proxy's HTML page recognised and never written;
     - SHA-256 checked before a file is used;
   - install, remove and receipts (`install.rs`):
     - downloads go to `.downloads`, unpack to `.staging`, and only a complete pack with
       its licence file is renamed into place;
     - zip-slip is refused;
     - `include`/`exclude` patterns take a subset of a large archive;
     - retries with backoff for up to 12 minutes, because a filtering proxy holds files
       for several minutes;
   - the threaded manager the interface polls, which never blocks it (`manager.rs`);
   - folder listing in natural order, and a search index built on a worker thread
     (`library.rs`).
2. **The engine's audition voice** (`cantus-engine/src/preview.rs`): `EngineMessage::Preview`
   and `EngineEvent::GarbagePreview`. It plays after the master strip, as the click does,
   and never allocates or frees on the audio thread.
3. **The sample browser**, a dock pane (`Pane::Browser`):
   - it opens with Alt+F8, from the toolbar's first window key, or from View ▸ Browser;
   - it lists installed packs, then *My folders* (add a folder with +, remove one with ✕
     on hover), as a tree with a search field;
   - a click auditions a sample (decoded on its own thread, the last 24 kept);
   - a drag shows a chip with the sample's name and rings the target under the pointer:
     - on a **Pulsus pad**, the pad plays the file;
     - on **Exemplar's key map**, the file becomes a zone on the key under the pointer;
     - on the **channel rack**, the file becomes a new sample channel (a one-pad Pulsus
       with its own strip). On a one-pad channel it replaces that channel's sample;
     - on the **arrangement**, the file becomes an audio clip, through round 19's import.
   - Each drop is one undo step.
   - While no pack is installed, a starter card offers *Cantus Starter Drums* in one
     click, with a progress bar, Cancel, and *More packs…*.
4. **The *Sound packs* window** (the ⋯ key in the browser's header):
   - packs by category, each with its size, licence and an Installed / Update / Installing
     chip;
   - for the selected pack: description, download and on-disk sizes, file count, licence
     (SPDX id and what it means), the date it was read, credits with their URLs, and where
     the licence file is kept;
   - Install / Update / Install again, Remove (after a confirmation), Show the folder,
     *Install from a file…* (zip packs), and Cancel with progress and phase text
     ("Waiting for the network (…); trying again in 30 s").
5. **Strings** live in `locales/areas/samples.{en,es}.txt`, registered in `i18n.rs`.
6. **Screenshots:** `cargo run -p cantus-ui --example shoot -- <dir> samples` writes
   `samples-browser`, `samples-search`, `samples-starter` and `samples-content`, each in
   slate and paper.

## The packs (all CC0-1.0; licences read 2026-10-05)

| Pack id | What | Download | On disk | Source and licence evidence |
|---|---|---|---|---|
| `cantus-starter-drums` (recommended) | 81 curated one-shots: 5 kicks, 10 snares, 9 hats, 6 toms, 9 cymbals, 13 claps and snaps, 26 percussion, 3 licence files | 33.0 MB | 33.0 MB | Virtuosity Drums (Versilian x Karoryfer) @ `9f04cf9`; Body Percussion @ `4ac9d89`; VCSL @ `c1ea7bc`. Each repository's `LICENSE` is CC0 1.0 and ships in `Licences/` |
| `afr-kicks-2024` | 16 synthesized electronic kicks (606/707/808/909-style, deep house) | 1.4 MB | 1.9 MB | github.com/xyproto/synth release v1.4.4. The release notes say "CC0 licensed"; `CC0-license.txt` is in the zip |
| `body-percussion` | 202 claps, snaps, stomps, chest and belly hits | 61.7 MB | 61.7 MB | sfzinstruments/body_percussion @ `4ac9d89`, LICENSE CC0 1.0 |
| `gogodze-phu-vol-ii` | Vintage kit, 7 mics, hi-hat in 5 openness steps | 139.5 MB | 484.0 MB | Karoryfer release v1.001; LICENSE CC0 1.0 (in the zip and the repo) |
| `gogodze-phu-vol-i` | Five Ghanaian bobobo drums and a cajón | 31.2 MB | 89.1 MB | Karoryfer release v1.001; LICENSE CC0 1.0 |
| `frankensnare` | About a dozen snares, claps, tambourine | 365.6 MB | 359.1 MB | Karoryfer release v2.100; `license` is CC0 1.0 |
| `big-rusty-drums` | 1980s Polish kit: sticks, brushes, mallets | 619.6 MB | 706.9 MB | Karoryfer release v1.100; LICENSE CC0 1.0 |
| `swirly-drums-dry` | Brushed kit and odd percussion, dry samples only (the wet copies, 1.4 GB, are excluded) | 868.6 MB | 341.1 MB | Karoryfer release v1.104; `license` CC0 1.0 |
| `unruly-drums` | Every drum with snare wires; hi-hat in 6 steps | 676.3 MB | 796.5 MB | Karoryfer release v1.100; LICENSE CC0 1.0 |
| `vsco2-percussion` | 329 orchestral and hand percussion samples | 320.5 MB | 320.5 MB | sgossner/VSCO-2-CE @ `4403009`, LICENSE CC0 1.0 |
| `vcsl-tx81z` | 189 multisamples of a Yamaha TX81Z (to load into Exemplar) | 134.5 MB | 134.5 MB | sgossner/VCSL @ `c1ea7bc`, LICENSE CC0 1.0 |

Every hash was computed from a real download on 2026-10-05 through the corporate proxy. No
file was ever quarantined. Every `LICENSE` above was read inside the downloaded archive or
at the pinned commit.

### Checked and left out

| Source | Why |
|---|---|
| Dustyroom *Fake Acoustic Drum Kit* (500 one-shots) | Older articles call it CC0; its page on 2026-10-05 says **"Everything in the pack licensed CC BY 4.0"**. It is also downloadable only through itch.io (no stable URL) |
| Karoryfer *The Hat with the Phat* | The repository says CC0; the shop page says "GPL 3" (research `samples.md` §2.1). Ask Karoryfer |
| Virtuosity Drums (complete, 1.23 GB) | CC0, and its release asset has a SHA-256 digest from GitHub (`c6c5d0fe…`). It can be added as a zip pack once its installed size is measured. The starter takes the best of it already |
| Salamander Grand, Naked Drums, DrumGizmo DRS/Muldjord, Dim Cabasa | CC-BY: the owner's ruling (ADR 0030, open point 1) |
| Signature Sounds (CC0) | The licence is fine. Downloads go through a store page; there are no stable file URLs |
| Freesound CC0 | Original-quality downloads need OAuth2 (an API key and a user login). Files must be vetted one by one (rips posted as "CC0") |
| 99Sounds, Splice/Cymatics "free" packs, Hydrogen legacy kits, archive.org drum-machine dumps | Redistribution forbidden, or the licence is unclear (research `samples.md` §1.6, §2.3) |

### Adding or changing a pack

Hashes and sizes are never typed by hand. `crates/cantus-content/tools/` holds two
scripts (Python 3; they need `curl` and `git`):

- `build_packs.py <work> starter|body|tx81z|vsco-perc` downloads every file of a
  file-list pack from its pinned commit and checks it is audio, not a proxy page. It
  hashes each file and writes `<work>/<name>.json`. To list a repository's files it uses
  a blobless clone, which downloads no audio.
- `make_manifest.py <work> crates/cantus-content/manifest/content.json` reads those
  lists and the release archives in `<work>/dl`. It measures what each pack's
  `include`/`exclude` keeps and writes the manifest.

Re-running it on this round's downloads gives a byte-identical `content.json`. Bump a
pack's `version` whenever its files change; an installed older version then shows
*Update*.

## Tests

- `cantus-content`, 13 tests (`tests/content.rs`) plus 1 ignored:
  - the bundled manifest is valid, CC0-only and bilingual, and has one small recommended
    pack;
  - a newer manifest format is refused by name;
  - validation rejects path traversal, absolute paths, backslashes, bad hashes, plain
    `http`, duplicate ids, two files at one path, and a licence file that is not shipped;
  - a files pack installs with its licence and a receipt, is found again, shows *Update*
    for a newer version, and is removed;
  - **a corrupted download is refused** and nothing is installed or kept;
  - a zip pack keeps only what it includes, strips its folder and deletes the archive;
  - a zip without its licence is not installed;
  - an offline install takes the user's archive through the same checks, and a different
    archive is refused;
  - **a proxy's HTML page** (declared `text/html`, or disguised as binary) is refused and
    never written;
  - **an interrupted download resumes** from where it stopped (a loopback server that
    drops the connection, then honours `Range`);
  - a cancel keeps the part file;
  - 404 is not retried while 5xx and 429 are;
  - globs.
- **Ignored:** `CANTUS_CONTENT_NET=1 cargo test -p cantus-content -- --ignored` downloads
  the starter pack for real and checks every hash.
- `cantus-engine`:
  - `preview.rs`, 3 unit tests (it plays once and hands the sample back; a stop fades;
    a new audition replaces the old one);
  - `tests/preview_rt.rs`, 2 tests (**start, replace, stop and natural end touch the heap
    zero times on the audio thread**; every sample comes back to be dropped by the
    interface; the audition is not in the master meter).
- `cantus-ui`, `tests/samples_browser.rs`, 6 tests:
  - the tree lists packs and folders, opens them, leaves out non-audio files, and search
    spans both;
  - a click auditions and a second click stops;
  - **a drag onto a pad, aimed through the geometry the editor publishes**, makes the pad
    a sample pad playing the file in one undo step, and undo restores it;
  - a drop on the rack makes a sample channel with its own strip, in one undo step;
  - the starter offer installs the pack in one click, then disappears and lists the pack;
  - the Sound packs window shows names, licences and credits.

## Files touched outside my area

- `Cargo.toml`: the workspace member, the default member, and the `cantus-content`
  workspace dependency. `Cargo.lock`: ureq, rustls, rustls-platform-verifier, ring, zip.
- `crates/cantus-engine`:
  - `lib.rs`: `pub mod preview`;
  - `messages.rs`: `EngineMessage::Preview`, `EngineEvent::GarbagePreview`;
  - `processor.rs`: one field, two message arms, a mix after `render`, and
    `give_back_preview`;
  - `backend.rs`: `EngineHandle::preview` and the garbage arm.
- `crates/cantus-ui`:
  - `Cargo.toml`: the dependency;
  - `lib.rs`: `pub mod samples`;
  - `views/mod.rs`: three modules;
  - `app.rs`: the `samples` field, `poll_samples` at the top of `frame`,
    `samples_browser::frame` after the demos, and the `ToggleBrowser` arm;
  - `settings.rs`: `sample_folders`, `sample_audition` (both `#[serde(default)]`);
  - `keymap.rs`: `Action::ToggleBrowser` (Alt+F8), `ALL` is now 71, and the file format is
    version 3 with `NEW_IN_V3`. Other builders adding actions will conflict on the count
    and the version: the integrator merges them into one list;
  - `i18n.rs`: the area;
  - `audio_files.rs`: `free_strips` is now `pub(crate)`;
  - `views/workspace.rs`: `Pane::Browser`, its keys and glyph, and where it is placed (at
    the left of the root);
  - `views/workspace_frame.rs`: one arm;
  - `views/transport.rs`: the toolbar's window keys (6 now) and the View menu;
  - one `drop::publish` line each in `views/channel_rack.rs`, `views/arrangement.rs`,
    `views/inst/sampler_map.rs` and `views/pulsus_editor_grid.rs`.
- `examples/shoot.rs`: the `samples` kind (scene in `examples/shoot_samples/scene.rs`).

## Left

1. The pack tools are Python. Porting them to an `xtask content` command, so they use the
   crate's own downloader, would remove the Python dependency.
2. **Projects reference absolute paths** under `Content/`. A portable `content://` reference
   with "missing content: download it" on project load needs a schema bump; that round
   is not this one.
3. **A remote manifest** (update the list without a release) with a minisign signature;
   **mirrors** (format 2).
4. **Electronic drums:** rendering Pulsus presets to WAV as a CC0 "pack" built on the
   user's machine (no hosting) is the research's recommendation and the cleanest source
   of 808/909-style sounds.
5. The browser has no keyboard navigation yet (up and down to audition, as in FL). Under
   the keymap rules that needs actions and a focus owner.
6. Drag onto a mixer insert, onto an existing audio clip (replace), or onto a step of the
   rack is not handled; neither is dragging a folder (Exemplar would map it).
7. The browser is not in the default layout; see the owner's decisions.

## Decisions for the owner

1. **CC-BY** packs (Salamander Grand, Naked Drums, DrumGizmo): allowed as opt-in downloads
   with credits shown, or not?
2. **Hosting:** keep pointing at upstream (free, clear provenance, at the mercy of an
   author deleting a release), or mirror on a `cantus-content` GitHub repository with a
   signed manifest?
3. **Default layout:** show the browser at the left on first run, as FL does, and offer the
   starter kit then?
4. **Pulsus renders** as Cantus's own CC0 electronic kit: build them?
5. Ask Karoryfer about *The Hat with the Phat* (CC0 or GPL-3?).
