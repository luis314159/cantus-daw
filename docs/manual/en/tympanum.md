# Tympanum, the acoustic drummer

*Cantus 0.1.0-beta.15 · [Leer en español](../es/tympanum.md) · back to the [manual](README.md)*

Tympanum is Cantus's acoustic drum kit: a kit standing in its room, heard through its
microphones, each microphone on a strip of the mixer if you want it there. It plays two kinds
of kit. A **modelled** kit is computed as you play — nothing to download, and its virtual
microphones can be moved. A **recorded** kit is a free multi-microphone sample pack (DRSKit,
CrocellKit, Virtuosity Drums, …) or one you have, played in the room it was recorded in.
Pulsus stays Cantus's pad drum machine; Tympanum is the drummer.

**Add it** from the device picker (the **+** above the track headers, its *Drums*
shelf): **Tympanum** stands beside Pulsus. A new Tympanum plays the modelled **Studio Maple**
kit with its generated voice — instant, nothing to download.

## The Kit view

The window opens on **Kit**: the kits at the left, the kit in its room in the middle, the
pieces and their cards below.

**The kits list** — **All · Recorded · Modelled** at its top filters it, and a search box finds
a kit by name.
- **Modelled** — *instant · no download*: **Studio Maple** (rock and pop, twelve pieces and
  twelve microphones), **Jazz Birch** (measured off Virtuosity Drums, public domain) and the
  other modelled kits. A click plays the kit.
- **Recorded** — *real drums · free packs*. A kit not on your disk shows what it takes
  (**Get** and its size); a click downloads it from its official source, with its progress and a
  click to cancel. Some come **from the author's site** (one click, straight from the author's
  own page — Cantus never hosts them) and some are **Manual**: *Open the page*, download it
  there (a login, a torrent), then *Install from a file…*. A kit on disk says **On disk**.
- **Your own kits** — **Install from a file…** takes an SFZ, a DrumGizmo kit (its `.xml`, or
  the archive it came in) or a folder of WAV files. A DrumGizmo kit is built for Tympanum in the
  background, one file per microphone per hit: the window keeps answering, its box shows the
  progress (*Building its mics · 30 %*), and a click on it cancels and leaves nothing behind.
  The kit you were playing keeps playing until the new one is in.
- **Kits on disk · Manage…** — what they weigh; *Manage…* opens the Content window to update
  or remove them.

**Changing the kit** is one undo step and carries your mixer: an output keeps its strip, level
and route when the new kit has a microphone of the same name or the same family (Kick, Snare,
Toms, Overheads, Room…); a strip made for one microphone is renamed after the new one; strips
left with nothing go. A preset that names no kit keeps the kit and its mixer.

**The picture** — the kit drawn in its room (*Drawn* or *Photographic*: Settings ▸ Appearance ▸
Instrument pictures). **A press on a piece plays it and selects it**; Alt-click walks through its
articulations; dragging up and down while you press sets how hard. *Show mics* draws the
microphones (on a modelled kit you drag them to move them).

**The room card** — **Room** is the picture and the sound in one step: **Stage** plays the kit's
own room (a recorded kit as recorded; a modelled kit dry, on its own), **Booth**, **Studio** and
**Hall** put one of Cantus's rooms on the room microphones. **Floor**, **Walls** and **Rug**
change the picture only.

