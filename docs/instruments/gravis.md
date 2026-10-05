# Gravis — the bass instrument

`InstrumentKind::Bass`. One instrument, three engines — **808**, **Synth**, **String** —
behind one mono/legato voice with glide, a sub derived from the main phase, a drive that
keeps the fundamental, and an output stage. Built in round 17 against
`research/instruments/bass.md` (the benchmarks: Trilian, IK MODO BASS 2, SubLab XL, FL's
Transistor Bass) under ADR 0023.

| Layer | Files |
|---|---|
| model | `crates/cantus-core/src/inst/bass.rs` — the keys, ranges, units (`params!` builds `PARAMS` and the index table `ix` together) |
| DSP | `crates/cantus-dsp/src/gravis.rs` (the voice, the engines' dispatch, the output path) · `gravis_voice.rs` (AHDSR, glide, seeded noise, smoothing) · `gravis_808.rs` · `gravis_synth.rs` · `gravis_filter.rs` · `gravis_string.rs` + `gravis_string_engine.rs` · `gravis_out.rs` (oversampling, drive, clipper, subsonic, mono-below, duck) · `gravis_tests.rs` |
| engine | `crates/cantus-engine/src/inst/bass.rs` (spec → settings, the modulation matrix on a fixed grid, telemetry, host hooks) · tests `crates/cantus-engine/tests/inst_bass.rs` |
| editor | `crates/cantus-ui/src/views/inst/bass.rs` · `bass_edit.rs` (the cards) · `bass_808.rs` · `bass_string.rs` · `bass_synth.rs` · tests `crates/cantus-ui/tests/inst_bass.rs` |
| strings | `crates/cantus-ui/locales/inst/bass.en.txt`, `bass.es.txt` (224 keys each) |

## What was built

### The voice (every engine)

- **Mono / legato** from `cantus_dsp::voices::Allocator`: a 16-note stack, last / low /
  high priority, return to a held note, the sustain pedal. *Legato* glides without a new
  attack; *Mono* attacks every note (overlapping notes still glide when the glide says so)
  and restarts when it returns to a held note. The String engine adds *Strings*: one voice
  per string, so a chord rings and a note cuts only its own string.
- **Glide** by time or by rate (ms per octave), exponential (99 % at the time), linear or
  S-shaped; *legato only*, *always* or *off*. Stepped once per sample.
- **The legato window** (0–30 ms, default 5): the snapshot sends a note-off before a note-on
  at the same tick, so notes that only *touch* in the piano roll arrive as a release and a
  new note. A note arriving within the window after a release is played legato: the
  envelope returns to its decay from where it is, the pitch glides. A gap longer than the
  window is a new note. Nothing is delayed, so notes followed by a rest are untouched.
- **Declick**: a retrigger hands the sounding voice to a second slot that fades out over
  the declick time (default 1.5 ms) while the new note starts at 0°.
- **Phase**: Reset (every note at 0° rising — identical hits against the kick), Random
  (seeded by the note's id and key), Free.
- Pitch bend (range 0–24 st), per-note tuning (the MPE / CLAP note expression), octave,
  transpose, fine, A4 reference.

### 808

- A `u32` phase accumulator: exact periodicity over a bar-long sustain, bit-identical on
  every platform.
- Pitch envelope (0–48 st, time to −60 dB), the TR-808's **knock** (up to +24 st for about
  6 ms), the slow **sigh** (up to 2.5 st over ~300 ms, off by default), a **click** (seeded
  noise high-passed at its tone), an **AHDSR** reaching exact zero.
- **Harmonics** as exact Chebyshev partials 2–8 generated from the phase by recurrence, at
  levels fixed relative to the fundamental (tilt in dB per octave of harmonic number,
  even/odd balance), only while `n·f < 0.45·fs`: they cannot alias and cannot touch the
  fundamental. *Shape* adds a band-limited soft triangle the same way.

### Synth

- PolyBLEP saw / square / pulse / triangle; **1–7 unison voices on Szabo's measured
  supersaw curves** (the seven offsets, the 11th-order detune curve, the linear centre and
  parabolic side mix); a second oscillator (level, semitones, cents).
- **The main phase is a `u32` at exactly the note's frequency**; the centre voice *is* it
  and the **sub counts its wraps** (a flip-flop per octave), so the sub's zero crossings
  fall on the main's through glides, bends and the pitch envelope.
- **Filters, written here** (`gravis_filter.rs`, `f64` state, zero-delay feedback,
  pre-warped): a transistor **ladder** (24 dB), a **diode ladder** (the TB-303's loaded RC
  ladder with spread poles, solved as a 4×4 system per sample; its resonant frequency and
  self-oscillation gain calibrated numerically, so — as on the transistor ladder — the peak
  is at the cutoff, and the spread poles make it darker and softer below it), and an
  **SVF** low-pass / band-pass. **Bass
  compensation** scales the input by `1 + comp·k`, so at 100 % the passband stays at unity
  whatever the resonance. The feedback node saturates (the cheap ZDF nonlinearity) at −16 dB
  under the signal, so an undriven filter is all but linear; *filter drive* pushes into it.
- Filter and amplitude ADSRs, envelope amount in octaves, key tracking.
- **The 303 accent**: at or above the accent velocity (default 100) a note gets its own
  filter envelope, the shortest decay (≤ 200 ms), +60 % level and more resonance; the
  **sweep capacitor** charges from that envelope in ~30 ms and leaks over ~350 ms, so a run of
  close accents climbs. An accent on a legato (slid) note is still an accent — the timing
  never swallows it (the KVR complaint against Transistor Bass).
- **Width** above a **mono crossover**: the side voices go to a side signal, high-passed at
  *mono below* (4th order), so everything under it is mono.

### String

- **One bidirectional digital waveguide per string** (displacement waves): two rails, each
  `half` samples, **both read with third-order Lagrange interpolation**, which stays clean
  while the length moves (slides, bends, glides).
- The nut inverts; the bridge inverts through a **one-pole loss filter** designed from two
  T60s in seconds (the fundamental's *sustain*, the high frequencies' by *brightness*, *age*,
  string type and *mute*) and **four first-order allpasses** for **stiffness**, fitted per
  note by bisection so partial 12 (or the highest under 5 kHz) is as sharp as a stiff
  string's `√(1 + B n²)`, with `B = π³Ed⁴/(64TL²)` per string (≈ 4.4·10⁻⁴ for a 34″ E, scaled
  by the vibrating length and the stiffness control).
