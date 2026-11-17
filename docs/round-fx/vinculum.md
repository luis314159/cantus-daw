# The fx side round, wave B — `vinculum`: Vinculum, the bus compressor (Phase A)

Vinculum (Latin *vinculum*, a bond — what glues a mix) is a bus compressor with three
models, each derived from a published circuit description or paper. **Phase A is the DSP
only**, in `cantus-dsp`, with no contract yet: no `EffectKind`, no schema, no ADR number.
Generic names in everything the user will see: **VCA**, **Opto**, **Vari-mu**.

## Done

| File (owned) | What |
|---|---|
| `crates/cantus-dsp/src/vinculum.rs` | the unit: settings, the three models, side-chain high-pass, make-up, mix, the meter, rest |
| `crates/cantus-dsp/src/vinculum_law.rs` | the static laws (VCA, opto cell, vari-mu valve) and the shared two-stage ballistics |
| `crates/cantus-dsp/src/vinculum_valve.rs` | the vari-mu's valve colour at 2× through an IIR half-band (its design computed) |
| `crates/cantus-dsp/src/vinculum_tests.rs`, `vinculum_tests_time.rs`, `vinculum_measure.rs` | the tests and the measurements |
| `crates/cantus-dsp/tests/vinculum_alloc.rs` | no allocation after construction (a counting allocator) |
| `crates/cantus-dsp/examples/vinculum_sounds.rs` | the offline renders (no engine) |

Shared file: `crates/cantus-dsp/src/lib.rs`, one line `pub mod vinculum; // fx:vinculum`.

### The models

