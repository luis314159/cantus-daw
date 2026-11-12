# Round 39 — after beta.14: what the release runs and the owner's screen showed

Branch `r39/unsettled`, from master `45e4bb69` (beta.14, released 2026-11-11). Five things,
each found after the tag; nothing here changes a sample or the song's schema.

## 1. A shared helper's end nothing chooses between (ADR 0063 §10)

CI's Windows run failed `session_faults9`'s hundred aborts once ("the gain not blamed in 1 of
100") — the residual ADR 0063 §9 left by design. Settled by the next end: the candidates are
suspects, told so on the card, and reloaded each runs alone for the session. The whole account
is in `abort-blame.md` ("After the tag").

## 2. The macOS budget test

`inst_analog::eight_voices_render_within_budget` took 10.19 s for 4.5 s of audio in the macOS
job's pool step (`CANTUS_WORKERS=3 --features fuzz`); one wall-clock render was the house's
only budget test not taken as the best of several. Now the best of three, the limit unchanged
(`abort-blame.md`).

## 3. The download page after a release

A release published by the workflow's own token starts no other workflow, so the site's
`release: published` never fired and the download page kept beta.13's files until a manual
dispatch. The release's publish job now starts the site's deploy (`abort-blame.md`; held by
`xtask`'s `a_published_release_starts_the_sites_deploy`).

## 4. The bar's menus (ADR 0110, amended)

The owner, on his screen: *View* ran off the foot of the window and nothing scrolled. Two
causes:

- **`kit::menu::fit` capped every menu at 400 pt.** egui sizes a popup's first frame in
  `default_area_size`; the scroll area took those 400 pt as its room, so a menu taller than
  that scrolled even where the window had room — and the bar's menus did not use `fit` at
  all, so they ran off the window instead. `fit` sets `min_scrolled_height` too, and every bar
  menu runs inside it.
- **The bar's menus were egui's own rows.** Check boxes, buttons, a submenu and a kit menu
  per action: two row heights, boxed entries (File, Edit, Clip, View and Help alike). Each is
  now one kit menu of `Row`s (`transport_menus::bar_menu`) — one row height, the key at the
  right, unavailable entries dimmed; View's layouts a band of their own; Clip's two lists
  round the colour swatches at one width. A click closes the menu and does the row's work.
  `dance_import::file_menu_item`, `video_export::menu_item`, `views::interop::file_menu`,
  `stage::view_menu` and `tempo_ui::view_menu_entry` went with them (their rows are
  `MenuCommand`s and actions now).

Looked at: `shoot_ship`'s `menu-file`, `menu-edit`, `menu-clip`, `menu-help`, `view` and
`view-short`, Night and Paper, English and Spanish. Held by `menus_in_window.rs`'s
`the_view_menu_shows_whole_where_it_fits_and_scrolls_where_it_does_not` (through
`assemble`; without the `fit` change it fails "the menu is 400 pt tall"). The paths rig's
window size is a field now (`Rig::size`, 1600 × 900 by default) so a test can make it short.

## 5. The owner's surveys

The twenty-two open decisions (`integration.md`) and Chorda's listening test, for the owner and
the people he knows:

- **Two zips** the owner hands out (`F:\projects\daw-r39\compartir\`): the decisions page (each
  decision with its pictures, options and a comment) and the listening kit — opened from a folder,
  no network; each person types their name and downloads a JSON file named after them. Two
  claude.ai pages hold the same, for the owner.
- **The listening kit** (`chorda_abx`): a name field (the results say whose, the file is named
  after them), the Salamander Grand's credit (CC BY 3.0) on the page, no `alert`/`confirm`, and
  `--flac` (`chorda_kit/render.rs::write_flac`, `cantus-export`'s encoder: lossless, 22 MB
  against 59 MB of WAV, so the zip fits an e-mail; Edge plays it). It also saves to a claude.ai
  page's store when opened as one.
- **A site version was built and dropped.** Pages under cantusdaw.com/encuesta/ answered with no
  account, the Worker keeping the answers in Workers KV behind a link's code and the owner's
  password — built, and tested locally through the real Worker. The owner judged the Cloudflare
  setup (a KV namespace, two secrets set after a first deploy, the token's KV permission) too
  much for what it gives; nothing of it was merged. The KV namespace `cantus-encuestas` he made
  is unused.

## 6. The owner's listening test: the baseline for Chorda's next pass

The owner took the new kit (beta.14's Chorda at its defaults, `chorda_abx`, 2026-11-12;
`abx/2026-11-12-luis-almazan-beta14.json`):

| Test | Result |
|---|---|
| ABX 1, Chorda 2 / Salamander | 16 of 16 (p < 0.0001): told apart in every phrase |
| ABX 2, Chorda 1 / Chorda 2 | 16 of 16: told apart in every phrase |
| Preference, Chorda 2 / Salamander | 0 – 7: the Salamander in every phrase |
| Preference, Chorda 2 / Chorda 1 | 6 – 1: model 2, but model 1 in *Bass notes alone* |

No notes were written. The owner's direction (2026-11-12): **make Chorda sound as good as
the Salamander**, in round 40 — the third voicing pass (`docs/round38/chorda.md` §2: the
bass's low-mids, the nasal middle, the dull treble, the flattened *fortissimo*, the treble's
width) aimed at preference parity, the bass first looked at against model 1, then ABX 4 (a zip
for him and the people he knows) against this baseline.
