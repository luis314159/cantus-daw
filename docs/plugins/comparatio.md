# Comparatio — the reference comparator

*Effect kind `Reference`. References for function: ADPTR Metric AB, Mastering The Mix's
REFERENCE, Melda's MCompare — read about and listened to, nothing copied (ADR 0146,
provisional number).*

*Comparatio* is Latin for a comparing, a setting side by side. It sits on the **master's last
insert** (any strip takes it) and holds **reference tracks** — finished records you want
your mix to stand beside. You switch between your mix and a reference **at the same
loudness**, see how they differ, and test your ears blind.

- **The switch**: the big selector (or the lamps, or A and B) changes what is heard through
  an equal-power crossfade eased so neither gain has a corner: the click of a switch between
  two sines measured **−94 dBFS** above 2 kHz (a hard switch: −43 dBFS).
- **The loudness match** goes on the reference only, toward your mix: **integrated** (over
  what has played beside your mix, or over the reference's whole file), **short-term** (every
  100 ms, held through quiet passages), or **by hand**, with an offset in every mode. Matched
  within **0.06 LU** on the unit's output (the DSP's tests) and **0.1 LU** heard on the
  master in the app (`comparatio_e2e`).
- **What is measured** (Terminus's own loudness code, EBU Tech 3341/3342 within 0.03 LU):
  short-term, integrated and momentary loudness, true peak and loudness range on each deck;
  crest factor, peak to loudness and the two short-term histograms under **Dynamics**;
  correlation and width for the whole, the lows, the mids and the highs under **Stereo**.
- **Tonal balance**: both long-term spectra in thirds or sixths of an octave, each relative
  to its own loudness (level never shows as balance) — the reference's from its whole file
  as it was read, yours **live from the slot's own signal**; both are the signal's middle,
  `(L + R) / 2`, so like meets like. Under them **the difference**, its uncertainty a soft
  band, the line fading where the comparison is unsure.
- **The match EQ**: the suggestion that would move your mix toward the reference (smoothed,
  weighed by how sure each band is, scaled by *Amount* and held under *Most boost or cut*)
  is drawn over the difference **before anything is sent**. **Send to Aequalis** fits it into
  at most six editable bands and puts them on an Aequalis **just before Comparatio** — its
  output gain keeping your mix's loudness where it was — as one step of undo; sending again
  updates that Aequalis rather than adding another. Comparatio itself never filters your mix.
- **Blind**: A (your mix), B (the reference), X (one of them, hidden — both lamps dark);
  answer *X is your mix* or *X is the reference*; each trial's lamp, the score and **the
  exact chance of guessing it**. A new test takes its order from the clock, so no order can
  be learnt.

**The null**: at its defaults your mix is heard, untouched to the bit; with no reference
loaded it is untouched whatever is set. **No render ever hears a reference**: an export, a
stem, a freeze, a bounce — each hears your mix, whatever the panel says (tested on a strip
and on the master, heard, hidden and with either blind choice). No latency.

## The references

Up to **eight** in one unit, each a file **by path** — relative to the project when it is
inside the project's folder — **never copied into the project**. Each is read off the audio
thread (WAV, AIFF, FLAC, Ogg; resampled to the session's rate), and each keeps **its own
analysis**: its length, its loudness, its tonal curves and its overview are read once and
kept after its audio is let go, so the list shows each one before you choose it.

- **Load reference…** picks files (the last one is compared); **drop** audio files on the
  console to do the same; the numbered keys choose the one compared; **Remove** takes it off
  the list. Each is one step of undo.
- Each reference is kept with **its size and a hash** of the file as it was loaded (the
  file's head, tail and 62 places between: read in milliseconds, it tells a re-exported
  master from the one loaded). A reference whose file is **missing** says so, and one whose
  file **changed** since it was loaded says so too — never skipped silently. Loading the file
  again takes its new print.
- **Drag across the overview** to loop that stretch of the reference (*Reference playback*:
  Loop); the playback can also be its own playhead while the song plays, or **locked to the
  song** with an offset.

## The window

A **two-deck reference console** on a felt desk between two cheeks (walnut, steel caps or
teak by colourway): your mix's deck on the left and the reference's on the right — a jewel
lamp lit when it is heard, the short-term loudness big, the other readings named in full
with their units, a level column; the dark screen between them (tonal balance, the
difference and the match EQ with *Send to Aequalis*, dynamics, stereo); along the foot the
reference's strip (its numbered keys, *Load reference…* and *Remove*, the one compared's
name, length and loudness, its overview with the playhead and the loop), the big selector
and the blind drawer. The cards: **Loudness match** (match, offset, measured over,
crossfade), **Reference playback** (playback, ahead by, loop start, loop end), **Match EQ**
(amount, most boost or cut), **Display** (thirds or sixths). Every word whole in English and
Spanish, the screen dark in every theme.

What is heard offers automation and **MIDI learn** on the selector, the lamps and the A, B
and X keys (a footswitch for A/B). The blind test's order and trial are kept by its own keys
and are no control's: a lane or a controller moving them would give the answer away.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/comparatio.rs` (14 keys; `References`, `Reference`, `repair_project`), `Command::SetReferences` |
| DSP | `crates/cantus-dsp/src/comparatio*.rs` (the unit, the measure, the stereo bands, the tonal curve, the reader, the blind test, the match EQ); the report `docs/round-fx/comparatio.md` |
| Engine | `crates/cantus-engine/src/fx/comparatio.rs` (the unit, its telemetry), `comparatio_store.rs` (references read off the audio thread, their summaries and prints, a render strips them) |
| Editor | `crates/cantus-ui/src/views/fx/comparatio.rs` (the console, the strip, the keys, Send to Aequalis, the drop), `comparatio_screen.rs`, `comparatio_picture.rs` (the picture, the live tonal curves), `comparatio_art.rs`, `comparatio_demo.rs` |
| Face | `crates/cantus-ui/src/kit/skin/comparator.rs` (walnut, broadcast, ivory) |
| Strings | `crates/cantus-ui/locales/fx/comparatio.{en,es}.txt` |
| Plug-in | `crates/cantus-plugins/src/comparatio_post.rs` |

## Presets

Fourteen ways of comparing (none changes your mix's level, none chooses what is heard or
the blind order — loading one returns to your mix): Mastering: the whole file's loudness;
Mixing: following section by section; Level by hand; Reference one decibel down; Locked to
the song; Loop a chorus (45 to 75 seconds); Loop the first thirty seconds; Quick switching;
Smooth switching; Low end in detail; Gentle match EQ; Full match EQ; Album: locked, the whole
file; Worship mix against a live record.

## Exported

`Cantus Comparatio` in CLAP and VST3, the same editor, bit-identical to the engine with
your mix heard. **Loading a reference inside another host**: *Load reference…* (a native
file dialog on the plug-in's window thread). The state keeps the references **by path,
size and hash** (the project's own format: `refs` and `ir` in the `EffectSpec` JSON), never
their audio; on loading a session the one compared is read on a thread of the plug-in's own
at the host's rate, and the editor says plainly when its file is missing or changed. **A
host's offline render hears your mix** (CLAP's `render` extension, VST3's
`processMode == kOffline`). The match EQ is drawn in the plug-in but *Send to Aequalis* is
Cantus's (another host's chain is not ours to change). Its ids are blessed in
`crates/cantus-plugins/tests/golden/ids.txt`.

*Dropping a file on the plug-in's window* is not taken yet: the window library
(egui-baseview 0.7) does not pass the system's drop to egui; in Cantus it is.

## Not a pedal on Tabula

Comparatio is **not** offered as a Tabula pedal, ever: a reference is the master's
monitoring — what the whole mix is compared with — not a sound in a guitar's chain, and a
pedalboard's lane would hand the reference into a render's path's neighbour. Tabula refuses
it (`cantus_core::fx::tabula::pedal_allowed`).

## Ours, better

- **Matched by the same meter the master and the export read** (Terminus's), held to the EBU
  cases, with a short-term match that does not jump at a quiet passage's edge.
- **A switch you cannot hear**: −94 dBFS of click against −43 for a hard switch.
- **Like meets like**: both tonal curves are the middle of the signal, each relative to its
  own loudness, with the comparison's uncertainty drawn and a confidence that fades the line.
- **The match EQ is editable bands in Aequalis**, its loudness kept, never a filter hidden
  in the comparator.
- **Renders never hear a reference**, in Cantus and in another host's offline bounce.
- **A blind test that cannot be learnt**: each trial's hidden choice a bit of a seeded hash,
  the seed from the clock, the score's chance exact.
