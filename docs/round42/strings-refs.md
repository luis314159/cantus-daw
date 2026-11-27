# Round 42 — strings-refs: better references for the bowed strings

Builder: strings-refs (branch `r42/strings-refs` from `r42/integration` 7281bf1d1). Why: the
owner, after the classical renders (2026-11-26): "Va sonando mucho mejor pero deberías buscar
mejores grabaciones de referencia con más calidad"; and the analysis (Part 2) rested the violin
and viola on one Arcus note per group, the viola band on two instruments.

## 1. Sources and licences (checked 2026-11-26; `$CANTUS_REFS/strings-hq/SOURCE.md` has the terms verbatim)

| source | what | quality | licence | taken |
|---|---|---|---|---|
| **Iowa MIS 2012** (theremin.music.uiowa.edu) | violin (Tambovsky 1998, Iogansen), viola (Krutz 2001, Tabora), cello (Quenoil 1923, Chang), bass (Krutz 2004, White): runs of notes per string, arco + pizz, **pp / mf / ff** | anechoic chamber, Earthworks QTC40 at 5 ft; 24-bit 96 kHz mono (the violin zip is 16-bit, 61 files at 96 kHz and 11 at 48 kHz) | "may be downloaded and used for any projects, without restrictions" → measure only (`Iowa-MIS`) | 264 files, 3.8 GB |
| **Musopen, Mozart K465 session** (archive.org `musopen-mozart-quartet-in-c-major-k465-compressed`) | one spot mic per player, three takes (551 / 468 / 393 s) | 24-bit 96 kHz mono; room and distances not stated; each mic hears the others ~10–13 dB under its player | public domain ("recorded and released into the public domain", Musopen's Kickstarter compilation, PDM 1.0; the collection: "without copyright restrictions") | 12 files, 1.7 GB |
| **Good-sounds** double bass (Zenodo 820937) | player alejandro: 80 bowed, 79 plucked notes | UPF studio, Neumann, 24-bit 48 kHz | CC BY 4.0 (Zenodo field, CORA readme) | 159 files, 57 MB |

Read over the network: ~4.6 GB (the zips' members by range requests, CRC-32 checked); 5.5 GB on
disk (`fetch.py verify strings-hq`: 445 ok). Refused: **TU-Note** violin library (anechoic,
the best fit — CC BY-ND), URMP (no licence), PHENICX (NC), Aalto anechoic ("academic research",
MP3), Commons' PD 78s (band-limited), IA's "PD string quartets" (MIDI renders), MusicNet (mixed
terms, its Musopen quartets are these sessions), VSCO sections, Freesound ldk1609 (CC0 but a
login and 16/44.1 G notes only: a possible hand fetch).

## 2. What changed in the repository

- `tools/sound-refs/catalog.toml`: the `strings-hq` set (origins: eight Iowa zips, the Musopen
  zip, the Good-sounds zip, the Iowa pages, the archive.org/Zenodo records as `meta`),
  `files/strings-hq.tsv`, `sources/strings-hq/SOURCE.md`. A fetch of one file per origin into an
  empty folder verified (12 ok, 4 absent-meta).
- `tools/sound-refs/fetch.py`: `RangeFile` falls back to curl (TLS verified by the system's
  store) when Python's OpenSSL refuses a chain — at the probe or at a block a redirect sends to
  another node (archive.org). Never a bypass.
- `tools/soundlab/subjects/arcus-violin`, `arcus-viola`, `arcus-bass` (new), `arcus-cello`
  (extended): subject, references, README (how it was built), `phrases.json` (not the bass).
  `adapters/strings_hq.py` (a family's catalogue + strings-hq), `adapters/strings_hq_phrases.py`
  (`pick`, `measure`), `adapters/verdicts.py` (two reports' verdicts side by side).
- `crates/cantus-dsp/examples/arcus_match`: `render <out> [cello bass violin viola]` — the
  violin and viola at every key (default unchanged). No Arcus DSP touched.
- `tools/listening/strings_hq_pairs.py`; DEVELOPING §4 names the tools.

| subject | references (held-out notes) | instruments (bands weigh each as one) | candidate notes |
|---|---|---|---|
| arcus-violin | 642 (577; the analysis 334) | 6: Iowa 2012, Iowa, TinySOL, Good-sounds ×2, VSCO | 92 (was 44, one per group) |
| arcus-viola | 437 (806; 278) | 3: Iowa 2012, Iowa, TinySOL (was 2) | 92 (was 44) |
| arcus-cello | 698 (557; 280) | 6: + Iowa 2012, + Good-sounds margarita's open strings p/mf/f | 88 |
| arcus-bass | 784 (624; 275) | 6: + Iowa 2012, + Good-sounds bass (was 4) | 80 |

## 3. The classical measures again (current Arcus = r42/integration)

The cello and bass renders of this build are byte-identical to the analysis's (168 of 168): the
same Arcus, only the references changed. For the violin and viola, run `hq0` re-judges the
analysis's own sparse renders against the new references and `hq1` the dense renders. Reports:
`F:\projects\cantus-work\soundlab-hq\arcus-*\runs\hq1\report.html`; side by side:
`F:\projects\cantus-work\strings-refs\cmp-*.txt`. Counts over the analysis's 25 features
(in / mostly / out):

| | the analysis | new references, same renders | new references, dense renders |
|---|---|---|---|
| violin | 5 / 4 / 16 | 9 / 6 / 10 | 7 / 7 / 11 |
| viola | 3 / 6 / 16 | 6 / 9 / 10 | 7 / 9 / 9 |
| cello | 4 / 11 / 10 | — | 7 / 11 / 7 |
| bass | 3 / 11 / 11 | — | 6 / 11 / 8 |

**Where the conclusions change.** Out → in or mostly with the wider, better-founded bands:
harmonics 1–20 (cello, bass; the violin only with the sparse renders), the mean centroid
(violin, cello), the attack's noise burst (all four) and its first 25 ms (viola, cello), the
pitch spread (now in for all four), the vibrato's onset and jitter (violin) and rate and depth
(viola), the viola's harmonic order and AM, the violin's and viola's pizz T60 of the
fundamental, the bass's release and room. Worse: the cello's vibrato jitter (mostly → out, 0.13
against a band from 0.12: borderline), the bass's pizz rise (mostly → out, 10 ms faster in 4 of
12 groups) and pizz T60 (in → mostly). **With dense renders** the violin's and
viola's harmonics 1–20 stay out (2.0 and 2.1 dB RMS) and the violin's noise between harmonics is
out (3.2 dB under): the single-note groups had flattered them.

**What every reference set still says (the robust signals):**
- pizzicato far too clean — median HNR 22–33 dB over the band, harmonics 6–9 dB RMS out (all four);
- too little noise between harmonics, 2–4 dB under (all four), HNR by band out (all four);
- the spectrum settles too fast (t_steady 0.05–0.13 s under; all four);
- crest factor low (0.4–1.1 dB under; all four);
- the release too fast in the violin and viola (82 and 47 dB/s over the band);
- the violin's and viola's pluck rises in 2–3 ms against ≥4 ms.

**Phrases** (`F:\projects\cantus-work\strings-refs\phrases\*.json`; per-note deviation from the
phrase's own tuning, |dev| p50; vibrato peak to peak on notes ≥ 0.6 s; how far under its peak a
note starts):

| | references (Musopen + HSD) | Arcus integration | Arcus c7 (arcus-classical) |
|---|---|---|---|
| violin: |dev|, vibrato, start | 5.1 ct, 22 ct (HSD 20), 9.0 dB (181 notes) | 1.3 ct, 21 ct, 2.8 dB | 3.9 ct, 23 ct, 3.6 dB |
| viola | 4.6 ct, 21 ct, 8.4 dB (162) | 2.8 ct, 28 ct, 3.5 dB | 3.6 ct, 28 ct, 4.6 dB |
| cello | 7.0 ct, 13 ct (Musopen 25, HSD 10), 9.7 dB (319) | 2.5 ct, 31 ct, 3.5 dB | 3.9 ct, 31 ct, 3.6 dB |

Integration Arcus intonates too exactly (c7 is near the players); the viola's and cello's
vibrato is wider than the players' (28–31 against 21 and 10–25 cents); every Arcus note starts
within ~4 dB of its peak where the players start 8–10 dB under (the swell is missing). The
Musopen viola agrees with HSD's bleed-free stems (21 / 21 cents, 4.8 / 4.6 ct); the Musopen 1st
violin's vibrato (58 ct on 8 notes) is not trusted — bleed or fast passages.

## 4. Listening pairs

`F:\projects\cantus-work\listen\arcus-clasico\refs-hq\escuchar.html` (Spanish): per
instrument an Iowa 2012 note at mf (B4, F4, F3, F2; the bass also the Good-sounds bass) beside
Arcus's render of the same key, and an 8 s Musopen phrase beside the c7 phrase the owner last
heard ("ahora"). 16-bit 48 kHz, −23.0 LUFS each, peaks −6 to −15 dBTP; `pairs.json` has the cuts.

## 5. Open items

- **No bass phrase**: no free, good solo-bass phrase recording was found.
- **Viola pizzicato** rests on two instruments (the two Iowa violas); no free solo-viola single
  notes beyond Iowa ×2 and TinySOL exist that pass the rule.
- The Musopen spots carry ~10–13 dB of bleed; the 1st violin's vibrato figure needs a check
  by ear or a score-informed tracker.
- The players, room and distances of the Musopen session are not stated.
- The arcus-classical builder's scratch subjects (`cantus-work/soundlab-subjects`) and work
  folder are untouched; these runs used `SOUNDLAB_WORK=F:\projects\cantus-work\soundlab-hq`.
- Not verified on Linux (no WSL check run); `arcus_match` builds and runs on Windows, rustfmt clean.
