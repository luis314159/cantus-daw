# ADR 0133 — Vetus and Libra, lo-fi and utility

**Status:** accepted — Vetus's design v2 approved by the owner (2026-11-13: "síguele, se ve
chido"), Libra's balance (v2) his choice (2026-11-14) (fx side round, provisional number —
renumbered at merge) ·
**Date:** 2026-11-13 · **Schema:** none of its own (the contract's 62 declared the kinds;
Vetus's table is the kind's parameters, read with their defaults from older files) ·
**Relates to:** ADR 0004 (commands), ADR 0005 (the audio thread), ADR 0018 (null defaults),
ADR 0020 (our plug-ins outside Cantus), ADR 0031 §5.1 (at rest), ADR 0055 (the modulation
family's clock, LFO and line), ADR 0071 (effects that sound on insert), ADR 0074 (a face of
its own), ADR 0101 (presets by style and role), ADR 0130 §Colourways

## Context

Two everyday tools Cantus lacks as inserts: a lo-fi unit (bit crushing, sample-rate
reduction, vinyl and tape wear — Ableton's Redux and Vinyl Distortion, FL's Fruity Squeeze,
RC-20-class colour, iZotope Vinyl) and a utility (gain, polarity, channel operations, a
delay to align, a tuner — Ableton's Utility, FL's Fruity Balance and Stereo Shaper, Logic's
Gain).

## Decision

Two new effect kinds: `EffectKind::LoFi` (Latin *vetus*, old), `fx::vetus`, and
`EffectKind::Utility` (Latin *libra*, the balance), `fx::libra`.

### Vetus

1. **Six modules in a fixed order**, each with its switch (crossfaded over ~10 ms, so a
   switch never clicks; a module that lands off is made new): **bits** and **rate** (the
   cheap converter), **wow and flutter** (the medium's speed), **age** (dropouts and
   wear), **noise** (what the medium adds) and **tone** (the medium that plays it back —
   its noise included, so the hiss sits in the band). The order is the recording's:
   digitised, put on the medium, worn, played back.
