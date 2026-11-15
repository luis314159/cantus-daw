# Round 40 — `delay`: Delay synced to the tempo, and Delay as a CLAP/VST3 plug-in (ADR 0119)

**The plug-in's ids — forever, for the owner to see before the round merges:**

| | |
|---|---|
| Name | **Cantus Delay** (slug `delay`, the 25th exported plug-in) |
| CLAP id | `com.github.lalmazan.cantus.delay` |
| VST3 class id | `625C4F90AB5A380458F1F1D3D1DCA44C` (`[0x625C4F90, 0xAB5A3804, 0x58F1F1D3, 0xD1DCA44C]`: SHA-256 of `cantus/vst3/delay`, first 16 bytes — `desc.rs`' rule) |
| Features | CLAP `audio-effect`, `delay`, `stereo`; VST3 `Fx|Delay` |
| Parameter ids | FNV-1a of each key (`tests/golden/ids.txt`, `delay …` lines): `time_ms`, `feedback`, `tone_hz`, `ping_pong`, `mix`, `sync`, `note`, `cantus.sidechain`, `cantus.bypass` |
| `note`'s values | forever too: `3 × base + feel`, base 1/64 (0) … 4/1 (8), feel straight · dotted · triplet |

The owner's answer 22 (2026-11-12): *Delay's time synced to the tempo* and *Delay as a
CLAP/VST3 plug-in*. Both built.

## What changed

**1. The synced time** (`sync`, `note`; no schema bump — two parameters in the unit's state,
missing keys read as their defaults).

- **The sync off is the round-39 unit, instruction for instruction**: `process_free` is the
  old loop; a frozen copy of the round-39 unit in the tests is compared to the bit, through
  a time change and with a tempo told and a far note value. `DSP_VERSION` stays 6; no golden
  hash moved.
- **The synced head** follows the tempo the engine tells each pass (the tempo track's ramp is
  told in steps that each start a pass, so every block size hears the same tempo at the same
  sample). Inside a ramp the head goes straight to each step's time over the step — the
  physics of a tape whose speed follows the ramp, a few per cent of pitch; a small move glides
  over 50 ms; **a jump crossfades over 50 ms**. My first try glided everything over the
  time smoother's 50 ms; the no-click test caught it racing the head 28× on a jump (the
  past replayed at 28 times its speed) — hence the crossfade. In `f64`, so a long line's head
  never steps.
- **The line grows with the note** (its time at 20 BPM, the slowest tempo, in whole seconds, 2 …
  48 s) on ADR 0022's path (`MemoryNeed::DelayLine`): built in place when a unit is made with
  the sync on, asked once after a snapshot that turns it on, answered before an offline render's
  first block; the history copied in, so a line that grows mid-song changes no sample. Off
  again, the free line comes back and the unit is as good as new.
- **`follows_song` is true only while synced**, so with the sync off the engine cuts Delay's
  passes as it always did.
- **At rest** a new tempo is taken at once (silent lines: nothing can be heard moving), so a
  resting strip's skip is exact.

**2. The window** (`views/fx/delay.rs`, `delay_view.rs`). **_At the tempo_ became the switch**,
not a control beside it: two ways of putting a note value on the time that behave differently
after a tempo change would be a trap, and the old set-once is *Sync, then Free*. The time
card's column is *Tempo*: **Free · Sync** (the modulation family's words and control), the
note value's field (each value listed with its time at the song's tempo), and one sentence of
what the time is now. *Sync* picks the note nearest the time it was playing; choosing a note
turns the sync on; *Free* keeps the time it was playing in milliseconds (each one undo step:
`cmd.delay_sync_on`, `cmd.delay_sync_off`). Synced, the XL knob turns the note value, the
echoes are drawn at its time at the song's tempo, and a drag across picks the nearest note.
The inspector's strip names the note. Six factory presets named by a note value follow the
tempo (*Eighth echo*, *Dotted eighth*, *Quarter-note throw*, *Trap vocal echo*, *Sixteenth
bounce*, *Triplet echo*).

**3. The plug-in.** `desc::ALL` has Delay (25), the cargo feature `delay`, the bundler builds it
with its editor (`xtask`: `delay,editor` — one line outside my files, marked `// r40:delay`).
Its window is the DAW's own on `FxHost` (`editor::has_editor` is true for it now). Synced, it
follows the host's tempo through `Processor::position`, as the modulation units do; its longer
line comes from the plug-in's memory thread (Aula's, now for Delay too).

