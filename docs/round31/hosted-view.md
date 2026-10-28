# Round 31 (after its merge) — hosted plug-ins: their own editor or Cantus's view

Branch `r31/hosted-view`, cut from master `b63a6759` (0.1.0-beta.6). BACKLOG §6h point 2, the
owner's words: *"los plugins externos al cargar que carguen su preview nativo pero que se
pueda cambiar al preview de cantus y que se pueda guardar como default para cada plugin."*
ADR 0073 holds the decisions.

## What was built

- **Its own editor when inserted or loaded.** Before: an effect inserted from the insert
  menu opened nothing, and *Open the editor* on a hosted effect opened Cantus's generic
  editor; an instrument opened only its own window. Now every way in — the insert menu,
  *Add effect*, the palette, the picker's instruments, *Open the editor*, a double-click,
  F8, the rack's button — goes through `CantusApp::open_hosted`, which opens the instance's
  view: the one chosen for it this session, else its plug-in's default, else **its own
  window**. A plug-in loading in its helper opens once it is in (`pump_hosted_opens`); a
  busy helper opens it itself when free; any other failure to open its window falls back
  to Cantus's view so the plug-in is always on screen.
- **The switch** *Plug-in window | Cantus view* at the top of Cantus's view (effects and
  instruments), the **Editor** band of every insert's menu (the inspector's slot menu and
  the mixer's) and a hosted instrument's **Editor** row in the inspector. The plug-in's own
  window is the plug-in's (a native window, or its helper's): no switch can be drawn in it,
  so it is reached from those places, as Bitwig and Ableton do.
- **The default per plug-in**: *As default* beside the switch (*Use as default for this
  plug-in* in the menus, *Remember* in the inspector) — lit when the view showing is the
  remembered one; a click remembers it, another forgets it. Kept in `settings.json`
  (`hosted_views`) by `clap:<id>` / `vst3:<CLASS ID>`, never the version or the file;
  Settings' plug-in manager lists them under **Editors**, by name, with *Forget*.
- **Cantus's view** (`views::plugin_view`): the plug-in's pages as cards in a flush grid,
  cells centred; a **search** by name and page, every word; the count *n of m*; **meters**
  for what the plug-in reports read-only (CLAP and VST3 have nothing else for it); every
  control the house cell with *Modulate…*, MIDI learn and automation; each page's cells as
  wide as its longest name needs (84–132 pt); a value with no words of the plug-in's in the
  digits it needs; **only the rows in view drawn**.
- **A hosted instrument's Cantus view** (`views::plugin_view_inst`): a device window like
  an effect's — name and track, saved states as presets, A/B of whole states, swap,
  minimise, maximise, close — with the stopped, loading and missing cards and *Reload*.
- Fixed on the way: the generic editor's window height ignored the *Runs* card (its cells'
  names were cut at the bottom); it keeps its height while a search narrows the cards.
- The manual's *Plug-ins from other makers* step 5, in English and Spanish.

## Decisions (ADR 0073)

- **Switching is interface state, not a `Command`**: it never touches the project, so no
  undo step, no `cmd.*` string, no undo-fuzz arm; the instance is neither reloaded nor
  moved, so its state, values, automation, modulators and mappings stay as they were.
- **The default is the person's, not the project's**: no schema bump. A per-instance view
  in the project was considered and refused — it would override each person's own default
  on every computer the project is opened on.
- The session's choice per instance is forgotten when another project replaces the open
  one (its ids may come again).

## Tests added

- `hosted_view` (unit, 3): the key; the order chosen → remembered → its own; the defaults'
  words, a newer build's kept unread.
- `tests/hosted_view.rs` (10): opens its own editor, then its remembered default, then its
  own again; switching four times keeps the project, the revision, the history, the state
  and the automation clip; the default through a save and load, a rescan finding version
  2 in another file, an old file and an unreadable field; no editor (`nogui`): Cantus's
  view only, no switch; the switch and *As default* in the frame, the menus' rows and acts;
  the search (unit and through the editor); the instrument's view with its two meters, its
  names whole, its inspector rows, its state kept; an instrument with no editor; another
  project forgetting the choices; the manager's *Editors*.
