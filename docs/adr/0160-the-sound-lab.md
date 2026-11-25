# ADR 0160 — The sound lab: one measurement and one blind kit for every instrument

**Status:** Accepted (round 42, the `soundlab` builder; the owner's question of 2026-11-25) ·
**Date:** 2026-11-25 · **Builds on:** ADR 0157 (Arcus; its reference study), the blind kits of
`docs/round40/abx5.md` and `docs/round42/tymp-abx.md` · **Schema:** none (tools only; nothing
ships in the app)

## Context

The owner (2026-11-25, translated): *"Do we have good tools for making ABX tests and our own
analyses comparing our sound against recordings? I want to be sure everything is ready for
when we develop more instruments."* After round 42 come a guitar family, winds, a formant
choir, struck bars and membranes (`docs/PLAN-after-round-42.md`, track B), each to pass a
blind ABX against recordings and a measured comparison before it is in the picker.

The tools existed, one set per instrument (`docs/round42/soundlab.md` has the audit): Chorda's
voicing lab (two analysts' scripts, a phrase metric, a tuning gate), Tympanum's fit and ladder
(its own MUSHRA page), Arcus's `arcus_match` and the reference study (the best estimator of
the three, but named for the bow and run against one family's bands), a shared blind page for
Chorda and Tympanum filled by a Rust generator each, and Arcus's own hand page whose files were
named in clear (`arcus-cello.wav` beside `grabacion-cello.wav`: no blinding at all). A new
instrument would have meant a new set of scripts and a new page.

## Decision

- **One package, `tools/soundlab/`** (Python, `python -I`, numpy/scipy/soundfile), configured
  per instrument by **one subject file** (`subjects/<id>/subject.json`): the references'
  catalogue (path under `SOUNDLAB_REFS`, source, licence, credit, physical instrument,
  articulation, note, dynamic), the kinds of note (`sustained`, `decaying`), the features that
  apply, the split, the candidate's render command and file pattern, and the blind kit's
  `listening` section. `lab.py check|refs|render|measure|report|all|kit`.
- **The estimators are the Arcus study's, generalised, not reinvented**: its signal core and
  note analysis ported unchanged in method (`core.py`, `notes.py`; a test holds the port to
  the study's own records to 1e-4), named by what they read (a sustained or a decaying note),
  plus what no instrument had in shared form: BS.1770-4/R128 loudness (integrated, momentary,
  short-term, LRA, true peak), the centroid's trajectory, noise between partials for a
  decaying note, T60 per octave band (membranes, bars, cymbals), vibrato cycle by cycle
  (rate and extent jitter: pass 5's reading), stereo and the room's tail. One registry,
  every estimator documented.
- **The held-out band method is the report's core.** Cells (articulation, note, dynamic) are
  split by a seeded rule (SHA-256 one in three, or an instrument's own expression, evaluated
  without running code); a fit uses only the other cells' references; per group (articulation
  · register octave · dynamic class) and feature, the band is the held-out references' weighted
  10th–90th percentile **with each physical instrument weighing one**; the candidate's median
  is placed in or out (band widths for a scalar, the RMS of the elements' distances for a
  vector); runs are kept and compared before → now. JSON for tools, an HTML page with plots
  for people.
- **One blind page for every instrument.** `crates/cantus-ui/examples/abx_kit/abx.html` stays
  the only page; it gains **a MUSHRA part** (ITU-R BS.1534-3: an open reference, a hidden
  reference, the candidates and low-pass anchors at 3.5 and 7 kHz, each sample played and
  rated 0–100 before Next; ratings, plays and changes kept and saved) and **an order of parts**
  (`SUBJECT.parts`, default ABX then preferences — Chorda's and Tympanum's kits unchanged).
  Tympanum's ladder MUSHRA is generalised by it. A kit is filled either by an instrument's
  Rust generator (rendering in-process through the engine, as Chorda and Tympanum do) or by
  the lab's `kit.py` from files (any renders, any recordings): the same placeholders, the
  same guarantees — BS.1770 loudness match with `kit.rs`'s two passes and ceiling, 64-bit
  CSPRNG names checked against `names.rs`'s structure rule, the veiled token table, catch
  questions and the hidden reference as one clip under two names, the key beside the kit and
  never in it, offline `file://` use, answers returned as JSON. A subject declares what must
  stay hidden (`hide`), which the browser check reads.
- **Reading results** (`tools/listening/abx_read.py`, still plain Python): any number of
  files, per listener and pooled; the binomial p, the critical count and the power at the
  kit's trial count, and the trials needed for a stated power; catch questions flagged when
  failed; MUSHRA per source (median with a bootstrap CI, mean with Student's t CI), the
  BS.1534-3 post-screening (hidden reference under 90 in more than 15 % of items) and the
  anchor check; a sign test of each source against the hidden reference.
- **Tests that prove the lab measures right** (`tools/soundlab/tests/run.py`, one command;
  `--light` without downloads): outside values — BS.1770-4's published coefficients, EBU Tech
  3341/3342's cases on the signals they define, TinySOL's published pitches; the kits' Rust
  meter on a real generated kit; the study's records; synthetic notes of known values for
  every feature; the kit's guarantees on a built kit. `kit.rs` gains a cargo test that every
  lab subject is a page subject the Rust side accepts.
- **The protocol** a new instrument follows is `tools/soundlab/README.md`'s: references CC0 /
  CC BY (or terms as free, quoted) downloaded outside the repository → the subject file → the
  measured report → the owner's named listens → the blind kit → reading → the gate.

## Consequences

- A new instrument is a subject file (and its render example), not new scripts: the worked
  example (`subjects/example-tibia/`, a synthetic struck bar) and Arcus's cello
  (`subjects/arcus-cello/`, 578 free reference files) show the shape.
- The per-instrument labs stay (Chorda's analysts' scores, Tympanum's fit, Arcus's
  `arcus_match` fit); new work measures through the lab, and an instrument's fit tool reads the
  same split.
- The page's state gains `mushra` and `parts` (version 7); a kit made before reads as before.
- The gate's bar in the README (80 % power against p1 = 0.7, i.e. 37 pooled ABX trials; the
  MUSHRA interval) is the lab's proposal; the owner's verdict remains the ship decision.
- Not done here: the EBU loudness test set's own files are served behind a browser challenge
  (the test reads them when fetched by hand, `SOUNDLAB_EBU`); a decaying note's noise between
  partials is a new estimator with only synthetic validation; vibrato and the attack's burst
  keep the study's known biases (its README's table).

## What the tests hold

`tools/soundlab/tests/`: `test_loudness` (BS.1770-4 Tables 1–2; Tech 3341 cases 1–5 ±0.1 LU;
Tech 3342 cases 1–4 ±1 LU; −3.01 for one channel; true peak; the Rust meter within 0.05 LU on
Tympanum's kit), `test_features` (synthetic truths; TinySOL's 34 published pitches), 
`test_agreement` (the study's records), `test_band` (the split, the weighting, the placement,
the ABX tables' 0.0384, power, post-screening), `test_kit` (a built kit's guarantees; the
page's unveil in node). `cargo test -p cantus-ui --example tympanum_abx`:
`the_sound_labs_subjects_are_page_subjects`. `abx_browser_check.mjs` on the Arcus cello kit
(MUSHRA included) and on Chorda's and Tympanum's subjects.
