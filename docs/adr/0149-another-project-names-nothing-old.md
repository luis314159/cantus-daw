# ADR 0149 — Another project in place names nothing of the old one

> **Provisional number** (branch `r41/effect-ids`): renumber at the merge if 0149 is taken.

**Status:** Accepted (the owner asked for it first in the next version, 2026-11-19) ·
**Date:** 2026-11-20 · **Builds on:** ADR 0004 (commands), ADR 0019 and 0028 (hosted units
delivered by the host), ADR 0026 (the master's chain), ADR 0073 (hosted views) · **Amends:**
the engine handle's bookkeeping of units since round 22

## Context

Every id in a project (`EffectId`, `TrackId`, `StripId`, `ClipId`, pads, modulators, external
units) comes from one counter that starts again at 1 in each project. Many things outside
the project keep ids: the engine handle's `known_effects` and `known_instruments`, the
plug-in host's loaded instances and parked states, and the interface's windows, A/B memories,
selections, clipboards and modes. When another project took the open one's place, those
survived and named the new project's objects of the same id.

The owner found it on beta.15. He opened Aurora after Neon Tide, and Aurora's master EQ
showed no input and ignored its controls. Neon Tide's id 35 was a strip's compressor, and
Aurora's master EQ is also 35. `known_effects`, a set of bare ids, said "delivered", so no
unit was sent. The master's takeover looks at its own old units and then the incoming ones,
found neither and skipped the insert.

Read across every path, the same flaw has more faces:

- **On a strip,** the same id in the same place took over the old unit before an incoming
  one. A unit of another kind, or carrying another song's state (a delay line, a reverb
  tail), was configured with the new spec.
- **Instruments** were kept by `(track, kind)`, so another song's unit played the new
  song's track.
- **Hosted plug-ins** were kept by slot id with no check of which plug-in they were, so
  another plug-in's state could be loaded into one. A state parked for the id went into
  the new project's plug-in.
- **In the interface,** the audit of this round found about twenty more. The worst ones:
  - a session-view performance ending into the next project;
  - the effect and instrument A/B memories applying another song's settings;
  - the EQ and compressor windows drawing over an insert of another kind;
  - a mix-recorder take that kept recording the new project's strip;
  - *Last tweaked*, the session clipboard and a modulator's mapping mode acting on the new
    project's objects;
  - a hardware ping's latency laid on the new project's external unit.

The flaw has been there since round 22, and restarting Cantus cleared it. Beta.15 shipped
with it listed as a known issue.

## Decision

1. **The engine knows an insert by its id, its place and its kind**, not by its id alone.
   - `known_effects` is a map from id to (strip or master, kind). A slot that comes back
     with another place or kind is another insert: it is forgotten and sent fresh.
   - A unit the host delivers is known by its id and kind, and placed by the snapshot
     that holds it.
   - A garbage unit forgets its entry only when it is of the entry's kind.

   This covers every path that puts another project in place, including the ones that keep
   the old project in the history (*Replace project* from an import, a recovery or the
   AI's door, and its undo and redo). None of them needs to tell the engine anything.
2. **A takeover prefers the delivered unit and never takes another kind** (strips and the
   master: `Processor::take_effect`; instruments: `swap_native`).
   - The handle sends a unit only for an insert it does not know, so a delivered unit is
     always newer than a held one with its id.
   - A held unit of another kind is never configured with a spec it was not built for.
3. **Opening, *New*, a demo or a recovery** forgets everything the engine and the host
   hold. These are the paths where the old project is gone, all of them through
   `CantusApp::reset_bus_to`.
   - `EngineHandle::forget_project` forgets:
     - the units: effects, Tabula's pedals, instruments, dancers' feeds;
     - a snapshot that waited (it was the old project's), and the last one sent (nothing
       is cut or kept against it);
     - the key heard, the analyser and the scopes watched.

     It then releases every voice, and the next snapshot sends every unit fresh: nothing
     of the old song is taken over or heard.
   - `PluginHost::forget_project` lets every instance go with nothing parked, and forgets
     what was parked, missing, resting or given back for an id. The next sync loads each
     plug-in with the state the project holds.
4. **The interface has one place for "another project".**
   - `CantusApp::before_other_project` runs before the replacement. It ends the session
     performance into the old project, stops a mix-recorder take and seals a control
     surface's gesture.
   - `CantusApp::other_project` runs after it. It moves `epoch` and lets go of every
     window and memory that names an id (the list is in the code).

   Every replacing path goes through both: `reset_bus_to`, the *Replace project* command
   in `exec` and at the AI's door, and its undo and redo.

## Consequences

- Aurora after Neon Tide, and every demo after every other, runs exactly the inserts it
  runs when opened alone.
- Opening another project reloads its hosted plug-ins: one more load each, which is what
  every DAW does.
- A *Replace project* and its undo keep the plug-in host's instances where the id and the
  plug-in agree (the new project's state is loaded into it). **An instance of another
  plug-in under the same id never serves it:** `PluginHost::sync` lets it go with nothing
  parked. A parked state remembers its plug-in (format and id) and goes back only into
  that plug-in. A plug-in's own state that moved no parameter and was not saved is not
  carried across a *Replace project* to another plug-in of the same id: it was never that
  slot's.
- A future cache keyed by a project id must be cleared in `other_project`, or be checked
  against the epoch.

## What the tests hold

`crates/cantus-ui/tests/project_ids_e2e.rs`, on the boot path (`CantusApp::assemble`, the
real engine handle and processor):

- **Every demo after every other** (the 42 ordered pairs, each played a moment first). The
  engine runs, chain by chain, the units the demo runs when opened alone, with no insert
  skipped. The pairs share ids in another place or kind; the test fails if the demos stop
  colliding.
- ***Replace project*, its undo and its redo**, on the pair with the most collisions: each
  side runs its own inserts.
- **Nothing of the old song is heard.** A delay full of echoes, then the same song without
  its notes (same ids, places and kinds): the next project is, to the bit, what it is when
  opened alone.
