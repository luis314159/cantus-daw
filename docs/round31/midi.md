# Round 31 — midi (item 2): note effects, keys and scales, grooves

Branch `r31/midi` (worktree `F:\projects\daw-r31\midi`), from the contract commit. ADR 0069
(`docs/adr/0069-note-effects-keys-and-grooves.md`), schema 43, no keymap change (no new
`Action`: the switches live on the piano roll's toolbar and the inspector).

## What was built

- **Note effects before the instrument** (`cantus_core::notefx`, `cantus_engine::notefx`): a
  chain of up to eight per track — **arpeggiator** (up, down, up-down, down-up, as played,
  chord, random by seed; 1–4 octaves; 20 synced rates with dotted and triplet values; gate
  5–200 %; MPC swing 50–75 %; hold/latch; played or fixed velocity), **chords** (22 shapes
  plus the key's triad and seventh; inversion 0–3; close/open/wide voicing), **scale** (snap
  to the nearest note or drop). One engine path for clip notes, live input (also stopped, on
  the sample clock) and the loop's edge; sample-accurate; deterministic; allocation-free.
  Every parameter is an automation target (`AutoTarget::NoteFx`) — automatable and modulatable,
  from rows in the inspector's *Note effects* section (choices open lists,
  switches flip, numbers drag; each with the parameter menu).
- **Bake to notes**: the clip replaced, one undo step, by the notes the chain plays from it
  (groove included); the new clip plays past the chain (`Clip::skip_note_fx`, visible and
  switchable in the clip's inspector).
- **The song's key** (`Project::key`, 19 scales): the piano roll's toolbar *Key* list (roots
  beside scales); **Scale** lights the key's rows and marks the root and the keyboard;
  **Fold** shows only the key's rows (and any row a note is on), kept centred on the notes;
  **In key** snaps drawn, painted and dragged notes and makes `Up`/`Down` move by degrees.
  Chords and the scale read it, or an own key per effect.
- **Grooves** (`cantus_core::groove`): a clip's groove — built-in 8th/16th swings 50–75 % or
  the project's pool — applied non-destructively at snapshot compile with timing (0–150 %),
  accent and length amounts; *Commit*; *Extract groove* from a MIDI clip's notes or an audio
  clip's transients (the warp analysis's onsets, through its warp); the channel rack's
  *Groove* field sets the whole pattern; linked clips share it.
- **No leftovers**: MIDI export asks *As played* (grooves and note effects written out) or
  *As written*, and says which in the dialog; a clip's export now carries the pool and key;
  freeze, bounce, stems and export hear the chain (tested); the freeze subject holds the key
  and the pooled grooves where they matter; split, copy, link, paste and a recorded comp's
  pieces keep a clip's groove and bypass; a track coming in with a chain id another track
  holds is refused, and exploded takes get fresh ids (`notefx::renew_ids`, for track
  templates too); the manual EN/ES has three sections.

## How it was tested

- `cantus-core`: unit tests of scales, chords, the scale effect, parameters, grooves
  (MPC arithmetic, extraction round trip, invalid grooves); `tests/grooves.rs` (no amount /
  straight swing = no groove to the event; swing on every repetition; linked copies share;
  commit plays what was heard; baked events pass the chain; duplicate chain ids refused);
  the undo fuzz's round-31 arm with per-kind applied/refused coverage (seeds raised 60 → 120:
  one more arm spread the tempo arm's draws thinner).
- `cantus-project`: `tests/schema43.rs` (the v43 fixture whole, saved back equal; v41 none).
- `cantus-engine`: `notefx` unit tests (every mode, swing, gate, hold, chords, key chords,
  scale, chains in both orders, a baked clip); `tests/notefx.rs` — the null tests (empty,
  switched off, keyless scale, also on a kit: bit-identical), bake = render to the bit at
  blocks 1/7/64/1000/4096, block independence of chords→random swung arp→scale, real time
  and a 3-thread pool = offline, a stem, a frozen track, automation and an LFO on
  parameters, live chords, a stopped arpeggio; `rt_memory::note_effects_never_allocate`.
- `cantus-ui`: `tests/notefx_flows.rs` (chain edits one step each; bake and commit change no
  sample; extraction; MIDI as played/as written; transposition in key; the inspector, the
  lists and the piano roll render folded and not).
