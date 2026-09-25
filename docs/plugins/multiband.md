# Multiplex — the multiband dynamics

*Effect kind `Multiband`. Model `crates/cantus-core/src/fx/multiband.rs`, DSP
`crates/cantus-dsp/src/multiplex.rs` (+ `multiplex_filters.rs`, `multiplex_linear.rs`), engine glue
`crates/cantus-engine/src/fx/multiband.rs`, editor `crates/cantus-ui/src/views/fx/multiband.rs`,
strings `crates/cantus-ui/locales/fx/multiband.{en,es}.txt`. Benchmark: FabFilter Pro-MB 1.33
(`research/fabfilter/pro-mb.md`).*

## What it does

Up to **eight bands** that tile the spectrum edge to edge. Each band has **two-sided
dynamics**: a section above an upper threshold and a section below a lower threshold, each
with its own ratio and range, sharing a knee, an attack and a release in milliseconds. One
rule reads every ratio: **above 1:1 squeezes, below 1:1 stretches**, on either side.

| Section | ratio > 1 | ratio < 1 |
|---|---|---|
| above `thr` | downward compression | upward expansion |
| below `thr_lo` | upward compression | downward expansion (a gate, at a low ratio) |

So one band can hold its peaks down and lift its quiet parts at once (the OTT / Ableton /
Multiplicity model), which Pro-MB cannot — its users ask for exactly this (report §7).

Per band there is also a static gain, a lookahead, solo / mute / bypass, a stereo placement
(stereo-linked, mid only, side only), a key (the band's own input or the slot's sidechain,
chosen from any strip of the desk inside the editor) and a key filter (the band's own range,
or a free range set by two handles on the display).

## Decisions, and why

### Bands with stable numbers, placed by where they start

Parameters are sparse, `b{n}.{field}` like Aequalis, so a band keeps its number for life and an
automation lane that names `b3.thr` names the same band forever. A band's place is its `from`
(the frequency it starts at); the used bands sorted by `from` tile the spectrum, so the
crossovers are the `from` of every band but the lowest. Adding a band **splits** one — the new
band takes the part above the split and starts as a copy of the band it came from, so a split
alone changes nothing you hear; removing one gives its range to the band below. Contiguous
bands are what a crossover-based split needs; Pro-MB's "gaps" are simply bands whose ratios
are 1:1, drawn faint.

### Three ways to apply the gains, honestly named (`Mode`)

The detectors are the same in both; what differs is how the band gains reach the audio.

- **Dynamic** (default) — *the signal is not split.* The output is the input times the lowest
  band's gain, through one **shelf per crossover** that steps from one band's gain to the next
  (a Butterworth shelf of the slope's order, centred on the crossover in log frequency, built
  from TPT state-variable sections so its corner can move every sample). A shelf whose two
  sides are equal is the identity and is skipped. Consequences, all tested:
  - while no band moves, the output is the **input to the bit**, whatever the thresholds;
  - when every band moves by the same amount it is a **plain gain** — no phase shift anywhere;
  - phase shift exists only at a crossover whose two bands differ, and only as much as they
    differ; zero latency. This is what the benchmark calls Dynamic Phase, described in words.
- **Split** — a **Linkwitz–Riley tree** (6, 12, 24 or 48 dB/oct), with everything below a
  crossover passed through that crossover's all-pass before the next band joins, so the bands at
  unity **sum to an all-pass**: flat magnitude. The 6 dB/oct split is a one-pole low-pass and its
  complement, which sums to the input itself. The 12 dB/oct pair inverts its high half (odd
  Butterworth order), as it must to sum to an all-pass rather than a notch. Each band is a real
  band of the signal.
- **Linear** — a split with **linear-phase crossovers**: one linear-phase low-pass `L_c` per
  crossover (the Linkwitz–Riley magnitude of the chosen slope, zero phase, 4096 taps at 48 kHz,
  Tukey-windowed), run by uniformly partitioned FFT convolution (16 partitions of 256), and
  summed **by telescoping**: `y = g_top · x(t − D) + Σ (g_c − g_c+1) · L_c(x)`. The bands add up
  to the delayed input *by construction*, so at unity — or with every band at one gain — the
  output is the input delayed, **to the bit**, whatever the filters are. No phase shift anywhere.
  Latency 2048 + 256 = 2304 samples (48 ms) at 48 kHz, reported with the lookahead and shown in
  the toolbar. A crossover that moves is redesigned at the next block and cross-faded across it,
  so a dragged crossover does not zip. Solo and mute are its own gains (it is a true split).

