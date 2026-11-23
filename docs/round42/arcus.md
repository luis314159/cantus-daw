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
   cent (worst 12.4); with the closed loop, **164 of 168 within a cent, median 0.003 cent**, the
   four left 1.0–1.4 cents (pp far over the fingerboard on the lowest strings). The gate's
   last 2 % is **not yet met** (below, *Left for the round*). Plucked: **−0.0007 cent worst**
   (four instruments); Werckmeister III in E flat at A415: plucked within 0.001, bowed within
   0.01 cent of the table's pitch. Before (round 41's Fides bow, no correction): +3.1 … −1.2.
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

## Renders for the owner

`F:\projects\daw-r42\listen\arcus\`: `abx-{cello,bass}.wav` (−1 dBFS) and `*-lufs23.wav`
(−23 LUFS) from `tools/arcus/abx_phrase.mid`, the same notes `strings-rec` renders; notes at
three dynamics, the reference notes, a legato phrase with a CC1 swell, pizzicato, tremolo.

## Fides's bowed pitch (round 41's leftover)

**Tried, not shipped.** A model-2 table for Fides (key × pressure × position, two passes) was
built (`F:\projects\daw-r42\logs\fides-model2-attempt\`: the code, the tool, the table): at
Fides's default bow it left +3.5 … −1.5 cents and +19.6 at C3 with a light bow near the bridge.
Fides's bow (the STK's table) locks the period to whole samples and is chaotic at light
pressure; a table cannot fix either. The cure is Arcus's: the slip timed between samples and the
closed loop — a Fides model 2 built on Arcus's junction. Nothing of Fides changed; its golden
hashes did not move.

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

*Left for the round (not owner items):* the pitch gate's last four held-out points (pp sul
tasto on the lowest strings); the violin and the viola (out of the picker: at 48 kHz their upper
notes leave the bow–bridge segment too few samples — an oversampled junction; their bodies
unmeasured); Fides's model 2 on Arcus's junction.

## Tests (Windows, this branch)

`cantus-dsp` arcus 17 (+5 probes ignored: Schelleng, curves, torsion, harmonics, quantization,
CPU) — the pitch gate held as measured (164/168; the gate's last four points open); `cantus-core`
inst (arcus 2 among them); `cantus-engine` `inst_arcus` 8, `arcus_image` 1,
`rt_memory::an_arcus_never_allocates`; `cantus-ui` `arcus_pick_e2e` 1 (+1 ignored until `ALL`:
the picker), the lib's skin, i18n and arcus tests (41), `kit_lint`; clippy `--all-targets -D
warnings` on dsp, core, engine and ui; rustfmt. Not run here: the Linux check (WSL is the
integrator's), the whole UI suite.

## For the ALL commit (after the owner's verdict)

`READY` true and `BowedString` in `InstrumentKind::ALL`; the picker's row (Other instruments);
the agents' `INSTRUMENTS` row (`arcus`, *Arcus*) and the MCP golden (`CANTUS_TOOLS_DUMP=1`);
`inst_contract`'s `per_kind!` and count; the null corpus's `inst.arcus.*` and the bench come from
`ALL`; un-ignore `arcus_pick_e2e`'s picker test; the manual's index line; the site's card
(`web/src/units_data.rs`, within the wasm budget). A new Arcus stores its four macros' names and
routes (`NativeSpec::new`), so a test that expects a fresh spec to serialise to its kind alone
must not iterate `ALL` blindly (today only Organ is checked).
