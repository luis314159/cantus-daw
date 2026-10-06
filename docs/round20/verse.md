# Round 20 (second wave) — verse: a verse at every start

BACKLOG §6c. Cantus is made with Christian values; its motto is Colossians 3:23. Every
start shows a Bible verse with its reference, in the interface language — on the splash,
then in a card in the window while the work begins — **on by default, with an off switch**
(the owner: so someone who does not share our beliefs can turn it off).

## The owner's decisions (2026-10-05)

- **On by default**, *Settings ▸ General ▸ Show a verse at start* (On/Off), and *Don't show
  again* on the verse itself; remembered in `settings.json` (`verse.show`).
- **Spanish: the Reina-Valera 1960**, relying on the American Bible Society's quotation
  allowance (relayed by the integrator during the round, replacing the brief's "RV1909
  now"). The terms were read on the publisher's page
  (https://bibles.com/pages/american-bible-society-rights-and-permissions, 2026-10-05):
  under 500 verses, under 25 % of the work, under 50 % of any book, cited with the notice,
  the product available for non-commercial use. The list has **293 verses** (281
  passages); the largest share of a book is Philippians at about 10 %. The acknowledgement
  is in the About box and in `locales/verses/README.md`, and every reference on screen
  ends in "· RVR1960".
- **The public-domain Reina-Valera 1909 is bundled too**: one setting away (*Settings ▸
  General ▸ Spanish Bible*) and the automatic fallback if `es.txt` is ever removed or
  emptied — so the switch to a free text stays one setting (or one deleted file) away.
- **English: the World English Bible** (public domain everywhere; the KJV is under Crown
  patent in the UK), in its *Updated* edition, which says "LORD" where the classic WEB
  says "Yahweh" — what English readers know, and the counterpart of "Jehová".
- **The language switches live**, and the verse follows: the card keeps a *reference*, not
  a text, so switching Settings ▸ Language shows the same passage in the other
  translation (WEB ↔ RVR1960, or RV1909 if chosen).

## What was built

- **Data** — `crates/cantus-ui/locales/verses/`: `en.txt` (WEB), `es.txt` (RVR1960),
  `es-1909.txt` (RV1909), one passage per line (`PSA 33:3 | text`), the same 281 passages
  in the same order; `tools/list.txt` (the curated list, by theme: music and song, praise
  and thanks, work, gifts and making, strength, trust, peace, light and love);
  `tools/build_verses.py` (takes the exact text from the sources — eBible.org's VPL/USFM
  files for the WEB and RV1909, Bible Gateway's RVR1960 pages — and fails rather than drop
  a passage); `README.md` (sources, URLs, dates, licences, what the build changes —
  typography only — and what is still open).
- **Model** — `src/verse.rs`: `Translation` (WEB, RVR1960, RV1909: locale, short name,
  name and licence keys, the compiled-in file, parsed once), `Reference` (USFM book,
  chapter, verse or run), `parse`, `pick` (splitmix64 over the list), `Shown` (a reference
  in the current language's translation, and its citation), `VerseSettings` (`show`,
  `spanish`) and `Launch` (reads `settings.json` before the window opens, sets the
  language, chooses the passage).
- **Kit** — `src/kit/prose.rs`: prose in a box. A paragraph is broken into lines at spaces,
  measured with the role's own face (one layout, the glyphs' positions), each line painted
  with `text::paint` — so the kit's rules hold: measured before painted, never outside its
  box, shortening reported.
- **Views** — `src/views/verse.rs`:
  - *the splash's band*: the splash grows by 136 pt when a verse shows (520 × 456); under
    the unchanged composition, a hairline ornament with a small gold diamond, the passage
    centred in the condensed italic — as large as fits in four lines (×1.16 … ×0.92),
    five at most — the reference in the section caps in the brand's gold
    ("COLOSENSES 3:23 · RVR1960"), and a quiet *Don't show again* at the bottom right
    (clicked on the splash, the band says where to turn it back on and the switch is
    turned off once the application is in);
  - *the start card*: the same passage, carried over, at the bottom-right of the window
    above the status bar — the raised panel of the kit's floating windows, a gold edge,
    the passage, the reference, *Don't show again*, ✕. Not modal; it goes on ✕, after 45 s
    (one ticker wake-up, no frames meanwhile), or when the switch is turned off;
  - *Settings ▸ General* (the first section of the settings window): *Show a verse at
    start* On/Off, *Spanish Bible* Reina-Valera 1960 / 1909;
  - *About*: "Bible verses at start", each translation and its licence (the RVR1960's
    acknowledgement word for word).
- **Strings** — `locales/areas/verse.{en,es}.txt` (the 66 book names in each language,
  the settings, the link, the licences), registered in `i18n.rs::sources()`.
- **Start-up** — `lib.rs::run` reads `settings.json` (a few hundred bytes) before the
  window opens, to size the splash; the verse files are compiled in and parsed on first
  use. Nothing waits on the network or a thread.

## Tests — `crates/cantus-ui/tests/verse.rs`

- every verse file parses, has 200+ passages, no markup, brackets or double spaces;
- every locale has a translation in its own language, for either Spanish choice;
- every translation holds the same passages in the same order;
- the Spanish list stays inside the RVR1960 allowance: < 500 verses (held ≤ 400), every
  book under 15 %;
- every book is named in both languages; labels ("Lamentaciones 3:22–23");
- references parse; bad lines are refused; the motto is in every translation;
- the switch is on by default (also for a settings file written before it existed); off,
  no verse at launch (read from the file), and the card hides at once;
- *Don't show again* turns the switch off and remembers it;
- switching the language shows the same passage in the other translation (card: WEB →
  RVR1960 → RV1909 → WEB; splash: WEB ↔ RVR1960);
- every passage of every translation fits the splash at 75 % and 200 % (≤ 5 lines,
  nothing shortened, text, reference and link inside the band and apart);
- every passage fits the card at 75 % and 200 % (inside the window, below the toolbar,
  even in a 512 × 320-point window), and the longest of each translation painted;
- the card goes by itself; picks spread over the whole list.

Renders: `cargo run -p cantus-ui --example shoot_verse -- <dir>` — the splash and the card
(the motto and the longest passage of each translation) in English, RVR1960 and RV1909 at
75 %, 100 % and 200 %, and the settings window in both languages.

## Files touched outside the verse's own

`app.rs` (the `verse` field, one call in `frame`, the About lines), `boot.rs` (the splash's
size and band, *Don't show again* on it, the card handed to the application),
`lib.rs` (`pub mod verse`, the launch read and the splash's size), `settings.rs` (the
`verse` field), `i18n.rs` (two lines), `kit/mod.rs` and `views/mod.rs` (one line each),
`views/workspace_settings.rs` (one line: the General section first).

## Left

- **Written confirmation from the publisher** (BACKLOG §6c): whether a GPL program — which
  others may sell — counts as "available for non-commercial use". Until then `es.txt` is
  data under its own terms; a redistributor who sells Cantus deletes it and the RV1909
  takes over by itself (`README.md` says so).
- The ABS page words its notice slightly differently from the acknowledgement the owner
  gave ("El texto Bíblico ha sido tomado de la versión Reina-Valera © 1960 …"); both are
  recorded; one key changes it.
- The About box is still egui's stock window (not this round's); the credits were added in
  its style.
- Other locales (Almeida, Louis Segond 1910, Luther 1912) when those languages come.
- The RVR1960 prints each poetry line with a capital; joined into one paragraph that shows
  mid-sentence ("Cantadle canción nueva: hacedlo bien…"). Faithful to the text; a future
  look could set poetry in its lines.
