# Round 40 — fix-e: the review of 40a27bb7, Tympanum's and Exemplar's outputs

Branch `r40/fix-e`, based on the integrated commit 40a27bb7. Six findings, each verified
first, then a failing test, then the fix. Nothing here changes a shipped default: every
golden hash stays where it was (no `DSP_VERSION` bump).

## 1. An output's delay was cleared on every change

**Cause.** `OutDelay::set_ms` (dsp `tympanum.rs`) zeroed the whole line whenever the
whole-sample delay changed. `o{n}.delay` is automatable and the automation configures the
unit at every 16-frame grid point, so a 0 → 20 ms ramp played silence for the whole ramp and
dragging the knob clicked. And a line existed only for a delay *set* non-zero
(`sampler_outs::delay_mask`), so automating up from 0 did nothing.

**Fix.** `OutDelay` moved to `cantus-dsp/src/tympanum_delay.rs` and never forgets: it
behaves as a line fed every sample. A change while it holds sound **glides** the read head
(an exponential approach of 30 ms in integer 1/65 536-sample steps, at most half a sample per
sample — the head never runs backwards), reading between samples while it moves; arrived,
it reads whole samples, so a static delay plays exactly what it played. While all its head
can reach is silence the delay is taken at once. `OutDelay::skip` is "fed silence" for a
span the owner does not run, and a line rings until it is all silence at its delay, so the
skip and a unit at rest change nothing it will play — block-size independent, offline =
real time. **The rule for a line** (ADR 0123, fix-e amendment): set non-zero **or driven by
a lane** — `delay_mask | automated_delays(snapshot.automation, track)`, read without
allocating by the new `UnitKind::of_track` (backend, `units_for`, the processor's swap). An
Exemplar with a line renders output by output from the start, so a ramp up from 0 never
switches its render path. A line at 0 passes its input to the bit.

**Tests.** `tympanum_delay.rs`: an automated ramp and a knob's jump glide with no silent
gap and no step above 0.03 on a 0.5 tone at chunks 16, 7 and 1 (and 16 = 5 to the bit); a
static delay (0, 2.5, 50 ms) shifts by whole samples to the bit; skipping silence equals
feeding it at chunks 16, 11 and 3, the delay moved during the silence.
`tests/inst_out_lanes.rs` (the real engine): a lane on `o0.delay` gives the Exemplar its line
(`UnitKind::of_track` → `Exemplar { delays: 1 }`), the same for Tympanum (`Drums { delays }`);
the ramp under a held sine has no gap (every 50 ms window above 0.9 × the plain's RMS) and no
step beyond 1.2 × the tone's own; 512 and 333 frames to the bit. The existing
`exemplar_delay_room` tests (a static delay shifts its lane by whole samples at 512, 333 and
64; no line at default) hold unchanged.

## 2. A whole `configure` on the audio thread every 16 frames under automation

**Cause.** `processor_auto` called `InstrumentUnit::configure` for every moved lane's copy:
a recorded Tympanum dropped and re-cloned every file `Arc`, parsed every `t{i}` key and
rebuilt the 32 × 32 bleed matrix with a linear search per cell; a modelled kit rebuilt every
path; a recorded Chorda (Exemplar, up to 8 192 zones) walked every zone key.

**Fix.** `InstrumentUnit::configure_keys(spec, assets, keys) -> bool` (default false):
`NativeCopy` keeps the keys its lanes drive (built off the thread), and the processor calls
the cheap path first. **Tympanum** sets alone `o{n}.*` and `amb.level` (the outputs), the
Feel card (`humanize`, `timing`) and the bleed levels (`gain`, `bleed`, `p{n}.level`,
`p{n}.mute`, `b{p}.{m}.db`/`.inv`: the cells, and for a modelled kit `ModelEngine::regain`,
the paths from the geometry as set); anything else is a whole configure. **Exemplar** sets
`o{n}.*` alone: `Outs` keeps the keys as stored and derives the mixes, the lines and
`by_output` from them, the same code the whole configure runs. The bleed matrix's kit cells
are now gathered in one pass (was a search of the whole list per cell).

While proving "to the bit" a **latent bug of the whole path** showed: a modelled kit's
pads all carry id 0, and `Pulsus::set_pad` moves every ringing voice whose pad id matches
to the pad it sets — so *any* configure (a snapshot resend, a lane's control point) moved
every ringing drum to the last pad's piece. `ModelEngine::set_pads` now sets only a pad that
changed: a configure with nothing changed changes no sample. The ids themselves:
§7.

**Tests.** `tympanum_fixe_tests.rs`: lanes on `o2.level`/`amb.level`/`o1.pan`, on
`humanize`, on `b0.1.db`, and all at once, every 16 frames over a groove, through
`configure_keys` against a whole configure — the same bits, for the modelled studio kit and
for a recorded kit (the studio's mics, a file per mic); keys that change what the unit is
(`p0.tune`, `o1.on`) refuse the cheap path. `tests/inst_out_lanes.rs`: a modelled groove
with lanes on `o2.level` and `humanize` rendered offline runs fewer than 50 whole
configures for ~12 000 control points (`tympanum::configures()`), 512 = 333 frames.

## 3. Voice stealing cut recorded kits with no fade

**Cause.** `free_voice` → `end_voice` freed every play of the victim at once, against
`STEAL_MS`. A 15-mic kit fills `MAX_PLAYS` (320) with ~21 strikes; a dense groove cut a
ringing crash on every mic: a click.

**Fix.** A stolen voice's plays are *orphaned* (`orphan_voice`): each keeps the gain it has
now (the voice's fade, damping and hat weight, frozen into the play), fades on its own over
`STEAL_MS`, every mic together; the voice slot is free at once. A strike takes plays only
while `RESERVE` (64) stay for those fades; only a burst stealing more than that within one
fade cuts the fading plays nearest their end. `is_quiet` waits for the orphans.

**Test.** `stealing_fades_every_mic_of_the_victim_never_a_step` (dsp): 16 mics, 60 strikes
of a sound that rises over 10 ms and rings at full, 400 frames apart (three times the pool):
no mic ever steps by 0.02 (a cut was a step of 0.71).

## 4. The Feel queue's overflow struck strokes early

**Cause.** A held roll queued all 32 strokes at its note-on; three overlapping rolls (96)
overflowed `MAX_PENDING` (64) and the rest struck at once — a burst, early.

**Fix.** A roll is scheduled **one stroke at a time** (`Pending::roll`): each stroke, as it
sounds, schedules the next, at exactly the frame the whole schedule gave it (its lateness
recomputed from the hit, a pure function). A stroke that cannot wait (the queue full) is
dropped, never struck early; the hit's own stroke then sounds at its written moment.

**Test.** `overlapping_held_rolls_never_strike_early_nor_pile_up_on_one_frame`: three held
rolls 10 ms apart with 12 ms timing, rendered frame by frame: never more than three strokes
on a frame, stroke k never before 55 ms × k, all 32 of each roll heard.

## 5. A recorded kit refused once stayed silent all session

**Cause.** `cache::recorded` added a kit its resolver could not read to `REFUSED`, cleared
only by `set_resolver` at boot.

**Fix.** `cache::content_generation(generation)`: when the content library's generation
changes (an install finished, a pack removed, a user pack added) every refusal is forgotten;
true when one was. **The one UI line** (`cantus-ui/src/samples.rs`, `poll_samples`, beside
the install notices): it passes `Manager::generation()` each frame and, on true,
`resync_engine()` — the snapshot goes again, the kit is read and its files load. No other
UI change: the song already names the kit, so nothing has to be chosen again.

**Test.** `cantus-ui/tests/tympanum_install_e2e.rs` (through `CantusApp::assemble`): a song
naming a kit the machine lacks renders silence and the engine says why; *Install from a
file…* (`tympanum::kits::install_kit_path`, a user pack); the next frame forgets the
refusal; the kit plays offline and live in the same session.

## 6. Tympanum's output delays' memory

**Cause.** `Outputs::new` allocated 32 lines of 50 ms for every unit (~615 KB at 48 kHz,
2.5 MB at 192 kHz) and its reset wrote them all on the audio thread.

**Fix.** `Outputs::new(fs, delays)` builds lines only for the outputs in the mask (finding
1's rule), by output id (`Outputs::out_id` maps each mic to its output's line); a plain
Tympanum owns none. `UnitKind::Drums { room, delays }` (replacing `DrumRoom(u8)`) builds the
unit off the thread with exactly those lines, swapped in with the snapshot; `clear` writes
only the lines it has.

**Test.** `a_unit_owns_lines_only_for_its_outputs_and_a_line_at_zero_passes_to_the_bit`: a
plain unit's mask is 0, `Unit::with(fs, 0, 0b100)` holds o2's line only; a line at 0
renders the bits of no line; at 2.5 ms it delays.

## 7. (leftover) Tympanum's pads all carried id 0

**Cause.** `DrumPad::new` gives id 0 and `pulsus_kit::settings` keeps it, so every pad of a
modelled kit had id 0. Pulsus knows a pad by its id: a pad set again moved every ringing
voice to itself (a lane on `p{n}.tune` moved the ringing kick to the snare's piece); its
polyphony counts, its note-off choke and the foot's chick (which played the kit's *first*
pad) all treated the kit as one pad.

**Fix.** `cache::warm_modelled` gives each pad its place among the kit's articulations,
1-based. The seed rule is written in ADR 0124's fix-e amendment: a hit's seed is Pulsus's
`hit_seed(seed, pad id, tick, note id)`, the acoustic voice's identity hashes the pad id;
reordering a definition's pieces is a new model version. Pulsus's own pads are untouched —
its seeds and goldens are bit-identical.

**What moved.** Each Tympanum hit's draw (Tympanum has not shipped). Measured on a 4-s
studio groove (kick, hats, snare, toms, crash): RMS −15.09 → −15.03 dB, peak 1.254 → 1.212,
the sample-by-sample difference 7.3 dB below the signal — the same kit, each hit varied
another way. No golden hash holds Tympanum and every Tympanum test compares renders with
each other, so **no test or golden moved**; `tymp-engine`'s measuring tools
(`tympanum_measure`, `tympanum_groove`) will see per-hit variation change, not level.

**Test.** `each_pad_has_its_own_id_and_a_pad_changed_moves_only_its_own_voices`: ids are
1…n; a retune of the snare while the kick rings leaves the kick's bits alone (before: the
kick's voice went to the snare's piece).

## 8. (leftover) Exemplar's other automatable keys took the whole configure

**Fix.** `sampler::Unit::configure_keys` sets alone, besides `o{n}.*`: the added room's
level (`room.level`), every global (`gain`, `pan`, `filter.*`, the envelopes, the LFOs, the
macros, `voices`, `mode`…, kept as the last whole configure made them and updated a key at
a time — each key writes its own field), a group's (`g{m}.gain`… the group set again) and
the matrix's amounts (`mod{n}.amt`/`.curve`/`.on`: the matrix rebuilt from its 32 slots,
the whole configure's own code, now `Unit::matrix`). The whole way: a zone's key, the
trim, the capacity, `room.add`, and — while a zone streams — `tune`, `transpose` and `bend`
(they choose the levels a stream keeps, `max_shift_of`). `sampler::configures()` counts the
whole configures.

**Test.** `a_lane_on_a_global_a_group_or_the_matrix_sets_it_alone_to_the_bits_of_a_whole_configure`
(`tests/inst_out_lanes.rs`): lanes on `filter.cutoff`, `amp.release`, `gain`, `g1.gain`,
`mod1.amt`, `o1.level`, `lfo1.rate` and `transpose`, every 16 frames over three held notes
(output 1 on a strip), through `configure_keys` against a whole configure — the same main
and bus bits; a zone's key refuses; through the real engine a held sine with lanes on the
filter and the gain runs fewer than 100 whole configures for ~12 000 control points, 512 =
333 frames.

Nothing is left open.
