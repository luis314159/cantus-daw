# Round 42 — `arcus`: Arcus, the bowed string (ADR 0157)

*The `arcus` builder's report. Branch `r42/arcus` from `r42/contract`.*

## What was built

**Arcus** (`InstrumentKind::BowedString`): a cello and a double bass modelled, not sampled — a
string in physical units bowed by a force in newtons at a speed and a distance from the
bridge, met by friction, radiated through Fides's body. The design is ADR 0157's *builder's
design* section; the player's view is `docs/manual/en/arcus.md` (and `es`).

- **DSP** (`cantus-dsp/src/arcus*.rs`, MPL-2.0): the string (`arcus_string.rs`: Fides's
  two-loop waveguide on Gravis's public tuning maths, in m/s, each string's impedance from its
  tension; the McIntyre–Schumacher–Woodhouse junction solved in closed form — the hyperbolic
  curve's slip a quadratic's root — with the stick's end and the slip's end **timed between
  samples**; the thermal option; the string's **torsion** at the contact; the bridge's corner
  rounding while bowing), the tables (`arcus_tables.rs`: four instruments' strings, sources and
  status cited), the voice (`arcus_voice*.rs`: the dynamic's bowing map placed in the measured
  region, the attack in measured terms, re-bow, slur, portamento, staccato, spiccato, tremolo,
  pizzicato with **Fides's pluck**, the delayed vibrato on the loop's length, MPE, the hair's
  noise), the pitch (`arcus_pitch*.rs`: the offline table and a **closed loop** that trims it
  from the motion's own slip periods), the render on the pool's lanes into **Fides's body and
  output stage** (`fides::body`, its public API; the bass's body the cello's scaled ×1.72 with
  a radiation corner — no measured bass body was found, said plainly).
- **The model** (`cantus-core/src/inst/arcus.rs`): 47 keys (`model` and `poly` set), the
  choices' names; a new Arcus's four named macros (Expression, Contact, Body, Vibrato — mapped).
- **The unit** (`cantus-engine/src/inst/arcus.rs`): the spec's settings (the Contact and Force
  knobs in ×, as the read-out shows them), routes on the control grid, CC1/CC11/breath/
  aftertouch → the dynamic (`dyn.src`), CC21/CC2/aftertouch → vibrato (`vib.src`), CC65 →
  portamento, MPE, `set_tuning` (the table kept, added where a key becomes a pitch — 12-TET
  adds 0.0, the same bits), `note_art`, `fork`, voices across the pool.
- **The editor**: Play (the instrument drawn — **the picture now belongs to the
  `arcus-look` design agent**, branch `r42/arcus-look`; its API kept), the instrument card with
  its read-out (note, string, dynamic, contact × and β, force × and N with the region, speed ×
  and cm/s); Edit (every part, **the Schelleng diagram** — draggable); Matrix. 24 presets
  (with the four macros), both locales, the manual. Sheets v1 and v2 in
  `docs/round42/arcus/sheet*/` (`examples/shoot_arcus.rs`).
- **Tools** (`tools/arcus/`, `docs/DEVELOPING.md` §4): `listen.sh`, `abx.sh` and
  `abx_phrase.mid` (the ABX's shared notes; `abx_phrase.py` writes it), `compare.sh`,
  `pitch-table.sh`, `gate.sh`.

## Measured (the gate of ADR 0157)

*On a machine running two or three other builds: no CPU claim.*

1. **Schelleng** (`tests_gate2::the_helmholtz_region_has_schelleng_s_shape`, the cello's open
   D): Helmholtz motion holds between a lower and an upper force at every β from 0.06 to 0.22,
   at 10 and 20 cm/s — at 10 cm/s 0.88–2.67 N at β 0.06, 0.40–2.11 N at β 0.10, 0.08–2.21 N at
   β 0.22 (the limits' ratio 3 at the bridge, 28 towards the fingerboard: Schelleng's wedge).
   The lower limit falls as β^−1.7 (Schelleng: β⁻²); the upper only as β^−0.02 — flatter than
   Schelleng's 1/β (the torsion's yield lets a sticking bow hold more at a long β). Against
   Schoonderwaldt 2008: the lower limit moves with the bow's speed here; his machine found it
   fixed. Galluzzo 2014's cello-D figures were not available (paywalled): compared in kind.
