# Verses at start — sources, licences, how they were taken

Cantus shows a Bible verse at every start (BACKLOG §6c; `src/verse.rs`,
`src/views/verse.rs`), on by default, switched off in *Settings ▸ General* or with *Don't
show again* on the verse itself.

| File | Translation | Language | Licence |
|---|---|---|---|
| `en.txt` | World English Bible (the edition that says "LORD") | English | Public domain |
| `es.txt` | Reina-Valera 1960 | Spanish (default) | © — quoted within the publisher's allowance, credited |
| `es-1909.txt` | Reina-Valera 1909 | Spanish (one setting away; the fallback) | Public domain |

Every file holds the **same passages, in the same order** (`tools/list.txt`, 281 passages,
293 verses), one per line: `PSA 33:3 | text` (USFM book code, chapter:verse or
chapter:first-last). The screen shows the reference with the book's name in the interface
language (`locales/areas/verse.*.txt`) and the translation's short name after it:
*Salmos 33:3 · RVR1960*. Because the verse on screen is a reference, switching the
interface language shows the same passage in the other translation.

**No text was typed from memory.** `tools/build_verses.py` takes each passage from the
sources below and writes the three files; it fails rather than drop a passage that is
missing or too long for the splash (250 characters).

## Sources (taken on 2026-10-05)

- **World English Bible** — eBible.org, *World English Bible Updated* (ID `engwebu`,
  "Last updated 2026-10-02"): https://ebible.org/Scriptures/details.php?id=engwebu
  - text: https://eBible.org/Scriptures/engwebu_vpl.zip (`engwebu_vpl.txt`)
  - Psalm titles (to take off verse 1): https://eBible.org/Scriptures/engwebu_usfm.zip
  - Licence, from that page: *"The World English Bible is in the Public Domain. That
    means that it is not copyrighted. However, 'World English Bible' is a Trademark of
    eBible.org."* The text is unmodified, so the name may be used.
- **Reina-Valera 1909** — eBible.org (ID `spaRV1909`, public domain):
  https://ebible.org/Scriptures/details.php?id=spaRV1909, text
  https://eBible.org/Scriptures/spaRV1909_vpl.zip (`spaRV1909_vpl.txt`).
- **Reina-Valera 1960** — Bible Gateway, one page per passage:
  `https://www.biblegateway.com/passage/?search=<passage>&version=RVR1960`
  (e.g. https://www.biblegateway.com/passage/?search=Colossians%203%3A23&version=RVR1960).
  The verse text is the page's `span.text` elements, without headings, Psalm titles,
  verse and chapter numbers, footnotes or cross-references.

## Why the World English Bible, not the KJV

Both are free to bundle in most of the world, but the KJV is under Crown patent in the
United Kingdom, and the WEB is public domain everywhere. The *Updated* edition is used
because it says "LORD" where the classic WEB says "Yahweh": what English readers know from
their own Bibles, and the counterpart of the Reina-Valera's "Jehová".

## The Reina-Valera 1960: the owner's decision and its terms

The owner chose the RVR1960 (2026-10-05), relying on its quotation allowance. The terms,
read on the publisher's own page on 2026-10-05 — American Bible Society, *Rights and
Permissions*, https://bibles.com/pages/american-bible-society-rights-and-permissions —
*Summary of Fair Use Policy (all conditions must apply)*:

- no more than **500 verses** in total may be reproduced without permission — the list
  has **293**, and `tests/verse.rs` holds it under 400;
- Scripture may not make up more than **25 % of the new work** — a few hundred short lines
  in a digital audio workstation;
- the verses quoted do not amount to **50 % of a complete book** — the test holds every
  book under 15 % (the largest share is Philippians, about 10 %);
- all Scripture must be properly cited with the copyright notice;
- the product must be available for **non-commercial use** — Cantus is free.

The acknowledgement Cantus shows (About box; and here):

> Reina-Valera 1960 © Sociedades Bíblicas en América Latina, 1960. Renovado © Sociedades
> Bíblicas Unidas, 1988. Utilizado con permiso.

The same page words its own notice slightly differently: *"El texto Bíblico ha sido tomado
de la versión Reina-Valera © 1960 Sociedades Bíblicas en América Latina © renovado 1988
Sociedades Bíblicas Unidas. Utilizado con permiso."* Either carries the same facts; if the
publisher asks for its own wording, change `verse.licence.rvr1960` in
`locales/areas/verse.*.txt`.

**Still open (BACKLOG §6c):** whether a GPL program — which others may sell — counts as
"available for non-commercial use". Until the publisher answers in writing, `es.txt` is
**data under its own terms, not under the GPL**: a redistributor who sells Cantus, or who
is unsure, deletes `es.txt` (or empties it) and Cantus falls back to the public-domain
RV1909 by itself (`Translation::for_locale`). The switch between the two is also one
setting: *Settings ▸ General ▸ Spanish Bible*.

## What the build changes, and why

Only typography, never words:

- **Psalm titles** ("A Psalm by David.", "Salmo de David.") are not part of the verse in
  print, but the eBible.org verse-per-line files run them into verse 1: they are taken off
  (WEB: the USFM `\d` title; RV1909: everything before the word the 1909 sets in capitals).
- **The 1909's chapter capitals** ("EN el principio", "CANTAD á Jehová") are set as an
  ordinary capital, and its **[bracketed] supplied words** (printed in italics in the book)
  lose the brackets.
- **Poetry lines** are joined with a space; the RVR1960 keeps the capital it gives each
  line.
- A passage that begins mid-sentence begins with a capital on screen.
- Passages with "Selah" in the middle of a sentence were left out of the list.

## Rebuilding

```
py -3 tools/build_verses.py <work-dir>
```

`<work-dir>` holds `engwebu_vpl.txt`, `usfm_engwebu/`, `spaRV1909_vpl.txt` (unzipped from
the URLs above) and a `bg/` folder the script fills with the Bible Gateway pages (fetched
with curl, through the system's TLS; kept, so a rebuild needs no network). Then
`cargo test -p cantus-ui --test verse`.
