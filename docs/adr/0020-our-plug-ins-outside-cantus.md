# ADR 0020 — Our plug-ins outside Cantus: CLAP and VST3, a separate build

**Status:** Accepted · **Date:** 2026-09-24 · **Extends:** ADR 0005 (the audio thread), ADR 0017 (the plug-in family), ADR 0018 (the plug-in contract)

## Context

The user, on the round's direction: *keep our plug-ins native so they perform well, but
give people who want to take them out of our DAW the option, with the best performance
possible — a separate build or whatever is excellent.*

So two things must both hold. Inside Cantus nothing changes: the seven effects stay
native `FxUnit`s on the engine's audio thread, with no format layer and no new cost. And
outside it, Aequalis, Comprimo, Terminus, Spatium, Multiplex, Sibilus and Porta run in
any other host — Ableton Live, FL Studio, Reaper, Bitwig, Cubase, Studio One — as the
two open standards those hosts load: **CLAP** and **VST3**.

The effects' DSP is in `cantus-dsp`, their parameter tables in `cantus_core::fx`, and the
glue between them — the one reading of an `EffectSpec` into each DSP's settings — in
`cantus_engine::{effects, fx}` (ADR 0018). The export must reuse all three, not copy them:
a second mapping would drift, and the promise to the user is *the same effect*, which a
null test can hold only if it is the same code.

## Decisions

### 1. A separate crate, a separate build

`crates/cantus-plugins` (GPL-3.0-or-later, `cdylib` + `rlib`) wraps the engine's own
`EffectUnit` — constructed from an `EffectSlot`, configured from an `EffectSpec`,
processed with `process_keyed` — exactly as the engine does. It is a workspace member
but **not a default member**: `cargo build` / `cargo run` at the root build the DAW
exactly as before. Nothing in the DAW depends on it.

The build is its own cargo profile, `plugin`: `release` with `lto = "fat"`,
`codegen-units = 1`, `panic = "abort"`. A plug-in is a small hot loop called from
someone else's process; whole-program optimisation across `cantus-dsp`,
`cantus-engine` and the wrapper is what inlines the DSP into the host's call, and a
panic must never unwind into a host's C++ frames. `cargo run -p xtask -- bundle` builds
each plug-in **on its own** (`--no-default-features --features <name>`), so each binary
is one plug-in to a host; a development build lists all seven.

A second flavour, `cargo run -p xtask -- bundle --v3`, builds with
`-C target-cpu=x86-64-v3` (AVX2, FMA, BMI2 — every x86 CPU since Haswell, 2013) into
`target/bundled-x86-64-v3/`, in a target directory of its own. The flavours carry the
same ids, so a user installs one or the other. Rust never contracts `a * b + c` into an
FMA on its own, so the v3 flavour computes the same numbers (the measurement below
checks that it is only faster).

### 2. Our own thin wrappers, on `clap-sys` and `vst3`

The options weighed (the other session's standards research,
`research/instruments/standards.md` §4, was read for this decision):

| Option | Per-block path | Licence | Parameters | egui 0.36 editors | Verdict |
|---|---|---|---|---|---|
| **nih-plug** | its own smoothing and buffer layer | framework ISC; **VST3 output GPL-3** via `vst3-sys` | a derive on a struct, compile-time | `nih_plug_egui` pinned to egui 0.31 | no: maintenance mode, old egui, git-only |
| **nice-plug** (its maintained fork) | the same layer | ISC; VST3 on the `vst3` crate | the same derive: our 414-parameter EQ table would need a hand-written `Params` | `nice-plug-egui` on egui 0.36 | the fallback, if ours cost too much |
| **clack-plugin + clap-wrapper** | CLAP native; VST3 is the C++ wrapper translating every block's events and buffers into CLAP | MIT/Apache, MIT | runtime, CLAP-shaped | through baseview | a second translation layer per block for VST3, a C++ toolchain and the VST3 SDK sources in the build |
| **clap-sys + vst3, our own** | host call → our event read → `EffectUnit` | MIT/Apache both | runtime, from our tables | our choice (§8) | **chosen** |

