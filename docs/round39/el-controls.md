# Round 39 — `el-controls`: the control families in Slate

Branch `r39/el-controls`, on `r39/integration` (the kit's Slate foundation, ux-a, ux-b,
memory). Built from `des-controls`' owner-approved sheets
(`F:\projects\daw-r39\des-controls\docs\round39\design\des-controls\`, README = the measured
spec) and the icon family's set 5 (`docs/round39/design/icons/spec-rest.md`). No schema
change, no ADR of its own (ADR 0109's signs drawn, ADR 0111 §8's control sheets).

## What changed

Every painter keeps its signature and branches on the theme's system: a **recessed** family
(Slate, Graphite, Studio ink) draws the new families; **Classic** (high contrast, users'
older themes) draws exactly as before, so the kit's golden gallery (classic tokens) does not
move.

### Tokens (`kit/tokens.rs`, every theme file)

New, in both locales (`kit.token.*`), in the theme editor and the inspect mode:
`accent.automated` (`auto`), `accent.modulated` (the sheet's `mod`), `accent.destructive`
(the sheet's `danger` — the kit's `danger` is record red), `accent.primary_ink`,
`depth.scrim`, `text.disabled` (ink 4), `meters.mid`, `mixer.fader_cap_top/middle/foot`
(`fcap_*`), `knob.cap_lit/middle/shade` (`cap_*`). Slate's are the design's values; where a
promise needs it they moved the least (Paper's automated `#7a4fd6 → #784dd2` and modulated
`#b83a8a → #b23886` to 4.5:1 on a bar; Paper's meter middle `#9aa62c → #8e9928` to 3:1 in
its white groove). The other themes derive them from their own colours
(`hue_mod`, `hue_space`, their danger darkened to hold white at 4.5:1…), user themes from
their palette. New promises in `Tokens::pairings` (driven values 4.5:1 on deck, panel and
strip; the buttons' words; the meter middle) and in `floor()`, so the knobs (hue, contrast)
cannot break them.

### The driven signs (`kit/drive_sign.rs`, through `kit::automate::signs`)

The four signs of `signs.py`, each its own shape **and** colour: automated (a lane with two
nodes, `auto`), modulated (one period of a sine, `modulated`), MIDI learned (a socket, the
accent), VCA (a V, ink 2), drawn per height (stroke and nodes by device pixels).
`automate::signs` stays the one entry point (ADR 0109): `automate::driving(d)` tells the next
painter what drives it (the kit's `Param` does), and a knob, a fader, a slider or a value bar
then paints its arc, pointer, index and number in the sign's colour and wears the sign — in
the knob's open quarter (a dot on the small knob), 7 pt left of a fader's cap, before a
bar's number. A control a view painted itself and only then handed to `signs` still gets
it: the last painted shape is noted (`automate::note_painted`) and `signs` paints the
clip's arc and pointer over it and the sign where its shape says; anything else wears the
signs small at its upper right.

### The families

- **Knobs** (`kit/knob_slate.rs`, `kit/cell.rs`): groove, value arc (from the top when
  bipolar, with the notch), cap lit from above (rasterised once per size and theme into the
  atlas: shadow, rim, radial light, crescent), pointer; rest / hover / dragged / focused /
  disabled / automated / modulated (swing band at 38 %, live dot) / MIDI learned. The cell:
  name above in ink 2, value below in ink 1 with its unit a size down in ink 3 — accent
  held, `auto` when a clip drives it. Sizes: Mini = the sheet's S (20), M = 32, L = 52,
  drawn centred in the kit's unchanged allocations.
- **Faders, meters, sliders, value bars** (`kit/fader_slate.rs`, `kit/meter.rs`,
  `kit/slider.rs`): 2-pt groove, scale on the right, 0 dB detent either side, the 20 × 18
  cap (`fcap_*`, lit top, grips, engraved index); the meter as two 3-pt bars in wells of
  their own (5 on the master), colour cut from one gradient by height, 1-pt hold, clip lamp
  over the bars; the slider and the value bar new painters.
- **Fields and drop-downs** (`kit/well.rs`): typing = the well to its edge in a 2-pt ring of
  the accent at 55 %; drop-down chevron 7 × 4 in an 18-pt zone, pointing up when open, with
  the focus ring.
- **Menus** (`kit/menu_slate.rs`, `kit/menu.rs`): raised sheet, 24-pt rows, the 26-pt check
  column, shortcuts in ink 3, a submenu's chevron in their place, quiet title-case headings,
  the accent pill. **In a popup the popup's frame is the sheet** (`automate::frame`,
  `clips_chrome::menu_frame`) and each list paints its rows only — the track's menu is one
  menu, not boxes in boxes (ux-b's item 2, first half).
- **Switches** (`segmented.rs`, `chrome.rs`, `led.rs`, `mod.rs`'s focus ring): segmented
  (lifted neutral pill, never coloured), panel tabs (cut from the body, focus bar), floating
  scrollbars (4-pt thumb at rest, lane and grips under the pointer), the enable lamp (lit
  disc with halo / hollow ring), LED buttons as recessed keys with the lamp, the focus ring a
  hairline 2 pt out.
- **Tips** (`kit/hint.rs`'s `tip`): the tooltip sheet with its three lines, the driven sign
  leading; the keycap (`kit/dialog.rs`).
- **Buttons and the dialog frame** (`kit/dialog.rs`): primary / secondary / destructive /
  quiet × rest, hover, pressed, focused, disabled, busy; the frame (scrim, sheet, title,
  close ×, footer band).
- **Icon set 5** (`kit/glyph.rs`, data `kit/glyph_rest_data.rs` generated by
  `tools/icons/glyph_rest.js`): the whole Glyph → family map of spec-rest.md and set 5's
  drawings at every size; `kit::icons` takes the family's drawing wherever one exists (sets
  3 and 4 arrive with el-mixer's and el-views' data).

### ux-b's two menu items

- **Inside the window**: `kit::menu::fit` keeps a popup's contents within the window (a
  menu taller than the room scrolls; egui then keeps the sheet on the screen); the track's
  menu uses it (`views/arrangement.rs`, one line), and a row the keys light in a scrolled
  menu is brought into view (`widgets::menu_list`).
- **Elided rows**: the kit's menus are as wide as their longest row with the Slate measures.

## Files outside the kit (one line each)

`views/arrangement.rs` (the track menu in `menu::fit`), `views/clips_chrome.rs`
(`menu_frame` is the kit's sheet in a recessed family).

## How it is tested

- `kit::knob_slate::tests::the_control_tokens_are_the_designs_and_keep_their_promises`.
- `kit::drive_sign::tests` — every sign a shape of its own at every height (one ink, masks
  differ); an automated knob wears the clip's colour told before painting and told after,
  and none when nothing drives it.
- `kit::glyph::family_tests::set_5_draws_every_key_glyph_at_every_key_height`.
- `tests/menus_in_window.rs` (through `assemble`, 1600 × 900): the track's menu opened by
  Shift+F10 and by the right button lies inside the window, walked to its foot too, and the
  text probe finds no word of it cut.
- Kept green: ux_help_line, ux_whole_words, ux_signs_e2e, click_paths, panels, the kit's
  lint, tokens and gallery for every theme it draws, the instrument and effect editors'
  tests. Tests that read a caps label (an LED key's word, a cell's name) now read Slate's
  title case (`app_flows`, `topbar`, `hosted_*`, `inst_*`, `fx_multiband`, `fx_speculum`;
  each line marked `r39:el-controls`).
- `cargo clippy -p cantus-ui --all-targets` with `-D warnings`: clean. `cargo fmt -p`.
- **Failing on the base already, not touched here** (the same in `el-views`' run): the
  title-case top menus (`app_flows::clip_editing_is_visible_in_the_menu_bar`,
  `the_toolbar_is_drawn_by_the_kit`, `about::help_credits…`,
  `keyboard_reach::every_row_of_the_menus…` on "FILE"), `click_paths::p09` (the tempo well
  moved), `topbar::the_window_title…`, `font_coverage::only_plex…`, the glass display's
  Paper golden.

## The budget (counted in every run; release timing left to the integrator)

| | before (ADR 0108 / base) | now |
|---|---|---|
| Slate Night, whole 1080p window playing | 41 134 triangles, 318 allocations, 90 draws | **40 446** triangles, 318 allocations, 90 draws |
| mixer64 wired, cables all (limit 120 000) | — | **108 534** triangles, 19 allocations, 9 draws |
| mixer64 wide, desk alone | — | 83 516 triangles, 19 allocations, 9 draws |

The knob's groove, notch and cap, and the fader's cap, are one cached layer each (keyed by
size, theme and state); a first version drawing the groove as a vector arc reached 125 844
triangles on the 64-strip desk and was moved into the layer.

## Pictures

`docs/round39/el-controls/` — `<family>-<night|paper>-<1x|1.5x>.png` from
`cargo run -p cantus-ui --example shoot_controls -- <dir>` (knobs and the signs, faders,
meters, sliders and value bars, fields, menus, switches, dialogs and buttons, tips), and
`window-song-slate_<night|paper>-1280x720.png` (100 %) and
`window-song125-slate_<night|paper>-1920x1080.png` (125 %), and
`window-slate_<night|paper>-unsaved-1280x720.png` (a dialog on `kit::dialog` over the window;
`shoot_window`'s new `unsaved` case).

## Verified only in emulation

Everything was drawn by the CPU renderer and read on the pictures; no monitor, no real
pointer.

## Finished after the first report (the coordinator: no leftovers)

- **Automated, held**: a violet tick across the groove where the clip had the knob when the
  hand took it (`automate::held_clip`, set by `Param` at the drag's start).
- **Stepped knobs**: detent dots in place of the groove, the chosen one larger
  (`knob::stepping`, `Param::steps(n)`; the effects' stepped cells use it, `views/fx/controls.rs`).
- **The VCA's ghost cap**: a dashed cap where the VCA puts the level, the V beside it
  (`automate::vca_ghost`, which the desk calls before painting a governed fader).
- **A value's unit in ink 3**: one run still (tests read it whole), the unit's section in the
  installed recessed family's ink 3 (`tokens::unit_ink`, in the galley's key).
- **Keycaps on the help line**: `kit::hint::keys` lays the key names (Ctrl+Z, Shift, Esc,
  Alt of Alt+drag, F1–F12…) as keycaps; the device windows' footer uses it where it fits.
- **The dialogs on `kit::dialog`**: `window` (frame, no egui title bar, in front),
  `title`/`heading` (the title row and close key, the scrim), `footer` (the band, the
  buttons right to left, "Esc cancels"), `button_ui`; `sheet`/`head` for the dialogs laid
  out in an `Area`. On it: unsaved changes, recovery, record mode's three questions, the
  demos picker, MIDI import and export and the report, the shortcuts window, update, about,
  stems, template name, start page, quantise audio, the transcription, the clean editor,
  video export, the layout name, the dance import (both); `interop::form::button`/`buttons`
  and `workspace_widgets::button` paint the kit's buttons. Each view line is marked
  `r39:el-controls`.

## What is left

- **Settings, first run and export** wear `el-topbar`'s frame (`workspace_settings::frame`,
  their branch); at the merge the two frames should become one (`kit::dialog` or theirs).
- **The desk** (`el-mixer`) calls `automate::vca_ghost` with a VCA's level for the ghost cap
  to show on the strips; the top bar's hint panel (`el-topbar`) can use `hint::keys`.
- Icon sets 3 and 4: their data arrives with `el-mixer` and `el-views`.
- The owner's questions from the sheet stand: the automated knob's violet, the fader cap's
  weight, Paper's menus and dialogs, value bars in the inspector, Mute on = grey.
