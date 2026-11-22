# Orchestral strings — verified research for round 42 (2026-11-22)

*For the owner's request (BACKLOG-excellence.md, "Strings and winds, modelled and recorded"): violin,
viola, cello and double bass, arco and pizzicato first, modelled and recorded, rendered to Chorda's
and Tympanum's standard. This file checks and completes `research/instruments-next/notes.md`
§6.7, §6.8, §7 and §8, and `docs/round41/fides.md`. It does not repeat them.*

**This is not legal advice.** Patent status comes from Google Patents, which calls every status
"an assumption, not a legal conclusion". Items marked *lawyer* need counsel before a release.

**Labels.**

- **V** means verified 2026-11-22: the primary page, the repository's file tree or the record was
  read today.
- **NV** means not verified: it comes from a search snippet, a secondary source, memory of the
  literature, or a page that refused us.

## 0. The short version

- **Patents.** The planned design reads only on expired patents:
  - a waveguide string;
  - a friction curve with bow pressure, speed and position;
  - a modal body;
  - legato as a crossfade or glide between samples, chosen by interval;
  - seeded per-player variation in an ensemble.

  The closest claims are all expired: Synful's interval-matched transitions, Yamaha's bowed
  friction family, and the ensemble-from-one-part patents of Yamaha, IBM and Line 6.

  Possibly still in force, for a lawyer to check:
  - **US 7,915,514 / 8,404,958** (Fable Sounds): CC1 stepping through dynamic layers, virtual
    key-switches.
  - **US 7,557,288** (Yamaha): a legato "joint" module built after the next note-on.
  - **US 8,729,376** (Yamaha): a loop excited by a recorded waveform with its resonance removed. It
    matters only for a hybrid voice that drives the string with a recording.
  - **US 7,872,190** (Yamaha): a reed against the lip. It matters only for winds.

  **No SWAM, Audio Modeling or Sample Modeling patent exists in the US, EP or WO** (§1).
- **Content.** VSCO 2 CE is the only CC0 orchestral **section** set (V):
  - violin, viola and cello sections, plus a solo violin and a solo double bass;
  - sustain with vibrato, spiccato, pizzicato and tremolo;
  - **two dynamic layers**, one or two round robins;
  - no legato, no harmonics, no sordino, no col legno;
  - unlooped sustains of about 10–14 s;
  - 44.1 kHz stereo, 755 MB of strings.

  **VCSL has no orchestral bowed strings at all** (V), so the backlog's "VSCO 2 CE and VCSL" is
  VSCO alone for strings. Karoryfer adds two CC0 solo instruments (V):
  - the Bigcat **cello**: four dynamics with up and down bows, staccato, pizzicato;
  - D. Smolken's **double bass**: five dynamics with up and down bows, pizzicato. It is tuned in
    fifths.

  **No CC0 solo viola exists.** For measurement and training:
  - Iowa MIS: anechoic, arco and pizz at pp, mf and ff, every string chromatic;
  - TinySOL: IRCAM, CC BY 4.0;
  - the CNSM dataset: violin bridge admittances, CC BY 4.0.
- **Recommendation (§6).** Round 42 builds:
  1. the **modelled solo voice** on Fides's bow, tuned to a first-rank gate for **cello and
     double bass** with arco and pizzicato. Violin and viola come in the same engine but behind
     the gate;
  2. the **recorded CC0 pack** (VSCO sections plus the Karoryfer cello and bass) as the ABX
     reference and the sketch voice.

  Sections, the stage and the hybrid wait for round 43.

## 1. Patents

A patents sub-agent searched Google Patents (`xhr/query`) until the site answered HTTP 503 to every
request for over 30 minutes. Justia, uspto.report, FreePatentsOnline and RPX answered 403. The proxy
was not bypassed.

Rows marked **V** were read on `patents.google.com/patent/<number>` today. For **NV** rows, an
expiry is the 20-year term worked out from the priority date, not Google's status.

### 1.1 What was found

| # | Patent | Title / assignee | Priority | Status / expiry | Claim that matters, in one sentence | Design that avoids it | Src |
|---|---|---|---|---|---|---|---|
| 1 | **US 6,316,710 B1** | Musical synthesizer capable of expressive phrasing — Eric Lindemann (Synful) | 1999-09-27 | "Expired - Fee Related", anticipated 2019-09-27 | Stores recorded segments that include note **transitions** and chooses them by the control input. A dependent claim matches the stored transition's interval to the played interval. This is interval-aware legato itself. | Free: expired | V |
| 2 | US 7,718,885 B2 | Expressive music synthesizer with control-sequence look-ahead — Lindemann | 2005-12-05 | "Expired - Fee Related" (lapsed 2014-05-18) | Delays each note and reads the next note's controls during the delay to shape the transition | Free. This is the "read ahead from the piano roll" idea, now open. | V |
| 3 | US 7,750,229 B2 | Slowly varying spectrum plus quick fluctuations — Lindemann | 2005-12-16 | "Expired - Fee Related" | Algorithmic slow part plus a fast part from stored segments | Free | V |
| 4 | US 7,271,331 B2 | Expressive portamento from the pitch wheel — Lindemann | 2006-01-30 | "Expired - Fee Related" | Pitch-wheel glide that treats the end of note 1 and the start of note 2 differently | Free | V |
| 5 | ITVA20080018A1 / ITVA20080012U1 | Resynthesis by cutting audio into wave cycles with synchronous management ("SWT") — Stefano Lucato | 2008-04-04 | Google: "unknown". Italy only, no EP/US/WO family. At most 2028-04-04. | Cuts a sample into one-period cycles and keeps phase by padding each cycle's start with silence | Do not resynthesise by pitch-synchronous cycle slicing with silence padding. Plain playback, crossfades and resampling are clear of it. ***Lawyer*: Italian (UIBM) status.** | V |
| 6 | ITGE20040115A1 | Calculation of the modal resonances / impulse response of a musical instrument — Lucato, Tommasini | 2004-12-20 | "unknown"; term ended 2024-12-20 at the latest | Modal body resonances from an analysis of the sound (no claim text on Google) | Free: past its maximum term | V |
| 7 | ITGE20040090A1 | Temporal alignment of the phase of musical sounds — Tommasini ("Harmonic Alignment") | 2004 | past its maximum term (2024) | Phase-aligns a set of samples so they crossfade cleanly | Free. We may phase-align VSCO's two layers. | V (listing) |
| 8 | US 5,157,218; 5,144,096; 5,498,836; 5,396,025; 5,448,008; 5,428,185; 5,359,146 | Yamaha (Kunimoto and others): bowed-string friction, hysteresis, bow pressure and velocity, controllers | 1989–1994 | expired by term (≤ ~2014) | A friction curve that depends on bow velocity, its static region widening with pressure, inside a delayed-feedback string loop | Free. This is the core of our bow. | NV |
| 9 | US 6,175,073 | Tone synthesis from a physical-model generator (rubbed strings) — probably Yamaha | ~1997–98 | expired by term (≤ ~2019) | Loop signal changed by bow velocity and pressure, with abrupt slip | Free | NV |
| 10 | US 5,340,942; 5,256,830; EP 0393701 | Yamaha VL wind/reed excitation of a waveguide bore | ~1989–92 | expired by term | Reed/mouthpiece excitation of a bore | Free (for the winds, later) | NV |
| 11 | US 5,781,461 (EP 0974106) | Legato with a multitap delay line and a crossfader — Stanford (Jaffe, Smith) | 1996-09-04 | expired by term (≤ 2016-09) | Two read taps on one waveguide delay, crossfaded to glide between pitches | Free. This is a ready method for the modelled legato. | NV |
| 12 | US 7,872,190 (EP 2079080) | Tone synthesis (reed against lip) — Yamaha | ~2008 | **unknown**; could run to ~2028–29 | Contact model of a reed and the lip | Winds only. ***Lawyer*** before a modelled reed. | NV |
| 13 | **US 7,557,288 B2; US 7,816,599 B2** (EP 1729283, EP 1850320) | Tone synthesis with "joint" rendition-style modules for legato — Yamaha (Umeyama, Akazawa) | JP 2005–06 | 7,816,599 "Expired - Fee Related" (snippet). **7,557,288 unknown**, perhaps to ~2027 plus term adjustment. | Builds the stored joint module (the note-to-note connection) only after the next note-on, so legato has no latency | Our recorded legato (overlap detection, then a crossfade or glide between two ordinary samples) uses no stored joint module. ***Lawyer*: status and claims.** | NV |
| 14 | **US 7,915,514 B1; US 8,404,958 B2** | Advanced MIDI and audio processing — Fable Sounds LLC (Shrem, Itzkovich) | ~2007–08 | **unknown**; could run to ~2028–29 | Several features: virtual key-switches; the mod wheel steps through dynamic layers at two thresholds and ignores velocity while it moves; layer "morphing" matched in amplitude and timbre; alternate cycles per note, articulation and dynamic | Use a continuous equal-power CC1 crossfade of the layers, with no threshold stepping and no velocity lockout. That predates the patents (EXS24, Kontakt, GPO). ***Lawyer*: the largest open risk on the recorded side.** | NV |
| 15 | US 6,946,595 B2 (CA 2436679, EP 1388844) | Performance data processing for an ensemble feel — Yamaha (Tamura, Umeyama) | JP 2002-08 | CA "expired"; US expired by term (~2023) | Each channel stores a deviation applied to note-on timing and to when the delayed vibrato starts, so one part plays as an ensemble | Free. This is our seeded per-player variation. | NV |
| 16 | US 5,541,354 (EP 0690434) | Micromanipulation of waveforms in a sampling synthesizer — IBM | ~1994 | expired by term | Copies of one sample, each with its own random time-varying change, summed into N players ("12 violins from one") | Free, and prior art | NV |
| 17 | US 6,881,891; 6,998,528 | Multi-channel processing of one instrument signal — Line 6 | 2002-07-16 | maximum term 2022-07 (plus any adjustment) | Parallel channels with random delay, pitch and gain, drawn again at each detected attack | Free once confirmed | NV |
| 18 | US 4,205,580 (Kawai); 4,353,279; 4,467,691 | Ensemble effect by detuning | 1970s–80s | expired | Equal pitches, slightly detuned, make an ensemble | Free | NV |
| 19 | **US 8,729,376 B2** | Musical sound synthesizing apparatus — Yamaha (Kakishita) | ~2011 | **unknown**; perhaps to ~2032 (notes.md §8: active, ~2032-07-10) | A delay loop excited by a recorded waveform with its resonance removed | Matters only for a hybrid that **feeds a dereverberated recording into the string loop**. A recorded attack crossfaded under a modelled sustain, with the bow exciting the string, is a different structure. ***Lawyer*** if the hybrid goes ahead. | NV |
| 20 | US 8,937,237 | Touch-screen articulation — Apple | ~2012 | not checked | Touch gestures pick the articulation | Not MIDI legato. Avoid copying a touch-strip articulation UI without a check. | NV |

