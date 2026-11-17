# ADR 0133 — Rota, the rotary speaker

**Status:** proposed (fx side round wave 3) ·
**Date:** 2026-11-14 · **Schema:** 73 ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0020 (our plug-ins outside Cantus), ADR 0031 §5.1 (at rest), ADR 0071 (effects that
sound on insert), ADR 0101 (presets), ADR 0131 (Tabula: every unit is also a pedal)

## Context

The rotary speaker lives inside Cantus's organ (`organum_rotary`) but not as an effect (BACKLOG §3b *Still open*); the owner's pedalboard (ADR 0131) wants it, and a Uni-Vibe voicing. He asked (2026-11-13) that every new engine of wave 3 be a full unit as well as a pedal.

## Decision

A new effect kind, `EffectKind::Rotary`, `fx::rota`: the rotary speaker for any stereo signal. To be written by its builder, following the design (`tabula-design.md` §6): it wraps `organum_rotary::Rotary` — speed stop/slow/fast with ramps, horn and drum rpm, microphone spread, balance, drive; notes toggle the speed (`TAKES_NOTES = true`, the contract left it false); latency 0; at rest only when its lines are silent and the rotors' speed constant (`skip` advances the angle in closed form). **The Uni-Vibe** is a new Phasis style appended to its choices, by the same builder (Phasis keeps every existing key and default). The parameter table (every default a null), the DSP (`cantus_dsp::rota`), the editor, the presets and the starting preset or the reason for none.

