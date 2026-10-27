# Round 31 — editing (items 3 and 5): crossfades, clip gain envelopes, effects that sound

Branch `r31/editing` (worktree `F:\projects\daw-r31\editing`), from `r31/integration` at
f38bce43 (midi and workflow merged). ADRs **0070** (crossfades and clip gain envelopes) and
**0071** (effects that sound on insert); schema **44**; no keymap change (no new `Action`:
the switches live in menus and *Settings ▸ Editing*).

## What was built

- **Crossfades between overlapping clips** (`cantus_core::crossfade`, ADR 0070 §1–4): the
  later of two overlapping playing-lane audio clips carries `AudioRegion::cross` — the
  curve (equal power, linear, S) over the whole overlap. **Automatic**: an edit that makes
  an overlap crosses it in the same undo step (the command bus composes the fix, as the
  tempo's window fix; `CommandBus::set_auto_crossfades`, set from *Settings ▸ Editing* at
  `assemble` and in an app built from parts); an overlap that was there keeps its choice.
  **One engine path with comp seams**: the snapshot names the stretch on both clips
  (`xin`/`xout`, now with a curve each); equal power is the seam's `sin`/`cos` to the bit, so
  no golden hash moved and `DSP_VERSION` stays 5. The fades a crossfade covers give way to
  it. Butt joins stay butt joins; a clip wholly inside another is layered.
- **In the arrangement** (Studio One's drawing): the overlap veiled, the earlier clip's fade
  out falling and the later's rising on the curve, a square at the top — dragged sideways
  it lengthens or shortens the crossfade about its centre (both clips' edges move, each as
  far as its file has audio), double-clicked it opens **the crossfade editor** (the curve
  drawn, *No crossfade · Equal power · Linear · S-curve*, *Length* in ms), right-clicked its
  menu. A summed overlap is drawn as one (dashed edges, a hollow square). The clip's menu:
  *Crossfade the overlap* (the selection's when the clip is in it); the timeline's menu:
  *Automatic crossfades*, *Show clip gain envelopes*.
- **Clip gain envelopes** (`cantus_core::clip_gain`, ADR 0070 §5–7): points in the clip's
  content time (−∞ … +12 dB), automation's six shapes and tension between them. Slip carries
  the line, trim hides points, split gives both halves the line, glue joins each piece's
  line, reverse mirrors it with the audio, a warp keeps it where it is drawn, *bounce the
  warp* moves it onto the new file. In the engine (`cantus_engine::clip_env`): each point on
  the sample its tick rounds to, the shape at the sample's own fraction between two points —
  block-size-proof, allocation-free, no `exp` on a level stretch. On screen: the line on its
  own scale (0 dB a quarter down, a square-root scale to −∞), the waveform drawn with the
  line, fades and crossfades applied; press on the line to add a point and drag it,
  double-click a point to delete it, a bend handle per bending segment, the segment's shape
  from the menu, *Reset gain envelope*. MIDI clips have none (velocity is their gain).
- **Effects that sound on insert** (`cantus_core::starting`, ADR 0071): every unit decided in
  one table (below); every insert from the interface goes through `CantusApp::new_insert`
  (the slot menus, the picker, the palette, a swap, a return); the defaults stay the null;
  *Settings ▸ Editing ▸ Effects ▸ On insert ▸ Neutral settings* turns it off; an editor
  opened on a factory preset's values names it (*Juno I*); our exported CLAP/VST3 plug-ins
  start on the same preset in another DAW, their parameters' defaults still the null.
- **No leftovers**: freeze, bounce, stems and export hear crossfades and envelopes (tested);
  a bounce of one of two crossed clips is refused and a time range's edge moves out past the
  overlap (keeping the earlier clip reaching under the later); copy/paste/duplicate/links
  carry the region; DAWproject writes a crossed overlap as its two fades (read back as a
  crossfade) and an envelope as the clip's gain expression beside its audio (valid against
  the official XSD), reporting what it cannot say; schema 44 with `fixtures/v44.cantus`;
  Settings ▸ Editing in the sidebar, the Options menu and the palette; the manual EN/ES
  (crossfades, clip gain envelopes, the starting presets in the modulation and dynamics
  sections); BACKLOG §3b marked done.

### The starting presets (ADR 0071's table)

| Unit | Starts on |
|---|---|
| Aequalis, Latitudo | neutral: flat by design |
| Comprimo, Spatium, Reverb, Delay, Sibilus, Porta, Tonus | neutral: already sound at their defaults |
| Terminus, Culmen, Tonsor | neutral: safeties, must not surprise |
| Speculum | neutral: only measures |
| hosted plug-ins | their maker's initial state |
| Multiplex | *Mastering glue* |
| Fervor | *Tape on the bus* |
| Concentus | *Juno I* |
| Flexus | *Jet plane* |
| Phasis | *Phase 90, script* |
| Tremor | *Surf amp* |
| Vibrans | *Singer* |
| Anulus | *Bell* |
| Translatio | *Slow phase* |
| Fluctus | *Pump on the quarters* |
| Impetus | *More punch* |
| Discrimen | *More attack* |

## How it was tested

Tests first, then the code. New files: `cantus-core/tests/crossfades.rs` (7),
`cantus-core/src/{crossfade,clip_gain}_tests.rs` (8 + 7), `cantus_core::starting` (2),
`cantus-engine/tests/clip_xfades.rs` (7), `rt_memory::crossfades_and_clip_envelopes_never_allocate`,
`cantus-ui/tests/{clipfx,clipfx_boot}.rs` (9 + 1), two in `cantus-ui/tests/freeze.rs`,
`cantus-ui/src/clipfx.rs` (4), `cantus-project/tests/dawproject_clipfx.rs` (4), schema 44's
fixture test, the plug-ins' `a_fresh_plugin_is_the_dsp_on_its_starting_preset_and_its_defaults_stay_the_null`,
and the undo fuzz's `editing_sequences_undo_and_redo_whole` (60 sequences of the editing arm and
clip moves, half with automatic crossfades on) — plus the main fuzz now runs every other
sequence with automatic crossfades on.

