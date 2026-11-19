# Microtuning for Chorda and Tympanum — design note (2026-11-13)

Status: the design round 41 builds from (written in round 40's third phase, `research-p3`;
kept here so the next session has it).

Owner's request (2026-11-13): micro-tuning for the piano and the drums, for professional users, as an
advanced option that does not crowd ordinary users. BACKLOG §0.2 already lists "microtonal scales
(Scala files, MTS-ESP), per-note tuning in the piano roll"; this note makes it concrete for the two
instruments.

## Principles

- Default = equal temperament at A4 = 440 Hz, and every new control at its default touches no
  sample (null test to the bit). Nothing new on Play; the controls live in Edit ▸ Tuning, folded
  under "Advanced", plus a one-line summary on Play only when tuning is not the default
  ("Werckmeister III · A415").
- One project tuning that every pitched instrument follows (Chorda, Exemplar, Partes, Pulsus pads
  set to "follow"), with a per-instrument override. MTS-ESP: Cantus as client (follow a master
  plug-in) and as master (drive hosted plug-ins) — check the client library's licence first.
- Formats tested with outside bytes: Scala `.scl` + `.kbm` from the Scala archive (thousands of
  files), MTS-ESP's published SysEx/bulk dump (MIDI Tuning Standard) with real dumps.

## Chorda (the modelled piano) — what a pianist and a technician would expect

1. **Diapason**: A4 = 380–480 Hz (presets 415, 430, 432, 440, 442, 443).
2. **Temperament**: Equal, Pythagorean, ¼-comma meantone, Werckmeister III, Kirnberger III,
   Vallotti, Young, Kellner, just intonation in a chosen key; any Scala file.
3. **Stretch**: off / "as tuned" (the Railsback curve a technician sets, measured from the
   Salamander and real tunings) / amount ±; octave stretch derived from each key's
   inharmonicity, as a real tuner does.
4. **Unison width**: how far the strings of one key are detuned (tight concert tuning ↔ honky-tonk),
   per register — this is also what makes chords "beat"; Chorda's own voicing curve is the default.
5. **Per-key editor** (the professional's tool): a curve over the 88 keys for cents offset and
   unison width, drawn like an automation lane, with a strobe tuner reading each key as it sounds
   (the stretched partial, not only the fundamental), copy/paste, reset per region.
6. Recorded pianos (packs) follow the same tuning by resampling each zone by its cents (stretch and
   temperament are small shifts); unison width is model-only.
7. **Tuning presets by key** (owner, 2026-11-13: "presets for different kinds of tuning for
   different keys — the user may not want equal temperament, to sound unique, then adjust by
   hand"): a preset browser, Tuning section, grouped —
   - *Equal* (concert, stretched as tuned; "studio tight"; "bright, wide stretch").
   - *Historical*, each **rotated to any of the 12 keys** (the temperament's best key on the
     song's tonic): Pythagorean, ¼-comma and ⅙-comma meantone, Werckmeister III/IV, Kirnberger
     II/III, Vallotti, Young, Kellner, Neidhardt, Bach/Lehman — e.g. "Meantone in D",
     "Werckmeister III in E♭".
   - *Just intonation* in each key (major and minor), and 7-limit "barbershop" thirds.
   - *Character*: honky-tonk (wide unisons), old parlour upright (detuned, flat stretch),
     saloon, "slightly out" (random per key, seeded), baroque A415, Verdi A432.
   - *User*: any preset is a starting point — edit per key (cents, unison width), save as your own
     preset, import/export as Scala `.scl`/`.kbm`.
   Each preset shows **which keys sound purest**: a circle of fifths coloured by the beat rate of
   its fifths and major thirds (the way temperaments are compared), and an audition button that
   plays a short cadence in the chosen key. A "follow the song's key" switch rotates the chosen
   temperament when the project's key changes.

## Tympanum (drums) — tuning as a drummer and an engineer do it

1. **Per piece**: Tune in semitones + cents, shown as Hz and note name ("Tom 1: 112 Hz · A2 −18 c").
2. **Two heads** (modelled kits): batter and resonant tension separately — the interval between
   them sets the pitch glide, the sustain and the "boing"; presets "Same pitch", "Reso a fourth
   up", "Reso lower (fat, short)".
3. **Tune the toms to the song**: choose the song's key (or detect it) and an interval set (fourths,
   thirds, a chord); the toms and the snare move to the nearest notes, by cents.
4. **Cymbals**: size/pitch ± (the ride's bell and the crash's partials follow).
5. **Recorded kits**: pitch per piece by resampling (± 12 semitones, cents), every mic of the piece
   together so phase between mics holds.
6. A tuner strip in the Kit view's piece panel that reads the struck piece's fundamental.

## Proof

- Null tests at default; every key's measured pitch equals its table within 0.1 cent (model and
  recorded); stretch against the measured Railsback curve; block-size independence; offline =
  real time.
- Scala parser against the archive's files; MTS round-trip against published dumps.
- Undo fuzz arms for every new command; schema bump + migration + fixture.
- E2E through `CantusApp::assemble`: a project in Werckmeister III at A415 plays Chorda, Exemplar
  and a hosted MTS-ESP client in tune with each other.

## Size

About one builder-round for the shared tuning core + Chorda, half for Tympanum (tension pair and
"tune to the song" are the new DSP; the rest is UI and tables).
