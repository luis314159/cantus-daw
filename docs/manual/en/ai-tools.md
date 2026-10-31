# What an AI agent can do in Cantus

*[Back to the manual](README.md) · [Leer en español](../es/ai-tools.md)*

When you let an assistant such as Claude connect to Cantus (*Settings ▸ AI*), it works on
your project through **tools** — the same edits you make with the mouse, each one **a single
step of Undo** named "AI: …" in the history. Reading never asks; changing asks in the Cantus
window unless you allowed edits; removing or replacing your material always asks; files are
refused until you allow them, and then asked. The names of your tracks, clips and markers are
shown to the agent as data — never as instructions.

## The tools

| Group | Tools | What they do |
|---|---|---|
| Read | `get_project`, `get_track`, `get_clip_notes`, `get_mixer`, `get_device`, `get_history`, `get_selection` | The song, a track and its clips, a clip's notes or automation points, the mixer, a device's values, the undo history, what you have selected and where the playhead is |
| The song | `set_tempo`, `set_tempo_point`, `set_song`, `set_marker` | Tempo and tempo changes, the time signature (and metre changes), the key, the loop, the metronome, markers |
| Transport | `transport` | Play, stop, locate — not an undo step |
| Tracks | `add_track`, `set_track`, `set_instrument`, `set_instrument_param` | A new instrument track with its own mixer strip; name, colour, level, pan, mute, solo, order, routing; swapping the instrument; its parameters by key in their units |
| Clips and notes | `add_clip`, `add_notes`, `replace_notes`, `edit_notes`, `set_clip`, `duplicate_clip` | Clips and notes; transpose, quantise, humanise, move, velocity, length, legato; moving, resizing, looping and copying clips |
| Drums | `write_drum_pattern` | Step rows (`x...x...`) per pad, with swing and accents |
| Session | `set_session` | Scenes, slots, launch modes and follow actions |
| Mixer and effects | `add_strip`, `set_strip`, `set_send`, `add_effect`, `set_effect`, `replace_effect` | Buses and returns, faders, width, the strip EQ, sends; any of Cantus's effects on any strip or the master, with their parameters |
| Movement | `automate`, `replace_automation`, `add_modulator`, `set_modulator` | Automation clips with FL's shapes, in the parameter's own units; LFOs, envelopes, followers (sidechain), step sequences, random |
| Music | `lint_music`, `write_part`, `voice_chords`, `pedal_from_chart` | The music linter; a whole part over a chord chart; voiced chords; Chorda's sustain pedal from the chart |
| Units | `list_units`, `get_unit`, `list_presets`, `apply_preset` | The catalogue of every instrument and effect; presets (loading one replaces a device's settings: always asked) |
| Takes | `comp_takes`, `tidy_takes` | Comping recorded takes; deleting a comp, flattening, deleting unused takes |
| Export | `export_audio`, `export_project` | WAV, FLAC, MP3, Ogg, stems; MIDI and DAWproject — only into the project's folder or a folder you allowed |
| Removing | `remove_items` | Tracks, clips, notes, effects, strips, sends, modulators, markers, tempo points, scenes, session clips — always asked |

An agent never takes your material silently: drawing automation over points you drew, a
part where your notes are, a preset over your settings all ask first (or are refused).

Positions are given in **beats** (quarter notes from the start) or as **bar:beat** (`9:1`,
`9:2.5`), and Cantus answers with both. Values are in each parameter's own unit (dB, Hz, ms,
semitones…); a value outside its range is refused with the range, never quietly changed.

The agent also has the server's own tools (ADR 0081): it can wait for or cancel a long job,
group a turn of its work into one undo step (`begin_change`/`end_change`), undo its own last
step and list what it did.

## The catalogue of units

`list_units` and `get_unit` (and the resource `cantus://units`) describe every instrument and
effect Cantus has: each parameter's key, its name, its group (*Band 4*), unit, range, default
and taper, the names of a switch's choices, the factory presets and what they set, the
starting preset an effect loads on insert, its latency, and what the unit is for. It is made
from the same tables the knobs use, so it is always complete and always right.

## The music linter

`lint_music` reads the notes the way a reviewer listens, and says where and why:

- **a note ringing into the next chord** that does not hold it — an *error* when it is a
  semitone against the new chord (the classic: a chord's A♭ still sounding over E♭/G's G);
- **a clash**: a note held under a chord a semitone *above* one of its tones (the 11th over a
  major chord's third, the ♭6 over the fifth) — a 9th, a 13th or a major 7th is a colour;
- **a doubled note**: the same pitch written twice at once on a track;
- **outside the key** and the chord (*info*: often a passing note, sometimes a mistake);
- **outside the instrument**: below a piano's A0, a drum note with no pad.

Give it the chords (the chart) for the harmony checks; without them it checks doubles, the key
and the range. It changes nothing.

## Charts: chords in bars

The writers and the linter read chords the way a lead sheet writes them, one bar between bars
`|`, the chords of a bar sharing it equally:

```text
Fm9 | Dbmaj7 | Abadd9 | Eb Eb/G
C . . G7 | %                       C for three beats, G7 for one; then the bar again
```

`.` holds the chord before for one more share, `%` repeats the bar, `N.C.` is no chord. Any
symbol works: `C`, `Cm`, `C7`, `Cmaj7` (`CM7`, `CΔ`), `Cm7b5` (`Cø`), `Cdim7`, `Csus4`,
`C6/9`, `Cadd9`, `C9`, `C13`, `C7b9`, `C7#11`, `C7alt`, slash basses (`Eb/G`).

`write_part` lays a part over a chart — block chords, pulses, arpeggios or a bass line, on a
rate or a step rhythm (`x..x..x.`), with swing and a human touch — voiced to move as little as
possible from chord to chord, and **no note rings into the next chord**. `voice_chords` writes
the held chords; `pedal_from_chart` draws Chorda's sustain pedal: up on each change, down
again a moment later, so every chord rings and never the last one's.

## Prompts

A client lists ready-made requests: *Explain my mix* (changes nothing), *Write a part*,
*Arrange an intro*, *Tidy the project* (asks before removing anything) and *Teach me this
unit*, in the language of your interface.
