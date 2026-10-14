# Round 26 — content: CC BY in `cantus-content` (item 6, ADR 0042)

Builder: **content** · branch `r26/content` · base `a3b6355`.

## Done

- **The manifest accepts exactly CC0 1.0, CC BY 3.0 and CC BY 4.0** (`manifest::Licence`,
  ids spelled exactly). Anything else — NC, ND, SA, NC-SA, NC-ND, CC BY 2.0/2.5, Sampling
  Plus, `CC-PDDC`, GPL, lower-case or spaced ids, empty — fails validation with "licence
  `…` is not accepted (ADR 0042: …)". The licence URL must be the licence's own text
  (prefix per licence), so an NC/SA text under a CC BY id, or a CC BY text under CC0,
  fails too.
- **A CC BY pack's credit line**: `Pack::attribution` = `title`, `author`, `url` (https),
  optional `line` (the licensor's own wording, word for word) and `note` (the author's own
  words about crediting music, quoted on the pack page). Missing, blank title/author, a
  non-https link or a blank `line` fails. The licence file stays required for every pack.
  `is_cc0` is replaced by `licence_kind()`, `needs_credit()`, `credit_line()`.
- **Manifest format 2** (`FORMAT = 2`, `FORMAT_ATTRIBUTION = 2`): a manifest carrying a
  credit line must declare format 2, so format-1 builds (beta 1) refuse it by name instead
  of failing on an unknown field; the bundled manifest stays format 1 (valid, unchanged).
  *Superseded later this round: the bundled manifest is format 2, since it carries the CC
  BY Salamander (below).*
  This is the content manifest, not the project format: no project schema change.
- **The receipt carries the credit** (`Receipt::attribution`, `Receipt::credit_line()`).
  `serde(default)` + `skip_serializing_if`: old receipts load, CC0 receipts are unchanged on
  disk.
- **`credits::list(manifest, installed)`** — one source for About and the notices: every
  manifest pack (CC BY *required* with its full line, CC0 *courtesy* with its makers), plus
  installed packs the manifest no longer lists (from their receipts), without duplicates.
- **Sound packs…** (`views/samples_content.rs`): for a CC BY pack the licence row says
  "free for any use, with credit to the author"; a *Credit* band shows the credit line —
  the manifest's before the download, the receipt's once installed, then "Installed with
  this credit: it is kept in the pack's receipt, beside the licence (file)"; ADR 0042 §4's
  plain words (the pack is CC BY n; any music, commercial or not; change the sounds; the
  one condition is credit, which Cantus gives here, in About and in the pack's folder; the
  music you make is yours; whether a published track must credit the samples is yours to
  judge), then the author's own words when the manifest quotes them, otherwise "Most
  libraries like this one say it need not; the licence file in the pack has the exact
  terms". CC0 pages are as before.
- **The browser's one-click offer** (`samples_browser.rs`): a CC BY recommended pack reads
  "CC BY 4.0: yours to use in any song, free of charge; credit to <author>" instead of the
  CC0 sentence.
- **About ▸ Credits** (`views/about.rs`): a *Credits* section at the end of About — the
  Bible translations (moved here from the middle of the window) and every sound pack:
  "Name · CC BY 3.0 · credit required" + its line, "Name · CC0 1.0 · credited as a
  courtesy" + its makers, "… · installed, no longer offered" for receipts only. A
  **Credits** button beside Licence / Third-party notices / Privacy scrolls the window to
  them (`Link::Credits`; `about::show_credits(app, ctx)` opens About scrolled there, for
  a Help-menu item).
- **`THIRD-PARTY-NOTICES.md`**: the hand-written *Sound packs* section left
  `xtask/notices/head.md`; `notices_render::content(&Manifest)` generates a ***Content***
  section (after *The Rust toolchain*, linked from the contents): CC BY packs with their
  credit line, licence file and the author's words; CC0 packs in a table (pack, makers,
  sources). `notices::problems` names every CC BY pack whose credit line is missing
  ("pack X: its CC-BY-3.0 credit is missing: …"), even when the file is otherwise current.
  Regenerated (298 crates, 344 KB).
