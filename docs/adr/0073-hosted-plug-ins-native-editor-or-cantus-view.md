# ADR 0073 — A hosted plug-in in its own editor or in Cantus's view, chosen per plug-in

**Status:** accepted (round 31, after its merge) · **Date:** 2026-10-27 · **Extends:** ADR 0019
(hosting standard plug-ins), ADR 0024 (the hand-drawn kit), ADR 0028 (hosted instruments),
ADR 0057 (modulators and automation on any parameter), ADR 0062 (MIDI learn), ADR 0063
(plug-ins out of process) · **BACKLOG:** §6h point 2

## Context

The owner (2026-10-26): *"los plugins externos al cargar que carguen su preview nativo pero
que se pueda cambiar al preview de cantus y que se pueda guardar como default para cada
plugin."* Bitwig's and Ableton's device view of a plug-in's parameters and REAPER's generic
UI are the reference.

What there was: a hosted **effect** opened Cantus's generic editor (its parameters grouped as
the plug-in groups them, the house knob, A/B, presets, sidechain) when its editor was asked
for, with an *Open plug-in window* key for its own editor; inserted from the insert menu it
opened nothing. A hosted **instrument** opened only its own window; its parameters were rows
in the inspector. The generic editor drew every row of a card that was partly in view (a
500-parameter plug-in in one group drew 500 cells a frame), had no meters, a filter by name
only, and a window height that ignored the *Runs* card (its cells' names were cut).

## Decision

1. **Two views per instance** (`cantus_ui::hosted_view::HostedView`): **Native**, the
   plug-in's own editor in its own window (in its helper when it runs apart, ADR 0063), and
   **Cantus**, its parameters drawn with the kit in the device frame. The view an instance
   shows is, in order: the one chosen for it this session; else its plug-in's remembered
   default; else **Native** (`hosted_view::resolve`). A plug-in **without an editor of its
   own** has only Cantus's view: no switch, no default to remember (`hosted_has_editor`,
   from the loaded plug-in, else from its scan).
2. **It opens in its view when inserted or loaded**: the insert menu's plug-ins, *Add
   effect* and the palette (`add_picked_effect`), the picker's instruments (`apply_pick`)
   — and every *open the editor* after (`open_effect_editor`, `open_inst_editor`,
   `open_instrument_window`, F8, the rack's button, a double-click) — go through
   `CantusApp::open_hosted`. A plug-in still **loading in its helper** with Native as its
   view opens Cantus's frame meanwhile — its loading card, and *Its own window opens here
   once it has loaded* where the switch goes — and its own window takes the frame's place
   once it is in (`pump_hosted_opens`, every frame from `CantusApp::frame`) — only while
   that frame is still the one open; one that **fails or stops while loading** leaves its
   frame as it is, showing why with *Reload*, and is never opened again over another
   editor; one whose frame the user closed or replaced with another editor is no longer
   waited for (`Views::forget_pending`, from `close_fx_editor`, `open_fx_editor`,
   `close_cantus_view`, the instrument frame's close), and one removed is forgotten. A
   helper that is **busy** answers later and opens the window itself (`HostError::Busy`);
   a second press on the window's key meanwhile is harmless (`PluginHost::close_window`
   and `close_instrument_window` mean "a window was open" and leave one on its way to
   come). Only **switching to Cantus's view** cancels a window on its way
   (`close_window_or_pending`, `close_instrument_window_or_pending` →
   `MainThread::cancel_pending_editor`, the bridge's only: it is closed as it comes, never
   adopted beside Cantus's view). Any
   other failure to open its window (no display, a refusal) **falls back to Cantus's
   view**, so the user always sees the plug-in, and the status line says why. A missing, unloaded or
   stopped insert opens Cantus's frame, which says so with *Reload*, as before; a missing or
   stopped instrument keeps its status-line words (ADR 0028).
3. **The switch** — *Plug-in window | Cantus view* — is in **Cantus's frame** (the effect's
   generic editor and the instrument's new Cantus view, beside *As default* and the
   search), in the **inspector** (a hosted instrument's *Editor* and *Remember* rows; the
   *Window* row opens whichever view is chosen) and in **every insert's menu** (the
   inspector's slot menu and the mixer's, under an *Editor* band: the two views as radio
   rows and *Use as default for this plug-in* as a check). The plug-in's own window is the
   plug-in's — a native window, or its helper's — so no switch is drawn inside it: it is
   reached from the inspector and the menus, as Bitwig and Ableton keep theirs in the
   device panel. Switching closes the other view's window and opens the chosen one.
