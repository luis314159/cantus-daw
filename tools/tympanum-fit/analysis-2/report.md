# Tympanum b9 — element-by-element gap analysis (independent analyst, 2026-11-13)

> Kept in the repository with its scripts (this folder; `README.md`, *The two analyses*). The
> paths below are the round's: the listening folder is `TYMP_LISTEN`, the recorded kits' hit files
> `TYMP_MEAS`, the figures and CSVs land in `TYMP_ANALYSIS_OUT` (the figures are not kept: the
> scripts make them again).

Saved by the integrator from the analyst's hand-back (the analyst could not write report files).
Scripts s00–s16 and outputs in `scripts\`, figures in `figures\` (`<kit>_<element>.png`, `groove_*.png`,
`key_attack_smear.png`, `key_cymbal_tails.png`). gen = `-2-nuevo`, rec = `-3-grabado` (DRSKit studio,
Virtuosity jazz); cross-check against five recorded kits (DRS, Crocell, Muldjord, Big Rusty, Virtuosity,
`tmp\meas9`).

Already matched: levels per hit and velocity curves (±1.5 dB), hit-to-hit variation (no machine-gun).
The gaps are timing and texture.

## Ranked: the 15 most "synthetic" cues (targets)
1. Late, smeared attack on drums and ride: −15…−25 dB precursor then the crack 3–4.5 ms later; ~3 ms comb
   (cepstral z 5.5–9.7 vs 2.8–4.6). Target: strongest 2–16 kHz arrival ≤ 1.5 ms after first sound; 3 kHz+
   in first 1.5 ms ≥ −5 dB re the 2–50 ms max; cepstral z ≤ 4.5. Likely the virtual mics (close mic's
   highs cut by head_beam as off-axis; overheads carry the crack ~3 ms later; identical-source copies comb).
2. Cymbal tails turn to hiss (crash, ride, jazz open hat, groove ring-out). Target at 2–3.3 s: flatness
   ≤ 0.06, ≤ 5 comps/kHz, stability ≥ 0.45, 8–16 kHz no louder than rec (jazz crash −48 dB). Cause: noise
   bands fitted to band power including the partials; refit on the residual after removing partials;
   noise share of 4–16 kHz from ~50 % at 0.3 s to < 10 % by 1.5 s.
3. Snare wires weak and tonal: 4–12 kHz vs body ≥ −12 dB studio / ≥ −7 jazz; flatness ≥ 0.38; centroid
   0–100 ms ≥ 0.45 kHz studio / ≥ 1.0 jazz. Strongest 2–16 kHz arrival 4.3 ms vs 1.0.
4. Sub-sonic thump on stick hits: 20–55 Hz in 0–80 ms ≤ −40 dB re peak (now −10…−35); high-pass the knock,
   cut the early part below 0.8·f0.
5. Upper modes of toms/kick ring too long: 1–4 kHz at 500 ms ≤ −50 dB; T60 1–4 kHz ≤ 0.5 s studio / ≤ 1 s
   jazz (now 1.2–2.1 s); floor tom 250–1000 Hz at 500 ms ≤ −35 dB.
6. Kick click too strong/ringing: −23 dB studio v127, −42…−48 jazz (felt), single pulse, no 1–4 kHz ring.
7. Closed/half-closed hat too tonal and frozen: flatness ≥ 0.40, stability ≤ 0.20 incl. groove bar 1.
   Partial share reachable; plate chatter structural.
8. Ride: studio loud strokes 8–16 kHz T60 ≤ 2.5 s (now 5.5–5.9), stability ≥ 0.5; jazz ≥ 15 peaks/kHz,
   flatness ≤ 0.07; studio ping 3 ms after a precursor (DRS 0.4 ms); tail 9–12 kHz hump 10–15 dB.
9. Crash attack dull: first-3-ms centroid ≥ 4.5 kHz (≥ 5.5 at v127); 6–20 vs 0.2–1.5 kHz in first 3 ms
   ≥ −2 dB (v127 now −9.7).
10. Weak first push on toms/snare: punch ≥ +1.5 dB toms, ≥ 0 snare (largely item 1).
11. Open hat too smooth: 8–16 kHz fluctuation ≥ 1.6 dB, ≥ 8 peaks/kHz — structural (plate collision AM /
    chaotic coupling). Jazz tail 7–14 dB too loud at 0.8–2.4 s.
12. Isolated stick spikes: crest within ±2 dB (jazz ride +5…+7, jazz crash v40 +7.6).
13. Studio kick pitch: dominant 76 Hz rising with velocity, no 49–55 Hz line (now 69 Hz + 50 Hz partial).
14. Tonal highs on kick and soft toms: 3–8 kHz flatness ≥ 0.40 (now 0.21–0.38): modes above 3 kHz → noise.
15. Groove image/balance: mid-band L/R corr 0.87 vs DRS 0.67; jazz 4–8 kHz down 7–12 dB; groove sub-bass
    30–60 Hz +7…+12 dB on hits; per kit targets.

Recording artefacts (not targets): DRS noise floor −85…−90 dB 4–20 kHz; truncated DRS samples; DRS's hat
sizzle plateau and long soft-kick sustain; Virtuosity's spaced stereo and dark felt kick; 3–6 ms pre-roll
on jazz recordings. Tom glide and stroke clusters are low-confidence measures.

Verdict: almost all reachable in the current model; only plate chatter (item 11, part of 7) needs a new
technique. Per-element tables: see the analyst's hand-back in the round's transcript (kick, snare, toms,
closed/open hat, ride, crash, groove) — summarised in the ranked list above.
