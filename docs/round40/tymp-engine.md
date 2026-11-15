# Round 40 phase 3 — `tymp-engine` (Tympanum's engine, named outputs, the drum packs)

**Status: the sixth listen is ready (`compartir\tympanum-b6\`: the drums measured — next
section; the fifth, `compartir\tympanum-b5\`: every cymbal on model 5, the overheads in
phase; the fourth, `compartir\tympanum-b4\`, the measured
crash and hi-hat); the second's (`compartir\tympanum-b2\`) list is in but for
joining `InstrumentKind::ALL` (on the coordinator's word, with `tymp-ui`) and what *Left*
says.** Branch `r40/tymp-engine`, worktree `F:\projects\daw-r40\tymp-engine`. The owner at
checkpoint B: *"Se escucha como batería; aún no diría que se escucha real, pero sí suena a
batería — síguela mejorando."*

## drums-gen 3 (the owner away: measurable work only) — the cost, eight more cues, hybrid v3

Branch `r40/drums-gen` (r40/integration e714654a merged first: the hybrid's hooks, schema 71,
packs v2). Nothing here was judged by ear: every change had to raise `accept_b10.py`'s checks
with none that passed failing, and the saving had to leave the sound alone. The listen for
later: `compartir\tympanum-b11\` (NOTA.md in Spanish; b10 · b11 · recording).

**1. The cost: 18.7 → 7.3 % of a core** (`full_kit_with_mics_cost`, release, best of five,
the machine shared with other builders' runs, about a fifth of its threads busy; the denormals-flushed reading, as the
app runs, is the same). Model 5 alone (`full_kit_cost`) 10.2 → 6.0 %. The b10 reading
(18.6 %) was not the load: it is the code.

- **The release profile's thin LTO left the hot loops scalar.** The modes' rotation, the bands'
  filters and sums, a settled path's read: inlined into their callers they compiled to scalar
  code (`mulss`); each in its own `#[inline(never)]` function they run in vectors. Found by
  timing the same kernel stand-alone (2.4 ns a block of eight modes) against the crate (6–9 ns)
  and by profile flags (`CARGO_PROFILE_RELEASE_LTO=false` + one codegen unit: fast; either
  alone: slow). The ride's modes 463 → 141 ns a sample.
- The modes' sums in eight lanes over **a list of the live blocks** (`Modes::index`, rebuilt
  when the flags change) — a block's eight added one after another were a chain of dependent
  adds, and the hat walked 86 dead flags a sample. The bands' weighted sums likewise.
