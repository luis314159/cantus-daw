# Round 39 — lapis2: Slate Lapis told from Slate at a glance

The owner looked at whole-window shots of round 39's first Slate Lapis (`lapis.md`) and said
Lapis and Slate "look the same" — true: it was Slate with a lapis accent, and at rest the
accent shows in few places. He approved a proposal of three changes; this is their build.
**Slate does not change by a pixel** (below, *What holds Slate*).

## What Lapis is now

Slate's system — structure, faces, text, keys, knob and fader caps, meters, function hues,
status colours, corners — with three things of its own (`theme_slate_lapis.rs`):

1. **Lapis on the selection.** The accent is lapis, `#7ea2f6` (Night) / `#2443a8` (Paper), its
   tones derived as before. New: the **selection tint** `accent.selection_tint` — lapis at
   18 % (Night) / 10 % (Paper) — laid over every selected fill: a selected clip's body
   (×1.5), a selected note (×3: a note is small), the selected mixer strip and its head, the
   selected track's header (and its icon's ground) and lane (×0.35: the largest fill), the
   channel rack's selected row, Settings' chosen page. The active tab gets a 2-px lapis mark
   on its top edge (where Slate draws the focus bar only when focused). The browser's
   selected row was already the accent's.
2. **Gold on the readouts and marks**, the logo's metal, tuned per light so text keeps AA:
   - `text.readout`: the top bar's bar·beat, the tempo's value, and the ruler's bar number
     under the playhead in the arrangement and every bar number in the piano roll —
     `#e2b85a` Night (7.5:1 on the bar, 8.9:1 in a well), `#6e5008` Paper (6.1:1 / 5.5:1);
   - `text.readout_secondary`: the time beside the position and the arrangement's other bar
     numbers (and the piano roll's multi-metre numbers between the fourth bars) — `#c9a55a`
     Night (6.0:1), `#7a5a04` Paper (5.2:1 on the bar, 4.7:1 in a well);
   - `editor.loop_edge`: the loop band's top and bottom edges along its whole length, in the
     playhead's gold (`#e2b85a` / `#9a6e1c`), 40 % when the loop is off; the band's ends and
     the playhead's cap stay gold as before.
   The ticks after the position, the tempo's thousandths, the captions, the beat names and
   every body text, label and button keep Slate's inks. The arrangement's playhead line is
   the accent (lapis), as Slate's is its accent.
3. **A night-blue room.** Every surface, well and line of Slate's ladder (28 tokens: `ink`,
   `well`, `panel`, `toolbar`, tiles, the grid's three lines, control edges, placeholders,
   the meter well, strips, number cells, the device ground, cards and their titles, value
   pills, step keys, the lanes' two rows, menus and their band, the disabled ink, the wells'
   edge and lip; the track veil follows the panel) is Slate's own colour at **the same WCAG
   luminance**, its hue turned to the lapis's (222° Night, 226° Paper) and its saturation
   × 1.7 / × 1.8: Night's panel `#1d2329` → `#1c2232`, bar `#262d34` → `#242c3f`; Paper's
   panel `#f4f6f8` → `#f4f6fa`, bar `#e4e9ee` → `#e5e8f4`. The values were computed once
   (luminance matched to the 8-bit rounding) and are written in the file; a test re-derives
   the rule from Slate. The same luminance keeps every contrast Slate holds; the text, the
   keys, the caps (the metal), the meters, the cables (track colours) and the status colours
   are Slate's.

Settings ▸ Appearance's picture of Slate Lapis shows it: the night-blue and blue-grey rooms,
the gold clock in the bar, the chosen lane and strip in lapis, the gold loop and playhead.

## How it is built

Four new kit tokens, all **clear** in every other theme (Slate, Graphite, Studio ink, high
contrast, the retired ones, a theme derived from an old palette):

| token | key | clear means |
|---|---|---|
| `readout` | `text.readout` | each readout in its own place's ink |
| `readout_soft` | `text.readout_secondary` | the same, second ink |
| `selection_tint` | `accent.selection_tint` | each view's own selected fill |
| `loop_edge` | `editor.loop_edge` | the loop band without edges |

