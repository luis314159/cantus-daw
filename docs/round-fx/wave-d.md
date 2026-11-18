# Wave D — the X-Y pad and the formula controller (steps 1–3)

Branch `fx/wave-d` from `fx/integration` 82b55e97. Provisional numbers: **schema 67**
(64→65 and 65→66 are empty steps here, marked as waves B's and C's, filled at the merge),
**ADR 0147** (the X-Y pad), **ADR 0148** (the formula controller).

## The architecture (ADRs 0147 §1, 0148 §1)

Both controllers are **modulator sources** (ADR 0057), not effect kinds:
`ModSource::XyPad`, `ModSource::Formula`. They inherit routes into every target (hosted
plug-ins, sends, note effects, another modulator's amount), *Modulate…*, the handle drop,
depth, rings, the freeze guards, bake, templates and the engine's control grid. What is
new and shared:

- **Inputs are parameters**: `AutoTarget::ModInput { modulator, input }` (the pad's X/Y,
  the formula's a/b/c) with `Command::SetModulatorInput` — so recording automation writes
  a pad gesture as **two lanes**, MIDI learn maps an axis or a knob, a clip replays it,
  another modulator drives it (one level).
- **Links**: `RouteLink { target, output, range }` kept on the controller's source by
  target — which axis a route reads and FL's link range (from, to, bent by the automation's
  curve). `ModRoute` is untouched (88 literals across the workspace stay as they are).
- **The formula language** (`modulator::formula`): FL's syntax, compiled off the audio
  thread into a postfix program run on a fixed stack; at most 256 steps, 32 deep, 48
  nested, 512 characters; `cantus_dsp::math` only; output 0…1 (FL's) or bipolar.
- **Inertia**: two equal one-pole stages in closed form, stepped at each control point over
  the exact interval (16-sample grid accurate; every grid point a control point while a pad
  with inertia plays).

## Step 1 — contract, model, engine (numbers)

- `cantus-core` **827** tests green (808 before): `modulator::formula::tests` 9 (every FL
  manual example formula: 24 waves and shapes, verbatim, against the platform libm to
  1e-6; the boolean table; refusals and where), `modulator::fxd_tests` 10,
  `controller_targets` with `ModInput` as the 17th kind, the undo fuzz's own controller
  stream (`drng`, eight kinds; two always refused). **The fuzz found a real bug**: two
  controllers driving each other's inputs recursed without end in the freeze region's
  walk (stack overflow) — fixed (one level, as amounts), and a freeze region now takes in
  what drives a controller's input.
- `cantus-project` **194** green: schema 67, the v67 fixture (a pad "Sweep" and a formula
  with an automated knob on the master), `schema_67_keeps_the_pad_the_formula_and_their_input_lanes`.
- `cantus-engine`: `tests/fxd_controllers.rs` 6 (nulls to the bit — unlinked, at rest
  with inertia on and a symmetric range, a formula of 0 and one with knobs at 0; heard and
  block-size independent 1…8 192 at tolerance 0; live at irregular blocks = offline; a
  synced formula and a path from bar 5 = the full render there; inertia glides where its
  absence jumps; a listen checkpoint's forked processor carries the puck to the bit) and
  `rt_memory::controllers_never_allocate`. Whole engine suite **1 289** green.
- Words: `locales/fx/controllers.{en,es}.txt` (`cmd.fxd_set_input`, the kinds, the inputs,
  every formula error, the faces' words).

## Step 2 — design sheets v1

`compartir/xy/design/v1` and `compartir/formula/design/v1`, each with its `index.md`.
Shot with `cargo run -p cantus-ui --example shoot_fxd -- <dir>` (the clip-aware probe on
every shot); `tests/fxd_whole.rs` holds every window, language, scale and colourway to no
cut word, and a longer language to a taller window, never a smaller display.

Verified: `cantus-ui` lib 546 green (the side round's house-rule tests included: the
first palettes clashed with Cribrum, Fractus, the EQ, Caelum Aurora and Vetus Walnut and
were moved), `fxd_whole` 2, `fx_lint`, `kit_lint`, `kit_alloc`; clippy `-D warnings`
`--all-targets` clean on core, engine, project, ai and ui; 34 shots, no cut word.

## For the owner

- The names (proposals): **Tactus** (the X-Y pad, Latin *touch*) and **Calculus** (the
  formula controller, Latin *reckoning stone*).
- The sheets, the three colourways each (Titanium/Copper/Frost; Bakelite/Ivory/Ebony).
- The formula's changed names, so a render repeats: `Time` (ms since play), `Rand`
  (a hash of the position), `SeededRand` (our hash); refused: `MouseX`, `MouseY`, `Date`,
  `Tension`, `Warp`.

## Left for step 3 (after the owner's verdict) — done in step 3, below

The windows opened from the modulator's row, the pad's drag issuing `SetModulatorInput`
(merged per gesture) with recording, drawing and recording a path, the formula's typing
(draft, caret, keypad), the engine's telemetry of the puck (both axes) for the face, bake
of a pad with inertia (trace), presets, MCP/AI words for the two kinds and `ModInput`,
manuals, an e2e through `CantusApp::assemble`.

## Step 3 — the windows, presets, bake, the agents, the e2e

The owner's verdicts (`compartir/{xy,formula}/verdict.md`, 2026-11-17): both sheets
**approved**, the names **Tactus** and **Calculus**, the pad's polarity default *Both ways*,
the formula's rules for a repeatable render as proposed. His notes on the pad (the Spanish
cards stretched with empty space; *What it drives* left a band under its rows) are fixed.
ADRs 0147 and 0148 have a *Phase B (step 3)* section each; the manual has a section for
each controller in both languages (§5, after *Modulators*), and the AI page's *Movement*
row names them.

Commits: `10f0a3ed` + `92eb2d65` (AI), `0367f73a` (core: presets, `tidy_links`, bake),
`badab3aa` (engine: telemetry, `trace_route`; the UI's bake hooks), `635f3f22` +
`da3c0341` + `ebbb1ca4` (UI: the windows, the pointer tests, the shots), `95378678`
(recording over a playing path), `aad25a3f` (the e2e), these docs, and two follow-ups after them (the links' axis and range edited in *What it
drives*; the colourways in the ⋯ menu).

What was built (files):

- **The windows** — `crates/cantus-ui/src/views/fx/fxd_window.rs` (the floating device
  window: opened from the modulator's row, *Window · Open* or a double-click on its
  picture; moved, closed by its key or `Esc`, minimised, maximised, scaled; one at a time;
  closed when its modulator goes), `fxd_edit.rs` (every gesture one command, one undo
  step), `fxd_xy.rs` and `fxd_formula.rs` (the faces), `fxd_art.rs` (the cards: a row's
  slack shared by item), `modulator_rows.rs` (the *Open* key). The pad's drag is one
  `Batch` of two `SetModulatorInput` a frame (one step a drag; **two lanes, X and Y**, when
  a pass writes); *Draw path* (≤ 64 points over the loop) and *Record path* (while the
  song plays, sampled at song beats, laid over the loop it began in, 16 points a beat;
  recording over a playing path switches it off for the drag, in the same step); the
  formula typed at a caret on its display (a draft apart from the project; each edit that
  compiles committed, one step a session; a broken draft shows its error at its character
  and never reaches the audio; `Esc` drops it; a long formula shrinks to 60 % then scrolls
  to the caret); the keypad (24 keys) and the a/b/c dials (vertical drag, double-click
  0.5); presets in the header (arrows, the list with descriptions, *Custom*); *What it
  drives* — the X/Y tag swaps a route's axis, a bar's ends set its range, its middle bends
  it, a double-click gives the whole travel back; sized to its rows, scrolls by the wheel.
- **Core** — `crates/cantus-core/src/modulator_presets.rs` (+ `_tests.rs`): four Tactus
  presets (*Filter sweep*, *Width and pan*, *Figure eight*, *Performance pad*), nine
  Calculus presets in FL's syntax on one knob plan (a the rate, `2^(Round(a·6)−4)` cycles a
  beat; b the shape; c the depth); `apply`, `matching` (look, links and inputs aside),
  `by_id`, `of_kind`. `Modulator::tidy_links` wherever a route goes.
  `modulator_bake.rs`: a controller baked as the engine plays its route. Words:
  `crates/cantus-ui/locales/fx/controllers.{en,es}.txt` (`fxd.preset.<id>[.what]`, the
  window's words).
- **Engine** — `crates/cantus-engine/src/modulator.rs`: `ModTelemetry.pads` (puck x, y,
  where it heads x, y), `pad_mask`, `pad(id)`, no allocation; `trace_route` (a route's value
  through its axis and range × amount per control point) for the bake of a pad with
  inertia or a driven controller. `processor_auto.rs`: the puck reported.
- **AI** — `crates/cantus-ai/src/tools_mod.rs`, `tools_read.rs`, `tools_words.rs`,
  `tools_json.rs`, `tools_remove.rs`, `tools_project.rs`: `add_modulator` kinds `xy_pad` and
  `formula` with their settings, routes' `axis` and `range`, `change_routes`, inputs moved by
  `set_modulator` as `SetModulatorInput` (one step a call), a bad formula refused at its
  character, a removed route takes its link, `settings` and each route's axis and range
  read back, `get_mixer` lists strip and master modulators. **Bug fixed**: a `ModInput`
  target read as the modulator's amount (`target_of` ignored `param`; `words::target` used
  the amount's phrase) — now `{modulator_id, param: "x"|"y"|"a"|"b"|"c"}`, round-tripping,
  read "X-Y pad 1 · Y" / "Formula 1 · knob b". The tools' golden regenerated on purpose
  (`crates/cantus-ui/tests/golden/mcp-tools-list.json`; only `automate`,
  `replace_automation`, `add_modulator`, `set_modulator`, `remove_items` changed).
- **Docs** — `docs/adr/0147-the-x-y-pad.md`, `docs/adr/0148-the-formula-controller.md`
  (Phase B, Consequences, tests), `docs/manual/{en,es}/README.md` (*Tactus, the X-Y pad*;
  *Calculus, the formula controller*, with the formula language for users and worked
  examples), `docs/manual/{en,es}/ai-tools.md` (the *Movement* row).

Numbers (tests written in step 3):

- `cantus-ai` `tests/tools_fxd.rs` **5** (the crate 56 at the AI's commit).
- `cantus-core` `modulator::presets::tests` **9** (8 on the presets, 1 on `tidy_links`),
  `modulator_bake` **3** new.
- `cantus-engine` `tests/fxd_bake.rs` **5**: each baked curve within **0.002** of the
  trace at the control points, the baked render within **1 % of peak** of the modulated
  render, the route audible; the telemetry of a gliding puck.
- `cantus-ui` `tests/fxd_drag.rs` **17** (through `CantusApp::assemble`, real pointer and
  key events: 11 on the pad and the window, 5 on the formula), `tests/fxd_whole.rs` **2**
  (step 2's, on the new windows).
- `cantus-ui` `tests/fxd_e2e.rs` **6** (through `assemble` with the real engine wiring,
  `EngineHandle::manual`): a pad on two EQ bands moved, each heard **±12 dB** (the other
  within 0.1 dB), dry to the bit at rest and after undo; a recorded gesture writes exactly
  **two lanes** equal to the hand at all **96** touched ticks, its render block-size
  independent (64/333/4096) and live = offline at odd blocks 1…1 024, undone to the bit; a
  path (`xy.figure_eight`) followed within **0.5 dB** over 16 beats; `formula.square` live
  = offline to the bit over 6 bars, its **±15 dB** followed within **0.3 dB**; an edit to a
  formula heard (difference RMS 0.25) and undone to the bit, knob c halving the crest
  (**+14.80 → +7.46 dB**); unlinked controllers and depth-0 routes the dry master to the
  bit.

Suites (one crate per call, 2026-11-17, a machine shared with two other lanes' builds):
`cantus-core` **839** green (1 ignored), `cantus-dsp` **1 050** (29 ignored), `cantus-engine`
**1 294** (17 ignored), `cantus-project` **194**, `cantus-ai` **56**, `cantus-ui` **2 743**
green (54 ignored) and one red that is not this wave's: `preset_render`'s CPU budget for
two Arca presets (+0.22 to +0.26 over, a different pair on each run — measured with other
builds running; Arca's high gain is ~25 % of a core on a loaded machine, PLAN's quiet-machine
measurement is still owed). `locale_isolation` found `fxd_whole` setting the process's
language; it now sets its thread's (fixed, green). clippy `-D warnings --all-targets` clean
on all six; `xtask`'s `the_manual_builds_in_both_languages_and_its_links_hold` green.
`cantus-host` was not touched and not run.

**Verified only in emulation** (material for `docs/beta-checklist.md`): the windows were
driven by egui's events through `assemble`, never by a person's hand on a screen; the sound
was measured offline and through `EngineHandle::manual`'s processor, not through a real
driver; no real touch surface or XY controller moved a pad. (`Alt+A`, `Alt+Shift+A` and
the window's own parameter menus were untested here; the follow-up below tests them.)

Shots: `C:\projects\daw-fx\compartir\xy\design\v2-step3\` (25 PNG + `index.md`) and
`C:\projects\daw-fx\compartir\formula\design\v2-step3\` (23 PNG + `index.md`): both
languages, Slate and Paper, 75–200 %, every colourway, the hand, a drawn and a recorded
path, the links, the presets; the formula typing, its error, both polarities.

**Findings for elsewhere** (pre-existing, measured by the e2e, **not fixed**: fixing them
likely changes the goldens and the `DSP_VERSION`, so they are the integrator's and the
owner's):

1. A native effect unit starts a play or a seek at its **stored** value, not the automated
   or modulated one, and glides there over its de-zipper (~20 ms for Aequalis): a render
   from bar N differs from the full render on the first note for any automated native
   effect — plain automation too (a lane holding an EQ band at +9 dB differs by up to
   5.2e-2 from bar 3). Strip pan/gain and track gain are bit-exact from bar N.
2. Firmus starts its cutoff and gain smoothers at the factory defaults on a render that
   begins on a note (it snaps them only in idle blocks).

The e2e therefore proves render-from-bar-N on the strip's pan and fader, and on the EQ
from the first full beat.

The colourways (Titanium, Copper, Frost; Bakelite, Ivory, Ebony) are chosen in the window's
⋯ menu, one undo step a choice (`the_more_key_offers_the_face_s_colourways_one_step_a_choice`).

**Exported plug-ins: nothing.** Modulators (and so both controllers) live in the host —
Cantus's project and engine; the exported CLAP/VST3 effects carry no modulators, and the
controllers have no DSP of their own to export.

**The site** (checked read-only on `fx/web`): it has **no place for modulators or
controllers**. `web/src/units_data.rs` holds only `INSTRUMENTS` (9) and `EFFECTS` (42), and
`web/src/units.rs`' `group()` has those two tabs; no other page of `web/src` names
modulators (Fluctus is there as an effect). Nothing to add to a card. For the integrator,
if the site should sell them: a third tab (*Controllers*: the LFO, envelope, follower,
steps, random, Tactus and Calculus) in `units.rs`/`units_data.rs`, or a feature block on
the studio page. The manual's pages on the site (`web/assets/manual/`) are built from
`docs/manual/` and take the new sections when they are rebuilt.

## Step 3's follow-up (2026-11-17, evening): the count, the menus, the site, the glide's test

Four gaps the integrator found in step 3, closed on `fx/wave-d` (or, for the glide, handed
over as a failing test).

**1. A count that disagreed with its list.** In `xy-links-slate-en-k100.png` the footer said
*Drives 4 parameters* while *What it drives* showed 7 rows and the rails said *+3 more*.
What was wrong: **the count's source in the design sheets** — `shoot_fxd` put a fixed string
in the footer (`fxd.sheet.chip.xy` = "Drives 4 parameters", `fxd.sheet.chip.formula` =
"Drives 2 parameters", the v1 sheets' four and two links) while the `links` state drove ten
(the formula's eight) and the rest state none. No row was listed twice: the list showed rows
4–10 of ten (scrolled three rows, its scroll bar at the side) and the rails counted five a
side. The app's own window counted right (`links.len()`, one per route, routes unique per
target), but two smaller faults were found on the way: an axis that drives nothing engraved
a dangling `X —`, and a rail too short for its two names would have cut the count off the
end. Fixed:

- `fxd_xy::drives_chip(n)` is the one source of the chip — the window's and the sheets'; the
  fixed sheet strings are gone from both locales.
- `fxd_xy::rail_words(links, axis, fits)`: the names in the list's order, as many as the
  rail holds (at most two), the rest counted (`+3 more`; `5 parameters` / `1 parameter` when
  no name fits), so the count is never the part cut; `X` / `Y` alone when the axis drives
  nothing.
- The sheets' `links` state shows its list from the first row, so the rails' names are its
  first rows and the scroll bar says there are more.
- Test: `fxd_drag.rs` `the_chip_the_list_and_the_rails_count_the_same_links` — a pad and a
  formula with **0, 1, 4 and 7** routes (the pad's on both axes: 1 on Y alone, 2+2, 4 Y + 3
  X), through `CantusApp::assemble`: the chip painted is the count; the list, scrolled
  through by the wheel, lists every route once in order; each rail's painted words, never
  cut, are its axis's routes named or counted and add up to them.
- Re-shot in place, one file at a time (no folder emptied):
  `compartir/xy/design/v2-step3/` (25 + 4 new) and `compartir/formula/design/v2-step3/` (23
  + 4 new) — every sheet, since the footer's chip is in each; looked at: the links sheets
  say *Drives 10 / Mueve 8 parámetros* over a list from its first row and rails `X — Cutoff
  · Cribrum · Pan · Lead · +3 more`; the rest sheet *Aún no mueve nada* with bare `X`/`Y`.
  The four new ones per unit are the site's pairs (below). Each `index.md` says so. The
  kit's text probe found no cut word in any.

**2. Automation and MIDI learn on the windows' own controls.** The window's controls had no
parameter menu (a right-click on a card's knob opened only its typing field, which read a
typed `64` as 64, not 64 %); `Alt+A` worked but was untested. Now every control that stands
for a parameter wears **the kit's parameter menu** (`kit::automate::menu`, reused, not a new
one): *Create automation clip* / *Edit automation*, *Modulate…*, *Type a value…* (the cards'
knobs), *Learn MIDI controller* / *Change* / *Forget MIDI controller*, *Reset to default* —
and with it the signs of what drives it, the modulators' rings, the learn mode's outline
(a click learns, nothing moves) and the Alt-drag depth. The choices are answered as
`ModAct::Param` answers the inspector's (`app.automate`, `app.modulation_choice`); a reset of
a rail or a dial is one `SetModulatorInput` (none when already there).

- **Tactus**: X and Y on **the rails** (the axis's words along the pad's foot and left side,
  with a hint) and on the *Pad* card's *Horizontal* / *Vertical* knobs; *Amount*.
- **Calculus**: a, b, c on the *Knobs* card **and** the keypad's **dials**; *Amount*.
- **Not parameters, exempt**: the pad's *Inertia*, *Path*, *Loop in beats*, *Line* and
  *Polarity*, the formula's *Polarity*. The reason: they are the controller's own settings,
  and nothing automates a modulator's settings — `AutoTarget` has `ModInput` and
  `ModAmount` only, as the inspector's rows of every modulator (an LFO's rate, an
  envelope's times) show; making them targets is a model change (schema, engine) for
  another round. They are drawn **plain** — `controls::Cell::plain()`, the fx cells' twin of
  the kit's `Param::plain` that Chorda's tuning keys use — so the modulation and learn
  probes pass them by, and a right-click on one offers no automation. Their typed values now
  read in their units (`%`, `ms`).
- Code: `fxd_art::{Param, Params, AMOUNT, param_spot}` (the cards take each parameter's offer;
  `Moved::Param`), `fxd_xy` (`Live::params`, the rails' rects, `Action::Param`),
  `fxd_formula` (the dials), `fxd_window::feed_params` (asked of the app each frame as the
  inspector asks: `mod_offer`, `is_automated`), `fxd_edit::{param_target, param_choice}`.
- Tests (`fxd_drag.rs`, through `assemble`, real pointer and key events; each mutation one
  undo step): `the_pads_axes_and_amount_offer_the_parameter_menu` and
  `the_formulas_knobs_dials_and_amount_offer_the_parameter_menu` — on each of the 5 + 7
  controls a right-click, *Create automation clip* (a clip, one step, the window stays),
  *Edit automation* (no step), *Learn MIDI controller* with a controller turned (mapped, one
  step), *Forget MIDI controller* (one step), *Reset to default* (one step);
  `a_controllers_own_settings_offer_no_automation`;
  `the_learn_mode_reaches_every_parameter_of_the_windows` (the probe, and a click on the Y
  rail / dial c arms it without moving it);
  `alt_a_automates_and_alt_shift_a_learns_what_was_moved_last_on_the_window` (X, Y, the
  amount; knob b, dial c, the amount). `learn_everywhere.rs`:
  `the_controllers_windows_offer_learn_on_every_parameter` (green: no control without
  *Learn*, the settings passed by). The menus' pictures, taken by the tests
  (`CANTUS_PATHS_SHOTS`), were looked at.
- The manual (EN/ES, *Tactus* and *Calculus*) and ADRs 0147/0148 say so.
- Two findings of this pass (`Alt+A` after a pad drag automated only Y; the *Inertia* knob
  stopped at 2 000 ms while the model takes 10 000) are closed in *follow-up 2* below.

**3. The site** — for the integrator; `fx/web` was not touched (read-only: `web/src/units.rs`,
`units_data.rs`, `web/tools/units_pictures.py`, `tools/shots/check_units.py`).

*Where they go.* `units_data.rs` holds `INSTRUMENTS` (9) and `EFFECTS` (42) and `units.rs`
two tabs. Add a third:

- `units_data.rs`: `pub const CONTROLLERS: [Unit; 2] = [ … ];` after `EFFECTS` (the two
  entries below).
- `units.rs`: `use crate::units_data::{CONTROLLERS, EFFECTS, INSTRUMENTS};`; `group()` →
  `match tab { 0 => &INSTRUMENTS[..], 1 => &EFFECTS[..], _ => &CONTROLLERS[..] }`; the
  `tabs` array gains `(l.t("Controllers", "Controladores"), CONTROLLERS.len())`.
- `web/tools/units_pictures.py`: add `tactus calculus` to `NAMES`, then run it on a folder
  holding the shots copied under its names (below); then `python tools/shots/check_units.py
  --fix` (the sizes below are computed from the shots, the check makes them exact).

*The pictures* (`units_pictures.py` reads `<name>-{slate,paper}-{100,200}.png`; English from
one folder, Spanish from another with `--es`). Copy:

| `units_pictures.py` wants | English folder, from | Spanish folder (`--es`), from |
|---|---|---|
| `tactus-slate-100.png` | `compartir/xy/design/v2-step3/xy-start-slate-en-k100.png` | `…/xy-start-slate-es-k100.png` |
| `tactus-slate-200.png` | `…/xy-start-slate-en-k200.png` | `…/xy-start-slate-es-k200.png` |
| `tactus-paper-100.png` | `…/xy-start-paper-en-k100.png` | `…/xy-start-paper-es-k100.png` |
| `tactus-paper-200.png` | `…/xy-start-paper-en-k200.png` | `…/xy-start-paper-es-k200.png` |
| `calculus-slate-100.png` | `compartir/formula/design/v2-step3/formula-start-slate-en-k100.png` | `…/formula-start-slate-es-k100.png` |
| `calculus-slate-200.png` | `…/formula-start-slate-en-k200.png` | `…/formula-start-slate-es-k200.png` |
| `calculus-paper-100.png` | `…/formula-start-paper-en-k100.png` | `…/formula-start-paper-es-k100.png` |
| `calculus-paper-200.png` | `…/formula-start-paper-en-k200.png` | `…/formula-start-paper-es-k200.png` |

(`*-start-*` is the window as it opens on a playing path / a sine, its links listed; the
shots are the design sheets' frame, without the preset well — the app's window has it. A
shot of the window in the app would need `shoot_skins` to learn modulators; not needed for a
card.) Sizes: Tactus `big: (1800, 1255), thumb: (640, 446)` (shots 1064×742 and 2128×1484);
Calculus `big: (1800, 1290), thumb: (640, 459)` (968×694 and 1936×1388).

*The cards* (only what is built and measured; no sound of their own, so `ab: None`; the
modulators live in the host, so `plugin: false`):

```rust
pub const CONTROLLERS: [Unit; 2] = [
    Unit {
        file: "tactus",
        name: "Tactus",
        kind: ("X-Y pad controller", "Controlador de pad X-Y"),
        pitch: ("Play two parameters at once with one hand, and keep the gesture as a path that loops with the song.", "Toca dos parámetros a la vez con una mano y guarda el gesto como un trayecto que se repite con la canción."),
        what: long!(("Tactus is a modulator with a window of its own: a glass pad whose puck drives every parameter you link to its two axes, X and Y, each link with its own range and bend. The puck can glide after your hand, follow a path you draw or record while the song plays, and a gesture recorded as automation writes two lanes in one undo step.", "Tactus es un modulador con ventana propia: un pad de cristal cuyo disco mueve cada parámetro que enlazas a sus dos ejes, X e Y, cada enlace con su propio rango y curva. El disco puede deslizarse tras tu mano, seguir un trayecto que dibujas o grabas mientras suena la canción, y un gesto grabado como automatización escribe dos carriles en un solo paso de deshacer.")),
        features: long!(&[
            ("Two axes driving up to 32 parameters; a range and a bend per link", "Dos ejes que mueven hasta 32 parámetros; un rango y una curva por enlace"),
            ("Inertia: the puck glides after the hand, from at once to ten seconds behind", "Inercia: el disco se desliza tras la mano, de al instante a diez segundos detrás"),
            ("Draw a path, or record one while the song plays; it loops in time with the song", "Dibuja un trayecto o grábalo mientras suena la canción; se repite a tiempo con ella"),
            ("A recorded gesture writes two automation lanes, X and Y, in one undo step", "Un gesto grabado escribe dos carriles de automatización, X e Y, en un paso de deshacer"),
            ("Each axis automatable and learnable by a MIDI controller, like any parameter", "Cada eje se automatiza y se enlaza a un controlador MIDI como cualquier parámetro"),
            ("Bake its moves into automation lanes, inertia and path included", "Hornea sus movimientos en carriles de automatización, con inercia y trayecto"),
        ]),
        presets: Some(4),
        plugin: false,
        ab: None,
        big: (1800, 1255),
        thumb: (640, 446),
    },
    Unit {
        file: "calculus",
        name: "Calculus",
        kind: ("Formula controller", "Controlador por fórmula"),
        pitch: ("Type a formula of the song's time and three knobs, and it moves any parameter, the same on every render.", "Escribe una fórmula del tiempo de la canción y tres perillas, y mueve cualquier parámetro, igual en cada render."),
        what: long!(("Calculus is a modulator driven by a formula in FL Studio's syntax, typed on a calculator's display with a keypad of the language's words. Its plot shows the last four beats and the next four as the knobs a, b and c are now; a formula that does not compile shows its error at its character and never reaches the audio, and its random words are seeded so a render repeats.", "Calculus es un modulador movido por una fórmula con la sintaxis de FL Studio, escrita en la pantalla de una calculadora con un teclado de las palabras del lenguaje. Su gráfica muestra los últimos cuatro tiempos y los cuatro siguientes con las perillas a, b y c como están; una fórmula que no compila muestra su error en su carácter y nunca llega al audio, y sus palabras aleatorias tienen semilla para que un render se repita.")),
        features: long!(&[
            ("FL Studio's formula syntax: Sin, Cos, tri, saw, Round, If, Max, Min and more", "La sintaxis de fórmulas de FL Studio: Sin, Cos, tri, saw, Round, If, Max, Min y más"),
            ("Three knobs a, b, c, each automatable, learnable or moved by another modulator", "Tres perillas a, b, c, cada una automatizable, enlazable a MIDI o movida por otro modulador"),
            ("A broken formula shows its error at the character and keeps the last good one playing", "Una fórmula rota muestra su error en el carácter y sigue sonando la última buena"),
            ("A plot of four beats past and four ahead, now a lit bead", "Una gráfica de cuatro tiempos atrás y cuatro adelante, el ahora una cuenta encendida"),
            ("Repeatable: time from the song, random words seeded; live equals offline to the bit", "Repetible: el tiempo de la canción, el azar con semilla; en vivo igual que offline al bit"),
            ("Nine presets on one knob plan: a the rate, b the shape, c the depth", "Nueve presets con un mismo plan de perillas: a la velocidad, b la forma, c la profundidad"),
        ]),
        presets: Some(9),
        plugin: false,
        ab: None,
        big: (1800, 1290),
        thumb: (640, 459),
    },
];
```

Each claim's source: ADRs 0147/0148 Phase B and the follow-up; `fxd_e2e.rs` (a path from bar
3 and from bar 5 on the strip's pan and fader to the bit; `formula.square` live = offline to
the bit; recording writes exactly two lanes); `modulator_presets.rs` (4 and 9 presets);
the inertia's "ten seconds" is the model's, the agents' and the knob's range (follow-up 2). Check
the `long!` macro and the six-feature rule on the branch when merging.

**4. The glide on a render from bar N — not fixed, a failing test for the integration.**
`crates/cantus-engine/tests/render_from_bar.rs`,
`a_render_from_bar_3_is_the_whole_render_from_bar_3_with_an_effect_automated`, `#[ignore]`d
with the reason: a Firmus line on every beat, an Aequalis bell at 220 Hz stored at 0 dB, a
lane holding it at +9 dB for 16 beats; renders 0–16 and 8–16 compared. It asserts the lane
is heard (vs. the dry render), that from beat 9 on the late render is the whole one's to the
bit (passes), and that bar 3's first beat is too — which fails today, as measured:
**5.23e-2 at frame 140 (2.9 ms after the start)**, the e2e's 5.2e-2. Un-ignore it with the
fix (`cargo test -p cantus-engine --test render_from_bar -- --include-ignored`).

**Suites** (one crate per call, a machine shared with other lanes' builds): `cantus-core`
**839** green (1 ignored); `cantus-ui` **2 751** green (54 ignored; Arca's CPU budget green
this run); clippy `-D warnings --all-targets` clean on `cantus-core`, `cantus-ui` and
`cantus-engine` (its new test file); `cargo fmt` clean on the three. `fxd_drag` 23 (6 new),
`learn_everywhere` 9 (1 new). The engine's new test was run with `--include-ignored` (fails
as described). Not run: the Linux check in WSL, the other crates' suites (`cantus-dsp`,
`-engine` beyond the new file, `-project`, `-ai`; none of their code changed), `xtask`'s
manual-links test (the manual's edits add no link).

## Follow-up 2 (2026-11-17, night): the pad as the last thing touched; the inertia's range

The coordinator's order (the owner's "no leftovers"): close the two findings above, tests
first.

**1. `Alt+A` / `Alt+Shift+A` after a pad drag.** What was wrong: *last tweaked* held one
parameter, and a drag's command is one batch of X then Y, so it named **Y** alone. Now
`AutoUi::last_tweaked_with` keeps the other axis when a batch's parameters are exactly two
inputs of one modulator (the pad's drag, or an agent moving x and y); any other tweak clears
it (`note_tweak`, and the controllers' moves).
- `Alt+A` → `automate_pair`: a clip for each axis that has none, **both in one undo step**
  (one `Batch`, as a recorded gesture writes two lanes), the track's lanes opened once, the
  status "Automation clips created: … and …"; with both there, it shows X's (no step). A
  card knob is one axis and stays one.
- `Alt+Shift+A` → **X, then Y** (the kit arms one target at a time; chosen over learning a
  hardware XY pad's two CCs from one gesture, which would need a second detection mode, while
  X-then-Y serves a joystick and two knobs alike): X armed, Y queued
  (`ControllerUi::learn_next`), the status "Move a control … for X, then another for Y";
  when X is learnt Y is armed, and the control just learnt is passed over while Y waits
  (`learn_skip`, a 14-bit pair's halves too), so a knob still turning is not taken twice.
  `Esc` (`cancel_learning`) or another project drops both; a failed learn drops the queue.
  Each mapping is its own undo step.
- Tests (`fxd_drag.rs`, through `assemble`): `alt_a_after_a_pad_drag_automates_both_axes_in_one_step`
  (both made, one step, undone together; again with both there, no step and no third clip;
  with only Y there, X made alone, one step; a knob after the pad is one axis);
  `alt_shift_a_after_a_pad_drag_learns_x_then_y` (CC 21 → X, CC 21 again ignored while Y
  waits, CC 22 → Y, two steps; `Esc` drops the pair).

**2. The inertia knob's range.** What was wrong: the card's knob went 0–2 000 ms linearly
while the model, the agents' tool and the presets take 0–10 000 ms (`MAX_INERTIA_MS`), so a
pad set past 2 s showed 2 000 and a touch would have pulled it back. Now the knob spans
`fxd_xy::INERTIA_MAX` = `MAX_INERTIA_MS` on a new kit taper, `Taper::Swell(6.41)`
(`min + span·(e^(c·p) − 1)/(e^c − 1)`): three quarters of the travel to 2 s, half to ~390 ms,
the last quarter the slow glides; the flywheel's weight follows the knob's travel. The
presets (80–400 ms) and the agents' tool (`opt_in(0, MAX_INERTIA_MS)`) already agreed.
- Tests: `the_inertia_knob_reaches_the_models_whole_range` (the knob's top is the model's;
  2 s at 3/4 within 25 ms; every pad preset within range; a pad at 5 s reads "5000 ms", a
  touch keeps it and is no step, a turn moves it up from there, the top is 10 000 ms);
  `cantus-ai` `tools_fxd.rs` `an_agent_sets_a_pads_inertia_over_the_models_whole_range`
  (10 000 taken, 10 001 refused naming `inertia_ms`).
- The pad's sheets re-shot in place (the flywheel's weight moved with the taper).

ADR 0147 and the manual (EN/ES, *Tactus*) say all of it.

Suites after follow-up 2 (one crate per call, the machine shared): `cantus-core` **839**
green (1 ignored); `cantus-ui` **2 754** green (54 ignored) on a `--no-fail-fast` run — a run
before it stopped on `preset_render`'s Arca CPU budget (+0.25 on three Arca presets, the
known noise under load), green on the next; `cantus-ai` `tools_fxd` 6 green; clippy
`-D warnings --all-targets` clean on `cantus-ui`, `cantus-core` and `cantus-ai`; `cargo fmt`
clean on those and `cantus-engine`. Not run: the Linux check in WSL; the other crates' whole
suites.