Our own wrappers win on each criterion the user named:

- **Per-block overhead.** Between the host's `process` and the engine's unit there is
  one pass over the host's event list into a buffer made at activation, a copy of the
  input into the output when the host did not process in place, a copy of the input
  into the bypass line only when something will read it (a latent effect, a bypass
  fading, an open editor), and the MXCSR write. No per-sample smoothing layer (the DSPs
  glide on their own), no second translation for VST3. The measurement is below.
- **Maintenance.** Both standards are stable C ABIs (CLAP 1.x is ABI-stable by
  contract; VST3's interfaces never change once published). `clap-sys` 0.5 (CLAP 1.2.2)
  and `vst3` 0.3 (bindings generated from the VST 3.8 SDK headers, MIT since October
  2025) are thin, maintained, on crates.io and used by the frameworks above. The two format
  layers are about 2 500 lines with their GUI glue and the format-free core about
  1 100, all of it ours to read.
- **Licence.** `clap-sys` and `vst3` are MIT/Apache-2.0; the VST3 SDK is MIT. Nothing is
  GPL-only, nothing is a git dependency, no C++ is compiled. (`vst3-sys`, GPL-3 and
  unmaintained, is avoided.)
- **Parameters at run time.** Both standards ask for the list at run time; our tables are
  run-time data (Aequalis's is built from its band table). A derive-based framework
  wants them at compile time.
- **Editors.** We choose the windowing (§8) instead of inheriting a framework's egui pin.

The door to a framework stays open because the adapter is thin: `instance.rs` is the
whole format-free part, and the two format layers only translate calls.

### 3. Parameters: our tables, ids forever

Each plug-in's list is its table (`kind.params()`), in the table's order, with the
same keys, ranges, defaults and units, followed by two parameters of the exported
plug-ins' own:

- **Sidechain** (`cantus.sidechain`): whether the detector listens to the sidechain
  input. Every one of the seven has a stereo sidechain port (CLAP: a second input port;
  VST3: an aux input bus), but a CLAP host always passes every declared port, connected
  or not, so "is a key routed" — a slot property in Cantus (`EffectSlot::sidechain`) —
  must be the user's choice in another host, as it is in FabFilter's plug-ins.
- **Bypass** (`cantus.bypass`), flagged `CLAP_PARAM_IS_BYPASS` / `kIsBypass`, so a
  host's bypass button drives it. It is a **soft bypass that keeps the latency**: the
  input goes through a delay line as long as the effect's latency and is cross-faded in
  over 10 ms, and the effect keeps running underneath, so bypassing moves neither the
  timing nor the host's compensation and un-bypassing does not click. (ADR 0018 wanted
  this for the DAW's own bypass; it is here first.)

**The id rule:** a parameter's id is FNV-1a (32 bit) of its key's UTF-8 bytes, top bit
cleared (VST3 leaves the upper half to hosts) — the same number in both standards. Keys
are forever (ADR 0017), so ids are. `tests/golden/ids.txt` holds every id published:
each plug-in's CLAP id and VST3 class id and each parameter's id; the test fails if one
changes or disappears, and fails for a new key until it is appended
(`CANTUS_BLESS_IDS=1`). Another test proves no two parameters of a plug-in collide.

The plug-ins' ids: CLAP `com.github.lalmazan.cantus.<name>`; VST3 class ids are the first
16 bytes of SHA-256 of `cantus/vst3/<name>`, read as four big-endian words, a rule a test
holds.

**Names, units, groups and text.** Names are the DAW's labels from the same locale
files (`include_str!` of `cantus-ui/locales/…`); a banded parameter is named with its
band, counted from 1 as on screen (`Band 4 Gain`), and sits in a group (CLAP's `module`,
VST3's units through `IUnitInfo`): *Band n*, *Key EQ band n*, *Decay curve band n*, *Post
EQ band n*. Styles, modes and listen choices are lists with their names (`Classic`,
`Linear phase`, …) — `CLAP_PARAM_IS_ENUM`, VST3 `kIsList` — and on/off switches read
*Off*/*On*. Values print as the DAW's knobs print them: `readout` moved from
`cantus-ui`'s knob into `cantus_core::fx::readout` so both call one function, with a
parser beside it (`2k`, `1.5 s`, `-6`, `50%`). The language is the process's (`CANTUS_LANG`,
then `LANG`), English by default; hosts identify parameters by id, so a project moves
between languages untouched.

**The taper.** VST3 hosts see every parameter normalised through the DAW's knob taper
(logarithmic for frequencies, times and ratios). CLAP has no taper — a host maps the
plain range linearly onto its lanes, and a 20 Hz–20 kHz lane in hertz would put nearly
all of the audible range in its bottom tenth — so a **logarithmic, continuous parameter
is exposed to CLAP as its knob position (0…1)**, as nih-plug does for all of them; its
text still reads `1.2 kHz`. Linear and stepped parameters are plain values in their
units.

### 4. When a change takes effect: the DSPs' 16-sample grid

Every one of the seven DSPs updates coefficients, glides and detectors on a grid of 16
samples counted from the start of the stream (`aequalis::UPDATE`, `multiplex::UPDATE`,
`sibilus::UPDATE`…): that is what makes them independent of the block size. A parameter
change from the host — CLAP events, VST3 parameter queues, CLAP modulation — takes effect
**at the first grid point at or after its sample**. So a change lands on the same sample
whatever blocks the host cuts, the output is identical at any block size (the test
automates at block sizes 1 to 4096 and compares to the bit), and the unit is reconfigured
at most once per 16 samples however dense the automation. Sample-accurate splitting at
the event's own offset would be neither: the DSP would still only act on its grid, and a
host that sends a ramp one point per sample would reconfigure the EQ 512 times a block.

CLAP modulation (`CLAP_EVENT_PARAM_MOD`) is supported on every continuous parameter: an
offset in CLAP's units on top of the automated value, applied through the same grid.

### 5. State: `EffectSpec` JSON, both ways

A plug-in's state is the JSON of the `EffectSpec` a project stores for an insert —
`{"kind":"Gate","params":{…}}` — sparse for the kinds the DAW stores sparsely, with
`version`, `sidechain` and `bypass` beside the spec's fields, where `EffectSpec`'s
reader ignores them. So Cantus reads a plug-in's state as an `EffectSpec` as it is, the
plug-in reads Cantus's, and loading also takes a bare `{"key": value}` map — a user
preset saved in the DAW. Unknown keys are ignored and missing ones read their default:
the `EffectSpec` rule, so states move across versions. CLAP and VST3 write the same bytes.

### 6. Latency, and when it changes

Latency is the unit's (`EffectUnit::latency`), reported through CLAP's latency extension
and VST3's `getLatencySamples`. When a parameter changes it while running (Aequalis to
linear phase, a lookahead), CLAP's plug-in asks `request_restart()` from the audio thread
(the standard lets latency change only during `activate`) and calls `latency.changed()`
in the next `activate`. VST3 wants `restartComponent(kLatencyChanged)` on the UI thread
and gives a plug-in without an open editor no callback there, so each instance registers
with a thread timer (`SetTimer`, 20 Hz) on the thread that initialised it and compares
the effect's latency with what the host last read. On macOS and Linux that tick is the
next platform's work (`CFRunLoopTimer`, `IRunLoop`); until then a change there is seen
at the next activation. Tail: Spatium reports its decay (1.5 × the 60 dB time, plus the
pre-delay), infinite while frozen; the others report their latency.

### 7. The audio thread, and denormals

`process` allocates nothing and locks nothing: the event
buffers and the bypass line are made in `activate`/`setActive`, the `EffectSpec` holds
every key from the start so a change is a write, never an insertion, and the parameter
list's `BTreeMap` order is precomputed so writing all values is one pass with no lookup.
A counting allocator watches every plug-in's `process` with all of its parameters
automated inside the block: zero. One shared mapping was made fit for this:
`eq_bands` built each band's key with `format!` (408 allocations per configure, on the
engine's audio thread too) and now reads the table's `&'static str` keys, as
`comp_settings` already did. Nothing may panic either: under the `plugin` profile a
panic aborts the host's process, which is why clap-validator's fuzzing and the
every-sample-rate test matter (they found one, below).

Each block runs with FTZ and DAZ set (MXCSR bits 15 and 6; FPCR.FZ on AArch64) and the
host's mode restored after. The DSPs already flush their own state in software, so on
the test material the plug-in is bit-identical to the engine *without* the flags too.

### 8. Editors: one abstraction, two hosts

In another host the generic interface is what most people see first, so it is made
good before anything else: every parameter named as the DAW names it, its unit, its
group, its choices by name, and typed values understood (§3).

The goal is our **real editors** in the plug-in window. They took `&mut CantusApp`; what
an editor actually needs from its host is now a trait, `cantus_ui::views::fx::host::FxHost`:

| Need | Cantus (`DawHost`) | A plug-in in another DAW (`PluginHost`) |
|---|---|---|
| the parameters (`spec`) | the slot's `EffectSpec` | the plug-in's values, as an `EffectSpec` |
| `set(key, value, gesture)`, `set_many`, `end_gesture` | commands, a gesture merged into one undo step | CLAP gesture begin / value / end through a lock-free ring the audio thread drains into the effect and the host's output events (so the host records automation and undo); VST3's `beginEdit`/`performEdit`/`endEdit` |
| `telemetry`, `pairs` (the analyser tap) | `Meters::fx`, the engine's tap | the plug-in's own audio thread: 64 atomics, and a ring of (input, output) mono pairs made exactly as the engine's tap makes them, both only while the editor is open |
| `transport` | the song's position, tempo and metre | 120 BPM, stopped (the host's transport is the next step) |
| `key_source` / `set_key_source` | any strip of the desk | the plug-in's *Sidechain* switch (the host routes the input) |
| `status`, `notes_to_clip` / `makes_clips` | the status bar; Porta's hits become a MIDI clip on a new drum track | none: the button is not shown |
| the theme | `theme::p()` | the same palette and fonts installed on the window's egui context |

**The proof is Porta.** Its editor (`views::fx::gate`) was moved onto the trait whole:
`gate::editor(host, state, ui)` is the editor, and `gate::body` is the DAW's adapter —
it lends the editor state out of the app, builds a `DawHost`, and puts the state back —
so the DAW's editor and its tests (`crates/cantus-ui/tests/fx_*.rs`) are unchanged. In
the exported build (feature `editor`, on for Porta's bundle) the same function runs in
a CLAP plug-in window — and in a VST3 `IPlugView` — egui 0.36 through **egui-baseview
0.7.2** and **baseview 0.3.4** (both MIT OR Apache-2.0, crates.io, OpenGL), as a child
of the host's `HWND`, sized from the system's DPI (the device frame's designed size for the
kind, in logical pixels — 1060 × 720 before round 18), repainting
at about 30 frames a second while the meters move. The editor's words follow the same
language rule as the parameters. Porta's binary grows from about 1 MB to 6.7 MB with it,
because the editor still lives in `cantus-ui` with the rest of the DAW's interface (step
5 below).

**The plan for the other six** (written with Porta), and where it stands after round 18:

1. *(Done in round 18.)* Terminus, Sibilus, Multiplex and Spatium moved onto the trait
   the way Porta did (`app, strip, effect` → `host`, `app.fx.<kind>` → the editor's own
   state, a `framed` adapter for the DAW and an `editor` entry for any host).
2. *(Done in round 18.)* Comprimo and Aequalis too, with the few things only the engine
   reports (the EQ's dynamic deltas, the compressor's meter channel) as provided methods
   of `FxHost` that a plug-in host may leave empty; what only Cantus can do (a track's
   notes, another strip's spectrum) goes through `FxHost::daw`, `None` in a plug-in window,
   where that part is hidden. The seven share one entry, `views::fx::Editor` (`of(kind)`,
   `show(host, ui)`), and `cantus_plugins::editor::has_editor` reads it.
3. *(Done with Porta.)* The VST3 view is an `IPlugView` (`vst3/view.rs`) whose
   `attached(parent, "HWND")` opens the same `Editor` with a sink that calls the
   component handler (`beginEdit`/`performEdit`/`endEdit`); a new kind's editor needs
   nothing here.
4. What a plug-in host adds. *(Done in round 18:)* every editor now draws the device
   frame of ADR 0024 §8 — its header is the plug-in window's title bar — so presets (the
   factory's) and A/B live inside the plug-in (`PluginHost` keeps the other side and sends
   a switch as the standards' gestures), and the bypass lamp drives the standards' bypass
   parameter; the window opens at the frame's designed size for the kind (1040 × 640 for
   Aequalis and Multiplex, 1000 × 640 for the others). *(Still to do:)* the host's
   transport (CLAP's transport event, VST3's `ProcessContext`) into `FxHost::transport`;
   resizing (`can_resize`, CLAP's `set_scale`) so the frame's size menu can grow the host's
   window — today it scales the content inside a window of fixed size.
5. When the editors leave `cantus-ui` for a crate of their own (`cantus-fx-ui`: editors,
   widgets, theme — the other session's standards research proposes the same), the
   exported build stops compiling the rest of the DAW's interface.

## What the tests hold (`crates/cantus-plugins/tests/`)

A test host (`tests/common`) drives each plug-in through the library's own
`clap_entry` exactly as a host that loaded the binary does — factory, function table,
extensions, `clap_process` with separate or in-place buffers, event lists, streams that
read and write a few bytes at a time — and `tests/vst3.rs` does the same through
`GetPluginFactory` with a memory `IBStream` and `IParameterChanges` of its own.

- **Bit-identical, every plug-in** (`clap.rs`): through CLAP, with non-default settings
  that make each effect work, the output equals `cantus-dsp` called directly with the
  same settings (the DSP types themselves, set through the engine's mapping) to the bit,
  under the same denormal mode — and equals the engine's `EffectUnit` *without* FTZ/DAZ
  too, so the plug-in nulls against the DAW. Each effect changes more than a tenth of the
  samples, so no comparison is of two wires. At the defaults as well. Through VST3
  (`vst3.rs`), the same, with the state set through `IComponent::setState`.
- **Any block size**: blocks of 1, 7, 64, 1000, 4096 and a mixed sequence give output
  identical to the bit, for all seven.
- **Automation on the grid**: three changes of a parameter that matters to each plug-in
  (off the grid, on it, near a block's end) give output identical to the DSP set anew at
  the first 16-sample grid point at or after each change, at six block patterns; the
  changes audibly change the sound. VST3 automation lands on the same grid.
- **Ids never change** (`ids.rs`): 833 lines — each plug-in's CLAP id and VST3 class id
  and every parameter id — against `tests/golden/ids.txt`; no two parameters of a plug-in
  share an id; the FNV-1a rule and the SHA-256 class-id rule are recomputed.
- **State**: saved state parses as Cantus's `EffectSpec` and equals the DAW's spec for
  the same settings; the DAW's `EffectSpec` JSON and a DAW user preset (a bare map) load
  into the plug-in; a state round-trips to the same values and the same sound; another
  kind's state is refused; CLAP and VST3 write the same bytes.
- **Latency**: reported equal to the engine unit's for every plug-in; Aequalis switched
  to linear phase while running asks the host for one restart, and the next activation
  reports the new latency and calls `latency.changed()`. Spatium's tail follows its decay.
- **The sidechain** keys the detector only when *Sidechain* is on, bit-identical to the
  DSP given the key; off, the host's key buffers are ignored.
- **The bypass** is the input delayed by the latency to the bit (Terminus, with
  lookahead), and for an effect with no latency fades to the input and back to the
  running effect, exact outside the 10 ms fades.
- **The audio thread** (`alloc.rs`): a counting allocator sees zero allocations in
  `process` for all seven with every parameter automated inside the block, a sidechain
  connected.
- **Parameters**: count, ids, ranges and defaults are the tables'; every default's text
  parses back to it; what the DAW prints the plug-in prints (`-40.0 dB`, `2.0 kHz`,
  style names); every word a host is shown exists in English and Spanish (`text.rs`).
- **The editor** (`editor.rs`): the plug-in host's gestures reach the host as begin,
  values, end (a drag one gesture, a click its own, a learned pair two); values read back
  at once; the key is the *Sidechain* switch; DAW-only actions are not offered; and
  Porta's real editor draws against the plug-in host, its history and key spectrum fed
  from the plug-in's own audio thread. `examples/editor_window.rs` is a small Win32 CLAP
  host that embeds the editor through the GUI extension and captures the window.
- In the DAW, `crates/cantus-ui/tests/fx_gate.rs` and `fx_frame.rs` (and the other
  `fx_*` tests) pass unchanged over Porta's editor on `FxHost`.

## Performance

`cargo run -p cantus-plugins --example bench --profile plugin --no-default-features
--features all` runs each plug-in four ways on the same 512-sample stereo blocks at
48 kHz, with the settings the tests use (every effect working): the engine's
`EffectUnit::process_keyed` as Cantus calls it (**engine**); the same plug-in through the
bench binary's own `clap_entry` — events, grid, bypass line, FTZ, the host's buffers, the
sidechain port connected (**clap**); and the bundled binaries themselves, loaded with
`LoadLibrary` as a host loads them (**exported**, and the **x86-64-v3** flavour). The
ways take turns block by block, a different one first each block, over 6 000 blocks, and
each figure is the median, because this machine was running four other agents' builds
at the time: a busy machine's slow stretches fall on every way alike and the median
ignores the blocks an interruption lands in. `=` marks a way whose output was the
engine's to the bit in every block.

| Plug-in | engine | engine + FTZ | clap | wrapper | exported | x86-64-v3 |
|---|---:|---:|---:|---:|---:|---:|
| Aequalis | 45.6 | 45.5 = | 45.8 = | +0.7 % | 45.8 = | 44.5 = |
| Comprimo | 94.4 | 93.9 = | 94.7 = | +0.9 % | 95.1 = | 95.5 = |
| Terminus | 544.2 | 568.6 = | 571.3 = | +0.5 % | 606.3 = | 483.7 = |
| Spatium | 648.4 | 647.0 = | 649.9 = | +0.4 % | 653.5 = | 482.5 = |
| Multiplex | 267.6 | 266.4 = | 267.4 = | +0.4 % | 271.5 = | 254.0 = |
| Sibilus | 211.3 | 198.8 = | 200.4 = | +0.8 % | 200.4 = | 191.1 = |
| Porta | 34.7 | 34.3 = | 35.0 = | +2.0 % | 39.6 = | 38.8 = |

µs per 512-sample stereo block (10 667 µs of audio at 48 kHz), median of 6 000.
*engine + FTZ* is the engine's unit run with the plug-in's denormal mode, the fair
baseline for the wrapper: *wrapper* is *clap* against it. `=` is the engine's output to
the bit in every one of the 6 200 blocks.

**The wrapper costs under a microsecond a block** — 0.4 to 0.9 % of the six heavier
effects, 0.7 µs on Porta's 35 µs — which is at the edge of what this machine, busy with
four other builds, could resolve (runs a few minutes apart moved single medians by up to
±5 %). Per block the wrapper adds one pass over an (usually empty) event list, a few
atomic loads, the MXCSR write and restore, and no copy at all when the host processes in
place and nothing needs the bypass line. Every way was the engine's output to the bit in
every block, the separately linked binaries and the v3 flavour included.

The exported binaries run as the in-process build does, within the noise, with one
exception: Porta's shipped binary, which also carries its editor (6.7 MB), is about
4 µs a block slower than the same code in the bench — code layout in the larger image;
0.04 % of the block's time. The **x86-64-v3** flavour is faster where the DSP is wide —
Spatium by 25 %, Terminus by 15 %, Multiplex and Sibilus by 5 % — and computes the same
numbers. FTZ/DAZ themselves speed Sibilus up by 6 % on this material without changing
a bit of its output: **the DAW's audio thread would gain from setting them too** (a
one-line change in the engine's callback, for the engine's owner to take).

## Bundles and installation

`cargo run -p xtask -- bundle` (all seven; or name some: `bundle porta terminus`) builds
each plug-in with the `plugin` profile and lays out, in `target/bundled/`:

```text
Cantus Porta.clap                                   the library, renamed
Cantus Porta.vst3/Contents/x86_64-win/Cantus Porta.vst3
Cantus Porta.vst3/Contents/Resources/moduleinfo.json
```

`--v3` builds the x86-64-v3 flavour into `target/bundled-x86-64-v3/` (its own target
directory); `--no-build` only lays out what is built. `moduleinfo.json` is written from
the same descriptors the factory reads, so a VST 3.7.5+ host lists the plug-in without
loading it. A full bundle of the seven takes about five minutes on this machine (seven
fat-LTO links); Porta's, with its editor, the longest. Each binary is about 1 MB: the
engine's `EffectUnit` dispatch reaches every kind, so every binary carries the family's
DSP code, and lists only its own plug-in. (A per-kind dispatch would slim them to one
DSP each; it would duplicate the engine's glue, so it waits for a reason.)

**Installing on Windows.** Copy `Cantus <Name>.clap` into
`C:\Program Files\Common Files\CLAP` (all users) or `%LOCALAPPDATA%\Programs\Common\CLAP`
(this user), and the whole `Cantus <Name>.vst3` folder into
`C:\Program Files\Common Files\VST3` (all users) or `%LOCALAPPDATA%\Programs\Common\VST3`.
Then rescan in the host (Reaper: *Options → Preferences → Plug-ins → Re-scan*; Bitwig
and Studio One scan at start; FL Studio: *Plugin manager → Find plugins*; Ableton Live
12: *Settings → Plug-Ins → Rescan*, VST3 folder on; Cubase: *Studio → VST Plug-in
Manager*). One flavour at a time: the v3 build has the same ids.

**macOS and Linux, for later** (the bundler already writes these shapes; neither build
has been run on its system yet, and the latency tick and the editor are Windows-only so
far, §6 and §8):

```text
macOS   Cantus Porta.clap/Contents/{Info.plist, PkgInfo, MacOS/Cantus Porta}
        Cantus Porta.vst3/Contents/{Info.plist, PkgInfo, MacOS/Cantus Porta, Resources/moduleinfo.json}
        → ~/Library/Audio/Plug-Ins/CLAP, ~/Library/Audio/Plug-Ins/VST3 (signing and
          notarisation are a release step of their own)
Linux   Cantus Porta.clap                                          (the .so, renamed)
        Cantus Porta.vst3/Contents/x86_64-linux/Cantus Porta.so
        → ~/.clap, ~/.vst3
```

**Validation.** `clap-validator` (MIT, free-audio) was built from source at a pinned
revision — `cargo install --git https://github.com/free-audio/clap-validator --rev
b2f1d9b79b1d264a5747f46707d72b1aa40a02ef --locked` — a tool, not a dependency — and run on
every bundle. **Result: all seven pass every test that applies** — 31 of 44 each, none
failed; 12 do not apply (preset discovery, 64-bit buffers, note ports, the optional
audio-port configuration extensions, and Unix's `RTLD_NOW` scan); the one warning is
the scan time (150–230 ms for the effects, 580 ms for Porta with its editor, against the
tool's 100 ms guideline, which it says depends on the system; this machine runs a
realtime antivirus scan of every DLL load). Its first run found two real faults, both
fixed with a test of their own: a state load must ask the host to rescan values
(`rescan(CLAP_PARAM_RESCAN_VALUES)`), and Sibilus panicked below about 2.2 kHz, where
its band edges were clamped with a minimum above the maximum (`cantus-dsp`'s
`sibilus::band_edges` now collapses the band instead); every plug-in now runs at the
tool's thirteen rates, 1 234.5678 Hz to 768 kHz. Steinberg's `validator` is the VST3 counterpart; it is
C++ built from the (MIT) VST3 SDK with CMake, which this machine's round did not build.
It is optional: `cmake -B build vst3sdk && cmake --build build --target validator`, then
`validator "Cantus Porta.vst3"`.

## Consequences

- New: `crates/cantus-plugins` (the wrappers, `instance`, `params`, `state`, `text`,
  `editor`), `xtask` (the bundler), `cantus_core::fx::readout` (the DAW's readout and a
  parser, one place), `cantus_ui::views::fx::host` (`FxHost`, `DawHost`), the
  `plugin` cargo profile, `tests/golden/ids.txt`.
- Changed, small: the knob's `readout` forwards to `cantus_core`; `eq_bands` reads the
  table's static keys (no allocation on the engine's audio thread either); Porta's editor
  (`views::fx::gate`) is on `FxHost`, with `body` as the DAW's adapter.
- The DAW's build and behaviour are unchanged: `cantus-plugins` and `xtask` are not
  default members, and nothing in the DAW depends on them.
- **Every table key is now also a published id.** A key renamed in `cantus_core::fx`
  breaks automation in other hosts too; `ids.rs` fails the moment one changes. New keys
  are added with `CANTUS_BLESS_IDS=1`.
- **The DSP is shared, so a change to it changes the plug-ins.** The bit-identity tests
  keep the plug-ins equal to the engine; they do not freeze the sound.
- **For the merge:** the orchestrator will load these CLAP builds in Cantus's own CLAP
  host (`cantus-host`) and null them against the native inserts. What to expect: the
  plug-in is bit-identical to `EffectUnit` for the same `EffectSpec` (its state *is*
  that spec: load the insert's spec JSON with `state.load`), with *Sidechain* off unless
  the host routes a key, bypass off, and parameter changes only at block starts or on
  16-sample grid points — changes the DAW makes per snapshot land at the next grid point
  in the plug-in, so a null test with automation should compare from a grid point on.
  The plug-in runs with FTZ/DAZ; the engine does not; on material that leaves denormals
  in a DSP state the two can differ below −750 dBFS (the smallest normal float). Parameter ids are FNV-1a of the keys;
  logarithmic parameters travel to CLAP as knob positions (0…1), not in Hz.
- Not done, and why: the other six editors (the plan in §8 — mechanical, one file each);
  the host's transport into the editor; resizable editors; macOS and Linux builds (the
  bundler writes their shapes; the latency tick and the editor window need their run
  loops); CLAP's preset-discovery for factory presets; `.vstpreset` files; the VST3
  validator (needs the SDK's CMake build, not done on this machine).