### 1.2 Searched and found nothing

These are the exact searches (Google Patents `xhr/query` and the web).

- **Audio Modeling / SWAM:**
  - `inventor="Emanuele Parravicini"`: 0 results;
  - `assignee="Audio Modeling"`: 0;
  - `assignee="Sample Modeling"`: 0;
  - `inventor=Lucato&cpc=G10H`: 0;
  - `inventor=Tommasini&cpc=G10H`: 0;
  - `inventor="Stefano Lucato"`: 3, all Italian (rows 5–6);
  - `inventor="Giorgio Tommasini"`: 6, two of them on music (rows 6–7).

  The same tool found all 8 of Lindemann's patents, so the search worked. Audio Modeling's pages
  call SWAM "proprietary" and its names registered marks; that is trademark, not patent. No
  "patent pending" statement for SWAM was found. **SWAM's method is a trade secret, not a patent:
  we may build the same class of instrument, and must not use its names (SWAM, PolySWAM) in ours.**
- **Yamaha:** no patent from the 2010s or 2020s on bowed-string friction was found. Its recent
  filings found are neural synthesis (US 2021/0366454, 2021/0350783) and a bow as hardware
  (WO 2017/163469).
- **Stanford / CCRMA:**
  - US 4,984,276 could not be re-read today (503). notes.md §8 lists it as expired.
  - No Stanford patent specific to the bowed string was found. McIntyre–Schumacher–Woodhouse and
    Smith's waveguide bow are published research.
  - STK's header still says "possibly subject to patents held by Stanford University, Yamaha, and
    others" (V, `include/Bowed.h`). That is a 1990s caution; the patents it meant are past their
    terms (rows 8–11 and notes.md §8).
- **Companies with nothing found** (by web search, after Google blocked us): AAS (Applied Acoustics
  Systems), Roland (SuperNATURAL "Behavior Modeling", V-Piano, V-Bow), Native Instruments,
  Spitfire, VSL (the "VSL legato patent" turned out to be a forum joke), Garritan, EastWest and
  Orchestral Tools.
  - Roland's LA/SA synthesis (a sampled attack plus a synthesised sustain, 1986–88) would have
    expired by about 2008. Their numbers were not found (NV).
  - NI's patent-marking page was not read.
- **Modartt:** beyond US 7,915,515 (the piano, to 2027-10-20, in notes.md §8), no bowed-string
  patent was found.

### 1.3 For a lawyer, before release

- On USPTO Patent Center (maintenance fees, term adjustment), check the status and claims of:
  - US 7,557,288;
  - US 7,915,514 and US 8,404,958;
  - US 8,729,376;
  - US 7,872,190.
- The Italian status of ITVA20080018A1.
- Re-open every NV row on Google Patents once it answers again, and check the EP and JP
  counterparts.

## 2. Recorded content we may install on demand

Licence policy (ADR 0030, ADR 0042):

- CC0 is preferred;
- CC BY 3.0 and 4.0 are accepted with credit;
- NC, ND and SA are refused.

Every file tree below was read through the GitHub API (`git/trees/<branch>?recursive=1`). WAV
headers were read with 64-byte range requests. No library was downloaded.

### 2.1 VSCO 2 Community Edition — the strings, file by file (V)

Sources:

- the repository: <https://github.com/sgossner/VSCO-2-CE> (the API's `license.spdx_id` is
  `CC0-1.0`; `LICENSE` is the CC0 1.0 legal code; last push 2021-04-06);
- the vendor page: <https://versilian-studios.com/vsco-community/> ("under a Creative Commons 0
  (i.e. public domain) license … no rules, no royalties", "44.1 kHz, 16- or 24-bit").

The repository's `Readme.txt` (2016) still *asks* "that you do not sell the samples directly" and
asks for credit to Versilian Studios / Sam Gossner and Ivy Audio / Simon Dalzell. That is a request
beside a CC0 dedication, not a condition. We honour it anyway: the pack is free, and the credit is
shown.

- **Branches.** `master` holds the raw WAVs. `SFZ` holds the mappings (75 `.sfz` files). Release
  1.1.0 (2019-01-17) is the SFZ download.
- **Total.** 3 313 files, 3.24 GB, of which **strings are 767 files, 755 MB** (harp 35 MB included).

| Folder | Articulation | Pitches (sounding, from the SFZ key centres) | Dynamics | Round robins | Format |
|---|---|---|---|---|---|
| Violin Section/susVib | sustain, vibrato | 11 notes, G3–D6, about every minor third | 2 (v1, v2) | 1 | 16-bit stereo 44.1, ~12 s, **unlooped** |
| Violin Section/Spic | spiccato | 11, G3–D6 | 2 | 2 | stereo 44.1 |
| Violin Section/Pizz | pizzicato | 11, G3–D6 | 2 | 2 | stereo 44.1 |
| Violin Section/Trem | tremolo | 11, G3–D6 | 2 | 1 | stereo 44.1 |
| Viola Section/susvib, spic, pizz, trem | the same four | 12–13, C3–D6 | 2 | 1 (sus, trem) / 2 (spic, pizz) | stereo 44.1 |
| Cello Section/susvib, spic, pizzT, trem | the same four | 13–14, C2–F5 | 2 (v1/v3 on sus) | 2 (sus, spic, pizz) / 1 (trem) | sus **24-bit** stereo, ~10 s |
| Solo Violin/Arco Vib | sustain, vibrato | 15, G3–C7 | 2 (p, f) | 1 | 16-bit stereo, ~14 s |
| Solo Violin/spic, Pizz, Trem | spiccato, pizzicato, tremolo | 13–15, G3–C7 | 2 | 2 (spic, pizz) / 1 | stereo 44.1 |
| Solo Contrabass/SusVib, SusNV | sustain with **and without** vibrato | 13–14, E1–B3 | 2 (v1, v3) | 1 | 16-bit stereo, ~10 s |
| Solo Contrabass/Spic, Pizz, Trem | spiccato, pizzicato, tremolo | 8–14, E1–B3 | 2 | 2 (spic, pizz) / 1 | stereo 44.1 |

