# Instruments to add next — research report (2026-10-10, second pass)

*The owner asked for "more instruments — the ones discussed and more, after research", and
then for this to be done better than a first pass that ran three web searches and took most
of its claims from memory. This pass verified the DAWs' stock line-ups, about forty reference
products, the licences of every proposed sample or voice source and the relevant patents
against primary sources (vendors' manuals, release notes and product pages; Google Patents;
licence files; Sound On Sound, MusicTech, MusicRadar, Attack, KVR, CDM), read the
repository's earlier research (`research/instruments/*.md`, `research/content/*.md`) and the
docs of every instrument Cantus already has. It feeds `docs/BACKLOG-excellence.md` §3e.
Nothing here is legal advice.*

## 0. The short version

1. **Three of the first pass's "new instruments" already exist in part.** Unda has an
   additive engine (*Partes*, 256 partials) and a granular one (*Grana*, deterministic
   grains); Pulsus II has 16 layers per pad, seeded round robin, chokes and a per-pad output
   hook. The work is to **grow** them, not to start over (§2).
2. **The 2025–26 bar moved to the sampler, the note tools and the drum machine, not to new
   synths.** Bitwig 6.1 rebuilt its Sampler with spectral stretch and 256-grain granular;
   Logic 12.3 added granular sync with formants; FL shipped Fruity Slicer 2 (2025) and
   deeper FPC pads (2026); Kontakt 8.6 and Serum 2 put arpeggiators and chord tools inside
   the instrument; Cubase 14 and Bitwig 5.3 added drum machines; **Cubase 15 is the first
   DAW with a built-in vocal synthesizer (Omnivocal, Yamaha's, in beta)** (§3, §5).
3. **The final order** (§9): slicer → arpeggiator/chord/scale → Pulsus per-pad outputs and
   chains → modelled grand piano → Exemplar spectral and granular modes → physical-model
   resonators → guitar → additive resynthesis → orchestral section players → vowel choir
   (lyric singing later) → acoustic drums.
4. **Piano** (owner's plan): build now on the published waveguide/closed-form path; evaluate
   Modartt's method after its US patent (**US 7,915,515, verified active to 2027-10-20; its
   EP/FR/JP/CN family has lapsed**) expires; legal review before release — which must also
   cover **seven active Casio, Kawai and Yamaha piano-resonance patents** (to 2029–2042)
   found in this pass (§6.4, §8).
5. **Content and voices.** CC0 sources are enough for orchestra (VSCO 2 CE, VCSL), electric
   guitars, basses and acoustic drums (Virtuosity Drums, six mics; Karoryfer kits).
   **Sonatina is out** (a retired, NC-like licence), **Philharmonia is out** for
   redistribution, Salamander and the best multi-mic kits are **CC-BY and wait on the owner's
   ruling**. **No shippable singing voice exists**: the open corpora are mostly
   non-commercial and the standard open vocoder (OpenVPI NSF-HiFiGAN) is CC BY-NC-SA; lyric
   singing needs a voice recorded for Cantus. No permissive choir or acoustic-guitar sample
   library exists either (§7).

## 1. Method and limits

- Six research agents worked in parallel (DAWs; synths and samplers; acoustic, drum and
  guitar references; singing synthesis; licences and patents; DSP literature), each told to
  cite the page it read and to list what it could not verify. Their findings were merged
  here and cross-checked against the repository's earlier reports; where they disagreed the
  newer primary source won (for example FL Studio 2026.1.7, 1 Oct 2026, over 2026.1.3).
- The shared web-search budget (200 searches per turn) ran out part-way; the remaining gaps
  are listed in §10 rather than filled from memory.
- Facts from the repository's own docs (what Cantus's engines do) were read from
  `docs/instruments/*.md`, `docs/STATUS.md` and the ADRs on 2026-10-10.

## 2. What Cantus already has — the engines the next instruments extend

Read from the repository (`docs/instruments/*.md`, `docs/STATUS.md`, ADR 0023, 0030, 0038) on
2026-10-10. **The first pass missed most of this**, and it changes the order: three of its
"new instruments" are partly built already.

