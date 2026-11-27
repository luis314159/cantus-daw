# How a drum kit is simulated: the physics and the mathematics, element by element

*2026-11-26. The owner asked (Spanish): "Tal vez deberías hacer más estudios de cómo se simulan
los elementos de las baterías, cómo se describen matemáticamente." This survey studies how each
element of a kit is simulated and described mathematically. The elements are the heads, the
strike, the snare wires, the cymbals and hats, the room and mics, and the variation between hits.
For each it gives the physics, the equations, the state of the art and what it costs in real time.
It then judges where Tympanum's code stands against that state of the art. It ends with a
recommendation per element (sample, model or both) and a ranked plan that fits the sample-first
pivot (`docs/round42/tympanum-real.md` §8, branch `r42/tympanum-real`). It writes no code.*

It builds on, and does not repeat:

- ADR 0124 and its six amendments: models 4 (*acoustic*), 5 (*measured*), the hybrid residual,
  and the level law;
- `docs/round42/tympanum-real.md`: the library, the measures (§3), the findings (§4), brushes
  (§5) and the pivot (§8);
- `crates/cantus-dsp/src/pulsus_acoustic*.rs`, `pulsus_measured*.rs` (model 6's onset is
  `pulsus_measured_onset.rs` on `r42/tympanum-real`), `tympanum_rec*.rs`, `tympanum_room.rs`;
