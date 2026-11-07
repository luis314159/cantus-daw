# Chorda listening test

This folder was written by Cantus (`cargo run --release -p cantus-ui --example chorda_abx -- <dir>`).
It holds seven short phrases, each played by three pianos through Cantus's own engine:

- **Chorda model 2** — Cantus's modelled grand piano as a new track plays it (round 37),
- **Chorda model 1** — the same instrument as round 28 made it (what old songs keep),
- **the Salamander Grand Piano** — a Yamaha C5 sampled by Alexander Holm (CC BY 3.0), played
  by Exemplar exactly as *Sound packs…* installs it.

Every file is matched in loudness (ITU-R BS.1770 integrated, the same target for the three
versions of a phrase) and trimmed to the same length. Nothing else was done to them.

## How to take it

1. Open `index.html` in a browser (double-click it). It needs no network.
2. Use headphones or good speakers in a quiet room. Set the volume once, on the first clip,
   and leave it.
3. **ABX 1** (Chorda 2 / Salamander) and **ABX 2** (Chorda 1 / Chorda 2): 16 trials each. In
   every trial A and B are the two pianos (in a random order) and X is one of them, drawn
   at random. Play A, B and X as often as you like — switching while one plays keeps its
   place (keys: `A`, `B`, `X`, space to stop) — then say which one X is. If you cannot
   tell, guess.
4. **Preference**: 14 blind questions, two clips of the same phrase each. Say which sounds
   more like a fine grand piano, well recorded, or that you have no preference; a word on
   why helps more than anything else.
5. **Results** shows each ABX's score and its p-value (the chance of scoring that well by
   guessing), the preferences with a sign test, and — once both ABX are finished — which
   piano was which in every trial. *Download results (JSON)* saves everything; send that
   file back.

## Reading the result

- The ABX says whether you **can tell** two pianos apart (p < 0.05: you can). Chorda and the
  Salamander are different pianos, so telling them apart is expected and is not a failure.
- The **preference** is what closes Chorda: model 2 should be preferred to model 1, and be
  at least on par with the Salamander (no significant preference for the Salamander).
- Your notes — "metallic", "thin", "dull", "synthetic attack", "no body", "too much room" —
  are what the next voicing pass aims at.

Answers are kept in this browser (local storage) until you start again; reloading the page
continues where you were.
