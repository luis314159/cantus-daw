# Lenis — resonance suppressor

*Effect kind `ResonanceSuppressor`. References for function: the "soothe-class" resonance
suppressors and the dynamic-EQ resonance modes of other processors — read about and
listened to, nothing copied; the algorithm is our own, from the literature cited in
`docs/round-fx/lenis.md` (ADR 0143, provisional number).*

*Lenis* is Latin for gentle. It finds, frame by frame, the frequencies that stand out of a
sound's own spectrum — a cabinet's ring, a vocal's whistle, a room's boxy build-up — and
takes a share of each away, leaving the notes and the tone:

- **The analysis**: a low-delay asymmetric STFT (Mauler and Martin's window pair; a
  1 024-point frame and a 256-sample hop at Standard), so the latency is a quarter of what a
  symmetric frame of the same resolution costs.
- **The detector**: each bin is compared with the outline of the spectrum around it — an
  upper envelope drawn through the prominent peaks (so a note's harmonics are not
  resonances), against a greatest-of envelope whose sides move along a slope (so a peak on a
  steep roll-off is still found and a cliff's knee is not); under 800 Hz it knows a note's
  partials. Two time scales: a slow average that confirms, a fast one that measures; an
  onset lift that follows a ring in 8 ms with no added latency.
- **The cut**: a share of each bump (Depth: 100 % to the outline, 50 % half way), Soft or
  Hard, shaped by Sharpness (narrow rings only, or broad build-ups too) and Selectivity (how
  far a peak must stand out), with attack and release; the focus between two limits (a
  third of an octave's fade) and six weighted bands; Linked, Independent or Mid and side;
  the delta to hear what is removed; mix and output.

**The null**: at depth 0 (the default) the output is the input delayed by the latency, to
the bit — the frames nothing touches are the dry itself. **The latency** is the quality's
frame: 255 / 511 / 1 023 samples at 48 kHz (Fast 5.3 ms, Standard 10.6 ms, Precise
21.3 ms). A quality change starts the analysis again (a gap of one latency: two outputs
delayed differently cannot crossfade without a comb) and reports the new latency, which
Cantus's delay compensation and a host's follow. CPU 3.3 % of a core linked at Standard,
5.7 % independent or mid and side.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/lenis.rs` (37 keys) |
| DSP | `crates/cantus-dsp/src/lenis.rs` (the unit), `lenis_stft.rs` (the window pair), `lenis_detect.rs` (the detector, the curve, the focus); tests `lenis_tests.rs`, `lenis_tests_measure.rs`, `tests/lenis_alloc.rs` |
| Engine glue | `crates/cantus-engine/src/fx/lenis.rs` (settings, the latency, the telemetry's five pages of the 128-point picture) |
| Editor | `crates/cantus-ui/src/views/fx/lenis.rs` (the window, the cards), `lenis_screen.rs` (the screen), `lenis_focus.rs` (the gates and nodes on the pointer), `lenis_panel.rs` (the dial, the readouts, the push keys), `lenis_picture.rs` (the picture from the pages), `lenis_art.rs` (the bench analyser) |
| Face | `crates/cantus-ui/src/kit/skin/side_fxb.rs` (Laboratory, Ivory, Graphite) |
| Strings | `crates/cantus-ui/locales/fx/lenis.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/resonancesuppressor.toml` (fourteen); starts on *Gentle* |
| Tests | `crates/cantus-engine/tests/fx_lenis.rs`, `rt_memory`, `fx_contract`; `crates/cantus-ui/tests/lenis_e2e.rs`, `lenis_drag.rs`, `fxb_whole.rs`, `learn_everywhere.rs`; the CLAP/VST3 suites (`clap.rs`, `vst3.rs`: the latency per quality and its change, the delta) |
| Exported | `Cantus Lenis` (CLAP and VST3), its latency reported per quality |

## The window

A bench analyser on its black epoxy bench, its enamelled front panel between two carrying
handles, over a row of cards:

- **The screen**, dark in every theme: the input's spectrum as a fine phosphor line, the cut
  hanging under it as a veil down to the output's line, and along the top a bar per
  frequency of the cut (0 … 24 dB). With *What is removed* on, the removed part's own
  spectrum is the picture. At the foot the **focus**: the curve (left alone, as set, twice),
  the two gates **From** and **To** with their tabs (drag them), the six numbered bands (drag
  a node left and right for its frequency, up and down for its amount; the wheel sets its
  width; a double-click switches it; a band dragged while off is switched on by the same
  step). Each drag is one step of undo; each grip's menu offers automation and MIDI learn.
- **The panel**: the **Depth** dial (drag up or down, shift for fine, double-click for
  none), the smoked readouts of the most cut now (held a second) and the latency, and the
  push keys — Quality, Curve, *Listen · What is removed*, Channels — a click pushes one in,
  each key's menu offers automation and MIDI learn.
- **The foot**: the maker's badge, the model, the input and output sockets with their
  signal lamps.
- **The cards**: Detection (Sharpness, Selectivity), Time (Attack, Release), Focus (From,
  To), Focus band (the band chosen on its numbers or on the screen: its lamp, frequency,
  amount, width), Output (Mix, Output).

## Presets

Fourteen, fitted to the dry's loudness (`preset_tool fit`): *Gentle* (the starting preset),
*Harsh vocal*, *Sibilance tamer* (Fast, Hard), *Guitar fizz* (Hard), *Piano ring*
(Precise), *Mix bus gentle* (80 % wet), *Master polish* (Precise), *Drum room ring*
(Precise, Hard), *Acoustic boom* (Precise, 80 – 600 Hz), *Podcast voice*, *Snare ring*
(Fast, Hard), *Wide air* (Mid and side), *Strings and brass* (Independent), *Surgical*
(everything to the outline). Every quality, both curves and every channel mode among them.
Roles: vocal, drums, bus, master, creative.

## Exported

`Cantus Lenis` in CLAP and VST3, bit-identical to the DSP. Its latency is the quality's
frame, reported to the host before activation (`clap_plugin_latency`,
`IAudioProcessor::getLatencySamples`); a quality changed while it plays asks a CLAP host for
a restart (`request_restart`, the latency then reported on activation with
`clap_host_latency::changed`) and calls a VST3 host's
`IComponentHandler::restartComponent(kLatencyChanged)` from the UI thread's tick
(`clap::lenis_reports_its_latency_per_quality_and_a_change_re_reports_it`,
`vst3::lenis_s_latency_follows_its_quality_through_vst3`). The delta through the plug-in is
the DSP's and the delayed input less the output, to the bit
(`clap::lenis_s_delta_is_what_it_takes_away`). Its ids are blessed in
`crates/cantus-plugins/tests/golden/ids.txt`.

## A pedal on Tabula

Lenis may be a pedal: a guitar's rig makes the rings it tames (a cabinet's peak, a fuzz's
fizz — its listening set was a guitar through two of Fragor's pedals and a honky cabinet).
Its face shows Depth, Sharpness, Selectivity and Focus from; the curve, the quality and the
bands stay on its editor's screen, opened from the pedal. The board keeps its latency as it
keeps Saltus's or Arca's: a board of one Lenis is Lenis inserted alone, to the bit, at each
quality, live as offline (`fx_lenis::as_a_pedal_it_is_the_insert_alone_at_each_quality`).

## Ours, better

- A note's harmonics are not resonances: a steady note's low partials untouched (0.00 dB)
  at a strong setting, an inharmonic ring among them still cut.
- A ring followed in 8 ms with no look-ahead (the onset's lift), and a latency a quarter of
  a symmetric frame's.
- The screen shows where it works and how much, at 60 Hz, and lets you place the focus on
  the picture itself.
- Exact rest: a silent Lenis costs nothing, and the null is the dry itself, to the bit.

## Left for the owner's ear

- The +0.76 dB transient overshoot on a tone between two bins at the fastest attack, and the
  pre-echo before a hard strike at −27.3 dB (`docs/round-fx/lenis.md`, the third pass).
- A note's first frames from digital silence can take a bin or two down for a few frames.
- The presets are fitted for level; their character is our reading of what each is for.
