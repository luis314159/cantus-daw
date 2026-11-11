# Round 39 — `delay`: Delay's own window (ADR 0115)

The owner's request: Delay was the only effect on the website and in the README with no
window of its own (it was edited only under its insert in the inspector). It now has one, in
a face of its own, and stays in the inspector as the other windowed effects do: a click on its
insert opens the window, the mini view under it keeps the quick controls.

## The design

`docs/round39/delay/` — `sheet-{night,paper}-{1x,2x}.png` (drawn by `src/sheet.py`, SVG
through Edge headless, from the DSP's own model of the echoes) and `compare-{night,paper}.png`
(the sheet beside Concentus, Aula and Spatium at 100 %).

- **The face**: a tape echo at night — oxblood vinyl (350°), the echoes on a phosphor screen
  in coral (354°) that cools to rust (16°) as the tone darkens each repeat, skirted knobs, a
  rocker, an engraved upper-case name, screws, printed rules (`kit/skin/tone.rs`, `DELAY`).
- **The display is the control**: L above the centre line, R below; the sound at 0, each
  repeat a stem at its time, its level against the first (drawn as its square root, so the
  quiet ones show), on its side with ping-pong, its colour from the tone's loss at 8 kHz per
  pass; dashed, the decay through the heads and the feedback's level in each lane. Drag across
  for the time, up and down for the feedback — one undo step; Shift fine; double-click the
  defaults.
- **Tempo**: the unit's time has no sync and no DSP was added; *At the tempo* sets the time
  once from a note value at the song's tempo, names the note when the time is one, and says it
  does not follow a tempo change.
- **What is heard**: repeats to −60 dB, the tail, the tone's loss per repeat at 8 kHz, the
  first repeat's side, a sentence on how the sides repeat.
- Under them: **Time** (XL knob, at the tempo), **Repeats** (feedback, tone), **Output**
  (ping-pong, mix). Presets and A/B are the family's header.

## What changed

| Layer | Files |
|---|---|
| Model | `cantus-core/src/effect.rs`: `has_editor` is false for the old Reverb only |
| Editor | `cantus-ui/src/views/fx/delay.rs` (the window, the cards, *At the tempo*, *What is heard*), `delay_view.rs` (the echoes' model, the display and its drag, the inspector's strip, the demo set-up and feed) |
| Shared, additive (`// r39:delay`) | `views/fx/mod.rs` (the editor everywhere an editor is listed), `views/fx/frame.rs` (`Out::at_rest`: an editor that is still asks for no frame), `kit/skin/tone.rs` (`DELAY`), `i18n.rs`, `app.rs` (`--editor delay`), `views/picker_snap.rs`, `kit/skin.rs` (a comment), `agent_units.rs` (no "tempo delay": the time has no sync) |
| Strings | `locales/fx/delay.{en,es}.txt`; `cantus-plugins/src/text.rs` reads them too |
| Plug-ins | `cantus-plugins/src/editor.rs`: `has_editor` asks for an exported kind (Delay's editor runs on `FxHost`, Delay is not exported) |
| Pictures | `examples/shoot_skins.rs` shoots Delay; `examples/shoot_delay.rs` now shoots the inspector's view and the window over the app; `tools/shots/reshoot.sh` and `web/tools/units_pictures.py` no longer special-case Delay; `web/assets/units/delay-*`, `docs/images/units/delay-*`, `thumbs/delay.jpg` from the window; `units_data.rs`' sizes by `check_units.py --fix` |
| Site | `web/src/units_data.rs` (Delay's card: its window, the drag, the tempo), `units.rs`, `pages/unit.rs` (no inspector special case), `counts.rs` (every effect has a face of its own) |
| Docs | ADR 0115 (new), ADR 0074 §6, `docs/plugins/delay.md` (new), the manual EN/ES (a Delay section; the site's HTML rebuilt), README (the effects' line, Delay's row and details, the workspace's Effects line), BACKLOG §3b (sync and the plug-in export, the owner's) |

No DSP file changed: Delay's sound is the same to the bit and no golden hash moved.

## Tests

- `cantus-ui` lib: `views::fx::delay::view::tests` (4) — **the echoes drawn are the unit's
  own**: the real `EffectUnit` (the engine's wiring of the slot) is fed an impulse, ping-pong
  off and on, and every repeat the display draws is found at its time, on its side (nothing
  on the other), at its level (DC gain, to 1e-3) and with its 8 kHz content (to 2 %); the
  counts, span and note values; the drag's mapping; the heard history.
- `tests/delay_window_e2e.rs` (6, `CantusApp::assemble` + pointer events): a click on the
  insert in the inspector opens the window; a drag on the echoes sets time and feedback as
  one step labelled *Move the echoes* that undo takes back; each knob (time, feedback, tone,
  mix), the ping-pong switch and *At the tempo* (a quarter at the song's tempo) write their
  parameter and undo restores it; at rest the open window asks for no frame, a note from the
  tap moves it, nothing between the note and its first repeat, the first repeat lights when
  the note reaches it, and it is still again when the tap goes quiet; the window in Spanish;
  every English word with its Spanish twin.
- The family's walkers now include Delay and pass: `fx_frame`, `learn_everywhere`,
  `modulate_everywhere`, `skins` (a face and a key of its own), `skins_body` (no plain token
  in its body or header menu), `master_chain`, `picker`, `library_fixes`, `legacy_reverb`;
  `kit::skin::tone::tests` (siblings, the contrast floor in every preset).
- **`cargo test -p cantus-ui --no-fail-fast`: 2 622 passed, 0 failed, 54 ignored.** (A first
  run lost ten tests in `pack_drums`, `pack_missing`, `padout_bounce`, `ml_service_boot` and
  the pitch tests to F: filling up — 2 MB free; I deleted my `target/debug/examples` (6.5 GB),
  they passed alone, and the whole suite passed again.)
- `cargo test -p cantus-core -p cantus-plugins`: all pass (the plug-ins' editor test after
  the `has_editor` fix).
- `web/`: `cargo test --features ssr` 23 passed; `cargo fmt --check` clean.
- `RUSTFLAGS="-D warnings" cargo clippy --all-targets` on `cantus-ui`, `cantus-core`,
  `cantus-plugins`: clean.

## Shots

`docs/round39/delay/`:
- `window-{slate,paper}-{100,200}.png`, and `-es` (the window, `shoot_skins delay`);
- `inspector-{slate,paper}-{100,200}.png` (the insert with its open key and the mini view's
  quick controls, `shoot_delay`);
- `app-{slate,paper}.png` (the window opened over the application, beside the inspector);
- the design: `sheet-*`, `compare-*`.

Seen and fixed while looking: the tempo note was cut ("…a tempo cha…") — it is a wrapped
paragraph now; the caption "REPEATS TO −60 DB" set dB in capitals — the captions carry no
units now (the tips say −60 dB, the value says "at 8 kHz"); the darkened repeats were hardly
darker — the rust end sits a third into the ground and each head shrinks with its highs; the
ping-pong switch sat at the card's top — it is level with the mix knob; a second picture on
one context had lost its fonts — the app shot has its own context.

## Left over

- **The owner's to decide** (BACKLOG §3b): a **synced time** (a new parameter and DSP — not
  added: the brief said no DSP that does not exist), and **exporting Delay as a CLAP/VST3
  plug-in** (the brief assumed one exists; it never did — new ids are forever).
- The unit pictures were made from the debug build of `shoot_skins` (the CPU raster is the
  same code in both profiles); `reshoot.sh units` makes them again in release with the rest.
- `docs/round39/shots.md` (the shots builder's report) still describes `shoot_delay` as the
  units' picture; `reshoot.sh` itself is updated.
- Verified only headless: the drag and the glow were driven by synthetic pointer events and
  analyser pairs, not with a real interface playing.
