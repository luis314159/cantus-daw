# Pulsus II — the drum instrument

*Round 17 (ADR 0023). Research: `research/instruments/drums.md`. Pulsus grows in place: the
channel rack, the piano roll's lanes and every existing kit are built on it, so there is no
native kind — the typed model (`cantus_core::pads`) got the new fields, the DSP
(`cantus_dsp::pulsus`) the new voices, and old projects sound exactly as they did.*

## What was built

**Identity.** Every pad has a `PadId` (u16, never reused in its kit; assigned in pitch order
when an old file is read). `InstrumentParam::Pulsus { pad: PadId, field }`, `SetPadName`,
`SetPadSample`, `RemovePad` and the new `SetPadLayers` name the pad, not its row, so
inserting a lower pad no longer retargets an edit, an undo step or (later) an automation
lane. `Lanes::pad_id` / `lane_of_pad` let a view follow a pad; the editor's selection is a
`PadId`.

**Model versions.** A kit carries `version`. A kit with none in its file (everything
Pulsus I wrote) reads as **model 1** and renders **bit-identically** to Pulsus I — held by
three SHA-256 goldens recorded before the first line of Pulsus II (a busy kit using every
voice, a choke group, pans, drive and a sample; the same rendered from bar 2; and the v14
fixture's kit), plus every existing golden and null test. New kits are **model 2**. Model 2
differs from model 1 in exactly three ways:

1. **Noise and every choice a hit makes are seeded from the note** — `hash(kit seed, PadId,
   note tick, note id)` — not from a per-pad hit count that a seek resets. A render that
   starts at bar 5 equals bars 5… of a render from the top (test below). Round robin uses
   the note's **ordinal** (its count among its pitch's note-ons on the track, computed in
   `Snapshot::compile`), so *Cycle* survives a seek too.
2. **The kick is tuned to a note**: Tune 0 is F1 (43.654 Hz) instead of 45 Hz (52 cents
   sharp). The display shows the tail as `43.7 Hz = F1 +0 ct`.
3. **Drive reaches every voice** (model 1 kept it on the kick), after the voice and 2×
   oversampled, instead of inside the kick unoversampled.

