
# ADR 0017 — The plug-in family: Aequalis, Comprimo, the recorder, and how they meet the host

**Status:** Accepted · **Date:** 2026-09-24 · **Extends:** ADR 0005 (the audio thread), ADR 0015 (sends, solo, EQ), ADR 0016 (docks and groups)

## Context

The user asked for effects "like FabFilter's Pro bundle", instruments of the same class,
a recorder inside the mix, and an interface worthy of a first-rank product — and for
research agents to study every Pro-bundle product, with the official screenshots, so that
ours would be *better*, not a copy. The eight reports are in `research/fabfilter/` (the
screenshots stay local, git-ignored: they are FabFilter's images). Their shared verdict:
FabFilter leads on **interaction**, not DSP; its displays are often read-only, its styles
opaque, its dynamics in percentages, its contrast low, and its undo local to the plug-in.
A native plug-in in the host can fix every one of those.

This ADR records the structural decisions of the first two plug-ins and the pieces they
share. Round 15 of STATUS has what was built and measured.

## Decisions

**An effect with an editor is a parameter model plus a window.** The parameter model stays
the generic `key → f32` (so the command bus, undo, the AI bridge and later automation treat
every effect alike), and `EffectKind::has_editor` says the inspector shows a live preview
and opens a window instead of a knob per parameter. The window is drawn by the UI crate;
nothing about it reaches the engine.

**A band's number never changes.** Aequalis addresses its 24 bands as `b{n}.{field}`; a
band removed frees its number and no other band is renumbered, so anything that names
`b3.gain` names the same band forever. The benchmark does the same and its users rely on it.

**Sparse parameters, exact undo.** An EQ has 337 parameters; a new one stores none, and
the file holds only what moved (`EffectKind::sparse`). That needed one new command,
`ClearEffectParam`: setting a parameter that was never stored has "clear it" as its
inverse, so undo returns the effect's map to exactly what it was — the undo fuzz checks it
with the EQ in its random walk.

**Filters matched to the analog curve.** The RBJ/bilinear designs cramp near Nyquist
(a 16 kHz bell is 4.3 dB out at 48 kHz). Aequalis designs every second-order section by
magnitude matching — analog poles mapped exactly, zeros solved to match DC, Nyquist and the
band frequency (after Vicanek) — except a high or tilt shelf above 0.35·fs, where the
measurement showed the pre-warped bilinear design tracks better. The test holds both
claims with numbers, against the same analog prototype. Cuts are Butterworth cascades,
6–96 dB/oct. The channel strip's own three-band EQ (ADR 0015) keeps its RBJ design for now:
changing it moves renders of existing projects, and that is a decision of its own.

**Glides and dynamics on a fixed sample grid.** Band parameters glide (no zipper when a dot
is dragged) and dynamic bands move, both every 16 samples counted from the start of the
stream — never from the start of a block — so the output is independent of the block size,
as every null test requires.

**A compressor's style is a detector, spelled out.** `Style::spec` names each style's
detector (peak or an RMS window), feedback share and release behaviour, and the editor
prints it. No black box.

**A sidechain is a route.** A slot can be keyed from another strip
(`EffectSlot::sidechain`), chosen inside the compressor with no send to set up. The key is
an edge of the processing graph like an output or a send: `Project::downstream` includes
it, so the key strip is processed first, a key that would close a loop is refused, and a
strip something is keyed from cannot be removed. The engine keeps the post-fader output of
strips that are keys in buffers outside the strip array (so one strip's effects can read
another's output while it is borrowed); a key strip that made no sound this segment is
silence, not "no key".

**One analyser tap, for the editor that is open.** The engine copies the analysed effect's
input and output (mono pairs) into a lock-free ring; the FFT runs on the UI thread. Only
one effect is analysed at a time — the one whose editor is open — so the cost is paid once.
A compressor's meter (input, output, gain reduction) travels in `Meters`, folded to the
maximum since the UI last read it so a transient between frames is not lost.

**The recorder records what leaves a fader.** `CaptureTarget` is the master (after its
fader, before the metronome, which now goes in after the master strip) or one strip after
its fader. The audio thread writes into a lock-free ring; the UI drains it every frame into
a writer thread that owns the WAV file. A strip that is skipped because it is silent still
records its silence, so a take has the length of the time it ran. The master take is
bit-identical to the output; a null test holds it.

**A house knob.** Every parameter that is not a fader or a bipolar bar is a `Knob`: drag
vertically (Shift fine), double-click reset, arrows when focused, right-click to type an
exact value, and an arc drawn *from the default*, so a moved control says so at rest. An
untouched knob issues no command — a test holds that, because the first version did.

**Screens without a screen.** `cantus_ui::raster` renders an egui frame on the CPU to PNG,
and `cargo run -p cantus-ui --example shoot -- <dir>` writes the mixer and each editor in
both themes. It is how the interface is reviewed on a machine whose display is locked, and
how "better" is shown rather than claimed.

## Consequences

- New: `EffectKind::{Eq, Compressor}`, `ClearEffectParam`, `SetEffectSidechain`,
  `EngineMessage::{SetCapture, SetAnalyser}`, `CaptureTarget`, `Meters::{eq_deltas, comp}`,
  `cantus_dsp::{aequalis, comprimo, spectrum}`, `cantus_ui::{takes, raster}`,
  `views::{eq_editor, comp_editor}`, `widgets::knob`; schema 12, fixture `v12.cantus`.
- Known and deliberate: the master's balance at centre multiplies by 0.99999994, not 1
  (`cos(π/4)·√2` in f32). Making it exact moves every existing render by one rounding step
  and re-records the golden hash; it is listed for a decision of its own.
- Not yet: lookahead and linear phase (both need plug-in delay compensation), spectral
  dynamics, EQ match and sketch, the limiter, reverb, multiband, de-esser, gate, and the
  instruments. The order is in STATUS round 15.
