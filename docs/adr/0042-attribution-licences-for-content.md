# ADR 0042 — Content under attribution licences (CC BY) is accepted, with the credit shown

**Status:** Accepted · **Date:** 2026-10-10 · **Round:** 25 · **Amends:** ADR 0001 (bundled
content), ADR 0030 §5 (default content is CC0 only) and its open question 1

## Context

ADR 0001 allowed only original or CC0 samples and presets, and ADR 0030 §5 kept every
downloadable pack CC0, with a test that fails otherwise, leaving CC BY "until the owner
rules on it". The round-25 instrument research (`research/instruments-next/notes.md`)
found that several of the best free libraries are CC BY, not CC0: the **Salamander Grand
Piano** (CC BY 3.0), **Naked Drums** and the **DrumGizmo** kits, Dustyroom's acoustic kit
(CC BY 4.0), and **VocalSet** and the choral datasets for voice work. CC BY allows
commercial use, copying and changes; its one condition is credit to the author.

The owner ruled on 2026-10-10: *"citing is no problem — we accept that."*

## Decision

1. **Accepted licences for content** (samples, presets, impulse responses, voice data —
   downloaded as packs per ADR 0030, or original content shipped in the repository):
   **CC0 1.0** and **CC BY 3.0 / 4.0**. Original Cantus content stays CC0.
2. **Still refused**: anything **NC** (non-commercial — Cantus may be sold, BACKLOG §6f), **ND** (no derivatives — an instrument transforms its samples), **SA**
   (share-alike — its reach over music made with the samples is unclear; out unless the
   owner rules again on a specific pack), retired Creative Commons licences (e.g.
   Sonatina's Sampling Plus), and anything whose licence texts disagree between sources.
3. **The credit is never hidden.** A CC BY pack's manifest entry carries its credit line
   (author, title, licence, link) exactly as the licensor asks; Cantus shows it in
   *Sound packs…* (before and after the download), in *About ▸ Credits*, and in the pack's
   receipt; `THIRD-PARTY-NOTICES.md` gets a *Content* section generated from the manifest
   (`xtask notices`). The licence file stays required beside the pack (ADR 0030 §2, step 4).
4. **Users are told plainly**, on the pack's page: the pack is CC BY; music made with it is
   theirs; whether a published track must credit the samples is the user's to judge (most
   such libraries say it need not; the pack page quotes the author's own words when they
   give them).
5. **The test changes with the rule**: the manifest test of ADR 0030 §5 accepts
   `CC0-1.0`, `CC-BY-3.0` and `CC-BY-4.0`, and fails for a CC BY pack without a credit line
   or licence file; everything else still fails it.

## Consequences

- The Salamander Grand can be offered as a download for Exemplar now, before the modelled
  piano (BACKLOG §3e, Chorda) exists; the acoustic-drum and voice items gain real content.
- A few more strings and a credits view to maintain; the generated notices keep them in
  step with the manifest.

## What the tests hold (when the change lands in `cantus-content`)

- The manifest test accepts exactly CC0 1.0 and CC BY 3.0/4.0, and requires a credit line
  and a licence file for every CC BY pack.
- `xtask notices --check` fails when a CC BY pack's credit is missing from
  `THIRD-PARTY-NOTICES.md`.

## What landed (round 26, 2026-10-13)

The rule is now code in `cantus-content`, `xtask notices` and the interface
(`docs/round26/content.md` has the detail and the pictures).

- **Manifest** (`manifest.rs`): `Licence` names the three accepted ids (`CC0-1.0`,
  `CC-BY-3.0`, `CC-BY-4.0`, spelled exactly); any other id fails validation with "not
  accepted". The licence URL must be that licence's own text (`…/publicdomain/zero/1.0/`,
  `…/licenses/by/3.0/`, `…/licenses/by/4.0/` — a `by-nc/` or `by-sa/` text under a CC BY
  id fails). A CC BY pack carries `attribution` — `title`, `author`, an `https` `url`, and
  optionally `line` (the licensor's own wording, used word for word) and `note` (the
  author's own words about crediting, quoted on the pack's page). Without it the pack
  fails. The licence file stays required for every pack (ADR 0030 §2). `Pack::is_cc0`
  is gone: `licence_kind()`, `needs_credit()`, `credit_line()` replace it. The composed
  line is `“title” by author (url), licensed under CC BY n (licence url)`.
