# Chorda: the modelled piano, recorded pianos and the piano's mixer

*Cantus 0.1.0-beta.15 · [Leer en español](../es/chorda-pianos.md) · back to the [manual](README.md)*

Chorda is Cantus's piano. Its own sound is **modelled**: strings, hammers and soundboard
computed as you play, nothing to download. It can also play a **recorded piano** — a free
sample pack (the Salamander Grand, bitKlavier's Steinway D, Upright Piano KW, …) or one you
bought — with the same window, the same notes and the same mixer strips. This chapter is
about choosing the source, getting the pianos, mixing their microphones and the two things a
recorded piano adds: **Add our room** and a microphone's **delay**.

## Choose the source

Open Chorda (double-click its track). The **Source** list is at the left of the **Play**
view; **All · Recorded · Modelled** at its top filters it.

- **Modelled** — *Chorda 2*, today's model. *Previous models* unfolds to *Chorda 1*, kept so
  older songs sound as they did.
- **Recorded** — the free pianos Cantus knows, the Salamander Grand first. A piano not on
  your disk shows what it takes (**Get · 742 MB**); a click downloads it from its official
  source, with its progress and *Cancel*. A piano on disk says **On disk**; the one playing
  says **Playing**. *Piano in 162* and *Helsinki 1967 Pro* come only from their authors' pages
  (a torrent, a login): **Open page**, download it there, then *Install from a file…*.
- **Your own pianos** — what you installed from a file.

A click on a piano that is on disk switches the source: **one undo step**. Your notes, your
automation, your strips and the mixer's routes stay; switching back to the model plays its
settings exactly as they were.

### Install from a file…

The dashed box at the end of the list takes an **SFZ**, a **Decent Sampler** preset, an
**SF2** or a **zip / tar.gz** of one. A download of a piano Cantus knows (recognised by its
size and checksum, whatever its name) installs as that piano; anything else becomes one of
*Your own pianos*, played where it is (an archive is unpacked into Cantus's content folder).
Tympanum's kits list has the same box for drum kits.

## The Play view

As the design draws it: the piano in its room (the picker of the piano and its finish over
the picture; the room, floor, walls and rug on the card over its corner — the look, never the
sound), the keyboard under it, the **Hammer**, **Resonance** and **Output** cards, and **The
piano** at the right:

*(The picture is drawn **Drawn** — light, the default — or **Photographic**, with traced
reflections and soft shadows: Settings ▸ Appearance ▸ Instrument pictures. The sound never
changes.)*

- **Lid** — closed, short stick, full stick.
- **Perspective · Where you listen** — from over the hammers to the hall; the mics of the
  mixer it reaches are listed under it, and **Open the mixer ›** goes there.
- **Pedals** — *Sustain* and *Soft* show where the player's foot is (controllers 64 and 67)
  or where you set them; *Dampers* sets how firmly the dampers stop the strings; the curve
  under shows the dampers against the pedal, the half-pedal band shaded.

Moved in this version: Chorda's **Room** level is the Mixer's *Room* strip fader, the
**Width** is the Mixer's *Piano* strip (Chorda 1 keeps it in Output), **Dampers** is the row
under Pedals, and the **macros** are in the Edit view. The first time you open the new Play
view, the status line says so.

## The Mixer view

**Mixer** at the top of the window shows the piano's own mixer:

- The picture and a card for the **selected output**: its name as the pack names it, who
  recorded it and its licence. For bitKlavier, **Images on disk** — how many of its nine
  microphone images you have, what they take, and **Get the other n**.
- **Outputs** — **All to Main** puts every output back on the piano's own strip; **Each mic
  to its own strip** gives every output a strip of its own in Cantus's mixer, in order, playing
  into the piano's strip. One undo step each. At unity the split piano sounds exactly like the
  unsplit one.
- The strips — **Piano** at the left (the whole piano's level; the model's Width), the
  outputs under their family heads (*Perspectives*, *Noises*; a head folds its strips), and
  **Room** at the right. Each output has polarity (ø), pan, mute, solo, its fader and its
  **Out** cell: *Main*, its own strip, or any strip of the desk.
- **bitKlavier's images.** Each microphone image is its own download (about 1.5 GB on disk
  each). An image not on disk is a dashed strip with its download; an image on disk loads or
  unloads from its top cell (*as recorded* · *unloaded*): an unloaded image takes no memory.
- The model's five outputs (Hammers, Player, Audience, Room, Mechanics) are listed; until the
  model renders them apart they play together on Main, and the row says so.

## Add our room

A recorded piano carries the hall it was recorded in (its far microphones hear it). A piano
recorded close or dry may want more room: on the **Room** strip, **Add our room** steps
through *Off · Small room · Studio · Hall* — the rooms Tympanum's kits play in. When it is on,
the strip's fader is the room's level; it plays on the piano's own strip. **Off adds
nothing**: no room is built, no memory taken, the piano sounds exactly as before.

## A microphone's delay

Each output has a delay (up to 50 ms) to line its microphone up with another — a far pair
arriving later than the close one — or to push it later on purpose. It is the **Delay** cell on
every output strip, under ø and pan: drag it sideways (0.05 ms a point, `Shift` 0.01), or
double-click it and type a value (`Enter` keeps it, `Esc` leaves it); 0 is *as recorded* and adds
nothing. Each change is one undo step, and it can be drawn as automation. A delay that changes
while the microphone sounds **glides** to its new time (no click, no silence); at rest the
output is delayed by exactly the whole samples you set. Cantus builds a delay line only for the
outputs that have a delay or an automation lane on it; the first time an output gets one, the
notes sounding at that moment stop.

## Disk and memory

- Packs are kept as **FLAC** when they are installed: bitKlavier's images take about 1.5 GB
  each instead of 3.8 GB; every file is checked sample for sample before the original goes.
- Long samples **stream** from disk through Cantus's sample cache (*Sample cache…* in the settings). Several
  bitKlavier images at once fill that cache fast (about 4 GB each); raise its limit if you play
  four or more together.
- One image costs about 6 % of one processor core; four images about 25 %.
