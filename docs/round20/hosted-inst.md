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

COMPAT_TABLE

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
