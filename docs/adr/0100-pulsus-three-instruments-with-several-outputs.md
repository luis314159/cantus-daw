# ADR 0100 — Pulsus III: instruments with several outputs, a kit split to strips

**Status:** accepted for the first delivery (round 37, drums builder; items 2–5 — chains per
pad, kits from folders and SFZ, original kits, the agents — follow in this ADR) · **Date:**
2026-11-04 · **Builds on:** ADR 0023 (native instruments), ADR 0031 (the graph and the pool),
ADR 0038 (voices across the pool), ADR 0022 (what is heard), ADR 0064 (freeze), ADR 0092 (the
recording mode), ADR 0093 (VCAs) · **BACKLOG:** §3e item 3 (and item 11's *per-microphone
faders*, which this contract serves next) · **Schema 56**

## Context

Every leader lets a drum instrument's pads leave on mixer channels of their own: FL's FPC
outputs, Ableton's Drum Rack chains, Logic's Drum Machine Designer (a summing stack), Bitwig's
Drum Machine. Pulsus II had the model (`DrumPad::output`, stored and ignored since round 17) and
a DSP hook (`Pulsus::process_add_bus`) that nothing called, whose buses summed back to the kit
only to 1e-6 — the order of the additions differed. The engine's tracks had one output each,
and every system that reasons about "what feeds this strip" (freeze, the recording mode, solo,
stems) knew only `Track::strip`.

The owner's bar (BACKLOG §3e item 3): *split to 16 strips in one click; the split kit sums to
the unsplit kit to the bit.*

## Decision

### 1. Model 3 sums each pad on its own lane

Floating-point addition is not associative, so "the buses together are the kit, to the bit"
holds only if the unsplit kit adds the very same partial sums in the very same order. Kit
**model 3** (`PULSUS_MODEL`, `cantus_dsp::pulsus::LANES_MODEL`) renders, every stretch of up
to 256 frames, each pad's voices (in voice order) into a lane that starts at +0.0, then hands
the lanes out in **pad order** (`Pulsus::process_buses`). Unsplit, the lanes are added into
the kit's buffer; split, each lane goes to its pad's bus and the kit's strip gathers the pad
strips — in pad order, because *Split to strips* places them in pad order and the canonical
gather (ADR 0031 §1.3) adds strips in processing order. Same lanes, same order, same bits.

Models 1 and 2 keep their sum in voice order, so **every old song renders as it did** (the
goldens do not move; `DSP_VERSION` stays 5). A kit made now is model 3. Measured: model 3
differs from model 2 on a busy groove by at most 1.8e-7 (−135 dBFS) — the rounding of the
sum's order, nothing else. A model 1 or 2 kit can be split (its pads are routed and the lanes
used), but its split sums to its unsplit self only within that rounding; the kit's existing
*upgrade* is offered to models 1 and 2 and takes either to 3 — from model 2 it changes only
the sum's order (inaudible by measurement, but it makes a split exact), and its tip says the
measured difference (`pulsus.upgrade_lanes_tip`). *(Corrected in round 37's fixes: the first
text said a model 2 kit was offered none; the code and the manual offer it.)*

A pad's ringing voices follow it when its output changes (`set_pad` retargets them), so a
pad is heard where it is routed now, all of it.

**The condition, stated (review of the first delivery, item 5).** The kit track's own level
and balance multiply each bus (`Σ(lane·g)` split, `(Σ lanes)·g` whole): equal to the bit when
the track's level is 0 dB and its balance centred (the default, and every split the tests
make); at any other level, or under an automated level, the two agree within rounding —
tested at −4.5 dB with a balance of 0.3 and under a level ride, within 1e-6
(`at_a_kit_level_off_unity_the_split_agrees_within_rounding`). The kit's strip's own fader,
pan and inserts act after the sum and change nothing of this.

### 2. Several outputs: buses, carried as tap tracks

