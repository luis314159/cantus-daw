# Where each note becomes a frequency — the map for round 42's tuning (2026-11-22, master 7eb64c6ec)

Read-only survey (no build). Every native pitched instrument has one place where the u8 key
first becomes an f32/f64 pitch; adding `+ off(key)` there keeps the bits at the default
(`x + 0.0 == x` for non-negative x). Five need other handling: Chorda, Exemplar, Gravis
Strings, Lamina, Organum. The song's key (ADR 0153) reaches effects only, never instruments.

## 1. How notes reach the DSP (cantus-engine)

- `NoteOn {key:u8, velocity, id, channel}` — `inst/mod.rs:64-72`; no pitch field.
  `NoteExpression::Tuning` (`mod.rs:86`, semitones: MPE, CLAP tuning, clip bends);
  `Controller::PitchBend` (`mod.rs:107`, semitones, ±2 at full throw).
- Bends: live MIDI `processor.rs:1086-87`; automation `graph/track_controls.rs:28`; MPE member
  `inst/mpe_live.rs:200,216,235` → Tuning; clip expression `inst/expr_play.rs:33,88`.
- Dispatch `graph/track.rs:163-205`; voices `cantus-dsp/src/voices.rs` (`Action::Start/Glide{key}`).
- Configure: `apply_instrument` `processor.rs:1131-1169` (from `processor_snap.rs:122`): native
  `configure` 1149, hosted `configure_hosted` 1155, Firmus `set_settings` 1158.
- `cantus_dsp::pitch::midi_to_hz` (`pitch.rs:115`) is used only by tests/benches/examples;
  instruments have their own: `va_math::midi_hz` (130, Calor), `gravis::voice::hz` (21),
  `exemplar_builtin::midi_hz` (56, f64), `firmus::pitch_hz` (117, powf).

## 2. Instruments (cantus-dsp/src unless noted)

| Instrument | Note → pitch | Bends | Rate | Insert `off(key)` |
|---|---|---|---|---|
| Firmus (test synth) | `firmus.rs:189-194`, powf | none | note-on | per voice at `:214`; `v.pitch + off` at `:190,:192` |
| Calor | `calor_voice_run.rs:54,79` (`key as f32`); tick `calor_voice_tick.rs:82-108` | bend `calor_notes.rs:101-108`; MPE `x[TUNING]`; glide | ctl block | `:54`, `:79`, `last_pitch` (`calor_notes.rs:250,276`); osc3 keyboard-off stays untuned |
| Unda | `unda_voice.rs:249,293`; `unda_voice_run.rs:102,131-133`; unison `unda_osc.rs:252-257` | PB; MPE `unda_synth.rs:192`; glide | ctl | `:249`, `:293`, `last_pitch` (`unda_synth.rs:164,168`) |
| Modulus | `modulus_voice.rs:260,360`; `op_hz` `:411-438` | bend `modulus_synth.rs:151`; MPE `:235` | ctl | `:260`, `:360`, `last_pitch` (`modulus_synth.rs:202,208`); fixed-Hz operators untuned |
| Gravis | `gravis.rs:489-491` `note_of`; `sample()` `:1010-1014` | bend `:458`; MPE `:463`; glide | sample | Synth/808 at `note_of`; Strings: see §3 |
| Lamina | `lamina.rs:383-394` (octave-folded k), powf | `set_bend` ratio `:501-507` | note-on | inside the exponent at `:394` (see §3) |
| Organum | `organum_wheels.rs:246-250` u32 increments; `wheel_hz` `:106-110`; `organum.rs:757,809` | none | configure | per wheel in `tune()` `:248`, guarded `if off != 0` |
| Exemplar | `exemplar_trigger.rs:131-138` `base_semis`; ratio `:288-291`; `exemplar_render.rs:385-390` (fast path `semis == 0.0`) | bend; legato glide `exemplar_trigger.rs:452-470` | ctl | `+ off(key)*keytrack/100` in `base_semis`, `(off(to)-off(from))*kt` in `glide_slot` (`:465`) |
| Chorda | `chorda_tables.rs:126-180` `tuning_for` → `[f64;88]`, per-key cents `:174-177` | bend ignored (engine `chorda.rs:925`) | design time | cents into `Tuning.tune[i]` in `tuning_of` (engine `chorda.rs:191`) before `settled` (`:224`) |
| Fides | `fides_voice.rs:485,445`; `:597-601`; string delay `fides_string.rs:239,294`; modal `fides_modal.rs:202` | bend `fides.rs:353,458`; MPE `fides_voice.rs:458` | ctl | `:485`, `:445` (keytrack scales it — consistent) |
| Pulsus, Tympanum | key picks a pad/piece | — | — | unpitched: leave alone |