- `tests/hosted_view_isolated.rs` (2, this build's `cantus-sandbox`): an isolated insert
  opening once loaded and switching with its helper's state unchanged; an isolated
  instrument opening in its remembered Cantus view at once, switching, its meters from the
  helper.
- `tests/hosted_view_boot.rs` (1): `Files::read` + `CantusApp::assemble` read the default
  and an insert opens in it.
- `tests/frame_budget.rs` (1 + 1 ignored): 500 parameters scrolled from the first page to
  the last: at most 96 cells a frame (48 measured), meters drawn, at most 200 allocations
  a scrolled frame (161 measured); the release timing asserts p50 ≤ 8 ms.
- Changed: `hosted_plugins.rs` looks for the switch where it looked for *Open plug-in
  window*.
- The test plug-ins: a copy of `cantus-test-clap` or `cantus-test-clap-synth` whose file
  name contains `nogui` has no `gui` extension; `PluginHost::set_params_for_tests`.

## Shots (`cargo run -p cantus-ui --example shoot -- <dir> 1 hostedview`)

`docs/round31/hosted-view-{night,paper}.png` (the switch, *As default*, the pages, a test
plug-in described as a 38-parameter synth, its *Output* page with two meters),
`hosted-view-search-{night,paper}.png` ("env" typed: two pages, *9 of 38 parameters*),
`hosted-view-inst-{night,paper}.png` (the test synth's Cantus view beside the inspector's
*Editor*, *Remember* and *Window* rows), and `hosted-view-paper-es.png`,
`hosted-view-search-night-es.png` in Spanish.

## Results

- Windows, `cargo test -p cantus-ui -p cantus-host -p cantus-sandbox --no-fail-fast`:
  **2 093 passed**, 0 failed, 30 ignored (one new test, the manager's, was built mid-edit
  in that run and failed; run again on its own it passes with its file: 10/10).
- `cargo clippy --workspace --all-targets -- -D warnings`: clean on Windows; WSL Ubuntu
  24.04 (`tools/wsl/check.sh r31/hosted-view clippy`): clean, and the plug-ins without
  their editor too; `cargo fmt --all -- --check` clean (run on Linux: on Windows the whole
  workspace's file list is too long a command line, so the packages touched were formatted
  one by one).
- WSL Ubuntu 24.04, the whole workspace (`check.sh … test`): **4 706 passed**, 0 failed,
  57 ignored (4 689 before the branch).
- The 500-parameter scroll in this machine's dev build (optimised): build + tessellate
  p50 0.55 ms, at most 48 cells drawn, 161 allocations a frame (median). The release timing
  test (`a_500_parameter_plugin_scroll_timing`, `--ignored`) was not run here.

## The review (read-only reviewer on d87de6cf), fixed

Each with a test that fails before its fix (checked by turning the fix off), but #7's.

1. **An instrument's view state was the track's** — A/B, the chosen view, the frame's
   preset, the badge, the cards' cache: a replace or a swap on the track inherited them,
   and A/B could apply one plug-in's state to another. Now kept by
   `CantusApp::hosted_identity` (format, id, which load: `load_id`, a number per load in
   `cantus-host`), and the instrument view's state is cleared with the project
   (`reset_bus_to`). Test: `an_instruments_view_state_belongs_to_its_plugin_not_its_track`.
2. **A plug-in loading in its helper showed nothing, and a failed load was dropped**: now
   Cantus's frame shows its loading card (and *Its own window opens here once it has
   loaded*), the window takes its place once loaded, and a load that fails leaves the frame
   with why and *Reload*. Tests: `a_loading_plugin_shows_its_card_then_its_window_or_why_it_failed`
   (in process, the host's loading switch) and the isolated test, now always slow (the test
   gain's 2.5 s first activation).
3. **A window on its way survived a switch**: `PluginHost::close_window` and the instances'
   `close_window_now` now cancel an editor their helper has not answered yet
   (`MainThread::cancel_pending_editor`, the bridge's only). Test:
   `tests/hosted_view_isolated_busy.rs` (a 3 s state load keeps the helper busy).
4. **Setting or forgetting a default changed the view reported for an instance already
   showing**: the view it opened in is pinned. Test: `a_default_set_later_never_changes_what_shows`.
5. **The count of a stopped, loading or missing plug-in was the previous one's**: counted
   from its own list unless its cards are laid out. Test:
   `the_count_is_the_plugins_showing_even_when_it_stopped`.
6. **Tests that did not test what they said**: one page of 500 parameters (one card, so
   only the row culling keeps the bound: 64 cells measured, 72 allowed); the rescan made
   real (database forgotten, version 2 found in another file, inserted: Cantus's view by
   the default); an instrument's knob (`Subject::Instrument::set`, one undo step); the
   isolated pending path forced.
7. Nits: `Cell::label`'s doc line back on it; a preset name that cannot be a file is
   refused before the stale-state word (`preset_file_name`) — no test: in process the
   state is always current, so nothing differed to observe.
8. **The frame header cut the name** ("CANTUS TEST GAIN (I…)"): `Header::name_w` (measured
   by the frame) — the preset well narrows to its least before the name is cut, and the
   swap cap gives its word up when the name and context would still be cut. Every unit's
   frame; reshot: the hosted view in English and Spanish, Comprimo, Terminus, Spatium,
   Fervor (unchanged where the name fitted). Test:
   `kit::shell::the_preset_well_gives_way_before_a_long_name_is_cut`.

After the review: Windows `cargo test -p cantus-ui -p cantus-host -p cantus-sandbox
--no-fail-fast` **2 101 passed**, 0 failed, 30 ignored; clippy `--workspace --all-targets -D
warnings` clean; WSL Ubuntu 24.04 on 5e32dc07: clippy clean (and the plug-ins without their editor), `cargo fmt --all -- --check` clean, the whole workspace **4 714 passed**, 0 failed, 57 ignored.

## The second review (on d87de6cf..85449ea2), fixed on `r31/close`

Each with a test that fails before its fix (run before the fix: each failed as the
reviewer said).

1. **(High) `load_id` changed when a plug-in only restarted**: it hashed the instance's
   address, and a restart by a new instance (every VST3 on `kLatencyChanged`: the in-process
   one has no restart of its own, the bridged one restarts this way) replaced it — the
   view chosen, A/B and the frame's preset were dropped as another plug-in's. Now a number
   given when an instance is made and anew only by `reload`, `reload_instrument`,
   `move_plugin` and `move_instrument`; no address. Test: `cantus-host`
   `tests/load_identity.rs` (the VST3 gain's latency restart keeps it, a CLAP restart keeps
   it, a reload changes it).
2. **(Medium) A second press on the window's key cancelled the window on its way**:
   `close_window` / `close_instrument_window` mean "a window was open" again; only the
   switch to Cantus's view cancels one on its way (`close_window_or_pending`,
   `close_instrument_window_or_pending`). Test:
   `a_second_press_while_its_window_is_on_its_way_does_not_cancel_it`.
3. **(Medium) A waiting plug-in took back a frame the user had moved on from**: a failure
   leaves its frame as it is (never reopened over another editor); its own window replaces
   only its own frame still open; closing or replacing that frame forgets the wait. Test:
   `a_waiting_plugin_never_takes_back_a_frame_the_user_moved_on_from` (and the loading test
   now keeps its frame up through the failure, as the ADR says).
4. **(Low-medium) The header measured at text size 1.0**: now at the theme's text size,
   measured again when it changes. Test: `tests/header_text_size.rs` (a process of its own:
   it installs the kit's tokens).

Results after the second review (045f3ade): Windows `cargo test -p cantus-ui -p cantus-host -p cantus-sandbox --no-fail-fast` **2 106 passed**, 0 failed, 30 ignored; clippy `--workspace --all-targets -D warnings` clean on Windows and in WSL (and the plug-ins without their editor); `cargo fmt --all -- --check` clean (WSL); WSL Ubuntu 24.04, the whole workspace on `r31/close`: **4 718 passed**, 0 failed, 57 ignored.

## What is left

Nothing of point 2. For the owner's eyes: the view with a real large plug-in (Surge XT,
Vital, Diva-class page counts) — its pages and names are the plug-in's own.