4. **Switching is interface state, not a `Command`.** It changes which window shows, never
   the project: no undo step, no `cmd.*` label, no arm in the undo fuzz. The instance is
   not reloaded or moved, so its state, its values, its automation lanes, its modulators
   and its controller mappings are exactly as they were — in process and out of process.
   The session's view per instance — the one chosen, or **the one it opened in**, pinned so
   that a default set or forgotten later changes what opens next, never what shows — is
   kept in `views::plugins::State::views` under the insert or the track **and the identity
   of the plug-in it is for** (`CantusApp::hosted_identity`: the format, the id and which
   load of it — `load_id`: a number given when an instance is made and given anew where a
   new load happens, `reload`, `reload_instrument`, `move_plugin` and `move_instrument`;
   so a reload, a move to another process or another plug-in on the track is another
   load, while a **restart** — in place, or by a new instance where the backend has no
   restart of its own (VST3 on `kLatencyChanged`) — and a state loaded into it are not.
   No address is hashed: a restart replaces the instance's memory, never its load). A hosted
   instrument's Cantus view keeps its A/B, its frame (preset, lists) and its badge by that
   identity too, and the cards by subject and identity: a replace or a swap on the track
   starts from the new plug-in's own default, and no plug-in's state is ever applied to
   another. All of it is forgotten when another project replaces the open one
   (`reset_bus_to`).
5. **The default per plug-in is a user setting, never the project's**
   (`Settings::hosted_views: ViewDefaults`, `settings.json`): *Use as default for this
   plug-in* remembers the view showing; its inverse forgets it (back to the plug-in's own
   editor). The key is the plug-in's **stable id** — `clap:<plug-in id>` or
   `vst3:<CLASS ID>` (capitals: scanners write either case) — never its version, file or
   name, so the default survives a restart, a rescan, a moved file and an update. The words
   are kept as text (`"native"`, `"cantus"`): a word a newer build writes is kept in the
   file and not read here, and the settings file's lenient reader (round 23) keeps every
   other choice when this field is unreadable. No project schema change: the choice belongs
   to the person on this computer (one user wants Cantus's view of a synth, the next its
   own), as FL Studio keeps its wrapper's preferences.
6. **Cantus's view** (`views::plugin_view`, shared by effects and instruments through
   `Subject`):
   - **grouped** by the plug-in's modules or pages (CLAP `module`, VST3 units; a page path
     `A/B` reads `A › B`), each a card, in a **flush grid**: the cards of a flow row share
     the room left and take the row's height; the cells are centred in their card;
   - **searchable**: one field filters by **name and group**, every typed word must appear
     (case folded, ASCII without allocation); the count says *n of m parameters*; the
     window keeps its height while searching;
   - **meters where the plug-in reports them**: a parameter the plug-in marks **read-only**
     (CLAP `CLAP_PARAM_IS_READONLY`, VST3 `kIsReadOnly`) is drawn as a meter — the value
     in a well, a lit bar where the knob would be, its name — read every frame
     (`param_value`: a call in process; out of process, the helper reports changed values
     every tick). This is how both standards let a plug-in show a host a gain reduction or
     a level; **neither has a meter API beyond it**, so nothing else is invented. An
     in-process plug-in's own words label a meter; an isolated one's number does (asking
     its helper for words at every move would flood it);
   - **every control with the parameter menu of our own units**: the same
     `controls::Cell` and `mod_offer` — *Modulate…*, MIDI learn, *Create automation clip*,
     the automation and modulation marks and rings (ADR 0057, ADR 0062) — on
     `AutoTarget::Plugin` for an insert and `AutoTarget::PluginInstrument` for an
     instrument; a gesture is one undo step;
   - **label/value units**: the plug-in's words in the pill, its name below in capitals;
     each group's cells are as wide as its longest name needs (84 to 132 pt) — cut, with
     the whole name as the tip, only past that; a value the plug-in has no words for reads
     in the digits it needs (`0.42`, `12.5`, `2400`);
   - **only the rows in view are drawn**: the cards out of view are skipped and, inside a
     card, only the rows that cross the scroll area's clip are laid out, painted and
     sensed.
7. **A hosted instrument's Cantus view** (`views::plugin_view_inst`) is a device window of
   the workspace like an effect's editor, in the same frame: the plug-in's name and the
   track, its saved states as presets (one folder per plug-in id, shared with its inserts),
   A/B of whole states (`SetInstrumentPluginState`, one step each), the swap (the picker
   for the track's instrument), minimise, maximise and close; its body the view's card and
   the parameters, or the stopped, loading, not-loaded or missing card with *Reload*.
   `FxHost` gained `daw_inst` and `swap_tip` for it. The count (*n parameters*) is the
   plug-in's showing — counted from its own list when its cards are not laid out (stopped,
   loading, missing).
8. **The device frame's header gives the name room first** (every unit's frame, review):
   the frame measures the name and its context at the theme's text size, as it paints
   them (`frame::header_name_w`, kept with the header's words and the text size, measured
   again when either changes) and passes it as `Header::name_w`; the preset well narrows (to its least,
   120 pt) before the name is cut, and the swap cap gives its word up when the name and
   context would still be cut. Units whose name fits are laid out as before.

## Consequences

