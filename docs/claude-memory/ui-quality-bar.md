---
name: ui-quality-bar
description: "User judged Cantus's UI ugly and plain on 2026-09-25 — the app, plug-ins and instruments must be redesigned to the level of the best DAWs, FL Studio as the aesthetic reference"
metadata:
  node_type: memory
  type: feedback
  originSessionId: ab6ce7df-a46e-4ebb-a229-b046ae37783d
  modified: 2026-09-25T22:29:18.412Z
---

On 2026-09-25, after trying the build, the user said the application, our effect plug-ins,
and (they suspected) the instruments look "super ugly, plain". Their specific complaints:

- The mixer's side panel (the inspector) looks very, very ugly.
- Letters sometimes run into each other.
- Labels read "like loose words thrown down".
- Interface elements should be resizable by dragging their edge, and movable within the
  interface.
- The mixer can't be scrolled down easily.
- Many small details make it hard to move around quickly.

They want the whole style rethought to the level of the best DAWs, researched with
parallel agents. "FL Studio is beautiful."

The same day they added: "the style I like most, by far, is FL Studio, but take even more
cool things from the others — let's get inspired and make something super pro". So FL
Studio is the **primary** visual reference. Ableton, Bitwig, Logic, Studio One and the
top plug-in/instrument UIs supply specific ideas on top of it (e.g. Bitwig's two-column
inspector, section toggles instead of vertical scroll, a hover status line).

They then shared research on how FL is built: Delphi, and its own vector renderer since
FL 12, so it is crisp at 4K and identical on every OS. They asked: "make it as excellent
as FL; it doesn't matter if we vectorize components by hand — it must look excellent and
have incredible performance". Hand-drawn vector components in egui/epaint are the path:
gradients, bevels, shadows, vector icons, a UI zoom, with a measured frame-time budget.

**Logos and splash (2026-09-25).** The user designed ten logo variants in
`docs/icons/cantus—Iconos.html`. It is a bundler export: each page's template holds one
SVG with `sc-camel-*` attribute names. They want these:
- drawn natively in vectors, as the app draws;
- **selectable** as the app's logo;
- **tinted by the configured interface colour**;
- **#2 "C de onda con playhead-cruz" kept as it is by default**;
- shown on a **splash while loading, like FL Studio**.

They also said: "let's make something as hardcore as FL Studio, almost from the machine's
bare metal, but in Rust". Performance and craft come from the ground up.

**Why:** the functional depth is there; the look and the feel of moving around decide
whether it feels like a first-rank product.

**How to apply:**
- Treat visual quality and navigation speed as first-class requirements, not polish.
- Judge any UI change by rendering it (`examples/shoot`) and comparing it with the
  references in `research/ui/` (FL Studio, Ableton, Bitwig, Logic/Studio One, pro
  plug-in and instrument UIs) and with `research/ui/cantus-audit.md`.
- Typography needs clear label/value units and hierarchy; no stray words.
- Panels need to be resizable and movable.
- Every scrollable area must scroll in both directions where content can overflow.

The redesign ADR is shared with the instruments session (`daw-b0`), which moves the
instrument frame onto it. See [[mixer-direction-fl-parity]] and [[cantus-workflow-habits]].

**2026-11-04 (round 37):** the owner asked for a "very, very hard" visual comparison with FL
Studio to beat it — "hacer los gráficos a pie como ellos" (hand-drawn, inspired not copied).
Done side by side at the same scale (private artifact https://claude.ai/artifact/9haNNcaDVADAwCC7AffnkZ,
summary in research/ui/fl-look-2026-11/notes.md): FL wins 10, Cantus 2 (rack, EQ), split 1
(mixer: we win information, lose finish). The gap is material finish — depth (wells, bevels,
specular), saturated user colour, one icon family, type scale, clip content visible. Proposed
a kit-level "finish" round after round 37, accepted when the same 13 shots win or tie.

**2026-11-05:** after finish's steps 1–3 (materials, controls, icons; FL 9 / Cantus 2 / tie 1)
the owner: "¿Es lo mejor que puedes hacer? Haz algo excelente hasta que tenga el detalle y
calidad superior a FL Studio." → the bar is **every element WINS at 100 %**, not ties; a loop
per element (shoot → compare → harsh critique → fix), then a blind A/B judge agent, then the
owner's eyes. Show the owner progress often.
- 2026-11-06 blind judge (fresh agent, unlabeled pairs, 100 % Night) on finish's second delivery:
  Cantus 7 of 8 (FL won toolbar pictograms); FL still ahead in small-scale finish (thin clock,
  condensed type, distinct pictograms). Gotcha: when shuffling pairs, parse key.txt per side
  ("a = FL Studio, b = Cantus" fooled a regex) — the judge's own identification caught it.