**The contract** (wave 3's contract commit): `EffectKind::Rotary` declared and wired into
every list as an exact pass-through stub — one placeholder `out` parameter (0 dB, a null),
an engine unit that applies it, is always at rest and has no latency, a plain kit editor,
a placeholder face (a tone-family sibling's look under its own key), its row of Tabula's
faces (`views/fx/tabula_faces.rs`), its locale pair, an empty preset file, its exported
CLAP/VST3 descriptor and ids, and schema 73's fixture.

## Phase A-1: the DSP (2026-11-14, `cantus_dsp::rota`, Phasis's Uni-Vibe)

**Rota wraps Organum's cabinet** (`organum::rotary::Rotary`: horn and drum lines read at
the Doppler delay, directivity, the wall's reflection, the drum's baffle, two
microphones). Its `tick` was split, bit-identically, into `advance` + `tick_moved` (the
organ's tests unchanged) so the insert turns the rotors itself:

- **Fixed-point angles** (turns × 2⁶⁴): at a constant speed the angle advances by one
  integer a sample, so `skip(n)` adds `n` times it — the same bits as playing the silence
  (`rest_is_exact_and_the_angle_skips_in_closed_form`, slow, fast and parked).
- **Speed** glides to its target by a first-order lag with its own time constants up and
  down, and lands exactly (a millionth of a turn a second from the target is the target);
  the brake slows to a creep and parks both rotors at 90°.
- **Defaults** are Herrera, Hanson & Abel's measurements (AES 127th Convention, 2009, as
  setBfree documents them): horn 40.32 / 423.36 rpm, drum 36 / 357.3 rpm, spin-up 0.161 /
  4.127 s, spin-down 0.321 / 1.371 s — inside the published 122 and 147 measurements
  (HammondWiki: a 147 at 48 / 400 and 40 / 342; another at 48–49 / 409 and 48–49 / 396).
- **Drive**: the cabinet's valve amp as a 12AX7 stage's static curve (Koren's triode solved
  off the audio thread into a 1 025-point table, cubic Hermite between), 2× oversampled
  through the minimum-phase half-bands.
- **Notes** toggle slow and fast (`Rota::toggle`; `TAKES_NOTES = true` in the engine phase).
- **The null**: the mix starts at 0, the dry untouched to the bit. **Latency 0**: the delay
  the wet has (the microphones' distance, a sample of the minimum-phase oversampler) is the
  sound's own, not a delay to compensate.
- **Rest**: silence in for 8 192 samples (longer than the lines) and the output under
  −110 dB for 2 048: the lines and filters cleared; at rest only the rotors move, and a
  rotor changing speed is never at rest.

**The Uni-Vibe** is Phasis's new style, appended (`PhasisStyle::{Classic, Vibe}`, the
classic the default — every existing test and render unchanged; the engine's literal takes
`..PhasisSettings::default()`), with an `intensity` beside it. After Darabundit, Wedelich &
Bischoff (DAFx-19): four first-order all-pass stages at `1/(2π·R·C)` with the paper's
capacitors (15 nF, 220 nF, 470 pF, 4.7 nF) and its measured cell ranges (12.7k / 6.86k /
7.69k / 6.22k bright, 2.79M / 2.59M / 3.32M / 4.16M dark), so the notches are spread and
move unevenly; an incandescent lamp with a thermal lag (12 ms, typical), its light growing
as the filament's power to the 1.5, and CdS cells that follow brightening in 4 ms and
darkening in 60 ms (typical: the paper measures the curves' asymmetry, not the constants);
`ln R` follows the light. The lamps are a state the LFO alone drives: a skip steps them
sample by sample, exact.

### Measured (phase A-1)

- Horn spin-up τ 0.161 s (to 423.4 rpm), drum 4.127 s; down 0.321 s and 1.371 s, each to
  a sample; the horn's turns over 10 s equal its rpm to 10⁻⁶.
- Drive: 0 bypasses the curve; harmonics −20.9 dB at 40 %, −10.7 dB at 90 % (a 200 Hz tone
  at 0.5).
- Uni-Vibe at full intensity, 2 Hz: every cell inside its measured extremes, log-means
  269k / 198k / 239k / 248k against the paper's 405k / 233k / 290k / 240k; the stages always
  at least 365× apart in frequency; over a 1 Hz cycle the light spends 10 234 samples rising
  through 10–90 % and 12 272 falling.

## Phase B (2026-11-15/16): the engine, the window, presets, export

- **The engine unit** (`cantus-engine/src/fx/rota.rs`): the table read into `RotaSettings`
  (the ramps the published times × `ramp`); notes throw the switch to the sample when
  `notes` is on (`TAKES_NOTES`); no latency; at rest only parked (a turning rotor's angle is
  its history); telemetry: each rotor's angle and speed, the switch as notes left it.
- **The window** (design sheets v1 and v2, approved by the owner 2026-11-16): the cabinet cut
  away, from above with the microphones dragged, the ramps with each rotor's dot; walnut with
  a pale steel horn and a vermilion drum, Cherry and Black colourways; its pedal on Tabula.
- **Presets**: thirteen, levels fitted (`preset_tool fit`: −5.3 … +6.7 dB); *Chorale* the
  starting preset at +3.6 dB. **Phasis's lamp vibe**: its circuit card, three presets.
- **Export**: the direct DSP in the plug-ins' tests, notes through CLAP and VST3; `rpm`, `cm`
  and `×` read back from their text (`readout`).

## Consequences

- Any track can have the organ's cabinet; the organ keeps its own (one DSP, two uses).
- A note stream can drive an effect's switch; Rota is the second effect after Saltus to
  hear notes in the exported plug-ins.
- The window's rotors are the engine's: what is drawn turning is what is heard turning.

## What the tests hold

- DSP (`rota_tests.rs`): the ramps' time constants and landing, the brake, the closed-form
  skip, drive's harmonics, the null, rest.
- Engine (`fx_rota.rs`, `fx_contract`, `rt_memory`): the null to the bit, no latency, block
  size and split independence, a board of one, no allocation; notes toggle the switch.
- UI: `rota_e2e.rs` (through `CantusApp::assemble`: Chorale heard, fast undone to the bit,
  the track's notes throwing the switch, the null); `arca_rota_drag.rs` (the switch clicked,
  the microphones dragged, one undo step); `arca_rota_whole.rs` (no word cut or shortened,
  EN/ES, 75–200 %).
- Plug-ins: the CLAP/VST3 suites with Rota's direct DSP; `notes.rs` (its notes at their
  sample in both formats); the ids blessed.