- **Found and fixed on the way**: the notices head said the engine crates are "MIT OR
  Apache-2.0 (`LICENSE-MIT`, …)" — stale since ADR 0047 (MPL-2.0; `LICENSE-MIT` no longer
  exists). It now says MPL-2.0 (`LICENSE-MPL`, each crate's `LICENSE` with Exhibit A).
- **Shots**: the Sound packs window shots were **blank at any scale but 100 %** (also the
  existing `shoot … samples` scene): with `set_pixels_per_point` ≠ 1, egui's first frame
  still has a provisional 5000-point screen, the window takes its default position there
  (x ≈ 2110) and is then constrained to the right edge, off screen. Both
  `examples/shoot_content.rs` (new) and `examples/shoot_samples/scene.rs` now open the
  window on the second frame. The real app is unaffected (the window opens long after the
  first frame).
- `tools/make_manifest.py` can write a CC BY pack (`licence=`, `attribution(...)`), checks
  the licence and the credit line, and writes format 2 only when needed.

## Review fixes (adversarial review, same day)

1. **Required or courtesy by licence alone.** A CC0 pack may carry an `attribution` (the
   credit its author asked for): `credits::list` now marks it *courtesy* (from
   `Licence::needs_credit`, never from "has a line"), the notices list it in the CC0 table,
   About says "credited as a courtesy", and Sound packs shows its line with "CC0 asks for
   no credit; the author would like this one, and Cantus gives it gladly". A receipt-only
   pack is required unless its id is CC0. `make_manifest.py` allows it.
2. **The licence's link is never lost.** `Attribution::full` / `Pack::credit_full`: when
   the licensor's own `line` does not contain the licence URL, the licence name and link
   follow it (CC BY 4.0 §3(a)(1)(C)) — in the notices entry, About ▸ Credits and the pack
   page. The notices check now looks for the full credit.
3. **Credits in the reader's language.** The composed line on screen comes from
   `samples.credit.line` (EN `“title” by author (url), licensed under …`, ES `«title», de
   author (url), con licencia …`), the licensor's line + link from `samples.credit.licensor`;
   a licensor's own line stays verbatim. The receipt and the notices keep the English line.
4. **The licence URL is exactly the licence's text** (`Licence::is_text_url`): the prefix,
   then nothing, `legalcode`, `legalcode.<lang>` or `deed.<lang>`, optional trailing `/`;
   `..`, `?`, `#`, `%` refused. Jurisdiction ports (`by/3.0/us/`) are now refused (they
   are other licences); none is in the manifest.
5. (a) About keeps "credit required" for an installed CC BY pack the manifest no longer
   lists, adding "installed, no longer offered" (`about.credits.no_longer` replaces
   `about.credits.installed`). (b) The Sound-packs test gives the receipt a different line
   than the manifest, so it proves the installed page shows the receipt's. (c) `Attribution`
   and `Credit` lost `deny_unknown_fields` (receipts embed them without a format gate); a
   receipt with unknown fields in it, its credit and its attribution still loads.

6. **CC0 is worded as a dedication, not a licence**: a CC0 courtesy line reads "“title” by
   author (url), dedicated to the public domain under CC0 1.0 (url)" (receipt, notices,
   `samples.credit.line_cc0`), in Spanish "«title», de author (url), obra dedicada al
   dominio público con CC0 1.0 (url)" ("obra" keeps it right for any title's gender).

