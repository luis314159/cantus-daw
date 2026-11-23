# Round 42 — `tuning`: the project's tuning, in every instrument (ADR 0155)

Branch `r42/tuning` from `r42/contract`. The model, the formats and the engine of ADR 0155 in
every pitched unit; `tuning-face` builds the window and MTS-ESP behind the seam made here.

## What was built

**The model** (`cantus-core/src/tuning.rs`, `tuning_temper.rs`):

- `Tuning::offsets()` for every system. `Equal` at any diapason. A **temperament** rotated to
  its key with **A at the diapason** (as Pianoteq and Logic anchor it), built from its
  **construction rule** — which fifths are narrowed by which fraction of the Pythagorean or
  syntonic comma — or its ratios: Pythagorean, ¼- and ⅙-comma meantone, Werckmeister III and
  IV, Kirnberger II (the standard D♭/A♭ spelling) and III (the letter to Forkel), Vallotti
  (modern), Young II, Kellner, Neidhardt III, Lehman's Bach, 5-limit just major (Malcolm) and
  minor (Ellis's duodene), 7-limit (C7 pure). A **Scala scale** through its keyboard map or
  Scala's template (degree 0 on key 60, key 69 at the diapason): any period (Bohlen–Pierce's
  3/1, a one-step 78 c scale), degrees in any order, a linear map (`size 0`), a formal octave
  of any degree, an **unmapped key `NAN`** (it sounds nothing), keys outside `first..last`
  at 12-TET (never silent), every pitch kept between 1 Hz and 20 kHz.
- With a map, the map's reference key sounds at `reference_hz × a4_hz/440`: a `.kbm` imported
  at 440 keeps its numbers, and the diapason lowers the whole map alike.
- `Tuning::resolved_for_key(song_root)`: a temperament that follows the song's key takes its
  root; every table is resolved through it (`TrackTuningSnap::of`, `of_project`), so a key
  change re-rotates with no command of its own.

**The formats:**

- `tuning_scala.rs`: `read_scl`, `read_kbm`, `write_scl`, `write_kbm`, `latin1`, `ScalaError`
  (each refusal with its reason and line). Latin-1 (strict UTF-8 with Ableton's `@ABL`),
  CRLF/LF/lone CR, comments anywhere, a blank description, a count of 0, text after a value,
  tabs, bare integers, `600.`, negative cents, ratios below 1/1, non-ascending degrees, terms
  past 64 bits (kept as their exact cents: `u128` and a 128-bit log2), trailing data ignored.
  A `.kbm` shorter than its size is refused (a missing middle entry would shift every key; Surge
  refuses it too) — write the tail as `x`.
- `tuning_mts.rs`: every message of the MIDI Tuning Standard (`00`–`09`: bulk dump, key-based
  dump, single-note change real-time and banked, scale/octave 1- and 2-byte, dumps of them,
  requests) read and written; the frequency word `/16384` as the spec says, `7F 7F 7F` "no
  change"; CA-020's checksum written, FluidSynth's and Scale Workshop's recognised on read and
  a mismatch reported with the tuning still loaded.

**The engine:**

- `apply_instrument` hands the track's table after every configure (`TrackSlot::set_tuning`:
  Firmus, the native or hosted unit); unpitched units (Pulsus, Tympanum, the dancer) always get
  `EQUAL`, so no keyboard map silences a drum. A note on an unmapped key is skipped where the
  slot plays it (clips, live, MPE).
- Offline copies: every render, freeze, stem and range render installs its snapshot through
  the same path; the listen fork copies each unit with its table.
- **MTS-ESP's seam:** `EngineHandle::set_external_tuning(Arc<dyn ExternalTuning>)` (the handle
  keeps every one it sent: the audio thread never drops the last). Each compile reads the
  master's table into the `External` tracks (an offline render plays that one); the live engine
  reads it once per block and hands it on (`processor_tuning.rs`), allocation-free.
- `Snapshot::tuning`: the project's resolved table, for strips no track owns.

**Every pitched instrument** at its insertion point (`pitch-map.md`), each keeping its own table
and doing nothing for the same table again: Firmus, Calor (voice start, legato, the glide's
`last_pitch`; osc3 keyboard-off untuned), Unda, Modulus (fixed-Hz operators untuned), Gravis
(synth and 808 at `note_of`; the mono string's offset held in `extra` — its fret is
`round(note − open)`; poly strings in each string's base), Lamina (inside the exponent, the
folded key's own offset), Organum (each wheel by its key's offset, a multiplication taken only
when it is not zero), Exemplar (`(key + off − root)·keytrack`, glides between tuned keys, the
streamed zones' mip reach raised by the largest upward offset), Chorda (the offsets in its
technician's table — part of the design key, so a tuning is a design of its own and the same
tuning reuses one; `chorda::with_offsets`; `prepare` asks for the tuned design), Fides
(`Voice::tune_next`, scaled by the keytrack with the key).

