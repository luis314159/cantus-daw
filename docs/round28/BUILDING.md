# Round 28 — what needs neither the owner's hardware, nor their paperwork, nor other software

**The owner's goal:** *"the best DAW in the world, made with excellence"* (Colossians
3:23). Tests before code, nothing half done, look at what you build (`shoot` examples).
**The owner's rule, this round and every round:** every gap a feature leaves is closed in
the same round. Whatever you would write under "Left" in your report comes back to you as
work — so finish end to end (reachable from the real app, drawn, translated, documented,
tested), and only list as open what truly needs the owner (their ears, their hardware, a
decision, a lawyer).

The ask (2026-10-14), five items: (1) the top bar à la FL (BACKLOG §6e); (2) voice: the
real-time auto-tuner (§3d) and the Melodyne-class pitch editor (§3c) on one base; (3)
effects (§3b): the modulation family and the dynamics/analysis family; (4) our own sound
(§3e/§5): a downloadable acoustic drum kit and, if research allows it complete, the
modelled piano Chorda; (5) from the leaders (§0.2): take comping à la Logic/Studio One and
Bitwig-style modulators on any parameter, hosted plug-ins included. Nothing that needs a
third-party SDK or hardware (no Ableton Link, no MIDI 2.0).

Read, in order: this file (the rules and **your** section completely); `CLAUDE.md`;
`docs/STATUS.md`'s top ("The next session" and the newest round's section);
`docs/BACKLOG-excellence.md` at the sections your brief names; the ADRs your brief names.

## The builders

Never more than three builders compile at once (a 31 GB machine). Wave 1 starts from the
contract commit; each wave-2 builder starts from `r28/integration` as it is when a slot
frees (so it already contains the wave-1 work merged before it).

| Builder | Wave | Branch / worktree | Delivers | Schema | ADR |
|---|---|---|---|---|---|
| **voice** | 1 | `r28/voice`, `F:\projects\daw-r28\voice` | §3d: the shared pitch tracker and formant-preserving shifter in `cantus-dsp`, and **Tonus**, the real-time auto-tuner | — | 0053 |
| **modfx** | 1 | `r28/modfx`, `F:\projects\daw-r28\modfx` | §3b modulation: Concentus, Flexus, Phasis, Tremor, Vibrans, Anulus, Translatio, Fluctus; host transport in the exported plug-ins | — | 0055 |
| **topbar** | 1 | `r28/topbar`, `F:\projects\daw-r28\topbar` | §6e: the top bar | 26 | 0052 |
| **pitch** | 2 | `r28/pitch` | §3c: the pitch editor (analysis, notes, editor, resynthesis, non-destructive) on voice's base | 27 | 0054 |
| **dynfx** | 2 | `r28/dynfx` | §3b dynamics/analysis: Tonsor, Impetus, Discrimen, Speculum | — | 0056 |
| **leaders** | 2 | `r28/leaders` | §0.2: modulators on any parameter (first), then comping | 28 (modulators), 29 (comping) | 0057, 0058 |
| **sound** | 2 | `r28/sound` | §5/§3e: the acoustic drum kit pack (first), then Chorda | 30 if needed | 0059 (Chorda); the kit amends 0042 |

The **integrator** (the session that wrote this) merges each branch into `r28/integration`,
runs adversarial reviews on it (several lenses, a refuter per finding), sends every
confirmed finding and every "Left" item back to its builder, runs the whole suite + fuzz +
clippy on Windows and Linux, writes STATUS, the manual and the release notes, builds the
test version, and asks the owner before anything goes to `master`.

**The contract commit** (on `r28/integration`) already declares: the thirteen effect kinds
(`EffectKind::{Chorus, Flanger, Phaser, Tremolo, Vibrato, RingMod, FreqShift,
VolumeShaper, Clipper, Transient, TransientSplit, Analyzer, AutoTune}`) as exact
pass-through stubs wired into every list (core, engine `make()`, the picker, the mixer
desk, the editors' dispatch, the plug-in export's descriptors, features, ids and test
arms, the bench, the fx contract test), the instrument kind `InstrumentKind::Chorda` as a
silent stub, **schema 25** (`fixtures/v25.cantus`), and empty locale areas `topbar`,
`pitch`, `modulators`, `comping`, `drums`. Its report lists, per stem, the files you own —
see *File owners* below.

**What the contract (`a327c53`) leaves you — read before you start:**
- Per effect stem you own `crates/cantus-core/src/fx/<stem>.rs` (`NAME_KEY`, `SPARSE`,
  `TAKES_SIDECHAIN`, `TAKES_NOTES`, `CHOICES`, `PARAMS` — `EffectKind::choices()` reads
  `CHOICES`, so stepped choices never need `cantus-plugins/src/params.rs`),
  `crates/cantus-dsp/src/<stem>.rs`, `crates/cantus-engine/src/fx/<stem>.rs`,
  `crates/cantus-ui/src/views/fx/<stem>.rs`, `crates/cantus-ui/locales/fx/<stem>.{en,es}.txt`.
  Chorda: `crates/cantus-core/src/inst/chorda.rs`, `crates/cantus-dsp/src/chorda.rs`,
  `crates/cantus-engine/src/inst/chorda.rs`, `crates/cantus-ui/src/views/inst/chorda.rs`,
  `crates/cantus-ui/locales/inst/chorda.{en,es}.txt`; `READY = false` hides it — set it true
  when it plays and put Chorda in a demo (the demos test requires every ready instrument).
- The stubs apply one placeholder parameter, `out` (dB, default 0 — a null at its default).
  Rewrite `PARAMS` freely.
