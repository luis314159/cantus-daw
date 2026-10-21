# Instruments as a family: the stock bar, and the interface language of the best instruments

*Researched 2026-09-24 for Cantus. This is the instrument counterpart of
[`research/fabfilter/family.md`](../fabfilter/family.md), and it extends that report's house rules (§7) to
instruments. Sources are the vendors' own pages (product pages, edition-comparison pages, online manuals,
release notes), press reviews (Sound On Sound, MusicRadar, MusicTech, gearnews, Synth Anatomy) and, for
Vital and Surge XT, their public source code and manual. Everything is paraphrased. Opinions that reached
this report only as a search-engine excerpt, without the page being read, are marked **[excerpt]**. Colours
and contrast ratios were **measured from the official screenshots** with Pillow and the WCAG 2.x formula
(JPEG and video frames, so ±2 per channel). The screenshots are the vendors' images, kept in
`screenshots/` (git-ignored) for internal reference only.*

*Scope note: the other reports in this folder cover each category's engines in depth (wavetable, analog,
FM/additive/granular, sampler, drums, bass, keys) and the plug-in standards
([standards.md](standards.md)). This one covers what cuts across them: what DAWs ship, and how the best
instruments look and behave. Plug-in hosting and export belong to another session (ADRs 0019 and 0020,
crate `cantus-host`); §6.8 only notes what the instrument side needs from that contract.*

---

## 0. The short version

1. **The stock bar is high, and it is set by three DAWs.** Live 12 Suite (21 instruments), Logic Pro 12
   (about 28) and FL Studio 2026 All Plugins (39 generators) each ship at least a subtractive synth, a
   wavetable or hybrid synth, an FM synth, a multisampler, a quick sampler or slicer, a drum machine with
   drum synthesis, a piano, an electric piano and a granular or spectral engine. Bitwig 6 adds a modular
   environment and, since 6.1 (August 2026), a sampler with spectral stretching and granular playback.
   Cubase 15, Fender Studio Pro 8 and Reaper 7 are well below that bar. **To clear every DAW's bar, Cantus
   needs about eight engines, not two** (§7).
2. **Only a handful of stock instruments are regularly called good enough to skip a third-party purchase:**
   Logic's **Alchemy**, **Sampler** and **Studio Bass/Piano**; Live's **Operator**, **Wavetable**, **Meld**,
   **Drift** and **Drum Rack**; FL's **Sytrus**, **Harmor** and **FLEX**; Bitwig's **Polymer**, **The Grid**
   and the 6.1 **Sampler**. What they have in common is **depth behind a simple face** plus a **large,
   well-tagged preset library**, not any single DSP trick (§3).
3. **Top instruments share one layout grammar.** A header with the preset name, ‹ ›, a browser button, a
   favourite, undo/redo and a master level; sources in a row across the top half (oscillators → filters);
   **modulation sources in a strip across the bottom half**, each tile drawing its own shape live; **macros
   at the left or top**; and a **keyboard along the very bottom**. Serum 2, Vital, Pigments, Phase Plant and
   Omnisphere all follow it (fam-01,
   fam-02, fam-03,
   fam-09).
