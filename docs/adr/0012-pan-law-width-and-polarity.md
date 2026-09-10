# ADR 0012 — The pan law belongs to the project, and what the four laws are

**Status:** Accepted · **Date:** 2026-09-09 · **Extends:** ADR 0006 (project format)

## Context

Cantus panned tracks with one hard-coded expression: an equal-power taper on the far
channel, with the near channel clamped so it never rose above the fader. That is a
defensible control, but it was never a *decision* — nobody chose it, and nothing recorded
what it was.

The moment a second option exists, the question is not "which is better" but **where the
answer lives**. A pan law changes the level of every panned track in a piece. Two people
opening the same file, or the same person on a second machine, must hear the same mix, or
the file does not mean anything. That rules out the settings dialog, which follows the
machine (round 8), and rules out a compile-time constant, which would silently re-balance
every existing project the day it changed.

## Decision

**The pan law is a field of `Project`.** It is written to the project file, it is undoable
like any other change (`Command::SetPanLaw`), and it is shown on the master strip in the
mixer — where somebody mixing looks for it — rather than in Settings.

Four laws ship. The name of each is how much a **centred** track is attenuated, which is
the convention Logic and Pro Tools use; all four agree at the extremes, where hard left is
the whole signal in the left channel.

| Law | Centre | Constant | What it is for |
|---|---|---|---|
| `0` | 0 dB | neither | Compatibility: exactly what Cantus did before this ADR |
| `-3` | −3.01 dB | power (`L²+R²`) | **The default.** Music heard on two speakers |
| `-4.5` | −4.52 dB | the geometric mean of the two | The hedge; Pro Tools's default |
| `-6` | −6.02 dB | amplitude (`L+R`) | Anything that will be summed to mono |

Constant power is right when the two channels are heard as a stereo field, which is how
music is listened to. Constant amplitude is right when they will be summed, which is what
a mono club rig, a phone speaker and a lot of radio do. There is no law that is right for
both, which is why this is a decision a musician makes per piece rather than one we make
once for everybody.

## `0 dB` is a compatibility guarantee, not a compromise

Every project written before this ADR migrates to `0` (schema 4 → 5), and
`PanLaw::ZeroDb::gains` is the old expression verbatim — there is a test that sweeps two
thousand pan positions and asserts the two agree *exactly*, not approximately. New
projects start at −3 dB.

Seeding old projects with the new default instead would have been a one-line migration and
the worst kind of bug: one you only hear, in somebody's finished mix, months later.

## The master's pan is a balance, and does not take the law

A pan law compensates for placing a **mono source** in a stereo field. The master bus
carries a mix that is already stereo, so running it through the −6 dB law would take 6 dB
off every centred mix in exchange for nothing. The master's pan is therefore always the
balance formulation — unity at the centre, the equal-power taper on the far channel.

The null test `the_pan_law_is_part_of_what_a_project_renders` is what found this: a
centred mix under the −6 dB law came out 12 dB down instead of 6, because the law was
being applied twice.

## Mono is width zero

Mono is not a switch. It is one end of the **stereo width** control — mid/side, where 0 is
mono, 1 is as recorded and 2 is twice as wide — and modelling it that way means there is
one value to store, one control to reach for, and no way for a boolean and a knob to
disagree about what the channel is doing. The mixer still shows a *Mono* button, because
that is what people look for, but it is a shortcut to width 0 rather than a second switch.

Width sits **after** the inserts and **before** the pan, which is the order a console
strip has: narrowing a stereo reverb has to happen before the channel is placed, or it is
the placement that gets narrowed.

## Polarity is an input-stage switch

The Ø inverts the channel at its *input*, before the inserts, which is where a console
puts it — so what leaves the strip is the whole channel inverted, tails and all. On the
master it is the last thing before the output, which is the same place for the same
reason. `a_track_against_its_inverted_copy_is_exactly_silence` is the null test: a track
summed with a polarity-inverted copy of itself cancels to zero, not to a small number.

## Consequences

- `Project` gained `pan_law`; `Track` gained `width` and `phase_invert`; `Master` gained
  `pan`, `width` and `phase_invert`. That is a schema bump (5), a migration and a
  regenerated fixture, per the standing rule.
- The audio thread never learns which law is in force: `Snapshot::compile` resolves it
  once and the processor reads two multipliers, exactly as it did before.
- **A control at its default must not touch a sample.** `apply_width` returns early at
  width 1 rather than computing a mid/side round trip that is only *algebraically* the
  identity, and `width_at_unity_and_polarity_off_change_nothing` asserts it.
- The width control is capped at 2. Past that the side signal swamps the middle and the
  mono sum falls apart; that is a limit, not a taste.