**Wire at unity.** In the dynamic mode the wire is *dynamic*: the path is the identity whenever
no band's gain differs from 1, sample by sample. In the split mode the wire is *static*: when no
band **can** act (every section at 1:1 or range 0, no gain, no solo or mute) the split is not run
and the dynamic path — the identity — is heard. Switching between the two paths is a **10 ms
cross-fade**, so turning a ratio off 1:1 never clicks.

**Solo and mute need real bands**, so in the dynamic mode they are heard through the split (same
cross-fade); the toolbar says so while it happens. Solo and mute take a band whole, whatever part
of the image it acts on. Switching a band off fades it over 5 ms and then leaves an exact wire.

### Detection

Per band: the key (own input or sidechain) → a band-pass (two state-variable sections each
side, 24 dB/oct) at the band's edges, or at the free range → the level of the placement
(the louder of left and right, the mid, or the side) → a **linear-domain envelope** (attack
while the level rises, release while it falls, both time constants in ms) → the static
two-sided curve in dB → the gain. Smoothing the *level* rather than the gain is what makes
"attack" mean the same in all four behaviours: the gate opens at the attack time, upward
compression recovers at the attack time when the music comes back.

The gain computer (`multiplex::gain_db`) is the curve the editor draws, and `response_db`
evaluates the device's response from the same filters the audio runs (TPT filters are exactly
the bilinear transform of their analog prototypes, so the response is exact, not a sketch).

### Lookahead and latency

Each band has its own lookahead (0–20 ms). The audio is delayed by the **largest** lookahead of
any band that is on — that is `latency()` — and each band's gain by the difference, so each band
looks ahead by exactly its own amount. Pro-MB always costs 20 ms when lookahead is on. The
latency is shown in the toolbar at all times ("No latency" / "Latency 2.0 ms · 96 smp").

### Glides and grids (block-size independence)

Thresholds, ratios, ranges, knee, band gain, output gain and crossovers glide over ~20 ms,
updated every 16 samples counted from the start of the stream. Detectors and the split's gains
are per sample. The dynamic path's shelves are set every 16 samples and interpolated linearly
in between, from the gains at the previous update to the gains now — it follows the per-sample
gain 0.33 ms (at 48 kHz) behind the split. Meters hold the extreme of the last one or two
1024-sample windows, so a frame never misses a peak.

## What the editor shows, and what can be dragged

The display is three panes on one frequency axis, with the meters and the output on the right:

1. **Level** (dBFS): the input spectrum (fill) and output spectrum (line) from the analyser tap;
   each band's **detector level as a bar** at its column's edge, lit red where it is over a
   working upper threshold and green where it is under a working lower one; the **upper
   threshold as a solid line with a notch** and the **lower as a dashed line** — both dragged
   vertically; a free key range as a bar with two handles — dragged. Band number and state
   (off, S, M, mid, side, key) at the top of each column.
2. **Gain** (dB): each band's **lens** (how far it can cut and boost, with bracket ends), its
   **static gain as a numbered dot** (dragged vertically), what it applies now as a tick with its
   value, the **device's response now** as the gold curve (solo, mute and bypass included; with
   a mid- or side-only band, the side's curve beside the mid's, each named) and, dashed, the
   deepest of the last three seconds. Three shapes that cannot be confused: dot for gain, line for threshold, bracket
   for range — the benchmark's #1 complaint is a dot mistaken for a threshold.
3. **History**: one lane per band, its gain change over the last five seconds (cuts down,
   boosts up) and the value now.

**Crossovers** are dashed lines through both panes with a handle where the panes meet, showing
the frequency — dragged horizontally, kept a third of an octave from their neighbours. A
double-click splits the band under the pointer; the right-click menu splits, bypasses, solos,
mutes, places and removes. Keys over the display: `[` `]` step through the bands, `↑` `↓` move
the selected band's threshold by 1 dB (Shift: 0.1), `Delete` removes it.

Under the display, **the selected band's panel is docked** (it never covers the analyser): on /
solo / mute, placement, key, free key range, a knob per parameter in real units (ratios as
`4.0:1` or `1:2.0`), and the band's **two-sided transfer curve** whose two threshold points and
two ratio slopes are dragged directly, with the band's operating point on it live.

The toolbar: mode, slope, what to hear (output, delta, key), the sidechain strip, the latency.
Every drag and knob turn is one step of the project's undo; a split or a removal is one step.

Factory presets: *Mastering glue*, *De-harsh*, *Low-end control*, *Up and down*, *Vocal
smoother*, *Drum punch*.

## What the tests hold (numbers)

DSP (`cantus-dsp`: `multiplex::tests`, `multiplex::filters::tests`, `multiplex::linear::tests`):

