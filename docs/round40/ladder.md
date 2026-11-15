# Round 40 — the representation ladder (Tympanum, before a "model 6")

Off-line study, Python only; no engine change. Tools: `tools/tympanum-fit/ladder.py` (the
rungs), `ladder_measure.py` (the measures and the tables below), `ladder_kit.py` with
`ladder_kit.html`/`ladder_kit.js` (the blind listening kit and its scorer), `ladder_whistle.py`
(the cymbal tails after the owner's listen). Companion to
`research-p3/drum-strategies.md` (strategy A, §3.0 and §9), `analysis-drums-2/report.md`
and `drums-gen/NEXT-drums.md`.

**The question.** Model 5 calibrated *parameters* inside a fixed *representation* for nine
listens. Can that representation reach the recording at all — and if not, which richer
description is the cheapest one that does? Each *recorded* hit is resynthesised through
richer and richer descriptions (an oracle: no fitting error across hits, no engine, no
microphones), then measured against the recording and put in front of the owner blind.

## The owner's listening (2026-11-13, headphones) — what it changes

`compartir/tympanum-escalera-results/escalera-tympanum-escalera-1-Luis_Almazan.json`,
scored by `ladder_kit.py score`. MUSHRA (0–100, one listener, 1–4 plays a sample):

| item | R0 (hidden) | R1 | R2 | R3 | R3b | R3o | R4 | R5 |
|---|---|---|---|---|---|---|---|---|
| snare (DRSKit v127) | 100 | 24 | **64** | 25 | 50 | 39 | 34 | 5 |
| kick (DRSKit v127) | 100 | 71 | 63 | **100** | 100 | 100 | 100 | 8 |
| floor tom (Virtuosity v127) | 100 | 70 | **80** | 45 | 65 | 100 | 45 | 11 |
| closed hat (DRSKit v127) | 100 | 60 | 67 | 3 | **100** | 75 | 21 | 0 |
| open hat (Virtuosity v127) | 100 | 45 | 23 | 13 | 38 | 50 | 8 | 0 |
| crash (DRSKit v127) | *44* | 82 | 30 | 25 | 73 | 100 | 31 | 0 |
| ride (Virtuosity v80) | *70* | 70 | 25 | 60 | 40 | 30 | 30 | 0 |
| **mean** | 88 | 60 | 50 | 39 | **67** | 71 | 38 | 3 |

ABX against the recording, all heard: snare R3 16/16, R4 16/16; crash R3 14/16, R4 16/16,
R3b 15/16. The hidden reference was not found on the crash (44) nor the ride (70): those
two items' rankings are weak this round.

His words: *"En los platillos hay un sonido apreciable como 'iuuu iuuu iiuu' que se
escucha casi al final."*

### The "iuuu" near the end: tails that swell instead of decaying

`ladder_whistle.py` looked for it five ways in the last part of every cymbal and hat clip
(the kit's rungs, the other kit's hits, model 5's own b9 renders, each against its
recording). What is *not* it: gliding tones (ridges moving ≥ 25 cents: 0–4 a second in every
rung, as in the recordings), slow swoops of brightness, isolated "musical noise" blobs, a
lone band at the end — on every one of these the recordings score as high as the rungs.
What is it: **the tail climbs again near its end**, which a free decay never does.

- *Where.* The most a band (octave, 100-ms levels after 1 s) climbs over its own running
  minimum: Virtuosity open hat, 8–16 kHz, **+17 to +19 dB** under R3/R3b/R4 against 0 in the
  recording, 125 Hz +12 dB under R1 and every table; DRSKit open hat 125 Hz +11–13 dB
  against 5; crash and ride smaller (R3b's 8 kHz +4–5 dB against ≤ 1). In model 5's own
  engine renders (b9) the last second of crash, ride and open hat holds 8–16 kHz **+13 to
  +23 dB** over the recording, and the jazz open hat's 200–300 Hz +17 to +28 dB.
- *Why (1): the measuring render.* The hits files every fit used hold an event about 3.0 s
  after the open hat's v127 strike — +13.6 dB in DRSKit's, +14.3 dB in Virtuosity's and
  +10.1 dB in model 5's own render, so it belongs to the render (the hat's release or choke
  at the note's end), not to either kit; Virtuosity's ride and bell have late jumps of
  +8–14 dB at 3.1–3.9 s too. Both model 5's fit and the ladder's tables average their late
  times over a window that widens with time (± 0.15 t: 2.7–3.7 s for the 3.2-s
  breakpoint) and so pull those events back over the last second as a swell. Model 5's fits
  carry it: **40 of 128 layers** have a band within 30 dB of the loudest that rises between
  2.2 and 3.2 s — the open and half-open hats at v127 by +14 to +22 dB (35–300 Hz), the
  rides by up to +10–11 dB, the toms' lows by up to +7 dB.
- *Why (2): the highs stop decaying.* Bands fitted on the recording's noise floor keep a
  flat or rising last segment (DRSKit crash 12.8 kHz +2.2 dB/s, rides' 5–11 kHz +7…+15 dB/s
  in some layers). The engine already makes the slope past 3.2 s fall at least 10 dB a
  second; `synth.py` (the ladder's R1) does not — the R1 crash's 12–13 kHz band brightens
  from 1 s to its end, the DRSKit recording's is dark.
- *Why (3): narrow lines as noise.* The recording's long stable lines (DRSKit crash
  12.1 kHz) come back from a 1/12-octave table as a narrow, flickering noise band — a
  whistle; at 11.7-Hz bins (R3b) they stay thin lines.

So "iuuu" is a band of noise (bright "i" from the highs, dark "u" from the 35–300 Hz
bands) welling up in the last second and then cut by the decay or the clip's end — once a
hit, hence "iuuu iuuu iiuu" over a row of hits.

**Fix, for model 5 now and any model 6:** (a) fit tails only on the clean hit — stop each
band's analysis at the first late jump (or at the render's note end, ~3 s) and below the
recording's floor, with one-sided windows; (b) **monotone envelopes** — after a band's
bloom peak its level never rises, its late slope no flatter than its own clean 0.5–1.5-s
slope, and a band above 4 kHz decays at least as fast as the one below it; (c) the same
clamp in `tables_blob.py` and in the run-time table reader (the last segment already falls
≥ 10 dB/s); (d) stable lines go to the modes or to a per-bin layer, never into a
1/12-octave noise band. A run-time check worth a test: a struck cymbal's every band, 1 s
after the strike, never climbs more than ~3 dB over its running minimum (the recordings'
beating).

### What his scores say about each layer

- **R4's texture statistics: drop them.** Latent modulators, partial widths and bursts on
  top of R3 scored 38 against R3's 39, and 16/16 in both ABX tests. The cross-band
  co-modulation the strategies study proposed is not what a listener misses (the
  recorded cymbals do not co-modulate, result 1 above); the hats' plate chatter stays open
  (below), but as a physical event, not as statistics on a smoothed table.
- **R3b is the storable layer for hats and cymbals** (mean 67, next to the oracle's 71; the
  closed hat 100). The 1/12-octave table (R3) is wrong for them (closed hat 3, crash 25).
- **The kick is solved** by any fine noise table (R3, R3b, R3o, R4 all 100 against R1's 71).
- **Snare and floor tom: the attack is the gain, the STFT layers hurt.** R2 (model 5 plus
  the hit's own minimum-phase attack) is the best storable rung on both (snare 24 → 64,
  floor tom 70 → 80); replacing model 5's band noise by a 1024- or 4096-point STFT layer
  takes the snare back down (R3 25, R3b 50) and is heard 16/16. A 21–85-ms frame smears
  the wires' rattle and the head's decay that model 5's band filters, with 0.5–5-ms early
  breakpoints, keep in time. What the snare still misses (measured): the attack's *timing*
  — the strongest 2–16 kHz arrival, the 1.5-ms spike, the first 3 ms — which a
  minimum-phase grain cannot hold, and the wires' fine-time rattle.
- **Open hat: poor everywhere (best 50, the oracle)** — this item carried the render's
  3-s event the hardest (the +17–19 dB swell); it must be heard again once the fit is clean
  before it says anything about the representation.
- **Crash and ride:** the hidden reference was missed (44, 70); R1, R3b and R3o rank high,
  R2–R4 low, but the item is too uncertain to separate the attack's effect on cymbals.

### Model 6, per element (replaces the measures-only recommendation below)

| element | noise layer | attack | other | evidence |
|---|---|---|---|---|
| kick | R3: STFT 1024/256, 110 × 40 table | model 5's knock (R2 not needed) | — | 100 for every table rung |
| snare, rimshot, sidestick, toms | **model 5's 38 band filters** (time-fine), monotone tails | **R2's five minimum-phase grains** per piece/zone/velocity | next: a time-resolved attack (per-layer onset phases above 1.5 kHz in the first 10 ms, as `early.py` fits below it) and a wire-rattle event stream gated by the snare-side head; ABX snare R2 vs R0 to measure it | snare 24 → 64, floor tom 70 → 80; STFT layers 16/16 |
| closed, half, open hat | **R3b**: STFT 4096/1024 per bin (2049 × 10–40 times) | R2's grains (the chick) | clean fits (no render event), monotone tails; plate chatter only after a clean re-listen | closed hat 100 |
| crash, ride, bell | **R3b** beside the modes (the cut probe: the modes stay) | model 5's stick until an ABX of R1 against R2 settles the grains on cymbals | monotone tails, the clean fit | mean 73 (crash), weak item |
| all | — | — | the swell fix in model 5's fit and blob first: cheap, and every listen so far heard it | 40 of 128 layers |

Cost against model 5 (the model alone, quiet machine, 7.9 %): drums unchanged (bands kept,
grains free), the kick's bank → one 1024-point IFFT a hop (−0.4 %), the hat's and the
cymbals' banks → one 4096-point IFFT a hop each (≈ the same flops as a fully sounding bank,
−0.3 % each): **≈ 6.5–7 %**. Tables: R3b ≈ 82 KB a layer at 40 times (≈ 20 KB at 10) for the
≈ 36 hat and cymbal layers of a kit — 0.7–3 MB a kit; the grains' magnitudes 33 KB a kit.

**Next listen:** model 5 with the swell fixed (fit and blob), against the same recordings,
then the kit again with R2 for the drums and R3b for hats and cymbals built on the clean fits.

## Results in one page (the measures, before the listen)

1. **Co-modulation is not what separates the cymbals.** With the shared decay taken out,
   the recorded crash, ride and bell read **≈ 0** cross-band envelope correlation in both
   kits (−0.03 … +0.04, at 5–40 Hz and at 40–150 Hz) — exactly what model 5 reads. The
   strategies study expected real cymbals to co-modulate and model 5 not to; the
   recordings do not. The excess is where the analysis heard *structural* cues: the
   **hats** (closed: +0.17/+0.31/+0.24 in three of four hits against ≈ 0.0–0.06 for model 5;
   DRSKit open and half-open +0.06…+0.12) and the **rimshot** (DRSKit +0.37/+0.75,
   Virtuosity v80 +0.21) — the plates' and the wires' chatter. R4's latent modulators
   reach them (open hat 2/4 → 4/4 hits, rimshot noise group 43 → 57 %).
2. **Cymbals and hats need fine frequency resolution, not texture statistics.** At a
   4096-point resolution a cymbal's wash is many *stable* narrow components (stability
   0.45–0.65). A 1/12-octave table (R3) smooths them into noise and is *worse* than model
   5's modes alone on the fine-structure measures (crash 38 → 25 %, open hat 38 → 21 %);
   partial widths (R4) do not help (25 %). The same residual held **per 11.7-Hz bin**
   (R3b: 2049 bins × 40 times, random phase) brings them to the recording's values: fine
   structure crash 67 %, open hat 71 %, half-open 62 %, closed 69 %, ride 62 % (DRSKit crash
   stability 0.56 against R0's 0.51, R3 0.34; half-open tail components 15.7 against 15.8,
   R3 30.7). Phase coherence is not it: the same table with each bin's phase advanced
   coherently overshoots into too tonal (flatness 0.07 against 0.18, peaks 16.5 against
   9.5/kHz). And the modes cannot go: dropping a cymbal's modes above 4–8 kHz collapses
   peaks and stability even with the fine table (see *Cost*).
