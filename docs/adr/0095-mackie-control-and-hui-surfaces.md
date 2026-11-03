# ADR 0095 — Mackie Control and HUI surfaces

**Status:** accepted (round 36) · **Date:** 2026-11-02 · **Extends:** ADR 0004 (commands),
ADR 0062 (MIDI learn and controller mapping — "Mackie/HUI not built here"), ADR 0079 (MIDI
out), ADR 0036/0024 (the kit)

## Context

ADR 0062 made any knob map to any parameter and left the *protocols* of console surfaces out.
A Mackie Control (the MCU and its extenders, and the many controllers that speak it — Icon,
Behringer X-Touch, SSL UF8, Presonus FaderPort in MCU mode…) and the older Digidesign HUI are
what a studio owns: motor faders, V-pots, banks, scribble strips, meters, transport, jog.
FL Studio, Cubase, Logic, Reaper and Ableton (through scripts) all drive them. Generic mapping
stays beside it; a surface is a *device*, not a mapping.

## Decisions

### 1. Protocol and meaning are separate, and pure (`cantus-core`, MPL)

| File | Holds |
|---|---|
| `external_mcu.rs` | The Mackie layout, byte for byte: decode a unit's message to a `SurfaceEvent`; encode every host message (fader, LED, ring, meter, scribble SysEx, the 7-segment digits, the device query and its connection reply); `render` — what to send *now*. |
| `external_hui.rs` | The HUI layout: the zone/port selects, 14-bit faders over a controller pair, V-pot, jog, ping, the four-character names; a stateful `Decoder`; `render`. |
| `external_surface.rs` | **Meaning**, once: `SurfaceSet` (the units, the shared bank, flip, name/value, shift), `SurfaceEvent` → `SurfaceAction`, `SurfaceView` (the mixer as a unit shows it), the fader scale, the pan, the texts. |
| `external_emulation.rs` | `VirtualMcu` and `VirtualHui`: the *device* side of both protocols, so everything is tested — and demonstrated — with no hardware. |

Nothing here touches a project or a port. The interface thread hands the set the mixer as a
`SurfaceView` and applies each `SurfaceAction` as an **ordinary command** (a fader move is
`SetStripGain`, a touch groups the move into one undo step, a mute is `SetStripMute`…): a surface
can never do what the screen cannot, every move undoes like the mouse's, and automation
recording sees it as a mouse move (ADR 0027).

### 2. What a surface does

- **Strips.** A bank of eight strips per unit over the mixer's strips in the order the mixer
  shows them. *Bank* ◂▸ moves by every unit's eight (the main and its extenders), *channel*
  ◂▸ by one; the bank stops where the last strip is on the last channel; empty channels are
  dark and ask for nothing. Extenders (`mcu-xt1`…) continue the strips to the right of the main
  unit and follow its bank; they have no master, transport or jog.
- **Faders** are motorised in **the console scale of the mixer's own faders and of automation**
  (`automation::range::fader_db_at/fader_pos_of`: 0 dB at 80 % of the travel, −∞ at the bottom,
  +12 dB at the top — one definition, so a curve drawn at 0 dB plays 0 dB), 14 bits, set to a tenth
  of a dB. **A fader the hand touches is never moved by the host**; on release it lands where the
  mixer is; a unit's own move is not echoed back to it. The master is channel 8.
- **V-pot**: pan, 4 % a tick (a tenth of that with Shift), push = centre; its ring shows the pan
  (the middle LED and the centre light at centre). **Flip** swaps fader and V-pot (the fader
  edits the pan, the V-pot the gain in half-dB ticks, push = 0 dB).
- **Buttons**: *Rec* arms, *Solo*, *Mute*, *Select* (idempotent: the command carries the state it
  wants, computed from the view, so a missed release or a lost message cannot invert it). LEDs
  follow the mixer.
- **Scribble strips**: the top line the strip's name (six ASCII characters, anything else `?`),
  the bottom its gain (`-6.0dB`, `-inf`) or, after *Name/Value*, its pan (`L40`, `C`, `R12`).
  The 7-segment displays show the bank page and the position (bars.beats.sub.ticks or a
  clock), with the dots lit.
- **Meters**: 0–12 steps of 5 dB from −60 dB (a channel pressure per change).
- **Transport and keys**: play, stop, record, cycle, click, scrub, rewind/forward (held), the
  jog wheel (ticks to the host, which moves the playhead), marker, ◂ ▸ for the previous and next
  marker, save, undo (Shift+undo is redo).
- **Only what changed is sent**, and everything again after a refresh (a unit connected, a bank
  moved, flip, name/value). Tests assert a quiet mixer sends nothing.