- **Plug-in golden ids are blessed and append-only** (`crates/cantus-plugins/tests/golden/
  ids.txt`): each stem has five lines (clap, vst3, `out`, `cantus.sidechain`,
  `cantus.bypass`). If you drop or rename `out`, delete that one never-shipped line; add your
  parameters' ids with `CANTUS_BLESS_IDS=1 cargo test -p cantus-plugins --test ids`.
- **Shared files where you edit only your own line or block:** `crates/cantus-bench/src/
  kernels.rs` and `crates/cantus-engine/tests/support/heavy.rs` (your heavy-settings line),
  `crates/cantus-engine/tests/fx_contract.rs` (your `must_rest` line),
  `crates/cantus-plugins/tests/common/mod.rs` (your test-settings line, and your own variant
  in the direct-DSP enum in place of the stub's), `crates/cantus-plugins/tests/clap.rs` (your
  automation key), `crates/cantus-plugins/src/desc.rs` (your descriptor block: features,
  categories, description).
- `EffectKind::ALL`'s order matters: the new kinds sit at indices 12–24 after Reverb, and the
  v25 fixture test reads `ALL[12..]`. Do not reorder.
- The picker has a new **Pitch** category (`picker.cat.pitch`) for Tonus.

## Rules (as in rounds 20–27)

- Your worktree and branch only. The untracked `.cargo/config.toml` is there: `[build]
  jobs = 3`, `debug = 0` and `incremental = false` for dev and test, and `TMP`/`TEMP` on
  `F:\projects\daw-r28\tmp` (**C: is nearly full** — never write big files there). **Never**
  touch `F:\projects\cantus` (the owner's tree, with the owner's uncommitted files), never
  push, never merge, never change a remote.
- **Commit WIP early and often** on your branch (sessions get cut by API errors; you will
  be resumed with your context). Write files in pieces under ~300 lines per tool call.
- Shell: Git Bash has no cargo on `PATH` — prefix `export PATH="$HOME/.cargo/bin:$PATH" &&`.
  A heredoc delimited by `EOF` fails to parse: use `XEOF` or the Write tool. UAC refuses
  test binaries named `*setup*` / `*update*` (os error 740): run a copy under a neutral
  name from the crate folder. The Zscaler proxy may answer a download with an HTML page for
  ~10 minutes: verify what arrives (a zip starts with `PK`, gzip with `1f 8b`), wait, retry;
  **never bypass the proxy or TLS checks**.
- **RAM:** do not start a second `cargo` while one runs; prefer `-p <crate>` (and
  `--test <name>`) over `--workspace` until the end; the whole workspace once, at the end.
- **The base is fmt-clean and CI-clean**: run `cargo fmt --all` before your last commit;
  `cargo clippy --workspace --all-targets -- -D warnings` must be clean (and
  `cargo clippy -p cantus-plugins --no-default-features --all-targets -- -D warnings` if you
  touch the plug-ins). Code behind `#[cfg(windows)]` must not leave Linux dead-code
  warnings (CI is Linux too; `docs/DEVELOPING.md` §4).
- **Tests first**, each seen failing before the code that makes it pass. Every mutation is a
  `Command` (ADR 0004) with its exact inverse; a new command or batch shape gets an arm in
  `crates/cantus-core/tests/undo_invariants.rs` (additive — say so) and `cmd.*` strings.
- **Null tests before DSP**: a control at its default touches no sample, to the bit;
  block-size independence at tolerance 0 (1, 37, 64, 512 frames); offline render equals
  real time; parallel equals serial. No golden hash moves except on purpose (then
  `DSP_VERSION` and the measured size of the change, in one commit — ask the integrator first).
- **Deterministic maths:** `cantus_dsp::math::*`, never std's transcendentals (clippy
  enforces it); every random choice is a hash of a seed (and the note/event).
- **Real time (ADR 0005):** the audio thread never allocates, locks, blocks or does I/O
  (`rt_memory.rs` counts it). `cantus-engine` is `forbid(unsafe_code)`; OS/unsafe code lives
  only in `crates/cantus-rt`.
- **A model change is a schema bump** with the number in the table above: `SCHEMA_VERSION`,
  `migrate_N_to_N+1` (even if it writes nothing), the fixture section in
  `crates/cantus-project/examples/write_fixture.rs` and `fixtures/vN.cantus`; every old
  fixture keeps loading. The integrator chains the numbers at merge.
- **Start-up wiring goes in the boot path.** The shipped app never calls `CantusApp::new`:
  start-up is `cantus_ui::run` → `boot::Boot` stages → `CantusApp::assemble`. Anything
  wired at start (a service, a cache, a setting applied) is wired there, with a test that
  fails if it leaves that path (`the_boot_font_stage_arms_the_fallback` is the model).
  Reviewers will ask, for everything you add: *is it reachable from `cantus_ui::run`?*
