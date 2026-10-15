# Round 28 — sound: an acoustic drum kit to download, then Chorda

*Builder: sound · branch `r28/sound` (from `r28/integration` at `405bd9e`) · ADR 0042
(amended), ADR 0023 (amended), ADR 0059 · schema 28 on this branch (30 in the round's
table) · BACKLOG §5, §3e.*

*(The drum kit's report and the final measurements are written at the end of the round;
this file opens with the decision the brief asks for before any of Chorda's code.)*

## B. Chorda — the decision, before code (2026-10-15)

**Decision: Chorda can be finished this round to the measurable part of its *done when*,
and I build it now.** What follows is why, what "finished" means in numbers, how the
patents are avoided, and what stays the owner's.

### Why it can be finished

- **The engine's pieces exist.** Gravis's bidirectional waveguide string
  (`gravis_string.rs`: rails, phase-delay tuning to a fraction of a cent, a one-pole loss,
  first-order dispersion allpasses), Lamina's hammer shape and strike scaling
  (`lamina_voice.rs`), the voice allocator (`voices.rs`), voices split across the pool
  with a post-voice stage summed in voice order (ADR 0038: Unda, Calor and Exemplar already
  do it), the modulation matrix, the kit shell (Play / Edit / Matrix) and the instrument
  contract tests (`inst_contract.rs`: block size, determinism, headroom, silence). Chorda
  is new physics on a known chassis, not a new chassis.
- **The physics is published and closed-form.** Waveguide strings (Smith, *PASP*), a
  dispersion filter designed to the stiff-string law `f_n = n f₀ √(1 + B n²)` (Van Duyne &
  Smith 1994; Rauhala & Välimäki 2006; a least-squares phase fit), loss filters from two
  decay times (Välimäki et al. 1996), a hysteretic felt hammer (Stulov 1995) solved against
  the string's wave impedance with a fixed number of iterations (Chaigne & Askenfelt 1994;
  Bank 2000), unisons coupled at a bridge admittance (Weinreich 1977: beating and two-stage
  decay), phantom partials from tension modulation (Bank & Sujbert 2005), dampers and the
  pedal as felt losses, and a soundboard as a post-voice filter. The data the brief asks
  for is readable here: Balázs Bank's thesis (measured string parameters, decay times and
  the soundboard model; `home.mit.bme.hu/~bank/thesis`), Rigaud, David & Daudet's
  parametric models of inharmonicity and tuning across the keyboard (DAFx 2011, from
  measurements of several pianos), Chaigne & Askenfelt's strings, and the perceptual
  tolerance of decay (Tolonen & Järveläinen, −25 % / +40 %, via Bank et al. 2003).
- **The cost fits.** A voice is one to three strings of ~100 operations a sample (a delay
  line, a fractional delay, 4–16 dispersion sections, a loss filter, the bridge); 64
  pedal-down voices are ~20 k operations a sample, and the shared stages (the sympathetic
  strings at the keys' own pitches, the soundboard) a few thousand more — inside 25 % of one
  core at 256 frames if written for it (independent strings side by side, coefficients
  computed off the audio thread), and split across the pool besides.

### What "finished" means — the tests that will hold it

1. **Tuning:** on every one of the 88 keys, one string alone (unisons off), every partial
   below 5 kHz (at most 32) within **1 cent** of the target `n f₀ √(1 + B n²)`, with `f₀`
   from the stretched tuning and `B` from the inharmonicity model across the keyboard —
   measured on the rendered sound (FFT, interpolated peaks over seconds), not computed from
   the filter's formula.
2. **Decay:** each partial's T60 measured on the rendered sound (unisons off, pedal up, key
   held) within **−25 % / +40 %** of the published measurements at the notes they were
   published for, and of the model fitted to them in between.
3. **Half-pedal:** continuous — the decay of a held note changes monotonically and smoothly
   as the pedal moves from up to down (no step between two models, no click), and
   re-pedalling (up and down again within a note) leaves no discontinuity.
4. **Cost:** 64 voices with the pedal down inside **25 % of one core at 256 frames**
   (measured, best of several runs, on this machine; the number in ADR 0059).
5. **The family's rules (ADR 0023):** a control at its default changes no sample, the same
   bits at block sizes 1, 37, 64 and 512, offline equal to real time, parallel equal to
   serial, exact rest once the sound has died, headroom (no factory preset past 0 dBFS on
   the dense riff), deterministic randomness (hashes of the note).
6. **The product:** a `model_version` stored per instance, presets made with care, the kit
   shell with a picture of what plays (strings, hammer, pedal), `docs/instruments/chorda.md`,
   the manual in both languages, `READY = true`, a demo song.

### The patents (US; Google Patents' status, not a legal conclusion)

- **Modartt US 7,915,515** (to 2027-10-20): no presynthesis of per-partial frequencies and
  dampings from a soundboard parameter for a partial-based synthesizer. Chorda's strings are
  waveguides — partials are not synthesised one by one — and the dispersion and loss
  filters are designed from the string alone (its `f₀`, `B` and decay times); the
  soundboard is a separate filter after the strings.
- **Casio US 8,178,772**: no resonators tuned to delays that do not match the keys' pitches.
  The sympathetic strings are the keys' own strings, at their speaking pitches.
- **Yamaha US 10,818,278**: no second loop at a non-speaking pitch (no duplex scale model).
- **Yamaha US 8,729,376**: no delay loop excited by a band-split sampled waveform — the
  hammer is a physical model, no sample excites anything.
- Kawai US 9,330,648, Casio US 10,109,268 / 11,881,196, Yamaha US 12,039,960: their
  structures (stored resonance in a sample player, register IRs, excitation routed to
  low-string channels, a noise-derived damper envelope) are not used.
- The legal review against every claim stays the owner's, before release
  (`docs/beta-checklist.md`).

### What stays the owner's

The blind listening test (≥ 20 listeners against Pianoteq 9 and a sampled grand), the
legal review, and the presets' last word by ear.

### If a number cannot be met

If one of the four measurable targets proves out of reach with the published approach,
Chorda stays hidden (`READY = false`), this file says which and by how much, and the
integrator decides — nothing half-done reaches the picker.