The engine's contract (`cantus_core::InstrumentSpec::output_strips`): an instrument may play
into strips besides its track's. **Bus 0** is the track's own output; bus `k ≥ 1` is the
`k`-th strip its spec names (for Pulsus, the distinct `DrumPad::output`s in pad order; pads
naming one strip share a bus). `InstrumentUnit::process_buses` (default: everything on bus 0)
is the same for native units — an Exemplar kit's microphones (BACKLOG §3e item 11) are its
next user; nothing else implements it yet.

`Snapshot::compile` appends, **after the project's tracks** (a project track's index is still
its index in the project), one **tap** per bus: a `TrackSnapshot` with `tap: Some(Tap { source,
bus })`, the source's level, balance, audibility and lateness, no events, and the bus's strip.
The source lists its taps in `buses`. A tap's id is `TAP_BIT | source << 8 | bus`
(`padout::tap_id`): never a project id.

On the audio thread (`cantus_engine::padout`) the source renders every bus in one sweep, bus 0
into its own buffer and bus `k` into its tap's preallocated buffer, at the segment's offset;
it hands its taps its level, balance, audibility and their automation ramps each pass (the
channel's own level is every output's). The plan gives a tap an edge from its source
(`plan_compile.rs`), so the pool never runs a tap before its source; a tap is otherwise **a
track like any other** — its own compensation line, peak, stem tap and strip. That is the
point of the design: delay compensation, sends, VCAs, solo and mute, stems, freeze and the
recording mode reach a pad's strip by the paths every track already has, instead of a second
set of paths for "outputs".

Rejected: a second output buffer set per track node with a new `Input::Bus` in the plan — it
would need its own compensation lines (`Edge::Bus`), its own stems, its own freeze taps and its
own entries in every rule about what a track feeds; and preallocating 32 buses for each of 256
track slots is 64 MB.

### 3. A strip's own pan law

The project's default law is −3 dB at centre (constant power), applied at **every** strip — so
a pad strip feeding the kit's strip would take the kit 3 dB down. A strip gains its own law,
`MixerStrip::pan_law: Option<PanLaw>` (REAPER's per-track law; `None` follows the project).
*Split to strips* gives each pad strip the **0 dB law (a balance)**: centred it passes the pad
untouched, and the kit's strip places the kit through the project's law as before. Hence the
split kit at unity sums to the unsplit kit to the bit **under any project law**.

### 4. Split to strips, Merge back

`padout::split_with` (one `Batch`, `cmd.split_kit`): for every pad that sounds (a synthesized
voice, or a sample pad with a file or a layer) and is still on the kit, an `InsertStrip` — named
after the pad, placed right after the kit's strip in pad order, output into the kit's strip (the
master when the kit has none), the 0 dB law, the kit strip's dock — and a `SetPadOutput`.
`padout::merge` (`cmd.merge_kit`): every pad back on the kit, and each strip the pads played on
removed unless something else uses it (another track or pad, a strip routed or sending into it,
an insert keyed from it, a strip following it as a VCA). Each is one undo step; undo of either
is exact. The new command `SetPadOutput { track, pad, output }` refuses a strip that is not
there or a VCA; `RemoveStrip` refuses a strip a pad plays on; `first_free_strip` skips one.

Why into the kit's strip, not beside it (FL's way): three of the four references sum the pads
back into the kit (Ableton, Logic, Bitwig), which keeps the kit's strip a kit bus — its inserts,
fader and VCA govern the whole kit — and makes *Merge back* a plain reversal.

### 5. What sees the new strips