- Shots: `shoot_notefx` (chain, choice list, add list, groove, piano roll lit, folded, key
  list, channel rack, a clip's groove section) in Night and Paper, EN and ES, 100 and 200 %;
  the best in `docs/round31/midi-shots/`, three in the manual (`docs/manual/img/notefx-*.png`).

## Lines changed in shared files

- `graph/track.rs` (midi's): `notefx` field and its init; `all_notes_off` split into
  `instrument_notes_off` + chain reset; `reset` resets the chain; the note-effect methods
  (`fx_active`, `configure_notefx`, `set_notefx_param`, `fx_play`, `fx_next_due`, `sound`);
  in `run_track` the `fx`/`base` locals, the clip-event branch into the chain and the
  chain's due step; `fx_step`. **Merge with automation:** `sound()` calls
  `self.note_off(m.pitch, m.tag)` — marked `r31 automation:` — pass `m.release` there once
  `note_off` takes the release velocity.
- `processor_snap.rs`: one line, `s.configure_notefx(&t.note_fx, snapshot.key)`.
- `processor_msg.rs`: `sound_live` through the chain (block).
- `processor_pass.rs`: `fire_due_now` through the chain (block).
- `processor_auto.rs`: the `Dest::NoteFx` arm. `automation.rs` (engine): `Dest::NoteFx` and
  its `dest_of` arm.
- `views/piano_roll.rs`: 7 marked lines (`// r31 midi`): the `scale` module, the axis fold,
  rows, keys, toolbar, draw/paint snap, drag move. `piano_roll_paint.rs`: 4 marked lines (a
  folded keyboard is drawn by `piano_roll_scale.rs`).
- `cantus-core`: `command.rs` (8 variants, labels, `supersedes` arms, apply arms calling
  `notefx_cmd`, `edit_shared` made `pub(crate)`, one `InsertTrack` check line);
  `model.rs` (`Track::note_fx`, `Clip::{groove, skip_note_fx}`, `Project::{key, grooves}` and
  their constructors); `snapshot.rs` (`note_fx`, `direct`, `key`; `compile_events` applies
  grooves); `links.rs` (`ClipContent` carries both); `clipboard.rs` (split/copy/notes clip);
  `freeze.rs`/`freeze_guard.rs` (subject and touches); `automation/{mod,range,set}.rs`
  (`AutoTarget::NoteFx`); `lib.rs` (modules).
- `cantus-project`: `format.rs` (schema 43; `migrate_41_to_42` is an empty placeholder for
  automation's 42), `midi/export.rs` (`Notes`, `export_with`), `midi/import.rs` (two fields).
- `cantus-ui`: `app.rs` (the `notefx` field, the lists' frame call, the transpose hook);
  `lib.rs`; `views/inspector.rs` (module, `Act::NoteFx`, the two section calls);
  `automation.rs`, `views/modulator.rs` (NoteFx arms); `record_pass.rs`, `channel_rack.rs`
  (Clip fields; the rack's groove field); `comping.rs` (renew ids); `views/interop.rs` (the
  MIDI dialog's notes choice).
- Tests: `undo_invariants.rs` (arm 48, `below(49)`, 120 seeds, coverage counts),
  `rt_memory.rs` (one test), `links.rs`/`midi_file.rs`/`interop_flows.rs` and
  `shoot_interop.rs` (new fields).

## Numbers

Schema **43** (needs renumbering only if automation's 42 lands under another number);
keymap unchanged (10); ADR **0069**.

## What needs the owner

- **Listening**: the arpeggiator's feel (gate default 80 %, first step on the chord), the
  swing amounts against an MPC, chord voicings — by ear.

## Suite results

- **Windows**: `cargo clippy --workspace --all-targets` with `-D warnings` clean; `cargo test`
  of `cantus-core`, `cantus-project`, `cantus-engine`, `cantus-ui`: **3 188 passed**, 15
  failed — all hosted-plug-in tests (`hosted_instruments`, `hosted_plugins`,
  `controllers_review*`, `frame_budget`'s two hosted cases, `modulators`' plug-in window): the
  scan they assert on now also finds plug-ins installed on this machine (Odin2, Surge XT in
  `C:\Program Files\Common Files\CLAP|VST3`), e.g. "one instrument scanned: left 5, right
  1". Nothing of this branch touches hosting; the integrator should see them on any branch of
  this machine.
- **Linux (WSL, CI's flags)**: `tools/wsl/check.sh r31/midi clippy` clean (with the plug-ins
  without their editor); tests of `cantus-core`, `cantus-project`, `cantus-engine` and
  `cantus-ui`'s lib (i18n included) and `notefx_flows`: all green.