2. **Attack** (`…::the_attack_settles_within_guettler_and_askenfelt_s_limits`, every string at
   its open note and a fifth up, velocities 0.15–0.95, the defaults): **0 of 80 over**
   Guettler & Askenfelt's limits — 50 ms (90 for a multiple flyback), in ms, or in the violin G's
   periods (10 / 18) for a string slower than it (the cello's C2, the bass) — the worst the bass's
   B1 at velocity 0.35, 180 ms against its 286. Before the torsion, 8 of 48 test attacks on the
   cello's D settled within 50 ms; after, 15 (`probe_torsion`).
3. **Pitch** (`tests_pitch`, 168 points **held out** of the table: keys 23–49 cents off,
   dynamics and contacts between the grid's): with the table alone, 71 of 1 392 points over a
   cent (worst 12.4); with the first closed loop (timing the main slips) 164 of 168 within a
   cent, median 0.003, four left at 1.0–1.4 (pp far over the fingerboard on the lowest strings:
   there the motion settles for about a second with the slips' period steady and the
   fundamental's phase drifting — `probe_the_last_points`: the force 2–3.6× the region's lower
   limit and 0.33–0.5 of its upper, the bow at speed from the start, so inside the playable
   wedge; nothing to clamp). With the loop that hears the fundamental's phase (`arcus::ear`,
   ADR 0157): **168 of 168 within a cent, worst 0.52, median 0.011 cent** — the gate met.
   Plucked: **−0.0007 cent worst** (four instruments); Werckmeister III in E flat at A415:
   plucked within 0.001, bowed within 0.01 cent of the table's pitch. Before (round 41's Fides
   bow, no correction): +3.1 … −1.2.
4. **Vibrato**: at A3 **6.0 Hz, ±22.1 cents**; each harmonic's swing through the body 1.5–7.5
   dB, all different, and 0.2–1.5 dB without it — the AM is the body's.
5. **Body**: the cello's and bass's signature modes ring within 1 % of the table over the
   body's size; the impulse dies; the loop is passive. Against Fritz 2007: the table's own
   fidelity only — no measured cello admittance was found.
6. **CPU** (`tests_gate2::probe_cpu`, release, best of five, **two other builds running —
   not a claim**): one cello voice with its body and output stage **0.48 % of a core** at
   48 kHz (1.16× Fides's bowed string timed beside it, 0.42 %); eight voices 2.34 % (0.29 %
   each past the first). Sixteen across the pool: the pool = serial tests hold the bits; the
   bench's `inst.arcus.v1/v8/v32` come with `ALL`, the integrator measures quiet.
7. **The house invariants** (all pass): a new instance silent and at rest; every instrument ×
   technique × friction × mode finite and sounding; block sizes 1, 37, 64, 512, ragged the same
   bits; the pool the serial bits; exact rest and as good as new; 12-TET the same bits, an
   unmapped key silent; never silent from a keyboard alone; through the engine `inst_arcus` (7),
   `rt_memory::an_arcus_never_allocates`, `arcus_image`.

