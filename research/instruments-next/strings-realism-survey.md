# What makes virtual strings sound real: a survey of the best products, and what Arcus should add next

*2026-11-25. The owner asked (translated): "Evaluate digital string instruments like ours. What do
they take into account to sound very real — articulations, or what else? We are going to implement
more things for more realism." This file surveys the best modelled, sampled, hybrid and neural
strings. It reads the underlying literature briefly, compares all of it with Arcus as it stands
(ADR 0157, ADR 0158, `docs/round42/arcus-match.md` passes 1–6, BACKLOG §3e), and ends with a ranked
plan. It writes no code.*

It builds on three earlier studies and does not repeat them:

- `research/instruments-next/strings-verified.md`: patents, licences and the first product scan.
- `research/instruments-next/bowed-realism.md`: the physics, ranked, with the measured reference
  database `bowed-refdb/`.
- `research/instruments-next/notes.md`.

## How to read the labels

Every claim carries one label:

| Label | Meaning |
|---|---|
| **[M]** | The product's own manual or support documentation, read for this survey |
| **[R]** | An independent review (Sound On Sound unless named) |
| **[V]** | The vendor's marketing or product page: a claim, not a measurement |
| **[L]** | Published literature (paper, thesis, patent) |
| **[I]** | My inference: reasoning from the sources, not a source |
| **NV** | Could not be verified today: from a search summary, a secondary citation or memory |

Sources are listed with URLs in §10. Nothing was downloaded except two public PDF manuals: SWAM
Solo Strings v3.0.0 and Spitfire Solo Strings v3.3. They were read as text in a scratch folder and
not kept in the repository. No audio was fetched, and no proprietary content is copied here beyond
short quotations.

---

## 0. The short version

### 0.1 What the best products have in common

1. **Realism is spent on the joins, not the sustain.** Every premium library records or models
   several kinds of note-to-note connection:
   - fingered, bowed (re-bow), portamento, runs [M Spitfire];
   - slurred, agile, fast runs and portamento, chosen by playing speed [R Berlin Strings];
   - regular against agile legato, switched by an "auto-speed" controller [V VSL];
   - re-bow samples on repeated legato notes [R CSS];
   - same-string slurs, cross-string legato and split portamento [M SWAM].

   Arcus has one join: a 30 ms finger change with a 3 dB dip, the same for every interval,
   direction and string. **This is the largest gap after the player work now in progress.**
2. **Interpretation has moved from key-switches to inference.** Products read velocity, overlap,
   playing speed and note length:
   - Berlin's adaptive legato, Spitfire's "Total Performance" and its triggers "by speed of
     playing", VSL's auto-speed, Embertone's predictive script and PolySWAM's "zero keyswitches";
   - Dreamtonics' 2026 *Instrument X*, with "Smart Articulation" by pitch and note length;
   - NotePerformer's ~1 s look-ahead.

   **For samples, inference costs latency.** CSS's legato waits ~100 / 250 / 333 ms. Spitfire's
   "Tightness" control trades realism for timing. The Joshua Bell Violin's predictive script is
   sometimes audibly late. SWAM's automatic double-stop detection adds latency. Arcus can infer at
   **0 ms live**, and with **look-ahead on playback** because Cantus is the DAW (Lindemann's
   look-ahead patent has expired). Offline also equals real time. This is a real advantage, and
   Arcus does not yet use it beyond tying legato notes.
3. **The left hand is modelled, not only the bow.** SWAM's central legato control is *which
   string the note is on*: Alternate Fingering decides same-string slur, cross-string legato or a
   split portamento [M]. SWAM never puts vibrato on an open string [M]. Sample Modeling was faulted
   because its open strings take vibrato [R]. **Arcus always plays in first position** (the highest
   string whose open pitch is at or under the key). It has no rule for open strings, no shift on
   the same string and no cross-string legato.
4. **Variation is engineered:**
   - round robins, plus "neighbouring zones", which borrow adjacent keys' takes to get up to 24
     variants from 8 [M Spitfire];
   - per-player tuning and delay curves [M VSL Humanize];
   - "Random Bow Amount" (pressure, speed, position) and "Random Finger" (pitch) [M SWAM].

   Arcus varies its vibrato per note and per cycle. Its bow, its intonation and the tone inside a
   note do not vary.
5. **The room and the radiation are part of the instrument:**
   - every sampled library ships 3–13 microphone positions;
   - SWAM added a room ("Ambiente") and a "Dyn" pan that "emulates small movements of the player"
     [M];
   - Instrument X models the microphones [V].

   Arcus is dry and mono, and its ABX references are recorded in rooms. Two things are cheap in a
   modal model and hard for samples: directional tone colour (Weinreich 1997: above ~850 Hz the
   radiation pattern changes fast with angle and frequency, and vibrato sweeps it) and the
   sympathetic ring of the open strings (SWAM has an "Open Strings" control).
6. **Modelled instruments are praised for playability and faulted on three things:**
   - secondary techniques: col legno "weedy", pizzicato "modest", sordino "too subtle"
     [R SWAM, SWAM Sections];
   - shrill high violins [R];
   - the skill they ask of the player [R].

   Arcus already avoids silence without a controller, and the player layer in progress answers the
   skill. The techniques that physics gives almost free are the opening: ponticello, tasto,
   flautando, harmonics and sordino.

### 0.2 The plan in one table (details in §7)

**In progress elsewhere; not in this plan:**

- the bow's position along the stick (frog to tip);
- the bow direction;
- on-string against off-string attacks;
- the automatic player layer: bow alternation, vibrato onset and depth by note length and
  dynamic, portamento on shifts, humanised timing.

Several items below **extend** that layer and must be coordinated with it.

| # | Addition | Realism | Effort | Fits modelling? |
|---|---|---|---|---|
| 1 | **Note-to-note transitions by interval, direction, string and speed.** A finger's hammer when the note rises, its lift when it falls, a cross-string slur that lets the old string ring, a shift glide, and joins that get shorter in fast runs. Fitted to the measured phrases. | high | M | yes: native |
| 2 | **A fingering engine**: string and position per note, same-string shifts, open-string rules (no vibrato, longer ring), "sul G/C/D/A" marks | high | M | yes: native |
| 3 | **The attack shaped like a bow stroke.** The force is placed before the motion, with each stroke type's measured force and speed contours. This closes `t_steady` and the noise burst. | high | M | yes |
| 4 | **Directional radiation** (two or more radiation directions from the modal body) and an early-reflection stage, the first step of round 43's stage | high | M | yes: native |
| 5 | **Sympathetic open strings** and the string's ring after the bow lifts | med-high | S–M | yes: native |
| 6 | **Variation inside a note.** A slow drift of bow force, speed and β, intonation scatter, and the pitch settling at the onset, all seeded. | med-high | S | yes |
| 7 | **Look-ahead planning on playback.** Slurs and shifts start before the beat. Strong beats get down-bows. Bow is shared out over long notes. Vibrato stops before a shift. | med-high | S–M | yes: Arcus's own advantage |
| 8 | **Speed-aware choices**: the joins in fast runs, the bow moving to its upper half, sautillé or spiccato on fast repeated notes, chosen by tempo | med-high | S–M | yes |
| 9 | **The techniques physics gives**: sul ponticello, sul tasto, flautando, natural and artificial harmonics, sordino | medium | M | yes: native |
| 10 | **The portamento's shape and kinds** (S-curve, level dip, vibrato off while sliding; Flesch's B and L shifts), with velocity and CC control | medium | S | yes |
| 11 | **Brightness with dynamic** (the loud centroid; open item) | medium | M–L | yes (uncertain) |
| 12 | **Col legno** (battuto and tratto), **Bartók** pizzicato, and the pluck's thump and brightness through a fingerboard contact | medium | M | yes: new exciters |
| 13 | **Double stops and chords** on adjacent strings; three- and four-note chords rolled | medium | S–M | yes |
| 14 | **Fingered trills** (hammer and lift, measured rate), measured tremolo locked to the tempo, ricochet | low-med | S | yes |
| 15 | **Sections**: distinct seeded players, auto-divisi, out-of-phase bowing (round 43, already planned) | high (orchestral) | L | yes |

A **hybrid** voice (recorded attacks under the modelled sustain) is not in the top 15. It is
warranted only if item 3 fails, and it carries Yamaha's US 8,729,376 risk if a recording is fed
into the loop (`strings-verified.md` §1, row 19).

---

## 1. The products, one by one

Each entry gives what the product models or samples and how it does it. It then covers the
aspects of the brief that its sources speak to, and what reviewers praise and fault. Where a
source is silent on an aspect, the entry says nothing about it rather than guess.

### 1.1 Modelled

#### Audio Modeling SWAM Solo Strings v3 (violin, viola, cello, double bass)

**What it is.** "The sound is not produced by playing back samples, using pre-recorded
articulations"; it is SWAM ("Synchronous Wave Acoustic Modeling", Stefano Lucato) "combined with
Physical Modeling" [M, manual v3.0.0 p.5]. The current version is 3.12.4 (2026-10-08;
`strings-verified.md` §4.1).

**Dynamics.** One continuous **Expression** control: "on bowed strings, it's mostly related to the
bow speed which in turn controls the dynamics" [M]. Without an expression controller, "THE
INSTRUMENT WILL NOT WORK when played in realtime" [M]. In the studio an expression curve is
required.

**Articulations: played, not switched.** "Almost all articulations are performed by the
combination of the three principal controls: note-on velocity, expression, and bow-pressure"
[M §"How to perform the main articulations"]:

| Articulation | How it is played |
|---|---|
| Détaché | Notes separated with the sustain pedal down |
| Slurred legato | Overlap, high velocity, same string |
| Cross-string legato | Overlap, high velocity, another string |
| Portamento | Overlap, low velocity. The threshold is set by "Portamento Max Time"; "Portamento Control" chooses velocity or a CC. |
| Martelé | "Bow Lift" On String, high velocity and expression, then the expression falls. For a scratchier bite, bow pressure 0.65–0.80 before the attack. |
| Spiccato | Bow Lift Off String, short notes |
| Flautando | Very low bow pressure |
| Scratch | Maximum pressure |
| Tremolo | Off / Slow / Fast (1.5×). "Hz", "Sync" or "Sync/Acc" (an accent every 3 or 4 strokes). Manual tremolo by key-switch, or with the sustain pedal (a bow change on note-on and note-off). |
| Harmonics | 2, 3, or "4 Control" (a chosen overtone) |
| Play Mode | Bow, Pizzicato, Col Legno |
| Sordino | On / off |

Combinations are allowed: "tremolo/vibrato, tremolo/glissando, glissando/vibrato,
glissando/harmonics/crescendo" [M].

**The left hand** [M, "Play Modes ▸ Left Hand", "Advanced"]:

- *Alternate Fingering* (Mid, Bridge, Nut+Open) chooses the hand's position and so **which string
  the note is on**. Set before the note, it decides between a slurred and a cross-string legato, and
  between a continuous and a "split" portamento across two strings. *PortamSplit Ratio* sets where
  a split portamento changes string.
- "Vibrato is not possible on open strings", which are reachable only in Nut+Open.
- *Random Finger* varies the finger's position, and so the pitch.
- *Temperament* spreads or compresses the intonation.
- A *String Model* is either "Real" (four strings of fixed thickness and tension) or "Virtual
  Adaptive Resizing" (one string re-sized per note, for bends across the whole range).

**Polyphony** [M]:

- *Mono String Crossing* stops the old string's ring on chosen intervals, so a transition does not
  leave a "bichord". *Mono CrossString Muting* lists them: 1 or 2 semitones, "Exclude Harm", or
  All.
- *Mono Poly Release* lets the old string ring on.
- *Double* and *Double/Hold* play double stops, "always on adjacent strings", with a preferred
  string pair.
- *Auto* detects double stops and adds "an additional latency … at the attack" to do so.

**The bow** [M]:

- *Gesture Mode*:
  - **Expression**: bow speed relative to pressure; an automatic bow change when the expression
    rests at zero.
  - **Bipolar**: the controller's sign is the bow's direction.
  - **Bowing**: the controller is the bow's **displacement**, and its rate of change the speed.
- *Bow Start* (down or up, for staccato).
- *Bow Lift* (off or on the string, mainly at the release).
- *Bow Pressure Accent* (by velocity).
- *Attack Control* (Vel Soft / Vel Hard / Expression / Mix) and *Attack Ramp Speed*.
- *Interactive Bow Compensation* corrects the pressure "to maintain a good vibrating sound".
- *Random Bow Amount* varies the pressure, speed and position.
- *Dynamic Transitions* shapes the expression through a legato.
- *Advanced Legato*: off is "a fast sweep transition"; or "from 4th" / "from 5th", where a
  "sophisticated model" takes over for intervals of that size or larger.

**Timbre and room** [M]:

- *Rosin* (the stickiness), *Bow Noise*, *Pizzicato Tone*, *Pizzicato Fingerboard Interference*;
- *Timbral Correction* (gain on chosen harmonics);
- *String Resonance* ("release time, open strings vibration");
- **Open Strings**: "amount of sympathetic effect (resonance) produced by open strings";
- an EQ, early reflections, and a reverb;
- the v3 **"Ambiente"** room [M, the v3 knowledge-base article];
- *Panpot Type* Dyn1 and Dyn2, which "emulates small movements of the player".

**Vibrato** [M]: depth, rate, *Vibrato Rate Rand*, and *Vibrato Fade In* (ms after each note-on).

**Control** [M]:

- an MPE profile;
- a breath-controller mode, in which a new attack can come from the breath envelope while a note
  is held;
- high-resolution CC and NRPN;
- key-switches on their own channel.

