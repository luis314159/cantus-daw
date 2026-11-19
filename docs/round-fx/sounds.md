# The site's sound samples, redone on recordings (branch `fx/sounds`, 2026-11-18)

The owner, listening to the site: the guitar effects must be heard on the royalty-free guitar
we downloaded and credited ("si no qué chiste"), and every unit's samples should be thought
again so they sell the product better. This file is the plan and what was done.

## 1. Principles

1. **The point in the first two seconds.** Every excerpt starts on the music (bar 5 of a
   loop, or a voice's first word), so the effect is heard at once, in a musical context.
2. **Dry next to wet, at one loudness.** Every effect excerpt on a recording and its dry
   take are brought to −19.3 LUFS by a gain alone (lowered as one where a peak would pass
   −1.5 dBFS). The ear compares the effect, not the level. Nothing else runs after the unit:
   no limiter, no EQ, no hidden mastering. The page says so.
3. **Real recordings wherever one exists, credited where heard.** Synthetic sources remain
   only where no recording fits.
4. **Range, not repetition.** The presets are chosen for how far the unit goes: clean,
   crunch and high gain for an amp, not three clean presets.
5. **Honesty.** A card's claims stay as measured. A take says it is a factory preset. The
   level the preset itself leaves is printed by the generator, and Try it shows it.
6. **Format.** MP3 CBR 96 kbit/s at 48 kHz, as Try it. The old card excerpts were at 44.1 kHz;
   the new ones use the same encoder settings as Try it. Peaks stay at or below −1 dBFS
   after decoding.

## 2. The sources

| Id | What | Licence and credit (shown EN/ES beside the excerpts) |
|---|---|---|
| `guitar` | *Strummed Guitar Chords* by Carbonthief (Renoise forum library, `https://files.renoise.com/forum/downloads/Instruments/Carbonthief-Strummed_Guitar_Chords/`): 24 dry DI strums. Played as Am, F, C, G, a chord a bar at 90 BPM, struck on beats 1 and 3, eight bars, peak −6 dBFS (a real DI's level, so drive presets meet the level they were made for). | CC0 (the archive's `LICENSE.txt` and `desc.html`). "Guitar: Carbonthief, CC0 (Strummed Guitar Chords, DI)". |
| `kit` | A rock groove at 90 BPM, eight bars (kick, snare with ghosts, closed hats opened on the last "and" of every second bar, tom fills closing bars 4 and 8), played by **Tympanum on the recorded kit of Gogodze Phu Vol. II** (Karoryfer Samples) through the engine's offline render. Gogodze has no cymbals, so none plays. | CC0 1.0 (the pack's `LICENSE`; `cantus-content` manifest). "Drums: Gogodze Phu Vol. II by Karoryfer Samples, CC0, played by Cantus's Tympanum". |
| `voice` | The Gettysburg Address read by Britton Rea, from its first word (bars 1–4 at 90 BPM, 10.7 s). | Public domain, released by its author (Wikimedia Commons `File:Gettysburg_by_Britton.ogg`). "Voice: Britton Rea, public domain (Wikimedia Commons)". |
| `piano` | A ballad phrase (pedal down) on the **Salamander Grand Piano V3**, played by Chorda's recorded source (the Exemplar engine; the modelled Chorda being retuned is not involved), from round 40's listening set `pianos-c` (`ballad - 1 Salamander.flac`); the first 10.7 s. | CC BY 3.0, as `content.json`'s attribution: "Piano: Salamander Grand Piano V3 by Alexander Holm (FLAC edition by FreePats), CC BY 3.0, played by Cantus's Chorda". |
| `pad` | Calor's factory *Velvet pad* holding Am, F, C, G, a bar each, 90 BPM. | Cantus's own: "Pad: Cantus's own Calor (Velvet pad)". |
| `mix` | The `kit` drums (−21 LUFS), Gravis on the roots in eighths (−24 LUFS) and the `guitar` (−21 LUFS), summed. | The three credits above, in one line. |

**The level each recording is fed at** is where such a track sits in a production:
- the drums at −18 LUFS (peaks near full scale, as a drum bus);
- the mix at −16 LUFS (a pre-master, peaks about −0.7 dBFS);
- the voice, the piano and the pad at −20 LUFS;
- the guitar at its DI level (peak −6 dBFS, about −19 LUFS).