**Pitched effects** (`FxUnit::set_tuning`, handed with the song's key): a strip follows its
tracks' table when they share one, the project's otherwise; the master and stems through it the
project's. Tonus (targets and the snap's distances to tuned notes), Saltus (harmony by the tuned
interval, melody to the key's tuned pitch), Nitor (the tonic sub and lift on the tuned tonic),
Loquax (the carrier at the key's tuned pitch); Tabula's pedals hear it.

**Hosted instruments:** at note-on, only when the offset is not zero, a per-note tuning
expression (CLAP `NOTE_EXPRESSION_TUNING`, VST3 `kTuningTypeID`), or on the MPE route the member
channel's bend before the note; every later tuning expression of the note rides on it. The CLAP
test synth already read per-note tuning; the end-to-end test proves it hears it.

**MTS SysEx to hardware:** `ExternalUnit::mts` (off by default, not written; schema 80's — it
is the tuning's), an **MTS** LED in the unit's row (both locales), the MIDI-out thread's
`MidiOut::send_sysex`, and `external_service::send_tunings`: the track's tuning as real-time
single-note changes for every key, sent when it changes or to a new out thread.

## Measured

- **The archive:** 5 401 of 5 401 files of Scala's archive v94 read (90 563 degrees; the most,
  579, in `gann_wolfe.scl`), none refused; each written and read back to the same cents within
  1e-9; the 14 historical files the table cites hold Cantus's temperaments within 0.002 c
  (`bash tools/test/scala-archive.sh`).
- **Temperaments:** every row equals the published table (§5.2) within 0.0011 c; every rotation
  keeps every interval within 1e-9 c, A on the diapason.
- **Pitch, every instrument** (Werckmeister III in E flat at A415, keys 45/57/63/70, the key's
  strongest partial before and after, its frequency from the phase slope over 0.7 s): every
  instrument within **0.04 c** of its table — Firmus, Calor, Unda, Modulus, Gravis (synth,
  mono string, poly strings), Lamina, Organum, Exemplar, Fides within 0.003 c; Chorda's first
  partial within 0.036 c. For the measure, what makes a partial beat is switched off (a
  second oscillator's detune, Calor's drift/chorus/other oscillators, Organum's vibrato,
  rotor, leakage and its 16' wheel, Chorda's unison, coupling, sympathy and phantoms): a
  beating partial has no single frequency to read to a tenth of a cent (with them on the
  offsets measured 0.1–1 c off, both renders' beating differing). The path is the same.
- **Chorda's design:** every one of its 88 designed keys' first partial moves by its table's
  offset within 0.0004 c; the default table keeps the reference design's key, to the bit.
- **The default:** a track playing `Equal` in a tuned project renders the untuned song to the
  bit for every instrument; Pulsus and Tympanum ignore a tuning (its unmapped keys too);
  `tuning_null.rs` + `tools/test/ab-compare.sh` against `r42/contract`: AB.
- **Block sizes** 1, 64, 512 and 1000 equal to the bit with the tuning set; **offline = live**
  (irregular live blocks) to the bit for every instrument but Chorda (whose live keyboard
  comes from its designer: its own tests hold that path); a track following an MTS-ESP master
  plays the master's table live (read each block) and offline (the compile's table), to the
  bit of the same table set as a temperament; no allocation (`rt_memory`: a master whose table
  moves every read, five instruments playing, a temperament installed while playing).
- **MTS:** the MMA's nine consistent example words decode within 0.001 Hz; our dumps decode alike
  in an independent oracle (ODDSound's 0BSD parser's behaviour) within 0.0062 c.

## Verified only in emulation

- MTS SysEx to hardware: bytes on a test port, decoded back; no real synthesizer heard it.
- Hosted per-note tuning: the CLAP test synth; no third-party plug-in (Surge XT, Vital) tried.
- MTS-ESP: a stand-in master in the tests; the library itself is `tuning-face`'s.
- Nobody has listened to a temperament yet.

## For the integrator and the other builders

- **Arcus** uses the same seam: implement `InstrumentUnit::set_tuning(&TuningTable)` — keep a
  copy of `table.0`, and add `cantus_dsp::tuning::at(&table, key)` where the key first becomes
  a pitch (the loop length's target), before the correction table; nothing at the default (`x +
  0.0`), a multiplication only under `!= 0.0`. Reusing Fides's voice: `Voice::tune_next(offset)`
  before `start`/`glide_to`. The track skips unmapped keys already.
- **tuning-face:** the Chorda editor's "designing" caption should ask for
  `chorda::with_offsets(tuning_of(values), &track_table)` (it asks for the knobs' design only).
  An MTS-ESP master's live changes reach every hosted plug-in and every model instrument but
  Chorda, which redesigns at the next snapshot (its design is never made on the audio thread).
- Shared files touched (additive, marked `r42:tuning`): `cantus-core/src/{lib,snapshot,
  external_unit}.rs`, `cantus-core/tests/undo_invariants.rs` (the `mts` switch in the
  `SetExternalUnit` arm), `cantus-engine/tests/rt_memory.rs`, `cantus-engine/src/{backend,
  messages,effects,listen_fork,stems_master,sync_out,external_out}.rs`, `graph/{track,master}.rs`,
  `fx/{mod,tabula}.rs`, `inst/mpe_live.rs`, `cantus-host/src/inst_unit.rs`, `cantus-ui/src/
  {external_service,sync_service}.rs`, `views/external_bands.rs`, `locales/areas/external.*`,
  `xtask/notices/head.md`, `docs/{DEVELOPING,beta-checklist}.md`.

## Left (the owner's)

- Listen: a cadence in E flat and one in F sharp in Werckmeister III on Chorda, Exemplar, Calor
  and Organum (the checklist's *Ronda 42*).
- A synthesizer that reads MTS, and a Scala-generated dump to compare (the checklist).