New tests: `a_licence_url_that_only_starts_like_the_licence_is_refused`,
`a_cc0_pack_may_carry_a_courtesy_credit_that_is_never_required`,
`the_licensors_own_line_is_given_with_the_licence_and_its_link`,
`a_receipt_with_fields_a_newer_cantus_wrote_still_loads` (content); the notices, Sound
packs and About tests extended (Spanish composed line, receipt's line after install,
licence link after a licensor's line, CC0 courtesy, receipt-only "credit required"). Each
was seen failing first (the notices test against the old `credits.rs`). The fixture's CC0
kit now carries a courtesy attribution. Pictures re-shot and looked at.

## Pictures

`cargo run -p cantus-ui --example shoot_content -- <dir> <scale> <en|es>` (fixture
manifest, nothing downloaded), in `F:\projects\daw-r26\shots\content\`, 75 % and 200 %,
English and Spanish, Cantus Night and Paper (56 PNGs): `content-piano-before-*` (CC BY 3.0
with the author's words, not installed), `content-piano-installed-*` (receipt credit +
"kept" line), `content-choir-*` (CC BY 4.0 with the licensor's own line, no author's
words), `content-cc0-*`, `content-starter-*` (CC BY starter offer), `content-about-credits-*`
(scrolled to Credits), `content-about-top-*`. Looked at: nothing overlaps or elides; long
credit lines wrap; the first pass showed "Help ? About Cantus" (the ▸ glyph is not in the
UI font) — the strings now say "Help > About Cantus" as the other locale strings do.

## Tests (each seen failing first)

`crates/cantus-content/tests/licences.rs` (new, 8):
- `exactly_cc0_and_cc_by_3_and_4_are_accepted` — the three ids pass, 13 refused ids fail
  with "not accepted"; `Licence::ACCEPTED`, names, `needs_credit`.
- `a_cc_by_pack_needs_its_credit_line_and_its_licence_file` — no attribution, blank author
  or title, http or empty link, no or unshipped licence file, blank licensor's line: each
  fails; the error says "credit".
- `the_licence_link_is_the_text_of_the_licence_named` — 4.0 id with 3.0 text, CC0 with a
  BY text, a `by-nc` text fail; deed, legal code and a port pass.
- `the_credit_line_is_the_licensors_own_or_title_author_link_and_licence` — composed line
  holds all five parts; the licensor's `line` is used verbatim; CC0 has none.
- `a_manifest_with_credit_lines_is_one_older_builds_refuse_by_name` — format 1 + credit
  line fails ("format 2"); format-1 CC0 stays valid.
- `the_fixture_manifest_with_cc_by_packs_parses` — the shared fixture; removing an author
  fails the whole manifest.
- `the_receipt_carries_the_credit_and_old_receipts_still_load` — a real install
  (`file://`) writes the attribution; read back from disk; a pre-round-26 receipt (literal
  JSON) loads with no credit; CC0 receipts grow no field.
- `the_credits_list_names_every_pack_cc_by_in_full_and_cc0_as_a_courtesy` — required vs
  courtesy, receipts the manifest no longer lists, no duplicates.

`crates/cantus-content/tests/content.rs`: the bundled-manifest test now holds "every
licence accepted, credit line iff CC BY, format ≤ FORMAT" instead of "only CC0".

`xtask/src/notices_tests.rs` (2 new, 3 updated for the new signatures):
- `the_content_section_credits_every_pack_of_the_manifest` — every CC BY credit line and
  licence file in the *Content* section, the author's words, the CC0 row, the contents
  link; the bundled manifest renders.
- `a_cc_by_credit_missing_from_the_notices_fails_the_check` — removing one credit line is
  reported by pack id, also when the file is otherwise current.
- `the_notices_name_every_crate_a_release_ships` now renders with the bundled manifest.

`crates/cantus-ui/tests/samples_browser.rs` (3 new):
- `a_cc_by_pack_shows_its_credit_and_the_plain_words_before_and_after_download` — EN and
  ES: credit line, "CC BY 3.0", the licence words, "the music you make with it is yours",
  "yours to judge", the author's words; once installed the "kept in the pack's receipt"
  line (absent before).
- `a_cc_by_pack_without_the_authors_words_says_what_such_libraries_usually_say` — the
  licensor's own line verbatim, the fallback sentence; a CC0 page unchanged.
- `the_starter_offer_of_a_cc_by_pack_names_its_licence_and_author`.

`crates/cantus-ui/tests/about.rs` (2 new):
- `credits_name_every_sound_pack_cc_by_in_full_beside_the_bible_translations` — EN/ES band,
  sub-heading, required/courtesy labels, both CC BY lines, CC0 makers, RV1909.
