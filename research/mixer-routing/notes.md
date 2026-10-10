# Mixer routing: FL Studio, Ableton, Bitwig, Logic, Studio One, Reaper (and Reason's cables)

Research for BACKLOG §6d.3 ("cabling between channels, as in FL Studio"), October 2026.
The design that follows from it is `docs/round25/mixer-routing-design.md`.

**Sources and versions.** Official documentation, read in October 2026: the FL Studio
online manual (no version stated; FL Studio 2025.1 release post of 9 July 2025 for what
changed), the Ableton Live 12 Reference Manual, Bitwig's "latest" user guide (screenshots
of 5.1–5.3; Bitwig says a rewritten guide follows 6.1), the Logic Pro for Mac guide (up to
12.3; Apple's pages were read as downloaded HTML), the Studio One manual — now published by
Fender as **"Fender Studio Pro 8.1"** — and *Up and Running: A REAPER User Guide v7.81*
(PDF, page numbers are the guide's). Reason 12.7's operation manual for the cable view.
Anything not confirmed in those pages is marked **unverified**. Look and strip anatomy
come from this repository's earlier research (`research/ui/*.md` and its screenshots).

---

## 1. FL Studio (the reference)

Pages: mixer `https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/mixer.htm`
(the routing content now lives here; the old `mixer_routing.htm` and `mixer_trackrouting.htm`
return 404), mixer menu `…/html/mixer_mixermenu.htm`, track properties
`…/html/mixer_trackprops.htm`, Fruity Limiter `…/html/plugins/Fruity%20Limiter.htm`, the
wrapper `…/html/plugins/wrapper.htm`. Screenshots: `research/ui/screenshots/fl-studio/05-mixer-annotated-anatomy.png`,
`10-mixer-routing-send-icons.png`.

| Feature | FL (manual's words where quoted) | For Cantus |
|---|---|---|
| **Routing iconography, relative to the selection** | the selected track shows a green ▼ ("audio leaving") with cables coming off it; each possible destination an ▲ (click = send); small ▲ where another track already sends; a source of the selected track shows a **greyed** switch "to prevent feedback loops" | **have** the arrows (`mixer_parts.rs:32`); add the cables |
| **Cables** | drawn to the selected track's destinations; a cable to the Master "drops off the bottom of the mixer rather than making a direct connection"; Mixer menu ▸ Options ▸ View ▸ **"Routing cables"** shows or hides them (beside "Lines between tracks", "Names at top", "Waveforms") | **take**: our master route is implied the same way; Off/Selected/All |
| **Kinds in the line** | "A sidechain send is identifiable by any wire that is low contrast in color"; "Grayed out cables indicate the volume control is turned to minimum" | **take, stronger**: kinds by stroke and end mark, not only contrast |
| **Every route has a level** | after a send is switched on, a **knob on the destination track** above its switch "controls the signal level received from the source track"; at 0 the link stays and passes nothing | our sends have this (the knob in the send row); our output is unity — §8 |
| **Create** | left-click a destination's switch = send from the selected track; **right-click = "route OR sidechain to a specific track with master routing disabled"** | click = send, Ctrl+click = route the output (have) |
| **Master by default, "only" variants** | every track goes to the Master unless set; a plain route **adds** a send and keeps the Master; "…only" removes the Master send | Ctrl = set the output; never removes sends (see pitfalls) |
| **Track routing submenu** | Route selected to this track (only) · Sidechain selected to this track (only) · Create submix… · Route/Sidechain this track to selected (only) | **have** route/send selected here, create submix |
| **Sidechain** | "Sidechain to this track" is "a macro that creates a send with volume set to 0"; Fruity Limiter's selector lists only tracks already sidechained to its track; the wrapper auto-maps sidechained tracks to plug-in inputs in track order | ours is a key chosen on the effect (ADR 0017); no zero-level send needed |
| **Pre-fader** | sends are post-fader; pre-fader needs the Fruity Send plug-in in a slot | piece F would do better |
| **Multi-select** | Ctrl+Shift+click/drag labels or meters, then Ctrl+Shift+right-click the target ("this track") | **have** (Ctrl/Shift-click selection) |
| **Channel rack** | Route selected channels to this track (Ctrl+L), … starting from this track (Shift+Ctrl+L), 'TRACK' auto-assign | **have** route picker and *first free strip* |
| **Find the other end** | Select ▸ Channels routed to this track (Alt+L), Tracks this one is routed to, Tracks routed to this one (Ctrl+Alt+L); Reset selected track(s) routing | add *select routed to / from* to the route menu |
| **Current (C) track** | a utility track receiving the selected track's output | not taken |
| **PDC** | "APDC also applies to inter-track routing, … and sidechains" | ours compensates sends; keys deliberately not (ADR 0018) |
| **FL 2025.1** | up to 500 dynamic tracks, undoable deletion, "Unroute tracks routed to the selected track(s)"; no change to cables | — |

**Unverified**: whether cables take the track colour; any cable style setting; the FL
version that introduced cables.

## 2. Ableton Live 12

Pages: `https://www.ableton.com/en/manual/routing-and-i-o/`, `…/mixing/`,
`…/live-audio-effect-reference/`.

- **In/Out section** — "Live's 'patchbay'"; shown from View ▸ Mixer Controls or
  Ctrl+Alt+I. *Audio From / Audio To* chooser pairs (type, then channel or tap); any track
  can take another's output, at **Pre FX / Post FX / Post Mixer**.
- **Sends Only**: a track's output can be *Sends Only*; the return then "will then act as a
  submixer channel".
- **Returns**: send knobs feed return tracks; "every return track has a Pre/Post toggle";
  sends *on* return tracks are **disabled by default** (a return can feed itself), enabled
  by right-click.
- **Groups**: Edit ▸ Group Tracks sets members' outputs to the group unless they already
  went elsewhere than Main.
- **Device sidechain**: built-in devices carry their own routing choosers (Compressor:
  Sidechain on, source, gain, dry/wet, EQ, Listen).
- **No lines between tracks**: routing exists only as chooser text.

## 3. Bitwig Studio

Pages: `https://www.bitwig.com/userguide/latest/the_mix_view/`, `…/recording_clips/`,
`…/intro_to_tracks/`, `…/routing`; sidechain tutorial `https://bitwig.com/learnings/sidechaining-tutorial-49`;
6.0 release notes. Repository: `research/ui/bitwig-studio.md` 414, 486-487, 506, 529-530, 579-593.

- **Track I/O** choosers in the header and the mixer's I/O section; output defaults to
  Master; an audio track's input lists "the audio outputs of all other tracks".
- **Sends**: "a level knob for each FX track"; modes Pre, Post and **Auto** (the FX track
  decides); **ring colour = mode (yellow post, blue pre)**; click the name to toggle,
  Shift-click toggles all; *Disable All Unused Sends*; new projects since 4.3 start with
  sends disabled and the first touch enables one.
- **FX track send-receive pane** (repository research): the FX track lists every track
  feeding it with a meter and the amount — a mixer for one bus.
- **Audio/Note Receiver** devices import any source; **sidechain** choosers on devices list
  every track, pre or post fader, down to pads and chain points.
- **Modulation routing** (repository research): click a routing button, targets light up,
  drag sets the amount, right-click lists routings with ✕ — the clearest connect gesture.
- **No cables between tracks.**

## 4. Logic Pro

Pages: sends `https://support.apple.com/guide/logicpro/lgcp8ea0091c/mac`, auxes
`…/lgcp8e7db552/mac`, subgroups `…/lgcp8e8310ed/mac`, strip controls
`…/channel-strip-controls-lgcpbc219210/mac`, plug-in window `…/work-in-the-plug-in-window-lgcpbc21a1fd/mac`,
track stacks `…/track-stacks-overview-lgcp9bc4b63d/mac`.

- **Sends**: up to 12 per strip; Post Pan (green ring), Post Fader (blue), Pre Fader (blue,
  the knob moves left of the slot); the send menu has a **search field** and shows named
  auxes; **Shift-click a send and its aux flashes three times**; *Select Channels with Same
  Destination*.
- **Auto aux**: "An aux channel strip is created automatically when a send assignment is
  made", unless that bus is already some strip's input.
- **Subgroups**: select strips, choose a bus in one Output slot — all outputs change, aux
  created.
- **Signal flow**: the inspector shows the selected strip and, on its right, its routing
  destination; the mixer's Single view shows a strip's signal flow (the latter from Apple's
  search summary, **unverified** in the text).
- **Side chain**: plug-in header pop-up with search; the slot then shows a left-pointing
  arrow and a help tag naming the source.
- **Track stacks**: folder (VCA-like) or summing (bus); a subtrack routed outside its
  summing stack stops following it.
- **No cables in the mixer.**

## 5. Studio One (Fender Studio Pro 8.1)

Pages: `https://fenderstudiopromanual.fender.com/en/Content/Mixing_Topics/The_Console.htm`,
`…/Effects_Signal_Routing.htm`.

- **I/O**: every channel to Main Out by default; several selected channels take an input or
  output at once.
- **Buses and FX channels show a count of their sources**; clicking it lists them and jumps
  to one.
- **Sends**: the "+" lets you "select an existing destination Channel, add a new one, or make
  a Sidechain connection"; per send a Prefader button, level −∞…+10 dB, pan locked to the
  channel by default; "always post-inserts".
- **Create**: Add Bus / Add FX; **"Add Bus for Selected Channels"**; dragging an effect into
  empty console space makes an FX channel; **dragging an effect onto a channel's send slot
  makes the FX channel and the send**; Alt-drag moves a send between racks, plain drag
  copies.
- Buses nest without limit; loops are blocked; *Hide Sources* on a bus.
- **Sidechain** is a send to an insert's sidechain input, picked from the plug-in's menu.
- **Listen Bus**: an independent monitor feed. The Channel Editor's "Routing" view is for
  parallel chains inside an FX Chain, not between channels. **No cables between channels.**

## 6. Reaper (v7.81)

Guide: `https://www.reaper.fm/userguide/ReaperUserGuide781c.pdf`.

- **Route button** (§17.2): three stripes — green = master/parent send, yellow = has sends,
  blue = has receives; hover lists them; click opens the Routing window; "**click and drag
  from this button to another track to create a send**".
- **Routing window** (§2.22): master/parent send, sends, receives, hardware audio and MIDI
  outputs; each send with volume, pan, audio/MIDI. Send types (§17.3): Post-Fader (Post-Pan)
  default, Pre-Fader (Post-FX), Pre-Fader (Pre-FX). Channel mapping and plug-in pins (§17.4).
- **Drag modifiers** (p. 47, 210): drag = new send; **Ctrl+drag = new send and the source's
  master send off**; **Alt+drag = several sources into one destination (submix)**;
  Shift+drag = copy the sends of all selected tracks. On a send: Shift+click mutes, Alt+click
  removes, right-click menu (Mute, Remove, parameters, **Go to send destination track**).
  "Show sends when size permits" draws each send in the strip with a small level knob.
- **Routing Matrix** (§2.23, §17.13; View ▸ Routing Matrix, Alt+R): sources × destinations;
  click a vacant cell to create; right-click for parameters; **drag a send down a column to
  copy it**; the cell's symbol shows the send type and **a bar's height the level**; filters
  for master/parents, tracks, hardware, MIDI; one window also switches to Track Wiring and
  the grouping matrices.
- **Track Wiring view** (§2.26) — the nearest thing to cables between strips: straight lines
  for master sends, **curves for track-to-track sends**; movable boxes; **drag-to-create from
  tap-specific handles** (first FX = pre-FX, Fader = pre-fader, Master = post-fader); declutter
  options "**Only show wires on track on mouseover**" and the same for hardware wires; the
  guide sells it for finding "any routing you may have created and forgotten about".
- **Sidechain** (§17.5): a 4-channel target, a send to channels 3/4, the detector set to the
  auxiliary input; Alt+drag onto an FX sends straight to it (garbled table — likely).
- **Feedback** blocked unless allowed per project, which disables PDC.

## 7. Reason (the classic cable view)

Reason 12.7, `https://docs.reasonstudios.com/reason12/routing-audio-and-cv`.

- Tab flips the rack; **colour by signal type** (audio reds, effects green, CV yellow,
  Combinator blue).
- **Reduce Cable Clutter** [K]: *Show for selected devices only* (others transparent),
  *Hide auto-routed cables*, *Hide all cables* (coloured dots in the jacks) — and patching
  still works with cables hidden.
- Hovering a jack names the far end; *Scroll to Connected Device*; a jack's menu lists every
  device (unsuitable greyed, occupied marked) and its first entry for an audio output is
  "new Mix Channel".
- New devices are auto-routed; **drag a cable end into empty space to disconnect**.
- (Tracktion Waveform's rack wiring is described only in third-party posts — unverified.)

---

## 8. What Cantus takes

| Idea | From | In the design |
|---|---|---|
| Arrows relative to the selection, one click | FL | kept: the send row, "FROM SELECTED" |
| Cables, the master route "dropping off" rather than drawn | FL, Reason's *hide auto-routed* | arc band; master route implied except for the selection |
| Kind in the line, level visible | FL (contrast), Reaper (straight vs curved), Reason (colour), Logic/Bitwig (tap by ring colour), Reaper matrix (bar height) | stroke + end mark per kind; a level pill on sends; the matrix cell's arc = level |
| Declutter | Reason (selected only, hide all), Reaper (wires on mouseover) | Off · Selected (default) · All, hover lights |
| Drag from strip to strip, modifiers | Reaper (Ctrl = also cut the master send, Alt = many into one, Shift = copy), Reason, Bitwig's modulation grammar | plain = send, Ctrl = output, Alt = key; a multi-selection routes together |
| Disconnect by dropping an end on nothing | Reason | yes |
| Matrix | Reaper (click cell, right-click parameters, drag down a column to copy, filters) | the Routing tab, with Shift+drag down a column to copy a send |
| Find the other end | Logic (Shift-click flashes the aux), FL (select routed to/from), Reaper (go to destination), Reason (scroll to connected) | *Show the destination*, *Select routed to / from*, edge chips, hover lighting |
| Route state on the strip | Reaper's route-button stripes, Studio One's source count | the port: filled when the strip goes anywhere but the master; a source count on buses' ports |
| Receives as a list | Bitwig's send-receive pane, Reaper's receives, Studio One's source list | inspector *Receives* |
| Create the destination from the send | Logic auto-aux, Studio One (new channel from "+", effect dropped on a send slot), Reason ("new Mix Channel") | *New return ▸ Empty / Reverb / Delay*, named |
| Named send slots with level | Logic, Studio One, Bitwig, Reaper "show sends" | Extra large send list |
| Pre/post per send | Bitwig, Logic, Reaper, Studio One, Ableton (per return) | piece F (optional, schema 23) |
| One output + sends | Ableton, Bitwig, Logic, Studio One, Reaper | kept |

## 9. What Cantus avoids

- **FL's "route only" deleting effect sends** — a documented complaint: making a submix with
  "…only" also removes the tracks' effect sends, and users ask to "lock" them
  (`https://forum.image-line.com/viewtopic.php?t=302758`); an accidental "route only" is hard
  to see (`…?t=112621`). In Cantus, Ctrl sets the **output** and never touches sends.
- **FL's sidechain as a zero-level send** shown only by low contrast, and a limiter whose key
  list is empty until such a send exists. Ours is chosen on the effect and drawn as its own
  kind.
- **FL's many-routes-with-levels model**: a schema, engine, solo and PDC change for what sends
  do, and "where does this go?" with several answers.
- **Routing only as chooser text** (Ableton, Bitwig, Logic, Studio One): invisible until looked
  for.
- **Reaper's sidechain through channels 3/4** and feedback switched on per project.
- **Auto-created unnamed auxes** (Logic): ours are named and placed in the right dock.
- **Sends disabled by default and enabled on first touch** (Bitwig ≥ 4.3) — surprising; our
  new send is live at its default level (open question 3 is the level, not the state).
- **Sends Only / no master send** (Ableton, Reaper): its common use is a ghost track that only
  keys a compressor; Cantus keys are post-fader and **pre-mute**
  (`crates/cantus-engine/src/graph/strip_run.rs:247-258`), so a muted strip still keys.

## 10. Two facts about Cantus found on the way

1. "Nothing feeds this strip yet" (`views/mixer_strip.rs:579-591`) counts only **tracks**, so
   a return fed by sends — every demo's Reverb and Delay — claims nothing feeds it. That is
   what the owner saw.
2. `RemoveStripSend`'s inverse appends the send (`cantus-core/src/command.rs:1296`, `:1310`):
   undoing the removal of a send that was not the last one changes the order of
   `MixerStrip::sends`. Suspected from reading, not run; the design's first test.
