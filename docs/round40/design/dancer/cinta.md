# J · Cinta, the cassette: what the rig draws today, what it would need

**Files:**
- `cinta-sheet.png` / `.html`
- `cinta-moves-night.gif`, `cinta-moves-paper.gif`
- `cinta-window-night.gif`, `cinta-window-paper.gif`
- Design source: `src/cinta.py`

## What it is

An 80s cassette that breakdances.
- **Head and body:** a graphite shell with the knob's rim and four screws. A brand-free cream
  label with two retro stripes (vermilion and lapis); no logo, no maker's design.
- **Face:** two **reels for eyes** (white hubs with six teeth, turning as it dances). Lids of
  the label make the moods; the tape opening is its mouth.
- **Clothes:** a **backwards lapis cap** with a gold button, lapis track pants with a
  vermilion stripe, white gloves, chunky sneakers with gold laces.
- **The tape:** slips out of the shell's bottom and swirls with every move, its curls
  following the body late.
- **Moves:**
  - *toprock*: a cross-step on each beat, arms flung open;
  - a shuffle;
  - signature, **Rewind**: the toprock runs backwards at ×8, the reels spin back, the tape
    whips and a gold rewind sign flashes; then a freeze on the beat, and it restarts.

## What `cantus-dance` draws today

| Cinta needs | Today |
|---|---|
| Shell, label, stripes, window, cap | `rect_into` → `rrect_into` (shared, new); `hard` for the cap |
| Reels and teeth | `ellipse_into`, `ribbon_into` |
| The tape | a `ribbon_into` along 26 points that follow `Body::top`/lag (as Fuga's scarf tails) |
| Limbs, gloves, sneakers | `kit::hose_into`, `mitten_into`, `shoe_into` |

## What it would need

1. **Rewind** evaluates its own groove at −8× time: still a pure function of the beat.
2. **The reels' turn:** `0.8 · beat + shape`, a per-character extra.
3. **`rrect_into`** (shared); two-bone IK for the legs (shared).
4. **Its own moves:**
   - *Bounce* → toprock;
   - *Step left/right* → shuffle;
   - *Jump* → Rewind.

## Cost

About 45 fills: the teeth of both reels are one path; the tape is one ribbon plus one sheen.
All opaque. Fine.