Notes:

- **File names are an octave low.** Most names use C3 = middle C, so `VlnEns_susVib_A3_v1` maps
  to key 69 (A4). The solo violin's Arco Vib and Pizz use scientific pitch. Map from the SFZ key
  centres, not from the names (V: `SViolinVib.sfz`, `ViolinEnsSusVib.sfz`, `CelloEnsSusVib.sfz`,
  `ContrabassSusVB.sfz`, `ViolaEnsSusVib.sfz`).
- **What is missing:**
  - a solo viola and a solo cello;
  - legato transitions of any kind;
  - non-vibrato sustains (except the bass);
  - harmonics, con sordino, col legno, sul ponticello, trills, portato and staccato (only
    spiccato);
  - release samples;
  - more than two dynamic layers.

  The SFZ uses no `loop_*` opcode, so sustains end after 10–14 s.
- **The SFZ.**
  - Each instrument has a key-switch file (`*-KS.sfz`: C2 sustain, C#2 tremolo, D2 spiccato, D#2
    pizzicato through `sw_last`).
  - Sustains have `ampeg_release` 0.8–4 s.
  - Velocity picks the layer; there is no CC1 crossfade.
- **Not stated anywhere read:**
  - the section sizes (how many players);
  - the room;
  - the microphones.

  For a section patch these matter (§6).

**Verdict.**

- (a) **Ship as an on-demand pack: yes**, with credit.
- For a first-rank recorded voice it is thin. Two dynamic layers is the most common complaint
  about BBC SO Discover's single layer (§4). There is no legato, and the sustains have no loops.
- Cantus must add:
  - **loops** (crossfade loops found offline, stored as metadata, never by editing the files);
  - a scripted legato;
  - a spectral or filter-based dynamics morph between the two layers, so CC1 changes the tone and
    not only the level. Use a continuous crossfade, not threshold stepping (§1, row 14).

### 2.2 VCSL — no orchestral strings (V)

Source: <https://github.com/sgossner/VCSL> (`CC0-1.0`, last push 2026-01-14; 4 550 tree entries).

A search of the whole tree for violin, viola, cello, bass, fiddle, bow, string, erhu and lute finds
only:

- *Chordophones/Zithers/Psaltery, Bowed and Plucked* (37 files);
- harps (Concert Harp, Folk Harp);
- keyboards.

**There is no violin, viola, cello or double bass.** The backlog's §3e item 9 and the owner's
request say "VSCO 2 CE and VCSL" for strings. For strings that is **VSCO alone** (plus Karoryfer,
§2.3). VCSL still serves the harp, and the winds later.

### 2.3 Karoryfer and friends (sfzinstruments on GitHub) (V)

