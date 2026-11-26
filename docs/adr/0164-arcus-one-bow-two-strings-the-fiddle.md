# ADR 0164 — Arcus's strings under one bow: double stops, a crossing's ring, the sympathetic strings, the grit and the steel set (the country fiddle)

**Status:** Proposed (round 42, the `fiddle` builder; the number is provisional, the
integrator's to confirm) · **Date:** 2026-11-25 · **Builds on:** ADR 0157 (Arcus), ADR 0162
(the player) · **Schema:** 85 (84 reserved for Chorda's branch) · **`DSP_VERSION`:** 17 (16
reserved for Chorda's branch)

## Context

The owner heard round 42's fiddle demos and said: "it still sounds not very real … make it
sound like a country fiddle", then: "the two notes sound like two violins harmonising, and the
fast notes lack something, they sound fake there". The fiddle's double stops and drones were a
second Arcus instance with the same preset: two bows (two attacks, two hair noises, two
players' timings), two pitch loops tuned to equal temperament, a drone 18 velocity steps
quieter. A mono Arcus that moved to another string retuned its one waveguide, so the string
the bow left went silent at once.

The reference study (`docs/round42/fiddle.md` §1, `tools/arcus/fiddle`, 31 measure-only files
catalogued as the `fiddle` set) measured what the ear heard:

- the previous note 60 ms after the next onset: −28.7 dB in our fast runs, −10 to −16 dB in
  dry violin recordings (the string the bow leaves rings);
- 73 % of our fast onsets carried a spectral re-articulation against 10–17 % in real fiddling
  (the demo lifted the bow between strokes);
- in real double stops the lower voice is level with the upper or louder (+1.7 dB), its
  interval 6–9 cents off equal temperament, toward the just ratio in the 78s; ours were
  −4.6 dB and tempered to 0.7 cents;
- the steady noise between the harmonics (2–8 kHz) is far higher in fiddling.

## Decision

Five settings, each **off at its default — nothing changes, to the bit** — and on in the
*Fiddle* preset:

1. **`bow.pair`** (0/1) — **one bow on two strings.** A voice gets a second `BowedString`
   (allocated with the voice) and a second note's pitch state. The bow's gesture runs once:
   both strings read the same bow-velocity buffer and the same hair-roughness buffer, so their
   strokes, bow changes and noise are one, to the sample. Each string's force is its own
   stroke's — Schelleng's limit at its impedance, the shared speed and its own β — times its
   share by the **tilt** (`bow.tilt`, 0.5 even; leaning by the two velocities): the lower
   string `min(1, 2t)`, the upper `min(1, 2(1 − t))`, glided over 8 ms. The contact is at
   **one distance from the bridge**: the second string's β is the first's over the two stopped
   lengths. Each string has its own pitch loop and ear, its own register balance and T60, and
   both sum into the voice's level envelope and the one body.
   - *How a user asks*: in a mono or legato Arcus, a note that starts on the same sample as the
     last one, on another string, joins its voice. A unison, or two notes the first position puts
     on one string, takes the lower to the string below (a fourth finger against the open
     string). A drone needs no new property: a note held under others stays bowed on its string
     (when the allocator's note ends under it, the drone takes over the bow; its note-off is
     the voice's, not the allocator's).
   - *A crossing rings*: a note on another string swaps the voice's two strings. The one the
     bow leaves rings free, its note's stopped pitch, at `ring`'s decay; a ringing string bowed
     again keeps its wave. A third string starts from rest.
   - **`dstop.just`** (0 … 1) tunes a held double stop's interval from the tempered one toward
     its just ratio (16/15 … 15/8). The stopped string moves to the open one, and two stopped
     strings meet halfway.
2. **`sym`** (0 … 2) — **the open strings in sympathy** (`arcus::sym`). The instrument's four
   open strings are loops of their open periods (a fractional delay, a one-pole loss), driven by
   the force all voices put on the bridge and heard there. A string a voice is on (stopped, or
   bowed or ringing in a voice) is damped (T60 40 ms) and not fed. At 1, a string's steady
   answer at a shared partial is 20 dB under that partial in the drive. Its T60 is the open
   string's own, capped at 3 s (the body takes the energy through the bridge). Which strings
   are taken changes only at a note's event, so the samples never depend on where blocks are
   cut. The coupling is one-way: what the bridge gives back to the bowed strings is left out.
3. **`bow.grit`** (0 … 2) — the hair's noise rising near the bridge and with the force: at 1,
   +5 dB from 0.35 of the region's upper force limit to it, and +3.5 dB from β 0.1 to 0.035.
4. **`steel`** (0 … 1) — steel strings: the bridge's corner up by 1.3 octaves, the upper
   partials' loss exponent × 0.6, the ring × 1.5.

5. **`Articulation::Chop`** (appended; a song stores its name) — the chop. Arcus plays it from
   the air at the frog: the force lands past the Helmholtz region at 2.5 × its upper limit, at
   0.3 of the stroke's speed, with the hair's noise +12 dB while the bow presses, and the bow
   stops on the string after 60 ms (the raucous, unpitched crunch). It is never a bow change on
   a ringing string, and its strokes are heard but never trim the pitch loop (as a tremolo's).
   Every other instrument plays it as a staccato: Exemplar's code is the staccato's, MusicXML
   writes no mark, and the MCP enum and the piano roll's mark (a cross) gain it.
6. **`tuning.strings`** (stepped) — the violin's cross-tunings: standard, AEAE, GDGD, ADAE,
   calico (A E A C#) and DDAD (`upper::tables::CROSS`, `VIOLIN_CROSS`). A string tuned `d`
   semitones keeps its length and decay; its tension goes × `2^(2d/12)`, so its impedance and its
   Schelleng limits follow. The fingering, the sympathetic strings and the open-string rule read
   the crossed table; a voice keeps its tuning while it rings; the lines are sized for the lowest
   (DDAD's D3). Standard is the violin's own table, the same reference, to the bit.

The *Fiddle* preset turns on `bow.pair`, `sym` 1, `bow.grit` 1, `steel` 1 and `dstop.just`
0.6. The demos put each drone in the fiddle's own clip, at the melody note's velocity.

## Consequences

- One Arcus plays a fiddle's double stops, drones and crossings as one instrument; the second
  Arcus is gone from the demos (the twin fiddle in *Tren* stays: it is a second player).
- A voice is about twice as heavy while its second string sounds (a free string's loop costs
  what a bowed one does).
- The bowed-pitch tables are per key, so a double stop's strings are each corrected as a
  single note would be. A third string at once (a triple stop) is not modelled: it plays as
  the last two.
- Out of scope, and in `docs/round42/fiddle.md` §5: bow rocking (a tilt curve), the bridge's
  coupling back into the bowed strings, the cross-tunings' own pitch-table rows (an open
  string's bowed pitch is corrected by its key's row, measured on a standard string), and
  the chop measured against a recorded one (none under a free licence was found).

## What the tests hold

- `cantus-dsp` `arcus::tests_fiddle`:
  - every block size gives the same bits (1, 31, 64, 256, 1000) with every setting on;
  - the instance comes to rest exactly;
  - a double stop is one voice, and its F#5 is tuned 0.6 of the way to 5/3;
  - without a pair the two notes slur as before;
  - a string the bow leaves rings under the next;
  - the open strings ring after the bow lifts (> 6 dB over none);
  - `probe_double_stop_levels`: each half of a double stop at its note's level alone, ±0.5 dB;
  - a chop's top over a staccato's (`a_chop_scratches_where_a_staccato_speaks`);
  - every cross-tuning's open strings on their strings, open (`a_cross_tuning_moves_the_open_strings`).
- The null at the defaults:
  - every golden render and `LOWER_HASHES` unchanged;
  - `cantus-engine` `inst_arcus::the_extras_at_their_defaults_change_no_sample` (the keys at
    their defaults; tilt and just without a pair);
  - `every_shape_is_block_size_independent_and_deterministic` with a fiddle shape.
- `rt_memory::an_arcus_fiddle_never_allocates`, on the audio thread and the pool.
- `cantus-ui` `arcus_fiddle_e2e`, through `CantusApp::assemble`, the picker and the preset
  browser:
  - both notes of a double stop sound, and without the pair one is gone;
  - F#5 is tuned toward the just sixth;
  - the sympathetic tail is louder than with none.
- `cantus-project` `schema_85_keeps_arcus_s_strings_under_one_bow` (fixture v85; v83 reads the
  defaults).
