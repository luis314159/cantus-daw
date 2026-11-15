# Tympanum's modelled drums — handover (round 40, tymp-engine)

The owner on the seventh listen (2026-11-13): *"va mejor pero aún está lejísimos de la
grabación"*. This is an honest account of where model 5 stands, what it cannot reach as
built, and what a step change would take. Read with ADR 0124 (both amendments),
`docs/round40/tymp-engine.md`, `analysis-drums/report.md` and `tools/tympanum-fit/README.md`.

## Update — drums-gen 3 (2026-11-14, the owner away: measurable work only)

Details: `docs/round40/tymp-engine.md`, section *drums-gen 3*. The listen for later:
`compartir\tympanum-b11\` (NOTA.md in Spanish). Done:
- **Cost 18.7 → 7.3 % of a core** (every mic; model 5 alone 10.2 → 6.0 %), the render the same
  to −96 dB re peak (`dump_diff.py`). The cause was the release profile's thin LTO leaving the
  hot loops scalar: keep model 5's kernels in their own `#[inline(never)]` functions.
- **accept_b10 57 → 71 of 162**, none that passed fails: the bands' skirts (a tom's low bands
  high- and low-passed, a cymbal's high-passed), the studio kick's long fundamental, the
  studio snare's wires, the drums' upper partials and noise capped (`trim_b11.py`,
  `<TABLE>_DECAY`).
