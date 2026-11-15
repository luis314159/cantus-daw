# Tabula — the pedalboard

*Effect kind `Board`. References: Line 6 Helix (Native), Neural DSP's Archetype pages,
Native Instruments' Guitar Rig, IK Multimedia's AmpliTube, Logic's Pedalboard, FL Studio's
Distructor (ADR 0134, provisional number).*

Tabula (Latin, *the board*) is a pedalboard of Cantus's own effects: up to sixteen of them as
pedals, in any order, along one chain with an optional A/B split, eight macros along the
top, an input, a mix and an output. Every pedal **is** the unit — its own engine, its own
full editor — so a board of one pedal sounds exactly as that unit inserted alone.

| Layer | File |
|---|---|
| Model | `crates/cantus-core/src/fx/tabula.rs` (the board, its keys, the macros, the starting rig), `fx/tabula_presets.rs` (the flat map a board preset is, the factory's boards), `fx/tabula_random.rs` (Randomise) |
| Commands | `crates/cantus-core/src/command_board.rs` (and `command_board_tests.rs`) |
| Engine | `crates/cantus-engine/src/fx/tabula.rs` (the board: order, split, fades, lines, fan-out, rest, telemetry, `KnownPedals`, `Boards`) |
| Editor | `crates/cantus-ui/src/views/fx/tabula.rs` (the window), `tabula_board.rs` (layout, pedals, cables, split boxes, picker), `tabula_art.rs` (the range's motifs), `tabula_faces.rs` (every unit's pedal face), `tabula_edit.rs` (each gesture one command), `tabula_host.rs` (`PedalHost`: a pedal's full editor) |
| Face | `crates/cantus-ui/src/kit/skin/side.rs` (`TABULA`: fine black vinyl, brass), the `FineTolex` material |
| Strings | `crates/cantus-ui/locales/fx/tabula.{en,es}.txt` |
| Presets | the factory's boards in code (`tabula_presets::factory`, fourteen), their styles and roles in `crates/cantus-ui/presets/effects/board.toml` |
| Tests | `crates/cantus-engine/tests/fx_tabula.rs`, `fx_contract.rs` (`a_board_of_one_is_the_unit`, split and rest), `rt_memory.rs` (`tabula_never_allocates`), `crates/cantus-core/tests/undo_invariants.rs` (the board's arm), `crates/cantus-ui/tests/tabula_e2e.rs`, `crates/cantus-plugins/tests/tabula_board.rs` |
| Exported | `Cantus Tabula` (CLAP/VST3): the macros and four levels as parameters, the board in the state (`crates/cantus-plugins/src/board_post.rs`) |

## What the references do well, and what people complain about

- Helix and Guitar Rig make the signal flow visible (a grid of blocks, a rack) and let a
  chain split and join; AmpliTube and Logic's Pedalboard show real pedals on a floor;
  Neural DSP's pages are beautiful, but each holds only its own fixed pedals.
- People complain about a split that changes the timing between its lanes (comb filtering),
  about bypass that clicks or jumps the latency, about a chain whose order cannot be
  changed by dragging, and about "macros" that only reach a few hand-picked knobs.

## What Tabula does

- **Any of Cantus's units as a pedal** — drives, filters, modulation, delays, reverbs, the
  amp, the pitch and the rotary once their builders land — each drawn as a real enclosure in
  its own paint, its face's three or four knobs on it. Sizes: mini, standard, wide, double,
  round; any pedal can wear any.
- **Pre → (A ∥ B) → Post.** A pedal on lane A or B opens the split: a splitter box feeds
  both lanes, a mixer box joins them (its knob the A/B balance). The shorter lane is delayed
  to meet the longer one, to the sample — an impulse through the two lanes arrives once.
- **Latency** = the Pre lane's + the longer split lane's + the Post lane's. A footswitch
  crossfades in 10 ms; a pedal without latency then stops running, a latent one plays its
  input through a dry line of its own latency — **a footswitch never changes the latency.**
- **Structure without a click.** A pedal added fades in, one taken off fades out and is
  handed back to be freed off the audio thread; a new order or a lane dips the board to its
  dry signal for 10 ms each side. All of it lands on a 16-sample grid: the same bits at any
  block size.
- **Macros.** Eight knobs; Learn, touch a macro, touch a pedal's knob: the knob is mapped
  from where it stands to the far end of its range, and wears a ring. A macro wins over
  automation of what it moves. Randomise sets every pedal's face knobs within musical
  ranges, seeded, one step of undo.
- **Every gesture is one step of undo**: a knob, a footswitch, a pedal moved, added or taken
  off, a shape, a macro's maps, a randomised board, a factory board loaded.
- **A pedal's full editor** opens in place of the board (double-click, Enter, or *Open its
  full editor*): the unit's own window, its keys the pedal's (`p{id}.{key}`), its
  automation and modulation the board's lanes.
- **Keyboard and screen reader.** Tab chooses the next pedal, Space its footswitch, Enter its
  editor, Delete takes it off, Ctrl+←/→ move it, ←/→ choose its knob and ↑/↓ turn it (Shift:
  finely); the board reads as a list of pedals, each a toggle and its knobs sliders.
- **Exact rest and nulls.** An empty board is a wire, an empty split at 50 % is a wire, and
  a board rests when every running pedal rests and its lines are silent.

## Parameters

| Key | What | Range | Default |
|---|---|---|---|
| `m1` … `m8` | the macros | 0 … 100 % | 0 |
| `in` | the level into the board | −24 … +24 dB | 0 dB |
| `out` | the level out of it | −24 … +24 dB | 0 dB |
| `mix` | dry/wet, the dry aligned to the latency | 0 … 100 % | 100 % |
| `ab` | the split's balance (0: A only, 100 %: B only) | 0 … 100 % | 50 % |

A pedal's parameter is `p{id}.{key}`: `p2.time_ms` is pedal 2's delay time. Automation,
modulation, MIDI learn and the AI's tools use that key as it is.

## The starting rig and the presets

A new Tabula starts on the guitar rig: the green overdrive low, a chorus, a delay and a
spring reverb. The factory's boards (each a whole board, loaded in one step): Guitar rig,
Blues crunch, Ambient swells, Fuzz lead, Phaser funk, Wet, dry, wet; Bass: parallel drive,
Bass: chorus; Synth in space, Synth on tape; Drums: crushed in parallel, Drums: a room;
Vocal chain, Vocal doubler. A board preset is a flat map of numbers (`tabula_presets`), so it
travels every path a preset does — the menus, the browser, A/B, the agents, your own presets.

## Exported (CLAP/VST3)

The host sees the board's twelve parameters (the macros and the four levels) — a pedal's own
knobs are content, not parameters, so they can never retarget after a reorder. The whole
board is saved in the state as the project saves it; edits on the plug-in's own window (the
same editor) go to the board, are posted to the audio thread with a unit built for each new
pedal, and are saved with the state. Lost in export: a host's automation of individual
pedals' knobs (map them to macros), the host's undo of board edits, user impulse responses'
paths (machine-local).