- the element-by-element listening pages at
  `F:\projects\cantus-work\listen\tympanum-real\analisis-elementos\`. They hold the kick
  (`bombo`) and snare (`caja`) so far: clips at pp/mf/ff for every kit, spectrograms, thirds,
  decay, partials and variation.

## How to read the labels

| Label | Meaning |
|---|---|
| **[L]** | Published literature (paper, thesis, book), read or checked for this survey |
| **[R]** | An independent review (Sound On Sound unless named) |
| **[V]** | A vendor's product page or marketing: a claim, not a measurement |
| **[C]** | Cantus's own code or documents, read for this survey |
| **[I]** | My inference or a back-of-envelope estimate: reasoning, not a source |
| **NV** | Not verified today: from memory or a secondary citation |

Every paper in §12 was checked against a publisher page, a repository record or its PDF. Public
PDFs of about thirty papers were read as text in the session's scratch folder and are not kept in
the repository. No audio and no sample library was downloaded.

---

## 0. The short version

### 0.1 What the physics says, in one paragraph per element

1. **A head is a 2-D membrane under tension.** Its modes are Bessel functions. Their frequencies
   are *inharmonic*: 1 : 1.59 : 2.14 : 2.30 : 2.65 … for an ideal head. They are pulled towards
   harmonic by the air: the air's mass lowers the modes with nodal diameters, and the air in the
   shell stiffens the round ones. A tom or a kick has two heads, coupled through the air inside.
   Its lowest modes split into close pairs and beat. A hard stroke stretches the head and raises
   its tension. The pitch starts high and glides down as the energy decays (a Kirchhoff–Carrier /
   Berger nonlinearity). The glide is roughly proportional to the head's energy. Where the stick
   lands sets each mode's amplitude: the mode's shape at that point.
2. **The strike is a collision, not a pulse.** A stick or beater is a mass meeting a compliant
   head through a nonlinear contact force: Hertz, or Hunt–Crossley with its damping. Measured on
   a real snare and tom, the contact lasts **3–8 ms** at mezzoforte, not ~1 ms. It is shorter for
   harder strokes and nearer the rim. Its force has secondary bumps when the head's travelling wave
   comes back from the rim. The stick rings too, with its own modes near 0.4, 1.0, 1.7 and
   2.7 kHz for ~100 ms. A harder hit is brighter because the contact gets shorter and stiffer. In
   a Hertz contact the contact time falls only as velocity^(−1/5).
3. **Snare wires are many thin strings colliding with the snare-side head.** That head is driven
   by the batter head through the air in the shell. The buzz is a distributed, one-sided collision
   (a power-law penalty). It is the hardest part of a drum to simulate stably. The best
   simulations (NESS, Edinburgh) run far from real time; cheap ones gate noise by the head's
   motion.
4. **A cymbal is a thin, curved plate driven far into its nonlinear range.** The von Kármán
   equations couple its modes cubically. A hard hit passes through three regimes: periodic,
   quasi-periodic (energy exchanged between modes), then **wave turbulence**. In the last, energy
   cascades to high frequencies: the crash's swell and wash. Full simulation is real time only
   for small gongs on one core (2023); a kit's cymbals are not. A hi-hat is two such shells in
   contact.
5. **Mics and room are linear and well understood.** Each mic hears each piece through a delay,
   a 1/r level, its polar pattern, the head's directivity, the proximity effect up close, and the
   room's reflections. What makes bleed sound real is *coherence*. Every mic must hear the same
   strike, and a sampled kit gets that only if every mic plays the same take.
6. **No two real hits are the same.** Strike position, velocity, stick angle, the head's state
   and what is still ringing all change the sound. Real drummers also change the sound with
   intent: laid-back strokes are louder (Danielsen et al. 2015). A sample engine has round robins.
   A model has the variation for free, if its inputs vary physically.

### 0.2 The finding that matters most for Tympanum's code

- **Model 6's contact is three to five times too short.** `pulsus_measured_onset.rs` drives the
  modes through a half-sine of 0.8 ms (ff) to 1.8 ms (pp); model 4 uses 0.35–2.2 ms for a stick.
- The measured contact on a snare and a tom is 3–6 ms (Wagner 2006, a KTH thesis with a
  contact-sensing stick) and 5–8 ms at mf (Dahl 2011's survey). It falls with velocity and towards
  the rim.
- The recorded snares' bands peak at 5–11 ms with the lows last (`tympanum-real.md` §4.4). That
  is what a 3–8 ms contact with returning wavefronts produces. That section's "the stick's
  contact lasts ~1 ms" is not what the literature measured.
- This bears on the modelled family only. The sample-first flagship kits have the real contact in
  their recordings.

### 0.3 Sample, model, or both: the recommendation per element

| Element | Flagship kits (sample-first) | What modelling adds on top | The method and first paper |
|---|---|---|---|
| Head body (kick, snare, toms) | **sample** | retune, damp and glide without resampling the stick and room | sinusoidal + residual decomposition (Kirby & Sandler 2020); energy-based glide (Avanzini, Marogna & Bank 2012); extra loss per mode |
| Strike position | **sample** both zones where recorded | a continuous `pos` between them | crossfade with one draw; the modal re-weighting `J_m(j_mn r/a)` only for the decomposed part |
| Velocity | **sample** layers | a seamless join between layers | a spectral-tilt morph between adjacent layers, fitted per pair (§8.5), not a waveform crossfade |
| Excitation (sticks, beaters) | **sample** | — | (modelled family only: Hunt–Crossley mass on the modal head, Avanzini & Rocchesso 2001) |
| Rimshot, cross-stick | **sample** | — | no simulation literature found; the recordings are the model |
| Brushes, taps | **sample** | — | — |
| Brushes, sweeps | **sample** loops | speed and pressure control | a stochastic bristle-impact process (Cook 1997) through the snare's measured bands, matched to Big Rusty / Unruly / Swirly |
| Snare wires, own buzz | **sample** | — | (modelled family: Avanzini & Marogna 2010's lumped wire on the modal head) |
| Sympathetic buzz and tom ring | in the recordings when the whole kit was set up | **model** when a piece is retuned, damped, muted or swapped | modal banks of each piece (model 5's tables) driven by the other pieces' dry lanes (Clifford, Lindsay Smith & Reiss 2012) |
| Cymbals (crash, ride, china) | **sample** | the re-strike swell only | — (von Kármán is out of real time for a kit) |
| Hi-hat | **sample** at every recorded opening | continuous CC4 between openings (present) | — (Sekiguchi & Samejima 2023 is offline) |
| Close mics, overheads, bleed | **sample**, one take for every mic (present) | rebalance and polarity; a modelled kit's mics move | delay, 1/r, polar pattern, proximity, head beaming (present for modelled kits) |
| Room | the kit's own room mics (present) | Cantus's rooms (present) | measured responses per mic family (Aula) |
| Variation | round robins (present, `vary`) | a measured, small per-hit change beyond the takes | the recordings' own take-to-take statistics (§8.4), not a free random draw |

### 0.4 The ranked plan (details in §10)

1. **Sympathetic resonance and snare buzz from other pieces**, modelled from the pieces' own
   tables and driven by the other lanes. It is the one place modelling beats samples whenever the
   kit is changed. One listening test with an FXpansion co-author could not tell it from the real
   thing (Clifford et al. 2012).
2. **A measured variation layer** on top of the round robins. It draws from the recordings' own
   take-to-take statistics: third-octave levels, f0 in cents, onset timing and a position tilt. It
   is the "infinite round robin" that MODO DRUM's review praised.
3. **A velocity join without cancellation**: spectral morphing between adjacent layers. The
   physics says brightness changes smoothly with velocity.
4. **Retune and damp a sampled drum by decomposition.** Retune and re-damp only its modes, glide
   included, and leave the stick, wires and room as recorded.
5. **For the modelled family**: replace the half-sine onset with a real contact, a Hunt–Crossley
   mass on the modal head at the measured 3–8 ms. Add the energy-based glide. Then brushes as
   many-bristle contacts.

---

## 1. Tympanum today, read from its code

### 1.1 The voices

| Voice | What it is | Where | Its physics |
|---|---|---|---|
| Model 4, *acoustic* | modal synthesis from the ideal membrane's Bessel table | `pulsus_acoustic*.rs` [C] | 64 Bessel ratios (`MEMBRANE64`) compressed towards harmonic by `ratio = ideal^(1 − 0.18·heads)`; the strike point's mode shape `|J_m(j·ρ)|`; a radiation weight `1/(1 + 0.55 m)`; T60 ∝ `(f0/f)^γ`; the lowest five modes doubled a few cents apart (two heads); three shell resonances; a half-sine contact (stick 0.35–2.2 ms, beater 1.6–7 ms) driving the modes by its second difference; wires as noise gated by the low modes; cymbals as a 28-mode low bank whose square drives a 52-mode high bank |
| Model 5, *measured* | analysis–resynthesis from DRSKit / Virtuosity tables | `pulsus_measured*.rs` [C] | up to 512 modes per piece (frequency, amplitude per layer, T60, bloom); 38 quarter-octave noise bands, each with 22 envelope breakpoints over 0–3.2 s; a stick burst and a knock; the glide fitted per layer; random phases and ±1.5 dB per mode each hit |
| Model 6 | model 5's tables entered through a contact | `pulsus_measured_onset.rs` (r42/tympanum-real) [C] | every mode grown by the integral of a half-sine force 0.8–1.8 ms long (×2 for a felt beater); the bands rising over 6 ms (100 Hz) to 2 ms (8 kHz); the wires lagged 1.5 ms through the cavity |
| Hybrid | model 5 plus a recorded spectral residual for the first 30–200 ms | `tympanum_residual.rs` [C] | the recording's power the model lacks, under a mask per STFT bin |
| Recorded | multichannel sample playback | `tympanum_rec*.rs` [C] | one take per hit on every mic; bleed matrix; continuous hat openness; round robins with `vary`; tune by resampling, damp by an envelope |
| Room | modelled kits' late field | `tympanum_room.rs` [C] | eight-line FDN with Jot's loss rule; early reflections as image paths of the virtual mics |

### 1.2 Where each stands, in one line

- **Model 4** is a "physically informed" modal model in Cook's sense (Cook 1997). Its ingredients
  are right: Bessel modes, strike-point weights, radiation, pairs, glide, gated wires. But each is
  a heuristic constant, not the physics. The air loading is a power-law compression where the
  literature has a mass load on some modes and a stiffness on others (§3.2). The glide is not tied
  to the head's energy (§3.4). The contact is not a collision (§4).
- **Model 5** is analysis–resynthesis: deterministic modes plus dynamically filtered noise. It is
  the method of Fletcher & Bassett (1978) and Aramaki & Kronland-Martinet (2006), refined. It is
  the right family for *data*, but it throws away the waveform's phase structure in the first
  tens of milliseconds (random phases, noise bands). That is where the owner hears it fail (§4.7).
- **Model 6** fixes the timing of the onset with a contact force, but a contact three to five
  times shorter than measured (§0.2).
- **The recorded path** has what the best sample engines have (one take for all mics, a bleed
  matrix, a continuous hat). It lacks what §8 of the plan lists: the velocity join, more
  variation, position morph and sympathetic buzz.

---

## 2. The simulation methods, briefly

Every element below is simulated by one of six families. They differ in what they assume and
what they cost.

| Method | Idea | Strength | Weakness | Key sources |
|---|---|---|---|---|
| **Modal synthesis** | the solution as a sum of the linear system's modes, each a damped oscillator `q̈ + 2σ q̇ + ω² q = φ(x_s) f(t)` | exact for linear systems with known modes; cost linear in the mode count; any loss per mode | modes must be known (analytic for a circular head, not for a real shell); nonlinear coupling becomes a dense tensor | Adrien 1991; Avanzini & Marogna 2010; Ducceschi & Touzé 2015 |
| **Finite differences (FDTD)** | the PDE on a grid, stepped in time; energy-conserving schemes guarantee stability | handles any coupling (heads, air, wires, collisions, nonlinearity) | cost ∝ grid points × sample rate; numerical dispersion; collisions need a solve per step | Bilbao 2009 (the book); Bilbao 2012; Torin et al. 2014 |
| **Digital waveguide mesh** | 2-D/3-D grids of delay lines and scattering junctions | simple, local | equivalent to a finite-difference scheme, with direction-dependent dispersion | Van Duyne & Smith 1993; Savioja & Välimäki 2003 |
| **Functional transformation (FTM)** | analytic transforms of the PDE into parallel recursive filters | exact modes for simple shapes, tension modulation in closed form | simple geometries only | Trautmann & Rabenstein 2003; Petrausch & Rabenstein 2005 |
| **Analysis–resynthesis** | partials and noise bands measured off recordings, resynthesised | sounds like *that* drum; cheap | only the measured drum; phases and transients are where it fails | Fletcher & Bassett 1978; Aramaki & Kronland-Martinet 2006; Kirby & Sandler 2020 |
| **Sampling** | play the recording | real by construction | fixed drums, finite layers and takes, size | (the commercial engines, §2.1) |

"Physically informed" synthesis (Cook 1997, PhISM) sits between them. It keeps the physics'
structure (modes, a stochastic excitation, a filter for the body) with parameters set by ear or
by measurement. Tympanum's model 4 is that.

Commuted synthesis (Smith, *Physical Audio Signal Processing*, CCRMA) is a reminder that matters
here. A linear time-invariant body can be moved before the nonlinearity and folded into a recorded
excitation. Bleed, mic colour and the room are all linear, so a recorded kit can be *rebalanced*
freely: what each mic hears is a filter of what was played. The strike itself cannot be moved
that way.

### 2.1 What the products do

| Product | Kind | What is known | Label |
|---|---|---|---|
| Toontrack Superior Drummer 3 | sampled | core library ~230 GB raw at 44.1 kHz/24-bit, recorded by George Massenburg at Galaxy Studios; close mics, bleed channels, an 11-mic surround room | [V] Toontrack's page, [R] MusicRadar, MusicTech |
| FXpansion BFD3 | sampled | many velocity layers (up to 80 on a snare, per one review), 13 hat articulations; the snare's drags, flams, half edge, rimshot, side stick and rim click | [R] SOS 2014; the layer count NV |
| XLN Addictive Drums 2 | sampled | 8–16 layers per piece, up to 8 round robins a layer; light on resources | [R] bonedo, MusicRadar |
| IK Multimedia MODO DRUM | modal synthesis + samples | drums "modal synthesis" with head, shell, edge, wires and damping controls; **cymbals sampled**; "infinite round-robin"; a 6 GB install suggests sampled data | [R] SOS, Dec 2019 (Nick Magnus) |
| Roland Prismatic Sound Modeling (TD-50X …) | modelled | "does not use sampled waveforms"; recordings are used for analysis; the snare separated into body, wire and overtone, each modelled | [V] Roland's engineers |
| AAS Chromaphone 3 | physical resonators | a membrane and a tube make kicks, snares, congas; the kicks lean electronic | [R] SOS, MusicRadar, Future Music |
| NI Maschine Drum Synths | synthesis engines | kick engines "based on classic analog drum machines and acoustic models" | [V] |
| Sunhouse / Evans Sensory Percussion | trigger + machine learning | up to 10 zones per drum classified by ML from a sensor; it plays samples | [V] D'Addario |
| Kirby & Sandler (QMUL) | research | tom components resynthesised from the real DFT so well that an ABX test could not tell them apart | [L] DAFx 2020 |

What the reviewers say about the modelled ones:

- **MODO DRUM**: the kits "sound no less like real drums than you'd expect from a top-quality
  sample library", with no sign of sounding "synthesized"; the infinite round robin "clearly works
  well". The verdict calls it "a valuable complement to other drum libraries". The cons include
  "only one articulation for the cymbals" [R].
- Kirby & Sandler put it plainly in their introduction. Sample-based instruments like Superior
  Drummer 3 "are currently the most convincing way" to replicate an acoustic kit. Drum synthesis
  sounds "clearly synthetic and can't compete with sample-based drum machines in terms of
  realism". Finite differences are, for now, too costly for real time [L].
- No review found rates Chromaphone's drums as realistic acoustic kits.

The pattern: **the modelled products that reviewers call realistic model the drums and sample the
cymbals** (MODO DRUM). Or they are hardware whose maker says nothing about the method beyond
"analysis of recordings" (Roland). None simulates a cymbal from physics.

---

## 3. Membranes: kick, snare and tom heads

### 3.1 The 2-D wave equation and the Bessel modes

A head of radius `a`, tension `T` (N/m) and surface density `σ` (kg/m²), clamped at the rim:

```
σ ∂²z/∂t² = T ∇²z                       c = √(T/σ)
z_mn(r, θ, t) = J_m(j_mn r / a) · cos(m θ) · cos(ω_mn t)
f_mn = j_mn · c / (2π a)
```

`j_mn` is the n-th zero of the Bessel function `J_m`; `m` counts nodal diameters, `n` nodal circles
(the rim included). The first ten [L Fletcher & Rossing ch. 18; computed]:

| Mode (m,n) | (0,1) | (1,1) | (2,1) | (0,2) | (3,1) | (1,2) | (4,1) | (2,2) | (0,3) | (5,1) |
|---|---|---|---|---|---|---|---|---|---|---|
| `j_mn` | 2.405 | 3.832 | 5.136 | 5.520 | 6.380 | 7.016 | 7.588 | 8.417 | 8.654 | 8.772 |
| ratio to (0,1) | 1.000 | 1.593 | 2.136 | 2.295 | 2.653 | 2.917 | 3.155 | 3.500 | 3.598 | 3.647 |

Every mode with `m ≥ 1` is degenerate: a cosine and a sine twin at one frequency. A real head's
uneven tension splits each twin into two close frequencies that beat (Worland 2010 [L]). So does
the second head (§3.3). This is one cause of the slow, irregular amplitude beating in a real tom's
tail.

Fletcher & Bassett (1978) measured a bass drum whose two heads were tuned near 40 Hz. Its
components agreed with this theory to within 1 % in most cases and 5 % in some. The frequencies
*fell* as the amplitude decayed (§3.4) [L].

### 3.2 Air loading and the enclosed cavity

The air moves with the head, and it acts differently on the two kinds of mode (Christian et al.
1984 for timpani; Avanzini & Marogna 2010 for drums) [L]:

- **Modes with nodal diameters** (`m ≥ 1`) push air from one side of the line to the other. They
  compress nothing. The air adds *mass*, which lowers their frequencies, the low ones most.
- **Axisymmetric modes** (`m = 0`) change the enclosed volume. The air in a closed shell adds
  *stiffness*, which raises them. Its lumped form is the adiabatic air spring
  `K_air = ρ₀ c₀² S_eff² / V`, where `S_eff = ∫ φ dS` is the mode's volume displacement and `V`
  the shell's volume (Avanzini & Marogna 2010 model exactly this as a spring/damper on the (0,1)
  mode).
- **Radiation**: the (0,1) mode is a monopole and radiates best, so it loses its energy
  fastest. The others are dipoles and quadrupoles and radiate less (Rossing et al. 1992; Zhao's
  measurements) [L].

On a timpani the air loading moves the (1,1), (2,1), (3,1), (4,1), (5,1) modes to nearly
harmonic ratios of about 1 : 1.5 : 2 : 2.5 : 2.9. That is why a timpani has a pitch and a tom has
a vaguer one (Fletcher & Rossing ch. 18; the decimals NV).

**In a modal model** this is not a compression of every ratio. It is a per-mode correction: a
mass term lowering the `m ≥ 1` modes, an air spring raising the `m = 0` modes, and, with two
heads, the coupling of §3.3. Model 4's `ideal^(1 − 0.18·heads)` [C] compresses all modes alike. It
cannot raise the (0,2) mode while lowering the (1,1), which the physics does.

### 3.3 Two heads coupled through the air

A tom or a kick is two membranes sharing one volume of air. The air spring couples their
axisymmetric modes:

```
m₁ ẍ₁ + k₁ x₁ = −K_air (x₁ − x₂)        (batter head, mode (0,1))
m₂ ẍ₂ + k₂ x₂ = −K_air (x₂ − x₁)        (resonant head)
```

(both displacements measured in the same direction in space, so that both heads moving that way
together, one into the shell and the other out of it, compress nothing). Two close tunings
give a *doublet*. One frequency has the heads moving together, the other against each other, and
their spacing grows with the coupling. Avanzini & Marogna's simulations show exactly this: strong
coupling between similarly tuned heads gives doublets, "especially noticeable in the low frequency
range". Their paper lists what is still missing: the shell's role in the coupling and the cavity's
own resonances [L].

Rossing et al. (1992, the snare) and Obata & Tesima (1935) measured the pairs. Fletcher & Bassett
(1978) saw "a splitting of the resonances" from the closely coupled heads [L]. The resonant
head's own tuning sets how long a tom sings and how its pitch sits. This is the control drummers
use most after the batter's tension.

The full treatment couples both heads to a 3-D air field inside and outside the shell, by finite
differences (Bilbao 2012; Torin & Bilbao 2013; Bilbao & Webb 2013 for timpani) [L].

### 3.4 Tension modulation: the pitch glide

A hard stroke stretches the head. Its area grows, so does its tension, and every mode starts
sharp and falls as the vibration decays. The model, in Berger's approximation of the von Kármán
plate (Berger 1955), is the 2-D analogue of the Kirchhoff–Carrier string (Avanzini & Marogna 2010;
Marogna & Avanzini 2009) [L]:

```
σ ∂²z/∂t² = (T₀ + T_NL(z)) ∇²z − D ∇⁴z − (losses) + f(r, t)