- **Kit only in views** (ADR 0024): no stock egui widgets, no literal colours, no
  `FontId::new`; labels in clear label/value units, nothing that overlaps, every overflowing
  area scrollable. **Look at what you build** with the `shoot` examples (add yours), at
  75 % and 200 %, English and Spanish, in Cantus Night and one light theme; PNGs go to
  `F:\projects\daw-r28\shots\<builder>\`, never into the repository.
- **Strings:** your locale areas/files only, both languages (the i18n test requires every
  English key in Spanish), good Spanish. Never edit `en.txt` / `es.txt` or `i18n.rs` (the
  contract registered your files; if you need another file, one additive line in
  `i18n.rs` — say so).
- **Effects** (ADR 0017/0018): each unit passes the family's rules — a null test at its
  defaults (a dedicated `crates/cantus-engine/tests/fx_<stem>.rs` as Sibilus has), the
  table-driven `fx_contract.rs` (block size, split invariance, exact rest twin), exact rest
  (`at_rest`/`skip`; a free-running LFO is never at rest), latency reported through
  `FxUnit::latency`, a kit editor, factory presets made with care, a page
  `docs/plugins/<stem>.md`, a line in the manual (EN and ES), and the CLAP/VST3 export
  bit-identical to the engine (`crates/cantus-plugins/tests/{clap,vst3}.rs` run over every
  descriptor). When you rewrite `PARAMS`, update the plug-in export's test arms and golden
  id lines for your kinds only.
- **Licences (ADR 0047):** `cantus-core/dsp/engine/project/rt` are MPL-2.0 — never copy GPL
  code into them (read GPL references, never port them); MIT/BSD/Apache code may be used
  with its notice (`cargo run -p xtask -- notices`). A new crate needs an approved licence
  and its notice; prefer our own implementation.
- **Finish:** your crates with `cargo test -p <crate> --no-fail-fast` (UI work: `-p
  cantus-core -p cantus-project -p cantus-ui` at least), clippy as above, `cargo fmt --all`;
  then `docs/round28/<name>.md` (done / measurements / tests and what each holds / files
  outside your area / *left* — which should be empty or owner-only / notes for the
  integrator), your ADR, and a short report as your final message.

## File owners (one owner per file; anything else: an additive hook, said in your report)

| Owner | Files |
|---|---|
| **voice** | `crates/cantus-dsp/src/{pitch*.rs,psola*.rs,formant*.rs,tonus*.rs}` (new, declared with `#[path]` from `tonus.rs` or one line in `lib.rs` each — say so), the Tonus stem's files (core `fx/tonus.rs`, engine `fx/tonus.rs`, UI `views/fx/tonus*.rs`, `locales/fx/tonus.*`, `crates/cantus-engine/tests/fx_tonus.rs`, `crates/cantus-ui/tests/fx_tonus.rs`), `docs/plugins/tonus.md`, ADR 0053, `docs/round28/voice.md` |
| **modfx** | the stems `concentus flexus phasis tremor vibrans anulus translatio fluctus` (core, dsp, engine, UI, locales, tests, `docs/plugins/`), shared modulation DSP under `crates/cantus-dsp/src/modfx_*.rs` (new), the exported plug-ins' host transport (`crates/cantus-plugins/src/{clap,vst3}*` — the transport part only), ADR 0055, `docs/round28/modfx.md` |
| **topbar** | `crates/cantus-ui/src/views/transport*.rs`, `views/record_options.rs`, `views/workspace_status.rs`, `crates/cantus-ui/src/settings.rs` (additive fields), `crates/cantus-dsp/src/click.rs`, the metronome lines in `cantus-core/src/audio.rs` (`RecordSettings`), master loudness in `crates/cantus-engine/src/{messages.rs (Meters), graph/master_run.rs}` (additive), pattern mode (where it lives: `cantus-core/src/snapshot.rs` is shared — keep it to an additive hook), the process memory reading in `crates/cantus-rt` (new module), `locales/areas/topbar.*`, `examples/shoot_topbar.rs` (new), ADR 0052, `docs/round28/topbar.md` |
| **pitch** | `crates/cantus-core/src/pitch*.rs` (new), `crates/cantus-ui/src/{pitch*.rs, views/pitch*.rs}` (new), the pitch render in `crates/cantus-engine/src/pitch*.rs` (new), `locales/areas/pitch.*`, ADR 0054, `docs/round28/pitch.md`; additive hooks in `clips.rs` (`ClipEditor::Pitch`), `gestures.rs`, `snapshot.rs`/`backend.rs` (the rendered source) |
| **dynfx** | the stems `tonsor impetus discrimen speculum`, shared DSP `crates/cantus-dsp/src/dynfx_*.rs` (new), the analyser's telemetry path (additive in `graph/exec.rs`/`strip_run.rs`), ADR 0056, `docs/round28/dynfx.md` |
| **leaders** | `crates/cantus-core/src/{modulator*.rs, automation/*}`, `crates/cantus-engine/src/{automation.rs, processor_auto.rs, modulator*.rs}`, the hosted parameter path in `crates/cantus-host/src/{unit.rs, inst_unit.rs, backend.rs}` (additive), `crates/cantus-ui/src/{comping.rs, record_session.rs, views/arrangement_takes.rs, views/modulator*.rs}`, the crossfade render in `crates/cantus-engine/src/processor_rec.rs` (`add_clips`), `locales/areas/{modulators,comping}.*`, ADRs 0057/0058, `docs/round28/leaders.md` |
| **sound** | `crates/cantus-content/**` (manifest entry, any reader), `crates/cantus-content/manifest/content.json`, the Chorda stem (core `inst/chorda*`, dsp `chorda*.rs`, engine `inst/chorda*.rs`, UI `views/inst/chorda*.rs`, `locales/inst/chorda.*`), `docs/instruments/chorda.md`, `locales/areas/drums.*`, Exemplar's SFZ importer (`views/inst/sampler_sfz.rs`, additive), ADR 0059, `docs/round28/sound.md` |

`app.rs`, `keymap.rs`, `views/mod.rs`, `kit/mod.rs`, `lib.rs` files: one additive line each
if you must (a `mod`, an action arm, a key) — list it in your report. `effect.rs` belongs to
nobody: if your kind needs `takes_sidechain`/`takes_notes`/`has_editor` changed from the
contract's value, change that one arm and say so.

---

## voice — the pitch base and Tonus, the real-time auto-tuner (BACKLOG §3d; the base of §3c)