**Below the picture**:
- the row of pieces with their keys;
- **Feel** (the whole kit) — **Timing** (each hit up to that many milliseconds late, never
  early), **Strength** (how much each hit's strength varies from the written one) and **Never
  the same hit twice**;
- **Hi-hat pedal** — its **Openness controller** (controller 4 by default): open and close the
  hat with a continuous pedal; **Foot chick when it closes** plays the foot's *chick*;
- **Kit** (every piece) — **Tune**, **Ring** and **Level**;
- **the selected piece's panel** — its size, **Hear** (hold it for a roll), its
  **Articulations** (each on its key, a click to hear it), **Where it is struck** (a click on
  the head sets it and plays there; the edge is brighter), **Spread**, **Sound**, **Heard by**
  (which microphones hear it) and **Open the mixer ›**.

**Voice: Generated · Hybrid** (a modelled kit, under the lit kit in the list). *Generated* is
Cantus's model alone. *Hybrid* plays, under the model, the first moments of a real kit's hits —
each piece's attack as it was recorded — through the same virtual microphones, bleed and room.
Its recorded attacks are a free pack of about 5 MB (the jazz kit's from Virtuosity Drums, CC0;
the studio kit's from DRSKit, CC BY 4.0): **Get · 5 MB** once, then it plays offline. Until the
pack is on disk, *Hybrid* sounds exactly like *Generated*.

## Playing it

Tympanum follows the General MIDI drum map (kick on 36, snare on 38, closed hat 42, open hat
46, crash 49, ride 51, …). A few notes past the map play a drummer's figures on the snare:
**flam** on 91, **drag** 92, **buzz** 93 (eight quick, soft bounces) and a **held roll** on 94
(strokes every 55 ms for as long as the note is held); the hat's **foot splash** on 95. A
recorded kit plays its pack's own recordings of its articulations. Every microphone of a hit
plays the same take, and a take never repeats the one before it on that key.

## The Mixer view

**Mixer** at the top of the window shows the kit's microphones:

- **The band of microphones** (a modelled kit): the selected microphone's **Pattern**
  (cardioid, super, hyper, omni), **Distance**, **Aim** and **Bleed**; **What it hears** — each
  piece in that microphone against its own; **Every mic** — each piece in each microphone in dB,
  a click on a cell turns it off (a recorded kit's microphones stay where they were recorded).
- **Outputs** — **All to Main** (every microphone on Tympanum's own strip), **Pieces to strips**
  (one strip per family — kick, snare, toms, cymbals, overheads, room — its microphones into
  it) and **Each mic to its own strip** (a strip for every microphone, in order, playing into
  Tympanum's strip). One undo step each; at unity the split kit sounds exactly like the
  unsplit one.
- **The strips** — each microphone with its meter, polarity (ø), pan, **Delay** (up to 50 ms
  to line a far microphone up with a close one; drag it sideways or double-click and type;
  0 is *as recorded*), mute, solo, fader and **Out**: *Main*, its own strip or any strip of the
  desk. A delay changed while the kit plays glides to its new time.

In Cantus's own mixer the kit's strips sit under one **band** (*Drums · Tympanum · 12 mics*)
that folds into one strip; *Each mic to its own strip* groups them by family inside it, and a
drum bus fed only by them can join the band — see *Bands on the desk* in
[the mixer](README.md#6-mix-the-mixer-inserts-and-sends).

## The recorded kits

| Kit | Microphones | Licence | On disk |
|---|---|---|---|
| Gogodze Phu Vol. II | 7 | CC0 | 0.5 GB |
| DRSKit, every microphone | 13 | CC BY 4.0 | 1.9 GB |
| CrocellKit, every microphone | 15 | CC BY 4.0 | 5.9 GB |
| MuldjordKit, every microphone | 13 | CC BY 4.0 | 2.7 GB |
| Virtuosity Drums | 7 | CC0 | 1.5 GB |
| Big Rusty Drums | 6 | CC0 | 0.7 GB |

Every pack's licence and credit are shown before you get it and in the About window's *Credits*; a CC BY
kit's credit goes with your music where the licence asks for it (MuldjordKit asks, in its
author's words, for *"Drum samples provided by DrumGizmo.org"* in an album's credits). Long
samples stream from disk through Cantus's sample cache, and a kit loads only the microphones
that are on.

## What it costs

A modelled kit is computed as it plays: on the laptop Cantus is measured on, the model alone
takes about a tenth of one processor core in a busy groove, and every microphone on its own
strip about a fifth. A recorded kit costs what its voices cost to stream. Nothing runs while
the kit is silent.
