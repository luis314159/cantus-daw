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