**One undo step per gesture of the hands.** Every fader and V-pot moved from the first touch to
the last release is one undo step, however many faders are ridden at once and in whatever order
their messages interleave (an engineer riding two faders through a passage undoes the passage, not
one fader of it); a fader let go while another is still held keeps the gesture open. The step is
sealed only when the last touch ends (only its own step: a mouse drag going on is never sealed),
and moves with no touch held (a V-pot, a fader without touch sensing) end it after 600 ms of
stillness. The step grows as targets join, as a controller's does (`widen_merging`). In the
recording mode each move is asked of the mode's lock first (`locked_against`), as a controller's
is: a refused target does not move and says nothing at every message, and the others keep moving.

### 3. Mackie Control: the layout, and its source

**Source:** EMATech's *TouchMCU — Mackie Control protocol*
(<https://github.com/EMATech/TouchMCU/blob/main/doc/mackie_control_protocol.md>), the public
write-up of the Logic Control/MCU reference every host implements. From it, and held byte for
byte in `tests/external_surface.rs`:

- a strip's motor fader is its channel's pitch bend (14 bits; the master is channel 8);
- buttons and their LEDs are notes (0x7F on, 0x00 off, 0x01 blinking): Rec 0x00+, Solo 0x08+,
  Mute 0x10+, Select 0x18+, V-push 0x20+, bank ◂ ▸ 0x2E/0x2F, channel ◂ ▸ 0x30/0x31, flip 0x32,
  name/value 0x34, SMPTE/beats 0x35, transport 0x5B–0x5F (rewind, forward, stop, play, record),
  cycle 0x56, click 0x59, scrub 0x65, undo 0x51, save 0x50, marker 0x54, shift 0x46; the SMPTE
  and BEATS LEDs 0x71/0x72; fader touch 0x68 + strip, the master's 0x70;
- a V-pot turns as controller 0x10 + strip and the jog wheel as 0x3C: bit 6 set is
  counter-clockwise, bits 0–5 the amount ("Clockwise `0b00000001`, counter-clockwise
  `0b01000001`"); its ring is controller 0x30 + strip;
- a meter is a channel pressure `D0 sv` (strip in the high nibble, 0–0xC, 0xE/0xF the overload);
  **the unit lets a meter decay by itself (about 300 ms a division)**, so a lit meter is sent
  again every 150 ms ([`METER_REFRESH_MS`]) and a dark one is not;
- **the ten timecode digits are controllers 0x40–0x49 with 0x40 the rightmost, the two
  assignment digits 0x4A (right) and 0x4B (left)**; bit 6 of the value is the dot, bits 0–5 the
  character;
- the scribble strips are SysEx `F0 00 00 66 14|15 12 <offset> <ASCII> F7` (0 the top line,
  0x38 the bottom);
- the handshake: the host's *Device Query* (`00`); the unit's *Host Connection Query* (`01`, a
  7-byte serial, a 4-byte challenge) answered with the *Host Connection Reply* (`02`, the serial,
  the response by the document's formula, quoted in the test). A unit that never handshakes is
  driven all the same. A real unit's query arrives as SysEx from its port (§6).

### 4. HUI: the layout, and its source

**Source:** *MACKIE HUI MIDI protocol — the results of a 2-day reverse-engineering session by
theageman* (2010, © 2011 SSEI, www.ssei-online.de), published at
<https://forum.bome.com/uploads/short-url/aX7AWqj6FpgMiEMQi0kACDDTSSi.pdf> (Mackie never
published the protocol). Its pages, as cited in `external_hui.rs` and quoted in the tests:

| What | Bytes (page) |
|---|---|
| Zone/port, unit → host | `B0 0F zz`, `B0 2F yp` — y `40` pressed, `00` released (p. 9: "the solo button on the fifth channelstrip … b0 0f 04, b0 2f 43") |
| Zone/port, host → unit (LEDs) | `B0 0C zz`, `B0 2C yp` (p. 7) |
| Strip zones 00–07 | port 0 fader (touch), 1 select, 2 mute, 3 solo, 4 auto, 5 v-sel, 6 insert, 7 rec/rdy (p. 14) |
| Zone 08 keys | 1 shift, 3 undo, 7 save (p. 14) |
| Zone 0a channel selection | 0 ◂ channel, 1 ◂ bank, 2 channel ▸, 3 bank ▸ (p. 14; p. 7 "next bank … zone 0a, port 3") |
| Zone 0d cursor | 1 left, 3 right, 5 scrub (p. 14) |
| Zone 0e transport | 0 talkback, 1 rewind, 2 fast forward, 3 stop, 4 play, 5 record (p. 14) |
| Zone 0f | 0 return to zero, 1 end, 3 loop (p. 14) |
| Faders | `B0 0z hi`, `B0 2z lo`, value = hi << 7 + lo, both ways (pp. 7, 11) |
| V-pots / jog | `B0 4p vv` / `B0 0D vv`: **vv > 40 is +(vv − 40), vv < 40 is −vv** (pp. 10–11) |
| V-pot ring | `B0 1y vv`: 1–b one LED (6 the centre), + 40 the LED under it (p. 6) |
| Strip names | `F0 00 00 66 05 00 10 yy gg hh ii jj F7` (p. 4) |
| Ping | the host sends `90 00 00` about every second, the unit answers `90 00 7F`; without a ping for about 2 s it goes offline and its faders stop answering (p. 4) |

The host pings every [`PING_EVERY_MS`] (1 s) and says a unit **lost** after [`LOST_AFTER_MS`]
(3 s) without an answer (the surfaces' log names it); when it answers again it is sent
everything. Zones not mapped are read, ignored and **logged by zone and port**
(`Decoded::unmapped`), so one capture from a real unit extends the table.

### 5. The device list (settings), two clicks

Settings ▸ Surfaces (`views/external_surfaces.rs`, the kit): a list of units, each with its kind
(Mackie Control, Mackie extender n, HUI), its MIDI input and output ports (offered from what is
present; the pair of the same device pre-chosen by name, as controller feedback does), an on/off
switch, a *Test* that lights every LED and moves every fader, a log of the last messages with
unmapped HUI ports named, and *Add virtual surface* — the emulation connected to the app's own
host, so the whole setup can be tried with nothing plugged in. Saved in the machine's settings
(`settings.surfaces`, additive, `serde(default)`), not in the project: a surface belongs to the
studio, not the song.

### 6. Input path

A surface's input port is **taken whole** by the controller filter (`FilterSet::take_port`, an
additive method): none of its messages reach the engine as notes (a Mackie button's note 0x5E
must not play a key) or the generic map. Its channel messages arrive through the same per-port
ring and `Heard` stream as a mapped controller's (ADR 0062 §1). **Its SysEx too:** the MIDI
callbacks send everything from `F0` up to the clock follower, so a port taken whole keeps its
SysEx in a fixed buffer and a ring of its own (`external_sysex::route_sysex`, no lock, no
allocation) and wakes the interface, which hands it to the surface — a Mackie's *Host Connection
Query* is answered. Everything is decoded by the set on the interface thread and answered through
the shared output pool.

### 7. Faders without touch sensing

Every MCU and HUI senses touch, and the motors never move a touched fader. Some controllers in
MCU mode have no touch sensing: for them Settings offers *Touch-sensing faders* off, and a fader
the unit moves is held [`NO_TOUCH_HOLD_MS`] (600 ms) before its motor follows the mixer again —
said on the page, so nobody is surprised by motors fighting the hand.

*Not chosen:* a surface as a generic mapping preset (banks, motor feedback, touch and the displays
need a state machine); a thread per device (the interface thread already drains the rings every
frame).

## Consequences

- A Mackie surface (or its emulation) drives the mixer, the transport and the playhead; every
  action is a command with its undo.
- The emulation is a first-class citizen: the virtual units run the same tests as a real unit
  would and serve the settings page as a trial surface.
- A protocol fact wrong against hardware is a one-line table change, with a test that names it.

## What the tests hold

`tests/external_surface.rs` (core): every Mackie message as TouchMCU prints it (the timecode and
assignment digits on 0x40–0x4B, the V-pot sign bit) and every HUI message as theageman's notes
print it, page by page (the solo example of p. 9, the strip, keys, channel, cursor, transport and
loop zones of p. 14, touch and move of p. 11, the V-pot and jog encoding of p. 10); the host
connection reply by the document's formula; the ping every second and the loss after three, and
a unit back online sent everything; a steady meter re-sent before it decays and a dark one not;
faders without touch held after a move; the whole surface through the virtual units (names,
values, LEDs, rings, meters, displays, motor faders in the console scale and never against a
hand, banks, transport, jog, flip, name/value, an extender, only what changed is sent).
`cantus-engine` `external_sysex` tests: a surface's SysEx reaches the surface, another port's goes
to the follower. `cantus-ui` `tests/external_ui.rs`: a virtual Mackie drives the mixer through
commands (one undo step per fader move), a surface's port is taken whole, a Mackie connection query
from its port is answered through the service.