| Layer | Files |
|---|---|
| DSP | `cantus-dsp/src/delay.rs` (the two loops, the line's need and supply, the tempo), `delay_sync.rs` (new: note values, the line rule, the read, the head), `delay_tests.rs` (new: the old tests and 11 new) |
| Parameters | `cantus-core/src/effect.rs`: `sync`, `note` at the end of `DELAY_PARAMS`, `DELAY_CHOICES` (marked `// r40:delay`) |
| Engine | `cantus-engine/src/effects.rs`: `delay_settings`, `MemoryNeed`/`NeedTicket`/`EffectMemory::DelayLine`, Delay in `set_position`/`follows_song`; `lib.rs` exports `delay_settings`; `tests/fx_delay_sync.rs` (new) |
| Plug-ins | `desc.rs` (Delay), `Cargo.toml` (`delay`), `build.rs` (its version resource name), `tests/clap.rs` (the parameter its automation test moves), `instance.rs` (the memory thread for Delay), `memory.rs`, `editor.rs` (comments), `tests/common/mod.rs` (Delay's settings and direct DSP), `tests/transport.rs` (a synced Delay in another host), `tests/golden/ids.txt` (Delay's ids) |
| Window | `views/fx/delay.rs` (the Tempo column, `set_sync`, the knob), `delay_view.rs` (`heard_at`, the note names, the drag, the spans to 320 s, the strip) |
| Strings | `locales/fx/delay.{en,es}.txt` (the parameters, the 27 notes, the column; three round-39 keys retired) |
| Presets | `presets/effects/delay.toml` (six follow the tempo) |
| Agents | `agent_units.rs` (Delay's line says how to sync it) |
| Tests (UI) | `tests/delay_window_e2e.rs` (the note list and the switch), `tests/delay_sync_e2e.rs` (new) |
| Docs | ADR 0119; `docs/plugins/delay.md`; the manual EN/ES (Delay's section); README (Delay is a plug-in; its card); `web/src/units_data.rs` (Delay's card: `plugin: true`, the sync); BACKLOG (appended); `beta-checklist.md` (Ronda 40, delay) |

## How it is tested

- **`cantus-dsp` `delay::tests`: 13 passed** (2 kept from before, 11 new): the free time to the
  bit against a frozen copy of the round-39 unit (through a time change, with a tempo told and
  a far note value); the note values promised; the line each note needs; each note's echo at
  its time at six tempos; through a ramp and a jump the same bits at block sizes 1, 7, 64,
  256 and a mixed cut; no click, zipper or sweep through a ramp and a jump (the output's
  steepest step held to the sine's); the head straight inside a ramp's step; a line that
  grows mid-song changes no sample; held to the line until the longer one comes; at rest a new
  tempo is taken at once and skipping is exact; back to the free line, as good as new.
- **`cantus-engine` `tests/fx_delay_sync.rs`: 5 passed** — through the tempo track's ramp and
  a jump every block size (1, 37, 64, 333, 512, 4 096) renders the same to the bit; offline =
  real time (the processor driven in blocks of 17 … 1 024); the echo a quarter at 120 and at
  90 BPM across a tempo change while the free time stays 500 ms; a song saved without the keys
  renders as the sync off, and no note value moves a sample while it is off; a whole note
  turned on while playing asks for its 12 s line once and plays a whole note late once it has
  it. **The whole `cantus-engine` suite passed** (its golden hashes stand: `DSP_VERSION` 6), and
  `rt_memory` (55), `fx_contract`, `effects_and_midi` again on my own build.
- **`cantus-core`: the whole suite passed** (the undo fuzz walks Delay's two new keys through
  `SetEffectParam`; no new command).
- **`cantus-plugins`: the whole suite passed** with Delay in every family test — bit-identical
  through CLAP and VST3 to the DSP and to the engine's unit, any block size, automation of its
  feedback on the grid, state both ways, the bypass, every sample rate, the editor opens in its
  face, the names and choices in both languages, the version resource (`build.rs` names
  *Cantus Delay*), the ids' golden file blessed (11 lines, `delay …`); `transport.rs`: a
  synced Delay follows a host's 90 BPM bit for bit at block sizes 1 … 4 096, and a host's tempo
  change bit for bit with the engine's unit told the same tempos.
- **`cantus-ui`**: `delay_window_e2e` 7 passed (new: the note list turns the sync on as one
  step; *Sync* picks the nearest note, the knob reads it, `SetTempo` 100 moves what is said to
  450 ms, *Free* keeps 450 ms, undo, a drag steps the note; Spanish says *Libre · Al tempo*);
  **`delay_sync_e2e` 1 passed** (`CantusApp::assemble`, the window's `set_sync` through its
  host, renders through `compile_for_render` across a tempo change: echo 0.5 s at 120 and
  2/3 s at 90 BPM, the free time 0.5 s; *Free* keeps 666.7 ms; undone, the render is the
  free one to the bit); the family's walkers (`fx_frame`, `learn_everywhere`,
  `modulate_everywhere`, `skins`, `skins_body`, `skins_text`, `master_chain`, `legacy_reverb`,
  `picker`, `preset_audit`, `stated_preset_counts`, `fx_lint`) and the library's delay, i18n
  and preset tests (36) passed. The whole `cantus-ui` suite was not run here (slots); the
  integrator's run covers it.
- **Clippy** `--all-targets` with `RUSTFLAGS=-D warnings` on `cantus-dsp`, `cantus-core`,
  `cantus-engine`, `cantus-plugins`, `cantus-ui`, `xtask`: clean. `cargo fmt` on each.
- **Linux**: nothing behind `cfg(windows)` was added; not checked in WSL (the integrator's).

**A hazard for the integrator (found here, not mine to fix):** `target-pd` is shared with
`pads`, and cargo hashes a workspace crate without its path, with relative paths in its
dep-info — so the two worktrees write the **same** artifacts for `cantus-dsp`, `cantus-core`,
`cantus-engine`, `cantus-ui`, and whoever built last wins: a build that sees the other's sources
older than the output counts it fresh. My plug-in build compiled against `pads`' `cantus-dsp`
(no `sync`) twice. From then on I built with a profile of my own (`[profile.delay]` in my
untracked `.cargo/config.toml`, artifacts in `target-pd/delay/`); **`pads`' builds before that
may have linked my `cantus-core`/`cantus-engine` or I theirs** — worth a clean check of `pads`'
results. The plug-in tests also build into the worktree's own `target/test-plugins` (374 MB,
as before).

## Verified only in emulation

- The synced time through a tempo ramp and a jump was **measured, not heard**: the bits, the
  echo times and the output's steepest step (no click). Its sound on a real ramp is the
  owner's to hear.
- **Cantus Delay was driven only through its CLAP entry point in-process and the VST3 tests'
  host** — never loaded in REAPER, FL Studio or Bitwig. The bundle was not built here
  (`cargo run -p xtask -- bundle delay`).
- The window's clicks were synthetic pointer events (`paths_rig`).

## What the owner should look at or listen to

- `docs/round40/delay/`: the window with the sync on (Slate and Paper, 100 % and 200 %, and in
  Spanish) — the *Tempo* column (*Free · Sync*, the note, the sentence), the knob reading the
  note.
- Listen: a song with a tempo ramp and a jump, Delay on *Dotted eighth* (it follows the tempo
  now) — `docs/beta-checklist.md`, *Ronda 40 — Delay*.
- The ids above, before the round merges.

## Decisions taken (the owner may overrule)

- *At the tempo* **became** the switch (§2 above, ADR 0119 §5).
- Inside a ramp the repeats **slide** (pitch follows, as tape); a jump **crossfades**.
- The note list goes to **4/1** (four whole notes) and the line to **48 s**; a dotted 4/1 under
  30 BPM is held at 48 s. The plug-in's name is **Cantus Delay** (the app's name; the slug,
  which the ids come from, is `delay` either way).
- Six presets named by a note value follow the tempo now.

## Left over

- Inside another host a long note's line comes from the plug-in's memory thread a moment
  after it is chosen (until then a synced time over 2 s is held at 2 s); automating the note
  longer while the DAW plays asks at the next snapshot. Both said in ADR 0119.
- §3b's other delay ideas (multi-tap, ducking, reverse, dub filter) stay in the BACKLOG.