**VCA — a console bus compressor, feed-forward.** The key is the louder side's peak
(`max(|L|, |R|)`, through the side-chain high-pass), in dB; the gain computer is
Giannoulis, Massberg and Reiss, *Digital Dynamic Range Compressor Design — A Tutorial and
Analysis*, JAES 60:6 (2012), eq. 4 (threshold, ratio, a 3 dB soft knee — the knee width
is our choice: a console bus compressor's knee is firm, not hard). The ballistics follow
the computer in the log domain as the paper recommends, as its *smooth branching* detector
(eq. 17): the attack's coefficient up, the release's down. Ratios 2 / 4 / 10, attacks
0.1 / 0.3 / 1 / 3 / 10 / 30 ms, releases 0.1 / 0.3 / 0.6 / 1.2 s on the panel (the DSP
takes any value in range). **Auto** is the two-capacitor programme-dependent release: a
fast stage releasing over 0.1 s and a slow stage that charges toward it over 0.4 s while
the reduction lasts and lets go over 1.6 s, holding 60 % of what it has — the reduction is
the larger of the two, so a lone hit comes back at 0.1 s and sustained work at the slow
speed (the same structure as Comprimo's dual release, so the family stays one).

**Opto — a T4-style photocell, feedback.** The cell is the shunt arm of a divider at the
input, so the gain is `1 / (1 + G)` with `G` the photoresistor's conductance. The
electroluminescent panel is driven from the **output** (the classic leveller's
topology): its light follows the output's mean square over 5 ms, and the conductance it
settles at is `c·(u − u₀)^p` (`u` the output over the threshold, the panel lighting at
`u₀`). Because the drive is the output, the static curve is explicit backwards — the
input is `y·(1 + G(y))` — and the local ratio is `1 + p·u/(u − u₀)·G/(1 + G)`: very soft
at first, tending to `1 + p`. **Compress**: `p = 2`, `u₀ = 0.5` (lighting 6 dB under the
threshold) — about 3 : 1 at depth. **Limit**: `p = 8`, `u₀ = 0.25` — about 10 : 1 at
depth (9.2 : 1 at 20 dB over the threshold, 9.9 : 1 at 30; the first pass's `p = 7` gave
8.7). The
threshold is where the cell gives 1 dB. The cell's time: the conductance rises over
50 ms (which, inside the feedback loop, is the published ~10 ms attack measured on the
reduction), falls first over 55 ms and then over a slow second stage (charging over
0.3 s, releasing over 3 s, holding 25 %) — the photocell's memory: the longer and harder
it was lit, the longer the tail. The panel's light state, the cell and the gain it holds
are one sample behind the output they light (stable: the loop gain at 50 ms is far below
one).

**Vari-mu — a remote-cutoff valve, feedback, solved at zero delay.** A valve whose gain
falls exponentially with its grid bias, the bias the rectified output over the threshold,
has the local ratio `1 + K·a·u` (`K = ln 10 / 20`), which written with its own reduction
is `R = R₀ + K·GR`: the ratio rises linearly with the reduction. The law, in dB of the
output `o` over the threshold, is `GR(o) = (R₀ − 1)/s·(e^{s·o} − 1)` until the ratio
reaches its ceiling, then a line of slope `R_max − 1`; the output first passes the
rectifier's soft knee. **Compress**: the exponential valve itself — `R₀ = 1.5`, `s = K`,
a 4 : 1 ceiling, a 6 dB knee. **Limit**: `R₀ = 4`, `s` set so the ratio reaches 20 : 1 at
12 dB of reduction (the published figures for the classic variable-mu limiter's two
modes: "compress starts at 1.5 : 1", "limit starts at 4 : 1 and moves to 20 : 1 when
limiting over 12 dB"), a 2 dB knee. The six time positions are the published table of the
classic six-position valve limiter — 0.2 ms / 0.3 s, 0.2 / 0.8, 0.4 / 2, 0.8 / 5, and two
programme-dependent ones (0.2 ms with 2 s for a peak and 10 s for many; 0.4 ms with 0.3 s
for a peak and 10 s for sustained programme; we hold 15 s for the published 10 – 25 s
pair). The loop is **solved at zero delay through its equilibrium**: the charge `c` (the
reduction, dB) at which the output it leaves asks for exactly `c`, `c = GR(knee(x − c))`
— the static curve — is tabulated once when the unit is made (257 points over the knee
and the rising ratio, solved by bisection to the float; past the ceiling the law is a
line and the curve is closed), interpolated per sample (within 5·10⁻⁴ dB of the solve),
and the charge moves toward it at the table's attack and release (smooth branching, as
the VCA). So the measured reduction rises and falls at the stated constants at every level
and either character, and with no sample of delay in the loop the limit's 20 : 1 at
0.2 ms cannot ring. *(The first pass moved the charge toward the output's own demand at
the rectifier's rate, Newton-solved each sample: that is the physical loop, and it reaches
its equilibrium about `R` times faster than the rectifier's constant, `R` the local ratio —
0.06 ms for the table's 0.2, and a partial release in 184 ms for 300. No published
measurement of the classic unit's closed-loop attack was found to say whether the table's
figures are the rectifier's or the unit's, so the second pass holds the measured
reduction to the table.)*

**The valve colour** (vari-mu only, a control, default 0): a balanced push-pull stage
cancels its even orders, so the colour is odd — the cubic soft clipper `u − u³/3`, its
headroom set so a full-scale sine at full depth carries 1 % third harmonic — and its depth
rises with the reduction (30 % at rest, 100 % at 20 dB: a valve biased further down its
curve bends more). It runs **at 2×, on the residual only**: the input is interpolated by a
polyphase IIR half-band (two chains of first-order allpasses in `z⁻²`; Valenzuela and
Constantinides, IEE Proc. G 130:6, 1983 — the six coefficients computed from their
elliptic design and checked against an independent evaluation), shaped, and only
`f(u) − u` is decimated by the same half-band and added to the untouched dry sample. So
the dry path is never filtered or delayed (latency 0); only the small residual carries the
half-band's few samples of group delay.

### The null contract and how the colour is handled

- **Mix 0** hands every sample through untouched, to the bit (the needle still moves).
- **A threshold above the signal with no make-up** touches no sample: every law gives
  exactly 0 dB below its knee (the opto's panel is dark below `u₀`, the vari-mu's
  rectifier gives nothing below its knee), a gain of exactly 1 is skipped, and a wet
  sample equal to its dry one is the dry one. The parameter defaults are that null: the
  threshold at its top (+24 dBFS), no make-up, no colour.
- **The valve colour colours by design**, so it is a control of its own with the default
  0: at 0 it is skipped (its filters emptied); above 0 the vari-mu touches the signal even
  when it does not compress, and nothing else does. The family's rule — a control at its
  default touches nothing — holds through it; the starting preset (Phase B) will set it.

## Measured

Every figure below is from the tests (`cargo test -p cantus-dsp --lib vinculum -- --nocapture`).

| Target | Measured | |
|---|---|---|
| VCA static curve = Giannoulis eq. 4 (ratios 2, 4, 10; −40 … +6 dB) | within 0.01 dB (DC input, settled) | met |
| VCA attack and release = their time constants | within 2 % + 1 sample (0.1 – 30 ms; 0.1 – 1.2 s) | met |
| VCA Auto: a lone hit at the fast speed, sustained work slower | to 37 %: after 20 ms 0.100 s, after 1 s 0.76 s, after 3 s 0.89 s | met |
| Auto on a drum loop (Pulsus, 100 BPM, ratio 4, attack 10 ms) | swing (σ of the reduction) 1.21 dB vs 1.64 at 0.1 s and 1.05 at 1.2 s; held just before each downbeat 1.26 dB vs 0.40 at 0.1 s and 4.32 at 1.2 s; mean 1.93 dB | met: less pumping than 0.1 s, far more recovery than 1.2 s |
| Opto static curve = the cell's law solved independently (f64, bisection) | within 0.02 dB | met |
| Opto ratio: Compress → 3 : 1, Limit → about 10 : 1 | Compress 1.35 at the threshold, 2.23, 2.75, 2.97 (30 dB over; unchanged); Limit 1.65, 6.94, 9.23, **9.93** (first pass 1.59, 6.05, 8.07, 8.70) | met (second pass: `p` 7 → 8) |
| Opto attack: published ~10 ms | 8.6 ms to 63 % of the reduction | met |
| Opto release: published ~60 ms for the first half | 57 ms (after 0.1, 1 and 10 s) | met |
| Opto full release: published 0.5 – 5 s, longer after more reduction | to 10 % of the reduction: 0.96 s after 0.1 s, 4.13 s after 1 s, 4.24 s after 10 s | met; the memory is nearly full after 1 s (the T4 is said to keep building over longer) |
| Opto Limit's times (16 dB over, 1 s) | half release 64 ms (first pass 63), to 10 % 5.01 s (4.93); attack to 63 % **0.9 ms** (first pass 1.1) | release met; the attack is far under the published ~10 ms average — see Left |
| Vari-mu static curve = the valve's law solved independently | within 0.02 dB (Compress, Limit) | met |
| Vari-mu ratio: Compress from 1.5 : 1, rising | 1.48 at 1 dB, 1.85 at 3, 2.19 at 6, 2.88 at 12, 3.80 at 20, 4.00 at 30 | met |
| Vari-mu ratio: Limit 4 : 1 → 20 : 1 at 12 dB | 3.28 at 1 dB (in the knee), 6.58 at 3, 11.9 at 6, **19.98 at 12**, 20.0 beyond | met |
| Vari-mu releases 0.3 / 0.8 / 2 / 5 s | 300 / 800 / 1997 / 4992 ms | met |
| Vari-mu attacks 0.2 / 0.2 / 0.4 / 0.8 / 0.2 / 0.4 ms | 0.188 / 0.188 / 0.396 / 0.792 / 0.188 / 0.396 ms to 63 % of the reduction (on the sample grid: 9 / 19 / 38 samples), the same at 3.2 and 9.8 dB (Compress) and 6.9 and 18.3 dB (Limit); first pass 0.06 / 0.06 / 0.12 / 0.25 at 9.8 dB | met (second pass) |
| Vari-mu release to a quieter level (20 → 8 dB over) | Compress 299 / 796 / 1988 / 4955 ms, Limit 299 / 796 / 1989 / 4949, for 300 / 800 / 2000 / 5000 (first pass: 184 ms for 300, Compress) | met (second pass) |
| Vari-mu equilibrium table = the law's solve | worst 2.5·10⁻⁴ dB (Compress), 5.2·10⁻⁴ dB (Limit), exactly 0 under the knee | met |
| Vari-mu positions 5 and 6: a peak fast, sustained programme slow | 5: 2.00 s / 8.56 s; 6: 0.30 s / 9.98 s (to 37 %) | met |
| The colour: 1 % third at full depth, odd only | third −50.5 dB at the rest depth 0.3 (stated −50.5), second < −110 dB | met |
| Aliasing of the colour (0 dBFS sine, full depth, the third's alias) | naive −40 dB everywhere; at 2×: 9 kHz → 21 kHz −95 dB, 10 kHz −127, 11 kHz −139, 13 kHz −125, 15 kHz −140 | met (9 kHz's third falls in the half-band's transition) |
| Limit at 0.2 ms does not ring | settled output steady to 1e-5 | met |
| Latency 0 | `latency() == 0`; a click stays on its sample | met |
| Block size 1 / 37 / 64 / 512 | bit-identical, every model and switch | met |
| Exact rest | silence out within a second (the colour's half-band tail), then at rest and as new; the slowest positions rest 240 s after heavy music (position 6: 15 s held) | met |
| No allocation after construction | 0 allocations over every model and switch | met |

**CPU**, stereo, 48 kHz, release build, best of 3, 10 s of programme
(`cargo test --release -p cantus-dsp --lib cpu_per_model -- --ignored --nocapture`), **with
the machine loaded by other builders (six cargo/rustc processes running)**: VCA 0.18 % of
a core, VCA Auto 0.20 %, Opto 0.14 %, Vari-mu 0.22 % without colour, 0.36 % with it,
Vari-mu Limit with colour 0.31 %. **Second pass** (the same test, five to seven other
cargo/rustc processes running, two runs): VCA 0.17 %, VCA Auto 0.20 – 0.23 %, Opto
0.14 – 0.16 %, Vari-mu 0.20 – 0.23 % without colour, 0.34 – 0.35 % with it, Limit with
colour 0.32 – 0.37 % — the table costs less than the first pass's Newton step (a Newton
solve of the equilibrium itself each sample measured 0.34 – 0.50 % / 0.51 – 0.69 % /
0.61 – 0.74 %, which is why it is tabulated). To be measured again on a quiet machine.

## Tests and what each holds

`vinculum_tests.rs`: `mix_zero_touches_no_sample`, `a_threshold_above_the_signal_touches_no_sample`
(and the defaults), `the_colour_is_the_only_thing_that_touches_a_quiet_signal`,
`block_size_does_not_change_a_bit`, `latency_is_zero_and_a_click_stays_where_it_is`,
`silence_comes_to_an_exact_rest_and_rest_is_as_new` (the rest twin), `the_slowest_positions_rest_too`,
`the_vca_follows_the_published_gain_computer`, `the_opto_settles_on_its_cell_law`,
`the_opto_ratios_are_the_cells`, `the_vari_mu_settles_on_its_valve_law`,
`the_vari_mu_ratio_rises_with_level`, `the_vari_mu_table_is_its_equilibrium`,
`the_limit_does_not_ring_at_its_fastest`,
`the_valve_is_odd_and_as_strong_as_stated`, `the_half_band_is_the_published_design`,
`the_meter_reads_each_model`. `vinculum_tests_time.rs`: the VCA's time constants, Auto,
the opto's published times (and the Limit's, `the_opto_limit_keeps_the_cells_times`), the
vari-mu's positions, its attacks at two levels and both characters
(`the_vari_mu_attacks_are_their_time_constants`), its release to a quieter level
(`the_vari_mu_lets_go_of_a_quieter_level_at_its_release`) and its programme-dependent two.
`vinculum_measure.rs`: Auto on a drum loop, the colour's aliasing, CPU (ignored).
`tests/vinculum_alloc.rs`: no allocation. 27 + 1 pass (the whole `cantus-dsp` suite: 947); clippy `-p cantus-dsp --all-targets`
clean; `cargo fmt -p cantus-dsp`.

## Sounds

`C:\projects\daw-fx\compartir\vinculum\sound\` (README and `table.md` there): a drum bus
(Pulsus) and a mix bus (the CC0 DI guitar — "Guitar: Carbonthief, CC0" — with Pulsus drums
and a Gravis bass, the owner's Am – F – C – G at 90 BPM), each model gentle, slammed and
slammed in parallel, and the VCA's 0.1 s, 1.2 s and Auto releases side by side; dry twins;
level-matched by RMS. **Second pass**: rendered again into a temporary folder and compared
byte for byte; the ten whose output changed were replaced one by one (with `table.md`):
`05-drums-opto-slammed`, `06-drums-opto-parallel`, `07-drums-varimu-gentle`,
`08-drums-varimu-slammed`, `09-drums-varimu-parallel`, `25-mix-opto-slammed`,
`26-mix-opto-parallel`, `27-mix-varimu-gentle`, `28-mix-varimu-slammed`,
`29-mix-varimu-parallel`. The dry twins and every VCA and Opto Compress file are
bit-identical to the first pass. The drum bus's slammed vari-mu now lets more of each
hit's first fraction of a millisecond through (its attack is the table's 0.2 ms, not
0.06), so its level match was lowered 2.2 dB to keep the peak under 0 dBFS.

## Verified only in emulation

Everything: the models are checked against their own laws solved independently and against
published figures (time constants, ratios), not against the hardware or a capture of it.
Nobody has listened yet. The published figures are the manufacturers' and their
reproductions' (manuals, spec sheets); no measurement of a real unit was available.

## Left (Phase B and the owner)

- The owner's ears on the set above (which model, Auto, the colour's amount).
- The contract: an `EffectKind` (proposed `BusCompressor`), its parameters (model,
  threshold, ratio, attack, release, Auto, Compress/Limit, time position, side-chain
  high-pass, mix, make-up, colour; keep `out`), the schema number, the ADR number.
- The Opto Limit's attack: 0.9 ms to 63 % at 16 dB over the threshold (4 ms at 8 dB
  over, 0.2 ms at 26), against the published "~10 ms" (an average; Compress measures
  18.7 / 8.6 / 3.6 ms at the same three levels; the 8 and 26 dB figures are from an f64
  re-run of the same loop, which gives the test's 16 dB figures to 0.05 ms). The panel's eighth power lights the cell
  far past where it settles, so the cell's 50 ms rate closes an enormous gap quickly. It
  was 1.1 ms in the first pass (`p = 7`) and was not one of the two misses; closing it (a
  bounded conductance — the photoresistor's minimum resistance — or a drive-dependent
  rate) changes how the Limit sounds on hits, so it waits for the owner's ear.
- An external key (another strip's signal, as Comprimo has) — the classic bus compressor
  has one; the DSP takes it in one line (the key level), the engine wiring is Phase B.
- CPU measured again on a quiet machine.

## Phase B (3), to the finish (2026-11-17): sheet v1 approved

- **The owner's note on v1**: the rack rails' column of small square holes (with their cage
  nuts) read as the "vents" ornament he had removed from Lapsus and Vetus. The rails are
  plain brushed steel now (a few faint lines down each, the lit fold and its shadow); the
  unit's ears keep their slots and screws (`vinculum_art::rack`; `racks::CAGE` gone).
  Re-shot (all 23 of the sheet's shots, since the rails are in every one) in
  `compartir\vinculum\design\v2-step3\`, looked at (Slate English, Paper Spanish vari-mu).
- **The lamps are controls** (`vinculum_face::{lamps, lamp_edit}`, laid out before they are
  drawn so the pointer and the picture agree): *External key* opens the key's list under it
  (its own input or any strip — the same command as the Side chain card's field; in a
  plug-in window it throws the sidechain switch), *Auto release* lights Auto and a second
  click gives back the release it had (0.3 s when it remembers none), *Limit* throws the
  character; the colour's lamp only shows. Each change one undo step. New tips in both
  languages.
- **Pointer tests** (`tests/vinculum_drag.rs`, four, real pointer events through the window
  the app draws): the ratio's 10:1 chosen, the attack's knob stepped round (30 ms → 0.1 ms),
  the release's 0.6 chosen, the model's Vari-mu (the panel becomes the valve's), its fourth
  time position, the character's bat — six clicks undone one step each back to Glue; Auto's
  lamp lit and giving back 1.2 s; the limit lamp on the opto; the key's lamp opening its
  list, a strip chosen (one undo step), a click elsewhere closing it.
- **Presets** fourteen (`presets/effects/buscompressor.toml`): VCA — Glue, Mix bus glue,
  Punchy drum bus, Drum smash, Parallel drums, Bus pump, Bus catch; Opto — Opto vocal bus,
  Smooth leveller, Opto vocal limit; Vari-mu — Valve mastering glue, Valve warmth, Valve
  limiter, Lo-fi squash. Each model compressing and limiting (a test holds it). Fitted with
  `preset_tool fit` (dev profile, the dev build is optimised here), the level carried by the
  make-up first and the output last: −2 … +2.8 dB; Glue −1.5 dB, so the starting preset in
  `starting.rs` and the editor's demo moved too. The opto vocal presets' thresholds went
  10 dB lower after the first fit showed 0.2 dB of reduction on the vocal source (a single
  lead, quieter than the band). `preset_audit`'s Vinculum line removed (floor 12).
- **Export** `Cantus Vinculum` (CLAP and VST3; `desc.rs`, `build.rs`, the `vinculum` feature,
  xtask; VST3 class id by its rule): the external key through the sidechain input in both
  (`clap::vinculum_s_external_key_presses_the_input_only_when_asked`,
  `vst3::vinculum_s_key_is_the_vst3_sidechain_bus`: bit-identical to the DSP keyed by the
  same sound, the unkeyed quiet input untouched to the bit, keyed energy under a quarter);
  each model bit-identical through CLAP (`vinculum_s_models_match_the_dsp`); the Direct path
  (its DSP and the output's glide); 16 ids appended and blessed. The plug-ins' text now
  carries wave A's and wave B's six locale files (`pane_words` iterates every kind; it was red
  in wave B before).
- **AI**: the effect words name Vinculum's models, Compress/Limit, the sidechain key and its
  high-pass, parallel and what it is for; aliases `vari_mu` and `opto`; MCP golden
  regenerated on purpose (the two aliases twice, the description twice).
- **Site** card (EFFECTS 35, presets 14, big 1800 × 1209, thumb 640 × 430 — pictures and
  sound excerpts to make at the integration).
- **Tabula: a pedal.** A compressor pedal is natural. Face: model, threshold, make-up, mix
  (standard, the needle, "V"). On a board it compresses the board's own signal; a board
  takes a key when a pedal does and passes it on, so a keyed board reaches its detector
  exactly as an insert's — the same generic path every keyed pedal (Comprimo's, Caelum's)
  uses, so "no key on a board" was not kept: unkeyed it hears its own input only.
  `fx_vinculum::as_a_pedal_it_compresses_its_own_input_and_hears_the_board_s_key` (unkeyed
  board = the insert alone to the bit; keyed board = the insert keyed, to the bit, offline and
  live); `fx_contract::a_board_of_one_is_the_unit` now covers it too.
- **Docs**: ADR 0142 phase B, consequences; `docs/plugins/vinculum.md`; the manual EN/ES
  (*Vinculum, the bus compressor* before Aula).

### Measured (the e2e through `CantusApp::assemble`, `tests/vinculum_e2e.rs`)

A band (a chord and a line, each strip −18 dB) routed into a bus strip, a bass line straight
to the master; Vinculum inserted on the bus as the picker does (Glue, latency 0). The band
peaks −23.8 dBFS, the bass −8.3. The band alone is the master less the bass alone.

- The threshold 20 dB under the band's peak at 10:1, no make-up: the bus **7.7 dB** quieter.
- Live in blocks of 256/333/512/17/1024 = the offline render, to the bit.
- The threshold 6 dB over the band: on its own key the band passes **untouched to the bit**;
  the bass strip routed as the slot's key presses it **6.0 dB** while the bass plays (1.1 …
  2.9 s) and **0.00 dB** before.
- The side chain's high-pass at 500 Hz: **4.7 dB** (the bass's low end out of the key).
- An edit (10:1 → 2:1) heard and undone to the bit; every key to its default (the key still
  routed): the dry master to the bit.

### Shared files touched (marked `fxb:vinculum`)

Core: `fx/tabula.rs` (`pedal_allowed` and its test), `starting.rs`. Engine: tests
`fx_vinculum.rs` (the pedal), `fx_loquax.rs:161` (clippy's `is_multiple_of`, as wave A does).
UI: `views/fx/tabula_faces.rs`, `kit/skin/racks.rs`, tests `preset_audit.rs`, golden
`mcp-tools-list.json`. AI: `tools_mixer.rs`, `tools_args.rs`. Plug-ins: `Cargo.toml`,
`build.rs`, `src/desc.rs`, `src/text.rs`, tests `common/mod.rs`, `clap.rs`, `vst3.rs`,
`golden/ids.txt`. xtask: `main.rs`. Web: `units_data.rs`. Docs: the manual EN/ES.

### Verified only in emulation

- The exported plug-in only in our own CLAP/VST3 harness, not in a third-party DAW.
- The presets have not been heard by the owner; their levels are the fit's loudness match,
  their character our reading of what each preset is for.
- The opto Limit's attack (0.9 ms against ~10 ms published) stays until the owner listens.

### Suites (Windows, 2026-11-17, one crate at a time, clippy `-D warnings --all-targets` on each)

cantus-core 810, cantus-dsp 1 127, cantus-engine 1 309, cantus-project 194, cantus-plugins 82,
cantus-ai 50 — all green, clippy clean on each (the plug-ins' clippy, which builds the UI,
caught two `if`s inside the lamps' `match` that clippy wants as guards; fixed).
