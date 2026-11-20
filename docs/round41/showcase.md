# Round 41 — showcase: the site and the README after an outside review

Builder: `r41/showcase` (worktree `F:/projects/daw-r41/showcase`, from master `7b287c39`).
Scope: the reviewer's recommendations for cantusdaw.com (`web/`) and the README, each checked
against the source first. **No new measurement** of the new version (the owner's
instruction), no audio or pictures re-rendered, nothing pushed, deployed or published.
**Nothing here has been built yet**: no cargo or trunk was allowed during this pass; the list
of what needs a build check is below.

## Per point

### Web

1. **Images without alt text — mostly false.** Every picture that carries content already had
   an alt in both languages (the program, the studio, the AI, the score, the session, every
   unit). The empty alts are right: the logo beside the word "Cantus", the hero's decorative
   copies (`aria-hidden`), and the unit thumbnails inside a button or link that already says
   the unit's name (an alt would make a screen reader read it twice). Two real gaps, fixed:
   the recording mode's two pictures shared one generic alt — each now says what it shows; and
   the share image had no `og:image:alt` / `twitter:image:alt` — added, in the page's
   language, from what `og.png` and `og-es.png` actually show.
2. **macOS "preview" and "Coming soon" — true.** The hero, the phone menu, the JSON-LD, the
   meta descriptions and the FAQ said "macOS preview" unconditionally, while the download card
   said "Coming soon" (beta.15 has no disk image). Now one phrase from one place
   (`Shelf::mac`, `Shelf::wait` in `shelf.rs`): **"macOS preview"** only while the latest
   release holds the `.dmg`, otherwise **"macOS: next beta"** / **"macOS: próxima beta"**, and
   "Next beta" on the card where the download would be. The FAQ says plainly that betas 11–14
   carried an unsigned preview and beta.15 was published for Windows and Linux only; the
   roadmap says the preview comes back in the next beta. `assets/release.json` refreshed to
   beta.15 with `tools/release_json.py` (the site workflow does this on deploy anyway), so a
   local build shows the truth too. The press kit's platform line follows.
3. **No stability warning by the downloads — true.** `top::BETA_NOTE`: "It is a beta: save often
   and keep backups of your projects." / "Es una beta: guarda a menudo y conserva copias de
   seguridad de tus proyectos." — on the home's download card, on `/download` under the meta
   line, and bold in the README's Download section.
4. **Windows 10 "untested", no guidance — true.** Now "Windows 10 not tried yet" with "On
   Windows 10? … Tell us whether it runs — that is how Windows 10 joins the list", linking a
   prefilled **hardware report** (`top::WIN10_REPORT`; the form already has a *Windows,
   processor, memory* field). Home card, `/download` and the README.
5. **Benchmark says beta.14, download is beta.15 — true but it was honest; made explicit.**
   Not re-measured. The numbers stay beta.14's, with their dates; now: the hero's proof strip
   names the machine (AMD Ryzen 7 7735HS, Windows 11), the programs' versions and both dates,
   and — computed, not typed — says "The download is beta 15; these numbers are beta 14's and
   have not been measured again on beta 15 yet" whenever the site's version differs from the
   measured one (`pages::measured::newer_note`); `/measured` gets the same as a chip. The
   README's comparison states version, dates and machine in its footnote. Found on the way:
   the "rows where a leader has more" list still showed **convolution reverb** and **ripple
   edit / insert time**, both built since (Aula; insert and delete time in round 40) — removed,
   with a comment, like the round-37 ones before them.
