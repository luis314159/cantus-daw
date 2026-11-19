# Round 40 — dancer-show: bringing a hidden dancer back

Branch `r40/dancer-show`, from master 2a79ded2. The owner's report (2026-11-18): *"Once the
dancer is hidden, I don't know how to bring it back in an easy way."* Shots in
`F:\projects\daw-r40\compartir\dancer-show\`.

## The cause

Two separate things hid a dancer, and they did not know about each other:

1. **A dancer's own Hide** (the pill over a floating or docked dancer,
   `views/stage_float.rs`) wrote that track's `view.mode` = 2 into the project.
2. **The stage's switch** (*View ▸ Stage*, `Action::ToggleStage`, Alt+D) flipped only
   `settings.stage.hidden`.

After a dancer's own Hide, *View ▸ Stage* stayed ticked and toggling it changed nothing on
screen; the only way back was the editor's *Where it shows* switch.

## What the user sees and does now

- **One action shows them** (`stage_show.rs`, `show::toggle`). *View ▸ Stage* / Alt+D (the
  profile's binding) / the palette: when a dancer is on screen it hides them all (the switch,
  as before); when none is, it turns the switch on and, if **every** dancer of the song is
  hidden, puts each back where it was — floating or docked — as **one undo step**
  (`Command::Batch`, `cmd.stage.show_dancers`). One Ctrl+Z hides them again. The Stage tab
  reopens when one goes back docked.
- **Where it was** is remembered per track in the session (`StageState::last_shown`: noted by
  `gather` every frame a dancer shows, and by its Hide); one hidden before the song was opened
  comes back floating. No schema change: the memory is the session's, the project keeps only
  `view.mode`.
- **The tick tells the truth**: *View ▸ Stage* is ticked only while a dancer is on screen —
  the switch on and a dancer floating, or one docked with the Stage tab in the workspace
  (`show::on_screen`).
- **Hiding tells you how to get it back**: a dancer's Hide (now its own undo step,
  `cmd.stage.hide_dancer`) leaves a card in the corner stack the audio and plug-in notices
  already use (`views/audio_notice.rs`, no new widget kind): *"Keys hidden — Alt+D or View ▸
  Stage shows it again."* with the key the current keymap gives `ToggleStage` (a keymap with
  none says *View ▸ Stage* alone), the Stage tab's figure, and **Show again**. Show again is
  the hide's own undo while nothing came after it (the history stays as it was), else the
  dancer shown as a step of its own (`cmd.stage.show_dancer`). The card goes by itself after
  12 seconds, or when the dancer shows again.
- **A way back where the dancer lives**:
  - the dancer track's menu (right-click its header, or Shift+F10) has **Show the dancer**
    while it is not on screen;
  - the dancer's window says why over its stage in Play (and under *Where it shows* in Edit):
    *Hidden: it shows on no screen*, *The stage is off: no dancer shows* or *On the Stage tab,
    which is closed* — with **Show**.
- **The Stage tab's empty state**: with hidden dancers it says *The dancers are hidden* /
  *A dancer of this song is hidden…* (or *N dancers…*) and offers **Show the dancers**, which
  brings back every hidden one in one step. A song with no dancer keeps the old words.
- **The palette** finds the stage's action as *Show the dancers* / *Bring the dancer back* /
  *Hide the dancers* (*Mostrar los bailarines*, *Traer de vuelta al bailarín*, *Ocultar los
  bailarines*) besides its name.
- A song with no dancer: the action says how to add one on the status line.

## Commands

No new `Command` variant: every change is a labelled `Command::Batch` of
`SetInstrumentValue`s (`cmd.stage.hide_dancer`, `cmd.stage.show_dancer`,
`cmd.stage.show_dancers`), whose inverse is the core's and which the undo fuzz already
covers. No schema bump.

## Tests

`crates/cantus-ui/tests/dancer_show_e2e.rs`, on the app `CantusApp::assemble` builds, drawn by
its own frame:

- `a_hidden_dancer_comes_back_from_the_notice_the_key_and_one_undo_hides_it_again` — a dancer
  added the way the picker adds one; its pill's Hide clicked; the tick off; the card's words
  name the keymap's key; its Show again clicked brings it back with the hide undone (not
  stacked); hidden again, the stage's key pressed brings it back drawn, as `show_dancers`; one
  undo hides it; the key twice switches the stage off and on, its own mode untouched.
- `a_docked_dancer_goes_back_to_the_stage_tab` — docked, hidden, the empty tab's button
  clicked: back docked, the tab open, ticked.
- `the_tracks_menu_and_the_editor_show_a_hidden_dancer` — the track's menu offers the row only
  while hidden, and its row clicked shows it; the editor's Show clicked shows it.
- `the_palette_finds_it_as_show_the_dancers_in_both_languages`.

`stage_companion::view_menu_switches_and_the_stage_key_hides_the_dancers` now adds a dancer
first: in a song with no dancer the key no longer switches the stage off unseen (it stays
on and the status line says how to add one).

**Results.** The whole `cantus-ui` suite (lib + 371 test targets, run in batches of 25 with
the binaries deleted between them — F: had 28 MB free): **2 977 passed, 4 failed**, none
from this branch: `slate_window_golden` mixer ×2 (1 774 px, the strip's polarity/swap glyphs
of master's 79e5cd01, whose goldens were not refreshed — the rendered mixer has no dancer
in it), `picker::opening_for_a_purpose_offers_instruments_or_effects` (20 instruments
against 14 expected: the picker's catalogue, untouched here) and
`scene_vector::a_lid_drag_rebuilds_within_its_budget` (a timing budget: 2 s under three
parallel builds). Clippy `-D warnings` on `cantus-ui --all-targets`: clean.

## Shots

`cargo run -p cantus-ui --example shoot_stage -- <dir> show` (with `CANTUS_SHOT_SLATE=1` for
Slate Night and Slate Paper): `show-{hidden,view-menu,track-menu,tab,editor}-{slate,paper}-{en,es}.png`
— the card after a Hide; the View menu with *Stage* unticked; the dancer track's menu (a
right-click on its header) with *Show the dancer*; the Stage tab's *The dancers are hidden*
with *Show the dancers*; the dancer's window with *Hidden: it shows on no screen* and
*Show*. The card's menu path is written *View › Stage*: the interface's font has no ▸.

## Left

- Only the owner confirms the feel of the card's timing (12 s).
- When a docked dancer is hidden, the card sits over the Stage tab's foot (both live at the
  window's bottom right) for its 12 s; both offer the same way back.
- The inspector's *Shown: Hidden* row stays words only (the track's menu and the editor carry
  the button).
- The channel rack and mixer strip have no dancer row: a dancer has no pad and no audio strip
  of its own; the track's menu, the editor and the Stage tab are where it lives.
