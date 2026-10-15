# Exemplar — the multisample sampler

`InstrumentKind::Sampler`. ADR 0023; research in `research/instruments/sampler.md`.

Exemplar plays **groups → zones → samples**: key × velocity ranges, round robin and random
choices drawn from the note itself, velocity and key crossfades, release samples, loops with
crossfades, exclusive classes — through a resampler that is **alias-free by default** at a
cost that does not grow with the transposition. It reads the formats sampler users own (WAV,
AIFF, FLAC, Ogg; SFZ, Decent Sampler, SoundFont 2/3) and writes SFZ, and every edit — an
import of a thousand zones included — is one undoable command in the project.

## Files

| Layer | Files |
|---|---|
| model | `crates/cantus-core/src/inst/sampler.rs` — keys, zone/group descriptors, `param` by pattern |
| DSP | `crates/cantus-dsp/src/exemplar.rs` + `exemplar_{resample,play,read,voice,engine,trigger,render,builtin,loop}.rs`, tests `exemplar_tests{,2}.rs` |
| engine | `crates/cantus-engine/src/inst/sampler.rs` (unit, mip-maps), `sampler_decode.rs` (AIFF/FLAC/Ogg), tests `crates/cantus-engine/tests/inst_sampler.rs` |
| editor | `crates/cantus-ui/src/views/inst/sampler.rs` + `sampler_{map,wave,presets,import,sfz,ds,sf2,xml}.rs`, tests `crates/cantus-ui/tests/inst_sampler.rs` |
| strings | `crates/cantus-ui/locales/inst/sampler.{en,es}.txt` |

Shared files touched (each in a `SHARED:` commit): `crates/cantus-engine/Cargo.toml` and
`Cargo.lock` (claxon, lewton), `crates/cantus-engine/src/samples.rs` (decoder dispatch and
the mip-map hook), `crates/cantus-ui/src/views/inst/frame.rs` (one line: the footer shows
Exemplar's memory).

## How it works

**The model is flat keys.** `z{n}.*` for zone *n* (< 1024), `g{m}.*` for group *m* (< 64),
plain keys for the instrument. `z{n}.sample` is the asset key (a path the sample cache
decodes); `z{n}.gen` names a built-in sound instead. A zone exists when it has one of the
two; only fields away from their default are stored. A group's envelope keys *inherit*:
stored, they override the instrument's; cleared, the group follows it again (SFZ's own
inheritance, which is how imports keep each group's release).

**The resampler** (`exemplar_resample.rs`, de Soras 2005): every sample is kept at octave
mip-map levels (level *k* = low-passed with a 120 dB half-band and decimated by 2^k). A
voice at ratio *r* reads the level whose local ratio lies in [1, 2) (hysteresis down to
0.9), interpolates it with a Kaiser-windowed sinc (256 phases, coefficients interpolated
between phases) at **twice the output rate**, and decimates with a linear-phase half-band
FIR (polyphase, on contiguous slices) whose stop band starts where anything left would
fold back above 20 kHz. The read position leads by the decimator's group delay, so output
and position line up; the decimator is primed on note-on. At a ratio of exactly 1 the voice
reads the file directly: **bit for bit**. Tiers: Draft (linear on the file, lo-fi on
purpose), Standard (16 taps, 80 dB), **High** (32 taps, 100 dB, default), Best (64 taps,
120 dB) — the same live and offline.

The mip-maps are built **off the audio thread**: `SampleCache::attach` calls
`inst::sampler::attach_mips`, which adds each zone's levels as extra assets
`z{n}.sample~k`, only as many as the zone needs (its highest key above its root, the bend
range and an octave of modulation margin, at 44.1 kHz or more), cached by the decoded
file's address and never evicted (ADR 0005's rule for `Arc`s the audio thread holds). A
sample looped end to end (a single cycle) is mip-mapped as a periodic signal.

**Loops** are a mapping from an unwrapped position: lap *j* of a forward loop reads
`i − j·len`. Crossfades are applied to the mapped signal tap by tap, so the interpolator
sees one continuous looped signal; releasing a sustain loop only says from which lap the
mapping stops wrapping, so the tail follows without rewriting audio already read.
Forward, ping-pong (no sample repeated or skipped at the turns), backward; continuous or
sustain; linear or equal-power crossfades.

