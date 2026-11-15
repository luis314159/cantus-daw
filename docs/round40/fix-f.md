# Round 40 — fix-f: the review of 40a27bb7 (Tympanum, Chorda, user packs)

Branch `r40/fix-f`, based on the integrated commit 40a27bb7. Nine findings, each verified
first, a failing test written, then fixed. Shots in `F:\projects\daw-r40\compartir\p3\fix-f\`;
after the follow-ups in `…\fix-f2\` (with `tympanum-install-*`: the kits list while a kit
installs from a file).

## 1. Changing Tympanum's kit never used `change_kit`

**Cause.** The kits list's modelled row, recorded row and *Install from a file…* set the
`kit` text alone (`set_inst_text`). The routes, `o{n}.` levels and power, bleed cells and
mic moves stayed keyed to the old kit's outputs: after Studio (12) → *Each mic* → Gogodze
(7), strips kept old names with other mics on them, five strips carried nothing, a stale
`o3.on = 0` unloaded a new mic, sub-groups were wrong, and the leftovers could not be
removed ("in use"). `cmd::change_kit` existed, but `definition()` returned `None` for a pack
not read yet (kits load lazily), which would have dropped every route.

**Fix.** `views/inst/tympanum_kit_change.rs`: every kit change goes through
`kit_change::change_kit` (one undo step, `cmd.tympanum_kit`). It reads both kits'
definitions first (`cantus_engine::inst::tympanum::cache::recorded`, read-only use), then
plans step by step on a copy of the song: the kit's sub-groups taken apart; the core
`change_kit`; a strip made for one mic and named after it renamed to the new mic; strips
left with nothing removed (`padout::orphans`, new ones only); old groups removed; a kit that
was *Each mic to its own strip* gives the new kit's other mics strips too; sub-groups made
again by family. Core `change_kit` now maps names first for every output, then families
(a family match never takes the output another's own name asks for); an old kit not known
carries by the song's slots (name, family); a new kit that cannot be read changes the name
alone, keeping the mixer; a carried slot without its mic no longer aborts the command.

**Tests.** `tympanum_kit_change_e2e::a_kit_change_carries_a_split_mixer_both_ways_in_one_step`
(assemble; Studio split → Gogodze from its published program files → Studio; strips named
after their mics, no orphans, one family per group, snare level carried, no stale power key;
undo/redo each way; save and reload). Core unit test
`an_unknown_old_kit_carries_by_its_slots_and_an_unknown_new_kit_keeps_everything`; the undo
fuzz's kit change also draws an unreadable kit.

## 2. *Each mic to its own strip* failed when old sub-groups existed

**Cause.** `route_all` removed the old sub-groups and then inserted new ones at indices
counted from the pre-removal `strip_groups.len()`; `InsertStripGroup` refused `index > len`
and the batch rolled back.

**Fix.** `route_all` plans on a copy step by step (`kit_change::Steps`), so the new groups
are inserted where the removals leave the list; `regroup` groups every strip made for the
kit's outputs (one family each), not only the strips just made; `kit_groups` also counts a
group whose other members are strips left empty by mics sent to Main. The core plan reuses
an empty strip made for an output and named after it (no second "Tom 1" beside the first).

**Test.** `each_mic_again_after_two_went_back_to_main_is_one_step` (twice, then a kit change).

## 3. A preset, A/B or Mix on a recorded Chorda switched it back to the model

**Cause.** `apply_inst_spec` clears every key the preset lacks; Chorda's presets name no
`source`, `rec.*`, `o{n}.*` or room look. Same for Tympanum: a preset without `kit` dropped
the kit, the mics' mix, the moves and the bleed.

**Fix.** `preset_target` → `keep_source_through_preset`: a preset naming no source (or the
one playing) keeps Chorda's `source`, `rec.*`, `o{n}.*`, the room's look always and the
piano's look while a recorded piano plays; Tympanum's `kit`, `o{n}.*`, `m{n}.*`,
`b{p}.{m}.*`. A preset naming another piano or kit goes through the switch first
(`preset_source_switch`: `recorded_switch` / `kit_change::plan`), in the same undo step.

**Tests.** `chorda_source_e2e::a_preset_and_ab_keep_a_recorded_piano_its_outputs_and_its_look`
(a piano installed from a file, Jazz applied, A/B both ways), and
`tympanum_kit_change_e2e::a_preset_keeps_the_kit_and_its_mixer_or_changes_kit_as_the_list_does`.

## 4. A user's DrumGizmo kit installed but never played

**Cause.** `Format::of` accepted the `.xml`, the receipt named it, but `read_pack_kit`
called a stub reader. The engine plays whole files (a take's `channel` is not read), so a
direct reader of DrumGizmo's multichannel hits could not play them right without engine
work (fix-e's files).

**Fix.** `cantus_content::user_pack::dg`: at install, a user's DrumGizmo kit (loose, or the
one in an archive — its *basic* setup if it ships several) is built with the very build our
DrumGizmo packs take (`kitbuild::mics::render_mics`): its MIDI map (the one sharing the kit
file's suffix) or General MIDI by name, the hi-hat's states by openness on CC4, its groups
as chokes, every channel a mic (`…L`/`…R` pairs stereo). The receipt names `Tympanum.sfz`.
A receipt naming the `.xml` (an earlier build of this round) is asked to be installed again.

**Tests.** `user_pack::a_users_drumgizmo_kit_is_built_for_tympanum_every_mic_its_own_files`
(DRSKit's published files, hits written for them) and
`tympanum_e2e::a_users_drumgizmo_kit_installs_from_a_file_and_plays_on_its_mics`
(assemble, *Install from a file…*, all eleven mics sound).

## 5. Install from a file of an archive failed for good after one failure

**Cause.** The archive was unpacked straight into `user-<name>-<hash>`; a failure left it,
the retry's `create_new` met the old files ("two entries land on the same file"), a folder
without a receipt was invisible and could not be removed, and the budget was `u64::MAX / 4`.

**Fix.** `install_user_with`: a fresh staging folder (`.user-staging-<id>-<pid>`), renamed
into place once whole (an existing pack replaced), removed on any error; the archive's
declared size (zip entries; a tar.gz's gzip trailer, at least its own size) checked against
the free space first (`NoSpace`), the unpack's budget the free space; `clean_orphans` at
start-up (`CantusApp::assemble`) and before each install removes staging folders and user
packs with no receipt. Tympanum's install error line now says why.

**Tests.** `user_pack::a_failed_unpack_leaves_nothing_and_a_retry_installs`,
`an_archive_bigger_than_the_free_space_is_refused_before_it_unpacks`.

## 6. Scene caches never freed

**Cause.** `inst.tympanum_scenes` and `chorda.scenes` never pruned; every `Scene` started
its own OS thread and kept a full-size texture.

**Fix.** `InstEditors::prune_scenes` each frame: scenes of tracks gone or no longer hosting
the instrument drop; another song (the app's epoch) drops them all. `scene::live`: one
shared worker thread for every scene (newest job per scene wins; a dropped scene's queued
jobs are skipped); `scene::workers()` counts.

**Test.** `scenes_of_deleted_tracks_and_old_songs_are_freed`.

## 7. *Pieces to strips* named strips in English in Spanish

**Fix.** `route_all` maps the core's English family name to `t(family.label_key())`.
**Test.** `pieces_to_strips_names_the_strips_in_spanish`.

## 8. MuldjordKit's credit

**Fix.** Its attribution note is now its page's own request — album credits: "Drum samples
provided by DrumGizmo.org" (read from drumgizmo.org's kit page, 2026-11-13); the pack's
page shows it under "In the author's words". **Test.** `content::muldjordkit_asks_for_its_own_album_credit`.

## 9. Minor UI

- The Out lists (Tympanum's and Chorda's, now one `inst_mixer::out_targets`) leave VCAs out.
- `inst_out::set_route` keeps the output's name and family from the definition, so a song
  whose kit goes missing still names its routed rows (Tympanum's list did not).
- The Ambience strip's Out cell says where the room mics go (*Main*, own strip, a chosen
  strip, *Several*).
- `set_image_on`'s error line is translated (`refusal_words`), not `{e:?}`.

**Tests.** `the_out_list_offers_no_vca`; core
`a_route_from_main_keeps_the_outputs_name_and_a_vca_is_refused`.

## Follow-ups (the coordinator, before the merge)

### A. Install from a file off the window's thread, with progress and cancel

`cantus_content::Manager::install_user` runs `user_pack::install_user_job` on a worker, as
a pack's download runs: its phases (`Unpacking`, then a DrumGizmo kit's `Building`, hits
written of all) in `Manager::status` under the user pack's id, `Manager::cancel` stops it
(the archive unpack and the build both watch the flag; the staging folder goes, nothing is
left), `poll` reports it finished (`InstallError::User` carries a user install's own
reason). `install_any` (Chorda's and Tympanum's shared path) now starts the job and records
what it is for (`Samples::user_installs`: the track, the instrument); the track keeps
playing what it plays until `CantusApp::user_installs_done` (from `poll_samples`, every
frame) gives it the kit (`kit_change::change_kit`) or the piano (`chorda_switch`), or says
why not. While it runs, the *Install from a file…* box of Tympanum's kits list and of
Chorda's source list becomes a progress box — name, bar, *Unpacking · 45 % · 120 MB of
260 MB* or *Building its mics · 30 % · 87 of 291* — and a click on it cancels.
`clean_orphans` now runs when the manager is made (and at start-up), never during an
install (it would have taken another install's staging folder).

**Tests.** `tympanum_kit_change_e2e::install_from_a_file_runs_in_the_background_and_a_cancel_leaves_nothing`
(assemble; a DRSKit kit installs while frames keep coming — the studio kit plays until it
is in, no frame waits on it — then plays its 11 mics; a second one cancelled at once leaves
no folder and changes nothing); `user_pack::a_drumgizmo_archive_keeps_only_what_plays_and_a_cancel_leaves_nothing`;
the existing Chorda/Tympanum install e2e tests now wait for the job (`wait_user_installs`).

### B. No doubled disk

After a DrumGizmo kit from an archive is built into per-mic files, the unpacked source
goes: an archive holding only the kit keeps the receipt, `Tympanum.sfz`, its main-mics
sidecar and `Mics/` alone; one holding other instruments too loses only the kit's own files
(its kit file, instrument files, audio and MIDI maps — `user_pack::dg::remove_source`).
Building again means installing the user's archive again. A loose `drumkit.xml` is read
where it is (the user's folder is never touched). **Test:** the content test above lists
the pack folder.

### C. A modelled kit's mic names in the interface's language

The modelled kit's mics are Cantus's: `tympanum.mic_name.<name>` in both locales (Spanish:
*Bombo fuera, Bombo sub, Caja arriba, Charles, Tom 1, Tom 2, Base 1, Base 2, Ride, Aéreos,
Sala cerca, Sala mono* — the band and piece words). `def::mic_name`/`mic_said` say them in
the strips, the mic panel's header, the matrix's tips, the picture's tips and *Heard by*;
the core keeps the English names as the outputs' stable names (songs, carrying by name).
New strips are named through `inst_out::plan_with` (a translator for the core's names:
mics for *Each mic*, families for *Pieces to strips*), and a kit change renames a strip
that bore the old mic's name in either form. Recorded kits keep their pack's names.
**Test:** `a_modelled_kits_mics_are_named_in_spanish`.

## Not done / verified only in emulation

- The DrumGizmo fixture has no audio: the hits are written by the tests; a real DRSKit or
  MuldjordKit download installed from a file should be tried once (beta checklist), with
  how long its build takes.
