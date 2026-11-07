# Pulsus III: pads on their own strips, a chain per pad, kits from anywhere

*Cantus 0.1.0-beta.12 · [Leer en español](../es/drum-kits.md) · back to the [manual](README.md)*

Pulsus is Cantus's drum instrument: up to 32 pads, each a synthesised drum or samples. This
chapter is about three things a drum kit needs in a mix: each pad on **a mixer strip of its
own**, **effects on one pad** before the kit is summed, and **kits made from your own
samples** — a folder of one-shots or an SFZ kit.

## Split to strips

Open the kit (double-click its track) → **⋯ ▸ Split to strips**, or right-click the kit's
strip in the mixer → **Split to strips**. Every pad that makes a sound gets a strip of its
own, named after the pad, placed right after the kit's strip, in the order of the pads. Each
pad now has its own fader, pan, inserts, sends, mute and solo. One undo step.

The pad strips play **into the kit's strip**: the kit's strip is the kit's bus, so a
compressor or a fader there still acts on the whole kit, and the kit's track still mutes and
solos all of it. With every new strip at 0 dB and centred, the split kit sounds **exactly**
like the kit did before — not nearly: the same samples, to the bit (with the kit track's own
level at 0 dB and its balance centred; at another level the two agree to within −120 dB).
A pad's strip pans as a balance (its pan law is 0 dB), so it passes the pad untouched; the
kit's strip places the kit through the project's pan law as before. The strip's menu says
which law it follows (**Pan law ▸**), and lets you change it.

**Merge back** (the same menus) puts every pad back on the kit's strip and removes the
strips the split made — unless something else uses one (a send into it, another track).
One undo step.

Good to know:

- Soloing a pad's strip plays that pad alone, through the kit's strip.
- Freezing the kit freezes the kit's strip, with every pad strip inside it. The kit's
  track alone cannot be frozen while it is split (its sound is several outputs).
- The recording mode bakes the kit's strip as one render.
- Bouncing the kit's track in place bounces **the whole kit** (every pad, as the kit sounds
  unsplit) onto the kit's strip; the pads' own strips — their faders and inserts — are not in
  the bounce. To bounce what the mixer does to the pads, bounce the kit's strip. A pad's
  strip cannot be bounced on its own: it is one output of the kit.
- Loading another kit onto a split kit keeps the split: each new pad takes the strip of the
  old pad on the same note (or of the same kind of drum; for kits of samples, of the same
  name, then in the same place). Changing the instrument or deleting the track removes the
  pad strips with it, in the same undo step.
- Routing the kit's track to another strip takes the whole kit there: the pads' strips and
  chains that played into the old strip play into the new one, in the same undo step.
- Splitting needs room: the engine plays at most 256 tracks — the project's, and one more for
  each pad on a strip or chain of its own — and 256 strips. A split past that is refused and
  said; remove some tracks or strips first.
- A track template of a kit keeps its pads' strips and chains (inserted, they are new strips
  of their own). Exploding a kit's takes gives the take tracks no chains: those are the kit's.
- Stems: a pad's chain is part of the kit's sound — in the kit's track stem, and, with the kit
  straight to the master, in the stem of what goes to the master directly.
- DAWproject has no outputs for an instrument: a split kit is written whole, and the export
  report says so.

**Older kits.** A kit made before this version is *Model 2* (or *Model 1*). It splits too, but
sums to itself only within rounding (about −135 dB); **Upgrade to Pulsus III** in the kit's
header makes it exact. Nothing else changes.

## A chain per pad

Select a pad and open **Edit**: the **Chain** card lists the pad's own inserts, in order.
**Add insert ▾** puts any Cantus effect at the end (with its starting preset); each insert
has its switch, **Edit** (its editor) and **Remove**. The pad's sound goes through its chain
before it joins the kit — or before it goes to its own strip, when the kit is split; split
and merge keep the chain on its pad. Once the pad has a chain, **More…** opens the device
picker onto it: every effect, plug-ins included.

A chain costs nothing while its pad is silent: Cantus does not run its inserts until the pad
plays again, and what comes out is exactly what running them would give.

A chain follows its kit: route the kit's track to another strip and every chain goes with it.
**Add insert ▾** offers what a strip offers (Spatium is the reverb).

## Kits from your samples

**A folder.** Drag a folder of one-shots from the file manager onto a kit's pads: it becomes
that kit. Drop it anywhere else in the window: it becomes a new kit track. Cantus reads the
names, in English or Spanish:

| A name with | Goes to |
|---|---|
| kick, bd, bombo, bass drum | Kick (note 36) |
| rim, rimshot, side stick, aro | Rim (37) |
| snare, sd, caja, redoblante | Snare (38) |
| clap, palmas | Clap (39) |
| hat / hh / hihat / charles + closed, cerrado | Closed hat (42) |
| … + pedal, foot, pie | Pedal hat (44) |
| … + open, abierto | Open hat (46) — the three hats choke each other |
| tom + low/floor/grave, tom, tom + high/agudo | Toms (41…50) |
| crash, splash, china · ride · cowbell, cencerro | 49 · 51 · 56 |
| perc, shaker, pandereta, conga, bongó, clave, maraca… | Percussion (54, 58, 60…) |

Files that differ **only by a number** (*Kick 1, Kick 2*) become one pad that takes them in
turn (round robin) — except a number that names the drum: *Tom 1, Tom 2, Tom 3* (high to
floor), *Crash 1, Crash 2* and a leading one (*808 Kick*, *909 Kick*) are pads of their own. Files that differ by a **dynamic** (*soft, medium, hard, pp, mf, ff,
vl1…vl4, suave, fuerte*) become one pad's velocity layers, the softest for the softest hits.
A name that says nothing goes on the next free pad, in the folder's order. Up to 32 pads and
16 layers a pad; what does not fit is named in the log. One undo step. A folder is read in
the background; one of more than 512 sounds (or 4 GB) is not a kit and is refused, and a
folder dropped away from a kit's pads must name a drum in some file.

**An SFZ kit.** Drag an `.sfz` file the same way: one pad per key, its regions as layers
(velocity ranges, tuning, level, pan), round robins and random choices kept, and the kit's
exclusive groups (a hi-hat's) as a choke group. Tested with the free *Virtuosity Drums* kit.

## The factory kits

The kit's header lists the sixteen factory kits: Pulsus classic, Studio, 808, 909, Boom
bap, Electro, Percussion, Trap, House, Lo-fi, Rock, Pop, R&B, Reggaeton, Cinematic and Jazz
brushes — every one synthesised by Pulsus itself, no samples. Each comes with its own
**Kit level** (the kit card), calibrated; 0 dB on a kit of your own changes nothing. **⋯ ▸ Browse presets…** opens
the preset browser on the kit: filter by style, listen to a kit before you choose it, load it
in one undo step.

## For agents

`split_kit_outputs` (and `merge`), `set_pad_chain` (add or remove an insert on a pad) and
`make_kit_from_folder` (a folder or an `.sfz`, only inside the project's folder or a folder you
allowed in Settings ▸ AI, with file access on) do the same, one undo step each — see
[What an AI agent can do](ai-tools.md).