**Notes** go through the shared `voices::Allocator` (poly with stealing, mono, legato); a
note starts one *layer* per matching zone (up to 256 layers in all). A stolen note's layers
fade over 5 ms. Zone choice: key, velocity, keyswitch, round-robin position, random range,
trigger (attack, first, legato; release and key-up at note-off). **Randomness is seeded
from the note**: the random number is a hash of the note's id and key (shared by every
zone, like SFZ's `lorand`), and the round-robin turn of a clip's note is its **ordinal** —
how many notes of its pitch come before it on the track — so turns cycle strictly
(1, 2, 3…, never a sample twice in a row) and a bounce, a freeze and a playback from any
point choose the same samples (round 28, `rr.order` *In order*; *By note*, a turn drawn
from the note's id, is how Exemplars saved before keep playing). Live input (one id per
key) cycles strictly with a per-key counter. Release samples lose
`rt_decay` dB per second held and can start at the level the note had reached. Exclusive
classes silence with a 5 ms fade (or a release). Everything steps on a 32-sample control
grid counted from each layer's own start, never from a block boundary.

**Zones a controller starts** (round 26, schema 24: `trig` = 5 with `cc`, `cclo`, `cchi`;
SFZ `on_loccN`/`on_hiccN`, a piano's pedal noises): when controller `cc` moves into
`cclo…cchi` from outside it, the zone starts on the next sample — at its root, full
velocity, one round-robin turn and one random number per trigger (counted since the last
stop), chokes first — owned by no note. It plays like a one-shot: a zone that loops is
held while the value stays in the range and released when it leaves; one that does not
plays to its end (SFZ players do the same), unless its class is silenced. The sustain pedal
reaches it as CC 64, the mod wheel as 1, expression as 11. The map leaves such zones out
(no key plays them); the zone card shows the controller and its range instead of the key
and velocity ranges. A value already in the range when the unit starts (0, for a range
that holds it) triggers nothing.

**Streaming from disk** (round 26, ADR 0050): with the engine streaming (a live engine), a
sample no zone loops or plays backwards, longer than its head, keeps only its head in memory
(16 384 frames at level 0, more when a zone starts later) and streams the rest in 4 096-frame
blocks from a cache file at its own exact depth (`i16`/`i24`/`f32`), decoded once; mip
levels are computed from level 0 by the reader. Each layer copies what its next control
step reads into a window of its own, so the resampler takes the very paths it takes in
memory: streamed playback equals in-memory playback to the bit. A late block in real time
fades the voice out over 5 ms and is counted (`EngineStats::stream_late`, the status line);
an offline render waits.

**The empty instrument** plays the built-in sine at −18 dB over the whole keyboard, so a
fresh Exemplar is never a silent track (and the contract tests exercise the real engine).

**Built-in sounds** (`exemplar_builtin.rs`) are computed at start-up — four band-limited
single cycles, noise, Karplus–Strong plucks at C2/C4/C6, a two-dynamic bell, a detuned
ensemble pad, a kick, three snares, two closed and an open hat, a key click, an organ and a
choir cycle — so the factory presets are real multisampled instruments while the licence
ruling on recorded content is open.

## Parameters

Instrument (listed by `params()`; the matrix targets are marked ◆):

| Key | Unit | Range | Default |
|---|---|---|---|
| `gain` ◆ | dB | −48…12 | 0 |
| `pan` ◆ | % | −100…100 | 0 |
| `transpose` ◆ | st | −48…48 | 0 |
| `tune` ◆ | ct | −100…100 | 0 |
| `vel.amp` | % | −100…100 | 100 (amplitude ∝ velocity², SFZ's default) |
| `voices` | notes | 1…64 | 32 |
| `mode` | poly / mono / legato | 0…2 | poly |
| `glide` | ms | 0…5000 | 0 |
| `bend` | st | 0…24 | 2 |
| `quality` | Draft / Standard / High / Best | 0…3 | High |
| `amp.attack`, `amp.hold` | ms | 0.1…20 000 | 0.1 (= instant) |
| `amp.decay`, `amp.release` | ms (to −60 dB) | 1…30 000 | 1000, 250 |
| `amp.sustain` | % | 0…100 | 100 |
| `filter.type` | off / LP / BP / HP (12 dB SVF) | 0…3 | off |
| `filter.cutoff` ◆, `filter.reso` ◆ | Hz, % | 20…20 000, 0…100 | 20 000, 0 |
| `filter.key`, `filter.vel`, `filter.env` | %, st, st | | 0 |
| `env2.*` | ms, % | | 0.1 / 500 / 0 / 250 |
| `lfo1.rate` ◆, `lfo1.shape`, `lfo1.delay`, `lfo1.pitch` ◆ | Hz, —, ms, ct | | 5, sine, 0, 0 |
| `lfo2.rate` ◆, `lfo2.shape` | Hz, — | | 0.5, triangle |
| `start.vel` | ms | 0…1000 | 0 |
| `sw.lo`, `sw.hi`, `sw.default` | key (−1 none) | | −1 |

Group `g{m}.`: `gain` dB, `pan` %, `tune` ct, `attack` `hold` `decay` `sustain` `release`
(override the instrument's), `vel` %, `poly` (layers at once, 0 = no limit), `mute`,
`name` (text).

Zone `z{n}.`: `sample` (text, asset) or `gen` (built-in 1…21); `group`; `lo` `hi` `root`
(keys); `vlo` `vhi`; `tune` ct (±4800); `gain` dB; `pan` %; `key` (ct per key, 100);
`start` `end` (frames; end 0 = the file's end); `mode` (no loop, one shot, loop, loop while
held); `dir` (forward, ping-pong, backward); `ls` `le` (frames, inclusive); `xf` ms; `lxc`
(linear / equal power); `rev`; `vfin` `vfout` `kfin` `kfout` (fade widths, steps / keys);
`fc` (power / gain curve); `seq` of `seqlen`; `rlo` `rhi` (random range); `trig` (attack,
release, first, legato, key-up); `rtdecay` dB/s; `rtfollow`; `excl` `offby` `offmode`;
`sw` (keyswitch); `delay` ms; `name` (text).

Sources produced: `env1` (amp), `env2`, `lfo1`, `lfo2`, `vel`, `relvel`, `key`, `rnd`,
`macro1…8`, `mw`, `at`, `pb`, `expr`, `sus`.

Telemetry from slot 8: layers sounding, sample memory (MiB), zones, block peak, then up to
16 (zone, file position) pairs — the map lights the zones that sound and the waveform draws
their playheads — and the keyswitch in force (slot 48).

## Import and export

- **WAV / AIFF / FLAC / Ogg**: decoded by the sample cache (WAV exactly as before). A
  dropped file or folder is mapped by metadata (`smpl` unity note, pitch fraction and loop;
  `inst` range and gain; AIFF `INST` + `MARK`), then by the note and dynamic in the name
  (`Harp_C3_mf`, `A#2`, `v3`, `rr2`): key ranges split halfway between roots, velocity
  layers from dynamics, round robins from `rr` tokens; files with no root go one per key
  from C1 (a kit).
- **SFZ** (`sampler_sfz.rs`): headers `<control>` `<global>` `<master>` `<group>`
  `<region>` with inheritance, `#define`, `#include`, both comment styles, paths with
  spaces; the report's tier-1 opcodes (mapping, playback, level and pitch, envelope,
  triggers and articulation, voice control, crossfades) and the tier-2 filter. Each SFZ
  group (and envelope) becomes an Exemplar group. Unmapped opcodes are counted and logged
  ("unsupported opcode width (12 regions)"); the editor shows the log.
- **Decent Sampler** `.dspreset` (unencrypted; `.dslibrary` zips are not read yet):
  volume multiplies across `<groups>`/`<group>`/`<sample>`, pan and tuning add; tags
  become exclusive classes; `seqMode` round robin and random; loops, crossfades (frames),
  envelopes, velocity tracking, triggers.
- **SoundFont 2 / SF3**: presets → instruments → zones → samples; instrument generators
  absolute, preset ones added, ranges intersected; timecents, centibels (scaled by 0.4 as
  FluidSynth does), absolute cents, 0.1 % pan converted. The preset's samples are written
  as 16-bit WAVs (SF3: the Ogg streams as `.ogg`) under
  `<data dir>/samples/soundfont/<bank>/`. A bank with several presets asks which.
- **SFZ export**: one `<group>` per Exemplar group with its effective envelope, one
  `<region>` per zone; SFZ → Exemplar → SFZ → Exemplar keeps every mapped field.

An import is **one batch of commands** (one undo step) that clears the old zones and
groups and writes the new ones.

## The editor

On the kit's device shell since round 18 (ADR 0024 §8, `docs/instruments/REDESIGN.md`):
the display first, then a card per section in its function's hue (source blue for the
map, the sample and the zone; tone for the filter; dynamics for the envelopes; modulation
for the LFOs; output for voice, output, group and keyswitches), every number a kit cell —
value pill, knob, label abbreviated through `<label>.short` in both languages. Drawn with
the kit's brush in tokens only; no stock egui widget is left in the editor's files.

- **Edit** (taller than the window: it scrolls): the files row (Import ▾ — samples, a
  folder, an instrument — as a menu field; *Export SFZ…* as an action; a SoundFont's
  presets as a menu when one waits; Quality Draft · Standard · High · Best as a segmented
  control with each tier explained in the help line; the import notes as an LED). The
  **map** card — keys across, velocity up; zones labelled level (the micro face when the
  label face does not fit, then elided in the middle, never rotated); fades as gradients
  inside the zone; a stack of identical boxes badged ×n and stepped through by clicking
  again; the root as a notch; sounding zones lit; every C named; the keys held on the
  shell's keyboard lit in the key strip; *Fit* in its title bar. Drag a zone to move it, an
  edge to resize it; arrows / Shift-arrows / Tab / Delete; Ctrl + wheel zooms. Files
  dropped from the system land on the key under the pointer. Beside it the **zone** card:
  23 cells (root, key and velocity ranges, key tracking, group, delay, tune, gain, pan, the
  four fades, release decay, round robin, random range, exclusive class, silenced-by,
  keyswitch), the trigger as a menu field, *Follow* and *Soft* as LEDs, *Remove*. Then the
  **sample** card — the waveform as its display with S/E flags on the top edge and L/R on
  the bottom (Alt snaps to a zero crossing), the loop and its crossfade shaded, playheads,
  the span shown in its title bar; under it the loop mode and direction as segmented
  controls, Reverse as an LED, the crossfade curve, and start, end, loop points,
  crossfade and loop length as label | value rows you drag. Beside it the **seam** card:
  the join as its display, the file's own continuation faint behind, the **seam error in
  dB** in a pill coloured by audibility; *Find best loop* and *Snap to zero*. Then the
  instrument's cards — amplitude envelope, filter (type as the card's ‹ › selector),
  output; modulation envelope, vibrato (shape selector), LFO 2, voice (poly · mono ·
  legato selector); the selected zone's group (its LED mutes it) and the keyswitches —
  and the **zone table**: every zone a row of wells you drag, the selected row lit, only
  the rows in view laid out.