Upgrading is one command (the editor's *Upgrade to Pulsus II*): the version, plus taking
back the drive Pulsus I stored on every non-kick pad and never used. One undo step.

Every Pulsus II field is `#[serde(default)]` at the value that changes no sample, so a model
1 kit can use the whole new section without being upgraded. No schema bump (the
coordinator's).

**Voices** (`PadVoice` gains `Clap` and `Tom`; a stepped `model` field per voice):

| Voice | Models | How |
|---|---|---|
| Kick | Pulsus · **808** · **909** | Pulsus I's two-stage sine; 808: a damped sine (bridged-T behaviour) with a small sweep, the *sigh* (pitch sagging with amplitude², so the tail lands on the note), a low-passed pulse click; 909: a saw-shaped near-sine (`tanh(1.6 sin)`), a deep exponential fall (Tune depth in st), a click of low-passed noise and a pulse on separate envelopes |
| Snare | Pulsus · **Modal** | Pulsus I's two bodies and noise; Modal: 12 membrane modes (Bessel zeros, strike position by J_m(j·ρ), hardness as a half-sine contact spectrum), **wires whose level follows the head's rectified motion** after a 1.5 ms onset, and the snappy tail |
| Metal | Hat · **808 cymbal** · **808 cowbell** · **Modal cymbal** | the six squares through BP → HP; the cymbal: the same six through **two bands** (≈0.48× and 1× tone), the high band on a short and a long decay (three decays with the low band's), then the HP; cowbell: 540/800 Hz squares, BP, a 12 ms first decay into the tail; modal cymbal: 32 dense inharmonic modes and HP'd noise whose envelope **swells** (smoothstep over *Swell* ms) before it decays |
| Clap | — | band-passed noise re-triggered *Hands* times *Spacing* ms apart (±15 % seeded jitter), then a room tail on the same band |
| Tom / perc | modal, five materials | membrane (with *Shell/Air* pulling the modes towards harmonic), bar (1 : 2.756 : 5.404 …), wood, plate, bell; 4–32 modes, damping τₖ = τ₀·(f₀/fₖ)^γ, position, hardness, **tension glide** (all modes × (1 + g·E), E the energy) updated on a 16-sample grid from the hit |
| Sample | — | the pad's file, or up to **16 layers** |

**Sample layers** (`DrumPad::layers`): file, velocity low/high, cross-fade width, tune,
level, pan, start (ms), reverse. Layers with the same velocity range are a zone; adjacent
zones cross-fade **equal-power**, centred on their boundary. The pad's round robin picks
within a zone: *Stack* (all), *Cycle*, *Random*, *No repeat* — all computed from the hit's
ordinal and the kit's seed, never from history, so a bounce equals playback. Layers ride
along on synthesized pads too (a synth body with a sampled click). Model 2 interpolates
samples with 4-point Hermite; model 1 keeps linear.

**The section, on every voice**: amplitude attack (0–100 ms) and hold (0–500 ms) before the
decay, decay curve (−1 faster … 0 exponential … 1 linear); a pitch envelope for the voices
without a sweep of their own (±48 st, 5–400 ms); filter off/LP/HP/BP with cutoff,
resonance and an envelope amount in semitones (updated on an 8-sample grid from the hit);
transient attack and sustain (±24 dB over the first 8 ms → 20 ms, a soft ceiling on
boosts); drive soft/tape/clip (2× oversampled, polyphase half-band) or raw (not, the
Microtonic grit); start phase for the kick; velocity → level 0–200 % (100 % = Pulsus I's
v^1.5), → tone (50 % = Pulsus I's brightness law), → pitch (st), → decay (%).

**Chokes**: 16 groups; each choked voice fades over **its own pad's fade time** (1–250 ms,
default 4 ms — Pulsus I's); **polyphony per pad** (1–8, 0 = no limit; 1 is a self-choke,
oldest first, a hit's layers going with it); **choke on note-off**.

**Hi-hat pedal**: the kit's controller (CC4 by default, number and reversal per kit). A
metal pad with *Follows the pedal* opens continuously with it — its decay slides
logarithmically from closed (Decay) to open (Open decay) and its band drops up to 3 st, on
ringing voices as well as new hits; a pedal falling from ≥ 0.35 to < 0.1 chokes the ringing
hats and plays the **chick** (a short closed hit, its velocity from how far the pedal
fell). A plain CC from live MIDI now reaches Pulsus (it reached only native instruments).

**Voice pool**: 48 (model 1 keeps Pulsus I's 24 so it steals exactly as it did).

**Factory kits** (`PulsusParams::factory`): Pulsus (classic), Studio (every voice on the GM
map), 808, 909, Boom bap, Electro, Percussion, Trap — tagged, each a real kit.

**The editor** (`views/pulsus_editor*.rs`), opened from the inspector's *Open Pulsus
editor*. Since round 18 it sits in the **kit's device shell** (ADR 0024 §8), like the native
instruments though it is not one: the 28-pt header is the title bar (name in the source
hue, the factory kit as the preset well with ‹ ›, undo/redo, Play | Edit, ⋯ — add a pad of
any voice, close — and the size menu), the footer holds the help line, the *n pads* and
model chips and the output meter, and the window scales 75–200 %. The body is display
first — the **Pads** card (the grid as its display; + Pad ▾, the model and *Upgrade*
under it) beside the **Hit** card — then cards of kit cells: Voice (voice and model as
menu fields with ‹ ›, tune, decay, level, pan), Character (the model's own fields; a
tom's material, a sample pad's file), Envelope, Filter (type as the card's selector; its
cells dimmed, not hidden, while Off), Shape (drive mode as selector), Velocity (the law
drawn), Choke and voices, Hi-hat pedal, Layers (a table of wells you drag), Kit (seed,
pedal CC, reversed pedal). Play shows the displays with Voice, Character, Envelope,
Filter and Choke; Edit everything, scrolling. The **pad grid**: lowest pad bottom-left, each cell with its name, note, voice
and model, its **rendered hit** (synth pads too — the preview runs the real DSP), its
**choke group as a lettered chip, the group's pads joined by a link**, its output badge, a
struck-through name when muted; click to select and hear (higher on the cell is harder).
The **display** draws the selected pad's hit (waveform line over its envelope), its
amplitude envelope, and its pitch on a log-Hz axis (Kick 3's idea, for every voice that has
a pitch), with the tail's note and cents; A/H/D, the pitch curve's start and its tail are
handles (a drag is one undo step). The **section** below shows only what the voice and model
use, in real units; the layers list edits zones, cross-fades, tune and level (each drag one
step). Cell edges use `text_dim` (≥ 3:1 in the three themes; a test holds it).

## The hook the coordinator needs (per-pad outputs)

`DrumPad::output: Option<StripId>` is in the model (serialised only when set) and the grid
shows it. The DSP side is done: each pad has a `bus: u8` (`PadSettings::bus`) and
`Pulsus::process_add_bus(Some(bus), l, r)` renders only that bus's voices, each voice
exactly once per segment when every bus is called; `process_add` is `None` (all). What the
processor needs:

1. In `pulsus_kit::configure`, map each pad's `output` to a bus number 1…n (0 = the kit's
   strip) — today it passes 0 for every pad — and give the processor the table
   `bus → strip slot`.
2. In the track render, call `drums.process_add_bus(Some(0), …)` into the track buffer as
   now, and for each bus ≥ 1 `process_add_bus(Some(b), …)` into a per-pad scratch buffer
   that is added into that strip's buffer before the strips run (up to 16 preallocated
   stereo buffers per slot, `MAX_PAD_OUTS` in the report).
3. The routing null test (report §9.10) then needs the sum order considered: the buses sum
   in a different order from one buffer (`the_buses_together_are_the_kit` agrees to 1e-6,
   not bit-exactly).

## Parameters

Per pad (`PadField`, the name the command bus uses). Pulsus I's fifteen keep their names;
ranges depend on the voice (`PadField::info`), and `PadField::applies(pad)` says which a
voice and model use.

| Group | Fields (unit, default) |
|---|---|
| Identity | Voice, Model, Mute |
| Mix | Level (dB, 0), Pan (L–R, C) |
| Pitch | Tune (st, 0; Tom −36…+48), Pitch drop / sweep / 909 depth (st), Pitch fall (s), Start phase (°, 0), Pitch env (st, 0), Pitch time (ms, 50) |
| Envelope | Attack (ms, 0), Hold (ms, 0), Decay (s, per voice), Curve (0) |
| Character | Click, Sigh (808), Noise decay (s), Tone (Hz), Body/noise, High-pass (Hz), Wires, Material, Modes (12), Damping, Position, Hardness, Glide (st, 1), Shell/air, Band mix, Swell (ms, 40), Hands (4), Spacing (ms, 10), Q (2) |
| Filter | Filter (off), Cutoff (Hz, 1 000), Resonance (0.1), Env → cutoff (st, 0) |
| Shape | Transient (dB, 0), Sustain (dB, 0), Drive (kick 0.2, others 0), Drive mode (soft) |
| Velocity | → level (%, 100), → tone (%, 50), → pitch (st, 0), → decay (%, 0) |
| Voices | Choke group (0–16), Choke fade (ms, 4), Voices (0 = ∞), Choke on note-off |
| Layers | Round robin (stack); per layer: velocity lo/hi, X-fade, tune (st), level (dB), pan, start (ms), reverse |
| Pedal | Follows the pedal, Openness (0), Open decay (s, 1.2) |
| Kit (`KitField`) | Model (1/2), Seed (1), Pedal CC (4), Reversed pedal |

## What the tests hold

`crates/cantus-engine/tests/inst_pulsus.rs`
- **A Pulsus I kit renders exactly as it did**: three SHA-256 goldens recorded from the
  unchanged Pulsus (commit e20e3f4) — a busy four-bar kit that uses every Pulsus I voice,
  a choke group shared by a sample and a hat, pans, drive and a 44.1 kHz stereo sample; the
  same rendered from bar 2 (so the hit-count seed is pinned too); the v14 fixture's kit
  through the file loader. `a_pre_strip_project_renders_exactly_as_it_did` and every null
  test still pass.
- **Seek**: the studio kit with round-robin layers (No repeat and Random), eight bars —
  rendered from bar 5, it equals bars 5–8 of the render from the top.
- **Headroom**: the default kit with every pad at once at 127 below 0.95; the studio groove
  below 0.95; every pad of every factory kit alone below 0.6 and above 0.02.
- **Defaults change nothing**: a model 2 kit serialised and read back renders identically;
  a filter set on one pad changes the render.
- **The audio thread never allocates**: a counting global allocator around configure,
  400 blocks of hits on every pad (layers, round robin, the pedal, chokes, drive).

`crates/cantus-dsp/src/pulsus_tests.rs` (Pulsus I's own tests are kept as they were in
`pulsus_legacy_tests.rs`, and pass on the model 1 path)
- every model and material sounds, stays under 0.9 alone, and reaches **exact zero**;
- every model with the whole section switched on somewhere is **identical at blocks
  1, 7, 64, 480, 1000, 4096 and 8192**, tolerance 0;
- deterministic, and a different seed changes the render; a model 2 hit sounds the same
  with or without the hits before it (model 1 still does not — that is its golden);
- Cycle visits every layer in order; **No repeat never repeats over 10 000 hits** for 2–16
  layers and four seeds; Random is uniform (χ² over 12 000 hits of 6 layers < 20.5);
  cycled layers come back in turn through the whole voice path;
- equal-power velocity cross-fades hold the level within ±1 dB across the zone;
- a choke with fade F is ≥ 60 dB down by F and not by F/2, for F = 1, 4, 50, 250 ms, on an
  envelope voice and a modal one; polyphony 1 is a self-choke; choke on note-off;
- the pedal opens a hat monotonically, a reversed pedal at 0 equals a normal one at 1; a
  CC ramp closing on a ringing hat chokes it with a chick, and renders identically at every
  block size (the CCs split blocks as the processor does);
- **tuning**: every kick model at Tune −12, −5, 0, +7, +12 lands within ±2 cents of F1
  transposed (model 1's kick measures +52 cents, as it always did); a modal tom's
  fundamental within ±5 cents for all five materials at three tunings;
- full drive lifts a kick's tail by at most +12 dB in every mode; the 2× drive's alias
  floor is at least 30 dB below the unoversampled curve's (soft, tape; 10 dB for clip);
- the buses together are the kit (1e-6), the per-pad-output hook;
- a CPU ceiling: 48 of the heaviest voices (32-mode toms and cymbals, 2× drive) for 2 s.

`crates/cantus-core/src/pads_tests.rs`: every field of every voice round-trips and clamps;
a blank pad sits on every declared default; every factory kit stays inside its ranges; a
kit as Pulsus I wrote it reads as model 1 with ids in pitch order and round-trips as model
1; ids are stable under an insert and repaired when duplicated; zones cross-fade at equal
power; kit fields clamp; the upgrade takes back only the unused drive. The undo fuzz walks
`PadId` addressing, `SetPadLayers` and the kit fields.

`crates/cantus-ui/tests/inst_pulsus.rs`: the editor opens in Play and Edit and paints its
name, its pads and the kick's note (F1); the upgrade, a knob gesture, a layer edit and a
factory kit are one undo step each and come back exactly; the selection follows a pad when
a lower one is inserted, and an edit still lands on it; the grid's edges, chips, names and
captions pass 3:1 / 4.5:1 in Slate, Paper, Parchment and Ink & Gold (on the kit's tokens);
every word fits its box in Play and Edit at 75 %, 100 % and 200 % and in Spanish (no end
elided, none cut by its clip, no two overlapping); a real pointer drag on a cell is one undo
step. `CANTUS_INST_SHOTS=<dir>` writes `pulsus-{play,edit}-{slate,paper}.png`.

## Measured (48 kHz, this machine, shared by a dozen builds)

| What | Number |
|---|---|
| Model 2 kick tail, Tune 0 | **43.653 Hz = F1, −0.00 ct** (Pulsus, 808 and 909 models; ≤ 0.03 ct at −12, −5, +7, +12 st) |
| Model 1 kick tail, Tune 0 | 45.000 Hz, **+52.6 ct** from F1 — kept, as the golden requires |
| Modal tom fundamental | +1.75 ct at every material and tuning (the tension glide still settling one second in) |
| Render from bar 5 vs bars 5–8 of a full render (noise, Random and No-repeat round robin, sample layers) | **max difference 0** — bit-identical |
| Round robin | Cycle exact; No repeat: 0 repeats in 10 000 hits × 15 layer counts × 4 seeds; Random: χ² 6 layers / 12 000 hits < 20.5 |
| Loudest single pad, full velocity | 0.515 (808 kick, −5.8 dBFS); Pulsus kick 0.497 (−6.1 dBFS) |
| Default kit, every pad at once at 127 | < 0.95 |
| Studio groove with sample layers | 0.917 peak |
| Eight different voices on one sample at 127 | 1.15 (float, not a groove; held below 1.2 so a hotter kit shows up) |
| Drive alias floor, 4.99 kHz tone driven fully | soft **−63.2 dB** (−26.8 unoversampled), tape −61.3 (−30.4), clip −36.6 (−22.8) |
| CPU, the heaviest voices (32-mode tom/cymbal + 2× drive) | 48 voices, 2 s of audio in 2.19 s: **2.3 % of one core per voice**, 5.9× Pulsus I's kick and snare; a Pulsus/808/909 voice costs about what Pulsus I's did |

## Not done, and why

- **Per-pad outputs to strips**: the model field, the grid badge and the DSP buses are in;
  the processor work is the coordinator's (the hook above).
- **Schema bump and fixture**: not bumped, as instructed; the coordinator's at merge (the
  new fields are all `#[serde(default)]`, and a model 1 kit round-trips as model 1).
- **Drag a file from the OS onto a pad**, **user kit presets as files**, and the shared
  preset browser: the factory kits load from the header; saving a kit is not built.
- **Layer limits and smoothing** (SD3), **kill Quietest** (polyphony takes the oldest),
  **the pedal choosing a sample hat's openness layer** (the pedal drives synthesized metal
  only), a drawable velocity curve (the curve is drawn from the sensitivity, not edited).
- **Humanise, roles, morph/snapshots, key-aware tuning, grooves, similarity, AI kit
  building** — P1/P2 of the report, not started.
- **808 metal accuracy**: the six ratios are still the commonly quoted ones; the report's
  doubt about Werner, Abel & Smith (ICMC 2014) was not resolved here.
- **The rack's pad rows** do not yet show the choke chip and output badge; the rack's
  selection is still an index (commands and the editor follow identity).
- **Automation of pad fields** has no lanes yet; the addresses are now stable for when it
  does.
- Sound: I cannot listen. The numbers say plausible (tuning exact, headroom kept, click-
  free starts); ears must say good.