3. **Drums need the fine table in time, not in frequency.** R3 (1024/256, 1/12 octave ×
   40 times) is the drums' rung — snare 39 → 59 % (wires 1/4 → 4/4 hits, 1–4 kHz at
   500 ms 1/4 → 4/4, cepstral comb 1/4 → 4/4, crest 2/4 → 4/4), rimshot 19 → 41 %; R3b's
   85-ms frames smear a drum's decay (mid tom 48 → 29 %).
4. **The magnitude-only attack helps where the attack is a ping or a chick, not where it
   is timed.** R2's minimum-phase grains raise the attack measures of the open hat
   (12 → 75 %), ride (38 → 75 %), bell (25 → 50 %), kick (25 → 50 %); they do not reach the
   strongest-arrival time, the first-1.5-ms spike or the first-3-ms centroid on the snare,
   toms or crash — not even under the oracle R3o. A minimum-phase grain pulls its window's
   energy to its start; the recordings' arrivals 1.5–6 ms after the first sound (through
   their microphones) are time structure. That is the study's "R2 fails" branch for the
   snare and crash attacks, *if* listeners hear it — the ABX decides.
5. **The measures' own ceiling.** The hits files hold one take per velocity, so the
   tolerances are fixed, not the recording's spread. The same stochastic rung rendered
   with other seeds meets *itself* on only 56–72 % of the cells for drums and the closed
   hat and 83–93 % for cymbals and the open hat (`ladder_seeds.py`). So the drums' best
   rungs (50–64 %) already sit at the measures' ceiling — only ears can rank them — while
   the cymbals' best (R3b, 62–70 %) are still short of theirs.