6. **Do sound packs cost money? — true that it was unclear.** ADR 0030/0042 and the manifest
   (`crates/cantus-content/manifest/content.json`: 34 packs, 17 CC0, 17 CC BY 3.0/4.0, no
   price field) say every pack is free. The site's *Free* chapter said "Donations and extra
   sound packs can fund the work", which reads as paid packs. Now a promise of its own: "The
   sound packs are free too — every pack Cantus offers today is CC0 or CC BY with the credit
   shown, downloaded only when you ask; the music you make with them is yours." The FAQ, the
   press kit and the README's licence table say the same. The owner's option of paid extras
   one day is kept in softer words ("Donations — and, one day, optional extras — can fund the
   work, but never unlock the essentials"); **he should confirm or remove that clause**.
7. **Repeated content — partly true.**
   - The home shows the program's picture in the hero and then the tour's three tabs as crops
     of **the same picture** (Arrange, Mix, Inspector). They zoom into it, but it is the same
     shot. Not changed: the only other existing whole-window pictures are the studio's
     (already on `/studio`) and README PNGs in English only, which would put an English
     interface on the Spanish page. **New shots needed** (EN and ES, Night and Paper, from
     `shoot_site` or `shoot_showcase`): a piano roll with a melody and its velocity lane, a
     channel rack with a pattern, and a mixer close-up of a different song (Morning Light or
     Sunday Strut) — then the tour's tabs become Arrange (crop), Piano roll, Channel rack,
     Mixer.
   - The measured numbers appear twice on the home: the hero's proof strip and the *Measured*
     chapter. That is summary and detail by design (round 39's Mock 1); left as is.
   - **Wrong counts found and fixed:** the site said "six demo songs come with it" — seven
     ship (*With All Your Heart* is not on the site's player). The download page now says
     seven, with the menu and shortcut; the listen chapter says "six of the seven"; the menu
     says "Demo songs made in Cantus". The press kit said nine instruments and twenty-five
     effects and beta.13 — now eleven, forty-two, beta.15, with ARA 2 and Linux audio.
8. **Social proof — partly in place already.** `proof.rs` (round 39) shows GitHub's stars and
   downloads, counted at build time, only from 50 stars / 500 downloads, and a *Made with
   Cantus* row only with authors' permission; today (5 stars) it stays hidden, correctly.
   Added, static and true: the FAQ's "Where do I report a problem?" now says reports get
   fixed — issue #1 (a flicker on many screens) was reported 14 November 2026, had its cause
   and a workaround three days later and the fix shipped in beta.15 — with a link to the
   closed issues (`top::CLOSED_ISSUES`); the README's new *Feedback* section says the same.
   No shields badge on the site: ADR 0034 forbids external requests. **Proposals** (not
   implemented): the reporter of issue #1 wrote that the workaround "worked like a charm" —
   ask him whether he may be quoted; list songs in `MADE_WITH` as users offer them; a "hardware
   that works" table from the hardware reports (`docs/hardware.md`) once a few exist; KVR
   ratings once the product entry exists.
9. **Faith with context — drafted, not published.** `docs/round41/why-cantus-draft.md`: a
   first-person "Why I'm building Cantus" in English and Spanish, every sentence traceable to
   the repository (sources listed), the lines most in need of his own words marked
   **[check]**, the verse quoted only from public-domain translations (World English Bible,
   Reina-Valera 1909 — never the RVR1960). It also says where it would go: a line and a link in
   the home's *Free* chapter beside the verse, the whole text on a new `/why/` page, a
   paragraph in the README. The faith on the live site is unchanged.
10. **The demo video — moved to another branch** (the coordinator's change of scope): no
    `video-plan.md` here. Marked spots, no embed: a `TODO(round 41, video)` comment in
    `web/src/top.rs` before the hero's picture, and an HTML comment above the README's
    picture.

### README

11. **First screen — done.** What it is (one bold line and one sentence), why it matters (*Why
    Cantus*), badges, two download buttons (Windows installer, Linux AppImage, by their stable
    names), the picture, and the motto with its context ("the motto Cantus is built under").
12. **Badges — done, each true.** Latest release (shields.io, live), licence (static:
    GPL-3.0-or-later + MPL-2.0, linking the licence table), CI (`ci.yml` on master, GitHub's
    own badge — live, see the owner's list), platforms (static: Windows | Linux).
13. **Highlights — done.** Eight bullets; the old long ones moved, whole, to *In depth: what
    the latest versions added*, further down (two stale numbers in them updated: "6 000+
    tests" → "more than 8 000", "511 factory presets" → "more than 750 today", from the
    per-unit counts the `stated_preset_counts` test keeps true).
14. **Jargon — done for the reader's path.** No ADR, round or beta.N on the first screen, the
    highlights, the first-song steps or the comparison (except the measured version in its
    footnote). *In depth* opens with one line explaining ADR, round and beta; the tour's
    shoot commands fold into a *How the pictures are made* box; the controllers' ADRs link to
    `docs/adr/`. The full list keeps its ADR references (it is the reference section).
15. **Comparison table — done.** Six rows against REAPER, FL Studio and Ardour: window ready,
    CPU at rest, memory (marked **lost**), price, source, systems. Only the scorecard's facts:
    measured rows from `docs/scorecard/README.md` (Cantus beta.14 2026-11-11, the others
    2026-11-09, the Ryzen 7 7735HS laptop), documented rows from `features.md` and its matrix
    (vendors' pages read 2026-11-04, REAPER 7.84, FL Studio 26.1.7.5, Ardour 9.8). Ardour was
    never measured: "not measured". No capacity claim (the live DAWbench is not made).
16. **First song in five minutes — done**, with what exists: the audio setup at first run,
    *File ▸ Demo songs…* (`Ctrl+Shift+N`, seven songs), the *Beat* starter from *File ▸ New from
    template…*, the channel rack, piano roll and *Add an effect*, *File ▸ Export audio…*
    (`Ctrl+Shift+E`) with the *Streaming* preset — each checked in the manual.
17. **Build from source — done.** `git clone …cantus-daw.git`, `cd cantus-daw`, `cargo run
    --release -p cantus`, then what Windows (Build Tools, LLVM, the ASIO SDK command) and
    Linux (`packaging/linux/deps.sh`) need. The old "Rust 1.95, no system libraries on
    Windows" line was wrong and is gone. `docs/DEVELOPING.md` said `cd cantus` after cloning
    `cantus-daw` — fixed.
18. **Community — issues only, in the README.** A *Feedback* section points to the issues and
    the three forms. Placeholder for the owner: **"Community channel: GitHub Discussions /
    Discord / issues only — to decide"**; nothing about it is in the README.
19. **Licences — done.** A table: the engine crates MPL-2.0, the application GPL-3.0-or-later,
    your music yours, sound packs CC0/CC BY with credit, the name and logos (TRADEMARKS.md),
    contributions (CONTRIBUTING.md, CLA.md). One line says why GitHub's sidebar and some
    catalogues show the licence as unknown: the root `LICENSE` is a pointer text, so no
    licence is detected (the AppImage catalogue lists it as "unknown"). The LICENSE files are
    not restructured — the owner's decision.

Also: the README's layout table described `cantus-dance` as "the dancing character"; the
row is removed (the owner keeps the dancers off the README; `LICENSE` still lists every
crate).

### Promotion plan

`docs/round41/promotion-plan.md`: prepare (with what to close first), Show HN and the Rust
communities, musicians' forums (ideally with beta.16 and macOS), Spanish-speaking
communities, a weekly rhythm, and measurement **without UTM links** — the site has no
analytics by design (ADR 0034, PRIVACY.md), so it uses GitHub's traffic and referrers, release
download counts, issues and Cloudflare's request totals. All of it is the owner's to do.

## Commits (branch `r41/showcase`)

- `1327ad27` web: one phrase for macOS, a beta note and a Windows 10 ask by the downloads,
  the scorecard's version and machine, free sound packs said plainly
- `f6c11da9` README: first screen, badges, eight highlights, first song in five minutes, a
  dated comparison, build in three commands, licences and feedback
- `da651f2f` the Why draft, the promotion plan, why GitHub shows no licence
- this report

## Needs a build check (when allowed: `bash /f/projects/daw-r40/slot.sh`, `-j 2`)

In `web/` (its own crate and lock file):

1. `cargo check` (wasm32) and `cargo check --features ssr`, then `cargo test` and `cargo
   clippy -D warnings` — the edits are Rust inside `html!` and `format!`, not compiled:
   - `shelf.rs`: new `Shelf::mac` and `Shelf::wait`, `use crate::lang::Lang`.
   - `body.rs`: `platform()` lost its `l` argument and gained `wait`; the `state` closure
     takes a third argument; `if *os == Os::Windows { … }` inside `html!`; imports.
   - `pages/download.rs`: a guarded match arm `Os::MacOs if shelf.macos`; `if shown ==
     Os::Windows` in `html!`; imports `Shelf`, `BETA_NOTE`, `WIN10_REPORT`.
   - `pages/measured.rs`: the `more` array is now `[_; 6]`; `newer_note`; an `if` inside the
     chips' `<ul>`.
   - `top.rs`: the proof strip's `newer_note(l).map(…).unwrap_or_default()` child; new pub
     consts; `shelf.mac(l)`.
   - `head.rs`: a new `os` named argument and `image_alt` in two `format!` strings;
     `og_image_alt`.
   - `route.rs`: guarded arms `Route::Home if mac` / `Route::Download if mac`.
   - `studio.rs`: an `if`/`else` expression as the `alt` attribute.
   - `ahead.rs`, `nav.rs`, `listen.rs`: strings and one import.
2. `cargo fmt` in `web/` (long `l.t(…)` lines in plain Rust, outside `html!`, may be
   rewrapped).
3. xtask's `package_ci_tests` (they read `web/src/top.rs`: the stable download links and
   `VERSION` from `build.rs` — untouched, but top.rs changed).
4. `cantus-ui`'s `stated_preset_counts` (it reads the README's per-unit preset lines —
   untouched, but the README was rewritten around them).
5. `trunk build --release`, the pre-renderer, then `node tools/contrast.mjs --quick` and a
   look with `node tools/shoot.mjs` at `/`, `/es/`, `/download/`, `/es/download/`,
   `/measured/`: the new lines reuse existing classes (`note`, `dl-other`, `chips li`) — check
   they sit well, and that the hero's facts read "macOS: next beta".
6. The README on GitHub's renderer (badges, the two download buttons, the tables).

## What needs the owner

- **The Why text** (`docs/round41/why-cantus-draft.md`): approve, rewrite or reject; the
  **[check]** lines first. Nothing goes live until then.
- **The community channel**: GitHub Discussions, a Discord, or issues only.
- **"One day, optional extras"** in the *Free* chapter and the FAQ: keep or remove.
- **The beta.15 release notes on GitHub** still list a macOS disk image and a "macOS is a
  preview" paragraph (both languages), but no `.dmg` was published: edit the release body.
- **The CI badge**: live; on 2026-11-20 the last runs of `ci.yml` on the public master read
  *cancelled* and *failure* (the release saga). A green run before the README is published,
  or drop the badge.
- **The Linux AppImage's name** may become `Cantus-x86_64.AppImage` after beta.16 (the
  catalogue's request; cantus-bc and the owner decide). This pass did not touch the name: the
  site's `top.rs`, the README's download button and its Download table all still link
  `Cantus-linux-x86_64.AppImage` — the integrator updates them together when the link
  changes.
- **The licence shown as unknown** by GitHub and the AppImage catalogue (the root `LICENSE`
  is a pointer); restructuring it is his decision.
- **New pictures** for the tour (point 7) and the **video** (another branch).
- **The press kit is English only** (`web/assets/press/index.html`); a Spanish twin would
  match the rest of the site.
- **Permission to quote** issue #1's reporter, if a user's words are wanted on the site.
- Deploying the site and publishing the README (the integrator, on his word).
