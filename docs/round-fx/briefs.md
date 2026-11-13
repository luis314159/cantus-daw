# The effects side round — the briefs

Read `docs/round-fx/BUILDING.md` first (the rules, the design bar, the family's rules), then
`docs/round-fx/contract.md` (your files and anchors), then your section here. Reusable DSP is
named per unit; read it before writing your own, and put shared pieces you write in a new
`cantus-dsp/src/<stem>_*.rs`. The names are proposals the owner may still change before a
release (plug-in ids are forever once shipped): keep the name in one constant and the locale.

## cribrum — Cribrum, the auto filter (wave 1, ADR 0130)

The filter Cantus has no unit for: FL's Love Philter and Fruity Filter, Ableton's Auto Filter,
FabFilter Volcano and Soundtoys FilterFreak are the references.

- **Filters:** the five zero-delay-feedback models of `va_filter.rs` / `va_filter_models.rs`
  (the synths' filters — reuse them, do not rewrite them) in low-, band-, high-pass, notch
  and peak where the topology gives it, 12 and 24 dB/oct; cutoff, resonance (self-oscillation
  where the model has it, kept stable), drive into the filter (oversampled when the model is
  nonlinear: `fervor_os.rs`), and a **formant/vowel** mode (two or three resonant band-passes
  morphing A-E-I-O-U).
- **Movement, the unit's soul:** an **LFO** (`modfx_lfo.rs`, its phase a function of the song
  position when synced; shapes incl. sample & hold that is a hash of the seed and the step,
  1/64…8 bars, dotted/triplet, phase and stereo offset), an **envelope follower** on the input
  or a **sidechain** (`dynfx_follow.rs`; `TAKES_SIDECHAIN` true), and a **step envelope /
  drawn curve** synced to the song (as Fluctus draws its curve — reuse its shape model if it
  fits), each with its depth onto cutoff (and resonance/vowel), summed.
- **Two filters** in serial or parallel with their own settings (Love Philter's chains, simply
  done), and a stereo spread of the cutoff.
- **The display (the centre):** the live frequency response of both filters over the input's
  spectrum, the cutoff dot dragged (x cutoff, y resonance) and moving with the modulation, the
  LFO/curve drawn under it in time with the playhead, the envelope's level shown. Material
  and lettering of its own (ADR 0074).
- **Done when:** the contract and null tests; each model's measured response matches the
  synths' filters (the same solver, so the same curve) and self-oscillates in tune where it
  should (±5 cents); a synced sweep rendered from bar 5 equals bars 5… of the whole render;
  the step envelope never clicks (its smoothing tested); presets: wah, dub sweep, sidechain
  pump-filter, vowel talk, telephone, rising build-up, …; the design sheet approved.

## fragor — Fragor, the pedals (wave 1, ADR 0131)

BACKLOG §3b asked by name: distortion pedals and fuzz. A pedalboard of circuit models —
Ibanez TS808 / Tube Screamer, ProCo RAT, Boss DS-1, Dallas Arbiter Fuzz Face, EHX Big Muff Pi
as the references (their **published schematics** are the source; component values in the
ADR). Fervor (saturation, `fervor_*.rs`) stays the clean colour unit; Fragor is the dirty one.

- **Models:** each pedal's stages as its circuit does them — the op-amp's frequency-dependent
  gain into the clipping diodes (in the feedback loop for the TS, to ground for RAT/DS-1), the
  tone stack (the TS's active tone, the RAT's filter, the Big Muff's tone stack with its mid
  scoop), the Fuzz Face's two transistors with **bias/"battery" (starve)** that reproduces the
  dying-battery gated splat (§3b's *done when*), the Big Muff's cascaded clipping stages.
  Solve the nonlinear stages properly (wave digital filters or a Newton-solved ODE per stage,
  K-method — research what fits and say why in the ADR; never port GPL code), **oversampled**
  (`fervor_os.rs`; aliasing measured below a stated level), deterministic maths.
- **The pedalboard:** up to **three pedals in a chain** in any order, each with its knobs as
  the real pedal labels them (Drive/Tone/Level, Distortion/Filter/Volume, Fuzz/Volume + Bias,
  Sustain/Tone/Volume), its footswitch, a noise gate before the chain (Porta's detector if it
  fits), input and output level, mix, and a mono/stereo choice.
- **The display (the centre):** the pedals drawn as **enclosures** — each its own colour,
  knobs and footswitch with its LED, cables between them, dragged to reorder — and above them
  a live view of what the chain does (the transfer curve at the current level and the
  spectrum of the harmonics, or a scope). Drawn in vector, crisp at 200 %, original designs
  (no product's graphics, logos or trade dress reproduced; names in the UI are generic
  descriptions, e.g. "Green overdrive", "Op-amp distortion", "Germanium fuzz", "Triangle muff";
  the circuit references only in the ADR and the manual's history note, as fair description).
- **Done when:** the contract and null tests (every pedal bypassed or a chain of none is a
  null; at minimum drive the output still sounds — the family's starting preset rule); each
  model's small-signal frequency response matches its schematic's analysis (SPICE-class
  computation in a test, written from the published values) within a stated tolerance; the
  Fuzz Face's bias sweep reproduces the gating (a test on a decaying note); aliasing measured;
  CPU per pedal measured; presets for guitar, bass, drums, synths and vocals; the design sheet
  approved.

## lapsus — Lapsus, the time shaper (wave 2, ADR 0132)

BACKLOG §3b *Time manipulation*: Gross Beat is the reference (FL's best-known effect), with
Ableton's Beat Repeat and Effectrix ideas on top.

- **Time envelope:** a drawn curve over 1/4 bar … 4 bars synced to the song position, its
  value the **read position** relative to the live position (a buffer of the past few bars,
  allocated off the audio thread when the length is set): flat 0 = through, ramps = slow-down /
  speed-up, steps = repeats. Interpolated reads (`exemplar_resample.rs`-class quality, no
  zipper), click-free joins (crossfades of a few ms where the curve jumps, their length a
  control). **Tape stop** and **tape start** with a curve of their own, **half-time**,
  **stutter / repeat**, **reverse**, **scratch** — each a preset and sample-exact at any tempo.
- **Volume envelope** in the same grid (gates, pumps, trance-gate) with its smoothing.
- **Slots:** 36 patterns in a bank (as Gross Beat's 36), switched by MIDI notes (`TAKES_NOTES`
  true, export included — the note ports exist since ADR 0053) or automation, a **hold/retrigger**
  mode, and a latency mode where reading the future is asked (report it via `latency`).
- **The display (the centre):** the grid with the time curve and the volume curve, edited with
  the mouse (points, tension, a pencil snapped to the grid, copy/paste), the playhead moving,
  the slot bank beside it, the waveform of the buffer shown faintly under the curve.
- **Done when:** the contract and null tests (a flat curve is a null to the bit — the buffer's
  delay is zero at 0); every preset renders identically from bar 5 as in the full render; tape
  stop at 60, 120 and 174 BPM ends on the bar to the sample; no click at the joins (measured);
  the note switching tested through the exported plug-in too; the design sheet approved.

## vetus — Vetus (lo-fi) and Libra (utility) (wave 2, ADR 0133)

Two units, one builder: the second is small. **Vetus** first.

**Vetus — lo-fi.** References: XLN RC-20 Retro Color, iZotope Vinyl, FL's Fruity Squeeze/
Bitcrusher roles.

- Modules, each on/off with its amount: **bits** (1–24, with or without dither, the
  quantiser's character), **sample rate** (down to ~1 kHz, with and without the anti-alias
  filter — the aliasing *is* the sound when off), **wobble** (wow and flutter, synced or free,
  a delay modulated as Vibrans does — reuse `modfx_*`), **noise** (vinyl crackle and hiss,
  tape hiss, hum 50/60 Hz — every random a hash of the seed and the sample index, so offline =
  real time), **tone** (the bandwidth of an old medium: tape, radio, telephone, gramophone),
  **age/degrade** (dropouts as a deterministic process), and one **Character** macro across
  them.
- **The display (the centre):** an object that ages with the settings (a record or a cassette,
  vector, original), with each module's state shown on it; and an output scope/spectrum.
- **Done when:** the contract and null tests (every module off is a null; bits 24 with no
  dither is a null); the noise is the same offline and live; presets (lo-fi hip-hop, cassette,
  AM radio, 8-bit, gramophone, …); the design sheet approved.

**Libra — utility.** References: Ableton Utility, FL's Fruity Balance, Waves InPhase, a tuner.

- **Gain** (±36 dB, smoothed), **pan/balance** (the pan law of ADR 0012), **width** (or point
  to Latitudo if it is the same — say so), **polarity** per channel, **swap / mono / L only /
  R only / M / S**, **DC removal**, **delay per channel** in samples or ms (alignment of two
  mics), **mute**; and a **tuner** in its display (`pitch_stream.rs`'s tracker; note, cents,
  a needle that rests calmly; reference A = 415–466 Hz).
- **The display (the centre):** a correlation meter and a small goniometer beside the tuner,
  calm at rest.
- **Done when:** the contract and null tests (the defaults a null to the bit, gain 0 dB exact);
  the tuner within ±1 cent on a sine and ±3 on a sung note from the pitch tests' material; the
  delay exact to the sample; presets (mono check, flip phase, align 1 ms, …); the design
  sheet approved.

## tabula — the pedalboard, with amp and cabinet (wave 3, ADR 0134; asked 2026-11-13)

*The owner:* "¿el plugin de efectos podría ser como una pedalera simulation, donde adentro
podamos poner múltiples efectos? … algo gráfico donde quitas, pones, ajustas, cambias orden."
He chose wave 3 (after Lapsus and Vetus/Libra) and **an amp and cabinet inside**. It is
BACKLOG §3b's *Distructor-like multi-effect* and *amp and cabinet simulation with IR loading*.
The name is a proposal (*Tabula*, the board).

- **A container unit**: an ordered list of pedals, each **any of our native units** (Fragor's
  drives, Cribrum as wah/auto-wah, Concentus, Flexus, Phasis, Tremor, Delay, Spatium, Aula,
  Porta, Comprimo, Libra's tuner, Vetus …), added from a pedal picker, removed, bypassed by its
  footswitch, reordered by drag, adjusted on the pedal; serial with an optional parallel split
  (A/B paths and a mixer); **macros** (8, mapped to any pedal's parameters — the exported
  plug-in's fixed parameter list) and **randomise** within musical ranges.
- **Amp + cabinet**: amp models (preamp stages, tone stack, power amp sag — circuits from
  published schematics, as Fragor's) and a cabinet by impulse response (Aula's convolution;
  factory IRs we make or CC0/CC BY, user IRs dropped in), a mic position on the cab.
- **Model and engine**: the children's specs inside the slot's spec (schema bump), commands
  with exact inverses for add/remove/move/bypass (undo fuzz arms), automation keys per pedal
  (`p{n}.<key>`, as Tympanum's), children built off the audio thread with the snapshot, the
  latency summed and reported, exact rest when every child rests.
- **The display**: a top-down board in 2D vector with the kit — **reusing Fragor's
  enclosure/knob/footswitch/cable component** (`views/fx/pedal_draw.rs`); not the scene
  engine of ADR 0126 (round 40's, still in flux), which may later give an optional
  photographic view.
- **Done when**: an empty board is a null; a board of one pedal equals that unit inserted
  alone, to the bit; reordering is one undo step; the exported plug-in exposes the macros and
  reloads the whole board from its state; the design sheet approved.
