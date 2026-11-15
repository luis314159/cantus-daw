# Round 40 — builder `look`: the owner's answers on the look

Branch `r40/look` (worktree `F:\projects\daw-r40\look`), base `f00ed550`. The owner's answers
1, 2, 4, 12, 17 and 18 to round 39's decisions (`docs/round39/integration.md`, «The owner's
answers»). Each is configurable, with his answer as the default: *Settings ▸ Appearance ▸ The
window* holds five choices. They are settings of this machine (`settings.json`, `look`) and never
go into the project. ADR 0110 and ADR 0111 are amended (ADR 0111 §10).

## What changed

| # | The owner's answer | Default | The other choice | Where |
|---|---|---|---|---|
| 1 | The mixer's right dock pinned at the window's edge | **At the window's edge**: the next empty strips, numbered, fill the room before it | *After the last strip* (round 37–39) | `views/mixer.rs` (`dock_at_edge`, `geometry`, the ghosts' span) |
| 2 | Maximise keeps the inspector beside the area | **The inspector beside it**, on its side, at its width; `I` hides it alone | *Nothing: the whole workspace* (round 39) | `views/workspace_fold.rs` (`toggle_maximise`, `inspector_beside`, `drop_kept_inspector`), `views/workspace.rs` |
| 3 | The top bar configurable | **None** (round 39's bar) | *Automation arm*, *Typing keyboard*, *Both* | `views/transport_keys.rs` (`auto_arm`, `typing_key_in`), `views/transport.rs` (widths and their cache key), `views/transport_panel.rs` (the typing key in More's *Inputs*), `views/auto_modes.rs` (the arm's behaviour split from its paint) |
| 4a | Value bars in the inspector | **Value bars** (des-controls' dense rows: a 20-pt well filled to the value, the number inside) | *Slim bars* (round 39) | `views/inspector_look.rs` (`value`, `bar_rect`) |
| 4b | Mute on in colour | **In colour**: a muted strip's M (and a muted track's M in its header) lit in the theme's accent | *Grey* (round 39) | `views/mixer_look.rs` (`Pal::mute_on`), `views/arrangement_head_paint.rs` (one line) |
| 4c | Paper's menus and dialogs improved | In a light theme: a crisper sheet edge, a contact shadow under the edge, a lighter scrim, a firmer menu hover and firmer separators | — (Night unchanged) | `kit/dialog.rs` (`sheet_edge`, `sheet_shadow`, `scrim_colour`), `kit/menu_slate.rs`, `views/workspace_settings.rs` (the scrim) |
| 5 | Slate Lapis's room bluer | **C, the bluest**, the owner's choice: saturation × 4.2 Night, × 5.5 Paper (round 39: × 1.7 / × 1.8) | — | `theme_slate_lapis.rs` (the tokens; the rule `turned` / `with_room` / `at_gain`) |
| 6 | Bordeaux and Pewter softer | Bordeaux: the wine room gain 1.6 → 1.05, the rose gold `#eba487` → `#e0a892`. Pewter: the lime, orange and meters a step less saturated, the room gain 1.05 → 0.85 | — | `tools/themes/themes.py` (the specs), which regenerated `theme_added.rs` and `theme_redrawn.rs` |

- **Settings.** `views/owner_look.rs` defines the settings struct `LookSettings`, the Settings section,
  `set` (which applies a change at once: an area maximised now is re-maximised the new way) and
  `apply` (called at start-up in `CantusApp::assemble`).
  - A small UI-thread copy holds the choices the painters read where no app is at hand: the M
    key, the inspector's rows, the More panel's height.
  - The labels are in both locales (`locales/areas/owner_look.{en,es}.txt`).
  - A settings file written before round 40 opens with the defaults.
- **Groups for FL users.** The manual (`docs/manual/{en,es}/README.md`, chapter 6) now says it:
  FL has no "group" button, you group by routing. Cantus offers a bus (*Create submix from
  selected*, as the demo's *Drum bus*) or a VCA group. The manual's chapter 3 describes the five
  settings, and the top bar section describes the extras.

## Slate Lapis's room — the strengths, and the owner's choice (item 5)

Every room token (the 28 surfaces, wells and lines of Slate's ladder: panels, the bars, the
arrangement's and the mixer's grounds, the wells, the grid, the menus) is built the same way.
It keeps Slate's own colour at the same WCAG luminance, turns it to the lapis's hue (222° Night,
226° Paper), and multiplies its HSL saturation by a gain.

The same luminance keeps every contrast Slate holds, so every strength keeps ADR 0111's floors.
`every_stronger_room_keeps_every_promise` checks each candidate against every kit promise.

| | Night gain | Night panel · bar | Paper gain | Paper panel · bar |
|---|---|---|---|---|
| Slate (reference) | — | `#1d2329` · `#262d34` | — | `#f4f6f8` · `#e4e9ee` |
| round 39 | × 1.7 | `#1c2232` · `#242c3f` | × 1.8 | `#f4f5fa` · `#e5e8f4` |
| A | × 2.5 | `#17213a` · `#1f2b48` | × 2.8 | `#f3f5fc` · `#e3e8f9` |
| B | × 3.3 | `#122143` · `#1a2a51` | × 4.0 | `#f3f5fe` · `#e1e8fe` |
| **C — chosen** | **× 4.2** | **`#0c204c` · `#13295d`** | **× 5.5** | **`#f3f6ff` · `#e1e8ff`** |

The owner chose C ("me gusta más la C, los más azules"). The shipped tokens are the rule's at
× 4.2 and × 5.5, held by `the_written_room_is_the_rule_at_its_gain`. One adjustment for the
calm rule: Paper's track veil stays round 39's paper (`#f4f6fa`); a veil as blue as the new
room put the blue tracks' clip bodies at rest at chroma 20.4, past `arrangement_legibility`'s
calm of 20. Night's veil follows its panel as before.

What stays unchanged:
- the accents;
- the selection tint;
- the playhead line;
- the gold readouts;
- plain Slate.

## How it is tested

| Test | What it holds |
|---|---|
| `tests/owner_look_e2e.rs` (new, through `CantusApp::assemble`, 1920 × 1080; each choice made by a click on its words in Settings, and kept across a restart) | (1) Maximise keeps the inspector at its side and width; `I` hides it alone; *Nothing* gives the whole workspace; choosing the inspector again while maximised brings it back at once. (2) The top bar: no extras by default; *Both* puts the arm key on the bar (a click arms) and the typing key on the bar or, folded with the inputs, in More (a click turns it on); each extra alone; in Spanish nothing overlaps on the bar and the arm key stays on it. (3) The right dock at the edge with the next strips before it, or just after the strips. (4) The inspector's Level section painted as value bars, none with *Slim bars*. (5) Two muted strips' M keys lit in the accent, not with *Grey*. |
| `views::owner_look::tests` | The defaults are the owner's answers; a settings file from before round 40 loads with them; a lit M reads in **all 24 themes** (the letter 4.5:1 on the accent with the desk's ink and the header's; the lit key 3:1 on its well). |
| `theme_slate_lapis::tests` | `the_written_room_is_the_rule_at_its_gain`; `every_stronger_room_keeps_every_promise` (× 1.7 … × 5.5, both lights); the existing ones (Lapis is Slate but for its room, selection and metal; the room keeps Slate's luminance and the lapis's hue; the gold readouts read). |
| `views::arrangement_legibility` | Across all 24 themes, with Bordeaux and Pewter softened and Lapis C. |
| `tools/themes/themes.py` | The checker measures every promise of the softened pair before it writes. |

## Pictures (for the owner)

Made by `cargo run -p cantus-ui --example shoot_look40 -- docs/round40/look --all`. Each is the
whole window at 1920 × 1080, on the song the program opens with, playing in a loop, the Chords
track chosen. The `-old` shots carry round 39's choices and round 39's Paper sheets; the shots
without it carry the new defaults.

- `mixer-slate_{night,paper}-{100,150}[-old].png`: the right dock at the edge with the next
  strips before it (before: just after the strips), and two muted strips' M keys in colour
  (before: grey).
- `maximise-…`: the arrangement maximised with the inspector beside it (before: the whole
  workspace).
- `inspector-…`: the mixer with the Chords strip chosen and Level and Sends open, as value bars
  (before: slim bars).
- `topbar-…`: the bar with both extras (before: without). `topbar-slate_{night,paper}-100-es.png`
  shows Spanish at 1920 × 1080.
- `menu-…`: the bar's Edit menu open. `dialog-…`: the unsaved-changes question. These show
  Paper's sheets before and after; Night's are unchanged.
- `settings-slate_{night,paper}-100.png`: *Settings ▸ Appearance ▸ The window*.
- `arrangement-` and `mixer-r39_bordeaux-100.png` beside `…-bordeaux-100.png`, and
  `…-r39_pewter-100.png` beside `…-slate-100.png` (Pewter's id is `slate`): Bordeaux and Pewter
  before and after.
- `lapis/room-*.png`: the three strengths he chose from. `lapis/chosen-{arrangement,mixer}-slate_lapis_{night,paper}-{100,150}.png`: C as it now ships.

## Verified only in emulation

Everything here was seen only on the CPU renderer's pictures and in headless frames, with
synthetic clicks — never on a monitor or with a real mouse:
- the dock at the edge;
- maximise with the inspector;
- the bar's extras and its fold;
- the value bars and the coloured M;
- Paper's sheets;
- Lapis C;
- the softened Bordeaux and Pewter.

The owner judges all of it on his screen; the steps are in `docs/beta-checklist.md`, «Ronda 40 —
`look`».

## What is left / for the owner

- **The fold with both extras.** At 1920 × 1080 and 100 % the bar with both extras folds the
  inputs, and the typing key with them, into More: in English too, not only in Spanish (without
  the extras, English folds only the tools). The arm key always stays, because the transport
  never folds. If he wants the typing key always on the bar, another group must fold first:
  his decision.
- **Paper's sheets** are a proposal against "to be improved". He judges whether they now look as
  finished as Night.
- **The headless click.** In the test rig, a click on a Settings segment did not take until the
  page had stopped moving. The test now clicks a word only after it has stood still for a frame,
  and tries up to three times. Nothing suggests a fault with a real mouse.

## Files outside `look`'s list (for the integrator)

- `views/arrangement_head_paint.rs`: one line, the header's M colour.
- `views/auto_modes.rs`: the arm key's sense, paint and behaviour split, so the bar draws it in
  its own style.
- `settings.rs`: one field. `app.rs`: one start-up line. `views/mod.rs`: one module.
- `views/workspace_settings.rs`: one call (the section) and the scrim.
- `docs/manual/{en,es}/README.md`, `docs/beta-checklist.md` (appended).

## Run at the end (Windows)

- **The whole suite:** `cargo test -p cantus-ui --no-fail-fast`, first pass: 2 650 passed and
  10 failed, across 291 binaries. Every failure was this branch's change meeting an older
  assertion, and each was fixed and run again:
  - The lib's maximise test expected the whole workspace. It now sets *Nothing*, and a new
    unit test holds the inspector kept by default.
  - The Lapis hue check allowed too little for near-white. It now allows 14° above lightness 0.97.
  - `arrangement_legibility`: Lapis Paper's bodies were at chroma 20.4 (see the veil, above).
  - `mixer_desk::the_desk_fills_its_width_with_the_next_empty_strips` now measures "filled"
    up to the dock where it is pinned.
  - `slate_window_golden` (6 shots) was **blessed** (`CANTUS_BLESS=1`) for the intended
    changes: Slate's window with the dock at the edge and the inspector's value bars.
- **Second pass:**
  - the lib: **537 passed**;
  - `mixer_desk` 38, `owner_look_e2e` 5, `slate_lapis_e2e` 1 and `slate_window_golden` 6:
    all passed;
  - the binaries the late changes touch, all passed (154 tests in 24 binaries): `kit_gallery`,
    `kit_lint`, `kit_tokens`, `kit_parts`, `kit_alloc`, `kit_glyphs`, `inspector`,
    `final_look`, `modulators`, `learn_everywhere`, `modulate_everywhere`, `topbar`,
    `topbar_layout`, `panels`, `menus_in_window`, `lo_arrange_e2e`, `el_views_e2e`,
    `slate_e2e`, `slate_budget`, `mixer_icon_e2e`, `cables39_e2e`, `at_rest_e2e`,
    `frame_budget`, `workspace_budget`.
- `RUSTFLAGS="-D warnings" cargo clippy -p cantus-ui --all-targets`: clean.
- `cargo fmt -p cantus-ui`: done. It also reflowed two `pads` lines in the shared `i18n.rs`;
  I reverted that, so the integrator's `cargo fmt --all` makes that change.
- **Not run:** the Linux check (WSL), which is the integrator's.