- `the_credits_button_scrolls_to_the_credits` — the button returns `Link::Credits` and the
  band lands at the top of the body; `show_credits` does the same (seen failing with the
  scroll disabled, and once more until the scroll survived the window's first frame).

## Files outside my area

- `xtask/Cargo.toml` (+ `cantus-content` dependency) and `Cargo.lock` (one line: xtask's
  dependency list). xtask now builds `cantus-content` (rustls, zip 8) — already built by
  the workspace for `cantus-ui`.
- `xtask/notices/head.md` (the generator's input): Sound packs section removed, contents
  entry, engine licence corrected.
- `crates/cantus-ui/examples/shoot_samples/scene.rs` (the Sound packs shots: `attribution`
  field, window on the second frame) and the new `crates/cantus-ui/examples/shoot_content.rs`.
- No `app.rs`, `keymap.rs`, `views/mod.rs`, `kit/mod.rs` change; no schema change; no
  undo-fuzz arm (no command).

## The Salamander Grand Piano, end to end (the owner's "no left for later")

The two items this report first left open are closed: the Salamander is installable
**and** playable, and *Help ▸ Credits* exists.

- **`.tar.gz` packs** (`cantus-content/src/tar.rs`, `install.rs`): a strict tar reader of
  our own — ustar `prefix`/`name`, GNU long names (`L`), pax `path`/`size`, octal and
  base-256 sizes, every header's checksum verified, a truncated archive an error — over
  `flate2`'s `MultiGzDecoder` (already a dependency; no new crate, so no new notice). It
  never creates anything itself: the installer takes each kept entry through
  `strip`/`include`/`exclude`, refuses a name that climbs out (`../`, `/abs`, `C:`, `\`)
  and any kept link or special file, and writes plain files only. **An unpack budget for
  zip and tar alike** (`install::budget`: the manifest's installed size + ¼ + 1 MiB): a
  gzip bomb, a header that claims 16 GiB, a zip entry over it — refused before or while
  writing, nothing installed. `Source::TarGz` needs manifest format 2; *Install from a
  file…* takes a `.tar.gz` too.
- **The pack** (`manifest/content.json`, now format 2, `updated` 2026-10-13): downloaded
  once into `F:\projects\daw-r26\downloads\` (gzip magic `1f 8b` checked; not committed),
  `SalamanderGrandPiano-SFZ+FLAC-V3+20200602.tar.gz`, **741 757 374 bytes, SHA-256
  `b7760e168494cf095344e217b0af013fc449ad033abbbdf1c65211cf11dc038b`**. Kept: `samples/**`
  but the four pedal noises (Exemplar cannot play them, below), `readme.txt` (the licence
  file: FreePats' note "Published under the terms of the Creative Commons Attribution 3.0
  license" and Alexander Holm's own README, "Licence: CC-by"), and
  `SalamanderGrandPiano-V3+20200602.sfz` — **639 files, 745 036 906 bytes installed**
  (`make_manifest.py`'s `tar_gz_pack` measured them; it now writes the whole entry). The
  author gives no wording for the credit and no words about crediting music, so the
  attribution is title/author/link only (`“Salamander Grand Piano V3” by Alexander Holm
  (FreePats page), licensed under CC BY 3.0 (legal code)`) and the pack page says what
  such libraries usually say. The real archive installs from the file through the bundled
  entry in about 2 s here (`tar.rs::the_real_salamander_installs_from_its_archive`,
  ignored: `CANTUS_SALAMANDER_ARCHIVE`).
- **Pack instruments** (`PackInstrument` in the manifest and the receipt, format 2):
  `id`, `name` (EN/ES), `file` (an `.sfz` inside the pack), `shelf`, optional `layers`, a
  calibration `gain_db` (±24). `Manager::instruments()` lists them from the receipts, so an
  installed pack stays playable whatever later manifests say. The Salamander offers two:
  **Salamander Grand Piano** (8 velocity layers) and **Salamander Grand Piano · 16 layers**.
- **One step to play it.** The instrument picker lists every installed pack instrument
  (`PickChoice::Sound`, key `pack:<pack>/<id>`) on its shelf (Keys) and with the samplers,
  by its author, found by its name, its pack's name and "Exemplar"; the details pane adds
  *Pack* and *Licence*. Choosing it — or *New track with …* on the pack's page in *Sound
  packs…*, which also names the instruments before the download — gives a new track (or a
  track's instrument, its clips kept) an Exemplar with the SFZ imported, **one undo step**
  (`apply_sound`: `InsertTrack` with the spec built from the import, or one
  `SetInstrument` batch).
- **How Exemplar plays it** (`docs/instruments/exemplar.md`'s importer, checked against
  the real file): FLAC decodes (claxon, already in the engine); the SFZ's opcodes —
  `lokey/hikey/pitch_keycenter`, `lovel/hivel` (16 layers), `amp_veltrack`,
  `ampeg_release` per group (1 s; 5 s for the undamped top notes), `trigger=release` with
  `rt_decay` (string resonances in three velocity bands, hammer noises with
  `pitch_keytrack=0` at −37 dB), `volume` — all map. **Not played: the pedal noises**
  (`on_locc64`/`on_hicc64`: regions a controller starts; Exemplar starts zones from keys).
  The importer used to put such regions (`lokey=-1`) on *every* key — found with the real
  file; they are now left off the keys and the log says so (4 regions). The sustain pedal
  itself works (Exemplar holds notes on CC 64 and plays the release samples when it lets go).
- **Memory, load and level, measured** on the real samples (`pack_sounds.rs::
  the_real_salamander_loads_plays_and_sits_at_the_calibrated_level`, ignored:
  `CANTUS_SALAMANDER`, `CANTUS_LAYERS`, `CANTUS_GAIN`, `CANTUS_SHOTS`). Exemplar decodes
  every sample to `f32` and adds a half-rate mip level for pitch-up headroom, so:

  | Instrument | Zones | Load (decode + mips) | Samples in memory | Triad at 100 | Dense riff at 127 |
  |---|---|---|---|---|---|
  | 8 layers, 0 dB | 397 | 16.9 s | 1 914 MiB | −18.9 LUFS | **+5.0 dBFS** (fails) |
  | **8 layers, −5.5 dB** | 397 | 15.8 s | 1 914 MiB | −24.4 LUFS, peak −8.7 | −0.5 dBFS |
  | **16 layers, −5.5 dB** | 637 | 32.4 s | 3 732 MiB | −26.6 LUFS, peak −10.6 | −0.5 dBFS |

  At 0 dB the triad sits on ADR 0039's calibration (native median −18.7 LUFS) but the
  headroom rule (no factory sound past 0 dBFS on the dense riff at 127) fails by 5 dB — a
  real grand at *fff* with four-note chords and a two-octave run. As with Lamina (ADR
  0039: "the most the headroom test allows"), the pack's `gain_db` is **−5.5 dB**: peak
  −0.5 dBFS, the triad 5.5 dB under the synths (Lamina's default sits at −22.9). The
  memory is in the pack's description and the picker names say which is which; 8 layers
  is the default for that reason. Listen-check: `F:\projects\daw-r26\shots\content\
  salamander-8-layers.wav` and `-16-layers.wav` (a scale from velocity 30 to 114, chords at
  35, 80, 120): no NaN, no DC, peak −1.4 / −3.5 dBFS, the scale's level rising −42 → −21 dB
  RMS, the chords −37.6 / −23.8 / −15.2 dB, the release falling to −88 dB in 1.5 s.
- **Help ▸ Credits** (`views/transport.rs`, the Help menu's file, one item): opens About
  scrolled to its credits (`about::show_credits`).
- **Pictures** (`shoot_content`, 75 % / 200 %, EN/ES, Night/Paper, 100 PNGs in all): the
  Salamander's page before its install (credit, plain words, "once installed, the
  instrument picker offers …") and installed ("Installed with this credit …", "In the
  instrument picker: …"), with *New track with …* in the pack's header, under its name,
  so the main action after the install is in view without scrolling on a 1280×720 screen
  at 100 % and 200 % (`pack_sounds.rs::the_new_track_button_is_in_view_without_scrolling`,
  both languages; the shots are on that screen),
  and the picker found by "salamander" with its details (vendor Alexander Holm, Keys ·
  Samplers, Pack, Licence CC BY 3.0). Long names elide on the cards' covers as every long
  name does; the full name is under each card.

New tests (each seen failing first; the SFZ fix seen failing on the real file first):
`cantus-content/tests/tar.rs` — a tar.gz pack's strip/include/exclude, receipt and cleanup;
GNU long names and pax paths from Python's `tarfile` (fixtures `gnu.tar.gz`, `pax.tar.gz`);
`../`, `../` inside, absolute, drive letter, backslash, symbolic and hard links refused with
nothing installed or left in staging; a gzip bomb, a 16 GiB header and an oversized zip
entry refused; a damaged checksum and a truncated archive refused; tar.gz needs format 2;
(ignored) the real archive. `cantus-content/tests/instruments.rs` — instrument validation
(id, name, file inside the pack, `.sfz`, layers ≥ 1, gain finite within ±24, unique ids,
a files pack must carry the file, format 2); the receipt keeps them, `Manager::instruments`
lists them, still after the manifest drops the pack, gone with it.
`cantus-ui/tests/pack_sounds.rs` (synthetic FLAC + SFZ in Salamander's layout, written with
`cantus-export`'s FLAC encoder) — controller-triggered and key-less regions left off the
keys and logged; thinning keeps N layers, the softest and the loudest among them, every
velocity covered once, release zones untouched; the picker offers the installed
instrument on Keys with its author, found by "piano" and "exemplar", in Spanish by its
Spanish name, not when not installed; choosing it adds a track with an Exemplar whose
zones point at the installed files, the pack's gain, one undo step, and replaces a
track's instrument in one undo step; it plays the FLAC samples, louder layers for harder
strikes (rendered offline); *Sound packs…* names the instruments before the install and
offers *New track with …* after it; (ignored) the real Salamander's load, memory, level
and listen-check WAV. `cantus-ui/tests/about.rs::help_credits_opens_about_at_the_credits`
— the Help menu's *Credits* opens About at the credits band.

Files outside my area this time: `crates/cantus-ui/src/picker.rs` (the `Sound` variant,
`is_instrument`, `apply_pick`'s first branch), `swap.rs` (`available`), `picker_catalog.rs`
(`build_with`, `sound`, `key_of`), `views/picker.rs` (`refresh` takes the sounds; its two
callers), `views/picker_panel.rs` (the caller), `views/picker_grid.rs` (the picture),
`views/picker_details.rs` (two rows), `views/inst/sampler_sfz.rs` (controller-triggered
regions), `views/transport.rs` (the Help item) — each additive.

## Second review (findings 8–14, 16–18)

Each fixed on this branch with a test seen failing first.

- **8 GNU headers** — the ustar prefix (bytes 345…) is read only for POSIX magic
  (`ustar\0` `00`); GNU's `ustar  \0` keeps access/change times there, which named every
  entry `15263610101/Kit/…`. GNU `D` (dump folder) entries are folders. Fixture
  `gnu-incremental.tar.gz`, made by GNU tar 1.35 `--format=gnu -G` (owner 0, fixed mtime).
- **9 Cancel/crash** — an archive unpack starts from an emptied staging folder and clears
  it on any failure or cancel (the download is kept); a manager clears every staging
  folder a crash left when it starts.
- **10 pax records** belong to the next real entry: an `L`/`K`/`g`/`x` header in between
  reads its own size.
- **11 Sparse** — pax `GNU.sparse.*` and old-GNU `S` entries are refused ("a sparse file:
  packs hold plain files only") instead of writing the map as data.
- **12 Windows aliases** — `check_path` refuses components ending in a dot or space and
  device names (CON, PRN, AUX, NUL, COM1–9, LPT1–9, with or without an extension); an
  archive (zip or tar) whose entries fold to the same Windows file (case, trailing
  dots/spaces, duplicates) is refused, so the receipt counts distinct files.
- **13 V7 folders** — a file entry whose name ends in `/` is a folder.
- **14 Entry budget** — each kept entry costs 1 KiB (an NTFS file record) against the
  unpack budget besides its bytes: 3 000 empty files against a 4 KB pack are refused.
- **16 Missing pack** (`views/samples_missing.rs`) — when the open project has an Exemplar
  whose samples lie in a pack's folder (`<content folder>/<pack>/…`, or `…/Content/<pack>/…`
  from another computer) and the files are not there, a kit-drawn card at the lower left
  names the pack and the tracks and says the rest plays as usual, with **Install in Sound
  packs…** (opens the window at the pack) or, when this computer has the pack at another
  place, **Use the installed pack** (repoints the files, one undo step), and **Not now**.
  Rescanned when a project is opened (`epoch`), when the installed packs change, and at
  most once a second after edits; a file that comes back (the pack reinstalled at the same
  place) is retried by the engine (`SampleCache::retry`, `CantusApp::resync_engine`, a new
  `pub(crate)` method in `app.rs`). Pictures `content-missing-*` and
  `content-missing-elsewhere-*` (1280×720 at 100 % and 200 %, EN/ES, Night/Paper).
- **17 Receipts from a newer Cantus** — an unknown shelf reads as `Other`
  (`#[serde(other)]`), so the pack stays installed.
- **18 `on_loccN=-1`** — SFZ's "no controller trigger": a keyed region carrying it plays
  from its keys and is not logged as unsupported; 0 or more is a trigger.

Tests: `cantus-content/tests/tar_review.rs` (8 — GNU times, pax record scope, sparse,
Windows aliases and `check_path`, V7 folders, empty-file flood, cancel/crash staging,
unknown shelf); `cantus-ui/tests/pack_missing.rs` (6 — named with its tracks and installed
from Sound packs in both languages, the rest of the project still renders, another
computer's project uses the local copy in one undo step, Not now, any content folder name,
reinstalling here retries the files); `pack_sounds.rs::
an_unassigned_controller_trigger_keeps_a_keyed_region`. Finding 15 (loading on the
interface thread) belongs to the sampler builder and is not touched here.

## Third review (missing packs 1–5, tar 1–3)

Each with a test seen failing first (for the missing-pack ones, by putting each old
behaviour back into the new code: six of the eleven tests fail then).

- **Missing packs, off the interface thread.** A frame now only hashes the project's sample
  paths and track names (no disk, no allocation). A scan starts only when that hash, the
  project (`epoch`) or the installed packs change — `Manager::generation`, bumped by every
  finished install and every removal, so reinstalling the same version counts — and runs
  on a worker thread (`cantus-missing-packs`); the frame takes its answer when it is ready.
  Measured: 639 pack files and sixty edits that touch no sample start no scan, the worst
  frame stays under 8 ms in the test build, and changing a sample path starts exactly one.
- **One detector over every pack file** the project plays: Exemplar zones, Pulsus pads and
  their layers, and audio clips' sources (`Owner`), each relinked with its own command
  (`SetInstrumentText`, `SetPadSample`, `SetPadLayers`, `SetAudioSourcePath`) in one undo
  step.
- **The right words.** Three states: *not installed* (no receipt), *incomplete* (installed,
  but files the project plays are not in its folder — deleted, folder renamed or moved;
  *Install again in Sound packs…*) and *at another place* (this computer's copy serves at
  least one file: *Use the installed pack*, then *incomplete* for what it cannot serve).
  *Find the folder…* repoints the files a chosen folder holds (a renamed pack folder).
  The card never says "not installed" for an installed pack.
- **Reinstall retries at once**: the generation changes, the scan runs, and every file that
  came back is retried by the engine with no edit needed.
- **Layering**: the cards sit in the middle layer, so windows open above them take their
  clicks, and they step aside while *Sound packs…* is open (a click on the window where the
  card was reaches the window). Pictures of the whole workspace with the card and with
  *Sound packs…* opened from it: `content-missing-*` and `content-missing-*-window-*`
  (1280×720 at 100 % and 200 %, EN/ES, dark and light).
- **Tar 1, 8.3 short names**: `check_path` refuses a component shaped like an 8.3 short name
  (one to six characters, `~`, digits, an extension of at most three: `LICENC~1.TXT`), and
  every unpacked file is created new (`create_new`): staging starts empty, so a file that
  already exists is an alias the file system itself merged, and the archive is refused —
  whatever the alias rule.
- **Tar 2, concurrent installs**: each install unpacks into its own folder
  (`.staging/<id>.<process>.<n>`) while holding the pack's lock file (`.staging/<id>.lock`,
  `File::try_lock`); a second install of the same pack is refused (`InstallError::Busy`), and
  the start-up cleanup removes only folders whose lock it can take, never a live install's.
  Before the move into place, the files on disk must number what the unpack wrote.
- **Tar 3, folders**: every folder an archive creates costs an entry against the unpack
  budget like a file, and a path may have at most 32 levels and 400 bytes; the unpack checks
  Cancel between entries (tar and zip).

Tests: `cantus-content/tests/tar_review2.rs` (5 — 8.3 names in tar and zip and
`check_path`, depth and length, folders against the budget, a live install keeps its
staging and a second install of it is `Busy`, two installs at once both whole);
`cantus-ui/tests/pack_missing.rs` (11, rewritten for the worker: not installed in both
languages, the card steps aside and the window takes the click, the rest still plays,
another computer's project relinked in one undo step, partly served then incomplete,
deleted files say incomplete and a same-version reinstall brings them back and retries,
Not now, any content folder name, drum pads, layers and audio clips, finding a renamed
folder, edits start no scan and a frame stays cheap with 639 files).

## Polish: the preset field names the pack instrument

Choosing a pack instrument (the picker, or *New track with …*) leaves Exemplar's preset
field reading what was loaded — "Salamander Grand Piano · 8 layers" (`samples.sound.
preset_layers`, both locales; the 16-layer entry's own name already says so) — instead of
"Init", so a preset saved from there starts from that name
(`pack_sounds.rs::the_preset_field_names_the_pack_instrument`). Picture:
`content-salamander-exemplar-*` (the real SFZ, 1280×720 at 100 % and 200 %, EN/ES).

## Left

- **Pedal noises** (controller-triggered zones), **Exemplar's disk streaming** and loading
  off the interface thread (review finding 15) now belong to this round's **sampler**
  builder. When pedal noises play, the pack's version goes up with the four pedal files
  included (they are left out today). *Done later this round by the sampler builder: the
  pedal noises play (ADR 0023's round-26 amendment, schema 24) and the four files are in
  the pack.*
- The starter card's words ("Start with a drum kit", "kicks, snares…") assume a drum pack;
  fine while the recommended pack is the CC0 drum kit.

## Notes for the integrator

- `manager::Msg::Done` now boxes its result (the receipt grew; clippy's
  `large_enum_variant`).
- Final run on this branch: `cargo test -p cantus-content -p xtask -p cantus-ui -p cantus-core
  -p cantus-project --no-fail-fast` — 106 test binaries, 1 145 tests, all pass (three
  ignored tests need the real Salamander or the network);
  `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo fmt --all` applied.
- `FORMAT` is now 2; code comparing `m.format == FORMAT` elsewhere would break (none found).
- Anyone constructing `Pack` or `Receipt` literally needs `attribution: None` and
  `instruments: vec![]` (fixed in this branch: content tests' support, cantus-ui tests,
  the samples scene, `shoot_content`). `Source` has a `TarGz` arm and `PickChoice` a
  `Sound` arm: an exhaustive `match` elsewhere needs one (none other found).
- `picker_view.refresh` takes the installed sound packs' instruments now
  (`app.samples.content().instruments()`); a branch that adds a caller must pass them.
- `THIRD-PARTY-NOTICES.md` is regenerated here; if another branch changes `Cargo.lock`,
  regenerate after the merge (`cargo run -p xtask -- notices`).
- The About window's order changed: Bible credits moved from the middle into *Credits* at
  the end.
- The blank-window-at-scale ≠ 100 % quirk may affect other `shoot` scenes that open an
  `egui::Window` at 75 %/200 % (raster.rs is not mine: a warm-up frame there would fix all).
