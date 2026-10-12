# ADR 0046 — The theme family (Cantus Night by default) and the mark as the icon

**Status:** Accepted · **Date:** 2026-10-10 · **Round:** 25 (builder *look*) ·
**Extends:** ADR 0024 (the kit: colour roles, presets, appearance, logos §12) ·
**Schema:** none (the project model is untouched; `appearance.json` gains a field with a
serde default)

## Context

The owner, 2026-10-10: *"Generate more themes for more variety, and set as default one
that goes with the web version"* and *"for the taskbar icon use the web icon (it has no
background and stands out more); make the taskbar logo's colour changeable, and keep the
logo configurable in the options too."*

The website (`web/style/base.css`) is one design in two lights: a warm **night** stage
(`night` #120d0a … `night-3` #2a1d15, `night-line` #3b2a1f, ivory `night-ink` #f6ece2,
cells #25190f/#33231a) and cool **paper** sections (#f3f5f4, `mist` #e5eae8, `line`
#cbd5d2, ink #14100d), with the logo's **copper** (#d4622a / #a94617), **amber**
(#f5ad3d / #ffdf9e) and **verdigris** (#3e8e7e / #1f5f54). It follows the system's
preference, with night as its fallback and its `theme-color`. Its favicon is the mark —
the C of waves and the playhead — with no tile behind it.

The day before (BACKLOG §6d.4, 2026-10-09) the owner had asked for a white-with-blue
theme and its dark twin as the new default. The newer request names the website's theme
as the default.

Before this round the shipped default was Slate (FL-derived, ADR 0024 §2) with Paper as
the light slot; the window/taskbar/`.exe` icon was logo #2's tiled small master; the
window's icon was set once by eframe and never again.

## Decision

1. **A theme family, written once as the kit's tokens** (`cantus-ui/src/theme_family.rs`):
   every one of the 65 colour roles explicit per theme; the older 22-colour palette that
   today's views read is *taken from* those tokens plus a text-legible accent
   (`palette_of`), so the two can never disagree. Eight themes:
   - **Cantus Night** (dark) — the website's night stage: espresso surfaces, ivory text,
     copper-lit accent text, amber keys and focus, verdigris selection and meter green,
     the mark's amber as the playhead. **The default for new installs.**
   - **Cantus Paper** (light) — its twin from the paper sections: stone surfaces, ink
     text, copper-deep accent, verdigris-deep selection and focus, the night palette's
     dark meter well.
   - **Daylight** (light) and **Blue Hour** (dark) — BACKLOG §6d.4's white with blue
     details and its dark twin: kept as two of the new themes, **not** the default, since
     the owner's newer request names the website's theme as the default. (The website
     itself is copper on night; a blue default would not "go with the web version".)
   - **Lagoon** (dark teal slate, amber selection), **Ultraviolet** (deep violet, neon
     magenta keys, cyan selection) — variety for producers.
   - **High Contrast** dark and light — for low vision: every text role at 7:1 or more
     (13:1 and 9:1 measured at their tightest), a focus ring no other role uses.
2. **The default.** `default_theme()` is Cantus Night; a fresh `Appearance` has the dark
   slot on `preset:cantus_night`, the light slot on `preset:cantus_paper`, mode Dark (the
   website's own fallback and `theme-color` are night). **A saved choice is never
   overridden**: an `appearance.json` naming `preset:slate`/`preset:paper` keeps them. A
   pre-appearance `theme.json` equal to Paper — what every release of that era wrote on
   its first run, before anyone chose anything — is read as "no choice" and gets today's
   defaults, as the migration already did for a preset equal to the shipped default.
3. **A contrast floor for the family**, on top of the kit's promises (ADR 0024 §14): text
   4.5:1 on every surface it is read on (header cells and the toolbar included); meters
   3:1 on their well; the playhead 3:1 on both kinds of row; the selection 3:1 on a
   selected strip; LEDs and record 3:1 on a strip; the glyph 4.5:1 on a highlighted key.
   The older presets keep the kit's floor only (they predate this one).
4. **The mark is the icon** (`kit/mark.rs`, std-only so build scripts include it): the
   favicon's geometry at 40 px and up; below 24 px the C is one solid arc carrying the
   wave's gradient, and below 40 px the playhead's line and crossbar sit on whole pixels
   (11 thin spokes at 16 px read as an asterisk, not a C). The `.exe`'s and the
   installer's `.ico` (`cantus-app/icon.rs`, which `xtask package` finds) hold it at 16,
   20, 24, 32, 40, 48, 64, 128 and 256 px in its designed colours.
5. **The icon's colour is a setting** (`Appearance::icon: IconColour` — `Original` (the
   default: amber → copper), `Theme` (the theme's brand colour), `Preset` (Amber, Copper,
   Verdigris, Blue, Violet, Rose, Silver, Graphite) or `Custom([u8; 3])`). One colour
   becomes the original's shape of light (`MarkColours::from_one`: a lighter tint at the
   bottom of the C, the colour, a deeper shade at the top; a light playhead). The
   Appearance page's **Themes and logo** tab shows it as chips on a dark and a light
   taskbar, with an eyedropper chip for a colour of the user's.
