# Aequalis — the EQ

Effect kind `Eq`. DSP: `crates/cantus-dsp/src/aequalis.rs` (+ `aequalis_phase.rs`,
`aequalis_fft.rs`, `aequalis_fit.rs`, tests in `aequalis_tests.rs`). Parameters:
`crates/cantus-core/src/fx/eq.rs`. Engine glue: `eq_bands` and the `Dsp::Eq` arms of
`crates/cantus-engine/src/effects.rs`. Editor: `crates/cantus-ui/src/views/eq_editor.rs`
(+ `eq_editor_display.rs`, `eq_editor_panel.rs`, `eq_editor_parse.rs`). Benchmark:
FabFilter Pro-Q 4 (`research/fabfilter/pro-q.md`).

## What it does

24 bands with numbers that never change, nine shapes (bell, low/high shelf, low/high cut
6–96 dB/oct, notch, band pass, tilt shelf, all pass), per-band placement (stereo, L, R,
M, S), dynamic bands, three processing modes, an audition (solo) per band, a measured
auto gain, a gain scale, and an editor where the display is the control.

## Decisions, and why

**Three processing modes, latency by mode only.** *Zero latency* is the matched cascade
(round 15). *Natural phase* keeps the cascade and adds a 128-tap correction FIR, designed
as `H_analog / H_digital · e^{−jω·64}` on a 512-point grid and Tukey-windowed; the result
follows the analog prototype in magnitude **and** phase up to Nyquist, 64 samples late.
*Linear phase* designs an FIR from the analog magnitudes (zero phase, frequency sampling
on a grid twice the FIR length, 4-term Blackman–Harris window) and runs it by uniformly
partitioned overlap-save convolution in 512-sample blocks. Its latency is `L + 512`:
2 560 / 4 608 / 8 704 / 16 896 samples for Low / Medium / High / Very high at 44.1–48 kHz
(the benchmark's are 3 072 / 5 120 / 9 216 / 17 408 at 44.1 kHz — ours are 512 shorter),
scaled ×2 and ×4 at 88.2–96 and 176.4–192 kHz. The latency never depends on the bands:
adding the first band cannot move the project's delay compensation (Pro-Q 4.10 had to
learn this for spectral bands).

**One 2×2 stage for mixed placements.** Bands on L/R and on M/S at once are combined into
one MIMO stage, `Y = T⁻¹·diag(M,S)·T·diag(L,R)·X`: four FIRs, not two stages in series, so
the latency does not double (Pro-Q doubles it). The natural-phase correction is built the
same way from composed 2×2 responses, so it is exact for any mix of placements.

**Allocation-free switching.** Every buffer for the longest FIR is allocated in `new`
(about 4 MB at 48 kHz, zero pages the OS commits only when linear phase is used), so the
mode and resolution change on the audio thread without allocating. A new design fades in
over one block (natural phase: 256 samples) instead of switching; designs are rate-limited
to one per `N/8192` blocks so a dragged band costs a bounded share of the CPU.