**Which rung each element needs (measures; the listening test is the judge):**

| element | rung | what it fixes | what no rung reaches |
|---|---|---|---|
| kick | R2 + R3 | the beater's spectrum (attack 25 → 50 %), the body's decay | 20–55 Hz in the first 80 ms, the jazz kick's late arrival |
| snare, rimshot | R2 + **R3** (+ R4's latent modulators for the rimshot) | wires' level and decay, comb, crest, 1–4 kHz ring | the attack's timing (arrival, spike, first 3 ms) |
| sidestick, toms | R3 (small gains; at the measures' ceiling) | the decay | the attack's timing |
| hats (closed, half, open) | R2 + **R3b** + R4's modulators | chick, fine structure, plate co-modulation | first-3-ms centroid |
| ride, bell, crash | R2 + **R3b** | ping, wash fine structure, tails | 8–16 kHz at 400 ms, tail components (crash) |

**Can pure generation reach "indistinguishable"?** *Snare: plausibly* — R3 meets the
measures as often as a re-seeded copy of itself does; what is left is the attack's
timing, which a time-resolved attack (a short-frame layer or fitted phases above 1.5 kHz)
can carry if the ABX says it is heard. *Cymbals: not yet shown* — R3b closes most of the
fine-structure gap a smoothed table leaves, but it is a per-bin magnitude table (closer to
a spectral sample of the hit than to a model with few parameters), and even it stays short
of its ceiling on the attack and the tails. The listening kit puts R3, R3b and R4 against
the recording blind.

**Model 6, as the measures alone read it** (superseded by the owner's listening above): keep model 5's modes (they carry the stable partials;
cutting them fails); replace the 38-band bank by a hit-anchored random-phase STFT layer
with **per-piece resolution** — drums 1024/256 with the 110 × 40 table, cymbals and hats
4096/1024 per bin (2049 × 10–40 times); the five-grain minimum-phase attack per piece,
zone and velocity (magnitudes stored, pulses built at load); the latent modulators only
for the hats and the rimshot's wires. Drop partial widths (no measured gain, +30–60 % on
the modal bank) and the burst streams (the fit chose them in 3 of 48 hits). Estimated cost
**≈ 5–5.5 % of a core for the model alone against model 5's 7.9 %**; tables ≈ 0.3 MB per
kit with R3, ≈ 1.5–5 MB per kit with R3b.

**Listening kit:** `compartir/tympanum-escalera/` (26 MB, `index.html`, Spanish first) —
seven MUSHRA items (DRSKit snare, kick, closed hat and crash at v127; Virtuosity floor tom
and open hat at v127, ride at v80): reference, hidden reference R0, R1, R2, R3, R3b, R3o, R4
and the anchor R5, in an order drawn per listener; then five ABX tests of 16 trials —
snare R3 and R4, crash R3, R4 and R3b, each against the recording. The key is in
`compartir/tympanum-escalera-key/` (`clave.txt`, `clave.json`);
`ladder_kit.py score <key dir> <results.json>` reads a listener's file.

## Tables

**Share of (measure × hit) cells within tolerance of R0** (four hits per articulation;
the rungs R1–R4 build on each other, R3b and R3o are R2 plus their own noise layer):

| articulation | R1 | R2 | R3 | R4 | R3b | R3o | R5 | measures never met by R4 |
|---|---|---|---|---|---|---|---|
| crash | 53 % | 52 % | 53 % | 55 % | 68 % | 63 % | 27 % | fl816, c3, hf3, flat, peaks, eff, stab, at400, tail_flat, tail_eff |
| hat_closed | 56 % | 58 % | 46 % | 60 % | 62 % | 58 % | 21 % | xc, xc_fast, kurt, c3, hf3, flat, peaks, eff, stab |
| hat_half | 38 % | 40 % | 47 % | 43 % | 63 % | 57 % | 22 % | xc, xc_fast, fl816, c3, hf3, flat, peaks, eff, stab, fl48, at400, T8_16, tail_flat, tail_eff |
| hat_open | 50 % | 57 % | 48 % | 55 % | 70 % | 67 % | 22 % | fl816, c3, hf3, flat, peaks, eff, stab, at400, T8_16, tail_flat, tail_eff |
| kick | 40 % | 44 % | 50 % | 50 % | 50 % | 40 % | 38 % | xc, xc_fast, fl816, kurt, c3, arrival, spike, cep, c15_60, sub, hi500, T_low |
| ride | 58 % | 63 % | 62 % | 63 % | 70 % | 70 % | 25 % | c3, hf3, flat, peaks, eff, stab, at400, T8_16, tail_flat, tail_eff |
| ride_bell | 45 % | 52 % | 52 % | 52 % | 67 % | 57 % | 32 % | fl816, kurt, c3, hf3, flat, peaks, eff, stab, at400, T8_16, tail_flat, tail_eff |
| rimshot | 19 % | 23 % | 41 % | 47 % | 39 % | 52 % | 22 % | xc, xc_fast, fl816, kurt, c3, arrival, spike, crest, cep, c15_60, sub, hi500, T_low, wires, flat412, grain |
| sidestick | 38 % | 40 % | 40 % | 40 % | 35 % | 38 % | 29 % | xc, xc_fast, c3, arrival, spike, crest, cep, c15_60, sub, hi500, T_low |
| snare | 39 % | 39 % | 59 % | 59 % | 52 % | 64 % | 36 % | xc, xc_fast, kurt, c3, arrival, spike, c15_60, sub, flat412, grain |
| tom_floor | 42 % | 38 % | 42 % | 42 % | 46 % | 38 % | 38 % | xc, xc_fast, fl816, kurt, c3, arrival, spike, crest, cep, c15_60, sub, hi500 |
| tom_mid | 46 % | 46 % | 48 % | 48 % | 29 % | 52 % | 40 % | xc, xc_fast, fl816, kurt, c3, arrival, spike, cep, c15_60, sub, hi500 |

**By element and group of targets** (attack: arrival, spike, first-3-ms centroid, 6–20 vs
0.2–1.5 kHz, crest, cepstral comb; body: 1–4 kHz at 500 ms, low band and 8–16 kHz decay,
20–55 Hz, 15–60 ms centroid, 8–16 kHz at 400 ms; noise: wires, 4–12 kHz flatness, wire
grain, 4–8 and 8–16 kHz fluctuation, kurtosis, co-modulation; fine: flatness, peaks/kHz,
effective components, stability, tail flatness and components — % of cells met):

| element | group | R1 | R2 | R3 | R3b | R4 | R3o |
|---|---|---|---|---|---|---|---|
| kick | attack | 25 | 50 | 45 | 40 | 45 | 45 |
| kick | body | 56 | 38 | 62 | 88 | 62 | 44 |
| kick | noise | 44 | 44 | 44 | 25 | 44 | 31 |
| snare | attack | 20 | 25 | 45 | 35 | 45 | 50 |
| snare | body | 25 | 19 | 56 | 50 | 56 | 44 |
| snare | noise | 61 | 61 | 71 | 64 | 71 | 86 |
| rimshot | attack | 15 | 35 | 35 | 40 | 35 | 40 |
| rimshot | body | 12 | 6 | 44 | 44 | 44 | 44 |
| rimshot | noise | 25 | 25 | 43 | 36 | 57 | 64 |
| sidestick | attack | 15 | 25 | 30 | 25 | 30 | 30 |
| sidestick | body | 19 | 12 | 31 | 31 | 31 | 25 |
| sidestick | noise | 88 | 88 | 62 | 50 | 62 | 62 |
| tom_mid | attack | 40 | 35 | 30 | 20 | 30 | 35 |
| tom_mid | body | 31 | 38 | 56 | 38 | 56 | 62 |
| tom_mid | noise | 69 | 69 | 62 | 31 | 62 | 62 |
| tom_floor | attack | 25 | 10 | 25 | 25 | 25 | 15 |
| tom_floor | body | 38 | 44 | 56 | 69 | 56 | 62 |
| tom_floor | noise | 69 | 69 | 50 | 50 | 50 | 44 |
| hat_closed | attack | 38 | 50 | 38 | 50 | 38 | 50 |
| hat_closed | body | 75 | 75 | 88 | 50 | 100 | 88 |
| hat_closed | noise | 70 | 70 | 65 | 65 | 80 | 50 |
| hat_closed | fine | 38 | 38 | 6 | 69 | 25 | 56 |
| hat_half | attack | 38 | 50 | 50 | 50 | 50 | 50 |
| hat_half | body | 38 | 38 | 50 | 38 | 50 | 38 |
| hat_half | noise | 65 | 65 | 75 | 80 | 70 | 95 |
| hat_half | fine | 17 | 17 | 21 | 62 | 17 | 33 |
| hat_open | attack | 12 | 75 | 50 | 62 | 50 | 38 |
| hat_open | body | 62 | 50 | 62 | 50 | 50 | 62 |
| hat_open | noise | 75 | 75 | 75 | 80 | 95 | 95 |
| hat_open | fine | 38 | 38 | 21 | 71 | 25 | 54 |
| ride | attack | 38 | 75 | 62 | 62 | 62 | 88 |
| ride | body | 38 | 38 | 25 | 25 | 38 | 25 |
| ride | noise | 95 | 95 | 100 | 100 | 100 | 100 |
| ride | fine | 42 | 42 | 42 | 62 | 42 | 54 |
| ride_bell | attack | 25 | 50 | 12 | 88 | 25 | 12 |
| ride_bell | body | 0 | 25 | 25 | 25 | 25 | 12 |
| ride_bell | noise | 95 | 95 | 95 | 90 | 90 | 100 |
| ride_bell | fine | 25 | 25 | 38 | 54 | 38 | 50 |
| crash | attack | 25 | 12 | 25 | 12 | 25 | 38 |
| crash | body | 50 | 50 | 75 | 62 | 75 | 75 |
| crash | noise | 85 | 85 | 90 | 95 | 95 | 95 |
| crash | fine | 38 | 38 | 25 | 67 | 25 | 42 |

The cross-band correlation per hit (the recording, model 5 as the engine renders it through
its twelve mics, and every rung) and every per-articulation table (each measure, R0's value
per hit, hits met per rung, the first rung from which every later one meets it) are in
`tmp/ladder/measures.md`, regenerated by `ladder_measure.py <dir> report`.

## Reproducing

    P="downloads/chorda/venv/Scripts/python.exe -I"
    $P tools/tympanum-fit/ladder.py tmp/ladder               # 48 hits x R0-R5, R3o (~10 min, 6 jobs)
    $P tools/tympanum-fit/ladder_measure.py tmp/ladder       # the measures (~40 min) and measures.md
    $P tools/tympanum-fit/ladder_dense.py tmp/ladder all     # R3b for every hit, into measures.json
    $P tools/tympanum-fit/ladder_measure.py tmp/ladder report
    $P tools/tympanum-fit/ladder_cut.py tmp/ladder           # the modes-cut probe
    $P tools/tympanum-fit/ladder_dense.py tmp/ladder         # R3b against R3c (coherent phase)
    $P tools/tympanum-fit/ladder_seeds.py                    # the measures' noise ceiling
    $P tools/tympanum-fit/ladder_whistle.py swell tmp/ladder compartir/tympanum-b9   # the tails that climb (also: glides, warble, lone, blips, excess)
    $P tools/tympanum-fit/ladder_kit.py build tmp/ladder compartir/tympanum-escalera

Every rung is seeded from the hit's name: the same files every run.

What was verified only off line: everything here. No rung ran in the engine; the cost
figures are arithmetic against model 5's measured split, not a Rust benchmark; the kit's
page was driven end to end in Edge (390 and 1000 px, every clip decoding), not yet heard
by anyone.

## Method

**Material.** The recorded hits as the engine's sampler renders them through each kit's
microphone mix (`tmp/meas9`, the same files every fit and analysis used): DRSKit (studio,
CC BY 4.0) and Virtuosity Drums (jazz, CC0); the half-open hat at CC4 0.5
(`meas11/o50`, `dg/vo50`). Twelve articulations — kick, snare (centre), sidestick, rimshot,
mid tom, floor tom, closed / half-open / open hat, ride bow, ride bell, crash — at **v80
and v127**, both kits: 48 hits. (The brief asked for v55; the hits files hold
40/80/110/127. v80 was taken over v40 because several v40 hits sit only 20–25 dB above the
recordings' noise floor, which the residual rungs would carry and R1 would not.) Mono: the
two channels averaged, as every measure in the analyses does; every rung is mono too.

**The rungs** (per hit; every random draw seeded from the hit's name and the part):

| rung | description | what it tests |
|---|---|---|
| R0 | the recording | — |
| R1 | model 5's own fitted description of that hit: its modes (drums with their fitted onset phases and early parts), 38 quarter-octave noise bands at 22 breakpoints, the generic stick — the fits the engine's tables were built from (`fit9`/`fitv3`, the ride refitted at 440 partials; each fit's level equals its hit's to 0.01 dB), rendered as `synth.py` renders them | the current representation's ceiling |
| R2 | R1 with the stick replaced by the hit's own attack: five **minimum-phase grains** (0–1.5, 1.5–3, 3–6, 6–10, 10–20 ms), each with the energy spectrum the recording has in its window beyond the modes (subtracted as a waveform where the fitted onset phases hold, in power elsewhere — per bin the lesser), the noise bands faded in over 10–20 ms and the grains before it; above ~300 Hz | does a magnitude-only, phase-free attack carry the crack, beater and ping? |
| R3 | R2 with the noise as a **fine table**: the residual (the recording's STFT 1024/256 less the modes' and the attack's power) averaged into 110 bands of 1/12 octave (35 Hz–21 kHz) × 40 log-spaced times (0.5 ms–4 s), read back per frame and bin, random phase | what a run-time table can hold |
| R3b | R2 with the residual held **per bin** at 4096/1024 (11.7 Hz × 40 log times), random phase (`ladder_dense.py`) — added after R3 failed the cymbals' fine structure | does frequency resolution carry the wash? |
| R3o | the **oracle** of phase-free noise: the residual's own STFT magnitude per frame and bin, random phase | the ceiling of any magnitude-only noise layer (not a description a table can hold: it is the hit's magnitude spectrogram) |
| R4 | R3 plus texture statistics: K ≤ 6 **latent modulators** (eigenvectors of the recording's excess covariance of 1/3-octave log-envelope deviations over R3's, each driven by a seeded Gaussian process with its measured modulation spectrum), **partial widths** for cymbals and hats (each mode's measured −3 dB width beyond what its window and decay give a pure decaying sinusoid, as a slow seeded FM wobble mixed from four shared noises), and a **PhISEM-style burst stream** for wires and hat plates (Poisson events following the noise's envelope, 0.7-ms decays, shaped to the table's spectrum); three scalars per hit chosen against the hit's own co-modulation, fluctuation and envelope kurtosis | is texture beyond the smooth table audible, and can statistics make it? |
| R5 | anchor: R1 low-passed at 3.5 kHz | the bottom of the scale |

The brief's R3 ("STFT magnitude with random phase, per frame/bin") is split in two on
purpose: per frame and bin it already contains the recording's own envelope fluctuations up
to ~45 Hz — the very co-modulation R4 is meant to add — and it is a spectrogram of the hit,
not a description a run-time model can store or vary. R3 is the table a model 6 can hold;
R3o tells whether *any* phase-free noise layer can reach the recording.

Three implementation lessons, each found by measuring a rung against R0 before trusting it:
a single minimum-phase pulse for the whole first 20 ms puts a crash's 20-ms build into its
first millisecond (hence the five grains); cut short, a pulse with a drum's low end smears
broadband (+30 dB at 16 kHz on the kick) — hence the grains above ~300 Hz and the segments
high-passed before analysis; and a random-phase STFT layer re-analyses ~0.8 dB hot on a steep
decay unless a last broadband gain per frame corrects it.

**Measures** (`ladder_measure.py`): the analyst's battery (`analysis-drums-2/scripts`
s04, s09, s12, s14, s15; `metrics.py`) — drums: strongest 2–16 kHz arrival after the first
sound, the 3-kHz+ spike in the first 1.5 ms, crest of the first 50 ms, the cepstral comb z,
first-3-ms and 15–60-ms centroids, 20–55 Hz in the first 80 ms, 1–4 kHz at 500 ms, the low
band's decay; wires: 4–12 kHz vs 100–400 Hz, 4–12 kHz flatness, wire grain; cymbals and
hats: first-3-ms centroid and 6–20 vs 0.2–1.5 kHz, 4–12 kHz flatness and peaks/kHz, 6–16 kHz
effective components, fine-structure stability, 4–8 and 8–16 kHz envelope fluctuation,
8–16 kHz at 400 ms, its decay, and the tail's flatness and components. A rung **meets** a
measure when it is within a tolerance of R0's own value (from ±0.04 in flatness to ±2 dB in
level measures and ±15–20 % in times and counts; `TOL` in the script).

**The new measure: cross-band envelope correlation.** 1/3-octave bands 2–16 kHz; each
band's Hilbert log-envelope (800 Hz), less a cubic trend in log time over the texture window
(the decay every band shares is not co-modulation), band-passed to 5–40 Hz (and 40–150 Hz);
the mean correlation of non-adjacent pairs. Windows: drums 40–300 ms (after the attack,
which has its own measures), closed hat 20–150 ms, half-open 20–800 ms, open hat 50 ms–1.5 s,
cymbals 0.1–1.5 s. Two first drafts were misleading and are worth recording: without the
trend removal every drum and the jazz cymbals read 0.5–1.0, the recordings included (the
shared decay leaking through the band-pass); with the window starting at 10 ms the same
snare read 0.68 or 0.25 depending only on the clip's edges.

## Cost and size of a "model 6" built from the passing rungs (estimate, ours)

Flop counts per sounding object at 48 kHz, converted with model 5's measured split (quiet
machine, `NEXT-drums.md`: the model alone 7.9 % of a core for a busy full kit — hat 1.9,
crash 2.0, ride 1.8, drums 0.5–0.7 each; on the busy machine after the ninth listen 18.5 %).
Nothing here is measured in Rust; it is arithmetic to rank the options.

| layer | model 5 | model 6 (R3 / R3b) | model 6 + R4 |
|---|---|---|---|
| noise | 38 quarter-octave bands × 2 band-pass sections ≈ 700 flops/sample ≈ 34 MFLOP/s with every band sounding | **R3**: a 1024-point real IFFT every 256 samples (~26 k flops), 513 random phases, 110-band × 40-time table read per frame, window and overlap-add ≈ 35 k flops/hop ≈ 6.5 MFLOP/s — **~5× cheaper**. **R3b**: a 4096-point IFFT every 1024 samples (~125 k flops) + 2049 bins of magnitude and phase ≈ 140 k flops/hop ≈ 6.6 MFLOP/s — the same cost, 16× the frequency resolution | + K = 6 latent drivers at frame rate and 6 × 24 loadings: < 0.1 MFLOP/s; burst stream ≈ 1 MFLOP/s |
| attack | 0.4-ms stick + 2-ms knock (free) | five minimum-phase grains per piece/zone/velocity (≈ 40 ms of pulse, one multiply-add a sample while it plays; built at kit load from a magnitude table) — free at run time | — |
| modes | 48 (drums) – 512 (ride) phasors ≈ 8 flops/mode/sample | unchanged (the cut probe below: they cannot go) | partial widths: +4–6 flops/mode/sample, **+30–60 % of the modal bank** (crash +55, ride +120 MFLOP/s) |
| latency, determinism | none; bit-identical | frames anchored to the object's own sample counter, computed when it reaches a frame start, overlap-added: no look-ahead, any block size gives the same bits; Aula's FFT (ADR 0112); seeded phases | same |

**CPU, the model alone (quiet machine, estimate):** model 5 7.9 %. Model 6 at R3/R3b: drums
~0.15–0.25 % each instead of 0.5–0.7 (their bank was most of their cost), cymbals −0.3 %
each → **≈ 5–5.5 %**. With R4's partial widths on the cymbals and hats: **≈ 6.5–7 %** — still
under model 5. The R3b layer's one real cost is latency-free scheduling: a 4096-point IFFT
lands in one block every 1024 samples (≈ 125 k flops at once per sounding object; a busy kit
of six objects ≈ 0.75 M flops in one 128-sample block): spread the frame's work over its hop
(the FFT's stages in slices) or keep a 2048/512 compromise.

**Cut probe** (`ladder_cut.py`, §3.1f of the strategies — "with a fine noise layer the
cymbals' modes above a texture frequency can go"): **refuted at table resolution.** R3 with
the modes above 4, 6 or 8 kHz dropped (the residual table then holding what they did):

| hit | R3 (all modes) | cut 8 kHz | cut 6 kHz | cut 4 kHz |
|---|---|---|---|---|
| DRSKit crash v127: peaks/kHz (R0 9.5) · stability (R0 0.51) | 6.0 · 0.34 (215 modes) | 3.8 · 0.12 (130) | 1.6 · 0.08 (99) | 0 · 0.02 (59) |
| DRSKit ride v127 (R0 15.4 · 0.51) | 12.9 · 0.49 (439) | 6.6 · 0.18 (238) | 3.5 · 0.11 (178) | 0 · 0.01 (120) |
| Virtuosity ride v80 (R0 19.8 · 0.51) | 13.3 · 0.57 (440) | 9.5 · 0.43 (288) | 4.5 · 0.14 (233) | 0.1 · 0.02 (148) |
| DRSKit open hat v127 (R0 8.8 · 0.45) | 3.1 · 0.28 (140) | 2.3 · 0.12 (85) | 1.1 · 0.07 (61) | 0 · 0.02 (41) |

A 1/12-octave table cannot carry a cymbal's fine spectral structure; its modes (or a per-bin
table, R3b) must. The modal bank's cost stays.

**Table sizes** (u8 at 0.5 dB):

| item | per layer | per kit (≈ 13 pieces × 4 layers + hat states ≈ 60 layers) |
|---|---|---|
| model 5's band envelopes (38 × 22) | 0.8 KB | ≈ 50 KB (its whole blob, modes included, is 552 KB for both kits) |
| R3 table (110 × 40) | 4.4 KB | 264 KB |
| R3b table (2049 bins × 40 times) | 82 KB | ≈ 4.9 MB — or ≈ 1.2 MB with 10 times (the per-bin shape changes slowly; the 40-time envelope can stay at 1/12 octave) |
| attack grains (5 × 110-band magnitudes; pulses built at load, 3 variants) | 0.55 KB | 33 KB stored, ≈ 1.4 MB of pulses in memory |
| R4 statistics (6 × 24 loadings, 6 modulation spectra, 3 scalars) + partial widths (1 byte/mode) | ≈ 0.7 KB | ≈ 40 KB |