6. **Applied live to the window**: `apply_appearance` asks `window_icon::WindowIcon` for
   the colours; each frame `sync` renders the mark at the sizes the window's monitor wants
   (`GetSystemMetricsForDpi(SM_CXICON / SM_CXSMICON)` at the window's DPI) only when the
   colours or the scale changed, and gives both to the window with `WM_SETICON`
   (`ICON_BIG`, which the taskbar button and Alt+Tab show, and `ICON_SMALL`, the title
   bar's) through `cantus_launch::taskicon` — the crate that already owns the taskbar
   identity (AppUserModelID); `winit`'s `set_window_icon` sets only `ICON_SMALL`, so it
   could never reach the taskbar. Our icons are released once replaced. eframe's own
   one-shot icon setter is switched off on Windows (an empty `IconData`) so it cannot
   overwrite the user's colour when the window first becomes active; until the first
   frame the taskbar shows the `.exe`'s icon, which is the same mark. Elsewhere the icon
   goes through `ViewportCommand::Icon`.
7. **The in-app logo stays configurable** (logos 1–10, kit/logo.rs): its tint gains a
   third choice, **Icon colours** (`Tint::Icon`), which recolours the wave and the
   playhead roles with the icon's colours — so the title-bar mark and the splash can
   follow the taskbar icon when the user wants, and keep their own colours by default.

## Consequences

- What changes live: the running window's taskbar button, its Alt+Tab entry and its
  title-bar icon. What does not: icons Windows reads from files — the `.exe`, a pinned
  shortcut while Cantus is closed, the Start menu, the installer — which keep the designed
  colours (said in the Appearance page). Whether Windows shows the window's icon or the
  shortcut's for a *pinned* running button sharing our AppUserModelID was not verified on
  hardware this round: to check on the owner's PC.
- `cantus-ui` depends on `cantus-launch` (a small library: windows-sys only). Its unsafe
  code stays in `cantus-launch`, beside `appid.rs`; `cantus-ui` keeps
  `forbid(unsafe_code)`.
- Light taskbars: the original's pale amber end and playhead are weak on white (≈1.4:1).
  **Amended after the round's push (owner, 2026-10-11):** the default is now
  **Automatic** — the designed gradient on a dark taskbar and its deep copper twin
  (`MarkColours::ON_LIGHT`, every stroke ≥ 3:1 on Windows' light taskbar) on a light one,
  following Windows' *default mode* (`SystemUsesLightTheme`, not the apps' mode) as it
  changes, read every two seconds by the real window only (`window_icon::Taskbar`). *As
  designed* stays a choice for those who want the pale amber anyway; Graphite and Copper
  remain. A file written before loads as Automatic, which on a dark taskbar is the
  designed mark.
- `Tint::Icon` and `IconColour` are new enum variants/fields: older builds read new files
  with the defaults for what they do not know (the field) but would refuse a theme file
  whose logo tint is `Icon`.

## What the tests hold

- `theme::tests::every_theme_defines_every_colour_role` — every preset carries all 65 kit
  tokens and 22 palette colours, and is named in both languages.
- `theme::tests::every_theme_meets_the_contrast_floor` — §3's floor for the family, the
  kit's for all, 7:1 for the high-contrast pair.
- `theme_family::tests::every_family_theme_keeps_every_promise_as_written` — the raw
  tokens, before any floor, and the theme resolves to exactly them.
- `tests/theme_roundtrip.rs` — every preset switched into from Cantus Night is what
  starting in it gives, and back is Cantus Night, to the pixel; the real views through
  every preset and back.
- `kit::appearance::tests` — a fresh install shows Cantus Night/Paper; a saved Slate/Paper
  is kept; an old file without `icon` loads as `Auto`; the icon colour round-trips;
  `the_automatic_icon_reads_on_a_dark_and_a_light_taskbar` — the default follows the
  taskbar, every stroke of each mark ≥ 3:1 on its taskbar, and the designed amber fails
  on white (the reason for the twin).
- `window_icon::tests` — the mark renders at every icon size with nothing behind it, and a
  colour is applied once, not every frame; `kit::logo` — the *Icon* tint follows the icon.
