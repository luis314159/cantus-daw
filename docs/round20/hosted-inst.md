# Round 20 — hosted-inst: CLAP/VST3 instruments on tracks

Builder: hosted-inst · Branch `r20/hosted-inst` · Decision: **ADR 0028**
(`docs/adr/0028-hosted-instruments.md`) · Schema **19** on this branch (the integrator
renumbers one of automation's and this one to 20).

## What was built

A track can now play a CLAP or VST3 instrument exactly as it plays Unda or Calor — and
eight free, open-source instruments (Surge XT, Dexed, OB-Xf, Six Sines, Firefly Synth 2,
JC303, RipplerX, sfizz), in both formats where they ship both, were downloaded, scanned,
played and saved through Cantus to prove it (table below).

| Layer | What |
|---|---|
| Model (`cantus-core`) | `InstrumentSpec::External(ExternalPlugin)` — the insert's record reused; `Command::{SetInstrumentPluginParam, SetInstrumentPluginState}` with exact inverses, in the undo fuzz (arm 25, and `External` in arm 20); `ExternalPlugin::params` now reads its keys as text or numbers (a record inside a tagged enum is buffered by serde, and a buffered key is text) |
| Schema (`cantus-project`) | 19, a no-op migration; fixture `v19.cantus` with a hosted instrument installed nowhere; a test that it opens with its bytes and values (0.1 + 0.2) intact |
| Engine (`cantus-engine`) | `UnitKind { Native(kind), Hosted(key) }` on `NativeUnit`, `hosted_key` (FNV-1a of format + id, no allocation); five default `InstrumentUnit` hooks (`configure_hosted`, `set_position`, `param_events`, `clear_param_events`, `stop`); `EngineMessage::ReplaceInstrument`, `EngineEvent::InstParam`; `EngineHandle::{add_instrument, replace_instrument, knows_instrument, take_inst_params}`; **instrument latency in delay compensation** (a track leaves at its instrument's latency — natives too); `render_with_instruments`; stems now send native instrument units (they were silent in stems — found on the way) |
| Host (`cantus-host`) | `HostedInstrument` (the adapter, `inst_unit.rs`), `Silent` (stands in during a restart); `unit::ParamSync` (values, no-echo and gestures, extracted from `HostedUnit` so both share it); `NoteKind::Midi` in the contract — CLAP sends it as `CLAP_EVENT_MIDI` when the note port speaks MIDI (pitch bend as a channel tuning expression otherwise; notes as MIDI when the port speaks only MIDI), VST3 through the controller's `IMidiMapping` read at activation; the session by track (`session_inst.rs`, `session_inst_idle.rs`): `make_instrument`, sync, idle, restarts, gestures as commands, save, render, the plug-in's window; notices `InstrumentLoadFailed`, `InstrumentRestartFailed`, `InstrumentWindowClosed` |
| Test plug-in | `cantus-test-clap-synth` (`crates/cantus-test-plugins/clap-synth`): a sine per note, sample-accurate and block-size independent, gain, latency with restart, mod wheel, pitch bend, a transport readout, a performed gesture, a state with a tag, a minimal editor |
| App (`cantus-ui`) | `CantusApp::set_hosted_instrument(track, &PluginInfo) -> bool` (one command, one undo step), `hosted_instruments()`, `open_instrument_window` / `toggle_instrument_window`, `is_hosted_instrument`; `apply_pick` handles `PickChoice::Hosted` for `ReplaceInstrument` and `AddInstrument`; `open_inst_editor` on a hosted track opens the plug-in's own window; the inspector's hosted-instrument card (`views/inspector_hosted.rs`); host notices on the status line; export bounces hosted instruments; strings in `locales/areas/hosted.{en,es}.txt`; `examples/shoot_hosted.rs` |

## For device-ux and browser (the API)

- **`CantusApp::set_hosted_instrument(track: TrackId, info: &PluginInfo) -> bool`** — put a
  scanned instrument on a track. Loads it, replaces the instrument in one command (undo
  brings the old one back, with its sound — the host parks unloaded states), returns
  `false` and sets the status line when the plug-in will not load.
- `apply_pick` already routes `PickChoice::Hosted(info)` with `info.kind ==
  PluginKind::Instrument` there, for `ReplaceInstrument { track }` and `AddInstrument`
  (a new track). I added only those two arms at the top of the match; device-ux owns the
  rest of `apply_pick`. Hosted **effects** in the picker still go through the existing
  `CantusApp::add_plugin(strip, &info)`.
- **`CantusApp::hosted_instruments() -> Vec<PluginInfo>`** — every scanned instrument,
  CLAP first, by name: what the gallery lists under "plug-ins".
- **`CantusApp::open_inst_editor(track)`** now opens a hosted instrument's own window (the
  chrome's "open" button needs nothing else); `toggle_instrument_window(track)` and
  `app.plugins.instrument_window_open(track)` for a close button or a lit state.
- A missing plug-in: `app.plugins.instrument_missing(track) -> Option<&str>` (the reason);
  `app.plugins.hosted_instrument(track) -> Option<&HostedInst>` (name, params, latency,
  `has_editor`).

## Tests

Always run (repository test plug-ins):

- `cantus-host/tests/hosted_instruments.rs` (13): scan says instrument (0 in / 2 out); a
  chord on beat 2 starts on its sample with exact silence before it; blocks 7, 64, 256,
  1000, 4096 identical; offline render (fresh instances, blocks of 333) equals playing to
  the bit; a project value reaches it and undo takes it back; state through save → open
  in a fresh host, same samples; a missing instrument opens, plays exact silence, says so,
  keeps its record; latency 100 (through a restart) cancels an inverted twin exactly and
  the latent path starts 100 samples late; a gesture in its window is one undo step and
  does not echo; tempo and play state reach it; a stop releases every note to silence;
  switching to Firmus and undoing brings its sound back; its window opens and closes.
- `cantus-host/tests/vst3_backend.rs`: a VST3 instrument (the repository's Pulse) through
  the adapter — a note between calls lands on the next call's first sample, release
  velocity, voices, a stop.
- `cantus-ui/tests/hosted_instruments.rs` (3): the picker's `Hosted` choice is one undo
  step, the inspector card shows name, format and *Loaded*, undo unloads, `AddInstrument`
  adds a track; opening the editor from the track opens the plug-in window, not the
  native frame; a missing instrument shows *Missing: silent*, cannot open, and saves back
  untouched.
- `cantus-core`: undo fuzz arms; `a_hosted_instrument_round_trips_through_json`.
- `cantus-project`: `a_missing_hosted_instrument_opens_with_its_data_intact` (v19).

On a machine where several builds share cargo's package lock, every test binary's
no-op `cargo build` of a test plug-in can wait minutes for it:
`CANTUS_TEST_PLUGINS_PREBUILT=1` makes the host's and the UI's tests use the plug-ins
already in `target/test-plugins` (the host suite then runs in about two minutes).

Screens: `cargo run -p cantus-ui --example shoot_hosted -- <dir> [ppp] [es]` renders the
inspector with a hosted instrument loaded and missing, in Slate and Paper; looked at in
both languages (the track row shows the plug-in's own name; *Missing: silent* dims the
status row and its tip gives the reason).

Real instruments (ignored; `CANTUS_PLUGINS` names the folder):

```text
set CANTUS_PLUGINS=C:\projects\daw-r20\plugins
cargo test -p cantus-host --test real_instruments -- --ignored --nocapture --test-threads 1
```

`CANTUS_PLUGIN_FILTER=surge` narrows it to names containing the text. Each instrument is
loaded through the adapter (a four-note chord with the transport playing, mod wheel and a
two-semitone bend, blocks of 512; again from a fresh instance; again at blocks of 97; state
saved and loaded into a fresh instance with every writable value compared) and through the
whole host (put on a track, the riff played, saved with its state, reopened in a fresh host,
played again). The table is written to `target/real-instruments.md`.

## Compatibility (2026-10-05, Windows 11 x64, 48 kHz)

Eight products, nineteen plug-ins (Six Sines and sfizz ship a multi-output variant too);
every one **loads, plays, keeps its state and reopens** in Cantus. Columns: *Chord* is
the adapter's peak for a four-note chord at velocity 0.8 (with the mod wheel and a bend
in the middle); *Repeatable* is a fresh instance giving the same samples at the same
blocks; *Blocks* is the same at blocks of 512 and of 97; *Params (stored)* is what
the plug-in lists and, in brackets, what a project keeps (MIDI-CC inputs excluded);
*Values back* counts stored parameters equal after saving the state and loading it into
a fresh instance; *Bytes* says whether that instance saves the same bytes again; *In Cantus* is the peak of the
riff played through the whole host (track, strip, master), and *Reopened* means saved
to a `.cantus` file, opened in a fresh host, loaded and sounding.

| Instrument (version, licence) | Format | Load | Params (stored) | Latency | Chord dBFS | Repeatable | Blocks | Values back | Bytes | In Cantus dBFS | Reopened | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| Surge XT 1.3.4, GPL-3.0 | CLAP | 10.5 s | 775 | 0 | −4.1 | no | no | 775/775 | same | −16.5 | yes | needs `SurgeXTData` beside it for its factory patches (portable layout); loading is mostly its data scan (2.4–10 s, by disk cache) |
| Surge XT | VST3 | 10.6 s | 2855 (775) | 0 | −4.7 | no | no | 775/775 | same | −16.5 | yes | 2 080 of its parameters are MIDI-CC inputs, now hidden and not stored |
| Dexed 1.0.1, GPL-3.0 | CLAP | 2.7 s | 156 | 0 | −6.7 | varies | no | 156/156 | same | −20.4 | yes | |
| Dexed | VST3 | 1.6 s | 2238 (158) | 0 | −6.7 | varies | no | 158/158 | same | −20.4 | yes | |
| OB-Xf 1.0.3, GPL-3.0 | CLAP | 0.9 s | 102 | 0 | −8.0 | no | no | 102/102 | same | −20.8 | yes | factory patches are a separate download (not needed to play) |
| OB-Xf | VST3 | 1.4 s | 2183 (103) | 0 | −8.5 | no | no | 103/103 | same | −21.2 | yes | |
| Six Sines 1.2.1, MIT | CLAP | 3.1 s | 2554 | 0 | −5.5 | yes | no | 2554/2554 | same | −16.0 | yes | prints *"thread-error: this code must be running on the main thread"* in the whole-host test (see below) |
| Six Sines | VST3 | 0.4 s | 4650 (2570) | 0 | −5.5 | yes | no | 2570/2570 | same | −19.2 | yes | single-file VST3 (old layout): found and loaded |
| Six Sines, Seven Outs | CLAP / VST3 | 1.6 / 0.4 s | 2554 / 4650 | 0 | −5.5 | yes | no | all | same | −16.0 / −19.2 | yes | only the main stereo out is heard (multi-out is left) |
| Firefly Synth 2 2.1.6, MIT | CLAP | 2.8 s | 3324 | **16** | −7.5 | yes | no | 2028/2028 | differ | −20.0 | yes | a **CLAP bundle folder** on Windows: the scanner walks into it and finds the binary; 16 samples of latency, compensated; a 1.4 MB state |
| Firefly Synth 2 | VST3 | 7.0 s | 3454 (2028) | **16** | −7.5 | yes | no | 2028/2028 | same | −20.0 | yes | |
| JC303 0.13.0, GPL-3.0 | CLAP | 1.9 s | 40 | 0 | −12.3 | yes | no | 40/40 | differ | −21.5 | yes | |
| JC303 | VST3 | 1.7 s | 2121 (41) | 0 | −12.3 | yes | no | 41/41 | differ | −21.5 | yes | |
| RipplerX 1.5.19, GPL-3.0 | VST3 | 1.1 s | 2159 (79) | 0 | −0.3 | yes | no | 79/79 | same | −11.5 | yes | VST3 only (no Windows CLAP) |
| sfizz 1.2.3, BSD-2-Clause | VST3 | 5.6 s | 543 (396) | 0 | +3.4 | yes | no | 396/396 | same | −13.0 | yes | no SFZ loaded: plays its built-in sine (loud by design); VST3 only; *sfizz-multi* (16 outs) the same |

**Not tested**, and why: Odin 2 and Geonkick ship only Inno Setup installers this machine
cannot unpack without running them (not done); Vitalium has no Windows binaries in its
GitHub release; OB-Xd 2 is installer-only with an unclear licence (OB-Xf replaces it).
Sources, hashes and file paths: `C:\projects\daw-r20\plugins\SOURCES.md` (outside the
repository).

What the table says:

- **Everything plays and comes back.** All 19 load in the scanner's separate process and
  in Cantus, play a chord with finite output, follow the transport, take the mod wheel and
  a pitch bend without complaint, return every writable value through their state, and
  reopen from a saved project with their sound.
- **No real plug-in is identical across block sizes**, and several are not even
  repeatable from one fresh instance to the next (Surge XT, OB-Xf, at times Dexed —
  presumably free-running oscillator phases and analogue-style drift). Cantus gives every
  note to the sample (the repository's synth proves it to the bit); synths like these render in internal sub-blocks (Surge XT's is 32 samples) and
  run their modulation per sub-block, so where a host's block boundaries fall changes
  their output slightly. A bounce of a project with such a plug-in therefore matches
  playback exactly only at the same block size.
- **"Bytes differ"** (Firefly CLAP, JC303) means the state written again after loading is
  not byte-identical (something in it changes on each save); every value still comes
  back, so nothing audible is lost.
- **CLAP and VST3 of one synth sound alike through the adapter**: identical peaks for the
  repeatable ones (Dexed, Six Sines, Firefly, JC303), within their own run-to-run spread
  for the others, the bend reaching VST3 through `IMidiMapping`. Six Sines plays the riff 3 dB louder as CLAP
  than as VST3 through the whole host while its chord is identical in both; the cause
  was not investigated (the riff's notes are shorter and overlap, so the two builds may
  differ in voice handling).
- **The mod wheel reaches VST3 plug-ins as the parameter their controller maps it to**
  (sfizz's *Controller 1* showed it). The runs found two defects, both fixed: MIDI-mapped
  changes were not echoed to the controller (they are now, as the host's own changes
  always were), and those parameters were treated as settings — JUCE plug-ins expose
  about 2 000 of them, which the project stored and the generic editor would have listed;
  the VST3 backend now reports them hidden and read-only.
- **Six Sines' thread message** appears only in the whole-host part of the test, where one
  thread runs both the processor and the host's frame (the adapter part, where nothing
  calls the plug-in's `on_main_thread`, is silent). Cantus's thread check answers
  correctly for each call (main outside processing, audio inside it), so the likeliest
  cause is a check of Six Sines' own against the thread it saw processing; in the
  application the audio callback has a thread of its own. Not yet confirmed in the
  running application; sound, state and values are unaffected.

## Left

- **Hosted instruments (and hosted inserts) in stems**: `render_stems` builds every unit
  itself; it needs to take hosted units as `render_with_instruments` does.
- **Multi-output instruments** (Six Sines, Seven Outs; sfizz-multi): only the main stereo
  out is heard. Routing extra outs to strips is Pulsus's per-pad-outputs job too.
- **MPE**: per-note expressions travel through the adapter, but nothing in Cantus makes them
  per note yet (live MPE input, an expression lane in the piano roll).
- The plug-in's **own presets** in the gallery (CLAP preset-discovery, VST3 programs).
- CLAP `render` (offline mode hint), `voice-info`, `note-name`; VST3 `INoteExpressionController`.
- Crash isolation for a running plug-in (unchanged from ADR 0019).
- A screenshot pass of the picker's hosted entries belongs to browser's gallery.
- Six Sines (CLAP) prints a thread-check message in the whole-host test; confirm it is
  silent in the running application, where audio has its own thread.
- The plug-in's window is opened, focused and closed through the existing native window
  code; a lit "window open" state on the track header is device-ux's chrome.

## Notes for others

- **Engine**: `NativeUnit::kind` is now a `UnitKind`; build one with `NativeUnit::new`
  (native) or `NativeUnit::hosted` (anything else). `InstrumentUnit` gained five defaulted
  methods; `set_position` is called for **every** unit before each segment, so a native
  instrument can sync its LFOs to the song now (round 17's item).
- **Delay compensation** now counts instrument latency (`Processor::compensate` seeds each
  strip's arrival with its tracks' instrument latency). No native instrument reports any
  today, so nothing renders differently.
- **Stems** were silent for native instruments (no units were sent); fixed with one call,
  as `render` does.
- **Automation** (same schema number): the integrator renumbers one migration and fixture;
  the two hosted-instrument commands are good automation targets (a value by plug-in id).