The first attempt fed them at Cantus's own audition sources' loudness, which sits 8 dB down
(`SOURCE_TRIM`; the drum source is −34 LUFS). There, `comprimo-drum_bus`, `tonsor-safety` and
the other dynamics presets left the take bit-identical to the dry one. **The excerpts on the
site before this branch had the same fault:** `comprimo-drum_bus.mp3` measures exactly as
`source-drums.mp3`, at −19.44 dBFS peak and −34.0 LUFS.

The raw files are not in the repository, as ADR 0030 requires. The guitar's FLACs are in
`C:\projects\daw-fx\refs\audio\carbonthief\x\SampleData` with the README and licence. Gogodze
is installed as a Cantus pack (`%LOCALAPPDATA%\Cantus\Content\gogodze-phu-vol-ii`). The voice
is `voice.flac` in the takes folder, gathered by the effects round. Only the short MP3
excerpts are committed.

**The modelled Chorda is not used** while it is being retuned (DSP_VERSION 10). The synthetic
band's keys are Lamina; the piano above is the recorded Salamander.

## 3. Which source a preset is heard on (`render_unit_sounds::source_for`)

- A preset for **drums** goes on `kit`.
- A preset for a **voice** goes on `voice`.
- Anything else goes on `guitar` when the unit's natural source is a guitar (Delay, Tremor,
  Porta, Impetus, Discrimen), or when the preset's id says *guitar*.
- On `piano`: the reverbs, Spatium and Aula. The space shows on a piano's decays.
- On `pad`: the effects made for synthesisers, Concentus (the Juno chorus), Flexus, Anulus
  and Translatio.
- Anything else goes on `mix`: buses, the master and tone shaping (Aequalis, Comprimo,
  Terminus, Multiplex, Sibilus, Fervor, Latitudo, Culmen, Tonsor, Speculum, Libra, Fluctus).
  The demo song was not used: it plays the modelled Chorda, which is being retuned. The mix is
  recorded drums, bass and the recorded guitar.
- **No excerpt: Comparatio.** It matches a mix's loudness to a reference's. An excerpt matched
  to one loudness would erase exactly what it does, and one left at its own level would show
  only a gain. Its card keeps its window's pictures and no ▶.
- **Still synthetic:**
  - Tonus and Vibrans need a sung, pitched line, and no recording here is one. No CC0 sung
    vocal was searched for in this pass.
  - The instruments keep their phrases (`--effects` keeps their excerpts untouched; Chorda is
    being retuned).
  - Reverb has no card excerpts, as before.
- **Cedo** keeps its Try it take: a synthetic pad ducked by the song's beat. A kick heard
  beside the ducked pad would need a track mixed in after the unit, so it was not done.
- **Purus**'s faults (hum, buzz, clicks, crackle) are synthetic, added to the CC0 guitar. Its
  note says "added to show the repair".

The presets of these cards are chosen as before (`pick`: a role not heard yet, then a style
not heard yet), three a unit. There is one new rule. **An excerpt too close to its dry take
to hear is left out**, and the next preset in the same order takes its place. "Too close"
means the gain-matched difference against the dry take is below −30 dB. The generator prints
that difference and the preset's own level change for every excerpt.

Left out (measured):
- **Speculum, all eight presets** (−120 dB: it is an analyser and passes the audio
  untouched). Its card has no excerpt now; before, it had three that were the dry source.
- Sibilus `cymbals`, `bus`, `guitar_squeak`.
- Porta `vocal`, `guitar`, `noise`, `live_vocal`, `percussion`.
- Latitudo `vocal`.
- Tonsor `safety`.
- Impetus `vocal_consonants`.
- Discrimen `vocal_breath`.

Kept but subtle (−26 to −30 dB): Comprimo `glue`, Multiplex `deharsh`, Porta `snare`, Tonsor
`analog_warmth`, Impetus `pluck`.

## 4. The guitar units: Try it and their cards

The cards of the units that have Try it (the eleven guitar units, plus Loquax, Lenis,
Vinculum, Cedo and Purus) now list **Try it's own takes**. These are the same files, so
nothing is stored twice. The card's ▶ plays the first of them.

The guitar units' presets are now chosen by hand (`render_tryit::UNITS`), four each:

