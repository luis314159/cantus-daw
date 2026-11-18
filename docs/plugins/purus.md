# Purus — real-time repair

*Effect kind `Repair`. References for function: iZotope RX's De-hum and Dialogue De-reverb,
Waves X-Hum and NS1, Accusonus ERA De-Reverb, Acon DeVerberate — read about and listened to,
nothing copied (ADR 0142).*

*Purus* is Latin for clean. It repairs a track **as it plays**, where the audio editor's
denoise and declick (ADR 0096) repair a take offline: three modules, each switched on when it
is needed, each with its mix, its output and a key to **hear only what it takes out**.

- **Hum removal**: the mains at 50 or 60 Hz and up to sixteen harmonics, notched (a −3 dB
  width you set, a depth to 80 dB), at the frequency you set, **followed** within 2 Hz as
  the mains drift, or **found by itself** (50 or 60, then followed). **Gentle** learns the
  hum and subtracts it instead, holding what it learned while a bass note sits on the hum's
  own frequencies (50 ms ahead: its latency).
- **Click and crackle removal**: each click rebuilt from the music around it (the audio
  editor's own detector and rebuilding, shared to the bit); crackle subtracted in the high
  band, where an impulse stands far above the music. The sensitivity stops at 0.7: past it
  the repair harms the music more than the clicks do.
- **Room removal**: a room's late tail predicted from the signal (Habets's model, with the
  Direct sound it assumes, 0 dB by default) and taken down to the reduction set — **only as
  far as the signal's own free decays show a tail is there**: a dry voice passes untouched
  (every octave within 0.01 dB of a real dry voice, the residual at the transform's round
  trip), a room's tail comes down as the model predicts; the decay set by hand or **estimated** from the music's
  own pauses; the early reflections kept for as long as you say.