The org listing (<https://api.github.com/orgs/sfzinstruments/repos>) gives each repository's licence.
The page <https://sfzinstruments.github.io/strings> lists the bowed strings. The commercial ones
(*Merciful Cello*, the *Vengeful* set) are not ours to ship.

| Library | Licence (V) | What it is (V, file tree and readme) | Size | Use |
|---|---|---|---|---|
| **Karoryfer x Bigcat cello** — <https://github.com/sfzinstruments/karoryfer-bigcat.cello> | CC0-1.0 (`LICENSE`, CC0 1.0). Readme: "royalty-free for all commercial and non-commercial use … open-source". Copyright 2016 Karoryfer Lecolds and bigcat instruments; played by Kamila Borowiak. | Solo cello. **Sustain**: 17 pitches (minor thirds, named C1–C5), **4 dynamics** (p, mp, mf, f), **up and down bow** (`_d`/`_g`, Polish *dół*/*góra*), 136 files, ~4.8 s each (short: needs loops). **Staccato**: 4 dynamics × 4 round robins (270 files). **Pizzicato**: 13 pitches × 4 round robins. **Noises**: harmonics on four strings, 16 knocks (*stuk*). SFZ programs with 4- and 8-voice unison, legato "first" maps and a scripted marcato (staccato attack over sustain). Mono 24-bit 44.1 kHz. | ~141 MB of samples | (a) ship; (b) the **solo-cello ABX reference** for the modelled voice |
| **D. Smolken double bass** — <https://github.com/sfzinstruments/dsmolken.double-bass> | CC0-1.0. Readme: "Royalty-free for all commercial and non-commercial use. Copyright 2013 D. Smolken." | 1958 Otto Rubner bass, Spirocore strings, **tuned in fifths (C G D A)**, not the orchestra's E A D G. **Arco**: 16 pitches × **5 dynamics** (pp–ff) × **up and down bow** (159 files, ~4 s). **Pizzicato**: 11 pitches × 3 dynamics × 4 round robins. Arco and pizz noises (creak, screech, slap, body, click…). Three SFZ files (arco, pizz, key-switched). Mono **32-bit float** 44.1 kHz. | ~290 MB | (a) ship; (b) the **solo-bass ABX reference** |
| Meatbass, Sneakybass (Karoryfer; the same Rubner bass) — <https://github.com/sfzinstruments/karoryfer.meatbass>, `…/karoryfer.sneakybass` | CC0-1.0 (Sneakybass readme: "CC-0 license") | Meatbass: arco with mod wheel, 3 or 5 velocity layers, 2 round robins, unlooped. Sneakybass: very quiet pizzicato. | 0.25 / 0.33 GB | (a) ship as extra presets |
| String Cyborgs — `…/karoryfer.string-cyborgs` | CC0-1.0 | Bowed bass and cello samples **used as synthesiser oscillators** (Zinc, Blackheart, Ironface). Not an orchestral instrument. | 62 MB repo | a synth pack, not this round |

Karoryfer's GUI banks carry a digital signature for Sforzando. We use only the samples and the SFZ
mappings, so that does not touch us.

### 2.4 Other sources checked

| Library | Licence (where verified) | Strings, articulations, layers | Size | (a) ship? | (b) measure/train? |
|---|---|---|---|---|---|
| **University of Iowa MIS** — <https://theremin.music.uiowa.edu/MIS.html> | "may be downloaded and used for any projects, without restrictions" (V). Not a CC licence. The 2012 violin page states no licence and asks for donations (V). | **Pre-2012** violin, viola, cello and bass: **arco and pizzicato at pp, mf and ff**, every string separately, chromatic (violin G3–B7). Mono 16-bit 44.1 kHz, Neumann KM 84 at 5 ft, **anechoic chamber** (V, <https://theremin.music.uiowa.edu/MISviolin.html>). **2012** violin: arco and pizz at **ff only**, stereo 24/44.1, Earthworks QTC40 (V, `MIS-Pitches-2012/MISViolin2012.html`). No vibrato or technique marks were found. Viola, cello and bass pages: not read (NV, same layout expected). | ~GB | **possible**: "without restrictions" covers redistribution in plain words. ***Lawyer*** for a pack. Credit the University of Iowa Electronic Music Studios. | **yes, the best for modelling**: anechoic means string plus body with no room — references for the body fit and the attack |
| **TinySOL** (IRCAM Studio On Line subset) — <https://zenodo.org/records/3685367> | **CC BY 4.0** (V) | violin, viola, cello and contrabass, **ordinario only**, pp–ff (metadata levels 0–4), one note per clip, 2–10 s. Mono 16-bit 44.1 kHz. 2 913 clips over 14 instruments. | 1.0 GB | yes with credit (CC BY accepted). Thin: ordinario only. | **yes**: a solo viola reference that we otherwise lack. The full SOL is commercial, so only TinySOL. |
| **Good-sounds** (MTG-UPF, Korg-funded) — Zenodo <https://zenodo.org/record/4588740> | **CC BY-NC 4.0** on Zenodo (search result, NV). A 2025 CORA copy (doi:10.34810/DATA2314) is said to be CC BY 4.0: **not verified** (the page denied us). | violin and cello notes and scales, "good" and "bad" sounds, 15 players | 13.9 GB | **no** (NC), unless the CC BY copy is confirmed | only if the CC BY copy is confirmed |
| **Virtual Playing Orchestra** — <http://virtualplaying.com/virtual-playing-orchestra/> | Mixed (V): VPO itself "no restrictions on using this library to make music", but "I do not feel it is right to repackage and sell". Sources: **Sonatina (CC Sampling Plus 1.0, retired)**, **No Budget Orchestra (CC BY-SA 4.0)**, **Philharmonia (no licence text)**, VSCO 2 CE (CC0), Iowa. | full orchestra | 618 MB (v3.2, NV) | **no** (SA, NC-like and unknown parts) | no |
| **No Budget Orchestra** (Mattias Westlund) | CC BY-SA 4.0 (V, through VPO's page) | — | — | **no** (SA) | no |
| Sonatina (and `peastman/sso`) | CC Sampling Plus 1.0 (notes.md §7) | — | — | **no** | no |
| Freesound | per file. The only CC0 string sounds found were VSCO's own uploads. Carlos Vaquero's UPF cello packs (e.g. *Violoncello: Single notes Tenuto Non Vibrato D3–C#4*, pack 9524, Zoom H2n XY, mf, 442 Hz) show **no licence without a login** (V: page read, licence NV). | — | — | only file by file, CC0 or CC BY, after vetting | the same |
| Pianobook, Decent Samples free strings, Spitfire LABS / Splice INSTRUMENT, BBC SO Discover, Spitfire Symphony Orchestra Discover | use allowed in music, **redistribution forbidden** (notes.md §7) | — | — | **link only** | no |
| **CNSM dataset** (Pauget Ballesteros, Fritz et al.) — <https://zenodo.org/records/18696785> | **CC BY 4.0** (V) | one violin played daily for six months by a soloist, plus two control violins: laser-vibrometer **input admittance**, standard recordings, the soloist's **bowing by motion capture** (V, <https://arxiv.org/abs/2605.18305>, 2026-05-18). Contents of the zip not listed on the record. | 654 MB | n/a | **yes**: violin admittance for the body fit, and real bowing gestures |

### 2.5 What the content allows

- **Sections:**
  - VSCO gives the violin, viola and cello sections and a solo bass. **There is no CC0 bass
    section**, so it would be built from the solo bass by our seeded ensemble.
  - The sections have two layers and no legato.
  - Honest level: the "finished sketch" of backlog §3e item 9, not BBC SO Core.
- **Solo instruments:**
  - The Karoryfer cello (4 dynamics, both bow directions) and the Smolken bass (5 dynamics, both
    directions) are the best CC0 solo recordings found. They are the natural **ABX references**
    for a modelled cello and bass.
  - The violin has only VSCO's solo (2 dynamics, vibrato only), Iowa (anechoic, pp/mf/ff) and
    TinySOL.
  - The viola has **no CC0 solo at all**: TinySOL (CC BY) and Iowa only.
- **The gap is Cantus's own recordings.** That means a CC0 quartet session with sustain and
  non-vibrato at 4–5 dynamics, up and down bows, true legato intervals, harmonics, sordino and col
  legno, recorded close and in a room. Until it exists, the modelled voice is the only way to
  first-rank legato and dynamics.

## 3. Published measurements to model against

Each DOI was resolved through the Crossref API, and abstracts were read through OpenAlex
(`api.openalex.org/works/doi:…`) on 2026-11-22. **V** means the title, the venue and, where quoted,
the abstract were seen. Numbers not in an abstract are marked NV until the paper is read.

### 3.1 Bodies: bridge admittance and radiation

| Source | What it gives | Access / licence | Src |
|---|---|---|---|
| Woodhouse, "The acoustics of the violin: a review", *Rep. Prog. Phys.* 77, 115901 (2014), doi:10.1088/0034-4885/77/11/115901 | The reference review of the bowed string, body vibration, radiation and playability | open PDF at iopscience (OpenAlex `oa_url`); licence of the PDF not read | V |
| Bissinger, "Structural acoustics of good and bad violins", *JASA* 124, 1764–1773 (2008), doi:10.1121/1.2956478 | 17 violins: **the same five "signature" modes below 600 Hz** in all. The only robust quality marker is the **A0 cavity mode (~280 Hz) radiativity**, higher in excellent violins. Damping, mobility, radiativity and directivity up to 4 kHz. | paper, paywalled. Bissinger's VIOCADEAS data are not public (none found). | V (abstract) |
| Gough, "A violin shell model: Vibrational modes and acoustics", *JASA* 137, 1210–1225 (2015), doi:10.1121/1.4913458; Gough, "The violin: Chladni patterns, plates, shells and sounds", *EPJ ST* 145, 77–101 (2007), doi:10.1140/epjst/e2007-00149-0 | Mode shapes and frequencies from a shell model; how A0, CBR, B1− and B1+ arise | paywalled | V (titles) |
| Zhang, Woodhouse, Stoppani, "Motion of the cello bridge", *JASA* 140, 2636–2645 (2016), doi:10.1121/1.4964609 | **3 cellos, bridge motion up to 2 kHz.** A rigid-body rotation holds to at least 1 kHz. The instantaneous centre moves from the sound-post side to the bass-bar side as frequency rises. Transfer functions at the **four string notches**, so each string sees a different body. | open copy at doi:10.17863/cam.7954 (Cambridge repository) | V (abstract) |
| Maestre, Scavone, Smith, "Joint modeling of bridge admittance and body radiativity for efficient synthesis of string instrument sound by digital waveguides", *IEEE/ACM TASLP* 25, 1128–1139 (2017), doi:10.1109/taslp.2017.2689241; WASPAA 2015, doi:10.1109/waspaa.2015.7336951 | **The method for our body.** A modal fit of a measured admittance by parallel recursive filters, made passive by semidefinite programming. **One bank of resonant filters shared** by the bridge reflectance (2-D, both polarisations), string–string coupling and any number of radiation outputs (microphones). | paywalled | V (abstract) |
| Pérez Carrillo, Bonada, Pätynen, Välimäki, "Method for measuring violin sound radiation based on bowed glissandi and its application to sound synthesis", *JASA* 130, 1020–1029 (2011), doi:10.1121/1.3605291 | Measure a body's radiation from a played glissando with a bridge pickup, without a hammer. **We could measure a real violin, viola, cello and bass the same way** for Cantus's own CC0 bodies. | paywalled | V (title) |
| **CNSM dataset**, Zenodo 18696785 (2026), with arXiv 2605.18305 (Pauget Ballesteros, Lalitte, Lostanlen, **Fritz**) | Laser-vibrometer input admittance of 3 violins, recordings and mocap bowing | **CC BY 4.0** | V |
| Malvermi et al., "Feature-based representation for violin bridge admittances", arXiv 2103.14895 (2021) | Hammer tests on Cremonese violins; the plotted data are in the ancillary files | **CC BY 4.0** (arXiv) | V |
| "Bridge admittance measurements of 10 preference-rated violins" (Acoustics 2012 Nantes), hal-00810717 | admittances of 10 violins with players' ratings | HAL page refused (bot wall) | NV |
| Fritz, Cross, Moore, Woodhouse, "Perceptual thresholds for detecting modifications applied to the acoustical properties of a violin", *JASA* 122, 3640–3650 (2007), doi:10.1121/1.2799475; Fritz et al., *JASA* 131, 783–794 (2012), doi:10.1121/1.3651790 | **How large a change in body modes listeners hear.** These are the tolerances of our body fit. | paywalled | V (titles) |
| Jansson, *Acoustics for violin and guitar makers* (KTH, online book) | the bridge hill (~2–3 kHz) and body modes, for makers | free online; URL not re-checked | NV |
| Askenfelt, double-bass eigenmodes (STL-QPSR, 1982) | bass body modes | KTH archive, not checked | NV |

**Gap.**

- No public measured admittance was found for the **viola** or the **double bass**. For the cello,
  only the paper's figures (Zhang 2016) were found.
- Fides's cello body comes from cited low modes (`docs/round41/fides.md`).
- A first-rank body needs measured data: either a measurement campaign on instruments we can
  borrow (§6 risk 4), or the CNSM violin data scaled.

### 3.2 Strings: tension, mass, stiffness, losses

- **Manufacturers' tables.**
  - D'Addario's tension tables are an image on its silk-colour chart page
    (<https://www.daddario.com/pages/orchestral-strings-silk-color-chart>, V, the numbers unreadable
    as text). D'Addario states tension in kgf (NV).
  - Pirastro's figures were found only on retailers: Evah Pirazzi violin E 7.8–8.5 kg, A
    5.3–5.6 kg, D ~4.8 kg, G 4.6–4.9 kg (NV, they disagree).
  - **Use these only as a range check**, not as a source.
- **Measured tables.**
  - Desvages & Bilbao's model reads "a table of measured string parameters on violins, violas and
    cellos (from Percival [26])" (V, DAFx-15 paper text, §4.1). That is G. Percival's Glasgow
    thesis (2013), whose *Vivi* code is GPL. The **numbers are facts we may use**, but no GPL code.
    The table itself was not read (NV).
  - Pickering's string measurements (*J. Catgut Acoust. Soc.*, 1985–1989; with Kerr,
    doi:10.1121/1.422101, an abstract only) (NV for the tables).
- **Inharmonicity tolerance.** Schelleng 1973 (doi:10.1121/1.1913322, V abstract):
  > inharmonicity of 0.1 cent per square of mode number will not perceptibly degrade bowed-string
  > performance

  This bounds Fides's stiffness for bowed strings.
- **Losses.** Mansour, Woodhouse & Scavone 2016, "Enhanced wave-based modelling of musical
  strings":
  - Part 1, plucked: doi:10.3813/aaa.919021;
  - Part 2, bowed: doi:10.3813/aaa.919026, open copy doi:10.17863/cam.6398.

  Part 2 has realistic damping, body modes coupled to **both polarisations**, and bow-hair and
  bow-stick coupling. Its abstract: "the biggest influence on behaviour is found to come from the
  choice of model to describe the friction force at the bow" (V).

### 3.3 Bow–string friction

| Model | Source | What it says | Src |
|---|---|---|---|
| Hyperbolic / exponential friction curve (friction a function of sliding speed) | McIntyre, Schumacher, Woodhouse, "On the oscillations of musical instruments", *JASA* 74, 1325–1345 (1983), doi:10.1121/1.390157; McIntyre & Woodhouse, *Acustica* 43, 93–108 (1979) | The classic. STK and faust use a table form of it. | V (DOI) / V (DAFx ref) |
| Exponential fit to measured rosin | Desvages & Bilbao (DAFx-15; *Appl. Sci.* 6, 135 (2016), doi:10.3390/app6050135, **CC BY**) | μ(v) = sign(v)·(0.4·e^(−\|v\|/0.01) + 0.45·e^(−\|v\|/0.1) + 0.35), which gives μs = 1.2 and μd → 0.35. A drop-in curve with published numbers. Its finite-difference scheme in two polarisations is energy-balanced, runs at 44.1 kHz, and has fingers and fingerboard as contacts. | V (formula read in the DAFx-15 PDF; abstract and licence from OpenAlex) |
| **Thermal (plastic) friction**: friction depends on the contact temperature | J. H. Smith & Woodhouse, "The tribology of rosin", *J. Mech. Phys. Solids* 48, 1633–1681 (2000), doi:10.1016/S0022-5096(99)00067-8; Woodhouse, "Bowed string simulation using a thermal friction model", *Acta Acust.* 89, 355–368 (2003) | Rosin softens at the contact, giving hysteresis that a speed-only curve cannot | V (DOI) / NV (2003 pages) |
| **Which model works best** | Galluzzo, Woodhouse, Mansour, "Assessing friction laws for simulating bowed-string motion", *Acta Acust.* 103, 1080–1099 (2017), doi:10.3813/aaa.919136 (open copy at the Cambridge repository) | Against bowing-machine transients, "the friction model based on contact temperature performed significantly better than more traditional ones based on instantaneous sliding speed". Some details are captured by no model. | V (abstract) |
| Playability | Serafin, Smith, Woodhouse, *Proc. IEEE WASPAA* 1999, pp. 87–90, doi:10.1109/aspaa.1999.810856 | Dropping torsion waves "does not degrade playability". The thermal ("plastic") model "is more playable". | V (abstract) |
| Torsion | "Further empirical data for torsion on bowed strings", *PLOS ONE* (2019), doi:10.1371/journal.pone.0211217 | Measured on a cello string: every release is preceded by a reverse torsional motion. **CC BY.** | V (abstract, licence); author NV |

**Design consequence.**

- Start with the published exponential curve (Desvages & Bilbao): closed form, cheap, and the
  numbers are in a CC BY paper.
- Put a **thermal friction option** behind the same interface, judged by the attack tests below,
  because the best evidence (Galluzzo 2017; Serafin 1999) says it is both closer to measurements
  and more playable.
- Torsion can be left out (Serafin 1999).

### 3.4 Schelleng diagram, Helmholtz motion, attacks

- **Schelleng 1973** (V abstract). Maximum and minimum bow force for Helmholtz motion as functions
  of the bow–bridge distance β. The tolerance between them is "typically in the ratio of 1 to 10".
- **Schoonderwaldt, Guettler, Askenfelt 2008**, "An empirical investigation of bow-force limits in
  the Schelleng diagram", *Acta Acust.* 94, 604–622, doi:10.3813/aaa.918070 (V abstract). A bowing
  machine on violin D and E strings: 11 values of β × 24 forces × 4 speeds (5, 10, 15 and 20 cm/s).
  - The **upper limit matches Schelleng's equation in modified form** (taking the hyperbolic curve
    into account).
  - The **lower limit is independent of bow speed**, contrary to Schelleng. It is set by ripple and
    corner rounding.

  **These are the regions our model must reproduce** (a "Schelleng test" over β × force at fixed
  speed).
- **Galluzzo & Woodhouse 2014**, "High-performance bowing machine tests of bowed-string transients",
  *Acta Acust.* 100, 139–153, doi:10.3813/AAA.918694 (V abstract). A **Schelleng diagram and a set
  of Guettler diagrams for the open D string of a cello**: measured targets for our cello.
- **Guettler & Askenfelt 1997**, *JASA* 101, 2903–2913, doi:10.1121/1.418520 (V abstract). The
  listening limit for the pre-Helmholtz transient:
  - **50 ms** (about 10 periods, violin open G) for attacks with prolonged periods;
  - **90 ms** (about 18 periods) for multiple-flyback attacks;
  - 20 string players judged it.

  **The attack gate**: a modelled note's pre-Helmholtz time must sit under these limits at every
  default velocity.
- **Guettler 2002**, "On the creation of the Helmholtz motion in bowed strings", *Acta Acust.* 88,
  970–985 (NV: the Crossref lookup failed). The "Guettler diagram" of bow force against bow
  acceleration, with the region of a perfect attack. **A keyboard-only user's velocity should map
  to a force–acceleration path inside it.**
- **Woodhouse & Galluzzo 2004**, "The bowed string as we know it today", *Acta Acust.* 90, 579–589
  (NV). The review of Helmholtz motion.
- **Askenfelt 1989**, "Measurement of the bowing parameters in violin playing. II: Bow–bridge
  distance, dynamic range, and limits of bow force", *JASA* 86, 503–516, doi:10.1121/1.398230
  (V abstract). Players' real bow position, speed, β and force in sustained notes, crescendo,
  sforzando and saltellato. **This is the source for the default bowing contours.**

### 3.5 Vibrato

| Source | Measured | Src |
|---|---|---|
| Mellody & Wakefield, *JASA* 107, 598–611 (2000), doi:10.1121/1.428326 | violin vibrato: **5.9 Hz mean, ±15.2 cents mean excursion**, one sinusoid. Each partial's **amplitude** varies at multiples of the rate. **Removing the FM changed little perceptually; removing the AM changed the sound markedly.** | V (abstract) |
| Fletcher & Sanders, *JASA* 41, 1534–1544 (1967), doi:10.1121/1.1910516 | about 6 Hz. All harmonics move by the same cents. Harmonic **intensities vary at the vibrato rate, differently per harmonic and in opposite phase**. Open strings ring in sympathy at the rate or twice it. | V (abstract) |
| Geringer & Allen, *JRME* 52, 167–178 (2004), doi:10.2307/3345438 | 40 violinists and cellists: **mean rate ~5.5 Hz**, no difference between instruments. **Violin width larger than cello.** Oscillation goes above and below the pitch. | V (abstract) |
| MacLeod, *JRME* 56, 43–54 (2008), doi:10.1177/0022429408323070 | violin and viola: high register **+0.32 Hz faster and ~26 cents wider** than low; **forte ~4 cents wider** than piano; violinists slightly faster and wider than violists | V (abstract; open PDF at libres.uncg.edu) |
| Geringer, MacLeod, Allen, *JRME* 57, 351–363 (2009), doi:10.1177/0022429409350510 | the perceived pitch of vibrato tones | V (title) |

**Design consequence.**

- Vibrato must be a **frequency modulation of the string that passes through the body**. The AM
  that listeners hear comes from the partials sweeping across body resonances. Fides's body
  already does this when the string's pitch moves.
- Do not use a pitch LFO after the body, or an amplitude LFO.
- Defaults: rate 5.5–6 Hz, widening with register and dynamics as MacLeod measured, a delayed
  onset, and seeded per-player variation in a section.

### 3.6 Bowing gestures as data

- **Maestre et al.**, "Statistical modeling of bowing control applied to violin sound synthesis",
  *IEEE TASL* 18, 855–871 (2010), doi:10.1109/TASL.2010.2040783 (V abstract; open at
  hdl.handle.net/10230/12400).
  - Bow velocity, force and β per note are modelled as **sequences of cubic Bézier segments**, with
    note classes by articulation, dynamics and context and a Gaussian-mixture model.
  - **The method is published, but the database is not**: no release was found (NV, search).
- **QUARTET dataset** (Maestre, Papiotis, Marchini et al., 2017): 96 quartet recordings with mocap
  and bowing signals. **Licence not found** (NV).
  - The related **EEP dataset** is "free of charge for internal non-commercial use only" (search
    result, NV). **Refused** unless QUARTET's own licence says otherwise.
- **CNSM** (CC BY 4.0) has one soloist's bowing by mocap (§2.4). **It is the only open bowing data
  found.**

### 3.7 Sections: timing and pitch spread

- **Timing.**
  - Wing, Endo, Bradbury, Vorberg, "Optimal feedback correction in string quartet
    synchronization", *J. R. Soc. Interface* 11, 20131125 (2014), doi:10.1098/rsif.2013.1125,
    **CC BY** (V abstract). Two professional quartets; players correct onset asynchronies with
    near-optimal first-order phase correction. The asynchrony figures were not read (the full
    text refused us, NV).
  - Rasch, "Synchronization in performed ensemble music", *Acustica* 43, 121–131 (1979): a classic
    figure of **~30–50 ms rms asynchrony** in small ensembles (NV). Rasch 2001 is a book chapter,
    doi:10.1093/acprof:oso/9780198508465.003.0004 (V title).
- **Pitch spread.**
  - **No measurement of pitch spread in a string section was found** (one search).
  - The closest analogue is choral: Ternström, "Perceptual evaluations of voice scatter in unison
    choir sounds", *J. Voice* 7, 129–135 (1993), doi:10.1016/s0892-1997(05)80342-x (V title).
    Unison choir dispersion is reported at about 20–30 cents (search summary, NV).
- **Design consequence.** Seeded per-player tuning, timing and vibrato must be **measured on our
  own sections**: VSCO's section samples against its solo violin, or our recordings. The "chorus"
  of a real section is also each player's different instrument (body) and bowing, not only
  detuning (IBM US 5,541,354, row 16, expired, made the same point).

## 4. What the best products do (2025–2026)

From a products sub-agent's web research. Spitfire's and Native Instruments' sites answered 429,
MusicTech failed TLS, and VI-Control answered 403: those figures are NV. Nothing was bypassed.

### 4.1 The products

| Product | Facts (V unless marked) | Praise / complaints | Sources |
|---|---|---|---|
| **SWAM Solo Strings 3** (violin, viola, cello, bass; modelled) | v3.0 2021-03-31; **v3.12.4, 2026-10-08**; 3.12.0 (2026-04-28) added performance work and rates above 48 kHz. Bundle €/$339 list. Dynamics are **one continuous Expression** (CC11 by default): **without an expression controller it stays silent**. With a keyboard the vendor suggests CC1 for Expression and aftertouch for vibrato. Legato retriggers the held note when an overlapping note is released. RAM ~30 MB (Mac) / 52 MB (Win) per instance; disk 36–180 MB per instrument. | SOS 2019: "some of the most playable instruments I've come across". Complaints: col legno "weedy", pizzicato "modest", the bass stops at E1. KVR: "takes a lot of work", "stiff", bow control weak. | kb.audiomodeling.com (release notes, requirements, expression warning); soundonsound.com/node/4920417; kvraudio.com/forum/viewtopic.php?p=9222907 |
| **SWAM String Sections** | SOS May 2024. 4–6 modelled players a section; **divisi anti-phasing**; ~175 MB RAM a section; one 2.5 GHz 4-core CPU per instance | **High CPU with several instances**; unwanted portamento; shrill high violins; subtle sordino; no col legno | soundonsound.com/reviews/audio-modeling-swam-string-sections |
| **PolySWAM** | **Released 2026-10-05**, $399. 37 SWAM engines in one instrument. Each note of a chord goes to its own player by register (**automatic divisi**). **No key-switches**: legato, staccato and portamento come from how you play. Pizz, tremolo and sordino on buttons. 162–390 MB RAM. | no independent review yet | audiomodeling.com/polyswam; sonicstate.com/news/2026/08/19/… |
| **Spitfire BBC SO Discover** (free) | 33 instruments, 47 techniques, ~200 MB, one mix; strings long, short, pizz and tremolo; **no legato, one dynamic layer, no round robins** | "way better than a 200 MB footprint suggests"; the complaint is the single layer | soundonsound.com/reviews/spitfire-audio-bbc-symphony-orchestra-discover |
| **Spitfire Symphony Orchestra Discover** (free, **2025-11-11**) | 44 instruments, 73–74 techniques, **3 legato patches**, 11 solos, 5.68 GB, Kontakt Player, AIR Lyndhurst | bonedo 5/5 (NV) | soundonsound.com/news/spitfire-audio-launch-spitfire-symphony-orchestra-discover; bedroomproducersblog.com/2025/11/11/… |
| **BBC SO Core** | sections 16/14/12/10/8; legato, portamento and slurred legato plus the full set of techniques; one mic mix; 28.2 GB; $449 (NV) | quiet overlapped legato slides into portamento; GUI hard to read | admin-new.spitfireaudio.com/bbc-symphony-orchestra-core; SOS review |
| **Logic Studio Strings** | since Logic 10.4 (NV); articulations by pop-up or key-switch; "Legato Transitions"; dynamics by controller (NV) | no review found | support.apple.com (NV) |
| **NI** | Session Strings Pro 2: 26 articulations, phrase animators, 38 GB (NV). Newest: **LCO Producer Strings** (2025-09-29, $99, textures) | — | soundonsound.com/news/lco-producer-strings-native-instruments |
| **Ableton** | *String Quartet by Spitfire*: 9 techniques plus **9 MPE presets** (technique per note), 972 MB. *Orchestral Strings* (SONiVOX): Simpler-based, no legato. | — | ableton.com/en/packs/string-quartet/; ableton.com/en/packs/orchestral-strings/ |
| **Bitwig** | *Orchestral Strings* (Orchestral Tools, Teldex): 12 sampler presets, 2.02 GB, no legato | — | bitwig.com/sound-content/orchestral-strings-113/ |
| **FL Studio** | new FLEX (2026-07-09) with "Strings" packs: presets, **no articulations, legato or expression** | — | image-line.com/news/all-new-flex |
| **Cinematic Studio Strings** 1.7 | legato modes "Low Latency" and "Expressive"; **up to ~¼ s delay**; the classic 100/250/333 ms by velocity (NV); CC1 dynamics, CC2 vibrato | "exceptional"; the complaint is **the legato latency** that users must shift by hand | cinematicstudioseries.com/strings/; SOS review |
| **VSL Synchron Strings Pro** | polyphonic legato by default; auto-speed; CC1 or velocity for dynamics; **CC20 crossfades vibrato types**; 13 mics | "a dream to play"; storage and RAM | vsl.co.at/instruments/synchron/strings-pro; SOS review |
| **Sample Modeling Solo & Ensemble Strings** (sampled plus modelled) | anechoic samples with "harmonic alignment", body impulse responses, **continuously variable ensemble size**; CC11 dynamics, CC1 vibrato | violin "exquisite"; CC mapping unreliable; open strings take vibrato | soundonsound.com/node/4921022 |
| **Spitfire Abbey Road One** | 91 players, **5 dynamic layers, 5 round robins**, no-vibrato-to-vibrato crossfade, **no legato** | "the best in 20 years" | SOS review |
| **ProjectSAM Lineage Strings** (2026-06) | 35 players; "Adaptive Phrasing" detects the articulation from playing (NV) | no review yet | rekkerd.org |

### 4.2 The bar Cantus must meet

1. **Legato latency.**
   - Sampled legato costs 70–333 ms (CSS) and users fix it by hand.
   - The modelled voice must add **0 ms**.
   - Because Cantus *is* the DAW, the recorded voice can read the next note ahead on playback and
     start the transition early. That is Lindemann's expired look-ahead (§1, row 2).
   - Live, a fast mode of ≤ 100 ms with a constant, published delay.
2. **Legato by playing, not by key-switch** (PolySWAM, Lineage, VSL auto-speed).
   - Speed comes from velocity and overlap.
   - **Portamento only when asked**: accidental slides are the commonest complaint.
   - Polyphonic legato for divisi; a re-bow on a repeated note; the held note returns when an
     overlapping note is released.
3. **Dynamics change the tone.** Three to five layers at least, or the model's continuous
   dynamics. CC1 with **velocity alone also working**.
4. **Vibrato as its own control** (CC2/CC20/aftertouch), with an automatic, delayed vibrato for a
   keyboard-only player.
5. **Sound from a keyboard alone, at once.** SWAM's silence without an expression controller is
   its best-known barrier. Accept CC1, CC11, CC2 (breath), aftertouch and MPE as well.
6. **Real sections.**
   - Distinct players, not copies.
   - Automatic divisi (PolySWAM).
   - Anti-phasing.
   - Seats in a room.
7. **Good secondary techniques.** Reviewers single out weak col legno, weak pizzicato (SWAM),
   subtle sordino and shrill high violins.
8. **Cost.**
   - The modelled solo voice should take ≤ ~50 MB and well under 1 % of a core.
   - Sections must not hit "high CPU with several instances" (SWAM Sections' main complaint).
9. **The free floor.** SSO Discover (Nov 2025) is free, with 3 legatos and 11 solos. The DAWs' stock
   strings (FL, Ableton, Bitwig) have no legato and no expression; Logic is the exception (NV).
10. **Few controls in front:** dynamics, vibrato, legato speed, section size, room. The rest
    behind.
11. **A render equals live playback to the bit, look-ahead included.** No product above promises
    it.

## 5. Open references and their licences

| Reference | Licence (V) | What is in it for us | May it enter `cantus-dsp` (MPL-2.0)? |
|---|---|---|---|
| **STK `Bowed`** — <https://github.com/thestk/stk> (`include/Bowed.h`, `src/Bowed.cpp`, `include/BowTable.h`) | **MIT-style**: "Permission is hereby granted, free of charge …", plus a *non-binding* request to send modifications back (`LICENSE`, © 1995–2023 Cook and Scavone) | A waveguide bow "à la Smith (1986), after McIntyre, Schumacher, Woodhouse (1983)": neck and bridge delays split at β = 0.127; `BowTable` = (\|(x+offset)·slope\| + 0.75)^−4, clipped to [0.01, 0.98], slope 3; a one-pole string loss; vibrato 6.127 Hz. **A six-biquad violin body "provided by Esteban Maestre" (2011)**, its coefficients fixed for one sample rate (not stated in the file). | **Yes, it may be ported** with the notice kept in THIRD-PARTY-NOTICES. But it is a toy against the bar: no Schelleng/Guettler behaviour by design, a fixed-rate body. Use it as a test oracle, not as the design. |
| **faust `physmodels.lib`** — <https://github.com/grame-cncm/faustlibraries> | **LGPL** with GRAME's exception for *code generated by the Faust compiler* ("you are free to choose … any other license"). The `.lib` source itself is LGPL. | `bowTable(offset, slope) = pow(abs(x)+0.75, −4) : min(1)`, `violinBowTable(bowPressure)`, `bowInteraction`, `violinModel(stringLength, bowPressure, bowVelocity, bowPosition)` | **Read only** (house rule). A hand port of the `.lib` text is a derivative of LGPL source, which the exception does not obviously cover. *Lawyer* if ever wanted. The same maths is in STK (MIT) and in the papers anyway. |
| **Mutable Instruments Elements** — <https://github.com/pichenettes/eurorack> (`elements/dsp/resonator.cc`, `voice.cc`) | **MIT** (file headers, V) | A **bowed modal resonator**: up to `kMaxBowedModes` banded waveguides (Essl & Cook), each a delay plus a band-pass, summed and passed through `BowTable` with the bow strength. The bow "exciter" is a `FLOW` noise model. | Yes (MIT). It fits a bowed bar, glass or plate (Fides has these), **not an orchestral string**. |
| **J. O. Smith, *Physical Audio Signal Processing*** — <https://ccrma.stanford.edu/~jos/pasp/Bowed_Strings.html> | © Julius O. Smith III (W3K, 2010; page dated 2026-08-21). No open licence stated. | The bow as a **nonlinear two-port** between two velocity-wave delay lines; child pages: *Digital Waveguide Bowed-String*, *The Bow-String Scattering Junction*, *Bowed String Synthesis Extensions*, *Linear Commuted Bowed Strings* (bowing as periodic plucking; stochastic excitation) | Algorithms, not text: implement from the description. Commuted-synthesis patents are expired (notes.md §8). |
| **Serafin's thesis**, *The sound of friction: real-time models, playability and musical applications* (Stanford, 2004; defended 2003 per Springer) | not found online today (NV). The NIME 2004 paper "Toward a generalized friction controller" is open at <https://nime.org/proc/nime2004_serafin/> (search, NV). | Real-time friction models (hyperbolic, plastic, elasto-plastic), playability maps; glass harmonica and saw | Algorithms from the papers |
| **Desvages & Bilbao 2016**, *Appl. Sci.* 6, 135, doi:10.3390/app6050135 — **CC BY** (OpenAlex). Also DAFx-15. Thesis: Edinburgh 2018, hdl.handle.net/1842/31273 (search, NV). | CC BY (the article) | A **2-polarisation finite-difference** bowed string with nonlinear contact for the bow hair, fingers and fingerboard, an energy-balanced scheme, and the exponential friction curve of §3.3. Gesture-driven. | The article is CC BY (cite it). Its NESS code is not open (not found). **Too costly for 16-player sections** (two nonlinear vector solves a sample), but **the reference for an offline "ground truth"** the waveguide is measured against. |
| **Maestre et al. 2010** (bowing contours, Bézier + GMM) | paper only; **database not public** (NV) | the method for **bowing-gesture synthesis from the score**: how a keyboard note becomes force, speed and β contours | Method only |
| **Percival's *Vivi*** (Glasgow thesis 2013) | code **GPL** (NV) | string tables (facts) and a violin model with ML-trained bowing | **No code.** Numbers only, cited. |

**Not found:** any other MIT or BSD bowed-string implementation of quality, beyond STK and Elements.
Searched: GitHub org listings of the libraries above, plus the web. A wider code search was not
run.

## 6. Recommendation for round 42

### 6.1 Which voice first

**The modelled voice first, with the recorded pack beside it as its reference.**

- **Why the model leads.**
  - The recorded CC0 content cannot reach first rank. VSCO has two dynamic layers, no legato and
    unlooped sustains, and there is no solo viola or solo cello in it (§2). Products with one or
    two layers are exactly what reviewers mark down (§4).
  - The model is where Cantus can **lead** the market:
    - 0 ms legato;
    - continuous dynamics from velocity alone, so a keyboard-only player is never silent (SWAM's
      barrier);
    - MB-scale memory;
    - pool-split voices;
    - render = live to the bit.
  - Fides already has the friction bow and a cello body (round 41), so the risk starts lower.
  - Every relevant patent found is expired (§1).
- **Why the recorded pack comes in the same round.**
  - It is small work: Exemplar already plays SFZ-like zones; the pack is presets, key-switches,
    loops and CC1.
  - It gives users strings at once, sections included.
  - It gives the **ABX references** the done-when needs (the Karoryfer cello, the Smolken bass,
    VSCO).
- **The hybrid waits for round 43 or later.**
  - It needs both voices finished.
  - The seam between a recorded attack and a modelled sustain is hard to hide (phase, spectrum,
    noise floor).
  - Its only live patent neighbour (US 8,729,376) needs a lawyer if the attack is fed into the
    string loop.

### 6.2 Scope that one builder-round can bring to first rank

**Builder A — `Arcus`** (working name). A modelled bowed string as its own instrument, sharing
Fides's DSP (`fides` bow and waveguide). Fides stays the general resonator.

1. **The cello and the double bass to the gate below**, arco and pizzicato. Why these two first:
   - Their CC0 solo references are the best found (4 and 5 dynamics, both bow directions).
   - Fides's cello body exists.
   - There are measured cello targets: Galluzzo 2014's Schelleng and Guettler diagrams for the
     cello's open D; Zhang 2016's bridge motion.
   - The low register forgives the body's upper modes less harshly than the violin's 2–4 kHz
     bridge hill.
2. **Violin and viola in the same engine** (string tables, bodies scaled, the CNSM violin
   admittance for the violin's body fit), **shipped only when they pass the same gate**.
   Otherwise they stay outside `InstrumentKind::ALL` with the reason written down. The owner's
   no-leftovers rule means saying so plainly at the start, not trimming at the end.
3. **Techniques this round:**
   - arco sustain, legato and portamento (portamento only when asked, by a note property or CC);
   - détaché with re-bow and bow direction (up/down as a note property, alternating
     automatically);
   - pizzicato (Fides's pluck on the same string and body);
   - tremolo (fast bow reversals);
   - automatic delayed vibrato with its own control.

   Harmonics, sordino, sul pont/tasto and col legno are the next pass.
4. **Playing:**
   - velocity → bow force and acceleration chosen to fall inside the Guettler region;
   - CC1 / CC11 / aftertouch / breath / MPE pressure → continuous dynamics, as bow force and
     speed along Askenfelt's measured relation;
   - CC2 or CC21 → vibrato;
   - MPE slide → contact point.

   There is no silent state: velocity alone plays.

**Builder B — the recorded CC0 pack.** Exemplar presets over:

- VSCO's sections (violins, violas and celli; sustain, spiccato, pizzicato and tremolo);
- VSCO's solo violin and bass;
- the Karoryfer cello and the Smolken bass;
- Meatbass and Sneakybass as extras.

The work:

- **offline loop finding**, stored as metadata;
- articulation by key-switch **and** note property;
- **continuous** CC1 crossfade with a tone filter tied to the dynamic;
- release envelopes;
- a scripted legato: overlap window, interval-aware crossfade and glide, and **playback
  look-ahead from the piano roll**;
- credits shown in the pack card;
- every pack on demand.

**Left for round 43:**

- sections from seeded modelled players: tuning, timing, vibrato and **body** per player, divisi
  by voice allocation, anti-phasing;
- the stage and microphones (Decca tree, outriggers, room);
- violin and viola if they missed the gate;
- harmonics, sordino and col legno;
- the hybrid.

### 6.3 The gate (measured before anyone listens, then the owner's ear)

1. **Schelleng test.** Over a grid of β × force at 10 and 20 cm/s, Helmholtz motion holds inside
   Schelleng's modified upper bound and above a speed-independent lower bound. The shape should
   match Schoonderwaldt 2008 and Galluzzo 2014 (cello open D).
2. **Attack.** At every default velocity, the pre-Helmholtz transient is ≤ 50 ms for prolonged-
   period attacks and ≤ 90 ms for multiple-flyback attacks (Guettler & Askenfelt 1997).
3. **Pitch.** The bowed pitch's offset is corrected deterministically. Fides measured +3.1 to
   −1.2 cents (C2–C6) and a tracker failed. Instead, use a correction **table measured offline per
   pitch × force × β**, applied to the loop length, and assert **≤ 1 cent** over the grid. Hold
   Chorda's 0.1 cent for plucked and struck notes.
4. **Vibrato.** 5.5–6 Hz, ±15–25 cents by default, widening with register and dynamics (MacLeod).
   The partials' AM comes from the body, not an LFO: assert that each harmonic's AM depth differs
   and that the AM vanishes with the body off.
5. **Body.** The cello body's modal fit is within the perceptual thresholds of Fritz 2007 at the
   signature modes, and passive (Maestre 2017).
6. **CPU.** One solo voice ≤ 0.5 % of a core at 48 kHz on the bench's quiet run. Sixteen voices
   split across the pool.
7. **The house invariants:** render = live; any block size the same bits; pool = serial; null at
   defaults; an e2e test through `assemble`.
8. **The ear.** A blind ABX against the Karoryfer cello and the Smolken bass (solo note, and a
   four-bar phrase with legato and a CC1 swell), as Chorda's.

### 6.4 Risks

1. **The sound is not convincing in the attack and the high register.** This is SWAM's own
   criticism, after a decade. It is the biggest risk, and only the owner's ear answers it.
   Mitigation:
   - the measured gate above before listening;
   - the thermal friction option (Galluzzo 2017);
   - noise from bow hair as a separate measured layer;
   - accept a second listening pass, as Chorda had.
2. **Bowed pitch drift.** It is physical, and Fides's tracker made it worse. Mitigation: an
   offline correction table, deterministic, tested over the grid.
3. **CPU of a thermal friction model.** It needs a state per contact and an iterative solve.
   Mitigation:
   - a fixed iteration count, so the cost is deterministic;
   - the exponential curve as the default;
   - measure before choosing.
4. **No measured bodies for viola or bass.** Mitigation: measure borrowed instruments with
   Pérez Carrillo's glissando method or a hammer. The owner can arrange the instruments (a
   beta-checklist item). Until then, scale the cello and violin bodies and say so plainly.
5. **Recorded pack quality.** It has two layers and no legato, so its honest bar is "finished
   sketch", and the pack card must say so. Patents: a **continuous** CC1 crossfade only (Fable
   Sounds, *lawyer*). No stored legato "joint" built after note-on (Yamaha US 7,557,288,
   *lawyer*). Look-ahead is from Lindemann's expired US 7,718,885.
6. **Scope creep.** Four instruments × techniques × sections × stage is three rounds of work.
   Holding round 42 to two first-rank solo instruments plus the pack keeps the CLAUDE.md rule
   (two or three features a round, a mid-round integrated review).

## 7. What could not be verified

- **Patents:** every NV row in §1 (Google Patents 503). The status of US 7,557,288, 7,915,514,
  8,404,958, 8,729,376 and 7,872,190. Roland's LA/SA numbers. NI's patent-marking page. EP and JP
  counterparts.
- **Content:**
  - the section sizes, room and microphones of VSCO 2 CE (not stated on the pages read; the
    versilstudios.net page has an **expired TLS certificate** and was not opened);
  - the Iowa viola, cello and bass pages (only the violin's were read);
  - Good-sounds' CC BY copy (CORA refused);
  - QUARTET's licence;
  - the Carlos Vaquero packs' licences (login);
  - VPO's current size.
- **Measurements:**
  - Guettler 2002's and Woodhouse & Galluzzo 2004's page data;
  - Woodhouse 2003 (thermal) pages;
  - the Rasch 1979 figures;
  - Wing 2014's asynchrony numbers (full text refused);
  - any string-section pitch-spread study (none found);
  - Percival's and Pickering's string tables (cited second-hand);
  - D'Addario and Pirastro tensions (image only, or retailers);
  - Jansson's book URL;
  - Askenfelt's double-bass modes;
  - the HAL 10-violin admittance paper.
- **Products:** everything marked NV in §4 (Spitfire and NI sites rate-limited); no 2025–26
  independent review of SWAM v3 or PolySWAM was found.
- **Open code:** a wide code search for MIT/BSD bowed strings was not run; Serafin's thesis PDF was
  not found.
