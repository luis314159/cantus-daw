# Drum instruments: competitive research for Pulsus II

*Researched 2026-09-24 for Cantus. Sources are the makers' product pages, online manuals and release notes (Native
Instruments, XLN Audio, Toontrack, Sonic Charge, Sonic Academy, D16, Roland, Ableton, Image-Line), press reviews
(Sound on Sound, MusicRadar, MusicTech, CDM) and a few forum threads. Everything is paraphrased. Where a figure comes
only from a search-engine excerpt, or from the literature without the paper being re-read here, the text says so.
Gearspace and ResearchGate refused automated reads (HTTP 403) and were not worked around. Screenshots are the makers'
own, kept in `screenshots/` (git-ignored) for internal reference only. Colour values were measured
from those screenshots with a script (JPEG and resampled PNG, so ±2 per channel).*

*The Cantus side was read from `crates/cantus-dsp/src/pulsus.rs`, `crates/cantus-core/src/pads.rs`,
`crates/cantus-ui/src/views/channel_rack.rs`, `docs/STATUS.md` (Round 12, the drums plan) and ADRs 0013–0017.*

---

## 0. The short version

1. **The market splits into three kinds of drum instrument, and nobody owns more than one.** *Samplers with a pad
   grid* (Battery 4, FPC, Drum Rack, Maschine, XO) are about layers, round-robin and routing. *Recorded acoustic
   kits* (Superior Drummer 3, EZdrummer 3, Addictive Drums 2) are about mic bleed, articulations and groove
   libraries. *Drum synthesizers* (Microtonic, Kick 3, PunchBox 2, Nepheton 2, Drumazon 2, Roland's TR-808/909)
   are about tuning, envelopes and circuit character. Pulsus already mixes synthesized and sampled pads in one
   kit. That is the rarest of the three combinations, and it should be kept.
2. **Per-pad multi-out is universal, and it is always a chore.** Superior Drummer 3 has 16 stereo outputs and
   32 internal buses. XO has 8 outputs, Battery has 4 buses plus host outputs, and FPC offsets pads onto mixer
   tracks. In a host, every one of those needs setting up on the DAW side. In Cantus a pad can **be** a mixer
   strip with one click, and the kit stays one clip and one note list. This is our biggest structural win.
3. **The best interface idea in the category is XO's map**, not a better pad grid. It places thousands of
   one-shots by sound similarity, with a "similar samples" list and a Playground for trying beats and sounds
   without committing to them. Algonaut Atlas 2 and Live 12's "swap similar" followed it. XLN has never
   published how the map is built. Google's 2016 *Infinite Drum Machine* did the same thing with t-SNE on audio
   features.
4. **The DSP that sells is small and well understood.** The ingredients are:
   - a two-stage pitch envelope and a click for kicks;
   - two tuned bodies plus noise that decays on its own for snares;
   - six detuned squares through band-pass and high-pass filters for 808 metal;
   - modal resonator banks for toms and bells.

   Roland's own 909 plays **6-bit samples** for its hats and cymbals. The makers win on **controls and display**:
   Kick 3 draws the pitch curve over the rendered waveform, and Microtonic has one morph slider for the whole kit.
5. **Articulation and feel are where the recorded kits lead.** The hi-hat opens continuously with MIDI CC4.
   Layer limits and "smoothing" stop fast repeats from sounding like a machine gun. Battery has ten articulation
   generators (flam, drag, ruff, roll). There are groove libraries you can search by tapping (Tap2Find), and
   Bandmate suggests grooves that fit your audio. Pulsus has none of this yet.
6. **Legibility is an open goal again.** All but one of the products are dark. Battery's pad cells stand out from
   the grid by only **1.16:1**, and its kick-cell label is **4.2:1**. Kick 3's knob bodies are **1.24:1**, and
   dimmed rows in Superior Drummer 3 fall to **2.6–3.4:1**. The light Microtonic reads better than all of them.
7. **Pulsus II should close the parity gaps first, then use the host.** The parity gaps:
   - sample layers, round-robin, velocity response;
   - a full per-pad envelope, filter and transient section;
   - the hat pedal (CC4);
   - more synth models: 808/909 styles, clap, modal tom, cymbal, cowbell;
   - a pad grid with waveforms.

   Then the host-native wins:
   - pads as strips;
   - grooves as clips;
   - one undo history;
   - a similarity browser shared with the sampler;
   - the AI bridge building a kit from a description.
8. **One reproducibility bug found in today's Pulsus:** the noise seed counts hits *since the last reset*, and a
   seek resets it. A section exported from bar 5 therefore does not match the same bars of a full export.
   Round-robin would inherit the same flaw. Section 9 gives the fix: derive the seed from the note's position,
   not from a running counter.

---

## 1. Products and versions

| Product | Latest version and date | Price | Formats | Source |
|---|---|---|---|---|
| **NI Battery 4** | 4.3.1 (KVR listing; NI does not publish the date) | USD 199 (update 99) | VST2, VST3, AU, AAX; macOS 13–15, Win 10/11 | NI product page; KVR |
| **XLN XO** | 1.8.10, 7 Aug 2026 (1.8.5 on 15 May 2026 added double-click reset) | EUR 149; EUR 9.99/month rent-to-own | VST, AU, AAX, standalone | XLN release notes |
| **Toontrack Superior Drummer 3** | 3.4.4, 26 Mar 2026 (3.4.0 on 22 Apr 2025 was the last major one) | EUR 399 (crossgrade 239) | VST, VST3, AU, AAX, standalone | Toontrack product page and release notes |
| **Toontrack EZdrummer 3** | 3.1.2, 11 Nov 2025 | EUR 179 (upgrade 99) | VST3, AU, AAX, standalone | Toontrack |
| **XLN Addictive Drums 2** | 2.9.1, 28 Apr 2026 (2.6 added snapshots and the audio recorder) | from EUR 179 (Custom collection) | VST, AU, AAX, standalone | XLN release notes and shop |
| **Image-Line FPC** | ships with FL Studio 2025 (selected pad now outlined) | bundled | FL only | FL manual; FL 2025 beta notes |
| **Ableton Drum Rack + Drum Sampler** | Live 12.4 (Apr 2026); Drum Sampler since 12.1; 12.3 added a Rhythm Generator layout for Push | bundled | Live only | Ableton manual, blog, release notes |
| **NI Maschine 3** | 3.7, **24 Sep 2026** (today): random note generator, Euclidean sequencer, per-note chance | USD 109 | plug-in + standalone | NI; Sonicstate |
| **Sonic Charge Microtonic** | 3.x (the current guide is 3.3.4) | EUR/USD 99 | VST2.4, VST3, AU; standalone on Mac only | soniccharge.com; manual mirror |
| **Sonic Academy Kick 3** | 1.1.6 (demo); reviewed Jan 2025 | GBP 79.95 (upgrade from Kick 2: 26.96) | plug-in | Sonic Academy; MusicTech |
| **D16 PunchBox 2** | 2.1.0; released **June 2026** | EUR 99 (intro 79) | VST2, VST3, AU, AAX | d16.pl; CDM (29 Jun 2026) |
| **D16 Nepheton 2** (808) / **Drumazon 2** (909) | 2.0.3 / 2.x (Drumazon 2 released Sep 2023) | EUR 119 each | VST2, VST3, AU, AAX | d16.pl; KVR; Synth Anatomy |
| **Roland Cloud TR-808 / TR-909** | Roland Cloud software | TR-909 lifetime key USD 149 (49 on promotion to 9 Oct 2025); or membership | VST3, AU, AAX | roland.com; BPB |
| Algonaut **Atlas 2** | 2.x, reviewed 2025–26 | not checked | plug-in + standalone | SOS review; Algonaut manual |
| Audialab **Emergent Drums 2** | 2.x | not checked | plug-in | Audialab; NI NKS page |

Also rated in 2026 round-ups (from search excerpts only):
- **Xfer Nerve**, **Sugar Bytes DrumComputer**, **Steven Slate Drums 5.5** (free) and **MT Power Drum Kit 2** (free).
- One round-up calls Drumazon 2 "flawless" as a 909.
- Magnetic Magazine's April 2026 piece groups "drum generators" (Emergent Drums and the like) as a category of its own.

---

## 2. Screenshots saved

All are official images, downloaded with `curl` from the makers' sites. Each was checked as a real PNG or JPEG and
opened for analysis.

| File | What it shows |
|---|---|
| `dr-01-battery4-overview.png` | Battery 4 main view (5313×2736, annotated by NI):<br>- header;<br>- library browser on the left;<br>- a 4×4 slice of the cell matrix with colour-tinted cells that show **only a name** (no waveform, no level);<br>- the quick-access waveform in red with S/E markers;<br>- the bottom modules: Volume Envelope (A/H/D), Pitch Envelope (Amount/Decay), Velocity (To Volume/To Pitch), Engine (Sampler: Standard/Vintage/DFD), Filter (High/Low Cut as one bar), a one-knob Compressor, and Sends;<br>- page tabs Main · Effects · Modulation · Setup · Editor · Master. |
| `dr-02-battery4-setup-page.png` | The Setup page:<br>- MIDI Input: a velocity curve with 3 handles, All Ch, Key Track, Note Latch;<br>- Voice Groups: Kit, Voices 32, Fade 10 ms, Mode Oldest, Excl Off;<br>- Cell Activation: Trigger Note On, Condition Always;<br>- Articulation: "3 Stroke Ruff" with an R L R L sticking icon, Speed, Dynamics;<br>- MIDI Echo: Tune, Gravity, 16th, Time, Feedback;<br>- Humanize: Amount, Sound, Time. |
| `dr-03-battery4-editor-mapping.png` | The Mapping Editor: a velocity (1–127) × layer grid with sample blocks, and per-layer Tune (st), Pan, Vol (dB), Root, Low Vel and High Vel. |
| `dr-04-xo-space-load-samples.png` | The XO Space:<br>- thousands of dots on near-black, coloured by drum type (red kicks, blue snares, yellow hats, green, pink, teal);<br>- the 8 channel rings down the left: FLEX 2, FLEX 1, OP HH, CL HH, CLAP, SNARE, KICK 2, KICK 1, each with M/S, lock and ◀ ▶;<br>- a mini "kit constellation" puck;<br>- OUT and CUE sliders. |
| `dr-05-xo-tweak-edit.png` | The XO sequencer close-up:<br>- **hexagon steps** coloured per lane;<br>- small dots under the steps for rolls;<br>- A/B, Follow, and Chain OFF/AB/AAAB;<br>- the **Accentuator** curve with 1/2, 1/4, 1/6, 1/8 sliders and dice;<br>- per-lane Groove (Swing16, Hickup, Vintg16) and a **NUD**ge arc;<br>- a Global groove. |
| `dr-06-xo-lockup.png` | XO composite:<br>- the preset browser;<br>- Space with a **Similarity** strip and History;<br>- the per-channel strip: Sample, Pan, Envelope, Pitch, Tone, Cut, VelSens, Playback, FX Sends, Sequencer, Groove, Nudge;<br>- the Playground's "Similar Samples" panel with Randomize. |
| `dr-07-sd3-drums-kit.jpg` | Superior Drummer 3 Drums page:<br>- a photographic kit with the selected cymbal outlined in cyan;<br>- an inspector with Level, Reverse 200 ms, MIDI Monitor;<br>- **Voice and Layer: Layer Limits Hard 4 / Gradient 2 / Soft 2, Voice Limit 8**;<br>- Soft Hit Level, **Smoothing** ("level of smoothing for fast repeating hits"), Hi-Hat CC Edit, Envelope and Offset, Velocity Gate, MIDI Mapping;<br>- the song track underneath, with coloured groove blocks (Intro, Verse, Fill, Chorus). |
| `dr-08-sd3-mixer.jpg` | SD3 Mixer:<br>- Show Channels: Mics 14 of 22, User Mics, **Buses 2 of 32**, **Outputs 1 of 16**;<br>- per-channel **Bleed Enabled / Bleed Level** rows;<br>- insert effects (EQ, Comp 76, EQ84, Classic Compressor) opened as floating panels;<br>- a Macro Controls strip. |
| `dr-09-sd3-tracker.jpg` | SD3 Tracker (audio to MIDI):<br>- a waveform with detected hits;<br>- Sound Recognition "Snare", Amount, **Reduce Bleed**, Detected Sounds 4;<br>- a "Match with Snare sound" confidence lane (0–100 %) with a threshold line;<br>- a velocity lane;<br>- Trigger, De-Trigger, Select Similar, and Nudge in ms. |
| `dr-10-sd3-grid-editor.jpg` | SD3 Grid Editor:<br>- instrument rows (Kick … Cymbal 5) with diamond notes;<br>- Quantize, Q Max, Swing, Randomize and Nudge in the toolbar;<br>- a velocity lane with Velocity, Dynamics and Randomize sliders. |
| `dr-11-microtonic-main.png` | Microtonic 3 (light grey):<br>- 8 channel buttons with note names and mutes;<br>- per-patch: mix osc↔noise, EQ freq and gain, distort, level, pan, choke, output A/B;<br>- oscillator: sine/triangle/saw, osc freq, pitch mod (decay/sine/noise icons) with amount and rate, attack/decay faders;<br>- noise: LP/BP/HP, filter freq, Q, stereo, envelope shapes, attack/decay;<br>- velocity: osc, noise, mod;<br>- patterns a–l and chain;<br>- 16 steps with trig, acc, fill and len rows;<br>- the transport has step rate, swing and fill rate;<br>- **sound morph** slider at the top. |
| `dr-12-kick3-main.png` | Kick 3:<br>- the **pitch curve (nodes, log-Hz axis 20 Hz–20 kHz) drawn over the rendered waveform**;<br>- Pitch/Amp display toggle, Gate, Snap, Tags, LOG/LIN, Simplify, Phase 0°;<br>- layer controls: Pitch, Envelope 20.0 k, Pan, Length 258 ms, Key Track / Selective / Sustain, Harmonics;<br>- five layer slots (Sub, "Melodic HT-01" click, three "Add new layer");<br>- a limiter with GR/OUT meters, Macros, and a drag-to-export clip (Key C0, Velocity 100). |
| `dr-13-nepheton2-synth.jpg` | Nepheton 2 (808):<br>- 16 voice strips with Level, Tone, Decay, **Sweep** (added to the kick), Snappy, HH Freq;<br>- **CHOKE** switches;<br>- two FX buses with 5 slots each (Dynamics, Distortion, EQ, Delay, Filter / Chorus, Reverb, Bitcrusher);<br>- a 16-lane step grid with per-step articulation (Normal / **Flam** / **Sub-step** 2–4), velocity Normal 54 / Accent 97 / Global accent 97, length 16, shuffle 57. |
| `dr-14-punchbox2-kick.jpg` | PunchBox 2's five kick generator modes as skeuomorphic plates:<br>- **606** (Volume, Decay, Attack);<br>- **808** (Volume, Tone, Decay, Sweep Time);<br>- **909** (Volume, Attack, Tune, Decay, Tune Depth);<br>- **Sample** (start, ring mod, width, pan, high/low cut);<br>- **Wavetable** (Phase, Volume, Position, Low/High Cut). |
| `dr-15-roland-tr808-edit.png` | Roland Cloud TR-808:<br>- the hardware panel (Level/Tone/Decay/Snappy/Tuning per voice);<br>- the 16 coloured step keys;<br>- Edit Variation A–H;<br>- Last Step, Weak, Flam type, Sub-step type;<br>- 12 lanes each with an A–H variation strip, M/S and a per-lane shuffle slider. |

---

## 3. Anatomy of each product

### 3.1 Native Instruments Battery 4 (the drum sampler reference)

- **Cell matrix:**
  - up to **128 cells**, in up to 8 rows × 16 columns;
  - **up to 128 samples per cell**, layered or split by velocity;
  - cells are tinted by a colour scheme (Kicks, Snares, Toms, Hats, Cymbals, Claps, Other), with solo and mute
    inside the cell;
  - the right-click menu has add/replace, *render effects*, load/save cell, rename, and **output routing**.
- **Quick-access area** (always visible):
  - the selected cell's waveform with S/E markers, envelope overlay, sample picker ‹ ›, Tune, Reverse, Pan,
    Key Range, phase invert, L↔R swap, Level.
  - Its tune and pan act on the *cell*; layer-level tune and pan live on the Editor page.
- **Main page:**
  - Volume Envelope (AHD or AHDSR);
  - Pitch Envelope (Standard: Amount, Decay 1, Break, Decay 2; or Easy: Amount, Decay);
  - Velocity → Volume and → Pitch;
  - **Engine** with seven modes: Sampler Standard / Vintage (emulates two classic hip-hop samplers) / DFD (disk
    streaming), Stretch Standard (granular: grain, speed, smooth), Stretch Pro, Beat (REX/ACID with sync);
  - Filter (high/low cut), a one-knob compressor, delay and reverb sends.
- **Effects page**, per cell: Saturation, LoFi, Filter/EQ (Solid G-EQ, 3-band, LP, HP, BP, peak/notch), Compressor,
  **TM (Transient Master)**. TM changes attack and sustain envelopes *independently of level*, unlike a
  compressor.
- **Modulation page:** 2 LFOs, a modulation envelope, and **8 slots** (source → amount → destination, with invert).
- **Setup page:**
  - **velocity curve** with handles; MIDI channel; key track; note latch;
  - **Voice Groups:** 128 groups; voices 1–127; kill Any/Oldest/Newest/Highest/Lowest; **fade 0–999 ms**; an
    exclusive-group option. This is Battery's choke.
  - **Cell Activation:** trigger on note-on or note-off. Conditions: always, on key, on controller range,
    **cycle round-robin**, **cycle random** with a reset CC.
  - **Articulation:** ten generators (alternate stroke, release stroke, flam, drag, 3-stroke ruff, roll, buzz,
    muted, speed roll, Geiger counter) with speed and dynamics;
  - MIDI Echo (tune, gravity, sync, 1–100 echoes);
  - **Humanize** (amount, sound: velocity/tune/volume, time).
- **Editor page:**
  - destructive Wave Editor (crop, cut, normalise, fades, DC, reverse, snap to zero crossing);
  - Loop Editor (up to 4 loops per layer, count 0–127/∞, tune ±12, crossfade);
  - **Mapping Editor**: velocity zones per layer, velocity crossfades, stack/auto-spread, root, low/high velocity,
    tune ±12 st, pan ±100, volume.
- **Master page:** **4 buses** plus the master, each with the full effect set, plus convolution/algorithmic reverb
  and delay.
- **What it lacks**:
  - no sequencer;
  - no intelligent browser;
  - no synthesis beyond the sampler;
  - no display of the waveform in the cells.

  Reviewers call it "largely unchanged since 2013" (Dubspot, 7.2/10).

### 3.2 XLN XO (similarity map + beat playground)

- **Space:** every one-shot in the core library (8,700+) and in the user's folders is analysed and placed on a 2-D
  map by similarity. Dots are coloured by type. Click a dot to load it into the selected channel. A
  **Similarity** strip lists the nearest neighbours, and there are History, a Shortlist, search and filters. The
  mapping method is not published.
- **Eight channels** with fixed roles (Kick 1, Kick 2, Snare, Clap, CL HH, OP HH, Flex 1, Flex 2). Per channel:
  - sample start/end and fades;
  - pan;
  - envelope with **Transient / Hold / Decay**;
  - pitch **±12 st in 0.1 st steps**;
  - Tone (a tilt EQ) and Cut (soft HP/LP);
  - **VelSens 0–200 %**: 0–100 % narrows velocity; 100–200 % also softens the transients of soft hits;
  - Playback: **poly or mono (choke)**, **4 mute groups**, reverse, **output bus: 8 outputs + master**;
  - two FX sends.
- **Sequencer:**
  - 16 steps per lane, patterns A/B, Chain (AB, AAAB), per-step velocity by dragging;
  - **rolls of 2–4 hits per step**;
  - **Accentuator**: a velocity curve built from 1/2, 1/4, 1/6 and 1/8 components, with dice;
  - **Groove** templates per lane or global (Swing16, Hickup, Vintg16, "Annoy"…) and a **Nudge** arc per lane.
- **Playground:** the *Beat Combiner* auditions other patterns per lane, and the *Sample Combiner* auditions
  similar sounds, **non-destructively**. You then keep the change or revert.
- **Effects:** two sends (15 reverb/delay algorithms), master Crunch (8 algorithms), filter and soft clip.
- **Export:** drag out WAV (samples, kit, stems, beat) or MIDI.

### 3.3 Toontrack Superior Drummer 3 (and EZdrummer 3)

- **Content:** a 230+ GB core library; 7 kits, 16 kicks, 25 snares, 32 cymbals, about 350 electronic sounds.
  **22 mixer channels** of mics, 11 ambience mics (up to 11.1 surround), **16 internal stereo outputs and 32
  buses**, and 35 insert effects.
- **Per instrument** (inspector, `dr-07`):
  - Level; Reverse (ms);
  - **Voice and Layer**: *Layer Limits* Hard / Gradient / Soft, which restrict which dynamic layers can play, and
    a *Voice Limit*;
  - **Soft Hit Level**;
  - **Smoothing**, which evens out fast repeated hits so a roll does not jump between layers;
  - **Hi-Hat CC Edit**, for continuous openness from CC4 on e-kits and for editing;
  - Envelope and Offset; a **Velocity Gate**; MIDI Mapping.
  - Instruments are articulation-rich (hat tip, shank, open levels, pedal).
- **Mixer bleed:** each mic channel can take the bleed from the rest of the kit or from one chosen source, with
  "Bleed Enabled" and "Bleed Level" rows.
- **Grooves:**
  - about 1,640 MIDI grooves in the core, tagged by song part;
  - **Tap2Find**: tap a rhythm and get matching grooves;
  - **Song Creator**: turns a groove into a song structure;
  - **Edit Play Style**: hit density and velocity per drum within a pattern.
- **Grid Editor:** Quantize, Q Max, Swing, Randomize, Nudge; the velocity lane has Velocity / Dynamics / Randomize.
- **Tracker:** audio in, MIDI out. The model is machine-learned. A search excerpt says it looks at **512 frequency
  bands** over time; this was not found in a primary source. It matches each hit to a sound, gives a
  match-confidence lane with a threshold, **reduces bleed** across the tracks you select, and has Find Tempo.
  Reviews say the results sometimes need manual clean-up.
- **EZdrummer 3** has the same engine at song-writer depth:
  - 7 kits in 3 rooms, about 15 GB, 2,500+ grooves;
  - **Bandmate**: drop in audio or MIDI and it proposes grooves that fit;
  - Tap2Find, Song Creator, and a grid editor.

### 3.4 XLN Addictive Drums 2

- 18 kit pieces, with mixer channels for each plus overheads and room.
- **Edit page, per piece:**
  - pitch ±12 st, with separate offsets for the overhead and room channels;
  - volume envelope (attack, decay, sustain level, sustain time, release; velocity → attack);
  - **pitch envelope** (start, hold, release; velocity);
  - **Response**: volume 0–200 % (velocity range), velocity → filter, and **sample-layer limits** (exclude soft or
    hard layers);
  - **Tone Designer** on kick and snare, which reshapes the decay of the tonal band;
  - Cut (HP/LP), and "No Alts" (switches off the alternate hits).
- **Hi-hat:** a CC openness value (0 = closed, 127 = fully open) is turned into one of the kit's Closed/Open stroke
  types. Openness is *quantised* to the recorded strokes.
- **2.6 (2025):**
  - an **audio recorder**: auto-trimmed to the half bar, drag-out, with the date and tempo in the file name;
  - **5 snapshots** for A/B;
  - arrow-key browsing;
  - a Shortlist of beats.
- Also a **Beat Transformer**, which changes a groove's feel and density.

### 3.5 Image-Line FPC and the FL channel rack

- 16 pads in each of banks A and B (32). Each pad has **layers with velocity ranges** (drag the end points;
  "Spread even").
- **Layer selection among overlapping layers:** Cycle, Random, or **Random (avoid previous)**.
- Per layer: volume, pan, and tune ±1 octave. Per pad: volume and pan envelopes with spline segments.
- **Cut / Cut-by** for chokes. **Output offset** sends pads to consecutive mixer tracks, which is FL's multi-out.
- A pattern manager of MIDI loops.
- The FL habit it serves: a **channel rack row per drum**, each on its own mixer insert. Cantus already mirrors
  that with one-pad kits on their own tracks (ADR 0013).

### 3.6 Ableton Drum Rack and Drum Sampler

- **Drum Rack:**
  - 128 note slots, shown 16 at a time;
  - each pad is a **chain**, so any instrument or sample can sit on it, and several chains on one pad make a
    layer;
  - chains have **Receive** and **Play** notes, and up to **16 choke groups**;
  - up to **6 return chains**;
  - 16 macros with **macro variations** (snapshots) and randomise.
- Pads have no key or velocity zones: layering on a pad is a nested Instrument Rack.
- **Drum Sampler (12.1+):**
  - start and length, an AHD envelope, transpose and detune;
  - LP/HP filter and one EQ band;
  - **one playback effect at a time** with an X/Y pad: stretch, loop, FM, ring mod, sub oscillator, noise, punch,
    degrade.
- **Live 12 similarity:** "swap similar" replaces a pad's sample, or the whole kit, with similar sounds from the
  browser.

### 3.7 Native Instruments Maschine 3

- Groups of 16 pads; patterns, scenes and song; a sampler with slicing and **stem separation**.
- **Six mono Drum Synth plug-ins with 29 engines**: 9 kicks, 9 snares, 3 hi-hats, 3 toms, 3 percussion,
  2 cymbals.
- Modulation editor with curves. **3.7 (today):** a random note generator, a Euclidean sequencer (step, fill,
  rotate) and **per-note chance**.

### 3.8 Sonic Charge Microtonic (the synth-drum reference)

- 8 channels, each a patch of **oscillator + noise**. Nothing is sampled, and "oscillators are oversampled"
  against aliasing. Figures from the user guide:
  - **Oscillator:**
    - sine, triangle or saw, 20 Hz–20 kHz (default 632.46 Hz);
    - **pitch mod**: *decaying* ±96 st with a rate of 10 ms–∞; *sine* ±48 st at 0–2 kHz; *random* (low-passed
      noise) ±48 st at 0–20 kHz;
    - attack 0–10 s, decay 10 ms–10 s (default 316.23 ms).
  - **Noise:**
    - LP, BP or HP, 20 Hz–20 kHz; **Q 0.1–10,000** (default 0.707); stereo on/off;
    - **envelope shapes: exponential, linear, and "modulated" (for claps)**;
    - attack 0–10 s, decay 10 ms–10 s.
  - **Mix and output:**
    - mix osc/noise; **distortion 0–100, deliberately not anti-aliased**; EQ at 20 Hz–20 kHz, ±40 dB;
    - level −∞ to +10 dB; pan ±100; **choke** on/off (priority by channel number); **output A or B**;
    - **velocity sensitivity 0–200 %**, separately for oscillator, noise and modulation.
- **Pattern engine:**
  - 12 patterns (A–L) of up to 16 steps, with accent (velocity 127, or 64 without);
  - fill with a fill rate of 2×–8×; swing 0–100 %; step rate 1/8 to 1/32 including triplets.
- **Morph:** one slider interpolates all eight patches between two end states. Patterns are not affected.

### 3.9 Sonic Academy Kick 3

- **Five layers**, each a synthesized **sub** or a **sample** (500 click samples). The sub can add up to **nine
  extra harmonics**, each with its own gain, ratio and decay.
- **Bézier node editors** for pitch and amplitude, with LOG/LIN views and **Pitch Snap** (nodes snap to notes).
  **Selective key tracking**: choose which nodes follow the played note, so the tail transposes while the
  thump's pitch stays fixed.
- **Phase:** rotation, phase lock and phase align between layers.
- Effects: clip, tube, wave, tape distortion, bit crusher, ring mod, delay, reverb, filter, drive, dual
  compressors, 4-band EQ with analyser, and a limiter. Also Gate and Length (synced to BPM), 30-step undo, and
  an "AI import" that turns a kick sample into an editable preset.
- The screenshot's branding reads "Sonic Digital".

### 3.10 D16: PunchBox 2, Nepheton 2, Drumazon 2

- **PunchBox 2 (June 2026):**
  - a main kick generator in **Sample / 909 / 808 / 606 / Wavetable** modes, plus **three sample layers** (click,
    body, noise);
  - an **Advanced Editor with multi-stage pitch and amplitude envelopes** for each layer and the master;
  - **real-time pitch detection** in the channel strip;
  - 4 re-orderable effects plus mono bass and a limiter; **10 patchbay topologies**;
  - **each layer to a separate output**; drag-out export with pitch, normalisation and duration options.
  - CDM missed the old sine generator.
- **Nepheton 2 (808):**
  - per-voice extras the hardware lacks: kick **Sweep**, tuning on more voices, per-channel HP/LP with
    resonance, 2-band EQ and dynamics;
  - two FX buses;
  - sequencer with **per-step flam and sub-step (2–4)**, accent and global accent, shuffle, 1,200+ patterns.
- **Drumazon 2 (909)** has the same frame for the 909.

### 3.11 Roland Cloud TR-808 and TR-909

- **ACB** (Analog Circuit Behavior): Roland models the circuits from the original units and schematics.
- **TR-909:** a hybrid, true to the original. Analog models for the drums; **samples for the closed and open hats,
  crash and ride**. In the 1983 hardware these were **6-bit** recordings of Paiste and Zildjian cymbals made in
  Roland's office (Wikipedia, citing Roland).
- **Extras over the hardware:**
  - bass-drum and cowbell tuning, per-instrument gain (drive) and pan;
  - **per-part shuffle and last step**;
  - **3 sub-step types and 9 flam types**, weak hits;
  - 8 variations A–H per pattern, 11 parts, 16 steps;
  - 44.1–192 kHz.

### 3.12 The similarity and generative newcomers

- **Algonaut Atlas 2:**
  - an AI model trained mainly on drums labels and clusters a sample folder into a star-map, with classes named in
    drum-machine terms plus "Other";
  - a sequencer of up to **32 steps with triplets** and **a length per lane** (polymeter);
  - sample variation and humanisation.
- **Audialab Emergent Drums 2:**
  - a 16-pad instrument whose samples are **generated** by two models ("Creamy", "Crunchy") trained without source
    recordings in the output;
  - "Deep Sampling" makes variations, with a similarity slider.

---

## 4. The DSP

### 4.1 Where Pulsus stands today

`pulsus.rs`, 1,080 lines with its tests:

| Voice | Today's algorithm | Numbers |
|---|---|---|
| Kick | sine; pitch = base·(1 + span·e₁ + knock); two exponential pitch stages (body `pitch_decay`, knock 8 ms); click = high-passed noise, 4 ms; `tanh` drive normalised to peak | base 45 Hz; drop 15.5 st (start ≈ 110 Hz); pitch decay 40 ms; knock ×(1 + 2.5·click); drive k = 1 + 3·drive, default 0.2 |
| Snare | two sines at 180 Hz and 180 × 1.8333 = 330 Hz with a 0.35 pitch bend over 15 ms; noise → BP(tone) × 1.7 + HP(tone/2) × 0.35 on its own decay | tone 1.9 kHz; mix 0.55; noise decay 0.19 s |
| Hi-hat | six PolyBLEP squares at 205.3 × [1, 1.4827, 1.8003, 2.5461, 2.6303, 3.8967] Hz → BP(tone) → HP(hp_cutoff); 2 ms stick noise | tone 9.5 kHz; HP 7 kHz |
| Sample | linear interpolation, file rate → engine rate, tuned by the pad, stereo | decay default 4 s |
| Kit | 32 pads, 24 voices, quietest-voice stealing, **choke = 4 ms exponential fade**, velocity^1.5 amplitude, brightness 0.75 + 0.5·v, loudest pad about −6 dBFS | |

Its strengths are real:
- block-size independence by capturing parameters at the note-on;
- zero is exactly zero;
- the choke test measures the choke itself;
- a headroom test.

Two findings from reading it:
- **The kick's 45 Hz base is 52 cents sharp of F1 (43.65 Hz)**, so the default kick is in no key at all.
  Pulsus II should store the kick's pitch as a note plus cents, and show the note.
- **The 808 hat ratios need checking.** Pulsus's six frequencies (205.3, 304.4, 369.6, 522.7, 540, 800 Hz) are
  the values usually quoted for the 808 metal voice. A search excerpt of Werner, Abel & Smith (ICMC 2014) instead
  gives the six Schmitt-trigger oscillators' nominal frequencies as spanning **205.3 to 1149.9 Hz**. The paper
  could not be read here. Check it before "808 accuracy" is claimed. The 540 and 800 Hz pair is the 808
  *cowbell's*, which may explain the mix-up.

### 4.2 Kick design

- **What makes a kick** (Reid, *Synth Secrets*, SOS; acoustic analysis):
  - a membrane's pitch falls as its amplitude falls (tension modulation). In an acoustic kick the fall is "a
    couple of semitones", about 10 %, not an octave;
  - the beater's click is hundreds of very short high partials lasting a few ms;
  - the upper modes of a two-headed kick are close to a harmonic series shifted by a few Hz.

  Electronic kicks exaggerate the sweep to 1–3 octaves.
- **808 bass drum** (Werner, Abel & Smith, DAFx-14; Reid):
  - a **bridged-T resonator** is struck by a shaped trigger pulse, and it decays by itself; no VCA is needed;
  - the pulse gives the click; the *tone* control is a low-pass on the output;
  - the pitch "goes slightly flat at long decays" (the "sigh"), which is different from the frequency jump during
    the attack;
  - the Decay knob changes the damping, not a VCA.

  Model it as a 2-pole resonator (y[n] = 2r·cos θ·y[n−1] − r²·y[n−2] + x[n]) excited by a band-limited pulse.
  Decay sets r. The sigh is a small, amplitude-dependent θ drift. Soft saturation in the feedback path gives the
  attack's frequency lift.
- **909 bass drum** (Reid): a **saw VCO → waveshaper → near-sine**. The pitch EG has an instant attack and a slow
  decay. A separate click path mixes a noise burst (low-passed) with a pulse on its own envelope. The panel
  controls are Tune, Tune Depth (as on PunchBox's 909 plate), Attack (click) and Decay.
- **Modern kick instruments add three things:**
  1. **A drawn pitch curve with key tracking per node** (Kick 3): the body is transposed with the note while the
     transient stays fixed. This is the 808-bass trick.
  2. **Start phase** (Kick 3's Phase 0°): a sine that starts at 0° has no step. Starting at 90° gives a hard click
     for free.
  3. **Harmonic layers and saturation** for small speakers: Kick 3 has up to 9 harmonics; PunchBox and Kick 3 add
     drive with a limiter at the end.
- **Tuning to the key:** the tail frequency is f = 440·2^((n − 69)/12). In the kick range: E1 41.20, F1 43.65,
  G1 49.00, A1 55.00, B♭1 58.27 Hz. PunchBox 2 shows a live pitch detector, and Kick 3 snaps nodes to notes.
  Neither knows the song's key. Cantus can, once the project carries a key.
- **Saturation aliasing:** `tanh` at k = 4 on a 50 Hz sine makes harmonics that fold only at high drive. The
  **click noise through the same `tanh`** does alias. Pulsus II should either saturate the body only, or run the
  shaper at 2× with a half-band filter. Microtonic deliberately leaves its distortion unfiltered for grit; offer
  that as a mode, not as the default.

### 4.3 Snare

- **Acoustics:** the batter head's modes are coupled through the air to the snare head, and the snare head rattles
  against the wires (Rossing, Bork, Zhao & Fystrom, JASA 1992). Bilbao (JASA 2012) simulates the full
  head–air–wire system with finite differences; that is far too costly per voice, but it is the reference.
- **808/909:** two bridged-T (808) or VCO (909) bodies, plus noise. The noise is split into a band-passed and a
  high-passed path with separate envelopes, and "snappy" sets the level of the noise envelope (Reid). The
  180/330 Hz pair in Pulsus matches the partials Reid uses in *Synth Secrets*.
- **A better Pulsus snare at low cost:**
  - a **modal body** of 6–12 membrane modes;
  - **wire noise whose amplitude follows the body's rectified amplitude** after a short onset delay (1–3 ms), so
    the wires rattle while the head moves;
  - the existing separate noise decay kept as the "snappy" tail.

  Add a **Wires** control (coupling 0–1) and a **Snappy** control (tail level).

### 4.4 Toms, congas and bells: modal synthesis

- **Ideal circular membrane** (Fletcher & Rossing, *The Physics of Musical Instruments*): mode frequencies are
  proportional to the Bessel zeros j_mn. Relative to (0,1):

  | Mode | (0,1) | (1,1) | (2,1) | (0,2) | (3,1) | (1,2) | (4,1) | (2,2) | (0,3) | (5,1) |
  |---|---|---|---|---|---|---|---|---|---|---|
  | Ratio | 1.000 | 1.594 | 2.136 | 2.296 | 2.653 | 2.918 | 3.156 | 3.501 | 3.600 | 3.649 |

  Air loading and a second head pull these towards the near-harmonic set of a timpani (Rossing's figures for the
  (1,1), (2,1), (3,1) and (4,1) modes are roughly 1 : 1.5 : 2 : 2.5). Tom presets should interpolate between the
  "ideal" and "loaded" tables with one **Shell/Air** control.
- **Strike position** weights mode (m,n) by the mode shape at the strike radius, J_m(j_mn·r/a). A centre hit
  excites only the m = 0 modes; an edge hit brings in the higher m. That gives one **Position** knob with a
  physical meaning.
- **Mallet or stick:** use a contact model (Hunt & Crossley 1975: F = k·xᵅ + λ·xᵅ·ẋ). Or, cheaper, a half-sine
  force pulse of contact time 0.3–5 ms, which low-passes the excitation; harder means shorter. Avanzini & Marogna
  (IEEE TASLP 2010) build membrane percussion from exactly these modules: a modal membrane, a nonlinear mallet
  and air-cavity coupling.
- **Pitch glide:** tension modulation raises every mode by β·E(t), where E is the energy envelope (Avanzini,
  Marogna & Bank, JASA 2012). That is a physically motivated version of Pulsus's snare "bend".
- **Bars and bells** (cowbell, clave, woodblock): the modes of a free–free bar are 1 : 2.756 : 5.404 : 8.933.
  Plates and bells need measured tables. The 808 cowbell is instead two squares through a band-pass with a
  two-stage envelope (Werner, Abel & Smith, "More Cowbell", AES 2014).
- **Cost:** each mode is a 2-pole resonator: 3 multiply-adds per sample, with r = e^(−1/(τ·fs)) and
  θ = 2πf/fs. 16 modes × 32 voices × 48 kHz is about 74 M multiply-adds per second, fine with 4- or 8-wide SIMD
  across modes. Frequency-dependent damping (τ_k = τ₀·(f₀/f_k)^γ) is what makes it sound like skin and not glass.

### 4.5 Hats and cymbals

- **808 metal voice** (Werner, Abel & Smith, ICMC 2014; Reid):
  - six square (Schmitt-trigger) oscillators are summed;
  - a **pair of band-pass filters** splits them into a lower and an upper band;
  - the **cymbal** gives each band its own VCA and decay (Reid counts three envelopes), so the mix of low, mid and
    high changes as it rings; high-pass filters follow;
  - the **hats** use the upper band only, with a short (closed) or long (open) envelope;
  - the VCAs are "swing-type", with a characteristic attack.

  Pulsus has the hat path. The **cymbal path (two bands, three decays) is missing**.
- **909:** 6-bit samples of real cymbals. These cannot be shipped (ADR 0001: bundled audio must be original or
  CC0). A "6-bit / 909 character" option on a synthesized or CC0-recorded cymbal can evoke them.
- **Real cymbals** are strongly nonlinear. Energy cascades from low modes to high ones over tens of ms, which is
  the "shimmer" swell (Chaigne, Touzé & Thomas 2005; Bilbao 2010, shell models). A cheap version: a dense
  inharmonic modal bank (32–64 modes) plus high-passed noise whose envelope *rises* for 20–80 ms before it
  decays. That gives a **Swell** control.
- **Hat openness (CC4):**
  - Superior Drummer and AD2 turn a continuous pedal value into a choice among recorded openness levels. AD2's
    convention is 0 = closed, 127 = open. Some e-kit pedals send the reverse; a Toontrack forum thread describes
    "more open, lower number".
  - A synthesized hat can do better: map openness **continuously** to decay (e.g. 40 ms → 1.2 s, logarithmic),
    band-pass centre (+0 to −3 st) and a "sizzle" noise level.
  - A falling CC4 on a *sounding* open hat should close it: choke with a "chick" (foot-close) transient.
  - Provide **CC number, invert and a pedal curve**.

### 4.6 Clap, rim and the rest

- **Clap** (808, 909 and Microtonic's "modulated" noise envelope): band-passed noise (about 1–1.5 kHz) with
  **3–4 quick re-triggers about 10 ms apart**, then a longer "room" tail on the same noise. Controls: Bursts,
  Spacing, Tail, Tone, Q.
- **Rim and clave:** a short, high bridged-T or 2-pole resonator ping (a few hundred Hz to 2.5 kHz, 10–40 ms), plus
  an optional click.

### 4.7 Sample playback: layers, round-robin, velocity

- **Velocity layers:**
  - Battery's zones can have **velocity crossfades**, applied equal-power;
  - SD3's **layer limits** keep a hit inside a sub-range of dynamic layers;
  - SD3's **Smoothing** damps the layer changes caused by fast repeats;
  - AD2 and XO scale velocity response at 0–200 %.
- **Round-robin:** Battery chains cells with *Cycle Round Robin* or *Cycle Random*; FPC offers Cycle, Random and
  **Random avoid-previous**; AD2's "No Alts" switches it off. How repeatable the random modes are is not
  documented anywhere. Cantus should make them repeatable by design (§9).
- **Interpolation:** Pulsus uses linear. For pitched-up drums and cymbals a **4-point, 3rd-order Hermite** (or a
  short windowed-sinc for the "HQ" render) removes the audible aliasing.
- **Velocity → timbre:** pure gain scaling sounds like a volume knob. Pulsus already brightens with velocity.
  Microtonic's three separate sensitivities (oscillator, noise, modulation) are the model to generalise:
  velocity → level, tone (filter or noise), pitch-envelope depth and decay.

### 4.8 Transient shaping

- NI's Transient Master and SPL's Transient Designer work independently of level (the *differential envelope*
  idea):
  - two envelope followers on the same signal, one fast (attack about 1 ms) and one slow (about 20–50 ms);
  - attack gain = f(fast − slow), positive only at onsets;
  - a second, slow-release pair drives the **sustain** gain;
  - both outputs are in dB, bounded to ±24 dB.
- For a **synthesized** pad the same effect is exact and costs nothing: scale the first N ms of the amplitude
  envelope, or the decay slope. Offer the shaper for sample pads and map it onto the envelope for synth pads.
  The control and the meaning are the same.

### 4.9 Similarity maps

- XO, Atlas 2 and Live 12 do not publish their method. The public precedent is Google Creative Lab's *Infinite
  Drum Machine* (2016): audio features, then **t-SNE** (van der Maaten & Hinton 2008) into 2-D.
- A practical pipeline for Cantus:
  1. per file, compute: log-mel statistics (mean and variance), attack time, spectral centroid, flatness and
     roll-off, pitch confidence and estimate, length;
  2. normalise to about 40 dimensions;
  3. **UMAP** (McInnes et al. 2018) or t-SNE to 2-D, cached by file hash;
  4. k-nearest neighbours for "similar".
- **Role classification** (kick, snare, hat…) can be a small classifier trained on a CC0 labelled set, or kNN
  against a shipped reference set.
- A text–audio embedding (CLAP-style) would also allow "find me a dusty rimshot" by text. That is P2, and it
  connects to the AI bridge.

### 4.10 Humanising

- **Timing:** Hennig et al. (PLoS ONE 2011) found that the deviations of human timing are **long-range
  correlated** (1/f-like). Listeners preferred correlated deviations to independent white-noise jitter.
  Battery's and SD3's "Randomize" are presumably white; that is not documented.
- Offer **Amount (ms) and Colour (white ↔ 1/f)**. Make it **deterministic**: generated from the note's position
  and a seed, so export equals playback.
- **Velocity:** random ± plus **accent patterns** (XO's Accentuator; the per-step accent in Nepheton and Roland).
- **Articulations as note generators:** flams, drags, ruffs, rolls (Battery's ten; Nepheton's and Roland's
  sub-steps). In Cantus these belong as **note-editing tools** that write real notes (see P1): they undo, show
  in the rack and export as MIDI. Engine-side effects would do none of that.

---

## 5. The interface: praise, criticism, legibility

**Layouts:**
- **Battery** (`dr-01`): a browser on the left, a large cell grid, a waveform strip, then a row of module
  panels with six page tabs at the bottom. It is dense, and every page is a tab away.
- **XO** (`dr-04/05/06`): the map fills the window; the channels are rings down the left; the per-channel
  controls are one strip along the bottom. It is the most *spatial* UI in the category.
- **Superior Drummer 3** (`dr-07…10`): four big tabs (Drums, Grooves, Mixer, Tracker), a photo-real kit, a
  collapsible inspector on the right, and a song track that is always visible.
- **Kick 3** (`dr-12`): the display is the whole instrument, as in Pro-Q.
- **Microtonic** (`dr-11`): one fixed panel, light grey, everything visible at once.
- **D16 and Roland** (`dr-13/14/15`): hardware skeuomorphism.

**What works:**
- **Kick 3:** the pitch curve drawn *over the rendered waveform* on a log-Hz axis, with node handles. You see and
  hear the same thing. This is the single best display idea in the category, and Pulsus II should adopt it for
  every synth voice.
- **XO:**
  - the map plus the Similarity strip plus History;
  - hexagon steps with roll dots and the Accentuator curve under the grid;
  - Nudge as an arc per lane.
- **SD3:**
  - the inspector's collapsible sections, each with a power button;
  - song blocks coloured by part;
  - the confidence lane with a threshold in Tracker, which is an honest display of what the machine is unsure of.
- **Battery:** the mapping editor's velocity × layer blocks, and the layer's fields beside them.
- **Microtonic:** it *reads*. There is no hidden state, and one screen is the whole instrument.
- **Nepheton and Roland:** per-step articulation drawn inside the step (flam and sub-step glyphs); a per-lane
  shuffle slider beside the lane.

**What does not:**
- **Battery's cells show a name and nothing else.** There is no waveform, no level and no output badge. Choke
  ("voice groups") lives on another page with no mark on the cell. Routing is in a context menu.
- **XO** has 8 fixed channels and 16 steps. Search pauses for seconds (MusicRadar), and the filter popup cannot be
  detached.
- **SD3** needs 230 GB of content, and its UI layers floating effect windows over the mixer (`dr-08`).
- **Hidden hat openness.** CC4 behaviour sits in settings dialogs (SD3's e-drum settings, AD2's hi-hat page).
- **Every plug-in keeps its own undo** (Kick 3 has 30 steps), separate from the host's.

**Measured legibility** (WCAG 2 contrast; text ≥ 4.5:1, non-text boundaries ≥ 3:1):

| Element | Colours | Ratio |
|---|---|---|
| Battery cell label "Kick Aceton 1" | #ee9291 on #4c4244 | **4.21:1** (fails small text) |
| Battery cell label "ClosedHH Aceton 1" | #73dede on #2c3535 | 7.94:1 |
| Battery cell body vs grid background | #3a2e2f vs #262626 | **1.16:1** |
| Battery knob body vs panel | #646360 vs #454545 | **1.60:1** |
| XO channel label "KICK 1" | #ebeef1 on #393b41 | 9.55:1 |
| XO search placeholder | #919395 on #383b40 | 3.63:1 |
| SD3 dimmed row "Cymbal 3" | #a5a8af on #4d5057 | 3.40:1 |
| SD3 inspector section, off ("Hi-Hat CC Edit") | #909498 on #4d5057 | **2.64:1** |
| Kick 3 "ADD NEW LAYER" | #797979 on #202020 | 3.74:1 |
| Kick 3 knob body vs panel | #191919 vs #2b2b2b | **1.24:1** |
| Microtonic label "distort" (light UI) | #242424 on #9d9d9d | 5.75:1 |
| Microtonic step pad (off) vs panel | #e3e3e4 vs #929292 | 2.43:1 |
| Nepheton 2 knob label "DECAY" | #c8c8c7 on #1d1d1c | 10.10:1 |

The pattern is the same as FabFilter's (family.md §0.4):
- text is mostly fine;
- **control and cell boundaries are nearly invisible** (1.2–1.6:1);
- **"off" and "dimmed" states drop below AA**.

Colour-coded cells (Battery, XO) also carry meaning by colour alone. XO backs its colours with names on the
rings; Battery's scheme has no glyph.

---

## 6. What reviewers and users say

*Forum caveat:*
- the forum opinions below come from a handful of KVR and Toontrack threads and do not measure the user base;
- Gearspace returned 403 and was not used;
- review scores are the publications' own.

- **Battery 4:**
  - praised for depth per cell, CPU efficiency and a "fast grid workflow";
  - criticised because the "UI and core feature set feel dated and largely unchanged since 2013", it has no
    intelligent browsing or beat generation, and USD 199 is steep for a product in maintenance mode.
  - Dubspot rates it 7.2/10 and points to **Atlas 2 (8.1)** as the modern alternative.
- **XO:**
  - MusicRadar (2019): the Space and the Playground combiners are "imaginative"; the sequencer is fixed at 16 steps;
    search sometimes pauses for seconds.
  - KVR users:
    - the UX "doesn't bog me down"; auditioning while the beat plays is the standout;
    - **exported MIDI notes are "arbitrarily short"** (about a third of a 16th), which breaks downstream samplers;
    - requests: per-step pitch (808 lines), kit-only presets, larger steps, keyboard entry of rhythms.
- **Superior Drummer 3:**
  - MusicTech: the kits and bleed control are excellent; scaling from 70 to 250 % works on an 11" laptop;
    Tap2Find is praised;
  - the 230 GB footprint is a burden (Toontrack sells a pre-loaded SSD);
  - Tracker output sometimes needs editing;
  - it "isn't Maschine" for sample-based beat-making.
  - SOS: 11 ambience mics, bleed into any room mic, Tracker handles hat articulations well after a threshold tweak.
- **Kick 3:**
  - MusicTech 7/10 (Jan 2025): good sounds, a big preset set, and the AI import;
  - but "limited modulation", **macros limited to one control each**, and pricey next to Kick Ninja (GBP 60) and
    KickDrum 2 (GBP 36).
  - The same review says key tracking and velocity controls are *absent*, while the product page advertises
    selective key tracking. Treat that point as unresolved.
- **PunchBox 2** (CDM, June 2026): "a surprisingly deep sequel"; the multi-stage envelopes make it "almost an
  entirely new plug-in". The reviewer misses the sine generator, and the back-panel settings are buried.
- **FPC** (Musco, March 2026): it is for *playing* drums, with layering and velocity. It has no sound design, and
  it is overkill for simple step programming.
- **Microtonic:** only the maker's page and manual were read. It is widely cited as the synth-drum benchmark, but
  no independent 2026 review was fetched.

---

## 7. Gaps none of them fills

1. **Pads that are mixer channels without routing work.** Every multi-out plug-in (SD3's 16 outputs, XO's 8,
   Battery, AD2, PunchBox's layers) needs the DAW's multi-output instrument setup. FPC comes closest (output
   offset) and still assigns inserts by number. Nobody gives each pad a strip that is **named, coloured and
   ordered by the kit**, and follows it when pads are added or reordered.
2. **One timeline.** XO, SD3, Maschine, Microtonic, Nepheton and Roland each run a sequencer inside the plug-in,
   and export MIDI to reach the host (lossy: XO's short notes). Grooves live in the plug-in's library, not in the
   project.
3. **Synthesis and similarity in one place.** XO and Atlas map only samples. Microtonic and PunchBox only
   synthesize. Nobody places a synthesized patch in the same space as samples ("find the sample closest to this
   synth snare", or the reverse, "fit a synth patch to this sample"; Kick 3's AI import does this for kicks only).
4. **Key-aware tuning.** Kick 3 snaps to notes and PunchBox 2 detects pitch, but neither knows the song's key.
5. **Repeatable randomness.** Round-robin random, humanise and "cycle random" are not documented as reproducible
   anywhere. A bounce that differs from playback is a real complaint in production work, though no source
   measured it here.
6. **Continuous hi-hat on a synthesized kit.** The recorded kits quantise CC4 to recorded strokes. The 808 models
   have fixed open and closed pads. None found offers a pedal-driven synthesized hat with chick and close.
7. **Legibility.** Everything but Microtonic is dark, with boundaries at 1.2–1.6:1 and state hidden in tabs or
   dialogs.
8. **Undo and automation.** Each plug-in has its own undo, and its parameters are addressed by host-visible index
   (NKS, VST); pad parameters move when kits change.
9. **Audio-to-drums on the project's own audio.** SD3's Tracker needs the audio imported into the plug-in. A host
   can analyse any clip in place and write the result as a clip on the kit's track.
10. **A kit from a description.** Emergent Drums generates samples. Nothing builds a *whole editable kit and
    groove* from "dusty 90s boom-bap, loose hats".

---

## 8. How Cantus beats them

### P0: parity (without these Pulsus II is not credible)

1. **Sample pads with layers, velocity zones and round-robin:**
   - up to 16 layers per pad with velocity low/high and a crossfade width;
   - RR groups with modes Off / Cycle / Random / **Random no-repeat**;
   - seeded and position-derived (§9);
   - Hermite interpolation;
   - drag a file from the OS or the browser onto a pad (the drums plan already lists this).
2. **A full per-pad sound section on every voice:**
   - amplitude AHD with curve;
   - pitch envelope (amount and decay, with curve);
   - SVF filter (LP/HP/BP, cutoff, resonance, envelope and velocity amounts);
   - **transient** (attack and sustain in dB), drive with modes, start offset or phase, reverse.
3. **Velocity response as in the market:**
   - **sensitivity 0–200 %** for level (100 % = today's curve);
   - separate depths for tone, pitch and decay;
   - a drawable curve in the pad strip;
   - SD3-style **layer limits** and **smoothing** for fast repeats.
4. **Choke done properly:**
   - 16 groups;
   - a **fade time** parameter (1–250 ms, default 4 ms, today's value);
   - per-pad **polyphony** (1 = mono, as in XO's poly mode; up to 8);
   - kill Oldest/Quietest;
   - an option to **choke on note-off**, for a held pedal;
   - drawn as links on the pad grid.
5. **Hi-hat pedal:**
   - CC4 (number configurable, invert, curve) drives **continuous openness** on synth hats (decay, tone, sizzle);
   - on sample hats it chooses the openness layer;
   - a falling CC4 closes a ringing hat with a chick;
   - the GM pedal-hat pad (44).
6. **More voices** (§8.5): 808- and 909-style kick and snare models, **clap**, **modal tom/perc**, **metal**
   (808 cymbal with two bands and three decays, hat, cowbell, modal cymbal with swell), rim/clave. This brings
   the breadth up to Microtonic and Maschine's drum synths.
7. **Per-pad output to a mixer strip.** The minimum every competitor has, done the Cantus way from the start
   (P1.1).
8. **A pad-grid editor with a waveform per pad.** Synth pads show their *rendered* hit, which no sampler can.
   Add kit presets (save/load as files), and a preset browser in the shared family browser.
9. **Clean DSP:** a band-limited click, 2× oversampled drive (with a "raw" mode), and headroom kept at about
   −6 dBFS for the loudest pad. Every existing test keeps passing.

### P1: host-native, where we are clearly better

1. **Pads are channel-rack rows *and* mixer strips.**
   - A pad's **Output** is *Kit* (the track's strip) or *Strip n*.
   - "**Route pads to strips**" is one command: it creates or chooses strips named and coloured after the pads,
     under a **kit bus** strip (the strip tree of ADR 0014). The kit stays one instrument, one clip and one
     note list.
   - The rack's pad rows show the strip badge and route picker, as track rows already do.
   - Soloing a pad strip is the normal strip solo.
   - A null test proves "exploded" equals "kit" at unity (§9). FL users get their per-drum inserts; Ableton users
     get Drum Rack chains without the return-chain workaround.
2. **Grooves are clips.**
   - The groove library is a set of **clip presets** whose notes are tagged by **drum role** (Kick, Snare,
     ClosedHat…), not by pitch, so any groove plays on any kit.
   - Tap2Find becomes "tap on the pads or the keyboard → matching clips".
   - **Groove templates** (swing, feel) are extracted from any clip, or from audio via onset detection. They
     apply as an undoable quantise or as a clip property.
   - The **Accentuator**, **Beat Combiner** (per-lane alternatives) and **articulation generators** (flam, drag,
     ruff, roll, Euclid, chance as in Maschine 3.7) are **note tools** that write real notes into the clip.
3. **One undo history** for the kit, pads, layers, routing, grooves and notes. A drag is one command, and the
   undo fuzz covers the new commands (layer insert/remove included).
4. **The AI bridge builds a kit from a description.**
   - Tools: `create_kit(description)`, `set_pad(role, …)`, `write_groove(style, bars, feel)`.
   - The result is ordinary commands: editable, undoable, explained in the help strip.
   - With the similarity index it also picks user samples ("dusty rimshot"). "Make the snare darker" becomes Tone
     and Filter changes on one pad.
5. **One similarity browser for the whole app** (drums and the sampler):
   - an XO-style map of the user's library;
   - "**swap similar**" per pad and per kit (Live 12);
   - History and a Shortlist;
   - **synth patches placed in the same space** (render the pad and embed it). This fills gap 3.
6. **Audio → drums in place.** Onset detection plus role classification on any audio clip. The output is a clip
   on a kit track, with a confidence lane and threshold as in SD3's Tracker, and bleed rejection across the
   selected tracks. It needs no import.
7. **Kit snapshots and morph:**
   - A–D snapshots of the whole kit, with an automatable **Morph** (Microtonic's slider, generalised);
   - continuous fields interpolate; stepped fields switch at 50 %; sample layers crossfade.
8. **Key-aware tuning:** kick, tom and 808-bass pads can **follow the project key** (root or fifth). The pad
   display shows "43.7 Hz = F1 −1 ct ✓ in F minor".
9. **Sidechain from a pad** (the kick pad's strip feeds the bass compressor) through the existing send model, with
   no MIDI or audio routing tricks.
10. **Render pad to sample:** freeze a synth pad into a sample layer, like Battery's render-cell, and keep the
    patch.

### P2: later

- An **ambience and bleed matrix** for synthesized and sampled kits: each pad sends to "overhead" and "room"
  strips with a per-pad amount, delay (distance) and filter. That is SD3's realism for any kit.
- **Physical models** beyond modal: a finite-difference snare with wires after Bilbao (2012), and a nonlinear
  cymbal (Bilbao 2010).
- **Text-to-sound search** (a CLAP-style embedding) and ethical **generative variations** of the user's own
  samples (as in Emergent Drums). Licensing review first.
- **MPE and positional sensing:** strike position from a controller drives the modal Position control.
- Surround output; e-kit presets (SD3 and AD2 ship dozens).

### 8.5 Proposal: Pulsus II

#### Data-model changes over today's `pads.rs`

The design keeps the principle written on `DrumPad`: **flat `f32` fields addressed by a field enum**, so the command
bus sets and undoes them without knowing the voice, and `applies_to` filters the inspector. The changes:

| Change | Why |
|---|---|
| `DrumPad.id: PadId` (u16, never reused within a kit) and `InstrumentParam::Pulsus { pad: PadId, field }` | today a pad is addressed by **index into a pitch-sorted Vec** (`pad: u8`); inserting a lower pad renumbers every pad above it, which breaks automation and routing |
| `role: DrumRole` (Kick, Snare, Rim, Clap, ClosedHat, PedalHat, OpenHat, TomLow/Mid/High, Crash, Ride, Perc, Fx, Other) | grooves, AI, similarity swap and GM import are role-addressed, not pitch-addressed |
| `PadVoice` gains `Tom`, `Metal`, `Clap`, `Perc`, and a stepped **`model`** field per voice: Kick {Pulsus, 808, 909}, Snare {Pulsus, 808, 909, Modal}, Metal {Hat, Cymbal808, Cowbell808, CymbalModal} | 7 voices × a few models instead of 15 voices; the existing kick, snare and hat become the `Pulsus` models of their voices, so today's kits load unchanged |
| `layers: Vec<PadLayer>`, each with {sample path, vel_lo, vel_hi, xfade, rr_group, tune, level_db, pan, start, end, reverse, root} and new commands `InsertLayer`, `RemoveLayer`, `SetLayerField`, `SetLayerSample` | a Vec with its own commands keeps `DrumPad`'s flat fields flat; a synth voice *and* layers can sound together (a synth body plus a sampled click, as in Kick 3 and PunchBox) |
| `output: PadOutput { Kit, Strip(StripId) }` | per-pad routing (P1.1) |
| flat fields for the envelopes, filter, transient, velocity, humanise, choke and CC4 (table below) | parity (P0.2–P0.5) |
| kit-level `KitParams { seed, hat_cc, hat_invert, key_follow, morph, snapshots[4] }` | reproducibility, pedal, key and morph |
| `MAX_PADS` 32 (unchanged); `MAX_DRUM_VOICES` 24 → 48; **`MAX_PAD_OUTS` 16** stereo buffers, preallocated | modal voices and pad strips; ADR 0005 (no allocation on the audio thread) |
| Schema 13 → 14; the migration assigns `PadId`s in pitch order, sets `role` from the GM pitch and `model = Pulsus`, and leaves `layers` empty (moving `sample` into one full-range layer) | the standing rule: a schema bump comes with a fixture and a bit-identical render test |

#### Parameter list (per pad unless noted; the names are the `PadField` names)

| Group | Field | Range | Default | Unit / notes |
|---|---|---|---|---|
| Identity | Voice / Model | stepped | — | per voice (above) |
| | Role | stepped | from GM pitch | |
| Mix | Level | −60 … +12 | 0 | dB; −60 shows as −∞ |
| | Pan | −1 … 1 | 0 | shown L100–R100 |
| | Output | Kit / Strip n | Kit | a route, not automatable |
| Pitch | Tune | −24 … +24 | 0 | st, 0.01 resolution; synth pads show the resulting note and cents |
| | Key follow | 0 … 100 | 0 | % of (played note − pad pitch) applied; 100 = the pad plays chromatically (808 bass) |
| Amp env | Attack | 0 … 100 | 0 | ms |
| | Hold | 0 … 500 | 0 | ms |
| | Decay | voice range (today's) | voice default | s, logarithmic |
| | Curve | −1 … 1 | 0 | 0 = exponential (today), 1 = linear, −1 = faster-than-exponential |
| Pitch env | Amount | −48 … +48 | Kick 15.5, others 0 | st (Pitch Drop today) |
| | Decay | 5 … 400 | Kick 40 | ms, logarithmic (Pitch Decay today) |
| | Curve | −1 … 1 | 0 | |
| Filter | Mode | Off / LP / HP / BP | Off | |
| | Cutoff | 20 … 20,000 | 20,000 | Hz, logarithmic |
| | Resonance | 0 … 1 | 0.1 | |
| | Env amount | −48 … +48 | 0 | st of cutoff, following the amp envelope |
| Shape | Transient attack | −24 … +24 | 0 | dB |
| | Transient sustain | −24 … +24 | 0 | dB |
| | Drive | 0 … 1 | Kick 0.2, others 0 | exactly clean at 0 (today's rule) |
| | Drive mode | Soft / Tape / Clip / Raw | Soft | Raw = no oversampling (the Microtonic grit) |
| | Start | 0 … 100 | 0 | ms (samples) or ° phase 0–90 (synth) |
| Velocity | → Level | 0 … 200 | 100 | %; 100 = today's v^1.5 |
| | → Tone | 0 … 100 | 50 | %; 50 = today's brightness law |
| | → Pitch | −12 … +12 | 0 | st at velocity 127 |
| | → Decay | −100 … +100 | 0 | % |
| | Layer smoothing | 0 … 100 | 0 | % (SD3's Smoothing) |
| Humanise | Velocity | 0 … 30 | 0 | ± velocity units |
| | Time | 0 … 20 | 0 | ± ms, deterministic |
| | Time colour | 0 … 1 | 0.5 | 0 = white, 1 = 1/f |
| | Tune | 0 … 50 | 0 | ± cents |
| Voices | Choke group | 0 … 16 | 0 (hats 1) | 0 = none; named groups |
| | Choke fade | 1 … 250 | 4 | ms to −60 dB |
| | Choke on note-off | off/on | off | |
| | Polyphony | 1 … 8 | 8 | 1 = mono (retrigger fades the previous hit by the choke fade) |
| Round-robin (samples) | RR mode | Off / Cycle / Random / No-repeat | Off | |
| **Kick** (Pulsus) | Knock | 0 … 1 | 0.35 | today tied to Click; split out |
| | Click level | 0 … 1 | 0.35 | |
| | Click tone | 300 … 16,000 | 2,400 | Hz |
| | Harmonics | 0 … 1 | 0 | adds the 2nd and 3rd with decays proportional to the body's |
| Kick 808 | Decay | 0.05 … 4 | 0.8 | s (resonator damping) |
| | Tone | 200 … 8,000 | 1,500 | Hz, output low-pass |
| | Sigh | 0 … 1 | 0.3 | amount of amplitude-dependent pitch fall |
| Kick 909 | Tune depth | 0 … 48 | 24 | st |
| | Attack | 0 … 1 | 0.5 | click level (noise + pulse) |
| **Snare** | Body ratio | 1.3 … 2.5 | 1.8333 | today's constant, exposed |
| | Snappy | 0 … 1 | 0.55 | today's Mix |
| | Noise decay | 0.01 … 2 | 0.19 | s |
| | Tone | 300 … 8,000 | 1,900 | Hz |
| | Wires (Modal) | 0 … 1 | 0.5 | coupling of wire noise to body motion |
| **Tom / Perc** (modal) | Material | Membrane / Loaded membrane / Bar / Plate / Wood / Bell | Loaded membrane | mode table |
| | Modes | 4 … 32 | 12 | |
| | Damping | 0.1 … 1 | 0.5 | high-mode decay scaling γ |
| | Position | 0 … 1 | 0.3 | centre → edge |
| | Hardness | 0 … 1 | 0.5 | contact time 5 → 0.3 ms |
| | Glide | 0 … 4 | 1 | st of tension-modulation glide |
| | Shell/Air | 0 … 1 | 0.5 | ideal ↔ loaded mode table |
| **Metal** | Tone | 2,000 … 16,000 | 9,500 | Hz band-pass (today's) |
| | HP cutoff | 1,000 … 16,000 | 7,000 | Hz (today's) |
| | Openness | 0 … 1 | 0 closed | the CC4 target; automatable |
| | Open decay | 0.1 … 3 | 1.2 | s at openness 1 |
| | Band mix (Cymbal808) | 0 … 1 | 0.5 | low ↔ high band |
| | Swell (CymbalModal) | 0 … 200 | 40 | ms of the noise rise |
| | Stick | 0 … 1 | 0.5 | tick level |
| **Clap** | Bursts | 1 … 6 | 4 | |
| | Spacing | 3 … 30 | 10 | ms |
| | Tail | 0.05 … 0.8 | 0.25 | s |
| | Tone | 500 … 3,000 | 1,100 | Hz |
| | Q | 0.5 … 8 | 2 | |
| Kit | Seed | 0 … 2³²−1 | 1 | round-robin, humanise, noise |
| | Hat CC / Invert | 0 … 127 / off-on | 4 / off | |
| | Key follow source | Off / Project key | Off | |
| | Morph | 0 … 1 | 0 | A ↔ B snapshot |

#### Voice models to add (first-cut order)

1. **Metal: Cymbal808.** Two bands, three decays; the oscillators are already in the code. Then **Cowbell808**
   (two squares, band-pass, two-stage envelope).
2. **Clap.** Burst envelope plus tail.
3. **Tom/Perc modal.** A resonator bank with the membrane tables, Position, Hardness and Glide. This one engine also
   gives congas, bongos, woodblock, cowbell-as-bar and bells.
4. **Kick 808 and Kick 909.** A bridged-T-style resonator with sigh; a saw → shaper VCO with a click path.
5. **Snare Modal.** Modal body plus wires that follow the body.
6. **Metal: CymbalModal.** Dense inharmonic bank plus a swelling noise.

*Reference implementation worth reading:* Mutable Instruments **Plaits** has analogue bass-drum, snare and hi-hat
models, and its code is published under MIT. Check the licence header before borrowing anything. Surge XT and Vital
are GPL and must not be copied.

#### Editor layout (plug-in frame from family.md §7; docks in the inspector or floats)

```
┌─ header 32 px ─────────────────────────────────────────────────────────────────────────────────────┐
│ ■ Drums·Tr 3 │ PULSUS II │ ⟲ ⟳ │ A|B →Copy │ ‹ Kit: Boom Bap 1 ● › │ Δ │ ⏻ │ ? │ ⋯ │ ⛶             │
├─ pads (4×4, page A|B) ─────────┬─ selected pad: KICK · C1 · Kick/Pulsus ─────────┬─ pad strip ─────┤
│ ┌─────────┐┌─────────┐┌──────┐ │ ┌─────────────────────────────────────────────┐ │ Level  −3.0 dB  │
│ │CRASH  C2││RIDE     ││ ...  │ │ │ rendered hit (light fill)             20k ─ │ │ Pan    L12      │
│ │▁▂▅▇▅▃▂▁ ││▁▃▄▃▂▁   ││      │ │ │ pitch curve ●━━━●━━━━━━━━━━━━●  (log Hz) │ │ │ Out    → 7 Kick │
│ │→ M   ◌  ││→ 9  ◌   ││      │ │ │ amp env - - - - A H D handles          50 ─ │ │ Choke  —        │
│ └─────────┘└─────────┘└──────┘ │ │ tail 43.7 Hz = F1 −1 ct  ✓ F minor          │ │ Poly   8        │
│ ┌─────────┐┌─────────┐┌──────┐ │ └─────────────────────────────────────────────┘ │ Role   Kick     │
│ │CL HAT C1│━│OP HAT C1│ …     │ │ BODY   (Tune)(Drop)(P.Dec)(Decay)(Drive)(Phase) │ VELOCITY        │
│ │▇▃▁      │ │▇▆▅▄▃▂▁ │ ◉ CC4 │ │ CLICK  (Level)(Tone)(Knock) [sample ▾] (Start)  │ ┌─curve───────┐ │
│ └─────────┘└─────────┘       │ │ SHAPE  [LP▾](Cut)(Res)(Env) · Trans (Att)(Sus)  │ └─────────────┘ │
│  C1 link = choke group,        │ LAYERS (sample pads only):                      │ →Lvl 100 %      │
│  drawn as a bar with a letter  │  vel 127 ┌──┬──┬──┐  RR: Cycle · seed 1         │ →Tone 50 %      │
│  and a glyph, not only colour  │       64 │L1│L2│L3│  xfade 8                     │ HUMANISE        │
│  [+ pad]  [Route pads→strips]  │        1 └──┴──┴──┘  [+ layer] [drop file here] │ Vel ±8  Time ±3 │
├─ footer 28 px ─────────────────┴──────────────────────────────────────────────────┴─────────────────┤
│ Learn │ Seed 1 │ Hat CC 4 ⇅ │ Voices 14/48 │ Morph A ━━━●━━ B │ Similar ▸ │ Out −6.1 dB ▮▮      │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
          "Similar ▸" opens a bottom drawer: the map of the library (and rendered synth pads),
           the nearest-neighbour list for the selected pad, History and a Shortlist.
```

**Rules applied from family.md §7:**
- **Cells:**
  - a 3:1 edge against the grid;
  - name in `text` ≥ 4.5:1;
  - a mini waveform (a rendered hit for synth pads);
  - an **output badge** (→ M or → strip number);
  - a **choke chip** (letter plus glyph), with the choke drawn as a link between cells;
  - velocity flash on trigger;
  - a mute state shown by a struck-through name as well as a tint.
- **Display:** the Kick 3 idea on every synth voice. The waveform is re-rendered off the audio thread on parameter
  change (at most 30 Hz). The pitch curve is drawn on a log-Hz axis; handles drag the envelope times and amounts.
- **Values:** always visible (no hover-only panels), monospaced, with the note and cents beside every Hz value.
- **The channel rack's pad rows** get the same output badge and choke chip. Selecting a pad in the rack selects it
  here.

---

## 9. Test plan hints

The standing rules apply: null tests before engine wiring, an ADR per decision, a schema bump with a fixture, undo
fuzz, both locales.

1. **Block-size independence:**
   - extend `pulsus_is_block_size_independent` to every new voice and model, with blocks
     [1, 7, 64, 480, 1000, 4096, 8192] and 0.0 tolerance;
   - **add continuous-controller cases**: a CC4 ramp and a pedal close on a sounding open hat, with the event
     splitting blocks as the processor does. Any smoother must start at the event sample, never at the block
     start.
2. **Determinism:**
   - the same kit and seed give an identical render;
   - a **different seed gives a different render** (so the seed is actually used);
   - modal voices are covered too, because their resonator states must reset on `reset()`.
3. **Position-derived randomness (fixes finding 0.8):**
   - render bars 1–8, then render starting at bar 5 after a seek;
   - bars 5–8 must be **bit-identical** for noise, round-robin (Random and No-repeat) and humanise;
   - the seed = hash(kit seed, PadId, note tick or NoteId);
   - **Cycle** RR is by definition history-dependent. Derive its index from the note's ordinal *within the
     clip*, computed in `Snapshot::compile`, so it too survives a seek.
   - Today's `hits[pad]` counter fails this test by design; write the test first and watch it fail.
4. **Round-robin behaviour:**
   - Cycle visits every layer of a group in order;
   - No-repeat never repeats consecutively (property test over 10k hits);
   - Random is uniform within a χ² bound;
   - layer changes are click-free (no step above −60 dB at the switch).
5. **Choke timing:**
   - with fade F, the choked voice's residue (the existing subtraction trick) is ≥ 60 dB down at F + 1 sample and
     not before F/2, for F in {1, 4, 50, 250} ms;
   - polyphony 1 behaves as a self-choke;
   - choke on note-off fires at the note-off sample;
   - a pad in group 0 is never choked (keep the existing test).
6. **Headroom:**
   - keep `the_whole_kit_at_full_velocity_leaves_headroom`, extended to the new default kit (with clap, tom and
     cymbal): peak < 0.95, the loudest single pad < 0.6;
   - **drive at 1.0 must not exceed +12 dB over drive 0 at the tail** (today's reasoning on `saturate`);
   - transient +24 dB is clamped by a soft ceiling.
7. **Tuning:**
   - a kick at Tune = 0 with key follow lands its tail (after 5 × the pitch decay) within ±2 cents of the stated
     note (by FFT peak or zero-crossing period);
   - a modal tom's fundamental is within ±5 cents at every Material;
   - a sample at Tune +12 plays at 2× speed (keep the existing test).
8. **Velocity:**
   - loudness is monotonic in velocity for every pad at → Level 1–200 %;
   - at 0 % the level is constant;
   - equal-power crossfades keep the RMS within ±1 dB across a crossfade zone.
9. **Aliasing:** a hat at +24 st and a driven kick rendered at 48 kHz and at 192 kHz (then decimated) agree to
   within −60 dB in the band 0–20 kHz, except in Raw drive mode, which is tested only for finiteness.
10. **Routing null test:** a kit with all pads on *Kit*, against the same kit with every pad **routed to its own
    unity strip under a unity kit bus**, must render **bit-identically** at the master. This is the same shape as
    the 8 → 9 strip migration test. Mute and solo of pad strips are checked against muting the pad.
11. **Migration:** a v13 project with today's seven-pad kit and a sample pad opens as v14 and **renders
    bit-identically** (a pinned SHA-256, as for `v5.cantus`). `PadId`s are stable across an insert of a lower pad
    (automation still targets the right pad).
12. **Undo fuzz:** add `InsertLayer`, `RemoveLayer`, `SetLayerField`, `SetLayerSample`, route changes,
    snapshot and morph edits, and role changes to the 40-seed fuzz. A voice or model switch must be exactly
    invertible; remember the decay lesson in `PadField::set`.
13. **Real-time safety:** no allocation in `note_on` or `process_add` with 48 voices and 16 outputs (counting
    allocator); the stealing test extended to modal voices.
14. **UI:** shape-count and LOD tests for the pad grid (as for the rack), and contrast tests: every cell edge, choke
    chip and badge ≥ 3:1 in Paper, Parchment and Ink & Gold.

---

## 10. Sources

**Makers: products, manuals, release notes**
- NI Battery 4 product page: https://www.native-instruments.com/en/products/komplete/drums/battery-4/
- Battery manual: cell matrix, main page, setup page, editor page, modulation, effects, master, quick access:
  https://docs.native-instruments.com/ni-tech-manuals/battery-manual/en/cell-matrix (and the sibling pages
  `main-page`, `setup-page`, `editor-page`, `modulation-page`, `effects-page`, `master-page`, `quick-access-area`,
  `overview-of-battery`)
- KVR Battery listing (version 4.3.1): https://www.kvraudio.com/product/battery-by-native-instruments
- XLN XO product page: https://www.xlnaudio.com/products/xo
- XLN release notes (XO): https://www.xlnaudio.com/release_notes?filter=xo
- XLN release notes (AD2): https://www.xlnaudio.com/release_notes?filter=addictive_drums
- XO getting started (PDF): https://assets.xlnaudio.com/documents/xo-getting-started.pdf
- AD2 Edit page (support): https://support.xlnaudio.com/hc/en-us/articles/16593189628957-Interface-overview-Edit-Page-Top-Kit-piece-Select-Controls
- AD2 e-drum hi-hat: https://support.xlnaudio.com/hc/en-us/articles/11720576066973-E-drum-hihat-setup-for-Addictive-Drums-2
- AD2 2.6: https://www.xlnaudio.com/blog/addictive-drums-2-whats-new-in-version-2-6
- AD2 product page: https://www.xlnaudio.com/products/addictive_drums_2
- Superior Drummer 3: https://www.toontrack.com/product/superior-drummer-3/
- SD3 release notes: https://www.toontrack.com/release-notes/superior-drummer-3/
- EZdrummer 3: https://www.toontrack.com/product/ezdrummer-3/
- EZdrummer 3 release notes: https://www.toontrack.com/release-notes/ezdrummer-3/
- FPC manual: https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/plugins/FPC.htm
- Ableton manual, Racks: https://www.ableton.com/en/manual/instrument-drum-and-effect-racks/
- Ableton, all new features in Live 12: https://www.ableton.com/en/live/all-new-features/
- Ableton, Drum Sampler: https://www.ableton.com/en/blog/drum-sampler-exploring-live-121s-new-device-with-ned-rush/
- Ableton, Live 12.3: https://www.ableton.com/en/blog/live-12-3-is-here/
- Synth Anatomy, Live 12.4: https://synthanatomy.com/2026/04/ableton-live-12-stay-creative-and-in-focus-an-overview-of-the-new-features.html
- Maschine 3: https://www.native-instruments.com/products/maschine-3
- Maschine 3.7 (Sonicstate, 24 Sep 2026): https://sonicstate.com/news/2026/09/24/native-instruments-launches-maschine-37-update-/
- Microtonic: https://soniccharge.com/microtonic
- Microtonic user guide (mirror): https://manualmachine.com/soniccharge/microtonic/1791604-user-manual/
- Kick 3: https://www.sonicacademy.com/products/kick-3
- PunchBox 2: https://d16.pl/punchbox
- Nepheton 2: https://d16.pl/nepheton
- KVR, Nepheton 2: https://www.kvraudio.com/news/d16-group-releases-nepheton-2-with-intro-offer-59473
- Roland Cloud TR-808: https://www.roland.com/global/products/rc_tr-808/
- Roland Cloud TR-909: https://www.roland.com/global/products/rc_tr-909/
- Algonaut Atlas 2 manual: https://algonaut.audio/manuals/atlas/2_0_2/sample_map.html
- Emergent Drums 2: https://audialab.com/products/emergent-drums-2/

**Reviews and press**
- MusicRadar, XO (2019): https://www.musicradar.com/reviews/xln-audio-xo
- Splice blog, how to use XO: https://splice.com/blog/how-to-use-xln-audio-xo/
- Sound on Sound, Superior Drummer 3: https://www.soundonsound.com/reviews/toontrack-superior-drummer-3
- MusicTech, Superior Drummer 3: https://musictech.com/reviews/toontrack-superior-drummer-3-review/
- MusicTech, Kick 3 (Jan 2025): https://musictech.com/reviews/plug-ins/sonic-academy-kick-3-review/
- CDM, PunchBox 2 (Jun 2026): https://cdm.link/d16-punchbox-2-kick-review/
- Dubspot, Battery 4 (2026): https://blog.dubspot.com/plugins/battery-4
- Sound on Sound, Atlas 2: https://www.soundonsound.com/reviews/algonaut-atlas-2
- Bedroom Producers Blog, TR-909 price: https://bedroomproducersblog.com/2025/09/15/roland-909-plugin/
- Musco, FPC (Mar 2026): https://www.michaelmusco.com/2026/03/fl-studio-fpc-review.html
- LANDR, best drum VSTs 2026: https://blog.landr.com/best-drum-vst-plugins/
- MusicTech, best kick plug-ins 2026: https://musictech.com/guides/buyers-guide/best-kick-drum-plugins-2026-tested-reviewed/
- Magnetic, drum generators 2026: https://magneticmag.com/2026/04/best-drum-and-instrument-generators/

**Forums** (opinions; see the caveat in §6)
- KVR, XO thread: https://www.kvraudio.com/forum/viewtopic.php?t=539926&start=75
- Toontrack forum, CC4 on e-drum hats: https://www.toontrack.com/forums/topic/td3kw-what-does-midi-cc4-of-the-cy5-hihat-do-exactly/

**Synthesis technique**
- Gordon Reid, *Synth Secrets*, SOS, "Practical Bass Drum Synthesis": https://www.soundonsound.com/techniques/practical-bass-drum-synthesis
- … "Practical Snare Drum Synthesis": https://www.soundonsound.com/techniques/practical-snare-drum-synthesis
- … "Practical Cymbal Synthesis": https://www.soundonsound.com/techniques/practical-cymbal-synthesis
- … "Synthesizing Drums: The Bass Drum": https://www.soundonsound.com/techniques/synthesizing-drums-bass-drum
- Wikipedia, Roland TR-909 (6-bit cymbals, the design team): https://en.wikipedia.org/wiki/Roland_TR-909

**Literature** (cited from the reference literature; not re-read for this report unless linked)
- K. J. Werner, J. S. Abel, J. O. Smith, "A Physically-Informed, Circuit-Bendable, Digital Model of the Roland TR-808
  Bass Drum Circuit", DAFx-14: https://www.dafx.de/paper-archive/details.php?id=hwkd1Lfe8vmtOxCEjsNY-g
- K. J. Werner, J. S. Abel, J. O. Smith, "The TR-808 Cymbal: a Physically-Informed, Circuit-Bendable, Digital Model",
  ICMC/SMC 2014, pp. 1453–1460: https://pure.qub.ac.uk/en/publications/the-tr-808-cymbal-a-physically-informed-circuit-bendable-digital-
- K. J. Werner, J. S. Abel, J. O. Smith, "More Cowbell: a Physically-Informed, Circuit-Bendable, Digital Model of the
  TR-808 Cowbell", AES Convention 2014.
- N. H. Fletcher, T. D. Rossing, *The Physics of Musical Instruments*, 2nd ed., Springer 1998 (membrane modes,
  timpani).
- T. D. Rossing, *Science of Percussion Instruments*, World Scientific 2000.
- T. D. Rossing, I. Bork, H. Zhao, D. O. Fystrom, "Acoustics of snare drums", JASA 92(1), 1992.
- S. Bilbao, *Numerical Sound Synthesis*, Wiley 2009.
- S. Bilbao, "Percussion synthesis based on models of nonlinear shell vibration", IEEE TASLP 2010.
- S. Bilbao, "Time domain simulation and sound synthesis for the snare drum", JASA 131(1), 2012.
- F. Avanzini, R. Marogna, "A modular physically based approach to the sound synthesis of membrane percussion
  instruments", IEEE TASLP 18(4), 2010.
- F. Avanzini, R. Marogna, B. Bank, "Efficient synthesis of tension modulation in strings and membranes based on energy
  estimation", JASA 131(1), 2012.
- K. H. Hunt, F. R. E. Crossley, "Coefficient of restitution interpreted as damping in vibroimpact", J. Appl. Mech.
  1975.
- A. Chaigne, C. Touzé, O. Thomas, "Nonlinear vibrations and chaos in gongs and cymbals", Acoust. Sci. & Tech. 26(5),
  2005.
- K. van den Doel, D. K. Pai, "The sounds of physical shapes", Presence 7(4), 1998 (modal synthesis).
- H. Hennig et al., "The nature and perception of fluctuations in human musical rhythms", PLoS ONE 6(10), 2011.
- L. van der Maaten, G. Hinton, "Visualizing data using t-SNE", JMLR 9, 2008.
- L. McInnes, J. Healy, J. Melville, "UMAP", arXiv 1802.03426, 2018.
- Google Creative Lab, *The Infinite Drum Machine* (2016), an AI Experiment (t-SNE of drum sounds).
