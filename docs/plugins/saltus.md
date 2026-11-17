# Saltus — pitch

*Effect kind `PitchShift`. References: FL Studio's Pitch Shifter and Pitcher, DigiTech's
Whammy, Boss's OC-2 octaver, Eventide's harmonisers, Ableton Live's Shifter (ADR 0132).*

*Saltus* is Latin for the leap. One unit moves the pitch of what it hears four ways:

| Mode | What it does | Engine | Latency at 48 kHz |
|---|---|---|---|
| **Shift** | two voices at fixed intervals (±24 semitones, ±100 cents) | mono (PSOLA, formants kept) or poly (phase vocoder) | 2.7 ms · 85 ms |
| **Harmony** | two voices a number of scale degrees from the note heard — in a key and scale, in the chord held on MIDI, or singing the MIDI melody | mono | 2.7 ms |
| **Octave** | one and two octaves down, one up — from an analog-style divider, or clean | divider · mono | none · 2.7 ms |
| **Whammy** | one voice swept by a pedal from the heel's interval to the toe's, glided — or pressed to the toe by a held MIDI note | mono or poly | 2.7 ms · 85 ms |

The latency is fixed by the settings (mode, engine, octave style), never by the audio, and
reported to delay compensation. Every voice has its level and balance; the dry is delayed to
line up; formants are kept or move with the pitch, and the *throat* moves them on their own.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/saltus.rs` (29 keys; the defaults are the input delayed, a null) |
| DSP | `crates/cantus-dsp/src/saltus.rs` (the unit), `saltus_voice.rs` (the mono voices, harmony), `saltus_poly.rs` (the phase vocoder), `saltus_octave.rs` (the divider); tests `saltus_tests*.rs` |
| Engine glue | `crates/cantus-engine/src/fx/saltus.rs` (settings, notes, telemetry) |
| Editor | `crates/cantus-ui/src/views/fx/saltus.rs` (the cards), `saltus_display.rs` (the leap: the keyboard and its arcs, the legends, the engine switch), `saltus_staff.rs` (the grand staff), `saltus_pedal.rs` (the whammy pedal), `saltus_words.rs` (intervals and notes in words), `saltus_art.rs` (its motif on Tabula) |
| Face | `crates/cantus-ui/src/kit/skin/side.rs` (`SALTUS`: graphite anodised, chartreuse and lilac on a VFD) |
| Strings | `crates/cantus-ui/locales/fx/saltus.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/pitchshift.toml` (eighteen, in both engines); starts on *Octave up, twelve-string* (poly) — or the start the user chooses (ADR 0135) |
| Tests | `crates/cantus-engine/tests/fx_saltus.rs`, `rt_memory::saltus_never_allocates`, the family's contract (`fx_contract`: split, rest, a board of one); `crates/cantus-core/tests/saltus_commands.rs`; `crates/cantus-ui/tests/saltus_e2e.rs`; the CLAP/VST3 tests and `notes.rs` |
| Exported | `Cantus Saltus` (CLAP and VST3), hearing notes on its note port |
| Examples | `shoot_saltus` (the design sheet), `cantus-dsp/examples/saltus_sounds` (the listening files) |

## The window

Across a dark VFD screen, **the leap**: a keyboard, the note heard lit on it and named large
(its cents and frequency under it), and from that key **an arc to each voice** — as thick as
the voice is loud, labelled with the interval's name and the note it lands on; drag a
landing to another key to change its interval. Above it **a grand staff** writes the last
seconds as they are sung — the pitch heard as a line through the staff, each voice beside it
— and now, at its right edge, the notes themselves; each falls by a fine line to its key.
Harmony marks the scale's notes or lights the chord held; Octave draws its three octaves;
Whammy draws **the pedal** in depth beside the keyboard, rocking from heel to toe (drag it).
The modes are lit legends along the top; the **engine switch** at the right shows each
engine's latency (Mono 2.7 ms · Poly 85 ms; Analog no latency · Clean 2.7 ms). Under the
display, the mode's cards: the voices, the harmony, the octaves, the pedal, the sound
(engine, formants, throat) and the mix.

## On Tabula

Saltus is a pedal like every native unit: its face carries the leap (arcs over a small
keyboard) and the knobs Interval, Level, Dry and Pedal; a board of one Saltus renders the
insert to the bit and reports its latency.

## Ours, better

- The latency is the settings', never the audio's: delay compensation holds; the poly
  engine's 85 ms are reported, and the mono engine (2.7 ms) or the analog octave (none) are
  one click away on the display, each with its latency beside it.
- At its defaults it is the input delayed, to the bit; every engine's dry comes out exactly
  where its latency says (the engine's tests, on the whole mix).
- Harmony keeps the singer's intonation (bends and vibrato carried, the interval chosen by
  the scale), on a key or on the chords another track holds.
- The whammy is a pedal or a keyboard: a held note presses the toe, at its sample.
- Exact rest (the family's twins), no allocation on the audio thread, block-size
  independent, offline equal to live.
