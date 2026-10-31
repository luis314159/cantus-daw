# ADR 0084 — The mastering assistant

**Status:** accepted (round 34) · **Date:** 2026-10-30 · **Extends:** ADR 0026 (the master
chain), ADR 0052 (loudness), ADR 0083 (listening), ADR 0047 (licences)

## Context

`research/ai/generative-audio.md` §6: "AI mastering" services are proprietary, upload the
audio and hide what they did; Cantus already has mastering units on the master chain and a
conformance-tested loudness meter. BACKLOG §7 and the round's brief ask for an assistant on
the master channel that listens, proposes a chain of Cantus's own effects with loudness
targets per platform, everything editable by hand afterwards, an A/B against the
unmastered mix at the same loudness, a tool for agents and the prompt *Master for
streaming*; measured on the demo songs within ±0.5 LU of the target and under the ceiling.
Matchering (GPL-3.0) may inspire it; its code may not go into an MPL crate.

## Decision

1. **Where it lives.** The decision logic is GPL, in `cantus-ui` (`mastering.rs`,
   `mastering_targets.rs`); the engine gains only what is generic and ours (MPL): the
   listen of ADR 0083 and `listen::run_chain` — interleaved audio through a fresh copy of an
   insert chain (the stems' `Through`, ADR 0026), latency trimmed — for its trials.
2. **What it aims at.** `PLATFORMS`: Spotify (−14 LUFS, −1 dBTP), Apple Music (−16, −1),
   YouTube (−14, −1), TIDAL (−14, −1), Amazon Music (−14, −2), SoundCloud (−14, −1), CD
   (−10, −0.3), Club (−8, −0.3) and EBU R 128 (−23, −1), each with its source, whether the
   platform publishes the number itself, and the month it was checked — one table, because
   these change. `CURVES`: pop, rock, electronic, hip-hop, acoustic, classical — octave
   balances relative to 1 kHz, ours, drawn after Pestana et al. (AES conv. paper 8960, 2013)
   and round 28's pop reference, with the loudness range and width a master of the genre
   keeps. A **reference track** (any file the sampler reads) replaces the curve: its octave
   balance, its width and its range are matched; the platform still sets the loudness.
3. **How it decides** (Matchering's ideas — match a reference's loudness, frequency
   response and width, then limit — re-implemented from their description, with our
   measurements and units; no code taken):
   1. *Balance*: the mix's octave shape (relative to its mean over 63 Hz … 8 kHz) against
      the target's, corrected by an Aequalis of a cut under 20 Hz, a low shelf at 45 Hz,
      octave-wide bells and a high shelf at 11 kHz, in **closed loop** (run, measure, move,
      three times); a curve gets half the difference, ±3 dB at most, a reference 0.8 of it,
      ±6 dB.
   2. *Glue*: a loudness range over the genre's gets Comprimo at 1.5:1 (2:1 past 3 LU
      over), 30 ms attack, programme release, its threshold under the loud passages'
      short-term level.
   3. *Width and mono*: more than 2 LU lost in mono or a window under −0.2 correlation →
      Latitudo a little narrower with the bass mono under 120 Hz; stereo bass alone → bass
      mono; a mix 3 dB narrower than the target → up to 2 dB more side.
   4. *Loudness*: Terminus in true-peak mode at the platform's ceiling, its gain found by a
      secant search over trials of the whole chain on the whole mix to within 0.1 LU; a
      target under the mix's level is reached by the EQ's output trim. **The master strip
      plays after the chain** — width, then fader and balance, then polarity, as the engine
      does — and every trial plays it too (review 1: only the fader was counted, and a width
      of 1.5 put the true peak over the ceiling); a trial's true peak over the ceiling
      lowers the limiter's ceiling by the excess and the search runs again (three times at
      most). Automation or a modulator on the master's level, balance or width is left out
      of the mix the chain is made for, and the proposal warns that its numbers hold at the
      strip's resting values.
4. **What it listens to.** The mix as it reaches the master: the master chain out (to
   replace it) or in (to add after it), the master's fader, balance and width at rest —
   over the whole song or the loop.