- The hybrid packs refitted as **version 3** (not uploaded; the integrator's, with the
  owner's OK).

**Next, in order:**
1. **The owner's ear on b11**: the studio kick's long fundamental (−24 dB, T60 2.5 s) and the
   snare's wires (+5/+9 dB) are measured toward DRSKit, not heard. If he finds either too much,
   `trim_b11.py`'s `TAIL` and `WIRES_DB` are the knobs (cue 13 holds down to about −30 dB).
2. **The groove's image** (cue 15: mid-band L/R 0.88 / 0.86 against 0.67 / 0.38). Below 1 kHz
   a drum is one signal in every mic and in both capsules of a pair; the recordings' spaced
   pairs and rooms are not. Per-mic sources (the handover's step 4) or a third draw of the
   drums' noise for a pair's second capsule.
3. **The ~3 ms comb** (cue 1, cepstral z 7-10 at 2.96 ms): in the far mics' highs at
   −80…−130 dB, not an echo; the same per-mic sources would answer it.
4. **The toms' sub at high velocity** (−25…−33 against −42): their modes' share — the
   onset's coherent push (`early.py`'s phases).
5. **Crests** (cue 12, 2 of 16): the burst is one 0.4-ms spike; the recordings' attacks
   spread over 1-3 ms. A per-layer burst length wants a blob field (format 3).
6. The jazz groove's 4-8 kHz (−7 dB) and sub (+10 dB), the jazz kick's click (+20 dB) and
   its 1-4 kHz T60 (1.2 s against 0.9), the ride (cue 8), the studio snare's 3-10 kHz flatness
   (0.30 against 0.38).

## Update — drums-gen 2, after the gap analysis (2026-11-13, the tenth listen)

`analysis-drums-2/report.md` ranked 15 synthetic cues; `tools/tympanum-fit/accept_b10.py`
checks each one per kit (b9: 21/162 → b10: 57/162). The listen is in `compartir\tympanum-b10\`
(NOTA.md in Spanish). The details are in `docs/round40/tymp-engine.md`, section *drums-gen 2*. Done:
- the attack (close mics in a head's near field; a drum's highs drawn twice; a cymbal's mono
  mics hear its other side; the studio kit's overheads/rooms under its close mics);
- cymbal tails (the residual noise, T60 above the floor);
- the owner's "iuuu" (no band swells after its peak, with a test; the measuring render's
  pedal fixed);
- the kick refitted after its fall;
- stick, colour, tonal and hat calibrated through the engine without a rebuild
  (`CANTUS_MEASURED_BLOB`).

**Next, in order:**
1. **Cost.** Measure on a quiet machine. The drums' side (band filters above 1 kHz) is new
   and cost more. Next savings: low bands at a quarter rate, the ride's union capped per
   layer, a cymbal's side only where a pair or a mono mic hears it.
2. **The toms' comb** (cepstral z 8) and **sub thump** (20–55 Hz −14…−34 dB; the recording
   −35…−55): it is not the fit (the Python synthesis reads −32). Look at the engine's
   paths (the proximity shelf, the kick sub mic, the room).
3. **Studio snare wires** (4–12 kHz −11…−21 re body; DRSKit −9…−12), and the first 50 ms
   crests (cue 12). The ladder agent suggests a minimum-phase attack for snare and toms,
   keeping model 5's band filters.
4. **Kick pitch**: the studio kick renders 70 Hz at v110 although its table now leads with
   76 Hz. Check the union's clustering and the early parts.
5. **Groove image**: mid-band L/R correlation 0.88 (DRSKit 0.67). The jazz groove's 4–8 kHz
   is 7 dB low.
6. **The crash**: b9's stays the default. The residual-fit crash is the A/B in b10; switch it
   in if the owner prefers it (its tail measures closer).
7. **Refit** from fresh renders of the recorded kits (the pedal fix): `refit2.sh` in
   `tmp\dg2`. Then cal.sh for two rounds, then tables_blob.

## Update — drums-gen, after the ninth listen (2026-11-13)

The owner on b9 (`compartir/tympanum-b9`, purely generated): *"ya es un poco más real pero
nos falta un montón aún; necesitamos analizar casi elemento por elemento con detalle, como
con Chorda, o incluso ver estrategias nuevas."* An element-by-element gap analysis
(`analysis-drums-2/report.md`) and a research on new strategies
(`research-p3/drum-strategies.md`) are under way; the plan follows them. What drums-gen
changed and measured (details: `docs/round40/tymp-engine.md`, *drums-gen*):

- **Groove highs, found and fixed.** Not the room, not the mic combs, not repeated
  partials: the hat's half-open states (CC4 0.06/0.30/0.62 in the groove), never calibrated,
  and two table bugs (a mode absent from a layer took the group's T60 — a nearly closed
  stroke rang 300 quarter-open partials 2.6 s; state blends mixed partials in amplitude and
  noise in dB). Studio groove 6–16 kHz components 15 → 33/kHz (DRSKit 31), flatness
  0.25 → 0.32 (0.36); jazz 24 → 31 (Virtuosity 28), 0.23 → 0.30 (0.28). Five seeds each.
- **Drum onsets**: each mode's phase at the strike and an early part (blob version 2,
  `early.py`); the head's beam (`head_beam`, `PieceGeom::radius`) — the kick's first 30 ms
  8 kHz −25 → −43 dB re its peak (DRSKit −44).
- **Ride**: 440 partials a layer, 512 a table; soft/medium strokes at the recording's fine
  structure.

**Still open (measured):** the nearly closed hat (bar 1: 20 against 45 components/kHz);
the hard ride's 8–12 kHz wash (stability 0.27 against 0.50, two tonal calibration rounds
did not move it); the onset's shape through the twelve mics matches DRSKit's for the kick
(correlation 0.93) but not reliably for snare and toms; the jazz kick's early highs ~13 dB
hot; the snare's body darker than DRSKit's (15–60 ms centroid 0.23–0.33 kHz against
0.37–0.53); the sidestick's tail (−53 dB at 300 ms against the sample's end).

**Cost — over budget, to fix next.** Measured on a busy machine (other builds running;
best of five): model 5 alone 18.5 % of a core (hat 3.9, crash 4.9, ride 5.9, drums
1.4–1.6 each), the full kit with every mic 27.7 % (budget ~10 %). Part is the machine
(earlier the same test read 7.9 % quiet), part is real: tables of up to 512 modes (were
384), the ride's denser fit. Already done: only sounding bands' filters run; a table's
modes grouped by the layers that own them, so a stroke runs only its own blocks (hat −30 %).
Next, cheapest first: measure on a quiet machine; cull a block of modes under −90 dB re the
object's loudest; a cymbal's side signal only where a pair hears it; the bands below
~1 kHz at a quarter rate (half the biquads); the ride's union capped by per-layer budget.

## The handover as it stood after the eighth listen (tymp-engine)

## What model 5 is

Every piece (crash, hi-hat with five states and the foot, ride with bow and bell, kick,
snare with head/rim/sidestick, three toms; splash/china/stack on the crash's table) is
synthesised from tables fitted to DRSKit (studio kit, CC BY 4.0) or Virtuosity Drums (jazz
kit, CC0) as rendered through our own engine:

- up to 384 **modes** per piece (frequency fixed per piece; per velocity layer amplitude,
  T60, a rise for the bloom, the share struck at once; drums glide by one law);
- **noise in 38 quarter-octave bands** (35 Hz–21 kHz), each band's power envelope at 22
  breakpoints (0.5 ms … 3.2 s), interpolated in dB;
- a **stick**: a 0.4-ms broadband half-sine of noise and a 2-ms low-passed knock;
- per hit: random mode phases, ±1.5 dB per mode, ±0.7 dB per band, ±3 cents on drums;
- one persistent object per piece (re-strikes add; a hat's choke; re-strike bloom);
- a side signal for cymbals (their own noise, the modes in quadrature) the overhead pair
  spreads; the virtual microphones read the piece's mono main ± side.

Calibrated in closed loop through the engine (levels per table and per layer, the modes'
weight against the bands, the ride's bow per velocity). Cost: model 5 alone 7.9 % of a
core for a busy full kit; with the twelve virtual mics 12.0 % (the paths made 2.3× cheaper
this round; still above the ~10 % budget — see *Cost*).

## What it cannot reach, structurally

1. **Fine time structure.** Each band is stationary Gaussian noise under a smooth envelope
   (22 breakpoints). A real cymbal's wash is granular — chaotic plate motion, crackle,
   amplitude modulation at 5–40 Hz per band, partials that wander; a snare's wires rattle in
   bursts; a hat's plates chatter. None of that is in a smooth envelope over filtered noise.
2. **Partials are pure lines.** Measured partials are 3–6 Hz wide (FM, beating, doublets);
   ours are exact sinusoids with exponential decays. Calibration can set their share, not
   their texture.
3. **The attack is generic.** 0.4 ms of noise plus a knock stands for every stick on every
   piece. The first 5–30 ms — where a snare's crack, a kick's beater, a tom's slap and a
   crash's ping live — are what the ear uses most to judge "recorded or synthetic", and
   they are the least modelled part.
4. **One source, many mics.** The virtual mics read the same mono main (± side) through
   delays and simple colour; real close and overhead mics hear genuinely different
   radiation (and the recordings' own room). This combs and flattens in ways the
   recordings do not.

## The three largest audible gaps (my judgement), with the measures

1. **Cymbal and hat texture in the groove** — the wash is smoother, a hiss with lines in
   it, not a shimmer. Groove 6–16 kHz effective components 13–15 per kHz against DRSKit's
   31 and Crocell's 35; flatness 0.26 against 0.33–0.36. Per *hit* our participation ratio
   is as high as the recordings' (hat 35–41 vs 38–46; crash 18–32 vs 8–17): the gap is in
   the groove, where many hits and the mics overlap — not yet explained; suspects are the
   modelled room's late field (a small FDN), the twelve identical-source mic paths combing,
   and the persistent objects keeping the very same partials hit after hit.
2. **Drum attacks** — the crack and the beater. First-3-ms centroid: kick 0.11 kHz vs
   DRSKit 0.38 at v80 (our attack too dull) and +9 … +16 dB too much 6–16 kHz on the kick's
   first 250 ms (the knock's leakage); snare 6–16 kHz −3 dB; the sidestick's tail −93 dB at
   600 ms where DRSKit's sample ends at −190. Snare wires: flatness about right, but the
   rattle is a smooth noise.
3. **The ride** — still more tonal at soft strokes, a little too noisy at hard ones. Top
   three partials (> 1 kHz) 26/20/12/18 % at v40/80/110/127 (seventh listen 33/23/14/29)
   against DRSKit's 18/16/13/19; peak height over the 300-Hz median 23/30/18/17 dB (34/39/22/
   22) against 28/27/20/21; fine-structure stability 0.32/0.53/0.20/0.20 against
   0.54/0.66/0.50/0.51 (the hard strokes now too noisy: one trim per layer cannot make a
   few partials stand out *and* the rest blend). The bell's tones are right
   (top three 38–49 % against 39–50).

## What a step change would take

In order of expected gain per effort:

1. **A recorded-residual layer** (the largest step, and a policy decision: it plays
   fragments of recordings). For each piece × velocity × a few round robins, store the
   *residual* of the first 30–200 ms — the recording minus our modal + band synthesis of the
   same hit, so the residual is mostly the attack and the chaotic early part — and play it
   under model 5 (which keeps the tail, the re-strike behaviour, the tuning, the mics). From
   Virtuosity (CC0) for a credit-free default, DRSKit with its credit. Size: 13 pieces ×
   4 velocities × 3 round robins × ~0.15 s mono ≈ 15 MB at 16 bit (≈ 5 MB with a simple
   lossless coder) — a downloaded content item (ADR 0030), not the repository. This is how
   the "modelled" products that convince (MODO's drums carry sample data) get there.
2. **Measured fine structure, still synthesis.** Per band, measure the envelope's
   modulation (its spectrum 2–60 Hz and depth) and the early envelope at 1–2 ms resolution
   for the first 100 ms; drive each band's gain with that modulation (seeded). Give each
   partial a measured width: doublets (± its −6-dB half-width) or a slow seeded FM. Expect
   the groove's components and the stability to move toward the recordings; cost ~+30 %.
3. **A real contact.** Replace the generic stick with a Hunt–Crossley contact against each
   piece's driving-point response (the research's §2.3), with per-piece, per-velocity
   fitted stiffness; for drums a measured transient (the first 10 ms's spectrum per
   velocity) shaping it.
4. **Per-mic sources.** Fit the close mics' and the overheads' own spectra per piece (the
   packs have them separately) and give each mic its own colouring of the source, instead
   of one source through delays — the recordings' mic combs are not ours.

## Cost (the budget is ~10 % of a core with the mics on)

`cantus-engine` `full_kit_with_mics_cost` (ignored; release; best of five) 12.0 %;
`cantus-dsp` `full_kit_cost` 7.9 % (hat 1.9, crash 2.0, ride 1.8, drums 0.5–0.7 each).
Cheapest further savings: band filters for bands under −120 dB skipped; the side chain only
for pieces a stereo pair hears; the modes' live blocks culled against the piece's own peak
(−100 dB) instead of an absolute floor; reflections for close mics' far images dropped.

## Where things are

- Engine: `crates/cantus-dsp/src/pulsus_measured{,_tick,_data,_tests}.rs`, the blob
  `pulsus_measured.bin` (both kits), the paths in `tympanum_model{,_paths}.rs`.
- Fit tools: `tools/tympanum-fit/` (fit, synthesis, blob, calibration cycles for both kits,
  metrics — `metrics.py` carries the analysis's cymbal and drum measures).
- Fits are regenerated from the packs (`fit.py`); each kit's calibration is in
  `tools/tympanum-fit/data/gains*.json`.
- Not to touch without the coordinator (fix-e owns them): `cantus-dsp/src/tympanum.rs`,
  `tympanum_rec*.rs`, `cantus-engine/src/inst/tympanum{,_unit,_cache}.rs`, `sampler_outs.rs`.
