# The prompt for the video session

*Paste everything below the line into a new Claude Code session opened at
`F:\projects\cantus`. The owner's choices so far are written in; the ones still open are
asked at the step that needs them.*

---

You are making the Cantus launch video: 80 seconds, in English and Spanish, for the top of
cantusdaw.com and the README. It should look professional and polished, as good as any
commercial DAW's launch film. The owner, Luis, wants it made with AI (you), with a very good
voice-over.

**Already decided by the owner (2026-11-20):**
- the app in **Slate Lapis Night**, the dark theme, in every shot;
- **Onda**, the oscilloscope dancer, in the video doing something very simple and constant
  (`script.md`, "Onda, the one dancer"); no other dancer.

**Still his to decide; ask him at the step that needs it, and do not guess:**
- the voice: his own, cloned from a recording he makes (recommended), or a stock voice;
  ask before step 2;
- the end card: A, with Colossians 3:23, or B, without; make both and let him choose;
- the songs: the script's proposal stands unless he changes it;
- where it is published: ask at step 8.

You may run the work yourself or hand the steps to builder agents. If you use agents, keep
one owner per file, give each the plan, the script and these rules, and review what each
returns: frames looked at, numbers checked. Never relay an agent's claim unchecked.

## Read first, in this order (in your worktree, `F:/projects/daw-r41/video`)
1. `CLAUDE.md`: the project's rules. Follow them, especially: explicit staging, commit only
   to your branch, never push; builds through `F:\projects\daw-r40\slot.sh`; never two
   suites or a CPU measurement at once (ask the other sessions with ListAgents /
   SendMessage before a heavy build); disk checked before every build; the corporate proxy
   never bypassed.
2. `docs/round41/video/README.md`: the plan. The pipeline, the tools, the licences, the
   checks.
3. `docs/round41/video/script.md`: the cut, the narration in EN and ES, the facts to check.
4. `docs/STATUS.md`'s top section, and `web/README.md` (the site's look and its tools:
   `web/tools/cdp.mjs`).
5. `crates/cantus-ui/examples/shoot_showcase.rs` and `crates/cantus-ui/src/raster*`: how
   the app renders itself headless, frame after frame, with the engine stepped between.

## Work
- The plan lives on branch `r41/video-plan` (it is not on master yet). Make your branch from
  it, in a worktree of its own:
  `git worktree add -b r41/video F:/projects/daw-r41/video r41/video-plan`.
  Give it an untracked `.cargo/config.toml` (`[build] jobs = 3`, `debug = 0`,
  `incremental = false`), and read the files named below in that worktree.
- Everything generated (frames, takes, renders, the Python environment, downloaded models)
  lives in `F:\projects\video-work\`, never in the repository.
- Commit only the code and the sources: `film.rs`, `tools/video/` (cards, `build.sh`, the
  voice scripts with pinned versions), and updates to `docs/round41/video/`.

## Steps — each one finished and checked before the next
1. **Facts.** Check every bracketed fact and every line under "Words to check" in
   `script.md` against the code at your commit. Fix the script where the code says
   otherwise, and say so in your report.
2. **The voice.**
   - Set up Chatterbox Multilingual (MIT) in a virtual environment.
   - Make 4–6 takes per line in each language. Run the gate: Whisper transcribes each take
     back word for word, and you measure clipping, DC, the silences at head and tail, and
     the pace window.
   - Build a small A/B page with the two best takes per line, and **stop**: the owner picks
     by ear. You cannot hear, so never choose for him.
   - Not the F5-TTS checkpoints: they are CC-BY-NC, and ADR 0042 keeps NC out.
   - If the owner chose his own voice, he records a clean 30–60 s reference (quiet room,
     no music). Use only his voice, never anyone else's.
3. **The film.**
   - Write `crates/cantus-ui/examples/film.rs`: the scenes of the cut, each through
     `CantusApp::assemble` and the real engine. Render at 3840 × 2160 at scale 2.0, at
     60 fps, with the engine stepped 800 samples a frame and a drawn, eased cursor.
   - Look at frames of every scene as you go, at least every 10th and both sides of every
     cut. Fix what is wrong at its source: a scene's state, never by painting over it.
   - Shots only a GPU or a plug-in's window can give go on a list for the owner to record
     with OBS, or are cut.
4. **The cards.** HTML and CSS in the site's tokens (Slate Lapis, Barlow Semi Condensed,
   logo #03, gold only in fine marks), animated by a per-frame time variable, rendered with
   headless Chrome to transparent PNGs. Make both end cards, A and B, if the owner has not
   chosen.
5. **The soundtrack, in Cantus.**
   - A Cantus project with the chosen takes and the demo excerpts cut to the picture.
   - The music ducked under the voice by a sidechain, 6–8 dB.
   - The master chain to −14 LUFS integrated and −1 dBTP, measured with Cantus's own meters.
   - Exported at 48 kHz, 24-bit. Save the project in `video-work` and describe it in the
     report.
6. **The assembly.** `tools/video/build.sh`: one reproducible ffmpeg run per language that
   gives the master, the YouTube file, the site's MP4 and WebM with a poster, the muted
   loop and the vertical cut, plus the SRTs and chapters. The specs are in the plan's
   table.
7. **The checks.**
   - Loudness and true peak.
   - Sync at three marks, the playhead against the audio, to the frame.
   - The ducking measured.
   - Every caption at most 42 characters a line.
   - A last look at the frames: no overlap, no clipped label, no "no audio device", no
     dancer.
8. **Stop for the owner.**
   - Send him the EN and ES drafts (SendUserFile) with a short note: what each scene shows,
     what you could not do, and what he should look and listen for.
   - **Publish nothing:** not on the site, YouTube or the public repo, until he says so.
     The site's deploy and the public repo go through the integrator session, on his word.

## Standards
- Real pictures, real sound, honest words.
- Every number from the code at your commit, and no new measurement.
- First-rank finish: eased motion, clean typography, intelligible voice.
- The owner's decisions stand: no dancers, the faith as he placed it.

When something only the owner can judge comes up (a voice, a take, a picture's taste),
stop and ask. Write your report as `docs/round41/video/report.md`, and say plainly what was
verified by measurement only and what nobody has watched or heard yet.
