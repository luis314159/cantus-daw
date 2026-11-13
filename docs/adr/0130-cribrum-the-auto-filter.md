# ADR 0130 — Cribrum, the auto filter

**Status:** accepted (fx side round, provisional number — renumbered at merge) · **Date:**
2026-11-13 · **Schema:** none of its own (the contract's 62 declared the kind; Cribrum's
table is the kind's parameters, sparse, so old projects need no migration) · **Relates to:**
ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults), ADR 0020 (our
plug-ins outside Cantus), ADR 0023 (the VA filters), ADR 0031 §5.1 (at rest), ADR 0055 (the
modulation family's clock and LFO), ADR 0071 (effects that sound on insert), ADR 0074 (a face
of its own), ADR 0101 (presets by style and role)

## Context

Every market-leading DAW ships an auto filter (Ableton's Auto Filter, FL Studio's Love Philter
and Fruity Filter, Logic's AutoFilter, Bitwig's Filter+); FabFilter Volcano and Soundtoys
FilterFreak are the plug-ins people buy for it. Cantus had filters inside its instruments and
Aequalis's bands, but no insert that *moves* a filter. The owner asked for it (BACKLOG §3b)
with a panel of its own, "polished and professional like FL Studio and the others".

## Decision

A new effect kind, `EffectKind::AutoFilter` — **Cribrum** (Latin, *the sieve*).

1. **The synths' filters, not new ones.** Two filters, each one of the five zero-delay-feedback
   models of `cantus_dsp::va::filter` (ADR 0023: SVF, transistor ladder, OTA cascade, diode
   ladder, Sallen-Key), run by the same solver (two passes a sample), so the same curve and the
   same self-oscillation (resonance's 95 % is the model's threshold, 100 % is 1.05). The modes
   are what each topology gives: the SVF its low, band (×2, unity at zero resonance), high,
   notch (lp + hp) and peak (lp − hp), 24 dB/oct as a second Butterworth stage on low and high;
   the ladder and the OTA cascade their four stages **mixed** (`Σ cᵢ·yᵢ` with `y₀ = u`, the
   coefficients of `N(s)` in powers of `1 + s`: high-pass `1, −4, 6, −4, 1`, band `0, 0, 4, −8,
   4`, notch `1, −4, 8, −8, 4`, and the 12 dB forms over `(1+s)²`) — the Xpander's method,
   derived here; the diode ladder (whose stages load each other) and Sallen-Key their low-pass.
   A mode a model does not give falls back to its low-pass, and the editor greys it. Three
   read-only accessors were added to `VaFilter` (`taps`, `quiet`, `at_rest`); nothing in the
   solver changed, no golden moved.
2. **Vowels**: three of the SVF's band-passes at a tenor's first three formants for A, E, I, O,
   U (the Csound manual's table of formant values), interpolated between vowels in log
   frequency, log bandwidth and dB; the cutoff scales them (1 kHz is the table's voice, ±2
   octaves), the resonance narrows them. Each band is fed `k/4` of the input and read four
   times, so its state stays near the input's level and the SVF's level-dependent damping
   (which holds its self-oscillation) does not squash a narrow formant. A make-up of +12 dB
   (×4), measured on pink-ish noise, keeps the five vowels within ±2 dB of the dry level.
3. **Movement**: three sources summed, each with a depth onto the cutoff (octaves), the
   resonance and the vowel: the modulation family's LFO (ADR 0055: on the song or free; its
   six waves and a *drift*, the sample-and-hold's values joined by a raised cosine), a
   peak follower (`dynfx_follow`) on the input or the sidechain (`TAKES_SIDECHAIN`), mapped
   from −48 to 0 dBFS, and Fluctus's drawn curve (sixteen points, the automation editor's
   segment shapes). The second filter takes `track` of the movement (−100 … 100 %); a stereo
   spread opens the channels' cutoffs apart. The synced lengths are the family's seventeen plus
   1/32 triplet and 1/64 (`cribrum.div.{i}`).
4. **The curve's smoothing is a window, not a glide.** A glide would carry memory through
   silence, so a unit that slept would differ from one that processed it. The smoothing is a
   nine-point Hann window over the curve, centred, its points `smooth/8` apart, read on a grid
   of the song with that spacing and drawn straight between: a pure function of the song
   position. A step becomes an S of eight straight pieces (at 2 ms no sample moves more than
   1/60 of the step); a held level comes back exactly.
