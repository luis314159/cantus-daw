# Loquax — vocoder

*Effect kind `Vocoder`. References: FL Studio's Vocodex (the bar), Arturia's Vocoder V,
Waves' OVox and Morphoder, the Roland VP-330 and SVC-350, the EMS Vocoder 2000 — read
about, nothing copied (ADR 0139, provisional number).*

*Loquax* is Latin for the talkative one. A channel vocoder: the voice (the strip's own
sound) is split into 8 to 64 bands of Butterworth band-passes (order 4, 6 or 8, within
0.003 dB of the analytic response), each band's envelope followed and laid on the same band
of the **carrier** — Loquax's polyphonic synthesiser played by notes, another strip's sound
on the **sidechain**, or both. The consonants pass above their own corner; noise joins the
carrier on them; the formants move without moving the notes; Hold freezes the vowel.

| Carrier (`src`) | What carries the voice | Where it comes from |
|---|---|---|
| **Sidechain** (0) | another strip's sound | the slot's key (`EffectSlot::sidechain`), as for a compressor |
| **Synthesiser** (1) | saw, pulse, noise; 1–7 detuned copies; 16 voices | the notes of the track the slot listens to (`EffectSlot::midi_from`) |
| **Both** (2) | the two together | |
| **Robot** (`robot`) | the synthesiser droning `rnote` while the voice is above `rgate` | nothing — no notes, no key |

`swap` trades the roles: the key is the voice and the strip is the carrier (drums that
talk). **The null**: the mix starts at 0 — the signal untouched, to the bit. No latency.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/loquax.rs` (101 keys: the head, `b0` … `b63`, `obj`) |
| DSP | `crates/cantus-dsp/src/loquax*.rs` (the unit, the banks, the carrier, the voicing, the measures); tests `loquax_tests.rs`, `loquax_tests2.rs` |
| Engine glue | `crates/cantus-engine/src/fx/loquax.rs` (settings, the notes in time order, telemetry: the DSP's numbers and the bands' levels in 32 groups) |
| Editor | `crates/cantus-ui/src/views/fx/loquax.rs` (the cards, the ⋯ menu), `loquax_display.rs` (the deck), `loquax_art.rs` (the gooseneck, the bezels, the meters), `loquax_objects.rs` (the talk-box, the desk microphone) |
| Face | `crates/cantus-ui/src/kit/skin/side_fxa.rs` (`LOQUAX`: brushed aluminium; Ink and Road case colourways), the objects' palette `kit/skin/consoles.rs` |
| Strings | `crates/cantus-ui/locales/fx/loquax.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/vocoder.toml` (fifteen); starts on *Robot* |
| Tests | `crates/cantus-engine/tests/fx_loquax.rs`, `rt_memory`, `fx_contract`; `crates/cantus-ui/tests/loquax_e2e.rs`, `loquax_drag.rs`, `fxa_whole.rs`; the CLAP/VST3 suites (`clap.rs`, `vst3.rs`: the sidechain input; `notes.rs`: the notes) |
| Exported | `Cantus Loquax` (CLAP and VST3), hearing notes on its note port and its carrier on the sidechain input |

## The window

- **The object the voice speaks into** (`obj`, the ⋯ menu's *Microphone*): the gooseneck
  rising from its chrome socket (the default), the talk-box's clear tube rising from its
  driver to a clip on its stand, or the desk microphone on its weighted base. Each has its
  on-air lamp and its glow following the voice, and wears the colourway's finish (brushed
  aluminium, black lacquer with chrome lips, the road case's steel corners). **A look, kept
  per insert**: saved in the song and in the plug-in's state, one step of undo, never heard;
  each factory preset suggests one and loading it sets it (*Talk-box* the tube, *Radio* the
  desk microphone, *Robot* the gooseneck); a preset of yours keeps the one it was saved
  with; A/B leaves it, and it never marks a preset modified (`EffectKind::is_look`).
- **The bank**: a phosphor screen in a machined bezel, a column of lit cells per band on a
  log axis, the peak held; **the band levels drawn by dragging across it** (−24 … +12 dB in
  half decibels, snapped to 0 within a decibel; one stroke one step of undo); the formant
  shift as the synthesis bands' ticks moved against the analysis bands'.
- **The meter bridge**: the voice's needle and the carrier's (dBFS), the carrier in words,
  lamps for a consonant, the hold and the robot, the bank in words.
- **The cards** in the fewest even rows their words allow; where a language needs another
  row (Spanish: four), the window grows and the display keeps its size.

## Exported

The plug-in hears notes (a CLAP note port in either dialect, a VST3 event bus) and its
sidechain input carries once the plug-in's own *Sidechain* switch is on *Sidechain input*
(`cantus.sidechain`, as on every exported unit). Its ids are blessed in
`crates/cantus-plugins/tests/golden/ids.txt`.

## Not a pedal (yet)

On Tabula a vocoder needs its carrier: a board has no key of its own and a pedal hears no
notes, so a Loquax pedal would only ever drone the robot. Loquax stays off the board
(`pedal_allowed` false) until Tabula routes a key to a pedal (ADR 0139, phase B).

## Ours, better

- Slopes of order 4, 6 or 8 measured against the analytic Butterworth response; NCM 0.74 at
  16 bands on the speech-like phrase.
- Both carriers, a robot that needs nothing, the roles traded — and the band levels drawn.
- Three objects to speak into, each drawn by hand in every colourway.
