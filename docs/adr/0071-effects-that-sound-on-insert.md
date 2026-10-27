# ADR 0071 — Effects that sound on insert: a starting preset per unit

**Status:** Accepted · **Date:** 2026-10-26 · **Round:** 31 (editing) · **Extends:** ADR 0018
(the plug-in contract and the null rule), ADR 0055 (the modulation family), ADR 0047's
exported plug-ins

## Context

Round 28's modulation family enters at *Mix* or *Depth* 0 because ADR 0018 asks every
unit to be a null at its defaults (a control at its default touches no sample). The owner
found that it reads as "it does nothing" (2026-10-16): in FL Studio and Ableton a chorus
dropped on a pad is a chorus at once. Of the options of BACKLOG §3b he chose **B**: an
insert loads a musical **starting preset**, while each parameter's default stays the null —
so the rule, and every test that holds a unit to it, are unchanged. (Option C, defaults that
sound, was rejected: it breaks the family's rule.)

## Decisions

### 1. The decision per unit lives in the core, one table

`cantus_core::starting::start_of(kind)` decides for **every** `EffectKind`: a
`Start::Preset` — one of the unit's own factory presets, by its label key and values — or a
`Start::Neutral` with its reason. A test fails when a kind is neither, when a preset names a
key its kind does not have or a value out of range, or changes nothing; another (in the
application) when a starting preset is not, value for value, the factory preset of that key
the editor lists.

| Unit | Starts on | Why |
|---|---|---|
| Aequalis (EQ) | neutral — flat | an EQ's curve is what you draw (FL's Parametric EQ 2, Pro-Q) |
| Latitudo (stereo) | neutral — flat | a utility: width changes mono compatibility silently |
| Comprimo | neutral — sounds | −18 dB at 4:1 already compresses |
| Spatium, Reverb, Delay | neutral — sounds | 25–30 % wet already |
| Sibilus (de-esser) | neutral — sounds | −30 dB, 6 dB range already works |
| Porta (gate) | neutral — sounds | −40 dB already gates |
| Tonus | neutral — sounds | corrects to the chromatic scale at its defaults |
| Terminus, Culmen, Tonsor | neutral — must not surprise | safeties at the end of a chain: they catch only what goes over |
| Speculum | neutral — measures | it passes its signal untouched |
| hosted plug-ins | neutral — hosted | their maker's own initial state |
| Multiplex | *Glue* | four gentle bands (1.5–1.8:1) |
| Fervor | *Tape bus* | 7 dB of tape drive, 2× oversampled |
| Concentus | *Juno I* | the Juno-60's chorus I |
| Flexus | *Jet* | the classic through-feedback jet |
| Phasis | *Phase 90 (script)* | the four-stage pedal |
| Tremor | *Surf* | the amp tremolo |
| Vibrans | *Singer* | 30 cents, arriving after 350 ms |
| Anulus | *Bell* | a 587 Hz carrier, 60 % wet |
| Translatio | *Slow phase* | 0.7 Hz, half wet: the Bode's slow swirl |
| Fluctus | *Pump ¼* | the sidechain pump without a key |
| Impetus | *Punch* | more attack |
| Discrimen | *More attack* | +5 dB on the transients |

The strip's channel EQ is no insert: it starts flat, as it always did.

### 2. One place inserts: `CantusApp::new_insert`

Every way the interface inserts an effect takes its slot from `new_insert(kind)` — a
slot's menu and the mixer's (`add_effect`), the device picker, the command palette, a
swap of one unit for another (`replace_effect_with`), a reverb or delay return
(`full_wet`, which then sets the mix to 100 %) — so an insert sounds wherever it comes
from. It stays one `InsertEffect`, one undo step. Undoing it removes the effect; the
preset is the effect's values, not a step of its own.

What is *not* an insert from the interface keeps its own values: a project or template
opened, a paste or a duplicate of a strip (the values copied), the AI bridge's
`insert_effect` (the request says what it wants), the bundled templates and demos (their
presets chosen by hand).

### 3. A setting turns it off

*Settings ▸ Editing ▸ Effects ▸ On insert*: **Starting preset** (default) or **Neutral
settings** (`Settings::insert_neutral`) — then an insert is the unit at its defaults, as
before this round.

### 4. The editor names the preset it opened on

An editor opened on values that are, key for key, one of its kind's factory presets shows
that preset as loaded in its header (`frame::Chrome::recognised`, looked up once per
editor), so a fresh Concentus says *Juno I* — and *Juno I•* (modified) once a knob moves,
as for any preset loaded from the list.

### 5. Our exported CLAP and VST3 plug-ins start there too

A fresh instance of an exported unit, inserted by another DAW, starts on the same
starting preset (`cantus_plugins::instance::Shared::new`); each parameter's **default** —
what a host's *reset* and *default* give — is still the unit's null (a test holds every
parameter's default to `EffectParamInfo::default`). A saved state restores what it holds,
and a key missing from it reads as its default, as before. So Cantus and the plug-ins
agree: an insert in either sounds at once; a reset in either is the null.

## Consequences

- ADR 0018's null rule and its tests are unchanged: they test `EffectSpec::new`, which is
  still the defaults; `EffectSpec::new` is not what an insert starts on any more, and code
  that inserts from the interface goes through `new_insert`.
- New: `cantus_core::starting`, `CantusApp::{new_insert, set_insert_neutral}`,
  `Settings::insert_neutral`, the `clipfx.settings.*` strings, the plug-ins' starting state.
- Tests: every unit decided (the core), every starting preset the editor's factory preset
  (the application), an insert from the slot menu, the picker and a swap sounding (its
  preset's values) and the setting making it neutral, the header naming *Juno I*, the
  plug-ins starting on the preset bit-identical to the DSP there with their defaults the
  null.
- Left to the owner's ears: whether each starting preset is the one to start on (they are
  the classic settings of each unit's reference, chosen to be heard and to suit most
  material; changing one is one line of the table).