**Reviewers.** Praised as "some of the most playable instruments I've come across" [R SOS 2019].
The faults [R SOS; KVR, from `strings-verified.md` §4.1]:

- col legno "weedy";
- pizzicato "modest";
- the bass stops at E1;
- "takes a lot of work";
- "stiff";
- weak bow control.

#### SWAM String Sections and PolySWAM

- **String Sections** [R SOS May 2024]:
  - The players: 4–6 modelled violins, 3–5 of each other instrument "per divisi".
  - The ensemble: separate **timing and pitch precision** settings, which replaced one "Players
    Accuracy" knob; a **Divisi Anti-Phasing** parameter for several instances of one section; and
    one Room Simulator for every section (size, distance, angle, microphone proximity).
  - The cost: ~350 MB of RAM an instance and "high CPU load with multiple instances".
  - The faults: shrill long high violins, no col legno, a too-subtle sordino and unwanted
    portamento.
  - The comparison with samples: modelling lets you "perform the instrument"; samples hand you a
    player's recorded phrasing.
- **PolySWAM** [V, product page; released 2026-10-05, $399]:
  - "intelligent Voice Allocation (Complete, Multi, Mono)" routes notes to players by register in
    real time (automatic divisi);
  - "zero keyswitches": sustains, legatos, staccatos and swells come from touch and controllers;
  - a "Conductor" panel holds the technique buttons: con sordino, pizzicato, tremolo;
  - the stage: Stage Placement and Ambiente rooms.

#### Pianoteq (Modartt)

There is no bowed instrument. The product list holds pianos, harps, a classical guitar (Pianoteq
8, "complete with emulated gestures and natural instrument noises" [V, KVR news]) and percussion.
A Modartt forum moderator wrote that strings "would probably require new mathematical models from
scratch" (an old post, NV as current policy). The lesson for Arcus is the guitar's: Modartt models
**gestures and the instrument's incidental noises** as part of the instrument.

#### Other physical models of the bowed string

The STK bowed string, the NESS project (Edinburgh), Logic's Sculpture and AAS's Chromaphone are
research tools or generic exciter–resonator synthesizers. None is an orchestral solo instrument
with a player model [I]. The academic state of the art is in §4.2.

### 1.2 Hybrid: samples driven like a model

#### Sample Modeling Solo & Ensemble Strings

[R SOS, node 4921022]

- **The samples.** Anechoic recordings with "harmonic alignment", so the layers crossfade "pp to
  ff" without phasing.
- **The body and the tone.** Body impulse responses, and timbral shaping by harmonic number rather
  than by fixed frequency.
- **The ensemble.** **Continuously variable** in size, with a control for how far the players'
  vibratos run in step.
- **The controls:**
  - CC11 sets the dynamics, and the instrument is silent until it moves;
  - CC1 sets the vibrato depth and overrides the automatic vibrato;
  - CC38 sets the attack;
  - velocity sets the legato's speed, and a low velocity gives portamento;
  - the sustain pedal re-bows.
- **The key-switches.** Pizzicato, tremolo, col legno, harmonics, double stops, portato, open
  strings and the playing position.
- **The faults.** "Open strings respond to vibrato", which is unrealistic. CC mapping is "a bit
  flaky". Without a breath controller it is hard to drive the dynamics and the vibrato together.

#### Aaron Venture, the Infinite Series

The developer publishes **Infinite Woodwinds** (2019; v2.0 2020) and **Infinite Brass** (v1.6
2021) [KVR developer page]. **No strings product was found** (two searches; a 2016 forum mention of
"Strings 2020" is not evidence of a release).

The series' stated aims [V]:

- expression and playability that remove "the need for keyswitching and articulation-based
  approach";
- several microphone positions;
- "flexible instrument positioning for variable and custom section sizes".

Forum descriptions of the method are "ultra dry, clean, phase-aligned samples" with heavy
scripting (NV). The approach is Sample Modeling's: section size and seating come from one dry,
aligned player, placed in a room by processing.

#### Embertone: Friedlander Violin, Fischer Viola, Joshua Bell Violin (and Blakus Cello)

- **Friedlander Violin** [R SOS 2016]:
  - It is recorded **without vibrato**, and the vibrato comes from "multiple scripted background
    modulators" with style presets ("gentle", "passionate").
  - Two interval legato modes: bow change (the default) and slurred (while a key-switch is held).
    The transition's speed can be programmed.
  - Portamento is chosen by velocity below a programmable threshold.
  - A mod-wheel patch drives vibrato and volume together, so the vibrato grows with loudness.
  - An "Ensemble" mode of up to eight players "sounds a little synthetic".
- **Fischer Viola** [R SOS 2016]:
  - a **Color** control moves the bowing point between bridge and fingerboard;
  - its p and mf legato layers are phase-locked, so they can be crossfaded;
  - it has sordino, ponticello and tasto.
- **Joshua Bell Violin** [R SOS]:
  - **A predictive script picks the articulation.** Detached notes get a bow change, overlapped
    notes a slur, and quiet overlapped notes a portamento. Its fault: "a small lag is sometimes
    audible while the script works out what you're playing".
  - The held note comes back when an overlapping note is released, as VSL does it.
  - Its legato can be polyphonic.
  - Vibrato is sampled or scripted.
  - Sustains at four dynamics in normal, sordino, ponticello and tasto.
  - Ricochets, harmonics and a Bartók snap at high velocity; no col legno.
- **Blakus Cello**: portamento speed by velocity within a threshold (NV, search summary).

### 1.3 Sampled

#### Spitfire Audio: Solo Strings (manual v3.3), BBC Symphony Orchestra, Chamber Strings

**The continuous controllers** [M Solo Strings]. These are the de-facto standard across Spitfire's
range:

| CC | Control | What it does |
|---|---|---|
| CC1 | Dynamics | Crossfades the recorded dynamic layers |
| CC21 | Vibrato | Crossfades from senza to molto vibrato |
| CC11 | Expression | A trim on the instrument's level |
| CC17 | Release | How much release sample is heard; "helps blur the transitions … in slow passages" |
| CC18 | **Tightness** | Cuts into the note's start. See below. |

On Tightness the manual says: "We proudly cut our samples from the true beginning of the note, as
the bow engages the string … laggy and sometimes very small inconsistencies in timing … It Sounds
real!" It advises playing in tight, then loosening the control and moving the track earlier in the
DAW.

**The microphones.** Close, Tree and Ambient positions.

**The legato types** [M]:

- *fingered* ("adding or removing a finger … while the bow continues");
- *bowed* (the bow changes direction with the note, "a slightly more defined and strong change");
- *portamento*, which on large intervals also crosses strings.

**The Total Performance patch** [M]. It holds five true legatos: portamento, fingered, bowed, runs
and arpeggios.

- Velocity picks the legato: 1–19 portamento, 20–84 fingered, 85–127 bowed.
- Runs get a faster transition by playing speed.
- CC21's ranges choose the vibrato: 65–127 molto (exaggerated above 75), 33–64 *progressive*
  (the note starts without vibrato and grows into it, slowly at velocity 1–9 and quickly above),
  6–32 none, and 1–5 tremolo.
- The shorts are staccato (4 round robins) or spiccato (8 round robins) at pp, mf and ff, with a
  "noise" layer.
- A **"trillable legato"** plays a recorded trill after five alternations of a semitone or a tone
  within 200 ms.
- A mute simulation.
- An **"fp hall trigger"** adds the room when the dynamic is dropped quickly.

**Switching articulations** [M]:

