# Vinculum — bus compressor

*Effect kind `BusCompressor`. References for function: the console bus compressors of SSL's
G series, the Teletronix LA-2A's T4 cell, the Fairchild 660/670 and Manley Variable-Mu, and
their plug-in recreations (Waves, UAD, FabFilter Pro-C 2's Bus style, Cytomic The Glue) —
read about and listened to, nothing copied (ADR 0139).*

*Vinculum* is Latin for a bond — what glues a mix. A bus compressor with the three
characters engineers reach for by name, each with the controls its panel has:

- **VCA** — a console's bus compressor, feed-forward: the gain computer of Giannoulis,
  Massberg and Reiss (JAES 2012) with a 3 dB knee; ratio 2, 4 or 10 : 1, attack 0.1 … 30 ms
  and release 0.1 … 1.2 s on stepped switches, and **Auto**: a two-stage programme-dependent
  release, so a lone hit lets go at 0.1 s and sustained work slowly — the mix breathes
  without pumping.
- **Opto** — a T4-style photocell lit by the output (feedback): very soft at first, about
  3 : 1 at depth in **Compress**, near 10 : 1 in **Limit**; the times are the classic
  leveller's published ones, measured on the reduction: about 10 ms attack (quicker the
  harder the cell is lit), half released in about 60 ms, the rest over 0.8 s after a burst
  to 10 – 13 s after a long, hard passage — the cell's **memory**: the longer and harder it
  worked, the slower its second stage; a cell that has rested lets go as a new one.
- **Vari-mu** — a remote-cutoff valve whose ratio rises as it works (1.5 : 1 to 4 : 1 in
  Compress, 4 : 1 to 20 : 1 at 12 dB in Limit), its loop solved at zero delay; the classic
  limiter's six time positions (two programme dependent); and an odd-harmonic **valve
  colour** at 2× that grows with the reduction.

**The key** is the bus itself, or any strip of the desk routed as the slot's sidechain —
the drums pressing the whole mix — through the side chain's **high-pass** (20 Hz is off) so
the low end does not pull it. **Mix** makes it parallel. **The null**: the threshold starts at
its top (+24 dBFS), with no make-up and no colour — every sample untouched, to the bit. No
latency; CPU ≤ 0.37 % of a core.

| Layer | File |
|---|---|
| Parameters | `crates/cantus-core/src/fx/vinculum.rs` (12 keys) |
| DSP | `crates/cantus-dsp/src/vinculum.rs` (the unit), `vinculum_law.rs` (the laws and the ballistics), `vinculum_valve.rs` (the colour at 2×); tests `vinculum_tests.rs`, `vinculum_tests_time.rs`, `vinculum_measure.rs`, `tests/vinculum_alloc.rs` |
| Engine glue | `crates/cantus-engine/src/fx/vinculum.rs` (settings, the key, `out` after the DSP, telemetry) |
| Editor | `crates/cantus-ui/src/views/fx/vinculum.rs` (the cards), `vinculum_display.rs` (the layout), `vinculum_face.rs` (the faceplate's names, lamps and switches, the meter bridge), `vinculum_screen.rs` (the curve and the history), `vinculum_meter.rs`, `vinculum_parts.rs` (the gain elements), `vinculum_art.rs` (the rack, the plate, the hardware) |
| Face | `crates/cantus-ui/src/kit/skin/side_fxb.rs` (Console, Broadcast, Steel), `kit/skin/racks.rs` |
| Strings | `crates/cantus-ui/locales/fx/vinculum.{en,es}.txt` |
| Presets | `crates/cantus-ui/presets/effects/buscompressor.toml` (fourteen); starts on *Glue* |
| Tests | `crates/cantus-engine/tests/fx_vinculum.rs`, `rt_memory`, `fx_contract`; `crates/cantus-ui/tests/vinculum_e2e.rs`, `vinculum_drag.rs`, `fxb_whole.rs`; the CLAP/VST3 suites (`clap.rs`, `vst3.rs`: the external key) |
| Exported | `Cantus Vinculum` (CLAP and VST3), its detector keyed by the sidechain input |

## The window

The unit in its rack, dark in every theme, its faceplate in the colourway's finish (Console's
lacquer, Broadcast's enamel, Steel's brushed stainless):