4. **Drag-to-modulate has won.** Vital, Serum, Pigments, Phase Plant, Bitwig and FabFilter all connect a
   source by dragging it onto a control. The best ones **light up every valid target during the drag**,
   **preview the result before you let go** (Vital), draw the **range as an arc on the knob** and show **how
   many targets each source drives** (Serum 2's count badges). Nobody except Bitwig and Surge XT also gives a
   complete **list of every connection**, and nobody shows the **per-voice** value on the knob in a way you
   can read (§4.4–4.5).
5. **The newest pattern is a "Play" page:** a big preset name, 4–8 macros, the keyboard and an animated
   visual, with the full editor one tab away (Pigments 7 Play view, FLEX's Play page, Phase Plant's macro
   row, Omnisphere's Orb). Most users never leave it, and it is the right default for Cantus too
   (fam-07).
6. **Legibility is poor across the category.** Measured knob bodies against their panel: FLEX **1.0:1**,
   Serum 2 macros **1.1:1**, Phase Plant **1.4–1.6:1**, Vital **1.7:1**, Bitwig **1.7:1**, Omnisphere 3
   **1.9:1**, Pigments dark **2.2:1** (WCAG 1.4.11 asks 3:1). Label text mostly passes (5–13:1), but dim
   secondary text often fails (Pigments idle modulator names 3.3:1, captions 2.8:1; Serum 2 "ARTIST" 3.8:1;
   Omnisphere's rating dots 1.8:1). **Pigments' light theme is the one clear pass (knob 5.5:1, labels
   8.6:1)** and the only first-class light theme found. Cantus's own knob fails the same test today (§4.14).
7. **Cantus's rules for instruments** (§6): the plug-in frame from `fabfilter/family.md` §7 plus three bands
   every instrument has: a **macro bar**, a **modulation strip** and a **live keyboard**. There are three
   views (Play, Edit, Matrix) and one modulation model shared by all instruments, with per-instance colour
   *and* tag, range in the target's units, and per-voice dots. Instruments live **in the inspector as a
   compact Play strip** and open a **full editor window from the channel rack**, the same window the effects
   use.
8. **Where a native instrument beats a plug-in:** the host knows the clip, the project key, the other tracks
   and the undo history. So Cantus can audition a preset with *your* part rather than a demo riff, put the
   project's scale on every keyboard, modulate from another track without routing, draw note expressions in
   the piano roll, and keep one preset library, one learn mode and one undo across every instrument (§6.7).

---

## 1. Screenshots saved

All in `screenshots/`, all checked (correct PNG/JPEG signature, larger than 40 KB) and
viewed. Sibling reports in this folder have their own (`dr-`, `fm-`, `ky-`, `va-`…); they are not repeated
here.

| File | What it shows | Source |
|---|---|---|
| `fam-01-serum2-main.png` | Serum 2 main page (2380 px): tabs OSC/MIX/FX/MATRIX/GLOBAL, preset field with artist and description, sub + three oscillators (wavetable, wavetable, granular) + noise, two filters, 8 macros with target-count badges, ENV 1–4 and LFO 1–10 tabs with counts, a Path LFO, clip sequencer, arpeggiator, key/scale and keyboard. | xferrecords.com |
| `fam-02-vital-main-frame.png` | Vital main page. **A frame taken from Vital's official demo video** (`vital.audio/videos/full_screen.mp4`), since the site publishes no still. H.264, so colours are approximate. | vital.audio |
| `fam-03-phaseplant-overview.png` | Phase Plant 2.0.6 annotated overview from the manual: (1) header, (2) 8 macros with connection dots, (3) generators, (4) three effect lanes, (5) modulators, (6) footer with a latency readout. | kilohearts.com/docs |
| `fam-04-vital-drag-to-modulate-frames.png` | **Composite of three frames** from Vital's `drag.mp4`: an LFO tile is grabbed, every target turns teal, and the filter slider previews its range before the drop. | vital.audio |
| `fam-05-omnisphere3-main.jpg` | Omnisphere 3 main page: the patch browser always docked at the left, 8 parts, hardware-profile menu, layer page with a modulation "zoom", LFOs, envelopes. | spectrasonics.net |
| `fam-06-omnisphere3-patch-browser.jpg` | Omnisphere 3 full patch browser: Category / Keyword / Mood / Timbre columns with AND/OR toggles, Browse / Search / **Match**, **Sound Lock**, a description panel, rating dots, Sort, shuffle and audition. | spectrasonics.net |
| `fam-07-pigments-play-view-dark.jpg` | Pigments 7 **Play view** (dark): a large preset name with a type tag, 4 macros, an audio-reactive visual, a summary deck and the keyboard. The footer shows play mode, undo, history, **CPU 13 %** and a resize grip. | arturia.com |
| `fam-08-pigments-play-view-light.jpg` | The same view in the **light theme**. | arturia.com |
| `fam-09-pigments-synth-combi-view.jpg` | Pigments Synth page: engines, filters, **a 25-tile modulation strip** colour-coded by family, a Combinate editor, macros, and **a help line at bottom left** ("Combinate 3 Type: Select the type of Combinate"). | arturia.com |
| `fam-10-xo-space-search-filter.png` | XLN XO: the **Space** map (every sample as a point clustered by similarity) with the Search & Filter panel: type pills, favourites, range sliders (Main Freq, Length, Drumminess), date added, sample folders. | xlnaudio.com |
| `fam-11-pianoteq9-main.jpg` | Pianoteq 9 Pro on a tablet (**a photograph**, so it is not measured): **CPU 2 %** at top left, "(modified)" preset marker, A/B, undo, a value popup ("+0.20") above the finger, a light wood-coloured panel. | modartt.com |
| `fam-12-fl-flex-main.png` | FL Studio FLEX Edit page: pack art and preset list with stars, Play / Edit / Library / Store, **search with Tags, history, favourites and a dice**, envelopes, filter, effects and 8 named macros. | image-line.com manual |
| `fam-13-fl-flex-browser.png` | FLEX preset view and pack view (manual figure). | image-line.com manual |
| `fam-14-fl-sytrus-main.png` | FL Sytrus: MAIN / OP1–6 / FILTER 1–3 / FX tabs, X/Y modulation pad, **the FM/RM matrix as a grid of knobs**, preset notes text, keyboard with octave labels. | image-line.com manual |
| `fam-15-ableton-meld-expanded.png` | Ableton Meld expanded view: **the modulation matrix as a spreadsheet** (targets in rows; Envelopes / Modulation / MIDI & MPE / Cross-modulation in columns; values typed in cells, cyan for engine A and orange for B), with the device underneath. | ableton.com manual |
| `fam-16-ableton-drift-device.png` | Ableton Drift: a whole synth in one 375 px strip with **every value printed under its control**. | ableton.com manual |
| `fam-17-logic-alchemy.jpg` | Logic Alchemy (Advanced view) inside Logic: Browse / Simple / Advanced, four sources, filters, a modulation rack per target, MSEG, 8 performance pads and two XY pads. | apple.com |
| `fam-18-logic-studio-bass.jpg` | Logic Studio Bass: instrument art, playing style, **a fretboard strip showing where the note is played** instead of a keyboard. | apple.com |
| `fam-19-bitwig61-sampler-slicing.png` | Bitwig 6.1 Sampler docked in the device panel: play modes (Repitch, Spectral, Cycles, Textures, Fragments), slices named by note, and the **app status bar giving the gesture for the hovered control** ("DOUBLE-CLICK Maximize window"). | bitwig.com |
| `fam-20-bitwig6-modulators.png` | Bitwig 6 modulators beside a device: modulator tiles with **routing ports** (the arrow icons) and a macro knob. | bitwig.com |
| `fam-21-kontakt8-factory-instrument.png` | A Kontakt 8 factory-library instrument (Orchestral Tools): a big art panel and **tiny labels** under the few controls. | native-instruments.com |
| `fam-22-kontakt8-library-browser.png` | Kontakt 8 library browser, **cropped** from an official composite: Instruments / Combined / Tools / Leap / Loops / One-shots, Brand / Sound Type / Character facets, artwork tiles, **CPU and Disk readouts**. | native-instruments.com |

---

## 2. Stock instruments by DAW (September 2026)

### 2.1 Versions and what each ships

| DAW (current) | Release | Price | Instruments shipped | What changed for instruments recently |
|---|---|---|---|---|
| **FL Studio 2026** | July 2026 | from USD 99 (Fruity) to All Plugins; free updates for life | **39 generators** in All Plugins: Fruity (23) = 3x OSC, Autogun, BassDrum, BeepMap, BooBass, Channel Sampler, DirectWave Player, Drumpad, **FLEX**, Fruity DrumSynth Live, Fruity DX10, Fruity Granulizer, Fruity Kick, **FPC**, Fruity Slicer, **Fruity Slicer 2**, GMS, MiniSynth, Plucked!, SimSynth, Speech Synthesizer, Wave Traveller, **FL Keys**. Producer adds **Kepler, Slicex, SoundFont Player, Sytrus**. Signature adds **DirectWave** (full) and **Harmless**. All Plugins adds **Drumaxx, Harmor, Kepler Exo, Morphine, Ogun, Poizone, Sakura, Sawer, Toxic Biohazard, Transistor Bass**. | FLEX **rebuilt** (new browser with genre filters, lower CPU, 8 free "Core Series" packs of 200+ presets); Fruity Slicer 2 redesigned; Transmitter (a transient/sustain splitter, an effect). |
| **Ableton Live 12.4** | 12.0 March 2024; 12.4 5 May 2026 | Intro EUR 79, Standard 279, **Suite 599** | Intro 8: Drum Rack, Impulse, Instrument Rack, Simpler, **Drift**, CV Instrument, CV Triggers, **Drum Sampler**. Standard 12 adds the 8 Drum Synths, **Analog, Collision, Electric, Tension** (moved down from Suite in Live 12). **Suite 21** adds Bass, **Operator**, Poli, **Sampler**, **Wavetable**, **Meld**, **Granulator III**. Library: 71+ GB (Suite). | Meld (12.0), Drum Sampler (12.1), Meld Chord oscillator (12.2), **A/B compare on every native device** (12.3), Wavetable and Operator voice limits raised (12.2–12.4), screen-reader work across 12.x. |
| **Logic Pro 12.3** | 12.0 28 Jan 2026; 12.3 30 Jun 2026 | USD 199.99, or the Apple Creator Studio subscription (USD 12.99/month) | About 28 instruments (MusicTech's count for Logic 11): **Alchemy**, Sample Alchemy, **Sampler**, **Quick Sampler**, **Drum Machine Designer**, Drum Kit Designer, Drum Synth, Ultrabeat, **Sculpture**, ES1, ES2, ES E/M/P, EFM1, EVOC 20 PolySynth, **Retro Synth**, **Studio Piano, Studio Bass, Studio Strings, Studio Horns**, Vintage B3, Vintage Electric Piano, Vintage Clav, Vintage Mellotron, Klopfgeist, External Instrument, plus the **Session Players** (Drummer, Bass, Keyboard; **Synth Player** styles in 12). | Synth Player (12.0; its six styles sit inside the Keyboard and Bass Players and drive Alchemy by default); an Alchemy **granular sync mode** with formant control (12.3). |
| **Bitwig Studio 6.1** | 6.0 11 Mar 2026; 6.1 26 Aug 2026 | Essentials EUR 99, Producer 199, **Studio 399** (Bitwig's own page) | **Polymer**, **Phase-4**, **Polysynth**, FM-4, Organ, **Sampler**, **Drum Machine**, the E-drums (E-Kick, E-Snare, E-Hat, E-Clap, E-Tom, E-Cowbell), and **The Grid** (Poly Grid, FX Grid, Note Grid). Studio has 188 devices in all, with up to **43 modulators**. | 6.0 was about the DAW core (automation, clip aliases, a global key). 6.1 **rebuilt the Sampler**: Spectral stretching, Fragments granular (up to 256 grains per voice), Cycles, Textures, Analog/Digital repitch characters, slicing in every mode, automatic pitch and tempo detection. |
| **Cubase 15** | 5 Nov 2025 | Pro EUR 579, Artist 329, Elements 99.99 | **HALion Sonic** (a ROMpler), **Groove Agent SE 6**, **Retrologue**, **Padshop**, the **Drum Machine** (added in 14), **Writing Room Synths**, **Omnivocal** (beta vocal synthesis). | 40 new Drum Machine kits; Groove Agent SE 6 got a scalable UI; Omnivocal sings lyrics typed in the Key Editor. |
| **Fender Studio Pro 8** (was Studio One) | 13 Jan 2026 (SOS reviewed it in March 2026) | USD 199.99 perpetual | **Impact XT** (drums), **Sample One XT** (sampler), **Presence XT** (multisample player), **Mai Tai** (VA), **Mojito** (mono VA). | No new instruments; Sample One gained a modulation matrix (16 slots, 14 sources, 24 destinations) and waveform fade editing; a "3D" restyle. |
| **REAPER 7** | 7.74 June 2026 (7.80 in September) | cheap licence (see reaper.fm) | **ReaSynth** (basic synth), **ReaSamplOmatic5000** (one sample per instance, chainable), **ReaSynDr** (four drum sounds), plus JSFX script instruments. | Nothing notable for instruments. |

### 2.2 The category × DAW matrix

Instrument names are the vendors' own. "pack" means the sound ships as a sample pack or preset library, not
as an engine. "—" means nothing in that category ships. Cells marked "?" could not be confirmed from a page
read for this report.

| Category | FL Studio 2026 (All Plugins) | Live 12 Suite | Logic Pro 12 | Bitwig 6 Studio | Cubase 15 Pro | Studio Pro 8 | Reaper 7 |
|---|---|---|---|---|---|---|---|
| Subtractive / VA | 3x OSC, Sawer, Poizone, MiniSynth, SimSynth, Kepler, Kepler Exo | **Analog**, **Drift**, Poli, Bass | **Retro Synth**, ES2, ES1, ES M/P/E | **Polysynth**, **Polymer** | **Retrologue** | **Mai Tai**, Mojito | ReaSynth (basic) |
| Wavetable / hybrid | (Harmor, Sytrus waveshapes) | **Wavetable**, **Meld** | **Alchemy**, Retro Synth (wavetable mode) | **Polymer** (wavetable osc), Grid | ? | — | — |
| FM / PM | **Sytrus**, Toxic Biohazard, DX10 | **Operator** | EFM1, Retro Synth (FM mode) | FM-4, **Phase-4** | ? | — | — |
| Additive / spectral / resynthesis | **Harmor** | — | **Alchemy**, Sample Alchemy | Sampler (Spectral) | Padshop (spectral) | — | — |
| Granular | Fruity Granulizer | **Granulator III** | Alchemy (granular, sync mode in 12.3) | Sampler (**Fragments**), Grid | **Padshop** | — | — |
| Physical modelling | Plucked!, Sakura (strings) | **Collision**, **Tension**, **Electric** | **Sculpture** | Grid (resonator modules) | — | — | — |
| Modular | — (Patcher chains plug-ins) | Max for Live | — | **The Grid** | — | — | JSFX (scripting) |
| Multisampler | DirectWave, SoundFont Player | **Sampler** | **Sampler** | **Sampler** | HALion Sonic (player) | Presence XT | ReaSamplOmatic5000 (chained) |
| Quick sampler / slicer | **Slicex**, Fruity Slicer 2, Channel Sampler | **Simpler** | **Quick Sampler** | Sampler (Sliced) | Sampler Track | **Sample One XT** | ReaSamplOmatic5000 |
| Drum pads / kit host | **FPC**, Drumpad | **Drum Rack**, Drum Sampler, Impulse | **Drum Machine Designer** | **Drum Machine** | **Groove Agent SE 6**, Drum Machine | **Impact XT** | ReaSynDr, RS5K |
| Drum synthesis | Drumaxx, DrumSynth Live, Fruity Kick, BassDrum | 8 **Drum Synths** | Drum Synth, Ultrabeat | E-Kick/Snare/Hat/Clap/Tom/Cowbell | Drum Machine | — | ReaSynDr |
| Acoustic drum kit | FPC kits | pack | **Drum Kit Designer** + Drummer | — | Groove Agent SE kits | Impact / Presence content | — |
| Piano | FL Keys | pack (Suite) | **Studio Piano** | — | HALion Sonic content | Presence content | — |
| EP / organ / clav | FL Keys (EP) | **Electric**; organ in packs | **Vintage EP / B3 / Clav / Mellotron** | **Organ** | HALion Sonic content | Presence content | — |
| Bass | **Transistor Bass** (303-style), BooBass | Bass (M4L) | **Studio Bass** + Bass Player | — | HALion Sonic content | Presence content | — |
| Strings / brass | — | packs | **Studio Strings / Horns** | — | HALion Sonic content | Presence content | — |
| Vocal synthesis | Speech Synthesizer | — | (EVOC vocoder) | — | **Omnivocal** (beta) | — | — |
| Pattern / "player" generators | (piano-roll scripts, Gopher) | MIDI Generators, Expressive Chords, Sting/Patterns packs | **Session Players** (Drummer, Bass, Keyboard, Synth) | Note FX, Note Grid | Pattern sequencer, Chord Pads | Chord Assistant | — |
| ROMpler / preset player | **FLEX** | — | — | — | **HALion Sonic** | Presence XT | — |

**Reading the matrix.** Live Suite, Logic and FL each cover 14–16 of the 19 rows with real engines. Bitwig
covers about 12, strongest in modular, sampler and modulation. Cubase and Studio Pro lean on a ROMpler
(HALion Sonic, Presence) for acoustic sounds and have little synthesis. Reaper ships almost nothing. **The
rows every serious DAW fills are VA, a multisampler, a quick sampler, drum pads with drum synthesis, and a
piano or EP.** Wavetable, FM, granular and physical modelling separate the top three from the rest.

---

## 3. Which stock instruments are rated best-in-class, and why

| Instrument | What reviewers say | Why it clears the bar |
|---|---|---|
| **Logic Alchemy** | gearnews ranks it the stand-out of Logic's bundle for its many engines and "stunning" sound; guides call it Logic's flagship and set it beside Serum and Omnisphere **[excerpt]**. Logic 12's Synth Players load Alchemy by default (SOS). | Additive, spectral, granular, VA and sample engines in one; 3,000+ presets; **three faces** (Browse / Simple / Advanced) over one engine (fam-17). |
| **Logic Studio Bass / Studio Piano** | Studio Bass's articulations (slides, damped notes) are called very convincing; one user write-up says it will replace NI's equivalents much of the time **[excerpt]**. Studio Piano is a large step up from the old patches (damper noise, sympathetic resonance) but not as detailed as NI's paid pianos **[excerpt]**. | Playing-style models and a *player* (Session Players) that performs them; a playing surface that shows the instrument (a fretboard for bass). |
| **Live Operator, Wavetable, Drift, Meld** | Ableton's stock instruments are called "first-class" **[excerpt]**. gearnews says Live strikes the best balance between DAW and instruments. SOS: Meld is smooth, rich and "surprisingly versatile given the minimal number of controls". MusicRadar compares Meld to Arturia's MiniFreak and calls Live 12's new devices "exceptionally impressive". | **Every parameter is on one strip**, with values printed (fam-16). The devices are MPE-aware, they share one look, and Racks add 16 macros with variations. |
| **Live Drum Rack / Drum Sampler** | MusicRadar praises Drum Sampler's "appealing simplicity". | Any instrument or sample per pad, chokes, per-pad effects and sends: the model every drum host copies. |
| **Live Granulator III** | SOS: "seriously enhanced", MPE-capable, and "convincingly like a native Live instrument". | Capture straight into the device; drag-drop; a visual grain display. |
| **FL Sytrus** | Called FL's flagship FM synth and "one of the most powerful stock plugins in any DAW" **[excerpt]**. | 6 operators, 3 filters, an FM/RM **matrix of knobs**, an X/Y performance pad (fam-14). |
| **FL Harmor, FLEX** | "Have aged surprisingly well" **[excerpt]**. FLEX was rebuilt in 2026 around its browser. | Harmor's additive resynthesis is unique in any DAW. FLEX is a fast preset player: packs, tags and 8 named macros (fam-12). |
| **Bitwig Poly Grid / Polymer** | gearnews: a "Reaktor-like" modular environment. Polymer mixes and matches oscillators, filters and envelopes. | **Every parameter can be modulated by 40+ polyphonic modulators**, with an Inspector list of all sources and destinations (§4.5). |
| **Bitwig 6.1 Sampler** | Synth Anatomy calls 6.1 the sampler's move to "the next sound design level"; gearnews lists Spectral, Fragments (256 grains per voice), pitch analysis and slicing in every mode. | A sampler that is also a granular and spectral instrument. It is the new bar for "stock sampler". |
| **Studio Pro Sample One XT** | SOS: its new modulation matrix (16 slots, 14 sources, 24 destinations) is the depth you would expect from a third-party sampler. | Shows the bar moving: stock samplers now need a mod matrix. |
| **Reason's Objekt, Europa, Thor** (not a target DAW) | gearnews calls Objekt "one of the best DAW stock synths around". | A reminder that physical modelling, done well, is a differentiator. |

**What the praised ones have in common:**
1. **A simple face over a deep engine.** Alchemy has Simple and Advanced views; FLEX has Play and Edit;
   Meld hides its matrix behind an expand button; Pigments has Play and Synth.
2. **A large, tagged library.** Alchemy has 3,000+ presets, Pigments 1,700+, Omnisphere 3 added 26,000
   patches, and the FLEX rebuild was sold on its browser.
3. **Native integration** that a plug-in cannot match: Session Players driving Alchemy and Studio Bass,
   Drum Rack's per-pad chains, Bitwig's modulators reaching any device.
4. **Consistency.** Live's devices look like one family. FL's and Logic's instruments span 20 years of
   visual styles (Sytrus next to FLEX, ES2 next to Studio Bass), and reviewers do not praise that.

**The bar for Cantus, in one sentence:** a VA or hybrid synth as good as Wavetable or Polymer, a sampler as
good as Bitwig 6.1's, a drum machine as good as Drum Rack with drum synths, a piano and EP as good as
Logic's, an FM synth as good as Operator, and a preset library with a real browser, all in one visual
family.

---

## 4. The shared interface language of top instruments

Products studied: Serum 2, Vital, Pigments 7, Phase Plant 2, Diva, Omnisphere 3, Kontakt 8, Battery 4
(see [drums.md](drums.md)), XO, Pianoteq 9, Surge XT (for accessibility), and the Ableton and Bitwig devices.

### 4.1 Layout: where things live

| Zone | Convention | Examples |
|---|---|---|
| **Header** (one row) | logo, **page tabs**, then the **preset field** (browser button · favourite · name with a modified marker · ‹ ›), undo/redo, a **master level knob and meter**, and a menu or settings button | Serum 2 (OSC / MIX / FX / MATRIX / GLOBAL, "FX - Demon Woosh **\***"), Pigments (PLAY / SYNTH / FX / SEQ, "Krog Keys**\***"), Phase Plant (browse, ♥, name, author, description, save, new, undo, redo), Vital (VOICE / EFFECTS / MATRIX / ADVANCED), Pianoteq ("(modified)") |
| **Sound sources** | in a **row across the upper half**, left to right in signal order: oscillators (each with its own waveform display) → mixer → filters (each with its response curve) | Serum 2: SUB · OSC A · OSC B · OSC C · NOISE · FILTER 1 · FILTER 2. Pigments: ENGINE 1 · ENGINE 2 · UTILITY · FILTER 1 · FILTER 2 · routing. Vital: OSC 1–3 + SMP stacked at the left, filters beneath. |
| **Modulation sources** | a **strip of tiles across the lower half**, each drawing its live shape; one opens into a large editor | Serum 2: ENV 1–4, LFO 1–10, VELO, NOTE. Pigments: 25 tiles (VELO, AT, MW, KBD, EXP, ENV, LFO, FUNC, RAND, COMB, M1–M4). Phase Plant: modulator tiles along the bottom. Vital: envelopes and LFOs on the right half, MIDI sources in a grid. |
| **Macros** | 4–8 large knobs, **left column or top row**, renamable | Serum 2 (8, left), Vital (4, left), Phase Plant (8, top row), Pigments (4, right), FLEX (8, bottom), Omnisphere (Orb and global controls) |
| **Keyboard** | the **full width of the bottom edge**, with pitch and mod wheels at its left | Serum 2, Vital, Pigments, Sytrus, Pianoteq, Kontakt. Studio Bass uses a **fretboard**. Phase Plant, Omnisphere and Diva put it behind a button or omit it. |
| **Footer** (thin) | play mode and polyphony, undo history, **CPU %**, **latency**, resize grip, a help line | Pigments ("Play Mode Poly 16 · ↶ ☰ ↷ · 10 % · grip"), Phase Plant ("Latency: 16.33 ms"), Kontakt ("CPU 0 % Disk 0 %"), Pianoteq ("2 %" at top left) |

**Two sub-families.** *Page-per-section* instruments (Serum's tabs, Omnisphere's A–D layers, Sytrus's
OP tabs, Alchemy's A–D) trade overview for size. *One-page* instruments (Vital, Phase Plant, Drift) show
everything and rely on modulation tiles to stay compact. Reviewers praise the one-page ones for speed and
the paged ones for depth. The **Ableton device strip** (fam-16)
is the extreme one-page case: a full synth in 375 px of height, every value printed, no hover needed. It is
the best model for a Cantus inspector strip.

### 4.2 Preset browsers

| Product | Structure | Distinctive feature |
|---|---|---|
| **Omnisphere 3** (fam-06) | Directory and Author menus; **four facet columns (Category, Keyword, Mood, Timbre), each with an AND/OR toggle**; a description panel (author, path); a result list with **5-dot ratings**; Sort, shuffle, ↓ ↑ step, refresh, **▶ audition** | **Sound Match** (find patches like this one), **Sound Lock** (lock layers or effects while browsing others), a browser docked permanently at the left of the main page (fam-05). SOS: marking and rating favourites is "a vital step if you want to find them again". |
| **Kontakt 8** (fam-22) | tabs (Instruments, Combined, Tools, Leap, Loops, One-shots); **facets Brand / Sound Type / Character** as pill rows; artwork tiles; a user filter and a ★ | a compact Side Pane browser (SOS); library art as the primary cue |
| **XO** (fam-10) | Search; type pills (Kick, Snare, Clap, CI HH…); favourites; **range sliders for Main Freq, Length and "Drumminess"**; date added; folder checkboxes | **Space**: every sample as a point on a 2-D map, placed by sound similarity. You browse by moving, not by reading. |
| **FLEX** (fam-12, fam-13) | a preset list with stars beside the pack art; search with **Tags ▸**, **history (recent)**, **favourites**, a **dice (random)**; toggle between pack and preset views | every preset carries at least two tags (type and style); Shift-click an arrow to preview; Ctrl+↑/↓ steps; a "lock changes" icon keeps your edits while browsing |
| **Vital** (source code) | search, a sortable list (★ · name · **style** · author), **style filters** (Bass, Lead, Keys, Pad, Percussion, Sequence, Experimental, SFX, Template), favourites | simple, fast, fully keyboard-searchable |
| **Phase Plant** | name, author and a **description with hashtags** ("#mpe #lead #morphing") in the header; a browse panel | the tags live in the description text |
| **Surge XT** | text search with **field queries** (`AUTHOR=`), category arrows, ♥ favourites, middle-click for a random patch | queries typed like search operators |
| **Ableton Live 12** | browser **tags** (with a Quick Tags panel and auto-tagging in 12.2), **Sound Similarity** search (12.0), Splice "Search with Sound" (12.3), **hot-swap** a device's preset in place | MusicRadar: similarity search "can be a hugely inspiring way to mix up the sound palette", best with percussion. SOS: you cannot search for devices already loaded in the Set. |
| **Pigments 7** | a browser button, ♥, the name with a modified asterisk, ▲▼ steps, a type tag ("Keys", "Lead") on the Play page | colour-coded type tags; "sound banks" and expansions |

**The shared minimum:** search; a type/category facet; a style or character facet; favourites; author;
▲▼ stepping; a modified marker; audition; recent history. **The differentiators:** similarity
(Sound Match, XO Space, Live's Similarity), locking parts while browsing (Sound Lock, FLEX's lock), ratings,
AND/OR facets, and random.

**Gaps nobody fills:** auditioning a preset *with the user's own part* (all of them play the keyboard or a
demo phrase); a single library across instruments *and* the chains they sit in; "presets used in this
project"; a diff against the factory preset.

### 4.3 Macros

- **Count:** Serum 2 has 8, Phase Plant 8, FLEX up to 8 ("the number active varies from sound to sound"),
  Pigments 4, Vital 4, Live Racks 16 with **variations** (snapshots of all macros).
- **Naming:** renamable in Serum 2, Vital, Phase Plant, Pigments and FLEX, and set per preset by the sound
  designer.
- **Showing what a macro drives:**
  - Serum 2 puts a **number badge** beside each macro (1, 3, 6), the number of targets it drives
    (fam-01).
  - Phase Plant puts a **row of coloured dots under each macro name**, one per connection
    (fam-03).
  - FabFilter uses source-type dots.

  None of these name the targets without a click.
- **Play pages are built from macros.** Pigments' Play view is 4 macros, the preset name, a visual and a
  summary deck (fam-07); FLEX's Play page is the pack art
  and the macros. This is where most users stay.

### 4.4 Drag-to-modulate, and how live modulation is drawn

| Product | Connecting | Drawing the depth | Drawing the live value |
|---|---|---|---|
| **Vital** (fam-04) | drag a source tile; **every valid target turns teal** during the drag; a drop circle follows the pointer; hovering a target **previews the modulation** (the filter curve moves) before you release | a coloured arc on the knob (the skin has a `RotaryArc` and a `ModulationMeter`) | a **modulation meter** per control, **split left/right for stereo modulation** (skin tokens `ModulationMeterLeft/Right`) |
| **Serum 2** | drag a source onto a knob, or use the MATRIX page | a **coloured arc** around the knob; a **count badge** on every source tab | the arc moves while notes play |
| **Pigments** (fam-09) | drag and drop, colour-coded: each **family** of sources has one hue (keyboard pink, envelopes orange, LFOs yellow, functions green, random purple, macros cyan) | selecting a source shows its depth arcs on every target; "hover quick-edit" of the depth (Arturia's page) | source tiles animate their shape |
| **Phase Plant** | hover a module and a **green ⊕** appears; drag it to the parameter; **the target turns from blue to green** | an arc in the modulator's colour | coloured dots under macros |
| **Bitwig** (fam-20) | click a modulator's **routing port** (an output jack with a cord); assigned targets turn bright and **potential targets are shaded**; click a target and drag to set the depth | **blue tint for monophonic, green for polyphonic** modulation; amounts in orange in the Inspector | the ring moves; modulation can reach past the parameter's range ("relative") |
| **Surge XT** | select a source in the routing bar, which puts the UI in **routing mode (bright green)** | **blue depth sliders** drawn over the base controls | the Modulation List (see 4.5) |
| **FabFilter** | drag a source's handle; valid targets light up; the slot panel pops up | an arc in the source-type colour, yellow when types mix | arcs and a decorative "source flow" (`fabfilter/family.md` §5) |
| **Omnisphere / Alchemy / Diva** | menus and slots: source and target drop-downs ("MODULATION — Wheel → Cutoff", Alchemy's per-target rack with Depth knobs) | numbers in the slot | little or none on the knob |

**Conventions worth taking:**
1. Highlight every valid target during the drag, and shade the invalid ones.
2. Preview before the drop (Vital).
3. Draw the arc from the knob's own position, in the source's colour.
4. Show a count or dots on each source.
5. Show poly and mono differently (Bitwig).
6. Split stereo modulation (Vital).

**What nobody does well:**
- a **per-voice display**: with 8 voices playing, the knob shows one arc and one value, not eight;
- the **depth in the target's units** ("±1.5 oct", "±6 dB"); Bitwig's Inspector is the only place that
  lists amounts, and only as percentages;
- telling **two instances of the same source type apart** at a glance, when colour is by family (Pigments,
  FabFilter).

### 4.5 The modulation matrix

- **A grid of knobs.** Sytrus's FM/RM matrix: operators in rows, destinations in columns, a knob in every
  cell. It is dense but readable because the grid *is* the topology (fam-14).
- **A spreadsheet.** Meld's expanded view: **targets in rows**, sources grouped in columns (Envelopes,
  Modulation, MIDI & MPE, Cross-modulation), the depth **typed into a cell**, and colour by engine (cyan A,
  orange B), with "Copy to B" (fam-15). It is the clearest
  overview found. Measured: an empty cell against the grid is only 1.6:1, but values are cyan at 10:1.
- **Slot lists.** Serum's MATRIX page and Vital's MATRIX tab: one row per connection (source, curve, amount,
  bipolar, stereo, destination). Pigments and Alchemy put the list per target.
- **An overview of all connections.** **Bitwig's Inspector** has a Modulation Sources tab and a Modulation
  Destinations tab with amounts and transfer curves. **Surge XT's Modulation List** is sortable by source or
  target, with mute and amount per row. These two are the only complete overviews found, and they answer
  the FabFilter gap noted in `fabfilter/family.md` §5.6.

### 4.6 The on-screen keyboard: what it shows

| Shows | Who |
|---|---|
| Played notes (from MIDI and the mouse) | everyone with a keyboard |
| Octave labels | Sytrus (C3–C7), Serum 2 (C3) |
| Key and scale, transpose, swing | Serum 2's key/scale/swing row above the keys |
| Clip and arpeggiator mini-keyboards | Serum 2 (CLIP, ARP) |
| Pitch and mod wheels, bend range | Serum 2 (2/−2), Vital (PITCH WHL, MOD WHL), Pigments (up/down sliders) |
| Hold / latch | Pigments |
| Octave and fine tune | Pigments (Octave, Fine) |
| Playable range, key switches in another colour | Kontakt libraries (widely documented; not verified on a page read here) |
| Sample zones and root keys | samplers (Bitwig's "Use [keys] from C1", slice notes) |
| The instrument's own playing surface | Logic Studio Bass's **fretboard**, which shows where on the neck the note is played (fam-18) |
| Condition, hammer and range | Pianoteq (the "Condition" slider over the keys) |

**What no keyboard shows:** **voices** (how many are sounding and which are being released or stolen),
**per-note MPE state** (the bend, pressure and slide of each held note), and **the notes of the clip on this
track or the project's key**. A host can show all three.

### 4.7 Resizing, HiDPI and scaling

- **Continuous resize with a corner grip:** Pigments (the grip is visible in the footer), Phase Plant,
  Pianoteq 9 (SOS: "continuously resizeable with a corner drag handle"), Vital.
- **Fixed steps:** Diva from 70 % to 200 %; Surge XT's Zoom submenu, with "set as default", capped by the
  screen size; FabFilter's sizes plus a scaling submenu.
- **Recently added:** Groove Agent SE 6 got a scalable UI in Cubase 15.

A scalable UI is now expected. What still differs is whether the text scales with the frame (it does in
vector UIs like Vital, Pigments and Surge) or the controls only spread out (older bitmap UIs).

### 4.8 Themes

- **Pigments** ships dark and light themes (fam-07,
  fam-08). SOS notes the Pigments 6 light mode got
  darker knobs for contrast. **It is the only mainstream instrument found with a first-class light theme.**
- **Vital** has a real **skin system**: about 60 colour tokens (background, body, heading text, rotary arc,
  rotary body, modulation meter, and so on), per-section overrides for 30 sections (oscillator, filter,
  envelope, LFO, macro, keyboard, modulation drag-and-drop, preset browser…) and a built-in **Skin
  Designer** window. This is the closest thing to Cantus's editable palette.
- **Surge XT** ships Classic and Dark skins plus community skins, and follows the OS light/dark setting for
  its menus.
- **Ableton Live** has app-wide themes, including light ones, and its devices follow them. This was not
  re-verified on a page read here.
- **Dark-only:** Serum 2, Phase Plant, Omnisphere, Kontakt 8, XO, Bitwig and the FabFilter family.

### 4.9 Animation

- **Continuously animated:** wavetable 3-D stacks (Serum 2), oscillator waveforms and filter curves
  (all of them), envelope and LFO playheads, and sources drawing their output live (Pigments' tiles,
  Serum's LFO dot).
- **Decorative:** Pigments 7's audio-reactive visualiser, which has a **Disable Visualizer** button in the
  footer, an explicit admission that it costs attention and CPU. FabFilter's source flow can also be turned
  off.
- **Vital's pitch:** everything animates at 60 fps with GPU drawing, and the animation is the explanation
  ("animated controls, filter responses, waveforms, smooth LFOs, oscilloscopes, spectrograms").

### 4.10 Typing values

- **Surge XT:** "Edit Value" in the context menu.
- **FabFilter:** double-click, with a parser that takes `1k`, `A4` and `2x`.
- **Serum 2 and Vital:** double-click or type in value fields; printed readouts are editable.
- **Ableton:** every numeric box accepts typing; the device *is* a set of readouts.
- **Pianoteq:** a value balloon above the touched control (fam-11).

Direct numeric entry is universal. **Note-name entry** (a filter cutoff at "A4") is FabFilter's alone
among the products studied.

### 4.11 MIDI learn and controllers

- **Inside the plug-in:** Diva's "MIDI learn page and editable list", Surge XT's context-menu MIDI Learn,
  FabFilter's learn mode, Pigments' right-click learn.
- **At host level:** Live's MIDI Map mode, Bitwig's controller assignments and Remote Controls (8 knobs
  per page), Kontakt and NKS mapping for NI keyboards, Omnisphere 3's **300+ hardware profiles** (the HW
  menu maps a physical synth's panel onto Omnisphere's controls).

Instruments inside a DAW do not need their own learn, because the host maps controllers to any parameter
(`fabfilter/family.md` §7.5 item 7).

### 4.12 MPE

Now standard: Serum 2, Vital, Pigments, Phase Plant, **Diva 1.4.8** (added MPE), Omnisphere 3, Pianoteq,
Surge XT, Live's Drift, Meld, Granulator III and the other MPE-aware devices, and Bitwig (polyphonic
expressions end to end). Presentation varies:
- dedicated MPE sources in the modulation strip (Vital's LIFT, PRESSURE, SLIDE; Meld's matrix columns Vel,
  Pitch, PB, Press, Mod W, Rand, Note PB, **Slide**);
- a pitch-bend range per note (Drift's Note PB and PB Range);
- an MPE badge in the header (Omnisphere's "MPE").

**No instrument studied visualises per-note MPE state on its keyboard.**

### 4.13 Help, hover and onboarding

| Pattern | Who |
|---|---|
| **A help line at the bottom that names the hovered control and what it does** | Pigments ("Combinate 3 Type: Select the type of Combinate"); Bitwig's app status bar gives the *gesture* ("DOUBLE-CLICK Maximize window"); Ableton's Info View; FL Studio's hint bar |
| **Preset notes** that explain the macros and wheel | Sytrus ("Mod X adjusts filter cutoff, Mod Y adjusts high band"); Omnisphere's description panel; Phase Plant's description |
| **In-app tutorials** | Pigments (the lightbulb in the header opens interactive tutorials) |
| **Help bubbles** | FabFilter |
| **An AI assistant** | FL Studio 2026's Gopher now *acts* (routes, sets levels, writes scripts) |

The bottom help line, which appears in three of the best products, is the pattern Cantus already chose
(`fabfilter/family.md` §7.2 "Help strip").

### 4.14 Legibility, measured

Method as in `fabfilter/family.md`: the modal colour of a box on the element and on its surround; for text,
the mean of the 4 % of pixels furthest in luminance from the background; WCAG 2.x ratio. The Vital frame
comes from a video; the others are official PNG or JPEG images at their native size. Pianoteq (a photo) and
Alchemy and Studio Bass (reduced inside a laptop render) are not measured.

**Knob body against its panel** (WCAG 1.4.11 asks ≥ 3:1 for the boundary of a control):

| Product | Knob body | Panel | Ratio |
|---|---|---|---|
| FL FLEX | #545d71 | #595d72 | **1.01:1** (the knob is the panel; only its light arc, 4.8:1, shows) |
| Serum 2, macro knob | #2c3239 | #272c32 | **1.09:1** |
| Phase Plant, macro knob | #393838 | #4e4c4a | **1.37:1** |
| Phase Plant, effect knob | #2d2d2d | #4e4c4a | **1.61:1** |
| Vital | #222721 | #474b4d | **1.73:1** |
| Bitwig 6.1 Sampler | #424242 | #1c1c1c | **1.71:1** |
| Omnisphere 3 | #25252d | #4f4f57 | **1.87:1** |
| Pigments, Play deck (dark) | #4f5150 | #171b1e | **2.18:1** |
| Serum 2, oscillator knob on the light-grey deck | #404952 | #8193a1 | **2.88:1** |
| Pigments, Synth page (dark) | #787878 | #2b2b2d | **3.20:1** ✓ |
| Pigments, Play deck (light) | #565656 | #e8dcdc | **5.49:1** ✓ |

**Text:**

| Product | Element | Colours | Ratio |
|---|---|---|---|
| Pigments dark | labels / footer / macro names | #b7bbbe–#f9fafc on #181b20 | 8.9–17.8:1 |
| Pigments light | labels | #3e3735 on #e7dcdc | 8.6–9.1:1 |
| Pigments | **idle modulator names** (ENV 2, LFO 3) | #7a7d82 on #2b2c31 | **3.3:1** ✗ |
| Pigments | used modulator name (VELO, pink) | #d274b2 on #282728 | 4.9:1 |
| Pigments | **dim caption** ("offsets source by amount") | #6e6e70 on #2c2c2e | **2.8:1** ✗ |
| Serum 2 | labels on the light deck (PAN, WT POS) | #22272b on #8092a0 | 4.7:1 (just passes) |
| Serum 2 | cyan values, preset name | #32c3ff on #191d21 | 8.4:1 |
| Serum 2 | **"ARTIST:" label** | #687987 on #191d21 | **3.8:1** ✗ |
| Vital | labels (LEVEL, DELAY, MACRO 1) | #c4c8ca on #3a3e40 | 6.5–8.2:1 |
| Phase Plant | labels (Feedback, Glide, macro names) | #cccdcd on #4e4c4a | 5.4–5.6:1 |
| Phase Plant | small blue caps (SEMI CENT) | #79b7f1 on #232323 | 7.4:1 |
| Omnisphere 3 | knob labels (COARSE, RATE, CUTOFF) | #dcdde0 on #505058 | 5.5–6.0:1 |
| Omnisphere 3 | patch list | #d8d8d8 on #151514 | 12.9:1 |
| Omnisphere 3 | **rating dots, unselected row** | #3f4146 on #161616 | **1.8:1** ✗ |
| FLEX | labels, section titles | #dadfd6 on #5d6175 | 4.6–4.8:1 (marginal) |
| Sytrus | labels, info text | #a1acb0 on #3b4145 | 4.5–5.7:1 (marginal) |
| Meld | matrix row labels | #9b9fa9 on #3a3c45 | **4.1:1** ✗ |
| Ableton (Meld, Drift) | text on the light deck | #10121a on #9b9fa9 | 7.1:1 |
| Ableton | values on the dark display | #00e5f6 on #1f232d | 10.1:1 |
| Bitwig 6.1 | labels | #bdbdbd on #1c1c1c | 9.1:1 |
| Bitwig 6.1 | unselected mode names | #878787 on #1c1c1c | 4.7:1 |
| Bitwig 6.1 | **status-bar hint** | #939393 on #323232 | **4.2:1** ✗ |
| Kontakt 8 | factory instrument labels | #b2b7b1 on #1b1c1b | 8.1–8.4:1, but **tiny** (see below) |
| Kontakt 8 | browser facets, tiles, CPU | #aaada9 on #2b2b2b … | 6.3–12.4:1 |
| XO | filter labels and pills | #9a9c9d on #1d2125 | 5.9–6.8:1 |

**Reading this.**
- **Control boundaries fail almost everywhere.** Only Pigments' Synth page and its light theme clear
  3:1. The other instruments rely on the value arc or pointer (usually bright, 5–10:1) to show where a
  control is, which is FabFilter's "camouflage" problem again (`fabfilter/family.md` §6).
- **Primary text is fine; secondary text is where they fail.** The fails are idle and unused states,
  captions, metadata labels and rating dots: exactly the text that tells you *what is not happening*.
- **Size is the other failure.** The Kontakt 8 factory instrument's labels ("Articulation", "Legato") have a
  cap height of about **1/85 of the instrument's width** (fam-21).
  At a typical 630–700 px instrument width that is 7–8 px, below the 11 px floor set in
  `fabfilter/family.md` §7.4.

**Cantus's own knob, measured against the same rule** (tokens from `theme.rs`, knob drawn on `panel`, as
in `widgets/knob.rs`):

| Theme | Body (`panel_raised`) vs panel | Edge (`grid_bar`) vs panel | Track (`recess`) vs panel | Value arc (`gold`) vs panel | Pointer (`text`) vs body | Label (`text_dim`) vs panel |
|---|---|---|---|---|---|---|
| Ink & Gold | 1.11 | **1.71** | **1.08** | 7.51 | 12.2 | 5.66 |
| Paper | 1.08 | **1.77** | **1.33** | 4.60 | 15.3 | 5.62 |
| Parchment | 1.12 | **2.02** | **1.47** | 4.64 | 12.1 | 4.85 |

The pointer and the value arc pass, so the value is always readable. But **the knob's outline and its
track fail 3:1 in all three themes**. An untouched knob, which draws no value arc, is found only by its
pointer tick. The family rule "a body with a 3:1 edge" (`fabfilter/family.md` §7.2) is not met by the
widget yet. Instruments will multiply knobs by ten, so fix this first (§6.5 rule V1).

### 4.15 Accessibility

- **Surge XT:** screen-reader support on Windows and macOS, keyboard navigation throughout, editable
  shortcuts, typed value entry, optional announcements of patch and category changes. It is the reference
  among instruments.
- **Ableton Live 12:** screen-reader work in every point release (VoiceOver on the transport and status
  bar, NVDA and Narrator crash fixes, focus tracking in Learn View), and a keyboard workflow for automation
  breakpoints with spoken feedback (12.4). Native devices take part; third-party plug-in UIs generally do
  not.
- **Logic Pro** is widely reported to support VoiceOver across its native instruments; not re-verified on a
  page read here.
- **NI:** Komplete Kontrol and NKS have accessibility features for NI keyboards; the vendor page could not
  be fetched (404), so this is unverified.
- **Everything else studied** (Serum, Vital, Phase Plant, Pigments, Omnisphere, Diva, Kontakt instruments)
  documents no screen-reader or keyboard access. Custom GPU-drawn UIs usually expose nothing to the OS
  accessibility tree.

### 4.16 CPU and voice meters

| Product | Shows |
|---|---|
| Pigments | CPU % in the footer (10–13 % in the screenshots), play mode and polyphony ("Poly 16") |
| Kontakt 8 | CPU % and Disk % in the header, plus the voice count per instrument |
| Pianoteq 9 | CPU % at top left, large |
| Phase Plant | **latency** in the footer (16.33 ms, from a lookahead effect) |
| Serum 2 | a voice readout in the voicing section ("4 / 272"); no CPU meter visible |
| Surge XT | CPU usage on request (right-click the level meter) |
| Diva | no meter, but quality modes (draft for playing, divine for rendering) and a separate offline quality setting |
| Vital, Omnisphere, Phase Plant, Serum | no CPU display on the main page |

A CPU percentage measured inside a plug-in is a guess (it cannot see the host's other work), and a voice
count is the most useful thing a user can act on. **A host can show both exactly, per instrument** (§6.7).

---

## 5. Criticisms common to the category

| Criticism | Evidence |
|---|---|
| **Menu-diving and paged editors** | Omnisphere's Zoom pages, Alchemy's A–D and Morph pages, Sytrus's 11 tabs: a sound's structure is spread over pages you cannot see together. SOS's note that Omnisphere's GUI "looks much the same as ever" is praise from long-time users, and the price newcomers pay. |
| **Tiny text** | Kontakt library UIs (measured 7–8 px labels, 4.14); FabFilter's history of it (`fabfilter/family.md` §6). Library developers design for a fixed Kontakt panel size. |
| **Preset lock-in** | Every instrument has its own preset format and folders (Serum's, Vital's `.vital`, Omnisphere's, u-he's, Kontakt's `.nki`). NKS gives some cross-browsing, but only inside NI's ecosystem. Nobody searches across instruments except the DAW browsers (Live and Bitwig), which see only names and tags. |
| **Engines without preset management** | SOS's main complaint about Pigments 6: its Modal and Sample engines have no section presets. FabFilter's section presets are the exception. |
| **CPU opacity and cost** | Most instruments show no CPU figure. Diva's quality modes exist because its full quality is expensive. Visualisers need an off switch (Pigments' Disable Visualizer). |
| **Dark only** | Serum, Phase Plant, Omnisphere, Kontakt, XO, Bitwig. Only Pigments, Vital and Surge (skins) and Ableton (themes) offer light. |
| **Invisible control boundaries; failing secondary text** | Measured in 4.14: knob bodies at 1.0–2.2:1 in 8 of 11 cases; idle-state text at 2.8–3.8:1. |
| **Modulation you cannot survey** | Only Bitwig's Inspector and Surge's Modulation List show all connections. FabFilter, Phase Plant, Serum and Pigments show them per source or per target. |
| **Inconsistent families inside one DAW** | FL's generators span two decades of styles (Sytrus next to FLEX); Logic's ES2 and Ultrabeat sit next to Studio Bass. Ableton is the exception and is praised for it. |
| **Licensing friction** | Instruments come with iLok, NI Access, Spectrasonics or Arturia authorisation. None of this applies to native instruments. |
| **Separate undo, learn and preset systems per plug-in** | The same "plug-in wall" as `fabfilter/family.md` §4–6: the plug-in's undo ignores the host's, and learn needs MIDI routed to the plug-in. |

---

## 6. The Cantus instrument family rules

These extend `fabfilter/family.md` §7 (frame, components, interaction rules, visual rules, the modulation
model). Everything written there applies to instruments unchanged; this section adds what instruments need.

### 6.1 Principles

1. **One frame for effects and instruments.** The same header, footer and help strip, in the same
   positions (§7.1 of the FabFilter report). Instruments add **three fixed bands**: the **macro bar**, the
   **modulation strip** and the **keyboard**.
2. **Three views of every instrument, the same three everywhere:**
   - **Play:** the preset, 8 macros, the keyboard and one live picture;
   - **Edit:** every parameter;
   - **Matrix:** every modulation connection as a table.

   Play is the default after loading a preset. Edit is one click. Matrix is a tab.
3. **Modulation is visible at rest, per instance and per voice.** Every source instance has a colour *and*
   a tag (LFO 2, ENV 1). Every modulated control shows its range **in the target's units**, and while notes
   sound, one dot per voice.
4. **The keyboard is a live map of the instrument:** its range, zones, key switches, root keys, the notes
   sounding, the voices being released, the project's scale, and each held note's MPE state.
5. **One preset library for the whole app.** Facets, similarity, favourites, ratings and history are shared,
   and **audition plays the user's own clip**.
6. **The cost is visible:** voices in use out of the polyphony, the instrument's share of the audio thread
   (measured by the host, not guessed), latency, and disk streaming where it applies.
7. **The host owns state** (as in the FabFilter rules): undo, A/B, presets, automation, controller mapping,
   MPE routing and the modulation service.

### 6.2 How an instrument opens: inspector strip plus editor window

- **Selecting an instrument track** (in the channel rack, arrangement or mixer) shows the instrument's
  **Play strip** at the top of the inspector.
  - It holds the header row, 8 macros, a compact keyboard, the voice/CPU readout and the 4–6 "key
    parameters" the preset marks (the most-used values), laid out like an Ableton device strip, with every
    value printed.
  - This replaces today's inspector rows of Firmus sliders (`views/inspector.rs`).
- **Clicking the channel's button in the channel rack** (FL muscle memory), double-clicking the strip, or
  pressing Enter opens the **editor window**. It is the same `egui::Window` frame as `views/fx/mod.rs::window`,
  with the Play / Edit / Matrix tabs.
  - Several editor windows may be open, one per track, and each remembers its size and position in the
    project.
  - A **"dock" button** moves the window into a bottom panel (like Bitwig's device panel and Live's detail
    view) for users who work on one screen.
- **Why both:** the inspector is always there, so Play-level edits (macros, preset, level) never need a
  window. Deep editing needs room the inspector cannot give. Every leading DAW works this way (Live and
  Bitwig dock the device and float the plug-in; FL floats the plug-in from the channel rack).
- **Hosted third-party instruments** use the same header, footer and inspector strip, with the plug-in's
  own editor embedded in the window body (`cantus-host` `MainThread::open_editor`). The macro bar maps to
  the plug-in's parameters, and their names come from `ParamInfo`.

### 6.3 Component list (additions to `fabfilter/family.md` §7.2)

| Component | Anatomy | Rules |
|---|---|---|
| **Instrument header** | the effect header (channel chip, device name in the device hue, undo/redo, A/B, ‹ preset ● ›, bypass, ?, ⋯, ⛶) plus a **view switch (Play · Edit · Matrix)** and an **input chip** ("MIDI · MPE", "Keys 1") | the modified marker is a dot plus italic, never a dimmed name; ▲▼ and `[` `]` step presets; the view switch is keyboard-reachable (Ctrl+1/2/3) |
| **Instrument footer** | **voices "7 / 16"** (active / polyphony, with a steal count if any) · **CPU share of the audio thread** (host-measured) · latency badge · Play mode (Poly, Mono, Legato) · glide · output level and meter · size grip | always visible; a stolen voice flashes the count for 500 ms, and a tooltip explains it |
| **Macro bar** | 8 knobs (L size), each with a name, a **value readout** and a **target line**: the first target in small text ("Cutoff +2 oct") plus "+3" if there are more, and the source colour/tag ring | renamable (double-click the name); dragging the macro's handle modulates like any source; right-click → "Show targets" opens the Matrix filtered to it; empty macros show "Drag here to assign" in dim text (≥ 4.5:1) |
| **Modulation strip** | a row of **source tiles**: a live mini-graphic (LFO shape, envelope shape with a playhead, a random trace, MPE sources as live bars), a **tag** (LFO 1), a **target count**, and a drag handle | colour per instance from the colour-blind-checked rotation, plus the tag text; unused tiles show their tag at ≥ 4.5:1 (not the 3.3:1 Pigments uses); clicking a tile opens its editor below the strip |
| **Source editors** | LFO/MSEG (points with slope handles, sync, retrigger, phase), envelope (DAHDSR with slopes and a playhead per voice), random (sample & hold, smooth, seed), MPE/MIDI source (curve, range), step sequencer | the playheads show every sounding voice as a dot, not one |
| **Modulation target ring** | as in `fabfilter/family.md` §7.6: a static **range band** in the source colour, **one dot per voice** (up to 16; beyond that, a filled band showing min–max), striped when several sources mix | the range is labelled in target units on hover and in the popup ("±1.5 oct"); poly and mono are told apart by the dots (per voice) versus a single marker (mono), not only by hue |
| **Matrix view** | a table with one row per connection: source (tag + colour) → target (device · parameter) · **depth in target units** · curve · bipolar · via (another source scaling the depth) · per-voice or global · mute · delete | sortable and filterable by source, target and section; typing into Depth accepts `±1.5 oct`, `6 dB`, `50%`; shows connections from host-level modulators too (§6.6) |
| **Keyboard** | the full width, with an octave strip; a **range band** (dim outside the playable range), **zones or samples** as coloured bars above the keys, **key switches** in their own hue with a glyph, **root keys** marked, **held notes** filled, **released voices** fading, **MPE per held note** (a pitch offset line, a pressure bar, a slide dot), the **project scale** as marked keys | pitch and mod wheel strips at the left; octave ±; hold/latch; clicking plays with velocity by vertical position; the clip's notes can be shown as ghost marks when "follow clip" is on |
| **Oscillator / source panel** | waveform or wavetable display (animated), 3-D stack optional; engine picker; the main knobs in a row with printed values | the display is draggable where it means something (wavetable position, start point) |
| **Filter panel** | response curve (with a modulated "ghost" curve showing the modulation range), cutoff and resonance on the curve itself, a type picker | the curve is the editor, as in the EQ (`pro-q.md`); the ghost uses the source colour at low alpha |
| **Voice section** | poly/mono/legato, voices, glide time and mode, unison (voices, detune, spread, blend), pitch-bend range, MPE on/off and its bend range | the same component in every instrument |
| **Zone / sample map** (samplers, drums) | a key × velocity grid with zones as rectangles, waveform with start, loop and slice markers, root key | drag the edges; slices named by their note (as Bitwig 6.1 does) |
| **Pad grid** (drums) | 4×4 pads with name, colour, choke-group badge, output badge, a mini waveform or synth icon | follows Pulsus's existing one-pad-at-a-time editor (`views/inspector.rs::pulsus`) |
| **Play page picture** | one **meaningful** live picture: the combined output oscilloscope or spectrum, or the main oscillator's shape | never purely decorative (unlike Pigments' visualiser); it pauses when the window is hidden, and "reduce motion" freezes it |
| **Preset browser** | the host library (FabFilter §7.2) with instrument facets: **Type** (Bass, Lead, Keys, Pad, Pluck, Drums, FX, Sequence, Texture), **Character** (Bright, Dark, Warm, Aggressive, Clean, Evolving, Lo-fi…), Author, Pack, **Uses** (MPE, mod wheel, aftertouch) | **audition plays the selected clip on this track** (or a chosen phrase) through the preset; ▲▼ preview without committing; **Similar** (from stored audio features) and a **map view** like XO's Space; favourites, ratings (5 steps, ≥ 3:1 dots), recent history, **used in this project** |
| **Part lock** | a lock toggle on each section (oscillators, filter, modulation, effects, macros) | locked sections survive preset browsing (Omnisphere's Sound Lock, FLEX's lock) |
| **Help strip** | as in `fabfilter/family.md` §7.2: *what the control does · its gestures · its modulation or automation state* | for a modulated control it lists the sources ("LFO 2 ±1.5 oct, Macro 3 +2 oct") |

### 6.4 Interaction rules for instruments

Everything in `fabfilter/family.md` §7.3 applies (200 px linear drag, Shift fine, double-click reset,
right-click entry, parser, keyboard focus, one gesture = one undo step). In addition:

| Action | Rule |
|---|---|
| Connect modulation | drag a source's handle onto a control; **every valid target is highlighted and the others dimmed** during the drag; **hovering a target previews the modulation live** (Vital) at the default depth; release to commit (one undo step); Esc cancels |
| Set the depth | drag the ring outward from the knob, or wheel on the ring; the popup shows the depth in target units; Alt-drag moves every connection of that source together |
| Connect without a mouse | right-click → "Modulate by…" → pick a source; or in the Matrix view, "+" row |
| Remove | Alt-click the ring (bypass), Delete in the Matrix view, or drag the ring back to zero |
| Audition preset | Space previews with the track's clip (or the phrase); ▲▼ steps and plays; Enter commits; Esc restores the previous preset exactly (undo not touched) |
| Play the on-screen keyboard | click plays at the vertical-position velocity; Shift-click latches; the computer keyboard plays when the window has focus and "typing keyboard" is on (the existing Cantus note keys) |
| Macro edit | double-click the name to rename; right-click → "Show targets", "Clear", "Save macro state as variation" |
| Voice readout | click → a popover listing the sounding voices (note, velocity, envelope stage, age), for debugging hung or stolen notes |
| Views | Ctrl+1/2/3 for Play, Edit and Matrix; the view is stored per instrument instance |

### 6.5 Visual rules for instruments

Extending `fabfilter/family.md` §7.4:

1. **V1: fix the knob before instruments ship.** The knob's edge or track must reach ≥ 3:1 against its
   surface in all three themes (today 1.1–2.0:1, §4.14). Either draw the track in `text_dim` at reduced
   width, or give the body a `grid_bar`-darker/lighter edge chosen by a contrast check, as `theme::recess`
   already picks a colour by luminance.
2. **V2: idle states still pass.** Unused sources, empty macros, idle modulator names and rating dots use
   `text_dim`, which must pass 4.5:1. Status is shown with an icon or fill, never by dimming below AA.
3. **V3: colour by instance, with tags.** Each source instance gets a colour from the colour-blind-checked
   rotation and a short tag. Source families may share a hue *family* (Pigments' idea), but instances must
   differ in lightness and carry the tag.
4. **V4: one hue per instrument** (its name, value arcs, main curve), from the same validated set as the
   effects. Firmus and Pulsus get theirs now.
5. **V5: minimum sizes.** Labels ≥ 11 px, values ≥ 12 px at 100 %; keyboard keys ≥ 12 px wide in the
   editor and ≥ 8 px in the inspector strip; knobs S/M/L/XL = 28/36/56/80 px.
6. **V6: no decorative motion without a switch.** The Play picture must mean something. Reduced motion
   stops the tiles' animation (they show a static shape) but keeps the voice dots, which are information.
7. **V7: per-voice dots are drawn in the source colour with a 1 px contrast outline** in `ink`, so they
   read over any arc or theme.

### 6.6 The modulation model shared by all instruments

This builds on `fabfilter/family.md` §7.6 (sources as track-level objects, drag to connect, rings, slot
rows, the matrix, macros-only view) and makes it **polyphonic**:

- **Two scopes of source:**
  - **Voice sources:** per-note envelopes, per-voice LFOs, velocity, key, release velocity, per-note MPE
    (bend, pressure, timbre/slide), per-voice random. They have one value per voice and can only drive
    parameters that are evaluated per voice.
  - **Global sources:** macros, global LFOs, mod wheel, channel pressure, host modulators, envelope
    followers on other tracks, automation. They have one value.
- **Parameters declare what they accept:** global only (for example a reverb mix inside the instrument)
  or per voice (cutoff, pitch, pan, level, wavetable position). Dragging a voice source onto a global-only
  parameter is not offered, so that target is not highlighted.
- **A connection is:** source (instance id), target (parameter id), depth (in target units, stored
  normalised), curve (linear, exp, log, S, steps), bipolar or unipolar, optional "via" source scaling the
  depth, mute. It is **project data with undo**, the same for native instruments and host modulators.
- **One evaluation order:** base value + automation → global modulation → per-voice modulation → clamp,
  with the ring showing the result of each stage (band = range, dots = voices).
- **Host-level sources reach instrument parameters** (and effect and mixer parameters) through the same
  connections. An LFO on the track can drive the synth's cutoff and the delay's feedback at once. Per-voice
  sources stay inside the instrument.
- **The same model maps onto CLAP's polyphonic modulation** (per-note modulation of any parameter flagged
  as per-note), so a hosted CLAP instrument's parameters can be targets of Cantus voice sources, and our
  instruments can export their per-voice targets (§6.8).

### 6.7 What native instruments can do that plug-ins cannot

| # | Native advantage | The plug-in's workaround or gap |
|---|---|---|
| 1 | **Audition presets with the user's own part.** The browser plays the selected clip, or the clip under the playhead, through each preset as you step. | Demo phrases or the keyboard only |
| 2 | **The project key on every keyboard and arpeggiator.** One key and scale (Bitwig 6 added a global key; Serum 2 has its own per-instance KEY/SCALE) drives keyboard marks, scale-quantised modulation and arps. | Set again in each plug-in |
| 3 | **Note expressions drawn in the piano roll** reach the instrument's per-voice sources directly: bend, pressure and timbre curves per note, with undo. | Depends on the host's MPE or note-expression support and the format |
| 4 | **Modulation from other tracks without routing:** an envelope follower on the kick drives the pad's cutoff. | Sidechain routing, per-host MIDI and sidechain workarounds |
| 5 | **Exact voice and CPU telemetry.** The engine counts each instrument's time on the audio thread and its voices, stolen notes and disk streams. | A plug-in-side CPU guess, or nothing |
| 6 | **One undo, one learn, one preset library** across instruments, effects and chains. Presets for *track templates* (instrument + chain + macros) as one item. | Per-plug-in undo, learn and folders |
| 7 | **"Presets used in this project" and preset diffs** ("what did I change from the factory sound?"), because every parameter change is a command. | Not possible |
| 8 | **Resample to a pad or sampler in one step:** render a note or phrase of any instrument into Pulsus or the sampler, with the note mapped. | Bounce, import, map by hand |
| 9 | **Themes, localisation (en/es) and accessibility** from the app: every instrument parameter gets an AccessKit name, value and unit through egui, plus keyboard focus. | Dark-only, English-only, invisible to screen readers (except Surge XT) |
| 10 | **The AI bridge can set and explain instruments** with the same commands a user issues ("make the pad darker" → cutoff −1 oct as one undo step). | FL's Gopher is the only DAW assistant that acts, and only on FL's own functions |
| 11 | **No version drift or licensing.** Every instrument updates with the app; there is nothing to authorise. | iLok, NI Access, and three visual generations inside one bundle |
| 12 | **Per-note automation lanes and sample-accurate modulation**, because instrument and sequencer share the note ids the engine already carries (see 6.8). | Depends on the format and host |

### 6.8 The instrument contract, from the family's side (note for the `cantus-host` session)

A mid-task message asked for the native instrument contract. That message looked addressed to the
standards report, and [standards.md](standards.md) is being written separately, so it is left untouched.
This subsection records only what the family rules above **require** of the audio-thread contract, read
against `cantus-host/src/backend.rs` at snapshot `d196ba5` and against `cantus-engine/src/processor.rs`.

**What the engine already has.** `TrackSlot::note_on(pitch, velocity: u8, tag: u64)` fires notes between
segments that are split at event times, and passes a **stable note tag** (the clip note's id). The tag is
exactly what per-voice modulation, per-note expressions and voice telemetry need. Keep it.

**Where `cantus-host`'s `NoteEvent { offset, channel, key, velocity }` falls short for instruments** (it is
fine for the effects it was built for):

| Missing | Needed for (family rule) | CLAP | VST3 |
|---|---|---|---|
| **Note id** | per-voice dots (6.3), per-note expressions (6.7 #3), voice telemetry (6.3 footer), ending one of two overlapping notes on the same key | `note_id` on NOTE_ON/OFF/CHOKE/END | `noteId` on NoteOn/NoteOff |
| **Note-off as its own event, with release velocity** | release-velocity sources (6.6) | NOTE_OFF with velocity | NoteOffEvent velocity |
| **Choke** | drum choke groups (Pulsus) | NOTE_CHOKE | — |
| **Note end reported by the instrument** | freeing per-voice modulation and fading keyboard marks exactly when the tail ends | NOTE_END (plug-in → host) | — |
| **Per-note expressions** (volume, pan, tuning, vibrato, brightness/timbre, pressure, expression) | the MPE keyboard (6.3), note expressions from the piano roll | 7 note-expression ids | NoteExpressionValueEvent with the standard type ids; PolyPressureEvent |
| **Channel controls**: pitch bend, mod wheel, **sustain (CC64)**, channel pressure, program change | wheels on the keyboard, sustain for pianos (a basic need for keys), mod-wheel sources | MIDI or param events | no raw CC: mapped to parameters by the plug-in's **IMidiMapping** |
| **Per-voice parameter modulation** | voice sources driving parameters (6.6) | PARAM_MOD with note_id / key / channel targeting; parameter flags per-note / per-key / per-channel | none (note expressions only) |
| **Voice info** | the "7 / 16" footer and the steal count | voice-info extension (count, capacity, overlapping notes) | none |
| **Transport** (tempo, beat position, playing, time signature) | synced LFOs, arps, step sequencers | the transport event | ProcessContext |
| **More than one stereo output** | Pulsus per-pad outputs to mixer strips; multi-out samplers | audio-ports extension | multiple output buses |

**Suggested shape of the native trait** (a sketch for the owning session to accept or reject; the names
are illustrative):

```rust
pub struct NoteId(pub u32);

pub enum NoteEvent {
    On { offset: u32, id: NoteId, key: u8, channel: u8, velocity: f32, tuning_cents: f32 },
    Off { offset: u32, id: NoteId, velocity: f32 },
    Choke { offset: u32, id: NoteId },
    Expression { offset: u32, id: NoteId, kind: Expr, value: f32 }, // per note
}
pub enum Expr { Volume, Pan, Tuning, Vibrato, Timbre, Pressure, Expression }
pub enum ChannelEvent { Bend { offset: u32, semitones: f32 }, Controller { offset: u32, cc: u8, value: f32 },
                        Pressure { offset: u32, value: f32 }, Program { offset: u32, program: u8 } }
pub enum ParamEvent {
    Set { offset: u32, id: u32, value: f64 },                     // automation, gestures
    Mod { offset: u32, id: u32, voice: Option<NoteId>, amount: f64 }, // global or per-voice
}
pub struct Transport { pub tempo: f64, pub beat: f64, pub playing: bool, pub sig: (u8, u8) }

pub trait Instrument: Send {
    fn process(&mut self, t: &Transport, notes: &[NoteEvent], channel: &[ChannelEvent],
               params: &[ParamEvent], outs: &mut [StereoOut<'_>], sink: &mut InstrumentOut);
    fn reset(&mut self);           // transport jump, panic: voices and tails cleared
    fn latency(&self) -> u32;      // samples
    fn tail(&self) -> Tail;        // None / Samples(n) / Infinite
}
pub struct InstrumentOut { /* fixed-capacity, no allocation */
    // NoteEnded(NoteId), gestures (ParamOut), voice telemetry: active, capacity, stolen,
    // and per voice: (NoteId, key, amp stage, value of each per-voice modulated param) for the UI dots
}
```

**Why this shape:**
- **(a) Hosted instruments adapt to it.** CLAP maps almost one to one. VST3 maps notes, ids and
  expressions; channel controllers go through the plug-in's IMidiMapping; per-voice `ParamEvent::Mod` has
  no VST3 equivalent and is dropped with a visible "not supported" on the ring.
- **(b) Our instruments export without loss to CLAP** through `cantus-plugins`: note ids, expressions,
  polyphonic modulation, NOTE_END and voice info are all CLAP features. VST3 export keeps everything except
  per-voice modulation of arbitrary parameters, which can be offered as custom note expressions.
- **(c) Per-voice UI dots come for free.** The telemetry the UI draws is produced where the voices live, on
  the audio thread, in a fixed-size outbox, like the existing `Meters` and `ParamOut`.

**One disagreement with the current contract:** `NoteEvent::velocity == 0` meaning note-off is the MIDI
1.0 convention. It cannot carry a note id to end, a release velocity or a choke, and CLAP and VST3 both have
distinct note-off events. For instruments, `cantus-host` should adopt distinct On/Off/Choke events with ids
before the first hosted instrument ships. Changing it later breaks every backend.

### 6.9 ASCII sketch: the common instrument frame

Editor window, Play view:

```
┌───────────────────────────────────────────────────────────────────────────────────────────────┐
│ ■ Lead Vox · FIRMUS  ↶ ↷  A|B →  ‹ ★ Glass Pluck ● ›  [Play|Edit|Matrix]  MIDI·MPE  ⏻ ? ⋯ ⛶ │ header 32
├───────────────────────────────────────────────────────────────────────────────────────────────┤
│  GLASS PLUCK                         ┌──────────────────────────────┐   ◯ Tone     ◯ Space   │
│  Pluck · Bright · by Cantus          │   output scope / main osc    │   +1.2 oct   38 %      │
│  "Mod wheel opens the filter;        │   (live, meaningful)         │   ◯ Motion   ◯ Drive   │
│   aftertouch adds vibrato"           └──────────────────────────────┘   0.40       −3 dB     │
│  key params:  Cutoff 2.4 kHz   Reso 0.30   Attack 4 ms   Release 280 ms   Glide off           │ printed values
├───────────────────────────────────────────────────────────────────────────────────────────────┤
│ MACROS  ◯Tone ◯Space ◯Motion ◯Drive ◯Width ◯Env ◯Pitch ◯(drag here)   each: value · 1st target│ macro bar
├───────────────────────────────────────────────────────────────────────────────────────────────┤
│ ⟨PB⟩⟨MW⟩ │▌█▌▌█▌▐█▌█▌▌█▌▐█▌█▌▌█▌▐█▌█▌▌█▌ … │ scale marks · held notes · MPE bars · zones above │ keyboard
├───────────────────────────────────────────────────────────────────────────────────────────────┤
│ ? Tone: filter cutoff and brightness · drag, Shift fine, dbl-click reset · LFO 2 ±0.5 oct     │ help strip
│ Voices 7/16 · CPU 3.1 % · Latency 0 · Poly · Glide off · Out −6.2 dB ▮▮▮▯        ⟲ grip       │ footer 28
└───────────────────────────────────────────────────────────────────────────────────────────────┘
```

Editor window, Edit view (the same header, help strip and footer):

```
┌─ header ───────────────────────────────────────────────────────────────────────────────────────┐
│ ┌OSC 1 ▾ wavetable ┐┌OSC 2 ▾ VA ────┐┌SUB┐┌NOISE┐ ┌FILTER 1 ▾ ladder ─┐┌FILTER 2┐ ┌VOICE ─────┐│
│ │ ~~wave display~~ ││ ~~wave~~       ││   ││     │ │  curve + ghost     ││        │ │ Poly 16   ││
│ │ ◯Pos ◯Uni ◯Det   ││ ◯PW ◯Uni ◯Det  ││ ◯ ││ ◯   │ │ ◯Cut ◯Res ◯Drive   ││ ...    │ │ Glide ... ││
│ └──────────────────┘└────────────────┘└───┘└─────┘ └────────────────────┘└────────┘ └───────────┘│
├ MOD ─────────────────────────────────────────────────────────────────────────────────────────────┤
│ [ENV1 ⁄‾\ 3] [ENV2 ⁄\_ 1] [LFO1 ∿ 2] [LFO2 ⊓ 0] [RND1 ⌇ 1] [VEL ▮ 1] [PB] [PRESS] [SLIDE] [+] │ tiles: tag·shape·count
│ ┌ selected: LFO 1 ───────────────────────────────────────────────────────────────────────────┐ │
│ │ editable shape with slope handles · rate 1/8 sync · retrig · phase · one dot per voice      │ │
│ └─────────────────────────────────────────────────────────────────────────────────────────────┘ │
├ MACROS ─ … ─────────────────────────────────────────────────────────────────────────────────────┤
├ KEYBOARD ─ … ───────────────────────────────────────────────────────────────────────────────────┤
└─ help strip · footer ───────────────────────────────────────────────────────────────────────────┘
```

Inspector strip (always visible when the track is selected):

```
┌ FIRMUS · Glass Pluck ● ‹ › ★   [Edit ⛶] ┐
│ ◯Tone ◯Space ◯Motion ◯Drive              │ 4 of 8 macros, values printed
│ Cutoff 2.4k  Reso .30  Att 4ms  Rel 280ms │ key parameters
│ ▌█▌▌█▌▐█▌█▌▌█▌ (compact keys, held notes) │
│ Voices 7/16 · CPU 3.1% · Out −6.2 dB      │
└──────────────────────────────────────────┘
```

---

## 7. The bundle Cantus should ship at 1.0

In priority order. Each item names the stock instruments it must match (§2–3) and the sibling report with
the engine detail. "Clears" means the stock instruments it replaces in the matrix of §2.2.

| # | Instrument | Scope at 1.0 | Clears | Detail |
|---|---|---|---|---|
| 1 | **Sampler family: one engine, three faces** | **Quick** (one sample, slice by onset, beat or grid, time-stretch, reverse, loop); **Multi** (key × velocity zones, round robin, SFZ/SF2/Decent Sampler import); **Granular/Spectral** modes (grains, freeze, formant-aware stretch). Modulation matrix, filter, 3 envelopes, 2 LFOs. | Simpler/Sampler, Quick Sampler/Sampler, Slicex/DirectWave/Fruity Slicer 2, Bitwig 6.1 Sampler, Sample One XT, Granulator III, Padshop | [sampler.md](sampler.md) |
| 2 | **Pulsus 2, the drum machine** | 16+ pads; each a synthesised voice (808/909-class kick, snare, hat, clap, tom and cymbal models) or a sample layer; chokes; per-pad chain and **per-pad outputs** to mixer strips; pattern lane in the channel rack (exists). | Drum Rack + Drum Synths, FPC + Drumaxx, Drum Machine Designer + Drum Synth, Bitwig Drum Machine + E-drums, Impact XT, Groove Agent SE | [drums.md](drums.md) |
| 3 | **Flagship hybrid synth** (Firmus grows into it, or a new engine beside it) | 3 oscillators (wavetable, VA, sample) + sub + noise; FM/PM/RM between oscillators; 2 filters (ladder, SVF, comb, formant) in series/parallel; 3+ envelopes, 4+ LFOs/MSEG, random; the modulation model of §6.6; unison; 16+ voices; MPE. | Wavetable, Polymer, Retrologue, Mai Tai, Alchemy's VA/wavetable side, Kepler Exo | [wavetable.md](wavetable.md), [analog.md](analog.md) |
| 4 | **Keys: modelled piano, EP and organ** | A physically modelled or hybrid acoustic piano with a small footprint (Pianoteq's approach), a modelled tine/reed EP, and a tonewheel organ with drawbars and rotary. | Studio Piano, Vintage EP/B3, Electric, FL Keys, Bitwig Organ | [keys.md](keys.md) |
| 5 | **Bass** | An 808/sub bass voice (glide, drive, pitch envelope), an acid mono synth with a 303-style sequencer, and a modelled or sampled electric bass with articulations. | Transistor Bass, Studio Bass, Mojito, Bass (M4L) | [bass.md](bass.md) |
| 6 | **FM synth** | 4–6 operators, an algorithm picker *and* a free matrix drawn as a grid (Sytrus's clarity), DX7 patch import, fixed/ratio frequencies, per-operator envelopes. | Operator, Sytrus, FM-4, EFM1 | [fm-additive-granular.md](fm-additive-granular.md) |
| 7 | **Physical modelling: mallets, strings, modal** | An exciter → resonator model (mallet, bow, pluck, noise into string, beam, membrane, tube) with modal banks. | Collision, Tension, Sculpture, Pigments' Modal (as a stretch goal) | [fm-additive-granular.md](fm-additive-granular.md) |
| 8 | **Players / note generators** (note effects, not engines) | Arpeggiator, chord generator, step sequencer and a pattern generator that follow the project key and the chord track. | Session Players (partly), Expressive Chords, Sting/Patterns, Cubase pattern sequencer, Chord Assistant | — |
| 9 | **Preset library** | 1,000+ factory presets across 1–7, tagged with the facets of §6.3, every one with a description that names its macros. This is a deliverable, not an afterthought: every praised stock instrument ships a big, tagged library (§3). | FLEX Core Series, Alchemy's 3,000, Live's packs | — |

**After 1.0:** a modular environment (The Grid, Max for Live), additive resynthesis (Harmor), vocal
synthesis (Omnivocal), and orchestral and acoustic libraries, which are recordings rather than engines and
out of scope per the [README](README.md).

**Why this order:**
- Samplers and drums come first because every DAW has them, users judge a beat-making DAW by them, and
  Pulsus already exists.
- The hybrid synth is third because Firmus exists and needs the modulation model more than a new engine.
- Keys and bass are in every "make a song" template.
- FM and physical modelling are what separate Live, Logic and FL from the rest.
- The players turn the bundle from a toolbox into a band.

**The UI prerequisites, before instrument #1:**
1. Fix the knob contrast (§6.5 V1).
2. Build the shared **instrument frame**: header with view switch, macro bar, modulation strip, keyboard,
   help strip and footer, on top of `views/fx/mod.rs`'s window.
3. Build the **modulation service** with per-voice telemetry.
4. Build the **host preset library** with audition-with-clip.

The same order was suggested for effects in `fabfilter/family.md` §7.7.

---

## 8. Sources

**DAW vendors: editions, versions, manuals**
- FL Studio 2026, what's new: https://www.image-line.com/fl-studio/release/2026
- FL Studio editions (generators per edition): https://www.image-line.com/fl-studio/compare-editions
- FL Studio manual, FLEX: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/FLEX.htm
- FL Studio manual, Sytrus: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/Sytrus.htm
- FL Studio manual, Harmor: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/Harmor.htm
- Ableton Live editions: https://www.ableton.com/en/live/compare-editions/ ; prices: https://www.ableton.com/en/shop/live/
- Ableton Live 12 instrument reference: https://www.ableton.com/en/manual/live-instrument-reference/
- Ableton Live 12 release notes: https://www.ableton.com/en/release-notes/live-12/
- Ableton blog, Live 12.3: https://www.ableton.com/en/blog/live-12-3-is-here/
- Apple Logic Pro page and images: https://www.apple.com/logic-pro/
- Bitwig overview and editions: https://www.bitwig.com/overview/ ; what's new: https://www.bitwig.com/whats-new/
- Bitwig user guide, unified modulation system: https://www.bitwig.com/userguide/latest/the_unified_modulation_system
- Bitwig user guide, devices: https://www.bitwig.com/userguide/latest/introduction_to_devices
- Steinberg Retrologue page (image only): https://www.steinberg.net/vst-instruments/retrologue/
- REAPER (Wikipedia, version and built-ins): https://en.wikipedia.org/wiki/REAPER
- Fender Studio Pro (Wikipedia, release date): https://en.wikipedia.org/wiki/Studio_Pro

**Instrument vendors**
- Xfer Serum 2: https://xferrecords.com/products/serum-2
- Vital: https://vital.audio/ ; videos `https://vital.audio/videos/full_screen.mp4`, `https://vital.audio/videos/drag.mp4`
- Vital source (preset browser, skin tokens): https://github.com/mtytel/vital (`src/interface/editor_sections/preset_browser.cpp`, `src/common/load_save.h`, `src/interface/look_and_feel/skin.h`)
- Kilohearts Phase Plant: https://kilohearts.com/products/phase_plant ; manual https://kilohearts.com/docs/phase_plant
- Arturia Pigments: https://www.arturia.com/products/software-instruments/pigments/overview (gallery images under `medias.arturia.net/images/products/pigments/`)
- Spectrasonics Omnisphere 3: https://www.spectrasonics.net/products/omnisphere/ (interface tour images on `cdn.spectrasonics.net`)
- Native Instruments Kontakt 8: https://www.native-instruments.com/en/products/komplete/samplers/kontakt-8/
- XLN XO: https://www.xlnaudio.com/products/xo
- Modartt Pianoteq: https://www.modartt.com/pianoteq ; https://www.modartt.com/pianoteq_overview
- u-he Diva: https://u-he.com/products/diva/
- Surge XT manual: https://surge-synthesizer.github.io/manual-xt/

**Reviews and press**
- Sound On Sound, Ableton Live 12: https://www.soundonsound.com/reviews/ableton-live-12
- MusicRadar, Ableton Live 12 Suite: https://www.musicradar.com/reviews/ableton-live-12-suite-review
- Sound On Sound, Fender Studio Pro 8: https://www.soundonsound.com/reviews/fender-studio-pro-8
- Production Expert, Fender Studio Pro 8 first look: https://www.production-expert.com/production-expert-1/fender-studio-pro-8-first-look
- Sound On Sound, Omnisphere 3: https://www.soundonsound.com/reviews/spectrasonics-omnisphere-3
- Sound On Sound, Kontakt 8: https://www.soundonsound.com/reviews/native-instruments-kontakt-8
- Sound On Sound, Pigments 6: https://www.soundonsound.com/reviews/arturia-pigments-6
- Sound On Sound, Pianoteq 9: https://www.soundonsound.com/reviews/modartt-pianoteq-9
- Sound On Sound, Logic Pro Synth Players: https://www.soundonsound.com/techniques/logic-pro-synth-players
- MusicTech, Logic Pro 11 review: https://musictech.com/reviews/digital-audio-workstations/logic-pro-11-review/
- Synth Anatomy, Logic Pro 12 / 12.3: https://synthanatomy.com/2026/06/apple-logic-pro-12.html
- Synth Anatomy, Bitwig Studio 6.1: https://synthanatomy.com/2026/08/bitwig-studio-6.html
- gearnews, Bitwig Studio 6.1: https://www.gearnews.com/bitwig-studio-6-studio/
- Synth Anatomy, Live 12.4: https://synthanatomy.com/2026/04/ableton-live-12-stay-creative-and-in-focus-an-overview-of-the-new-features.html
- Synth Anatomy, Cubase 15: https://synthanatomy.com/2025/11/steinberg-cubase-15-daw-receives-a-major-update-with-new-features.html
- Attack Magazine, Cubase 15: https://www.attackmagazine.com/news/cubase-15-released-heres-whats-new/
- gearnews, best DAW stock synths (2024): https://www.gearnews.com/best-daw-stock-synths/
- Search excerpts only **[excerpt]** (pages not read in full): Audeobox Logic stock plug-ins guide
  (https://www.audeobox.com/learn/logic-pro/logic-pro-stock-plugins-guide/), Ben Rainey on house-music
  plug-ins (https://www.benrainey.co.uk/blog/best-plugins-for-house-music), beatstorapon's DAW comparison
  (https://beatstorapon.com/blog/best-daw-2026-comparison/), and a Logic 11 user write-up
  (https://beesbuzz.biz/blog/3645-Logic-11-a-quick-play-around).

**Not reachable:** Sound On Sound's Serum 2 review (HTTP 410 at the guessed URLs); NI's accessibility page
(404); Vital's tutorials page (404). The web-search budget for the session ran out part-way, so the Serum 2
browser details, Logic's VoiceOver support and Kontakt's key-switch colours are stated from the screenshots
or marked unverified.

**Measurements:** the colour samples and contrast ratios in §4.14 were computed with Pillow from the
screenshots listed in §1, using the WCAG 2.x relative-luminance formula. For text, the ink colour is the
mean of the 4 % of pixels furthest in luminance from the box's modal colour. Cantus's own ratios were
computed from the token values in `crates/cantus-ui/src/theme.rs`.