- **Manifest format 2.** A manifest that carries a credit line must say `"format": 2`;
  a format-1 build then refuses it by name ("update Cantus") instead of failing on an
  unknown field, and a format-1 manifest with a credit line is invalid. The bundled
  manifest is format 2, because it carries the CC BY Salamander (below); a manifest
  without a credit line may still say format 1 and loads as before. `make_manifest.py`
  writes format 2 only when a pack has a credit line, and refuses an unaccepted licence or a CC
  BY pack without one.
- **Receipt**: `cantus-pack.json` keeps the pack's `attribution` (absent for CC0, so a
  CC0 receipt is byte-for-byte what it was); receipts written before round 26 load with
  none (`serde(default)`).
- **`credits::list`**: one list for every view — each manifest pack (CC BY with its full
  line, *required*; CC0 with its makers, *courtesy*) and any installed pack the manifest
  no longer lists, from its receipt.
- ***Sound packs…***: a CC BY pack's page has a *Credit* band with the line (the
  manifest's before the download, the receipt's after it, with "kept in the pack's
  receipt"), the licence row says "free for any use, with credit to the author", and §4's
  plain words: the pack is CC BY n; use it in any music and change the sounds; the one
  condition is credit, which Cantus gives; the music is yours; whether a published track
  must credit the samples is the user's to judge — followed by the author's own words
  when the manifest quotes them, or by "most libraries like this one say it need not".
  The browser's one-click offer of a CC BY pack names the licence and the author.
- ***About ▸ Credits***: a *Credits* section at the end of About (so the buttons above
  never move with the list's length), reached by a *Credits* button beside the documents
  (`about::show_credits` for a menu): the Bible translations of the verse at start, then
  every sound pack — "Name · CC BY 3.0 · credit required" with its line, "Name · CC0 1.0 ·
  credited as a courtesy" with its makers.
- **`THIRD-PARTY-NOTICES.md`**: the hand-written *Sound packs* section of
  `xtask/notices/head.md` became a generated ***Content*** section (CC BY packs with their
  line, licence file and the author's words; CC0 packs in a table). `notices --check` (and
  `cargo test -p xtask`) names any CC BY pack whose credit line is missing, even when the
  file is otherwise current.
- **The Salamander Grand Piano is the first CC BY pack** (CC BY 3.0, Alexander Holm, the
  FreePats SFZ+FLAC edition): a `.tar.gz` source (a strict tar reader of our own; names
  that climb out, links and archives larger than the manifest says are refused, for zips
  too), SHA-256 pinned, `readme.txt` as the licence file. The four pedal-noise files were
  left out at first (Exemplar could not play them); they are in the pack since the
  sampler's controller-triggered zones (ADR 0023's round-26 amendment, schema 24). The manifest is format 2. Its two instruments
  (8 and 16 velocity layers, −5.5 dB for the headroom rule of ADR 0039) are offered in the
  instrument picker once it is installed, and on its *Sound packs…* page; choosing one
  gives a track an Exemplar playing it in one undo step. *Help ▸ Credits* opens About at
  its credits. The measurements are in `docs/round26/content.md`.

Review fixes, same round: required/courtesy is decided by the licence alone (a CC0 pack
may carry a courtesy `attribution`, never shown as required); a licensor's own line is
always followed by the licence's name and link unless it contains it
(`Attribution::full`); the on-screen composed line is localised (`samples.credit.line`),
the receipt and notices keep the English one; the licence URL must be exactly the
licence's text (`Licence::is_text_url`: prefix + nothing / `legalcode[.<lang>]` /
`deed.<lang>`, no `..`, `?`, `#`, `%`; ports refused); `Attribution` and `Credit` accept
unknown fields so newer receipts still load in older builds.