**Beside the references** (`compare.sh`; the Karoryfer cello's `A2_*_d.wav` — sounding A3 —
and the Smolken bass's `arco_d2_*_down.wav`, CC0, in `F:\projects\daw-r42\downloads\arcus\`):
the cello's harmonics 2–4 at −14/−34/−33 dB against −22/−35/−40, the same centroid (~230 Hz);
the bass's centroid 175–185 Hz against 200–300; the noise between harmonics within a few dB.

## Levels: the registers and the presets

- **The registers** (`tests_gate2::probe_levels`: a held note every three semitones at velocity
  0.6, integrated LUFS): the cello fell 0.47 dB a semitone from C2 to F♯5 (−21.5 to −39.9
  LUFS), the bass 0.16. A balance around A3 / the bass's middle (`balance_db_st`) evens them —
  the cello −18.6 … −29.0, the bass −12.0 … −21.9 (the body's own note-to-note unevenness, ±5
  dB, kept); the trims 16 dB (the four-string chord at velocity 127 still under full scale).
- **The presets** fitted to their roles (`tools/arcus/presets.sh fit`, Fides's tool with its
  `arcus` argument: each preset's audition phrase through the engine, its gain moved to the
  role's target, never past a −1.2 dBFS phrase peak), with model 2: **21 of 24 within 0.2 LU**
  (Lead −18, Bass −16, Pluck −21 LUFS); *Cello near the bridge* −1.4 LU and *Cello slow bow*
  −4.2 LU stop at the phrase-peak limit (their crest), *Bass tremolo* +0.2. **The default patch**
  (a new Arcus, gain 0) reads about −21 LUFS on the Lead phrase (the *Cello* preset's gain is
  +2.8 dB).

## Matched to recordings (model 2) — `docs/round42/arcus-match.md`

The owner's ask: about 1 060 recorded notes (Iowa MIS anechoic, TinySOL, VSCO 2 CE, Karoryfer,
Smolken) measured beside Arcus note by note; model 2's correction fitted on two thirds,
judged on the third held out: the cello's harmonics 8.7 → 4.1 dB outside the recordings'
spread, its spectrum 8.2 → 3.8, its centroid 7.1 → 1.3 semitones; the vibrato inside the
recordings' depth; the pluck's ring shorter. The bass's correction left flat (it moved the
held-out notes away); the pizzicato's brightness, the attack and the bass's own body open.
Plots in `docs/round42/arcus-match/`.

## Renders for the owner

