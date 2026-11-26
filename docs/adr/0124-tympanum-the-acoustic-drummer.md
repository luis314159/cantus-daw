# ADR 0124 — Tympanum, the acoustic drummer

**Status:** Proposed for round 40 phase 3 (contract; builders `tymp-engine`, `tymp-ui`) ·
**Date:** 2026-11-12 · **Schema:** 67 · **Builds on:** ADR 0023 (native instruments), ADR
0030/0042 (content and licences, amended by ADR 0125 §4 for the pack modes), ADR 0050
(streaming), ADR 0100 (Pulsus III), ADR 0123 (named outputs) ·
**Research:** `F:\projects\daw-r40\research-p3\` · **Design:** `F:\projects\daw-r40\design-p3\`
(the *Kit* and *Mixer* views, approved direction; its `src/` is the kit's 3-D model in cm) · **Pictures:** ADR 0126

## Context

The owner (2026-11-12): a Superior Drummer-class acoustic drum instrument, over Pulsus's
engine, with Pulsus staying the pad drum machine. His answers to the design's questions:
the name **Tympanum** (a new instrument, not Pulsus grown); the **modelled kits' microphones
move** (pattern, distance, aim); **Grooves later** (BACKLOG §5); a recorded kit plays **in
its own room by default, Cantus's rooms an option**. And for the pictures (2026-11-12):
the designer's **projected-vector render** for both Chorda and Tympanum, in a new scene
module (ADR 0126) that replaces ADR 0104's baked renderer.

## Decision

### 1. A native kind, not a new instrument spec

`InstrumentKind::AcousticDrums` (product name *Tympanum*, `instrument.tympanum`), module
`cantus_core::inst::tympanum`. A native kind gets automation, modulators, MIDI learn,
presets, undo, the agents and a plug-in export by the paths every native number has (ADR
0023); a new `InstrumentSpec` variant would touch every exhaustive match in the workspace
(≈140 sites) for nothing a key cannot say.

**What the project holds** — which kit (the text key `kit`: `model:<id>` or
`pack:<pack>/<instrument>`, `KitRef`) and only what moved from the kit's own values:

| Keys | What |
|---|---|
| `model` (stored), `seed`, `gain`, `humanize`, `hat.cc` (4) | the kit |
| `room` (0 the kit's own — default — 1 Small, 2 Studio, 3 Hall), `amb.level`, `amb.size`, `amb.decay`, `bleed` | the room and the *Ambience* strip; `bleed` is DrumGizmo's one fader, leaving a mic's own pieces alone |
| `p{n}.tune/damp/level/wires/mute` | a piece's sound |
| `m{n}.x/y/z/az/el/pattern` | a modelled kit's mic, moved from its place |
| `b{p}.{m}.db/inv/on` | one cell of the piece × mic bleed matrix (`on` unloads the cell's samples) |
| `o{n}.*`, `outputs` | each mic's internal mixer and route (ADR 0123) |

**What the kit is** — `tympanum_kit::KitDef`: pieces (`PieceDef`: kind, diameter and depth
in cm, a 3-D `Place`, its articulations with their keys, its choke group), mics (`MicDef`:
output id, name, family, channels, pattern, place, the pieces it is the *main* mic of, and
whether it can move), default bleed, the room's size, the default room, the licence and
credit. Built off the audio thread from Cantus's tables (`modelled_kits`, `modelled_kit`)
or from a pack (DrumGizmo XML, SFZ). Never in the song.

Keys: General MIDI drums on their own keys; every other articulation on 88–106 (ADR 0042's
rule). The hi-hat's openness is **continuous** on CC4 (`hat.cc`): the recorded openness
levels crossfaded at the hit and followed during it (an open hat closing as the pedal
falls, the open sample choked into the pedal chick); a modelled hat tightens.

### 2. The engine: Pulsus grown

`cantus_dsp::tympanum*` (owned by `tymp-engine`): Pulsus's modal voices for a modelled kit
(`pulsus_modal`, `pulsus_voice`), a multichannel sample voice for a recorded one, every mic
on its **own lane** (ADR 0123 §3) through the bleed matrix. Rules:

- **One draw per hit, shared by every mic**: `tympanum::hit_draw(seed, key, ordinal)`, a
  pure function of the kit's seed, the key and the hit's ordinal on the track — never SFZ's
  per-region `lorand` (formats.md §1.4), so a close mic and an overhead never play
  different takes. Never the same take twice running (roll smoothing).
- **Virtual mics** (modelled kits): position, distance, aim and pattern give each piece in
  each mic its delay (distance / c, fractional), level (1/r and the pattern's gain at the
  angle), polarity (a snare bottom) and a colour; moving a mic is `m{n}.*`, a command.
- **Streaming**: a recorded kit streams from disk (ADR 0050's reader) — DrumGizmo's
  one-file-per-hit holds every mic, so **one stream per hit serves all mics**; a mic whose
  strip is off (`o{n}.on` = 0) or a bleed cell off loads nothing; the old kit plays until
  the new one is ready. The audio thread never allocates, locks or does I/O
  (`rt_memory.rs`).
- **Rooms**: a recorded kit's own room mics by default; `room` 1–3 puts Aula's responses
  (ADR 0112) on the room outputs instead.
- **Outputs**: every mic a named output; *All to Main · Pieces to strips · Each mic to its
  own strip* (ADR 0123 §4). Split sums to unsplit to the bit.

### 3. Packs

DrumGizmo kits keep their hits **multichannel** at install (`Build::multichannel`, manifest
format 5: 24-bit at the kit's rate) instead of ADR 0042's stereo mix-down. The reader learns
`main` (which mics belong to which drum), `<chokes>`, `<image>`/`<clickmap>`, the metadata
licence (cross-checked against the manifest) and the MIDI map. SFZ multi-mic kits use ADR
0123 §5's rule. The order: **Gogodze Phu II** first (CC0, 7 mics, full bleed — the kit to
build the engine on), then DRSKit per-mic, CrocellKit, Virtuosity, Big Rusty; each licence
and SHA-256 read from its source. The pack's mode (*Pack · From the author's site · Manual*,
ADR 0125 §4) is in the manifest.

### 4. The pictures: the scene module (ADR 0126)

The kit in its room is drawn by the **scene module** (`crates/cantus-ui/src/scene/**`,
owned by the builder `scene`, ADR 0126): the designer's projected-vector render ported to
Rust — a 3-D model in centimetres, projected, facet-lit, depth-sorted vector paths through
the kit's painter, live and interactive, with a **pick** function (which piece or mic is
under the pointer). The owner chose it over ADR 0104's baked scene (2026-11-12). The kit's
geometry and materials (maple shells, coated heads, chrome hardware, lathed bronze cymbals,
stands, pedals) and its mics are ported from `design-p3/src/` by `scene`; `tymp-ui` uses the
module and never edits it (requests go to `scene`). Clicking a piece plays and selects it;
hovering lights its strip; the selected mic's pick-up is drawn as a cone; a modelled kit's
mic is dragged where it stands (`m{n}.*`).

## Consequences

- Schema **67**: the kind; the migration writes nothing. `READY` stays false and the kind
  stays **out of `InstrumentKind::ALL`** until it sounds: every kind-wide test (the contract
  twins, the null corpus and its hashes, the bench) iterates `ALL`. `tymp-engine` adds it in
  the commit that makes it sound, with the null corpus's new item measured and the agents'
  `INSTRUMENTS` entry; the integrator turns `READY` on when the owner has seen the views.
- Pulsus is unchanged: no song with a Pulsus renders differently.

## What the tests hold

- Contract: `tympanum` key tests, `hit_draw`'s purity, the v68 fixture's Tympanum.
- To come: block-size independence at 0 and offline = real time for a modelled and a
  recorded kit; one draw shared by every mic (every mic's take index equal per hit); a
  default key touches no sample; split = unsplit to the bit; streaming never stalls
  (`rt_memory.rs`, a load during playback); DrumGizmo read from the published kit files
  (bytes from outside, not our writer); an e2e through `CantusApp::assemble`.

## Amendment (round 40 phase 3, after checkpoint B) — the acoustic voice, the Feel card

The owner listened to checkpoint B (2026-11-12): *"se escucha como batería; aún no diría
que se escucha real — síguela mejorando."* Measured against the recorded kits piece by
piece (`cantus-ui/examples/tympanum_measure.rs`: the attack's time and band, the octave
bands in six windows after the onset, each band's T20/T40, the low partial early and late,
the level per velocity), Pulsus's model 3 voices were far from them: no velocity → timbre
at all on the kick and toms (the same centroid at 40 and 127), the high bands gone in
30 ms, a snare whose body sat at 3–4 kHz (a recorded snare's at 0.7–1.3 kHz), hats and
cymbals with their energy at 250–1 000 Hz (recorded: 4–16 kHz) and no wash building.

**Decision.** Tympanum's modelled pieces play **Pulsus model 4, *acoustic***
(`cantus-dsp/src/pulsus_acoustic*.rs`), which no Pulsus kit uses — so no Pulsus song
changes (its goldens hold). Its state lives outside `Voice` (`Pulsus::enable_acoustic`,
allocated with the kit; a kit without it plays a model 4 pad as silence):

- the modes are **driven by the strike's acceleration** — the second difference of a
  half-sine force as long as the contact — so the attack has its length, the head's slow
  push is not heard as a thump, and a harder hit (a shorter contact) is brighter;
- the stick's or the beater's own noise; tension modulation (a hard hit starts sharp and
  glides down); the lowest modes in pairs a few cents apart (two heads, beating); the
  shell's few short resonances; the head's dense high modes as two bands of noise;
- snare wires buzzing as the snare-side head moves them — noise gated by the head's
  motion, compressive, so a ghost note is mostly wires;
- cymbals and hats: a dense, inharmonic low bank the stick strikes and a high bank the low
  one drives (its square, amplitude-normalised, and noise at its energy) — the wash builds
  after the hit, as the energy cascades upwards; a hat's low cut follows its openness, the
  plates rattle half open;
- every hit varies its strike point, contact and tuning a little (seeded by the hit).

**New keys** (asked by `tymp-ui`; each null at its default): `timing` (each hit up to that
many ms **late**, seeded — there is no look-ahead, so never early), `vary` (1 never the
take before; 0 a free draw), `hat.chick`, kit-wide `tune` and `ring`, `p{n}.pos` (the strike
point moved towards the centre or the edge) and `p{n}.spread` (its scatter, 0.06 the
model's own), `floor`/`walls`/`rug` (a modelled room's reflections; set, never automated);
`humanize` now plays (each hit's level ± that much). Each mic's peak is in the unit's
telemetry at `METER_TEL + n` (16…). `tympanum_kit::heard_db` is the one estimate of what a
mic hears of a piece, for the engine and the views alike. `PieceDef::label` names a
modelled kit's piece by a locale key; `KitDef::finish` says the shells' finish.

**Rooms on a recorded kit.** `room` 1–3 replaces what a recorded kit's room mics recorded
with one of Cantus's rooms (the modelled kit's late field) fed by its other mics; 0, the
default, is the kit's own room. Aula's convolution responses there are left for later
(BACKLOG): the late field is allocation-free and already measured.

## Amendment (round 40, fix-e) — a modelled kit's pads each have their own id; the seed rule

A modelled kit's pads all carried id 0 (`DrumPad::new`), and Pulsus knows a pad by its id:
a voice follows its pad when the pad is set again (Pulsus III), a pad's polyphony counts the
voices of its id, a note-off chokes the voices of its id, and the foot's chick plays the pad
whose id the closing hat has. So every configure moved every ringing drum to the last pad's
piece, a polyphony limit or a choke would have treated the kit as one pad, and the chick
played the kit's first pad.

- **Each pad's id is its place among the kit's articulations, 1-based** — the pieces in
  their definition's order, each piece's articulations in theirs
  (`tympanum::cache::warm_modelled`). It is fixed by the kit's definition: reordering a
  kit's pieces or articulations changes its ids, and with them its hits' draws, so a
  definition that does is a new model version (as any change of the sound).
- **The seed rule**: a modelled hit's seed is Pulsus's `hit_seed(kit seed, pad id, tick,
  note id)` (the `seed` key, the pad's id, where the hit is — its tick when sequenced — and
  the note's id), and the acoustic voice's identity hashes the pad id. A pad set again with
  nothing changed is not set (`ModelEngine::set_pads`), so a configure that changes nothing
  changes no sample.
- Pulsus's own pads are untouched (their ids are the project's): its seeds and goldens are
  bit-identical. Tympanum had not shipped (it is not in `InstrumentKind::ALL`), so its hits'
  draws moved on purpose: the studio groove's level is the same (RMS −15.09 → −15.03 dB),
  each hit's variation is another (the difference 7.3 dB below the signal), no golden hash
  holds Tympanum, and every Tympanum test compares renders with each other, so none moved.
- Tests: `each_pad_has_its_own_id_and_a_pad_changed_moves_only_its_own_voices` (a lane
  retuning the snare while the kick rings leaves the kick's bits alone).

## Second amendment (round 40 phase 3, after the third listen) — model 5, *measured*

The owner at the third listen: *"suena súper feo, los platillos suenan como vidriosos"*.
Measured (`analysis-drums/report.md`): model 4's cymbals are a few loud, pure, frozen
partials in silence — 4–12 kHz flatness 0.002 against DRSKit's 0.12, the top three
partials 54 % of the energy above 1 kHz against 12 %, a fine structure that never moves.
That is structural: no tuning of a sparse modal bank reaches the recordings (research:
`research-p3/drum-synthesis.md`).

**Decision.** A new Pulsus model, **5, measured** (`cantus-dsp/src/pulsus_measured*.rs`),
that only Tympanum's modelled kits play (`tympanum_voicing::piece_measured`: the crash and
the hi-hat for now; the cache sets `model = 5` and `PadSettings::measured`, the table, with
`MEASURED_FOOT` for the pedal's articulation). Every hit is **synthesised** from small
parameter tables an off-line tool measured off a recorded kit (`tools/tympanum-fit`:
`fit.py`, `tables.py`, `calibrate.py`, its README the steps; DRSKit 2.1's hits through the
engine's own recorded path, CC BY 4.0 — credited in the generated file's header, like a
pack; no sample of it is in the repository or played):

- **modes** — up to 384 per piece (one set per piece: the layers' own, one each, clustered within
  6 Hz or 0.25 %) found standing over the spectrum's local median in a long window, each with its frequency and, per layer, its amplitude, T60, how long it takes
  to grow in (the **bloom**: a band that peaks late lends its modes a rise) and what the
  strike sets ringing at once. Their level per region is fitted in closed loop until the
  synthesis's share of tonal power equals the recording's; no partial above 4 % of the
  energy over 1 kHz, no band's modes over 92 % of its power, none outlasting its band;
- **bands** — the rest as noise in 32 quarter-octave bands, each band's envelope at 22
  breakpoints (0 … 3.2 s), the modes' power taken out, the recording's floor (room, other
  mics) not taken for the cymbal's tail; the energy below 450 Hz held at the median kit's
  (the reference's low end is mostly bleed);
- **the stick** — a sub-millisecond broadband burst and a dull 2-ms knock, fitted to the
  first 3 ms' energy at 0.2–1.5 and 6–20 kHz (the foot has the knock only);
- **states** — the hat's closed, three half-open (CC4 0.25, 0.5, 0.75: the recorded kit's
  pedal at those openings), open and foot states; a hit blends the two nearest states by
  openness and the two nearest velocity layers (below the softest, its colour and the level
  law of the two softest); each table's level corrected by the engine's own render against
  the recording (`gain_db`).

The engine (`measured::Bank`, allocated by `Pulsus::enable_measured` with the kit): each
piece is **one persistent object** — a re-strike adds to its ringing modes and takes an
envelope slot (four) for its bands, so a crash struck while it rings swells; a hat closing
on its own ring (a closed stroke, the pedal) chokes the ring in a second mode bank while
the new stroke sounds in the first; a grab chokes. Per hit: random phases, ± ~1.5 dB per
mode and ± ~0.7 dB per band (seeded by the hit: the same song, the same bits). Modes are
pairs of rotating phasors (the ring less the part growing in), blocks of eight skipped when
silent; one white noise through 32 band-passes per object; envelopes stepped every 16
samples by the object's own counter, so any chunking renders the same bits. The objects'
lanes follow the pads' in `process_buses`. Nothing allocates after the kit is built.

**Consequences.** Through the engine (`tympanum_measure`, the analysis's measures): crash
4–8 kHz flatness 0.12–0.21 (DRSKit 0.06–0.19), top three 7–12 % (5–14), velocity range
23 dB (22), peak at 27–295 ms (65–196); closed hat flatness 0.35–0.40 (0.26–0.34),
fine-structure stability 0.26–0.30 (0.23–0.28); the pedal's first 3 ms dull (0.5–2.1 kHz;
0.3–0.7); the groove's 6–16 kHz flatness 0.27 (0.33–0.36; model 4 0.07). Open: the ride,
the splash/china/stack and the drums still play model 4; the crash's stereo through the
virtual pair is still negative at 250 Hz–2 kHz in the groove (the paths, not the voice).
Pulsus's own kits never play model 5: no Pulsus song changes, no golden moves.

**What the tests hold** (`pulsus_measured_tests.rs`): a model 5 pad without its bank is
silent and nothing struck renders nothing; a groove of every articulation and a pedal move
renders the same bits in blocks of 1, 37, 64, 256, 333 and 512; a re-strike adds to one
object; a closed stroke and the pedal choke the open hat and the foot claps; harder is
louder (and the crash brighter); every piece comes to rest. `tympanum_e2e`: through
`CantusApp::assemble`, the modelled kit's crash and hats sound, live = offline at 512 and
333, the crash's 4–12 kHz flatness over 0.03 and the closed hat's over 0.15 (model 4:
0.002). `rt_memory`'s studio-kit test plays them allocation-free.

**After the fourth listen's measures (before the fifth).**

- **The overheads are never out of phase.** The spaced virtual pair heard every piece as a
  point source delayed differently at each capsule: a comb, and the crash's tail at
  −0.2 … −0.35 between the capsules at 250 Hz–2 kHz where the recordings show +0.1 … +0.4
  (`tools/tympanum-fit/stereo_sim.py` reproduces it from the paths' own rules). Now a
  stereo pair hears every piece's direct sound from the pair's centre (its capsules' places
  still set their levels; the reflections keep their own delays), and a model 5 cymbal —
  an extended plate — gives a **side** signal, uncorrelated with its main (its bands on
  their own noise, its modes in quadrature), that the pair spreads by the piece's `width`
  (`PieceGeom::width`, 0.7 for every cymbal and the hat): left = main − 0.7 side, right =
  main + 0.7 side. Pulsus hands a model 5 lane as main ± side; a mono voice's side is
  exactly nothing, so model 4's pieces are unchanged but for the centring.
- **Steeper bands**: each band is two band-passes in a row (12 dB an octave) — with one,
  a soft crash's strong mids leaked into its quiet highs and washed them with noise.
- **The ride on model 5** (table 3, one plate: the bow and the **bell** as zones, the
  bell's two or three tones at 1.56–1.76 kHz kept at their measured share and T60 —
  the fit now brings the six strongest partials to the recording's share); the splash,
  china and stack play the crash's table sized to them (a smaller plate higher and
  shorter — their own trash is not modelled yet).
- **Re-strike bloom**: a cymbal struck while it rings blooms as a harder hit (up to 12
  velocity units when the ring is as loud as the hit; a hat's plates do not carry over).

**The drums on model 5 (before the sixth listen).** The kick (table 4), the snare (5:
the head, the rimshot and the sidestick as zones 0, 0.5, 1) and three toms (6–8, DRSKit's
12″, 14″ and 16″; a kit's toms take the nearest, pitched by their diameter, the second
centre a semitone up) are measured the same way, with a drum's own rules in the fit: its
partials are its body (shares reckoned over the whole band, a median 60 Hz wide to find
them, up to 60 % of the energy in one), its low end its own, its level the whole band's,
and its **pitch glide** fitted per layer (the strongest low partial at 10–60 ms against
150–450 ms; bounded per piece, never less for a harder hit). The bands now reach down to
35 Hz (38 of them) — the kick's and the floor tom's lows were outside the first 32. The
engine: a drum's modes turn by the glide's ratio every control step until it settles, each
hit a few cents its own (±3) and pushing the glide again; a drum renders no side. What the
recording holds after the strike — the room, the other drums ringing in sympathy — is in
its bands and modes, so the tails no longer fall into black (the kick at 600 ms −32 … −43 dB
against model 4's −82 … −91 and DRSKit's −28 … −36; its 40–100 Hz T60 1.2–2.4 s, DRSKit's
1.5–2.8; the toms at DRSKit's ≈ 79 Hz, their lows 4–7 s; the groove's loudness range 35 dB,
model 4's 71, DRSKit's 43). `p{n}.pos` tilts a measured piece (±3 dB an octave at ±1: the
edge brighter); the kick's `beater` scales its stick's burst (×4 at plastic) and brightens
it. Each key null at its default.

**Output delay lines only where asked** (with `tymp-ui`): a Tympanum's outputs own a delay
line only when `o{n}.delay` is set (`Outputs::with_delays`, the shared
`sampler::outs::delay_mask` rule); `UnitKind::Drums { room, delays }` (was `DrumRoom(u8)`)
builds the unit with its room and its lines off the audio thread, and the plain unit owns
none. `OutDelay::none()` is a line-less output: `set_ms` on it is a no-op (a snapshot ahead
of the unit built with its line plays undelayed until the unit arrives).

**Cost, the blob, the CC0 kit (after the sixth listen).**

- **Cost** (`full_kit_cost`, release, one thread): a busy full kit — 16th-note hats, kick,
  backbeat, ride, a crash a bar, fills — 7.7–7.9 % of a core in Pulsus's model 5 (each piece
  alone 0.5–2 %: the drums ≈ 0.5–0.7 %, the hat, crash and ride ≈ 1.8–2 %).
- **The tables are a blob** (`pulsus_measured.bin`, format `CMT1`: little-endian, a
  version, envelopes as centi-dB `i16`, T60s and rises as `u16`, the direct part as `u8`),
  decoded once with the kit (`measured::tables()`, asked first by `Bank::new`, never on
  the audio thread). Both kits' tables are 497 KB, against 960 KB of Rust source for one;
  `the_tables_blob_reads_and_round_trips_to_the_byte` holds encode(decode(blob)) = blob, and
  a blob of another version or cut short is refused.
- **A measured piece feeds the modelled room less** (0.15 of a modal voice's 0.6): its
  recording's room is in its tables. And no band holds more than the recording's whole
  power at a breakpoint (a narrow low band's filter had smeared a short sound into a tail
  it never had): the sidestick at 600 ms −93 … −96 dB (was −67 … −72; DRSKit's samples end
  at −190).
- **One glide law per drum**: the excess grows with the velocity squared (the head's
  tension with the strike's energy), its largest the layers', one time constant; blended
  as the hit blends its layers — no step between velocities.
- **The jazz kit** (`model:jazz`, "Jazz Birch"): the studio kit's pieces and microphones on
  tables measured off **Virtuosity Drums (CC0 1.0)** — ids 17–24 in the same blob
  (`measured_set`), calibrated against Virtuosity's own hits through the engine. A
  modelled kit whose every table comes from a public-domain recording, should the owner
  want the default free of any credit; the studio kit keeps DRSKit's (credited, `credit`).

## Third amendment (round 40, `drums-hyb`) — the hybrid voice, a recorded residual under the model

The owner (2026-11-13), after the seventh listen: *"un híbrido como opción, pero el generado
debe mejorar mucho más."* Model 5 is far from the recordings mostly in the first tens of
milliseconds (`NEXT-drums.md`: the attacks, the cymbals' granular early wash); a builder
(`drums-gen`) keeps improving the pure synthesis. This amendment adds the hybrid **as an
option**; *Generated* stays the default.

**Decision.**

- **What is stored: a spectral residual, not a waveform difference.** Model 5 draws random
  mode phases, ±1.5 dB per mode and ±0.7 dB per band for every hit (seeded by the hit).
  `recording − model` as waveforms is right only for the one seed it was fitted with; under
  any other it adds the model's part a second time. The fit
  (`tools/tympanum-fit/residual_fit.py`) keeps the recording's own signal under the mask
  `√max(0, 1 − E|M|² / |R|²)` per STFT bin (512/128 at 48 kHz), `E|M|²` the model's power
  averaged over four of its hits: played with *any* hit of the model, the powers add up to
  the recording's. Where the model is louder than the recording (the kick's 6–16 kHz) a
  residual cannot take it away — that is the model's to fix.
- **How long**: per articulation, where the residual's share of the recording falls below
  −10 dB for good (5-ms frames), the 90th percentile of its hits, within 30–200 ms; faded
  out over its last 40 %.
- **Where it plays: at each piece's dry source**, before the virtual microphones — not per
  mic. The recordings' mics are not ours (DRSKit 13, Virtuosity 6, our kit 12, each in its
  own place), a per-mic residual could not follow a moved mic, the bleed matrix or a mic
  turned off, and would weigh twelve times as much. At the source the mics' paths, the
  bleed cells, the room and the outputs apply to it as to the model. The fit deconvolves
  it to the source by the engine's own response: `tympanum_hybrid probe` plays a residual
  of unit impulses through the kit at its default mics and subtracts the generated render
  (everything after the source is linear) — the piece's source → mix response; its
  magnitude smoothed to a sixth of an octave is divided out (zero phase) and its first
  arrival taken off. The residual is mono: a cymbal's side signal stays the model's.
- **Playback** (`cantus_dsp::tympanum_residual::Player`, in `ModelEngine` as `hybrid`): a
  fixed pool of 48 voices adds the residual into the piece's source lane right after
  Pulsus's buses; the take is `take_index(hit_draw(seed, key, ordinal))` — the recorded
  kits' own one-draw-per-hit rule, never the take before; the two velocity layers around
  the hit crossfaded with a gain `1/√(w_a² + w_b² + 2 w_a w_b ρ)` (ρ the layers' measured
  correlation) so the level holds; below the softest layer its level follows the two
  softest layers' law; a hat picks the state nearest its openness (closed key 42, key 46 at
  0.1, 0.5, 0.95, the pedal). Nothing allocates after the unit is built; the residual is an
  `Arc` loaded off the audio thread with each snapshot (`tympanum_hybrid::prepare`) and kept
  for the session. A player without data does nothing: **hybrid off is the model to the
  bit**, and so is hybrid on a kit whose pack is not installed (never silence).
- **The format `CRS1`** (`tympanum_residual.rs` documents it): articulations × layers ×
  takes of `i16` with a scale each, the rate, and the FNV-1a hash of the model's blob it
  was fitted on — a residual fitted on other tables still plays, and the engine logs that
  it is stale. `refit_hybrid.sh <kit> <content> <work>` fits it again in one command.
- **The option**: the text key **`voice`** (`hybrid`, or absent: generated) — schema
  **71**, a migration that writes nothing. `voice::set_voice` is one `SetInstrumentText`
  (exact inverse, one undo step; an undo-fuzz arm among kit changes). The kits list draws
  *Generated · Hybrid* under the kit being played, with *Get · size* while the pack is not
  on disk (choosing *Hybrid* starts the download too), the progress and *Cancel*, then the
  pack's licence.
- **Content and licence (ADR 0030/0042)**: each kit's residual is a downloaded pack — the
  jazz kit's **`tympanum-hybrid-jazz`, fitted from Virtuosity Drums, CC0 1.0** (a
  credit-free default, the pack dedicated to the public domain too); the studio kit's
  **`tympanum-hybrid-studio`, from DRSKit, CC BY 4.0**, credited in the manifest's
  `credits` and `attribution` like DRSKit itself, the change stated in its `LICENSE.txt`.
  The residual *is* processed recording (it plays fragments of the recordings' attacks):
  no byte of it is in the repository; `residual_zip.py` builds the zip and its manifest
  entry, and the owner uploads it as a release asset (`content-r40`) of the public
  repository.

**Consequences.** Through the engine (`hybrid_measure.py`, every articulation at four
velocities): the distance from the recording over third octaves falls by about a fifth in
both the first 50 ms and 50–200 ms, for both kits, and the groove's 6–16 kHz texture moves
toward the recording's (`docs/round40/drums-hyb.md` has the numbers); after 200 ms the
hybrid is the model. Each kit's pack is ~5 MB (5.7 MB of `i16` in memory, one `Arc` per kit
for the session); the player is a few dozen voices of two `i16` reads a sample. A refit is
due whenever model 5's blob changes.

**What the tests hold.** `cantus-dsp` `tympanum_residual`: `CRS1` round trip to the byte, bad
files refused, bytes written by the fit tool's own (Python) writer read, no data touches
nothing, chunking 1/37/64/256/333/512 the same bits, a take played exactly, never the same
take twice running, the velocity law, the hat's states, another rate. `cantus-engine`: the
unit with no residual = generated to the bit, with one heard, blocks 1/64/333/512 the same
bits; `rt_memory::tympanum_hybrid_never_allocates` (playing, the voice switched both ways
while it plays). `cantus-core`: the key, `set_voice`, the undo-fuzz arm; `cantus-project`
`schema_71_keeps_a_tympanums_voice`. `cantus-ui` `tympanum_hybrid_e2e`, through
`CantusApp::assemble`: hybrid before its pack = generated to the bit (never silence); the
manifest's own entry installed by the content manager from `file://`; the residual heard;
blocks 1/64/333 = 512; live = offline; *Generated* through the command = the model to the
bit, its undo the hybrid again.

## Fourth amendment (round 40, drums-gen, after the eighth listen) — the generated sound's causes

The owner: the generated kit must improve much more (a recorded hybrid only as an option).
Measured causes, each fixed in model 5 (pure synthesis; `docs/round40/tymp-engine.md`,
*drums-gen*):

- **A drum's onset is measured**: the blob's **version 2** carries per layer each mode's
  phase at the strike and an early part (amplitude, phase, a time constant of 3–20 ms),
  fitted by least squares to the recording's first 80 ms at the table's own frequencies
  (`tools/tympanum-fit/early.py`); the bands give up that power. The engine scatters the
  phases each hit, the more the higher the mode. Version 1 blobs still read (random
  phases).
- **A head beams its highs along its normal**: a mic off a drum's axis hears it through a
  low-pass at about `2c/(π a sin θ)` (`head_beam`; `PieceGeom::radius` from the piece's
  diameter). Cymbals are not pistons: unchanged.
- **A hat's states blend as recordings crossfade** — partials and noise in power, each
  mode's T60 geometrically, its bloom from the layers that have it; a measured opening
  holds a fifth of the way to the next; a mode absent from a layer rings there as that
  layer's own modes do (the table's union).
- Tables hold up to 512 modes, grouped by the layers that own them (the engine runs only
  the blocks a stroke sets ringing); only the sounding bands' filters run.

Tympanum has not shipped: its tables, seeds and sound moved on purpose; Pulsus's own kits
never play model 5 (no golden moves). Tests: `a_drum_starts_with_the_same_push_each_hit`,
`beam_tests`, the version-2 round trip, block independence and rest as before.

## Fifth amendment (round 40's close, 2026-11-14) — in the picker and in `InstrumentKind::ALL`

The owner: *"Add Tympanum at once"* — with his blind listening of it kept in the backlog
(`docs/BACKLOG-excellence.md`). `tympanum::READY` is true and the kind is in
`InstrumentKind::ALL` (after Chorda, before the dancer), so every kind-wide test now holds it:

- **The picker**: the device picker's *Drums* shelf, beside Pulsus (`picker.dev.tympanum.*`,
  both languages); a new Tympanum plays the modelled Studio Maple kit with the generated
  voice. The agents' `INSTRUMENTS` names it (`tympanum`). E2E: `cantus-ui`
  `tympanum_pick_e2e` (listed in both languages; chosen through `assemble`, a groove plays
  through the real engine wiring).
- **The family's contract** (`cantus-engine/tests/inst_contract.rs`, ADR 0023/0031): its
  split invariance, block sizes, headroom and the at-rest twins pass as every kind's do. Two
  bounds are its own, because its cymbals ring as real ones do — a ride measured off DRSKit
  reaches its −100 dB floor some 37 s after a hit: exact silence 45 s after the last note (6
  s for the others), and rest within 60 s of silence for the defaults and the heavy chord (30
  s for the others; the heavy chord, every cymbal of the kit struck at once, rests after 43 s).
- **As good as new at rest**: the live hits counted so far (the unit's, the model's
  `Pulsus::forget_hits`, the hybrid's `Player::forget_hits`) are forgotten when the unit
  comes to rest — the next unsequenced hit is drawn as a new unit's first, as a reset or a new
  unit would draw it (a sequenced hit takes its own ordinal, unchanged).
- **Checkpoints** (ADR 0031's listening renders): a modelled kit's unit forks (its whole
  state; `ModelEngine` and the hybrid's player are `Clone`); a recorded kit streams from the
  disk's board and does not — a song with one pre-rolls, as Exemplar does.
- **Learn MIDI controller on every control** (ADR 0062): the Kit view's knobs, bars and
  switches and the Matrix view's cells open their parameter's menu (automate, modulate,
  learn, reset) and take the learn mode's click (`controllers::stepped_inst`).
- **Staging**: a new instance on a beat (kick, snare, closed hat, eighths at velocity 100)
  sits at −22.8 LUFS, −4.1 LU from the family's reference, inside its ±4.5
  (`instrument_reference.rs`; a drummer is staged on a beat, not a triad).
- The room's surfaces (`floor`, `walls`, `rug`) were missing their names in both languages
  (the agents' catalogue found them): added; the kit list's *Get* and *On disk* buttons now
  give the help line a sentence (the sweep found six without).
- Its controls' menus offer *Modulate…* as every instrument's do; its Matrix view lists its
  modulation slots (no macro bands: the slots are its matrix).
- The agents' tool list names it (the MCP golden re-written with `CANTUS_TOOLS_DUMP=1`:
  `tympanum` in the instrument enums and descriptions, nothing else).
- **Not yet, on purpose**: no demo song plays it and it ships no mix presets (its kits are
  chosen in its own list) until the owner has judged its sound blind — both listed in the
  tests that would ask for them (`demos.rs`, `preset_audit.rs`), each failing once it is no
  longer true.

Its sound is not called done: the owner's blind listening kit (each modelled kit against the
recorded kit it was measured from, piece by piece and in a groove; ABX, preference and
"recording or synthesis?") and his verdict come first (`docs/BACKLOG-excellence.md`).

## Sixth amendment (round 42, `tympanum-real`) — measured against real kits of many genres; the level law

The owner (2026-11-26): *"a Tympanum le falta mucho"* — download professional recordings free
of copyright, transcribe what is not separated, measure where and how hard each drum is struck,
and iterate against several kinds of kits and genres. `docs/round42/tympanum-real.md` has the
library, the method and every number; `tools/tympanum-real/` the tools.

**The library** (catalogued in `tools/sound-refs/catalog.toml`: `drums-dg`, `drums-packs`,
`drums-freesound`, `drums-gmd`, beside the `tympanum` set): DRSKit, The Aasimonster (DrumGizmo,
CC BY 4.0), Big Rusty, Unruly (sticks and brushes), Gogodze Phu II, Virtuosity, Frankensnare,
Swirly, VSCO 2 percussion (CC0), real playing from Freesound (CC0/CC BY), and the Groove MIDI
Dataset's 1 150 human performances (CC BY 4.0) to drive every kit with the same notes. Every
recorded kit plays through Tympanum's own loader and the engine's render, so recorded and
modelled kits share one path.

**Decision 1 — model 5's level law** (`pulsus_measured_law.rs`, `DSP_VERSION` 19). A hit between
two of a table's four velocity layers blends them, and measured through Pulsus the blend's level
did not follow any recording: the studio kick at velocity 56 louder than at 80, its 100 → 127
step +0.7 dB, the 12″ tom 2.8 dB down from 80 to 96, the jazz ride 5.7 dB quieter at 127 than at
104. Real drummers play mostly there — in the Groove MIDI Dataset the median snare stroke is
velocity 45 and 41 % of snare strokes are ghost notes — so each table now adds, per velocity, the
level that brings its first 80 ms to a law: the studio kit's tables to DRSKit's own (as Tympanum's
recorded path plays it), the jazz kit's to its own made monotone and rising at least 0.1 dB a velocity unit (Virtuosity's 60-dB span from
16 to 127 is that library's curve, which the jazz kit already follows). The hardest hit keeps its
calibrated level; each velocity keeps its colour (only the level moves). `tools/tympanum-real/
law.py` regenerates the table from the `level_law_dump` test whenever the blob changes. The cause
— the blend takes its onset phases from the main layer while it mixes the layers' amplitudes — is
left for model 6.

**Decision 2 — the jazz rack toms' burst** (`BURST`): table 22 alone carried a stick burst, and it
made the toms' first 3 ms swing 0.4–10 kHz from take to take; it is scaled to 0 (the knock stays).

**Measured and left as they are:** the jazz closed hat's 6.1-dB take-to-take change (its close partials beating under the phase draw: a table refit; scaling its level draw moved nothing); the closed hat's length (with the pedal closed, as a song
without CC4 plays it, 241 ms at 8 kHz against the recordings' 193–224); brightness rising with
level holds for kick, toms and cymbals but not for a snare (four of seven recorded snares are
brighter soft — the wires dominate a ghost note).

**Consequences.** The weighted measures outside the recorded kits' range: studio 22.9 → 13.3,
jazz 38.1 → 22.6 (before the burst); no velocity step falls. Pulsus never plays model 5: no Pulsus
song changes and no golden hash holds Tympanum. Open (the doc's list): the grooves' crest 2–10 dB
above the recordings', the 1 kHz band, the jazz closed hat's take-to-take variation, brushes,
per-genre kits measured off Aasimonster / Gogodze / Big Rusty, strike position measured with the
wires off.

**What the tests hold:** `the_level_law_interpolates_and_a_table_without_one_adds_nothing`,
`every_table_grows_with_velocity_at_every_step` (no step falls by more than a hit's own
variation, at least 12 dB from 16 to 127), `a_table_without_its_burst_and_every_other_with_its_own`
(cantus-dsp); block-size independence and rest as before (the law is a gain at the strike);
`every_modelled_piece_grows_with_the_velocity_through_the_app` (cantus-ui `tympanum_e2e`, through
`CantusApp::assemble`).