The views read them through `Tokens::readout_or(own, soft)` and
`kit::color::selected_fill(c, tint, gain)`, which return `own` / `c` unchanged when the
token is clear, and the loop edges and the tab mark are drawn only when theirs has alpha. So
Slate's code paths paint exactly what they painted. When a theme gives a readout a colour,
the contrast floor holds it at 4.5:1 on the bar, its wells and the panel (and the pairings
list it); the knobs turn it as text. Names in both locales (`kit.token.*`).

## What holds Slate

- **Pixel for pixel, whole window**: the shoot (`shoot_lapis`) of the arrangement, the mixer
  in Wide and the piano roll in Slate Night and Paper at 1920 × 1080, 125 % (a clip, a strip
  and six notes selected, playing in a loop) is **byte-identical** before (the base
  `4c52b48b`) and after. The run is deterministic (the base shot twice: identical). Settings
  ▸ Appearance in Slate differs only inside Slate Lapis's own picture (the difference's
  bounding box is the tile, 889–1075 × 180–269), which is meant to change.
- **`tests/slate_window_golden.rs`** (new): Slate Night and Paper's whole window — the
  arrangement, the mixer in Wide, the piano roll, each with a selection, at 1280 × 720 —
  against `tests/golden/window/*.png`, with the kit gallery's tolerance for float noise
  (a channel off by more than 4 counts; fails over 0.2 % of pixels or any pixel off by more
  than 48). Bless with `CANTUS_BLESS=1` after an intended change to Slate.
- `tests/kit_gallery.rs` (the kit's golden sheets) stays green.

## Tests

- `theme_slate_lapis::tests`: only the room, the accent's tones, the tint and the metal
  differ from Slate, and each of them does; Slate's own four tokens are clear; the room is
  Slate's at the same luminance turned to the lapis's hue with more colour, and text keeps
  its contrast on every room token within 5 %; the readouts are gold (hue 35–50°) at 4.5:1
  on the bar, a well and the panel, the first stronger than the second; every kit promise
  holds; the theme resolves to exactly its colours and Slate's shape.
- `tests/slate_lapis_e2e.rs` (through `CantusApp::assemble`, Settings ▸ Appearance, a click
  on the picture, a restart): the room, tint, readouts and loop edge installed; text is
  Slate's; the chosen page's row painted through the tint; the playhead in the gold and the
  ruler's numbers in the second gold, none in Slate's azure; kept across a restart; the
  default (Slate) has all four clear.
- Every test that iterates the presets (legibility, contrast floor, colour roles, skins,
  `slate_e2e`'s switch-and-back) covers both Slate Lapis themes with the new tokens.

**Run at the end** (Windows): `cargo test -p cantus-ui --no-fail-fast` — the lib's 527 and
every integration binary green (`freeze` and `fx_tonus` failed once while F: was full —
"not enough space on the disk" — and passed on a rerun with room); `RUSTFLAGS="-D warnings"
cargo clippy -p cantus-ui --all-targets` clean; `cargo fmt -p cantus-ui -- --check` clean.
The Linux check (WSL) was not run on this branch.

## Shots

`cargo run --release -p cantus-ui --example shoot_lapis -- docs/round39/lapis2`, 1920 × 1080
at 125 %: `arrangement-`, `mixer-` (Wide, maximised), `pianoroll-`, `look-` (Settings ▸
Appearance) × `slate_night`, `slate_paper`, `slate_lapis_night`, `slate_lapis_paper`; and
side by side, Slate left and Slate Lapis right, Night above and Paper below:
`sbs-arrangement.png`, `sbs-mixer.png`, `sbs-pianoroll.png`, `sbs-look.png`.

Judged beside each other: the two are told apart at a glance — Lapis's room is night blue
where Slate's is grey (Paper: a cool blue-grey sheet where Slate's is neutral), the clock,
tempo and ruler numbers are gold, the selection is lapis — and Lapis stays quiet: the gold
is on six readouts and two marks, the tint is light.

## Open

- The arrangement's playhead *line* is the accent (lapis) in Lapis, as in Slate; only its
  cap is gold. Gilding the line too would be one token more (the line paints with the
  accent in every family); left as the brief has it (gold on the cap).
- Verified only on the CPU renderer's shots; the owner's eye on a real screen decides
  whether the 18 % / 10 % tint and the saturation gain are the right strength.