- **The loop is tuned by its phase delay at the fundamental**, loss filter, allpasses and
  both interpolators included, so the pitch is right to hundredths of a cent.
- **Pluck and pickup positions are the geometry itself**: a pluck adds its shape to the rails
  at its position; each pickup reads the displacement at its position through a Gaussian
  aperture (a humbucker: two coils 20 mm apart) — the two combs of an electric bass come
  from where things are, not from comb filters.
- **Two movable pickups** (mm from the bridge, levels, single coil or humbucker), a magnet
  nonlinearity `y/(1 + y/2h)` with *pickup height* (even harmonics growing with closeness
  and swing), the coil's resonant low-pass loaded by the passive **tone**, and a **piezo**
  (the force at the bridge, uncombed).
- **Frets as conditional reflections** (Rank & Kubin): wherever the string runs over a fret
  between the fretting finger and the bridge, a displacement past the fret's height (from
  the *action*) is taken out of both rails. *Buzz* sets how hard the frets are; 0 computes
  no collision at all.
- **Techniques**: finger (a rounded pluck, width from *hardness*), pick (a sharp one), slap
  (a **thumb** strike as a velocity pulse below the pop velocity, a **pop** — a large pull —
  at or above it; both hit the frets), palm mute. The thumb and the pop are **calibrated
  against the finger** (round 19): what the pickups hand on after a thumb is × 0.2
  (`THUMB_LEVEL`), after a pop × 0.3 (`POP_LEVEL`). The thumb's velocity step reaches the
  coil — a derivative — as a third-of-a-millisecond edge, and at unity it peaked ~20 dB
  over a finger pluck (+12 dBFS on one note, the slap preset +15 dBFS on a funk riff); the
  pop pulls 3.5× the finger's displacement. The physics (the step, the pull, the frets they
  slap) is unchanged, so the sound is the same, only at a level a mix can use. **Muting** shortens the T60; letting go
  damps the string to the *let go* T60.
