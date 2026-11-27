# ADR 0166 — Arcus's stroke shape and the classical player

**Status:** Proposed (round 42, the `arcus-classical` builder; the number is provisional, the
integrator's to confirm — 0165 may be taken) · **Date:** 2026-11-26 · **Builds on:** ADR 0157
(Arcus), ADR 0160 (the sound lab), ADR 0162 (the player), ADR 0164 (the fiddle) ·
**`DSP_VERSION`:** 20 · **Schema:** none (no stored key added)

## Context

The analysis of Arcus against recordings (`F:\projects\cantus-work\listen\arcus-analisis`, Part 2,
the owner's *"ve arreglando los puntos del clásico"*) ranked what sounds unreal in the classical
instruments: long notes flat as an organ's (they started 2–5 dB and ended 3–4.5 dB under their
peak; the HSD players' 7–15 and 10–35; the crest factor low on 31 of 46 groups), the spectrum
settling too soon (`t_steady` 0.05–0.34 s against 0.15–0.85), the violin's and the viola's
releases too fast (150–260 dB/s against ≤ 60–150), the loud cello and the bass's top dull,
intonation in music too perfect (3–7 cents p90 against 13–16), the pitch gate missed with the
player on, the pizzicato clean and 4 cents flat, the vibrato starting at 0 s inside a phrase.

## Decision

- **The stroke's shape is the bow's, the player's** (`cantus_dsp::arcus::shape`): a contour in dB
  inside every sustained note moves the bow's speed (the force riding it — `Bowing::scaled`:
  Schelleng's limit, the measured region and the force are linear in the speed, so the bowing
  keeps its place) and, for 0.6 of it, the dynamic the bowing is read at (the contact and the
  force's place), that share lagging the level by 0.3 s (the tone opens after the level rises).
  From silence it starts at the instrument's swell depth — within 15 dB (the bass 8), × (1 − 0.8 d)
  loud, less under C3 (none at E1) — over 1.6 × the swell's length (0.2 … 0.6 s); **what the bow
  may not carry stays the output's swell** it was (pass 4's rise); a fresh note in a fast passage
  bites at once; a bow change keeps its speech (a dip there rang the viola's change ticks over
  the references'); a slurred note starts 3 dB under where the last had sagged; the stroke holds
  still 0.3 s, then sags 6 dB (τ 1.2 s) and breathes (a seeded walk of about a dB, a step every
  0.45 s); the key let go **tapers** the bow 12 dB over 1.5 × `bow.release` — in proportion to
  how long the note was held, up to 0.8 s (the Paganini caprice's short notes) — and the bow
  lifts still moving. The hair's noise rides the speed. Guettler's attack gate bounds the depths.
- **The vibrato's way back** into a note inside a phrase (a bow change or a slur that is not a
  fast passage's): 0.1 of itself until 0.08 s, full by 0.38 s.
- **The player's intonation in a passage** (never a note from silence, so every pitch gate's
  note keeps its pitch): each note's seeded error (three uniforms, ≈ 8 cents; × 1.6 on a shift)
  corrected to 0.6 of itself with τ 0.18 s, a semitone step's tendency ±4 cents.
- **The string after the bow (model 2):** a released string the bow has left rings at the
  family's measured pluck T60 and its fall with the partials (`pizz_t60`), never longer than
  `ring`; **the corner** (`InstrumentSpec::corner_hz`) moves with the bow's force against the
  same bowing's mf force (power 0.8 above, 0.3 below, × 0.5 … 3) — on the violin and the viola
  only (on the cello it moved nothing, on the bass it broke the E string's attack gate); **a pluck** on a string the bow
  did not leave pushed uses the loop's DC blocker at 1/1024 of the fundamental (at 1/64 its phase
  lead, matched at the fundamental only, left the partials 3–4 cents flat), and its finger's
  contact shortens with the period from the lowest open string (power 0.8, at least 0.5 of it); the
  viola's and the cello's vibrato × 0.8 (the strings-hq phrases).
- **A fiddler's bow** (`bow.grit` > 0, the Fiddle character) plays its own player: the classical
  shape, taper, vibrato's way back and intonation give way to it (all at grit 1, so the Fiddle
  preset's measured lines keep their numbers); instead its fast separate notes are slurred two or
  three to a bow (the shuffle, seeded), its fast bow changes turn in 0.6 of `bow.change`, and its
  long notes' vibrato widens late (from 0.3 s, to 2.4 × the preset's depth and 1.12 × its rate).
- **No new key.** Everything is `player`'s (0: none of the player's; the string's model-2 changes
  stay); the tools try other values through `ArcusSettings::shape_try` (never stored,
  `ARCUS_SHAPE=name=value,…` in `arcus_match` and `arcus_upper_listen`), as the match tool tries a
  match.
- **The bowed-pitch tables**: the upper strings' measured again with the player on (0.6–2.95 s);
  the bass's rows again in pass 6's window (three passes); the cello's pass 6's (they keep both
  gates with the shape).

## Consequences

- Every Arcus note sounds different at the defaults: `DSP_VERSION` 20, `upper_tests::LOWER_HASHES`
  re-made, and `PASS6_HASHES` re-made as the player's null on DSP 20's string (the string's own
  model-2 changes move it too). Arcus has never shipped (`READY` false), so model 2 changes in
  place, as passes 2–6 and the player did; model 1 is untouched.
- Live playing still has no look-ahead: a note's end inside a slur cannot taper before the next
  note comes; the taper after a key and the sag carry what can be known. Look-ahead on playback
  (the survey's item 7) is still the next layer.

## What the tests hold

`cantus-dsp` `arcus::tests_classical` (with the player off every knob of the shape is inert, to
the bit; a held note swells, sags, tapers before the lift and rings on after it on all four
instruments; the intonation seeded and sparing a note from silence; a pluck's partials within
half a cent; a fiddler's shuffle); every earlier Arcus gate at the new defaults (the block sizes,
offline = real time, the pool, rest, the pitch gates).
