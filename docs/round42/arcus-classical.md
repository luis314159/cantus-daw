# Round 42 — Arcus's classical sound (the `arcus-classical` builder)

**The owner's request** (2026-11-26): *"Ve arreglando los puntos del clásico"* — the ranked list of
Part 2 of the Arcus analysis (`F:\projects\cantus-work\listen\arcus-analisis\index.html`, *clásico*;
its data `clasico\parte2.json`): violin, viola, cello and double bass, played by Arcus's bowed
string and its player (ADR 0162), measured against recordings by the sound lab (ADR 0160) on the
held-out cells, and the solos' phrases against the HSD quartet's players.

Status: **in progress** (this file is kept current at every clean commit).

## How it is measured

- `tools/arcus/classical/lab.sh <run> [<bin dir>] [<before>]` — the four instruments' held-out
  notes rendered at Arcus's defaults (the cello and the bass by `arcus_match render`, the violin
  and the viola by `arcus_upper_listen <dir> study`), measured by the lab, judged on the held-out
  cells, and `summary.py` puts the pass's rows beside the analysis's run (`analisis-clasico`, the
  base `e63ceddd2`, reproduced bit for bit by this branch's base build). The violin's, the viola's
  and the bass's subjects are the analysis's scratch subjects, kept in
  `tools/arcus/classical/subjects/`; the cello's is the lab's.
- `ARCUS_SHAPE=name=value,…` hands a shape to try to the renders (`arcus::shape::Knobs::parse`,
  `ArcusSettings::shape_try`, never stored) — one build, many tries.
- `tools/arcus/classical/phrase.py` — the solos' phrases (`arcus_demo <dir> solos`) by the
  arcus-expr phrase study's note cutter: intonation p90, the long notes' start and end under
  their peak, the vibrato's extent and onset, beside the HSD players' numbers.
- `tools/arcus/classical/clips.py` — the owner's clips: the analysis's windows of the solos.

## What changed (so far)

1. **The stroke's shape** (`crates/cantus-dsp/src/arcus_shape.rs`, the player's): a contour in dB
   moves the bow's speed (the force riding it: Schelleng's limit and the region are linear in the
   speed, `Bowing::scaled`) and, for a share, the dynamic the bowing is read at (the contact and
   the force's place: the tone opens as it swells). From silence it starts at the instrument's
   swell depth (the output's amplitude swell retired with the player on); a bow change starts its
   stroke 6 dB under; a slur starts where the last note had sagged; after the rise the stroke sags
   (6 dB, τ 1.2 s) and breathes (a seeded walk of about a dB, 0.45 s steps); the key let go
   **tapers** the bow (12 dB over 1.5 × `bow.release`) before it lifts, and the lift after a taper
   is a stopped bow's (25 ms). The hair's noise rides the speed.
2. **The free string after the bow** (model 2): a released string the bow has left rings at the
   family's measured pluck T60 at the note (never longer than `ring`), not the bowed table's.
3. **The vibrato's way back** into a note inside a phrase (after a bow change or a slur that is not
   a fast passage's): 0.1 of itself until 0.08 s, full by 0.38 s.
4. **The player's intonation in a passage** (never a note from silence): each note's seeded error
   (≈ 8 cents spread, × 1.6 on a shift) corrected to 0.6 of itself (τ 0.18 s), a semitone step's
   tendency (±4 cents).
5. **The corner's rise with the force** (model 2's string, `Knobs::bright`): built, measured below.

## Measures, before → after

(filled in as the runs come in)

## Open