2. **Bits**: `2^bits` levels over −1 … 1 (fractional bits a continuous step), rounding
   (mid-tread), truncation (towards −∞; a DC blocker at 8 Hz after it) or μ-law (μ = 255);
   optional triangular dither of one step from a hash of the seed and the song sample.
   Exact zero stays exact zero (digital silence is never dithered). **24 bits without
   dither is a wire**, to the bit (the float's own mantissa).
3. **Rate**: a sample and hold whose phase advances `rate/fs` a sample, raw (aliasing and
   the hold's images: the sound), behind a 6-pole Butterworth anti-alias at 0.45 of the new
   rate, or also smoothed by a 4-pole after it. While nothing sounds the phase is a function
   of the song sample (`n·rate/fs`), so a rested unit is where a new one would be.
4. **Wow and flutter** reuse the modulation family (ADR 0055): `ModLine`'s Hermite read, the
   family's `Lfo` and `Clock` (the wow synced to the song or free), and Vibrans's
   `sweep_for`, so the depth is in cents whatever the rate. The wow is a sine and a share
   (`wow.drift`) of the family's random wave joined smoothly at half its rate — a tape's
   wander, not a sine's. As in Vibrans, the delay is part of the effect, not latency.
5. **Age**: dropouts and wear as **deterministic processes of the song sample**: cells of
   25 ms, a hash per cell for whether a dropout starts, where, how long (10 … 120 ms) and
   how deep (to −30 dB with the amount), raised-cosine edges; the wear a smooth random of
   level (±2.5 dB) and brightness on a 0.7 s grid. A gain and a 2-pole low-pass
   (20 kHz … 1.5 kHz) apply them, read on the unit's 16-sample grid of the song.
6. **Noise is a pure function of the song sample**: every random a hash of a seed and a
   count, every shape a table read at the time since its event (dust ticks in cells of ⅔ ms,
   pops in cells of 10.7 ms, the hiss a short FIR on hashed white noise — a first difference
   for tape, an 8-tap binomial for vinyl — and the hum's phase the song's). Nothing that
   holds memory runs, so the noise at a moment of the song is the same offline, live, at
   any block size and from any bar, whatever came before. `noise.follow` lets it follow the
   music (a peak follower of the input; under −90 dBFS it lets none through).
7. **Tone**: a resonant 2-pole high-pass and low-pass at the band's edges, the medium's
   colour (tape: a head bump; radio: a forward middle; telephone: the carbon microphone's
   peak and a 24 dB top; gramophone: the horn's two resonances and a 24 dB top). Choosing a
   medium in the editor sets its band (one undo step). The editor draws the response
   analytically (each TPT section is its analogue prototype at the pre-warped frequency,
   `vetus::tone_db`), held to the heard within 0.1 dB.
8. **Character**, one macro: every module that is **on** moves a share of the way from its
   own setting to its oldest (4 bits, 4 kHz, 40 ct of wow, 14 of flutter, 80 % dropouts and
   wear, 80 % crackle, 60 % hiss, a 300 Hz – 3.5 kHz band), never younger than set; a module
   that is off stays off. The flavour is the switches, the amount one knob.
9. **Time**: everything that varies is a function of the song sample — the noise, the
   dither, the dropouts, the wear, the hold's resting phase, the grid where coefficients
   are designed and filters settle — so offline equals real time and a render from bar 5 is
   the whole render's bar 5, to the bit.
10. **Nulls and rest**: every default leaves the signal untouched (every module off; the mix
    100 % of an untouched wet path; Character 0; 0 dB). At rest (ADR 0031 §5.1) when every
    glide has landed, the lines, the hold and every filter are +0.0 (states under 1e-15 are
    flushed on the grid) — and the noise is off or follows a silent input. A noise that
    plays in silence is never at rest.

11. **The editor** — a window of its own (1120 × 800 at 100 %): at the centre the object
    that ages, **a cassette or an LP**, the user's choice (below); beside it *what comes out*
    (the output's spectrum over the input's, the converter's Nyquist, the bits' noise floor
    and the medium's band drawn and dragged there); *Character* a size up beside them, with
    the year it takes the sound back to; the six modules as cards in the signal's order,
    each card's lamp its switch. **A module's controls stay usable while it is off**
    (dimmed, they respond: set a module up before hearing it — the owner's note;
    `controls::card_lamp_live`). Turning the object back (dragging round it anticlockwise)
    raises Character; a double-click makes it new.
12. **Two objects, a view preference** (the owner's verdict on the design): *the cassette*
    (the default — its reels turning with the song, wavering with the wow; a smoked shell
    showing its tape; a level window holding the last second of the music, stepped by the
    bits and faceted by the rate; the tape creased and the label faded with age; dust, a
    spark at the head when a tick falls; the medium or the tape's type and the year printed
    on the label) and *the LP* (its groove the music of its last three turns at 33⅓, off its
    spindle with the wow, the strobe marks swaying, scratched, dusty, the hum's ripple, the
    label dated). The choice is a **user preference per unit kind**, like a colourway (ADR
    0130 §Colourways): `settings.json`'s `unit_views` (`{ "vetus": "record" }`), applied in
    `CantusApp::assemble` and when it changes (`views::fx::unit_view`), chosen in the window's
    ⋯ menu above the colourways; never in a song, never heard. The exported plug-in's window
    keeps a choice for its session.
13. **The face** comes in three colourways: **Cassette** (the face, the owner's choice: a
    tape's glossy black lacquer, its label's red, the band a label's cream), **Sleeve** (a
    record sleeve's light cream card, vermilion and teal) and **Walnut** (oiled walnut, old
    gold on a fluorescent display — at 31°: a brass at 44° on wood was 8° from Anulus's, and
    every offered colourway keeps the house rule since the integration). Every display is dark in both themes; no vents (the owner).
14. **Starting preset** (ADR 0071): *Worn cassette* — wow and flutter, tape hiss, the tape's
    band, a little wear, at its fitted level. Fourteen factory presets by style and role.
15. **Exported** as `Cantus Vetus` (CLAP/VST3), bit-identical to the engine's unit.

## Consequences

- Vetus costs per sample: up to six hashes and a few table reads for the noise, a Hermite
  read for the wobble, up to 14 SVF sections (the anti-alias, smoothing, age and tone), and
  per grid cell a dozen hashes for the age and, while they move, a few `tan`s.
- The noise is not a filtered random process (its shapes are tables): this is what makes it
  a function of the song; the owner's ear judges whether the ticks sound real.

## What the tests hold

- `cantus-dsp` (`vetus_tests.rs`, `vetus_tests2.rs` and each module's own): every module off
  is a wire at any mix; 24 bits without dither a wire in every character; every module heard;
  the same bits at block sizes 1, 37, 64, 512, 1000; live (irregular blocks) = offline; the
  noise of silence from sample 96 000 equals the whole render's; the wow's swing measured from
  zero crossings ±20 ct within 2; raw, a 9 kHz sine held at 8 kHz folds to 1 kHz at −0.2 dB,
  filtered at −53.6 dB; the hold's 7 kHz image −22.8 dB held, −47.9 dB smoothed; the
  telephone's band; the drawn band is the heard band within 0.1 dB; Character ages only what is
  on; a switch never clicks; it rests ~2.9 s into a silence and is then as good as new; the
  noise's levels (hiss −24, hum −30 dBFS at 100 %) and the crackle's density; the dropouts'
  rate (about four a second at 100 %) and depth.
- `cantus-engine`: the unit's mapping and null; `fx_vetus` (8: defaults a null, every module
  off a wire, 24 bits a wire, every module heard, block sizes, live = offline, from bar 5 to
  the bit with the noise following and with the noise playing through the silence);
  `fx_contract`'s split invariance and exact rest (heavy rests at ~3 s);
  `rt_memory::vetus_never_allocates`.
- `cantus-core`: the table's keys, defaults (the null), choices and the media's bands.
- `cantus-ui`: `vetus_face` (its own face; three colourways keeping contrast and painting the
  editor).

### Libra (the decision)

1. **A channel's everyday corrections**, in order: DC removal (a 5 Hz first-order
   high-pass), the channel operation (stereo, swap, mono, left or right to both, mid, side —
   `(a ± b)/2` on both sides), polarity per channel (inverted as `0 − x`, so silence stays
   +0.0), a delay per channel in **whole samples** (`round(ms·fs/1000)`, up to 100 ms; the
   lines are always written, so a delay set later has its history), width (mid/side, the
   side's share 0 … 2), the balance (ADR 0012's: `cantus_core::balance_gains`, unity at the
   centre and on the near side — the master's), the gain (±36 dB, on the contract's `out`
   key) and mute. The gain, balance, width and mute glide over ~20 ms; the rest are the
   console's buttons, at once.
2. **Null and rest**: every stage at its identity is skipped, not multiplied by one, so the
   defaults are a wire to the bit and 0 dB is exact; at rest when the glides have landed, the
   lines hold only +0.0 and the DC filter's state is +0.0 (flushed under 1e-15 per sample,
   made new when DC removal is off).
3. **No starting preset**: a utility inserts as a wire (ADR 0071's exception: it corrects
   what the user asks for). Its presets' levels are by design (`preset_measure`): a pad, a
   listen to one side.
4. **The window** (the owner's choice, v2): a **brass balance** at the centre — the beam
   tilting with the balance (the heavier side down) and its pointer on a fine scale, the
   arms' reach the width, each pan holding its channel's level, delay and polarity in words,
   the correlation along its foot; dragged across for the balance, up and down for the width.
   Around it the **tuner** (the streaming tracker `pitch::PitchTracker` on the output, read
   by `libra::tuner_reading` against a reference A of 415 … 466 Hz dragged beside it; the
   needle eases and rests in the middle when nothing is voiced) and the **goniometer** (the
   output's mid and side, 24 points a block in the telemetry). The tuner runs on the
   window's thread from the analyser tap: nothing on the audio thread.
5. **The face**: blue-grey anodised aluminium, signal green on a dark phosphor screen, amber
   for the balance and width marks; the balance's brass is an object's colour
   (`kit/skin/scales.rs`).
6. **Exported** as `Cantus Libra`, bit-identical to the engine's unit.

### Libra (what the tests hold)

- `cantus-dsp` (`libra_tests.rs`): the defaults a wire and a new unit at rest; the delay
  exact to the sample (1 ms and 0.25 ms at 48 kHz); the channel operations and polarity; width
  0 is mono and the balance never raises a side; the DC goes and the render is the same at
  every block size; mute glides and it rests; the tuner within ±1 cent on sines (A = 440 and
  432) and ±3 cents on the pitch tests' sung vowel.
- `cantus-engine`: the mapping and the null; `fx_libra` (a wire, −6 dB is half and the delay
  whole samples through the engine, block sizes, live = offline); `fx_contract`;
  `rt_memory::libra_never_allocates`.
- `cantus-ui`: `libra_e2e` (on the boot path: inserted, a wire to the bit; −6 dB is half; the
  side alone; undone, the wire again); the showcase's bass through *Bass in mono*.
- `cantus-plugins`: the CLAP/VST3 tests with Libra's `Direct` arm.