Read BACKLOG §3b (*Pitch correction*), §3c and §3d completely; `research/instruments/sampler.md`
(YIN/pYIN, Signalsmith Stretch), `research/instruments/fm-additive-granular.md` (PSOLA);
ADR 0018 and 0022 (MIDI into effects: `takes_notes`). Nothing exists yet: no pitch
detection, no PSOLA, no STFT utility (`cantus-dsp/src/aequalis_fft.rs` has a radix-2 FFT
that does not allocate after `new`). **Our own implementation**; read papers and permissive
code only (pYIN's reference code is GPL: read the paper, not the code).

**1. The base, in `cantus-dsp`, written to serve both §3d (streaming) and §3c (offline):**
- A **monophonic pitch tracker**: YIN's cumulative mean normalised difference with
  parabolic refinement, a voicing decision (aperiodicity + energy + the de-esser's
  sibilance cue, `sibilus_detect.rs`) that does not wobble on breaths and sibilants, and a
  **probabilistic/Viterbi smoothing (pYIN-style)** for the offline path. Streaming form:
  fixed hop, fixed cost per block, no allocation. Tested on synthetic voices (glottal pulse
  trains through formant filters, vibrato, glides, noise bursts, octave-ambiguous signals):
  gross error rate, fine error in cents, voicing accuracy — targets in your ADR, measured.
- A **formant-preserving pitch shifter**: pitch-synchronous overlap-add (TD-PSOLA) on
  epoch marks from the tracker, with a formant shift by grain resampling (the *throat*
  control) and smooth transitions (no click when the ratio jumps). An offline form for §3c
  (any ratio curve over a take, exact epochs from the whole signal) and a **streaming,
  fixed-latency** form for Tonus.
- **The latency target is the owner's: under 3 ms at 48 kHz (≤ 144 samples), reported to
  delay compensation** (`FxUnit::latency`), constant (never data-dependent). Design for it
  (causal analysis on past samples, grains that end at "now", asymmetric windows — research
  how low-latency correctors do it); measure it; if a very low voice needs a longer window
  for tracking, the *tracking* may look back further, the *output* latency stays fixed.
  State in the ADR the lowest f0 tracked and why.

**2. Tonus (`EffectKind::AutoTune`, stem `tonus`):**
- Targets: a **key + scale** (major, minor, the modes, harmonic/melodic minor,
  pentatonics, blues, chromatic…), **any set of notes** on a one-octave keyboard (per note:
  *on*, *remove*, *bypass*), or **from MIDI** (`takes_notes`: the notes held on the track's
  input or a MIDI clip routed to it become the targets, live). The key can follow the
  project's key if the project has one (if it has none, a key field on Tonus — do not invent
  a model change unless needed; if needed, ask the integrator for a schema number).
- **Retune speed** from 0 ms (the hard effect, clean, no warble) to slow/natural;
  **humanise** (long notes keep more movement), **flex/tolerance**, vibrato keep/add/remove,
  **formant preservation** on by default and a **formant shift** (*throat*), transpose,
  amount. A **detected note / correction amount** display in the editor (telemetry, the
  64-float ring of ADR 0018), drawn with the kit; one knob row on the front (*Key, Scale,
  Speed, Amount*), the rest folded away. Factory presets: *Natural*, *Tight*, *Hard tune*,
  *Throat up/down*, *MIDI lead*.
- The family's rules: the null rules for a corrector are **at 100 % bypass** and **with
  every note allowed, speed at its slowest and amount 0** (bit-identical to the input
  delayed by the reported latency); block-size independence; exact rest (silence in →
  `at_rest`); deterministic; CLAP/VST3 export bit-identical. If the exported plug-ins cannot
  take MIDI notes yet, make them (CLAP note ports, VST3 event bus) — no gap.
- **Done when** (measured, in your report): a sung-like synthetic line at speed 0 gives the
  hard effect with no warble; at a natural speed corrects with no click at note changes;
  latency measured ≤ 144 samples at 48 kHz and reported; MIDI targets follow notes in real
  time (a sample-accurate test); a real recorded voice the owner will judge by ear — prepare
  a demo project (your report says how to open it) and `shoot` the editor.
- Monitoring: a corrected voice is monitored through the track's chain (ADR 0025 §4);
  check that the monitored path is delayed by Tonus's latency only, and say what a singer
  hears.

Hand the base over to **pitch** (wave 2) with a clear API (in ADR 0053 and the module
docs): offline analysis of a whole take (f0 per hop, voicing, epochs, energy), the offline
shifter (ratio and formant curves), both deterministic and fast (measure: seconds of
analysis per minute of audio).

## modfx — the modulation family (BACKLOG §3b)

Eight units, each a full member of the family (the rules above), ours better than the
reference: read §3b's table and its *Modulation* line, and the reusable DSP — `delay.rs`
(fractional read with a smoothed time), `calor_chorus.rs` (a two-tap BBD ensemble on the
stream clock), `organum_scanner.rs`, `organum_rotary.rs`, the interpolators
(`gravis_string.rs::lagrange`, `unda_table.rs::hermite`), the LFOs (`modulus_env.rs::Lfo`),
`rest.rs`, `smoother.rs`. Shared pieces you write go to `cantus-dsp/src/modfx_*.rs`.