| Engine (kind) | What it already does that the next instruments can reuse | What it lacks (relevant here) |
|---|---|---|
| **Exemplar** (`Sampler`) | groups → zones → samples; key × velocity, round robin and random drawn from the note, crossfades, release samples, loops; an **alias-free resampler** (de Soras octave mip-maps + half-band FIR) whose cost does not grow with transposition; WAV/AIFF/FLAC/Ogg; SFZ, Decent Sampler, SF2/SF3 import; SFZ export; every edit one command | **no slicing, no time-stretch, no granular or spectral playback**, no disk streaming (everything preloaded), SFZ tier-2 opcodes (CC modulation, `egN`/`lfoN`) logged not mapped |
| **Unda** (`Wavetable`) | three oscillators with **two extra engines**: ***Partes*** (additive: up to 256 sine partials as recursive rotators, tilt, odd/even, inharmonic *stretch*, culled above 18–20 kHz) and ***Grana*** (granular: Hann/Gauss/Tukey/Expodec grains of a sample or the table, density, length, spray, pitch jitter, reverse probability; grains scheduled on the sample clock from the note and every draw a hash of (note, grain, purpose) — bit-identical at any block size; a fixed pool of 32 per voice) | Partes: **no resynthesis** from a file or clip, no spectral filter/blur/pluck; Grana: a transposed-up sample aliases (no mip-mapped source), no sync/count density modes, no loop mode, 32 grains only |
| **Gravis** (`Bass`) | a **bidirectional digital waveguide per string** (Lagrange-read rails, one-pole loss from two T60s, four allpasses fitted per note for stiffness, loop tuned by phase delay to hundredths of a cent), pluck and pickup positions as geometry, magnetic and piezo pickups, **fret collisions as conditional reflections** (Rank & Kubin), slides, a *Strings* voice mode (one voice per string); plus 808 and Synth (ladder, diode 303, accent) engines | only bass strings and bodies; no bowed exciter, no acoustic body, no strummer |
| **Lamina** (`ElectricPiano`) | a **nonlinear hammer** (mass on a power-law spring, p = 2.5, pulse computed once by RK4, hardness and velocity scaling `v^(−(p−1)/(p+1))`), cantilever modes as complex one-pole resonators (decay changes per sample without clicks), dampers, geometry in millimetres | not a struck *string*: no waveguide string, no soundboard, no sympathetic resonance |
| **Pulsus II** (drums, in the channel rack) | synthesized voices including a **modal tom/perc voice with five materials** (membrane, bar 1 : 2.756 : 5.404, wood, plate, bell; 4–32 modes, tension glide); **16 sample layers per pad** with velocity zones, crossfades, round robin (Cycle / No repeat / Random, seeded from the note's ordinal so a seek repeats it), choke groups, per-pad polyphony, hi-hat pedal (CC4), filters, drive; `DrumPad::output` and `process_add_bus` exist | **per-pad outputs are not wired** in the engine (STATUS: the track loop must route each bus to its strip), **no per-pad effect chain**, no "any instrument on a pad" (Drum Rack), no kit import from a folder |
| **Modulus** (`Fm`) | eight operators any-to-any, DX7 algorithm templates, anti-hunting feedback | — |
| **Calor** (`Analog`) | VA with seeded drift, 16 voice cards, Draft/Good/Best equal live and offline | — |
| **Organum** (`Organ`) | 91-wheel tonewheel organ and a rotary speaker | — |
| **Firmus** | the basic two-oscillator synth | — |

Cross-cutting facts that constrain the next instruments:

- **No MIDI/note effects exist** (no arpeggiator, chord or scale unit; `grep` finds only a
  comment about a hosted synth's arpeggiator). STATUS also records that the instrument
  contract does **not yet pass tempo and song position** to native units (`SongPosition`
  exists for effects), which an arpeggiator, a slicer at project tempo and synced granular
  scans all need.
- **Licences of the crates**: `cantus-dsp`, `cantus-core`, `cantus-engine` are
  **MIT OR Apache-2.0**; the app crates are GPL-3.0-or-later. GPL code (aubio, Rubber Band,
  Loris, reSID, VocalTractLab) cannot go into `cantus-dsp` without relicensing that crate;
  MIT/BSD/Apache references (Signalsmith Stretch, WORLD, Clouds, STK, sfizz) can be ported.
- **Content** (ADR 0030 §5): the manifest admits **CC0 only** today, and a test enforces it.
  CC-BY material (Salamander Grand, Naked Drums, the DrumGizmo kits) waits for the owner's
  ruling. Packs download on demand into `<data dir>/Content/` and never enter the repository.
- **Threads** (ADR 0038): an instrument may fork its voices to the pool, one lane per voice,
  summed in voice order by one thread. Anything that **couples voices** (a piano's
  sympathetic resonance and shared soundboard, a choir's ensemble) must be a post-voice
  stage summed in a fixed order, not a per-voice one.
- **Determinism**: grains, round robins, humanise, drift and strums must be drawn from a
  counter hash of (seed, note id/ordinal, event index) — the pattern Pulsus II, Calor and
  Grana already use — and analysis (onsets, partials, pitch) must run offline with its
  *result* stored in the project, so a later change to the analyser cannot change an old
  song.

## 3. Stock instruments by DAW, verified (October 2026)

Corrections to the first pass are in **bold**. Sources are in the last column; all were seen
on 2026-10-10.

| DAW, current | Instruments shipped (top edition) | New in 2025–26 for instruments | Sources |
|---|---|---|---|
| **FL Studio 2026** (26.1 on 7 Jul 2026; 2026.1.7 on 1 Oct 2026). FL 2025 was 9 Jul 2025 | **39 generators** in All Plugins. All editions (23): 3x OSC, Autogun, BassDrum, BeepMap, BooBass, Channel Sampler, DirectWave Player, Drumpad, FLEX, Fruity DrumSynth Live, Fruity DX10, Fruity Granulizer, Fruity Kick, FPC, Fruity Slicer, **Fruity Slicer 2**, GMS, MiniSynth, Plucked!, SimSynth, Speech Synthesizer (Windows), Wave Traveller, FL Keys. Producer adds Kepler, Slicex, SoundFont Player, Sytrus. Signature adds DirectWave, Harmless. All Plugins adds Drumaxx, Harmor, Kepler Exo, Morphine, Ogun, Poizone, Sakura, Sawer, Toxic Biohazard, Transistor Bass | 2025: Loop Starter, Gopher (AI help), Mobile Rack, vector UIs for BassDrum/Drumaxx/Sakura. 2025.2: **Fruity Slicer 2** in every edition, Kepler Exo anti-aliased oscillators. 2026: **FLEX rebuilt** ("up to 50 % lower CPU"), Transmitter (an effect), Mobile Rack adds a **Note Arpeggiator**, **FPC layers several samples per pad**, chord detection and Chord Stamp voice leading, one-click stem separation, Gopher acts (writes piano-roll and scripts) | image-line.com/fl-studio/compare-editions; forum.image-line.com (p=2014957, p=2065217, p=2076063); image-line.com/fl-studio-news/fl-studio-2025-2 |
| **Ableton Live 12.4** (12.4 on 5 May 2026; 12.4.6 on 15 Sep 2026) | Intro: Drum Rack, Impulse, Instrument Rack, Simpler, Drift, CV Instrument, CV Triggers, **Drum Sampler**, External Instrument. Standard adds the Drum Synths, Analog, **Collision, Electric, Tension**. Suite (21 instruments) adds Bass, Operator, Poli, Sampler, Wavetable, Meld, Granulator III. MIDI effects: Arpeggiator, Chord, Scale, Random, Note Length, Velocity, Rotating Rhythm Generator, Note Echo, Melodic Steps…; 17 MIDI Tools (generators Rhythm, Seed, Shape, Stacks; transformations incl. Arpeggiate, Strum, Ornament; Chop, Glissando, LFO in 12.1) | 12.1 (Oct 2024): Drum Sampler, Auto Shift. 12.2: Meld Chord oscillator, Operator 32 voices. 12.3 (Nov 2025): stem separation (Suite). 12.4: Wavetable 16 voices, Link Audio, an Extensions SDK. **No new instrument since Drum Sampler.** **Bass and Poli are 2018 Max for Live synths, not new** | ableton.com/en/release-notes/live-12; ableton.com/en/live/compare-editions |
| **Logic Pro 12** (12.0 ~28 Jan 2026; 12.4 dated 29 Sep 2026) | Alchemy, Sample Alchemy, Sampler, Quick Sampler, Drum Machine Designer, Drum Kit Designer, Retro Synth, Sculpture, ES1, ES2, EFM1, ES E/M/P, EVOC 20 PolySynth, Klopfgeist, Ultrabeat, Vintage B3/Clav/Electric Piano/Mellotron, **Studio Piano, Studio Bass** (11.0), Studio Strings, Studio Horns; **Session Players**: Drummer, Bass Player, Keyboard Player (11.0) and **Synth Player (12.0)**, following **Chord ID** | 12.0: Synth Player (pads, rhythmic chords, 808/pump/sequenced bass), Chord ID. 12.3: **Alchemy granular sync ("pitched grains") and formant shifting**. 11.x: Stem Splitter (6 stems in 11.2) | support.apple.com/en-us/126835 and /109503; apple.com/logic-pro; cdm.link/logic-pro-12-hands-on; rekkerd.org |
| **Bitwig Studio 6.1** (6.0 on 11 Mar 2026; 6.1 by 26 Aug 2026) | FM-4, Phase-4, Polymer, Polysynth, **Sampler**, Organ, Poly Grid; Drum Machine; **5.3 (Feb 2025) added 25 drum devices** (v8 808-style, v9 909-style, v0 digital; E-drums renamed v1); Note FX: Arpeggiator, Humanize, Note Repeats, Stepwise, Note Grid | 6.0: workflow only (automation clips, clip aliases, project key, Grid scale modules). **6.1: Sampler rebuilt — Spectral time-stretch (formants, harmonic bend, quantise) and Fragments granular (256 grains/voice)**, slicing in every mode, pitch/tempo detection, round robin and layered multisampling, Sampler inside The Grid | bitwig.com/whats-new; downloads.bitwig.com/6.1/Release-Notes-6.1.html; soundonsound.com/news/bitwig-studio-6-launches |
| **Reason 14** (11 May 2026; **not 13**); owned by LANDR | Polytone, Europa, Malström, Monotone, Subtractor, Thor, **Grain**, Mimic, NN19, NN-XT, Dr. Octo Rex, **Kong**, Redrum, Rytmik, Humana, ID-8, Klang, Pangea, Radical Piano; players: Note Tool, Random Tool, Beat Map, Dual Arpeggio, Note Echo, Scales & Chords | 13 (Jun 2024): Polytone. 14: workflow (track panel, folders), no new instrument. Objekt/Algoritm not on the Reason Rack 14 list (Reason+ only?) | reasonstudios.com/shop/product/reason-rack-14; reasonstudios.com/press/reason-14-available-worldwide |
| **Fender Studio Pro 8** (was Studio One; 8.0 on 13 Jan 2026; 8.1.0 listed by KVR) | Mai Tai, Presence XT, Impact XT, Sample One XT, Mojito, Deep Flight One, **Sub Zero** bass (Studio One 7.2, Jun 2025), Lead Architect, **Cinematic Lights** (7.1, Jan 2025) | 8.0: AI Extract Notes / Extract Drums (audio → MIDI), Chord Assistant, **a slicing editor and 16-slot mod matrix in Sample One**; SOS: "no new virtual instruments" | soundonsound.com/reviews/fender-studio-pro-8; en.wikipedia.org/wiki/Studio_One_(software) |
| **Cubase 15** (5 Nov 2025; 15.0.30) | HALion Sonic, Groove Agent SE 6, Sampler Track, **Drum Machine** (Cubase 14: a sample module + 38 synth modules, 128 pads), **Omnivocal** (beta vocal synthesis); Retrologue and Padshop **not verified** this pass | 14 (Nov 2024): Drum Machine, Pattern Editor (Euclidean). 15: **Omnivocal**, Groove Agent SE 6 redesign, 40 kits, melodic pattern sequencer, stem separation. (**"Underground" is not a Cubase instrument**; Cubase 14's effect is "Underwater") | soundonsound.com/reviews/steinberg-cubase-pro-14 and -15; kvraudio.com; musictech.com/features/opinion-analysis/cubase-15-ai-tools |

**Reading it.** Through 2025–26 the DAWs shipped *workflow, AI and sampler upgrades* rather
than new synths. The genuinely new stock instruments are few: Live's Drum Sampler, Cubase's
Drum Machine and **Omnivocal**, Bitwig's 25 drum devices and its **spectral/granular
Sampler**, Studio One's Sub Zero and Cinematic Lights, Reason's Polytone, Logic's **Synth
Player**, FL's **Fruity Slicer 2** and rebuilt FLEX. The bar therefore moved in four places:
**the sampler** (slicing, spectral stretch and granular as modes — Bitwig 6.1, Logic 12.3,
Fender's Sample One), **note tools and players** (arpeggiators and chord tools in every DAW,
now inside Kontakt and Serum too; chord-following players in Logic, FL and Fender), **drum
machines with deep pads**, and **vocal synthesis** (Cubase alone, in beta).

## 4. The reference products, verified

All pages seen 2026-10-10 unless a date is given. "Snippet" means a search-engine excerpt
whose page was not read in full.

### 4.1 Synths and sound-design instruments

| Product | Version, date | What it is now | Praised for | Criticised for | Sources |
|---|---|---|---|---|---|
| **Xfer Serum 2** | released 17 Mar 2025 (free to Serum 1 owners); 2.1.5 in the download links; Linux beta Aug 2026 | three oscillators, each **Wavetable, Multisample (SFZ import), Sample (loop detection, real-time slicing with score extraction), Granular, or Spectral** (real-time harmonic resynthesis with transient detection; imports samples, tables or PNG); an **arpeggiator and a clip sequencer** (12 slots each, piano roll, MIDI import, MPE editing); 4 envelopes, 6 LFOs (Path XY, Lorenz, Rössler), 8 macros; USD 249 | the Spectral mode; the clip sequencer "adds a new dimension" (MusicTech 8/10, 10 Apr 2025) | **"More CPU-hungry than ever"** (MusicTech); a KVR user calls the CPU "horrible" next to Hive; the multisample mode "a little gimmicky" | xferrecords.com/products/serum-2; kvraudio.com/news/…-63286; attackmagazine.com (17 Mar 2025); musictech.com/reviews/software-instruments/xfer-records-serum-2-review |
| **Vital** | 1.6.4 (KVR; undated) | wavetable synth, spectral warps, audio-to-table, MPE, microtuning; free Basic tier | UI "a triumph", drag-to-modulate, light 60 fps visuals | "busy"; development stalled (forum thread "Is this a dead synth?") | vital.audio; kvraudio.com/product/20186; forum.vital.audio |
| — Vital licensing | — | **code GPL-3.0**; factory presets and wavetables under a separate licence that forbids redistribution; the names "Vital"/"Tytel" may not name builds | — | — | github.com/mtytel/vital; vital.audio/eula (29 Feb 2024) |
| **Arturia Pigments 7** (the task said 6) | 7 released 16 Dec 2025 (7.0.1); 6 released 28 Jan 2025 | three engine slots over **six types: Virtual Analog, Wavetable, Sample, Granular, Harmonic (additive), Modal** (physical modelling, added in 6); 7 added filters, the Corroder FX, a new Play page, lower CPU; USD 199 | "a strong contender for the best soft synthesizer on the market" (Attack, 16 Dec 2025); lower CPU | no new engine; Play-page animation distracting | arturia.com; attackmagazine.com/reviews/gear-software/arturia-pigments-7-…; sonicstate.com (16 Dec 2025) |
| **Spectrasonics Omnisphere 3** | released 21 Oct 2025 (announced 16 Oct) | 26,400 patches, Adaptive Global Controls, Patch Mutations, 600+ wavetables, 36 new filters, 93 FX, 300+ hardware profiles, full MPE, new browser; USD 499 | **MusicTech 10/10 (15 Dec 2025): "extremely CPU-friendly"**, the browser (keyword, mood, timbre) | price; the size of the library overwhelms | spectrasonics.net; musictech.com/reviews/plug-ins/spectrasonics-omnisphere-3-review |
| **u-he Diva** | 1.4.8, 27 Aug 2024 | five oscillator models, five filters, circuit-level modelling, separate offline quality; €179 | "puts paid to the analogue vs digital debate" (MusicRadar) | "the CPU hit can be very brutal"; u-he says so itself | u-he.com/products/diva |
| **u-he Zebra 3** | released 20 Apr 2026; 3.0.2, 15 Jul 2026 | spline wavetable/additive oscillator (**up to 1,024 partials, inharmonic allowed**), 2-op FM oscillators, a **modal** oscillator, granular texture module, 13 filter models; CLAP/VST3/AU/AAX; €249 | decluttered UI, sound | **about 2× the CPU of Zebra 2 per module** (u-he); small knobs; no arpeggiator | u-he.com/products/zebra3; synthanatomy.com (15 Jul 2026); gearnews.com |
| **Kilohearts Phase Plant** | current version not found; granular generator added in 2.1 (Apr 2023) | generators: analog, wavetable, sample, noise, granular; three effect lanes of Snapins; USD 199 | single-window modular design (MusicRadar 4.5/5, MusicTech 9/10) | CPU on dense patches; premium effects cost extra | kilohearts.com/products/phase_plant; cdm.link (Apr 2023) |
| **NI Massive X** | 1.7, 3 Mar 2026 (every parameter automatable); 1.6, Jul 2025 (Play view, morpher) | two wavetable + two phase-modulation oscillators, Performers | — | — (NI entered preliminary insolvency in 2026: Synth Anatomy, Mar 2026) | native-instruments.com; synthanatomy.com/2026/03/… |
| **Arturia V Collection 11** | 24 Apr 2025 | seven new instruments (Pure LoFi, JUP-8000 V, SEM V, MiniBrute V, Synthx V, **Augmented Mallets**, Augmented Yangtze), 45 in all; arpeggiator gained Chance, Auto Regen, Cycles lanes | — | — | soundonsound.com/news/arturia-v-collection-11-released; synthtopia.com (25 Apr 2025) |
| **Image-Line Harmor** | — | additive engine editing a table of partial frequency and amplitude, **up to 516 partials** per note per unison voice; **resynthesis of dropped audio and of images**; real-time partial display | (earlier research: "has aged surprisingly well") | — | image-line.com manual, Harmor |
| **Apple Alchemy / Sample Alchemy** | Alchemy 2.0 in Logic 10.2 (2016); Sample Alchemy in 10.8; granular sync mode with formant control in Logic 12.3 (repo research) | four sources, each **additive, spectral or granular** resynthesis; partial tracker; Sample Alchemy grains 2–230 ms with several playheads | the flagship of Logic's bundle (repo research) | — | help.apple.com (Alchemy); cdm.link |
| **Ableton Meld** | Live 12 | two engines, 24 oscillator types, **6 scale-aware** ("Use Current Scale"), MPE matrix | smooth, versatile (SOS, repo research) | — | ableton.com/en/packs/meld |
| **Ableton Granulator III** (Robert Henke) | "[2023]", Live 12 Suite | **Classic, Loop and Cloud** (up to 20 unsynced grains) modes; grains 2 ms–2 s; built-in **Capture** of audio; full MPE; 8 voices | "seriously enhanced", MPE-capable (SOS, repo research) | — | roberthenke.com/technology/granulator3.html |
| **Reason Grain** | Reason 10 (2017) | four engines including **Spectral Grains** (additive resynthesis of each grain) | — | — | reasonstudios.com/devices/grain; soundonsound.com |
| **Output Arcade** | 2.16.1, 4 Aug 2026 (2.0 in Oct 2021) | **loops spread across the keys, synced and tuned**, black-key modifiers (Resequence, Playhead, Repeater), Note Kits, Kit Generator (auto-slices dragged audio into a kit) | easy browsing; lo-fi and sound design | **subscription only**; shallow as a portrait of real instruments (bonedo) | support.output.com (release notes); bonedo.de |
| **Plogue chip line** | chipsounds is now "legacy" (2009); the **chipsynth** line replaced it: C64 (every 6581/8580 revision), MD (bit-accurate YM2612 + SN76489), OPMZ (YM2151/2164/2414/2424), OPS7 (DX7-class OPS), PortaFM (OPLL, OPL2, OPK2), SFC; chipcrusher; all CLAP/VST3/AU/AAX | — | — | — | plogue.com/products/ and each product page |

### 4.2 Samplers and the sampler-as-instrument bar

| Product | Version, date | What matters for Cantus | Sources |
|---|---|---|---|
| **NI Kontakt 8** | launched 23 Sep 2024; **8.6.0 (Aug 2025) added Arpeggiator, Chord Builder, Humanizer, Scale Lock and Velocity Curve "Tools"**, stackable; 8.10.0 on 29 Apr 2026 (snippet) | MIDI tools inside the sampler are now standard; Leap (loops and one-shots); Creator Tools with Lua; KSP scripting. Reviews: the side-pane search "vastly appreciated"; Tool previews play Piano Uno instead of the loaded instrument (MusicTech, 3 Feb 2025) | docs.native-instruments.com (new in Kontakt 8); musictech.com/reviews/…/kontakt-8-review |
| **Bitwig 6.1 Sampler** | 6.1, 26 Aug 2026 | **Spectral** mode (time-stretch with Pure/Reharm characters, harmonic bend, quantise to key, preserve onsets, formants); **Fragments** granular (up to 256 grains per voice, triggerable by audio); Cycles (pseudo-wavetables); a Sliced Sampler; automatic pitch and tempo detection. The new bar for a stock sampler | downloads.bitwig.com/6.1/Release-Notes-6.1.html; bitwig.com/whats-new |
| **FL Studio Slicex / Fruity Slicer 2** | Fruity Slicer 2 shipped in FL Studio 2025.2 (Nov 2025), in every edition | slicing by embedded markers first, then **Dull/Medium/Sharp** transient detection or fixed beat divisions; per-slice gain, pan, attack, release, transpose, detune, filter, reverse, play-to-end, chromatic; time-stretch None/Resample/Stretch (élastique); **"Dump score to piano roll"** (auto-dump on load); **Random** (rearranges slices keeping length and groove), **Crazy**, **Stutter**, Flatten, Quantize/Swing. Slicex adds two decks, articulators and layering; its drum stretch is destructive | image-line.com manual pages Slicex.htm and Fruity Slicer 2.htm; kvraudio.com/news/…-64306 |
| **Ableton Drum Sampler** | Live 12.1, Jul 2024 | one-shot sampler with nine playback effects (stretch, loop, pitch env, punch, 8-bit, FM, ring mod, sub, noise), similar-sample swapping; MusicRadar: "you still need Simpler for slicing" | ableton.com manual §31.4; musicradar.com |

### 4.3 Keys, physical models, orchestral, guitar

| Product | Version, date | Technique, size | Praised | Criticised | Sources |
|---|---|---|---|---|---|
| **Modartt Pianoteq 9** | **released 20 Oct 2025**; 9.1 with the Syngular add-on 10 Dec 2025 | pure physical model, **about 50 MB**; new soundboard-vibration model for stereo and spatial image; up to 8 mics in 3D; hammer tone; thunder pedal; VST2 dropped; Stage €139 / Standard €269 / Pro €399 (per-note editing) | SOS (Robin Bigwood, Apr 2026): "still one of the best options out there"; users: less fatigue than v8, "a huge leap forward in expressiveness" | SOS: "occasional fleeting unnaturalness", stiff bass notes on the Steinway D; long-running user complaint of "harsh metallic ringing" (v8.2 threads, 2024) and a "synthetic nature… very difficult to define" | modartt.com/news; modartt.com/pianoteq_whats_new; soundonsound.com/reviews/modartt-pianoteq-9; forum.modartt.com (ids 12646, 11513); pianoclack.com |
| — Modartt in 2026 | **Airteq** (15 Sep 2026, a modelled pad/texture instrument, €99); Champagne Flute (free, 26 Aug 2026) | — | — | — | modartt.com/news |
| **Spectrasonics Keyscape** | launched Sep 2016; no 2025–26 update found | sampled, **77 GB**, 36 models, up to 32 velocity layers plus round robins | "feels and plays like a real instrument"; the C7 "up there with the best" (MusicTech 2016) | macro-only editing outside Omnisphere; two mic sets; RAM and SSD | rekkerd.org; musictech.com/reviews/spectrasonics-keyscape-review |
| **Logic Studio Piano** (Logic 11, 2024) | — | three sampled pianos, three mic pairs, pedal/key noise, resonance | — | — | apple.com/logic-pro/plugins-and-sounds; soundonsound.com/techniques/logic-pro-studio-piano |
| Other pianos | NI Noire (16 GB, CFX); Ravenscroft 275 (~35 GB expanded); Garritan CFX (20 layers, pedal-down and soft-pedal sets, ~130 GB) | sampled | — | — | native-instruments.com; bigfishaudio.com; ask.audio; musictech.com |
| **AAS Chromaphone 3** | 3.2.0 (Oct 2020) | **coupled modelled resonators** (beam, marimba, open/closed tube, string, drumhead, plate, membrane), two layers; ~110 MB | the sound (MusicRadar) | the Home tab adds little | applied-acoustics.com/chromaphone-3 |
| **AAS Strum GS-2** | 2.4.6 | fully modelled guitar, "no velocity layers"; chords voiced as a guitarist would; strum keys (down, up, mute, scratch); 700+ strum loops; ~70 MB | — | no independent criticism found | applied-acoustics.com/strum-gs-2 |
| **Audio Modeling SWAM** | Solo Strings 3.12.4, 8 Oct 2026; **PolySWAM** (polyphonic modelled ensemble) Sep 2026, USD 399 | physical + behavioural modelling with phase-synchronous sample morphing (a hybrid); **needs continuous expression input** (CC, pedal or breath controller) | SOS 2019: flute and sax unmatched, smooth legato | oboe synthetic until the player adapts; clicks on wide descending legato | kb.audiomodeling.com; soundonsound.com/reviews/audio-modeling-swam-solo-instruments; blog.dubspot.com |
| **Ample Guitar / Bass 4.1** | guitars 18 Jun 2026, basses 13 Mar 2026 | sampled; **strummer with 28 play styles per chord, "Strum Note" (a chord as one editable object), piano-roll/tab view, Guitar Pro GP3–GP8 import**; 9–12 articulations; 20+ GB | — | — | amplesound.net |
| **NI Session Guitarist / Bassist** | Picked Acoustic (SOS Jul 2020), Electric Sunburst (SOS Oct 2018), Upright Bass (Apr 2024) | sampled patterns: chords played on the keys become guitar voicings and grooves | "ridiculously easy", mix-ready | **you cannot make your own patterns**; fixed 3-semitone slide | soundonsound.com reviews; native-instruments.com |
| **Spitfire LABS → Splice INSTRUMENT** | Splice bought Spitfire in Apr 2025; **Splice INSTRUMENT launched 7 Oct 2025** (all LABS sounds; free tier plus USD 13/month); the standalone LABS plug-in "is no longer available" (Audiotool) | sampled presets with 2–5 controls each | simplicity, curation, playability | few controls | musictech.com; kvraudio.com/news-print.php?id=64929; audiotool.com |
| **BBC SO Discover** | free, instant download | **34 instruments, 47 techniques, one mix, 240 MB** | — | — | spitfireaudio.com/bbc-symphony-orchestra-discover; soundonsound.com/news |

### 4.4 Drums

| Product | Version, date | What matters | Praised | Criticised | Sources |
|---|---|---|---|---|---|
| **NI Battery 4** | still current; "Battery 5" exists only on an affiliate site (treated as false) | 7 sample modes, Time Machine Pro, 4 buses with sidechain, tag browser | deep per-cell editing, light CPU | UI "largely unchanged since 2013"; no smart browsing or beat building | native-instruments.com; blog.dubspot.com (16 Jun 2026) |
| **XLN XO** | — | **similarity map ("Space")**, 8-track sequencer, one-click kit swap, export samples/stems/MIDI | UI, the map, speed | basic editing, **no layering or round robin** | xlnaudio.com/products/xo; thomann reviews; KVR |
| **Toontrack Superior Drummer 3** | no SD4; SDX expansions only (Sep 2026) | ~230 GB, per-mic bleed controls, **Tracker** (audio → MIDI), ~1,640 MIDI grooves | — | disk size, RAM-hungry expansions | toontrack.com |
| **EZdrummer 3** | — | **Bandmate** (proposes grooves for dropped audio/MIDI), **Tap2Find**, Song Creator | "stunningly good" (MusicRadar); Gold (Production Expert, 30 May 2022) | audible repetition in some snare round robins | toontrack.com; production-expert.com |
| **Addictive Drums 2** | 2.5 adds Trig Gate, compressor Boost, visual EQ; 2.6 (2025) recorder and snapshots (repo research) | — | not verified | not verified | xlnaudio.com |

### 4.5 Vocal and choir

| Product | Version, date | What it is | Praised / criticised | Sources |
|---|---|---|---|---|
| **Cubase 15 Omnivocal** | Cubase 15, 5 Nov 2025; **open beta** | **the first vocal synthesizer built into a major DAW**: developed by Yamaha for Cubase; type a lyric on a note in the Key Editor's text field, converted to IPA (editable); male and female voices; formant, attack, air automatable | MusicTech (11 Dec 2025): "astonishing", convincing with effects and harmonies, laborious for a lead (IPA, automation); **MusicRadar: as a beta it "cannot compete with commercial alternatives"** | musictech.com/features/opinion-analysis/cubase-15-ai-tools; musicradar.com/music-tech/daws/steinberg-cubase-15-review (snippet); recordingmag.com |
| **Dreamtonics Synthesizer V Studio 2 Pro** | 21 Mar 2025; 2.1.0 (17 Jun 2025) added ARA2 in Pro Tools | hybrid concatenative + neural; renders "up to 300% faster", no GPU; AI Retakes, Vocal Modes, Smart Pitch; USD 99, voices USD 79 | the bar for quality | kvraudio.com/news/…-63333; soundonsound.com/news |
| **Yamaha VOCALOID 6** | 13 Oct 2022 (page shows 6.9) | VOCALOID:AI; one voicebank sings Japanese, English, Chinese; VOCALO CHANGER | weak source: AI voices "lack character", hard to tune | vocaloid.com/en/vocaloid6 |
| **ACE Studio 2.0** | — | 140+ voices, 8 languages, render credits then subscription | "next-gen AI features, stone age workflows" (MusicTech) | musictech.com/reviews/…/ace-studio-2-review |
| **Emvoice One** | — | renders **in the cloud** | sketches and layers; still synthetic | mixdownmag.com.au; kvraudio.com |
| **EastWest Hollywood Choirs** | — | sampled choir with **WordBuilder** (type words) | — | synthtopia.com (2017); soundonsound.com |
| **Arturia Augmented VOICES** | — | sampled voices + four synth engines, Morph | — | arturia.com |
| FL Studio | — | ships the old **Speech Synthesizer** (formant text-to-speech); RouteNote calls FL's tool "rougher" than Omnivocal | — | create.routenote.com; repo research (family.md) |

## 5. What a top DAW is expected to ship in 2026 (from §3–4)

| Expected now | Who ships it | Cantus today |
|---|---|---|
| A slicer with MIDI dump | FL (Slicex, Fruity Slicer 2), Live (Simpler), Logic (Quick Sampler), Bitwig, Fender (Sample One), Serum 2 | **missing** |
| Arpeggiator, chord, scale tools | every DAW; Kontakt 8.6, Serum 2, FL 2026 Mobile Rack | **missing** |
| A drum machine/sampler with deep pads, per-pad outputs and chains | Live Drum Rack + Drum Sampler, FL FPC (layers in 2026), Cubase Drum Machine, Bitwig v8/v9/v0, Logic DMD | Pulsus II, **outputs not wired, no per-pad chains** |
| Granular | Live Granulator III, Logic Alchemy (grain sync 12.3), Bitwig Fragments, Reason Grain, FL Granulizer | Unda *Grana* (basic) |
| Spectral/time-stretch sampler | Bitwig 6.1, Logic Alchemy, Serum 2 | **missing** (also needed by §3 *Time*) |
| Additive resynthesis | FL Harmor, Logic Alchemy, Reason Grain (spectral grains) | Unda *Partes* (no resynthesis) |
| Physical modelling | Live Collision/Tension/Electric, Logic Sculpture, Pigments Modal | Gravis string, Pulsus modal, Lamina — **no general instrument** |
| An acoustic piano | Logic Studio Piano; FL Keys (weak); Live packs | **missing** (CC0 Osiris pack only) |
| Strings/horns | Logic Studio Strings/Horns; Cubase HALion Sonic content | **missing** (CC0 VSCO 2 CE as Exemplar packs) |
| Players following chords | Logic Session Players + Chord ID; FL chord detection; Fender Chord Assistant | **missing** (§7) |
| Vocal synthesis | Cubase Omnivocal (beta); FL Speech Synthesizer (old) | **missing** |
| Stem separation | FL, Live Suite, Logic, Fender, Cubase Pro | §7 (not an instrument) |

## 6. The candidates in depth

Each candidate: the bar the leaders set (what users praise and complain about), the
technique and its references, how it fits Cantus's rules (null at defaults, block-size
independence at tolerance 0, exact rest, `cantus_dsp::math` only, voices on the pool per
ADR 0038, the hand-drawn kit), the CPU to expect, the risks, the build cost (S = one
builder-round on top of an existing engine, M = one builder-round of new engine, L = two or
more), and a measurable *done when*. Full citations are in §11.

### 6.1 Slicer — *Sectio*, a slicing mode of Exemplar

**The bar.** FL users slice daily: Fruity Slicer 2 (new in FL Studio 2025, so Image-Line is
still investing) and Slicex; Ableton Simpler's Slice mode; Logic Quick Sampler; Bitwig 6.1's
Sliced Sampler; Serum 2 even slices inside an oscillator; Output Arcade's Kit Generator
auto-slices dropped audio. What users rely on (manual pages, §4.2): **embedded markers read
first** (WAV `cue `/`LIST adtl`, ACID, REX), **one sensitivity control** (FL: Dull/Medium/
Sharp), grid slicing, **"dump score to piano roll"** so the loop replays from MIDI, per-slice
pitch/envelope/filter/reverse, and the creative re-orderings (*Random*, *Stutter*,
*Flatten*, *Quantize/Swing*). Complaints: Slicex's drum stretch is destructive; the detected BPM
must be corrected by hand when wrong (FL manual); Drum Sampler cannot slice (MusicRadar).

**Technique.** Onset detection by spectral flux on a log-magnitude STFT with adaptive peak
picking (Bello et al. 2005; Dixon 2006 showed plain spectral flux is as good as the complex
methods), **SuperFlux** for sustained material (Böck & Widmer 2013: a maximum filter across
frequency, up to 60 % fewer false onsets on vibrato); tempo and beat grid by Ellis's dynamic
programming (2007) — offline, deterministic. Slice start = onset minus a few ms, snapped to a
zero crossing or a 1–2 ms fade. Playback is Exemplar's zone engine, one zone per slice, so
the alias-free resampler, envelopes and filters come for free. Tempo following: **slice
replay is transient-perfect by construction** (each slice starts on its note); stretching
inside a slice (when the project is slower than the loop) uses gap-fill (loop the tail,
FL's "fill gaps") or, later, Signalsmith Stretch (MIT) once §3 *Time* lands.

**Fit.** Analysis runs on a worker when the file is dropped; the **slice points are stored
in the project** (a command, undoable), never recomputed at load — so a better detector
later cannot move an old song's slices. Rendering is ordinary zone playback: block-size
independence and offline = real time already hold in Exemplar. Null test: a loop sliced and
replayed from its dumped MIDI at its own tempo equals the file to the bit (slices are
contiguous and unfaded at their boundaries when fades are 0).

**Cost.** Negligible at play time (zone playback). Analysis: one FFT per hop, offline.
**Build: S** (the engine is Exemplar; new: the detector, the slice editor in the kit, the
MIDI dump, the re-ordering tools, tempo and position from the host — the contract gap noted
in §2).

**Risks.** No slicing patent found in this pass (the search budget ran out before ReCycle-,
warp- or onset-specific searches; §10). Code references: aubio is GPL (cannot enter
`cantus-dsp`); madmom's code is BSD but its trained models are **CC BY-NC-SA** — do not use
them. Writing our own spectral flux + SuperFlux + Ellis DP is small and keeps the crate MIT.
REX files need Propellerhead's SDK (licence not checked) — read WAV/ACID markers first.

**Done when** (measurable): on 50 hand-annotated CC0 loops (drums, bass, vocals), ≥ 95 % of
slice points within 10 ms of the annotation at the default sensitivity; a loop sliced and
replayed from its own dumped MIDI equals the source to the bit; a tempo change keeps every
slice on its grid position; *Random*, *Stutter*, *Reverse* and *Quantize* are each one undo
step; dropping a loop to hearing it chopped takes ≤ 2 clicks.

### 6.2 Arpeggiator, chord and scale tools — note effects in front of any instrument

**The bar.** Every DAW ships them (FL's Arpeggiator in the channel settings and its Chord
Progression tool, which in FL 2026 also writes basslines; Ableton's Arpeggiator, Chord,
Scale and the Live 12 MIDI generators; Logic's Arpeggiator and Chord Trigger; Bitwig's Note
FX). The 2025–26 market moved them **into instruments**: Kontakt 8.6 added Arpeggiator, Chord
Builder, Humanizer and Scale Lock Tools (Aug 2025); Serum 2 has an arpeggiator and a clip
sequencer with 12 slots each; V Collection 11's arpeggiator gained Chance and Cycles lanes;
Meld's oscillators follow the project scale. Complaints: Kontakt's Tool previews play a
piano rather than the loaded instrument (MusicTech).

**Technique.** No DSP: event processing. An arp is a note stack with modes (up, down,
up-down, as played, random, chord), rate in note values with swing, gate, octave range,
a step lane (velocity, ties, rests, probability, ratchets), latch; a chord unit maps one key
to a voicing (inversions, spread, strum time); a scale unit snaps or filters notes to the
project key. Probability and humanise draw from the note-id hash, never from a running
counter (the Pulsus II lesson).

**Fit.** Needs the instrument contract to carry **transport and song position** (STATUS
§"what round 17 left"). Events are generated at exact sample offsets from the song position,
so block-size independence is automatic if the step clock is computed from the absolute
tick, not accumulated per block. Null: a disabled or 1-step "as played" arp passes notes
unchanged, to the tick. "Bounce equals playback" and "render from bar 5 equals bars 5… of a
full render" are the tests. Recording the arp's output to a clip is a command.

**Cost.** Nothing on the audio thread worth measuring. **Build: S** (the contract change is
the only shared work; then three small units and their kit editors).

**Risks.** None found (event processing; long prior art). Trademark: avoid product names
(Cthulhu, Scaler) in UI text.

**Done when**: sample-accurate at any tempo, swing and block size (tested at 1, 37, 64 and
512 frames); bounce = playback; render from any bar equals the same bars of a full render;
works in front of every native instrument **and** a hosted CLAP/VST3 instrument; chords and
scale follow the project key; arp output recordable to the piano roll in one step.

### 6.3 Drum sampler with per-pad chains — finish Pulsus (Pulsus III)

**The bar.** FL's FPC, Ableton's Drum Rack (any instrument or sample per pad, per-pad
chains, sends, chokes), Logic's Drum Machine Designer, Battery 4 (deep cells, but a 2013 UI
and no smart browsing), XO (the similarity map, but **no layering or round robin**).
Repo research (drums.md): per-pad multi-out is universal and always a chore in a host.

**What already exists** (§2): Pulsus II has 16 layers per pad, round robin seeded from the
note's ordinal, chokes, modal and synthesized voices, filters, drive, the hi-hat pedal, and
the output hook. **The first pass listed this as a new instrument; it is a completion.**

**What is missing.** (1) **Per-pad outputs wired** (`DrumPad::output` → strip; the DSP side
is done, the track loop is not); (2) **a per-pad insert chain** (any Cantus effect, plus
hosted ones), with the same exact-rest rules as strips; (3) **any instrument on a pad**
(Drum Rack's idea: a Modulus or Unda voice as a pad); (4) **kit import** from a folder (name
parsing: kick/snare/hat…, then similarity) and from SFZ/Decent Sampler drum maps; (5) the
**similarity map** shared with the browser (XO's best idea; MFCC + a 2-D embedding computed
offline and cached).

**Fit and cost.** Per-pad chains multiply effect instances: a 16-pad kit with two inserts per
pad is 32 effect nodes. They are ordinary graph nodes and parallelise on the pool (ADR 0031);
the routing null test (the buses summed equal the kit) must hold to the bit by summing in
pad order. **Build: S–M.**

**Risks.** None in DSP. Content: CC0 kits only by default (ADR 0030); 808/909-style
one-shots must be **rendered from Pulsus and released CC0** (no clean CC0 drum-machine
source exists; repo research samples.md §2.2).

**Done when**: 16 pads each with its own chain, split to 16 mixer strips with one click and
back; the split kit summed equals the unsplit kit to the bit; a folder of one-shots becomes a
mapped kit in one drop; an Unda patch plays as a pad; chains at rest cost nothing (exact rest).

### 6.4 Modelled grand piano — *Chorda*

**The bar.** Pianoteq 9 (20 Oct 2025; ~50 MB; SOS Apr 2026 "still one of the best options";
complaints: "fleeting unnaturalness", stiff bass on the Steinway D, a long-running "metallic"
top end), Keyscape (77 GB, sampled), Logic Studio Piano (sampled, three mic pairs), Garritan
CFX and Noire. FL ships only FL Keys (a 3 MB sampler with no pedal model; repo research), so
**beating FL's stock piano is easy and beating Logic's and Pianoteq is the real bar**. Cantus
today has *no* acoustic piano engine: only CC0 Osiris Piano as a download (two velocity
layers, a character piano) — and Salamander (the standard free grand) is CC-BY and waits on
the owner's ruling.

**The plan, as the owner decided it (2026-10-10):**

1. **Now — build on the published path**, which needs nothing from Modartt: digital
   waveguide strings (J. O. Smith, *PASP*; Bensa, Bilbao, Kronland-Martinet & Smith 2003),
   dispersion allpasses for inharmonicity (Van Duyne & Smith 1994; Rauhala & Välimäki 2006;
   order ≤ 20 per string per Rocchesso & Scalcon via Bank et al. 2003), nonlinear hammers
   (Chaigne & Askenfelt 1994; Stulov 1995 hysteresis) — **Lamina's hammer is the base** —,
   double and triple unison strings with two-stage decay and beating (Weinreich 1977),
   longitudinal "phantom" partials (Bank & Sujbert 2005), a multirate soundboard (Bank et al.
   2003: ~100 operations per sample) and a sustain-pedal resonance model (Zambon, Lehtonen &
   Bank, DAFx-08: 1,000–1,500 resonators). Coefficients come **from closed-form physics and
   published measurements**, not from offline finite-element eigen-analysis of a coupled
   string–soundboard with real-time interpolation of the resulting coefficients.
   A modal string bank (Bank, Zambon & Fontana, IEEE TASLP 2010, "A modal-based real-time
   piano synthesizer") is the variant that splits best across threads; whichever is chosen,
   its coefficients must come from closed-form string physics.
2. **Later — evaluate Modartt's approach** (FE-derived modal coefficients interpolated in
   real time) once its patent has expired everywhere Cantus ships, *if* measurements show it
   beats ours. The engine stores a `model_version` so a second model can be added without
   changing any existing project's sound (the Pulsus II pattern; also the lesson of
   Pianoteq's v7/v8 quarrel, keys.md §6).
3. **Before release** — a legal review of our method against the claims of every relevant
   patent found (below). This report is not legal advice.

**The patents, verified on Google Patents (status is Google's assumption, not a legal
conclusion):**

- **US 7,915,515 B2** (Modartt / Philippe Guillaume), "Device for producing signals
  representative of sounds of a keyboard and stringed instrument": priority FR 2006-07-28,
  PCT filed 2007-07-13; **active**, all maintenance fees paid (years 4, 8 and 12, the last
  on 2022-09-21); **adjusted expiry 2027-10-20** (20 years from the PCT filing plus ~99 days
  of term adjustment). **Claim 1** (paraphrased from the granted text): a device with a
  real-time sound-production module and a **presynthesis module that generates timbre
  coefficients (a damping and a frequency per partial) from physical parameters including a
  soundboard parameter and a string parameter**; the real-time module builds the sound from
  partials using those coefficients. Claim 24 is the same on a recording medium; claims 2–23
  depend on claim 1. The description computes the coupled modes by finite elements and
  interpolation. **The family outside the US has lapsed**: EP 2047455 B1 "ceased", FR
  2904462 B1, JP 5261385 B2 and CN 101473368 B expired for non-payment, CA 2651981 abandoned.
  *Reading for the legal review:* claim 1 is broader than "finite elements" — any
  presynthesis that turns soundboard and string parameters into per-partial damping and
  frequency for an additive real-time stage may read on it. A **waveguide** string (where
  the partials are not synthesized individually) is the clearest distance from it until
  2027-10-20; a **modal** string bank whose per-partial coefficients are computed from a
  soundboard parameter is the variant to show the lawyer first.
- **Active digital-piano resonance patents** that a sympathetic/damper model must be checked
  against: Casio **US 8,178,772** (to ~2029-05-25: resonators, each a delay + low-pass in a
  feedback loop, **tuned to delay values that do not match the key pitches**, summed with the
  tone); Kawai **US 9,330,648** (to ~2035-01-30: stored high-register resonance plus a
  generated mid/low-register resonance from damper-pedal and key data, in a sample player);
  Yamaha **US 10,818,278** (to ~2038-01-12: a second delay loop at a pitch that is *neither*
  a speaking-length resonance nor its harmonic — duplex-scale resonance); Yamaha
  **US 8,729,376** (to ~2032: a delay loop excited by a band-split sampled waveform — commuted
  style); Casio **US 10,109,268** (to ~2037), **US 11,881,196** (to ~2042), Yamaha
  **US 12,039,960** (to ~2041, damper-pedal noise envelope). Several are sample-player
  specific; a full string model coupled at a bridge is a different structure, but **duplex
  resonance by a non-speaking-pitch loop** (Yamaha '278) and **detuned delay-loop
  resonators** (Casio '772) are exactly the kind of shortcut to avoid or clear. Expired and
  free to use: Karplus–Strong (US 4,649,783, 2004), Smith's waveguide patents (US 4,984,276,
  5,448,010), Yamaha US 5,212,334, the commuted-synthesis patents (US 5,500,486; 5,777,255),
  Staccato US 6,284,965, Viscount/Physis US 9,293,126 (lapsed 2020), Kawai US 7,476,797.
  Roland's modelling patents were not searched (budget; §10).

**Fit.** Each string group is a voice (pool-splittable). **Sympathetic resonance and the
soundboard couple voices**, so they live in a post-voice stage: every voice writes its bridge
force into a lane, the lanes are summed in voice order (ADR 0038), and the shared resonator
bank and soundboard run once on the sum. The hammer–string loop is delay-free and nonlinear:
solve it with a **fixed number of iterations** (no data-dependent exit) using
`cantus_dsp::math::pow`/`exp`; Bank's formulation stays stable to ~10× the velocity where the
naive method fails. Soundboard convolution must use a partition grid anchored to the stream,
not the host block. No task skipping under load (Pianoteq's "CPU overload detection" makes
live differ from render; Cantus shows a budget badge instead).

**Cost.** Waveguide string ~10–40 operations per sample plus the dispersion cascade
(≤ 20 allpasses ≈ 60 ops); three strings per note in the treble; soundboard ~100 ops/sample
shared; pedal-down resonator bank 1,000–1,500 × 4 MACs shared (multirate cuts it). Estimate:
a 64-voice pedal-down passage is a few percent of a modern core, split across the pool.
Pianoteq reaches 48 voices at ~30 % of a Raspberry Pi 4 (forum reports, keys.md).
**Build: L** (two rounds: Chorda I — strings, hammer, dampers, half-pedal, soundboard, one
perspective; Chorda II — mics, lid, una corda depth, per-key voicing curves).

**Done when**: partial frequencies within 1 cent of the target stretch curve on every key;
per-partial T60 within the perceptual tolerance of −25 %/+40 % against published
measurements (Tolonen & Järveläinen via Bank et al. 2003); half-pedal and re-pedalling
behave continuously (no switch between sample sets); a blind ABX/preference test (≥ 20
listeners, 10 excerpts: classical, pop ballad, lo-fi chords) against Pianoteq 9 and a
sampled grand shows **no significant preference against Chorda**; 64-voice pedal-down
passages inside 25 % of one core at 256 frames; null at defaults, block-size independence
and offline = real time; the legal review above is done.

### 6.5 Spectral and granular sampler — Exemplar's *Spectral* and *Grains* modes (with Grana grown)

**The bar.** **Bitwig 6.1's Sampler** (Aug 2026) is now the stock bar: a Spectral mode
(time-stretch with Pure/Reharm characters, harmonic bend, quantise-to-key, preserve onsets,
formants) and Fragments (up to 256 grains per voice, triggerable by audio), slicing in every
mode, automatic pitch and tempo detection. Ableton's Granulator III (Classic, Loop, Cloud;
grains 2 ms–2 s; **Capture** audio straight in; full MPE), Logic's Sample Alchemy and 12.3's
granular sync with formants, Serum 2's Granular and Spectral oscillators, Pigments' Granular,
FL's Fruity Granulizer (old). Complaints in the category: CPU (Serum 2), unitless controls
(Omnisphere), and nobody documents determinism (repo research).

**Why one item, inside Exemplar.** The first pass listed a separate "granular synth". But
Unda already has *Grana* (a grain engine with the right determinism), and granular and
spectral playback are used on **recorded samples and loops** — Exemplar's material. Bitwig
and Logic made the same choice (modes of the sampler). So: Exemplar gains **Grains** (Grana's
scheduler, grown: mip-mapped source so transposing up does not alias, sync/count density
modes, a loop mode, freeze/scan, 128-grain pool, capture from any track) and **Spectral**
(phase-locked vocoder stretch and pitch with formant preservation). The spectral engine is
the same one §3 *Time* (warp) and §3c (pitch editor) need, so it is built once.

**Technique.** Granular: Gabor 1947, Truax 1988 (real-time), Roads 2001; synchronous /
quasi-synchronous / asynchronous scheduling; coverage `e^(−λL)` decides cloudy vs continuous
(repo research §4.9). Spectral: phase vocoder with phase locking to peaks (Laroche & Dolson
1999); transient preservation; true-envelope formant preservation (Röbel & Rodet 2005).
Reference code: **Signalsmith Stretch (MIT)** — portable into `cantus-dsp`; Mutable
Instruments Clouds (MIT: granular, WSOLA, looping delay, spectral modes). Rubber Band is
GPL-2+ (only in GPL crates, or bought).

**Fit.** Grain onsets in absolute samples from the note start, every draw a counter hash —
Grana already proves bit-identical clouds at any block size. Spectral: the STFT hop grid is
anchored to the **note start**, not the block, so block size cannot move a frame; the
latency (Signalsmith reports input and output halves) is reported to delay compensation, or
the stretched sample is **pre-rendered on a worker and cached per (zone, ratio)** — the
deterministic choice for long stretches, and the one the agent research recommends.

**Cost.** Grains: ~15 flops per active grain-sample (repo estimate): 64 grains/voice ≈ 0.1–
0.3 % of a core per voice. Spectral real time: several FFTs per hop per voice — heavy; the
cache makes playback a plain read. **Build: M.**

**Risks.** No granular or spectral-stretch patent was searched successfully (§10). Prior art
is old (Gabor 1947, Truax 1988, phase vocoder 1966/1999). Use MIT/BSD references only in
`cantus-dsp`.

**Done when**: the same note renders bit-identical twice, at block sizes 1/37/64/512, and
offline = real time; a grain cloud transposed +24 st shows no alias component above −90 dB;
a 2× spectral stretch of a vocal keeps formants (spectral-envelope distance below a stated
threshold against the original) and transients (onset smearing ≤ 5 ms); capture from a track
in one click; 16 voices × 64 grains inside 10 % of one core at 256 frames.

### 6.6 Additive resynthesis — grow Unda's *Partes* to Harmor class

**The bar.** FL's **Harmor** (up to 516 partials per note per unison voice; resynthesis of
audio *and images*; editing planes per partial), Logic's **Alchemy** (additive, spectral and
granular per source; partial tracker), Zebra 3 (up to 1,024 partials, inharmonic), Pigments'
Harmonic engine, Reason Grain's Spectral Grains, Serum 2's Spectral oscillator (real-time
harmonic resynthesis).

**Technique.** Sinusoidal modelling (McAulay & Quatieri 1986) and SMS (Serra & Smith 1990:
partials + stochastic residual in ERB/Bark bands); reassigned bandwidth-enhanced partials
(Fitz & Haken 2002 — Loris is GPL, read but not port into `cantus-dsp`); harmonic assignment
from a pitch track (YIN, de Cheveigné & Kawahara 2002); transients kept from the original
attack (Verma & Meng 2000) or crossfaded. Synthesis: Partes's rotator bank (exact, per-sample
modulation, no alias by construction) with a harmonic "table" fast path; inverse-FFT
synthesis (Rodet & Depalle 1992) only past ~100 partials or for the residual.