- `Project::tracks_on_strip` includes a track whose pads play into the strip, and
  `Track::destinations` lists every strip a track is heard on: freeze regions (a pad strip's
  region holds the kit; the kit strip's region holds its pad strips), solo-follows (`solo_play`),
  the dormant rule (a kit wakes when any output is heard) and the record mode's regions use them.
- A tap rests when its source does not play its instrument or its strip does not run or plays a
  render (`padout::settle_taps`, called by `freeze::settle`, `freeze::isolate` and the record
  mode's `rest_for_master`).
- **Freeze**: a split kit's *track* cannot be frozen (`CannotFreeze::SplitKit`: one render of a
  track's source cannot hold several outputs); *Freeze* on the kit freezes its strip
  (`freeze_target_of_track`), which holds every pad strip. **The recording mode** bakes the kit's
  strip (or, for a kit straight to the master, each pad strip — the track itself stays live,
  `Why::SplitKit`).
- **Stems**: every pad strip is a strip stem; a kit's track stem is all its outputs (each tap
  adds its product into its source's track stem; a tap into a pad's chain, the chain's
  output — §12).

### 6. A chain per pad

A pad's chain is **a strip of kind `Chain`** (`vca::StripKind::Chain`) that the pad plays
into (`DrumPad::output`) and that outputs where the pad is heard — the kit's strip, or the
pad's own strip when the kit is split (`padout::heard_on` follows the chain). Being a strip
it is everything an insert chain already is: every Cantus effect and hosted plug-in (the
strip's own insert mechanism, so hosting cost nothing to add), keys, delay compensation,
automation, freeze. The desk does not show it (`mixer_desk.rs`); the pad's card does — its
inserts, each with its switch, *Edit* and *Remove*, and *Add insert ▾*
(`views/pulsus_editor_chain.rs`). `padout::chain_add` makes the chain with its first insert
(one batch: the strip, the pad onto it), `chain_remove` takes the chain away with its last —
so a pad with no insert has no chain. *Split to strips* moves a chained pad's **chain's**
output onto the new strip; *Merge back* moves it back and keeps the chain.

**A chain at rest costs nothing.** The engine runs a strip with inserts on every pass even
on silence (the inserts' `at_rest`/`skip` contract, ADR 0031 §5.1, was not used yet). A
chain strip now rests when nothing came in, its EQ and lines are quiet, no key or note
reaches its inserts and every insert is at rest: its inserts are told to `skip` and nothing
runs (`EngineStats::chain_rests` counts it). Measured (`a_chain_at_rest_costs_nothing`,
processing only): the studio kit with a chain on every pad and only the kick playing,
0.027 s, against 0.018 s with no chain at all — the one running chain and the taps' copies.
Exact: the same chains as ordinary strips (which never rest) render the same bits at 512
and 333 frames (`a_resting_chain_is_exact`). Generalising the rest to every strip is the
next step of ADR 0031 §5.1 and is not taken here (only the pad chains are this ADR's).

Sixteen pads, each with its own chain, split to sixteen strips sum to the unsplit kit to
the bit, offline at 512 and 333 frames and live on three workers
(`sixteen_chained_pads_split_to_sixteen_strips_sum_to_the_kit_to_the_bit`).

### 7. Kits from outside

`pads::import::kit_from_files` maps a folder (three levels deep, path order): the role a
name says in English or Spanish onto the General MIDI drum map (kick 36, rim 37, snare 38,
clap 39, closed/pedal/open hat 42/44/46 in one choke group, toms 41/43/45/47/48/50, crash 49,
ride 51, cowbell 56, percussion 54/58/60…), numbered takes as one pad's round robin
(*Cycle*), dynamics (*soft, hard, pp…ff, vl1…*) as velocity layers softest lowest (both:
velocity layers each cycling its takes), names that say nothing onto the next free pad in
order; at most 16 layers a pad and 32 pads, what does not fit said in the notice and the
log. `kit_from_zones` maps an SFZ kit read by Exemplar's SFZ reader (includes, defines):
one pad per key, its regions as layers (velocity range, tune, level, pan), a sequence as
*Cycle*, a random range as *Random*, and SFZ's exclusive groups — `group=` (the opcode,
not the `<group>` header) cuts every key whose `off_by` names it; a cut is one-way in SFZ and
a Pulsus choke group is shared, so keys that cut each other either way are joined into one
group, at most 16 (review 2: the first reader read the header's index). Exemplar's reader
keeps exclusive groups up to 63; a region above is not read as a choke and the log says so.
A kit whose hats (42/44/46) link any one of them chokes all three, as General MIDI kits do
(Virtuosity's open hat cuts nothing back, and its *silence* regions are not samples).
Names keep a number that names the drum — a leading one (*808 Kick*) and a tom's or a
cymbal's (*Tom 1…3* high to floor, *Crash 1/2* on 49/57) — and read any other as a take.
A folder is read bounded (512 sounds, 8 192 entries, 4 GB) on a thread of its own, landed by
the next frame; one dropped on no kit must name a drum in some file. Agents reach it through
`make_kit_from_folder`, a *Files* tool: only inside the project's folder and the folders the
user allowed (`resolve_touch`, every file and include judged by `may_touch`), never onto a
track that is not a kit, answering file names only. Tested with **Virtuosity
Drums' published `01-basic-kit.sfz`** and the 122 files it includes (CC0, byte for byte,
`crates/cantus-ui/tests/fixtures/sfz/virtuosity_drums`). A folder or an `.sfz` dropped on a
kit's pads loads onto that kit (its pads' outputs and chains carried by pitch, then voice);
dropped anywhere else it is a new kit track; one undo step either way.

### 8. Original kits

Sixteen factory kits — classic, studio, 808, 909, boom bap, electro, percussion, trap, and
now **house, lo-fi, rock, pop, R&B, reggaeton, cinematic and jazz brushes** — every one
synthesised by Pulsus itself: no sample, nothing from a machine or a pack. Each carries the
library's style and role keys (`preset.style.*`, `preset.role.drums`, ADR 0101) besides its
descriptive tags, so the preset browser's chips find it and the kit floor (16) is met; the
kit's ⋯ menu opens that browser on the kit (*Browse presets…*), and a kit loaded from it is
one undo step.

### 9. What goes with the kit

The strips made for pads are marked by their kind — `StripKind::Pad` (made by *Split to
strips*; schema 57 marks the v56 ones) and `StripKind::Chain` (`padout::pad_made`); an own pan
law no longer means pad-made (review 2: a user's strip with its own law is the user's, and a
VCA has no law to offer). They go with what made them: loading another kit carries the pads'
outputs over, every pitch first, then the voice; another instrument, or the kit's track
deleted, removes the pad strips and chains nothing else uses, in the same undo step — **in the
command bus itself** (`CommandBus::execute_inner`: `padout::may_orphan` asks every command,
`with_orphans_removed` wraps `SetInstrument`, `RemoveTrack`, `RemovePad` and any batch holding
one), so every path — a swap, a sound, a hosted pick, a preset, an agent — is covered by one
place. A chain is not on the desk (`MixerStrip::on_desk`): no cable, matrix cell, route menu
entry or stem shows it, and the agent's mixer JSON names every strip's `kind`. *Merge back* removes such
strips left before; `first_free_strip` never hands one to a new track. A bounce in place of
a split kit's track bounces the whole kit (every pad summed as the kit sums it, to the bit,
into the kit's strip: `padout_bounce.rs`), and says the pads' strips are not in it; a pad's
chain is the instrument's sound and is in the bounce (the kit's output and its chains summed
at unity before the kit's strip, `padout::kit_sum_strip`), split or not; a pad's
own strip refuses a bounce and points to the kit's strip. DAWproject writes a split kit
whole and says so (`x_split_kit`, Dropped, counting the pads heard on strips of their own),
a pad's chains likewise (`x_pad_chains`) and a strip's own pan law (`x_pan_law`); freezing a
kit's track and the recording mode say *chains* when the kit has chains but is not split; the reader clears a pad's output, whose strip id would not survive the trip.
A strip's own law is shown and set in its menu (*Pan law: its own ▸*, `SetStripPanLaw`).

### 12. The integrated review's fixes (round 37, `docs/round37/fixes.md`)

- **A kit's strips follow the kit.** Routing a kit's track to another strip (`SetTrackStrip`
  — the mixer, `CantusApp::route_track`, the agents' tools) re-points, in the same batch,
  every strip made for its pads that played into the old strip — its chains and, split, its
  pads' own strips — onto the new one (`padout::with_kit_strips_following`, beside
  `may_orphan` in `CommandBus::execute_inner`: one place for every path). A chain into a
  pad's own strip stays on it. A re-pointing that would make a loop is left out. Before, a
  chained pad stayed on the old strip through a hidden chain: `heard_on` read it as split
  (*Merge back* offered, freeze refused) and `first_free_strip` handed that strip to the next
  new track. `first_free_strip` now also never counts a strip a chain or a pad's own strip
  plays into as free. The undo fuzz's padout stream routes kits (its kind 8).
- **No room, said.** Every bus of a kit is a track the engine plays (a tap) and every split
  makes strips; past `MAX_TRACKS`/`MAX_STRIPS` (256 each, `padout::ENGINE_TRACKS`/
  `ENGINE_STRIPS`, held equal to the engine's by a compile-time assertion) a pad fell
  silently back onto the kit's strip. *Split to strips* and a chain's first insert are
  refused (`Refusal::NoRoom`, `padout.no_room` in both languages, the agent's refusal
  likewise) when they would pass either.
- **Templates and takes.** A kit's track template keeps the strips made for its pads (chains,
  pads' own strips) and a pad routed to a desk strip as a stand-in by name; inserted, they
  are new strips with new ids, effects and modulators remapped, placed after the track's
  strip, and a pad output the template does not bring is cleared (an older template's raw
  ids never name another project's strips). Exploding a kit's takes gives the copies no pad
  outputs: the chains are the kit's.
- **Stems.** A chain straight to the master (its kit has no strip) is part of the Direct
  stem (the engine adds the chain's output edge into it, as the master takes it; the export
  lists Direct when such a chain exists), so the strips' stems add up to the master. In a
  track-stems export a kit's track stem holds its pads' chains — the chain's output after its
  fader, in place of the dry pad (the engine's `StripSlot::chain_of`, `Outs::into_chain`) —
  as §9 says the chain is the instrument's sound. A chain whose inserts have latency is in
  the track stem late by that latency (the track stem is taken before compensation); the
  strips' stems are compensated as always.
- **One free-strip rule** (second review): `Project::strip_is_free`/`free_strips` is what
  `first_free_strip`, the audio import, a sample dropped on the rack, *Convert to MIDI…*,
  *Double* and the MIDI import hand out — two copies had never skipped pad strips, chains
  or VCAs. A kit template inserts its pads' own strips before its chains (the undo, in
  reverse, takes a chain before the strip it feeds) and keeps no VCA ids.
- **A sample kit carries outputs by name.** `carry_outputs` matches by pitch, then by voice
  only for synthesised voices (every pad of a sample kit is `Sample`), then by name, then by
  place.

### 11. A kit's own gain (schema 57)

`KitSettings::gain_db` (−24…+12 dB, step 0.1; *Kit level* on the kit card, `KitField::Gain`)
scales the kit after its pads: in model 3 each lane before it is handed out — so the split
kit at unity still sums to the unsplit kit to the bit, every bus scaled alike — and in models
1/2 each voice's gains. At 0 dB it is exactly 1.0, multiplied nowhere: a null at rest, no
golden moved, old and user kits unchanged (`the_kits_gain_is_a_null_at_unity_and_keeps_the_split_exact`).
It is stored (`skip_serializing_if` 0), so schema 57 (`migrate_56_to_57` writes nothing for
the gain; it marks v56 pad strips `StripKind::Pad`).

**The decision** (the integrator, 2026-11-06): Pulsus's single-pad rule stays (no pad alone
over 0.6 peak, which a kit gain also raises), and the drums role's window is **−19 ±2 LUFS**
on the groove phrase (library's constant: drums' crest factor; −16 sat above the instruments'
−18.7). Each factory kit's gain is the least of what brings its groove to −19, the room its
loudest single hit leaves under the rule, and what keeps a dense riff under full scale
(`instrument_levels.rs`). At −16 under the rule, fourteen kits sat 2.3–5.9 LU low.

**Rebalanced by the groove's balance, measured pad by pad** (each pad of the groove alone,
LUFS integrated): the groove's loudness is the kick's — classic's kick alone −20.7 LUFS of the
groove's −21.0, the snare 6.6 LU and the closed hat 22.5 LU under it. Lowering the kick to let
the kit's gain rise (the first idea) *lost* loudness — classic −21.2 → −21.0, studio −21.7,
jazz −22.8 — and pushed the dense riff past full scale (classic +0.1, rock +0.6 dBFS). What
works is the kick's **crest factor**: drive (`DrumPad::drive`, +0.4) gives it the same peak
and more body (classic's kick alone −20.7 → −18.6 LUFS, its single hit −4.8 → −4.7 dBFS),
as a mixed record's compressed kick has; and the snare and the hats come up toward it —
snare +3 dB (rock +2, studio +2), hats +3 dB — so classic's snare sits 8.7 LU under the
kick (11.7 with the drive alone) and its closed hat peaks 4.5 dB under the kick in the groove
(−11.7 against −7.2 dBFS; the hats' loudness stays about 25 LU under, as sixteenths of a
short closed hat are). The jazz kick stays under the
brush snare's peak (−5.5 against −5.3 dBFS alone) with drive 0.6 for body. Factory kits only
(`pads_kit.rs`, `mixed`): `PulsusParams::default`/`studio` are untouched.

Measured, LUFS on the phrase (`preset_render.rs`): classic −19.1, studio −19.2, rock −19.9,
jazz −20.2, 808 −20.6, house −20.3, boom bap −20.2, pop −19.8, R&B −19.8, reggaeton −19.5,
electro −19.0, trap −19.0, lo-fi −18.8, 909 −18.3, percussion −18.4, cinematic −18.4 — every
kit inside the window, **no kit on the known list**. Gains: trap −0.7, electro +1.0, 808 +2.0
(the riff), jazz +7.5, percussion +8.9, the rest +4.0…+4.6. The dense riff: every kit under
full scale (the tightest, rock −1.0, jazz −1.0, classic −1.2 dBFS). Tuned by numbers; the
owner's ear is the next pass.

### 10. Stop and seek, investigated (the integrator's question)

A play from the start right after a stop begins over the tails of the last one, split or
not: the processor **releases** on Stop and on Seek (ADR 0028: `all_notes_off` on stop, seek
and loop wrap) and never resets — tails ring, by design. One defect was found and fixed
in Pulsus: its `all_notes_off` did nothing, so a pad that chokes on its note-off (a grabbed
cymbal) rang on past a stop as if its note were still held; it is now let go as its note-off
would let it go (`all_notes_off_lets_go_of_a_pad_that_chokes_on_its_note_off`). The
instrument contract's `reset` doc, which said "seek", was corrected: a seek releases.

## Consequences

- Schema 57: a kit's own gain (`gain_db`, 0 by default and not written) and the pad strips'
  kind (`StripKind::Pad`, set by the migration on v56 pad strips); `fixtures/v57.cantus`.
- Schema 56: a pad's `output` is now heard, a strip's `pan_law` exists, kit model 3 exists. The
  migration writes nothing (old kits keep their model; no older strip has a law); an older build
  refuses a v56 file. `fixtures/v56.cantus` holds a split kit with one pad fader down. (Schemas
  54 and 55 are the piano builder's; on this branch they are empty steps.)
- A pad's voices sum in voice-slot order within its lane: a different set of notes (a pad's
  notes removed) can leave the other pads' voices in other slots of the pool, so "muted" and
  "removed" agree within rounding, not to the bit (tested at 1e-6).
- Exactness needs the kit's strip to be its own: a strip that also gathers another track sums
  that track first, then the pad strips (ADR 0031's canonical order) — equal within rounding.
- Cost: lanes add one 256-frame clear and add per sounding pad per stretch; nothing when the kit
  is silent.

## What the tests hold

- `cantus-dsp` `pulsus_lanes_tests.rs`: model 3 split to buses and summed back in pad order is the
  kit to the bit; lanes are block-size independent at 0 (1…4096 frames); model 2 is its
  voice-order sum; a moved pad takes its ringing voices.
- `cantus-core` `padout_tests.rs` and the undo fuzz (`round37_padout_command`, its own stream):
  split and merge as one undo step each, refusals, taps in the snapshot, taps' rest.
- `cantus-engine` `tests/padout.rs`: split at unity = unsplit to the bit at every block size, merged
  back = the kit; offline = real time; the pool (1 and 3 workers) = serial, no borrow failed; a pad
  fader moves only its pad; mute and solo of the kit and of a pad strip; a VCA over a pad strip is
  its fader to the bit; a send from a pad strip carries that pad alone; compensation around a latent
  pad strip; freezing the kit's strip or a pad strip plays the live mix to the bit (and at 64 and
  1024 frames); the recording mode bakes a split kit to the bit; stems. `tests/rt_memory.rs`: a
  split kit, merged and split while ringing, a ride on its level, a seek, serial and on the pool —
  no allocation.
- `cantus-ui` `tests/padout_e2e.rs`: through `CantusApp::assemble` and the real engine, *Split to
  strips* from the app's action plays the unsplit kit to the bit live, a pad strip's fader moves
  only that pad, *Merge back* and undo give the kit back.
- `cantus-project`: `schema_56_keeps_a_kit_split_to_strips` (v56 split, v53 whole).
- Review 2 (each a test that fails without its fix): Virtuosity's hats choke from the real
  file (`padout_kits.rs`), one-way cuts and clamped groups (`pads_import_tests.rs`), numbered
  toms/cymbals/machines; `make_kit_from_folder` outside the roots, through `..`, onto a synth,
  with files off, and an SFZ region outside — refused or left out, no path answered
  (`padout_agent.rs`, through the MCP dispatcher with the other two tools); the bus removes
  what a kit leaves on every path (`the_bus_removes_what_a_kits_pads_leave_unused_on_every_path`,
  and the agent's `remove_items`/`apply_preset`); a chain is no cable (`a_pads_chain_is_no_cable_of_the_desks`);
  a bounce keeps a pad's chain to the bit split or not; DAWproject names lost chains; carry by
  pitch first; the chain card clicked through `assemble`; the drop path through its thread,
  with too big and not-drums refusals; delay compensation around a latent chain.
- Round 37's fixes (§12): `padout_tests.rs` — a kit routed takes its chains and pad strips
  (one undo step, back exactly), a strip a chain plays into is not free, sample pads carry by
  pitch, name, place; the undo fuzz routes kits. `tests/padout_fixes.rs` (through
  `CantusApp::assemble` and the manual engine): the kit routed 1 → 2 with strip 2 muted is
  silent, the kick included; a split with no room refused and said in English and Spanish; a
  kit's track template keeps its chains in new ids (same project and another), undone in one
  step; exploded takes leave the chains to the kit; a chained pad of a kit on the master is in
  the Direct stem (the strips' stems add up to the master within 1e-5); the kit's track stem is
  the kit as heard, its chain included; a sample kit onto a split one gives each strip by name.
