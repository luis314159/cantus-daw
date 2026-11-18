# The effects side round — the final merge onto round 40

Branch `fx/final`, worktree `C:\projects\daw-fx\final`, from `master` `477cb6b4` (round 40:
schema 71, ADRs up to 0126, keymap 17, `DSP_VERSION` 7). Every branch of the side round is
merged here in order, renumbered onto master's next numbers, and tested whole; then (on the
owner's word) master is updated, pushed and the site published. **Everything lands together**
(the owner, 2026-11-17, a second change of plan): `fx/multimon` too, merged last, after `fix/look`.

## Merge order

| # | Branch | What | Merged at |
|---|---|---|---|
| 1 | `fx/integration` `0e6b2722` | Cribrum, Fragor, Lapsus, Vetus, Libra, Tabula, Saltus, Arca, Rota, Loquax, Fractus, Caelum (step 3) and their docs | `899cfe16` (and its BACKLOG §6k tip `6e4f6201` at `879f6418`) |
| 2 | `fx/web` `d8ae6fe9` | the site: every new unit's card and page (EN/ES), Try it | `2f170a7b` |
| 3 | `fx/excel` `67a79517` | Caelum's continuous Sustain | `95488de5` |
| 4 | `fx/wave-d` `2eac2c50` | Tactus (X-Y pad), Calculus (formula), their follow-ups | `2b87f947` |
| 5 | `fx/wave-b` `d95e96d6` | Vinculum, Lenis, Cedo | `c8b1e990` |
| 6 | `fix/flicker` | issue #1 (the window flicker), menus at 200 %, the one-step bus, Ctrl+G's folder | later |
| 7 | `fix/look` | the fader-cap setting reaching the mixer, see-through dropdowns, every popup audited | later |
| 8 | `fx/wave-c` | Purus, Comparatio (with the owner's three Comparatio follow-ups) | later |
| 9 | `fx/multimon` | several screens (a patched eframe in `third_party/eframe`) | later, last |

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
| 0151 | **0148** | Several screens | fx/multimon |

0121 is a gap on master (round 40) and stays one. The files are renamed and every reference
in `crates/`, `docs/`, `web/` follows (a token-exact rewrite: four digits not inside a longer
number, word or hex string; `tools/public/rvr1960-4.fingerprints` holds a hash that starts
`0130` and is left alone). *Provisional* is taken off the settled numbers; the builders'
reports in this folder keep their history but show the final numbers.

**`fx/multimon`** carries its own copy of 0149 (it was cut from `fix/flicker`) — that one is
0146; its 0151 is **0148** and its keymap 19 stays **19** (on top of `fix/flicker`'s 18). It vendors
a patched eframe (`third_party/eframe`): the workspace `[patch]`, `THIRD-PARTY-NOTICES.md`
(`cargo run -p xtask -- notices`) and the Linux check must take it with everything else.

### Keymap (`cantus-ui` `keymap::FILE_VERSION`)

| Branch | Provisional | Final |
|---|---|---|
| master (round 40) | 17 | 17 |
| fx side branches | 16 (beta.14's; no keymap change of their own) | — |
| fix/flicker | 18 | **18** |
| fx/multimon | 19 | **19** |

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
- `347f9cb7`: a Delay on a Tabula board holds the line its automated note can reach, as an
  inserted Delay does (round 40's A-M2 floor saw only inserted Delays): the automation
  runtime finds each board Delay's floor (`FxCopy::pedal_lines` — lanes on `p{id}.note` /
  `p{id}.sync` unless a macro is mapped there, lanes on a macro mapped to them) and the
  install sets it on the pedal. Test first: `fx_tabula_delay_line.rs` (failed before, max
  diff 0.2175 at 4.0 s: the whole note's echo lost) — by `p0.note` and by a macro, the
  inserted Delay's bits offline, its own offline bits in real time; `rt_memory`'s
  `a_synced_delay_on_a_board_never_allocates`.
- `8e62b9e3`: `dancer_window::the_window_draws_again_only_what_moved`, intermittent in
  parallel on master too: the themes test beside it switches the process-wide look and the
  stage's ink changed between two frames of one moment (9 of 12 runs failed with it beside,
  0 of 12 without). A file-level `LOOK` lock, as `frame_budget.rs` has: 20 of 20 pass.
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
- `cantus-engine` (after `347f9cb7`): 1 345 passed, 1 failed, 24 ignored. The failure is
  `sync_midi::the_out_thread_sends_on_time` (90th percentile 5.1 ms against its 5 ms bound,
  median 37 µs), red alone too while the machine ran at 100 % with 19 cargo/rustc processes
  of the other builders; the test's own comment and wave C's builder see it only under load,
  and nothing merged here touches MIDI out. Run again on a quiet machine before the close.
- Not yet run (lanes shared with the builders): the plug-ins, UI and AI suites, the
  engine with `CANTUS_WORKERS=3 --features fuzz`, the null hashes, the Linux check, the web
  crate (`cargo check` for wasm32 and `ssr`), Trunk.

## The mid-song render glide (wave D's finding) — done (`DSP_VERSION` 8)

`render_from_bar.rs` failed because every native unit is made at its stored values and, at
the first control point of a render that starts later, glided ~20 ms to the automated value
(Aequalis: 5.2e-2 on bar 3's first note); a render from 0 glided too in its first 20 ms.
Fixed on `fix/snap` (from `7c590dbd`), ADR 0027 amended (§5's *What*: a jump snaps what the
lanes drive — no other ADR is about the engine's smoothing; ADR 0005's null tests are what
it restores):

1. **`FxUnit::snap` / `EffectUnit::snap`** for every native kind (`b581f461` and `fix/snap`):
   the values the last configure set taken at once, the sound kept. `fx_snap.rs` (no longer
   ignored) holds every kind: made at A, configured at B and snapped renders a unit made at
   B to the bit (defaults → heavy, defaults → random, random → random). What each of the
   fourteen left snaps:
   - **Amp** (Arca): a model change made now, the knobs, presence, master, sag, mix and
     output; an amp resting in silence is built again at the new knobs (their operating
     points); the circuits' state, oversampler, cabinet and dry line kept.
   - **Aula**: the mix, level and width glides and the pre-delay's crossfade; the ring, the
     convolution and a response swapped in level by level go on.
   - **BusCompressor** (Vinculum): its output glide (the DSP takes its settings at once).
   - **Maximizer** (Culmen): both band stages as a first `set` (`Multiplex::snap`), the
     brickwall's gain and ceiling (`Terminus::snap`, so **Limiter** too), the drive; the
     loudness Match (a measure) kept.
   - **Multiband** (Multiplex): the bands' glides, crossovers, output, solo/mute and on/off
     fades, the split's blend, the static gains in the gain ring (as a new unit's when the
     look-ahead holds only silence); filters, detectors and the look-ahead's audio kept.
   - **Pedal** (Fragor): the chain (no dip), footswitch fades, knobs, input, output, mix,
     gate threshold; pedals move to the new knobs' operating points only while the board
     rests in silence.
   - **Rotary** (Rota): both rotors at their target speeds (braked: parked), mix and
     output; the rotors' angles and the cabinet kept (`fx_snap` compares a brake's two
     sides from the same side: the angle is state).
   - **Saturator** (Fervor): every continuous setting, the model's/curve's crossfade, the
     oversampling, the gains' ramps; filters redesigned, their state kept.
   - **Shimmer** (Caelum): beyond its glides, the input's (pre-delay, bloom, diffusion,
     modulation) and the tank's (lengths, depth, diffusion, freeze, shimmer amounts);
     Infinite's own climb (it follows the sound) kept.
   - **Space** (Spatium): beyond its glides, the FDN's and spring's lengths, depth and
     freeze, and the wet EQ (`Aequalis::snap`).
   - **Stereo** (Latitudo): every glide, fade and structure where `reset` puts them
     (`reset` is now clears + `snap`), derived again; a filter whose slope changes starts
     empty, as at the grid; the Haas line kept.
   - **TransientSplit** (Discrimen): the two sections' gains beside the output.
   - **Vocoder** (Loquax): beyond its glides, the bank retuned at the landed tuning, the
     bands' levels, engaged as the mix says; banks, followers, carrier voices kept.
   - **VolumeShaper** (Fluctus): the level on the curve where the song is (its smoothing
     starts there, as after a reset).
   - **Board** (Tabula): every pedal snapped through its own unit, footswitch fades at
     their ends, input, output, mix and A/B landed; a reorder's dip goes on.
2. **The processor calls it at a jump** (`processor_auto.rs`, `AutoState::jumped`): set at
   Play from stop and `Seek` / `SeekFrac` (`processor_msg.rs`), a loop's wrap
   (`processor_pass.rs::wrap_loop`), `sync_start` and `sync_jump` (Link, MIDI clock, MTC
   chase: `processor_sync.rs`) and a follower's locate while stopped
   (`processor_sync_follow.rs`); never at a snapshot's install (a knob or a lane edited
   while playing still glides). The control point after it — due at once — evaluates every
   lane, configures the units, then snaps every unit a lane or modulator drives (master and
   strips, boards through their pedals, native instruments, Firmus). Offline renders start
   with `Seek` + `Play`, so they start snapped.