- **Hammer-ons and pull-offs** (a legato note on the same string with no glide: the length
  changes at once plus the finger's own small strike or pluck), **slides** (with a glide:
  fret by fret with a click at each fret the *noises* control allows; continuous on a
  fretless), a legato note on another string is a soft pluck.
- **Fingering**: lowest fret, position (the hand stays within four frets), low strings; 4, 5
  or 6 strings; standard, drop, half-step-down tunings; 20–24 frets; fretless; scale 762–914
  mm.
- **Humanize** varies the pluck position (±3 %) and level (±1 dB), seeded by the note.

### Sub, drive and output (every engine)

- **Sub** one or two octaves down, sine / triangle / square, from the main phase (808,
  synth) or a phase at the string's pitch; never filtered or driven.
- **The drive keeps the fundamental**: the saturator (tube, tape, fold, hard) runs on the
  whole signal at 4×, and only what it *added* — `sat(x) − x` — goes on, through a notch
  that tracks f0 exactly and a high-pass under it, to be added to the clean signal. The
  fundamental is the clean one. **Key tracking** adds dB of drive per octave below a pivot
  note (more drive on low notes, which small speakers need most). Drive 0 is not computed:
  bit-exact.
- **Subsonic** high-pass (off by default), the **oversampled clipper** (4×, exact below 0.7,
  a tanh knee to 0.98; off by default), the output **gain**.
- **Oversampling without latency**: polyphase IIR half-bands (Valenzuela & Constantinides'
  elliptic design; ten coefficients for the first 2× stage, six for the second), a group
  delay of a couple of samples instead of a linear-phase FIR's thirty — a bass is lined up
  against the kick to the sample, and instrument latency is not compensated (ADR 0023).

### The editor

On the kit's device shell (ADR 0024 §8), built from the bound kit parts Calor shares
(`analog_parts.rs`): cards in their function hues, cells, segmented controls, LED buttons,
every word in a box and abbreviated through `<key>.short` when it does not fit.

- **The display card first**, bleeding to its edges, titled by what it shows; in its title
  bar what plays now (note and hertz, in a well), the engine (808 · Synth · String) and the
  voice mode (Legato · Mono, and Strings for the String engine) as segmented controls.
- **808**: the pitch curve over the waveform the DSP renders for these settings (rendered at
  12 kHz, only when a setting changes), on a square-root time axis so the drop and the knock
  get room; drag the curve's start (the drop, st) and its −20 dB knee (the time, ms);
  double-click resets. Beside it, the harmonics 1–8 relative to the fundamental.
- **String**: the bass from above, to scale, bridge at the left: body, fingerboard, frets
  with their numbers, nut, the strings (lowest at the bottom, as in tablature) named at the
  nut; **drag the pluck marker and the pickups**, read them in millimetres on dimension lines
  from the bridge; the fretting finger and the vibrating length lit while a note plays.
  Beside it, **the note's harmonics as the pickups hear them, with every notch marked** (▽ the
  pluck's, ▼ the pickups') at `k/β`, moving as you drag, and B with the 10th partial's
  stretch in cents.
