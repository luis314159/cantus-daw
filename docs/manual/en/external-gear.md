# External gear and control surfaces

*[Back to the manual](README.md) · [Leer en español](../es/external-gear.md)*

Two things live in **Settings ▸ MIDI**, under the ports: the **external gear** of the project
(a hardware synth or a rack processor that is part of a track) and the **control surfaces** of
your studio (a Mackie Control or a HUI with motor faders).

## An external instrument: a synth outside the computer

1. Right-click the track and choose **External instrument…** (or select it and press **External instrument** in Settings ▸ MIDI). Its notes now leave through a MIDI port
   to your synth, and the synth's sound comes back on a pair of your audio interface's inputs
   and plays through the track (the track's own instrument is silent: the synth is outboard).
2. Choose, in the *Routing* row, the **MIDI output port**, the **channel** and the **inputs**
   (*In 1-2* means interface inputs 1 and 2).
3. Make the synth play a sound, stop the transport and press **Ping**.

Cantus plays middle C four times and listens for the first sound that comes back. It says what it
found — *612 frames, 12.8 ms (jitter 0)* — and keeps it with the project. From then on every
other track waits that long, so your synth is **on time** with the rest of the song, in playback,
in bounces and in an exported file (they use the number kept in the project, not a new
measurement). **Measure again** whenever you change the interface's buffer, the patch's attack or
a cable. If the sound has a slow attack, correct it with **Adjust** (a trim in milliseconds, ±500).

If you change the interface's buffer, Cantus adjusts the stored number by what the interface
itself reports and says so (*measured at buffer 256, adjusted to 512*); on another interface it
asks you to measure again. Starting the transport stops a measurement; one that does not come back
is given up after 15 seconds and said.

**Rendering.** A freeze, a bounce in place or an export cannot play a synth or a rack that is
outside the computer: Cantus refuses them while a unit plays and says what to do — record the
gear's return to a track (the recording is placed by the measured latency), or switch the unit off.

## An external effect: a processor in a rack

**External effect** gives the track a *send* (a pair of interface outputs) and a *return* (a pair
of inputs). **Ping** sends a short burst of noise out of the send and finds it on the return; it
needs the cable connected, and says if nothing came back, if only hum came back, or if the
measurements disagree. The measured trip is kept and the rest of the mix waits for it. The track's sound goes out of the send pair (use outputs 3-4 or higher: 1-2 are the master's)
and what comes back on the return plays in its place, through the track's level, inserts and
sends.

## What the screen says

Each unit says whether it can play right now: *ready*, *switched off*, *no audio device is open*,
*another audio interface is open*, *the interface has no such channels*, *the MIDI port is not
there*, *its track is gone*. Cantus never opens a device or a port for a unit that is not
available, and never pings it. **On** keeps the setup and plays the track as if the unit were
not there. **Remove** takes it out and gives the track back the input it had — undo brings it back. Taking a unit away (or switching it off, or moving it to another port or channel) first releases every note it holds on the synth.

Everything is undoable and every step has a name in the history (*add external gear*, *measure
external gear*…). An agent can do all of it (`add_external_unit`, `ping_external_unit`,
`list_external_units`).

## Control surfaces: Mackie Control and HUI

Press **Add surface**, choose its kind — *Mackie Control*, *Mackie extender 1/2* (more strips to
the right of the main unit) or *HUI* — and its MIDI input and output ports (the output defaults
to the same device's). No hardware yet? **Add virtual surface** connects a virtual unit to
Cantus, to try the setup.

- **Faders** move the strips' faders (and follow them: the motors move to where the mixer is —
  never while you are touching one). One touch to one release is one undo step.
- **V-pots** pan; push resets to centre; **Shift** makes it ten times finer. **Flip** swaps
  faders and V-pots (the fader pans, the V-pot sets the gain).
- **Rec**, **Solo**, **Mute** and **Select** buttons and their lights follow the mixer.
- **Bank ◂ ▸** moves eight strips (sixteen with an extender), **channel ◂ ▸** one. The two-digit
  display shows the page.
- The **scribble strips** show each strip's name and, below, its gain; **Name/Value** shows the
  pan instead. The **meters** show levels; the large display shows the position.
- **Play, Stop, Record, Cycle, Click, Rewind, Fast forward, Scrub, the jog wheel, Marker ◂ ▸,
  Save and Undo** (Shift + Undo is Redo) drive the transport and the project.

A surface's port belongs to the surface: its buttons never play notes and never reach the generic
controller mapping, which stays as it was for every other device. The last messages each surface
exchanged are listed under the surfaces (for the HUI, any switch Cantus does not map is named by
zone and port: send them to us and we add it).

**Touch-sensing faders.** Every Mackie Control and HUI knows when a finger is on a fader. Some
controllers in Mackie mode do not: switch *Touch-sensing faders* off for them, and a fader you move
is held 0.6 s before the motors follow the mixer again — otherwise the motors would fight your hand.

*A HUI is mapped for its eight channel strips, channel and bank selection, shift, undo and save,
the cursor keys and scrub, the transport, loop and return-to-zero, and the jog wheel; its other
zones are read and ignored. Cantus pings it every second and says when it stops answering.*