**Fit.** Analysis offline on a worker; its **result** (per-hop, per-partial amplitude and
frequency deviation + a noise envelope) is stored as an asset beside the sample and read by
the voice — so render is bit-exact and independent of analyser changes. Rotators are
renormalised on a fixed per-note grid (Partes does this). Cost follows the note, not the
knob (partials ≤ Nyquist: 872 at A0, 54 at A4 at 48 kHz).

**Cost.** ~4–8 flops per partial-sample; 512 partials ≈ 1–2 % of a core per voice in the
bass, a fraction above (repo estimate). **Build: M.**

**Risks.** Apple/Camel Alchemy patents were not searched (§10). Celemony's US 8,022,286
(polyphonic note-object editing, to ~2029-06-15) concerns editing notes in polyphonic
*recordings* — relevant to §3c's polyphonic editor, not to resynthesis as an oscillator, but
the legal review should read it before §3c goes polyphonic.

**Done when**: a resynthesised sustained instrument (cello, voice, piano) has a log-spectral
distance below a stated threshold against the source and passes an ABX at 10 listeners as
"same instrument"; image-to-spectrum import works; spectral morph between two analyses;
no partial ever exceeds 0.45·fs (asserted); null at defaults.

### 6.7 Physical-model strings, mallets and resonators — *Collision/Tension/Sculpture* class

