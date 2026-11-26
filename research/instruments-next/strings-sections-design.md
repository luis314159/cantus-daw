# Arcus string sections: a design study for round 43

*2026-11-25. Round 43 will build sections for Arcus, the modelled bowed string (ADR 0157; the
player layer in progress, ADR 0162 proposed). The BACKLOG plans them under "Strings and winds,
modelled and recorded": 1–16 players with seeded variation, the stage, the Decca tree,
outriggers, the room, divisi, and a 16-player CPU budget. This file is the study behind that
plan. It covers what the literature measured about sections, the fifteen section libraries
parked in `strings-realism-survey.md` §11.5 and the most respected others, the stage and its
microphones, and a design and a phased plan mapped onto what Cantus already has. It writes no
code.*

It builds on these files and does not repeat them:

- `research/instruments-next/strings-realism-survey.md`: the products (§1), the gap table
  (§6.2), item 15 (§7) and §11;
- `research/instruments-next/bowed-realism.md`: the physics and the reference database
  `bowed-refdb/`;
- `docs/round42/arcus-expr.md`: the HSD quartet study;
- ADR 0124 (Tympanum's virtual microphones, room and bleed matrix);
- ADR 0123 (an instrument's named outputs);
- ADR 0038 (voices on the pool's lanes);
- ADR 0098 §6 and ADR 0104 (Chorda's perspective and scene);
- ADR 0160 (the sound lab's held-out method).

## How to read the labels

The labels are the survey's:

| Label | Meaning |
|---|---|
| **[M]** | The product's manual or support documentation |
| **[R]** | An independent review (the outlet and author are named) |
| **[V]** | The vendor's product page: a claim, not a measurement |
| **[L]** | Published literature: the full text, or the abstract where it says so |
| **[I]** | My inference: reasoning from the sources, not a source |
| **NV** | Could not be verified today: a secondary citation, a search snippet, or memory |

Sources are listed with URLs in §8. Nothing was downloaded except public PDF manuals and open
papers. They were read as text in a scratch folder and not kept. No audio was fetched.

**Refused and not bypassed:**

- VI-Control (403);
- musictech.com (403);
- the IEEE and ResearchGate pages of Pätynen, Tervo & Lokki 2011;
- the publisher's page of their 2012 JNMR paper;
- the CORE copy of Pätynen's 2011 thesis (403).

---

## 0. The short version

### 0.1 Key findings

1. **A section is at least four players, and a listener stops counting at about six.**
   - Ioannou & Kob (2019) recorded a real violin section through spot microphones. Three
     violins were heard as an ensemble by 35 % of listeners, four by 78 %. Six were counted as
     4.4 on average and nine as 5.2 [L].
   - Through a Decca tree, nine violins were heard as an ensemble by 98 % [L].
   - Thilakan & Kob (2021) found the same threshold of four. In reverberant conditions six
     could not be told from nine [L].
   - **An overdub of one violinist did not sound like an ensemble. The same overdub with
     different violins did** [L].
   - Each player therefore needs **its own instrument**, not only its own timing.
2. **The timing spread is tens of milliseconds, and it shrinks with tempo.**
   - Rasch (1979) measured trios: the SD of onset differences was 30–50 ms. The string trios
     were 49 ms and 37 ms; the strings' slow attacks made them the loosest of his three kinds
     of trio [L].
   - Asynchrony correlated 0.64–0.70 with tempo: the faster, the tighter [L].
   - Two professional quartets at 157 bpm measured 28.3 and 24.4 ms, with the first violin
     leading by 3–12 ms (Wing et al. 2014) [L].
   - Listeners notice a change in looseness of about 4–8 ms SD (Wing, Endo, Yates & Bradbury
     2014; my conversion from their ms² thresholds) [L→I].
   - Bow changes in a professional orchestra: half the strokes of three players fell within
     about 60 ms (Pätynen, Tervo & Lokki 2012; abstract only) NV.
   - **Per player against the section's mean** [I]:
     - σ ≈ 25–35 ms loose;
     - σ ≈ 12–20 ms tight;
     - rare outliers up to 100–200 ms, which Rasch found nobody noticed.
3. **The pitch spread between players is about 5–10 cents per note.**
   - In the real violin section, the SD across players of each note's mean F0 ran 0–13 cents
     with two players and 5–15 cents with six. Typical values were 5–10 cents. One player was
     10–30 cents flat throughout (Ioannou & Kob 2019) [L].
   - Violin duets stayed within about 10 cents of the score (Papiotis et al. 2011) [L].
   - Ternström's synthetic unison choirs: an F0 scatter of 0–5 cents was preferred and 14 cents
     was tolerable [L abstract].
   - The default should be σ ≈ 5 cents per note over a per-player bias of σ 2–4 cents, capped
     near 14 [I].
4. **The ensemble effect is beating, not reverb.**
   - Dolson (1983) compared a solo violin with a section. The minimum cue for an ensemble was
     "at least four to eight harmonics, each of which has a characteristic amplitude modulation
     proportional to its frequency" [L abstract].
   - This is what a few cents of spread does: harmonic k of a 440 Hz note beats at about
     k · 1.3 Hz for a 5-cent spread, so the 10th harmonic beats at about 13 Hz [I].
   - Pitch and timing variation together were rated above either alone. Timbre variation
     mattered less (Chandna, Cuesta & Gómez 2020, for unison singing) [L].
5. **The level rises as 10·log10 N, and the section's ff is the player's ff.**
   - Doubling the players adds 3 dB; going from 4 to 5 players adds 1 dB (Meyer 2009 §1.1.3)
     [L].
   - Players never hold a fixed phase, so the sum is incoherent [L, Ioannou & Kob; I].
   - Dynamics do not change the instruments' directivity; they change the spectrum (Pätynen &
     Lokki 2010) [L].
   - A bigger section is therefore louder and smoother, not brighter. The brightness of a
     section's ff must come from each player [I].
6. **Copies of one player phase; distinct players do not.**
   - The libraries say so throughout:
     - Hollywood Strings' "A/B divisi" are the two sides' close microphones of one take, so
       the halves have "no tuning, timing or phase differences" [R SOS];
     - VSL refused a room pair on its individually recorded players because of phasing [R SOS];
     - Dimension Strings' unison notes phase when one section is split into two [R SOS];
     - SWAM String Sections needs a different "Divisi Anti-Phasing" number for each unison
       instance [M].
   - Lokki (2007) found dry copies of one recording sounded like "a loud quintet", and
     decorrelated copies "like a string orchestra". **Once the copies were spread in space with
     early reflections, the decorrelation was no longer needed** [L].
   - This is the case for a cheap hybrid on the back desks: it is plausible, but it must pass
     the ABX.
7. **True divisi is what the most praised libraries record.**
   - Benjamin Wallfisch Strings re-recorded each section as halves: front and back desks for
     the violins and celli, left and right for the violas and basses [R SOS].
   - LA Scoring Strings recorded its divided sections "separately in their own physical
     position, with different players" [R SOS].
   - Fake divisi keeps a full section's weight on each note: Hollywood Strings' microphone
     split, LUX's "Chordal Divisi" note sharing [R SOS; M].
   - VSL Dimension's Auto Voicing gives chord notes to players by playing order, not by pitch,
     and was faulted for it [R SOS].
   - PolySWAM's allocation tells lines from chords with "a capture buffer that adds latency"
     [M].
8. **What reviewers praise most is the sustain that lives, and what they fault most is the
   CPU or the latency.**
   - Wallfisch's unlooped longs of 23–25 s give "a notably more organic sustained timbre"
     [R SOS].
   - SWAM String Sections: "high CPU load with multiple instances" [R SOS].
   - Legato latency must be moved earlier in CSS (≈100/250/333 ms), Berlin Symphonic Strings'
     low spiccatos and LUX's Live mode (−150/−250 ms) [M; R].
   - A timing dial is the norm. Spitfire's manual says of its untightened timing: "laggy and
     sometimes very small inconsistencies in timing. Which we love! It Sounds real! But it's
     not to everyone's taste." [M]
   - An intonation spread needs restraint. Users of VSL's Humanize on ensembles found them
     "out of tune" [forum, quoted by VSL's own forum].

### 0.2 The recommended design in a few lines

- **One Arcus instance is one section** (violins I, violins II, violas, celli or basses) of
  **N = 1–16 players**.
- **Every player is a full modelled string with its own bow and gesture.** Its seeded identity
  sets:
  - an intonation bias and a per-note scatter;
  - an onset habit and a per-note draw;
  - its own vibrato rate, extent and phase;
  - its bow's speed and force;
  - a body variant.
- **The bodies are shared per desk group** (front and back), and each player gets a cheap
  per-player body-difference filter. The body is the costly linear part, and Ioannou's overdub
  says distinct instruments are essential.
- **The section is a lens on the player layer.** ADR 0162 and the character decide the line
  once, as the leader would play it. Each player plays that line with its own offsets. **Player
  0 is the leader and is the solo Arcus to the bit.**
- **Divisi is true divisi.** Players are shared out by pitch, not by playing order:
  - by stand (outside upper, inside lower) by default;
  - by desks or by sides as options;
  - automatically by chord, or manually by note property.
  - Live chords are gathered within the players' own onset spread, which **costs no added
    latency**.
- **Each player sits on a stage** (American or European seating, desks of two) and is heard by
  virtual microphones through Tympanum's path engine:
  - a delay, 1/r, the pattern's gain and a colour per player × mic;
  - image-source early reflections;
  - one late field on the bus.
- **The microphones are named outputs** (ADR 0123): Spot, Leader, Tree, Outriggers and Ambient,
  plus a Perspective macro as Chorda's.
- **The controls:**
  - Players;
  - Tightness;
  - Intonation spread;
  - Vibrato independence;
  - Bowing (together, staggered, free);
  - Divisi mode;
  - Mutes share;
  - Seat, Hall and Mics.
- **The defaults are the literature's middle**: σ about 18 ms, about 5 cents, vibrato
  independent, bowing together.
- **The cost** (§4.1, estimates from loaded-machine measurements) for a 16-violin section with
  its stage: ≈ 15 % of one core in the default render (B), 22–27 % with a body per player (A),
  and ≈ 7 % in the hybrid (C). It is spread across the pool's lanes, with one lane per player.

### 0.3 The phased plan in one table (details in §6)

| Phase | What | Why first |
|---|---|---|
| **43-A** | The seeded identity (size 1, the leader); a section of N full players with shared desk bodies and per-player body filters; the timing, pitch, vibrato, level and bow spreads; uniform bowing; Tightness and Intonation; anti-phasing seeds; true auto and manual divisi; a seated stage for the section, with Spot, Tree and Ambient through Tympanum's paths and early reflections and a late field; the gates and the ABX | The most realism for the effort: spreads and seats are cheap in a model, and they are what makes a section |
| **43-B** | Leader and Outriggers; per-band directivity (Pätynen & Lokki); staggered and free bowing; the shared stage (every section on one stage, spill from geometry); five to seven halls; the Auto render tier (back desks as shadows), only if the ABX cannot tell it from Full; the piano roll's section marks | Depth and cost, once A sounds right |
| **Later** | Half-section mutes (after sordino, survey item 9); a whole-strings *Ensemble* instrument with true auto-orchestration; two section sizes at once (Duality); the stage drawn in both looks with seated figures; surround and height mics | Each one waits on another feature or on the frame budget |

---

## 1. What makes a section sound like a section: the literature

### 1.1 Onset asynchrony

| Measure | Value | Condition | Source |
|---|---|---|---|
| SD of onset differences between voice pairs | **30–50 ms** | Three professional trios in an anechoic room | Rasch 1979 (Acustica 43; read in the 1978 Dutch version) [L] |
| The string trio's SD | **49 ms** (Beethoven), **37 ms** (Dohnányi) | Strings were the loosest; Rasch attributes it to their slow attacks | Rasch [L] |
| Correlation with tempo | **r 0.64 and 0.70** | 18 movements; faster tempo means tighter | Rasch [L] |
| The melody's lead | **A few ms**, near the 5 ms resolution | Wind and string trios | Rasch [L] |
| Unnoticed outliers | **110–200 ms** | Casual listening | Rasch [L] |
| A quartet's mean asynchrony SD | **28.3 ms** and **24.4 ms** | Two professional quartets, Haydn op. 74/1, a mean inter-onset interval of 192 ms | Wing, Endo, Bradbury & Vorberg 2014 (J R Soc Interface) [L] |
| The first violin's lead | **11.7 ms** and **3.2 ms** | The same | Wing et al. 2014 [L] |
| Correction gain | **0.185 and 0.227** (optimum 0.25) | Each player corrects about a fifth of the last asynchrony | Wing et al. 2014 [L] |
| Detection of added looseness | **64 and 18 ms²** (≈ 8 and 4 ms SD) | Listeners hear tightness change | Wing, Endo, Yates & Bradbury 2014 (Front Psychol) [L→I] |
| Bow changes in an orchestra | **Half within about 60 ms** across three players | A professional orchestra, on video | Pätynen, Tervo & Lokki 2012 (JNMR), abstract NV |
| The HSD quartet's stems | **23–41 ms** median \|Δt\| (an upper bound: recorded one by one) | Mozart K465 | `docs/round42/arcus-expr.md` §1.1 [L→I] |
| Unison singers' voiced and unvoiced transitions | Span **0.09–0.13 s** for 4 singers | Choir sections | Chandna, Cuesta & Gómez 2020 [L] |

**Reading** [I]:

- Rasch's and Wing's figures are SDs of pair *differences*. One player's SD against the
  section's mean is about 1/√2 of that: σ ≈ 20–35 ms loose, 12–20 ms tight.
- Wing's correction model says the spread is a **coupled** process. Each player corrects about
  a fifth of its last error, so a player's habit carries from note to note. The model should
  therefore hold:
  - a per-player bias (the habit);
  - a short-memory per-note draw, an AR(1) process with a coefficient of about 0.8;
  - not white noise per note.
- No source measures accented against legato attacks inside a section. The proxies are Rasch's
  attack-time explanation and the tempo correlation.

### 1.2 Pitch spread and vibrato

| Measure | Value | Source |
|---|---|---|
| SD across players of each note's mean F0, real violin section | 2 players 0–12.8 c; 3: 0.6–7.4; 4: 3.7–8.6; 6: 4.6–15.5 c | Ioannou & Kob 2019 (ISMA) [L] |
| The minimum SD | Rises with the number of players | Ioannou & Kob [L] |
| One player | 10–30 c flat throughout; this drove the 15-cent values | Ioannou & Kob [L] |
| Violin duets' deviation from the score | Within about 10 c; biases shared by both players removed | Papiotis, Maestre, Marchini & Pérez 2011 (DAFx) [L] |
| Preferred and tolerable unison F0 scatter | 0–5 c preferred; 14 c tolerable | Ternström 1991/1993 (STL-QPSR; J Voice) [L abstract] |
| Choirs, frame by frame (including vibrato and flutter) | 20–30 c typical (2–60) | Ternström 2003 review NV; Cuesta et al. 2018 [L via Chandna 2020] |
| Solo violin vibrato | 6.63 Hz, 63 c width (four soloists); width grows with dynamic and register; rate grows with dynamic | MacLeod 2010 (String Research Journal) [L abstract] |
| Arcus's measured vibrato (the study) | 5.0–5.7 Hz, 15–44 c p-p; wider loud (×1.4–1.9) and high | `bowed-refdb/vibrato.json`, ADR 0157 pass 5 [L→I] |
| Vibrato phase between section players | No measurement found. Players are incoherent sources with "no static phase relationship" | Ioannou & Kob [L]; Sample Modeling's "Expr. Vibrato" at 0 keeps its players' vibratos asynchronous [R SOS] |

**Reading** [I]:

- The per-note scatter that a "section pitch spread" should copy is **the mean F0 per note
  across players**: σ ≈ 4–7 cents by default. It sits on a per-player bias of σ 2–4 cents.
  The ceiling is about 14 cents (Ternström's tolerable).
- Choir figures of 20–30 cents include vibrato, so they are not a guide for that parameter.
- Vibrato rate across players: no section data exists. σ ≈ 0.3–0.5 Hz about the instrument's
  mean is a guess within the study's 5.0–5.7 Hz band.
- Vibrato extent: about ±20 % per player.
- Vibrato phase: random per note. A section's vibrato is narrower than a soloist's:
  - Ioannou's players were section players;
  - MacLeod's 63 cents is a soloist's figure;
  - our study's 15–44 cents comes mostly from orchestral sample sets.

### 1.3 The chorus or ensemble effect

- **Dolson 1983** (Caltech thesis, a tracking phase vocoder on solo against section violin)
  [L abstract]:
  - the ensemble sensation needs at least 4–8 harmonics with an amplitude modulation
    "proportional to its frequency";
  - that is beating from small F0 differences, since the beat rate of harmonic k is
    k · f0 · Δc · ln2/1200;
  - it follows that the upper partials shimmer fastest [I].
- **Kahlin & Ternström 1999** ("The chorus effect revisited"): the abstract was not obtained NV.
- **Lokki 2007** ("How many point sources is needed to represent strings in auralization?")
  [L]:
  - the dry sum of copies of one recording is "a loud quintet";
  - PSOLA-decorrelated copies are "a string orchestra";
  - placed in space with early reflections, the decorrelation was not needed;
  - the paper recommends one point source per musician.
- **Pätynen, Tervo & Lokki 2011** (WASPAA, "Simulation of the violin section sound based on the
  analysis of orchestra performance"), abstract only NV:
  - contact microphones on an amateur section's bridges;
  - the onsets were measured;
  - copies of one anechoic violin were offset by a phase vocoder to the measured distribution,
    with amplitude modulation and pitch shift added.
  - **The distribution's numbers could not be read** (403).
- **Chandna, Cuesta & Gómez 2020** [L]: the synthesis used a pitch SD of 50 cents and timing up
  to 40 ms; above 40 ms their vocoder degraded. Pitch and timing variation together were rated
  above either alone, and timbre variation mattered less.
- **Ioannou & Kob 2019** [L]: one violinist overdubbed did not read as an ensemble; different
  violins did. Timbre may matter less for voices than for violins, whose bodies differ more.
- **Dünnwald.** No paper on ensemble sound was found under this name. His work is on violin
  quality (the Dünnwald bands). It is left out.
- **Meyer, *Acoustics and the Performance of Music*** (5th ed. 2009). Only the publisher's
  sample was read [L]: §1.1.3 gives the level against the number of players. His directivity
  figures are quoted in Pätynen & Lokki 2010 (§3.3 below). His pp/ff power tables were **not
  verified**.

### 1.4 How section size changes the timbre

| Effect | What happens | Source |
|---|---|---|
| Level | +10·log10 N: doubling the players is +3 dB, 4 → 5 is +1 dB | Meyer 2009 [L] |
| Correlation | Incoherent; a coherent sum (+6 dB a doubling) happens only with copies | Ioannou & Kob [L]; [I] |
| Spectral smoothing | Each player's body peaks fall at different frequencies, so the section's long-term spectrum is the players' average, and each harmonic beats | Dolson [L]; [I] |
| Bow noise | Incoherent noise and harmonics both sum at +10·log10 N. Summing does **not** lower the noise-to-harmonic ratio. The noise "averages out" because the attacks spread over σ (no single scratch), distance and room soften it, and the beating harmonics mask the steady noise | [I]; Ioannou: spot mics make violins countable [L]; Thilakan: reverberation and distance raise blending [L] |
| Countability | Saturates at 4–6 | Ioannou & Kob; Thilakan & Kob [L] |
| Directivity | A section is a spread of sources whose orientations and distances differ, so its directional tone colour (Weinreich 1997) averages out | [I] |

### 1.5 The dynamics against size

- At a constant dynamic per player, N players keep the timbre and gain 10·log10 N dB [L→I].
- Louder playing excites the upper harmonics disproportionately (Pätynen, Tervo & Lokki 2013)
  [L].
- Perceived dynamic strength is only partly the sound power; timbre is a cue (Weinzierl et al.
  2018) [L abstract].
- A section's pp is therefore still a few players' pp, and a 16-player pp is 9 dB above a
  solo's [I].
- Vasks's sections play pp all muted, mf half muted and ff open [R SOS]: a section uses mixed
  techniques to widen its dynamic colour.

### 1.6 Section legato against solo legato

- **No measurement of transition spread in string sections was found.**
- The nearest data:
  - bow-change spread of about 60 ms across three players (§1.1);
  - unison singers' transitions spanning about 0.1 s for four singers;
  - Rasch: onsets after rests and in tempo changes are the least synchronous.
- **Reading** [I]:
  - In a section legato, each player's finger lands at its own time. The note change smears
    over σ ≈ 10–20 ms, less than the onsets, because the left hand follows the beat inside a
    held bow.
  - This is why section legato sounds smooth with fingered changes that would click in a solo.
  - Portamento in sections happens where it is marked, and each player slides at its own
    moment.

### 1.7 Tight against loose attacks

- **Proxies only** [L→I]:
  - Rasch's loosest pairs were slow string attacks;
  - Wing's fast quartet (192 ms inter-onset) was tighter than Rasch's slow trio;
  - Spitfire's Tightness trims the bow's engage time [M];
  - Lineage's "Precision Control" runs from focused to loose with no published ms [V].
- **The design** [I]:
  - a Tightness from σ ≈ 8 ms (tight, accented, fast) to about 30 ms (loose, legato, slow);
  - scaled down further when the inter-onset interval is short;
  - with a rare outlier tail.

### 1.8 Unison against divisi

- Unison blends more than other intervals (Kendall & Carterette 1993); a lower combined
  centroid predicts blend (Sandell 1995). Both NV, via Lembke & McAdams.
- Half a section is −3 dB and closer to the countable range: eight players remain an ensemble,
  but a 6-player section halved is three, which is under Ioannou's threshold [L→I].
- **Orchestral practice:**
  - The bowing is uniform, set by the principal, so the section bows together.
  - **Staggered bowing:** on a note too long for one bow, players change at different times,
    never two neighbours together.
  - **Free bowing:** a deliberately unsynchronised section bowing for a fuller sound,
    associated with Stokowski.
  - Divisi is taken **by stand**: the outside player takes the upper note and the inside
    player the lower, unless the part says "by desks".
  - All three are from pedagogy and orchestration pages, of low authority [L].

---

## 2. The libraries

### 2.1 The fifteen parked in §11.5, and the most respected others

| Library | Sizes vn1/vn2/va/vc/cb | Hall, seating | Mics | Divisi | Controls for section feel |
|---|---|---|---|---|---|
| **Sonuscore LUX Orchestral Strings / Elements** | 18/16/14/12/10 [M][R] | Brno, Besední dům [M]; classical positions [R] | Close, Tree, Far, Bleed (strings-only or orchestral); 8 mixes [M]. The bleed is 22 close mics in the empty wind, brass and percussion seats [R] | **Fake**: "Chordal Divisi 1–3" share notes among sections, "not suitable for legato" [M] | Shorts Tightness; round robins ordered or random; Humanize (MIDI timing); adaptive legato speed; Live mode (first legato −150 ms, then −250 ms) or Lookahead (−250 ms) [M] |
| **VSL Synchron Duality** | A 14/12/10/10/6 (52); B 8/6/5/5/3 (27), **played at the same time** [V] | Synchron Stage A and the small dry Stage B, one conductor over video [V][R] | A: solo, mid, Decca (stereo and mono), surround, high, ribbon. B: "dark" and "bright" pairs [V] | NV | Room blend; agile, slurred and portamento legato [R] |
| **VSL Synchron Prime** | 14/10/8/8/6 and 6/5/4/4/3 [R SOS 2025] | Stage A | Room mix and mid [R] | — | Humanisation in the Flow panel [R] |
| **VSL Humanize** (the player) | — | — | — | — | Presets of N index steps, each a pitch curve over time (cents) and a note-on delay (ms); a threshold after note-off resets to step 1 [M]. VSL says instruments that double in unison or octaves get different curves [forum]. No ranges are published NV |
| **Performance Samples Vista / Vista II / Pacific** | Vista 5 vn/4/3/3; Vista II Lite 3/3/3/2 and 2/2/2/1; Pacific 16 vn/12/10/8 with a 3-violin fff overlay [R SOS; rekkerd] | NV | Vista: Close, Decca. Vista II: Close, Section, Decca, Outrigger, Ambient [R] | none | Slurred legato; the sustain pedal gives same-note bow changes [R] |
| **ProjectSAM Lineage** | 11/9/7/5/3 (35) [V] | Hilversum, MCO hall [V] | Close, stage, ambient, premix [V] | none; "no soloists" [V] | **Precision Control** (focused or loose); **automatic down/up bow alternation**, set or overridden mid-phrase; 8 round robins on shorts, 2 on longs; polyphonic legato; Arranger adds 50 ms to detect chords [V] |
| **Cinematic Studio Strings** | 10/7/7/6/5 [R SOS 2016] | Trackdown, Sydney [R] | Close, Main, Room, Mix [R] | none, which "avoids multiplied player bloat" [R] | Legato delays ≈100/250/333 ms by velocity zone [M] |
| **Audio Imperia Areia** | NV | NV | 4 mics + "Classic" and "Modern" mixes [V] | NV | "True legato" [V] |
| **OT Berlin Symphonic Strings** | 18/16/14/12/8 (68) [R SOS 2021] | Teldex, traditional seating [R] | Spot 1 (about one per two desks), Spot 2 ribbons, Decca (M50s), A-B, **Leader**, Outrigger, Surround [R] | NV | Adaptive legato (melodic, rapid, pattern) [R] |
| **CineStrings Core** | NV | Sony (MGM) scoring stage [V] | NV | NV | NV |
| **EastWest Hollywood Strings** | 16/14/10/10/7 [R SOS 2010] | EW Studio 1 | Close, Mid, Main (Decca), Surround, Vintage [R] | **Fake**: A/B = the left and right close mics of the same performances [R SOS; MusicRadar] | Up to 16 round robins; velocity sets the legato's speed [R] |
| **Spitfire Chamber Strings** | 4/3/3/3/3 (16) [M] | AIR Lyndhurst, "in situ, where they would be seated" [M] | C, T, A, O, ST, Gallery, Close ribbon; three mixes [M] | none | **Tightness** (CC18) trims the bow's engage, with a negative track delay to match; 1–8 round robins; neighbouring zones; legato speed CC16; release CC17 [M]. "CS Blend" mutes one player per desk NV |
| **Spitfire Abbey Road Orchestra: Strings** | 16/14/12/10/8 [R MusicRadar 2024] | Abbey Road Studio One | Many signals; Simon Rhodes's mixes [R] | NV | — |
| **OT Benjamin Wallfisch Strings** | 14/12/10/10/6 [R SOS 2024] | Teldex | 9 positions: Tree, Close, Wide, A-B, Outrigger, Surround, Immersive [R] | **True**: halves re-recorded; violins and celli front and back desks, violas and basses left and right; sustains and legatos only [R] | **Unlooped longs of 23–25 s**; up- and down-bow patches; vibrato tied to the dynamic [R] |
| **OT Pēteris Vasks Strings** | a 22-player chamber group [R SOS 2024] | St John's Church, Riga | 6 positions (omni and cardioid Deccas, close, ribbon, AB, surround) [R] | — | **Longs: pp all muted, mf half unmuted, ff none muted** [R] |
| **Spitfire BBC Symphony Orchestra** | 16/14/12/10/8, leaders sampled solo [R SOS 2019] | Maida Vale Studio One | 20 signals: Close, Close Wide, Tree, **Outriggers**, Ambient, **Leader**, Balcony, Sides, Mids, Atmos, and **five spill** signals [R] | — | Release, "attack tightness", vibrato, variation [R] |
| **VSL Synchron Strings Pro** | 14/10/8/8/6 [R SOS 2021] | Stage A | 13 mics [R] | — | Auto-speed legato [R] |
| **VSL Dimension Strings** | 8 vn, 6 va, 6 vc, 4 cb, **each player recorded alone** [R SOS 2013] | Silent Stage (dry), a mono mic per player; a room pair was rejected for phasing [R] | Per player | **True, by player**: 2×4 or 4 desks × 2 [V][R] | Per-player timing and tuning humanise; Auto Voicing by playing order (faulted) [R] |
| **OT Berlin Strings** | 8/6/5/5/4 [R SOS 2014] | Teldex | Close, Tree, Surround, A/B, concertmaster spot [R] | NV | Adaptive legato [R] |
| **LA Scoring Strings** | vn1 16, 8, 2 × 4 and a first chair; violas 2 × 3 + 6 [R SOS 2010] | — | — | **True**: divided sections "recorded separately in their own physical position, with different players" [R] | — |
| **NI Symphony Series / Session Strings Pro** | Symphony: vn I A/B 8+8, vn II A/B 7+7, va A/B 6+6 [V]. Session Pro: four 11-player sections (4/3/2/2) [R SOS 2011] | — | Session Pro: sections 1 and 2 share a seat at two distances; 3 and 4 sit opposite [R] | Symphony: **Auto Divisi** keeps a section's player count constant across the divisi notes [R] | The reviewer detuned the two halves by about 10 cents "for size" [R] |
| **Audio Modeling SWAM String Sections** (modelled) | Violins 4–6, others 3–5 per divisi [R SOS 2024]; a Section Size control [M] | A virtual room (size, absorption, distance, angle) [M] | Virtual [M] | **Divisi Anti-Phasing** number per instance, assigned automatically, with a warning on a clash [M] | Players Accuracy, split into **Timing** and **Pitch** since v1.2 [M]. The vendor warns the range "is not exaggerated" [M]. About 176 MB a instance [M] (350 MB per SOS) |
| **PolySWAM** | — | Stage placement, Ambiente [V] | — | Lines are told from chords by "a capture buffer" that adds latency; Complete / Multi / Mono allocation [M][V] | "Human Touch": pitch, timing, dynamics [M] |
| **Sample Modeling Solo & Ensemble Strings** | Size "continuously variable" [R SOS 2020] | — | — | — | Players' vibratos asynchronous at Expr. Vibrato 0, synchronised as it rises [R]; "pseudo-random detuning and pitch & dynamics modulation" [R rekkerd] |
| **Logic Studio Strings** | — | — | — | **Auto Voice Split** by register [M Apple] | — |
| **8Dio Century Ensemble** | 6/4/6/4/4 (sordino 6/4/5/4/3) [R, review copy] | — | — | — | — |
| **Free**: LABS Strings, VSCO 2 CE | LABS "fourteen" players [V]; VSCO's sizes unpublished | — | — | — | — |

### 2.2 What reviewers praise and fault, and what it means for a model

- **Praised:**
  - Duality's "amazingly unified nature of the performances" [R SOS]. Two sizes recorded at
    once blend naturally.
  - Wallfisch's unlooped sustains [R SOS].
  - The church acoustic of Vasks [R SOS].
  - LUX's spill as "glue" [R SOS].
  - Chamber Strings' timing "inconsistencies" as what "sounds real" [M].
  - Dimension's control: "an orchestral control freak's dream" [R SOS].
- **Faulted:**
  - latency that must be moved earlier (CSS, Berlin Symphonic, LUX Live, Abbey Road);
  - the CPU (SWAM Sections) and loading time (Abbey Road);
  - "nebulous tuning" (Vista's basses);
  - humanised ensembles "out of tune" (VSL forum);
  - phasing on split sections (Dimension);
  - unpredictable chord voicings (LUX's Ensemble);
  - fake divisi's lack of differences (Hollywood Strings);
  - missing con sordino (Abbey Road), no full-strings patch (Vasks, Berlin Symphonic), no
    vibrato control (Wallfisch).
- **What a model can do that no library can** [I]:
  - any player count, with true divisi at every count;
  - every player distinct with no phasing, by construction;
  - spill from geometry rather than 22 extra microphones;
  - 0 ms live latency, with the section's mean on the grid on playback through look-ahead;
  - no loops: each player's bow changes when its own bow runs out (ADR 0162);
  - half-section techniques at any share.
  - What it cannot do for free is the CPU (§4.1).

---

## 3. The stage and the room

### 3.1 Seatings and sizes

- **American seating** is "the standard over the last century" [R Hoffmann]. From the
  conductor's left: vn1, vn2, va, vc, with the basses behind the celli on the right.
  Stokowski is credited with it NV.
- **European (antiphonal) seating** is older (Mannheim, about 1777): vn1 left, vn2 right, celli
  and violas inside, basses left or behind [R Jacksonville SO; Classic FM]. "German seating"
  has no fixed meaning [R SSO].
- **Film scoring:**
  - The traditional layout is violins left and va/vc/cb right.
  - For 5.1/7.1 action scores, the high strings are split left and right.
  - "The conductor's preference always takes priority" [R CineMontage 2013, Kurlander].
- **Sizes:**
  - 16/14/12/10/8 is "an average breakdown in a very large orchestra" [R].
  - The film libraries cluster at 14/12/10/10/6 (Wallfisch, Duality A).
  - The chamber libraries sit at 4/3/3/3/3 to 8/6/5/5/3.
  - The smaller conventional sets (12/10/8/6/4, 8/6/4/4/2) are unsourced NV.

### 3.2 Distances and areas

- **Floor area per player** (Barron 1993, in Dammerud's thesis, Bath 2009): 1.25 m² per violin
  or viola, 1.5 m² per cello, 1.8 m² per bass. A 100-player orchestra needs about 150 m² net
  [L].
- **A full orchestra** is about 16 m wide and 12 m deep, with up to about 60 ms of direct delay
  between players. Across 12 m, sound between string players falls about 22 dB from distance
  and about 10 dB more at 2 kHz from the bodies in the way, which matters above 500 Hz
  [L Dammerud].
- **Risers:** at the back only, or semi-circular so the back string desks also rise. String
  players prefer curved risers; the model's top riser was 0.95 m [L Dammerud].
- **Desk spacing has no published figure.** Inferred from Barron's areas [I]:
  - a violin desk about 1.6–1.8 m wide and 1.3–1.4 m deep;
  - a cello desk about 1.9–2.0 × 1.5 m;
  - stand partners 0.7–0.8 m apart;
  - rows about 1.3 m apart;
  - the leader 1.5–2 m from the podium.

### 3.3 Directivity, where it matters for the microphones

The main source is Pätynen & Lokki 2010, *Acta Acustica united with Acustica* 96: 22
microphones at 2.13 m in an anechoic room, with "front" being the conductor [L]. Meyer's
figures are quoted there.

| Instrument | Below | The change | Above |
|---|---|---|---|
| **Violin** | ≈ omni under 500 Hz (Meyer: within −10 dB to 600 Hz) | 500 Hz octave to the sides; 1 kHz low-front and above-back; boundary about 1.0–1.25 kHz | 2 kHz front-above; **4–8 kHz "highly concentrated forward"**, only the front within −3 dB. Weinreich: the pattern changes partial by partial above about 1 kHz, so vibrato makes it shimmer |
| **Viola** | — | Boundary 630 Hz; Meyer's strong lobe toward the back plate at about 600 Hz | 1 kHz front-right, 2 kHz front-left; less directional than the violin |
| **Cello** | Within −6 dB everywhere under 300 Hz (Meyer: omni under 200 Hz) | 315 Hz to both sides, low; back attenuated above 500 Hz (front/back > 10 dB from about 500 Hz) | 1.25 kHz to the front at −11° elevation |
| **Double bass** | Strongest low down under 200 Hz | 200–750 Hz strongest at +11°; figure-of-eight-like at 500 Hz | Front-above above 750 Hz; cardioid-like at 1 kHz |

- The dynamic does not change the patterns [L].
- **Licences:**
  - The TU Berlin directivity database (Shabtai, Behler, Vorländer & Weinzierl 2017) is stated
    as CC BY-SA 4.0 in one paper and **CC BY-NC 4.0** in its version 2 record. NC and SA are
    outside ADR 0042, so it is measure-only until its DepositOnce record is checked.
  - Aalto's directivity page states no licence.
  - Cantus should fit its own few-band patterns to the published descriptions above [I].

### 3.4 The Decca tree, the outriggers, spots and rooms

| Mic | What the sources say | Source |
|---|---|---|
| Tree | Three omnis (M50: a capsule in a 40 mm sphere, more directional in the highs, +6 dB at 8–16 kHz on axis), 3–3.6 m high, over or near the conductor. L–R 1.5–2 m (Wikipedia, citing DPA, 0.6–1.2 m; Schoeps ≥ 1.5 m); the centre 0.7–1.5 m forward. "Not even the Decca Tree has a single specified set of dimensions" | [M Neumann; R SOS Robjohns 2020; R Abbey Road Institute; R Wikipedia, citing Pellowe] |
| Outriggers | Omnis at the tree's height or lower, in line with it, 6–9 m apart, about 1.5 m in from the orchestra's edge (or at 2/3 of the stage's width). Richard King: "the best string sound" is on the omni outriggers, set "quite a bit lower" than the mains | [R Mix 2006; CineMontage 2013; R SOS Senior 2015] |
| Spots | One per section, or one per two desks (KM84/184 class), about 1.5–2 m above. Allen Sides: one KM54 per two cellos, a U47 pair on the basses. King avoids cardioid string spots except on the bass | [R Mix 2006; R SOS 2015] |
| Leader | A spot on the concertmaster (BBC SO, Berlin Symphonic Strings, Berlin Strings) | [R SOS] |
| Ambient | "High above the orchestra near the back of the room" (BBC SO); a rear pair of DPA 4006 with spheres a few metres behind the mains facing the hall (King) | [M Spitfire BBC SO manual; R SOS 2015] |
| Spill | BBC SO's five spill signals; LUX's 22 microphones in the empty seats | [R SOS] |
| The share | "The room is about 80 % of the sound" (John Rodd) | [R Mix 2006] |

**Arrival times** [I] (c = 343 m/s, the geometry of §4.4):

- From the vn1 leader's desk, the tree's L–R difference is 4.7 ms and the outriggers' 14 ms.
- A 2 m L–R spacing gives at most 5.8 ms of difference.
- Spots should be delay-aligned to the tree, since a vn1 spot arrives 3–6 ms early. ADR 0123's
  output delay line does this.

### 3.5 Halls and stages to offer

| Room | Volume | RT mid | ITDG | Character | Source |
|---|---|---|---|---|---|
| Vienna Musikverein (shoebox) | 15 000 m³ | 2.0 s full, about 3.0 empty | about 12 ms NV | Narrow, rich, enveloping | [L Hough 2010; IOA Clements] |
| Boston Symphony Hall | 18 750 m³ | 1.8–1.9 s full, 2.5 empty | about 15 ms | Warm, clear shoebox | [L Hough; R Acoustics Today 2009] |
| Amsterdam Concertgebouw | 18 780 m³ | 2.0 s | about 21 ms NV | Wide, very reverberant | [L Hough] |
| Berlin Philharmonie (vineyard) | 21 000 m³ | 1.9 s | about 21 ms NV | Surrounding, less lateral | [L Hough] |
| A scoring stage (Synchron Stage A class) | 540 m², 10.5–12 m high (≈ 6 050 m³ [I]) | 1.6–1.8 s | 10–15 ms [I] | Controlled, film | [M VSL MIR room pack 6] |
| Teldex-like studio hall | 455 m² | about 1.7 s | [I] | Dry-ish and warm | [M VSL MIR room pack 2] |
| Abbey Road Studio One-like | 28.2 × 16.1 × 12.2 m, 6 172 m³ (an IOA paper says 4 500) | 2.2–2.4 s | [I] | Big studio bloom | [R Acoustics Today 2013; M Abbey Road] |
| Maida Vale 1-like | 6 200 m³ | NV | — | — | [L IOA Knowles & Crowe] |
| Recital hall (Wigmore-like) | about 2 900 m³ (check) | about 1.5 s (check) | short | Intimate, chamber | [L Hough, row misaligned] |
| Large church (Vasks's St John's class) | 15 000–30 000 m³ NV | 3–5 s NV | 20–40 ms NV | Long tail, soft attacks | NV |

**Further numbers for the halls:**

- **Stage enclosures:** narrow and high are preferred. Dammerud found these transition values
  between preferred and less-preferred halls: 22.9 m between the side reflectors at the
  strings, 12.8 m reflector height, 10.9 m stage depth [L].
- **Critical distance** r_c ≈ 0.057·√(Q·V/T) [I]:
  - about 5 m in the Musikverein and about 3 m on a scoring stage;
  - a tree 3–5 m from the front desks is near critical distance;
  - ambient mics at 15–25 m hear the direct sound 10–14 dB under the reverberant field.
- **Air absorption** (ISO 9613-1, 20 °C, 50 % RH, computed) [L→I]:
  - 0.47 / 0.99 / 2.97 / 10.5 / 36.5 dB per 100 m at 1 / 2 / 4 / 8 / 16 kHz;
  - at 20 m: 0.6 dB at 4 kHz, 2.1 dB at 8 kHz, 7.3 dB at 16 kHz.
  - It matters most for the late field: air alone gives a 15 000 m³ hall about 410 m² of
    absorption at 4 kHz.

### 3.6 Mapped onto what Cantus has

| Needed | Cantus has | Gap |
|---|---|---|
| Each player's sound reaching each mic: delay, 1/r, the pattern at the angle, a colour | **Tympanum's virtual mics** (ADR 0124 §2): per piece × mic a fractional delay (distance/c), 1/r, the pattern's gain, polarity, a colour; Catmull-Rom reads of a shared source (`tympanum_model_paths.rs`); a piston's high cut-off off its axis | Make "piece" a *seat*. The colour becomes the instrument's directivity per band (§3.3) at the angle between the player's facing and the mic |
| Early reflections | Tympanum's **image paths** from the room's walls and floor; Chorda's tapped-line pattern (`chorda_room.rs`) | Stage walls, the shell and the ceiling as image sources, per seat. ITDG and lateral energy set by the hall |
| The late field | Tympanum's 8-line **FDN room** (`tympanum_room.rs`, RT60 by Jot's rule, to 25 m); **Aula**'s generated halls (ADR 0112: early pattern, three-band late field, true stereo) | **One** late field per mic pair, on the bus, not per player. Air absorption in its highs |
| A bleed matrix | Tympanum's piece × mic cells (`b{p}.{m}.db/inv/on`) and `bleed` fader | **Spill from geometry**: every spot hears every section at its distance. Cells as Tympanum's, with a single *Spill* fader |
| Each mic its own strip | **ADR 0123**: `OutputId` per mic, families, *Each mic to its own strip*; split sums to unsplit to the bit; per-output delay lines | New families: `Spots`, `Main` (tree), `Outriggers`, `Rooms` (exists), `Leader` |
| A perspective knob | **Chorda's** *Perspective* (ADR 0098 §6, hammers ↔ audience) | The same macro over the mic outputs: Close ↔ Tree ↔ Hall |
| One draw shared by every mic | Tympanum: one draw per hit, never per mic | Each player rendered **once**, every mic reads it |
| The pair's comb | Tympanum's lesson: a spaced pair heard a point source as a comb, so its pair now hears the direct sound from its centre | For one player (solo) a tree's comb is real but should be measured. For N players the combs average out. A test holds the coherence by band (§5) |
| The picture | Chorda's scene (ADR 0104), Tympanum's kit scene, the figures (ADR 0159) | A stage drawn from the seats; figures are costly at 60 players (§6, later) |

---

## 4. The design

### 4.1 How N players are rendered, and what it costs

**What the cost is made of.** These are today's measurements (`tests_gate2::probe_cpu`, best of
five, on a machine with other builds running, so not a claim):

| | Value | Source |
|---|---|---|
| One cello voice with the body, the match's EQ, the radiation and the output stage | 0.48 % (pass 1), **0.82 %** (model 2, passes 2–6) of a core at 48 kHz | `docs/round42/arcus.md` §6; `arcus-match.md` passes 2 and 6 |
| Eight cello voices | 2.34 % (pass 1); **2.74–3.03 %** (model 2) | the same |
| One more voice past the first | **≈ 0.27–0.31 %**: the string, the bow and the pitch loop | derived [I] |
| The per-instance part (the body's 64 modes, the 12-section EQ, the radiation, the output) | **≈ 0.5 %**, paid once, because `Arcus` sums its voices into one `Body` | derived from the above; `arcus.rs` (`body`, `upper_body`, `eq`, `out` are fields of the instrument, not of the voice) [I] |
| A violin voice | about **1.5 ×** a cello voice (4× oversampled string); a viola 1.15 × (2×) | ADR 0157, the `arcus-upper` amendment |
| Planning figure | **≈ 1 % a full voice** (the round's own figure), used for design A | the brief |

Today the body is linear and shared, so a polyphonic Arcus has one instrument with eight
strings. **A section needs N instruments.** Ioannou's overdub result (§1.3) says one body for
sixteen players would sound like one violinist sixteen times. Three renders are possible:

| Design | What each player gets | What is shared | Risk |
|---|---|---|---|
| **A. N full voices** | Its own string, bow, pitch loop, **its own body**, EQ and radiation | Nothing but the stage | None in sound; the most CPU |
| **B. N strings, desk bodies (recommended default)** | Its own string, bow, pitch loop and gesture; a **body-difference filter** (three peaking sections at A0, B1− and B1+, shifted by its seeded identity) | One full body per **desk group** (front and back half: k = 2 a section), and the match EQ after it | The difference filter must carry enough of an instrument's identity. The ABX against A decides |
| **C. Hybrid, k full players and N − k shadows** | k front players as in B; each shadow reads one full player's string output **before the body** through its own time-varying delay (its timing offset, a few cents of pitch by the delay's slope, its own vibrato's phase), its own level envelope and its seat | The bow noise and the attack shape of its source player | Correlated attacks and noise between a shadow and its source, which is the phasing every library warns of. Lokki 2007 says space and early reflections hide it. Only an ABX can say |

**The estimates** [I] (% of one core at 48 kHz, before the pool spreads it):

| Assumptions per unit | A | B | C (k = 4) |
|---|---|---|---|
| Violin string, bow and loop ≈ 0.7 %; a full violin voice ≈ 1.2 %; a body group ≈ 0.55 %; a body filter ≈ 0.03 %; a shadow ≈ 0.05 %; the stage's paths ≈ 0.005 % each | | | |
| **16 violins** (strings only) | 16 × 1.2 ≈ **19–24 %** (24 % at the 1.5 % planning figure) | 16 × 0.7 + 2 × 0.55 + 16 × 0.03 ≈ **12.8 %** | 4 × 0.7 + 2 × 0.55 + 12 × 0.05 ≈ **4.5 %** |
| The stage for 16 players and 5 mics (9 channels): direct paths per player (144), early reflections per desk (8 desks × 5 mic centres × 6 images ≈ 240) | ≈ 1.9 % | ≈ 1.9 % | ≈ 1.9 % |
| The late field (one per stage, on the bus) | ≈ 0.5–1 % | the same | the same |
| **A 16-violin section in all** | **≈ 22–27 %** | **≈ 15 %** | **≈ 7 %** |
| **A full string orchestra, 16/14/12/10/8 = 60 players** (violin 0.7, viola 0.41, cello and bass 0.29 a string; 10 body groups) | ≈ 63 % + stage ≈ 70 % | 31 + 5.5 + 1.8 + stage 5 ≈ **44 %** | ≈ 20 % |

- **The pool.** Each player renders on its own lane and the lanes are summed in player order by
  one thread (ADR 0038). The heaviest single node is one player at under 1 % of a core, so the
  pool can spread the section. On 8 cores, B's whole orchestra is about 5.5 % of each core's
  wall time [I].
- **The memory.** About the cello voice's lines per player, plus one body per group. The
  violin's lines already hold its period at 4× (ADR 0157), so a section of 16 is small next to
  a sample library [I].
- **A saving to measure later.** The violin's 4× rate is needed from about E5 up (ADR 0157's
  `arcus-upper` amendment). A player's string could run at 2× below E5, which would roughly
  halve the violin's string cost in the low register. Each note would still be deterministic.
  This must be measured against the pitch gate before it is offered [I].

**Recommendation.**

- **B is the default render.** A is offered as *Quality: Full*, the reference that B's and C's
  ABX compares with.
- **C** becomes *Quality: Auto* only when the owner's blind ABX cannot tell it from A in a held
  chord and a melody. Its shadows sit on the back desks, where the direct sound is lower and
  the room is more.
- **Freeze always works:** a section renders offline to the bit as it plays.

### 4.2 The per-player variation

**Each player's identity** is a pure function of the section's seed and the player's index.
**Each note's draws** are a function of the identity and the note's id (as Tympanum's
`hit_draw`), never of a voice slot or of time.

- **Player 0 is the leader.** At N = 1 with the spreads at their defaults, the section is the
  solo Arcus to the bit. That gives this file's main null test, and item 6's "two solos are two
  players" (the survey §11.4).

| Parameter | Model | Default | Range | Source |
|---|---|---|---|---|
| **Onset**, per player per note | the leader's lead (−3…−12 ms for vn1's player 0) + a per-player bias b (σ_b = 0.5 σ) + an AR(1) draw (ρ 0.8, σ) + a 1 % tail at 3 σ | σ = **18 ms** | 6–35 ms through **Tightness** | Rasch 30–50 ms pairs; Wing 24–28 ms pairs and 3–12 ms lead; Wing's correction gain ≈ 0.2 → ρ ≈ 0.8 [L→I] |
| … by context | σ × min(1, IOI / 0.5 s)^0.5 (tighter when fast); × 0.6 for accented, staccato, spiccato and a start "from the air"; × 1.2 after a rest | — | — | Rasch's tempo correlation and after-rest onsets [L→I] |
| **Finger changes in legato** | each player's slur moves at its own time: σ_leg = 0.5 σ | ≈ 9 ms | follows Tightness | no data; §1.6 [I] |
| **Bow changes** | uniform bowing (the leader's direction); the change time spread σ_bow | **25 ms** | 10–40 ms | Pätynen 2012, about 60 ms for half of three players' strokes NV→I |
| **Intonation bias**, per player | N(0, 3 c), fixed | 3 c | 0–8 c | Ioannou & Kob: per-player tendencies [L→I] |
| **Intonation per note** | N(0, 4 c); the onset 2 × that, settling over τ ≈ 150 ms; total capped at ±14 c | total ≈ **5 c** | 0–15 c through **Intonation spread** | Ioannou 5–10 c; Papiotis ≤ 10 c; Ternström 5 preferred, 14 tolerable [L→I] |
| **Slow drift** inside a note | a random walk, σ 2 c, correlation time 2 s | 2 c | follows the spread | no data [I] |
| **Vibrato rate** | the instrument's `VibSpec` rate × (1 + N(0, 0.06)) per player; the cycle jitter as today | ±0.35 Hz at 5.5 Hz | 0–0.1 relative | no section data; the study's band 5.0–5.7 Hz [I] |
| **Vibrato extent** | × (1 + N(0, 0.2)) per player | — | 0–0.4 | [I] |
| **Vibrato phase and onset** | phase uniform per note; the onset delay × (1 ± 0.3) | independent | **Vibrato independence** 0 (locked to the leader) … 1 | Sample Modeling's Expr. Vibrato [R]; Ioannou: no static phase [L] |
| **Bow** per player | speed × (1 + N(0, 0.08)), force × (1 + N(0, 0.10)), β × (1 + N(0, 0.10)); a slow drift of each inside the note (σ 3 %, 1–3 s) | — | — | survey item 6 [I]; Maestre's contours [L] |
| **Level** | bias N(0, 1 dB) + per note N(0, 0.5 dB) | 1 dB | 0–3 dB | patent US 5,541,354 (15 % amplitude at 3 σ) NV; [I] |
| **Body** | its desk group's body + its body-difference filter: each signature mode's frequency × (1 + N(0, 0.03)), gain N(0, 1.5 dB) | — | — | the spread of the measured bodies: CNSM's three violins, the study's 7 cellos and 5 basses (their A0s); Fritz 2007's thresholds [L→I] |
| **Seat and facing** | from the stage: position, height, facing ± 10° | — | — | §3, §4.4 |

**Anti-phasing is by construction.**

- The section's seed defaults to a hash of the track's id and the section kind, so two
  instances never share players.
- Two Arcus instances of one kind with the same explicit seed get a warning, as SWAM's clash
  warning does.
- Two instances playing in unison (vn1 and vn2 doubling) are 2N distinct players.

**Live and playback.**

- **Live** has no look-ahead (ADR 0162). Every offset is shifted so that **the earliest player
  starts at 0 ms**, usually the leader. The section has no added latency, and its median comes
  about one σ late, as a real section behind its leader does.
- **On playback** the player layer reads ahead. The section's **median onset sits on the
  grid**, and players start both before and after it. This is the libraries' "move it earlier",
  done for the user.
- A choice *Align: Section | Leader* keeps the live feel on playback for those who want it.

### 4.3 Divisi

**The rule: players are shared out by pitch and by seat, never by playing order.**

- **By stand (the default, orchestral practice).** Players are numbered in seat order: desk 1
  outside, desk 1 inside, desk 2 outside, and so on. With k notes, player i takes the
  (i mod k)-th note from the top. For two notes, the outside players take the upper note and
  the inside players the lower. Each part is then spread over the whole section's area, as on
  a real stage.
- **By desks.** Contiguous groups, front to back; the front desks take the upper note
  (Wallfisch's violins and celli).
- **By sides.** The left and right halves (Wallfisch's violas and basses).
- **Level.** Each note's players sum incoherently, so a k-way divisi is 10·log10(N/k) per note.
  This is true divisi.

**Automatic divisi, by the chord's notes:**

- **On playback** (with look-ahead) the chord is known before it sounds, and the assignment is
  exact.
- **Live**, the onset spread is the gathering window:
  - a player whose scheduled onset has not come yet is still free, and a chord note that
    arrives in the meantime takes its share of the free players;
  - players already bowing keep their note until their next onset;
  - with σ ≈ 18 ms, most of the section is free for the first 15–30 ms;
  - **this costs no latency**, unlike PolySWAM's capture buffer and Lineage's 50 ms [I].
  - An optional *Chord window* (0–40 ms) holds back only the players that would start before
    it.
- **Continuity:**
  - players on a line keep it through a slur; reassignment happens only at their own new
    onsets;
  - when a chord closes to a unison, the players rejoin at that onset, slurring if they are
    already on that pitch;
  - a line that moves under a held chord note keeps its own players (the separation of lines
    from chords that PolySWAM makes).
- **More notes than players:**
  - each note gets at least one player, the outer voices first;
  - until double stops exist (survey item 13), the notes beyond N are played by extra voices
    up to 4, with a warning glyph in the piano roll;
  - a single-player section keeps today's polyphony.

**Manual divisi** is a note property in round 43's adaptive piano roll (§4.6):

- *div. i of n*;
- *solo*: the leader alone;
- *half*: the front desks;
- *tutti*: overrides automatic divisi.

**Mixed technique inside a section.** A *Mutes share* (0–1) puts that share of the players
con sordino, Vasks's half-section idea. It waits on sordino as a technique (survey item 9), and
a curve against the dynamic is an option (pp all muted, mf half, ff none).

### 4.4 The stage and the mics

**The default geometry** [I, areas after Barron; §3.2]:

- The conductor stands at the origin facing +y (upstage); +x is the conductor's right.
- Desks sit on arcs centred 2 m behind the podium, with radii 4.0 + 1.3 k m.
- Each desk holds two players 0.75 m apart. A source sits 0.9–1.0 m high, a bass's at 1.2 m.

| Section (16/14/12/10/8) | American: centroid (x, y) m, extent | European |
|---|---|---|
| vn1, 8 desks | (−5.8, 1.9); x −8.3…−3.3, y 0.2…4.2; the leader's desk (−3.3, 0.2) | the same |
| vn2, 7 desks | (−2.7, 4.2) | (4.0, 1.6): the right front |
| va, 6 desks | (0.9, 4.3) | the same |
| vc, 5 desks | (4.1, 1.7) | (−2.6, 3.8) |
| cb, 8 players | (6.2, 5.4), on a riser | (−6.2, 5.4): the left back |

- **Smaller sections** fill the front rows of their wedge. A **quartet or quintet** uses the
  survey §11.4 seating: vn1 left, vn2, va, vc right, cb behind.
- **Facing:** each player faces the conductor ±10° [I].
- **Directivity** is a few-band pattern per instrument fitted to §3.3's descriptions. Above
  about 1 kHz (the violin; 630 Hz the viola, 500 Hz the cello) a seeded fine structure moves
  with the vibrato. That is Weinreich's directional tone colour, and the survey's item 4.

**The default microphones** [S: from the sources; I: my values]:

| Output (ADR 0123 family) | Position, pattern | Basis |
|---|---|---|
| **Spot** (`Spots`), one per section | 1.8 m above the front desks' centroid; cardioid; a stereo pair on the basses | S: one per section or per two desks; I: the height |
| **Leader** (`Spots`) | 0.8 m above vn1's first desk; cardioid | S: BBC SO and OT's leader mics |
| **Tree** (`Main`), three channels L C R | L/R (∓1.0, −0.5, 3.2), C (0, +1.0, 3.2); an omni with the sphere's high-frequency directivity, angled out and down | S: 3–3.6 m, 1.5–2 m wide, centre 0.7–1.5 m forward; the M50's response |
| **Outriggers** (`Main`) | (∓3.5, +1.0, 2.8); the same capsule, angled out, a little lower than the tree | S: 6–9 m apart, lower (King) |
| **Ambient** (`Rooms`) | (∓3.0, −14, 6.0); omni, facing the hall | S: high and far back |
| **Spill** | each spot also hears the other sections at their distances (geometry), under one *Spill* fader and Tympanum-style cells | S: BBC SO spill, LUX bleed; Tympanum's matrix |

**The rendering, onto Tympanum's engine:**

- Each player is rendered **once**. Every mic reads it through a path: a fractional delay, 1/r,
  the pattern's gain at the angle, the instrument's directivity at the angle and air
  absorption. These are Tympanum's paths with the seat as the piece.
- **Early reflections:** image sources of the floor, the stage shell, the side walls and the
  ceiling, per **desk** to each mic **centre**. This is Tympanum's lesson about a pair, and it
  keeps the cost per desk rather than per player.
- **The late field:** one per stage, on the mic buses. Tympanum's FDN for speed; Aula's
  generated halls for the five to seven rooms of §3.5. Air absorption sits in its highs.
- **The spots' delay** is aligned to the tree by ADR 0123's output delay, on by default for
  spots.
- **The *Perspective* macro**, as Chorda's, crossfades the outputs' levels: Close (spots and
  leader) ↔ Tree and Outriggers ↔ Hall (ambient). The default is the tree with a little spot,
  because the reviewed libraries' default mixes are tree-led.
- **Outputs:** *All to Main* (the mix), *Each mic to its own strip*. Split sums to unsplit to
  the bit (ADR 0123).

**Where the stage lives** (an ADR decision):

- **Phase A:** each Arcus instance holds its own stage settings (seating, hall, mics). Two
  sections seat consistently because the seat comes from the section kind and the seating.
- **Phase B:** a **project-level stage**. Arcus instances that name it share the seating,
  the hall, the mic set and one late field, and the spill becomes real between instances.
  This is a schema change with a migration.

### 4.5 Controls and defaults

| Control | Default | Range | Notes |
|---|---|---|---|
| **Section** | (kind) | vn1, vn2, va, vc, cb, or Solo | sets the seat, the instrument and the default size |
| **Players** | by **Size** preset | 1–16 | Size presets: *Symphonic* 16/14/12/10/8, *Film* 14/12/10/10/6, *Chamber* 8/6/6/4/3, *Small* 4/4/3/3/2, *Solo* 1. The default is *Film* [I]. Ioannou: an ensemble from 4. Thilakan: 6 ≈ 9 in a room |
| **Quality** | Balanced (B) | Full (A), Balanced (B), Auto (C, if it passes) | |
| **Tightness** | 0.5 (σ 18 ms) | 0 (35 ms) … 1 (6 ms) | one knob, as Spitfire, Lineage and LUX |
| **Intonation spread** | 0.35 (≈ 5 c) | 0 … 1 (15 c) | separate from timing, as SWAM v1.2 |
| **Vibrato independence** | 1 | 0 (locked to the leader) … 1 | |
| **Bowing** | Together | Together (re-bows together; on long notes each player changes when its bow runs out, never two stand partners within 0.3 s, i.e. staggered), Exact (every change together), Free (each player's own) | Lineage's auto bowing is already Arcus's (ADR 0158) |
| **Divisi** | Auto, by stand | Auto or Manual only; by stand, desks or sides | |
| **Align** | Section | Section or Leader | playback only |
| **Mutes share** | 0 | 0 … 1, or by dynamic | after sordino |
| **Seating** | American | American, European, Scoring (stereo violins) | |
| **Hall** | Scoring stage | the §3.5 rooms | |
| **Mics** | Tree + a little spot | Spot, Leader, Tree, Outriggers, Ambient; levels; Perspective | |
| **Seed** | from the track | an explicit number | the clash warning |

Every control at its default reads as the default draws. "Null at defaults" holds for the
spreads' *amount* keys (§5).

### 4.6 How sections meet the player layer, the characters and the piano roll

**The player layer as the conductor's part.**

- ADR 0162's state is computed once per section: the bow's direction and its part, the start
  (string or air), the fingering, the shifts. So are the survey's look-ahead items 7–8 and the
  character (item 16). That is the line as the **leader** plays it, the way a section follows
  its principal's bowings and fingerings.
- Each player then plays that line through its identity:
  - its onset and finger-change offsets;
  - its intonation;
  - its vibrato rate, extent and phase;
  - its bow's speed, force and β;
  - its own bow position along the stick, which moves at its own speed, so it runs out at its
    own time;
  - its body and its seat.
- **One fingering for the whole section** is the orchestral norm. Portamento happens only where
  the line has it, and each player slides at its own moment.

**Characters** (survey item 16) are section-wide. *Lyrical*, *expressive*, *tasto* and *senza*
move force, speed, β, vibrato and portamento together. Each player keeps its own offsets around
the character's trajectory, as Vasks's players play one style together.

**The adaptive piano roll** (BACKLOG "One piano roll that adapts"). A section's playing profile
offers, as open ids stored by name (ADR 0162's convention):

- the solo's techniques;
- `section.div.<i>of<n>`, `section.solo`, `section.half`, `section.tutti`;
- `sord.on` / `sord.off` (a region or a note);
- `bowing.together`, `bowing.free`, `bowing.exact` (a region);
- a per-region **Tightness** as a continuous expression lane;
- the bow marks (down, up, part, start, string) as ADR 0162 defines them.

The roll draws a note's divisi as a glyph and its players' share as a thin bar under the note,
never as colliding text. The **score** shows *div.*, *unis.*, *solo* and *con sord.* from the
same properties. **MusicXML** carries *div.*, *unis.* and *con sord.* as text directions and a
note's string as `<technical><string>`. The writer maps them; the reader takes them where they
are present. No dedicated divisi element was verified NV.

---

## 5. The tests and the measurements

### 5.1 The house invariants, made for sections

| Test | What it holds |
|---|---|
| **The leader's null** | N = 1, Players' spreads at default, the stage off: the output equals the solo Arcus **to the bit**, so the player layer's hashes stay put |
| **The copies' null** | The identity off (every player the leader, with no seat): N players equal N × the solo, exactly, summed in player order. This shows the summation and the lanes are right before any spread exists |
| **Null at defaults** | Each new key at its default touches no sample. The *amount* keys (Mutes share 0, Spill 0, Chord window 0) are bit-exact against their absence. The spreads' defaults are the draws themselves, held by golden hashes |
| **Block-size independence** | 1, 64, 333 and 512 frames give the same bits, with the seats, the paths and the late field on |
| **Offline equals real time** | The render and the live playback of one song are the same bits, look-ahead included. Live input is causal by design and is tested apart |
| **The pool equals serial** | 16 players on 1 thread and on 8 give the same bits (ADR 0038) |
| **Determinism** | The draws depend only on (seed, player, note id). Moving a note's neighbour does not change another note's draws |
| **Rest** | A section at rest is exact. A player ends when its string is still. The stage's lines rest by Tympanum's rule |
| **Real time** | `rt_memory`: a 16-player section with the stage plays allocation-free, divisi reassignment included |
| **Split sums to unsplit** | Each mic to its own strip sums to *All to Main* to the bit (ADR 0123) |
| **End to end** | Through `CantusApp::assemble`: an Arcus *Violins I* at *Film* size plays a chord, divides it, sounds on its Tree and Spot strips, and stops |
| **Schema** | A bump with a fixture for the section keys, and every old fixture loading (an older Arcus is a section of 1) |
| **Undo** | Every new command in the undo fuzz (Players, Divisi, the note properties) |

### 5.2 The section's own gates, before anyone listens

1. **The spreads are what was asked.** Each player renders on its own lane, so the spread is
   measured exactly, not estimated:
   - the onsets' σ against Tightness (within 10 %);
   - the per-note mean F0's SD across players against Intonation spread (within 1 cent);
   - the vibrato rates' SD;
   - the bow changes' spread.
2. **No phasing.**
   - Two instances in unison, and the two halves of a divisi'd unison: their magnitude-squared
     coherence per third-octave band stays under a bound (to be set from VSCO's sections,
     about 0.3 [I]);
   - the sum's long-term spectrum divided by the sum of the parts' spectra stays flat within
     ±1 dB, which means no comb.
3. **The level law.** Over N = 1…16 at a constant dynamic, the section's long-term level
   follows 10·log10 N within ±0.5 dB.
4. **The ensemble cue.** The partials' amplitude-modulation depth and rate grow with the
   harmonic number, as Dolson describes. The modulation rate of harmonic k is near
   k · f0 · σ_c · ln2/1200.
5. **The divisi rules.**
   - By stand, the outside players take the upper note.
   - No player changes note inside a slur.
   - A live chord within the window reaches its share with no added latency.
   - Playback assigns exactly.
6. **The stage.**
   - Each mic's arrival delay equals distance/c within a sample.
   - A spot aligned to the tree has no pre-echo.
   - The tree's L–R coherence over the section by band is within the band measured from a
     reference with a tree (§5.3).

### 5.3 The sound lab: section references for the held-out method

**What the catalogue holds** (`tools/sound-refs/catalog.toml`):

| Set | What it gives for sections | Licence | Use |
|---|---|---|---|
| `strings-rec` / VSCO 2 CE | **Violin, viola and cello sections** (sustains with and without vibrato, tremolo, spiccato, pizzicato), unknown size; a solo violin and solo bass from the same project | CC0 | ship; **the core section reference**: its notes against Arcus's sections note by note, dynamic by dynamic |
| `study-hsd-string-quartet-k465` | Four players recorded one by one over a guide; asynchrony 23–41 ms median (an upper bound), balance; **not a section** | CC BY 4.0 | ship-credit; the leader's lead and the line's timing |
| `study-aalto-anechoic-orchestra`, `study-phenicx-anechoic` | One player per part in an anechoic room: **not sections**. The orchestra's timing across parts is real, the players following a video conductor | academic only; annotations NC | measure only |
| `study-cnsm-violin` | Three violins' admittances | CC BY 4.0 | **the body variants' spread** |
| The study's database (`bowed-refdb`) | 7 cellos and 5 basses with their A0s | per file | the cello and bass body spread |
| Freesound CC0 previews | Some "string section" sounds may exist | CC0 | extend the set by query; check each file's licence |

**What is missing, and where to look** (none of this was fetched; each needs a licence check):

- **Public-domain orchestral recordings with real sections.** Musopen's releases are offered
  as public domain, which makes them in-context references for the ABX and for whole-mix
  measures (the beating, the section's spectrum). They are not isolated stems NV.
- **Ioannou & Kob's violin-ensemble recordings.** Their availability is unknown; writing to
  the authors is the owner's call NV.
- **URMP** (Rochester multi-modal ensembles): its licence is unverified NV.
- The **TU Berlin directivity database** is measure-only until its licence is settled (§3.3).

**The held-out method** (ADR 0160) for sections:

- Cells are articulation × note × dynamic of VSCO's three sections. A third is held out by
  the SHA-256 rule.
- **What is fitted on the other two thirds:**
  - the defaults of σ_c (the partials' bandwidth and AM against a solo's);
  - the vibrato spreads;
  - the attack's rise widening against a solo;
  - the body filter's spread;
  - each judged on the held-out third.
- **Each physical instrument weighs one.** Here VSCO's sections are one "instrument" each, so
  the band is narrow and that is said plainly. **One size can be measured, not a band by
  size.** The size law (10·log10 N, countability) comes from the literature, not from our
  references.
- **The estimators to add to the lab:**
  - the partials' AM spectrum per harmonic (the beating);
  - the per-partial bandwidth (the pitch spread);
  - the onset envelope's 10–90 % rise against the solo's at the same note (the onset spread);
  - the noise-to-harmonic ratio in the attack's first 100 ms.

**The listening.**

- **The owner's blind ABX:**
  - Arcus Violins I (8 players, then 16) against VSCO's violin section, one note per dynamic
    and a four-bar phrase with legato and a CC1 swell;
  - then the Full (A) against the Balanced (B) render, and against the Auto (C) render;
  - then two instances in unison against one double-size section.
- **The survey's free floor:** the owner may compare by ear with Berlin Free Orchestra and BBC
  SO Discover's sections if he installs them. Their audio never enters the repository.
- **What the owner should hear** (survey item 15): eight violins holding a chord and playing a
  melody, with no phasing and no "copied" sound.

### 5.4 The CPU gate

Following `docs/RELEASING.md` L3–L5, the gate is **ratios against a reference timed beside
them**, never fixed milliseconds:

- **Balanced (B):** a 16-player section costs ≤ **0.85 ×** sixteen solo voices (each with its
  own body) timed beside it. The shared desk bodies must save what they promise.
- **Full (A):** ≤ 1.1 × sixteen solo voices; the stage's overhead stays small.
- **The stage:** with 5 mics ≤ 0.25 × the section's strings.
- **The pool:** no single lane over one player's cost × 1.2; sixteen players across the pool.
- **The absolute number for the record:** measured on a quiet bench by the integrator (the
  bench's `inst.arcus.section.v16` beside `inst.arcus.v8`), never in a test.

---

## 6. The phased plan

### 6.1 Round 43, phase A: the most realism per effort

1. **The seeded identity at size one** (the survey's item 6, widened). An identity struct:
   - the timing habit;
   - the intonation bias;
   - the vibrato rate, extent and onset habits;
   - the bow's speed, force and β habits;
   - the slow drifts inside a note;
   - a body-difference filter.

   Two Arcus solos are then two players. The null: identity "leader" equals today's bits.
2. **The section of N players**:
   - `Players` 1–16 and the Size presets;
   - render B (desk bodies plus per-player body filters), with A as Full;
   - each player on its own lane;
   - the per-note draws of §4.2;
   - uniform bowing with its change spread;
   - Tightness, Intonation spread and Vibrato independence;
   - anti-phasing seeds and the clash warning;
   - live alignment (earliest at 0 ms) and playback alignment (median on the grid).
3. **True divisi**:
   - Auto by stand, by desks or by sides;
   - the live gathering in the onset spread;
   - continuity through slurs;
   - manual `section.div.*`, `solo`, `half` and `tutti` as note properties, so the piano roll's
     profile has them from day one.
4. **The seated section on a stage, through Tympanum's paths**:
   - American and European seating;
   - Spot, Tree and Ambient as named outputs with Perspective;
   - early reflections per desk;
   - one late field;
   - three rooms (a scoring stage, a shoebox hall, a recital hall);
   - the spots' delay alignment.
5. **The gates of §5** and the lab's four section estimators. The ABX against VSCO's sections
   and Full against Balanced.

**Why this order** [I]:

- Items 1–2 are what Ioannou, Dolson, Rasch and Lokki say makes a section, and in a model they
  are draws, not recordings.
- Item 3 is where the libraries fail (fake divisi, playing-order voicing, capture latency) and
  a model does not.
- Item 4 is required for any section to be judged: the references are in rooms, and Lokki found
  the room hides what the copies lack.

### 6.2 Round 43, phase B, or round 44: depth and cost

- The **Leader** and **Outriggers** mics, and **Spill** from geometry.
- **Per-band directivity** per player, with the seeded fine structure moving with the vibrato
  (survey item 4).
- **Staggered and free bowing**; Exact bowing.
- **The project-level stage**, shared by every section: one late field, true spill between
  instances, a schema change and an ADR.
- **Five to seven halls** (§3.5) from Aula's generator, fitted to the published volumes, RT60s
  and ITDGs.
- **The Auto render (C)**, the back desks as shadows, **only if** the owner's ABX cannot tell it
  from Full.
- **The 2× low-register violin string** (§4.1), if the pitch gate holds.
- **A per-region Tightness lane** and the bowing regions in the piano roll.

### 6.3 Later

- **Half-section mutes** and the sordino curve against the dynamic, after sordino is a
  technique (survey item 9).
- **An *Ensemble* instrument**: whole strings from one part, with true orchestration across the
  sections. A chord voiced by register onto vn1, vn2, va, vc and cb, each a real section. This
  is LUX's "Chordal Divisi" done with true players.
- **Two sizes at once** (Duality): a large and a small section of one part, on two stages.
- **The stage drawn in both looks.** The seats are lit as they sound and the mics are dragged
  where they stand (Tympanum's scene). Seated figures (ADR 0159) at 60 players need a frame
  budget of their own: baked, instanced poses.
- **Surround and height mics.**
- **Double stops in a section** (survey item 13), so that more notes than players are divided
  as a real section would.

---

## 7. What could not be verified, and the open questions

- **Pätynen, Tervo & Lokki's WASPAA 2011 and JNMR 2012 numbers** (the onset and bowing
  distributions in a real section): 403. Their thesis (Aalto DD 86/2011) also refused. They are
  the most directly relevant measurements, and worth the owner's library access or a request to
  the authors.
- **Meyer's pp/ff power tables per instrument and section**: not verified.
- **No measurement of vibrato rate spread between section players** and **none of legato or
  portamento timing in sections** was found. §4.2's values there are inferences, to be fitted
  on VSCO's sections.
- **VSCO 2 CE's section sizes** are unpublished, so the held-out band is one size.
- **ITDGs of the Musikverein, the Concertgebouw and the Philharmonie** (Beranek 2004) are from
  memory; desk spacing has no published figure; the TU Berlin directivity licence conflicts
  between its two papers.
- **Open for the owner and the round:**
  - the default size (*Film* proposed);
  - whether an Arcus section is a new instrument entry in the picker per section (*Violins I*,
    …) or one Arcus with a Section choice (recommended: one Arcus, with presets per section);
  - whether the project-level stage comes in round 43.

---

## 8. Sources

### Literature

- Rasch, R. A. (1978). "Synchronisatie bij ensemblespel", *Mens en Melodie* 33, 41–46. It is
  the Dutch version of "Synchronization in performed ensemble music", *Acustica* 43 (1979),
  121–131. https://publications.tno.nl/publication/34637177/fvRTQS/rasch1979synchronisatie.pdf
- Wing, A. M., Endo, S., Bradbury, A. & Vorberg, D. (2014). "Optimal feedback correction in
  string quartet synchronization", *J R Soc Interface* 11:20131125.
  https://pmc.ncbi.nlm.nih.gov/articles/PMC3928944/
- Wing, A. M., Endo, S., Yates, T. & Bradbury, A. (2014). "Perception of string quartet
  synchronization", *Front Psychol* 5:1115. https://doi.org/10.3389/fpsyg.2014.01115
- Pätynen, J., Tervo, S. & Lokki, T. (2011). "Simulation of the violin section sound based on
  the analysis of orchestra performance", IEEE WASPAA, 173–176. doi:10.1109/ASPAA.2011.6082305
  (abstract only; 403).
- Pätynen, J., Tervo, S. & Lokki, T. (2012). "Temporal differences in string bowing of
  symphony orchestra players", *J New Music Research*. doi:10.1080/09298215.2012.708048
  (abstract via the index; 403).
- Pätynen, J. (2011). *A virtual symphony orchestra for studies on concert hall acoustics*,
  Aalto DD 86/2011. https://core.ac.uk/download/80704016.pdf (403).
- Pätynen, J. & Lokki, T. (2010). "Directivities of symphony orchestra instruments", *Acta
  Acustica united with Acustica* 96, 138–167. doi:10.3813/AAA.918265.
  https://users.aalto.fi/~ktlokki/Publs/patynen_aaua_2010.pdf
- Pätynen, J., Tervo, S. & Lokki, T. (2013). ISRA 2013, on the spectrum against the dynamic.
  https://users.aalto.fi/~ktlokki/Publs/isra2013_P046.pdf
- Lokki, T. (2007). "How many point sources is needed to represent strings in auralization?",
  ISRA 2007. https://users.aalto.fi/~ktlokki/Publs/P11.pdf
- Ioannou, S. & Kob, M. (2019). "Investigation of the blending of sound in a string ensemble",
  ISMA 2019, 42–49. http://pub.dega-akustik.de/ISMA2019/data/articles/000093.pdf
- Thilakan, J. & Kob, M. (2021). DAGA 2021, the ensemble impression of violins.
  https://pub.dega-akustik.de/DAGA_2021/data/articles/000566.pdf
- Papiotis, P., Maestre, E., Marchini, M. & Pérez, A. (2011). "Synchronization of intonation
  adjustments in violin duets", DAFx-11.
  https://www.dafx.de/paper-archive/2011/Papers/83_e.pdf
- Ternström, S. (1991/1993). "Perceptual evaluations of voice scatter in unison choir sounds",
  STL-QPSR 32(2-3), 41–49; *J Voice* 7(2), 129–135 (abstract).
- Ternström, S. (2003). "Choir acoustics: an overview of scientific research published to
  date", *Int J Research in Choral Singing* 1(1), 3–12 (NV).
- Kahlin, D. & Ternström, S. (1999). "The chorus effect revisited", EUROMICRO.
  doi:10.1109/EURMIC.1999.794765 (not obtained).
- Dolson, M. (1983). *A tracking phase vocoder and its use in the analysis of ensemble
  sounds*, Caltech PhD. doi:10.7907/0RC3-V146. https://thesis.caltech.edu/3297/ (abstract).
- Chandna, P., Cuesta, H. & Gómez, E. (2020). "A deep learning based analysis-synthesis
  framework for unison singing", ISMIR 2020. arXiv:2009.09875
- MacLeod, R. B. (2010). *String Research Journal* 1(1), 75–83. doi:10.1177/194849921000100105
  (abstract).
- Meyer, J. (2009). *Acoustics and the Performance of Music*, 5th ed., Springer, §1.1.3 (the
  publisher's sample).
- Weinzierl, S. et al. (2018). *JASA* 144, 1347. doi:10.1121/1.5053113 (abstract).
- Weinreich, G. (1997). "Directional tone color", *JASA* 101, 2338 (not opened; cited in the
  survey).
- Kendall, R. & Carterette, E. (1993). *Contemporary Music Review* 9, 51–67. Sandell, G.
  (1995). *Music Perception* 13, 209–246 (both NV, via Lembke & McAdams).
- Dammerud, J. J. (2009). *Stage acoustics for symphony orchestras in concert halls*, PhD,
  Bath. https://www.akutek.info/Papers/JJD_Stage_acoustics_PhDthesis.pdf. The same author's
  overview: https://iml.esm.rochester.edu/polyphonic-archive/article/stage-acoustics-for-symphony-orchestras-just-black-magic-part-i/index.html
- Hough, J. (2010). *Acoustics Australia* 38(2) (the halls' volumes and RTs).
  https://www.acoustics.asn.au/journal/2010/2010_38_2_Hough.pdf
- Clements, P. A., on the Musikverein, IOA proceedings.
  https://www.ioa.org.uk/system/files/proceedings/pa_clements_reflections_on_an_ideal_tradition_and_change_at_the_grosser_musikvereinssaal_vienna.pdf
- Knowles, I. & Crowe, N., on BBC Hoddinott Hall and Maida Vale, IOA proceedings.
  https://www.ioa.org.uk/system/files/proceedings/i_knowles_n_crowe_the_acoustic_design_of_the_bbc_hoddinott_hall.pdf
- Shabtai, N. R., Behler, G., Vorländer, M. & Weinzierl, S. (2017), the TU Berlin
  directivity database, *JASA*. Its licence is stated in arXiv 2307.02110 and in Ackermann et
  al., JAES 2024 (doi 10.14279/depositonce-5861.2); they conflict.
- ISO 9613-1, the air's absorption (formulas applied; tables not opened).
  https://www.iso.org/standard/17426.html

### Products: manuals, vendors and support

- Sonuscore LUX: https://manuals.sonuscore.com/lux-orchestral-strings.html ·
  https://manuals.sonuscore.com/lux-orchestral-strings-elements.html
- VSL:
  - Duality Strings: https://www.vsl.co.at/synchron/duality-strings
  - Synchron Prime: https://www.vsl.co.at/instruments/starter-editions/prime-orchestra
  - the Synchron Player's Humanize: https://www.vsl.co.at/manuals/synchron-player/edit-tab
  - the VSL forum on Humanize: https://forum.vsl.co.at/post/199360
  - the MIR room packs: https://www.vsl.co.at/manuals/mir-roompacks/roompack6 ·
    https://vsl.co.at/manuals/mir-roompacks/roompack2
- ProjectSAM Lineage: https://projectsam.com/libraries/lineage-strings
- Spitfire:
  - Chamber Strings Pro's manual: https://www.spitfireaudio.com/cdn/shop/files/Chamber_Strings_Pro.pdf
  - the BBC SO manual: https://www.spitfireaudio.com/cdn/shop/files/BBC_SO_Piano_Professional_User_Manual_v2.pdf
- Audio Modeling:
  - SWAM String Sections v1.4.0's manual: https://static.audiomodeling.com/manuals/stringsections/SWAM%20String%20Sections%20v1.4.0%20-%20User%20Manual.pdf
  - the accuracy KB article: https://kb.audiomodeling.com/en/swam-string-sections/i-dont-hear-a-difference-when-adjusting-the-players-accuracy-parameter-is-it-not-working
  - PolySWAM: https://kb.audiomodeling.com/support/solutions/articles/206000082778-what-can-polyswam-do-
- Native Instruments Symphony Series String Ensemble: https://www.native-instruments.com/products/symphony-series-string-ensemble
- Apple, Logic Studio Strings: https://support.apple.com/guide/logicpro-ipad/lpip40b910a5/ipados
- Aaron Venture: https://aaronventure.squarespace.com/infinite-series-announcement
- Neumann M 50: https://www.neumann.com/en-us/products/historical/m-50
- Abbey Road, the studio's RT: https://www.abbeyroad.com/news/the-ambiophony-system-gearthatmadeus-3131
- Segerstrom Hall's platform: https://www.scfta.org/Segerstrom/media/SCFTA/About%20Us/Information%20Sheets/RandH_Segerstrom_Concert_Hall.pdf

### Reviews and articles

**Sound On Sound:**

- LUX (Magnus, 2026): https://www.soundonsound.com/reviews/sonuscore-lux-orchestral-strings
- Duality (Stewart, 2024): https://www.soundonsound.com/reviews/vienna-symphonic-library-synchron-duality-strings
- Prime (Stewart, 2025): https://www.soundonsound.com/reviews/vienna-symphonic-library-synchron-prime-edition
- Vista (Magnus, 2021): https://www.soundonsound.com/reviews/performance-samples-vista
- CSS (Magnus, 2016): https://www.soundonsound.com/reviews/cinematic-studio-strings
- Berlin Symphonic Strings (Stewart, 2021): https://www.soundonsound.com/reviews/orchestral-tools-berlin-symphonic-strings
- Hollywood Strings (Stewart, 2010): https://www.soundonsound.com/reviews/ewql-hollywood-strings
- Wallfisch (Stewart, 2024): https://www.soundonsound.com/reviews/orchestral-tools-benjamin-wallfisch-strings-stringscapes
- Vasks (Stewart, 2024): https://www.soundonsound.com/reviews/orchestral-tools-peteris-vasks-strings
- BBC SO (Stewart, 2019): https://www.soundonsound.com/reviews/spitfire-audio-bbc-symphony-orchestra
- Dimension Strings (Stewart, 2013): https://www.soundonsound.com/reviews/vsl-vienna-dimension-strings
- SWAM String Sections (D'Silva, 2024): https://www.soundonsound.com/reviews/audio-modeling-swam-string-sections
- LA Scoring Strings (Ricard, 2010): https://www.soundonsound.com/reviews/audiobro-la-scoring-strings
- Session Strings Pro (Magnus, 2011): https://www.soundonsound.com/reviews/ni-session-strings-pro
- Sample Modeling (Magnus, 2020): https://soundonsound.com/node/4921022
- the Decca tree (Robjohns, 2020): https://www.soundonsound.com/node/4922475
- Richard King (Senior, 2015): https://www.soundonsound.com/people/richard-king-how-record-acoustic-ensembles
- AIR Lyndhurst: https://www.soundonsound.com/music-business/air-lyndhurst-london

**Others:**

- MusicRadar:
  - Hollywood Strings: https://www.musicradar.com/reviews/tech/east-west-quantum-leap-hollywood-strings-279130
  - Abbey Road Orchestra (Schmidt, 2024): https://www.musicradar.com/music-tech/plugins/spitfire-audio-abbey-road-orchestra-symphonic-strings-professional-review
- rekkerd:
  - Vista II Lite: https://rekkerd.org/performance-samples-releases-vista-ii-small-string-ensembles-lite/
  - PolySWAM: https://rekkerd.org/audio-modeling-brings-a-new-approach-to-expressive-polyphony-with-polyswam/
- Mix (Jackson, 2006), on scoring engineers: https://www.mixonline.com/?p=86318
- CineMontage (Lambert, 2013), Kurlander: https://cinemontage.org/?p=7926
- Abbey Road Institute, the Decca tree: https://abbeyroadinstitute.co.uk/blog/the-decca-tree-the-secrets-behind-the-legendary-recording-technique/amp/
- Hoffmann, R., "Alternative string seating": https://www.robin-hoffmann.com/dfsb/alternative-string-seating/
- Jacksonville Symphony: https://www.jaxsymphony.org/musical-chairs-why-orchestras-sit-the-way-they-do/
- Classic FM: https://www.classicfm.com/discover-music/orchestra-layout-explained/
- Singapore SO: https://www.sso.org.sg/bravissimo/seats-and-sounds-orchestral-seating-and-the-symphony-of-sounds
- Acoustics Today:
  - 2009, the halls: https://acousticstoday.org/issues/2009AT/Apr2009/files/basic-html/page23.html
  - 2013, Abbey Road: https://acousticstoday.org/issues/2013AT/Apr2013/files/basic-html/page12.html
- Wikipedia (low authority, cross-checked where possible): "Decca tree", "String section",
  "Free bowing", "Leopold Stokowski".
- Leech-Wilkinson, D. (2009). *The Changing Sound of Music*, CHARM, ch. 7:
  https://charm.kcl.ac.uk/studies/chapters/chap7.html

**Refused and not bypassed:**

- vi-control.net (403);
- musictech.com (403);
- the IEEE and ResearchGate pages of the WASPAA 2011 paper;
- Taylor & Francis for JNMR 2012;
- CORE for Pätynen's thesis.
