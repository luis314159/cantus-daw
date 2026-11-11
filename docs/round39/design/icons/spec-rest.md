# Spec for the kit — set 5, the rest of `glyph.rs`, and the whole map

The drawings are in `glyphs-r-rest.js`, `glyphs-r-rest2.js` and `glyphs-r-small.js`. Each
size is its own drawing. **Never scale one size to make another.** The grammar is set 2's
for key glyphs: keys of 16/20/24 px have 1-px lines, keys of 30/36 have 2-px lines, and a
line glyph fills about 0.6 of its key. Small bare marks follow set 3's rules (pixel cells,
quiet ink). The inks per state are those of `spec-mixer.md`.

## Set 5 — sizes and use

| Glyph | Sizes (100 % → 150 %) | What | Notes |
|---|---|---|---|
| `help` | keys 16, 20, 24 → 24, 30, 36 | ? | a question mark without a ring; the dot is a pixel (16–20), a disc from 24 |
| `warning` | keys | ⚠ | outlined triangle + mark; on/lit in `solo` amber |
| `agent` | keys | ✦ the AI area | a four-point sparkle (concave quadratic sides) and a small one |
| `device` | keys | a device's mark | a sine in a ring |
| `keyboard` | keys | the typing keyboard plays notes | a computer keyboard: two rows of keys, a space bar (no longer a piano: the piano is the piano roll's) |
| `score` | keys | the notation pane | a staff in the second ink, one note |
| `mouseMenu` | keys | has a right-click menu | a mouse, its right button filled |
| `automate` | keys | can be automated | an envelope of 45° segments with two points |
| `filter` | keys | a view's filter options | a funnel |
| `replace` | keys | swap a device for another | two cards and the turn between them; head pixels at 16–24, a chevron at 30–36 |
| `warp` | keys | the warp editor, Quantise audio | a warp marker (a tab with a point) over a wave packed on one side, stretched on the other |
| `pluginStopped` | keys | a hosted plug-in that stopped | set 2's plug pulled apart; lit in `rec` |
| `controller` | keys | MIDI learn, mappings | a knob beside a fader |
| `lanePencil` / `laneLine` / `laneCurve` | keys | a lane's tools | set 2's pencil drawing a stroke; a straight 45° segment between two points; a bent one |
| `session` | keys | the session view | a 3 × 3 grid of clip cells, one lit (main ink), the rest waiting (second ink) |
| `stage` | keys | the Stage tab | a figure dancing: one arm up, one out, a kick |
| `menu` ▸, `down` ▾ | 10, 12 → 15, 18; 8, 10 → 12, 15 | a menu; a field that opens a list | solid pixel triangles |
| `prev` ‹, `next` › | 10, 12 → 15, 18 | | 1-px chevrons |
| `close` | 10, 12, 16 → 15, 18, 24 | ✕ | 10–18: set 4's `clear`; 16 and 24 for a title bar |
| `minimise`, `maximise`, `restore` | 10, 12 → 15, 18 | the title bar | a bar at the foot; a window with a 2-px title edge; two windows |
| `resizeGrip` | 10, 12 → 15, 18 | ◢ a corner to drag | three diagonal lines, second ink |
| `resize` | 12, 16 → 18, 24 | ⤢ | a diagonal with an L head at each end |
| `typed` | 12, 16 → 18, 24 | a value can be typed | the text cursor |
| `sendOpen` | 12, 16 → 18, 24 | a send that would loop | set 3's send with its branch broken (every other pixel) |

## The whole map: every `Glyph` and its drawing in this family

| `kit::Glyph` | Family glyph (set) |
|---|---|
| Menu, Down, Prev, Next | `menu`, `down`, `prev`, `next` (5) |
| Star, StarFilled | `star`, `starFilled` (4) |
| More | `more` (3) |
| Help | `help` (5) |
| Power | `enable` (3) |
| Undo, Redo | `undo`, `redo` (2) |
| Resize | `resize` (5) |
| Close | `close` (5) |
| Bulb (the FX-on bulb) | `enable` in a key, or the insert row's `lamp` / `lampOff` (3) — the bulb metaphor is retired |
| Agent | `agent` (5) |
| Record | `record` (2), `arm` on a strip (3) |
| Phase, Swap, Mono | `phase`, `swap`, `mono` (3) |
| Send, SendOpen | `send` (3), `sendOpen` (5) |
| Plus | `add` (3) |
| Grip (◢) | `resizeGrip` (5); the insert row's drag handle is `grip` (3) |
| Check | `check` (4) |
| Device | `device` (5) |
| Play, Pause, Stop, ToStart, Loop, Metronome | set 2's transport |
| Keyboard | `keyboard` (5) |
| Arrangement, PianoRoll, Rack, Mixer, Inspector, Browser | set 2's window keys |
| Score | `score` (5) |
| Gear, Pick, Zoom, Magnet | `gear`, `pick`, `zoom`, `snap` (2) |
| Warning | `warning` (5) |
| MouseMenu, Automate, Typed | `mouseMenu`, `automate`, `typed` (5) |
| Minimise, Maximise, Restore | `minimise`, `maximise`, `restore` (5) |
| Replace | `replace` (5) |
| Link | `link` (3) |
| Key | `key` (3) |
| Cables | `routing` (3) |
| Filter, Warp, PluginStopped, Controller | (5) |
| Frozen | `freeze` (3) |
| LanePencil, LaneLine, LaneCurve, Session, Stage | (5) |

New in the family, with no `Glyph` today (the kit adds them): `skipBack`, `skipFwd`,
`countIn`, `follow`, `draw`, `select`, `slice`, `erase`, `muteTool`, `plugins` (set 2);
`fx`, `out`, `into`, `ret`, `vca`, `miniView`, `foldOpen`, `foldShut`, `lamp`, `lampOff`
(set 3); `folder`, `folderOpen`, `file`, `audio`, `midi`, `preset`, `instrument`, `effect`,
`pack`, `search`, `history`, `clear`, `speaker`, `download` (set 4).