| Unit | Presets | Why |
|---|---|---|
| Arca | Chime jangle, British crunch, Full stack, Tight rhythm | clean → crunch → driven → high gain, four amps and cabinets (before: three clean presets) |
| Fragor | Green push, RAT rhythm, Vintage fuzz, Muff sustain | overdrive, op-amp distortion, germanium fuzz, triangle muff: the five-circuit board's range |
| Tabula | Guitar rig, Blues crunch, Fuzz lead, Ambient swells | full boards: drive + chorus + delay, compressor + drive, fuzz + delay, compressor + chorus + delay + space |
| Saltus | Twelve-string, Octaves both, Wide doubler, Dive bomb | the poly engine on chords (before: two divider presets meant for single notes) |
| Rota | Guitar, Chorale, Tremolo, Open back | the guitar preset first, slow against fast, the room |
| Cribrum | Cry wah, Funk wah, Bar sweep, Talking | envelope wah, quack, a synced sweep, the vowel filter |
| Vetus | Worn cassette, Lo-fi hip-hop, Warped tape, 8-bit | tape, dust, wow, bit-crush |
| Lapsus | Half time, Tape stop, Reverse half, Baby scratch | before: one preset only (the automatic filter left the rest out) |
| Caelum | Guitar ambience, Octave shimmer, Fifth cathedral, Bloom pad | the guitar preset first |
| Fractus | Reverse build, Gated sixteenths, Trance gate, Grain pad | reverse, gate and grains |
| Phasis | Phase 90 script, Small stone, Lamp chorus, Synced eighths | the script pedal, the swoosh, the lamp vibe, the tempo-locked sweep |

Vinculum's take is rebuilt on the `mix` above (recorded drums in place of Pulsus).

## 5. The page (`web/src/sounds.rs`, `spotlight.rs`)

- An excerpt's file may be a path (`/tryit/arca/full_stack`), so the cards reuse Try it's
  takes.
- "Hear its presets" lists one **Without the effect** row for each source its excerpts use
  (a unit may be heard on drums, a voice and the mix). Under the list is one line with the
  recordings' credits.
- The note under the list says how the level was set. On a recording: "each matched to the
  dry take's loudness by a gain alone". On Cantus's own sources: "each at its own level".
- The home page's spotlight shows the same credit line under its four excerpts.

## 6. Pipeline

```text
set C=%LOCALAPPDATA%\Cantus\Content          (Gogodze Phu Vol. II installed)
set T=F:\projects\daw-fx-d\takes             (voice.flac, piano.flac, pad.wav, faults.wav)
set G=%T%\guitar                             (the four strums as WAV, from refs\audio\carbonthief:
                                              Cantus's decoder reads no 32-bit FLAC)
cargo run --release -p cantus-ui --example render_tryit -- %G% web target/shots/tryit --takes %T% --content %C%
python web/tools/tryit_pictures.py target/shots/tryit
cargo run --release -p cantus-ui --example render_unit_sounds -- web --effects --guitar %G% --takes %T% --content %C%
```

`render_tryit` runs first, because the cards read its manifests.

## 7. Measured (decoded MP3s, `mp3meter`)

- **Card excerpts:** 120 files, 17 MB (before: 172 files, 16 MB at 44.1 kHz and 5.5 s). Every
  excerpt on a recording and its dry take is 10.7 s at −19.3 LUFS (−19.7…−19.9 after
  decoding). The instruments' and the synthetic sources' excerpts are untouched. The highest
  peak is −1.84 dBFS (true peak −1.79 dBTP); no sample clips.
- **Try it:** 16 units, four presets each, 256 pictures, 27.9 MB of takes and pictures
  (before: three presets, 19 MB). Every take is within 0.47 LU of −19.3 LUFS. The highest peak
  is −3.0 dBFS (Purus). The level each preset leaves is in its manifest.
- **WebAssembly:** 248,138 B gzip (before: 247,947 B), under the 250,000 B budget.
- `cargo test --features ssr`: 26 passed, among them the new
  `sounds::tests::every_excerpt_has_its_file_its_source_and_its_credit`. `cargo fmt --check`
  is clean. `contrast.mjs --quick`: 96 views, every text at AA. `motion.mjs` passes.
- **Not compared by ear by anyone yet.** The listening page for the owner is
  `F:\projects\daw-fx-d\sounds-review\index.html` (now and new side by side, with the dry
  takes and credits).
