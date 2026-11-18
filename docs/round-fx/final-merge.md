# The effects side round — the final merge onto round 40

Branch `fx/final`, worktree `C:\projects\daw-fx\final`, from `master` `477cb6b4` (round 40:
schema 71, ADRs up to 0126, keymap 17, `DSP_VERSION` 7). Every branch of the side round is
merged here in order, renumbered onto master's next numbers, and tested whole; then (on the
owner's word) master is updated, pushed and the site published. **`fx/multimon` is not part
of this merge** (owner, 2026-11-17): it lands later, as its own second merge onto master.

## Merge order

| # | Branch | What | Merged at |
|---|---|---|---|
| 1 | `fx/integration` `0e6b2722` | Cribrum, Fragor, Lapsus, Vetus, Libra, Tabula, Saltus, Arca, Rota, Loquax, Fractus, Caelum (step 3) and their docs | `899cfe16` (and its BACKLOG §6k tip `6e4f6201` at `879f6418`) |
| 2 | `fx/web` `d8ae6fe9` | the site: every new unit's card and page (EN/ES), Try it | `2f170a7b` |
| 3 | `fx/excel` | Caelum's continuous Sustain | later |
| 4 | `fx/wave-b` | Vinculum, Lenis, Cedo | later |
| 5 | `fx/wave-c` | Purus, Comparatio | later |
| 6 | `fx/wave-d` | Tactus (X-Y pad), Calculus (formula) | later |
| 7 | `fix/flicker` | issue #1 (the window flicker), menus at 200 %, the one-step bus | later |
| — | `fx/multimon` | several screens | **after this merge, onto master** |

## The numbers: provisional → final

The side branches numbered in a private range (schema 62–67, ADRs 0130–0151) while round 40
used schema 62–71 and ADRs 0116–0126 on master. The final numbers follow master's last in
merge order: **schema +10, ADRs −3**. Every migration step is kept (the empty ones too), each
schema's fixture is written again under its final number by `write_fixture <N>`, and every
older fixture keeps loading.

### Schema (`cantus-project` `SCHEMA_VERSION`)

| Provisional | Final | What | Branch | Status |
|---|---|---|---|---|
| 62 | **72** | the side round's five kinds (Cribrum, Fragor, Lapsus with its bank, Vetus, Libra) | fx/integration | renumbered (migration, fixture block, tests) |
| 63 | **73** | Tabula's board and three kinds (Saltus, Rota, Arca) | fx/integration | renumbered |
| 64 | **74** | Fractus's lanes and three kinds (Loquax, Fractus, Caelum) | fx/integration | renumbered |
| 65 | **75** | wave B (Vinculum, Lenis, Cedo) | fx/wave-b | at its merge |
| 66 | **76** | wave C (Purus, Comparatio) | fx/wave-c | at its merge |
| 67 | **77** | wave D (the X-Y pad and formula controllers) | fx/wave-d | at its merge |
| — | — | `fx/excel` (Caelum's Sustain) and `fix/flicker` declare no schema of their own (`fix/flicker` is at master's 71) | | |

Round 40's own `fixtures/v62.cantus` (a track's record tap) is kept; the side round's
provisional `v62`–`v64` are not carried over — `v72`–`v74` are written instead.

### ADRs