- 2026-11-06 the owner liked the round-36 mixer fader more, then: "los dos están chidos, ¿configurable?" → an Appearance setting Fader: Classic (default) / Metal, long throw in both.
- 2026-11-06 second blind judge (24 pairs, Night + Paper, browser excluded): **Cantus 23/24**;
  FL kept toolbar pictograms (dark) narrowly. Remaining weaknesses sent to finish for a last pass.
- 2026-11-06 the owner delegated UI calls: "Haz lo que sea excelente y mejor para UX y UI." I decided
  the top bar: nothing folds at 1920×1080 @100 %; below that, fold by a defined priority (never
  transport/record/clock/tempo/undo/audio status). Use wins over looks, and looks must still win.
- 2026-11-07 third blind judge: Cantus 22/24 (FL's one win was a mis-cropped toolbar pair); 'very good, nearly first-rank', EQ first-rank; last list to finish (status micro-type, taller meters, clip content, rack grid, Paper contrast).
- 2026-11-06 fourth blind judge (fresh, stricter, 26 pairs incl. browser): Cantus 17/26 (dark 8/13, Paper 9/13). FL won top bar, transport, toolbar icons, mixer (slight), browser. Cantus's icons are the worst single item; empty-state browser shot loses. List sent to finish (6th delivery). Lesson: shoot populated views, check crops before judging, idle "—" values read as unfinished.
- 2026-11-06 fifth blind judge after finish's 6th delivery: Cantus 16/26 (W 8–5 both themes). Lost top bar, toolbar icons, mixer, strip, arrangement. Found the judge pairs compared our CPU-rendered shots (soft/halo) with FL's real screen captures → 7th delivery captures the real Cantus window. Lesson: compare like with like (real captures both sides); "ticks" for short MIDI hits read as dots — draw mini piano-roll bars.
- Judge 6 (2026-11-06, real captures both sides, finish 7th delivery): Cantus 17/26. Lost: toolbar icons both themes (blobs), browser both (sparse vs FL's thumbnails/waveforms/filters/player), arrangement both (garish clip colours, black note dashes, needless truncation), piano keys both, Paper mixer (washed pastels, black meter slabs, stray half strip "L"). Routed to finish as 8th delivery.
- Judge 7 (2026-11-06, after finish's 8th delivery): Cantus 14/26 — judges swing 14–17 on similar trees, so read their *defects*, not the score. Real bugs it found: piano-roll white-key separators not a keyboard's geometry, Paper browser tabs clipped, crop debris in a pair, "Per...ion" truncation. 9th delivery = concrete defects only, then close the round.
- Judge 8 (2026-11-06, finish's ninth delivery, judge9d pairs): Cantus 10/26 — night 7/13, Paper 3/13. Lost: transport, toolbar icons (thin outlines), mixer/fader materials, arrangement clips (saturated dashes), piano keys, most of Paper (pastel washes, beige empty meters). Won: rack, browser, headers, EQ, night top bar. Sent as finish's tenth delivery (last of round 37). Single-judge scores swing (17 → 14 → 10 on different captures): read the reasons, not only the tally.
- Judge 9 (2026-11-06, finish's tenth delivery, judge10d pairs): Cantus 20/26 (Night 10, Paper 10). FL kept toolbar icons, mixer, browser in both themes. Caveat: FL crops were softer/colour-fringed, which weighed against FL in 4 pairs — next time crop FL at native scale too. Those three are round 38's look items.
- Judges 10–12 (2026-11-07), owner asked for 26/26 then accepted "no element lost by both judges, then release": two fresh judges per round on 1:1 crops (no zoom either side). After finish's 11th: 20/18; 12th: 22/19; 13th: **20/22**. Won by both now: top bar, browser (a table with pack covers, real waveforms, tempo/length columns, full names). Still FL for both: toolbar icons (4 redraws: line = "pixel-art", filled = "blobs", two-tone = "abstract, blur together"; FL's = "grey-glass, detailed, distinct") and piano-roll keys (FL "better proportioned, larger, shaded C rows"). Lessons: one judge's score swings, two judges split on mixer/arrangement/piano — 26/26 from two judges is near a coin toss on close pairs; FL has no light theme so Paper pairs compete with FL's dark; verify crop sizes match across themes before judging (a stale layout JSON cut Night crops once).
- **2026-11-08 (after beta.12):** the owner, looking at the real app: "FL aún se ve a años luz de Cantus" — despite judges at 20/22 of 26. Lesson: crop-by-crop judging measured the wrong thing; the owner judges the whole window (gestalt). My whole-window diagnosis: Cantus shouts (saturated slabs in headers/strips/clips, warm brown-black base) where FL is calm cool slate with muted user colour; form-like label columns and boxed words (mixer NUMBER/NAME/LEVEL…, "Slot 3…10"); dead idle states (−∞ everywhere, black meter slabs); one flat type size and a small heavy clock; MIDI clips read as barcode dashes; no focal point. Proposed: a system-level visual direction (palette, type, density, idle states, clip content) judged on whole windows by the owner, mockup directions first.