5. **The null and rest.** The mix starts at 0 and the oversampling off: at its defaults the unit
   touches no sample and adds no latency. At mix 0 (landed) nothing runs but the gain and the
   dry delay; turned up, the filters start from new. At rest: the mix and the gain still, and
   while the filters run every filter, the oversamplers, the dry delay and the follower as a
   new unit's, every glide landed. The synths' tuning trim leaves a sag in a resonant SVF even
   in silence; a filter left with nothing of a signal (every integrator and solution +0.0 after
   a +0.0 sample out) is therefore made new — so a silent filter *is* a new one. A filter
   singing on its own is never at rest.
6. **Oversampling** 2× or 4× through Fervor's minimum-phase IIR half-bands (a sample or two of
   latency, reported; the dry path delayed to match), so a driven filter folds back less
   (measured below). Linear phase was not offered: an auto filter is played live.
7. **Drive** goes into the model's nonlinearities and half its decibels come back (`1/√drive`):
   louder where it saturates little, never the drive's whole gain.
8. **The editor** — a window of its own (1120 × 800 at 100 %): *What passes* (the display: the
   spectrum as grains lit up to the output and capped at the input, the two responses as they
   move and as set, a handle per filter dragged across for the cutoff and up and down for the
   resonance, the chosen filter's reach as a rail through its handle, the vowels named in vowel
   mode), *Movement* (the three sources as keys with their depth and a live bar; the chosen
   one's lane in time with the song, its controls and its depths as two titled groups, the
   cutoff's reach in Hz; the curve edited on the lane as Fluctus's), *Filter A/B* (one card,
   the responses as keys with their curves drawn, cutoff a size up) and *Output* (mix a size
   up). The face (`kit/skin/side.rs`): its colour awaits the owner's pick among three rendered
   variants; the placeholder is racing-green lacquer, lime on glass, coral movement.
9. **Starting preset** (ADR 0071): *Bar sweep* — a resonant ladder at 24 dB/oct swept 1.5
   octaves either way once a bar, fully wet. Sixteen factory presets by style and role.
10. **Exported** as `Cantus Cribrum` (CLAP/VST3), its curve points grouped *Curve point 1 …
    16*.

## Consequences

- Cribrum is the first effect built on the instruments' filters; a change to `va::filter`
  moves Cribrum's sound with the synths' (its own test holds every low-pass to the synths'
  bit for bit, so such a change is seen).
- The curve smoothing's window costs nine curve readings per grid point (every `smooth/8`
  samples), none per sample between; the filters cost one or two ZDF solves per channel per
  sample (three in vowel mode), times the oversampling.
- The colour is one sheet in `side.rs`: the owner's pick is a change of data only.

## What the tests hold

- `cantus-dsp` (`cribrum_tests.rs`, 18): the null and mix 0; every model's low-pass is the
  synths' filter to the bit; the drawn response is the heard one within 0.5 dB (0.096 dB
  measured) over every model, mode, slope and resonance; full resonance sings within ±5 cents
  (4.5 worst, the OTA cascade) at 1× and 2× and stays bounded; a render from bar 5 moves the
  cutoff as the whole render's bar 5 does to the bit; the same bits at every split; a synced LFO
  is a function of the song; the curve's steps never click (≤ 3 % of a step per sample at 2 ms,
  gentler as the smoothing grows) and a stepped sweep's worst kink is 8× smaller smoothed; the
  vowels peak at their formants and keep the level; the dry path waits for the oversampling;
  oversampling lowers the aliasing (−16.7 / −23.2 / −34.2 dB at 1× / 2× / 4×); it rests and is
  then as good as new; a self-oscillating filter never rests; the follower moves the cutoff with
  the level, and with the sidechain when keyed.
- `cantus-engine`: the unit's three (defaults, every key read, a wire); `fx_contract`'s split
  invariance and exact rest (default, heavy, random); `fx_cribrum` (6: the null, mix 0 a wire,
  heard, block sizes, live = offline, bar 5 to the bit); `rt_memory::cribrum_never_allocates`.
- `cantus-core`: the table's keys, defaults and choices; the starting preset.
- `cantus-ui`: `fx_cribrum` (a handle's drag is one undo step, a double-click resets; a point
  added and removed on the curve, one step each; the starting preset is a factory preset; the
  presets name real keys in both languages; the face is its own and keeps contrast);
  `cribrum_e2e` (through `CantusApp::assemble`: inserted from the picker on a playing track, the
  sweep heard in the master, a control changed darkens it, undone is the sweep to the bit);
  the showcase holds it (`demos`), the preset audit's floor of twelve.
- `cantus-plugins`: the CLAP/VST3 tests over every descriptor, Cribrum's `Direct` arm
  (bit-identical to the engine).