`F:\projects\daw-r42\listen\arcus\`, **re-rendered with model 2, the registers balanced and the trims at 16 dB** (the first set
played about 5 dB hot; the set before the match is kept in `before\`):
`abx-{cello,bass}.wav` (−1 dBFS peak) and `*-lufs23.wav` (−23 LUFS; the cello's peak there
−8.9 dBFS, the bass's −6.0) from `tools/arcus/abx_phrase.mid`, the same notes `strings-rec`
renders (raw, before the normalising: the cello −32.0 LUFS, peak −17.9 dBFS; the bass −20.2
LUFS, −3.1 dBFS); notes at three dynamics (the cello's mf note −17.6 dBFS peak, −34.3 RMS; ff
−9.1 / −26.4), the reference notes, a legato phrase with a CC1 swell, pizzicato,
tremolo (`tools/arcus/listen.sh … levels` prints every one). The presets on their audition
phrases and as held notes: `F:\projects\daw-r42\listen\arcus-presets\` (`tools/arcus/presets.sh`).

## Fides's bowed pitch (round 41's leftover): Fides's model 2

**Done** (ADR 0150's amendment, `DSP_VERSION` 12). The first try (a model-1 correction table,
`F:\projects\daw-r42\logs\fides-model2-attempt\`) left +3.5 … −1.5 cents and +19.6 at C3:
the STK's bow table locks the period to whole samples. Model 2 (`MODEL` 2.0, stored with every
new Fides; a song keeps its own, the key stored since round 41 — no schema move) gives a bowed
waveguide **string** Arcus's junction (`fides::bow2` on `arcus::string::slip_root`: the
hyperbolic rosin, the stick's and the slip's ends timed between samples), scale-free in Fides's
units (the grip a share of Schelleng's `|v_b|/((μs−μd)β)`, `v₀` half the bow's speed), the
bridge's corner while bowed (3 kHz or 6 × f0), a five-sample bow–bridge segment at least, the
grip scaled by `(350/f)^1.6` above 350 Hz, and the same ear-driven loop as Arcus. Tubes, pipes
and bowed modal objects keep model 1's friction (model 2's junction on a tube grew without
bound; a mode's pitch is its own). Model 1 is the same bits.

- **Measured** (`fides::tests::a_bowed_string_plays_in_tune`, five bowings — pressure 0 … 1,
  speed 0.2 … 1 — C2 to A♯7, 0.6–1.9 s): **model 2 within a cent everywhere, worst 0.61 cent**
  (pressure 0 at G2; C3 and up within 0.09); model 1 at the same points +6.4 … −9.0 (C2–C6),
  −5.0 at C7. The grip's law from `probe_model2_grip`: at the full grip C5 sat −1.0 cent, G5
  −32, C6 −67, C7 −103; the law's grip holds each within 0.1.
- **Through the engine** (`cantus-ui` `fides_bowed_pitch`: the presets as the browser applies
  them, body and output on, vibrato off, C2–G5 after the attack): *Bowed cello* within 0.01
  cent (model 1: +1.96 … +0.16), *Bowed strings pad* within 0.02 (model 1: +1.35 … −0.25). The
  bowed modal presets (Glass harmonica, Bowed vibraphone) are model 1's friction on their modes.
- **The size of the change** (`fides_pick_e2e`'s probe, `FIDES_MODEL` 1 against 2, every
  factory preset's audition phrase and five notes): BITS_LINE
- **Levels**: FIDES_LEVELS_LINE

## Verified only in emulation

- **The sound**: nobody has listened; every number is a measurement.
- **MPE and the controllers** from tests' expressions, never a controller.
- **The look**: shot on the CPU renderer.

## Left

- **The owner's ears** on the renders and the blind ABX, then the one commit that puts
  `BowedString` in `ALL` (the picker, the agents' `INSTRUMENTS` row and the MCP golden, the
  null corpus, the bench, `arcus_pick_e2e`'s ignored picker test, the manual's index, the site's
  card).
- **The owner's eyes** on the face (the `arcus-look` agent's picture).

*Left for the round (not owner items):* the violin and the viola (out of the picker: at 48 kHz
their upper notes leave the bow–bridge segment too few samples — an oversampled junction; their
bodies unmeasured). The pitch gate's last points and Fides's bowed pitch are closed (above).

## Tests (Windows, this branch, at the end)

`cantus-dsp` (release) the whole lib 1 356 passed after Fides's model 2; then `arcus::` and
`fides::` 32 passed (+13 probes ignored) with model 2's match — the held-out pitch gate 168 of
168 within a cent, Fides's bowed pitch within 0.61 cent C2–A♯7 at five bowings, the attacks,
the vibrato, rest, block sizes, the pool; `cantus-engine` + `cantus-core` (debug, every test)
**2 447 passed, 0 failed**; `cantus-ui` `arcus_pick_e2e` 1 (+1 ignored until `ALL`),
`fides_pick_e2e` 2 (+ its bits probe), `fides_bowed_pitch` 1, `kit_lint` 4; clippy
`--workspace --all-targets -D warnings` clean; rustfmt. Not run here: the Linux check
(WSL is the integrator's), the whole UI suite.

## For the ALL commit (after the owner's verdict)

`READY` true and `BowedString` in `InstrumentKind::ALL`; the picker's row (Other instruments);
the agents' `INSTRUMENTS` row (`arcus`, *Arcus*) and the MCP golden (`CANTUS_TOOLS_DUMP=1`);
`inst_contract`'s `per_kind!` and count; the null corpus's `inst.arcus.*` and the bench come from
`ALL`; un-ignore `arcus_pick_e2e`'s picker test; the manual's index line; the site's card
(`web/src/units_data.rs`, within the wasm budget). A new Arcus stores its four macros' names and
routes (`NativeSpec::new`), so a test that expects a fresh spec to serialise to its kind alone
must not iterate `ALL` blindly (today only Organ is checked).
