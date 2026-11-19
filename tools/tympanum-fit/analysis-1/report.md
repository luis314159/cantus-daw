# Tympanum modelled "Studio" kit vs recorded kits — why the cymbals sound glassy

> Kept in the repository with its scripts (this folder; `README.md`, *The two analyses*). The
> paths below are the round's: the listening folder is `TYMP_LISTEN`, the recorded kits' hit files
> `TYMP_MEAS`, the figures and CSVs land in `TYMP_ANALYSIS_OUT` (the figures are not kept: the
> scripts make them again).

Independent analysis, round 40, 2026-11-13 (audio only; saved by the integrator from the analyst's
hand-back). Scripts `scripts\` (s01–s17, `s*_out.txt`, `cymbal_metrics.json`), figures `figures\`.
Material `compartir\tympanum-b3`; recorded = median of DRS, Crocell, Muldjord, Big Rusty, Virtuosity
(`tmp\meas9` hit files) at v110 unless named. Caveats: `golpes` files are mono (stereo judged on the
grooves); recorded cymbals are multi-mic mixes (low end includes bleed/room).

## Core finding
Each modelled cymbal is a handful of loud, pure, static partials in silence; dark attack; highs ringing as
long as the mids (centroid rising in the tail) — a struck glass/bell. Groove 6–16 kHz flatness 0.07 vs
0.33–0.36, effective components 4.3 vs 31–35 per kHz — treble made of tones instead of noise. These
measures did not move between b2 and b3.

## Ranked differences and targets
1. **Few pure partials carry each cymbal.** Top-3 partials' share of energy > 1 kHz: crash 54 % vs 11.6 %,
   ride 54 vs 16.3, closed hat 30 vs 11, pedal hat 46 vs 8. Crash 2804 Hz holds 34.8 % (+51 dB), ride
   2243 Hz 37.1 % (+53 dB), 3687 Hz 16.8 %; no DRS partial > 5.3 %. Peak height over 300-Hz median (4–12 kHz):
   crash 37 vs 18.6 dB, ride 40 vs 28, closed hat 27 vs 12. **Target**: any single partial > 1 kHz ≤ 3–5 %,
   top-3 ≤ 12–16 %, peaks ≤ ~20 dB (crash) / 28 dB (ride) over the local median (0.5-s window).
2. **Empty space between partials.** Flatness 4–12 kHz: crash 0.0021 vs 0.119; ride 0.0021 vs 0.027; open
   hat 0.0026 vs 0.20; closed hat 0.031 vs 0.425; pedal hat 0.032 vs 0.47. Components/kHz: crash 3.5 vs 26,
   ride 2.7 vs 12.8, open hat 2.3 vs 37, closed hat 4.3 vs 25. **Target**: crash flatness ≥ 0.06 (0.12),
   ≥ 16 comp/kHz (26); ride ≥ 0.02, ≥ 12; closed/pedal hats ≥ 0.35 (0.43–0.47), ≥ 20 — essentially shaped
   noise. If modes: ≥ 50 modes/kHz above 3 kHz, Rayleigh amplitudes, random per-hit phases, few-Hz
   bandwidths; otherwise a noise floor carrying 30–60 % of HF energy.
3. **Highs ring as long as mids.** T60 8–16 kHz / 0.5–1 kHz: ride 1.19 vs 0.19, crash 1.0 vs 0.42, bell 1.17
   vs 0.11, open hat 0.94 vs 0.59. Ride 8–16 kHz T60 5.85 vs 1.82 s; at 400 ms −4.7 vs −28.5 dB; bell −4.4 vs
   −33. Centroid rises (3 → 4.5 kHz) where real falls (DRS ride 6.5 → 2 kHz). **Target**: ride ~8–10 s at
   0.5–1 kHz, 6–7 s at 3 kHz, 1.8 s (~33 dB/s) at 8–16 kHz, ≤ 0.5–1 s above 16 kHz; crash 5–6 s at 0.5–2 kHz,
   2.7 s (~22 dB/s) at 8–16 kHz; bell ~1.2 s at 8–16 kHz; centroid falling.
4. **No stick attack.** First-3-ms centroid: ride 1.55 vs 8.35 kHz, crash 1.7 vs 5.0, bell 1.5 vs 6.1, open
   hat 1.25 vs 6.2. 6–20 kHz re 0.2–1.5 kHz in first 3 ms: crash −19 vs −3 dB, ride −18.5 vs +1.4, bell −22 vs
   −6. Ride 8–16 kHz peak at 80–180 ms vs 2.5–4.7 ms (all kits); bell 160–170 vs 2.3–6.5 ms. **Target**:
   contact ≤ 0.3–0.5 ms, broadband to ≥ 12–15 kHz; first-3-ms centroid ≥ 5 kHz crash, ≥ 8 kHz ride/bell;
   6–20 kHz within ±3 dB of 0.2–1.5 kHz in first 3 ms; no low-mid pulse; ride/bell 8–16 kHz peak within
   5 ms and −25…−30 dB at 400 ms; crash: bright transient, then build-up (median 8–16 kHz peak ~130 ms,
   range 2.5–250), ~7 dB down at 400 ms.
5. **Frozen fine structure.** Stability (4–16 kHz log-spectrum residual correlation, frames 50 ms apart):
   crash 0.83 vs 0.54, ride 0.85 vs 0.58, open hat 0.81 vs 0.45, closed hat 0.75 vs 0.17, pedal hat 0.76 vs
   0.13. **Target** ≤ 0.55 crash/ride, ≤ 0.2 hats (nonlinear coupling, or random 5–40 Hz AM/FM of 2–4 dB on
   independent bands; noise synthesis has it built in).
6. **Cymbal stereo out of phase.** Groove crash tail L/R corr 250–1000 Hz −0.64…−0.66 vs +0.15…+0.44;
   2–4 kHz −0.57 vs +0.15…+0.22; mono loss 6–7 vs ~2 dB; groove 500–2000 Hz energy-weighted −0.15 vs +0.68
   (DRS). **Target** +0.2…+0.5 every band, never negative, mono loss ≤ 2.5 dB; overhead-pair level/time
   placement; no polarity/all-pass decorrelation on sparse partials.
7. **No cymbal body < ~450 Hz.** Energy < 450 Hz first 2 s: crash −39 vs −18.3 dB, ride −16.5 vs −9.4.
   **Target**: low modes 120–450 Hz with 5–12 s decays; ≈ −18 dB (crash) / −9 dB (ride) below 450 Hz; a
   little 80–300 Hz on hats.
8. **Bell reuses the ride's partials.** DRS bell: 1560 Hz and 1764 Hz hold 15 % and 19.5 %, T60 ~11 s; 8–16 kHz
   T60 ~1.2 vs 4.8 s. **Target**: 2–3 dominant tones ~1.5–1.8 kHz (15–20 % each, T60 ~10 s), top dies fast.
9. **Crash colour doesn't change with velocity.** Centroid 0–300 ms v40/80/110/127: 2.95/3.10/3.14/3.07 vs DRS
   2.20/3.75/4.80/5.11 kHz. **Target** ~2.2 → ~5 kHz (shorter contact on harder hits).
10. **Velocity → level too steep.** v40→v127 range: closed hat 31.7 vs median 20.3 dB; crash 29.2 vs 17.2;
    ride 25.3 vs 18.9; open hat 25.2 vs 19.1. v40 closed hat 13 dB under DRS. **Target** ~18–21 dB.
11. **Pedal hat starts bright and instant.** First-3-ms centroid 6.5 vs 0.46–1.9 kHz; high band peak 5–10 ms
    vs 20–45 ms (starting 15–21 dB below); flatness 0.03 vs 0.47. **Target**: dull contact < ~2 kHz, sizzle
    building over 20–40 ms, noise centred 7.5–9 kHz. Closed hat: DRS holds a dense sizzle plateau 0–50 ms; the
    model is −10 dB by 50 ms.
12. **Drums die into black.** Kick/snare at 600 ms −83…−91 vs −32…−46 dB (DRS); slope 300–600 ms ~145 vs
    6–47 dB/s; groove 100-ms loudness range 71.5 vs 43/30 dB. Real snare double slope (fast 100 ms, then a
    −40…−60 dB tail > 1 s). Every piece same 0.3–0.5 s T60 across 0.25–16 kHz. **Target**: room/sympathetic
    stage (kick/snare ≈ −35…−45 dB at 600 ms, ≤ 30–50 dB/s), decay per piece and band, sympathetic ringing.
13. **Kick and toms too short.** Kick 40–100 Hz T60 0.23 vs 1.5–2.8 s; at 300 ms −40…−48 vs −26…−30 dB; toms
    40–250 Hz T60 0.3–1.9 vs 3.7–7.3 s; tom glide v110–127 45–80 vs 180–207 cents; tom tuning 161/137/113 vs
    ~80–90/73 Hz; kick centroid flat 85–90 Hz vs DRS 89 → 120 Hz with velocity. **Target** kick T60 ~0.8–1.5 s,
    brighter attack at high velocity; toms 2–4 s low sustain, 150–200 cent glide at full force.
14. Minor: snare wires slightly tonal (flatness 0.37–0.40 vs 0.45–0.50) and short (−57 vs −47 dB at 300 ms);
    rimshot/sidestick 6–13 dB hot, rimshot centred ~1 kHz vs 0.5 kHz, lacks 1–1.6 s ring; drum bodies repeat
    (48 % of onsets with a twin > 0.95 corr vs 26–38 %); felt vs plastic too similar (plastic needs a much
    stronger 2–6 kHz click).

## Verdict
Tunable: items 3, 4, 6, 7, 9, 10. Structural: 1, 2, 5 — a sparse linear modal bank can't reach flatness ~0.1
and stability ~0.5. **Recommendation — hybrid built around noise-based spectral modelling fitted to the
recordings**: (1) wash = several independent noise sources in ~1/3-octave bands, each band's envelope fitted
per velocity, steeply falling HF T60, crash build-up, slow random AM; (2) tonal layer = a capped set of ~20–60
modes (0.15–3 kHz) for pitch, bell tones, body — no partial > 3–5 % of HF energy, random per-hit phases, slight
FM jitter; (3) stick = broadband ≤ 0.5 ms transient, brightness ∝ velocity, dominant on ride/bell, starting
the crash; (4) overhead-pair stereo with positive correlation. Dense nonlinear modal synthesis stays research.
Drums aren't glassy: add a room/sympathetic stage and longer low end.
