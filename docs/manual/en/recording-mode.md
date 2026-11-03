# The recording mode

*[Back to the manual](README.md) · [Leer en español](../es/recording-mode.md)*

A big session can make the computer work so hard that you cannot lower the audio buffer
without crackles. The **recording mode** takes the load away before you record: it plays
every strip that feeds the master from a recording of itself — or, if you choose, the whole
mix from one recording — so while you record the computer only reads audio and adds it up.
The strip you record on stays live.

**What it does, and what it does not.** It does not shorten your audio interface's own
round trip (the buffer, the driver and the converters). It lets you set a smaller buffer
without dropouts, because the computer has little left to calculate, and it keeps the track
you play on free of the delay other tracks' plug-ins add. The bar shows the buffer, the
round trip (as your driver reports it, or measured when Cantus has measured it), and the
processor load before and since.

## Enter it

1. **Arm** the track or tracks you are going to record on. Armed tracks, and the strips
   they go through, stay live. (You can arm another one later: see below.)
2. Open the **record options** (the small arrow beside the record button) and press
   **Prepare to record**.
3. Cantus shows what it will do: how many strips and tracks it renders, how many renders
   are already on disk, **up to how much disk the new files take** (each render is the song
   plus a tail of up to 30 seconds; a 5-minute stereo strip is at most about 120 MB), and
   what stays live and why (an armed track, a track that plays outboard gear, which a
   recording made off line cannot play).
4. Choose the **master**:
   - **Live** (the default): the master chain runs on the sum.
   - **Baked**: the whole mix — all but what stays live — is one recording, the lightest of
     all. Nothing of the master or the mix can move while the mode is on.
5. Choose how the **mix** behaves (with the master live):
   - **Live** (the default): each strip's fader, pan, mute and solo still work, so
     "more guitar in my headphones" needs no exit. A strip that goes into a baked bus, or
     sends post-fader into a baked reverb or delay return, keeps its mix locked, because
     that recording already holds its fader — the dialog says how many. A pre-fader send
     does not hold the fader: such a strip keeps its fader and pan.
   - **Locked**: nothing of the mix moves until you leave.
6. Choose what happens to a strip that cannot be rendered (for example a plug-in that
   cannot render offline): it **stays live and is named**, or the preparation **stops**.
7. Press **Prepare**. A bar appears across the top with its progress and **Cancel**.

Each strip is rendered after its effects and automation and before its fader and pan, so
with the controls untouched the sound is **bit for bit** what you hear normally — with the
master baked too. A bus is one recording that includes everything it sums.

The mode cannot start while the tempo follows Ableton Link, MIDI clock or time code, or
while a session scene plays: the recordings would play at another speed. If one of them
starts while the mode is on, the mode is left (and a take stopped).

## While it is on

A bar you cannot miss says **RECORDING MODE** (and *Master baked* when it is), how many
strips are baked, the buffer, the round trip, the processor load before and since, how many
strips stay live, and which inserts low-latency monitoring leaves out. Low-latency
monitoring is on while the mode is on (the record options say *forced by the recording
mode*). Strips that are baked look frozen.

Everything that would change the sound is refused with *"Recording mode: leave it to edit"*:
notes, clips, effects, instruments, automation, routing, the tempo, adding or removing
tracks, pattern mode, and undo or redo of those — on every strip but the armed ones, even a
strip that stayed live for another reason. Names, colours, markers, the loop and the
metronome stay editable; so do the armed tracks and their strips, a dancer's settings (a
dancer makes no sound), and the master while it is live.

**Arming another track** whose strip is baked asks you whether to bring that strip live:
its own chain plays again at once, aligned to the sample, and nothing is rendered again.
Only the recordings that hold that track come back live. With the master baked, the master
plays live again too (it held that track).

Record as always. Your new take lands on the live track and plays through its own strip.

You do not need to do anything to keep your project safe: entering and leaving the mode are
not undo steps and do not mark the project as changed, and the project file is **never
written in the mode's state**. If the program or the computer stops, you reopen the project
out of the mode, and the renders are still on disk.

## Leave it

Press **Leave** in the bar (or the record options). Choose:

- **Leave, keep files**: everything plays live again. The renders stay, so entering again
  renders only the strips that changed.
- **Leave, discard files**: the same, and the renders are deleted to free the disk.
- **Stay**.

You cannot leave while a take is being recorded: stop first.

## For an AI agent

`prepare_to_record` (with `master`, `mix_controls`, `on_failure`; with `dry_run` it only
answers the plan and the disk it needs), `get_recording_mode` and `leave_recording_mode`.
An agent that arms a baked track brings its strip live. See [What an AI agent can do in
Cantus](ai-tools.md).