- **The faceplate's strip**: the maker's name and the model's own name engraved, and the
  jewel lamps — **External key** (amber), **Auto release** (green, the VCA), **Limit** (red,
  the opto and the vari-mu), **Valve colour** (orange, the vari-mu). The lamps are controls:
  a click on the key's lamp opens the key's list under it (its own input, or a strip of the
  desk; in another DAW it throws the plug-in's sidechain switch); a click on Auto lights it
  and a second click gives back the release it had; a click on Limit throws the character.
  The colour's lamp only shows (the colour is a knob on the Valve card). Each change is one
  step of undo.
- **The meter bridge**: the gain reduction's needle and the output's, each behind its window
  on a backlit card, the reduction's peak held a moment; between them a window on the model's
  gain element — the VCA's chip on its board, the opto's cell with its panel glowing as it
  works, the vari-mu's valves.
- **The model's panel**: the model selector and the model's own stepped switches (the VCA's
  ratio, attack and release; the character's bat toggle; the vari-mu's six time positions).
  A click on an engraved position chooses it; a click on the knob steps it; a click on the
  bat throws it; the menu offers automation and MIDI learn. One click, one step of undo.
- **The screen**: the static transfer curve at the settings (make-up and mix included) with
  the signal's point on it, the reduction in large figures, the ratio the curve has where the
  signal is, and eight seconds of gain reduction hanging from the top over the input and
  output (the key's level too when one is routed).
- **The cards**: Compression (threshold, make-up), Side chain (the key's field, the
  high-pass), Valve (the colour, the vari-mu only), Output (mix, output).

## Presets

Fourteen, fitted to the dry's loudness (`preset_tool fit`, −2 … +2.8 dB, each mostly by its
make-up): **VCA** — Glue (the starting preset), Mix bus glue, Punchy drum bus, Drum smash,
Parallel drums (40 % of the smash under the dry kit), Bus pump (route the kick as the key
for the classic duck), Bus catch (10 : 1, the fastest attack: a limit); **Opto** — Opto
vocal bus (the key's high-pass at 100 Hz), Smooth leveller (70 % wet), Opto vocal limit;
**Vari-mu** — Valve mastering glue, Valve warmth, Valve limiter, Lo-fi squash (Limit, the
colour full, half under the dry). Roles: bus, master, drums, vocal, creative.

## Exported

`Cantus Vinculum` in CLAP and VST3. Its detector listens to the sidechain input once the
plug-in's own *Sidechain* switch is on *Sidechain input* (`cantus.sidechain`, as on every
exported keyed unit; CLAP's second audio port, VST3's aux bus), to its own input otherwise —
bit-identical to the DSP keyed by the same sound in both. The faceplate's key lamp throws
that switch in a plug-in window. No latency, no tail. Its ids are blessed in
`crates/cantus-plugins/tests/golden/ids.txt`.

## A pedal on Tabula

Vinculum may be a pedal (a compressor pedal is natural): its face shows the model, the
threshold, the make-up and the mix; the character and the times stay on its editor's
faceplate, opened from the pedal. On a board it compresses the board's own signal — a board
of one Vinculum is Vinculum inserted alone, to the bit. A board takes a key when one of its
pedals does, so a keyed board's key reaches Vinculum's detector exactly as an insert's
(`fx_vinculum::as_a_pedal_it_compresses_its_own_input_and_hears_the_board_s_key`).

## Ours, better

- Three bus compressors in one, each checked against its own law solved independently
  (within 0.01 – 0.02 dB) and against the published time constants and ratios — the opto
  against the LA-2A's published measurements and its CdS cell's (ADR 0139, *The opto
  matched*).
- A vari-mu limiter that cannot ring at 20 : 1 and 0.2 ms: its loop is solved at zero delay
  through its tabulated equilibrium; its colour runs at 2× on the residual only, so the dry
  path is never filtered or delayed.
- The key from any strip with no send to set up, through its own high-pass.
- Exact rest: a silent Vinculum costs nothing.

## Left for the owner's ear

- **The opto, matched** (2026-11-17, the owner's decision): its Limit's attack was 0.9 ms
  against the published ~10 ms; now 9.0 ms (Compress 10.2), and the cell remembers. To hear
  in `compartir\vinculum\sound` (04 – 06 and 24 – 26 beside their `-before` twins, and
  the memory excerpt, 30 – 31).