- *by key-switch* (taking effect at the next note-on);
- **by playing speed** ("Spiccato … if the time between playing each interval is between 0 and
  50ms");
- by velocity range;
- by CC range (Spitfire's UACC);
- grouped triggers.

**Round robins** [M]:

- **"Neighbouring Zones"** borrows adjacent keys' takes: "for an 8RR instrument, you effectively
  cycle through up to 24 different sounding notes". In legato it "alternates between 3 legato
  intervals to give a fake round robin".
- Two round robins can be layered at −6 dB for a thicker sound.

**Techniques recorded** [M]: long, con sordino, harmonics, flautando, sul ponticello, col legno
tratto, super sul tasto, staccato, spiccato, col legno (and with a pencil), pizzicato, Bartók,
trills (minor and major second), tremolo and "tremolo whispers".

**BBC Symphony Orchestra:**

- sections of 16/14/12/10/8;
- legato with portamento and slurred legato;
- 468 techniques over 57 instruments in Pro [V];
- a "performance legato" that keeps shorts, runs and portamento in one patch [blog review, NV];
- the free *Discover* has one dynamic layer and no legato [R].

#### Cinematic Studio Strings (CSS) and Cinematic Studio Solo Strings (CSSS)

**CSS** [R SOS 2016; V product page]:

- **The players.** 10/7/7/6/5, with "Full" and "Lite" ensembles and no divisi system.
- **The legato.**
  - Standard: two speeds by velocity (21–63 medium, 64–127 fast).
  - Advanced: three speeds (21–64 slow, 65–100 medium, 101–127 fast).
  - Portamento at velocity ≤ 20.
  - **The manual's delays: ~100 / 250 / 333 ms** [R, citing the manual]. A support answer quoted
    on a forum adds 60 ms for staccato (NV). The vendor's page: each legato note is "delayed by up
    to around 1/4 of a second" [V].
- **Re-bow samples** on repeated legato notes with the sustain pedal ("sampled, not scripted" [R]).
- **The controllers.** CC1 sets the dynamics over four layers, the vibrato "intensifying" with
  them. CC2 crossfades vibrato and non-vibrato.
- **The techniques.**
  - Marcato, with a spiccato overlay by velocity.
  - Spiccato, staccatissimo, staccato, sforzando.
  - Pizzicato and Bartók, col legno, harmonics, tremolo, trills.
  - Measured tremolo (synced or free).
  - A sordino emulation over the whole library.
- **The microphones.** Close, main and room.
- **The faults.** No aleatoric effects, the measured tremolo's repeated notes "can tend to get
  lost", and col legno could use "a tad more volume".

**CSSS** [R SOS 2018]:

- 13 articulations;
- a dual-mode legato; portamento at velocity ≤ 20;
- **CC5 acts as "a portamento on/off switch, not a level control"**, so slides cannot be tamed;
- **the vibrato cannot be crossfaded**: CC2 switches at 64;
- the top dynamic "falls just short of the full passionate delivery of a virtuoso soloist";
- the attack overlays at velocity ≥ 101 are "quite prominent".

#### Vienna Symphonic Library: Synchron Strings Pro, the Synchron Player, Dimension Strings

**Synchron Strings Pro** [V product page]:

- **The articulation families.** Short, Long, Legato, **Legato agile**, Portamento, Dynamics
  (sforzato, soft swell, crescendi and diminuendi short, long and very long), Tremolo, Trills,
  Pizzicato, Harmonics and Ponticello.
- **The controllers:**
  - **Dim.Ctrl/A (CC20)** crossfades the vibrato;
  - **Dim.Ctrl/B is "auto-speed"**: it switches between regular and agile legato by playing
    speed;
  - CC3 sets the marcato attack in the tremolos;
  - the "XF" presets crossfade velocity layers on CC1.
- **Repetitions.** "Alternate starting notes" are played when the gap exceeds 200 ms, and
  "connected" repetitions at higher tempo.
- **The rest.** 13 microphones in 8 sections; "release normal/soft"; measured tremolos
  time-stretched to the tempo.

**The Synchron Player** [M, Edit tab]:

- **Humanize**: per index, a note-on **Delay** in ms and a **tuning curve** in cents, scaled by a
  Humanize Amount;
- a Velocity XFade;
- real-time time-stretch, with its stated limits: phasing with several microphones, CPU, and no
  correct legato stretching.

**Dimension Strings** [R SOS 2013]:

- **Each player is recorded separately**, mono, with a microphone on every instrument and no stereo
  pair. VSL's reason: a pair "would cause phasing and would clash with per-player tuning edits".
- Eight violins as separate patches. Players are muted to build smaller sections; two groups of
  four give a real two-note divisi.
- VI Pro's Auto Voicing assigns chord notes by the order they are played, not their pitch.
- Humanize gives each player its own timing and tuning.
- "Shorts + Noise" matrices add incidental sounds.
- The portamento is praised as "fabulous".

#### Orchestral Tools: Berlin Strings, Berlin Solo Strings, Metropolis Ark

- **Berlin Strings** [R SOS]:
  - 28 players (8/6/5/5/4).
  - **Adaptive legato**: slow playing gets "slurred" transitions, faster playing "agile", very
    quick lines "fast runs", and velocities 125–127 portamento.
  - **The velocity also picks the bow attack**: soft, fast or accented.
  - Key-switches override the choice.
  - Close, Tree, Surround and A/B microphones, plus a concertmaster spot.
  - A trill "orchestrator", tempo-synced runs, played crescendi and swells, a simulated sordino.
  - No Bartók pizzicato.
  - The fault: "short and long dynamic performances are less distinct than expected".
- **Metropolis Ark 1** [R MusicTech; Equipboard]: combined high and low string ensembles built for
  power. Not built for nuance, and not studied further here.

#### 8Dio

- *Adagio Violins*: 11-, 3- and 1-player sections, with an emphasis on legato [R SOS 2012].
- *Deep Solo Violin*: "3 Advanced True Polyphonic Legatos" in several styles and speeds, with and
  without a mute [V].
- No detailed review of its vibrato or bowing controls was found.

#### Logic Pro Studio Strings (Apple)

[M, Apple's guides as returned by search]

- Articulations from a pop-up or by key-switch, in an **Articulation Set**.
- A **Legato Transitions** switch, which uses transition samples "for both monophonic and
  polyphonic modes".
- **Auto Voice Split**: the notes of a chord go to different players of the section, the first note
  to the lead voice and the others downward.
- The vibrato and dynamics controllers of Studio Strings were not confirmed (NV).

#### Others seen in the earlier scan

From `strings-verified.md` §4.1:

- **ProjectSAM Lineage Strings** (2026-06): "Adaptive Phrasing" infers the articulation from
  playing (NV).
- **Ableton's String Quartet by Spitfire**: nine MPE presets with a technique per note.
- **FL Studio** and **Bitwig**: stock strings with no legato and no expression.

### 1.4 Neural and machine-learned

- **Dreamtonics Instrument X** (August 2026). What the vendor and the news say [V; SOS news,
  Synthtopia, Magnetic Mag]:
  - It is a "Neural Acoustics Modeling platform", run locally, with a violin, a viola, a cello and
    a contrabass.
  - **Smart Articulation** chooses the technique by pitch and note length. Techniques can be set
    per note and stacked: pizzicato, col legno, sul ponticello, sul tasto, trills, tremolos,
    falls, portamentos, harmonics and a modelled sordino.
  - A **Dynamics Lane** is "mapped automatically from pitch, duration, and phrasing", with a
    manual override.
  - Five performance styles; "AI Performance Retakes" regenerate a phrase.
  - The microphones are modelled (spots, a Decca tree, room pairs). The dry instrument and the room
    are separated in training.

  No independent review of its sound was found; a VI-Control thread is sceptical (NV). The point
  for Arcus is the **direction**: the product's selling features are a performance layer
  (articulation, dynamics and variation inferred from the score), not an oscillator [I].
- **MIDI-DDSP** (Wu et al., ICLR 2022) [L]. A hierarchy of notes → note expression → synthesis
  parameters → audio, trained on URMP, which includes violin. Each note carries six expression
  attributes: volume, volume fluctuation, volume peak position, vibrato, **brightness** (the mean
  harmonic centroid) and **attack noise** (the noise in the first ~40 ms). The authors observe
  that "attack noise can determine whether two notes sound consecutively or separately". That is
  a measured statement of where legato lives.
- **DDSP** (Engel et al., ICLR 2020): the violin timbre-transfer demonstrations (NV, from memory).
- **NotePerformer** (Wallander): a notation playback engine that reads about **one second ahead**
  to shape phrasing. Its developer calls it symbolic, rule-based AI [Wikipedia].

**What this means for Arcus** [I]: neural synthesis is not needed, because the string, the bow and
the body are already physical. What these systems show is that realism now comes from **note-level
expression predicted from context**. A statistical player layer fitted to measured bowing (Maestre
et al. 2010, §4.3) is the modelling counterpart.

---

## 2. Across the products, aspect by aspect

This is the brief's checklist, answered across the field. "Modelled" means SWAM; "hybrid" means
Sample Modeling, Embertone and Aaron Venture; "sampled" means the rest. Arcus's column is in §6.

| Aspect | What the best do | Who does it best (source) |
|---|---|---|
| **Articulation set** | Sustain (vibrato, non-vibrato, espressivo, flautando, sordino, ponticello, tasto); legato; shorts (staccato, staccatissimo, spiccato, martelé, marcato, sforzando); tremolo (free and measured); trills (m2, M2); pizzicato and Bartók; col legno (battuto and tratto); harmonics; played dynamics (swells, crescendi, fp) | Spitfire Solo Strings (the manual's App. C); VSL Synchron Strings Pro (12 families); CSS |
| **Legato kinds** | Fingered (slur), bowed (re-bow), portamento, runs, arpeggios; cross-string against same-string | Spitfire's five in Total Performance [M]; SWAM's slurred, cross-string and split portamento [M] |
| **Legato by interval** | True recorded transitions for every interval up and down (samples). SWAM changes model above a 4th or 5th ("Advanced Legato") [M]. | CSS, Spitfire, Berlin, VSL |
| **Legato by speed** | The transition chosen by playing speed: Berlin's slurred, agile and fast runs [R]; VSL's auto-speed [V]; Spitfire's runs and speed triggers [M]. Three velocity speeds with delays of 100 / 250 / 333 ms in CSS [R]. | Berlin, VSL, Spitfire |
| **Recorded against scripted** | Recorded transitions for samples. Scripted crossfades only in the free and stock tiers: BBC SO Discover has no legato; FL, Bitwig and Ableton's Orchestral Strings have none. Logic uses transition samples. | — |
| **Dynamics** | CC1 crossfades 3–5 layers (Spitfire, CSS, VSL XF); velocity only on shorts; CC11 as a trim. SWAM's continuous Expression with no layers [M]; Sample Modeling's harmonic-aligned layers [R]. | SWAM (continuous); CSS for a "progression from silky smooth pp" [R] |
| **Velocity** | Attack strength on shorts and sustains (Berlin's soft, fast or accented bow [R]); the legato's kind or speed (Spitfire, CSS, Embertone); portamento at the extremes | — |
| **Vibrato** | Its own CC: CC21 (Spitfire), CC20 (VSL), CC2 (CSS), CC1 (Sample Modeling). A crossfade from senza to molto; a **progressive** vibrato whose onset speed follows velocity (Spitfire [M]); scripted vibrato on non-vibrato samples with styles (Embertone [R]); depth, rate, rate randomness and fade-in (SWAM [M]); none on open strings (SWAM [M]). Not crossfadable in CSSS: a fault [R]. | Spitfire, Embertone, SWAM |
| **Bow direction and changes** | Bowed legato as a recorded kind (Spitfire, Friedlander); re-bow samples (CSS); manual bow change by key-switch or sustain pedal, or bipolar and bowing gesture modes (SWAM [M]); "bow start" up or down (SWAM) | SWAM (the most complete) |
| **Bow position and pressure** | SWAM's Bow Pressure, Bow/Pizz Position (ponticello ↔ tasto) and Rosin [M]; Fischer Viola's "Color" [R]; sampled ponticello, tasto and flautando patches | SWAM |
| **Bow noise and release noise** | A Bow Noise control (SWAM); sampled shorts with a "noise" layer (Spitfire); "Shorts + Noise" matrices (VSL Dimension [R]); bow-noise reduction (Friedlander); release triggers with an amount (Spitfire CC17) | Spitfire, VSL |
| **Round robins and humanising** | 4–8 round robins on shorts; neighbouring zones; layered round robins (Spitfire [M]); alternate repetition starts above a 200 ms gap (VSL [V]); per-player delay and tuning curves (VSL Humanize [M]); random bow and finger (SWAM [M]); timing and pitch precision (SWAM Sections [R]) | VSL, SWAM |
| **Releases and the room** | Release samples with an amount (Spitfire CC17); release normal or soft (VSL); the fp hall trigger (Spitfire); String Resonance and Open Strings (SWAM [M]); rooms (SWAM Ambiente, Spitfire's microphones) | — |
| **String and position** | Alternate Fingering (SWAM [M]); a playing-position key-switch (Sample Modeling [R]); "sul G" is recorded only in some libraries; Instrument X picks the technique by pitch [V] | SWAM |
| **Portamento control** | Velocity (≤ 19–20 in Spitfire and CSS, ≥ 125 in Berlin, low velocity in SWAM and Sample Modeling); a threshold (Embertone); CC5 as a switch (CSSS: faulted [R]); a CC for its time (SWAM's Portamento Control = CC); a split point across strings (SWAM) | SWAM, Embertone |
| **Playing interpreted by script** | Adaptive legato (Berlin); Total Performance and the speed, velocity and CC triggers (Spitfire); auto-speed (VSL); a predictive script (Joshua Bell); zero key-switches (PolySWAM); Smart Articulation and a dynamics lane (Instrument X); a 1 s look-ahead (NotePerformer) | Spitfire, Berlin, Instrument X |
| **Microphones and stage** | 3–4 positions (Spitfire C/T/A, CSS, Berlin Close/Tree/Surround/AB); 13 microphones in 8 sections, up to Auro 3D (VSL); modelled rooms (SWAM, Instrument X); a Dyn pan "small movements of the player" (SWAM) | VSL; SWAM for a modelled room |
| **Ensembles and sections** | Fixed recorded sections (16/14/12/10/8 BBC; 10/7/7/6/5 CSS); separate players (VSL Dimension); modelled players with accuracy controls and anti-phasing (SWAM Sections); a continuous size (Sample Modeling); auto voice split (Logic) and voice allocation (PolySWAM); a seat per player (Aaron Venture's aims [V]) | VSL Dimension (real divisi); PolySWAM and Logic (automatic) |
| **MPE and breath** | SWAM's MPE profile and breath mode [M]; Ableton String Quartet's MPE presets; Sample Modeling for wind controllers | SWAM |
| **Latency and look-ahead** | Samples delay legato by 100–333 ms (CSS) or let you "tighten" and offset (Spitfire); SWAM is near zero but adds latency for auto double stops; the Joshua Bell script is "sometimes" late; NotePerformer delays by a second on purpose | Modelled voices, with look-ahead where a score exists |

**What reviewers single out** [R]:

- **Praised:**
  - legato and portamento (Dimension Strings' "fabulous" portamento; CSS "very effective");
  - a natural dynamic progression (CSS's pp → ff);
  - playability (SWAM);
  - consistency (CSS's basses);
  - "exquisite" tuning (Joshua Bell).
- **Faulted:**
  - unwanted portamento (SWAM Sections, BBC SO Core, CSSS's CC5 switch);
  - weak secondary techniques (SWAM's col legno and pizzicato, the sordino);
  - shrill high violins (SWAM);
  - legato latency (CSS);
  - script lag (Joshua Bell);
  - vibrato on open strings (Sample Modeling);
  - no vibrato crossfade (CSSS);
  - single-layer free tiers (BBC Discover).

---

## 3. What the field agrees on: the realism stack

These are inferences drawn from §1–2 [I], ordered by how consistently the products invest in
them:

1. **Transitions and gestures before the sustain.** Every flagship spends its effort on joins:
   kinds of legato, re-bows, repetitions, runs. The steady tone is the part that a good multi-layer
   recording, or a good model, gets right first. MIDI-DDSP's finding that attack noise decides
   "connected or separate" puts a measured number on what the products bet on.
2. **The articulation is inferred from playing, and continuous controls stay separate.** The
   inference uses velocity, overlap, speed and length. The separate controls are dynamics (CC1),
   vibrato (CC21, CC20 or CC2), a trim (CC11), release (CC17) and tightness (CC18). Key-switches
   remain only as overrides.
3. **The left hand is half the instrument:**
   - the string chosen;
   - same-string against cross-string joins;
   - open strings without vibrato;
   - double stops only on adjacent strings.

   SWAM, the only full model, makes these explicit. Sample libraries contain them implicitly,
   because the players chose a fingering.
4. **Variation is engineered,** because a static repetition is the first thing a listener catches:
   round robins, neighbouring zones, humanise curves, random bow and finger.
5. **The room, the microphones and the player's movement belong to the instrument.**
6. **Sections are not copies.** Separate players (Dimension), accuracy controls and anti-phasing
   (SWAM Sections), and voice allocation (PolySWAM, Logic) all exist because copies phase.
7. **The price of samples is latency and memory.** The price of models is the skill they ask and
   the techniques they lack. A model with an automatic player layer and a full set of techniques
   would sit where no current product sits [I].

---

## 4. The literature, briefly

The physics and the measured data are in `bowed-realism.md` §3 and `strings-verified.md` §3. Here
are only the findings that bear on the plan.

### 4.1 What makes a bowed string sound real to listeners

- **The attack's onset.** Guettler & Askenfelt (1997, *JASA* 101) are the reference. Listeners
  (20 string players) accepted pre-Helmholtz transients up to **~50 ms** with prolonged periods and
  **~90 ms** with multiple flyback [L]. Arcus's attack gate is built on this.

  The players' own notes are **slower to settle** than that limit. The study measured `t_steady`
  of 0.24–0.42 s across real cellos (`arcus-match.md` pass 6). A real note is therefore not "a
  clean Helmholtz start": it is a gesture whose force and speed build over a quarter of a second
  inside the acceptable region.
- **Vibrato is heard through its AM.**
  - Mellody & Wakefield (2000, *JASA* 107): removing the frequency modulation changed little;
    removing each partial's amplitude modulation changed the sound markedly [L].
  - Fletcher & Sanders (1967): partials swing in opposite phase; open strings ring in sympathy at
    the rate [L].
  - Meyer (1992) and Almeida, Schubert & Wolfe (2021, *Music Perception*) on timbre vibrato [L].
  - Wolfe's UNSW measurements show harmonics 2, 3, 5 and 13 several dB stronger at the bottom of a
    vibrato cycle than at the top [L].
- **Radiation and directional tone colour.** Weinreich (1997, *JASA* 101(4)): above about
  **850 Hz** a violin's radiation pattern changes very fast with angle and with frequency. Vibrato
  therefore sweeps each partial through directional lobes, which gives a spatial and timbral
  shimmer. The effect averages out in a section, which helps a solo violin stand out [L; the
  abstract and secondary accounts, the full text NV]. **A dry mono model has none of this** [I].
- **The body is perceived coarsely.** Fritz, Cross, Moore & Woodhouse (2007, *JASA* 122):
  trained listeners' thresholds were ~3–6 dB for a mode's or a band's level and 1.5–20 % for its
  frequency [L]. Arcus's measured bodies are already within reach of this.
- **The bow's noise and the attack's noise.** Chafe (1990) showed that bow noise is pulsed by the
  slip (`bowed-realism.md` §3.2). MIDI-DDSP links attack noise with the perception of notes as
  connected or separate [L].
- **Brightness follows force near the bridge.** Schoonderwaldt (2009, *Acta Acustica* 95): the
  spectral centroid rises mainly with bow force, more near the bridge [L, via secondary citation].
  Lampis's robot measured +0.10 octave of centroid a doubling of force (`arcus-match.md` pass 6).
- **Intonation is not equal-tempered, and it moves.** Violinists play between Pythagorean and equal
  temperament (Loosen 1993, via secondary citation [L, NV in the original]). String-quartet and
  solo studies since Greene (1937) lean Pythagorean. A model held to ±1 cent of equal temperament
  is **more exact than players** [I]. Cantus's project tuning (ADR 0155) covers the temperament,
  but not the players' scatter and context.
- **Ensembles:**
  - Rasch (1979): ~30–50 ms rms onset asynchrony in small ensembles [L, NV].
  - Wing et al. (2014): quartets correct asynchrony by near-optimal phase correction [L].
  - No pitch-spread measurement of a string section was found. Choirs scatter ~20–30 cents
    (Ternström 1993, NV) (`strings-verified.md` §3.7).

### 4.2 The bowed-string model: the state of the art

- **The junction:** McIntyre, Schumacher & Woodhouse (1983), solved in closed form. This is
  Arcus's bow. Smith's waveguides (*Physical Audio Signal Processing*) are its loop. Jaffe and
  Smith's expired legato patent (two taps crossfaded, US 5,781,461) is one ready method for a slur
  (`strings-verified.md` §1).
- **Friction:**
  - Woodhouse & Galluzzo: the hyperbolic curve is wrong in detail; the thermal model followed
    (2004 review "The bowed string as we know it today", NV title).
  - Galluzzo & Woodhouse (2014): high-performance bowing-machine transients.
  - Woodhouse & Galluzzo (2025): rate-and-state thermal friction (`bowed-realism.md` §3.3).
  - Arcus built a rate-and-state memory (pass 6). It matched Lampis's grid but lost in the
    instrument.
- **Woodhouse (2014)**, "The acoustics of the violin: a review", *Rep. Prog. Phys.* 77, 115901:
  string, body and radiation, and "playability" as the property that joins them [L].
- **Serafin** (thesis, Stanford 2004, "The sound of friction"): real-time friction models and
  playability.
  - Serafin, Smith & Woodhouse (1999, WASPAA): **leaving out the torsional waves did not degrade
    playability** in their model [L, abstract]. Arcus measured the opposite: torsion widened its
    Helmholtz region and made the Guettler wedge (ADR 0157). This is consistent with CCRMA's note
    that the strongly damped torsion helps Helmholtz motion establish. The difference is likely the
    friction law and the bow model [I].
  - Serafin et al. also found a temperature-dependent friction more playable [L].
- **Desvages & Bilbao** (DAFx-15; *Applied Sciences* 6(5):135, 2016; Desvages's thesis, Edinburgh
  2018) [L]:
  - a finite-difference string in **two polarisations**;
  - the bow, **the fingers and the fingerboard** as nonlinear contacts;
  - energy-balanced, and driven by gestures over time.

  It is the reference for what a one-polarisation waveguide misses: the bow's bounce, the
  finger's stop and release, the slap against the fingerboard (Bartók), and real glissandi under a
  finger. It is far costlier than a waveguide [I].
- **Bow gesture as data:**
  - **Askenfelt (1989, *JASA* 86)**: players' bow force, speed and bow–bridge distance in
    sustains, crescendi, sforzandi and saltellato [L].
  - **Demoucron** (thesis, KTH/IRCAM 2008, "On the control of virtual violins"): bowing patterns
    as **analytical functions** fitted to measurements; for spiccato and sautillé, a half-sine
    bow speed per stroke with a parametric bounce [L].
  - **Maestre, Blaauw, Bonada, Guaus & Pérez (2010, *IEEE TASLP* 18(4))**: bow speed, force and
    β as **sequences of cubic Bézier segments**, chosen by a Gaussian-mixture model by stroke
    type, dynamic and context. Used to drive a waveguide model and a spectral one [L].

  This is the published recipe for the player layer's contours (`strings-verified.md` §3.6).
- **Recent (2024–2026):**
  - Lampis, Mayer & Chatziioannou (*Acta Acustica* 2024): robot-bowed transients; a triangular
    playable region in the Guettler diagram [L, abstract].
  - Beigi & Conneely (arXiv 2609.14990, September 2026, **a preprint, not peer-reviewed**): a
    finite-difference string with implicit Stribeck friction, and a revised **minimum-bow-force
    law F_min = C·Z·v_b/β, C = 1.112** held out on 64 points. It replaces Schelleng's Z² form
    [L]. This bears on Arcus's open item: "the lower limit moves with the bow's speed".

### 4.3 Performance modelling

- **Maestre et al. 2010** (above), with the gesture data of CNSM (CC BY 4.0; one soloist,
  `strings-verified.md` §3.6). It is the only open bowing data found.
- **MIDI-DDSP** (§1.4): note-level expression predicted from the score, with attributes that
  Cantus's study already measures (brightness, attack noise, vibrato) [L].
- **NotePerformer's look-ahead**, and **Lindemann**'s expired look-ahead and transition patents
  (US 7,718,885; US 6,316,710; `strings-verified.md` §1): the method of reading ahead to shape a
  transition is free to use [L].
- **Yamaha US 6,946,595** (expired): a per-channel deviation of note-on timing and vibrato onset
  for an ensemble feel [L, NV, `strings-verified.md` §1 row 15].

---

## 5. Arcus today, and what is in progress

From ADR 0157 with its amendments up to pass 6, ADR 0158, `arcus-match.md`, BACKLOG §3e and
`crates/cantus-core/src/inst/arcus.rs`.

### 5.1 What it has

- **Instruments and model.** Cello and bass (model 2), violin and viola (4× and 2× oversampled).
  Measured or fitted bodies: the cello's to 10 kHz; the bass's own; the violin's fitted to CNSM;
  the viola's scaled. A 12-band match correction fitted on two thirds of ~1 060 notes.
- **The string and the bow.** An MSW junction in physical units, the twist at the contact, and a
  thermal option (off).
- **The pitch.** Within 1 cent: a measured table plus a closed-loop "ear" (168 of 168 points).
- **The dynamic.** Mapped the players' way (pp → ff: force ×4.5, β ×0.5, speed ×1.8), with a
  sustain's level swell of 0.25 → 0.05 s by velocity.
- **The attack.** Inside Guettler's region (0 of 80 attacks over the limits).
- **The bow noise** raised to the measured bands.
- **Joins and strokes:**
  - a **legato slur**: the finger moves over 30 ms with a 3 dB dip, the bow going on;
  - **re-bow**, the force easing into the turn;
  - détaché, staccato (the bow stopped on the string), **spiccato** (a 40 ms bounce, the string
    ringing free between strokes);
  - **tremolo** (its own pitch table);
  - **pizzicato** with a snap;
  - portamento on request (`porta` ms, default 0).
- **Vibrato model 2.** From 1 976 measured notes: rate and extent by dynamic and register,
  per-cycle and per-note variation, a delay longer for soft notes. Programmable (delay, rise,
  rate, extent, variation, `vib.dyn`), from CC21, CC2, aftertouch, CC1 or MPE pressure.
- **Inputs.** Dynamics from automatic, CC1, CC11, breath, aftertouch or velocity, and **never
  silent**. MPE pressure, slide (the contact point) and pitch.
- **The note's articulation** (schema 81): Sustain, Legato, Staccato, Spiccato, Pizzicato,
  Tremolo, and a bow Up or Down. The overlap rule is shared with Exemplar. The clip's look-ahead
  ties a legato note (`note_art::tie_legato`).
- **The parameters:** `bow.pos` (0.7–1.4× β), `bow.force`, `bow.speed`, `bow.attack`,
  `bow.release`, `bow.noise`, `bow.dir` (alternating, down or up), `bow.change` (5–80 ms),
  `trem.rate`, the pizzicato's position, softness and decay, `ring`, and the body's mix, size and
  ring.
- **The house rules.** Deterministic, offline equals real time, block-size independent, quiet at
  rest; ≤ 0.5 % of a core a voice.
- **The tools:**
  - `tools/soundlab` (ADR 0160): the held-out band method, and the blind ABX and MUSHRA kit;
  - the study's estimators and `bowed-refdb`: notes, phrases (`trans_ms`, `dip_db`, `ap_max`,
    re-bows), vibrato cycle by cycle, bowing, friction.

### 5.2 Open, said plainly by the passes

- **The attack.** `t_steady` is 0.08–0.13 s against 0.24–0.42. The noise burst is 3 dB outside the
  band for the soft cello and 6–18 dB for the bass. The pre-Helmholtz start is the cause.
- **Brightness at ff.** The loud cello's centroid is under its band: 0.28 harmonics, partly the
  keys the class holds.
- **The pizzicato.** Its thump and brightness; per-partial decays and stiffness.
- **The bodies.** The bass's own high end; the cello body's regressions at 2–4 and 7–10 kHz.
- **Pass 5's leftovers.** The short-note vibrato; the bass's E string in tremolo at forte.
- **Not built yet:**
  - harmonics, sordino, ponticello and tasto as their own sounds, col legno;
  - sections and the stage, and the hybrid voice (round 43);
  - the adaptive piano roll's open technique set (round 43).

### 5.3 In progress (another builder; not redone here)

- The bow's **position along the stick** (frog to tip).
- Its **direction**.
- **On-string against off-string** attacks.
- An automatic **player layer**: bow alternation; vibrato onset and depth by note length and
  dynamic; **portamento on shifts**; humanised timing.

Items 1, 2, 6, 7, 8 and 10 of §7 build on this layer. They must share its definition of a "shift"
and its bow state, not create rival ones.

---

## 6. The gap analysis

**Effort:**

- **S**: a pass inside a round (days);
- **M**: a builder's main item for a round;
- **L**: a round or more.

**Where it belongs:**

- **DSP**: the model (`cantus-dsp`);
- **Player**: the automatic player layer, at playback and live;
- **Roll**: the adaptive piano roll and playing profile of round 43 (BACKLOG, "One piano roll that
  adapts");
- **UI**: the editor;
- **Stage**: round 43's stage, microphones and room.

### 6.1 The left and right hands

| Feature | The best products' approach | Arcus today | The gap | Impact (why) | Effort | Where |
|---|---|---|---|---|---|---|
| **Legato: interval and direction** | Recorded transitions for every interval up and down; SWAM's own model from a 4th or 5th | One slur: the finger over 30 ms with a 3 dB dip | No dependence on interval, direction, or same against cross string. No finger hammer on the way up or lift on the way down. No sign of the shift on large intervals. | **High.** Joins are where listeners hear "synthetic" (§3.1); measured joins span 10–105 ms (p10–p90) | M | DSP + Player |
| **Legato: speed** | Slurred, agile and fast runs by playing speed (Berlin, VSL, Spitfire) | Fixed 30 ms; the `finger` parameter is global | No shortening in fast passages and no lighter bow in runs | **High** in fast music: a uniform join smears runs | S–M | Player |
| **Cross-string legato** | SWAM: the release of the old string stopped or left ringing by interval; split portamento | Not modelled. Each voice is one string chosen by key. | No two-string moment and no old string ringing | Medium-high: a fingerprint of real slurs across strings | M | DSP + Player |
| **String and position (fingering)** | SWAM's Alternate Fingering decides the string; position key-switches (Sample Modeling); string-labelled recordings (Iowa, TinySOL) | First position always (`string_for`) | No choice of string. No "sul G". No same-string shift. No avoidance of open strings in expressive lines. | **High.** Timbre per register changes with the string (a high note on a low string is darker and warmer), and so does the path of every portamento and shift | M | Player + DSP (per-string loops exist) + Roll (sul G mark) |
| **Open strings** | No vibrato on open strings (SWAM [M]); a fault where it exists (Sample Modeling [R]); a longer ring | Open pitches are played open and take vibrato like any note (no special case found in `arcus_voice*`) | Vibrato on open strings; the open string's longer ring and brighter tone are not chosen deliberately | Medium: listeners who play hear it at once; cheap to fix | S | Player + DSP |
| **Bow direction, position along the bow, on or off the string** | SWAM's bow start and lift; Spitfire's bowed legato; re-bow samples | Direction alternating or the note's | **In progress** (§5.3) | High | — | — |
| **Bow planning over a phrase** | Implicit in recordings (players plan the bow); NotePerformer's 1 s look-ahead | Alternation only (in progress) | Strong beats on down-bows; bow shared over long notes (slower speed, more force); a "retake" before a phrase | Medium-high: down-bow against up-bow accents and the swell of long notes are audible | S–M | Player (uses the clip's look-ahead) |
| **Attack and onset** | Recorded attacks at 3–5 dynamics with round robins; SWAM's Attack Control and Ramp Speed; Berlin's bow attack by velocity | Clean Helmholtz start plus a level swell; `t_steady` 0.08–0.13 s against 0.24–0.42 | Open: the bow's gesture before the motion, and stroke-specific contours | **High.** The onset is the strongest cue in a solo and in the ABX's single notes | M | DSP (gesture) |
| **Martelé, marcato, sforzando, fp** | Recorded (CSS marcato, VSL sfz and fp); SWAM's recipe (high velocity and expression, pressure before the attack) | Reachable through velocity and CC1; no named articulation | No preset contours for the bite then release; not in the note's art list | Medium | S | Player + Roll |
| **Dynamics and timbre** | 3–5 layers or a continuous model; CSS's pp → ff "progression" praised | Players' map; ff brightness under its band | Open: brightness at ff | Medium: heard in swells; uncertain fix | M–L | DSP |
| **Dynamics controls** | CC1 dynamics + CC11 trim (Spitfire); velocity for shorts | `dyn.src` picks one source; `gain` | No CC11 trim beside CC1, as libraries ship it. Users' templates expect CC1 = dynamic and CC11 = expression. | Low for realism, high for familiarity | S | Player + UI |
| **Vibrato's shape over the note** | Progressive vibrato with onset by velocity (Spitfire); scripted styles (Embertone); fade-in and rate randomness (SWAM) | Model 2: delay, rise and per-cycle variation; by dynamic. Onset and depth by length are in progress. | Vibrato stopping or narrowing through a shift or portamento and resuming after it; vibrato tapering at a note's end or in a diminuendo; a per-note senza or molto choice (a round 43 hook) | Medium | S | Player + Roll |
| **Bow-change noise** | Recorded re-bows; noise layers | The force eases into the turn; no click (pass 5) | The measured re-bow is a 7–11 dB dip with a 30–80 ms aperiodic burst. Arcus's burst is not checked against it per phrase. | Medium | S | DSP (measure first) |
| **Finger noises** | Contained in recordings ("Shorts + Noise"); Pianoteq's guitar "natural instrument noises" | None | No finger impact on the fingerboard (rising slur), no lift-pluck (falling slur), no slide noise in a shift | Medium in close, slow, solo playing; small in a mix | S–M | DSP (a contact exciter at the stop) |

### 6.2 Tone, variation, the room and the ensemble

| Feature | The best products' approach | Arcus today | The gap | Impact (why) | Effort | Where |
|---|---|---|---|---|---|---|
| **Variation inside a note** | Round robins; neighbouring zones; SWAM's Random Bow (pressure, speed, position) and Random Finger | Vibrato varies per cycle and per note; nothing else drifts | The bow's force, speed and β are constant through a sustain, and the intonation is perfect | **Medium-high.** A static sustain is a classic synthetic cue; models have no recorded "life" to fall back on | S | DSP + Player |
| **Intonation** | VSL Humanize tuning curves; SWAM's Random Finger and Temperament | ≤ 1 cent of the project's tuning, by design | No seeded scatter of a few cents; no onset pitch settling; no expressive intonation (a high leading tone) | Medium. Exact pitch is rarely noticed as such, but contributes to "too clean" [I] | S | Player (DSP stays exact underneath) |
| **Portamento's shape and control** | Velocity thresholds (Spitfire, CSS, Berlin, Embertone); SWAM's CC time and split ratio; CSSS's switch faulted | `porta` ms (default off); portamento on shifts in progress | Shape (slow start, fast arrival), a level dip, vibrato off while sliding, Flesch's B and L shifts, velocity or CC control | Medium: taste-defining, and unwanted slides are the commonest complaint | S | Player + DSP |
| **Releases and the ring** | Release samples with an amount; String Resonance (SWAM); room | `bow.release`, `ring`; on- or off-string in progress | The release's lift noise; the ring's colour from the open strings (next row) | Low–medium | S | DSP |
| **Sympathetic open strings** | SWAM's "Open Strings" and "String Resonance" [M]; implicit in recordings | None | No open strings ringing in sympathy, during the note or after it | **Medium-high.** The halo after a release, and the AM at the vibrato rate (Fletcher & Sanders) | S–M | DSP |
| **Ponticello, tasto, flautando** | Recorded patches; SWAM's bow position and pressure; Fischer's Color | `bow.pos` 0.7–1.4× and MPE slide; force and speed multipliers | The range does not reach real ponticello (β very small, glassy upper partials) or tasto and flautando (β large, light and fast). No named technique. | Medium: orchestral writing asks for them; a natural fit | M | DSP + Roll |
| **Harmonics** | Recorded natural and artificial; SWAM 2, 3 or a chosen one | None | A light finger at a node (natural); a stopped string plus a touch a fourth above (artificial) | Medium | M | DSP + Roll |
| **Sordino** | Recorded, or emulated (CSS, Berlin, SWAM: reviewers say "too subtle") | None | The mute's mass on the bridge: a body change, measured | Medium | S–M | DSP |
| **Col legno, Bartók** | Recorded (Spitfire: battuto, tratto, pencil); SWAM's col legno faulted as "weedy" | None | New exciters: a strike with the wood; a pluck hard enough to snap on the fingerboard | Medium: reviewers single them out | M | DSP |
| **Pizzicato quality** | Recorded with round robins; SWAM's Pizzicato Tone and Fingerboard Interference | A pluck with a snap; thump and brightness open | The thump, the fingerboard contact, decays per partial, the left hand's damping, Bartók | Medium | M | DSP |
| **Trills and tremolo** | Recorded trills; trill detection (Spitfire, five alternations in 200 ms); measured tremolo synced to the tempo | Tremolo by rate; trills only as fast slurs | A fingered trill (hammer and lift, a measured rate, no level dip), tremolo synced to the tempo, ricochet | Low–medium | S | Player + Roll |
| **Double stops and chords** | SWAM: two notes on adjacent strings, auto-detected with latency | Polyphony of up to 8 independent voices | No constraint to adjacent strings; three- and four-note chords not rolled | Medium where chords are written | S–M | Player |
| **Microphones, room, radiation** | 3–13 microphone positions; SWAM's Ambiente and Dyn pan; Instrument X's modelled microphones | Dry mono, one radiation path | No directional tone colour, no early reflections, no player movement | **High in context and in the ABX** (the references are in rooms) | M | DSP + Stage |
| **Sections and divisi** | VSL Dimension's separate players and humanise; SWAM Sections' precision and anti-phasing; PolySWAM and Logic voice allocation | None (round 43) | All of it | **High** for orchestral use | L | DSP + Player + Stage |
| **Expression interfaces** | SWAM's gesture modes (bipolar direction, bow displacement), breath mode, MPE | CC1, CC11, breath, aftertouch, velocity, MPE; never silent | A bipolar or bow-displacement mode for ribbons and XY pads | Low for realism, medium for players with controllers | S | Player |
| **Latency** | 100–333 ms (CSS) or tightness and offset (Spitfire); script lag | **0 ms** live; look-ahead only for tying legato | Unused advantage: anticipating joins and planning the bow on playback | Medium-high (it makes the other items better) | S | Player |
| **Articulation vocabulary and interchange** | Expression maps, articulation sets, UACC; MIDI 2.0's orchestral articulation profile | Six marks; MusicXML partly | The open technique set and maps | High for workflow, indirect for realism | L | Roll (round 43) |
| **Hybrid voice** | Sample Modeling (sampled base, modelled behaviour); Embertone (scripted vibrato on recorded notes) | Planned in BACKLOG §3e | — | Only if the attack cannot be closed physically | M–L | DSP + content (patent check) |

---

## 7. The ranked plan: the most realism for the effort

### 7.1 How the items are ranked and judged

**Ranking.** By expected realism gained per effort, under four facts:

- the owner's gate is a blind ABX and MUSHRA against recordings (ADR 0160);
- music, not single notes, found the faults of pass 5;
- the attack and the room are where the ABX's single notes are decided;
- the joins are where phrases are decided.

**Every item is measured the same way:**

1. Choices are made on the **training two thirds**; the held-out third judges each change once.
   For notes the split is soundlab's rule (`(note + dyn) % 3 == 0` for the cello subject). For
   phrases it is a seeded SHA-256 rule by phrase id, which soundlab already supports.
2. A feature is "in" when the candidate's median lies **inside the band of real instruments**,
   each physical instrument weighing one (ADR 0160, `bowed-refdb/README.md`).
3. The house invariants hold:
   - null at defaults (each addition has an off state that leaves today's bits);
   - block-size independence;
   - offline = real time, look-ahead included;
   - pool = serial;
   - `rt_memory`;
   - an end-to-end test through `CantusApp::assemble`.
4. The owner listens last, in the blind kit (`abx.html`, ABX then MUSHRA), against the CC0
   references.

**New estimators.** soundlab today measures **notes**. Items 1, 7, 8 and 10 need **phrase and
transition** estimators. The study's `phrases.py` has the start (`trans_ms`, `dip_db`, `ap_max`,
re-bow dips). It should be ported into soundlab as a `phrase` kind with these features:

| Feature | What it measures |
|---|---|
| `transition.trans_ms`, `.dip_db`, `.ap_max` | As in the study |
| `transition.pitch_shape` | The time to 10 / 50 / 90 % of the interval, and the overshoot in cents |
| `transition.hf_burst_db` | 1–6 kHz energy in ±20 ms of the join, re the steady level (the finger's transient) |
| `transition.class` | Interval bin, direction, and same or cross string where knowable |
| `rebow.dip_db`, `rebow.burst_ms` | The re-bow's level dip and its burst |
| `fluct.level_db`, `fluct.centroid_oct` | Slow modulation, 0.3–4 Hz, in the steady window, with the vibrato rate notched out (item 6) |
| `onset_pitch.cents[t]` | The pitch against the steady pitch at 20, 50, 100 and 200 ms (item 6) |
| `sympathetic.ring_db` | After a release, the energy at open-string partials against neighbouring bins (item 5) |
| `stereo.coherence[band]` and its modulation at the vibrato rate | Item 4; extends `f_stereo` |

### 7.2 The fifteen items

---

**1. Note-to-note transitions by interval, direction, string and speed.** *(DSP + Player; M; a
native fit for modelling.)*

- **Build:**
  - A transition model whose parameters are measured, not guessed. Its inputs are the interval,
    its direction, same or cross string (from item 2), the shift (from item 2), the inter-onset
    interval and the dynamic. Its outputs are the finger's glide time and curve, the level dip,
    and a finger contact:
    - rising: the stopping finger strikes, a short force pulse at the stop point;
    - falling: the lifted finger plucks the string slightly, a small left-hand pizzicato.
  - **Cross-string slurs.** The bow briefly touches both strings. The old string rings on under
    its own decay for 10–40 ms (to be measured), or is muted on small intervals. This follows
    SWAM's "Mono CrossString Muting" logic: stop the old string on seconds; let it ring on
    consonant intervals.
  - **Shifts on one string.** The loop's length glides (the portamento code exists), with a dip in
    the middle.
  - **Fast runs.** The joins shorten (the reference p10 is ~10 ms) and the bow lightens.
- **Measure:**
  - `phrase` transitions from `bowed-refdb/phrases.jsonl` and the sources behind it, binned by
    interval and direction: `trans_ms`, `dip_db`, `ap_max`, `pitch_shape`, `hf_burst_db`.
  - Held out by phrase. Today's single 30 ms / 3 dB join is the "before".
- **What the owner should hear.**
  - A slow eight-bar cello cantabile with steps, thirds, a sixth and an octave: rising slurs with
    a faint "tick" of the finger; falling slurs softer; a leap over strings that leaves the old
    string sounding for an instant.
  - A G-major scale in sixteenths at 120 bpm that is clean, not a chain of swoops.

---

**2. A fingering engine: the string and the position of every note.** *(Player + DSP + a Roll
mark; M; a native fit.)*

- **Build:**
  - A phrase-level solver (dynamic programming over string and position). It weighs the shifts,
    the string crossings, the hand's span, open strings in expressive notes, and the height of the
    position for timbre. Guitar tablature solvers are the template [I].
  - It honours user marks: *sul G / sul D / sul A / sul C*, "open", "no open strings". These marks
    belong in round 43's playing profile.
  - The voice receives the string and the stopped length. The string tables exist
    (`arcus_tables`). Today `string_for` always picks first position.
  - Open strings get no vibrato (SWAM's rule), no finger damping (a longer ring) and their
    brighter attack.
  - **This is the source of "shift" for the in-progress portamento-on-shifts.** It must be one
    definition, handed to that layer.
- **Measure:**
  - **Per-string timbre at the same pitch.** The Iowa MIS records chromatic runs *on each string*
    (`arcus-match.md`), and TinySOL plays "every note on more than one string" (its file names
    carry the string; check the exact token).
  - Fit and hold out by (note, string, dynamic): harmonics, centroid, noise and `decay`.
  - Today's first-position renders are the "before" for notes the references play on a lower
    string.
- **What the owner should hear.** The same melody *sul A* against *sul D* on the cello, and *sul
  G* on the violin (darker and warmer, the shift glides on one string). An open D against a
  stopped D (no vibrato, a longer ring).

---

**3. The attack shaped like a bow stroke.** *(DSP gesture; M; a native fit — the open item of
passes 4–6.)*

- **Build:**
  - Stroke-specific contours of bow force, speed and β over the first ~300 ms. Their sources:
    - Askenfelt 1989's measured sustains, crescendi and sforzandi;
    - Demoucron 2008's analytical stroke functions;
    - Maestre et al. 2010's Bézier segments.
  - **"Force before motion"** for martelé and accented starts. A soft start accelerates gradually
    from rest with its force placed first.
  - The pre-Helmholtz transient stays inside Guettler's 50 / 90 ms for the "good" attack classes.
    The spectrum is allowed to settle over 0.2–0.4 s, as the players' does.
  - The velocity and the note's articulation choose the contour. **It is shared with the
    in-progress on- and off-string attacks**: the same contour table, two starting states.
- **Measure:**
  - soundlab's `t_steady`, `attack.noise_burst_db_over_steady`, `build_order_rho` and
    `t_rise_3db_s`, per dynamic class, held out.
  - The Guettler gate must still pass (0 of 80 over the limits).
  - The targets are pass 6's bands: `t_steady` 0.24–0.42 s; the burst into 12.5–22.1 dB (soft
    cello).
- **What the owner should hear.** In the blind kit, single notes at p, mf and f against the Bigcat
  cello and the Smolken bass: the note "arrives" as a bow does, not "switches on". A détaché
  passage at p where each note starts with a soft grain.

---

**4. Directional radiation and an early-reflection stage.** *(DSP + the first step of round 43's
Stage; M; a native fit.)*

- **Build:**
  - The modal body radiates to **two or more directions**. Each mode above ~700–850 Hz gets its
    own sign and strength per direction. The upper body already gives each mode a sign and a
    strength, so this is a second set per direction.
  - Vibrato then sweeps partials through different lobes in each channel: Weinreich's directional
    tone colour, a decorrelated shimmer.
  - A seeded, slow sway of the player (SWAM's "Dyn" pan does this in a cruder way).
  - A small early-reflection set (a floor and a near wall), before round 43's full stage. Mono
    compatibility is a gate.
- **Measure:**
  - Inter-channel coherence per band, and its modulation at the vibrato rate, against the stereo
    references (`f_stereo` extended).
  - The vibrato AM per channel.
  - A mono fold-down within the band on every existing feature.
- **What the owner should hear.** A solo violin and a solo cello with vibrato on headphones: the
  dry mono render against the radiating one. The tone "lives" and fills the space without reverb.

---

**5. Sympathetic open strings, and the ring after the bow lifts.** *(DSP; S–M; a native fit.)*

- **Build:**
  - Per instrument, not per voice, the open strings as lightly damped waveguides driven by the
    bridge force. Their output is added at the body. Their phases are deterministic and their sum
    ordered, as the pool requires.
  - They ring when a played note or its partials coincide with an open string's partials: G, D,
    A and their harmonics on the cello.
  - They are heard most after a release and as AM at the vibrato rate (Fletcher & Sanders).
  - A "damp" switch for players who mute them.
- **Measure:**
  - `sympathetic.ring_db` after releases in the **anechoic** Iowa notes, where no room masks
    them. The notes that coincide with open strings are compared with the notes that do not.
  - The AM of coinciding partials during the note.
- **What the owner should hear.** A slow G-major scale on the cello. The G, D and A notes leave a
  soft halo when the bow lifts; the others do not. On and off, in the blind kit.

---

**6. Variation inside a note: the bow and the intonation.** *(DSP + Player; S; a native fit.)*

- **Build:**
  - Seeded, slow (0.3–3 Hz) processes on bow force, speed and β. Their sizes are to be measured;
    a few per cent is the expectation [I]. They are partly correlated with the vibrato: the
    study's "bow motion tied to the vibrato", not done in pass 2.
  - A per-note intonation scatter of a few cents, and an onset pitch that settles towards the
    target over 50–150 ms. The pitch "ear" then aims at the target plus the scatter, so the DSP
    stays exact underneath.
  - Everything is deterministic by seed and off by default, so today's bits are kept.
- **Measure:**
  - `fluct.level_db`, `fluct.centroid_oct` and `onset_pitch.cents[t]` in the references' steady
    windows and onsets. Fit the sizes; hold them out.
  - Arcus's present values are the "before": about 0, beyond the vibrato.
- **What the owner should hear.** A held 6-second cello note and a slow melody. The note no longer
  sounds "machine-steady", yet no wobble can be pointed at.

---

**7. Look-ahead planning on playback.** *(Player; S–M; Arcus's own advantage over samples.)*

- **Build.** On playback, the player layer reads the clip's next notes (≥ 1 s, as
  `note_art::tie_legato` already does when the snapshot compiles). With them it:
  - **starts slurs and shifts early**, so that the new pitch's midpoint lands on the written onset;
  - plans the **bow**: strong beats and phrase starts on a down-bow, a retake when needed, long
    notes given a slower bow with more force (with the in-progress bow position);
  - **stops the vibrato before a shift** and lets it bloom after.

  Live input keeps 0 ms with the same rules, minus the anticipation. Offline equals real time,
  because the look-ahead is part of the snapshot.
- **Measure:**
  - On renders: the transition's pitch midpoint against the grid (target ±10 ms); bow direction
    on strong beats; no audible change to a note with nothing ahead (the null).
  - Phrase features against the references, as in item 1.
- **What the owner should hear.** A legato phrase at 80 bpm that sits **on** the beat with no
  manual track offset. CSS users shift such phrases by hand by 100–333 ms; Spitfire's users trade
  "tightness" for realism.

---

**8. Speed-aware choices: the joins, the bow region, sautillé.** *(Player; S–M; a fit.)*

- **Build.** From the inter-onset interval and the tempo:
  - the join's time scales down, with a floor at the measured p10;
  - fast détaché moves to the middle and upper half of the bow;
  - separated repeated notes faster than a threshold become sautillé or spiccato off the string.
    The threshold is the user's: Spitfire's "by speed of playing", VSL's auto-speed and Berlin's
    adaptive legato all expose one;
  - notes shorter than about a vibrato cycle get none, if the in-progress layer does not already
    cover it.
- **Measure:**
  - `trans_ms` against the inter-onset interval in the references' fast passages: a regression
    held out by phrase.
  - Spiccato's bounce against `bowed-refdb/strokes.json`.
- **What the owner should hear.** Arpeggios and a fast passage in sixteenths at 140 bpm, written
  plainly without marks, that come out as a player would bow them.

---

**9. The techniques physics gives: ponticello, tasto, flautando, harmonics, sordino.** *(DSP + the
Roll's technique set; M; a native fit.)*

- **Build:**
  - **Ponticello**: a very small β (~0.02–0.04) with the force re-centred in its Schelleng
    region. The bow–bridge segment becomes a few samples long, the problem the upper strings
    solved by oversampling, so it may need 2× even on the cello [I].
  - **Tasto and flautando**: a large β, light force, fast speed.
  - **Natural harmonics**: a light finger as a resistive point at 1/n of the string, a third delay
    segment. **Artificial harmonics**: a stopped string plus a touch a fourth above.
  - **Sordino**: the mute's mass at the bridge, as a measured change to the body's upper modes.
  - Each one is a technique id in round 43's profile. Combinations follow (tremolo with ponticello,
    harmonics with glissando).
- **Measure.** References for these techniques must be found first:
  - the Philharmonia samples, already catalogued as lossy and measurement-only, are believed to
    include ponticello, harmonics, sordino and col legno (to verify);
  - or Cantus's own recordings.

  For each: harmonics, centroid, noise and `t_steady`, with the band by technique.
- **What the owner should hear.** A tremolo sul ponticello (glassy), a flautando line (airy), a
  run of natural harmonics, and a muted cello melody that is clearly muted. Reviewers called
  SWAM's sordino "too subtle".

---

**10. The portamento's shape and kinds.** *(Player + DSP; S; a fit.)*

- **Build:**
  - a glide curve (slow start, faster arrival: measure it);
  - a level dip;
  - the vibrato off during the slide, then its onset again;
  - **Flesch's B and L shifts**: sliding on the old finger then the new one dropping, against the
    new finger sliding into the note (a performance-practice distinction [I]);
  - control by velocity threshold (the libraries' convention) **and** by a continuous CC for its
    time (SWAM's), never only a switch (CSSS's CC5 was faulted);
  - split across strings when item 2 says the notes are on two strings.
- **Measure.** The references' slow transitions (`trans_ms` > ~100 ms): `pitch_shape`, `dip_db`,
  and vibrato extent through the slide.
- **What the owner should hear.** A romantic cello melody at two portamento amounts, tasteful and
  never accidental.

---

**11. Brightness with dynamic** *(DSP; M–L; uncertain.)*

- **Build.** Pass 6 showed the string is already at the sawtooth's limit and the gap is partly the
  keys the class holds. The candidates:
  - a β closer to the bridge at ff (Schoonderwaldt: the centroid rises with force mostly near the
    bridge);
  - the finite-width bow and the hair's compliance (`bowed-realism.md` §3.7);
  - the body's 2–4 kHz levels (pass 6's regression).
- **Measure.** `centroid_harm.loud` and `harmonics` octaves 0–3, loud, held out; the body curve.
- **What the owner should hear.** A pp → ff swell that opens up in tone, not only in level.

---

**12. Col legno, Bartók, and the pizzicato's thump.** *(DSP: new exciters; M; still modelling.)*

- **Build:**
  - **col legno battuto**: a short, hard contact by the stick (a mass and a stiff spring, ~1 ms,
    with some hair);
  - **col legno tratto**: friction with a low, noisy μ;
  - **Bartók**: a large-amplitude pluck with a one-sided collision with the fingerboard (Desvages
    & Bilbao's contact, simplified to a waveguide clip at the fingerboard point) [I];
  - the pluck path's own excitation shaping for the thump (pass 6's note).
- **Measure:**
  - The pizzicato features (`band_decay`, centroid, `thump_excess_db`) against the references.
  - For col legno, the Philharmonia samples (to verify) or Cantus's own recordings.
- **What the owner should hear.** A col legno ostinato that clicks with wood, and pizzicati that
  thump like a cello's rather than ping.

---

**13. Double stops and chords.** *(Player; S–M; a fit.)*

- **Build:**
  - Chord notes assigned to **adjacent strings** by item 2. Where that is impossible the voices
    stay free and the editor says so.
  - Three- and four-note chords **rolled**: the lower pair, then the upper pair. The spread is to
    be measured; a few tens of ms is the expectation [I].
  - A sustained double stop shares one bow: one speed, the force split.
- **Measure.** The owner's ear, since no open references with labelled chords were found. Timing
  can be checked against recordings of solo Bach if suitable CC material is found.
- **What the owner should hear.** A chorale of double stops and a final four-note chord that
  sounds played, not sampled.

---

**14. Fingered trills, measured tremolo, ricochet.** *(Player + Roll; S; a fit.)*

- **Build:**
  - a trill as a technique: hammer and lift joins (item 1) at a measured rate, without a level dip;
  - tremolo locked to the tempo (SWAM's Sync, the libraries' measured tremolo);
  - ricochet as a thrown bow with 3–6 bounces whose period shortens (Guettler & Askenfelt 1997–98
    on spiccato and ricochet kinematics).
- **Measure.** The trill rate and the evenness of the level in any trill recordings found; the
  ricochet's bounce periods against the literature.
- **What the owner should hear.** A cadential trill, and a measured tremolo in sixteenths at the
  song's tempo.

---

**15. Sections.** *(DSP + Player + Stage; L; round 43, already planned.)*

- **Build:**
  - Seeded players, each with:
    - **its own body** (modes perturbed within a measured spread);
    - its own vibrato rate and phase;
    - its own intonation offset and timing;
    - **its own bow changes**, never synchronised;
    - its own seat, with item 4's radiation, which averages out across the section as Weinreich
      predicts.
  - Automatic divisi by register and voice (Logic's Auto Voice Split, PolySWAM's allocation) and
    explicit divisi.
  - The budget: 16 players at ≤ 0.5 % of a core each ≈ 8 %, inside the pool.
- **Measure.** VSCO 2 CE's sections against its solo violin, kept apart in `bowed-refdb`: onset
  spread, pitch spread (beating), decorrelation; the band by section size.
- **What the owner should hear.** Eight violins playing a held chord and a melody, with no phasing
  and no "copied" sound. SWAM Sections' anti-phasing and VSL Dimension's separate players exist to
  defeat that sound.

### 7.3 What fits modelling, and what would need samples

**Native to modelling** (continuous control, no layer seams, no extra memory):

- items 1, 2, 4, 5, 6, 7, 9 and 10;
- most of 8 and 13;
- 15 (with its CPU).

These are things samples do **only by recording every case**: interval × direction × string ×
speed × dynamic for transitions, every string for every note, every technique combination.
Several of them samples cannot do at all: directional radiation that follows the vibrato,
sympathetic strings that answer whatever is played, joins with 0 ms latency.

**Modelling, with new physics** (still no samples): 3 (the stroke's contours), 11 (if the cause
is in the bow or the body) and 12 (contacts and collisions).

**Where samples or a hybrid would help:**

- **The room.** Measured room impulse responses (open CC0 or CC BY sets) are recordings, but of
  rooms, not instruments. Round 43's stage.
- **The attack, as a fallback.** If item 3 cannot bring `t_steady` and the burst into band, a
  hybrid could lay a recorded *noise residual* of real attacks (not a whole note) under the
  modelled onset, **added after the body and never fed into the loop**. Yamaha's US 8,729,376 is
  about feeding a de-reverberated recording into the loop (`strings-verified.md` §1 row 19). This
  still needs a lawyer's look.
- **Extended noises with no model.** Bowing behind the bridge, on the tailpiece, on the body;
  "seagull" glissandi of harmonics; aleatoric effects. They are rare. Exemplar's recorded strings
  can carry them.

---

## 8. Cautions for the builders

- **The patents** (from `strings-verified.md` §1; for a lawyer before release):
  - **Fable Sounds US 7,915,514 / 8,404,958**: a mod wheel stepping through dynamic layers at
    thresholds with a velocity lockout. Arcus has no layers. Exemplar uses a continuous crossfade.
  - **Yamaha US 7,557,288**: a stored legato joint built after the next note-on. Arcus's joins are
    computed; item 1 stores *parameters* of joins, not recorded joints [I: still to be checked].
  - **Yamaha US 8,729,376**: a recording fed into the loop. It matters only for the hybrid.
  - Free to use: Lindemann's transition, look-ahead and portamento patents, Jaffe and Smith's
    two-tap legato, and Yamaha's ensemble deviation (expired).
- **Determinism.** Every seeded process in items 5–8 and 15 draws from the instance's seed and the
  note's identity, never from wall time or thread order. The house tests (null, block sizes,
  offline = real time, pool = serial) apply to each.
- **One definition of a shift and of the bow's state**, shared with the in-progress player layer
  (§5.3).
- **Measure first, then tune by ear.** As passes 2–6 did, choices are made on the training two
  thirds and the held-out third is read once a change.
- **Do not over-humanise.** Products expose every variation as an amount. The owner's ear sets the
  default; the measured band sets the ceiling.

## 9. What could not be verified, and the open questions

**Not read in full:**

- the CSS and CSSS manuals (their legato delays come via SOS citing the manual, and a forum
  quoting support);
- Logic's Studio Strings parameter page (search excerpts only);
- the full texts of Weinreich 1997, Mellody & Wakefield 2000, Loosen 1993, Schoonderwaldt 2009 and
  Desvages & Bilbao 2016 (MDPI answered 403);
- Siedenburg et al. 2019 on transients (404 at the redirect);
- Acta Acustica's 2024 and 2026 pages (403);
- MusicTech (a TLS failure; not bypassed);
- ask.audio (403).

**Not found:**

- any Aaron Venture strings product;
- any Pianoteq bowed instrument;
- an independent review of Instrument X's strings;
- a measurement of pitch spread in a string section;
- open recordings of chords or trills with labels.

**To check before building:**

- TinySOL's string token in its file names;
- which techniques the Philharmonia samples hold;
- whether VSCO 2 CE or Iowa hold sordino, ponticello or harmonics.

**Open questions for the builders, each answerable by measurement:**

- how long the old string rings in a cross-string slur;
- how large the within-note bow drift is in real sustains;
- the onset pitch settling's size and time;
- a three- or four-note chord's roll time;
- whether ponticello needs oversampling on the lower instruments.

## 10. Sources

### Products: manuals and support

- Audio Modeling, *SWAM Solo Strings User Manual v3.0.0* (2021):
  https://static.audiomodeling.com/manuals/strings/SWAM%20Solo%20Strings%20-%20User%20Manual%20-%20v3.0.0.pdf
- Audio Modeling KB, "SWAM Solo Strings v3 main features improvements":
  https://kb.audiomodeling.com/support/solutions/articles/206000041686-swam-solo-strings-v3-main-features-improvements
- Audio Modeling, PolySWAM: https://audiomodeling.com/polyswam
- Spitfire Audio, *Solo Strings User Manual v3.3*:
  https://www.spitfireaudio.com/cdn/shop/files/Spitfire_Solo_Strings_Manual_V3.3.pdf
- Spitfire Audio, BBC Symphony Orchestra Professional:
  https://spitfireaudio.com/bbc-symphony-orchestra-professional
- Cinematic Studio Series, Cinematic Studio Strings: https://cinematicstudioseries.com/strings/
- VSL, Synchron Strings Pro: https://www.vsl.co.at/instruments/synchron/strings-pro
- VSL, Synchron Player manual, Edit tab: https://www.vsl.co.at/manuals/synchron-player/edit-tab
- Apple, Logic Pro Studio Strings (the guides as returned by search):
  https://help.apple.com/logicpro/mac/10.4/en.lproj/lgcpc7313036.html ;
  https://support.apple.com/guide/logicpro-ipad/lpip40b910a5/ipados
- Orchestral Tools, Berlin Strings collection: https://www.orchestraltools.com/store/collections/15
- 8Dio, Studio Series Deep Solo Violin: https://8dio.com/products/solo-studio-strings-violin
- Modartt, Pianoteq instruments: https://modartt.com/pianoteq_instruments ; forum "Stringteq":
  https://forum.modartt.com/viewtopic.php?pid=937508 ; KVR news on Pianoteq 8:
  https://www.kvraudio.com/news/modartt-releases-pianoteq-8---classical-guitar-revoiced-instruments-refined-gui-and-more-56416
- KVR, Aaron Venture developer page: https://www.kvraudio.com/developer/aaron-venture

### Reviews and news

- Sound On Sound:
  - SWAM String Sections: https://www.soundonsound.com/reviews/audio-modeling-swam-string-sections
  - Sample Modeling Solo & Ensemble Strings: https://www.soundonsound.com/node/4921022
  - Cinematic Studio Strings: https://www.soundonsound.com/reviews/cinematic-studio-strings
  - Cinematic Studio Solo Strings: https://www.soundonsound.com/reviews/cinematic-studio-solo-strings
  - Orchestral Tools Berlin Strings: https://www.soundonsound.com/reviews/orchestral-tools-berlin-strings
  - VSL Dimension Strings: https://www.soundonsound.com/node/4903812
  - Embertone Friedlander Violin and Fischer Viola: https://www.soundonsound.com/reviews/embertone-friedlander-violin-fischer-viola
  - Embertone Joshua Bell Violin: https://www.soundonsound.com/reviews/embertone-joshua-bell-violin
  - 8Dio Adagio Violins: https://www.soundonsound.com/reviews/8dio-productions-adagio-violins-vol-1
  - Dreamtonics Instrument X (news, 2026-08): https://www.soundonsound.com/news/dreamtonics-unveil-instrument-x-orchestral-alternative
- Synthtopia, Instrument X: https://www.synthtopia.com/?p=154519
- Magnetic Mag, "Dreamtonics shows how Instrument X was built":
  https://magneticmag.com/2026/09/dreamtonics-shows-how-instrument-x-was-built/
- VI-Control, Instrument X thread:
  https://vi-control.net/community/threads/dreamtonics-instrument-x-what-do-we-think.174688/
- MOTU forum (CSS support quote on its delays): https://www.motunation.com/forum/viewtopic.php?p=581767
- MusicTech, Metropolis Ark 1: https://musictech.com/2016/03/orchestral-tools-metropolis-ark-1-review/
- Wikipedia, NotePerformer: https://en.wikipedia.org/wiki/NotePerformer
- The earlier product scan and its sources: `research/instruments-next/strings-verified.md` §4.

### Literature

- Guettler & Askenfelt (1997), "Acceptance limits for the duration of pre-Helmholtz transients in
  bowed string attacks", *JASA* 101: https://knutsacoustics.com/files/acceptance-limits-jas002903.pdf
- Mellody & Wakefield (2000), "The time–frequency characteristics of violin vibrato", *JASA* 107,
  598–611 (doi:10.1121/1.428326).
- Fletcher & Sanders (1967), *JASA* 41, 1534–1544 (doi:10.1121/1.1910516).
- Fritz, Cross, Moore & Woodhouse (2007), "Perceptual thresholds for detecting modifications applied
  to the acoustical properties of a violin", *JASA* 122(6), 3640–3650 (doi:10.1121/1.2799475):
  https://neuroscience.cam.ac.uk/publications/perceptual-thresholds-for-detecting-modifications-applied-to-the-acoustical-properties-of-a-violin
- Weinreich (1997), "Directional tone color", *JASA* 101(4):
  https://knutsacoustics.com/files/weinreich,-g.-directional-tone-color-jasa-1997.pdf ;
  ASA lay paper: https://acoustics.org/pressroom/httpdocs/133rd/2pea4.html
- Wolfe (UNSW), "Articulation and vibrato on the violin": https://newt.phys.unsw.edu.au/jw/violinarticulation.html
- Woodhouse (2014), "The acoustics of the violin: a review", *Rep. Prog. Phys.* 77, 115901:
  https://core.ac.uk/download/42337723.pdf
- Serafin, Smith & Woodhouse (1999), "An investigation of the impact of torsion waves and friction
  characteristics on the playability of virtual bowed strings", WASPAA:
  https://ccrma.stanford.edu/~serafin/SerafinEtAl.pdf ; CCRMA note:
  https://ccrma.stanford.edu/~jos/mus423h/Impact_Torsion_Waves_Friction.html
- Serafin (2004), *The sound of friction*, PhD thesis, Stanford (cited via
  https://era.ed.ac.uk/bitstreams/339a88bd-99b0-4a6d-b0f4-3dff86a836b2/download).
- Desvages & Bilbao (2016), "Two-polarisation physical model of bowed strings with nonlinear
  contact and friction forces …", *Applied Sciences* 6(5):135: https://www.mdpi.com/2076-3417/6/5/135 ;
  DAFx-15: https://dafx.de/paper-archive/details/FfGMiq36GYceRsKJSfi59g ; Desvages's thesis
  (Edinburgh 2018): https://era.ed.ac.uk/bitstreams/339a88bd-99b0-4a6d-b0f4-3dff86a836b2/download
- Demoucron (2008), *On the control of virtual violins*, PhD thesis, KTH (urn:nbn:se:kth:diva-9424):
  https://www.avhandlingar.se/avhandling/fc2579831a/
- Maestre, Blaauw, Bonada, Guaus & Pérez (2010), "Statistical modeling of bowing control applied
  to violin sound synthesis", *IEEE TASLP* 18(4), 855–871:
  https://repositori.upf.edu/items/ee64bfe1-7f9a-44dc-bb6d-8e06bc0a197f
- Askenfelt (1989), "Measurement of the bowing parameters in violin playing II", *JASA* 86,
  503–516 (doi:10.1121/1.398230).
- Guettler & Askenfelt (1997), "On the kinematics of spiccato bowing", TMH-QPSR 38(2–3), 47–52;
  (1998) "… spiccato and ricochet bowing", *CASJ* (cited in
  https://knutsacoustics.com/files/thesis-guettler-020607.pdf).
- Schoonderwaldt (2009), "The violinist's sound palette …", *Acta Acustica u. A.* 95(5), 901–914
  (via https://www.frontiersin.org/articles/10.3389/fpsyg.2021.769831).
- Loosen (1993), intonation of solo violin performance, *JASA* (via
  https://core.ac.uk/download/345078758.pdf).
- Lampis, Mayer & Chatziioannou (2024), robot-bowed transients, *Acta Acustica*:
  https://doaj.org/article/25af9e17093b4392aacf544757b3fd64
- Beigi & Conneely (2026, preprint), "Learned Bow Control on a Measured Bowed-String Model …",
  arXiv:2609.14990: https://arxiv.org/abs/2609.14990
- Wu et al. (2022), "MIDI-DDSP: Detailed control of musical performance via hierarchical
  modeling", ICLR: https://arxiv.org/abs/2112.09312 ; https://midi-ddsp.github.io
- Engel et al. (2020), "DDSP: Differentiable Digital Signal Processing", ICLR (arXiv:2001.04643;
  from memory, NV).
- Patents (Lindemann, Jaffe and Smith, Yamaha, Fable Sounds): `strings-verified.md` §1, with
  their Google Patents numbers.
- Cantus's own: ADR 0157, ADR 0158, ADR 0160; `docs/round42/arcus-match.md`;
  `research/instruments-next/bowed-realism.md`; `research/instruments-next/bowed-refdb/README.md`;
  `docs/BACKLOG-excellence.md` §3e and "One piano roll that adapts".

---

## 11. Solo libraries from the owner's video (2026-11-25)

*The owner sent a comparison video and asked (translated): "Can you study these to see what we
should implement from them to improve?" The scope was then narrowed to the libraries that hold
**solo** instruments, because Arcus is a solo instrument today. The section libraries are parked
for round 43 (§11.5). This section adds only what §§0–10 do not already hold. SWAM, Spitfire Solo
Strings, CSSS, VSL and Embertone are not repeated here.*

### 11.1 The video

- **Title:** "I Tested 18 String Libraries (FREE vs $1,800)".
- **Channel:** Leo Brennauer (https://www.youtube.com/@leobrennauer).
- **Published:** 2026-11-15. The video runs 31:26 [YouTube page and oEmbed].
- **The method, in the description's words:** "scoring the same two tracks with each of them". The
  video opens with a 7-minute "Comparison (no talking)", then gives one chapter to each library.
- **Disclosures** [description]: the video is not sponsored; some libraries were given to the
  author for the test; some links are affiliate links.
- **Not learned:**
  - The page lists an auto-generated English caption track, but YouTube's caption endpoint
    returned an empty body to a plain request. It was not bypassed, so there is **no transcript**.
  - **No audio was fetched.** What the video plays, which MIDI it uses, its mix settings and the
    author's verdicts are **unknown** and are not guessed here.

### 11.2 Which of the 18 have solo strings

One check per library. Only three of the 18 hold a solo violin, viola, cello or bass.

| Library | Solo strings? | Source |
|---|---|---|
| **Pēteris Vasks Strings** (Orchestral Tools, 2023) | **Yes.** Five first chairs: violin 1, violin 2, viola, cello and bass | [R SOS; V KVR] |
| **Spitfire Symphony Orchestra** (Spitfire, solo update 2025-02-18) | **Yes.** Solo violin, solo violin 2, solo viola and solo cello. Each has "All techniques" and "Performance" patches. | [M SSO manual v5 pp. 62–68; R rekkerd] |
| **Berlin Free Orchestra** (Orchestral Tools, 2025, free) | **Yes, basic.** A solo violin and a solo cello, with one dynamic layer (mf) | [M OT helpdesk] |
| LUX Strings and LUX Strings Elements (Sonuscore) | No: five sections, 18/16/14/12/10 | [M] |
| Synchron Prime I (VSL) | No: ensembles 14/10/8/8/6 and 6/5/4/4/3 | [V] |
| Duality Strings (VSL) | No. Its "Solo" is the name of a mic position. | [V] |
| Vista / Vista II, Pacific (Performance Samples) | No. The smallest group is 2/2/2/1. | [V] |
| Lineage Strings (ProjectSAM) | No: "no soloists" | [V] |
| Cinematic Studio Strings | No. CSSS is a separate product (§1.3). | [R] |
| Areia (Audio Imperia) | No: sections and ensembles only | [V] |
| Berlin Symphonic Strings (OT) | No. It has a "Leader" mic only; Berlin Solo Strings is separate. | [R SOS] |
| CineStrings Core (Cinesamples) | No: sections only | [M] |
| Hollywood Strings I (EastWest) | No: sections and divisi only | [R SOS] |
| Spitfire Chamber Strings | No: ensemble and section patches | [M; I] |
| Benjamin Wallfisch Strings (OT, 2023) | No: ensembles and divisi | [R SOS] |
| Abbey Road Orchestra: Strings (Spitfire) | No: five sections | [R KVR, MusicRadar] |

**The ranking for realistic solo playing:**

1. **Pēteris Vasks Strings.** These are the only solos among the 18 that an independent review
   praised.
   - The first violin has "a lovely timbre with a heart-melting emotive delivery".
   - The viola and cello give "great heartfelt performances".
   - "The playing is of a high standard" [R SOS, Dave Stewart, 2024].
   - The library as a whole is "incredibly convincing" [R MusicRadar].
2. **Spitfire Symphony Orchestra's soloists.** They come with Spitfire's performance-legato engine
   (§1.3), and Andrew Haveron, a concertmaster, plays the violin [M].
   - **No independent review of these solos was found.**
   - SOS's 2024 review of SSO, written before the solos were added, had faulted the library for
     lacking them: "Some users may miss the inclusion of solo strings".
   - It is not verified whether the separately sold "Symphonic Strings" holds them.
3. **Berlin Free Orchestra's solo violin and cello.** They are free, with a single mf layer, a
   sustain + legato patch, a sustain and a staccato (2 round robins) [M].
   - A forum user found the library "doesn't seem very responsive to dynamics" [R Steinberg
     forum].

### 11.3 The three, in depth

**Pēteris Vasks Strings: the first chairs** (Orchestral Tools, 2023-04, €549).

- **The recording.** Sinfonietta Rīga, conducted by Normunds Šnē, in St John's Church, Riga, "a
  large, reverberant space", with the composer guiding the sessions [V KVR; R SOS].
  - Six mic positions: an omni Decca tree (the default) and a cardioid tree, "thinner and less
    ambient"; close spots; ribbons; an AB pair; surround mics [R SOS].
  - The soloists' close spots are **panned as a quintet is seated**: violin 1 left, violin 2 and
    viola between, cello right, and the bass in the centre behind [R SOS].
- **Three legato styles, kept apart** [R SOS; V KVR]:
  - *Lyrical*: "fairly quiet, moderate vibrato", "mournful and reflective".
  - *Expressive*: loud, with "a strong vibrato", "forthright and romantic".
  - *Sul tasto*: "very quiet, minimal vibrato", "tentative and tender".
  - The sections cross their dynamics in one patch. The soloists' styles stay separate patches,
    because "crossfading them would have compromised realism", given their differences in
    vibrato [R SOS].
- **The library's sound** [V KVR; R]:
  - little vibrato overall;
  - the soft layers con sordino, with the mutes coming off as the dynamic rises;
  - vibrato that grows with the dynamic.
- **Gestures.** The first chairs play the same gestures as the sections [R SOS]:
  - short glissandi (a 5th, an octave or two octaves) ending in a 2–3 s sustain;
  - glissando trills;
  - "waving longs": a semitone trill that turns into random slithering as CC1 rises;
  - finger tremolos;
  - "upbeats" (two quick short notes);
  - glissandi up the whole length of a string;
  - trills of a minor 2nd, a major 2nd and a minor 3rd, "animated, colourful" in stabs and
    flourishes.
- **The faults.**
  - The legato's latency is "considerable", deliberately so, and parts must be moved earlier
    [R MusicRadar].
  - "Legato playing will require some practice" [R MusicRadar].
  - Round robins and portamento are not described [R SOS].

**Spitfire Symphony Orchestra: the solo strings** (Spitfire; a free update on 2025-02-18, when SSO
was offered at £399).

- **The players** [M]:
  - violin: Andrew Haveron (concertmaster of the Sydney Symphony);
  - viola: Bruce White;
  - cello: Caroline Dale;
  - and **a second, separate solo violin** ("Solo Violin 2"), with its own patches [M p.68].
- **The patches** [M; R rekkerd]:
  - *Performance* holds fingered, bowed and portamento legatos.
  - *All techniques* holds long (with and without vibrato), spiccato, pizzicato, and short and long
    harmonics.
- **The rest** [M pp.62–66]:
  - The solos were tracked at AIR Studios with Close, Tree and Ambient mics.
  - "All our musicians are recorded in situ, i.e. where they would be seated on a standard scoring
    session".
  - The controls are those of Spitfire Solo Strings (§1.3):
    - Dynamics, Vibrato, Release and Expression;
    - Tightness, which "does detract from realism";
    - neighbouring-zone round robins;
    - the Hall Trigger;
    - speed-of-playing triggers.

**Berlin Free Orchestra: the solo violin and cello** [M].

- The violin plays G2–D6 and the cello C1–C5.
- Each has a single mf layer and no dynamic crossfade.
- For Cantus it is the free floor of the video's "FREE vs $1,800" range [I].

### 11.4 What is new for Arcus solo, and the changes to the plan

**The lessons** (only those §§0–7 do not already hold):

1. **A playing character is a bundle, not a knob** [R SOS on Vasks; I].
   - Each of Vasks's lyrical, expressive and sul tasto styles ties together a loudness, a vibrato
     depth and a bow contact point (tasto).
   - The vendor would not crossfade them: a crossfade between two recordings gives a vibrato that
     belongs to neither.
   - Arcus can morph between them because it has no recordings. Its danger is the reverse:
     independent knobs (CC1, vibrato, bow position) set in combinations no player would use.
   - The lesson is a **character** control. It moves bow force, speed and β, the vibrato's depth,
     rate and onset, and the portamento tendency **together**, along trajectories measured from
     players.
2. **Gestures are a composer's vocabulary, and the soloist plays them too** [R SOS].
   - The gestures are short glissandi into a sustain, glissando trills, upbeats, finger tremolos,
     trills up to a minor 3rd, and the aleatoric "waving long" on CC1.
   - Samples fix each gesture's length and tempo; a model can play it at any pitch, length and
     tempo [I].
   - Lineage's grace notes (up and down) and flourishes [V ProjectSAM] belong to the same family.
3. **A solo is one particular player, and two solos must be two players** [M SSO; R SOS].
   - Spitfire recorded a second solo violinist rather than reuse the first.
   - Vasks's five first chairs are five people, their close spots seated as a quintet.
   - Arcus varies its notes, but every instance plays with the same body and the same habits, so
     two Arcus violins in a duet are one player twice [I].
4. **The soloist sits in the room where the section sits** [M SSO; R SOS Vasks].
   - SSO recorded its solos in situ in the hall.
   - Vasks layers the first chairs over their sections "for focus".
   - Every Spitfire and OT section library adds a **Leader** spot mic.
   - A realistic solo is therefore placed at the front desk of round 43's stage, with its own
     spot. It is not a dry, centred mono voice [I].
5. **Sul tasto is a legato style, not only a timbre** [R SOS].
   - Vasks's third legato is a whole expressive mode: very quiet, minimal vibrato, tender.
   - §7 item 9 treats tasto as a technique switch. It should also be reachable as a phrase colour,
     through lesson 1's character [I].
6. **Beating the free floor is a test that can be run now** [I].
   - Berlin Free Orchestra's solo violin and cello are free, and the owner can install them.
   - The comparison is by ear only. Their audio never enters `bowed-refdb/`, which holds CC0 and
     CC BY recordings (ADR 0042).

**Changes to the ranked plan (§0.2, §7):**

| Change | What | Why |
|---|---|---|
| **New item 16: playing characters** (Player; S; med-high) | Presets for lyrical, expressive, tasto, senza and folk playing. Each moves force, speed, β, the vibrato's depth, rate and onset, and the portamento tendency together. The presets are measured from labelled recordings and morph continuously. Coordinate with the player layer in progress. | Lessons 1 and 5. The parameters exist; moving them together is cheap. **Rank it just after item 6, before item 7**, because a character colours every note. |
| **New item 17: gestures as note properties** (Player + piano roll; S–M; low-med) | Grace notes (up and down), upbeats, short glissandi into a sustain, glissando trills, finger tremolo and the aleatoric waving long, each a note property in round 43's adaptive piano roll | Lesson 2. It extends items 10 and 14, so **rank it after item 14**. |
| **Item 6 widened: a seeded player identity** | A seed per instance for the player's habits: an intonation tendency, vibrato rate and width habits, the bow's drift, and a small perturbation of the body. Two solos are then two players. | Lesson 3. This is item 15's "seeded players" at a size of one, and it is cheap. **Move it forward from item 15 into item 6**, so quartets benefit before the sections exist. |
| **Item 4 widened: the solo's seat** | Item 4's early reflections and radiation take a seat: the leader's chair, or quartet and quintet seating (vn1 left, vn2, va, vc right, cb behind), with a spot and a tree | Lesson 4. No re-rank. |
| **The ABX gains a free floor** | The owner compares Arcus with Berlin Free Orchestra's solo violin and cello in the same phrase. Neither their samples nor their audio enter the repository. | Lesson 6. The test costs nothing. |

The order of items 1–3 is unchanged. None of the three solo offerings goes past §1's products on
transitions, fingering or attacks.

### 11.5 Parked for round 43's sections

The other fifteen are section libraries. Their study waits for round 43's sections work (BACKLOG
"Strings and winds, modelled and recorded"; §7 item 15):

- LUX Strings and LUX Strings Elements;
- Synchron Prime I and Duality Strings;
- Vista II, Pacific and Lineage;
- Cinematic Studio Strings and Areia;
- Berlin Symphonic Strings and CineStrings Core;
- Hollywood Strings I and Spitfire Chamber Strings;
- Benjamin Wallfisch Strings and Abbey Road Orchestra: Strings.

A first pass gathered a few facts for that work, kept here so they are not redone:

- **True divisi.** Benjamin Wallfisch Strings is the only one recorded as half-sections (front and
  back desks, or left and right) [R SOS].
- **Fake divisi.** Hollywood Strings' A/B are the left and right close mics of one take [R SOS].
  LUX's "Chordal Divisi" only shares out the notes [M].
- **Spill between sections.** LUX has 22 mics in the empty wind and brass seats; the review heard
  them as "glue" [R SOS].
- **Two section sizes at once.** Duality recorded a 52-player and a 27-player section at the same
  time, in two rooms [V; R SOS].
- **Mixed technique inside a section.** Vasks's sections take off their mutes half at a time as the
  dynamic rises [R SOS]. Spitfire's "CS Blend" mutes one player of each desk [M].
- **Bowing and timing.** Lineage records up- and down-bows and alternates them automatically. It
  has a "Precision Control" from focused to looser timing [V].
- **Sustains that never loop.** Benjamin Wallfisch Strings' 23–25 s sustains are its most praised
  feature [R SOS].

### 11.6 Sources for §11

- The video: https://www.youtube.com/watch?v=baoVzLXuXiY
  - its oEmbed: https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v=baoVzLXuXiY&format=json
- Sound On Sound:
  - Pēteris Vasks Strings: https://www.soundonsound.com/reviews/orchestral-tools-peteris-vasks-strings
  - Spitfire Symphony Orchestra (Aug 2024): https://www.soundonsound.com/node/4932258
  - Benjamin Wallfisch Strings: https://www.soundonsound.com/reviews/orchestral-tools-benjamin-wallfisch-strings-stringscapes
  - LUX Orchestral Strings: https://www.soundonsound.com/reviews/sonuscore-lux-orchestral-strings
  - Duality Strings: https://www.soundonsound.com/reviews/vienna-symphonic-library-synchron-duality-strings
  - Hollywood Strings: https://www.soundonsound.com/reviews/ewql-hollywood-strings
  - Berlin Symphonic Strings: https://www.soundonsound.com/reviews/orchestral-tools-berlin-symphonic-strings
- Pēteris Vasks Strings, elsewhere:
  - MusicRadar's review: https://www.musicradar.com/reviews/orchestral-tools-peteris-vasks-strings-review
  - KVR's details: https://www.kvraudio.com/product/peteris-vasks-strings-by-orchestral-tools/details
- Spitfire Symphony Orchestra:
  - manual v5, read as text and not kept: https://www.spitfireaudio.com/cdn/shop/files/Spitfire_Symphony_Orchestra_Manual_v5.pdf
  - the solo update: https://rekkerd.org/spitfire-audio-updates-spitfire-symphony-orchestra/
- Berlin Free Orchestra:
  - the Orchestral Tools helpdesk: https://orchestraltools.helpscoutdocs.com/article/514-berlin-free-orchestra
  - the Steinberg forum: https://forums.steinberg.net/t/orchestral-tools-berlin-free-orchestra/979636
- VSL:
  - Synchron Prime Orchestra: https://www.vsl.co.at/instruments/starter-editions/prime-orchestra
  - Duality Strings: https://vsl.co.at/instruments/synchron/duality-strings
- Sonuscore's manuals:
  - https://manuals.sonuscore.com/lux-orchestral-strings.html
  - https://manuals.sonuscore.com/lux-orchestral-strings-elements.html
- ProjectSAM, Lineage Strings: https://projectsam.com/libraries/lineage-strings
- Performance Samples:
  - https://www.performancesamples.com/vista/
  - https://www.performancesamples.com/pacific-ensemblestrings/
- Audio Imperia, Areia: https://www.audioimperia.com/product/areia
- CineStrings Core's manual: https://ms.bestservice.com/download_files.php/5528/en
- Spitfire Chamber Strings Pro's manual: https://www.spitfireaudio.com/cdn/shop/files/Chamber_Strings_Pro.pdf
- Abbey Road Orchestra:
  - MusicRadar's review: https://www.musicradar.com/music-tech/plugins/spitfire-audio-abbey-road-orchestra-symphonic-strings-professional-review
  - KVR: https://www.kvraudio.com/product/abbey-road-orchestra-1st-violins-professional-by-spitfire-audio
- **Refused and not bypassed:**
  - VI-Control (403);
  - spitfireaudio.com's product pages (429);
  - orchestraltools.com's product pages (they render through JavaScript and came back empty);
  - YouTube's caption endpoint (an empty body).
