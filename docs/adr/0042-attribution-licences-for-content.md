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