T_NL(z) ∝ (E h / (1 − ν²)) · (A(z) − A₀) / A₀,     A(z) − A₀ ≈ ½ ∬ |∇z|² dS
```

(`E` Young's modulus, `h` thickness, `ν` Poisson's ratio of the head film; the exact constant as in
the papers, NV here.) The tension is spatially uniform and depends only on the total stretch.
Berger's form is accurate for clamped edges and gives the glide. It cannot give a cymbal's crash
(§6) [L Avanzini & Marogna 2010, citing Bilbao 2005].

**The cheap, accurate form: the glide follows the energy.** Avanzini, Marogna & Bank (JASA 2012;
Marogna, Avanzini & Bank, DAFx 2010) showed that the short-time average of the extra tension is
nearly proportional to the head's total energy. The double-frequency part of the tension is
inaudible by comparison. So a modal model needs only:

```
E(t)   = Σ_k ½ M_k (q̇_k² + ω_k² q_k²)          (the head's energy, from the modal states)
T̄_NL(t) ≈ β · E(t)
ω_k(t) = ω_k,0 · √(1 + T̄_NL(t) / T₀)            (every mode retuned together, once a block)
```

They call the extra cost negligible next to the linear model [L]. Three consequences follow.

- The glide falls **as the energy falls**, which is a sum of the modes' own decays. The low modes
  hold it, the high ones drop out fast. A single fixed time constant is an approximation.
- A **re-strike while the head rings** adds energy and pushes the glide again, by the physics and
  with no special case.
- The relative glide is `Δf/f ≈ ½ β E / T₀`, so **a looser head glides more** for the same
  stroke. Retuning a drum down must deepen its glide; resampling a recording does the opposite
  (§3.9).

Kirby & Sandler (2020) found that the phase functions of a tom's modal chirps share one simple
form across strike levels. That lets velocity be blended continuously [L]. Fletcher & Bassett's
40 Hz bass drum was still 5 % sharp 0.4 s after the blow [L].

### 3.5 Stiffness and frequency-dependent loss

A drum head is a thin film (polyester, sometimes with a coating or a second ply), so it has a
small bending stiffness. Losses grow with frequency. The standard lossy, stiff membrane in the
finite-difference literature (Bilbao 2009) [L]:

```
σ z_tt = T ∇²z − D ∇⁴z − 2σ·σ₀ z_t + 2σ·σ₁ ∇² z_t + f
D = E h³ / (12 (1 − ν²))

dispersion:   ω² = c² k² + (D/σ) k⁴            (the high modes a little sharp)
loss per mode: σ(ω) = σ₀ + σ₁ k(ω)²,   T60(ω) = 3 ln 10 / σ(ω) ≈ 6.91 / σ(ω)
```

Two parameters set a whole head's decay curve. Real heads add air damping (radiation, strongest on
the (0,1) mode), the coating, the hoop and any damping (gel, tape, a pillow in a kick). **A
measured per-mode T60, as model 5 has, is better than any two-parameter law** [I]. What the law
gives is a physically sensible way to *change* the damping: add `Δσ(ω)` to every mode (§3.10).

### 3.6 Shell modes and the cavity

- **The shell** radiates little next to the heads. Its coupled motion with the heads carries
  energy and shifts resonances (Rossing et al. 1992, as summarised by Avanzini & Marogna 2010).
  Shell material and bearing edges are heard mostly through that coupling, and through how the
  hoop and edge damp the heads [L].
- **The cavity's own air modes** are the axial standing waves `f_p ≈ p·c₀ / (2L)` of a shell of
  depth `L` (for a 14″-deep kick, the first near 480 Hz) and the radial and azimuthal modes of a
  cylinder. Torin, Hamilton & Bilbao (2014) found that adding the cavity and the snares raised
  the snare's mid-frequency energy. Without viscothermal loss in the air it left audible "hiss"
  and "ringing" [L].
- **A kick's port** in the resonant head lets the cavity breathe and shortens the low tail. It is
  a Helmholtz-type leak [I].

Model 4's "three fixed-ratio shell resonances" [C] is a placeholder; model 5 carries the shell in
its measured modes and bands, which is right for a measured drum.

### 3.7 Where the stick lands

For a point force at radius `r_s` (and angle θ_s), mode `(m, n)` is excited in proportion to its
shape there:

```
a_mn ∝ J_m(j_mn · r_s / a) · cos(m θ_s)
```

At the centre every `m ≥ 1` mode has a node: a centre stroke rings the round modes almost alone.
Moving out wakes the (1,1), (2,1) … modes.

| Measure | Centre → edge | Source |
|---|---|---|
| A snare without wires, a forte crescendo at four positions | (0,1) −5 dB, (1,1) +5 dB; the contact *shorter* towards the rim (a stiffer head there, the reflected wave arriving earlier) | Wagner 2006 [L] |
| A snare's batter head, three strike points | the (0,1) mode the strongest only for a central strike; off centre the spectral peaks move away from the modal frequencies | Skrodzka, Hojan & Proksza 2006 [L] |
| A bass drum | a centre blow puts more energy in the low components, an edge blow in the high | Fletcher & Bassett 1978 [L] |
| A snare's centre, halfway, edge and rimshot strokes | classified from audio at over 90 % | Tindale et al. 2004 [L] |
| Virtuosity's rack tom | the (1,1) region over the fundamental −16 … −24 dB at the centre, −7 … −13 dB off it | `position.py`, `tympanum-real.md` §4.7 [C] |

**So position is audible, systematic and cheap to model in a modal model.** It is one weight per
mode, and both contact time and brightness move with it. In samples it is a choice of zone. A
continuous position needs either a crossfade between recorded zones or a modal re-weighting (§3.10).

### 3.8 Methods and their cost for a head

| Approach | What it gives | Cost at 48 kHz on one desktop core | Label |
|---|---|---|---|
| Modal head, linear, ~100–300 modes | Bessel modes, any loss, position | one complex one-pole per mode, ~6–8 flops a sample: 300 modes ≈ 0.1 GFLOP/s, a fraction of a percent of an AVX core | [I] |
| + energy-based tension modulation | the glide, re-strikes, tuning-dependent glide | one energy sum and one square root per control block: negligible | [L] Avanzini, Marogna & Bank 2012 |
| + two heads via a lumped air spring | doublets, the resonant head's tuning | one coupled pair per axisymmetric mode | [L] Avanzini & Marogna 2010 |
| Full nonlinear modal (Berger tension computed at audio rate from all modal displacements) | the same glide, exactly | "hundreds or thousands" of squared modal displacements every sample | [L] Marogna, Avanzini & Bank 2010 |
| 2-D finite differences, one head | any tension field, uneven tuning, coupling to a grid | roughly 10⁴ points for a 14″ head at audio rate, ~10 flops each: a few GFLOP/s, about one core per head | [I] |
| 3-D finite differences with the air (NESS) | heads, cavity, outside air, radiation | 1.67 × 10¹¹ flop/s for a 1 m³ enclosure at 44.1 kHz; 55 s of serial C for 1 s of sound, 7–9 s on a 2013 GPU | [L] Bilbao et al. 2013 |
| Analysis–resynthesis (model 5) | that drum, measured | the drums ≈ 0.5–0.7 % of a core each; a busy kit 7.7–7.9 % | [C] ADR 0124 |
| Samples | that drum, real | memory bandwidth and disk streaming | — |

### 3.9 Tympanum against the state of the art (heads)

- **Modes.** Model 4's Bessel table and strike-point weights are the textbook. Model 5's measured
  modes are better for a measured drum, and they are what MODO DRUM-class products seem to start
  from [C][I].
- **Air and two heads.** Model 4 compresses all ratios by one exponent and doubles the five lowest
  modes by a random 0.3–1.2 %. The physics shifts the two kinds of mode in opposite directions. It
  splits only the axisymmetric ones by the air spring, with the split set by the two tunings. Model
  5 has the doublets only if the fit found them [C].
- **Glide.** Model 5 fits a glide per layer: excess ∝ velocity², one time constant, pushed again on
  a re-strike [C]. It is a curve fitted to the data, not a law. It does not follow the energy, so a
  damped head glides as long as an open one, and a retuned head glides the same. The energy law
  (§3.4) would make it follow the energy for one energy sum a block.
- **Retuning a sample** (the recorded path's `p{n}.tune`) resamples: modes, stick, wires and room
  all move together, and the glide moves the wrong way (§3.4). Every sampled product does this,
  but it limits a sampled kit's tuning to a few semitones [I].
- **Damping a sample** is an amplitude envelope [C]. A real damper shortens the *high* modes' and
  the fundamental's T60 differently and leaves the stick alone. An envelope shortens everything,
  the attack included.

### 3.10 Recommendation (heads)

**Sample the heads** for the flagship kits. **Add a decomposition for tuning and damping**:

1. Split each sampled drum hit, off line, into its modal part and a residual (stick, wires, room,
   noise). Kirby & Sandler (2020) do it via the real DFT, whose chirps carry each mode's glide.
   Sinusoidal tracking with model 5's tables as the frequency prior would also do [L][I].
2. **Retune** only the modal part. Scale its frequencies, and its glide by the energy law
   `Δf/f ∝ E/T₀`, so a lower tuning glides more. Leave the residual unshifted, or shift it far
   less. This should keep the stick and room honest over a far wider range than resampling.
3. **Damp** the modal part by an added loss per mode `Δσ(ω)`: a gel mostly the high modes, a pillow
   the low ones. This replaces the envelope.
4. **Position** within the decomposed part: re-weight each mode by
   `J_m(j_mn r′/a) / J_m(j_mn r/a)` between the sampled zone and the asked-for one. Use it only for
   small moves around a recorded zone, and crossfade recorded zones for large ones (§8).

What to implement first: Avanzini, Marogna & Bank 2012's energy-based glide inside model 5/6
(cheap, and the modelled family gains it at once). Then Kirby & Sandler 2020's decomposition as
an off-line tool, for the retune and damp of sampled kits. **Measure it** with the round's tools:
retune a DRSKit tom by ±2 and ±4 semitones each way and compare its features with the nearest
recorded tom of that size (`compare.py`).

---

## 4. The excitation: sticks, beaters, mallets and brushes

### 4.1 The contact is a collision

A stick or beater is a mass `m` meeting the head at the strike point. The force exists only while
the tip is pressed into the head. The standard lumped model is Hertz's power law, with
Hunt–Crossley's damping for the energy lost in the contact (Hunt & Crossley 1975; Avanzini &
Rocchesso 2001; Avanzini & Marogna 2010) [L]:

```
x(t)  = y(t) − z(x_s, t)                 compression: stick tip y below the head's surface z
F(x, ẋ) = k x^α + λ x^α ẋ    if x > 0,   0 otherwise
m ÿ = −F                                   the stick
z(x_s, t) = Σ_k φ_k(x_s) q_k(t)            the head, modal
q̈_k + 2σ_k q̇_k + ω_k² q_k = φ_k(x_s) F / M_k
```

- `α` is 3/2 for a sphere on an elastic half-space (Hertz). Felt is stiffer as it is compressed,
  with a larger exponent and a hysteresis. Stulov's hysteretic felt model (1995) is the reference
  for piano hammers [L], and the same physics applies to a felt kick beater [I].
- A modal head makes the model cheap. The head's displacement at the contact point is one weighted
  sum over the modes, and the force feeds back into every mode. This is exactly Avanzini &
  Marogna's impact block (2010) [L].
- Collisions are stiff, and naive discretisations of them are unstable or inaccurate.
  - Papetti, Avanzini & Rocchesso (2011) compare the numerical methods for Hunt–Crossley [L].
  - The energy-conserving schemes guarantee stability: Bilbao, Torin & Chatziioannou 2015;
    Chatziioannou & van Walstijn 2013 [L].
  - Two more recent schemes avoid the iteration. Ducceschi, Bilbao, Willemsen & Serafin (2021)
    solve one linear system with no Newton loop. Bhanuprakash, van Walstijn & Chatziioannou
    (2024) are aimed at real-time lumped collisions [L].

### 4.2 Contact time against velocity

**What a real stick does** (a snare and a tom, measured):

| Measure | Value | Source |
|---|---|---|
| Contact between drumstick and head | **3–6 ms**, measured with a copper-taped stick closing a circuit on conductive paint | Wagner 2006 (KTH master's thesis, supervised by Dahl and Askenfelt) [L] |
| Typical contact at mezzoforte, centre of a tom or a snare | **5–8 ms** | Dahl 2011 (a survey, citing Dahl 1997 and Wagner) [L] |
| With velocity | contact time **decreases** as the dynamic level rises | Wagner 2006 [L] |
| With position | contact time **decreases** towards the rim (a stiffer head, the reflected wave arriving sooner) | Wagner 2006 [L] |
| With grip | "controlled" strokes have shorter contact and a higher peak force than normal ones | Dahl & Altenmüller 2008, via Dahl 2011 [L] |
| The force's shape | not one bump: secondary contacts at ≈ 3.75 ms and 5.6 ms after the first touch, when the head's travelling wave returns from the rim (a 1.7 ms delay measured near the rim) | Wagner 2006 [L] |
| The stick's own vibration | modes near 400 Hz, 1.0, 1.7 and 2.7 kHz, ringing ≈ 100 ms after a forte stroke | Wagner 2006 [L] |

**What Hertz's law predicts.** For a mass on a stiff target with `F = k x^α`, energy balance gives
the maximum compression and the contact time [I, from the standard derivation]:

```
½ m v² = k x_max^(α+1) / (α+1)    ⇒   x_max ∝ v^(2/(α+1))
t_c ∝ x_max / v ∝ v^((1−α)/(1+α))
α = 3/2  →  t_c ∝ v^(−1/5)         (twice the speed: 13 % shorter)
α = 3    →  t_c ∝ v^(−1/2)         (a felt: twice the speed, 29 % shorter)
α = 1    →  t_c independent of v   (a linear spring: half its period)
```

On a drum the soft element is the **head**, not the stick. A head under tension behaves near
linearly at small amplitude, so the contact is long (milliseconds) and depends weakly on velocity.
It shortens at high levels as the head's tension rises (§3.4) and the stick's tip contact
stiffens. That matches Wagner's measurements [I]. It also explains a finding of
`tympanum-real.md` §4.6: in the recordings a snare does not reliably get brighter with velocity,
and four of seven are brighter soft. The head barely shortens the contact, and the wires carry a
ghost note [I].

### 4.3 How the excitation shapes the spectrum

A force pulse of length τ low-passes the head's response. For a half-sine [I, its Fourier
transform]:

```
|F(f)| ∝ τ · |cos(π f τ)| / |1 − (2 f τ)²|
flat below ≈ 1/(2τ);  first zero at 1.5/τ;  then an envelope falling 12 dB per octave
```

With τ = 5 ms the first zero is at 300 Hz. Yet real snares are bright, because the real force is
not a smooth half-sine. Its sharp onset (the first fraction of a millisecond), the secondary
contacts and the stick's ringing all carry the highs. A lumped Hunt–Crossley contact on a modal
head produces all of these by itself: the onset is as sharp as the compression law. A fixed
half-sine of fitted length produces none of them [I].

**So "harder is brighter" has three causes on a drum**: a shorter, stiffer contact; the head's
own nonlinear stiffening (§3.4); and the stick's modes, struck harder. Only the first is a
property of the force pulse alone.

### 4.4 Beaters and mallets

- A **felt kick beater** is a felt-on-membrane contact: a large exponent and hysteresis (Stulov
  1995's felt). Its contact is long and its brightness rises steeply with velocity (`α ≈ 3`
  above). A plastic or wood beater is harder: a shorter contact, a click (model 5's `beater`
  scales the burst ×4 for plastic [C]).
- A **soft mallet** on a tom or cymbal is the same with a larger mass and a softer felt: long
  contact, few highs. Timpani simulations use exactly this lumped felt-mallet model (Rhaouti,
  Chaigne & Joly 1999; Bilbao & Webb 2013) [L].
- No measured contact times for kick beaters were found today (NV); model 4's 1.6–7 ms [C] is
  plausible against the stick's 3–8 ms.

### 4.5 Rimshots, cross-stick, buzz rolls and the stick's ring

| Articulation | The physics | Simulation literature found | Recommendation |
|---|---|---|---|
| **Rimshot** | the stick strikes head and hoop at once: the head's modes, the hoop's bright modes, and a stick struck near its middle (a free–free beam, modes ∝ (βL)² with βL = 4.730, 7.853, 10.996: ratios 1 : 2.76 : 5.40) | none (searches found patents for electronic rims only) | sample |
| **Cross-stick** | the stick lies on the head, the butt strikes the hoop: mostly the stick's beam modes and the hoop, with a little head | none | sample |
| **Buzz roll / press roll** | the stick is pressed into the head and bounces at the head's own rhythm, many small contacts; a lumped mass with the hand as a stiff spring and a Hunt–Crossley contact produces the bounces by itself [I] | Miura 2005 measured inter-player variability in snare rolls (via Dahl 2011) [L]; no synthesis paper found | sample (Virtuosity has buzzes and rolls); a modelled kit could use the multi-contact model |
| **The stick's ring** | the stick's own modes, ≈ 0.4–2.7 kHz, for ≈ 100 ms (Wagner 2006); tapered and gripped, so not a free beam's ratios | measured, not simulated | in the samples; in a model, a few modes driven by the contact force's reaction |

### 4.6 Brushes: a distributed, stochastic friction

A brush is many thin wires. A **tap** is many small impacts arriving over milliseconds, not one
contact. Tympanum's first measures put a brush tap's rise at 6–16 ms against a stick's 3 ms, its
peak at 6–22 ms, and its wires 9–12 dB under the whole (`tympanum-real.md` §8) [C]. A **sweep**
is sustained friction. Each bristle sticks and slips or micro-impacts as it is dragged, and the
sum is a noise whose level and colour follow the hand's speed and pressure.

- No synthesis paper on snare brushes was found today. The searches found products and
  forum posts.
- The nearest literature gives the parts.
  - Cook's PhISM / PhISEM (1997): particle collisions as a Poisson process of small impulses,
    whose rate and energy decay, driving resonant filters. It is directly a model for many
    bristles [L].
  - Avanzini, Serafin & Rocchesso (2005): physical friction models (rubbing, braking, squeaks)
    driving modal resonators, interactively [L].
  - Aimi (2007): convolving a real struck object's excitation with a drum's sampled response
    gave "surprisingly realistic" brushed stirs and scrapes [L].
- The model these imply [I]: a bristle-event process with rate ∝ hand speed and energy ∝
  pressure, through the snare's measured response (model 5's bands and modes) and its wires.

Recommendation: **sample** taps and the recorded strokes (dig, stir, flutter, release, in Big
Rusty, Unruly and Swirly). **Model only the sweep's continuity**, as a held note with speed and
pressure on controllers. Drive the snare's measured bands with a PhISEM-type bristle process, and
match its level and spectrum by speed to the recorded stir loops (`brushes.py`'s measures).

### 4.7 Why model 5's attack sounds wrong, in these terms

Analysis–resynthesis can fool listeners. Fletcher & Bassett's 1978 bass drum, resynthesised from
its measured components, was picked out by a 31-person jury only 51 % of the time [L]. Kirby &
Sandler's tom components passed an ABX test [L]. Both keep what model 5 throws away: the
**deterministic waveform of the first milliseconds**.

- Model 5 draws every mode's phase at random each hit. It renders the head's dense highs as noise
  bands with 22 envelope breakpoints over 3.2 s. It adds a stick burst and a knock as separate
  noises [C].
- The first 10–30 ms of a real hit are the contact force (§4.2) convolved with the head. They
  are one coherent event, the same shape from one hit to the next up to small changes. They are
  not a noise.
- The hybrid residual puts the recording's own attack back (ADR 0124, third amendment) [C]. That
  it helps by "about a fifth" in the first 50 ms is consistent with this diagnosis. The rest is
  the phase coherence the mask cannot restore [I].

### 4.8 Tympanum against the state of the art (excitation)

| | Tympanum | State of the art |
|---|---|---|
| Contact model | a half-sine of fixed length; model 4 drives the modes by its second difference, model 6 by its integral [C] | a lumped mass with a Hertz/Hunt–Crossley contact on the modal head, the force from the compression (Avanzini & Marogna 2010) |
| Contact length | stick 0.35–2.2 ms (model 4), 0.8–1.8 ms (model 6) [C] | measured 3–8 ms, shorter when harder and nearer the rim |
| Force shape | one bump | onset, secondary contacts at the rim-reflection delays, the stick's ring |
| Velocity → timbre | a law on the half-sine's length [C] | emerges from the contact and the head's stiffening |
| A re-strike on a ringing head | adds energy; model 5 a "bloom" rule [C] | the contact sees the head's current motion: a stroke into a head coming up is shorter and brighter |

### 4.9 Recommendation (excitation)

**Sample** the excitation for the flagship kits: every recording has the real contact, stick ring
and secondary contacts.

**For the modelled family** (`tympanum-real.md` §8(c)), replace model 6's half-sine with
Avanzini & Marogna's (2010, §II) impact block. That is a lumped stick mass with a Hunt–Crossley
contact on the modal head at the strike point. Discretise it with a non-iterative energy-stable
scheme (Ducceschi et al. 2021, or Bhanuprakash et al. 2024) so the cost per sample is bounded.

- Add the stick's three or four modes (Wagner's frequencies), driven by the contact's reaction.
- Fit `m`, `k`, `α`, `λ` so that the contact is 3–6 ms at mf and shortens with velocity and
  towards the rim.
- The check is the one §4.4 of `tympanum-real.md` already defined: every band's peak at 5–11 ms
  with the lows last, and the grooves' crest inside the recorded kits'.
- **Cost**: during the ~5 ms of contact, one weighted modal sum and a scalar update per sample.
  That is a few hundred multiply-adds, negligible against the modes themselves [I].

---

## 5. Snare wires

### 5.1 The physics

A set of coiled steel wires is stretched across the snare-side (resonant) head and pressed
lightly against it. The player strikes the batter head. The air in the shell, and the shell,
carry the motion to the snare-side head, and the wires ride on that head. When the head moves
away faster than the wires' tension can pull them after it, they separate. Then they slap back:
many one-sided collisions along each wire, every cycle. That is the buzz (Rossing et al. 1992;
Bilbao 2012; Torin & Newton 2014) [L]. Three consequences matter for realism [I]:

- **A threshold.** Below some amplitude of the snare-side head the wires never leave it and
  just damp it. Above it they buzz. A ghost note and a sympathetic buzz live near that threshold,
  so the buzz's share of the sound changes steeply with level. That fits the round's finding that
  wires carry the ghost notes (`tympanum-real.md` §4.6).
- **A lag.** The wires answer only once the cavity has pushed the snare-side head. Model 6 puts it
  at 1.5 ms [C]; the shell's depth alone is ≈ 0.4–0.5 ms of sound travel; the rest is the head's
  own build-up.
- **A colour that is the heads' and not white noise.** The buzz is the snare-side head's motion
  chopped by impacts. Its spectrum follows that head's modes and the wires' own, and it moves with
  the snare head's tuning and the wire tension.

### 5.2 The equations (the NESS snare)

Bilbao (2012), then Torin, Hamilton & Bilbao (2014), simulate the complete snare drum. It has two
membranes, a rigid shell, a 3-D air field inside and around it, a lumped mallet, and a set of
wires under the snare head [L]:

```
batter / snare head:  σ w_tt = T ∇²w − D ∇⁴w − loss + (p_below − p_above) + Σ_wires f_c
wire q (a stiff string): ρA u_tt = T_s u_xx − E I u_xxxx − loss − f_c
air:                  p_tt = c₀² ∇² p  (+ viscothermal loss, Torin et al. 2014)
collision:            f_c = ∂Φ/∂η,   Φ(η) = K/(α+1) · [η]₊^(α+1),   η = penetration of wire into head
```

- The power-law potential is a model of the deformable contact and, for a large `K`, a penalty
  for a rigid one. It makes the scheme energy-conserving and stable (Bilbao, Torin &
  Chatziioannou 2015) [L].
- Without the air's viscothermal loss the 3-D field left audible hiss and ringing (Torin et al.
  2014) [L].
- The cost is not mainly the 3-D air. "When a realistic number of snares is included … the
  dominant part of the code in terms of computation time is the solution of the nonlinear system",
  a dense, intrinsically serial solve, ill-suited to GPUs (Torin et al. 2014) [L].
- Ducceschi et al. (2021) give a quadratised, non-iterative scheme for the wire–membrane
  collision: one sparse linear solve a step [L].

### 5.3 Cheaper models

| Model | What it does | Cost | Source |
|---|---|---|---|
| Lumped wire on a modal head | each wire a stiff string kept to its **first mode**, touching the head at **one point** (mid-length, the largest displacement), with a Hunt–Crossley contact; extends to several wires | a few oscillators and contacts per wire | Avanzini & Marogna 2010 [L] |
| Modal head + modal wires, quadratised collision | the distributed collision in modal form, one linear solve | real-time capable for small systems; not shown for a full snare | Ducceschi et al. 2021 [L] |
| Gated noise | noise whose envelope follows the snare head's motion (compressed), filtered to the measured wire band | trivial | Cook 1997's family; model 4 and model 6 [C] |
| Separately modelled "wire" element | the snare split into body, wire and overtone, each modelled from analysis | not published | Roland Prismatic [V] |
| Sampled | the wires in the snare's own recordings, every layer | — | every sampled product |

### 5.4 Sympathetic buzz

The snare-side head is driven by **any** sound that reaches it: the kick, the toms, the bass amp.
The incident pressure drives its low modes, and once they pass the threshold (§5.1) the wires
buzz. Recordings of the whole kit have it in the snare mics of every other piece's hits, when the
wires were on. Frankensnare records it explicitly (a kick buzzing six snares) [C].

Its kin, **sympathetic tom resonance**, was simulated for a multisample engine by Clifford,
Lindsay Smith (FXpansion) & Reiss (DAFx 2012) [L]:

- The tom's resonance is extracted from its own direct hits with spectral flux, high-passed at
  the exciting drum's peak (a snare does not excite a low tom's fundamental), and scaled from the
  exciting hit's peak level. The bleed path is a −8 dB shelf at 5 kHz.
- In a pairwise test with 35 expert listeners the simulations could not be told from the real
  recordings at a statistically significant rate.

Why model it when the recordings have it [I]:

- The recorded buzz and ring are right only for the kit **as it was recorded**: the snare at
  its tuning and wire tension, the toms where they were, every piece present.
- Retune or damp a tom, take a snare from another kit, turn the snare's wires off, or mute a piece,
  and the recorded sympathetic sound is wrong or missing. Only a model can follow.
- It is linear up to the wires' threshold. It costs one small modal bank per resonating piece, fed
  by the other pieces' dry lanes, and model 5's tables already hold every piece's modes.

### 5.5 Tympanum against the state of the art (wires)

- Model 4: noise gated by the low modes' sum, compressive, after a ~1 ms delay rising over 5 ms
  [C]. Model 6: the wires' bands lagged 1.5 ms through the cavity [C]. Both are the gated-noise
  family. They have no threshold, no dependence on the snare head's tuning, and no
  wires-off-but-still-damping state.
- The recorded path plays the wires as recorded, sympathetic buzz included, but cannot change it
  [C].
- The state of the art in simulation (Bilbao 2012; Torin et al. 2014) is far from real time. Of
  the real-time-capable methods, Avanzini & Marogna's lumped wires are the most physical
  published.

### 5.6 Recommendation (wires)

**Sample** the snare's own buzz. **Model the sympathetic buzz** as a correction that switches on
only when the kit differs from its recording (a retuned, damped, swapped or wires-off piece),
plus the tom resonance of §5.4:

1. A small modal bank of the snare-side head (from model 5's snare table: its low modes) is
   driven by every other piece's dry lane. Each path is a delay and a level from the kit's
   geometry, and the −8 dB / 5 kHz shelf of Clifford et al. as the start.
2. The bank's motion drives a buzz generator with a **threshold**. First the gated noise, fitted to
   Frankensnare's sympathetic recordings; later Avanzini & Marogna's lumped wires.
3. The output goes to the snare mics through the bleed matrix like any other source.

The check is Clifford et al.'s pairwise test, against Frankensnare's real sympathetic hits.

---

## 6. Cymbals and hi-hats

### 6.1 The physics

- **A cymbal** is a thin bronze shell, nearly a shallow spherical cap. Its edge is free, and its
  bell is held between felts on a stand. Its linear modes are dense and inharmonic [L Fletcher &
  Rossing ch. 20], hundreds of them below 10 kHz [I].
- **The bell** is thicker and stiffer. It has a few strong, long-lived modes of its own, the
  ride's "bell" tone. Model 5's ride holds two or three of them at 1.56–1.76 kHz [C].
- **The bow** gives the ride's ping and its wash.
- **A crash struck on the edge at high amplitude** goes far beyond the linear regime. Its
  displacement is several times the thickness, and the modes exchange energy.

### 6.2 The equations: von Kármán

For a thin plate (a flat gong; a cymbal adds curvature terms) [L Bilbao 2008; Ducceschi & Touzé
2015; Bilbao et al. 2023]:

```
ρ h w_tt = −D ∇⁴w + L(w, F) − loss + f(x, t)
∇⁴F     = −(E h / 2) L(w, w)
L(f, g)  = f_xx g_yy + f_yy g_xx − 2 f_xy g_xy
```

`w` is the transverse displacement and `F` the Airy stress function, the in-plane stretching. Its
modal form couples every mode to every other [L Ducceschi & Touzé 2015; Chaigne, Touzé & Thomas
2005]:

```
q̈_k + 2 ξ_k ω_k q̇_k + ω_k² q_k = − Σ_{p,q,r} Γ^k_{pqr} q_p q_q q_r  + f_k(t)
```

The coupling is cubic for a flat plate. A curved shell adds quadratic terms. Berger's
simplification (§3.4) gives pitch glides, but **not** the crash: Bilbao et al. (2023) note that
the full model's glides are not uniform across partials, as they would be under Berger's [L].

### 6.3 Three regimes, and wave turbulence

Touzé, Bilbao & Cadot (2012) followed a plate as the forcing grows [L]:

1. a **periodic**, weakly nonlinear response;
2. a **quasi-periodic** state, energy exchanged between internally resonant modes;
3. a **turbulent** regime, read in the light of wave turbulence theory.

- Wave turbulence predicts an energy cascade from the low modes the stick hit to ever higher ones.
  Düring, Josserand & Rica (2006) asked whether one can "hear a Kolmogorov spectrum" [L].
- Experiments found the cascade with spectra that do not match the weak theory (Boudaoud et al.
  2008; Mordant 2008) [L].
- Simulations show **a front moving to high frequencies, leaving a steady spectrum in its wake**
  (Ducceschi, Cadot, Touzé & Bilbao 2014) [L].
- That front is the crash's *swell*: the slow build of high-frequency energy over the first
  hundreds of milliseconds (Bilbao 2010; Bilbao et al. 2023) [L].
- The steady wake is the *wash*. Its partials give way to wideband noise at very high amplitude
  (Bilbao et al. 2023) [L].

**This is why sparse modal banks sound glassy.** A linear bank keeps its energy where the stick
put it, frozen. The real cymbal moves it upwards and turns lines into a noisy continuum. Model 4's
measured flatness, 0.002 at 4–12 kHz against DRSKit's 0.12, is this failure (ADR 0124, second
amendment) [C].

### 6.4 Methods and their cost for a cymbal

| Method | What it gives | Cost | Source |
|---|---|---|---|
| FDTD shell (von Kármán, spherical cap) | crash, glides, the slow build-up | off line | Bilbao 2010 [L] |
| Modal von Kármán (free circular plate) | the same, with exact modes; complex damping laws per mode | an explicit, naturally parallel scheme, but every mode coupled to every other; the papers render off line | Ducceschi & Touzé 2015, DAFx 2015 [L] |
| Real-time FDTD plate (quadratised, explicit) | a gong: glides, swell, crash | 0.15–0.99 s per second of sound for plates of 0.03–0.06 m², one thread, AVX2, three CPUs; time tracks the area | Bilbao, Webb, Wang & Ducceschi 2023 [L] |
| — extrapolated to a kit | a 16″ crash is ≈ 0.13 m², a 20″ ride ≈ 0.20 m²: by that scaling, roughly 1–3 s per second **each** | not real time on one core; a kit's 4–6 cymbals need 5–15 cores; and it is a flat plate with no bell, curvature, felt or stand | [I] |
| Two shells in contact (the hi-hat) | stick, two cymbals, two collision models (a spring; an algorithmic inelastic one) | off line | Sekiguchi & Samejima 2023 [L] |
| Modal + dynamically filtered noise | measured partials and a time-varying noise spectrum | cheap | Aramaki & Kronland-Martinet 2006 [L]; model 5 [C] |
| Physically informed / PhISM | resonators + stochastic excitation | trivial | Cook 1997 [L] |
| Sampled | that cymbal | — | every product, MODO DRUM's cymbals included [R] |

### 6.5 The hi-hat

Two cymbals face to face. The pedal holds the top one at an opening.

- **Closed**: the plates are clamped together and damp each other. A stroke is short and its
  "chick" is the plates clapping shut.
- **Half open**: the plates collide again and again, a rattle whose rate and level follow the
  opening and the stroke.
- **Open**: two nearly free cymbals.

Sekiguchi & Samejima (2023) is the only physical hi-hat model found: two thin spherical shells,
a point-mass stick, and a choice of a spring or an "algorithmically mimicked inelastic" collision
between the plates [L]. In samples, a continuous opening is a crossfade between recorded
openings. BFD3 adds a "tighten" control for its closed articulations [R]. Unruly and Gogodze
record five to six openings [C].

### 6.6 Tympanum against the state of the art (cymbals)

- Model 5's cymbals have up to 512 measured modes, 38 noise bands with time envelopes, a bloom
  (late-peaking bands lend their modes a rise), per-hit phase draws, a side signal for width,
  and a persistent object so a re-strike adds (ADR 0124) [C]. It is Aramaki-type analysis
  –resynthesis, done carefully, and it matched the measured flatness and velocity range.
- It does not move energy upwards in time from the stroke. The bloom is a fitted envelope, not a
  cascade, so a soft and a hard crash differ only by interpolation between measured layers.
- A re-strike on a ringing cymbal is, in the turbulent regime, not a sum. Model 5's "re-strike
  bloom" (up to 12 velocity units harder) is a sensible rule. A sample engine just adds the new
  sample [C][I].
- The owner's verdict on all of it stands: "todo era un desastre". Nothing in the literature runs
  a kit of physical cymbals in real time on a desktop core.

### 6.7 Recommendation (cymbals and hats)

**Sample them**: every flagship kit's cymbals and hats, every recorded zone (bow, bell, edge),
choke and opening. This is what MODO DRUM does too.

- **Model only**:
  - the hat's continuous opening between recorded openings (present);
  - the choke and the foot's chick (present);
  - a re-strike swell by **choosing the next sample one layer harder** when the cymbal is
    already ringing loud, the recordings' own stand-in for the cascade.
- **Do not** build a physical cymbal for the flagship. Revisit the real-time plate (Bilbao et al.
  2023) only as a separate instrument (a gong or a "modelled crash" for sound design), where 1–3
  cores per voice is acceptable.

---

## 7. The room and the microphones

### 7.1 What a mic hears of a piece

Everything after the source is linear and time-invariant to a very good approximation. So the
signal at mic `i` from piece `j` is a filter of that piece's radiated sound [I, standard
acoustics; Kinsler et al. 2000]:

```
x_i(t) = Σ_j  h_ij(t) ∗ s_j(t)

h_ij ≈  G_i(θ_ij) · B_j(φ_ij) · (1/r_ij) · δ(t − r_ij / c₀)  ∗  P_i(r_ij)  ∗  R_ij(t)
```

| Term | What it is | Typical form |
|---|---|---|
| `r_ij / c₀` | delay, ≈ 2.9 ms per metre | a fractional delay |
| `1/r_ij` | spherical spreading | −6 dB per doubling of distance |
| `G_i(θ)` | the mic's polar pattern | cardioid `½(1 + cos θ)`, figure-8 `cos θ`, … |
| `B_j(φ)` | the head's directivity | a piston of radius `a`: `2 J₁(k a sin φ)/(k a sin φ)`; highs beam along the axis |
| `P_i(r)` | proximity: a pressure-gradient capsule's bass rise close to a source | gradient part ∝ `√(1 + 1/(k r)²)`; at 5 cm, `k r = 1` near 1.1 kHz; a cardioid gets half the gradient's rise |
| `R_ij(t)` | the room: floor, walls, ceiling, then the late field | image sources (Allen & Berkley 1979), then a feedback delay network (Jot & Chaigne 1991) or a measured response |

Kokkinis, Reiss & Mourjopoulos (2012) write leakage in exactly this form, each mic a sum of
filtered sources [L]. Clifford et al. (2012) needed only a delay, a level and a high shelf to make
convincing bleed [L].

### 7.2 Why coherent bleed is the realism

When the close mics, overheads and rooms are summed, each piece arrives several times, delayed by
its path differences. The sum is a comb filter, and the comb is part of a recorded kit's sound.
It is right only if **every mic plays the same strike**. Play the snare's close mic from one take
and its overhead bleed from another, and the comb becomes noise. The image smears, and the
kit sounds assembled [I]. Tympanum's rule, one draw per hit shared by every mic (ADR 0124 §2),
is exactly the requirement [C]. So is DrumGizmo's one-file-per-hit format holding every channel
[C].

What sample engines sell on top is control over the bleed: SD3's bleed channels, BFD's tech
mics, Tympanum's piece × mic matrix [V][C]. What a model adds is bleed that follows a moved mic or
a moved piece. That is possible only in a modelled kit, or by the linear rebalancing of §2
(commuted reasoning), never by moving a recorded mic.

### 7.3 Overheads and rooms

- **A spaced pair** hears each piece with two delays. For an extended source like a cymbal, the
  two capsules hear partly uncorrelated sound.
  - Tympanum found its point-source spaced pair made the crash's tail anti-correlated at
    250 Hz–2 kHz where recordings are positively correlated.
  - It fixed this by hearing direct sound from the pair's centre and adding a decorrelated side
    signal of the cymbal's width (ADR 0124, fourth listen) [C].
  - That is a physically motivated fix: an extended vibrating plate is many uncorrelated sources
    [I].
- **Rooms** cost little when done the standard way.
  - The image method gives the early reflections (Allen & Berkley 1979) and an FDN with Jot's loss
    rule the late field (Jot & Chaigne 1991). Tympanum's modelled room is this [C].
  - Measured impulse responses by convolution are the other way (Aula) [C].
  - Simulating the room's wave field instead is out of reach: a 10⁴ m³ room at 44.1 kHz is
    1.67 × 10¹⁵ flop/s by FDTD (Bilbao et al. 2013) [L].

### 7.4 Tympanum against the state of the art (mics and room)

The modelled kits' virtual mics model the delay, `1/r`, the pattern, the head's beam, proximity
(a low shelf, `tympanum_model_paths.rs`), image paths and an FDN late field [C]. That is the
standard, complete set. The recorded kits play their own mics, coherent, with a matrix and an
optional Cantus room [C]. **Nothing here is behind the state of the art.** What is left is
curation (each flagship kit's default mix, §8 of the plan, item 6). It is an engineer's job, not a
physicist's.

### 7.5 Recommendation (room and mics)

**Sample** (the kits' own mics and rooms). **Keep the model** for the modelled family and for
Cantus's rooms. The one modelling addition worth making is a **per-mic-family room response**
for a recorded kit under a Cantus room:

- feed close mics, overheads and room outputs with responses of their own (near, mid, far
  positions in Aula's measured rooms);
- do not give every mic the same late field, so the room stays as decorrelated across mics as a
  real one.

---

## 8. Variation: why no two real hits are the same

### 8.1 What changes from one hit to the next

| Cause | What it does to the sound | Evidence |
|---|---|---|
| **Strike position** | re-weights the modes (§3.7); shortens the contact towards the rim | Wagner 2006; Skrodzka et al. 2006; Tindale et al. 2004 [L] |
| **Velocity** | level, contact time, glide depth, the wires' share | Dahl 2004 (accents: striking velocity and timing in four drummers) [L]; GMD's velocity spread (`tympanum-real.md` §3) [C] |
| **Intent and timing** | laid-back strokes louder than on-the-beat ones; a faster tempo louder with a lower spectral centroid | Danielsen et al. 2015 (ten expert drummers) [L] |
| **Grip and stroke type** | controlled strokes have shorter contact and a higher peak force | Dahl & Altenmüller 2008, via Dahl 2011 [L] |
| **Stick angle and the part that lands** | tip against shoulder; a different contact area and stick-mode excitation | Sensory Percussion classifies tip against shoulder on the rim [V]; Wagner 2006 compared stick models [L] |
| **The head's state** | still ringing from the last hit: the contact meets a moving head (§4.8); the glide is pushed again (§3.4); uneven tension splits the twins differently over a session | physics [I]; Worland 2010 [L] |
| **What else is ringing** | other drums' and cymbals' sympathetic sound, the wires | §5.4 |

The recorded kits measured by this round vary by **1.4–2.0 dB** in third-octave levels from one
take to the next at one velocity (250 Hz–16 kHz; `tympanum-real.md` §4.3) [C]. That number is
the target for any variation a model or a sample engine adds.

### 8.2 How a physical model gets it

A model gets variation for free **if it varies its physical inputs**:

- the strike point `(r_s, θ_s)`;
- the velocity;
- the contact's stiffness (the stick's angle and the part that lands);
- the hand's timing.

It must also **keep the state**, so the next hit meets the head as it is. The variation is then
correlated the way a real drum's is: an off-centre hit is brighter, *and* its (1,1) region is
louder, *and* its contact is shorter.

Model 5 instead varies its **outputs**: a random phase per mode, ±1.5 dB per mode and ±0.7 dB per
band each hit [C]. That kind of variation has no physical cause. The round measured where it
leads: the jazz closed hat's 6.1 dB stroke-to-stroke change came from the phase draw making a few
close partials beat differently each time. That is three to four times the recordings' spread, and
scaling the level draw changed nothing (`tympanum-real.md` §4.3) [C].

### 8.3 How samples get it

- **Round robins**: several takes per layer, never the same twice running. AD2 has up to 8 [R].
  Tympanum's recorded path has them, with `vary` [C].
- **Many velocity layers**: SD3 combines round robins with extra velocity layers, where AD2 leans
  on round robins (MusicTech's comparison) [R]; BFD3 is known for its many layers [R, the count
  NV].
- **Zones** for position: centre, off-centre, edge, rim (Virtuosity, Gogodze) [C].
- **"Infinite round robin"**: MODO DRUM's modelled drums [R]. Roland calls round robin "a simple
  sample waveform rotation" and claims more from modelling [V].

### 8.4 Recommendation (variation): a measured variation layer

Sample the takes, and add a per-hit variation **measured from the recordings themselves**, not
drawn freely:

1. **Off line, per kit and per articulation**, for the takes at each layer, compute each take's
   deviation from the layer's mean:
   - third-octave levels over the first 50 ms and over 50–300 ms;
   - f0 in cents;
   - the onset's sub-millisecond offset;
   - for drums, the position ratio (the (1,1) region over the fundamental, `position.py`'s measure).
2. Reduce those deviations to a few principal components with their covariance (a small Gaussian
   model per articulation). The principal components keep the causes correlated, as §8.2 asks.
3. **At play**, draw one deviation per hit from that model, seeded by `hit_draw` so every mic
   shares it. Apply it to the chosen take as:
   - a short minimum-phase EQ;
   - a pitch offset of a few cents (resampling by a hair, or the modal part only once §3.10
     exists);
   - a start offset under 0.1 ms.
4. **Bound it**: the total take-to-take spread must stay inside the recordings' 1.4–2.0 dB
   (`compare.py`'s variation feature). The check is a blind test with no audible repeats in a
   16th-note hat or a snare roll, against the same kit played straight.

### 8.5 The velocity join, by the same physics

- Two adjacent layers are two different strikes. Crossfading their waveforms sums two takes, a
  comb that can cancel. Model 5's layer blend did exactly that: the studio kick at velocity 96
  came out 3.3 dB under its 80 (`tympanum-real.md` §7) [C].
- The physics (§4.2) says the change from one velocity to the next is smooth: a slightly
  shorter contact, a little more glide, a level. So the join can be **one take, morphed**.
  - Play the nearest layer's take.
  - Apply the smooth spectral change between the two layers' *mean* spectra: a time-windowed EQ
    curve fitted per layer pair.
  - Set the level from the kit's measured law.
- This never sums two takes, so nothing can cancel. It keeps every mic coherent. And it
  generalises the plan's correlation-corrected crossfade (§8 of the plan, item 1) [I]. Keep that
  crossfade as the fallback for layers whose spectra differ too much to morph (an articulation
  change, not a velocity step).

---

## 9. The cost of each approach, in one table

At 48 kHz on one current desktop core. "Real time" means one voice in well under a core.

| Element | Approach | Real time? | Notes |
|---|---|---|---|
| Head | sampled | yes | streaming; Tympanum's recorded path [C] |
| Head | modal, linear, 100–300 modes | yes, ≪ 1 % | [I] |
| Head | + energy-based glide | yes, negligible extra | [L] Avanzini, Marogna & Bank 2012 |
| Head | + lumped two-head air coupling | yes | [L] Avanzini & Marogna 2010 |
| Head | full Berger tension at audio rate | yes, but costlier | [L] Marogna, Avanzini & Bank 2010 |
| Head | 2-D FDTD | about a core a head | [I] |
| Drum | 3-D FDTD with the air | no (seconds per second on a GPU) | [L] Bilbao et al. 2013 |
| Contact | lumped Hunt–Crossley on a modal head | yes, negligible | [L] Avanzini & Rocchesso 2001; [I] |
| Snare wires | gated noise | yes | [C] |
| Snare wires | lumped modal wires | yes | [L] Avanzini & Marogna 2010 |
| Snare wires | distributed FDTD collisions | no: the dense nonlinear solve dominates | [L] Torin et al. 2014 |
| Cymbal | sampled | yes | — |
| Cymbal | modal + noise (model 5) | yes: ≈ 1.8–2 % a piece | [C] ADR 0124 |
| Cymbal | real-time von Kármán plate | 0.15–0.99 s/s for 0.03–0.06 m²; a kit's cymbals no | [L] Bilbao et al. 2023; [I] |
| Hi-hat | two shells in contact | no (offline) | [L] Sekiguchi & Samejima 2023 |
| Mics | delay, 1/r, pattern, beam, proximity | yes | [C] |
| Room | image + FDN | yes | [C] |
| Room | 3-D FDTD | no (10¹⁵ flop/s for a hall) | [L] Bilbao et al. 2013 |

Model 5's busy full kit costs 7.7–7.9 % of a core [C]. Everything this survey recommends adding
to the sampled kits is in the "negligible" rows.

---

## 10. The ranked plan

Ranked by what each adds that **samples cannot**, against its cost. Every item keeps the rules:
a null at its default, and block-size independence. Every item is measured against the recorded
kits with the round's tools (`compare.py`, `grooves.py`) and then judged blind.

| # | Item | Why (the physics) | First paper to implement | How it is measured | Where it fits |
|---|---|---|---|---|---|
| 1 | **Sympathetic buzz and tom resonance**, modelled, switching on only where the kit differs from its recording (retuned, damped, swapped, wires off, muted) | linear coupling through the air up to the wires' threshold (§5.4) | Clifford, Lindsay Smith & Reiss 2012 (the extraction and the test); Avanzini & Marogna 2010 (lumped wires, later) | Clifford's pairwise test against Frankensnare's sympathetic hits; the snare mic's late 4–10 kHz in kick-only grooves | round 44 (plan §8 item 5) |
| 2 | **A measured variation layer** on the round robins | real variation has causes and correlations (§8.1–8.2) | the recordings' own statistics (§8.4); Danielsen et al. 2015 and Wagner 2006 for which features move | take-to-take spread inside 1.4–2.0 dB; no audible repeat in a 16th hat or a roll, blind | round 43 (plan item 2) |
| 3 | **The velocity join as one take morphed** | velocity changes the contact smoothly (§4.2) | the layers' own mean spectra (§8.5) | no step at any velocity in `level_law_dump`'s 17 points; every e2e velocity test through `assemble` | round 43 (plan item 1) |
| 4 | **Retune and damp a sampled drum by decomposition** (modes moved, stick and room kept) | tension sets pitch and glide; damping adds loss per mode (§3.4, §3.5) | Kirby & Sandler 2020 (decomposition); Avanzini, Marogna & Bank 2012 (glide ∝ energy / tension) | a DRSKit tom retuned ±2/±4 semitones against recorded toms of that size; the glide's depth against looser and tighter recorded toms | round 44 |
| 5 | **The modelled family's strike and glide**: a Hunt–Crossley stick on the modal head at the measured 3–8 ms; the energy glide; per-mode air loading; then brushes as bristle events | §4.1–4.2, §3.2, §3.4, §4.6 | Avanzini & Marogna 2010 (impact, air, wires), Ducceschi et al. 2021 (a stable non-iterative contact), Cook 1997 (bristles) | band-peak timing 5–11 ms with the lows last; the grooves' crest; `compare.py` outside-range score | round 43–44, after the flagship |
| 6 | **Position morph** between recorded zones; modal re-weighting for small moves once item 4 exists | `a_mn ∝ J_m(j_mn r/a)` (§3.7) | Wagner 2006 and `position.py` for the targets | the (1,1)/(0,1) ratio moving monotonically with `pos` between the recorded zones | round 44 (plan item 4) |
| 7 | **Per-mic-family room responses** under a Cantus room | each mic hears its own reflections (§7.1) | Allen & Berkley 1979; Jot & Chaigne 1991; Aula's measured rooms | L/R and close/room correlation inside the recorded kits' | after 1–6 |

**Not recommended**, with the reasons:

- a real-time FDTD snare (Bilbao 2012; Torin et al. 2014): not real time, the collision solve
  is serial;
- physical cymbals for the flagship (Bilbao et al. 2023): 1–3 cores per cymbal, and a flat
  plate;
- a physical hi-hat (Sekiguchi & Samejima 2023): off line;
- any further "realism" tuning of model 5's cymbals: the owner's verdict, and the physics of §6.3.

---

## 11. Cautions, and what could not be verified

- **The contact times** come from one study with a contact-sensing stick (Wagner 2006: one
  drummer, a snare with and without its snare head, several tensions and positions) and from
  Dahl's survey, which cites it and Dahl 1997 (a tom). Kick-beater contact times were not found
  (NV). Model 6's constants should be refitted against the recordings' band timing in any case;
  the literature says where to start, not where to stop.
- **The timpani ratios** 1 : 1.5 : 2 : 2.5 : 2.9 are the textbook's; the exact decimals are NV
  here.
- **BFD3's layer counts** are from one secondary source (NV).
- **The real-time cymbal extrapolation** (§6.4) is mine. It scales Bilbao et al.'s measured times
  by area. A curved shell with a bell would differ.
- **The cost estimates marked [I]** are back-of-envelope figures, not measurements. Model 5's
  costs are Cantus's own measured ones.
- **Rimshots, cross-sticks and brushes**: no simulation literature was found today. The physics
  above is standard, but there is no published model to implement.
- **Author order** for Danielsen et al. 2015 differs between repositories; the publisher's order
  is used below.
- **Torin's thesis** is dated 2015 on its title page and 2016 in Edinburgh's record.
- **Bilbao 2012** (JASA 131(1), 914–925) was verified through the reference lists of Torin et al.
  2014 and Ducceschi et al. 2021, not from the journal's own page.
- **Nothing was listened to** for this survey, and no audio or sample library was fetched. Every
  claim about how Tympanum sounds is the owner's or the round's own measures.

---

## 12. References

### Books and theses

- Bilbao, S. (2009). *Numerical Sound Synthesis: Finite Difference Schemes and Simulation in
  Musical Acoustics*. Wiley, Chichester.
- Fletcher, N. H. & Rossing, T. D. (1998). *The Physics of Musical Instruments*, 2nd ed.
  Springer, New York. Ch. 18 *Drums*; ch. 20 *Cymbals, Gongs, Plates, and Steel Drums*
  (doi:10.1007/978-0-387-21603-4).
- Rossing, T. D. (2000). *Science of Percussion Instruments*. World Scientific, Singapore (Series
  in Popular Science, vol. 3).
- Kinsler, L. E., Frey, A. R., Coppens, A. B. & Sanders, J. V. (2000). *Fundamentals of
  Acoustics*, 4th ed. Wiley.
- Trautmann, L. & Rabenstein, R. (2003). *Digital Sound Synthesis by Physical Modeling Using the
  Functional Transformation Method*. Kluwer, New York.
- Smith, J. O. III (2010). *Physical Audio Signal Processing*. W3K Publishing; online at CCRMA:
  https://ccrma.stanford.edu/~jos/pasp/ (commuted synthesis:
  https://ccrma.stanford.edu/~jos/waveguide/Commuted_Synthesis.html).
- Adrien, J.-M. (1991). The missing link: modal synthesis. In De Poli, Piccialli & Roads (eds.),
  *Representations of Musical Signals*, MIT Press, 269–297.
- Wagner, A. (2006). *Analysis of Drumbeats — Interaction between Drummer, Drumstick and
  Instrument*. Master's thesis, KTH Speech, Music and Hearing, Stockholm (supervisors S. Dahl and
  A. Askenfelt). https://www.speech.kth.se/publications/masterprojects/2006/AndreasWagner.pdf
- Torin, A. (2016). *Percussion Instrument Modelling in 3D: Sound Synthesis through Time Domain
  Numerical Simulation*. PhD thesis, University of Edinburgh (title page 2015).
- Aimi, R. M. (2007). *Hybrid Percussion: Extending Physical Instruments Using Sampled
  Acoustics*. PhD thesis, MIT Media Arts and Sciences (submitted 2006).
  https://dspace.mit.edu/handle/1721.1/38640

### Membranes, drums and tension modulation

- Avanzini, F. & Marogna, R. (2010). A modular physically based approach to the sound synthesis
  of membrane percussion instruments. *IEEE Trans. Audio, Speech, Lang. Process.* 18(4), 891–902.
  doi:10.1109/TASL.2009.2036903. https://avanzini.di.unimi.it/downloads/publications/avanzini_taslp10.pdf
- Avanzini, F., Marogna, R. & Bank, B. (2012). Efficient synthesis of tension modulation in
  strings and membranes based on energy estimation. *J. Acoust. Soc. Am.* 131(1), 897–906.
  doi:10.1121/1.3651097.
- Marogna, R. & Avanzini, F. (2009). Physically-based synthesis of nonlinear circular membranes.
  *Proc. DAFx-09*, Como.
- Marogna, R., Avanzini, F. & Bank, B. (2010). Energy based synthesis of tension modulation in
  membranes. *Proc. DAFx-10*, Graz.
- Berger, H. M. (1955). A new approach to the analysis of large deflections of plates. *ASME J.
  Appl. Mech.* 22(4), 465–472.
- Tolonen, T., Välimäki, V. & Karjalainen, M. (2000). Modeling of tension modulation nonlinearity
  in plucked strings. *IEEE Trans. Speech Audio Process.* 8(3), 300–310.
- Petrausch, S. & Rabenstein, R. (2005). Tension modulated nonlinear 2D models for digital sound
  synthesis with the functional transformation method. *Proc. EUSIPCO 2005*, Antalya.
- Christian, R. S., Davis, R. E., Tubis, A., Anderson, C. A., Mills, R. I. & Rossing, T. D.
  (1984). Effects of air loading on timpani membrane vibrations. *J. Acoust. Soc. Am.* 76(5),
  1336–1345.
- Rossing, T. D., Bork, I., Zhao, H. & Fystrom, D. O. (1992). Acoustics of snare drums. *J.
  Acoust. Soc. Am.* 92(1), 84–94.
- Obata, J. & Tesima, T. (1935). Experimental studies on the sound and vibration of drum. *J.
  Acoust. Soc. Am.* 6, 267–274.
- Fletcher, H. & Bassett, I. G. (1978). Some experiments with the bass drum. *J. Acoust. Soc. Am.*
  64(6), 1570–1576. https://physics.byu.edu/docs/publication/1492
- Worland, R. (2010). Normal modes of a musical drumhead under non-uniform tension. *J. Acoust.
  Soc. Am.* 127(1), 525–533. doi:10.1121/1.3268605.
- Skrodzka, E. B., Hojan, E. & Proksza, R. (2006). Vibroacoustic investigation of a batter head
  of a snare drum. *Archives of Acoustics* 31(3), 289–297.
- Rhaouti, L., Chaigne, A. & Joly, P. (1999). Time-domain modeling and numerical simulation of a
  kettledrum. *J. Acoust. Soc. Am.* 105(6), 3545–3562.
- Bilbao, S. (2012). Time domain simulation and sound synthesis for the snare drum. *J. Acoust.
  Soc. Am.* 131(1), 914–925.
- Bilbao, S. & Webb, C. J. (2013). Physical modeling of timpani drums in 3D on GPGPUs. *J. Audio
  Eng. Soc.* 61(10), 737–748 (expanding their DAFx-12 paper).
- Torin, A. & Bilbao, S. (2013). Numerical experiments with non-linear double membrane drums.
  *Proc. SMAC 2013*, Stockholm.
  https://www.ness.music.ed.ac.uk/wp-content/uploads/2013/06/BassDrumSMAC13_corrections.pdf
- Torin, A., Hamilton, B. & Bilbao, S. (2014). An energy conserving finite difference scheme for
  the simulation of collisions in snare drums. *Proc. DAFx-14*, Erlangen.
  https://www.pure.ed.ac.uk/ws/files/17485699/dafx14_submission_56_3.pdf
- Torin, A. & Newton, M. (2014). Collisions in drum membranes: a preliminary study on a
  simplified system. *Proc. ISMA 2014*, Le Mans.
  https://www.ness.music.ed.ac.uk/wp-content/uploads/2014/05/Snare_final.pdf
- Bilbao, S., Hamilton, B., Torin, A., Webb, C., Graham, P., Gray, A., Kavoussanakis, K. & Perry,
  J. (2013). Large scale physical modeling sound synthesis. *Proc. SMAC/SMC 2013*, Stockholm.
  https://www.ness.music.ed.ac.uk/wp-content/uploads/2013/06/smc13_r1.pdf
- Kirby, T. & Sandler, M. (2020). Advanced Fourier decomposition for realistic drum synthesis.
  *Proc. DAFx2020*, Vienna. https://dafx2020.mdw.ac.at/proceedings/papers/DAFx2020_paper_66.pdf

### Contact, collisions and the player

- Hunt, K. H. & Crossley, F. R. E. (1975). Coefficient of restitution interpreted as damping in
  vibroimpact. *ASME J. Appl. Mech.* 42(2), 440–445.
- Stulov, A. (1995). Hysteretic model of the grand piano hammer felt. *J. Acoust. Soc. Am.* 97,
  2577–2585. doi:10.1121/1.411912.
- Avanzini, F. & Rocchesso, D. (2001). Modeling collision sounds: non-linear contact force.
  *Proc. COST-G6 DAFx-01*, Limerick, 61–66.
- Papetti, S., Avanzini, F. & Rocchesso, D. (2011). Numerical methods for a nonlinear impact
  model: a comparative study with closed-form corrections. *IEEE Trans. Audio, Speech, Lang.
  Process.* 19(7), 2146–2158. doi:10.1109/TASL.2011.2118204.
- Chatziioannou, V. & van Walstijn, M. (2013). An energy conserving finite difference scheme for
  simulation of collisions. *Proc. SMC 2013*, Stockholm, 584–591.
- Bilbao, S., Torin, A. & Chatziioannou, V. (2015). Numerical modeling of collisions in musical
  instruments. *Acta Acustica united with Acustica* 101(1), 155–173. doi:10.3813/AAA.918813.
  arXiv:1405.2589.
- Ducceschi, M., Bilbao, S., Willemsen, S. & Serafin, S. (2021). Linearly-implicit schemes for
  collisions in musical acoustics based on energy quadratisation. *J. Acoust. Soc. Am.* 149(5),
  3502–3516. doi:10.1121/10.0005008.
- Bhanuprakash, A., van Walstijn, M. & Chatziioannou, V. (2024). Quadratic spline approximation
  of the contact potential for real-time simulation of lumped collisions in musical instruments.
  *Proc. DAFx24*, Guildford. https://dafx.de/paper-archive/2024/papers/DAFx24_paper_40.pdf
- Dahl, S. (1997). Spectral changes in the tom-tom related to striking force. *KTH TMH-QPSR* 38,
  59–66.
- Dahl, S. (2004). Playing the accent — comparing striking velocity and timing in an ostinato
  rhythm performed by four drummers. *Acta Acustica united with Acustica* 90, 762–776.
- Dahl, S. & Altenmüller, E. (2008). Motor control in drumming: influence of movement pattern on
  contact force and sound characteristics. *Proc. Acoustics'08*, Paris, 1489–1494.
- Dahl, S. (2011). Striking movements: a survey of motion analysis of percussionists. *Acoust.
  Sci. & Tech.* 32(5), 168–173. doi:10.1250/ast.32.168.
- Miura, M. (2005). Inter-player variability of a roll performance on a snare-drum performance.
  *Proc. Forum Acusticum*, Budapest, 563–568.
- Danielsen, A., Waadeland, C. H., Sundt, H. G. & Witek, M. A. G. (2015). Effects of instructed
  timing and tempo on snare drum sound in drum kit performance. *J. Acoust. Soc. Am.* 138(4),
  2301–2316. doi:10.1121/1.4930950.
- Tindale, A., Kapur, A., Tzanetakis, G. & Fujinaga, I. (2004). Retrieval of percussion gestures
  using timbre classification techniques. *Proc. ISMIR 2004*, Barcelona.
  https://archives.ismir.net/ismir2004/paper/000235.pdf

### Plates, cymbals and wave turbulence

- Bilbao, S. (2008). A family of conservative finite difference schemes for the dynamical von
  Kármán plate equations. *Numer. Methods Partial Differential Eq.* 24(1), 193–216.
- Bilbao, S. (2010). Percussion synthesis based on models of nonlinear shell vibration. *IEEE
  Trans. Audio, Speech, Lang. Process.* 18(4), 872–880. doi:10.1109/TASL.2009.2029710.
- Chaigne, A., Touzé, C. & Thomas, O. (2005). Nonlinear vibrations and chaos in gongs and
  cymbals. *Acoust. Sci. & Tech.* 26(5), 403–409. doi:10.1250/ast.26.403.
- Chaigne, A. & Lambourg, C. (2001). Time-domain simulation of damped impacted plates. Part I.
  Theory and experiments. *J. Acoust. Soc. Am.* 109(4), 1422–1432.
- Touzé, C., Bilbao, S. & Cadot, O. (2012). Transition scenario to turbulence in thin vibrating
  plates. *J. Sound Vib.* 331(2), 412–433. doi:10.1016/j.jsv.2011.09.016.
- Ducceschi, M., Cadot, O., Touzé, C. & Bilbao, S. (2014). Dynamics of the wave turbulence
  spectrum in vibrating plates: a numerical investigation using a conservative finite difference
  scheme. *Physica D* 280–281, 73–85. doi:10.1016/j.physd.2014.04.008.
- Ducceschi, M. & Touzé, C. (2015). Modal approach for nonlinear vibrations of damped impacted
  plates: application to sound synthesis of gongs and cymbals. *J. Sound Vib.* 344, 313–331.
  doi:10.1016/j.jsv.2015.01.029.
- Ducceschi, M. & Touzé, C. (2015). Simulations of nonlinear plate dynamics: an accurate and
  efficient modal algorithm. *Proc. DAFx-15*, Trondheim.
- Bilbao, S., Webb, C., Wang, Z. & Ducceschi, M. (2023). Real-time gong synthesis. *Proc. DAFx23*,
  Copenhagen. https://www.dafx.de/paper-archive/2023/DAFx23_paper_19.pdf
- Düring, G., Josserand, C. & Rica, S. (2006). Weak turbulence for a vibrating plate: can one
  hear a Kolmogorov spectrum? *Phys. Rev. Lett.* 97, 025503. doi:10.1103/PhysRevLett.97.025503.
- Boudaoud, A., Cadot, O., Odille, B. & Touzé, C. (2008). Observation of wave turbulence in
  vibrating plates. *Phys. Rev. Lett.* 100, 234504.
- Mordant, N. (2008). Are there waves in elastic wave turbulence? *Phys. Rev. Lett.* 100, 234505.
- Sekiguchi, S. & Samejima, T. (2023). Physical modeling and sound synthesis of the hi-hat.
  *Acoust. Sci. & Tech.* 44(5), 352–359. doi:10.1250/ast.44.352.

### Synthesis methods, analysis–resynthesis and friction

- Van Duyne, S. A. & Smith, J. O. III (1993). Physical modeling with the 2-D digital waveguide
  mesh. *Proc. ICMC-93*, Tokyo, 40–47.
- Savioja, L. & Välimäki, V. (2003). Interpolated rectangular 3-D digital waveguide mesh
  algorithms with frequency warping. *IEEE Trans. Speech Audio Process.* 11(6), 783–790.
- Cook, P. R. (1997). Physically informed sonic modeling (PhISM): synthesis of percussive sounds.
  *Computer Music J.* 21(3), 38–49.
- Aramaki, M. & Kronland-Martinet, R. (2006). Analysis–synthesis of impact sounds by real-time
  dynamic filtering. *IEEE Trans. Audio, Speech, Lang. Process.* 14(2), 695–705.
  doi:10.1109/TSA.2005.855831.
- Välimäki, V., Pakarinen, J., Erkut, C. & Karjalainen, M. (2006). Discrete-time modelling of
  musical instruments. *Rep. Prog. Phys.* 69(1), 1–78.
- Avanzini, F., Serafin, S. & Rocchesso, D. (2005). Interactive simulation of rigid body
  interaction with friction-induced sound generation. *IEEE Trans. Speech Audio Process.* 13(5),
  1073–1081. doi:10.1109/TSA.2005.852984.

### Mics, bleed and rooms

- Clifford, A., Lindsay Smith, H. & Reiss, J. D. (2012). Simulating microphone bleed and tom-tom
  resonance in multisampled drum workstations. *Proc. DAFx-12*, York.
  https://dafx.de/paper-archive/2012/papers/dafx12_submission_5.pdf
- Kokkinis, E. K., Reiss, J. D. & Mourjopoulos, J. (2012). A Wiener filter approach to microphone
  leakage reduction in close-microphone applications. *IEEE Trans. Audio, Speech, Lang. Process.*
  20(3), 767–779.
- Allen, J. B. & Berkley, D. A. (1979). Image method for efficiently simulating small-room
  acoustics. *J. Acoust. Soc. Am.* 65(4), 943–950.
- Jot, J.-M. & Chaigne, A. (1991). Digital delay networks for designing artificial reverberators.
  *AES 90th Convention*, Paris, preprint 3030.

### Products and reviews

- Magnus, N. (2019, December). IK Multimedia MODO Drum. *Sound On Sound*.
  https://www.soundonsound.com/reviews/ik-multimedia-modo-drum
- Toontrack, Superior Drummer 3: https://www.toontrack.com/product/superior-drummer-3/ ; MusicRadar
  review: https://www.musicradar.com/reviews/toontrack-superior-drummer-3 ; MusicTech review:
  https://musictech.com/2018/01/toontrack-superior-drummer-3-review/
- MusicTech, *Addictive Drums 2 vs EZdrummer 3 vs Superior Drummer 3*:
  https://musictech.com/guides/buyers-guide/addictive-drums-2-vs-ezdrummer-3-vs-superior-drummer-3-which-drum-plugin-is-best
- bonedo, XLN Audio Addictive Drums 2 Test: https://www.bonedo.de/artikel/xln-audio-addictive-drums-2-test/
- Walden, J. (2014, February). FXpansion BFD3. *Sound On Sound*.
  https://www.soundonsound.com/reviews/fxpansion-bfd3
- Roland, *Understanding Prismatic Sound Modeling*:
  https://articles.roland.com/roland-engineering-understanding-prismatic-sound-modeling/ ; support:
  https://support.roland.com/hc/en-us/articles/360059318732
- Applied Acoustics Systems, Chromaphone: *Sound On Sound* review
  https://www.soundonsound.com/reviews/aas-chromaphone ; MusicRadar, Chromaphone 3:
  https://www.musicradar.com/reviews/aas-chromaphone-3
- D'Addario / Sunhouse, Sensory Percussion:
  https://www.daddario.com/pages/learn-earn-sensory-percussion
- Native Instruments, Maschine sound details:
  https://www.native-instruments.com/en/products/maschine/production-systems/maschine-plus/sounds/
