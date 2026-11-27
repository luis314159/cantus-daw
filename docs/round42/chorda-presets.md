# Round 42 — `chorda-presets`: the voicing by hand and the character presets

**Status: in progress.** Branch `r42/chorda-c17` (the same worktree) from 7d85795ef. ADR 0098's
eighth amendment. Schema 88 (85–87 held, empty, for the integration's: 86 Tympanum's, 87
Arcus's). No `DSP_VERSION` change: every control at its default is the shipped voicing to the bit.

The owner (2026-11-26): character presets — *a very dark one, a very imposing one, and so on*,
each still a real instrument — and *"el usuario debe poder mover cada uno de los parámetros a
mano"*.

## 1. The voicing by hand (Part A)

`cantus_dsp::chorda::user::UserVoicing` over the shipped `Voicing`; parameters `voicing.*`
(`cantus_core::inst::chorda`), grouped by `VOICING_GROUPS`; the **Voicing** view in Chorda's
editor (Play · Mixer · Edit · Voicing · Matrix).

| Group | New controls | Range (default) | Chorda's own, shown with them |
|---|---|---|---|
| Hammers | Bass hardness, Treble hardness (`felt_bass`, `felt_treble`) | −100 … 100 % (0) | Hardness |
| | Hammer mass (`mass`) | 50 … 200 % (100) | |
| Strings | Bass / Middle / Treble sustain (`decay_*`) | 25 … 400 % (100) | Decay, Unison |
| | Overtone sustain (`hf`), Overtone spread (`hf_spread`) | 25 … 400 %, 0 … 200 % (100) | |
| | Unison unevenness (`uneven`), Second polarisation (`polar`) | 0 … 300 %, 0 … 400 % (100) | |
| Soundboard and case | Bass formant (`formant_hz`), gain (`formant_db`), Q (`formant_q`) | 150 … 1 500 Hz (600), −12 … 24 dB (12), 0.5 … 8 (3) | Soundboard, Bridge coupling, Sympathy |
| Action | Key knock (`knock`), Knock tone (`knock_tone`) | 0 … 400 % (100), −2 … 2 oct (0) | Action noise |
| | Key return (`key_return`) | 0 … 300 % (100) | |
| | Damper strength (`damper`), Bass dampers (`damper_bass`) | 25 … 400 % (100) | |
| | Damped floor (`damped`) | −120 … −30 dB (−70) | |
| Microphones | Arrival spread (`itd`) | 0 … 300 % (100) | Mic position, Width, Lid, Room, Room size |
| Tone | Top roll-off (`top_hz`) | 2 … 20 kHz (11.06 kHz) | Tilt, Colour |
| | Voicing amount (`amount`: each key's partial table) | 0 … 200 % (100) | |

Not exposed: the keyboard's design (made off the audio thread; the tuning passes rest on it),
the body's responses (they allocate), the per-key tables (the base; *Voicing amount* scales the
partial table).

## 2. The character presets (Part B)

(Filled in below as the fits are measured.)

## Open

- Nothing verified by ear yet.
