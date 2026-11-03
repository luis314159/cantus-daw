# Round 36 — editor: the audio editor (ADR 0096)

Branch `r36/editor`. **Schema 50 in the branch — renumber to 52 at merge** (`migrate_49_to_50`,
`fixtures/v50.cantus`, `tests/schema50.rs`; after mix's 50 and external's 51 it becomes
`migrate_51_to_52` and `v52.cantus`). One new `Command`: `SetAudioSourceMarks`.

## Done (first delivery)

- **Opened from a clip and back**; the window follows undo/redo/agents to the clip's current file.
- **Destructive edits as new content-addressed files** (`Audio/edit-<sha256:16>.wav`, written beside
  then renamed), one `Batch` per edit — one undo step, the original kept, same bytes = one file.
- **Operations** (`cantus_core::audio_edit`, pure): cut/copy/paste/delete, trim, silence, insert
  silence, fades (5 curves), gain, normalize (peak/RMS/LUFS), invert, reverse, DC, remove silence,
  noise reduction by profile, spectral region, declick, repair.
- **Spectral view**, log frequency, drawn at its pixel size; region pick and attenuation.
- **UI** in the kit, both themes; agents; manual EN/ES; locale area `editor`.

## The integrator's review — every item

1. **Joins** (`audio_edit_join.rs`): zero-crossing snap (±5 ms, toggle *Smooth joins*, on by default)
   and a **3 ms equal-power crossfade** at every join — delete/cut, trim's ends, paste's two seams,
   inserted and written silence, reversal edges, remove-silence cuts. Argued in ADR §3a (uncorrelated
   sides → equal power; 3 ms: above a 333 Hz cycle, under a consonant's attack). Test: a mid-wave
   220 Hz cut — raw step > 0.1, smooth < 0.03 (the tone's own max 0.0144), for snap only, crossfade
   only and both; pastes/silence/insert/trim likewise.
2. **Pitch/warp**: `Edit::keeps_timing` — kept by gain, normalize, invert, DC, fades, silence,
   denoise, attenuate, declick, repair; dropped by reverse, paste and any length change; the status
   says "dropped". Agent description corrected. Tests: core table; UI warp kept by gain, dropped by
   reverse with the word in the status.
3. **Empty take**: every result is checked (`WouldBeEmpty`); remove silence with keep 0 on silence is
   refused. Test.
4. **Denoise on a selection**: transform over the selection ± 4096 frames, 10 ms linear crossfades
   inside it (correlated sides); outside bit-exact. *Learn noise* clears the selection. Tests.
5. **Undo label / half cut**: the edit's name travels in the job and `Applied`; every act that starts
   work or touches the clipboard first checks busy and says "Wait…". Test (cut while busy leaves the
   clipboard, the step is labelled `cmd.editor.gain`).
6. **Agents off the UI thread**: `edit_audio`/`denoise_clip` are `ToolRun::Job`s (read, edit, write on
   the job thread; `JobDone::Then` plans the step).
7. **Cleaning**: *Remove unused audio…* (editor toolbar *Clean up…*, the audio clip's menu, agent
   `remove_unused_audio`): lists unused pool entries and unreferenced `edit-*.wav`/`.part` with sizes,
   removes the entries in one undo step, deletes only editor files referenced by nothing (project, undo
   **and** redo history, the saved file, its autosave and backup), only in a saved project's folder.
   History-holds argument in ADR §7. `.part` leftovers tidied at open. **No orphans**: a stale result
   (closed window, other take), a refused commit and a declined/stale agent plan remove the file they
   wrote (pending entries settled each frame). Tests (cleanup, closed window, declined agent edit).
8. **Another project closes the window**: `reset_bus_to` (every New/Open/template/recovery — marked
   line) and an epoch check for undoable replacements; results arrive after close are discarded. The
   folder is re-checked at commit (a Save As moves the file). Tests.
9. **Detail spectrogram**: zoomed in, the visible span ± one view width is recomputed at a hop of the
   view's resolution (64–512, power of two) off the UI thread. Test: a click on a **5-minute take**
   found in the detail view (hop ≤ 512, finer than the overview) and repaired to within 0.01.
10. **Region reach** narrowed to ±(N/2 + HOP); test checks the first samples outside keep their bits.
11. **Minor**: trim (and every length-changing edit) maps markers correctly (`moved_marks`); LUFS on
    < 400 ms → "too short to measure"; RMS/LUFS normalize has a peak ceiling (default on, −1 dBFS) or
    says the clipping; keep_ms 0 never touches audio outside the selection (test); agent `seconds: 0`
    and `lo_hz >= hi_hz` are bad arguments (test).
12. **Edison gaps**: paste across rate (the engine's sinc resampler) and channels (test 44.1 kHz stereo
    → 48 kHz mono); *Open file…* and *Paste from file…* (tests); **markers, regions, loop points saved
    with the file** (schema bump, `SetAudioSourceMarks`, undo-fuzz stream, `freeze_guard` arm, load
    repair, fixture; moved with the audio by edits — tests); **selection → new clip** (button and
    Alt-drag out of the window, one undo step — test); **declick** (AR detection + LSAR interpolation)
    and **repair selection**. **Record into the editor**: argued not here (ADR §10 — it would duplicate
    the recorder or drive the transport while *record* owns the recording mode; BACKLOG §3 notes it).
    **Insert effect rendered into a selection**: argued not here (ADR §10 — clip FX and bounce in place
    keep the effect editable).

## Tested (numbers)

- `cantus-dsp` `denoise` 12 tests (noise: hiss ≥ 12 dB down, voice within 1 dB; clicks: 4/4 found, none
  in a clean take, repaired within 0.03; region reach; bit-exact identities).
- `cantus-core` `audio_edit` 22 tests; `undo_invariants` with the marks stream; core lib 431.
- `cantus-project` with `schema50` (fixture marks load, v49 has none, bad marks dropped).
- `cantus-ui` `tests/editor.rs` **25 tests**; ui lib 439; `agent_tools`, `audio_clips`, `warp`,
  `ai_mcp_boot` (golden regenerated).
- Windows clippy `-D warnings` on dsp/core/project/ui `--all-targets`; Linux WSL clippy (workspace) and
  the touched tests.

## Shots

`docs/round36/editor/` — wave + spectrogram with marks/regions/loop, a picked region, noise learned,
samples, in Night and Paper, EN/ES, 100 % and 200 % (`cargo run -p cantus-ui --example shoot_editor`).

## Marked lines (`// r36:editor`)

- `cantus-dsp/src/lib.rs`: `pub mod denoise;`
- `cantus-core/src/lib.rs`: `pub mod audio_edit;` and the `SourceMarker, SourceMarks, SourceRegion`
  re-export.
- `cantus-core/src/model.rs`: `Project::source_marks` (field, serde attribute, `empty()` literal).
- `cantus-core/src/command.rs`: the `SetAudioSourceMarks` variant, its label, its apply arm, and
  `RemoveAudioSource` removing/restoring marks.
- `cantus-core/src/freeze_guard.rs`: `SetAudioSourceMarks` with the no-sound arm.
- `cantus-core/tests/undo_invariants.rs`: the `erng` stream, `marked` counts, its block, its assert,
  `round36_marks_command`.
- `cantus-project/src/format.rs`: `SCHEMA_VERSION = 50`, `migrate_49_to_50` (registered and the fn),
  the load repair of marks. `examples/write_fixture.rs`: the schema-50 block.
- `cantus-ui/src/lib.rs`: `pub mod editor;`
- `cantus-ui/src/app.rs`: the `editor` field; its `Default` in `assemble`; `crate::editor::frame`;
  `crate::editor::view::show`; the hook at the top of `run_action`; `crate::editor::close(self)` at the
  top of `reset_bus_to`.
- `cantus-ui/src/views/arrangement_audio_menu.rs`: separator, *Edit in audio editor…*, *Remove unused
  audio…* (items 19–21) and the `20 =>`, `21 =>` arms.
- `cantus-ui/src/ai_tools.rs`: `#[path] pub mod audio_editor;` and `audio_editor::TOOLS`.
- `cantus-ui/tests/golden/mcp-tools-list.json`: regenerate after merging the other builders' tools.
- `docs/manual/{en,es}/README.md`: one line each; `docs/BACKLOG-excellence.md` §3 *Editing*: the
  editor done, recording into it open.

## Owner-only

- Listening: noise reduction and declick on real recordings against iZotope RX / Audition / ReaFIR;
  the defaults (24 dB, strength 3, smoothing 50 %; declick 50 %); the 3 ms join on real material.
- Eyes on the window at 75–200 % in the theme family.