- **Play**: the map beside the selected zone's waveform, lit by what sounds, over the
  amplitude, filter, vibrato and voice cards.
- **Footer**: sample memory with mip-maps, and zones (the shell's chip, `sampler::chips`).
- The kit's thin widgets size themselves at 100 %, so the editor places the kit's painters
  itself at the window's scale (`sampler_parts.rs`, shared with Pulsus II): at 75 % and
  200 % and in Spanish every word fits its box (`tests/inst_sampler.rs`).
- **Presets** (12, every one built from the built-ins, macros named): Showcase multisample,
  Nylon pluck, Glass bells, Warm ensemble, Breath choir, Drawbar organ, Round-robin kit,
  Sub sine bass, Saw lead, Chip square (Draft on purpose), Noise sweep, Soft keys.

## What the tests hold

DSP (`exemplar_tests*.rs`): unity is bit-exact in every tier; the four-sine signal
(5/10/15/19 kHz at 44.1 kHz) played +12 and +24 semitones into 48 kHz has no spurious line
above −80 dBFS at High and −100 dBFS at Best, while Draft is shown to alias; pitch is right
to 0.05 cent from −24 to +31 semitones; pitching down −12/−24 keeps 100 Hz…17.5 kHz within
±0.1 dB; a loop an exact number of periods long has no seam step larger than the signal's
own (unity and resampled); a crossfade smooths a bad loop; ping-pong repeats and skips no
sample; a released sustain loop finishes its lap and plays the tail; velocity crossfades
keep constant power (power curve) and constant amplitude (gain curve) to 0.01 dB; round
robin and random choose the same zones for the same notes, use every slot, and live input
cycles 1-2-3-4; a release sample is 12 dB down after 2 s at 6 dB/s; a choke silences only
its class, over at least 5 ms; a render with loops, crossfades, filter, vibrato and a bend
is identical at block sizes 1, 7, 64, 512 and 4096 and ends in exact zeros; the built-in
numbering the presets use.

Engine (`inst_sampler.rs`, and `inst_contract.rs` for every kind): a stereo WAV at its root
plays back bit for bit through the unit; zones far above their root get mip-maps and a zone
at its root does not; a missing file is exact silence while the other zones play; a
three-sample multisample renders identically at block sizes 1, 64, 512, 1000; writing every
default out changes no sample; the built-in sine is in tune across C1…A7; 16 transposed
stereo voices render faster than real time; memory per minute is reported; telemetry names the
zone that sounds.

Editor (`crates/cantus-ui/tests/inst_sampler.rs`): opens in Play, Edit and Matrix and paints
its name, its zone labels and the seam error; an empty instrument invites a drop; a zone
dragged across many frames is one undo step, and a field set back to its default is
cleared; an import is one undo step and leaves what it does not name; every factory preset sets what it names
and names its macros; dropped files become zones (split by root and by dynamic) in one
step; SFZ maps the priority opcodes and logs the rest; SFZ export round-trips; Decent
Sampler multiplies volume, adds pan and maps tags; a synthetic SoundFont becomes zones and
WAV files with the right loop, attenuation and envelope; WAV `smpl` metadata gives root,
fine tune and loop.

## Measured

On the shared build machine (Windows, a dozen builds running), `cargo test` profile
(cantus-dsp and cantus-engine at opt-level 3).

**Aliasing** — the four-sine signal (5, 10, 15, 19 kHz at −14 dBFS each, 44.1 kHz)
played up into 48 kHz; the loudest spectral line that is not an expected partial
(`cargo test -p cantus-dsp measured -- --nocapture`):

| Tier | +12 st | +24 st |
|---|---|---|
| Draft (linear, no mip-maps) | −17.4 dBFS | −15.7 dBFS |
| Standard | −93.2 dBFS | −94.4 dBFS |
| **High** (default) | **−120.3 dBFS** | **−121.7 dBFS** |
| Best | −123.8 dBFS | −123.3 dBFS |

The research targets were High ≤ −80 dBFS and Best ≤ −100 dBFS; the tests hold those.
For comparison the report quotes linear interpolation at 19 dB and 4-point Hermite at
23.5 dB of SNR even on 2×-oversampled input (Niemitalo) — what Kontakt's "Standard" and
sfizz's default use.

**Pitch**: the built-in sine is within 0.0001 cent from C1 to A7 through the whole engine;
the resampler alone within 0.05 cent from −24 to +31 semitones (the test's bound).

**Passband pitching down**: 100 Hz … 17.5 kHz within ±0.1 dB at −12 and −24 semitones
(High).

**Cost**: a High, stereo, transposed voice costs about **2 % of one core** at 48 kHz on
the loaded build machine (32 voices, 4 s of audio rendered in 2.52 s: 1.97 % per voice);
the playhead alone takes 380 ns per output frame at +1 semitone and 430 ns at +36 (the
same within the machine's noise — the cost does not grow with the transposition, as de
Soras's scheme promises, where a kernel scaled by the ratio would cost 23× more at +36).
A voice at its root (ratio exactly 1) reads the file directly and costs a fraction of
that. With the whole workspace's tests running in parallel the same measurement read 3.8 %
per voice, so the budget test holds 16 voices under a real-time ceiling.

**Memory**: one minute of 48 kHz stereo is 21.97 MiB as `f32`; a zone played up to two
octaves above its root (plus the bend range and the modulation margin) holds 41.20 MiB with
its mip-maps. A zone played only near its root holds no mip-maps at all.

## Not done (and why)

- **Looped and reversed samples stay in memory** (ADR 0050 §1): loops can jump anywhere and
  are almost always short. A sample used whole (looped, short, or with streaming off) is
  `f32` as before, 22 MiB per minute of 48 kHz stereo, plus its mip-maps.
- **Loops at mip levels above 0** splice at the nearest level-*k* sample, so a very short
  loop that is not a multiple of 2^k frames long is off by up to 2^(k−1) frames per lap
  (a few cents on a 600-frame single cycle two octaves up). Built-in cycles are powers of
  two and exact.
- SFZ tier 2 beyond the filter (CC modulation, `<curve>`, v2 `egN`/`lfoN`), `amp_velcurve_N`
  and ARIA shapes are logged, not mapped; Decent Sampler's `<modulators>`, `<effects>` and
  `<ui>` are logged; SF2 modulators other than velocity are logged; `.dslibrary` zips and
  merged SF2 stereo pairs are not read yet.
- Slicing (Sectio), pitch detection for unnamed files, per-note MPE, the voice inspector,
  record-into-a-zone and auto-sampling are P1/P2.

## Round 28 (sound): round robin in order, drum rows, a kit built at install

- **Round robin in order** (`rr.order`: 1 *In order*, the default; 0 *By note*, every
  Exemplar saved before — schema 30's migration, `migrate_29_to_30`). A clip's note arrives with its ordinal
  (`InstrumentUnit::note_on_seq`); the zone whose `seq` is `ordinal mod seqlen + 1` plays.
  The ordinal is counted per **round-robin lane** (`inst::sampler::RrLanes`, review 1): the
  notes that reach the same set of samples, whatever their key or velocity — a tom on
  47, 48 and 50 is one lane, each velocity layer of a hi-hat its own — so a fill across a
  drum's keys and an accent pattern never repeat a hit. A user preset saved before round
  28 (no `rr.order`) keeps *By note* (`NativeSpec::read_unversioned`); one saved now spells
  the order out.
  An import (SFZ, Decent Sampler, a folder) sets it to *In order*. The choice sits in the
  *Edit* view's tools row. Tests: `exemplar_tests4.rs` (cycles by ordinal, a render from the
  middle, a 64-stroke roll never repeats, *By note* unchanged, live notes),
  `inst_sampler.rs` (engine: a clip's notes and a render from beat 5), `format.rs`
  (`schema_30_keeps_an_old_exemplars_round_robin_seeded`), `cantus-ui/tests/inst_sampler.rs`
  (an import plays in order).
- **Drum rows** (`kit`: 0 a keyboard, the default; 1 a kit). The piano roll and the channel
  rack show a row per key the zones play, named by the zone's group (SFZ `group_label`),
  else by General MIDI, as Pulsus shows its pads. A pack instrument on the *Drums* shelf
  turns it on; the *Drum rows* switch in the tools row toggles it.
  Test: `cantus-core/tests/sampler_kit_lanes.rs`.
- **DRSKit**, DrumGizmo's acoustic kit (CC BY 4.0), is a pack built at install: its 13
  microphones are mixed to one stereo file per hit and an SFZ is written for Exemplar
  (ADR 0042's round-28 amendment). 548 zones, 170 velocity layers (after the review's 24 dB range) each with
  at least two hits taking turns, the hi-hat's choke, General MIDI keys, 64 voices.