| Stem | Unit | First-rank means |
|---|---|---|
| `concentus` | **Concentus** — chorus | BBD-style modulated delays, modes **Juno I / II / I+II** (delay, rate and depth from published measurements of the circuit) and **Dimension-style 1–4**, 1–4 voices, stereo spread, a **mono-compatibility meter** in the editor |
| `flexus` | **Flexus** — flanger | **through-zero** (the delayed path crosses the dry path; the null at the centre point holds to the bit), feedback ±, tempo-synced LFO, an **envelope mode** (sweep by input level), a manual sweep |
| `phasis` | **Phasis** — phaser | 2–12 first-order allpass stages (TPT), feedback, Phase 90 / Small Stone voicings as presets, stereo phase offset, synced LFO |
| `tremor` | **Tremor** — tremolo and auto-pan | amplitude or pan, shapes (sine, triangle, soft-edged square, saw), tempo sync with swing, stereo phase, the equal-power pan law (ADR 0012) |
| `vibrans` | **Vibrans** — vibrato | pitch vibrato by a modulated delay, depth in cents, synced or free, a rise time |
| `anulus` | **Anulus** — ring modulator | a band-limited carrier (sine, triangle, square), frequency free or from MIDI notes (`takes_notes` — only if you make it work fully, export included), an LFO on the frequency, mix, no DC leak |
| `translatio` | **Translatio** — frequency shifter | single-sideband by a **Hilbert pair of allpass chains** (≥ 60 dB sideband rejection over 20 Hz–20 kHz at 44.1/48 kHz, measured), up / down / both outputs, feedback, an LFO on the shift, Bode-style |
| `fluctus` | **Fluctus** — volume shaper (LFO Tool / Kickstart) | a **drawn curve** (points + tension; reuse the automation editor's shape model if it fits) over 1/16 … 4 bars synced to the song position, pumping presets without a key, a smoothing control so steps never click, a curve editor drawn with the kit |

- **Tempo and position:** effects get the transport through `FxUnit::set_position(&SongPosition)`
  (once per pass). A synced LFO's phase is a **function of the song position** (a render
  from bar 5 equals bars 5… of a full render; offline = real time); a free LFO is
  deterministic from the start of playback. Test both.
- **The exported plug-ins do not forward the host's transport today** (`cantus-plugins`
  never calls `set_position`; `editor.rs` returns `Transport::default()`): make CLAP
  (`clap_event_transport`) and VST3 (`ProcessContext`) feed `SongPosition`, so a synced
  Tremor in another DAW follows that DAW's tempo — with a test for each format. This closes
  the gap for every synced unit, not only yours.
- Factory presets per unit, made with care (named, musical); `docs/plugins/<stem>.md` per
  unit; a manual section (EN and ES) for the family; `shoot` each editor.
- **Done when** all eight pass the contract tests and their own null / through-zero /
  sideband tests, the export is bit-identical, and every editor has been looked at in both
  themes and both languages.

## topbar — the top bar à la FL (BACKLOG §6e)

Read §6e completely and look at FL Studio's top strip (the owner's reference; search
`research/` for screenshots of it). Today: `crates/cantus-ui/src/views/transport.rs` (1 176
lines, hand-placed `Rect` maths; `enum Group`, whose order is the order groups are
*dropped* when narrow — dropped, not folded), called from `app.rs` (~l.3509,
`Panel::top("toolbar")`); the status bar `views/workspace_status.rs` shows CPU, voices and
dropouts from `EngineStats` (`cantus-engine/src/messages.rs`).

**What the bar must hold**, one visual group per function, separated by the kit's dividers:
1. **Transport**: to start, play, stop, record (+ the record-options ▾), loop.
2. **Tempo block**: BPM (exists); **tap tempo** (new: the average of the last taps, reset
   after a pause, one undo step per committed tempo); the **time signature**, editable (new
   UI for the existing `Command::SetTimeSignature`).
3. **Metronome** with **count-in / pre-roll** and **volume** in the same group (today in
   the record-options popover: `RecordSettings.count_in_bars`, `click`, `click_db`), plus
   its **sound** (two or three click voices in `cantus-dsp/src/click.rs`) and **accent**
   on/off — new model fields → **schema 26** (the migration keeps the old sound).
4. **Pattern / song mode** — new. *Pattern mode* plays the clip open in the channel rack /
   piano roll (the "current pattern") alone, looped over its length, through its track's
   chain; *song mode* plays the arrangement. Decide where the mode lives (FL saves it with
   the project; if you save it, it rides schema 26), make it work in the engine through an
   additive hook where the snapshot is compiled, and test it (pattern mode renders exactly
   that clip, looped, sample-accurate; recording in pattern mode does what FL does — or is
   refused with a clear hint).
5. **Snap / grid** of the focused editor (each editor has its own grid menu today; the bar
   shows and sets the focused one's).
6. **Position** as bars.beats.ticks **and** time (exists; keep it legible).
7. **CPU / RAM / voices / dropouts** — RAM is new: the process's working set (Windows
   `GetProcessMemoryInfo` behind a safe function in `crates/cantus-rt`; Linux
   `/proc/self/statm` without unsafe; macOS `task_info` or "—"), read off the audio thread
   at a low rate.
8. **Master meter with a peak / LUFS toggle** — the engine has no master-bus loudness: feed
   momentary and short-term LUFS (and true peak) from the master into `Meters`, reusing
   `cantus_dsp::terminus::loudness` (K-weighting, `LoudnessMeter`) — real-time safe, its
   cost measured, and a null on the audio (metering never changes a sample).
9. **Project name / title**, editable in place with the kit text field
   (`Command::SetProjectName` exists, unused); the window title follows it (`boot.rs` sets a
   fixed title — fix it on the boot path).