| Provisional | Final | Title | Branch |
|---|---|---|---|
| 0130 | **0127** | Cribrum, the auto filter | fx/integration |
| 0131 | **0128** | Fragor, the pedals | fx/integration |
| 0132 | **0129** | Lapsus, the time shaper | fx/integration |
| 0133 | **0130** | Vetus and Libra, lo-fi and utility | fx/integration |
| 0134 | **0131** | Tabula, the pedalboard | fx/integration |
| 0135 | **0132** | Saltus, pitch | fx/integration |
| 0136 | **0133** | Rota, the rotary speaker | fx/integration |
| 0137 | **0134** | Arca, amp and cabinet | fx/integration |
| 0138 | **0135** | The user's starting presets | fx/integration |
| 0139 | **0136** | Loquax, the vocoder | fx/integration |
| 0140 | **0137** | Fractus, glitch and grain cloud | fx/integration |
| 0141 | **0138** | Caelum, the shimmer reverb | fx/integration |
| 0142 | **0139** | Vinculum, the bus compressor | fx/wave-b |
| 0143 | **0140** | Lenis, the resonance suppressor | fx/wave-b |
| 0144 | **0141** | Cedo, the ducker | fx/wave-b |
| 0145 | **0142** | Purus, real-time repair | fx/wave-c |
| 0146 | **0143** | Comparatio, the reference comparator | fx/wave-c |
| 0147 | **0144** | The X-Y pad | fx/wave-d |
| 0148 | **0145** | The formula controller | fx/wave-d |
| 0149 | **0146** | The window holds still (issue #1) | fix/flicker |
| 0150 | **0147** | Route the selected to a new bus | fix/flicker |
| 0151 | *at its own merge* | Several screens | fx/multimon (later) |

0121 is a gap on master (round 40) and stays one. The files are renamed and every reference
in `crates/`, `docs/`, `web/` follows (a token-exact rewrite: four digits not inside a longer
number, word or hex string; `tools/public/rvr1960-4.fingerprints` holds a hash that starts
`0130` and is left alone). *Provisional* is taken off the settled numbers; the builders'
reports in this folder keep their history but show the final numbers.

**`fx/multimon`** carries its own copy of 0149 (it was cut from `fix/flicker`) — that one is
0146 — and its 0151 and keymap 19 are renumbered **at its own later merge onto master**: the
next free ADR after this merge's last (0148 if nothing else lands first) and the next keymap
after this merge's (19 if nothing else lands first).

### Keymap (`cantus-ui` `keymap::FILE_VERSION`)

| Branch | Provisional | Final |
|---|---|---|
| master (round 40) | 17 | 17 |
| fx side branches | 16 (beta.14's; no keymap change of their own) | — |
| fix/flicker | 18 | **18** |
| fx/multimon | 19 | at its own merge |

### `DSP_VERSION`

Master is at 7 (round 40, Chorda's candidate 16). The side branches were cut from beta.14 at
6 and none bumped it (each new unit is new sound, not a changed one; the null hashes at their
defaults are the dry signal's). After the merge it stays **7** unless a later branch changes
an existing unit's sound — `fx/wave-d`'s note asks the integration to fix the native effects'
~20 ms glide when a render starts mid-song, which changes rendered bits: that is **8**, with
the golden dump measured, in its own commit.

## Merge 1: `fx/integration` (`899cfe16`)

Conflicts (24 files), resolved file by file against round 40:

- **Additive**: `EffectKind` choices; the engine's memory needs, tickets and memories (Delay's
  line beside Lapsus's, Tabula's, Arca's, Fractus's); the analysed pedal beside *Hear the key*
  (`ctx.rs`, `processor.rs`, `processor_pass.rs`, `listen_fork.rs`); the undo fuzz's and
  `rt_memory`'s appended tests; the plug-ins' descriptors (37: round 40's Delay, then the
  twelve), features, build list, test settings and references; xtask's editor features;
  `ids.txt` round 40's Delay then the twelve.
- **`processor_msg.rs`**: round 40 made memory supply hand the old line back first (`give`,
  A-M3) and moved the request into `ask_memory`; the side round routed memory to a board's
  pedal and asked for each pedal's. Merged: `give` finds the pedal, then takes its spent
  memory, supplies, reads its busy; `ask_memory` asks for the unit and each of its pedals;
  `sweep_spent`, `adopt_pedal` and `request_memory` each whole (git had interleaved them).
- **`effects.rs`**: the side round's `hear_position` (a board hears late positions per pedal)
  replaced the callers of `set_position`, which round 40 used to give Delay its synced tempo —
  `hear_position` now does it for Delay too. `take_spent` and `busy` look inside a board, so a
  Delay pedal's old line is swept like a lone Delay's (round 40's sweep saw only top-level
  units).
- **`strip_run.rs`**: `hear_position`, then round 40's *Hear the key*.
- **`format.rs`, `write_fixture.rs`**: renumbered (above); round 40's migrations, then the side
  round's.
- **`app.rs`**: round 40's scene and look set-up, then the units' colourways, views and
  starting choices.
- **README**: "Thirty effects … all of them are also CLAP and VST3" (Delay became one in
  round 40). The README's grid still lacks the units after Libra — written once every unit has
  landed and its pictures are shot (below).

## Merge 2: `fx/web` (`2f170a7b`)

One conflict, `web/src/units_data.rs` (the comment over Libra's card): fx/web's cards kept
(Vinculum, Lenis, Cedo, Purus, Comparatio — `EFFECTS: [Unit; 42]`), their ADR comments on the
final numbers. Their `MERGE:` notes (preset counts, *plugin* flags, Cedo's drawn shape, the
v1 window sentences of Purus and Comparatio, the pictures' sizes) are settled when wave B and
wave C land — they depend on what those branches finally hold. The integrator's note in
`site.md` (Try it's unit row cut its last card at the edge) is done: the row fades at its
right edge and scrolls the last card clear (`web/style/units.css`).

## Fixes after the merges

- `4113c5ae`: round 40's listen rig (`listen.rs`) and the export's block loop (`render.rs`),
  which answer memory requests while the song plays, carry the board pedal the request is
  for (they did not compile against the side round's messages); `fx_delay_sync.rs` too.
- `5082c15c`: rustfmt on the merged lines.
- The MCP tools golden needed no change (`ai_mcp_boot` passes as merged).

## Verified so far (Windows, `fx/final` at `5082c15c`)

- `cargo clippy --all-targets -- -D warnings`, one crate at a time: `cantus-core`,
  `cantus-dsp`, `cantus-engine`, `cantus-project`, `cantus-plugins` (and with
  `--no-default-features`), `cantus-ai`, `cantus-ui`, `cantus` (the app), `cantus-bench` —
  clean. `cargo fmt -p … -- --check` clean for each (`cantus-ui` by `rustfmt --check` on its
  files: os error 206).
- Tests: `cantus-core` 873 passed, 0 failed, 1 ignored (the undo fuzz with every arm);
  `cantus-project` 205 passed, 0 failed, 1 ignored (every fixture v1…v74 loads; the side
  round's schema 72–74 tests); `cantus-dsp` 1 159 passed, 0 failed, 38 ignored; `cantus-ui`'s
  `ai_mcp_boot` (the tools golden) passes.
- Not yet run (lanes shared with the builders): the engine, plug-ins, UI and AI suites, the
  engine with `CANTUS_WORKERS=3 --features fuzz`, the null hashes, the Linux check, the web
  crate (`cargo check` for wasm32 and `ssr`), Trunk.

## What each later branch needs at its merge

New lines a branch brings still carry provisional numbers; they are settled with
`renum_new.py <first parent> 130 151 -3` (the integrator's scratch tool: it rewrites only
lines that are not already in the first parent, so a settled number is never shifted twice),
then the schema by hand.

- **`fx/excel`** (Caelum's continuous Sustain): edits ADR 0141 → **0138** (renamed here; git
  follows the rename), Caelum's plug-in ids (`isus` appended, `ishim`/`ilevel` gone — never
  released; re-bless `ids.txt` after round 40's and ours), the AI's words (the tools golden
  again), `units_data.rs`'s Caelum card. No schema.
- **`fx/wave-b`**: schema 65 → **75** (its migration kept), ADRs 0142–0144 → **0139–0141**;
  keep fx/web's Vinculum and Lenis cards; settle their `MERGE:` notes and Cedo's (preset
  counts, the plug-in flags, "a shape you draw"); `lapsus_bank.rs`'s one-hunk
  `ContentPost<T>` conflict (keep both impls); master's sums differ by 3e-8 where the key
  route reorders them (check the affected golden); Lenis's latency re-report gated on Linux
  (the VST3 tick is Windows-only); `CantusApp::fx` boxed (the stack budget);
  `write_fixture 75`.
- **`fx/wave-c`**: schema 66 → **76** (its empty 64→65 step was wave B's: keep one step per
  number), ADRs 0145–0146 → **0142–0143**; Purus's and Comparatio's cards' `MERGE:` notes and
  window sentences against the owner's verdicts; `write_fixture 76`.
- **`fx/wave-d`**: schema 67 → **77**, ADRs 0147–0148 → **0144–0145**; the native effects'
  ~20 ms glide when a render starts mid-song (its `#[ignore]`d engine test) → fixed with
  `DSP_VERSION` **8** and the golden dump in one commit; the site's Controllers note;
  `write_fixture 77`.
- **`fix/flicker`** (cut from master `477cb6b4`): ADRs 0149/0150 → **0146/0147**, keymap 18
  stays 18; its `beta-checklist.md` steps and `troubleshooting-graphics.md`.
- **`fx/multimon`**: not in this merge (above).

After the last: `docs/round-fx/site.md` §5 (uncomment wave B/C's lines in `shoot_skins.rs`,
the pictures, `check_units.py --fix`, every card's excerpts with Aula's re-rendered, Try it's
takes and pictures, the site's build and checks), the README's grid with every unit and the
counts that go with it, the whole suite on Windows and Linux, STATUS.

## Still to do on this branch (after the later merges)

- `fixtures/v75`–`v77.cantus` written by `write_fixture <N>` at wave B's, C's and D's merges
  (v72–v74 are written, `291ff166`).
- The MCP tools golden (`CANTUS_TOOLS_DUMP=1`) again after each later merge that adds tools or
  effect words.
- The site's §5 commands (`docs/round-fx/site.md`): pictures, excerpts, Try it, Aula's
  excerpts re-rendered, the README grid with every unit and its counts.
- The whole suite on Windows and the Linux check, plug-ins without default features, STATUS.
