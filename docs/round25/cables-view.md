# Round 25 — the cable view's options (builder *cables-view*)

**Branch:** `r25/cables-view` (worktree `F:/projects/daw-r25/cables-view`), from master
`fe66f79`. **Decision:** ADR 0043 §7 (amended). **Pictures:**
`F:/projects/daw-r25/cables-view-shots/`.

## The request

The owner, 2026-10-11: "For the cables in the mixer I'd like an option to show only the
current channel's sends, or more view options, to make working easier and to see more or
less information as one wants."

## What was built

The *Off · Selected · All* switch stays as it was. Right of it, a **funnel key** opens the
cable view's **options menu** (the kit's menu, FL's rows and bands). The key is lit (the
highlight glyph, the key's lit face) whenever anything differs from the default, so a
hidden kind is never forgotten; a second click on it closes the menu.

| Band | Rows | Default |
|---|---|---|
| Selected strips show | What goes out · What comes in · Both ways (radio) | Both ways |
| Kinds | Outputs · Sends · Sidechain keys (checks) | all on |
| Detail | Send levels · Outputs to the master (checks) | both on |
| Quick views | Everything (default) · Only the selection's sends · Only what feeds the selection (radio: which one the options are, none when custom) | Everything |

- **Scope** (`CableScope`): *out* is the selection's output, sends and keys; *in* is
  what arrives at it — feeders' outputs, sends, keys onto its inserts; *both* is round
  25's behaviour. In *Selected* it decides what is drawn, in *All* what is lit (the rest
  faint); hovering a strip follows it.
- **Kinds**: a hidden kind is filtered first in `mixer_cables::collect`, so it is neither
  drawn nor hit-tested; a selected cable whose kind is hidden is let go, and Delete never
  removes it (Delete only removes a cable drawn on the last pass).
- **Send levels** off: no pill, no *PRE* tag, no tap chip beside the selected send's pill
  (the cable keeps its pre-fader gap; the level stays in the hint bar, the send row and
  the inspector). **Outputs to the master** off: the selection's master cables go — noise
  with a big selection, where they all converge. *Send levels* is disabled while sends
  are hidden, *Outputs to the master* while outputs are.
- **Quick views**: *Only the selection's sends* is the owner's case in one click (out,
  sends only); *Only what feeds the selection* answers "what reaches this bus?"; both put
  the view on *Selected*. *Everything (default)* is the one-click way back.
- The switches keep the menu open, so several are set in one visit while the band
  redraws under it; a quick view closes it.
- **The legend** under ROUTING dims a hidden kind's sample and word, and its hover adds
  "Hidden by the cable options: Outputs, Sidechain keys".
- **Narrow desks**: the folded title row (a single cables key) keeps the options key
  while it has room; the desk-wide *Solo off* / *Unmute all* keys give way to it first.
  Where even that does not fit, the desk's ▸ menu has the same rows as a submenu,
  *Cable options ▸* — the options are always reachable.
- **Persistence**: `DeskPrefs::cable_opts: CableOpts`, kept per window layout in
  `mixer.json` like the width mode and the view. `#[serde(default)]` on both structs: a
  desk saved before (or with only some fields) reads as the default. No schema change, no
  `Command` (view preferences, not project data).

Two fixes the pictures asked for on the way:

- The mixer menus measured only their action rows, so a long band heading was cut
  ("LOS SELECCIONADOS MUESTR…"): `width_of` now measures headings too (row inset and
  heading margin included).
- On a narrow desk at 200 % the title row's "Mixer" word lay over the cables key (this
  was there before the options): the word now gives way when it would touch the keys at
  the left (the panel's tab says "Mixer" already).

## Files

`views/mixer_layout.rs` (`CableScope`, `CableKindSel`, `CablePreset`, `CableOpts`),
`views/mixer.rs` (`DeskPrefs::cable_opts`, `title_rect`, `open_cable_options`, the hint
word), `views/mixer_cables.rs` (the filter and the scope in `collect`, pills behind
*Send levels*), `views/mixer_paint.rs` (the options key, the folded row's room, the
legend's dimming and hover, the title word giving way), `views/mixer_menu.rs`
(`Menu::Cables`, `Sub::Cables`, the actions, menus open while switching, the popup's
anchor so the key toggles, band headings measured), `kit/glyph.rs` (`Glyph::Filter`),
`locales/areas/cables.{en,es}.txt` (18 keys each), `examples/shoot_mixer.rs` (variants and
the Cantus Night theme), `tests/mixer_cables_view.rs` (new), ADR 0043 §7.

## Tests (`tests/mixer_cables_view.rs`, 11, plus a `mixer_layout` unit test)

Seen failing first: the eight behaviour tests ran against the stub (the types and the
prefs field in place, nothing wired) and failed; the round-trip test passed from the
start (it holds serde's defaults). The whole-menu test was seen failing under the old
heading margin.

- `cable_options_round_trip_and_old_desks_read_as_today` — JSON round trip, a round-25
  `mixer.json` desk reads as the default, a partial one fills in, and the options survive
  a restart through the file.
- `only_sends_shown_draws_and_selects_no_output_or_key` — only sends drawn (the
  selection's own), a click where the hidden output ran selects nothing; keys alone; in
  *All* too.
- `the_in_scope_draws_the_feeders_cables` — *in* draws the feeders (lit) and not the
  selection's own; *out* the reverse; *All* + *in* lights only the feeders.
- `delete_on_a_hidden_kind_does_nothing` — hiding the selected cable's kind lets it go,
  `deletable_cable` is none, Delete leaves the route; shown again, Delete removes it.
- `detail_switches_hide_pills_and_master_outputs` — no pill, no grab area, no *PRE* on
  the band without send levels; no master route without it.
- `the_options_menu_switches_kinds_and_resets` — by real clicks: the key beside the
  switch opens the menu, a kind switches with the menu open, *Everything* resets and
  closes, a quick view from *Off* shows *Selected*, the key toggles the menu.
- `the_desk_menu_holds_the_cable_options` — the desk's ▸ menu has the submenu, a switch
  there keeps it open.
- `the_legend_says_which_kinds_are_hidden` — the legend's hover names the hidden kinds.
- `the_title_row_never_overlaps` — 1280 px at 75/100/200 %, and 720/900 px at 200 % (a
  desk beside the inspector), English and Spanish, filtered or not, with *Solo off* and
  *Unmute all* showing and a long strip name: no key overlaps another or leaves the row,
  no word lies over a key not its own.
- `the_options_menu_reads_whole_in_both_languages` — every heading and row uncut, at 100
  and 200 %.
- `the_options_words_exist_in_both_languages` — every key in English and Spanish, the
  Spanish its own.

## Pictures

`shoot_mixer` variants `-cables-sends` (also `-es`, `-75`, `-200`), `-cables-in` (the
drum bus selected: what feeds it), `-cables-in-all`, `-cables-lean` (no levels, no master
outputs), `-cables-menu` (the menu open, keys hidden; `-es`, `-200`), in Slate, Paper and
Cantus Night, at 1280 × 720 and 1920 × 1080; `-desk-menu` shows the new submenu row.
Read at both sizes and in both languages; the cut heading and the narrow-desk overlap
were found in them and fixed.

## Left

- At 1280 × 720 the desk is short enough that the band sits at its least (20 pt) and
  the legend's three sample lines do not show (the hover still says it all) — as before.
- At 200 % on a 1280 × 720 window the options menu is taller than the window below the
  key and is moved up over it (the kit's menu placement); it reads whole, and Esc or a
  click outside closes it.
- The matrix view has no such filter; it shows every route by design.