**The null**: every module starts off — the signal untouched, to the bit, undelayed.
**Latency** is only the modules that are on: the gentle de-hum's 50 ms, the de-click's 767
samples (16 ms), the room's transform (21.3 ms at 48 kHz, 23.2 ms at 44.1 kHz); 87 ms with
all three. Cantus compensates it on the strip; a plug-in reports it to its host and again
whenever a module is switched.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/purus.rs` (27 keys) |
| DSP | `crates/cantus-dsp/src/purus*.rs` (the unit, the de-hum, the de-click, the de-crackle, the de-reverb, the test kit); tests `purus_tests*.rs`, `tests/purus_alloc.rs`; the report `docs/round-fx/purus.md` |
| Engine glue | `crates/cantus-engine/src/fx/purus.rs` (settings, the latency read live, the telemetry with the 48 reduction bands in three pages, rest) |
| Editor | `crates/cantus-ui/src/views/fx/purus.rs` (the bench, its keys and drags, the cards), `purus_screen.rs` (the spectrogram, the comb, the clicks' lane, the room's tail), `purus_picture.rs` (the picture and the live analysis), `purus_parts.rs`, `purus_art.rs`, `purus_demo.rs` (the design sheet's take) |
| Face | `crates/cantus-ui/src/kit/skin/side_fxc.rs` (`PURUS`; champagne, surgical, graphite), `kit/skin/restoration.rs` |
| Strings | `crates/cantus-ui/locales/fx/purus.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/repair.toml` (sixteen); starts on *Mains hum 50 Hz* |
| Tests | `crates/cantus-engine/tests/fx_purus.rs`, `rt_memory`, `fx_contract`; `crates/cantus-ui/tests/purus_e2e.rs`, `purus_drag.rs`, `fxc_whole.rs`; the CLAP/VST3 suites (`clap.rs`, `vst3.rs`: the latency re-reported as modules switch) |
| Exported | `Cantus Purus` (CLAP and VST3) |

## The window

A restoration bench: a machined instrument on a restorer's cutting mat, its screen dark glass
in every theme.

- **The spectrogram**: the last six seconds of the slot, 20 Hz to 20 kHz. What is heard glows
  in a warm heat; **what was taken out** of each cell is laid over it in the residue's cool
  colour. Live, the columns come from the engine's analyser tap — the slot's input, read as
  late as the unit's latency, against its output — cut every 40 ms on the editor's thread
  (nothing more on the audio thread than the tap every editor has).
- **The comb**: the hum's harmonics as needles on the frequency rail (each as long as its
  depth now, a dot as big as its level; held ones bracketed in the gentle mode), hairlines
  across the picture. **Drag a needle up or down: the fundamental follows it** (to a tenth of
  a hertz; one drag, one step of undo; the card's pill keeps the fine value).
- **The clicks' lane**: a tick for each repair and the count.
- **The room's tail, before and after**: the room's decay as it was (a bright cream line), and
  after — the early reflections kept, then the tail taken down — the gap in the residue's
  colour. **Drag on it to set the reduction** (the handle on its right edge follows; 0 at the
  top, 30 dB half way down) and **drag the mark where the early reflections end** to keep
  more or less of them. Each drag one step of undo; while the room module is off it takes
  none.
- **Three cartridges**: each a bat toggle, a jewel lamp lit by what the module does, its
  reading, an emblem (a tuning fork engraved with the fundamental it follows, a jeweller's
  loupe over a click, wave fronts reaching a wall) and its keys — 50 Hz, 60 Hz and Gentle on
  the hum's, Set by hand and Estimated on the room's, *Hear what is removed* on each. A click
  is one step of undo; the menu offers automation and MIDI learn.
- **The cards**: Hum removal, Click and crackle removal, Room removal, Output.

## Presets

Sixteen, fitted to the dry's loudness (`preset_tool fit`): Mains hum 50 Hz (the starting
preset), Mains hum 60 Hz, Mains hum found by itself, Guitar amp hum and buzz, Live guitar DI
hum (gentle), Bass DI hum (gentle), Vinyl clicks, Vinyl clicks and crackle, Old cassette,
Podcast in a small room, Dialogue in a reverberant room, Far-microphone interview (Direct
sound 0 dB), Lecture hall (Direct sound 0 dB, 80 ms kept), Field recording, Video call and
Strong cleanup.

## Exported

`Cantus Purus` in CLAP and VST3, bit-identical to the DSP. **Its latency follows its
modules**: a module switched by the host's automation (or the plug-in's own window) changes
the latency; CLAP asks the host for a restart and reports the new latency on reactivation
(`latency.changed`), VST3 tells the host on its UI thread (`restartComponent` with
`kLatencyChanged`) and `getLatencySamples` reads the new one. *Hear what is removed* is each
module's delta, as in Cantus. Its ids are blessed in
`crates/cantus-plugins/tests/golden/ids.txt`.

## A pedal on Tabula

Purus may be a pedal — a de-hum pedal on a guitar's board, the ground loop's hum taken out
before a drive multiplies it: its face shows the fundamental, the harmonics, the depth and
the mix; the rest is in its editor, opened from the pedal. Its latency is the board's lane
latency as any pedal's, and follows a module switched on the pedal
(`fx_purus::as_a_pedal_it_is_the_insert_and_its_latency_is_the_board_s`: the board with
Purus as its one pedal is Purus inserted alone, to the bit, offline and live).

## Ours, better

- A gentle de-hum that keeps the bass: a note on 50, 100 or 150 Hz loses under half a
  decibel, where notches take 6.5–8.8 dB of it; its phase-locked reference holds a steady
  hum's third harmonic at −75 dBFS.
- A de-reverb held to the null: it measures whether a room is there before it takes any,
  so a dry voice passes untouched at its defaults (the owner's standard).
- What it takes out shown as what fell out of the picture, live, for every module at once.
- Latency only for what is on, re-reported to a host the moment it changes.
- The de-click shares the audio editor's cores to the bit; exact rest after silence.
