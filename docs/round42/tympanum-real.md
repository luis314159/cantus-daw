# Round 42 — `tympanum-real`: Tympanum against real drums of many genres

Branch `r42/tympanum-real` (from `r42/integration` e63ceddd2). The owner (translated):
*"Tympanum still lacks a lot. Download professional-quality recordings that are free of
copyright. If the drums aren't separated, convert the hits to MIDI, then analyse the sound
against what we have now and iterate. Consider the original kit type, where each drum is
struck and at what levels. Listen to several kinds of kits from different genres and adapt
Tympanum well, so it reaches levels no generated drum kit reaches today."*

This document is written as the work goes; its last section says what is done and what is
open.

## 1. The library (`tools/sound-refs/catalog.toml`, the `drums-*` sets)

Everything is under `$CANTUS_REFS/drums/` (`F:\projects\cantus-refs\drums`), outside the
repository; the catalogue holds each kit's licence, credit, the kit, room and mics, genre,
layers and whether it is multitrack or real playing (`[[set.kits]]`).

| set | what | licence | genre | kind |
|---|---|---|---|---|
| `tympanum` (round 40) | DRSKit 2.1, 13 mics | CC BY 4.0 | rock/pop studio | multi-mic hits |
| `tympanum` | Virtuosity Drums, 6 mic positions, up to 36 layers | CC0 | jazz (sticks) | multi-mic hits |
| `drums-dg` | The Aasimonster 2.1, 16 ch | CC BY 4.0 | death metal | multi-mic hits |
| `drums-dg` | MuldjordKit 3, 16 ch (Tama Superstar) | CC BY 4.0 | metal/rock | multi-mic hits |
| `drums-dg` | ShittyKit 1.2, 8 ch (Stagg GIA, cast-iron cymbals) | CC BY 4.0 | lo-fi/garage | multi-mic hits |
| `drums-packs` | Big Rusty Drums (Szpaderski 24″ kick, 14×8 snare; sticks, brushes, mallets) | CC0 | 1980s rock | close + overhead |
| `drums-packs` | Unruly Drums (wires on every drum; sticks and brushes) | CC0 | indie pop, brushes | close + overhead |
| `drums-packs` | Gogodze Phu II (7 mics, deadened toms, "Kim Jung Mi's Now, 1973") | CC0 | 1970s funk/soul | multi-mic hits |
| `drums-packs` | Frankensnare (12 snares 10″–22″, sympathetic buzz) | CC0 | snares | hits |
| `drums-packs` | Swirly Drums dry (brushes), VSCO 2 percussion (timpani) | CC0 | brushes, orchestral | hits |
| `drums-freesound` | unfa's 15-minute jam (a real kit, XY pair, no click); RutgerMuller's funk grooves and rides; Srirachasocks' brushes; soneproject's jazz-bar kit (sticks, brushes, many velocities); Matias.Reccius's metal kit; sandyrb's snares with overheads | CC0, CC BY 4.0 (per sound) | rock, funk, jazz, metal | real playing + hits (MP3 previews) |
| `drums-gmd` | The Groove MIDI Dataset's MIDI: 1 150 human performances, ten drummers | CC BY 4.0 | rock, funk, jazz, Latin, soul, gospel, punk, hip-hop … | performances (MIDI) |

Not taken, on licence: Salamander Drumkit and the AVL kits (CC BY-SA); MDB Drums, ENST,
IDMT-SMT-Drums, RBMA, MUSDB, MedleyDB, Cambridge-MT (NC, ND or proprietary); ADTOF (NC-SA);
madmom's and LarsNet's transcription models (NC). Not measured, on what the audio is: E-GMD
(a TD-17 module, no microphones), StemGMD and the WaivOps sets (rendered from sample
libraries). No CC0/CC BY multitrack of a real song with a real kit was found (ccMixter's CC BY
stems are programmed or unverified); the real playing is unfa's jam and RutgerMuller's grooves.

**How the kits are played.** Every recorded kit is read by Tympanum's own loader (the app's
DrumGizmo and SFZ readers) and played through the engine's offline render, each with its own
default mix — as a user hears it on loading it — so recorded and modelled kits go through the
same path: `tools/tympanum-real/content_root.py` gives Cantus's CC0 packs (as the app installs
them, joined by junction) and the DrumGizmo kits a receipt naming the programs Tympanum plays.

## 2. What Tympanum is measured against, and how we would show it is better

**The instruments to beat** (research 2026-11-26, nothing bought or downloaded):

| instrument | kind | how it could be measured without buying it |
|---|---|---|
| IK Multimedia MODO DRUM 1.5 | modal synthesis + samples (hybrid), 13 kits, size/tension/shell per drum | the free **MODO DRUM CS** (the Studio kit, full engine): the owner installs it on his machine (an IK account), renders `tympanum_real`'s probe MIDI and the GMD grooves; the measures run on its bounce |
| AAS Chromaphone 3 | physical resonators (drumhead, membrane) | 15-day full trial; official demo MP3s for listening only |
| Reason Kong (Physical Bass Drum / Snare / Tom) | physical models | Reason's trial |
| Roland Prismatic Sound Modeling (TD-50X and later) | modelled acoustic drums (no samples played) | hardware: official videos only (not measured) |
| Nord Drum 3P | modelling percussion synth | a CC0 Freesound sample exists (RutgerMuller 463597) — one hit, too little |
| Toontrack Superior Drummer 3 / EZdrummer 3, XLN Addictive Drums 2, Steven Slate Drums 5.5 (free tier), BFD3 (BFD Player), GetGood Drums, NI Abbey Road / Studio Drummer, Logic Drum Kit Designer (90-day trial), Steinberg Groove Agent 5 | sampled (the realism bar) | the free tiers and trials the owner can install; the same probe MIDI and grooves |

**How "better" is shown.** The same three tests for every instrument, against the recorded
kits' spread:

1. **The measures** (`compare.py`): every per-hit feature of §3 — dynamics, brightness with
   level, glide, decay per band, wires, wash, take-to-take variation, image — counted inside or
   outside the recorded kits' range, per piece; and the grooves' (`grooves.py`). An
   instrument's score is the share of features inside, and how far outside the others sit.
2. **A blind MUSHRA** per piece and per groove, with a recorded kit as the hidden reference and
   the instruments as the conditions, at one loudness (`tools/soundlab` already runs MUSHRA
   kits); listeners from the owner's circle.
3. **"Recording or synthesis?"**: each clip alone, the share of listeners who say "a
   recording" — the one number a generated kit has never reached at 50 %.

Nothing in this round can rank a commercial instrument: it needs the owner's machine and his
accounts (BACKLOG). Tympanum's own numbers before and after are below.
