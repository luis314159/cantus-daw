# Round 42 — `violet-paper`: Violet Paper

The owner, 2026-11-26: *"Pon un agente a hacer un violet paper theme please"*. Branch
`r42/violet-paper` from `r42/integration` (e63ceddd2). ADR 0111 §13.

## What was built

**Violet Paper**, a twenty-fifth theme, light only, in Settings ▸ Appearance's *New* group
(its own tile; *Violet Paper (light)* / *Papel violeta (claro)*). It is a spec in
`tools/themes/themes.py` (`violet_paper`), built and measured as the round-39 themes are, and
written by `themes.py --write` into `crates/cantus-ui/src/theme_added.rs`
(`violet_paper_tokens`, `violet_paper`). No schema or model change: a preset is an id.

| Role | Colour | Why |
|---|---|---|
| gutter / base / bars / wells | `#fbf8fc` / `#f6f1f8` / `#ece2f1` / `#e1d6e7` | Slate Paper's ladder at 280°, × 1.45 its colour, light × 0.975 (WCAG terms): warm off-white, a faint lavender cast, not pure white |
| text / secondary | `#231830` / `#5d4f71` | deep violet-grey inks, not black |
| accent (keys, selection, focus, primary, menu hover) | `#6a46b8` | amethyst; tones derived as Slate's |
| selection tint | `#6a46b8` at alpha 26 | over every selected fill, as Slate Lapis Paper |
| lane veil | `#f6f4f7` at alpha 150 | near-neutral, as Slate Lapis Paper's: the lavender base laid the violet tracks at chroma 20.4, past the calm 20 |
| readouts | `#4b2f8a`, `#5c4a7e` | deep violet on the clock and the rulers |
| playhead, loop edges | `#9a6e1c` | the logo's gold, never the selection's colour |
| automation | `#2c5bc4` | cobalt: Slate's violet `auto` would sit beside the amethyst |
| shadows, scrim, bevel shade | violet-grey (`#2e2440`, `#2a2238`, `#463a5a`) | Slate's are blue-grey |
| meters, LED, record, function hues | Slate Paper's (danger `#bd3131`, modulated `#ae3783` moved the least for 4.5:1) | |

System: Slate's (Barlow, recessed keys, Flow cables).

**Iterations** (all on the whole window, `docs`-free, in `F:\projects\daw-r42\tmp\violet-paper\`):
the first draft (280°/× 2.4/f 0.96) read pink-mauve in the mixer; four calmer rooms were judged
side by side (`var/grid-arr.png`: × 1.8; 268°/× 1.5; 276°/× 1.4; 286°/× 1.5); 280°/× 1.45 sits
between the cooler lavender and the rosier one.

## Measured contrasts

Text 15.2:1 on the base, 13.4:1 on the bars, 12.0:1 on the wells, 12.3:1 on a selected strip;
secondary 6.7 / 5.9 / 5.3:1 (base, bars, wells); amethyst 5.9:1 on the base, white on it 6.6:1;
selection 5.3:1 on a strip, 4.8:1 on a selected one; readouts 8.1:1 and 6.1:1 on the bar;
playhead 3.9 / 4.1:1 on the rows; meters 3.9 / 4.1 / 4.6:1 in their groove; LED 3.3:1 and
record 4.6:1 on a strip; lit key glyph 3.8:1; edge 3.8:1 and knob track 3.3:1 on a card.
ΔE (room + half the accent) 18.9 from Slate Lapis Paper, the nearest light theme (≥ 14 held).

## Files

- `tools/themes/themes.py` (the spec), `docs.py` (the module's words), `contact_sheet.py` (its
  name), `variant.py` (new: a spec's variant as a theme file, to judge it on the window before
  writing it).
- `crates/cantus-ui/src/theme_added.rs` (generated), `theme.rs` and `kit/appearance.rs`
  (`PRESETS` 24 → 25), `views/workspace_settings.rs` (the tile), `locales/areas/look.{en,es}.txt`.
- `crates/cantus-ui/examples/shoot_violet.rs` (new): eleven views in any themes or theme files,
  side-by-side sheets.
- Tests: `tests/themes39.rs` (Violet Paper held as the round-39 themes: promises as written,
  round 39's system, Slate's ladder, told from every other theme; Slate's system), `tests/themes_e2e.rs`
  (new e2e: chosen on its tile through `assemble`, room / amethyst / gold playhead painted,
  restart keeps it), `tests/menus_opaque_themes.rs` (Violet Paper among the looks).
- `docs/adr/0111-the-design-system.md` §13.

## Verified only in emulation

Every picture is the CPU renderer's. The owner's eye on a real screen decides the cast; the
spec's numbers (`hue`, `gain`, `f`) are the knobs.