10. More in reach where room allows (§6e's second bullet): MIDI-input and record-arm
    indicators, the audio-device chip (moved from the status bar), the tool / skin / zoom
    shortcuts, a quick-search/command box if one exists to open.

**Groups the owner can show, hide and reorder** (a *Customise top bar…* menu: tick to
show, drag to reorder, *Reset*; stored in `Settings`; the default an FL-like order), and
**responsive**: when the window is narrow, groups fold — in order of importance — into a
**⋯ overflow menu** (kit `Glyph::More`) where each folded group stays usable; never
clipped, never overlapping, at **every width from 800 px up and every UI zoom (75–200 %)**.
A property test sweeps widths × zooms × group sets × both languages and asserts that no two
groups' rects intersect and none leaves the bar. Keep the warm-frame allocation budget
(`tests/workspace_budget.rs`). What moves out of the status bar leaves it (no duplicates).

**Look first-rank**: judge against FL's strip with `shoot` PNGs — a new
`examples/shoot_topbar.rs`: widths 800 / 1280 / 1920 / 2560, zoom 75 / 100 / 200 %, EN and ES,
Night and a light theme, the overflow open and closed. Every control has a hint
(`kit::hint`); labels in label/value units; keys where FL has them (tap tempo, metronome,
pattern/song — check `keymap.rs` for conflicts; a keymap change bumps the keymap version as
ADR 0010 says).

**Done when** every group works from the real app (reachable from `cantus_ui::run`), the
property test holds, the shoots look right at every width, and the manual (EN/ES) describes
the bar.

## pitch — the pitch editor for recordings, Melodyne-class (BACKLOG §3c)

Starts from `r28/integration` **with voice merged**: read ADR 0053 and the base's module
docs (the offline tracker, epochs, the offline formant-preserving shifter). Read §3c
completely. Today: no audio editor view (an audio clip opens the Inspector:
`views/inspector_audio*.rs`); the piano roll (`views/piano_roll.rs`,
`piano_roll_paint.rs`) is the grid to reuse; double-click opens editors through
`gestures.rs::open_clip_editor` and `clips.rs::{ClipEditor, editor_for}` (ADR 0051 — the
editor follows the clip's content, no stored hint); the waveform peak cache
(`waveform_store.rs`, `waveform_disk.rs`: a worker thread, a key from path+size+mtime, files
in the data folder) is the pattern for an analysis cache.

1. **Analysis** on a worker when a take lands or on demand: the pitch track, voicing,
   sibilants/breaths, then **notes** — each a blob with its centre pitch, its drift and its
   vibrato kept apart — and the key/scale detected from the take (editable). Cached in the
   data folder like the peaks. **The segmentation that edits refer to is stored in the
   project** (ADR 0023's principle: analysis run offline, its *result* stored), so an
   improved analyser never moves an old edit.
2. **The editor** (a new `ClipEditor::Pitch`, opened from an audio clip — double-click
   offers it the way ADR 0051 decides, plus *Edit pitch* in the clip's menu and the
   Inspector): note blobs on a piano-roll grid over the waveform, the pitch curve drawn
   through them. Per note: **pitch** (drag; snap to the scale or free, Alt for cents),
   **pitch centre** (% toward the note, keeping the natural drift), **drift**, **vibrato**
   depth, **formant**, **amplitude**, **start/end and timing** (stretch a note without
   touching its neighbours), **split** and **join**. Macros on the selection: *correct
   pitch* (%), *correct drift* (%), *quantise timing* to the grid. A/B against the original,
   play from the editor, zoom/scroll like the piano roll, MIDI out of the detected notes
   (drag to an instrument track), and *double* (a second voice a third/fifth above in a new
   clip). Every edit is a `Command` with its inverse (fuzz arms).
3. **Resynthesis** that keeps the voice: the base's offline shifter with ratio, formant and
   time curves from the edits, transitions smoothed so a moved note never clicks; **non
   destructive** — the source file is never touched; the clip carries the edits (**schema
   27**: the model, the migration, the fixture) and the engine plays a **render of the
   edits** made off the audio thread (cached, keyed by source + edits hash), so **offline =
   real time** and bounce/stems hear exactly what the editor plays. Say what plays while a
   render is pending (the original, never silence) and make export wait for pending renders.
4. **Done when**: a synthetic sung take with known errors is corrected to within 5 cents of
   the targets; formants measured preserved at ± 3 semitones (spectral envelope distance,
   stated); a timing edit leaves its neighbours bit-identical; bounce = playback to the bit;
   undo of every edit exact; the editor shot in both themes and languages; a demo project
   with a real voice for the owner's ears (how to open it in your report); manual EN/ES.

## dynfx — dynamics and analysis (BACKLOG §3b)

Four units, each a full member of the family (the rules above). Read §3b's table rows
*Clipper*, *Analysis and visualisation*, *Transient splitter*, *Transient processor*. Reuse:
`terminus_os.rs` (`TruePeak`, `true_peak_of`, `Interp`), `fervor_os.rs::Oversampler`,
`fervor_shape.rs::Shaper`, `multiplex_filters.rs::Xover`, `multiplex_linear.rs`, the FFTs
(`aequalis_fft.rs`, `spectrum.rs::SpectrumAnalyser`), `terminus_loudness.rs` (K-weighting,
`LoudnessMeter`, `ColumnLoudness`). There is no standalone envelope follower: write one in
`cantus-dsp/src/dynfx_*.rs` (others may reuse it — e.g. leaders' follower modulator).

| Stem | Unit | First-rank means |
|---|---|---|
| `tonsor` | **Tonsor** — clipper | hard / soft / analog curves (and a knee), oversampling 1–16×, **true-peak aware** (a ceiling that holds in dBTP, inter-sample overs measured and shown), input/output gain with an auto-gain-match option, a clip meter (how much and how often), **a null at threshold 0 dBFS with no overs** |
| `impetus` | **Impetus** — transient processor | attack and sustain gain on **level differences, not a threshold** (two followers, fast and slow), a split-band option (only above/below a crossover), sidechain detection (`takes_sidechain`), output; **the default is a null** |
| `discrimen` | **Discrimen** — transient splitter | low- or **zero-latency** transient detection splitting the signal into a **transient** and a **body** part, each with its own gain, dynamics (a simple comp) and tilt EQ, summed back; **both parts summed with no processing equal the input to the bit**, the zero-latency mode measured |
| `speculum` | **Speculum** — analyser (Wave Candy / SPAN) | **oscilloscope** (triggered, synced to the beat), **spectrum** (FFT with averaging and slope; peak hold), **spectrogram**, **vectorscope / goniometer**, **correlation**, **loudness** (LUFS momentary / short-term / integrated, LRA, true peak), peak/RMS — modes as tabs or a layout, resizable, the editor window docked or floating; a pure pass-through (always a null) |

- **Speculum's data path:** today the analyser ring carries mono (input, output) pairs
  (`graph/exec.rs` `Analysis`, `strip_run.rs::analyse`, the UI drains it with
  `backend.rs::drain_analyser`). Speculum needs stereo of its own slot, more than one open
  at a time, and no dropped frames at 60 fps: extend it additively (real-time safe, no
  allocation), do the FFT/loudness on the UI side, and keep the frame budget —
  **it reads the same LUFS as the EBU R128 / Tech 3341 test vectors** (synthesise them in the
  test: the 1 kHz / stereo cases with their expected values), and draws at 60 fps under the
  frame budget (measure with the existing frame-budget tests).
- Factory presets per unit; `docs/plugins/<stem>.md`; the manual (EN/ES); `shoot` each editor.
- **Done when** each passes the contract tests and its own null tests, the export is
  bit-identical, and the editors are looked at in both themes and both languages.

## leaders — modulators everywhere (first), then take comping (BACKLOG §0.2)

### A. Modulators on any parameter, Bitwig-style (schema 28, ADR 0057)

Read ADR 0027 (automation) completely. Today: automation clips and lanes
(`cantus-core/src/automation/{mod,range,compile,edit}.rs`; `AutoTarget` names track/strip/
master gain-pan-width, sends, native effect params by key, native instrument params,
Firmus); the engine evaluates lanes on a 16-sample grid
(`cantus-engine/src/automation.rs`, `processor_auto.rs`: gain/pan ramp per sample, effect
and instrument params through `configure(&spec)` on parameter-set copies). Modulation
exists only *inside* instruments (`cantus-core/src/inst/modulation.rs`,
`cantus-dsp/src/modmatrix.rs`). **Hosted plug-ins' parameters are not automatable**
(ADR 0027: "a target `Plugin { effect, id }` is the next step"); hosted params reach the
plug-in as `ParamChange { offset: 0 }` per block (`cantus-host/src/{unit.rs, backend.rs,
inst_unit.rs}`).

- **Close the hosted gap first:** `AutoTarget::Plugin { effect, id }` (and the hosted
  instrument's twin) — automation lanes, "last tweaked", recording automation from a
  plug-in's own GUI if the host already receives its edits; **sample offsets** inside the
  block for hosted parameter changes (CLAP `param_value` events and VST3
  `IParameterChanges` both carry them).
- **Modulators** — sources: **LFO** (shapes, rate free or synced, phase, retrigger on
  play/note), **envelope** (ADSR, triggered by the track's notes or by an audio threshold),
  **follower** (the level of any strip, attack/release; share dynfx's follower if merged,
  otherwise write it and tell the integrator), **step** (up to 32 steps, synced, glide),
  **random** (sample-and-hold or smooth, seeded — a hash of seed and step, so a render
  repeats). One modulator drives **several targets**, each with its own **bipolar depth**;
  a target may take several modulators (summed, then clamped to the range). Any target an
  automation lane can address, hosted plug-ins included; also a modulator's own rate/depth
  (one level of nesting) if it falls out naturally.
- **With automation:** modulation offsets the automated (or static) base value in the
  normalised domain; the knob shows the base and a modulation ring (the kit); *Bake to
  automation* turns a modulator's effect on a target into an automation clip (one undo
  step). Every edit is a `Command` with its inverse (fuzz arms).
- **Determinism:** a synced modulator's phase is a function of the song position; free ones
  are deterministic from the start of playback; **offline = real time**, a render from bar 5
  equals bars 5… of a full render; block-size independence at tolerance 0 for native
  targets (hosted targets: the values sent and their offsets identical).
- **UI:** where modulators live and how one connects them — Bitwig drags a modulator's
  handle onto a knob; ours must be as fast: a *Modulators* panel per track/strip (and on
  the master), *Modulate…* on any knob's context menu (and in a hosted plug-in's parameter
  list), the connection's depth edited by dragging on the knob's ring with a modifier. Drawn
  with the kit, shot in both themes and languages.
- **Done when**: an LFO on a native effect param, an envelope on a filter cutoff, a follower
  ducking a strip, a step sequencer on a hosted plug-in's parameter (the CLAP test plug-in
  in `cantus-test-plugins`) all play and render identically; undo exact; schema 28 with its
  fixture; the manual (EN/ES); ADR 0057.

### B. Take comping, Logic / Studio One-class (schema 29, ADR 0058)

Read ADR 0025 §5 and `docs/round19/` first — round 19 built a lot: takes are clips on lanes
above 0 (`Clip::lane`), loop recording lays passes on lanes (`ui/src/record_pass.rs`),
comping is lane-0 clips that are windows onto the takes (`ui/src/comping.rs`: swipe,
promote, delete unused, explode), drawn in `views/arrangement_takes.rs`. **What is missing**
against Logic/Studio One — build all of it:
- **Crossfades at comp seams** (today butt joins): a seam crossfade length (default ~10 ms,
  equal-power, editable per seam and globally), rendered in the engine (`processor_rec.rs::
  add_clips`) — the crossfade *is* the model (a seam, or an overlap allowed on lane 0 with
  explicit fades: choose, write it in the ADR, enforce it in `Command::apply`, not only by
  convention).
- **Comp boundaries edited as a unit**: drag a seam and both sides move; swipe across
  lanes extends or shortens regions; *Quick swipe* off/on; the selected take for a region
  shown on the take lane.
- **Alternate comps** (Logic's comp A/B/C: named comps per track, switch, duplicate,
  flatten) and **flatten/merge** to one clip (a bounce of the comp, non-destructive source).
- **Grouped comping**: tracks recorded together (a drum kit's mics) comp together — swipe on
  one, the same region on the group's tracks (use the existing track/strip groups; say what
  defines the group).
- **Done when** the gestures work from the real app, crossfades render bit-identically
  offline and live, undo is exact, the shoots look right, the manual (EN/ES) describes
  comping, schema 29 has its fixture.

## sound — an acoustic drum kit to download (first), then Chorda (BACKLOG §5, §3e)

### A. The acoustic drum kit (amends ADR 0042; the Salamander's flow)

Read ADR 0030, 0042, 0050, `docs/round26/{content,sampler}.md`, and `research/content/
samples.md` (l.188–192, 463–474). The Salamander flow is the model: a pack in
`crates/cantus-content/manifest/content.json` (format 2: licence, credit, sha256, size,
tar.gz or zip, `instruments` that are `.sfz` files loaded by **Exemplar**), installed by the
manager on a worker, its instruments in the picker as `pack:<pack>/<id>`, streamed from disk
(ADR 0050). Exemplar's SFZ importer (`views/inst/sampler_sfz.rs`) supports `group`/`off_by`
(hi-hat choke), velocity layers with crossfades, `seq_length`/`lorand` round robins, CC
triggers; limits `MAX_ZONES = 1024`, `MAX_GROUPS = 64`, stereo output only.

**Licences, verified by the integrator's research (2026-10-14) — check again from the
primary sources before choosing:**
- The owner named **Naked Drums** or **DrumGizmo**. Naked Drums (Wilkinson Audio) is
  reported as *CC BY 4.0 in the SFZ port* `github.com/sfzinstruments/WilkinsonAudio.NakedDrums`
  by one source and as *commercial, free only in May 2016* by another: read that
  repository's LICENSE file; accept it only if it is plainly CC BY (or CC0).
- **DrumGizmo DRSKit 2.1**: CC BY 4.0 (README inside the zip), 2.8 GB zip, 13 mono mic
  channels per sample (44.1 kHz, 32-bit float), 35 instruments, velocity by `power` per
  sample, hi-hat choke by group, GM-like midimap, **no default mix** (we would write the
  pan/gain table); MD5 only (we pin our own SHA-256). An SFZ port may exist.
- **Virtuosity Drums v0.925** (Versilian + Karoryfer): **CC0**, 1.23 GB zip with a published
  SHA-256 (`c6c5d0fe…963a`), SFZ, stereo FLAC 48 kHz/24-bit per mic position (kick, snare,
  OH, mid, room, lo-fi), up to 36 velocity layers, hi-hat states and pedal; a jazz kit with
  two toms; its own default mix (kick + snare + OH).
- Refused: ShittyKit (extra conditions), Salamander Drumkit (SA), Aasimonster (commercial
  ROM in the kick).

Choose the best kit that is legally clean (prefer the owner's named ones when clean), and
make it **complete**: installable in one step, verified (size + SHA-256, our strict
tar/zip readers), credited everywhere credits show, its instrument(s) **ready in the
picker**, **streaming**, playable from the channel rack and piano roll with a GM-compatible
note map (kick 36, snare 38, …, hi-hat closed/pedal/open with the choke), velocity layers
and round robins correct, **no identical consecutive sample in a fast roll** where the kit
has round robins (asserted), the **multi-mic** handled: a stereo mixdown that sounds like a
mixed kit by default (made at install or by SFZ zones per mic — measure RAM/disk/CPU of
both and choose), and per-mic level if it fits Exemplar without a model change (if per-mic
faders need multi-output instruments, that is §3e item 11 — say so; do not half-build it).
Anything Exemplar's SFZ reader lacks for the kit (FLAC? `#include`, `$`-defines, CC
opcodes, `note_polyphony`, more than 1 024 zones…) you add, with tests. A real-archive
ignored test like the Salamander's (`CANTUS_*_ARCHIVE`). Download into
`F:\projects\daw-r28\downloads\` (never C:, never the repository). The manual (EN/ES) and
*About ▸ Credits*.

### B. Chorda, the modelled grand piano (ADR 0059) — if it can be finished

Read `research/instruments-next/notes.md` §6.4 (l.339–438) and §8 (patents) completely, and
the owner's decision in BACKLOG §3e (*The piano, as the owner decided it*): **digital
waveguide strings** with dispersion allpasses, Lamina's nonlinear hammer extended with
hysteresis (`cantus-dsp/src/lamina_voice.rs`), two/three-string unisons with two-stage
decay and beating, phantom partials, a multirate soundboard, a sustain-pedal resonance
model, continuous half-pedalling, dampers, una corda, a `model_version`; coefficients from
**closed-form physics and published measurements** — **not** a modal bank whose
per-partial coefficients come from a soundboard parameter (Modartt US 7,915,515 to
2027-10-20), **not** detuned non-speaking delay-loop resonators (Casio US 8,178,772), **not**
a non-speaking-pitch duplex loop (Yamaha US 10,818,278). Gravis's string
(`gravis_string.rs`: loss design, dispersion fit) is the base for the strings. Voices split
across the pool (ADR 0038); soundboard and sympathetic resonance in a post-voice stage
summed in voice order; the hammer–string loop solved with a fixed iteration count.

**First decide, in writing (`docs/round28/sound.md`, before code), whether Chorda can be
finished this round to its *done when* — the measurable parts: partials within 1 cent of
the stretch target on every key, per-partial T60 within −25 %/+40 % of published
measurements, half-pedal continuous, 64 pedal-down voices inside 25 % of one core at 256
frames, null at defaults, block-size independence, offline = real time — with presets made
with care, the kit shell (Play/Edit/Matrix), its page and the manual.** The blind listening
test and the legal review are the owner's (they go to `docs/beta-checklist.md`). If you
judge it cannot be finished, stop, say why and what it would take, and the integrator
decides; if it can, build it complete and unhide it in the picker (the contract hid it).