- **Null, to the bit**: an overlap with no crossfade is the sum it always was; no envelope,
  no points, or a line at 0 dB plays the clip unchanged; every comp seam and every golden
  hash is unchanged (`DSP_VERSION` stays 5 — equal power is the seam's own `sin`/`cos`).
- **The crossfade**: on every curve, at every sample of the overlap, the earlier clip's gain
  is exactly the curve's fade-out and the later's its fade-in at the same `u`; equal power
  holds `l² + r² = 1`, linear and S `l + r = 1`; monotone; before and after untouched.
- **The envelope**: lands on its points' samples, −6 dB half-way in dB, held before and
  after, carried by a slip, mirrored sample for sample on a reversed clip, the floor
  silent.
- **Block sizes 1, 37, 64, 1 000 and the live callback (uneven blocks) at tolerance 0**; the
  stems' master equal to the mix to the bit; no allocation or free on the audio thread,
  serial and pooled (played through, sought into the overlap, looped across it).
- **Freeze, bounce, stems, export**: a frozen track with a crossed overlap and an envelope
  plays its render to the bit; a bounce in place within −100 dB; one clip of a crossed pair
  refused; a time range starting inside the overlap moved out and the song unchanged.
- **The model**: the automatic crossfade is one undo step, undone and redone with its edit,
  off on a bare bus, a removed one stays removed; split keeps the line on both halves and
  the crossfade on the left; invalid envelopes refused by `SetClipAudio` and `InsertClip`;
  the sweep agrees with `partner` on 200 random layouts; schema 44 loads, v43 has neither.
- **The interface, from the real arrangement**: dragging the crossfade's square lengthens it
  about its centre in one step (content kept), a double-click opens the editor and Esc
  closes it, the covered fades lose their squares; a press on the line adds a point and
  drags it (one step), a double-click deletes it, a point is held between its neighbours;
  an app built from parts and the boot path (`assemble`) both tell the bus the setting.
- **Starting presets**: an insert from the slot menu is its starting preset (Juno I, mix
  50 %), a limiter starts neutral, a swap takes the new unit's preset, *Neutral settings*
  gives the defaults; every starting preset is, value for value, the factory preset the
  editor lists; an editor opened on one names it; the exported plug-ins start there,
  bit-identical to the DSP, every parameter's default still the null.

**Suites.** Windows, `cargo clippy --workspace --all-targets -- -D warnings` clean;
`cargo test` of every crate touched — cantus-core, -engine, -ui, -project, -plugins, -host,
-ai, -bench — **3 490 green** (engine 881, the rest 2 609). The editors' tests that check a
unit at its defaults now insert it neutrally (one line each, 13 files), and the plug-ins'
tests read a fresh instance's starting preset (ADR 0071 §5). Linux (WSL, CI's flags, `tools/wsl/check.sh` in `~/ci-r31-editing`): clippy clean (with
and without the plug-ins' editor); `cargo test --workspace` **4 598 green**. A first run
reported one target failed — its name lost to the script's `tail -60` — and a full rerun of
the same commit passed everything: a timing test, not this round's code, as far as one
rerun can say.

## Shared files (additive lines)

Additive except where said: `cantus-core` — `audio.rs` (+2 `AudioRegion` fields),
`bus.rs` (the `auto_xfade` field and its two methods; `apply_keeping_audio` split into the
tempo fix and the crossfade fix through a new `apply_fix`), `command.rs` (+2 validation
clauses), `snapshot.rs` (+3 `AudioClipSnapshot` fields, `crossfades()` after `seams()`,
`audio_room` made `pub(crate)`), `clipboard.rs` (split clears `cross` on the right half),
`lib.rs` (+3 modules); `cantus-engine` — `processor_rec.rs` (`add_clips`: the curves and the
envelope's cursor, 16 lines), `lib.rs` (+1 module); `cantus-project` — `format.rs` (schema
44, `migrate_43_to_44`, its fixture test), `write_fixture.rs` (+1 block), DAWproject
`write.rs`/`read.rs`/`read_timeline.rs`/`audio.rs`; `cantus-plugins/src/instance.rs`
(`Shared::new`'s initial values); `cantus-ui` — `app.rs` (+3: `apply_editing_settings` in
`from_parts_db` and `assemble`, `add_effect` through `new_insert`), `swap.rs` and
`routing_actions.rs` (1 line each: `new_insert`), `settings.rs` (+4 fields),
`views/arrangement.rs` (the cursor through `audio::edit::cursor`, the handle menus and the
crossfade editor's call, two timeline-menu toggles and their renumbered arms),
`views/workspace_settings.rs` (+ the Editing page), `macos/menu.rs` (+1 Options item),
`views/fx/frame.rs` (`recognise`, `Chrome::recognised`), `bounce.rs` (`overlaps_of`),
`audio_clips.rs` (glue merges envelopes), `warp_bounce.rs` (the envelope moved with the
content), `lib.rs`/`views/mod.rs` (+1 module each); `undo_invariants.rs` (+ the editing
arm and its test, auto crossfades on every other sequence); the locales' `clipfx` area
(EN/ES); `docs/BACKLOG-excellence.md` §3b (option B marked done).

## Shots

`cargo run -p cantus-ui --example shoot_clipfx -- <dir>` — five scenes (`xfade`,
`envelope`, `editor`, `settings`, `insert`) in Night and Paper, English and Spanish, at 75,
100 and 200 %. Looked at, and fixed what they showed (the editor's curve names and length
label were elided; Settings' curve row too). In `docs/round31/`: `editing-crossfade.png`
(a crossed overlap and a summed one), `editing-envelope.png`, `editing-xfade-editor-es-paper.png`,
`editing-settings.png`, `editing-insert-juno.png` (Concentus inserted, its header naming
*Juno I*).

## Merge and review 1

Merged `r31/integration` at 7fe6d444 (automation): schema 44 follows 42 (automation) and
43 (midi), `v44.cantus` written again with everyone's data (v42/v43 untouched); settings,
DAWproject's clip reading (lanes beside the gain expression) and `cantus-core`'s modules
joined side by side; the bus's crossfade fix sits inside `apply_keeping_audio`, so it is
part of the command a folded step replays and the step bookkeeping is unchanged.

The review's nine findings, each with a test that failed before the fix (checked by
switching each fix off): a split inside a crossed overlap (engine, to the bit, both clips,
every curve); the envelope through a tempo track (move and slip, a fixed source second);
a damaged envelope repaired on load (fixture test); a looping clip's line (UI ↔ engine at
the fourth repetition); the square's drag (4 and 13 frames, exact); a stale curve (core:
moved out with automatic crossfades off, and copies); the editor's range (no 4 s cap); the
square off the fade squares (hits kept, shot); costs (20 000 clips, 5 000 crossed clips in
a snapshot). ADR 0070 has the amendment.

Suites on 0d3cb083: Windows `cargo clippy --workspace --all-targets -- -D warnings` clean,
`cargo test --workspace --no-fail-fast` **4 682 passed, 0 failed** (log
`F:\projects\daw-r31\tmp\editing-review1-win-test.log`). WSL (CI's flags, log
`editing-review1-wsl.log`): fmt, clippy and the plug-ins without their editor clean; tests
4 656 passed, 2 failed while the Windows suite ran beside it —
`cantus-sandbox` `session::a_crash_mid_song_keeps_cantus_playing_says_so_once_and_reload_brings_back_what_was_set`
(19 016 late blocks: a starved CPU) and `cantus-ui` `tempo_review6_ui::one_kept_or_left_out_is_said_in_the_singular`
(the interface's language, set by another test of the binary); both passed three runs out
of three alone (`editing-review1-wsl-rerun.log`). Neither is code of this branch. The merge's
own run (986cffdb) passed 4 667 tests; its doctests failed to link only because a build of
the fixes ran beside it.

## Left for the owner

- **By ear**: whether each starting preset is the one to start on (the table above; one
  line of `cantus_core::starting` each), and the crossfade curves on real material (the
  default equal power for different recordings, linear/S for takes of one thing).
