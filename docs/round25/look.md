# Round 25 — builder *look*: the theme family and the mark as the icon

Branch `r25/look` (base 1226d3b). ADR 0046. The owner's requests of 2026-10-10: more themes,
a default that goes with the website; the website's tile-less mark as the taskbar icon, its
colour changeable, the in-app logo still configurable.

## Themes

Written once as the kit's 65 tokens each (`crates/cantus-ui/src/theme_family.rs`); the older
22-colour palette today's views read is taken from the tokens (`palette_of`).

| Theme | Light | One line |
|---|---|---|
| **Cantus Night** — *the default* | dark | The website's night stage: espresso #120d0a…#2a1d15, ivory text, copper-lit accent text, amber keys/focus/playhead, verdigris selection and meter green |
| **Cantus Paper** | light | Its twin from the website's paper sections: stone #f3f5f4/#e5eae8, ink text, copper-deep accent, verdigris-deep selection/focus, dark meter well |
| **Daylight** | light | BACKLOG §6d.4's white with blue details: near-white, cool grey-blue headers, one saturated blue for every active/selected state, dark-slate meters |
| **Blue Hour** | dark | Daylight's dark twin: navy slate, the same blue family |
| **Ultraviolet** | dark | Deep violet, neon magenta keys, cyan selection and playhead |
| **Lagoon** | dark | Cool teal slate, an amber selection (its complement) |
| **High Contrast** | dark | Black, white text (≥ 13:1), yellow keys and focus, cyan selection, bright control edges |
| **High Contrast Light** | light | White, black text (≥ 9:1), deep-blue keys and selection, a magenta focus ring |

Slate, Ink & Gold, Paper and Parchment stay selectable (listed after the family).

**The default.** New installs: dark mode on Cantus Night, light slot Cantus Paper, mode Dark
(the website's fallback and `theme-color` are night). A saved `appearance.json` keeps what it
names (tested with Slate/Paper). A pre-appearance `theme.json` equal to Paper — every release
of that era wrote it on its first run — counts as no choice. **§6d.4's white-and-blue is
Daylight/Blue Hour, not the default**: the owner's newer request names the website's theme
(ADR 0046 §1).

**Appearance page.** Each shipped theme's row has a preview swatch painted with that theme's
own tokens (toolbar with a lit key, browser text, rows, a selected clip, a section-hue clip,
the playhead, a meter).

## The icon

- **The mark** (`kit/mark.rs`, std-only, included by `cantus-app/icon.rs`): the favicon's
  geometry at ≥ 40 px; below 24 px one solid C arc with the wave's gradient (eleven 1.4-px
  spokes read as an asterisk at 16 px); below 40 px the playhead's line and crossbar on whole
  pixels. No tile.
- **The `.exe`'s and the installer's `.ico`**: 16, 20, 24, 32, 40, 48, 64, 128, 256 px of the
  mark in its designed colours (`xtask package` already takes the `.ico` this build script
  writes). The splash keeps logo #2 baked as before.