| Test | Holds |
|---|---|
| `a_multiplex_that_does_nothing_is_a_wire_to_the_bit` | empty (any mode, and then no latency), or every band at unity in dynamic or split mode, or the dynamic mode with bands under their thresholds: output == input, bit for bit, every slope |
| `the_linear_split_is_a_delayed_wire_at_unity` | linear mode, every slope, bands that can act on a signal under their thresholds: output == input delayed by 2304 samples, bit for bit, both channels; every band at −6 dB: output == delayed input × g, bit for bit |
| `the_linear_split_has_a_symmetric_impulse_response` | bands at −6 / +4 / −9 dB: the impulse response is symmetric about the latency within 1e-4 of its peak — linear phase, measured |
| `the_split_at_unity_sums_to_an_all_pass` | 8 bands, every slope: magnitude flat within **0.001 dB** from 20 Hz to 20 kHz (measured worst: 0.00023 dB, 24 dB/oct at 59 Hz) by an f64 DFT of the impulse response |
| `equal_gains_in_every_band_are_a_pure_gain` | all bands at −6 dB in the dynamic mode: output == input × g, to the bit |
| `static_band_gains_match_the_drawn_curve` | sines at six frequencies, all three modes, every slope: measured within **0.05 dB** of `response_db` |
| `a_cut_next_to_a_flat_band_has_no_bump` | one band at −6 dB: the response is monotonic between −6 and 0 dB, dynamic and split, every slope (numbers below; linear is the zero-phase blend of the two gains) |
| `each_side_of_a_band_does_what_its_ratio_says` | downward compression, upward expansion, upward compression, downward expansion on a steady tone: within **0.3 dB** of the static curve, all three modes |
| `attack_and_release_are_milliseconds` | after one attack time the level is 1 − 1/e (±0.01); after one release time it has fallen 8.69 dB (±0.05) |
| `lookahead_delays_the_audio_and_leads_the_gain` | 5 ms = 240 samples of latency; at unity a delayed wire to the bit; the gain is in place when a step arrives |
| `the_result_does_not_depend_on_the_block_size` | all three modes × output / delta / key listening, lookahead, M/S, sidechain, free key: identical at blocks 1, 7, 64, 1000, 4096 |
| `solo_and_mute_are_heard_through_the_split` | all three modes: soloing or muting leaves < −40 dB of the other band; the soloed band loses < 0.1 dB |
| `a_bypassed_band_fades_out_and_then_touches_nothing` | switching a band off moves no sample further than the signal does, and 250 ms later the output is the input, bit for bit (dynamic and split) |
| `a_key_moves_only_the_bands_that_listen_to_it`, `a_side_band_leaves_the_mid_alone`, `delta_is_what_the_bands_change` (output + delta = unity path within 1e-5, all modes), `silence_after_noise_is_exact_silence` (no NaN; exact zeros after 20 s), `two_runs_are_identical`, `a_solo_fades_in_and_out` (no sample jump beyond the signal's own) | |
| filters | halves −6.02 dB (−3.01 dB for 6 dB/oct) at the crossover and sum to the all-pass; each shelf passes √r at its centre and is monotonic |
| linear | the FFT matches a DFT and round-trips; a crossover's linear-phase low-pass is symmetric and −6.02 dB at its frequency (±0.01 dB from 60 Hz to 15 kHz, −5.98 dB at 40 Hz) |

Adjacent cut (one band of two at −6 dB, crossover 1 kHz) — the benchmark's unpublished number:

| Mode / slope | at the crossover | an octave below | an octave above |
|---|---|---|---|
| Dynamic 6 / 12 / 24 / 48 | −3.00 dB | −4.76 · −5.63 · −5.98 · −6.00 | −1.24 · −0.37 · −0.02 · −0.00 |
| Split 6 dB/oct | −2.04 dB | −3.97 | −0.70 |
| Split 12 / 24 / 48 dB/oct | −2.49 dB | −4.43 · −5.51 · −5.97 | −0.91 · −0.25 · −0.02 |

Engine (`crates/cantus-engine/tests/fx_multiband.rs`): a new Multiplex, unity bands in dynamic and
split mode, and dynamic bands under their thresholds all render the song **to the bit**; a busy
setup in each of the three modes is finite, identical at blocks 1, 7, 64, 1000, 4096, and
deterministic; a band keyed from the bass strip changes the render and stays block-size
independent; the unit reports 5 ms of lookahead as 240 samples of latency (240 + 2304 in linear
mode) and fills its telemetry. UI (`crates/cantus-ui/tests/fx_multiband.rs`): splits and removals
are single undo steps that restore the map exactly; a crossover and a threshold are dragged
through the published geometry (one undo step per drag); every preset names real parameters and
makes bands that act; the window (empty and full) and the inspector's picture paint.

Cost (release build, eight bands with both sections working and lookahead on half of them, 48 kHz,
measured while other builds shared the machine, so an upper bound; `cost_of_eight_bands`):
dynamic 3.8–6.3 % of one core (6 → 48 dB/oct), split 4.2–7.6 %, linear 7.6–8.6 %.

## Feature matrix against FabFilter Pro-MB

| Pro-MB | Multiplex | |
|---|---|---|
| Up to 6 bands | Up to **8** | better |
| Bands placed anywhere, gaps between | Contiguous bands; a "gap" is a band at 1:1, drawn faint; split by double-click | have (different model) |
| Crossover slope 6–48 dB/oct per crossover | 6/12/24/48 for the whole device | have (not per crossover) |
| Dynamic Phase | **Dynamic** — a wire to the bit at rest, a pure gain when bands move together, no bump at adjacent cuts (numbers published) | better |
| Minimum Phase | **Split** (Linkwitz–Riley, sums to an all-pass, tested to 0.001 dB) | have |
| Linear Phase (latency undocumented) | **Linear** — telescoped linear-phase split, a delayed wire to the bit at unity, 48 ms at 48 kHz shown in the toolbar, moved crossovers cross-faded | have (numbers published) |
| Compress **or** expand per band, direction by the sign of Range | Upper **and** lower section per band, each with its own ratio and range: all four behaviours at once | **better** (their top forum request) |
| Threshold on a knob only | Threshold **and** lower threshold drawn and dragged on the display, against the band's live level | **better** |
| Attack/release in % | In **ms** (0.02–250 / 5–5000) | **better** |
| Ratio slider without a number | Ratio as `4.0:1` / `1:2.0`, and dragged on a transfer curve | **better** |
| Knee hard…soft | Knee in dB (0–24) | better |
| Range ±30 dB | Range 0–60 dB per section | have |
| Lookahead 0–20 ms, always 20 ms of latency | Per band 0–20 ms; latency = the largest in use, shown at all times | **better** |
| Output level per band, M/S ring | Band gain; placement stereo / mid / side, with the mid and side responses drawn apart | have (no L/R-unlinked or M+S pair) |
| Stereo link 0–100 % | Linked (louder of L/R), or mid, or side | partial |
| Band solo / mute / bypass | Same; bypass fades; in dynamic mode solo/mute are heard through the split with a fade | have |
| Free trigger range | Free key range, drawn and dragged on the display | have |
| Ext sidechain per band via host routing | **Any strip of the desk, chosen in the editor**; each band chooses own or sidechain | **better** (no routing dance) |
| Audition (trigger) | Hear the key of the soloed (or all) bands | have |
| — | **Delta**: hear only what the bands change | better |
| Yellow response curve, instant only | Response now **plus a ghost** of the deepest of the last 3 s, **per-band history lanes**, per-band applied gain written live | **better** |
| No input meter | Input **and** output meters, L/R, with peak values | **better** |
| Floating band panel over the analyser | Docked band panel; analyser never covered | **better** |
| Undo inside the plug-in | Every gesture one step of the **project's** undo; A/B host-kept | better |
| Oversampling 2×/4× | — | missing (aliasing from very fast gain changes is not treated) |
| Mix 0–200 % | The slot's dry/wet (0–100 %) | partial |
| Band presets, horizontal zoom, full screen | — (the window is resizable) | missing |
| Mono/stereo only | Stereo | same |

## Not done, and why

- **A shorter linear mode.** 4096 taps is one fixed latency; a "low latency" setting (2048 taps,
  less accurate below ~120 Hz) is a parameter away.
- **Oversampling, auto release, auto threshold, learn-from-clip, band presets, L/R-unlinked and
  M+S-pair bands.** Each is its own piece of work; none is half-built.
- **Per-crossover slope.** One slope for the device keeps the split's compensation simple.

## For the merge

- `latency()` is the largest lookahead of any band that is on, plus 2304 samples (at 48 kHz) in
  linear mode; it changes when a band's lookahead or the mode changes. The slot's dry/wet
  (`EffectSlot::mix`) mixes the undelayed dry with the delayed wet — with latency, the slot mix
  should delay its dry path by the unit's latency, which is the delay-compensation work's to do.
- `CantusApp::demo_editor` sets a global `thr` on every non-EQ kind; Multiplex has no such key, so
  that command is refused (harmless); `demo_setup` sets the real demo.
