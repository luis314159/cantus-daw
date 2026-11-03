# VCA faders and mixer snapshots

*Cantus 0.1.0-beta.11 · [Leer en español](../es/mixing-studio.md) · back to the [manual](README.md)*

Two tools of a studio desk, in the mixer: **VCA faders** that move a whole group of strips
together without touching their own faders, and **snapshots** that store the desk and put it
back, whole or one channel at a time.

## VCA faders

A VCA is a strip with **no audio**: a fader, a mute and a solo. The strips that *follow* it
get its level added to their own, in decibels, at the gain stage — right where each strip's
fader acts. Their own faders, automation and modulators are not rewritten: a strip at −3 dB
following a VCA at −6 dB plays at −9 dB and still shows −3 dB. A VCA at 0 dB changes nothing,
not one sample.

**Make one.** In the mixer's ▸ menu, **Add VCA**; or right-click a strip (or a selection) →
**Follow VCA ▸ New VCA with this strip**. A VCA is drawn as a strip with a coloured "VCA"
plate where the meter would be, and the number of strips that follow it. Every strip a VCA
reaches wears a bar of the VCA's colour under its number; hover the number to see which.

**Follow one.** Right-click a strip → **Follow VCA ▸** and tick the VCAs it follows (with a
selection, all selected strips follow it together). A strip may follow several VCAs: their
decibels add, **each counted once** even when one VCA reaches the strip twice (a VCA can follow
another VCA). A cycle is refused. **A group can follow a VCA** too: right-click the group's
separator → **VCA of the group**.

**Mute and solo follow.** A muted VCA mutes everything it reaches; a soloed VCA solos
everything it reaches (and their routes, as FL's solo does).

**Automation.** The VCA's fader is an ordinary fader: draw or record automation on it, or put
a modulator on it. Each member's automation stays and the two add. A VCA cannot hold inserts,
sends, an output or a track, and a VCA somebody follows cannot be removed.

## Snapshots

The mixer's title row has a **Snapshots** field. **Take snapshot** stores the desk — each
strip's fader, pan, width, polarity, mute, solo, swap, inserts switch and EQ, each send's level,
switch and tap, each insert's bypass, dry/wet and native parameters, and the master's — under a
name; **Take snapshot of the selected strips** stores only those. A snapshot holds *state*,
never *structure*: routing, which inserts exist and a hosted plug-in's own state are not in it,
and a recall never creates, removes or re-routes anything.

Each snapshot's submenu:

- **Recall** puts every channel back as one undo step. **Recall the selected strips** puts back
  only those; **Recall everything, safe strips too** also reaches strips marked safe.
- **A/B with the desk** swaps the desk with the snapshot and back, to hear the difference.
- **Update from the desk** takes the state again; **Rename**; **Remove**.
- **Cue at the playhead** (see below).

**Safe strips.** Right-click a strip → **Safe from snapshot recall**: a recall of everything
leaves it alone (a recall naming it still reaches it). **Safe from solo** keeps a strip playing
when another is soloed (a click, a reference) unless it is muted.

**Cues: snapshots on the timeline.** A cue recalls a snapshot at a bar — the whole scene: from
there on its faders, pans, widths, send levels and native effect parameters hold, and its mutes,
solos, polarity, swap, insert switches, bypasses and strip EQ are switched (faded over 16
samples, never cut). A level holds until another cue or the next of your automation clips for
that control starts; a clip that was playing across the cue's bar is silenced from the cue on.
Cues are placed at the playhead and listed under the Snapshots menu, where each is removed;
adding and removing a cue is one undo step. In the session view, a scene's clips carry their own
envelopes instead (a scene is launched by playing, not at a bar).

## What an agent can do

`create_vca`, `assign_vca`, `take_snapshot`, `recall_snapshot`, `get_mix_snapshots`,
`remove_snapshot`, `add_mix_cue`, `remove_mix_cue` and `set_strip_safe` — see
[What an AI agent can do in Cantus](ai-tools.md).