- New: `hosted_view` (the choice, the defaults, `open_hosted`, the menus' rows),
  `views::plugin_view` and `views::plugin_view_inst`, `Settings::hosted_views`, the
  `hostedview.*` strings (`locales/areas/hostedview.{en,es}.txt`), `controls::Cell::width`,
  `FxHost::{daw_inst, swap_tip}`, `PluginHost::set_params_for_tests`, and test plug-ins
  without an editor (a copy of `cantus-test-clap` or `cantus-test-clap-synth` whose file
  name contains `nogui`).
- Also new (reviews): `Hosted::load_id`, `HostedInst::load_id`,
  `MainThread::cancel_pending_editor`, `PluginHost::{close_window_or_pending,
  close_instrument_window_or_pending}`, `Views::forget_pending`, `CantusApp::{hosted_identity, choose_hosted_view}`,
  `views::plugin_view_inst::{toggle_ab, on_b}`, `Header::name_w`.
- Changed: a hosted effect inserted from the insert menu now opens (its own window, by
  default); a hosted effect's *Open the editor* opens its own window unless Cantus's view is
  chosen or its default; the generic editor's *Open plug-in window* key became the switch.
  `open_hosted_window` and `toggle_plugin_window` keep their behaviour; their bodies are
  `open_hosted_window_now` and `open_plugin_window_now`, which also say what failed.
- The view is not in the project: the same project opens a synth in its own window on one
  computer and in Cantus's view on another. A per-instance choice that travels with the
  project was considered and refused: the owner asked for a default per plug-in, and a
  choice in the project would override the person's own on every machine it is opened on.

## What the tests hold

- `hosted_view` (unit): the key is the format and id, never the version, VST3 ids in either
  case one key; the order chosen → remembered → its own editor, and Cantus's view the only
  one without an editor; defaults set, forgotten, a newer build's word kept unread.
- `tests/hosted_view.rs` (in process): an insert opens its own editor (a real window on
  Windows; Cantus's view where none can open), the default once remembered opens Cantus's,
  its inverse its own again, saved at once; switching four times keeps the project equal,
  the bus revision, the history and the plug-in's state, its automation clip there; the
  default kept through a settings save and load, a rescan finding version 2 in another
  file, an old settings file and an unreadable field; a plug-in with no editor (`nogui`)
  opens Cantus's view, offers no switch, never its window; the frame shows the switch and
  *As default*, the menus' rows and their acts; the search by group, by name, by several
  words, through the editor with its count and its *no match*; a hosted instrument's
  Cantus view with its two meters (the synth's read-only *Tempo seen* and *Playing seen*)
  and its names whole, its inspector rows, switching keeping its state; an instrument with
  no editor; another project forgetting the session's choices. Review: an instrument's A/B
  and chosen view belong to its plug-in, not its track (another load on the track neither
  sees B nor gets the old state applied); a default set later never changes what shows; a
  loading plug-in shows its card, then its window, or why it failed; the count of a stopped
  plug-in is its own; an instrument's knob is one undo step; the rescan is a real one (the
  database forgotten, version 2 found in another file and inserted, opening in Cantus's
  view by the default).
- `tests/hosted_view_isolated.rs` (out of process, this build's `cantus-sandbox`): an
  isolated insert whose helper loads it slowly (the test gain's 2.5 s first activation)
  shows its loading card while it waits and its own window once loaded, and switching keeps the
  project and the state its helper reports; an isolated instrument whose default is
  Cantus's view opens there at once, switches, and shows its meters from the helper.
- `tests/hosted_view_isolated_busy.rs`: a window asked for while its helper is busy (a
  3 s state load) and then switched away from never appears beside Cantus's view; a second
  press on its key meanwhile leaves it to come (second review).
- `cantus-host` `tests/load_identity.rs` (second review): the VST3 gain's latency change
  restarts it by a new instance and keeps its `load_id`; a CLAP restart in place keeps it;
  a reload changes it.
- `tests/hosted_view.rs` (second review): a plug-in waiting for its helper that fails while
  the user has another editor in its frame's place never replaces it; one whose loading
  frame was closed never pops its window up.
- `tests/header_text_size.rs` (second review): at text size 1.4 the header's name and
  context read whole, measured again after the text size changed.
- `kit::shell` (unit): a long name and context (250 pt) read whole at 820, 1000 and 1640 pt
  (at 200 %), the well never below its least and never touching the name.
- `tests/hosted_view_boot.rs`: the boot path (`Files::read`, `CantusApp::assemble`) reads
  a plug-in's default from `settings.json`, and an insert after start opens in it.
- `tests/frame_budget.rs`: a 500-parameter plug-in (25 groups, a meter in each) scrolled
  from its first group to its last draws at most 96 cells a frame (48 measured), its meters
  among them, at most 200 allocations a scrolled frame (161 measured); one page of all 500
  (one card taller than the window, so only the culling of its rows keeps the bound) draws
  at most 72 cells a frame (64 measured); the release timing
  (`--ignored`) asserts build + tessellate p50 within twice the 4 ms of the mixer's desk.
