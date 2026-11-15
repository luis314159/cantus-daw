# The effects side round, wave B — `lenis`: Lenis, the resonance suppressor (DSP phase)

Branch `fx/lenis` from `fx/integration` d3b51a09. **DSP only**: no `EffectKind`, no core
parameters, no engine unit, no panel, no ADR number yet. The name is provisional — one
constant, `cantus_dsp::lenis::NAME` (*lenis*: gentle, smooth). The function of the
soothe-class processors (oeksound soothe2, and the dynamic-EQ "resonance" modes of others)
is the reference; the algorithm is our own, from the literature cited below, never from a
product.

## Done

| Layer | Files (owned) | Shared files |
|---|---|---|
| DSP | `cantus-dsp/src/lenis.rs` (the unit: settings, latency, the per-sample path, the frame, the meter, rest), `lenis_stft.rs` (the low-delay window pair, the two-channel packing), `lenis_detect.rs` (the detector, the curve, the focus); tests `lenis_tests.rs`, `lenis_tests_measure.rs`, `cantus-dsp/tests/lenis_alloc.rs`; example `cantus-dsp/examples/lenis_sounds.rs` | `cantus-dsp/src/lib.rs`: one line, `pub mod lenis; // fx-b:lenis` (after `latitudo`) |
| Docs | this file | — |

### The DSP API

```rust
use cantus_dsp::lenis::{Lenis, LenisSettings, LenisMeter, Channels, Mode, Quality, FocusBand,
                        latency_for, display_hz, DISPLAY, FOCUS_BANDS};

let mut u = Lenis::new(sample_rate);       // allocates here only (sized for the largest frame)
u.set(&LenisSettings { depth: 0.4, ..Default::default() }); // taken at the next hop
u.process(&mut left, &mut right);          // in place, any block size
u.latency();                               // samples: 511 at 48 kHz Standard (latency_for(&s, sr))
u.meter();                                 // LenisMeter, refreshed every frame
u.reduction_bins(0);                       // dB per bin, detector 0 (linked/left/mid) or 1
u.bin_hz(k); display_hz(p);                // bin and display-point frequencies
u.at_rest(); u.rest_blocker(); u.skip(n); u.reset();
lenis::detect::focus_weight(&s, hz);       // the focus curve, for drawing it
lenis::detect::reduction_db(excess, threshold, depth, mode); // the curve, for drawing it
```

`LenisSettings` (real units): `depth` 0…1 (shown as %), `sharpness` 0…1, `selectivity` 0…1,
`attack_ms` 0.5…200, `release_ms` 5…2000, `low_hz` / `high_hz` (the focus's limits, a
third-of-an-octave fade outside), `bands: [FocusBand; 6]` (`on`, `hz`, `amount` −1…+1,
`width_oct` 0.1…4 at half height), `channels` (`Linked`, `Independent`, `MidSide`), `mode`
(`Soft`, `Hard`), `quality` (`Fast`, `Standard`, `Precise`), `delta`, `mix` 0…1, `out_db`
±24. **`Default` is the null** (depth 0). There is no factory preset file yet: the
listening example's *gentle* and *strong* settings are the first two (see *Shown*).