**The bar.** Ableton Collision, Tension and Electric (in Live **Standard** since Live 12),
Logic's Sculpture, Pigments 6's Modal engine, Zebra 3's modal oscillator, AAS Chromaphone 3
(coupled resonators: beam, marimba, tube, string, membrane, plate) and Strum GS-2, Arturia
Augmented Mallets (V Collection 11), the free RipplerX (GPL). FL ships only Plucked! and
Sakura. Physical modelling is what separates the top three DAWs from the rest (repo research).

**Why cheap now.** Gravis already has the hard part of a plucked string (bidirectional
waveguide, fitted dispersion, fractional tuning, pluck/pickup geometry, collisions), and
Pulsus II already has a **modal resonator voice with five materials**. One exciter →
resonator instrument can reuse both: exciters (pluck, pick, bow, hammer/mallet, noise, a
sample, live audio), resonators (waveguide string, modal bar/beam/plate/membrane/tube), a
body (modal or short convolution), coupling between two resonators.

**Technique.** Karplus & Strong 1983; Jaffe & Smith 1983; Smith's *PASP*; Karjalainen,
Välimäki & Tolonen 1998; Välimäki et al. 2006 (review); bowed string: McIntyre, Schumacher &
Woodhouse 1983 (friction curve; solve with a fixed-iteration or table method); modal
synthesis: Adrien 1991, MOSAIC/Modalys; bars: Chaigne & Doutaut 1997; fractional delay:
Laakso et al. 1996 (Thiran for fixed pitch, Lagrange while bending — Gravis already reads with
third-order Lagrange). Open references: STK (MIT-style), faust `physmodels.lib` (LGPL with an
exception — read, don't link into `cantus-dsp`).

**Cost.** A waveguide ~10 ops/sample + filters; 4–32 modal biquads per mallet voice. Trivial
per voice; pool-splittable. **Build: M.**

**Risks.** The waveguide and commuted-synthesis patents are expired (Stanford/Yamaha list in
§8). Avoid the Casio delay-loop resonator and Yamaha duplex claims in any *sympathetic* add-on
(see Chorda). Bowed strings are the hard sound (attack noise, bow-pressure realism).

**Done when**: nylon guitar, harp, marimba, vibraphone, kalimba, steel pan, bowed cello and
glass presets each pass a 10-listener "convincing in solo" panel; pitch within 0.1 cent at any
note and bend; renders repeat to the bit; MPE pressure/slide drive bow pressure and position.

### 6.8 Orchestral instrument — section players on Exemplar with CC0 content

**The bar.** **BBC SO Discover is free** (34 instruments, 47 techniques, 240 MB, one mix);
Splice INSTRUMENT (ex-LABS, free tier) wins on simplicity — 2–5 controls per preset; Logic
Studio Strings/Horns; Kontakt libraries. Complaints: LABS's few controls; Discover's single
mix.

**Content, verified.** **VSCO 2 Community Edition** (CC0, ~3 GB: strings, brass, woodwinds,
keys, percussion; SFZ) and **VCSL** (CC0, ~5 GB, 4,000+ samples, curated for light libraries;
**no choir**). **Sonatina Symphonic Orchestra is excluded**: CC Sampling Plus 1.0, retired by
Creative Commons on 2011-09-12, whole-work copying non-commercial only, and its author says
the origin of some old soundfonts is unknown. **Philharmonia samples are excluded** for
redistribution ("must not be sold", not to be made available as samples or an instrument);
University of Iowa MIS says "any projects, without restrictions" (usable as *source
material* for instruments we build, with a courtesy credit). Karoryfer's String Cyborgs and
Bigcat cello are CC0.

**Technique.** Not an engine: an Exemplar front-end with **articulations** (sustain, legato,
staccato, pizzicato, tremolo, trills where recorded) by key-switch *and* by note property in
the piano roll; **CC1 dynamics crossfading velocity layers**, CC11 expression; **scripted
legato** (overlap detection with a timing window ~50 ms, a short interval-dependent
crossfade and pitch glide — VSCO has no recorded transitions, so true legato is not possible
from it); release triggers; round robin; a section mixer (close/room if present). SFZ v1/v2
opcodes for all of this exist (`trigger=legato`, `sw_previous`, `xfin_*`, `seq_*`).

**Risk.** Honest expectation: CC0 chamber recordings ("natural and a little loose") **will
not beat BBC SO Discover in a blind test**. The goal is "sounds finished in a four-bar
sketch" with no account, no download gate and no licence friction — and a path to a
**Cantus-recorded** CC0 library later (§5). **Build: S–M** (front-end, articulation model,
presets) **plus content work**.

**Done when**: a four-bar string line with legato and a CC1 swell from the piano roll is
rated "finished" by 7 of 10 producers; strings, brass, woodwinds, harp and percussion each
have Play-page presets; articulations editable as note properties; every pack CC0 and
downloaded on demand; nothing in the repository.

### 6.9 Choir and singing — a vowel choir now, lyric singing gated on a voice we may ship

**The bar.** **Cubase 15's Omnivocal** (Nov 2025, open beta) is the first vocal synthesizer
inside a major DAW — Yamaha's engine, lyrics typed per note in the Key Editor, converted to
editable IPA; MusicTech found it "astonishing" for backing vocals but laborious for a lead;
**MusicRadar: as a beta it "cannot compete with commercial alternatives"**. The quality bar
is **Synthesizer V Studio 2 Pro** (Mar 2025, hybrid concatenative + neural, CPU-only, USD 99
plus USD 79 per voice) and VOCALOID 6; ACE Studio 2 and Emvoice render with credits or in
the cloud. Sampled choirs: EastWest Hollywood Choirs (WordBuilder), Voices of Rapture,
Arturia Augmented VOICES. FL has only the old Speech Synthesizer.

**The hard fact: no voice we may ship exists today.**
- Open singing corpora are mostly **non-commercial**: Opencpop (CC BY-NC-ND), M4Singer,
  CSD, OpenSinger (CC BY-NC-SA), Kiritan (research only), JVS-MuSiC audio (NC), JSUT-song.
- Usable: **VocalSet** (CC BY 4.0; 10.1 h, 20 singers, all five vowels, many techniques, no
  lyrics), the **Choral Singing Dataset** (UPF, CC BY 4.0, SATB), **Dagstuhl ChoirSet**
  (CC BY 4.0), PJS (Japanese, CC BY-SA 4.0). All need the owner's CC-BY ruling (ADR 0030).
- The de-facto open vocoder for DiffSinger/OpenUtau, **OpenVPI NSF-HiFiGAN, is CC BY-NC-SA
  4.0 — unusable**; its code is AGPL-3.0. No CC0 DiffSinger or OpenUtau voicebank was found.
- Code that is fine: **WORLD (Modified BSD; README: "no patent in all algorithms")**, Sinsy
  (Modified BSD), NNSVS, ENUNU, OpenUtau (MIT), DiffSinger (MIT / Apache-2.0), DDSP
  (Apache-2.0), HiFi-GAN and Vocos code (MIT; weights' licences unclear).

**Two units, in this order.**
1. **Vowel/formant choir** — no recorded data needed. FOF synthesis (Rodet 1984, CHANT) or a
   Klatt cascade/parallel formant synthesizer (Klatt 1980) with Peterson & Barney (1952)
   vowel formants; per-voice seeded jitter in pitch, timing and formants for an ensemble;
   vowel morphing (ah-eh-ee-oh-oo) on a macro and by note property; breath noise. FOF onsets
   come from a phase accumulator anchored to the note start: deterministic and block-size
   independent. Each choir voice is a pool voice. **Build: S–M.** Patents: none found on
   formant choirs (Klatt 1980 and CHANT 1984 are prior art); the classic Vocaloid patents are
   expired (US 7,016,841 to 2023, EP 1688911 to 2022, US 5,895,449 to 2017). **Avoid
   Yamaha US 7,613,612** (active to ~2027-12-27: applying one voice's spectral envelope to a
   recorded unison/ensemble spectrum to make a choir from one voice); summing independently
   synthesized voices is a different technique.
2. **Lyric singing** (later) — WORLD-style source–filter synthesis (Morise et al. 2016:
   "fast enough for real-time processing") driven by a phoneme timing and pitch model, with
   §3c's analysis shared; or a DDSP-style harmonic-plus-noise model. **Neural inference is
   only acceptable as our own single-threaded kernels with a fixed reduction order, or as an
   offline cached render** (PyTorch does not guarantee reproducibility across platforms;
   ONNX Runtime splits reductions by core count). Before training any neural vocoder fed a
   source spectrum and spectral envelope, read **Yamaha US 11,756,558** (active to
   ~2040-08-26). **Gate:** a voice database recorded by consenting singers for Cantus and
   released CC0 (or CC-BY if ruled in), never a person's voice without a written licence.

**Done when** (choir): a choir pad from vowels alone, with no recorded content, judged
"usable in a mix" by 7 of 10 producers against Arturia Augmented VOICES' synth layers; vowel
morph without clicks; bit-identical renders. (Singing, later): a typed English and Spanish
lyric on a piano-roll melody is intelligible (≥ 90 % word recognition by listeners) and in
tune (pitch error < 10 cents on sustained notes); every voice's licence and consent listed
in the notices.

### 6.10 Guitar (and bass) — extend Gravis's string model; a strummer in the piano roll

**The bar.** Ample Guitar/Bass 4.1 (Jun/Mar 2026: a strummer with 28 play styles per chord,
"Strum Note" = a chord as one editable object, tab view, **Guitar Pro import**), AAS Strum
GS-2 (fully modelled, guitarist voicings, strum keys), NI Session Guitarist (chords on the
keys become voicings and grooves — but **you cannot make your own patterns**), IK MODO BASS
2 (repo research). Logic Studio Bass shows a fretboard.

**Why cheaper than the first pass implied.** Gravis's String engine already models a
plucked string with frets, collisions, slides, pickups and a per-string voice mode. A guitar
adds six strings with a **chord-voicing and string-assignment** layer, an **acoustic body**
(modal bank or a short minimum-phase convolution, plus the bridge admittance coupling the
strings), **sympathetic coupling** (Karjalainen, Välimäki & Tolonen 1998's stable scheme),
pick direction, palm mute, harmonics, and a **strummer** that writes strums as note
properties in Cantus's own piano roll (strum time, direction, mute) — the host-native win:
no plug-in can put "Strum Note" into the DAW's editor.

**Content.** CC0 electric guitars exist (Karoryfer Shinyguitar, Emilyguitar, Black & Green
Guitars) for an Exemplar preset path; **no permissively licensed acoustic guitar** was found,
so the acoustic guitar must be the model.

**Cost.** Six waveguides + body ≈ trivial per voice. **Build: M.** **Risks:** the
physical-model patents in this space are expired (§8); acoustic-guitar realism (pick noise,
body) is the risk to measure.

**Done when**: a chord progression from the piano roll strums like a guitar (down/up, mutes,
timing spread) and follows the chord track; nylon and steel acoustic presets pass a
10-listener panel as "a guitar" in a mix; string/fret shown on a fretboard strip; Guitar Pro
or MusicXML tab import (later).

### 6.11 Acoustic drums — a multi-mic kit front-end on Pulsus with CC0 kits

**The bar.** Superior Drummer 3 (~230 GB, per-mic bleed, Tracker audio→MIDI, ~1,640
grooves), EZdrummer 3 (**Bandmate**, **Tap2Find**), Addictive Drums 2, Logic's Drummer /
Drum Kit Designer. Complaints: disk size and RAM, audible repetition in round robins.

**Content, verified.** **CC0**: Virtuosity Drums (Versilian + Karoryfer, **six mixable mic
positions**), Karoryfer Big Rusty / Unruly / Swirly Drums, Frankensnare, Gogodze Phu II
(seven mics). **CC BY 4.0** (needs the ruling): DrumGizmo CrocellKit (15 mics, 5.5 GB),
DRSKit (13), MuldjordKit (16 channels; asks for an album credit), Naked Drums (16 mics, 10
round robins). **Avoid**: DrumGizmo Aasimonster (its kick channel includes a commercial Alesis
DM5 module output); SM Drums (no licence on the authors' site); Hydrogen kits unless the kit's
own `drumkit.xml` licence is CC0/CC-BY.

**Technique.** Pulsus II's layers and round robin already exist; add **mic buses** (each
pad's samples carry one channel per mic; every mic is a bus with its own strip), **bleed**
(a hit on the snare also plays its overhead and room channels and, optionally, the kick mic's
bleed), CC4 hi-hat openness, layer limits and repetition smoothing (SD3's "smoothing"), and a
**groove browser of CC0 MIDI patterns** we write. **Build: M** (mostly routing and content).

**Done when**: rock, jazz and funk kits play a groove from MIDI with per-mic faders; a
repeated-snare roll shows no identical consecutive sample (asserted); a 10-listener panel
rates the kit "usable in a mix" against EZdrummer 3's stock kit; every kit CC0 (or CC-BY once
ruled) and downloaded on demand.

### 6.12 Smaller items, briefly

- **Chiptune / chip synth.** Plogue retired chipsounds in favour of the **chipsynth** line
  (C64, MD, OPMZ, OPS7, PortaFM, SFC), all CLAP. Technique: emulate chip state at clock time
  with integer state and insert band-limited steps at fractional positions (minBLEP, Brandt
  2001; polyBLEP, Välimäki & Huovilainen 2007) — deterministic by construction. reSID is
  GPL-2+ and Nuked-OPN2 LGPL-2.1: read, do not port into `cantus-dsp`; SameBoy is MIT.
  Loved, tiny CPU, **Build: S**. Avoid chip and console trade names in the UI ("2A03-style").
- **Playable loops** (Output Arcade class: loops across the keys, synced and tuned,
  black-key modifiers). It falls out of the slicer + Spectral mode; a Play page and a
  kit format. **Build: S** after 6.1 and 6.5.
- **Tape keys** (Mellotron-like) and **clavinet, harpsichord, celesta, toy piano**: Lamina
  and the 6.7 resonators cover most as models; Logic ships Vintage Clav and Mellotron.
- **Hybrid/texture synth** (Omnisphere, Zebra 3, Modartt's new Airteq): Unda + Exemplar
  Grains + the 6.7 resonators behind one Play page; after the engines exist.
- **Brass and woodwinds by model** (SWAM): the best-sounding needs continuous expression
  input (breath/MPE); a research item, not a near-term unit.
- **Players** (Logic's Session Players including the Synth Player in Logic 12, following
  Chord ID; FL 2026's chord detection and Gopher writing piano-roll content; Cubase's
  melodic pattern sequencer): §7 (AI) — the note effects of 6.2 are their foundation.

## 7. Content licences, verified (for on-demand packs; ADR 0030)

| Library | Licence (from its own page/repo) | Verdict for Cantus | Contents, size |
|---|---|---|---|
| **VSCO 2 Community Edition** | **CC0 1.0** | yes | orchestral (strings, brass, woodwinds, keys, percussion), ~3 GB, SFZ |
| **VCSL** | **CC0 1.0** ("no royalties, no credit") | yes | 4,000+ samples, ~5 GB; pianos, harpsichords, harps, organs, percussion, TX81Z; **no choir** |
| **Osiris Piano**, **Virtuosity Drums** (Versilian + Karoryfer) | **CC0 1.0** | yes | character upright/C2 piano; jazz kit with **six mic positions** |
| **Karoryfer** free libraries (Big Rusty, Unruly, Swirly Drums, Frankensnare, Gogodze Phu I/II, Shinyguitar, Emilyguitar, Black & Green Guitars, the basses, Bear Sax, Weresax, War Tuba, String Cyborgs, Bigcat cello…) | **CC0 1.0** on every repository checked | yes (The Hat With The Phat: repo CC0, shop page "GPL 3" — ask first, ADR 0030) | drums 0.9–2.3 GB each; guitars 0.1–0.5 GB |
| **Salamander Grand Piano V3** | **CC BY 3.0** (no CC0/PD relicensing found anywhere) | **only after the owner's CC-BY ruling**, with attribution to Alexander Holm | Yamaha C5, 16 layers, 707 MiB SFZ+FLAC |
| Salamander Drumkit | CC BY-SA 3.0 (share-alike only for modified samples) | only after a ruling on BY-SA | ~370 MB |
| **DrumGizmo CrocellKit / DRSKit / MuldjordKit**; **Naked Drums** | **CC BY 4.0** (+ DrumGizmo's credit request) | after the CC-BY ruling | 13–16 mic channels; 0.35–5.5 GB |
| DrumGizmo Aasimonster | CC BY 4.0, but its kick channel includes an **Alesis DM5 module** output | **no** | — |
| **VocalSet**, **Choral Singing Dataset (UPF)**, **Dagstuhl ChoirSet** | **CC BY 4.0** | after the CC-BY ruling (training/analysis data for a choir or voice) | 10.1 h of vowels and techniques; SATB recordings |
| PJS (Japanese singing) | CC BY-SA 4.0 | after a BY-SA ruling | — |
| **Sonatina Symphonic Orchestra** | **CC Sampling Plus 1.0 — retired by CC on 2011-09-12**; whole-work copying non-commercial only; author: origin of some old soundfonts unknown | **no** | — |
| **Philharmonia samples** | custom: "must not be sold", not to be made available as samples or an instrument | **no** (for redistribution) | — |
| University of Iowa MIS | "may be downloaded and used for any projects, without restrictions" | as source material for our own instruments (courtesy credit) | raw single notes |
| Freesound | per sound: CC0, CC BY 4.0, CC BY-NC 4.0, legacy Sampling+ | CC0 only today (vet each file: rips are posted as "CC0") | — |
| Hydrogen kits | mixed per kit (GPL-2, CC BY-SA, CC BY, CC0) | only kits whose `drumkit.xml` says CC0 (or CC-BY once ruled) | — |
| Pianobook, Decent Samples libraries, Spitfire/Splice, BBC SO Discover | use in music allowed, **redistribution forbidden** | **link only** | — |
| Opencpop, M4Singer, CSD, OpenSinger, Kiritan, JVS-MuSiC, JSUT-song | non-commercial / research / no redistribution | **no** | — |
| OpenVPI NSF-HiFiGAN weights | **CC BY-NC-SA 4.0** | **no** | — |

**What is missing entirely:** a permissively licensed **choir or vocal** sample library (none
in VCSL or VSCO; Dave Choir has no licence) and a permissive **acoustic guitar**. Both must
be modelled or recorded by Cantus.

## 8. Patents, verified on Google Patents

Google Patents labels every status "an assumption, not a legal conclusion"; this report is
not legal advice. Expiry dates are Google's adjusted estimates.

| Patent | Holder | What it covers (paraphrase) | Status, expiry | Relevance |
|---|---|---|---|---|
| **US 7,915,515 B2** | Modartt | claim 1: a presynthesis module generating per-partial damping and frequency from physical parameters incl. a soundboard and a string parameter, feeding a real-time partial-based synthesizer; description: FE analysis of coupled string–soundboard modes + interpolation | **active, fees paid through year 12; 2027-10-20**. **EP 2047455, FR, JP, CN members lapsed; CA abandoned** | Chorda: stay on waveguide/closed-form; legal review |
| **US 8,178,772 B2** | Casio | resonators (delay + low-pass loop) with delays **not** matching key pitches, added to the tone | active, ~2029-05-25 | Chorda/guitar sympathetic models |
| **US 8,729,376 B2** | Yamaha | delay loop excited by a band-split sampled waveform | active, ~2032-07-10 | avoid commuted-sample excitation shortcuts |
| **US 9,330,648 B2** | Kawai | stored high-register resonance + generated mid/low resonance from damper and key data (sample player) | active, ~2035-01-30 | damper resonance design |
| **US 10,109,268 B2** | Casio | fundamental and harmonics attenuated, then convolved with register impulse responses | active, ~2037-12-18 | resonance by IR convolution |
| **US 10,818,278 B2** | Yamaha | a second delay loop at a pitch that is neither a speaking-length resonance nor its harmonic (duplex resonance) | active, ~2038-01-12 | duplex scale modelling |
| **US 11,881,196 B2** | Casio | damper off: a struck key's excitation routed to low-register string channels | active, ~2042-04-08 | sympathetic routing |
| **US 12,039,960 B2** | Yamaha | damper-pedal sound with an envelope derived from added noise | active, ~2041-04-27 | pedal noise |
| US 4,649,783; US 4,984,276; US 5,212,334; US 5,448,010; US 5,500,486; US 5,777,255; US 6,284,965 | Stanford, Yamaha, Staccato | Karplus–Strong, digital waveguides, commuted synthesis, truncated recursive filters | **expired** (2004–2019) | free to use |
| US 9,293,126 B2 | Viscount (Physis) | stringed-instrument model | lapsed 2020-03-22 | free |
| US 7,476,797 B2 | Kawai | resonance generator | lapsed (fees) | free |
| US 7,016,841; EP 1688911; US 5,895,449; US 5,642,470 (Fujitsu chorus) | Yamaha, Fujitsu | concatenative singing; chorus from per-part synthesis | **expired** (2014–2023) | free |
| **US 7,613,612 B2** | Yamaha | a choir from one voice: its spectral envelope applied to a recorded unison spectrum | **active, ~2027-12-27** | choir: synthesize independent voices instead |
| **US 11,756,558 B2** | Yamaha | trained waveform model fed a source spectrum and spectral envelope | **active, ~2040-08-26** | any neural singing vocoder |
| US 8,338,687; US 11,295,723 | Yamaha | pitch-curve model; trained harmonic + non-harmonic models | lapsed (fees) | free |
| **US 8,022,286 B2** | Celemony | note-object processing of polyphonic recordings (Melodyne DNA) | active, ~2029-06-15 | §3c polyphonic editing, not instruments |

**Not searched (budget):** Roland's modelling patents; slicing/warp/onset patents
(Propellerhead ReCycle, Ableton warp); granular; Apple/Camel Alchemy resynthesis; NI loop
instruments; Dreamtonics. These belong in the pre-release legal review.

## 9. The priority, and why

Criteria, in order: (a) what an FL-style producer (beats, melody, vocals) opens every
session and Cantus lacks; (b) what a blind comparison against the leaders' stock
instruments leans on; (c) build cost on the engines Cantus already has (§2); (d) legal and
content risk.

| # | Unit | Build | Why here (evidence) |
|---|---|---|---|
| 1 | **Slicer** (Exemplar *Sectio*) | S | FL users slice daily and Image-Line shipped Fruity Slicer 2 in 2025; every top DAW and even Serum 2 slices; Cantus has nothing; zone playback, resampler and envelopes already exist in Exemplar |
| 2 | **Arpeggiator, chord and scale note effects** | S | in every DAW, and in 2025–26 moved into Kontakt (8.6) and Serum 2; small; the foundation of chord-following players (§7); needs only the transport/position contract change |
| 3 | **Pulsus III: per-pad outputs, per-pad chains, any instrument on a pad, kit import** | S–M | drums are the first thing a beat-maker judges; FL 2026 deepened FPC (layers), Cubase 14 and Bitwig 5.3 added drum machines; Pulsus II already has layers, round robin and the output hook — the first pass listed this as a new instrument, it is a completion |
| 4 | **Chorda, the modelled grand** (owner's plan: published path now, Modartt's method evaluated after 2027-10-20, legal review before release) | L | the piano is in every song template and is what a blind test against Logic (Studio Piano) and Pianoteq hits first; Cantus has no piano engine; Lamina's hammer and Gravis's dispersion fitting are the base. Not higher only because of its cost and the review. **Meanwhile**: the owner's CC-BY ruling would let Salamander Grand (CC BY 3.0) download into Exemplar at once |
| 5 | **Exemplar Spectral and Grains modes** (Grana grown) | M | Bitwig 6.1 made spectral stretch + 256-grain granular the stock-sampler bar; Logic 12.3 and Serum 2 followed the same direction; the spectral engine is shared with warp (§3 *Time*) and the pitch editor (§3c); replaces the first pass's separate "granular synth" because Unda's *Grana* exists |
| 6 | **Physical-model resonators** (exciter → string/bar/plate/membrane/tube, Collision/Tension/Sculpture class) | M | in Live **Standard** and Logic; Pigments 6 and Zebra 3 added modal engines; cheap because Gravis's waveguide and Pulsus's modal voice exist |
| 7 | **Guitar** (Gravis's string model × 6, acoustic body, strummer as piano-roll note properties) | M | no permissive acoustic-guitar samples exist, so it must be a model; Ample 4.1 and Strum GS-2 set the strummer bar; host-native strums beat Session Guitarist's fixed patterns |
| 8 | **Additive resynthesis** (Unda *Partes* → Harmor class) | M | FL's Harmor and Logic's Alchemy are their makers' flagships, but the daily-use part (stretch, reharmonise) arrives with #5; Partes already synthesises |
| 9 | **Orchestral section players** (Exemplar front-end, VSCO 2 CE / VCSL CC0) | S–M + content | strings and horns are everywhere in trap and film; BBC SO Discover is free and better recorded, so the aim is "finished sketch, zero friction", then our own recordings |
| 10 | **Vowel choir** (FOF/formant, no recorded data); **lyric singing later**, gated on a CC0 voice recorded for Cantus | S–M (choir) | Cubase's Omnivocal shows the direction but MusicRadar finds the beta uncompetitive; open singing voices and the standard open vocoder are non-commercial (§7), so lyric singing cannot ship today without our own voice |
| 11 | **Acoustic drums** (mic buses and bleed on Pulsus, CC0 kits, CC0 grooves) | M | Virtuosity Drums (CC0, six mics) and Karoryfer kits make it possible; Superior/EZdrummer set the bar on grooves and Bandmate-style suggestion (§7) |

**Against the first pass** (its order: slicer, additive, granular, arpeggiator, piano,
vocal/choir, orchestral, drum sampler, physical-model strings, acoustic drums, guitar):

- **Additive and granular were not "new instruments"**: Unda already has *Partes* and
  *Grana*. Granular folds into Exemplar's new modes with spectral stretch (#5), and additive
  becomes "resynthesis for Partes" (#8).
- **The drum sampler moved from 8 to 3**: most of it exists; what remains is cheap and is a
  beat-maker's first judgement.
- **Guitar moved from 11 to 7**: Gravis's string engine makes it a medium job, and it is the
  only way to an acoustic guitar.
- **Vocal synthesis moved from 6 to 10 and split in two**: the choir is feasible now; lyric
  singing has no shippable voice data or vocoder (verified licences) and is gated on
  recording our own.
- **Piano moved from 5 to 4**, with the owner's three-step plan and the active resonance
  patents now listed.
- Corrected facts: Pianoteq 9 was released 20 Oct 2025 (the first pass gave no date);
  Pigments is at 7 (Dec 2025); Reason is at 14 (May 2026), not 13; Cubase 15 ships
  Omnivocal; Bitwig 6.1's Sampler and Logic 12's Synth Player are the 2026 bar; Spitfire
  LABS became Splice INSTRUMENT (Oct 2025); Plogue's chipsounds is legacy, replaced by the
  chipsynth line; Sonatina is not just "unclear" but under a retired NC-like licence; the
  Pianoteq patent is US-only now (the EP/FR/JP/CN family lapsed).

## 10. What could not be verified

The shared web-search budget (200 searches per turn across all research agents) ran out
part-way; after that only known URLs were fetched. Open items:

- **Patents not searched**: Roland V-Piano/SuperNATURAL modelling; slicing, warp and onset
  detection (Propellerhead ReCycle, Ableton warp); granular synthesis; Apple/Camel Alchemy
  resynthesis; NI loop instruments; Dreamtonics. **Claim text** was read only for
  US 7,915,515 (claim 1 and 24 paraphrased); the resonance patents' claims are paraphrased
  from their summaries, and their claim 1 may be narrower or broader. Whether US 7,915,515
  carries a terminal disclaimer: not seen.
- **DAWs**: Fender Studio Pro 8.1's contents and date; whether Retrologue and Padshop still
  ship in Cubase 15; Logic's complete current instrument list from an Apple page (the Apple
  PDF returned 404; assembled from apple.com, Wikipedia and a snippet); whether Objekt and
  Algoritm are in perpetual Reason 14; exact dates of Logic 12.0–12.4, Live 12.0 and
  Bitwig 6.1 final (sources disagree by days to weeks).
- **Products**: Keyscape's current version and price; Phase Plant's current version;
  Kontakt 8.10/8.12 (snippets only); Vital 1.6.4's date and its current prices; Plogue
  prices and the chips inside legacy chipsounds; Addictive Drums 2 reviews; XO's current
  version; Pianoteq half-pedalling and Scala support on a primary page (snippets only);
  Synthesizer V's rap mode and ARA from Dreamtonics itself (403); the MusicRadar Cubase 15
  review body (only its verdict line and a search summary of the Omnivocal judgement).
- **Licences**: Neil Thapen's original Pink Trombone; NUS-48E; HiFi-GAN pretrained weights;
  Rex SDK terms; the Decent Samples EULA and Pianobook's official terms page (404; quoted
  second-hand); Karoryfer's shop-page licence statement; SM Drums; mic counts of Big Rusty,
  Unruly and Swirly Drums; Nes_Snd_Emu, Gb_Snd_Emu and C blip_buf licences.
- **Techniques**: the pitch-synchronous granular citation; Serafin's bowed-string thesis and
  Smith's bowed-string papers; Spitfire's legato method (VSL's from forum posts only);
  Chaigne & Doutaut's DOI; other open-source piano models.
- **Estimates, not measurements**: every CPU figure in §6 except those quoted from vendors or
  papers; they must be measured on Cantus's bench before any "done when" relies on them.

## 11. Sources

All accessed 2026-10-10 unless dated otherwise. The repository's earlier reports
(`research/instruments/*.md`, `research/content/*.md`) are cited where their facts were
re-used and not re-verified.

**DAW vendors and release notes**
- https://www.image-line.com/fl-studio/compare-editions ; https://forum.image-line.com/viewtopic.php?p=2014957 (FL 2025) ; https://forum.image-line.com/viewtopic.php?p=2065217 (FL 2026) ; https://forum.image-line.com/viewtopic.php?p=2076063 (2026.1.7) ; https://www.image-line.com/fl-studio-news/fl-studio-2025-2
- https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/Slicex.htm ; …/Fruity%20Slicer%202.htm ; https://www.image-line.com/support/flstudio_online_manual/html/plugins/Harmor.htm ; …/Sytrus.htm
- https://www.ableton.com/en/release-notes/live-12/ ; https://www.ableton.com/en/live/compare-editions/ ; https://www.ableton.com/en/manual/live-instrument-reference/ ; https://www.ableton.com/en/packs/meld/ ; https://roberthenke.com/technology/granulator3.html
- https://support.apple.com/en-us/126835 ; https://support.apple.com/en-us/109503 ; https://www.apple.com/logic-pro/ ; https://cdm.link/logic-pro-12-hands-on/ ; https://rekkerd.org/apple-releases-logic-pro-12/ ; https://www.soundonsound.com/techniques/logic-pro-synth-players ; https://www.soundonsound.com/techniques/logic-pro-studio-piano
- https://www.bitwig.com/whats-new/ ; https://downloads.bitwig.com/6.1/Release-Notes-6.1.html ; https://www.bitwig.com/userguide/latest/synth/ ; https://www.soundonsound.com/news/bitwig-studio-6-launches
- https://www.reasonstudios.com/shop/product/reason-rack-14/ ; https://www.reasonstudios.com/press/reason-14-available-worldwide ; https://www.reasonstudios.com/devices/grain
- https://www.soundonsound.com/reviews/fender-studio-pro-8 ; https://en.wikipedia.org/wiki/Studio_One_(software)
- https://www.soundonsound.com/reviews/steinberg-cubase-pro-14 ; https://www.soundonsound.com/reviews/steinberg-cubase-pro-15 ; https://musictech.com/features/opinion-analysis/cubase-15-ai-tools/ (11 Dec 2025) ; https://www.musicradar.com/music-tech/daws/steinberg-cubase-15-review ; https://www.recordingmag.com/news/cubase-15-a-new-reference-standard-for-music-production/ ; https://create.routenote.com/blog/cubase-15-has-just-landed-so-can-this-daw-still-compete/

**Instrument vendors and reviews**
- Serum 2: https://xferrecords.com/products/serum-2 ; https://www.kvraudio.com/news/xfer-records-releases-serum-2---free-update-63286 ; https://musictech.com/reviews/software-instruments/xfer-records-serum-2-review ; https://www.attackmagazine.com/news/serum-2-is-finally-here-and-its-free-for-previous-owners/
- Vital: https://vital.audio ; https://github.com/mtytel/vital ; https://vital.audio/eula ; https://www.kvraudio.com/product/20186
- Pigments: https://www.arturia.com/products/software-instruments/pigments/overview ; https://www.attackmagazine.com/reviews/gear-software/arturia-pigments-7-the-best-gets-even-better/ ; https://www.sonicstate.com/news/2025/12/16/arturia-pigments-7-out-today-free-upgrade
- Omnisphere 3: https://www.spectrasonics.net/products/omnisphere/ ; https://musictech.com/reviews/plug-ins/spectrasonics-omnisphere-3-review/ ; https://www.sonicstate.com/news/2025/10/16/spectrasonics-ominisphere-30-is-out/
- u-he: https://u-he.com/products/diva/ ; https://u-he.com/products/zebra3/ ; https://synthanatomy.com/2026/07/u-he-zebra-3-modular-synthesizer-plugin.html
- Phase Plant: https://kilohearts.com/products/phase_plant ; Massive X: https://www.native-instruments.com/en/products/komplete/synths/massive-x/ ; https://synthanatomy.com/2026/03/native-instruments-massive-x-out-now.html
- Kontakt 8: https://docs.native-instruments.com/online-guides/kontakt-manual/en/new-in-kontakt-8 ; https://musictech.com/reviews/software-instruments/native-instruments-kontakt-8-review
- V Collection 11: https://www.soundonsound.com/news/arturia-v-collection-11-released ; Arcade: https://support.output.com/en/articles/16102145-arcade-release-notes ; https://bonedo.de/artikel/output-arcade-20-test/
- Plogue: https://www.plogue.com/products/ (and each chipsynth page)
- Alchemy: https://help.apple.com/logicpro-instruments/mac/10.2.1/en.lproj/lgsi2618652a.html
- Pianoteq: https://www.modartt.com/news ; https://www.modartt.com/pianoteq_whats_new ; https://www.modartt.com/pianoteq_features ; https://www.soundonsound.com/reviews/modartt-pianoteq-9 ; https://forum.modartt.com/viewtopic.php?id=12646 ; https://forum.modartt.com/viewtopic.php?id=11513
- Keyscape: https://musictech.com/reviews/spectrasonics-keyscape-review ; Noire: https://www.native-instruments.com (keys/noire) ; Garritan CFX: https://ask.audio/articles/review-garritan-cfx-concert-grand
- AAS: https://www.applied-acoustics.com/chromaphone-3/ ; https://www.applied-acoustics.com/strum-gs-2/
- SWAM: https://kb.audiomodeling.com (Solo Strings release notes) ; https://www.soundonsound.com/reviews/audio-modeling-swam-solo-instruments
- Ample Sound: https://www.amplesound.net/en/index.asp ; NI Session Guitarist: https://www.soundonsound.com/reviews/ni-session-guitarist-picked-acoustic
- Spitfire/Splice: https://www.kvraudio.com/news-print.php?id=64929 ; https://www.spitfireaudio.com/bbc-symphony-orchestra-discover
- Drums: https://www.native-instruments.com/en/products/komplete/drums/battery-4/ ; https://www.xlnaudio.com/products/xo ; https://www.toontrack.com/product/superior-drummer-3/ ; https://www.toontrack.com/product/ezdrummer-3/ ; https://www.xlnaudio.com/products/addictive_drums_2
- Vocal: https://www.kvraudio.com/news/dreamtonics-releases-synthesizer-v-studio-2-pro-63333 ; https://www.soundonsound.com/news/dreamtonics-synthesizer-v-gains-pro-tools-ara2-support ; https://www.vocaloid.com/en/vocaloid6/ ; https://musictech.com/reviews/digital-audio-workstations/ace-studio-2-review/ ; https://mixdownmag.com.au/reviews/review-emvoice-one-vocal-synthesis/ ; https://www.arturia.com/products/software-instruments/augmented-voices

**Licences and datasets**
- https://versilian-studios.com/vsco-community/ ; https://github.com/sgossner/VSCO-2-CE ; https://versilian-studios.com/vcsl/ ; https://github.com/sgossner/VCSL ; https://github.com/orgs/sfzinstruments/repositories ; https://github.com/peastman/sso ; https://creativecommons.org/retiredlicenses/ ; https://archive.org/details/SalamanderGrandPianoV3 ; https://github.com/endolith/Salamander-Drumkit ; https://philharmonia.co.uk/resources/sound-samples/ ; https://theremin.music.uiowa.edu/MIS.html ; https://freesound.org/help/faq/ ; https://drumgizmo.org/wiki/ (crocellkit, drskit, muldjordkit, the_aasimonster) ; https://github.com/sfzinstruments/WilkinsonAudio.NakedDrums
- https://zenodo.org/records/1193957 (VocalSet) ; https://zenodo.org/records/1286570 (Choral Singing Dataset) ; https://zenodo.org/records/4607766 (Dagstuhl ChoirSet) ; https://wenet-e2e.github.io/opencpop/liscense/ ; https://raw.githubusercontent.com/M4Singer/M4Singer/master/dataset_license.md ; https://zenodo.org/record/4785016 (CSD) ; https://github.com/openvpi/vocoders/releases ; https://github.com/mmorise/World ; https://github.com/stakira/OpenUtau ; https://github.com/openvpi/DiffSinger ; https://github.com/nnsvs/nnsvs
- Code: https://github.com/Signalsmith-Audio/signalsmith-stretch (MIT, via the research agent) ; https://breakfastquay.com/rubberband/license.html ; https://github.com/pichenettes/eurorack (Clouds, MIT) ; https://github.com/thestk/stk ; https://github.com/libsidplayfp/resid ; https://github.com/nukeykt/Nuked-OPN2

**Patents** — https://patents.google.com/patent/ + US7915515B2, EP2047455B1, US8178772B2, US8729376B2, US9330648B2, US10109268B2, US10818278B2, US11881196B2, US12039960B2, US4649783A, US4984276A, US5212334A, US5448010A, US5500486A, US5777255A, US6284965B1, US9293126B2, US7016841B2, EP1688911B1, US5895449A, US5642470A, US7613612B2, US11756558B2, US8338687B2, US11295723B2, US8022286B2

**Papers** (DOIs checked against Crossref where marked in the agent notes)
- Onsets and tempo: Bello et al., IEEE TSAP 13(5) 2005, doi:10.1109/TSA.2005.851998 · Dixon, DAFx-06 · Böck & Widmer, DAFx-13 (SuperFlux) · Ellis, JNMR 36(1) 2007, doi:10.1080/09298210701653344
- Time-scale: Verhelst & Roelands, ICASSP-93, doi:10.1109/ICASSP.1993.319366 · Laroche & Dolson, IEEE TSAP 7(3) 1999, doi:10.1109/89.759041
- Additive: McAulay & Quatieri, IEEE TASSP 34(4) 1986, doi:10.1109/TASSP.1986.1164910 · Serra & Smith, CMJ 14(4) 1990, doi:10.2307/3680788 · Fitz & Haken, JAES 50(11) 2002 · Rodet & Depalle, AES 93rd (1992), preprint 3393 · Röbel & Rodet, DAFx-05
- Granular: Gabor, Nature 159 (1947), doi:10.1038/159591a0 · Truax, CMJ 12(2) 1988, doi:10.2307/3679938 · Roads, *Microsound*, MIT Press 2001
- Piano: Bank et al., EURASIP JASP 2003(10), doi:10.1155/S1110865703304093 · Bensa et al., JASA 114(2) 2003, doi:10.1121/1.1587146 · Chabassier, Chaigne & Joly, JASA 134(1) 2013, doi:10.1121/1.4809649 · Stulov, JASA 97(4) 1995, doi:10.1121/1.411912 · Chaigne & Askenfelt, JASA 95(2) 1994, doi:10.1121/1.408459 · Van Duyne & Smith, ICMC 1994 · Rauhala & Välimäki, IEEE SPL 13(5) 2006, doi:10.1109/LSP.2006.870376 · Smith & Van Duyne, ICMC 1995 · Bank & Sujbert, JASA 117(4) 2005, doi:10.1121/1.1868212 · Bank, Zambon & Fontana, IEEE TASLP 18(4) 2010 · Weinreich, JASA 62 (1977) and Zambon, Lehtonen & Bank, DAFx-08 (cited from keys.md, not re-read)
- Strings and modal: Karplus & Strong, CMJ 7(2) 1983, doi:10.2307/3680062 · Jaffe & Smith, CMJ 7(2) 1983, doi:10.2307/3680063 · Smith, *Physical Audio Signal Processing* (2010, online) · Välimäki et al., Rep. Prog. Phys. 69(1) 2006, doi:10.1088/0034-4885/69/1/R01 · Karjalainen, Välimäki & Tolonen, CMJ 22(3) 1998, doi:10.2307/3681155 · McIntyre, Schumacher & Woodhouse, JASA 74(5) 1983, doi:10.1121/1.390157 · Adrien 1991 (in *Representations of Musical Signals*) · Bilbao, *Numerical Sound Synthesis*, Wiley 2009 · Laakso et al., IEEE SPM 13(1) 1996, doi:10.1109/79.482137
- Chips: Brandt, ICMC 2001 · Välimäki & Huovilainen, IEEE SPM 24(2) 2007, doi:10.1109/MSP.2007.323276
- Voice: Rodet, CMJ 8(3) 1984, doi:10.2307/3679809 · Klatt, JASA 67(3) 1980, doi:10.1121/1.383940 · Peterson & Barney, JASA 24(2) 1952, doi:10.1121/1.1906875 · Morise, Yokomori & Ozawa, IEICE E99-D(7) 2016 · Liu et al., AAAI 2022 (DiffSinger), arXiv:2105.02446 · Wang, Takaki & Yamagishi (NSF), arXiv:1904.12088 · Kong et al. (HiFi-GAN), arXiv:2010.05646 · Engel et al., ICLR 2020 (DDSP) · Zhang et al. (VISinger 2), arXiv:2211.02903
