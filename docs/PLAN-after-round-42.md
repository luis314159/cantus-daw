# The plan after round 42: users and depth (the owner, 2026-11-24)

The owner asked for a plan for after this session. He works on bringing people to Cantus;
Claude works on what has real impact, depth before breadth. The two tracks run side by
side. This file is the plan; `docs/next-session.md` points to it.

**The rule for both tracks:** an instrument or feature is finished, measured against the
market and liked by real users before the next one starts.

## Track A: the owner, bringing users

**Week 1: what only he can do, and what blocks the launch**
1. Upload the launch video (Lapis Night, Onda) to YouTube.
2. Open the social accounts from the social kit (`social/kit`).
   - Decide the bio's language, the Mastodon server and the Bluesky handle.
   - Verify the accounts.
3. Revoke the `cantus-github-public` token.
4. Listen to the two Arcus ABX pages (`abx-ciego/abx.html`, `arcus-upper/abx.html`) and
   write down the verdicts. Arcus joins the picker only after them.
5. Approve the holder design sheet for Arcus (stand, musician, mannequin) when it comes.

**Weeks 2–4: the first 100 real users**
1. **A place for feedback.** One place where people can report problems and ask for things:
   GitHub Discussions on `cantus-daw`, or a Discord. Everything is answered within 48 hours.
2. **Two niches first, where no big DAW serves people well:**
   - **Worship and church musicians**, in Spanish and English: worship teams, home
     studios, Christian producers.
   - **Mexican regional producers**: norteño, banda, sierreño.

   For each niche:
   - one demo song made in Cantus;
   - a 60-second clip for Reels, TikTok and Shorts;
   - a post in the communities where they gather.
3. **The free, open-source DAW communities:** KVR (a release announcement), LinuxMusicians,
   Reddit's r/WeAreTheMusicMakers, r/linuxaudio and r/edmproduction (follow each one's
   self-promotion rules), and Spanish-speaking producer groups.
4. **Recruit 10–20 beta testers** with a short form: their system, their audio interface and
   the style they make. Ask them for the hardware checks in `docs/beta-checklist.md` that
   only a real device confirms.
5. **Content every week.** One short clip of one thing Cantus does well: Chorda, Tympanum,
   the microtuning, the dancer or the AI tools. Real and honest, as the showcase rule says.

**Each month: measure**
- Downloads per release (GitHub's release counts).
- Feedback reports received and answered.
- How many testers keep using it after two weeks.
- The three complaints heard most often. They go to Claude as the next round's top
  priority.

**Decisions only he can take, without hurry**
- The legal review of Chorda's patents (ADR 0059).
- The MIDI Association membership, to propose the guitar to MIDI 2.0's articulation profile.
- Whether and how to earn money (his `docs/monetize.md`). Cantus's own sound packs stay free
  forever.

## Track B: Claude, depth before breadth

**1. Close round 42 properly**
1. Close the round's verification.
   - The full Windows suite, then WSL.
   - The Arcus holders (after the owner's approval).
   - Arcus into `InstrumentKind::ALL` after his ABX verdicts.
2. Write the round up.
   - STATUS, the integration report, the memory and `next-session.md`.
   - The backlog's §3e table marks updated: the arpeggiator, Fides and Tympanum are done.
3. Ship it: beta.18 through `docs/RELEASING.md`'s preflight.

**The order, revised (the owner, 2026-11-25).** The adaptive piano roll moves forward. It
leads round 43, beside the musicians' polish to 100 % and Chorda's sound. It gives Arcus the
per-note control it lacks today (the bowing's direction, the techniques, the vibrato), and every
later instrument is built on it. The guitar's engine follows in round 44, on top of it. The
expressive musician (BACKLOG, "The musician who plays expressively") waits until both
musicians are at 100 %. The depth items below run beside the piano roll as their inputs
(testers, listeners) arrive.

**2. A depth round before new instruments (round 43a): make what exists excellent**
- **Chorda's sound.** The owner's ABX against Salamander was 0–7. The goal: listeners cannot
  tell them apart, or Chorda is preferred. This is the most heard instrument; it matters
  more than a new one.
- **Arcus after its ABX.** Brightness with force, the attack's noise and violin/viola
  radiation (backlog "Arcus, after round 42").
- **The first ten minutes.**
  - Install, open, a template per style (worship, regional, pop, beat), the first sound
    and the first export, without reading a manual.
  - Measured with two or three people who have never seen Cantus.
- **Memory.** Win the quiet scorecard's memory line against REAPER and FL; today it is the
  one we lose.
- **Stability.** A long soak with real projects and hosted plug-ins, opt-in crash reports
  that respect privacy, and every crash from testers fixed first.
- **The look against FL.** Measured again with blind judges since the round-39 redesign.
- **The testers' three main complaints**, as they arrive.

**3. Round 43 leads with the adaptive piano roll; round 44 brings the guitar family on one engine**
- The adaptive piano roll comes first (backlog: playing profiles, the technique brush, the
  strum tool, tablature, MIDI 2.0's articulation profile).
- Then **one plucked-string engine** done to the bar: the acoustic and electric guitar first,
  measured against Ample Guitar.

  Once it passes the owner's ear as a guitarist, the same engine gives:
  - the bajo sexto and bajo quinto;
  - the requinto;
  - the banjo;
  - the lap and pedal steel.

  Each one ships only with a study and an ABX.
- The white paper and the CLAP draft for the techniques' gap.

**4. After that, one engine at a time, each to the bar before the next**
1. The blown-tube engine: the winds, then the accordion's reeds.
2. The formant choir.
3. Struck bars and membranes: tuned and hand percussion, the marimba, the tambora.
4. The world's instruments, family by family, on those engines.

## How we know it is working

- Each release: the scorecard (§0) moves on at least one measured line.
- Each instrument: it passes the owner's blind ABX before it is in the picker.
- Each month: more users who stay, and their complaints shrinking.