`LenisMeter`: `level_db[128]` (the input's spectrum as the detectors hear it, dBFS, 128
points log-spaced 20 Hz – 20 kHz), `reduction_db[128]` (the most over the detectors and the
point's bins), `max_reduction_db`, `frames` (the editor sees when it moved).

### How it works (each file's header says it in full)

```text
     ┌──────────────── ring (the dry, delayed by the latency) ────────────────┬─ mix ─┬─ × out
in ──┤                                                                         │       │
     └─ every hop: frame ─ wa ─ FFT (L + jR) ─ detector(s) ─ gains ─ IFFT ─ ws ┘  delta: dry − that
```

- **The transform** (`lenis_stft.rs`): an STFT whose analysis window is long and whose
  synthesis window is short — the low-delay asymmetric pair of Mauler & Martin (EUSIPCO
  2007): √Hann rising over `N − M`, √Hann falling over `M`; the synthesis window is
  `Hann(2M)/wa` over the last `2M` samples, so `wa·ws = Hann(2M)` and the frames sum to a
  constant at a hop of `M/2`. `M = N/4`: a 1024-point analysis (47 Hz bins) with a latency
  of `2M − 1 = 511` samples (10.6 ms) at 48 kHz. Fast 512 (255 samples, 5.3 ms), Precise
  2048 (1023, 21.3 ms); doubled at 88.2/96 kHz and again above, so the times stay. Both
  channels go through one complex FFT (left real, right imaginary) and are taken apart by
  conjugate symmetry. The FFT is Aequalis's (`aequalis::fft`, twiddles in `f64` once).
- **A frame nobody turned down is not transformed back.** It adds its windowed input to
  the overlap straight from the ring (`wa·ws`), and an output sample that no turned-down
  frame covers is taken from the dry ring itself: the input delayed by the latency, to the
  bit. So at depth 0, at mix 0, on silence and wherever nothing sticks out, Lenis is a pure
  delay. All-zero frames skip the FFT too.
- **The detector** (`lenis_detect.rs`), per bin, per frame, for each detector (Linked: one
  on both channels' power; Independent: one each; Mid/Side: one on the mid, one on the
  side):
  1. **Two averages of the power**, 30 ms and 5 ms, flushed to +0.0 under −160 dBFS.
  2. **The upper envelope over the partials**: the loudest bin within ±120 Hz (f/6 under
     720 Hz), then a triangle as wide — a morphological closing in dB. A note's harmonics
     become the smooth outline over them, so a resonance is a bump in that outline and
     never a harmonic itself.
  3. **The envelope**: over a box of `W` octaves around the bin (Sharpness: 3 octaves at 0,
     3/8 of one at 1; never narrower than `4 + 2·bridge + 1` bins a side), the greatest of
     the least-squares line through the box (against octaves: it follows any slope), the
     mean of the two sides beyond a guard (each moved along the box's slope to the bin),
     and the higher side less 3 dB — the "greatest of" cell-averaging detector of radar
     (GO-CFAR, Hansen & Sawyers 1980), which keeps a roll-off's knee or a band's edge from
     being taken for a peak. The peaks are clipped out (estimate + 1 dB) and the estimate
     made again, three times: a robust smoothing in the spirit of a median (as
     harmonic–percussive separation smooths across frequency, Fitzgerald 2010), every sum a
     difference of prefix sums with the constants precomputed — O(1) a bin.
  4. **The excess** = upper envelope − envelope, on each time scale; the slow one is what
     counts, raised to the fast one less 4 dB (a sudden resonance is caught at once) and held
     under the fast one plus 3 dB (it lets go as soon as the ringing stops).
  5. **The curve**: `e` = excess − threshold (Selectivity: 1.5 dB at 0 … 6.5 dB at 1). Hard:
     `4·depth·e`; Soft: `4·depth·e²/(e + 4 dB)` (no slope at the threshold). × the focus
     weight (0…2), never more than the excess itself (no bin is dug below the envelope),
     never more than 30 dB.
  6. **Spread and time**: the most of three neighbouring bins, then `[¼ ½ ¼]` (no bin is cut
     alone: a needle in frequency is a long ring in time, and an isolated bin switching on
     and off is the "musical noise" of spectral subtraction — Berouti et al. 1979, Cappé
     1994); then attack and release, one-pole in dB at the frame rate; released to +0.0
     under 10⁻³ dB.
- **On a grid**: frames every hop (128 samples at 48 kHz Standard) counted from the start
  of the stream; `set` waits for the next hop's first sample (a new unit takes it at once);
  the mix, the output gain and the delta switch glide 10 ms per sample, landing exactly.
- **Delta** = `dry − (the normal output before the output gain)`, computed as that
  subtraction: at 0 dB out it is the input at the latency minus the output, to the bit.
- **Exact rest**: after `N + 2M` exact zeros in, with every average and reduction +0.0, no
  frame still turned down, no pending setting and the glides landed. `skip(n)` advances the
  stream clock; `reset()` clears the audio state **and keeps the clock** (the frame grid),
  so a reset twin and a processed one agree to the bit.

### The design choices

- **STFT against a filterbank.** A filterbank would have no block latency, but the
  detector needs a resolution well under a resonance's width (a 1/12-octave peak at 3 kHz
  is 175 Hz: four bins of 47 Hz) over the whole band — 150+ bands a channel, each a
  dynamic bell whose overlapping phases make ripple when neighbours cut together — and a
  per-band envelope follower can't see the *shape* of the spectrum around a band, which is
  what tells a resonance from a loud region. The STFT gives the whole spectrum each frame
  for O(N log N) and applies any smooth per-bin gain exactly. Not measured against a built
  filterbank (none was built): argued, and the STFT's own costs measured — latency 10.6 ms
  (target 10–20 ms met), pre-echo under −27 dB, CPU below.
- **The window pair, measured** (`the_window_pairs_compared`, a +12 dB Q 8 peak at 3 kHz
  and a +12 dB Q 6 one at 250 Hz on pink noise, Hard 50 %; a struck 2.5 kHz resonance's
  pre-echo in the 20 ms before each strike): at the same 10.6 ms latency the asymmetric
  1024/256 pair cuts the 250 Hz peak by −2.5 dB against −1.5 dB for a symmetric 512, with
  the same pre-echo (−27.7 / −27.5 dB under the strike) and the same 3 kHz cut (−9.4 /
  −9.7); a symmetric 1024 needs twice the latency (21.3 ms) for −2.9 dB and has more
  pre-echo (−25.4). So Standard is 1024/256; Precise (2048/512, 21.3 ms) is for the low
  mids (−4.6 dB at 250 Hz).
- **What a resonance is**: the spectrum against its own smooth envelope, as asked, with two
  refinements found by measuring: the *upper envelope over the partials* (without it a sung
  note's harmonics stood 20–40 dB out of their envelope and the first listening render of the
  voice lost 13 dB), and the *greatest-of* envelope (without it the knee of a roll-off — a
  low-passed source, the top of a band-limited file — stood +38 dB out). Cepstral smoothing
  was considered and not taken: a lifter short enough to bridge harmonics blurs narrow
  resonances, it is pulled up by the peaks it should stand under, and it can't be made
  edge-safe.
- **Smoothing in time and frequency, against musical noise and zipper**: the 30 ms power
  average (measured: noise's excess 99.9th percentile +3.4 dB against +8.9 for a 12 dB
  resonance; in the first detector 15 ms gave +5.7 against +9.2 and 30 ms +4.4 against
  +9.1), the spread over three bins, attack and
  release in dB, the gains applied per bin in the frequency domain with a window pair that
  sums exactly (no zipper by construction: a gain changes once a hop, cross-faded by the
  overlap).

## Measured (48 kHz; `cargo test -p cantus-dsp --release --lib lenis -- --nocapture --include-ignored`)

- **The null**: depth 0, and mix 0, at every quality × channel mode × curve (38 cases, the
  focus's limits above the band among them): the output **is** the input delayed by
  `latency()` (511 / 255 / 1023 samples), bit for bit, on both channels. The dry path is
  the ring itself, so the latency compensation is exact; a host aligns Lenis by
  `latency()` like any other unit.
- **Flat noise barely touched** (8 s; the removed part's level under the input, and the
  worst third-octave band's change): at the defaults with depth 50 % — pink **−53 dB**,
  0.01 dB; white −67 dB, 0.01 dB. Depth 100 %: pink −47 dB / 0.02 dB. Only at selectivity
  0 (a threshold of 1.5 dB) does noise get worked: pink −24 dB / 0.58 dB.
- **A resonance cut, its neighbours kept** (pink noise with +12 dB at 3 kHz, Q 8; the peak's
  sixth of an octave, and 200 Hz–1 kHz, 1–2 kHz, 5–10 kHz, 10–16 kHz): Soft at depth
  25/50/75/100 % **−3.4 / −6.7 / −8.9 / −9.4 dB**; Hard −5.7 / −9.6 / −9.7 / −9.7 dB; every
  neighbouring region within **0.01 dB**. Hard saturates past ~50 %: the cut never goes
  under the envelope the detector sees (≈ 9.7 of the 12 dB), by design.
- **Against sharpness** (Hard 50 %; a narrow Q 20 peak at 4 kHz and a broad Q 1.5 bump at
  1.5 kHz, both +10 dB): sharpness 0 / 0.25 / 0.5 / 0.75 / 1 — narrow −8.5 / −8.2 / −7.8 /
  −7.0 / −5.8 dB; broad −7.1 / −5.2 / −2.4 / −1.6 / −1.6 dB. The broad bump is let go as
  sharpness rises; above 0.75 the box's floor (`4 + 2·bridge + 1` bins) holds it.
- **Notes are not resonances** (a steady note, harmonics at 1/k with a slow vibrato,
  Soft 50 %): the note alone changes **≤ 0.16 dB** in 400 Hz–2 kHz and ≤ 0.01 dB above, at
  110 (Standard and Precise), 220 and 330 Hz. The same note with a +10 dB Q 8 peak at 3 kHz:
  the peak −2.2 / −2.5 / −1.2 / −2.6 dB; with a strong setting (Hard 80 %, selectivity
  25 %) −5.9 / −5.5 / −4.6 / −4.4 dB, 800 Hz–2 kHz and 5–15 kHz untouched (≤ 0.04 dB).
  **A miss**: under ~720 Hz the bridge is small and a note's low partials are peaks of
  their own — that setting takes 1.2 / 0 / 3.4 / 4.6 dB in 400–800 Hz; the focus's low limit is
  the tool (the listening set uses 300 Hz for drums, 700 Hz for guitar, 1 kHz for voice).
- **The detector's statistics** (`diagnose_the_excess`, the excess over 200 Hz–16 kHz):
  pink noise median −0.1 dB, 99th percentile +2.4, 99.9th +3.4; a +12 dB Q 8 resonance on
  it +8.9; pink noise low-passed at 5 kHz (eight poles) 99.9th +2.3 (the knee is not a
  peak; +38 dB before the greatest-of envelope); a Q 4 +8 dB peak on a 12 dB/octave slope
  **+3.4 only** (+0.8 before the sides were moved along the slope) — **a partial miss**:
  broad peaks on steep slopes are found weakly.
- **Time response** (a 3 kHz tone, −20 dBFS, over pink noise from 1 s to 2 s, Hard 60 %; it
  is held at −20.9 dB): attack 1 / 5 / 20 / 50 ms — half the reduction **13 / 19 / 32 /
  56 ms** after the onset, 90 % 45 / 51 / 80 / 149 ms; release 50 / 80 / 200 / 500 ms — half
  let go **56 / 75 / 163 / 371 ms** after the end, 90 % 144 / 208 / 483 / 1 176 ms. Nothing
  before the tone (< 0.5 dB). The fast detector catches the onset (half within 13 ms at a
  1 ms attack); the last 10 % follows the slow 30 ms average — **a miss against "a few ms"**
  for the full depth; the release is a one-pole time constant (90 % ≈ 2.3 × release plus
  the detector's ~60 ms).
- **The latency, checked on the signal**: processed hard, the output's cross-correlation
  with the input peaks exactly at `latency()`; 255 / 511 / 1023 at 48 kHz, 511 at 44.1,
  1023 at 96, 2047 at 192 kHz (Standard).
- **Delta** = input at the latency − output, **bit for bit**, at mix 100 % and 60 %,
  Linked, Independent and Mid/Side.
- **Block size**: 1 / 37 / 64 / 512, a setting changed mid-stream (depth and curve; the
  channel mode with mix, output and delta; Fast to Precise; the defaults to heavy):
  identical to the bit. A setting given anywhere in a hop is taken at the next hop: the
  same bits.
- **Rest**: at rest 1.3 s after the music stops (Standard and Precise, release 60 ms);
  19.8 s with a 2 s release (the reduction lets go at its own pace). Processing zeros,
  skipped, reset-and-skipped, and a new unit that heard only silence for as long: the same
  bits on music afterwards. A new unit is at rest; silence in is +0.0 out.
- **No allocation after `new`** (`tests/lenis_alloc.rs`, a counting allocator): at 44.1,
  48, 96 and 192 kHz, every quality × channel mode × curve with the delta and a focus band,
  the defaults, settings changed mid-stream (the frame size among them), blocks of 1, 37,
  64, 512, 1024, the meter, the per-bin reductions, `reset`, `at_rest`, `skip`: **0**.
- **Deterministic maths**: `cantus_dsp::math::*` only (clippy's `disallowed-methods` clean
  in the library, the tests and the example); the detector's `log10` is our own plain
  arithmetic (exponent + `atanh` series, within 2·10⁻⁶ of `math::log10` over 10⁻¹⁴…10⁶,
  tested), a third of the library call's cost.
- **CPU** (one core, 48 kHz, a minute of a harsh stereo signal in blocks of 256, release
  build, best of three; no `rustc` running at the start or the end of the run, other
  builders' idle `cargo` processes present): Standard **Linked 2.97 %**, Independent 5.04 %,
  Mid/Side 4.97 %; Fast 2.62 / 4.43 / 4.43 %; Precise 3.26 / 5.59 / 5.56 %. At depth 0 (the
  transform and the averages only) 0.95–1.37 %. (Caelum, the shimmer reverb, 3.1 % with
  16 lines.) The detector was 3.9 % before its constants were precomputed and its `log10`
  made our own.
- **The suite**: `cargo test -p cantus-dsp --no-fail-fast` — 938 passed in the library
  (Lenis's 18, two more ignored: the CPU and the detector's statistics), every
  integration test and `lenis_alloc` green; `cargo clippy -p cantus-dsp --all-targets --
  -D warnings` clean; `cargo fmt -p cantus-dsp` applied.

### Tests and what each holds

`lenis_stft.rs`: `the_window_pair_reconstructs` (five shapes, the overlap sums to 1 within
10⁻⁶, the FFT path's window equals the direct one), `two_channels_share_one_transform`.
`lenis_tests.rs`: `depth_zero_and_mix_zero_are_the_input_delayed_to_the_bit`,
`the_latency_is_fixed_and_reported`, `the_output_does_not_depend_on_the_block_size`,
`a_setting_given_anywhere_in_a_hop_is_taken_at_the_next_hop`,
`delta_is_the_input_minus_the_output_to_the_bit`,
`silence_in_is_silence_out_and_a_new_unit_is_at_rest`,
`it_comes_to_rest_exactly_and_its_twins_agree`, `the_meter_shows_the_resonance_where_it_is`,
`the_focus_curve_is_one_where_nothing_acts_and_follows_its_bands` (and the curve's shape),
`the_detector_s_log_is_close_to_the_library_s`. `lenis_tests_measure.rs`: the measurements
above (`a_resonance_on_pink_noise_is_cut_and_its_neighbours_kept`,
`flat_noise_is_barely_touched`, `the_reduction_follows_depth_and_sharpness`,
`a_note_s_harmonics_are_not_resonances_but_a_peak_over_them_is`,
`the_time_response_follows_attack_and_release`, `the_window_pairs_compared`;
`cpu_at_48k` and `diagnose_the_excess` ignored). `tests/lenis_alloc.rs`.

## Shown to the owner

`C:\projects\daw-fx\compartir\lenis\sound\` — the guitar (Carbonthief, CC0) through two of
Fragor's pedals and a honky cabinet, a synthetic harsh sung line with whistling "s" sounds,
a synthetic boxy kit; each dry, *gentle* (Soft 35 %), *strong* (Hard 60 %, selectivity
30 %) and the strong *delta*; level-matched, aligned; `README.md` (what to listen for) and
`render-log.txt` (settings, the meter, the change per octave band).

## Verified only in emulation / needs the owner

- Everything is DSP-only and measured on synthetic signals and one recorded guitar; **the
  sound is for the owner's ears**: whether the deltas sound like rings and whistles (right)
  or like a thin copy of the instrument (too much); any watery or chirping artefact; any
  smear of the attacks; whether the guitar sounds less harsh or only thinner in the mids
  (Lenis acted mostly at 500 Hz–1.4 kHz on it and left the cabinet's broad 2.7 kHz peak
  nearly alone — see the misses).
- No real music beyond the guitar DI: a mix, a real vocal and real drums would say more.

## What the later phases need (Phase B)

1. **Core**: an `EffectKind` (the contract's `ALL` index), `PARAMS` for every field — depth,
   sharpness, selectivity (%), attack, release (ms), low and high limits (Hz), six focus
   bands × (on, frequency, amount %, width in octaves), channels (Linked / Independent /
   Mid-Side), curve (Soft / Hard), quality (Fast / Standard / Precise), delta, mix, output —
   plus the `out` key; schema bump and fixture; commands with undo and the parameter menu
   (automate, modulate, MIDI learn); the undo fuzz arm; the starting preset in
   `starting.rs` (ADR 0071: the defaults stay the null).
2. **Engine unit** (`cantus-engine/src/fx/lenis.rs`): `FxUnit` over `Lenis` — `latency`
   from `latency_for` (**it changes with Quality**: the engine must re-report the delay
   compensation, as for Sibilus's look-ahead; a quality change resets the unit — a
   10–20 ms gap, documented); `at_rest`/`skip` straight through; `reset` keeps the stream
   clock (say so to the engine's reset semantics); the meter (`LenisMeter`, 2 × 128 floats a
   frame) to the editor's telemetry; `tests/fx_lenis.rs` (the null — delayed by the
   latency), its rows in `fx_contract.rs` and `must_rest`, `rt_memory.rs`, the bench kernel.
3. **A panel with its own personality — the design sheet first**, for the owner's
   approval. A proposal to draw: *Lenis* is calm water over rough stones. The **centre is a
   dark display** in both themes (Slate and Paper keep it dark): the live input spectrum as
   one fine bright line, and the **reduction hanging under it per frequency** as a
   translucent veil whose depth is the cut (dB scale on the right), so the eye sees *where*
   it smooths and *how much*, moving at 60 Hz and settling to a still line at rest (no dead
   slab, no −∞); over it the **focus curve** as a thin band with **draggable handles** — two
   vertical gates for the low and high limits, a node per band (drag in frequency and
   amount, a wheel or side handles for the width), double-click to add, right-click for
   the band's menu. Under the display, controls grouped by function with label/value units,
   nothing abbreviated in English or Spanish: *Depth / Profundidad* (the large one),
   *Sharpness / Nitidez*, *Selectivity / Selectividad*; *Attack / Ataque*, *Release /
   Liberación*; *Curve: Soft · Hard / Curva: Suave · Dura*; *Channels: Linked · Independent ·
   Mid/Side / Canales: Enlazados · Independientes · Medio/Lados*; *Quality: Fast · Standard ·
   Precise / Calidad: Rápida · Estándar · Precisa* with its latency in ms beside it; *Listen
   to what is removed / Escuchar lo que se quita* (the delta); *Mix / Mezcla*, *Output /
   Salida*. A material of its own (e.g. a pale stone-grey plate with fine engraved strokes
   and a single cool accent), unlike any existing unit's; Aequalis's curve and Delay's
   window as the quality bar; FabFilter-class and oeksound-class windows as references for
   function, never copied. At 75–200 %, EN and ES, Slate and Paper, at rest and playing;
   the mini view under the insert (the veil alone); the exported CLAP/VST3 window the same.
4. **Presets**: ten or more, named and made by ear — the starting preset (Soft, ~35 %,
   from ~200 Hz), *Gentle polish*, *Harsh vocal* (from 1 kHz, focus on 2–5 kHz), *De-ess
   wide* (focus 5–10 kHz), *Boxy drums* (from 250 Hz, focus 400–900 Hz), *Fizz tamer*
   (guitar, 2–8 kHz), *Room ring* (Precise, low mids), *Mix bus air* (Linked, light, high
   shelf focus), *Strings and brass*, *Mid only* / *Side only* (Mid/Side with the side or the
   mid focus), *Surgical* (Hard, sharp) — each a `LenisSettings`, fitted to level.
5. **Export**: the CLAP/VST3 descriptor, bit-identical to the engine (the latency reported
   to the host, and re-reported on a quality change); golden ids appended.
6. **Docs**: the ADR (the low-delay asymmetric window, the upper envelope over partials,
   the greatest-of envelope with moved sides, the two time scales, the "untouched frames are
   the dry" null), `docs/plugins/lenis.md`, the manual EN/ES, the AI tools' description, the
   site's unit card.
7. **End to end**: insert it from the picker on a playing track through
   `CantusApp::assemble`, move Depth, hear the master change (and the delta switch), with
   the delay compensation holding the track in time.

## Left (this phase)

- **The misses above, to weigh by ear before more work**: broad peaks (Q ≲ 4) on steep
  slopes are found weakly; a note's low partials (under ~720 Hz) read as peaks (the focus's
  low limit decides); the last 10 % of the attack follows the 30 ms average. Each has a
  known next step (a slope-aware edge test, a pitch-aware bridge, an onset-gated fast
  path) — worth taking only if the owner hears the need.
- The owner's listening; then the phases above.