## 3. Different handling

- **Chorda**: the ladder, inharmonicity and unison detune are designed off the audio thread
  (`chorda_engine.rs:718-737`), cached by `DesignKey` (`key_of` `:239`, `ensure` `:490`): a tuning
  change is a redesign and must be part of the design key; identical `tune[]` = same design.
  Keys outside 21..108 have no table.
- **Exemplar**: streamed zones' mip reach from the maximum shift (`exemplar_engine.rs:150-155`,
  core `inst/sampler.rs:999-1004` `max_shift_of`) must add the largest positive offset.
  keytrack 0 (drums, slices) makes the offset vanish, correctly.
- **Gravis Strings**: fret = `round(note - open)` (`gravis.rs:993`, `gravis_string_engine.rs:238,260-262`):
  an offset in `note_of` would be rounded away; put it in `extra()` (`gravis.rs:553`) and
  `sample()`'s `extra` (`:1011`) as a held per-note value.
- **Lamina**: keys octave-folded (`lamina.rs:385-391`): `off(key)` vs `off(k)` differ for a
  non-octave tuning.
- **Organum**: a wheel serves several keys through drawbar footages: only an octave-periodic
  (pitch-class) tuning is consistent.

## 4. Pitched effects

| Unit | Where | Insert |
|---|---|---|
| Tonus | `tonus.rs:368`; `tonus_scale.rs:130-152`; `tonus.rs:414-418`, `:504` | target `n + off(n)` for distance and `want` (own key/a4 params) |
| Saltus | `saltus_voice.rs:253,301-302,326,336` | `(t+off(t)) - (n+off(n))`, `k + off(k)` |
| Nitor | `nitor.rs:532-536` (fixed 440); `nitor_low.rs:238` | `midi + off(24+tonic)` |
| Loquax | `loquax_carrier.rs:181-194,265,283` | `f32::from(key) + off(key)` at `:265` |
| Arp/Chord/Scale note effects | output u8 keys | none (tuning downstream) |

## 5. Hosted CLAP/VST3

No tuning sent at note-on today. `inst_unit.rs:288-330` forwards Tuning expressions: CLAP
`clap/events.rs:233` (`CLAP_NOTE_EXPRESSION_TUNING`, semitones), VST3 `vst3/audio_thread.rs:508`
(`kTuningTypeID`, `0.5 + v/240`), MpeMidi as member-channel bend. Insert: in `note_on`
(`inst_unit.rs:241`) push a Tuning expression at offset 0 only when `off != 0`. Route `Off`
(`core external.rs:122-131`) cannot carry it: MTS-ESP or MTS SysEx.

## 6. The song's key — the precedent (ADR 0153)

`Project::key` `model.rs:1293` → `Command::SetProjectKey` `command.rs:1011` (applied `:3604` →
`notefx_cmd.rs:70-76`) → `Snapshot::key` `snapshot.rs:301` (filled `:515`) →
`EngineMessage::SetSnapshot` → `install_snapshot` `processor_snap.rs:36` → note effects `:125`,
strips `rebuild_chain` `:229` → `unit.set_song_key` `:521`, master `graph/master.rs:160`,
`replace_effect` `:1096`, stems `stems_master.rs:72`. Hook `FxUnit::set_song_key` (`fx/mod.rs:106`,
no-op default), `EffectUnit` `effects.rs:571`, Tabula `fx/tabula.rs:1085`. Instruments: no
caller (`bass::Unit::set_song_key` `inst/bass.rs:251` unused). The twin: `InstrumentUnit::set_tuning`
(no-op default) after configure in `apply_instrument`; the offline copies too (freeze,
`listen_fork.rs:898`, stems).

## 7. Bits at the default

`x + 0.0 == x` except -0.0 (all sums here are non-negative keys). No new multiplicative
`exp2(off)` without an `!= 0` guard (Organum's u32 rounding, Lamina's ratio). Exemplar's
fast path stays taken. Chorda's DesignKey reused. Hosted: send nothing at 0.
