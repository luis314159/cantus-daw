# strings-hq: better references for the bowed strings (round 42, strings-refs)

Why: the owner, listening to Arcus's classical renders beside the earlier references (2026-11-26):
"Va sonando mucho mejor pero deberías buscar mejores grabaciones de referencia con más calidad".
The analysis also found the violin and viola groups resting on few notes, and the viola band on
two instruments only. **Access date of everything here: 2026-11-26.** The pages and records as
served that day are in `pages/`.

## 1. University of Iowa MIS, the 2012 recordings (`iowa2012/<violin|viola|cello|bass>/`)

- **URLs:** https://theremin.music.uiowa.edu/MIS.html; the pages `MISviolin2012.html`,
  `MISviola2012.html`, `MIScello2012.html`, `MISdoublebass2012.html`; the zips
  `sound files/MIS/Strings/<inst>2012/<Inst>.<arco|pizz>.mono.2496.zip`.
- **Terms, verbatim (MIS.html):** "Since 1997, these recordings have been freely available on this
  website and may be downloaded and used for any projects, without restrictions." The 2012 pages
  state no other licence ("These freely available recordings have been used by countless musicians
  and in over 270 research papers"; they ask for a donation). Not a CC licence: **measure only**,
  as the earlier Iowa sets (credit: University of Iowa Electronic Music Studios, Lawrence Fritts).
- **What was taken:** every `.aif` of the eight mono 24/96 zips (arco and pizzicato of each
  instrument; the `__MACOSX` entries left), by HTTP range requests, CRC-32 checked.
- **The recordings (the pages' tables):** Anechoic Chamber, distance 5 feet, Earthworks QTC40,
  Metric Halo interface; runs of notes on each string at **pp, mf and ff**, arco and pizzicato.

  | | instrument | player | date | files | as delivered |
  |---|---|---|---|---|---|
  | violin | Nicolai Tambovsky (1998) | Leonid Iogansen | 2011-12-11 | 72 | **16-bit** (61 at 96 kHz, 11 at 48 kHz) despite the zip's "2496" name |
  | viola | Anton Krutz (2001) | Manuel Tabora | 2012-05-11 | 66 | 24-bit 96 kHz |
  | cello | Charles Quenoil (1923) | Yoo-Jung Chang | 2012-05-11 | 48 | 24-bit 96 kHz |
  | double bass | Anton Krutz (2004) | Mike White | 2012-05-11 | 78 | 24-bit 96 kHz |

  These are **other instruments and players** than the pre-2012 Iowa sets already measured (a
  third viola, a fifth cello, a fifth bass). The viola page links some files under another file's
  label (e.g. its "sulD mf C5B5" links the sulA A4B4 file); the zips' own names are kept.

## 2. Musopen, Mozart String Quartet in C major K. 465: the session (`musopen-k465/`)

- **URL:** https://archive.org/details/musopen-mozart-quartet-in-c-major-k465-compressed
  (`Mozart_Quartet_in_C_Major_K465.zip`, 8 621 082 102 bytes: the Pro Tools session, uploaded to
  the Internet Archive's `musopen` collection in 2012).
- **Terms:** the item has no licence field of its own. Musopen's Kickstarter compilation
  (https://archive.org/details/musopen-lossless-dvd-flac, licence field
  `http://creativecommons.org/publicdomain/mark/1.0/`), verbatim: "This project raised money to
  purchase and release music to the public domain. This project successfully completed in 2012.
  The music recorded and released into the public is as follows: … String Quartets: … Mozart
  Quartet in C Major K 465". The collection (https://archive.org/details/musopen): "We provide
  recordings, sheet music, and textbooks to the public for free, without copyright restrictions."
  The same session template's Figaro files are on Wikimedia Commons as CC0 ({{Musopen}}). So:
  **public domain** (credit given anyway: Musopen, musopen.org).
- **What was taken:** the four players' spot microphones of three takes, by range requests
  (CRC-32 checked): `1st violin F`, `2nd violin F`, `viola F` takes 02, 05, 09 and `celli F` takes
  01, 04, 08 — the cello's counter runs one behind the others (its _01 is their _02: the same
  length to the sample). 12 files, 96 kHz 24-bit mono WAV (bext), 550.8, 468.4 and 393.2 s per
  take. Left: the Decca tree, the "STEREO" pair, the surrounds, the other nine takes.
- **The recording:** the players, the room and the distances are not stated (the track names are
  the session template's: "celli" is the one cello). Each spot also hears the other three: when a
  player rests, its mic drops only ~13 dB. Real music — phrases, vibrato, legato, bow changes,
  intonation — but **with bleed**: `tools/soundlab/adapters/strings_hq_phrases.py` picks the
  windows where each player dominates its own mic.

## 3. Good-sounds, the double bass (`good-sounds-bass/sound_files/bass_alejandro_recordings/`)

- **URL:** https://zenodo.org/records/820937 (`good-sounds.zip`, 13 906 331 564 bytes).
- **Licence:** the Zenodo licence field `cc-by-4.0` (records 820937, 820936 and 4588740 alike);
  CORA's readme (doi:10.34810/DATA2314, 2025): "Creative Commons Attribution 4.0 International
  (CC BY 4.0)". Credit: Romaní Picas, Parra Rodríguez, Dabiri, Serra, MTG-UPF.
- **What was taken:** the 159 `neumann` files of the pack `bass_alejandro_recordings` (player
  "alejandro"): 80 bowed notes ("good-sound-bow"), 40 + 39 plucked ("good-sound-pizzicato-hard" /
  "-soft"); the dataset's `database.sqlite` (already in `study/good-sounds/`) names each note.
- **The recording:** the UPF studio, Neumann microphone, 48 kHz 24-bit mono. A sixth double bass.

## Looked at and refused (2026-11-26)

- **TU-Note Violin Sample Library** (TU Berlin, anechoic, single notes in four dynamics, two-note
  transitions, solo pieces; doi:10.14279/depositonce-6747): **CC BY-ND** — refused (ND).
- **URMP** (Rochester): no licence stated, registration form — not used.
- **PHENICX-Anechoic**: non-commercial only. **Aalto anechoic orchestra**: "free for academic
  research", MP3 — neither meets the rule (both stay out of the new subjects).
- **Wikimedia Commons** lossless string audio under PD/CC0/CC BY: almost all 78 rpm transfers
  (band-limited, surface noise) — not better than what is here. The CC0 Musopen Figaro spot mics
  there are orchestra **sections**.
- **Internet Archive** "public domain string quartet" items: computer MIDI renders.
- **MusicNet** (CC BY 4.0 labels; recordings from the Gardner Museum, the European Archive and
  Musopen, mixed terms, one 11 GB tar.gz): not taken; its Musopen quartets are the same sessions.
- **VSCO 2 CE**: its solo violin and solo contrabass are already measured; the rest are sections.
- **Freesound** ldk1609's violin packs (3559-3565, CC0, 44.1 kHz 16-bit stereo AIFF, G notes
  only): the originals need a login — a possible hand fetch, little to add.