- **Synth**: the filter's response (the analog prototype with the bass compensation, so the
  passband visibly stays put) with one handle — across for the cutoff, up for the resonance
  — the live cutoff dashed while a note plays, the unison voices as ticks at their detune
  (outer detune in cents) and the accent meter (the sweep's charge).
- **Play**: under the display, one card per section — Source, Tone, Dynamics, Output — with
  the controls that matter live (808: pitch, tone, envelope; Synth: oscillators, filter,
  accent; String: playing, tone, sustain; every engine: drive character, drive, sub,
  glide), their widths following what they hold.
- **Edit**: the display, then every control of the engine in cards (808: pitch, tone,
  envelope; Synth: oscillators, filter, envelopes, 303 accent; String: playing,
  electronics, strings and neck), then the cards every engine shares: voice and glide, LFO,
  sub, drive, *With the song* (which says, with an unlit lamp and the whole note in the help
  line, what waits for the host) and the level. It scrolls past the window. Every drag on a
  display or a cell is one undo step.
- 14 factory presets with tags, each naming what its macros do: Trap 808 long, Punchy 808,
  Distorted 808, Clean sub, Acid 303, Reese, Wobble, Square and sub, P-bass fingers, J-bass
  bridge pickup, Slap and pop, Fretless, Picked five-string, Flatwounds muted.

## Parameters

Units as the knob prints them; `%` is stored 0…1. Stepped keys hold an index.

| Key | Range | Default | Unit / meaning |
|---|---|---|---|
| `engine` | 0–2 | 0 | 808 · Synth · String |
| `mode` | 0–2 | 0 | Legato · Mono · Strings (String engine) |
| `priority` | 0–2 | 0 | Last · Low · High |
| `glide` | 1–2000 | 60 | ms (or ms per octave) |
| `glide.trigger` | 0–2 | 0 | Legato only · Always · Off |
| `glide.rate` | 0–1 | 0 | Time · Rate per octave |
| `glide.shape` | 0–2 | 0 | Exponential · Linear · S |
| `legato_window` | 0–30 | 5 | ms |
| `declick` | 0–5 | 1.5 | ms |
| `bend_range` | 0–24 | 2 | st |
| `octave` / `transpose` / `fine` | ±2 / ±12 / ±50 | 0 | oct / st / ct |
| `a4` | 415–466 | 440 | Hz |
| `phase` | 0–2 | 0 | Reset · Random (seeded) · Free |
| `velocity` | 0–1 | 0.6 | % of level from velocity |
| `lfo.rate` / `lfo.shape` | 0.05–40 / 0–3 | 2 / 0 | Hz / sine · triangle · square · saw (source `lfo1`) |
| `808.shape` | 0–1 | 0 | % soft triangle |
| `808.pitch` / `808.pitch_time` | 0–48 / 1–500 | 12 / 45 | st / ms to −60 dB |
| `808.knock` / `808.click` / `808.click_tone` | 0–1 / 0–1 / 500–16000 | 0.2 / 0.15 / 3000 | % / % / Hz |
| `808.attack` `hold` `decay` `sustain` `release` | 0–50, 0–2000, 10–10000, 0–1, 5–4000 | 1, 0, 1200, 0.6, 250 | ms, ms, ms, %, ms |
| `808.sigh` | 0–1 | 0 | % |
| `808.harmonics` / `808.tilt` / `808.even` | 0–1 / ±6 / ±1 | 0 / −3 / 0.3 | % / dB/oct / % even |
| `syn.wave` / `syn.pw` | 0–3 / 0.05–0.95 | 0 / 0.5 | saw · square · pulse · triangle / % |
| `syn.unison` / `syn.detune` / `syn.spread` | 1–7 / 0–1 / 0–1 | 1 / 0.3 / 0.5 | voices / % (Szabo curve, printed in cents) / % (Szabo mix) |
| `syn.width` / `syn.mono_below` | 0–1 / 40–300 | 0 / 120 | % / Hz |
| `syn.osc2` / `syn.osc2_semi` / `syn.osc2_fine` | 0–1 / ±24 / ±50 | 0 / −12 / 7 | % / st / ct |
| `syn.filter` | 0–3 | 0 | Ladder 24 · Diode 303 · SVF LP · SVF BP |
| `syn.cutoff` / `syn.reso` / `syn.bass_comp` / `syn.drive` | 20–20000 / 0–1 / 0–1 / 0–1 | 400 / 0.2 / 0.5 / 0 | Hz / % / % / % |
| `syn.env` / `syn.key_track` | ±6 / 0–1 | 2 / 0.5 | oct / % |
| `syn.f_attack` … `syn.f_release` | ms, %, ms | 1, 300, 0, 200 | filter ADSR |
| `syn.attack` … `syn.release` | ms, %, ms | 1, 600, 0.8, 120 | amplitude ADSR |
| `syn.accent` / `syn.accent_sweep` / `syn.accent_vel` | 0–1 / 0–1 / 1–127 | 0.5 / 1 / 100 | % / on / velocity |
| `str.strings` / `str.tuning` | 4–6 / 0–2 | 4 / 0 | strings / standard · drop · half step down |
| `str.scale` / `str.frets` / `str.fretless` | 762–914 / 20–24 / 0–1 | 864 / 21 / 0 | mm / frets / on |
| `str.type` | 0–3 | 0 | roundwound · halfwound · flatwound · tape |
| `str.stiffness` / `str.sustain` / `str.bright` / `str.age` | 0–2 / 0.3–20 / 0–1 / 0–1 | 1 / 6 / 0.45 / 0.2 | % of B / s / % / % |
| `str.action` / `str.buzz` | 1–4 / 0–1 | 2.2 / 0.3 | mm at the 12th fret / % |
| `str.technique` | 0–3 | 0 | finger · pick · slap · palm mute |
| `str.pluck` / `str.hardness` / `str.pop_vel` | 20–400 / 0–1 / 1–127 | 180 / 0.5 / 110 | mm from the bridge / % / velocity |
| `str.mute` / `str.release` / `str.noise` | 0–1 / 20–2000 / 0–1 | 0 / 150 / 0.3 | % / ms T60 when let go / % |
| `str.fingering` | 0–2 | 1 | lowest fret · position · low strings |
| `str.pu1` / `str.pu2` | 20–300 | 60 / 159 | mm from the bridge |
| `str.pu1_level` / `str.pu2_level` | 0–1 | 1 / 1 | % |
| `str.pu_type` / `str.pu_height` / `str.piezo` / `str.tone` | 0–1 / 1.5–5 / 0–1 / 0–1 | 0 / 3 / 0 / 1 | single · humbucker / mm / % / % |
| `str.humanize` | 0–1 | 0 | % (seeded per note) |
| `sub.level` / `sub.octave` / `sub.wave` | 0–1 / 1–2 / 0–2 | 0 / 1 / 0 | % / oct down / sine · triangle · square |
| `drive` / `drive.type` | 0–1 / 0–3 | 0 / 0 | % (0 is bit-exact) / tube · tape · fold · hard |
| `drive.keytrack` / `drive.ref` / `drive.lock` | ±6 / 12–60 / 0–1 | 3 / 36 / 1 | dB/oct below the pivot / note / keep the fundamental |
| `subsonic` / `clip` | 0–40 / 0–1 | 0 / 0 | Hz (0 off) / on |
| `duck.source` | 0–2 | 0 | none · the project's kick (its notes) · sidechain |
| `duck.depth` / `duck.pre` / `duck.hold` / `duck.release` / `duck.shape` | 0–40 / 0–10 / 0–200 / 10–500 / 0–2 | 6 / 2 / 20 / 90 / 1 | dB / ms / ms / ms / linear · exponential · S |
| `key.snap` | 0–2 | 0 | off · the song's scale · root and fifth |
| `gain` | −48…+12 | 0 | dB |

Modulation sources: `env1` (amplitude), `env2` (the synth's filter envelope), `lfo1`, `vel`,
`key`, `rnd` (seeded per note), `mw`, `at`, `pb`, `expr`, `macro1…8`. A route's depth is a
fraction of the target's travel **in its knob's taper** (a cutoff or a time moves in octaves),
evaluated every 32 samples counted from the start of the stream.

## Host hooks (for the coordinator)

Kick-aware ducking and tuning to the song's key need what only the host knows. The DSP and
the unit implement their half; the processor does not call them yet.

1. **The kick duck** — `cantus_engine::inst::bass::Unit::kick_in(&mut self, ahead: u32)`
   ("a kick starts `ahead` samples from now"; the dip reaches `duck.depth` exactly then,
   holds `duck.hold`, releases over `duck.release` with `duck.shape`). The hook needed:
   when `duck.source` is 1, the processor, which has every track's sorted note events in the
   snapshot, calls `kick_in(ahead)` on the bass unit for every note of the **kick source**
   whose start falls within the next `duck.pre` ms — in the segment loop, before `process`,
   with `ahead` the samples from the segment start to that note (at most
   `duck.pre · fs / 1000`). The kick source: the Pulsus pad whose voice is `Kick` on any
   track (or a track the user picks, a text key `duck.track` to be added). Proposed trait
   method so hosted instruments can share it:
   `fn host_event(&mut self, event: HostEvent)` with
   `HostEvent::KickAhead { samples: u32 }`. Zero latency: it reads notes, not audio. Source
   2 (sidechain audio) waits for the effects' sidechain route to reach instruments.
2. **The song's key** — `Unit::set_song_key(&mut self, key: Option<(u8, u16)>)` (root 0 = C,
   and the scale's pitch classes as a mask, bit 0 = the root). The hook needed: a project
   key (set by the user or estimated from the notes), carried in the snapshot and passed on
   configure — `HostEvent::SongKey { root, mask }` or a field beside the spec. With
   `key.snap` 1 every incoming key moves to the nearest pitch class of the scale (ties go
   down); 2, to the root or fifth. Note-offs follow the key their note-on was snapped to.
3. **Glide in note values** needs the tempo (`HostEvent::Tempo { bpm }`); not offered until
   then.

## What the tests hold

`cantus-dsp` (`gravis_tests.rs`, 23 tests):

- **Pitch within 0.5 cent from B0 to C5 on every engine**, every semitone, by the phase
  slope of the fundamental: worst **808 0.0002 ¢, synth 0.042 ¢, string 0.028 ¢** (6-string,
  24 frets, dispersion on). The 808 settles in tune after its pitch envelope; the string is
  in tune on a 4-string with fret buzz and at 200 % stiffness.
- A stiff string's 8th partial is sharp (**+37.6 ¢**; the stiff-string formula says +46.7 ¢:
  the four first-order allpasses are fitted at partial 12, within 30 % at the 8th).
- Chebyshev harmonics 2–8 are exactly at their set level (±1 %), nothing between them
  (< −90 dB).
- **Drive keeps the fundamental**: A1 at drive 25/50/75/100 % in all four types moves the
  fundamental by less than 0.1 dB; DC stays under −80 dBFS; the harmonics grow with the
  drive; with the lock off the same test sees the fundamental move (> 0.5 dB).
- Neutral controls are bit-exact; the sub's zero crossings fall on the main's through a
  one-octave glide with vibrato, at one and two octaves down; glide times (linear, S,
  exponential to 99 %, constant rate); touching notes slide inside the legato window and
  attack outside it; Legato returns without a restart, Mono restarts; low priority; a run of
  accents climbs and unaccented notes charge nothing; bass compensation holds the passband at
  unity at any resonance (and without it the ladder drops to 1/(1+k)); both ladders resonate
  within 5 % of the cutoff; the pluck at the middle notches harmonics 2, 4, 6 by more than
  25 dB; 24 random string settings stay finite and never gain energy; a retrigger does not
  click (and without the declick the test sees the click); every engine goes exactly silent
  and says so; **renders are identical at block sizes 1, 7, 64, 480, 4096**; the half-band
  rejects images by more than 70 dB and passes the band within 0.05 dB; the duck reaches its
  depth on the kick's sample and starts `pre` early; every engine peaks near −6 dBFS;
  (round 19) a slap's thumb and pop each peak under −1 dBFS across the neck with a
  loudness within 6 dB of a finger pluck's at the same velocity, and every technique on a
  dense funk riff at full velocity peaks under −1 dBFS (it was +12.6 dBFS for the slap).

`cantus-engine` (`inst_bass.rs`, 9 tests; `inst_contract.rs` stays green):

- Every engine (and Strings mode) through the real render: identical at every block size
  and twice over; modulation (an LFO on the cutoff, a macro on the drive) identical at block
  sizes 1, 31, 33, 480 and audible; every default stored explicitly and a depth-0 route
  change nothing; pitch through the unit (**808 0.0003 ¢, synth 0.027 ¢, string 0.020 ¢**);
  **aliasing** — a synth saw at C6: **−54.9 dB** re the fundamental below 12 kHz (−37.9 dB
  over the whole band, PolyBLEP's residue near Nyquist), the 808's harmonics at C7:
  **−107.8 dB**; six strings at velocity 127 in Strings mode peak at 0.918; telemetry;
  a CPU budget; every engine exactly silent after its notes.

`cantus-ui` (`inst_bass.rs`): every engine opens in Play, Edit and Matrix and paints its
name, its own picture, the engine and the voice mode; a knob gesture and a real pointer drag
of a cell are one undo step each; **a real pointer drag of the
pluck marker lands at the millimetre under the pointer and is one undo step**; switching
engines is one step and keeps every engine's settings; every preset sets what it names,
names its first macro, connects only real sources to real targets, and comes back in one
undo; every label has words in English and Spanish and nothing painted is a key; no word
is elided, cut or painted over another at 75 % and 200 %, in English and Spanish, for every
engine (Play in the window, Edit whole).

## Measured

| What | Number |
|---|---|
| Pitch error, B0–C5, every semitone (DSP) | 808 0.0002 ¢ · synth 0.042 ¢ · string 0.028 ¢ |
| Pitch error through the unit (every 7th semitone) | 808 0.0003 ¢ · synth 0.027 ¢ · string 0.020 ¢ |
| Fundamental change under drive (A1, 25–100 %, four types) | < 0.1 dB (asserted) |
| Aliasing, synth saw C6, filter open | −54.9 dB below 12 kHz · −37.9 dB full band |
| Aliasing, 808 harmonics at C7 | −107.8 dB |
| CPU, six strings ringing, every feature (48 kHz, 512-sample blocks) | 5.1 % of a core per string |
| CPU, one 808 voice, drive 100 % + harmonics + clipper | 5.2 % of a core |
| CPU, one synth voice, 7 unison, width, diode filter | 6.2 % of a core |
| Level, full velocity | 808 −6 dBFS · synth −7.5 dBFS · string −5 to −8 dBFS |
| Slap (round 19), one note | thumb −1.8 to −3.5 dBFS · pop −2.3 to −7.3 dBFS (were +11 to +12 and +3 to +8) |

CPU was measured on a build machine shared by about a dozen builds at once, so the
numbers are pessimistic.

Screenshots: `cargo run -p cantus-ui --example shoot -- <dir> 1 bass` with
`GRAVIS_ENGINE=808|synth|string` (and `GRAVIS_VIEW=edit` for the Edit view) sets up the
engine's showcase preset and injects what a playing engine reports.

## What is not done

- **The host hooks** above (kick duck, song key, tempo-synced glide): the parameters, the
  DSP and the unit's methods exist and are tested; the processor wiring is the coordinator's.
- **Slide and accent as note properties** (the research's P1: the piano roll drawing glide
  curves, a "slide from previous" flag) and string / technique lanes in the host piano roll:
  model and piano-roll changes outside this instrument's files. Gravis reads overlap, the
  legato window and velocity today.
- Poly for the 808 and the synth (the research's "Poly up to 8"): mono only.
- The translation preview (phone / laptop / club listen) and the translation meter.
- Harmonics (touching a node), let-ring and sympathetic coupling, tension modulation, the
  knuckle's recoil on a thumb slap, an active EQ circuit; the upright bass (P2).
- The synth's filter is not oversampled: at high filter drive it can alias.
- A 4-string's lowest notes below E1 are played on the E string detuned rather than
  refused.