- **A drum's glide** turned the hit's own angles by `θ · glide` with a short f64 series instead
  of a cosine and a sine per mode per control step (only the sounding blocks; with no glide
  left, the hit's own angles to the bit).
- **The paths' rings mirrored past their end** (`MIRROR`): a settled path reads its chunk's
  window in one run, no wrap per tap; **eight groups' colours side by side** (`BATCH`): each
  colour's one-poles waited on their own previous sample, eight overlap.
- A mode under 1e-20 is set to zero at a control step (no subnormal arithmetic where the
  hardware does not flush).

Proof the saving changed nothing: `full_kit_dump` (both kits, a groove with every piece)
before and after, `dump_diff.py`: −96.1 dB (studio) and −103.3 dB (jazz) re peak at most
(the summation order only; the phasors' recursion is bit for bit as it was).

**2. The open cues: 57 → 71 of 162**, none that passed fails (studio and jazz hits, both
grooves):

| cue | studio b10 → b11 | jazz b10 → b11 |
|---|---|---|
| 3 snare wires | 0/3 → 2/3 | 2/3 → 2/3 |
| 4 sub-sonic thump | 0/7 → 2/7 | 0/7 → 1/7 |
| 5 upper modes ring long | 2/9 → 2/9 | 5/9 → 8/9 |
| 6 kick click | 0/1 → 1/1 | 0/1 → 0/1 |
| 12 first-50-ms crests | 0/8 → 2/8 | 0/8 → 0/8 |
| 13 studio kick pitch | 0/2 → 2/2 | — |
| 14 tonal highs | 1/3 → 2/3 | 0/3 → 0/3 |
| (the rest: 1, 2, 7, 8, 9, 10, 15 unchanged) | | |

What did it, found part by part through the engine (`variant.py`: a fit with one part of
some pieces silenced — modes or bands in a range, the stick, the early part, the glide):

- **The bands' skirts.** A band is two second-order band-passes, 12 dB an octave each way. A
  tom's body bands (80-700 Hz) put their noise under the head's fundamental (the toms' and
  cymbals' 20-55 Hz in the first 80 ms 10-30 dB over the recordings') and, 200-500 ms on,
  into 1-4 kHz: removing the tom's 1-4 kHz modes or bands changed nothing; removing its
  20-700 Hz bands took its 1-4 kHz T60 from 1.34 s to 0.41. Now (`Object::split`) a tom's low
  bands go apart, high-passed under its fundamental (eighth order, `Table::low_hz`) and
  low-passed over 1.1 kHz (fourth order); a cymbal's bands are high-passed under 90 Hz. Not a
  snare's (the high-pass took the push out of its first 6 ms, cue 10), not a kick's (its
  skirts are part of the groove's 4-8 kHz, cue 15), not the studio floor tom's (its soft
  strokes' 3-8 kHz flatness went 0.45 → 0.39). The modes are untouched.
- **The studio kick's pitch**: the recording's fundamental rings on under its first fall
  (66-80 Hz at 0.3-0.6 s, T60 3-6 s); ours fell in 0.31 s, so at 250-600 ms its partials were
  broad lobes whose maximum sat between them (71.6 Hz at v110, 84.4 at v127). `trim_b11.py`
  gives each layer a second partial at its fundamental (86 Hz at v80, where its recording
  leads), 24 dB under, T60 2.5 s: 67.0 / 86.1 / 76.4 / 76.4 Hz against 67.6 / 86.2 / 75.8 /
  76.2, the 47-57 Hz line −25…−50 dB (≤ rec + 4). At −12 dB it cost the groove's 4-8 kHz
  1.5 dB (its low energy over the whole groove), at −24 nothing.
- **The studio snare's wires**: 2.8-13 kHz +5 / +9 / +9 dB at v80 / v110 / v127 from 18 to
  450 ms (the whole of 3-10 kHz, so it stays as flat as it was): 4-12 kHz re the body
  −10.4…−12.0 dB (DRSKit −9.3…−12.3), the 0-100 ms centroid 0.53-0.81 kHz. Its 3-10 kHz
  flatness stays 0.30 (0.38): removing its modes over 1.2 kHz does not move it — the noise
  itself is combed through the mics.
- **The drums' upper partials** capped at 0.6-8 kHz (studio toms 0.4 s, jazz toms and kick
  0.8 s), and their noise there after the colour's corrections (`<TABLE>_DECAY` in
  `data/gains*.json`, applied by `tables_blob.py` — `calibrate_bands.py`'s +10 dB in the tail
  region had lifted it back). The studio kick's v127 stick 0.84 × (its click 3.8 dB over
  DRSKit's).

**Still open, measured**:
- **The ~3 ms comb** (cepstral z 7-10 at 2.96 ms on the kick and toms, every mic but the
  kick's own): not an echo — no path is 142 samples apart and the dry source has no such
  peak; the log spectrum's ripple sits at 8-24 kHz in the far mics' highs at −80…−130 dB,
  where the recordings carry their noise floor (the analysis lists DRSKit's −85…−90 dB floor
  as an artefact). A real fix wants per-mic sources, not a floor of noise.
- The toms' sub-sonic thump at high velocities (−25…−33 dB; rec −42…−43), their modes'
  own share of it.
- The groove's mid-band L/R correlation (0.88 / 0.86 against 0.67 / 0.38): below 1 kHz a
  drum is one signal in every mic and both capsules of a pair.
- The crests (2 of 16), the jazz groove's 4-8 kHz (−7 dB) and sub (+10 dB), the jazz kick's
  click and 1-4 kHz T60 (1.2 s against 0.9), the ride (cue 8).

**3. The hybrid packs, version 3.** The new tables (the blob's hash `0x582AA7B6`, was `0xED1998F7`)
make the v2 packs stale, as every model change does. Both kits refitted with
`refit_hybrid.sh` on this branch's model (content root `tmp\hyb\v3\content`: junctions to
the downloaded DRSKit and Virtuosity Drums), `residual_zip.py` `VERSION = 3`, the manifest's
two entries at version 3 (URL `…/content-r40/tympanum-hybrid-{kit}-3.zip`, size, SHA-256,
`installed_size`; `cantus-content`'s tests green). Not uploaded by this branch; **uploaded at
the close-out with the owner's OK** (2026-11-14, public download verified), and the ignored,
networked `the_published_hybrid_pack_downloads_matches_its_hash_and_fits_this_model` passes
against them on `r40/integration`.

| pack (`F:\projects\daw-r40\tmp\hyb\v3\`) | bytes | SHA-256 | installed |
|---|---|---|---|
| `tympanum-hybrid-jazz-3.zip` | 5 176 330 | `dc4536d60a5716504332915bd9129a930f7b7ca009504409700469ae07e3dcfc` | 5 770 574 |
| `tympanum-hybrid-studio-3.zip` | 5 241 324 | `1b7cd0dd13832b9e122d4ec5f6d210cd5fd5f98ba096ec97ed99ebca2ffa12e0` | 5 769 331 |

Each starts with `PK` and holds `residual.crs` (byte-identical to the fit's; `CRS1`, model
`0x582AA7B6` at byte 12), `LICENSE.txt`, `README.txt`. `hybrid_measure.py` (third octaves
60 Hz-16 kHz, mean |dB| from the recording, v2 → v3): jazz 0-50 ms generated 4.08 → 4.33,
hybrid 3.53 → 3.63; 50-200 ms generated 4.01 → 4.12, hybrid 3.22 → 3.18; studio 0-50 ms
generated 4.29 → 4.48, hybrid 3.46 → 3.54; 50-200 ms 4.17 → 4.30 and 3.13 → 3.13. **By that
measure (not accept_b10's) the generated model moved 0.1-0.2 dB away from the recording**:
the cues raised are narrow (a band, a window, a ratio) and the snare's wires and the kick's
tail add level where the third-octave average weighs it; the owner's ear decides. Detail:
`tmp\hyb\v3\measure-{jazz,studio}.txt`, the fits' reports `v3\{jazz,studio}\report-*.txt`,
the listen `v3\listen-{jazz,studio}\` (generated · hybrid · recording).

Tests: `cantus-dsp` `a_drums_noise_rests_under_its_fundamental_and_its_low_bands_under_its_highs`
(the high-pass −60 dB at 0.4 f0, the low-pass −40 dB at 4 kHz, which pieces split), the
model-5 and Tympanum tests (84), `no_band_swells_after_its_peak`; `cantus-engine` Tympanum
(14), `rt_memory` Tympanum (2). Ignored, for the measures: `full_kit_dump`, `cost_split`,
`cost_breakdown`, `dump_source`, `dump_hit_source_and_output`, `dump_hit_per_mic`,
`print_paths`. The fits behind the blob: `tmp\dg3\keep\fit_b11.json`, `fitv_b11.json`
(`trim_b11.py` on drums-gen 2's `fit_r4` / `fitv_r4`).

## drums-gen 2 (after the ninth listen) — the gap analysis's fifteen cues, measured

The owner on b9: *"a bit more real, but a whole lot is still missing"*. An independent
analysis (`analysis-drums-2/report.md`) ranked the 15 most synthetic cues with numeric
targets; `tools/tympanum-fit/accept_b10.py` runs each of them as a check per kit (the
analyst's own measures; where the recording itself misses an absolute target, the check
passes within a stated margin of the recording). The b9 render (c11) passes 21 of 162; the
tenth listen's passes 57. Still synthesis only: no recording fragment is played.

**1. The late, smeared attack and the ~3 ms comb (cues 1 and 10). The cause, found by measurement:**
`head_beam` cut every mic off a head's axis with the far-field piston law, so the snare's own
mic (21 cm away, over the rim) heard its head through a 1.35 kHz low-pass, the toms' 1.2–2.1 kHz,
and the overheads (+5 dB, a pair centred) carried the crack 3.5 ms later. Soloed through the
engine: the snare's 2–16 kHz in its first 20 ms was −1 dB re the whole from the overheads and
−6 from its own mic; in DRSKit's own mix, −6.5 and −3.6. One identical source in every mic
combed the stroke (cepstral z 6–15; the recordings' 3–4.5). The fix:
- **Near field** (`near_field`): a mic in the column over a head (within 1.4–1.8 radii of its
  axis) and inside its Rayleigh distance `π a² f / c` hears its highs. The beam's low-pass rests
  on a floor at its level at `f_nf = r c / (π a²)` (open for the close mics). A kick's highs
  beside it, in the snare's mic, stay beamed.
- **A drum's highs drawn twice** (`DRUM_SIDE_HZ`): above 1 kHz a drum's noise and its stick
  burst are rendered again from a second noise. The side is the difference, and every path
  beyond 0.5 m (and every reflection) adds it, so the close and far mics carry uncorrelated
  highs and the same push below. A measured **cymbal**'s mono mics hear its other side (and
  its own burst) instead of its main (`Path::main`), so the hat mic and the overheads no
  longer comb the hat into fixed lines.
- **The studio kit's mix**: overheads +5 → −3 dB, rooms +1/−5 → −5/−9, ride +1. The jazz
  kit keeps the spaced balance Virtuosity was recorded with.

Studio snare's strongest 2–16 kHz arrival 4.5 → 0.9 ms (DRSKit 1.1); its 3 kHz+ in the first
1.5 ms −14 → +3 dB re 2–50 ms (+1.5); cepstral z 5.5–6.2 → 4.3–5.3 (3.5). The toms' comb is
still z 8 (an open item).

**2. Cymbal tails turned to hiss (cue 2).** `fit.py` now caps a cymbal band's noise by the
recording's own residual (`residual_share`: the 10th-percentile floor between the partials).
It reads a partial's T60 only while that partial stands 10 dB over its neighbourhood's floor,
and lets a long partial outlast its band's average (`FLOOR_T60`). Ride v127 at 2–3.3 s:
4–12 kHz flatness 0.05 → 0.02 (DRSKit 0.01), stability 0.11 → 0.34 (0.29); its loud 8–16 kHz
T60 5.6–5.9 → 2.6–2.9 s (2.4–2.5). The jazz open hat's tail flatness 0.12–0.14 → 0.02–0.05
(Virtuosity 0.01–0.02). The crash stays b9's (the owner's best-rated); the residual-fit crash
is the A/B in b10 (`*-crash-2b-nuevo-residuo`).

**The owner's "iuuu" (found by the ladder agent).** 40 of 128 layers swelled 2.2–3.2 s
after the strike: the measuring render closed the pedal on the open hat's v127 ring at 3 s
(the next articulation's CC4 a second early), and bands were fitted on the floor. Fixes:
`tables_blob.py`'s `no_swell` (past its peak a band never rises; past 1 s it falls at least
as fast as it did from 300 ms) with the test `no_band_swells_after_its_peak`; the render's
pedal now moves 125 ms before a stroke. The open hat's v127 tail no longer climbs.

**3–14.**
- Snare: the early noise under 0.8·f0 cut (`subcut`). Toms: under 1.3·f0, their upper
  modes no longer than the recording's 1–4 kHz decay (1–9 s → 0.5–1.5 s; DRSKit 0.35–0.45).
- The kick: its partials read after its pitch falls (DRSKit's goes ~110 → 76 Hz; the glide
  may reach 45 %).
- `calibrate_stick.py`: each layer's burst and knock levelled through the engine (the kick's
  by its click: −11 → −23 dB at v127, DRSKit −25). The closed hat's partials are calibrated
  by its flatness (`calibrate_tonal.py`). The jazz closed hat's stability is 0.08–0.15
  (Virtuosity 0.07–0.17).

**Still open, measured** (the full table: `accept_b10.py`):
- the toms' sub-sonic thump (20–55 Hz −14…−34 dB, recordings −35…−55);
- the studio kick still reads 70 Hz at v110 (76);
- the studio snare's wires (4–12 kHz −11…−21 re body; −9…−12);
- the first 50 ms crests (cue 12, b9 5/8 → 0/8, mostly ±2–4 dB);
- the groove's mid-band L/R correlation (0.88; DRSKit 0.67) and the jazz groove's 4–8 kHz
  (−7 dB);
- the ride's jazz ping lines (cue 8).

**Cost.** Done:
- a path 40 dB under its mic's loudest is dropped;
- model 5's mode blocks and bands rest 100 dB under the loudest a piece's hits struck
  (they ran to −160 dB absolute).

The drums' side adds band filters above 1 kHz. Measured at the end (release, best of five,
four other compiles running): model 5 alone **10.8 %** of a core (the ride 3.4, the crash 2.8,
the hat 1.6, each drum 0.7–1.0); the full kit with every mic **18.6 %**. b9's reading was
18.5 / 27.7 %, under heavier load. Still over the ~10 % budget. The savings listed in
`NEXT-drums.md` remain to do.

Tools: `accept_b10.py`, `calibrate_stick.py`, `fit.py` (`RESID`, `FLOOR_T60`, `SUBCUT`),
`tables_blob.py` (`no_swell`, `_STICK_`), the measure example's `TYMP_SPEC=key=value,…` (a mic
soloed, a level moved), and a debug build's `CANTUS_MEASURED_BLOB` (a calibration cycle
without a rebuild). The fits behind the blob: `tmp\dg2\keep\fit_r4.json`, `fitv_r4.json`.

## drums-gen (after the eighth listen) — the generated sound: causes found and fixed

The owner: *"a hybrid as an option, but the generated one must improve much more"*. Branch
`r40/drums-gen` (from tymp-engine 6c4dd2d3, with fix-e via r40/integration 305df85d — the
pads' unique ids, so every number here is on the shipping seeds). Pure synthesis: no
recording fragment is played. Measured through the engine against DRSKit (studio) and
Virtuosity (jazz) with `tools/tympanum-fit/` (new: `groove_hf.py`, `attack.py`,
`grain.py`, `early.py`, `calibrate_hat.py`, `fit_hat_tonal.py`, `calibrate_bands.py`,
`calibrate_tonal.py`, `make_listen.py`; `tympanum_measure model`, `TYMP_GROOVE_KEYS`,
`TYMP_SEED`).

**1. The groove's fine highs — the cause, suspect by suspect** (6–16 kHz effective
components per kHz, the analysis's measure; DRSKit 31, model 15):

| test (studio groove) | comps/kHz | flatness |
|---|---|---|
| as built (6c4dd2d3) | 15.1 | 0.25 |
| modelled room off | 14.9 | 0.25 |
| direct paths only (no reflections) | 15.2 | 0.25 |
| one mic alone (snare top / overheads) | 14.9 / 20.9 | 0.28 |
| each hit's partials detuned ±30 / ±100 cents | 15.5 / 17.0 | 0.25 |
| **the modes off** | **43.8** | **0.33** |
| DRSKit | 30.9 | 0.36 |

Not the room, not the mics combing, not the same partials hit after hit: the hat's partials
themselves, in the half-open strokes the groove plays (CC4 0.06 / 0.30 / 0.62), which no
calibration had ever seen (`calibrate.py` measures the closed and the open stroke 0.1–0.6 s
after the hit). Single strokes at those openings: 6–16 effective components in their first
170 ms against DRSKit's 24–50. Found on the way and fixed:

- **A mode absent from a layer took the group's median T60** (`tables.union`): a nearly
  closed stroke (closed blended with a quarter open) rang ~300 quarter-open partials for
  2.6 s. Now each layer's local median (an octave around); in the engine the T60 is the
  layers' geometric blend, and the bloom and the share struck at once come from the layers
  that have the mode (by their share of it), not the main layer's defaults.
- **A blend of two hat states mixed partials in amplitude but the noise in dB** — two
  recordings crossfaded mix in power; now both do. Near a measured opening the plates play
  as measured there (`STATE_HOLD`, a fifth of the way each side: the recorded kits play
  their closed sample at CC4 0.06).
- The half-open states' partials trimmed (see the table at the end for where it stands).

**2. Drum attacks.** Random mode phases scattered the head's first push: the recorded
kick's and tom's first 5 ms below 1.5 kHz is a coherent pulse. `early.py` fits, per layer,
each mode's phase at the strike and a short early part (3–20 ms: a head loses much of its
energy to the air at once) by least squares on the recording's first 80 ms at the table's
own (union) frequencies, the bands giving up that power; blob format 2 carries them
(`Layer::{ets, ph, ea, ep}`); the engine scatters the phases each hit, more the higher the
mode (0.1 rad + 1 rad per kHz). Onset residual below 1.5 kHz in the first 40 ms: 0.04–0.11
(kick, snare), 0.08–0.24 (toms) — random phases 0.6–0.9. The sidestick (a click its few
modes cannot carry) keeps random phases. The stick's knock re-fitted after.
**The head's beam**: every mic heard a full copy of the head, so the kick's first 30 ms
had 13–25 dB too much 8–16 kHz (its own mic alone: within 3 dB of DRSKit). A head's highs
leave along its normal (a piston of radius `a` is heard at angle θ only below about
`2c/(π a sin θ)`): `head_beam` in the paths' colour, `PieceGeom::radius` from the piece's
diameter (`tympanum_unit`). Kick 3–10 ms at 8/16 kHz −32/−37 → −47/−57 dB re its loudest
(DRSKit −48/−63); the snare's long-term 6–16 kHz from 9–14 to 17–24 effective components
(DRSKit 26–30: the comb lines of twelve copies halved). Each drum's colour through the
mics then corrected per band and region of the hit (`calibrate_bands.py`, smoothed). A
grain measure (`grain.py`: envelope fluctuation per octave) found the snare's and hat's
noise as grainy as the recordings' — "smooth noise" is not measurably the gap.

**3. The ride**: refitted with 440 partials per layer (DRSKit's ride has 430–500 standing
10 dB over the local median; the fit kept 260, and the table cut its union to 384 — now
512): soft and medium strokes now at the recording's fine structure (stability 0.46/0.58
against 0.54/0.66, 17–22 peaks per kHz at 4–12 kHz against 18–21; was 0.32/0.53, 5–15).
The hard strokes' 8–12 kHz is still too much wash (stability ≈0.25 against 0.50):
`calibrate_tonal.py` raises each region's partials through the engine, one more round to go.

**4. Cost — over budget.** Only the sounding bands' filters run (`band_range`, under
−140 dB both ways); a table's modes are grouped by the layers that own them (`tables.union`),
so a stroke runs only its own blocks of eight (the hat 5.6 → 3.9 %). Measured on a busy
machine (best of five): model 5 alone 18.5 % of a core (hat 3.9, crash 4.9, ride 5.9,
drums 1.4–1.6), every mic on 27.7 % — the same test read 7.9 % quiet before, so part is the
machine, part the tables of up to 512 modes. To do next (`NEXT-drums.md`).

**Where it stands** (shipping seeds; studio against DRSKit, jazz against Virtuosity):

| | eighth listen | now | recording |
|---|---|---|---|
| studio groove, 6–16 kHz components/kHz (5 seeds) | 15.1 | 33.4 ± 0.8 | 30.9 |
| studio groove, flatness | 0.25 | 0.32 | 0.36 |
| jazz groove, components/kHz | 23.5 | 30.8 ± 1.2 | 28.0 |
| jazz groove, flatness | 0.23 | 0.30 | 0.28 |
| nearly closed hat (groove bar 1), components/kHz | 7.3 | 19.7 | 44.8 |
| studio kick v80, 8 kHz at 3–10 ms re its peak | −25 dB | −43 dB | −44 dB |
| studio kick v80, onset shape (< 1.5 kHz, 12 ms) correlation | −0.12 | +0.93 | 1 |
| studio ride v40/v80/v110/v127 stability | .32/.53/.20/.20 | .41/.51/.27/.28 | .54/.66/.50/.51 |

The ninth listen: `compartir\tympanum-b9\` (NOTA.md in Spanish). Tests: `cantus-dsp`
`a_drum_starts_with_the_same_push_each_hit` (two kicks' first 6 ms below 300 Hz correlate
> 0.8 though each draws its own variation), the blob's round trip at version 2, block
independence and rest for the drums and cymbals (as before, green), `beam_tests`
(`head_beam`: a 22″ kick beside it below 1 kHz, a smaller head wider, a cymbal open).

## After the seventh listen ("va mejor pero aún está lejísimos") — the ride, the cost, the handover

- **The ride's bow** less tonal, per velocity (a trim per state and per layer, and each
  layer's level against its recording's): studio top three 33/23/14/29 % → 26/20/12/18
  (DRSKit 18/16/13/19), peak height 34/39/22/22 → 23/30/18/17 dB (28/27/20/21); the bell
  unchanged. The jazz kit likewise against Virtuosity. `compartir	ympanum-b8\`.
- **The virtual mics' paths 2.3× cheaper** (a settled path's weights once a chunk; a piece
  silent a ring's length skipped, the same bits): the full kit with every mic 28.3 → 12.0 %
  of a core (best of five); model 5 alone 7.9 %.
- The groove's fine highs did not move (15 components per kHz against DRSKit's 31).
- **`NEXT-drums.md`** (repository root): the honest handover — what model 5 cannot reach,
  the three largest audible gaps with their measures, what a step change would take.

## After the sixth listen — cost, the blob, the CC0 jazz kit

- **Cost**: a busy full kit 7.7–7.9 % of a core in model 5 (release; each piece 0.5–2 %).
- **Tables as a blob** (`CMT1`, 497 KB for both kits instead of 960 KB of Rust for one),
  decoded with the kit; a round-trip test.
- **The sidestick's ring**: a measured piece feeds the modelled room 0.15 (its tables hold
  the recording's room) and no band holds more than the recording's whole power — the
  sidestick at 600 ms −93 … −96 dB (was −67 … −72).
- **Tom glides**: one law per drum (velocity squared, one time constant), blended between
  layers. (The measure's early/late peak picking still reads our toms erratically — it
  lands on different partials; DRSKit's read 9 → 207 cents — to be checked by ear.)
- **The jazz kit** (`model:jazz`, the existing "Jazz Birch" strings): the studio kit on
  Virtuosity Drums' (CC0) tables, ids 17–24; calibrated through the engine against
  Virtuosity's hits (levels within ±1 dB after two cycles); e2e through `assemble`. The
  studio kit's `credit` names DRSKit.
- The fits are not kept in the repository (`tools/tympanum-fit/README.md`): the blob and
  each kit's `gains*.json` are.

## The sixth listen — the drums measured

- **Kick, snare (head, rimshot, sidestick), three toms on model 5**, fitted off DRSKit with a
  drum's rules (ADR 0124): bands down to 35 Hz, the pitch glide per layer, a few cents each
  hit. Through the engine against DRSKit: the kick at 600 ms −32 … −43 dB (model 4
  −82 … −91; DRSKit −28 … −36), its 40–100 Hz T60 1.2–2.4 s (0.2; 1.5–2.8); the snare at
  600 ms −40 … −45 (−88; −38 … −46); the toms at ≈ 79 Hz (160; 77–82) and their lows
  4–7 s (0.7–1.6; 4.6–6.8); the groove's loudness range 35 dB (71; 43), its broad bands
  within 2 dB of DRSKit's.
- `p{n}.pos` tilts a measured piece; the kick's `beater` scales its stick's burst.
- **`delay_mask`** (asked by `tymp-ui`): outputs own a delay line only when `o{n}.delay` is
  set; `UnitKind::Drums { room, delays }`; `OutDelay::none()` takes `set_ms` safely. Tests:
  `cantus-dsp` (a line-less output delays nothing; lines only where masked), the engine
  (`an_output_delay_gets_a_unit_with_its_line_and_no_other`).
- `compartir	ympanum-b6\` (NOTA.md in Spanish): each drum at four velocities, the fifth
  listen's, now, DRSKit; the groove three ways.
- Still apart: the sidestick rings longer than DRSKit's (the modelled kit's room); the
  toms' fitted glides are uneven across velocities.

## The fifth listen — every cymbal measured, the overheads in phase

After the fourth listen's measures (the coordinator's list):

- **Overheads never out of phase** (`tympanum_model{,_paths}.rs`): a stereo pair hears every
  piece's direct sound from its centre, and a model 5 cymbal's side signal (its own noise,
  its modes in quadrature) is spread across the pair by `PieceGeom::width` (0.7 for every
  cymbal). Groove, through the engine: the crash's tail +0.21 … +0.46 in every octave
  (was −0.22 … −0.35; DRSKit +0.11 … +0.34), mono loss ≤ 2.3 dB; 500–2000 Hz +0.77
  (was −0.13; DRSKit +0.68).
- **Density**: bands two sections deep (a soft crash's mids leaked into its highs: crash
  v40 4–8 kHz flatness 0.17 → 0.06, DRSKit 0.06); the union of a piece's modes keeps one
  member per layer, 384 modes; the six strongest partials matched to the recording's share;
  the modes' weight trimmed through the engine (`calibrate.py`, two cycles); a fit bug
  (a cache keyed on a freed array's id) fixed.
- **The ride on model 5** (bow and bell as zones of one plate; the bell's tones kept):
  top three 47–61 % → 18–31 % (DRSKit 13–19), its highs at 400 ms −1 … −5 → −21 … −24 dB
  (−23 … −26), the bell's top three 37–46 % (39–50). Splash, china, stack on the crash's
  table sized to them.
- **Re-strike bloom**: a cymbal struck while it rings blooms (≤ 12 velocity units).
- **Cost**: bands and modes laid out side by side (structure of arrays): a busy groove's
  crash and 16th-note hats 4.0 % of a core with 384 modes and the side signal.
- Groove 6–16 kHz: flatness 0.26, 13.4 components per kHz (DRSKit 0.36, 31).
- `compartir	ympanum-b5\` (NOTA.md in Spanish): each cymbal at four velocities, the fourth
  listen's, now, DRSKit; the groove three ways.
- Still apart: the ride a little more tonal than DRSKit; the loud open hat's top three
  12–19 % (8–10); the groove's fine high components 13 per kHz against 31. Next: the drums
  (the analysis's list).

## After the third listen — model 5, *measured* (the fourth listen)

The owner: *"suena súper feo, los platillos suenan como vidriosos, tiene muchísimo para
mejorar."* The research (`research-p3/drum-synthesis.md`) and the independent analysis
(`analysis-drums/report.md`) agreed: model 4's cymbals were a few pure, frozen partials in
silence. Built (ADR 0124's second amendment):

- **The measuring tool** `tools/tympanum-fit/` (Python; its README the steps): DRSKit's hits
  rendered through the engine's own recorded path (`tympanum_measure hits`, now a mono
  float WAV — the FLAC encoder ran this machine out of memory; `TYMP_HAT_OPEN` for the
  hat's half-open states) → per piece, state and velocity: up to 256 modes (closed-loop
  tonal level), 32 quarter-octave band envelopes at 22 breakpoints, the stick; → Rust
  tables (`cantus-dsp/src/pulsus_measured_tables.rs`, generated, credited CC BY like a pack;
  `THIRD-PARTY-NOTICES.md` says so) → each table's level calibrated through the engine.
  The fit's own data: `tools/tympanum-fit/data/`.
- **Pulsus model 5** (`pulsus_measured{,_tick,_tests}.rs`): one persistent object per
  piece (a re-strike adds; a hat's choke in a second bank), modes as phasor pairs (ring less
  bloom), one noise through 32 band-passes, envelope slots, the stick; seeded per hit.
  Tympanum's crash and hi-hat play it (`tympanum_voicing::piece_measured`, the cache);
  Pulsus's kits never. The ride, the other cymbals and the drums still play model 4.
- **Measured** (the analysis's measures, through the engine, against DRSKit): crash 4–8 kHz
  flatness 0.00 → 0.12–0.21 (0.06–0.19), top three partials 37–60 % → 7–12 % (5–14),
  velocity range 29 → 23 dB (22); closed hat flatness 0.03–0.08 → 0.35–0.40 (0.26–0.34),
  stability 0.72–0.79 → 0.26–0.30 (0.23–0.28), range 32 → 22 dB (21); the pedal's first
  3 ms 3–6.5 → 0.5–2.1 kHz (0.3–0.7); the groove's 6–16 kHz flatness 0.07 → 0.27
  (0.33–0.36), its components 4.3 → 11.8 per kHz (31–35). Cost: a busy groove's crash and
  16th-note hats 4.1 % of one core (release, `busy_groove_cost`).
- **Still apart**: the soft crash has fewer fine partials at 4–12 kHz than DRSKit; the loud
  open hat keeps a couple of partials too strong (top three 13–20 % against 8–10); the
  crash's stereo through the virtual overheads is negative at 250 Hz–2 kHz in the groove
  (−0.2 … −0.35; DRSKit +0.1 … +0.3) — the paths' time and dipole sign on a tonal source,
  not the voice; a re-strike's extra bloom (energy coupling) is not modelled yet.
- **The fourth listen**: `compartir\tympanum-b4\` (NOTA.md in Spanish): the crash and the
  hat's closed, open and pedal strokes alone at four velocities — model 4, model 5 and
  DRSKit at one loudness — and the groove three ways.

## 1. The modelled kit's realism, measured

`cantus-ui/examples/tympanum_measure.rs` (+ `tympanum_measure/{analyse,hits,groove}.rs`;
`tympanum_groove` shares the groove) plays every articulation alone at velocities 40, 80,
110 and 127 through the engine's offline render and measures each hit: peak, RMS over
500 ms, time to peak, the spectral centroid of the first 3 ms (the stick) and of 15–60 ms
(the body), the octave bands 31 Hz–16 kHz in six windows after the onset, each band's peak
time, T20 and T40, the strongest low partial early (10–60 ms) and late (150–400 ms) — the
glide — and the four strongest partials; `wav` mode reads a DrumGizmo hit (its channels
summed). References, all through the same engine: Gogodze Phu II (7 mics), DRSKit (13) and
CrocellKit (15); DRSKit's raw hits for the cymbals. Model 3 (Pulsus's voices, checkpoint B)
against them:

| | model 3 (checkpoint B) | the recorded kits |
|---|---|---|
| velocity → timbre, kick and toms | none (the same centroid at 40 and 127) | the attack brighter the harder |
| highs of kick and toms | gone in ~30 ms (T20) | T20 70–150 ms |
| snare body (15–60 ms centroid) | 3.6–4 kHz | 0.4–1.3 kHz |
| hats and cymbals | energy at 250–1 000 Hz | closed hat 7.5–9 kHz, open 4–7 kHz, crash 0.9–2.6 kHz |
| the cymbal's wash | no build-up | its 4 kHz band peaks ~130 ms after the hit |

**Pulsus model 4, *acoustic*** (`cantus-dsp/src/pulsus_acoustic{,_drum,_metal,_tests}.rs`;
ADR 0124's amendment says why and how): the modes driven by the strike's acceleration (the
contact's length is the attack, a harder hit brighter), the stick's or beater's noise,
tension glide, two heads beating, the shell, the head's dense highs, snare wires gated by
the snare-side head (growing in over 5 ms), cymbals whose high bank the low one drives (the
wash builds), a hat's low cut following its openness and its plates' rattle, per-hit
variation. Tympanum's modelled pieces play it (`tympanum_voicing.rs`, `piece_acoustic`; the
cache sets `model = 4`); no Pulsus kit does, so no Pulsus song changes. Its state is
allocated with the kit (`Pulsus::enable_acoustic`). Against DRSKit after it: hat closed
body centroid 5.4–7.8 kHz (DRSKit 7.5–8.6), open 4.3–6.9 (4.2–6.0), the crash's 1.5 →
2.5 kHz from velocity 40 to 127 (0.9 → 2.6); the kick 69 Hz with its body centroid
142 Hz (DRSKit 71 and 141), the snare 208 Hz with 554 Hz (212 and 524). Each piece's level
set against DRSKit's: at velocity 127 every piece within 2 dB of it but the ride (−4 dB) and
its bell (−3). Still apart: the ride's peak comes with its wash (~240 ms) where DRSKit's is
the stick's ping; the hats' highs a little darker than DRSKit's.
The last measurement: `F:\projects\daw-r40\tmp\meas5\compare.txt` (the model beside the
three kits).

A slip the measurements found, fixed: the voicing first read a piece's diameter as inches
(it is cm) — a 25 Hz kick and an 80 Hz snare.

## 2–10. The rest of the list

- **(2) A kit change keeps the old kit playing** until the new kit's files are in
  (`tympanum_cache::{attach, wants_all}`, the sample cache's `wants_now`): the old kit's
  files stay held, its text rides along as a `k:` marker the unit plays; an offline render
  never waits. A unit test in the cache; `rt_memory` plays the change.
- **(3) DrumGizmo kits that play**: `kitbuild_mics.rs` writes one 24-bit file per mic per
  hit (a stereo pair one stereo file) and an SFZ multi-mic kit with its `<sfz>.mics.json`
  (which mic is which drum's own); `drumkit_mix` builds a multichannel pack too. New packs,
  each licence read on its drumgizmo.org page today, the page's MD5 equal to the
  download's, the sizes measured by building them:
  - **DRSKit, every microphone** (`drskit-tympanum`, `DRSKit_basic.xml`, 13 mics, CC BY
    4.0; 2 803 397 710 B, SHA-256 `529f2dca…f090`, MD5 `8c4d4b61…c133`; 1 886 696 064 B
    installed).
  - **CrocellKit, every microphone** (`crocellkit-tympanum`, `CrocellKit_default.xml`, 15
    mics, CC BY 4.0; 5 646 502 341 B, SHA-256 `341d1f23…d5c6`, MD5 `fa2be0f8…90d3`;
    5 911 171 355 B installed). Its kit has no key 45 (the groove's first fill tom).
  - **MuldjordKit**: the download in `downloads` was truncated (MD5 not the page's
    `8a66a3e9…`, no zip directory) — deleted; to fetch again.
  - **Virtuosity Drums, Big Rusty Drums** as Tympanum kits: not done (see *Left*).
- **(4) The pack modes' download rules** (`install.rs`, `manifest.rs`): *Manual* is never
  fetched (`InstallError::Manual { page }`; the file the user brings installs through the
  same checks); *From the author's site*: a changed hash, an HTTP error, a web page or a
  short file → `InstallError::AuthorChanged { page, reason }` (the view opens the page and
  offers *Install from a file…*); the manifest refuses an author-site pack whose file is
  not on the author's own host (`file://` exempt). `tests/pack_modes.rs` — **verified only
  in emulation** (a loopback server).
- **(5) The recorded kit's own room by default**; `room` 1–3 puts one of Cantus's rooms
  (the modelled kit's late field) on its room mics, fed by its other mics
  (`Outputs::room_from`). Aula's convolution there: *Left*.
- **(6) Commands**: kit change carrying routes and keys, mic moves, bleed cells
  (`tympanum_cmd.rs`) with the undo-fuzz arm (green), and the plans.
- **(7) Pulsus-only paths**: `with_kit_strips_following` generalised — a Tympanum's (or
  Chorda's) output strips follow it to a new strip in the same undo step
  (`tests/inst_out_follow.rs`). Pulsus-only, with why: `padout::heard_on` (a pad's route
  through its chain — named outputs have no chains) and `padout::carry_outputs` (a pad
  kit's swap; Chorda has `chorda_source::carry_outputs`, Tympanum's kit change carries its
  routes in `tympanum_cmd::change_kit`). **Asked of the integrator**:
  `cantus-project/src/dawproject/write.rs` reports a split Pulsus kit as dropped
  (`interop.report.x_split_kit`); a Tympanum with output strips should say the same, with
  `instrument.output_strips().len()` as `n` (not my file).
- **(8) `rt_memory`**: `tympanum_split_and_changing_kits_never_allocates` — the studio kit
  with every mic on its own strip, the Feel card's timing, the pedal, then a recorded kit
  streamed in while it plays; serially and on the pool; not one allocation or free.
- **(9) tymp-ui's keys**: `timing` (each hit up to that many ms *late* — no look-ahead, so
  never early), `vary`, `hat.chick`, kit-wide `tune` and `ring`, `p{n}.pos` (−1 the centre
  … 1 the edge, **0 the piece's own spot** — an offset, not the view's 0…1 "from
  centre"), `p{n}.spread` (default 0.06, the model's own scatter), `floor`/`walls`/`rug`
  (indices into the view's lists, 0 the room's own; a modelled room's reflections follow);
  `humanize` now plays. Each mic's peak at `METER_TEL + n`
  (`cantus_engine::inst::tympanum::METER_TEL` = 16). `tympanum_kit::heard_db` (with
  `heard::{direct_gain, mic_now, pattern_gain}`) for *What it hears*. Also asked by the
  coordinator: `PieceDef::label` (locale keys `tympanum.piece.*`, both languages: the
  views should show `t(&p.label)` when it is set) and `KitDef::finish`
  (`chorda_look::Finish`, the studio kit Maple; `shoot_scene`'s kit got the field).
- **(10) Joining `ALL`**: waits for the word.

Also: clippy `-D warnings` on every file of mine the coordinator listed, and the new ones.

## How it is tested

- `cantus-dsp` `pulsus::acoustic::tests`: silent without its state; every acoustic voice
  the same bits in blocks of 1, 37, 256 and 512; no thump below a kick's lowest mode; a
  harder hit's attack brighter (snare, tom, kick). `dump_hits` (ignored) writes dry hits.
- `cantus-engine` `inst::tympanum::tests`: the studio kit the same bits in blocks of 1, 37,
  64, 333 and 512; every new key at its default touches no sample, and moved changes it;
  the Feel card's timing lands each hit on its own frame whatever the block, never early;
  the foot's chick off; the mics' peaks in the telemetry. `rt_memory` as above.
- `cantus-content`: `pack_modes.rs`, `licences.rs`, `content.rs` (the bundled manifest
  with both new packs valid), `tympanum_build.rs`, `kit_build.rs`.
- `cantus-core`: the undo fuzz; `inst_out_follow.rs`.

**Verified only in emulation**: the pack modes (a loopback server); streaming under a live
device (offline renders only). Nothing hardware-bound; `beta-checklist.md` has the steps.

## For the owner to listen to

`F:\projects\daw-r40\compartir\tympanum-b2\NOTA.md` (the second listen) and
`F:\projects\daw-r40\compartir\tympanum-b3\NOTA.md` (the third), in Spanish.

## After the second listen (the owner: *"Sigue mejorándolo"*)

- **The modelled kit**: the cymbals' stick ping on their high modes at once (a ride now
  peaks on its ping, ~2 ms, its wash under it; DRSKit's peaks at once), a brighter ride, the
  crash's ping, the hats a touch brighter; the sidestick on the acoustic model (a wooden
  click and the rim). Levels kept against DRSKit's.
- **A player's feel** (the unit, `tympanum.rs`): figures on the snare's keys past General
  MIDI — **flam** 91 (the grace 18–28 ms before, 0.4 of the stroke), **drag** 92, **buzz** 93
  (eight bounces, quicker and softer), **held roll** 94 (strokes every 55 ms while the note
  is held; its note-off ends it); the hat's **foot splash** 95; the kick's **beater** (kit key
  `beater`, 0 felt … 1 plastic: a shorter contact, a brighter click). Unit tests on the
  pending strokes.
- **Aula on a recorded kit's rooms 1–3**: `UnitKind::DrumRoom(n)` (`inst/mod.rs`, the way
  `Zones` sizes an Exemplar) — the snapshot asks for a unit built off the audio thread
  with Aula's factory response of the room (Small → *small*, Studio → *studio*, Hall →
  *concert*), the other mics sent to it, its return on the room mics. Only such a unit pays
  for the convolution. Until it arrives, the late field plays. (`listen_fork.rs` got the
  arm the new kind needs.)
- **Packs**: **MuldjordKit, every microphone** (`muldjordkit-tympanum`, 13 mics — the
  trigger channel left out, the two kicks' and two rides' mics kept apart; CC BY 4.0 on
  the page and in its README; MD5 `8a66a3e9…24f2` = the page's, SHA-256 `db94f910…4c96`,
  2 536 576 789 B; 2 721 963 340 B installed). **Virtuosity Drums** (`virtuosity-drums`, CC0
  on GitHub, release v0.925, 1 227 151 376 B, SHA-256 `c6c5d0fe…963a`, 1 469 668 563 B
  installed; 7 mics read from its level controllers). **Big Rusty Drums** gains its
  Tympanum kit (version 2; its SHA-256 re-checked = the manifest's): its 32 faders are per
  piece and per mic, so the SFZ mic rule now falls back to its sample folders (6 mics) —
  tested with its published program files (`tests/fixtures/sfz/big_rusty_drums`).
  A DrumGizmo build keeps only the channels the manifest lists, and pairs `…L`/`…R` only
  where the manifest pans them apart.
- **DAWproject**: a Tympanum with output strips is reported like a split kit
  (`interop.report.x_split_outputs`, both languages) and written whole; its named outputs'
  strip ids are cleared on the way out and on the way back (they would have pointed at
  nothing or at another strip). Test in `dawproject_padout.rs`.

## Left

- Join `InstrumentKind::ALL` (the null corpus item measured, the contract twins, the bench,
  the agents' `INSTRUMENTS`) — on the coordinator's word, with `tymp-ui`.
- The modelled kit by the owner's ear (the third listen).
- The player's figures are a modelled kit's; a recorded kit plays its pack's own
  recordings of them.

## Checkpoint B (kept): what was in

| Piece | Where | What |
|---|---|---|
| Kit definitions | `cantus-core/src/inst/tympanum_kit.rs`, `tympanum_hits.rs`, `tympanum_studio.rs` | `Place` in the design's frame (cm; x to the audience's right, y towards the audience, z up; `from_design` converts `kit-model.json`); `MicDef` grew `pan`, `level_db`, `spread`; `RoomDef::back`. A recorded kit's strikes (`KitHits`: articulations → velocity layers → takes → one `MicTake` per mic) and the lock-free registry the app's loader fills (`register`, `recorded`). The **studio** modelled kit: the design's 12 pieces and 12 mics where the picture puts them; `piece_pad` (Pulsus's modal voices per articulation) |
| Named outputs | `cantus-core/src/inst_out.rs`, `inst_out_plan.rs` | `family_of_name` (English, Spanish, the packs' words: `KDrumInside`, `FTom1`, `AmbL`, Gogodze's `front/side/wndw/retro`, bitKlavier's names); `definition` (every output of an instrument with its route); `plan` — *Each to its own strip* (strips in output order after the instrument's strip, 0 dB law), *Pieces to strips* (one per family), *All to Main* (strips nothing else uses removed); Pulsus keeps ADR 0100's split/merge through it |
| DSP | `cantus-dsp/src/tympanum*.rs` | `Outputs` (each mic a lane, mixed with `OutMix` and `OutDelay`, emitted **in output order**); `take_index` (one draw per hit, never the take before on the key, pure); the recorded engine (`rec`: one `Playhead` per mic of the drawn take, in memory or streamed — ADR 0050's reader; the hat crossfaded between its two nearest recorded levels by CC4 at the hit and following during it, choked by a less open strike or by the pedal, the foot's chick when it closes fast; SFZ chokes; polyphony); the modelled engine (`model`: Pulsus III lanes per piece into rings, virtual mics reading each piece by its paths — direct, floor, ceiling, walls — fractional delay, 1/r, pattern, the head's dipole polarity, off-axis colour and proximity; the room's late field, `room`) |
| Engine | `cantus-engine/src/inst/tympanum*.rs`; one hook each in `samples.rs` (`wants_of`) and `inst/mod.rs` (`units_for` → `cache::prepare`) | the unit (keys read allocation-free from the spec: `p{n}`, `m{n}`, `b{p}.{m}`, `o{n}`, `room`, `amb.*`, `bleed`, `gain`); the kit cache (modelled kits ready, recorded kits converted, the app's resolver); the files a kit plays = only mics that are on and cells that are on |
| App | `cantus-ui/src/tympanum_load*.rs`, `views/inst/sampler_sfz.rs` (`flatten`, `parse_flat`), `views/inst/sampler_sfz_mics.rs`; one line in `app.rs` (`assemble` installs the loader) | the SFZ mic rule (ADR 0123 §5, logged); an SFZ multi-mic kit read with every controller at its `set_ccN`, a mic-choosing controller (Gogodze's *Mic mix*) left to the mixer, hats on CC4, round robins as takes, `group`/`off_by` as chokes; pieces by the General MIDI map, placed as the studio kit's |
| Packs | `cantus-content/manifest/content.json` (format 5) | Gogodze Phu Vol. II gains its Tympanum kit (`Programs/Kit.sfz`, `plays: tympanum`), version 2; licence re-read 2026-11-12 (CC0 1.0 in the archive's `LICENSE` and on GitHub), size 139 517 148 and SHA-256 `3ff7728f…7d49` verified on download |
| Content | `cantus-content/src/drumgizmo.rs` | DrumGizmo's `channelmap`/`main`, `<chokes>`/`choketime`, `<metadata>` (licence, author, title, `<image>`/`<clickmap>`), `samplerate`, the MIDI map — read; not yet played (below) |
| Strings | `locales/areas/tympanum.{en,es}.txt`, block `# r40:tymp-engine` | `cmd.outputs_each`, `cmd.outputs_family`, `cmd.outputs_main` |

### Checkpoint B: how it was tested

- `cantus-ui/tests/tympanum_e2e.rs`, through `CantusApp::assemble` and the real engine: a
  recorded kit installed as a pack (3 mics, folders, CC4 hats, round robins) and the
  modelled studio kit (12 mics) each play a groove; each mic sounds soloed; *Each mic to
  its own strip* (one undo step, a strip per mic into the kit's strip) **plays the unsplit
  kit to the bit** offline at 512 and 333 frames and live on the processor; *All to Main*
  brings it back. Gogodze's **published program files** (`tests/fixtures/sfz/
  gogodze_phu_vol_ii`, CC0, byte for byte) read into seven mics by their folders, four hat
  levels on CC4, every strike heard by all seven mics, the open hat choked by the closed,
  the tuning knob at its default.
- `cantus-dsp` (`tympanum`): the draw is pure; a take never repeats the one before; every
  mic of a hit plays the same take; a strike renders the same bits in chunks of 1, 37, 64,
  255 and 256; a hat between two levels plays both and follows the pedal, and a closed
  strike chokes it; the room decays by its RT60 and comes to rest; a zero delay is no line.
- `cantus-core`: the studio kit matches the design; `family_of_name`; a layer's velocity
  curve.

**Verified only in emulation**: nothing hardware-bound yet. Streaming was exercised by the
offline render of Gogodze (1 701 files), not under a live device.

### Checkpoint B: the first listen

`F:\projects\daw-r40\compartir\tympanum-b\` — `NOTA.md` (in Spanish) says what each file
is: the groove through Gogodze's full mix and each of its seven mics soloed, and the
modelled studio kit's mix and four of its mics.

### Checkpoint B: what was still to do

DrumGizmo kits played (multichannel streams: one stream per hit serves every mic), DRSKit
per-mic, CrocellKit, Virtuosity, Big Rusty, MuldjordKit (licences and hashes from their
pages); the pack modes' download rules in `install.rs`/`fetch.rs`; Aula's rooms on a
recorded kit's room mics; the commands (kit change carrying routes and keys, bleed, mic
moves) with their undo-fuzz arms; `heard_on`/`carry_outputs`/templates/DAWproject notes
generalised; Tympanum into `InstrumentKind::ALL` (null corpus, contract twins, bench,
agents' `INSTRUMENTS`); `rt_memory.rs` with every output split and a kit loading;
migrations 66/67's fixture blocks checked; the modelled kits' sound by the owner's ear.
