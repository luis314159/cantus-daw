# ADR 0014 — The mixer has strips of its own; a track feeds one

**Status:** Accepted · **Date:** 2026-09-10 · **Extends:** ADR 0012 (pan law), ADR 0013 (channel rack)

## Context

Until schema 8 a track *was* a mixer channel: its fader, pan, width, polarity and inserts
lived on the track, and the mixer drew one strip per track. That is the simplest model
and it was right for a one-instrument DAW. It stopped being right the day the channel
rack made every drum its own track: a kick and a snare on their own tracks could not share
a compressor, a group of tracks could not be turned down together, and the mixer showed
exactly as many strips as there were tracks — a desk that grows and shrinks under you.

The user's ask was FL's shape: many mixer channels, numbered, present whether or not
anything feeds them, more on demand, the master on the left, and any channel routable
into any other.

## Decision

**A `MixerStrip` is a thing of its own** (`Project::strips`, numbered from 1 in the order
the mixer shows them). A project opens with sixteen; `+` adds one; a strip nothing feeds
can be removed. The strip holds what belongs to a *channel of the mixer*: fader, pan
through the pan law, width, polarity, mute, the inserts, a colour and a name of its own —
and `output`, which is the master or another strip.

**A track feeds a strip** (`Track::strip`, `None` for the master). Several tracks may feed
one strip, which is a bus; a strip may feed another, which is a group. The track keeps a
level and a pan of its own — FL's channel volume — at unity by default, and its pan is a
*balance* (unity at centre), not the law: the law is applied once, at the strip, and
applying it twice was the master-pan bug ADR 0012 records.

**A new track takes the first strip nothing feeds**, so the desk fills in left to right
and the rack's strip number is meaningful the moment a track exists.

**Processing order is computed once, off the audio thread.** `Project::strip_order` is a
topological sort — every strip after all the strips feeding it — carried in the snapshot
as `strip_order`. The audio thread runs strips front to back and never sorts. Cycles are
refused by `SetStripOutput` (`routing_would_cycle`); a hand-edited file that holds one
still plays, with the cyclic strips sent to the master.

**Every strip has its buffers preallocated** (`MAX_STRIPS` × the largest block, a few
megabytes), so routing can change with a snapshot and nothing is allocated on the audio
thread for it (ADR 0005). A strip nothing fed and with no inserts costs one comparison.

## The migration is bit-identical, and a test holds the hash

Schema 8 → 9 makes one strip per track and **moves** the track's gain, pan, width,
polarity and inserts onto it, leaving the track at unity; then pads the desk to sixteen.
The signal meets the same operations in the same order with two multiplications by 1.0
in front — the track's unity gain and its centre balance, both returned as exact `1.0`
rather than computed. `a_pre_strip_project_renders_exactly_as_it_did` renders `v5.cantus`
and compares a SHA-256 of the samples against the value recorded before the strips
existed. It passed first time, which is the only kind of evidence this claim deserves.

One behaviour did change, on purpose: a muted track now sends **silence into its strip**
rather than being zeroed after the inserts, so a reverb tail rings out when a track is
muted instead of being cut. In a render from the start the two are identical; live, the
new one is what a console does.

## Consequences

- `SetTrackWidth` / `SetTrackPhaseInvert` became `SetStripWidth` / `SetStripPhaseInvert`;
  the effect commands are keyed by `StripId`; `SetTrackStrip`, `InsertStrip`,
  `RemoveStrip`, `SetStripName/Gain/Pan/Mute/Output/Color` are new. The undo fuzz runs all
  of them, including the routes the command must refuse.
- `Meters` carries both `tracks` (a track's own output, for the arrangement header) and
  `strips` (post-fader, for the mixer).
- The inspector shows the track's **Channel** (level, balance, *Mixer* route) and, further
  down, the **strip** it feeds with the inserts. The rack's channel button shows the strip
  number, or `M`, and opens the route picker. The mixer selects a strip and, when a track
  feeds it, the track.
- Sends (a tap into a strip that also continues to its output) were the next thing this
  model wanted; they landed in ADR 0015 as `MixerStrip::sends`, one more edge in the
  same routing graph and one more loop in the processor, as predicted.
- Track groups in the arrangement, when they come, must be **this tree** (the playlist
  plan's decision 3) — a group is a strip that its members feed.