What the tests hold now: `crates/cantus-content/tests/licences.rs` (exactly the three ids;
credit line and licence file required; the licence URL matches the id; the licensor's
line word for word; format 2; the receipt carries the credit and old receipts load; the
credits list), the fixture `crates/cantus-content/tests/fixtures/manifest-cc-by.json`
(a CC0 pack, a CC BY 3.0 pack with the author's words, a CC BY 4.0 pack with the
licensor's own line), `xtask/src/notices_tests.rs` (the Content section; a missing CC BY
credit fails the check), `crates/cantus-ui/tests/samples_browser.rs` and
`crates/cantus-ui/tests/about.rs` (the credit and plain words before and after the
download, in both languages; About ▸ Credits; the Credits button scrolls to them).

## Amendment, 2026-10-15 (round 28, sound): the acoustic drum kit, and packs built at install

**The kit chosen, its licence read at the source.** The owner named Naked Drums or a
DrumGizmo kit (BACKLOG §5).

- **Naked Drums is refused.** The SFZ port (`github.com/sfzinstruments/
  WilkinsonAudio.NakedDrums`) carries a CC BY 4.0 `LICENSE`, but it was put there by the
  people who converted the kit, not by its maker. Wilkinson Audio sold Naked Drums (USD 60)
  and made it a free download in May 2016 (Bedroom Producers Blog, 2016-04-30); its own
  product page, in the Internet Archive from 2019–2020, offers it free and points at the
  SFZ conversion "by some very cool people", but names no licence anywhere, and the store is
  gone. The porter's announcement (KVR, 2020-06-30) claims no permission. Rule 2's "anything
  whose licence texts disagree between sources" covers a licence the rights holder never
  stated: until Wilkinson Audio grants CC BY in writing (the owner may ask them), it stays
  out.
- **DrumGizmo's DRSKit 2.1 is accepted** (CC BY 4.0). The rights holders state it
  themselves: the kit's page on drumgizmo.org ("The DRSKit drumkit source files are released
  under the Creative Commons Attribution 4.0 International License") and the `README.md`
  inside the archive (which the pack keeps as its licence file). Their page adds: "When the
  samples are used in a composition, any context in which other credits (e.g. to
  performers) are presented must include an attribution." That is the licensor saying how
  the attribution CC BY 4.0 already asks for is given (§3(a)(2): "in any reasonable manner
  based on the medium"), not a further condition; Cantus quotes it word for word as the
  author's words on the pack's page (§4), so a user publishing music knows it. It goes to
  the pre-release legal review with the rest (owner-only).
- The archive is DrumGizmo's own (`drumgizmo.org/kits/DRSKit/DRSKit2_1.zip`, 2 803 397 710
  bytes); its published MD5 (`8c4d4b61…c133`) matched the download, and the manifest pins
  our SHA-256 (`529f2dca…f090`). Virtuosity Drums (CC0) stays the source of the starter
  kit's one-shots; DRSKit is the owner's named kit, the fuller one (two kicks, three toms,
  two crashes, a ride, sticks and brushes, 13 microphones, up to 30 strengths per drum).

**Packs built at install (manifest format 3).** A DrumGizmo kit is one multi-channel file
per hit (DRSKit: 13 microphones, 32-bit float, 44.1 kHz, 4.3 GB unpacked) that no sampler
plays as it is. A pack may now carry a **`build`** step that the install runs after
unpacking, before the pack is moved into place (`cantus_content::kitbuild`):

1. read the kit (`drumgizmo.rs`: the kit file's channels and instruments, each
   instrument's samples with their `power` and which channel of which file each
   microphone is — DRSKit's files store their channels in another order than the kit
   lists them);
