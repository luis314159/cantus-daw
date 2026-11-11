# Round 39 integration: what is left (integ-fix's handoff, 2026-11-09 night)

`r39/integration` is at `432a687e`. Merged so far: ux-a, ux-b, memory, kit, conv (Aula),
fixes, el-topbar, el-controls, el-mixer, el-cables (`2fa98e01`). Every cargo command goes
through `bash /f/projects/daw-r39/slot.sh cargo …` with `CARGO_PROFILE_DEV_DEBUG=0
CARGO_PROFILE_TEST_DEBUG=0`, `TMP`/`TEMP` = `F:\projects\daw-r39\tmp`. `cargo fmt -p <crate>`
one crate at a time (several `-p` hit os error 206). Stage files by explicit path.

## Done by integ-fix (in the commits since `1b80041b`)

- The 15 cantus-ui failures of the first merge, each at its cause (see the `integ-fix:` commits).
- An engine event is 64 B again: Aula's request waits in a per-unit `NeedMailbox`
  (`cantus_engine::need_mailbox`, ADR 0112 item 4).
- One dialog frame: `kit::dialog` is the face, head, scrim and button of every dialog in a
  recessed family; el-topbar's Settings/first run/export keep their layout on it.
- The help line in the status line registers as `hint_panel`, keeps the capability icons,
  and sets long hints whole on two small lines (ADR 0109).
- The VCA ghost at el-mixer's marker: `kit::automate::paint_vca_ghost`, used by the kit
  fader and the desk.
- Settings in front of the start page; the cable-style row whole.
- BACKLOG §8: views_nav's scroll-bar zoom test (failed once, passed 124 runs).

## Done by integ-2 (2026-11-09 night, after integ-fix)

- 1. Group separator on des-mixer's desk (`3d44581d`): chevron at the number's place, name up
  the column, count in the output row; `groups_gather_fold_and_dissolve` passes.
- 2. el-views merged (`3ea90170`); 3. el-arrange (`c204f667`) and icons (`b0907889`) merged;
  set 6 plugged into the picker (`tools/icons/track_glyphs.js` → `assets/icons/tracks.txt`,
  ids in kebab case, search EN/ES/aliases, recent row in Settings, ADR 0113 §4).
- 4. logo merged; #03 default + exe icon (integ-fix's scripts run) + window icon in its own
  colours (`window_icon::render`), ADR 0111 §9.

- 5. stack (`19f4f9df`) and lapis (`e7bc3657`) merged; lapis's loop ends and playhead cap
  moved into el-arrange's ruler. The 10 desk/inspector tests that failed since el-mixer fixed
  (`b421fd22`: the send level learns and modulates again; the rest reach controls where
  des-mixer put them); MCP golden (set_track's icon) and the click paths' add key (`0f3c065e`).
- 6. Windows: workspace without cantus-ui 4 130 passed / 0 failed; cantus-ui 2 583 passed and
  the 12 failures above, all fixed and rerun green; clippy -D warnings clean. Linux (WSL,
  `~/ci-r38-integ`, at `42f6a2ed`): 6 671 passed, the same 12 failed; clippy clean at `41be6d9b`; rerun at `42ebe18e`: 6 686 passed, 0 failed.
- 7. Release exe up 30 s on an empty song (145 MB private) and on the demo (183 MB).
- 8. Shots: `docs/round39/integration/shots/` (`examples/shoot_integration.rs`), Slate Night
  and Slate Lapis Night, 1920 × 1080 at 125 %.

Left:
track icons are not yet on the mixer's strip heads (the icons sheet shows them there);
STATUS.md and the round's report.

## Still to do, in this order (integ-fix's list; 1–8 done above)

1. **`app_flows::groups_gather_fold_and_dissolve` fails.** The test says the separator
   shows the group's name, but the name is not among the desk's painted texts after the
   el-mixer merge. `views/mixer_parts.rs::separator` paints it rotated
   (`Policy::Rotate90`) between `Row::Name`/`Row::Meter` and `Row::Fader`. Find out whether
   des-mixer's rows leave it no room (the name is then missing in the app too, so fix it),
   or whether the rotated text paints as a mesh `painted_text` cannot read (then read it
   another way). Strips 2–4 also still paint when the group is folded; check that as well.
2. **Merge `r39/el-views` (`f6a75691`).** `crates/cantus-ui/src/kit/mod.rs` conflicts with
   el-cables' `pub mod` lines; both sides add modules, so keep both.
3. **Merge `r39/el-arrange` (`32027e86`) and `r39/icons` (`b06bde78`).** Plug the set-6
   track glyphs (316 icons, 17 families) into el-arrange's per-track icon picker, searchable
   by English and Spanish names and aliases, as `docs/round39/design/icons/set6-additions.md`
   specifies.
4. **Logo #03 as the default logo and app icon.** Merge `r39/logo` (`b2cfda00`, design
   files only). #03's small masters are already copied, untracked, to
   `crates/cantus-ui/assets/logos/small/03-{16,20,24,30,32,48}.svg`; they are
   byte-identical to `docs/round39/design/logo/m03/`. Ready-made edit scripts, each run
   once with `python <script>`:
   - `F:\projects\daw-r39\tmp\integfix-logo\logo.py`
     changes `kit/logo_small.rs` (`M03`, `small_master_03`, `small_for`) and `kit/logo.rs`
     (`Logo::DEFAULT = Logo(3)`, `svg_for` draws #2 and #3 from their small masters below
     48 px, plus a test).
   - `F:\projects\daw-r39\tmp\integfix-logo\icon.py` changes `crates/cantus-app/icon.rs` and `build.rs`: the `.ico`,
     the Linux icon sizes, the macOS `.icns` and the splash are rendered from #03, with the
     small masters below 48 px. The running window's own icon on Windows stays the mark in
     the user's colour (ADR 0046); ask the owner whether that should become #03 too.
   - Both scripts import `sub.py`, which is in the same folder. If the folder is gone, use
     the descriptions above and make the edits by hand.
   Then record the logo change in ADR 0111 (logo only, no accent change). Run the kit and
   logo tests, `slate_e2e`, and `cargo build -p cantus-app` (its build script renders the
   icons).
5. **Merge `r39/stack`** (the release build's stack overflow at start, fixed by agent
   `stack`) and **`r39/lapis`** (the Slate Lapis theme, built by its own agent) once each
   reports.
6. **Full suite and clippy.** Run `cargo test -p cantus-ui --no-fail-fast`, the workspace
   without cantus-ui, and `RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets`.
   Two failures are known to be flaky: `cantus-sandbox::session_faults9` (it passed two
   reruns; already in BACKLOG from beta.12) and `views_nav`'s scroll-bar zoom test (BACKLOG
   §8).
7. **Start the release exe once and keep it running for 30 s.** The suite does not catch
   the stack overflow.
8. **Shoot whole windows** (mixer, arrangement, piano roll, browser) in Slate and Slate
   Lapis at 1920×1080 into `docs/round39/integration/shots/`.
9. **Not done by anyone yet:** the Linux check in WSL on the merged tree
   (`tools/wsl/check.sh`), STATUS.md, and the round's integration report.
