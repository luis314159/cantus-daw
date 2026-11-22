# ADR 0143 — Comparatio, the reference comparator

**Status:** proposed (fx side round wave C; wave B
holds 0139–0141 in parallel) · **Date:** 2026-11-17 · **Schema:** 76 · **Relates to:** ADR 0005 (the audio
thread), ADR 0022 (memory built off the audio thread), ADR 0052 (the master's loudness
meter, whose code Comparatio measures with), ADR 0112 (Aula's response by path), ADR 0134
(Arca's cabinet by path)

## Context

Mastering engineers compare a mix with a finished record at the same loudness, by ear and
by measure, and test their ears blind (ADPTR Metric AB, Mastering The Mix's REFERENCE,
Melda's MCompare — function only). The DSP was built first (`docs/round-fx/comparatio.md`:
Terminus's own loudness meter, EBU Tech 3341/3342 cases within 0.03 LU, the match within
0.06 LU, an eased equal-power switch at −94 dBFS, ABX and blind preference with an exact
binomial p).

## Decision

A new effect kind, `EffectKind::Reference`, model `cantus_core::fx::comparatio`, engine
unit `cantus_engine::fx::comparatio`, store `cantus_engine::fx::comparatio_store`, DSP
`cantus_dsp::comparatio`.

- **Parameters** (12 keys): `listen` (your mix, the reference, X hidden, the blind
  preference's One and Two), `match` (integrated, short term, by hand), `offset`, `refl`
  (the reference's loudness over what has played or its whole file), `fade`, `sync` (its
  own playhead, locked to the song with `soff`, a loop `lstart` … `lend`), `seed` and
  `trial` (the blind order), `res` (the display's). **The default is a null: the mix heard,
  to the bit** — and with no reference loaded, whatever is set.
- **The reference is a file by path** in `EffectSpec::ir`, set with `SetEffectIr` (the
  command Aula and Arca use; `takes_ir` is true for the kind), never copied into the
  project. It is read **off the audio thread**: decoded and resampled to the engine's rate
  with the audio clips' decoders and sinc (`samples::decode_audio`) on a thread of its own
  when live, at once when offline; split into two channels; its whole file analysed
  (`analyse_whole`: the integrated loudness for the whole-file match; the readings,
  histogram and tonal curve for the panel). Until it is in, the compiled snapshot names no
  reference (the mix plays); its arrival asks for a compile (`SampleCache::poll_loaded`).
  The unit asks for it through the memory path (`MemoryNeed::Comparatio`) and holds an
  `Arc` the store has a sibling of, so letting one go on the audio thread never frees.
  References longer than 20 minutes are cut there; at most three unheld are kept.
- **It is a monitoring unit, and a render never hears the reference.** Only the live
  engine's own answer (`EffectMemory::build_live`, called by `EngineHandle`'s poll) hands a
  unit its reference; every render, freeze, stem and range render answers its units' needs
  with `EffectMemory::build`, which gives none, and the compile for a render
  (`compile_as(render = true)`) also takes every reference away and sets `listen` to the
  mix. A test renders a Comparatio set to the reference, with its reference in the store,
  on a strip and on the master (blind X), and gets the dry project to the bit.
- **Its place**: an ordinary insert, offered on any strip and meant for the master's last
  slot (what the master sends out is what is compared). A fixed post-chain master slot (the
  DSP report's sketch) was not built: an insert keeps every existing path (the chain's
  order, bypass, the picker, the plug-in export) and the render guarantee holds wherever it
  sits. The master's meters read what is heard, the reference included — the point of
  monitoring.
- **A swap or a removal while the reference is heard** holds the DSP on the mix until it
  hears the mix (the crossfade), then swaps: no cut. A blind trial in progress is left by
  the same fade.
- **Transport**: the song's real samples (`project_samples`), so a locked reference follows
  the song in time through tempo changes.
- **Telemetry**: what is heard and the fade, the match gain and its target, the playhead and
  length, the whole-file loudness, both sources' 13 readings, both sources' stereo bands,
  and the two short-term histograms in ten pages. The tonal analyses are the interface's:
  the reference's from its whole file (the store), your mix's from the slot's signal
  (step 3: the slot's scope, as Speculum's).
- **No starting preset**: it starts at its defaults (it measures, `Neutral::Measures`).
- **Not a Tabula pedal**, ever: a reference is the master's monitoring, not a pedal's sound.

## Decision, phase B (step 3; the owner approved sheet v1 on 2026-11-17)

- **A list of references** (`EffectSpec::refs`, `fx::comparatio::References`; still schema
  76 — this branch's own migration, `migrate_75_to_76`'s note extended, `fixtures/v76.cantus`
  regenerated with a list of three): at most **eight**, each a path with **the size and a
  hash** of the file as it was loaded (`Reference`), plus the Aequalis its match EQ was last
  sent to. The one compared stays `EffectSpec::ir`, so the engine's path is unchanged.
  `Command::SetReferences` sets the list and the one compared **together**, as one step whose
  inverse is the pair it replaced; it refuses a path twice or empty, a ninth, one compared
  that the list lacks, and any other kind. A hand-edited list is repaired on load
  (`repair_project`). Its own undo-fuzz stream counts it applied and refused.
- **Each reference keeps its own analysis**: the store reads every reference in the list
  once (the one compared for the unit, the others for their summary) and keeps a
  `Summary` — length, whole-file loudness and readings, the tonal curves of the signal's
  middle in thirds and sixths, a 240-peak overview, the file's print — after its audio is let
  go (the audio of the ones not compared is swept beyond three, as before).
- **The print** (`comparatio_store::fingerprint`): the size and a 64-bit FNV-1a of the size,
  the first and last 64 KiB and 62 blocks of 4 KiB between — milliseconds whatever the
  file's length; a re-exported master changes every sample, so it changes the print. The
  editor compares it with the one kept and says **missing** or **changed since it was
  loaded**; loading the file again takes its new print. Chosen over a whole-file hash so that
  loading never waits on the disk.
- **Your mix's tonal curve is the slot's analyser tap** (the input's middle, the pair's first),
  measured on the editor's thread (`picture::Live`), in thirds and sixths; the reference's is
  the same middle of its whole file. Like meets like; nothing is added to the audio thread.
- **The match EQ**: `match_eq::suggest` on the difference (the *Match EQ* card's `eqamt` and
  `eqmax`, display keys like `res`: no sample changes) drawn over the difference before it
  is sent; **Send to Aequalis** fits it (`aequalis::fit`, at most six bands, minimum phase)
  and puts an Aequalis **just before the unit** — the mix is what is changed, and it is then
  compared — with its output at the suggestion's makeup (the mix's loudness kept): a `Batch`
  of `InsertEffect` and `SetReferences` (its id kept), one step; a second send **replaces
  that Aequalis's spec in place** (`ReplaceEffect`, same id), one step. A user's own EQ is
  never touched. Only Cantus can send (a plug-in has no chain of ours).
- **Loading**: *Load reference…* (a native dialog) and a file dropped on the console add the
  files and compare the last; the numbered keys choose; *Remove* takes the one compared off.
  Each one step. A drag across the overview sets a loop (`sync` 2, `lstart`, `lend`), one
  step.
- **Learn**: what is heard (`listen`) offers automation and MIDI learn on the selector, the
  lamps and the A, B, X keys (a footswitch for A/B). The blind test's `seed` and `trial` are
  kept by its own keys and are **no control's** (learn_everywhere excuses them by name, as it
  does Loquax's object): a lane or a controller moving them would give the answer away. A new
  test's seed comes from the clock, so no order can be learnt even after a preset or a
  reload set it back to 0.
- **Presets** (fourteen) are ways of comparing; none names what is heard or the blind order,
  so loading one returns to your mix.
- **The plug-in** (`Cantus Comparatio`, CLAP and VST3): its state is the project's format —
  `refs` and `ir` in the `EffectSpec` JSON, **never audio**; the main thread keeps the list
  (`comparatio_post::RefPost`), asks the store to read the one compared on a thread of its
  own at the processor's rate, and posts the file to the processor as Fractus's lanes are
  posted; the plug-in's memory builder answers the unit's need by **waiting on its own
  thread** for the store (giving up when the read fails or the processor goes). A host's
  **offline render hears the mix**: CLAP's `render` extension (offered by Comparatio alone)
  and VST3's `processMode == kOffline` set the processor's unit to your mix.
- **Tabula**: still never a pedal — the render guarantee strips Comparatio *slots*; a pedal
  inside a board would sit outside it. `pedal_allowed` refuses it and a test holds it.

## Consequences

- The editor (design sheet v1, `compartir/comparatio/design/v1`): a two-deck console — your
  mix's deck and the reference's, the screen between (tonal balance and the difference,
  dynamics, stereo), the reference's strip, the big selector, the blind drawer.
- **The CLAP/VST3 plug-in (step 3)**: the reference's path in the plug-in's state (with the
  file's size and a hash; a missing file reported, never skipped silently; the audio never
  in the state), a *Load reference…* key in its editor (a native file dialog on the
  plug-in's UI thread) and a drop onto the editor; decoding and resampling on a plug-in
  background thread at the host's rate from `activate`; locked sync on the host's
  transport. The plug-in host answers the unit's need as the live engine does.
- Step 3 also: factory presets (ways of comparing), the reference list (several references
  in the project, one selected), the mix's tonal curve from the scope, the match-EQ
  suggestion sent to Aequalis, the AI's words, the manual, an e2e through
  `CantusApp::assemble` (a WAV the test writes loaded as the reference, heard on the master
  at the mix's integrated loudness within 0.1 LU, the mix back to the bit, an export
  without it).

- **The owner's answers (2026-11-17), built on the same branch:**
  - **Each reference keeps its own playback** (`Reference::playback`: `sync`, `soff`,
    `lstart`, `lend`). The unit's keys stay the live values (automation, the engine and the
    plug-in's parameters unchanged); comparing another reference saves the unit's playback
    into the outgoing entry and brings the newcomer's own back (its defaults the first
    time) — `fx::comparatio::switch_to` / `switch_command`: `SetReferences` and the
    `SetEffectParam`s in one `Batch`, one step, its inverse exact. The loudness match
    (`match`, `offset`, `refl`), the crossfade and the display stay the unit's: they are
    *how* you compare and should not change under you when you switch; *where* a reference
    plays (its chorus, its offset to your song) is the reference's own. A playback out of
    range is refused by `SetReferences` and repaired on load. Still schema 76; the plug-in's
    state carries it (the same JSON).
  - **A drop onto the plug-in's window**: egui-baseview 0.7.2 is carried patched in
    `third_party/egui-baseview` (`[patch.crates-io]`, as cpal and jack-sys are): its new
    `drop::input` passes baseview's drag-and-drop to egui and answers the system's drop
    target with `AcceptDrop`; the editor takes the drop as it does in Cantus.
  - **Only the one compared is in memory**: the store keeps no spare reference
    (`SPARE` = 0). The others keep their summaries; comparing one reads it again from disk on
    its thread **adopting its summary** (no second analysis while its print is unchanged).
    While it is read the snapshot names none, so **your mix is heard** and the strip says
    it is being read — never silence, never the previous reference passing for the new one;
    when it arrives it fades in from your mix like any switch. Eight references hold the
    audio of one (measured).

## What the tests hold

`cantus-core` `fx::comparatio` (keys, the null) and an undo-fuzz arm for its reference;
`cantus-engine` `tests/fx_comparatio.rs` (the null with and without a reference asked for;
heard at the mix's loudness and back to the bit; a swap fades through the mix; no render
holds the reference, strip and master; blocks 1/37/64/512/4096 and a fork to the bit; a WAV
read and resampled through the store, its whole-file loudness, a missing file reported;
live = offline), `fx_contract`, `rt_memory::comparatio_never_allocates` (heard, matched,
blind, locked, looped, swapped while heard, taken away); `cantus-project` schema 76's
fixture (a reference by path); `cantus-ui` `fxc_whole`.

**The owner's answers**: `fx::comparatio::tests::switching_keeps_each_reference_s_own_playback`,
the undo fuzz's switches (`switch_command` batches, applied and refused);
`fx_comparatio::eight_references_hold_the_audio_of_one` (768 000 bytes held for eight 2 s
references, one's; the fifth compared: read again, its summary the same `Arc`, the first let
go); `rt_memory::comparatio_never_allocates` (another reference being read, then arriving:
nothing allocated on the audio thread); `comparatio_drag::each_reference_keeps_its_own_loop`;
the plug-ins' `a_file_dropped_on_the_plug_in_s_window_is_compared`.

**Phase B**: `cantus-core` `fx::comparatio` (a list valid only whole, unique and within
eight; the one compared shown; a hand-edited list repaired; never a pedal) and
`undo_invariants` (Comparatio's references applied and refused, from a stream of their own);
`cantus-project` schema 76's fixture (a list of three with a print, the match EQ's amount);
`cantus-engine` `fx_comparatio::every_reference_in_the_list_keeps_its_own_analysis` (each
summary its own loudness, curves and overview; the print equal to the file's, changed by one
byte in the middle; a missing file named; a plug-in's request and wait); `cantus-plugins`
`tests/comparatio.rs` (the state by path, size and hash, never audio, round-tripped and
repaired; a reference loaded with a state heard matched, the mix untouched to the bit; an
offline render the mix to the bit; a missing file reported) and the golden ids; `cantus-ui`
`comparatio_drag` (the selector, lamps and keys; the numbered keys, Remove, Load reference…
and a drop; the overview's loop; Send to Aequalis inserted then updated in place — each one
undo step), `comparatio_e2e` (through `CantusApp::assemble`: a WAV loaded as the editor loads
it, no render holds it, live = offline for the mix, the reference heard within 0.1 LU of the
mix, back to the mix to the bit), `learn_everywhere`, `preset_audit`, `fxc_whole`.

## Amendment — round 41: the one compared is always listed (2026-11-22)

**Found:** the undo fuzz (seed 517 with Fides's key drawn in arm 17, r41/fuzz-two) failed
an undo: `SetEffectIr` — still the phase-A way to set the reference — set a Comparatio's
`ir` with no list holding it; a `SetReferences` after it returned the inverse
`refs: None, selected: Some(…)`, a pair its own check refuses, so the undo could not apply.
A phase-A project (an `ir` and no `refs`) loaded the same state: the repair on load skipped a
slot with no list at all, so the editor's next *Load reference…* left an undo that failed.

**Decision:** the model's invariant is phase B's own words — **the one compared is always one
its list holds** — and every path into the model keeps it:

- `SetEffectIr` on a Comparatio **adds the file to the list in the same step** (after the
  others; a ninth refused, as `SetReferences` refuses it; clearing it leaves the list), and
  its inverse is a `SetReferences` of the pair it replaced. Refusing the command on a
  Comparatio was the other choice; keeping it working keeps phase A's documented way (and any
  script that took it) valid, and no editor, agent tool or plug-in path uses it (the editor
  and Send to Aequalis use `SetReferences`, the agents' `set_impulse_response` takes Aula
  alone, the plug-in keeps its state and repairs it with `repair_project`).
- `repair_project` (on load, and the plug-in's state) **lists a file compared with no list**,
  takes off an empty path compared, and writes a list left empty as `None`.
- A slot brought in whole (`InsertEffect`, `ReplaceEffect`, a pasted strip) is not checked;
  if one ever carries a pair no command sets, the inverse of the next `SetReferences` or
  `SetEffectIr` on it is a `ReplaceEffect` of the slot as it was
  (`fx::comparatio::settable` says which), so the undo is exact either way.

No schema change: the saved model is the same; a phase-A file is listed on load.

**Tests:** `undo_invariants::a_reference_set_by_file_is_listed_and_every_step_undoes_exactly`
(the reduced sequence — failed before, at the `SetReferences` undo — plus a second file, a
listed one, clearing, an empty path and a ninth refused, every step undone and redone to
the bit), `a_list_less_reference_brought_in_whole_still_undoes_exactly` (three hand-made
pairs, each by `SetReferences` and by `SetEffectIr`), `fx::comparatio::tests::
a_file_compared_without_a_list_is_listed_on_load`; the main fuzz 1..=600 with its own draws
and with Fides's key in arm 17, and the takes-through-tempo fuzz 1..=200.