**Dynamics in linear phase.** The FIR carries the static curve; a dynamic band's movement
runs as a minimum-phase section on the output, and its detector hears the input delayed by
the latency, so the movement lines up with the audio. Only the moving part is not linear
phase (exactly the band's movement; exact when its static gain is 0 dB, the usual case).

**Dynamics, readable.** Threshold, range, ratio, attack and release are dB and ms
(Pro-Q: percentages where 50 % is "auto"). New: **direction** (`dir`: above = compress/boost
the loud moments, below = act on the quiet ones) and **detector source** (`key`: this band's
own signal, or the key strip). The key strip is the slot's sidechain, chosen in the editor
from the desk — no routing. A band set to listen to a key with no key routed falls back to
its own signal. The editor shows the band's last six seconds: detector level, the threshold
(dragged there), and the gain it moved.

**The audition is a parameter** (`solo` = band + 1). It has to reach the audio thread, and
the only channel is the spec; so it is undoable and saved. The editor ends it when it
closes, and removing the auditioned band ends it in the same step, so a saved project is
not left soloed. What is heard: a band-pass for a bell/notch/band pass, the shelf's side of
its corner, and for a cut what it removes; a band placed on L/R/M/S auditions only there.

**Auto gain is measured.** K-weighted (BS.1770 filters) power of input and output over
about 1.5 s; the gain that equalises them glides in (0.3 s) and holds below −70 dB (silence).
Pro-Q's auto gain is an estimate from the curve; this one is what a listener compares.

**Gain scale** (`scale`, 0–200 %) multiplies every gain and dynamic range, in `eq_bands`,
so the curve drawn and the sound come from one reading. **Gain–Q interaction** (`gq`, bells):
Q doubles every 12 dB of gain, applied in the same place.

**EQ match** listens to this strip's input and the key strip's output (third-octave power,
averaged while *Listening*), takes the level-matched difference (mean 0 dB over
100 Hz–10 kHz, a third-octave either side smoothing), scales it by *Amount*, clamps it to
*Max boost/cut*, and fits it into at most *Bands* ordinary bands: greedy placement
(shelves at the edges, bells elsewhere, Q from the bump's width), then damped
Gauss–Newton on all gains and a coordinate search on frequency and Q. The residual is
stated (RMS and worst dB) — the benchmark has no amount, no limits and no residual.

**The key's spectrum comes from the unit.** The analyser tap carries only (input, output)
pairs, so the unit measures the key itself (the editor analyser's FFT and scale, 4096
points every 2048 samples) and sends 31 third-octave levels in its telemetry. The editor
draws it dashed, hatches the collisions, and EQ match averages it.

**Keys are forever.** New band fields `dir`, `key`, `gq`; new globals `mode`, `lp_res`,
`auto`, `solo`, `scale`. Every default is what an old file meant. No schema bump.

## Telemetry (64 floats)

`[0, 24)` detector level per band, dBFS (−200 when not dynamic) · `[24, 55)` key spectrum,
third octaves from 20 Hz · `55` key present · `56` auto gain dB · `57` latency samples ·
`58` auditioned band + 1. Dynamic gain per band still travels in `Meters::eq_deltas`.

## The editor

- **Header**: strip chip and name, presets (8 factory), A/B, band count, auto gain, gain
  scale and output knobs, bypass.
- **Chips, visible at rest**: processing mode (segmented), LP resolution with each one's
  latency in ms, the **latency badge**, analyser In / Out / Key / Freeze / Grab /
  Collisions, analyser resolution–speed–range–tilt menu, key strip picker, Match, Piano,
  dB range.
- **Display**: double-click to create (shape by position), press on the curve and pull,
  a dashed ghost of what a click would make, rubber-band selection, relative multi-band
  drag (log-frequency and dB offsets), Ctrl-drag for Q, wheel for Q/slope (Ctrl gain, Alt
  dynamic range; a burst within 500 ms is one undo step), Alt locks the axis, Alt+click
  bypasses, double-click a dot to type values (`2k`, `A4`, `C#2+13`, `2x`), right-click for
  the band menu (shape, placement, **split L/R or M/S**, dynamic, audition, bypass, type,
  remove). The dB range widens itself when a drag passes it. Dynamic bands show their
  reach, a ▲▼ handle to drag the range, the live curve and a tick at the live gain.
  Placed bands get L/R/M/S badges and per-channel result curves named at their end.
  A plate beside the selected dot has its numbers. Cursor readout: frequency, note, EQ gain,
  input, output and key levels in dB. A legend names what is drawn.
- **Frequency axis**: drag up/down to zoom around the pointer, sideways to pan, wheel to
  zoom, double-click to reset; or a **piano** (A0 up), with drags quantised to semitones.
- **Spectrum Grab**: a visible toggle; the spectrum stops, peaks are labelled (Hz or note,
  and dB); press on one and pull to make a bell there with a Q from the peak's width.
- **Keyboard** (display focused): 1–9, 0 select; Ctrl+number adds; Tab/Shift+Tab step;
  ←/→ a semitone (Shift 10 cents); ↑/↓ 0.5 dB (Shift 0.1); `[`/`]` Q by a sixth of an
  octave (cuts: slope ±6); Delete removes; B bypass; D dynamic; S audition; P placement;
  Ctrl+A all; Enter types; Esc cancels a drag, ends the audition, then deselects. Space,
  undo, redo and save still reach the app.

## What the tests hold

(`cargo test -p cantus-dsp aequalis`, `-p cantus-engine --test fx_eq`,
`-p cantus-ui --test fx_eq`; `EQ_REPORT=1 … -- --nocapture` prints the numbers.)

- Matched vs bilinear, worst dB 20 Hz–20 kHz at 48 kHz (unchanged from round 15): 16 kHz
  bell Q2 0.61 vs 4.27; 12 kHz bell −12 dB 2.44 vs 5.17; 14 kHz shelf 0.27 vs 1.80.
- **Natural phase vs the analog prototype, 20 Hz–20 kHz**: 16 kHz bell +9 dB Q2: 0.000 dB,
  0.01° (zero latency: 0.63 dB, 12.4°); 12 kHz bell −12 dB: 0.001 dB, 0.02° (ZL 2.44 dB,
  25.1°); 10 kHz shelf: 0.000 dB, 0.01° (ZL 9.8°); three bands incl. an 80 Hz 24 dB/oct cut:
  0.036 dB, 1.1° (ZL 0.89 dB, 4.3°). Asserted: < 0.1 dB, < 2°, never worse than ZL.
- **Linear phase vs the drawn curve** (four bands, 40 Hz–18 kHz): Medium 0.014 dB, High
  0.004 dB worst; impulse response symmetric about the latency to 6e−8; peak exactly at
  the reported latency. Asserted < 0.1 dB.
- L/R + M/S bands together: one stage, the stated latency, each output within 0.1 dB of
  the 2×2 matrix.
- Flat in every mode: the input delayed by exactly the stated latency, to the bit (DSP,
  and through the engine's renderer for ZL, NP and LP Low/Medium/High).
- Block sizes 1, 7, 64, 1000, 4096 give identical output in every mode with a static band,
  a key-driven M band, a below-threshold S band, auto gain and output gain (DSP), and
  through the renderer with a dynamic band; keyed from another strip too.
- Dynamics: below-threshold band lifts a quiet tone by 5.5+ dB and leaves a loud one; a
  key-driven band ignores its own signal and follows the key (−8.5 dB+).
- Audition: at the band's frequency within 0.5 dB, 100 Hz below −20 dB, the other side of
  a left band silent — in all three modes.
- Auto gain: a +9 dB high shelf on noise; after 8 s the K-weighted loudness out equals in
  within 0.5 dB (measured 0.00 dB; the gain settled at −8.7 dB).
- The key spectrum reads a −6 dB sine at −6 ± 1.5 dB in its third octave.
- No NaN at the extremes (10 Hz Q 40 +30 dB, 96 dB/oct at 20 kHz, 0.1 ms attack, +12 dBFS
  noise) in any mode.
- EQ match fit: a curve made of three bands is fitted back within 0.5 dB RMS with four
  bands, and the stated residual is the real one.
- UI: rubber band + relative multi-band drag = one undo step; pulling the curve = one;
  keyboard select/nudge/Q/bypass/delete; typed `A4`/`2x`/`3` + Enter = one step; split = one
  step; audition ends on close and with its band; a match's bands at free numbers, one step;
  presets name only real parameters and have names in both languages; both themes paint.

## Feature matrix against Pro-Q 4

| Feature | Pro-Q 4 | Aequalis | |
|---|---|---|---|
| Bands | 24 | 24, numbers stable | have |
| Shapes | 10 incl. flat tilt | 9 (no flat tilt) | missing: flat tilt |
| Fractional slopes, brickwall | yes | 6–96 in 6 dB steps | missing |
| Analog-matched ZL | yes | yes, error published per case | better (numbers in tests) |
| Natural phase | yes, latency undocumented | 64 samples, analog mag+phase within 0.04 dB/1.1° | have, stated |
| Linear phase | 5 resolutions, L+1024 | 4 resolutions, L+512 | better latency; missing "Maximum" |
| L/R + M/S in LP | doubles latency | one 2×2 stage | better |
| Latency shown | help text only | badge always on screen, per-resolution ms in the menu | better |
| Dynamic EQ | %, auto, one direction | dB/ms, ratio, above **or** below | better |
| Dynamic in LP | up to High | every resolution (movement minimum phase) | have, trade-off stated |
| Sidechain | one external input | any strip, per band, no routing | better |
| Gain change over time | live curve only | live curve + 6 s history with draggable threshold | better |
| Spectral dynamics | yes | — | missing (next) |
| Analyser pre/post/SC, freeze | yes | yes, all visible chips | have |
| Resolution / speed / range / tilt | yes | yes | have |
| Spectrum Grab | hover-activated | a toggle; peaks labelled in Hz/note **and dB** | better |
| Collision view | vs one reference | vs the key strip, hatched as well as tinted | have (+ colour-blind safe) |
| EQ Match | inside the instance list; bands slider | one click; amount, limits, band budget, residual | better |
| EQ Sketch | yes | — | missing |
| Auto gain | curve estimate | measured K-weighted loudness | better |
| Gain scale, output | yes | yes | have |
| Band solo | hold to listen | toggle, per placement, visible at rest | have |
| Gain–Q interaction | yes | yes | have |
| Split to L/R or M/S | yes | yes | have |
| Piano display | yes | yes, semitone drags | have |
| Typed values `2k`, `A4`, `C#2+13`, `2x` | yes | yes | have |
| Axis zoom / pan | yes | yes, + wheel | have |
| Auto-expanding range | yes | yes | have |
| Rubber band, multi-drag | yes | yes | have |
| Keyboard | presets only | select, nudge, Q, bypass, solo, dynamic, placement, delete, type | better |
| Undo | plug-in local | project undo, one step per gesture | better |
| Character modes | yes | — | missing (P2) |
| Instance list | yes | the mixer is it (not built here) | missing here |
| Cursor dB readout | requested by users | freq, note, EQ, in/out/key dB | better |
| Themes / languages | dark, English | light and dark, EN/ES | better |

## Not done, and why

Spectral dynamics (needs its own STFT engine and an honest low-latency variant), EQ Sketch
(the fit engine is ready for it), fractional slopes and brickwall, flat tilt, a "Maximum"
LP resolution (1.4 s latency and 4× the memory for detail Very high already reaches),
character modes, MIDI-following bands, and a cross-track masking matrix (needs a tap on
every strip, not one). Analyser settings are editor state, not saved in the project.

## Outside the EQ's own files

- `crates/cantus-core/src/effect.rs` (a test): the parameter count is now
  `EQ_BANDS × fields + EQ_GLOBALS.len()` instead of `… + 1`, since the EQ has globals
  besides `out`.
- `crates/cantus-ui/src/views/fx/mod.rs`: `demo_setup` and `demo_feed` dispatch `Eq` to
  `eq_editor::demo_setup` / `demo_feed` (two match arms).
- `crates/cantus-ui/examples/shoot.rs`: the EQ shot calls `views::fx::demo_feed` after its
  own feed, so the screenshot shows the key strip, the collisions and the dynamics history.

New files: `aequalis_{fft,phase,fit,tests}.rs` (declared from `aequalis.rs` with `#[path]`,
so `cantus-dsp/src/lib.rs` is untouched) and `eq_editor_{display,panel,parse}.rs` (declared
from `eq_editor.rs` the same way).