3. **Instruments**: Firmus takes its first settings at once and snaps (level, cutoff).
   `inst_contract.rs::snap::*` holds every native instrument to the same rule; those that
   needed a snap: **Organ** (drawbar ramp, percussion, levels, swell, drive, output, rotor
   speeds), **Electric piano** (drive, output, tremolo depth), **Bass** (output level),
   **Chorda** (stage's perspective/width/lid, room size), **Acoustic drums** (modelled
   paths' delays and levels, the outputs' delays, the recorded room's Aula). Wavetable,
   Analog, FM, Sampler and Dancer needed none.
4. Tests: `render_from_bar.rs` un-ignored, and a live test — playback started at bar 3
   after one seek or several equals the render from bar 3 to the bit, from the start the
   whole render; `rt_memory.rs::the_snap_at_a_jump_never_allocates` (every native kind under
   a lane: play, seek, the loop's wraps, play from stop).
5. **`DSP_VERSION` 8**, measured with `CANTUS_GOLDEN_DUMP` against `7c590dbd` (the same
   renders by both builds): **no golden hash moved** — v5, the Pulsus kits (v1, v14, v1 from
   bar 2) and the post-fader sends are the same bits, as is the demo project with no lane
   (none of them has a lane on a native unit, and their Firmus tracks begin idle). What moves
   is a render with a lane on a native unit, in what its glide was at the start: the EQ song
   of `render_from_bar.rs` by 5.6e-2 in its first 20 ms, 1.3e-3 by 50 ms, ≤ 3e-5 to 0.5 s,
   nothing after (the same from bar 3, which is now the whole render's bars 3…); the demo
   project with lanes on Multiband/Stereo/Maximizer/Tremolo 8.6e-2 in the first 20 ms, 1e-2
   to 50 ms, under 4e-6 after 100 ms, nothing after 0.5 s; on Amp/Pedal/Vocoder/Bus
   compressor 7.2e-2, 9e-3, 2.6e-6, nothing after 0.5 s; on Delay (its time)/Chorus/
   Saturator/Space the difference rings on in the echoes and the tail of what entered them
   while the old units glided (1.1e-1 to 0.5 s, 6.9e-3 after 1 s) — the time-based units
   replaying their first milliseconds, not a later difference.
6. Suites on Windows (`fix/snap`): `cantus-engine` 1 403 passed, 0 failed, 24 ignored;
   `cantus-dsp` 1 247 passed, 0 failed, 44 ignored; clippy `--all-targets -D warnings` and
   fmt clean on both. Not run here: the UI and AI suites, the Linux
   check, the engine's fuzz features.

7. **The exported plug-ins snap too** (`cantus-plugins`, CLAP and VST3; ADR 0027's
   amendment): `instance::Processor` keeps where the last block left the host's song and
   marks a jump — playback from stop, a playing song more than `JUMP_SAMPLES` (64 samples
   at its tempo) from there (a seek, an unannounced loop wrap), a CLAP transport event
   inside a block that does the same, the host's announced loop wrapping inside a block —
   and an activation, a reset, an offline render begun (CLAP's `clap.render`, now offered;
   VST3's `processMode` turning `kOffline`). At the next grid point, after the values in
   force are configured, the unit snaps and the bypass fade lands; a parameter change while
   the song plays on still glides. Tests: `tests/snap.rs` (CLAP: a seek, playback from
   stop, a seek inside a block, an offline render begun — each equals a fresh instance made
   at the new value, to the bit; the same change without a jump glides),
   `vst3.rs::a_vst3_seek_…`/`a_vst3_offline_render_…` (the same through VST3),
   `alloc.rs::a_jump_snaps_without_allocating` (every plug-in). The Direct path
   (`through_*_is_bit_identical_to_the_dsp`, the automation-on-the-grid tests) is
   unchanged. On the way: `cantus-ui`'s `views/fx/fxd.rs` did not compile on this branch
   (wave D's controller header lacked round 40's `views_w`) — `views_w: 0.0`, as every other
   header sets it. `cantus-plugins`: 112 passed, 0 failed; clippy `--all-targets -D warnings`
   clean with and without default features; `cargo fmt` over the crate (merge leftovers in
   `build.rs`, `editor.rs`, `text.rs` and four test files formatted too).

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
  ~20 ms glide when a render starts mid-song (its `#[ignore]`d engine test) is fixed on
  `fix/snap` (`DSP_VERSION` **8**, above): keep `render_from_bar.rs` un-ignored as
  `fix/snap` has it, and any new native kind wave D brings needs its `snap` (`fx_snap.rs`
  fails for it otherwise); the site's Controllers note; `write_fixture 77`.
- **`fix/flicker`** (cut from master `477cb6b4`): ADRs 0149/0150 → **0146/0147**, keymap 18
  stays 18; its `beta-checklist.md` steps and `troubleshooting-graphics.md`.
- **`fx/multimon`** (last): ADR 0151 → **0148** (its 0149 is flicker's 0146), keymap 19; the
  patched eframe through `[patch]`, the notices regenerated, the Linux check building it.

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