- **Colour options** (Appearance ▸ Themes and logo ▸ *Taskbar icon*): **As designed**
  (amber → copper, the default), **Theme accent** (the theme's brand colour), eight presets —
  Amber, Copper, Verdigris, Blue, Violet, Rose, Silver (for a dark taskbar), Graphite (for a
  light one) — and **Your colour** (an eyedropper chip opening a colour picker). Chips stand on
  Windows' dark and light taskbar colours. Persisted as `appearance.json`'s `icon` (serde
  default: old files load as *As designed*).
- **Live**: `window_icon::WindowIcon` renders the mark at the window's DPI sizes
  (`GetSystemMetricsForDpi` SM_CXICON/SM_CXSMICON) when the colour or scale changes and sets
  both `ICON_BIG` (taskbar button, Alt+Tab) and `ICON_SMALL` (title bar) with `WM_SETICON`
  (`cantus_launch::taskicon`; winit only sets the small one). eframe's one-shot setter is off on
  Windows so it cannot overwrite the user's colour. **Verified on this PC**: Cantus run with an
  isolated `CANTUS_DATA_DIR`; `WM_GETICON` returned the mark at 40 px/20 px (125 % scale); after
  `appearance.json` was edited to *Blue* (and the window repainted), it returned the blue mark —
  `target/look-shots/icon-{original,blue}-{big-40,small-20}.png`. Not verified: what Windows
  shows for a **pinned** running button that shares our AppUserModelID with the installer's
  shortcut (the taskbar capture was hidden behind a window) — to check on the owner's PC. Files
  (`.exe`, Start menu, pinned-and-closed, installer) keep the designed colours; the page says so.
- **The in-app logo** keeps its ten logos and tints; the tint gains **Icon colours**
  (`Tint::Icon`): the wave and playhead roles follow the taskbar icon, so the title-bar mark and
  the splash can follow it when the user chooses. Default unchanged (#2, original colours).

## Tests

`cargo test -p cantus-ui -p cantus-launch -p cantus --no-fail-fast`: **803 passed, 0 failed,
18 ignored**. `cargo clippy --workspace --all-targets -- -D warnings`: clean (one pre-existing
`iter_cloned_collect` in `tests/mixer_desk.rs` fixed — two lines, `to_vec()`).

New or extended:
- `theme::tests::every_theme_defines_every_colour_role`, `every_theme_meets_the_contrast_floor`
  (the family's floor: meters 3:1 on their well, playhead 3:1 on both rows, glyph 4.5:1 on a
  lit key, text 4.5:1 on header cells and the toolbar, LEDs/record/selection 3:1; 7:1 for
  the high-contrast pair), `the_default_theme_is_cantus_night`.
- `theme_family::tests::every_family_theme_keeps_every_promise_as_written`.
- `tests/theme_roundtrip.rs`: every preset switched into from Cantus Night equals starting in
  it, and back is Cantus Night, to the pixel; the real views through every preset and back.
- `kit::appearance::tests::a_fresh_install_shows_the_website_s_theme_and_a_saved_choice_is_kept`,
  `the_icon_colour_round_trips_and_makes_a_mark`; `kit::logo` — the Icon tint;
  `window_icon::tests` — every icon size renders with nothing behind it; a colour is applied
  once, not every frame.

## Pictures

`cargo run -p cantus-ui --example shoot_window -- target/look-shots` and
`cargo run -p cantus-ui --example shoot_look -- target/look-shots [es]` (new) write:
- `window-<theme>-{main,themes,pianoroll,es,z75,z200}-1280x720.png` for every preset;
  `window-cantus_{night,paper}-*.png` (1280 and 1920, appearance/colours/themes/settings,
  75 %/200 %, Spanish).
- `look-<theme>-z{75,100,200}[-es].png` — all twelve preview swatches and the icon chips.
- `marks.png` — the mark at 16–64 px in every icon colour on a dark and a light taskbar, 16 and
  24 enlarged; `mark-sheet.png`; `icon-*.png` from the live window.

## Decisions

- Default mode stays Dark (the website's fallback); "follow the system" is one click away.
- §6d.4's white-and-blue ships as Daylight/Blue Hour, not as the default (ADR 0046).
- The window-icon Win32 code lives in `cantus-launch` beside the AppUserModelID code;
  `cantus-ui` depends on it and keeps `forbid(unsafe_code)`.
- The title-bar mark and the splash follow the icon only when the user picks the logo's
  *Icon colours* tint; by default they keep logo #2 as designed.

## What is left

- On the owner's PC: pin Cantus, change the icon colour, and see whether the pinned running
  button follows (Windows may prefer the shortcut's icon for a pinned AppUserModelID); if not,
  set `System.AppUserModel.RelaunchIconResource` on the window or accept it and say so.
- The original gradient's pale amber end and playhead are weak on a white taskbar (≈1.4:1):
  an "auto" option following Windows' taskbar light/dark (registry `SystemUsesLightTheme`)
  could pick Copper/Graphite there.
- The base-theme menus on the Look tab list names only; they could carry the preview swatch.
- STATUS.md is the integrator's; the round's summary line: 8 new themes, Cantus Night default,
  the mark as the icon with a live colour setting.
