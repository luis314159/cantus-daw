# ADR 0140 — Lenis, the resonance suppressor

**Status:** proposed (fx side round wave B) ·
**Date:** 2026-11-17 · **Schema:** 75 ·
**Relates to:** ADR 0005 (the audio thread), ADR 0018 (null defaults), ADR 0031 §5.1 (at
rest), ADR 0071 (effects that sound on insert), delay compensation (the strips' latency)

## Context

Nothing in Cantus found and tamed resonances moment by moment (the function of oeksound's
soothe2 and the dynamic-EQ resonance modes — function only, never their algorithm). The DSP
was built first (`docs/round-fx/lenis.md`): a low-delay asymmetric STFT (Mauler & Martin),
a per-bin detector against the spectrum's own envelope (a greatest-of CFAR envelope under
the prominent peaks, harmonic awareness under 800 Hz, an onset lift), the cut a share of
each bump; 10.6 ms of latency at Standard; CPU 3.3 % of a core linked.

## Decision

A new effect kind, `EffectKind::ResonanceSuppressor`, model `cantus_core::fx::lenis`,
engine unit `cantus_engine::fx::lenis`, DSP `cantus_dsp::lenis`.

- **Parameters** (37 keys): depth, sharpness, selectivity, attack, release, the focus's
  limits, six focus bands (on, frequency, amount, width), channels (Linked, Independent,
  Mid and side), curve (Soft, Hard), quality (Fast, Standard, Precise), delta, mix, out.
  **The default is a null**: depth 0 — the output is the input delayed by the latency, to
  the bit (the DSP's dry ring), and the engine's compensation holds every other path back
  by as much (tested through the engine at each quality).
- **A quality change resets the unit and reports a new latency.** The three frames have
  different latencies (255 / 511 / 1 023 samples at 48 kHz); two outputs delayed
  differently cannot crossfade without a comb, and the engine must re-align anyway. The DSP
  takes the change at its next hop, starting again from silence (a gap of one latency,
  5–21 ms); the engine reads `latency()` each pass, so its compensation follows.
- **The engine's reset is a new unit**: `Lenis::restart` clears the audio state and puts the
  frame grid's clock at 0 (the engine skips a reset unit to where the stream is), so a reset
  twin and a new one agree to the bit. A quiet detector now holds the state silence leaves
  (calm, no lift, no onset), so a unit at rest is as good as new.
- **Delta** hears what is taken away (`dry − output`, to the bit at 0 dB out).
- **Telemetry**: the meter's 128 display points travel in five interleaved pages of 26 (the
  page is the frame count's); the editor keeps the whole picture.
- **Starting preset** (ADR 0071): *Gentle* — depth 50 %, Soft, the focus from 150 Hz.

## Phase B (2026-11-17): the editor, presets, export (design sheet v1 approved as shown)

- **The window as approved** (`compartir/lenis/verdict.md`): the bench analyser, its screen,
  the Depth dial, the readouts, the push keys and the cards. Phase B holds its controls on
  real pointer events (`tests/lenis_drag.rs`): the focus's two gates dragged along the
  screen, each of the six band nodes dragged in frequency and amount (a band dragged while
  off is switched on in the same step, never cleared), the Depth dial turned, the push keys
  pressed — each gesture one step of undo, a pressed key pressed again none.
- **The Focus band card's lamp offered its menu only on the frame it was clicked**, so
  automation and MIDI learn never reached the band's switch (`learn_everywhere` flagged the
  six `b{n}on` keys); the lamp's offer is made every frame now. The card shows one band at a
  time: the other five are excused by the band shown, and
  `every_lenis_band_offers_its_switch` chooses each band in turn.
