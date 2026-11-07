# Saltator: a character that dances to your song

*[Back to the manual](README.md) · [Leer en español](../es/dancer.md)*

**Saltator** is an instrument that makes no sound: it draws a character that dances to
your music. Put it on a track and it dances to the beat at once, with nothing to set up;
then make it yours — choose the character, automate its moves bar by bar, play moves from
a MIDI clip or your keyboard, let it react to the drums, put it on stream. Where it shows
(over the workspace, on the Stage tab, in the companion window, the streaming mode) is
[the stage's page](stage.md); a clip of it with the song's audio is [the video
export](video.md); your own characters are [importing a character](import-characters.md).

## Add one

*Add track* (or the device picker, Ctrl+T) ▸ the **Visual** shelf ▸ **Saltator**. The new
track is named *Saltator*, has a dancing figure for its icon, and Fuga, the cast's ninja,
dances on it. Its window opens.

**The cast.** Fuga the ninja (masked, a torn crimson scarf in the wind: flips, high kicks,
a crouched landing, a martial bow) and Volta the mecha (gunmetal armour, a V visor of
light whose slits show its mood, a speaker core that pulses on the beat, jets that fire
when it jumps: pop and lock, the rocket jump, a hero stance) lead it; then the family —
Trino the songbird, Miso the cat, Lana the lamb, Tic the metronome robot, Corchea the
eighth note, Sol the child. A song made before Fuga keeps the character it had: a dancer
saved without one keeps Trino.

## The window

**Play** is a little theatre:

- **The stage** — the character dancing live, exactly as you hear the song (an output's
  latency is taken into account), with a spotlight that follows it and swells on the
  beat. At the top: the bar and the beat, and the move it is dancing.
- **Character** — every character Cantus includes, each dancing its own routine, and every
  character imported into the project. Click one to choose it (one undo step). Under them:
  its name, its line and its licence. **Import** brings your own (a Lottie, a sprite sheet
  or a DragonBones character: [importing a character](import-characters.md)); for an
  imported one, **Edit moves…** chooses which of its animations plays each move.
- **Export video…** (the stage's title bar) — the song with its dancers as a video: MP4 or
  WebM, square, vertical or a transparent layer ([exporting a video](video.md)).
- **Moves** — the move map as sixteen pads, one per key from **C3** to **D#4**. The move
  dancing now lights up. Press a pad to dance that move at once (a live note, as on the
  keyboard).
- **Dance** — *Move* (*To the beat*, or one of the sixteen moves by name), *Travel* (*In
  place*, *Walk left*, *Walk right*, *Wander*), *Speed* (¼× to 4× the beat), *Energy*.
- **Body** — *Lift* (how far off the ground), *Facing* (left, front, right), *Spin* (°),
  *Size* (×).
- **Look** — *Mood* (Happy, Excited, Calm, Surprised, Sleepy, Cool, Shy, Neutral) and
  *Colour* (turns its colours around the wheel; eyes and outlines stay).
- **Ears** — *Reacts to*: what it listens to. Click the field (or its ‹ ›) to go through
  *Nothing*, *The whole song* and each channel of the mixer. *Reacts* says how much of its
  energy the level decides.

**Edit** has every number as a card, and **Where it shows** (*Floating*, *Docked*,
*Hidden*; *Always on top*) and its position on the stage.

Every number is an ordinary parameter: right-click it for *Automate*, *Learn MIDI
controller* and the modulators.

## How it dances

- **To the beat, by itself.** With *Move* at *To the beat* it dances its own routine, one
  move a bar, every step landing on the beat — no automation needed.
- **Automation.** Draw an automation clip of *Move* and it dances the move you chose from
  the clip's first bar line to where it ends (between clips a value holds; end the clip on
  *To the beat* to hand back to the routine). Every number can be automated the same way.
- **Notes.** A note on a key of the move map plays that move from the note's start: held,
  it goes on; let go, it finishes the step it is in. Velocity is how big it dances. Notes
  in a session clip move it too.
- **Modulators.** An LFO on *Colour* cycles its colours; a step sequencer on *Move* changes
  its moves every step; learn the mod wheel on *Energy* and it dances bigger as you push it.
- **React to.** Choose a channel in *Reacts to*: a follower listens to it and the dancer
  dances bigger when it is loud. It is one undo step; choose *Nothing* to stop.
- **MIDI learn.** Right-click any number ▸ *Learn MIDI controller* and move a knob; a fader
  on *Move* goes through the moves by name.

## Presets

*To the beat*, *Party*, *Chill* and *Cool* set how it dances. A preset never changes who
dances or where: the character, its place, its size and where it shows stay.

## Agents

An AI agent sees Saltator in the unit catalogue (`get_unit dancer`: every number, the moves
and moods by name, the move map, the characters). It can add one (`add_track` with kind
`dancer`), automate its *Move* and set its numbers like any instrument's.