2. mix each hit the pack builds to **stereo**: each microphone at its gain, panned with
   the equal-power law, its polarity reversed if asked, **at its bleed level on the drums
   it does not face** (a tom's close microphone 20 dB down on a snare hit, as a mixing
   engineer's gate would have it), each instrument raised or lowered for the balance, the
   whole mix at its gain — sums in the manifest's order with `cantus_dsp::math`, written as
   24-bit WAV at the kit's rate, so every machine writes the same bits;
3. measure every hit (the RMS of its first 50 ms) and write an **SFZ** for Exemplar:
   velocity layers by level on SFZ's velocity curve (a hit `L` dB under the drum's
   loudest is what velocity `127·10^(L/40)` plays; each hit's gain makes every velocity
   play at that curve, so the loudness is continuous and the timbre changes from layer to
   layer), round robins of `round_robin` hits per layer taken **in order** (ADR 0023's
   round-28 amendment: no sample twice in a row), the keys the manifest gives (General MIDI,
   DrumGizmo's own for the rest), chokes as exclusive classes with a release, the
   manifest's labels as the groups' names, the credit at the top;
4. remove every unpacked file the pack does not keep (`keep`, the licence among them), and
   count what is left for the receipt.

The manifest pins every number of the mix (`mics`, `keys`, `chokes`, `gain_db`,
`round_robin`, `range_db`), so a pack is reviewable data and the same archive always builds
the same files; the unpack's budget is the build's `unpacked_size`; a build that fails
installs nothing and leaves nothing. The progress shows the unpacking, then *Mixing the
microphones: n of m hits*. A pack instrument may set Exemplar's `voices`, and one on the
*Drums* shelf turns Exemplar's drum rows on. Format-2 builds refuse a format-3 manifest by
name (as format 2 did).

**Mixed at install, measured against the alternatives** (DRSKit, this machine):

| | Stereo mix at install (chosen) | A stereo stem per bus (close, overheads, room) | Every microphone a zone |
|---|---|---|---|
| Disk (24-bit) | **490 MB** (471 MB after review 1's 24 dB range) | 1.47 GB | 3.2 GB |
| Heads in memory (streaming) | **123 MiB** | ≈ 370 MiB | ≈ 800 MiB |
| Voices per hit | **1** | 3 | 13 |
| Zones | **591** | 1 773 (> 1 024) | 7 683 (> 1 024) |
| Per-microphone level | fixed by the mix | per bus, by Exemplar's groups | per microphone, needs multiple outputs |

Faders for each microphone need an instrument with several outputs (BACKLOG §3e item 11,
Pulsus III's mic buses), not Exemplar's single stereo output: left there, not half-built.

**Measured** (`kit_build.rs`, `pack_drums.rs`, ignored tests on the real archive), after
review 1: the real archive installs from its file in 113–116 s (copy, unpack 4.3 GB, mix)
to 496 files and 470 879 054 bytes; peak −4.9 dBFS, nothing clamped; Exemplar imports its
548 zones in 0.09 s, holds 117 MiB of heads streaming, renders 13.6× faster than real time
on one thread; a rock groove sits at −26.0 LUFS with −8.2 dBTP, everything at velocity 127
at −0.8 dBTP — ADR 0039's headroom rule keeps the instrument's gain at 0 dB; 170 velocity
layers, each with at least two hits in turn.

**Review 1 (2026-10-15).**

- **General MIDI's keys for their own drums only.** The first map put DrumGizmo's extra
  articulations on keys General MIDI reserves for other instruments (a crash tip on the
  tambourine's 54, a hi-hat on the cowbell's 56, brushes on the congas). Now the kit's
  drums sit on their General MIDI keys (the fully open hi-hat on 46), the General MIDI
  keys it has no sound for stay silent, and the other articulations are on 82–100, above
  General MIDI's percussion (`kit_build::the_bundled_kit_plays_general_midis_keys…`).
- **The range a zone's gain can make up.** Every kept hit is raised to SFZ's curve by its
  zone's gain, which stops at +24 dB: a range of 30 dB played the softest hits up to 6 dB
  under the curve. The range is now at most 24 dB (`Build::validate`), DRSKit's is 24, and
  `layers` leaves out a hit further down whatever the range asked.
- **The download is kept until the pack is in place.** A build cancelled, failed or cut
  short by a crash starts again from the archive instead of fetching 2.8 GB again; it is
  removed only after the swap into place.
- **The space needed is stated and checked.** `Pack::space_needed` (download + unpacked +
  installed for a built pack: about 7.6 GB for DRSKit) shows on the pack's page; an
  install is refused before anything is fetched when the content folder's volume has less
  (`InstallError::NoSpace`, `cantus_rt::disk::space`), and says so in both languages.
- **Turns by drum and by layer, rows in the interface's language** (ADR 0023's amendment):
  a tom fill across 47/48/50 and an accent layer never repeat a hit; the kit's row names
  and Exemplar's group names translate (`locales/areas/drums.*`: General MIDI's 47 names,
  DRSKit's labels).