5. **Applying** is one undo step, `Command::Batch` labelled `cmd.master_assist`:
   `RemoveEffect` of the master's own inserts when replacing, `InsertEffect` of each unit
   (fresh ids from the bus), the master chain switched on if it was off. The units are
   ordinary inserts: every value editable by hand, nothing hidden. The panel's *Apply*
   names the inserts it takes out when it replaces.
6. **The panel** (`views/mastering.rs`, `views/mastering_draw.rs`; opened by *Mastering
   assistant…* on the master strip's menu and at the top of the master chain's *Add effect*
   list): platform and balance (‹ › fields), a reference to load, whole song or loop,
   replace or add; *Listen and propose* on a worker with its progress and *Stop*; the
   numbers (integrated, true peak, LRA, loudest 3 s, width, mono fold-down, lowest
   correlation) for the mix, the proposal and the target; the octave balance drawn (the
   mix's bars, the proposal's line, the target's rings); the chain with why each unit is
   there; *Apply*; **A/B** — the mix and the proposed master played through the preview
   voice (ADR 0030's audition voice, so the project and its history do not change to
   compare), the louder turned down to the quieter's loudness, switching at the same
   place; the platform's source under it.
7. **For agents**: `master_assist` (a job, class Read: it changes nothing; it answers the
   target with its source, the mix's numbers and spectrum against the target, the chain
   with every non-default setting in its unit, why, the trial's prediction and a
   `proposal_id`), `apply_master_chain` (class Edit: adds after the master's own inserts or
   onto an empty chain) and `replace_master_chain` (class **Destructive**: takes the
   master's own inserts out, and its sentence names them — review 1). Both refuse a
   proposal listened to before the project last changed (`Stale`), and proposals and
   checkpoints are forgotten when another project replaces the open one (the application's
   epoch). A listen is at most 20 minutes long, and one that keeps the master's samples (the
   assistant, audition's WAV) at most 20 minutes' worth of frames at 48 kHz (five minutes at
   192 kHz — review 2): a longer range is refused with "choose a range", its length in
   minutes and seconds (review 3), and the panel says to set a loop — or, the loop chosen,
   to shorten it. Measured at the cap (a 19-minute song through the panel): **1.1 GB** of
   working set at the peak, 1.07 GB held while the panel shows its A/B — the mix and the
   mastered audio 0.9 GB, the listen's 24 checkpoints 190 MB (ADR 0083; 2.8 GB before
   checkpoints kept only the cells in use — review 3). The prompt
   *Master for streaming* (`master_for_streaming`, both locales) walks an agent through
   audition → masking and clicks → master_assist → apply → audition again.

## Consequences

- The result is checked on a real render, not on the trial: `the_assistant_lands_the_
  platform_loudness_under_its_ceiling` applies the chain and listens to the song, as the
  agent's prompt does.
- **Measured** (`tests/listen_measure.rs`, every demo song, master chain replaced, Spotify,
  Apple Music and Club, the result read from a real render of the song with the chain in
  place): the integrated loudness within **0.07 LU** of the target in all 21 cases (the
  worst −0.07, the showcase on Spotify −0.05) and the true peak **0.09 dB or more under the
  ceiling** in every one (−1.09 … −1.13 dBTP for −1, −0.39/−0.40 for −0.3); the trial's
  prediction equal to the real render's reading to the hundredth. Mixes from −15.9 to
  −19.2 LUFS; three or four units each; 30 s (a 1-minute song) to 3 min (the showcase) of
  listening and trials.
- A platform's numbers are data with a date; when one changes, its row changes.
- The assistant does not judge taste: the curves are guides, the reference is the user's,
  and every value it sets is a knob on the master.

## What the tests hold

`mastering_targets.rs`: every target sane and sourced, the eight asked for present, the
curves relative to 1 kHz. `mastering_tests.rs`: for Spotify, Apple Music and Club the
trial within ±0.5 LU, and the real render after *apply* within ±0.5 LU of the target with
the true peak at or under the ceiling; undo restores the project. `tests/ai_listen.rs`: the
same through the MCP server with the application as host. `tests/listen_measure.rs`
(ignored, the numbers of the round's report): every demo song for three platforms.