- **Presets** fourteen (`presets/effects/resonancesuppressor.toml`): *Gentle* (the starting
  preset), *Harsh vocal*, *Sibilance tamer*, *Guitar fizz*, *Piano ring*, *Mix bus gentle*,
  *Master polish*, *Drum room ring*, *Acoustic boom*, *Podcast voice*, *Snare ring*, *Wide
  air*, *Strings and brass*, *Surgical*; every quality, both curves and every channel mode
  among them (a test holds it); never the delta. Roles vocal, drums, bus, master, creative.
  Fitted by `preset_tool fit` with `out` (the starting preset too, in `starting.rs` and the
  editor's demo).
- **Export** `Cantus Lenis` (CLAP and VST3; `desc.rs`, the `lenis` feature, xtask; the VST3
  class id by its rule): the engine's unit through the generic processor, bit-identical to
  the DSP (the plug-ins' Direct path names Lenis's DSP). **Its latency is reported per
  quality** (255 / 511 / 1 023 at 48 kHz) before activation in both standards; **a quality
  changed while it plays re-reports it**: CLAP asks the host for a restart once and tells
  `clap_host_latency::changed` on the next activation; VST3 calls
  `restartComponent(kLatencyChanged)` from the UI thread's tick and `getLatencySamples`
  gives the new frame's. The delta through the plug-in is the delayed input less the
  output, to the bit. Ids appended and blessed.
- **AI**: the effect words say what Lenis is and is for (rings, whistles, harshness, the
  focus, the curve, the qualities and their latency, the delta; harsh vocals, sibilance,
  guitar fizz, ringing drums and rooms, boomy acoustics, a mix or master polish) and two more
  aliases (`resonance_tamer`, `deharsh`) — no product's name; the MCP tools golden
  regenerated on purpose.
- **A pedal on Tabula** (`pedal_allowed`): a guitar's rig makes the rings Lenis tames (its
  listening set was a guitar through two fuzz pedals and a honky cabinet). Its face: depth,
  sharpness, selectivity, focus from (standard, the curve motif, "L", brushed). The board
  already keeps a latent pedal's latency (Saltus's, Arca's) and follows its change as it
  follows Saltus's mode, so nothing in Tabula changed.

## Consequences

- A latent insert by default (10.6 ms), like any linear-phase unit; a live monitoring path
  bypasses it hard where low-latency monitoring asks (ADR 0064 §4).
- One more latent insert whose latency moves with a parameter; the engine's compensation,
  Tabula's board and both plug-in standards follow it, each tested.
- A card lamp's offer (`controls::card_lamp_offer`) must be made every frame, not only on
  the click: Lenis's card was the one caller that short-circuited it.
- Lenis joins Tabula: `a_board_of_one_is_the_unit` covers it with every other pedal.
- The editor keeps where its grips stood last frame (`State::{node, gate, dial,
  push_key}`), so tests point at what the user sees.

## What the tests hold

`cantus-dsp`: 22 + `a_clone_carries_on_exactly_as_the_original`, `a_restarted_unit_is_a_new_one`;
`cantus-engine/tests/fx_lenis.rs` (the dry mix delayed by exactly the latency at each
quality, a quality change reporting its latency, the starting preset at every block size
and live, the delta, forks), `fx_contract` (split and at rest), `rt_memory::lenis_never_allocates`.

Phase B: `cantus-ui/tests/lenis_drag.rs` (the gates, each band's node switched on and moved
in one step, the Depth dial, the push keys — each gesture one undo step);
`lenis_e2e.rs` through `CantusApp::assemble` (the defaults the dry master delayed by 511,
1 023 and 255 samples to the bit with a twin strip held back by the compensation; a
+15 dB 3 kHz ring cut 9.4 dB with 500 Hz and 10 kHz unchanged to 0.000 dB; the delta the
ring; an edit undone to the bit; live = offline; the presets' qualities, curves and channel
modes); `learn_everywhere::every_lenis_band_offers_its_switch`;
`cantus-engine/tests/fx_lenis.rs::as_a_pedal_it_is_the_insert_alone_at_each_quality`;
`cantus-plugins`: `clap::lenis_reports_its_latency_per_quality_and_a_change_re_reports_it`,
`clap::lenis_s_delta_is_what_it_takes_away`, `vst3::lenis_s_latency_follows_its_quality_through_vst3`,
the family's bit-identity, state and automation tests, the ids golden.
