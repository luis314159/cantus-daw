# The Cantus launch video: the plan

*Round 41, 2026-11-20. The owner asked for a 60–90-second video for the top of
cantusdaw.com and the README. It should look professional and polished, be made with AI
(Claude) in a session of its own, and carry a very good voice-over. This folder holds that
session's plan, the script (`script.md`) and the prompt that starts it (`prompt.md`).*

## What the video must be

- **Real.** Every picture is the real app rendering itself, and every sound is the real
  engine playing a demo song. No mock-ups, no stock footage, no sound Cantus did not make.
  The picture may be framed, zoomed and cut; what it shows may not be invented.
- **Honest.** Every claim in the narration is checked against the repository at the commit
  the video is made from: the counts, the platforms, the licence, "free". No figure is
  measured for it: the owner put new measurements aside for later.
- **First-rank.**
  - Fluid camera moves: eased zooms onto a control, never a jerky mouse.
  - Typography and colour in the site's own look: Slate Lapis, Barlow Semi Condensed, the
    logo #03, gold only in fine marks.
  - Sound mixed and mastered: dialogue intelligible over the music, the music ducked
    under it, −14 LUFS integrated and −1 dBTP for the web.
  - Every frame looked at before it ships.
- **In English and in Spanish.** Two narrations and two caption sets, written as Spanish,
  not translated word for word.
- **The owner's decisions stand.**
  - **One dancer, Onda,** by his choice (2026-11-20): the figure drawn by an oscilloscope's
    amber trace. It dances something very simple and constant, a steady groove on the beat
    with none of its drop moves, in the app's own Stage. It is the real dancer as the app
    renders it, with no special version made for the film. The other dancers stay out, and
    the site and the README stay without dancers.
  - The faith is his to place: an end card with Colossians 3:23 is offered as an option,
    his choice.
  - Nothing is published without his word.

## The pipeline

```
script.md ──► voice (Chatterbox, EN + ES) ──► takes checked (Whisper, loudness) ──► owner picks
    │
    ├──► film (cantus-ui example, the real app frame by frame, 60 fps) ──┐
    ├──► cards (HTML in the site's look, headless Chrome frame by frame) ─┼──► assemble (ffmpeg,
    └──► soundtrack (demo excerpts + voice, mixed and mastered IN CANTUS) ┘     one script)
                                                                              │
                                     review frames (Claude reads them) ◄──────┤
                                     owner watches the draft ◄────────────────┘
```

### 1. The picture: the real app, frame by frame

Cantus renders its whole interface on the CPU with no screen: `cargo run -p cantus-ui
--example shoot` and its siblings, through `cantus_ui::raster`. `shoot_showcase.rs` already
steps the engine between frames, so meters, the playhead and the clocks move. The video
session adds one example, `crates/cantus-ui/examples/film.rs`, a director's script in code:

- **Scenes as functions.** Each scene opens a demo song through `CantusApp::assemble` (the
  boot path), does what a user does (play, open the mixer, insert an effect, turn a knob,
  open Chorda's room), and renders **N frames at 60 fps**. The engine is stepped
  `48000 / 60 = 800` samples per frame, so picture and sound share one clock.
- **The cursor is drawn,** eased between targets (cubic in and out, 250–400 ms per move),
  with a small click ripple. It moves the way a calm demonstrator moves.
- **Rendered large, framed later:** 3840 × 2160 at scale 2.0 (1920 × 1080 points). The
  assembly crops and zooms from it, so a push-in on a knob stays sharp.
- **Deterministic.** The same commit gives the same frames, so a fix to one scene re-renders
  only that scene.
- **Output:** a PNG sequence per scene, `film/<scene>/00000.png`, plus `film/<scene>.json`
  with the frame count, the song, the start tick and the zoom targets in pixels.
- **Fallback for what the CPU renderer cannot show** (the *Photographic* pictures need a
  GPU; a hosted plug-in's own window): the owner records those shots with OBS at 60 fps
  from a written list, or they are left out.

The app's look for every shot: Slate Lapis Night, the default. One shot of Paper, the
light theme, shows that both exist.

### 2. The cards: titles and captions in the site's look

Titles, lower thirds, the counts and the end card are HTML and CSS in `tools/video/cards/`,
in the site's tokens (`web/style/base.css`), Barlow Semi Condensed (SIL OFL) and logo #03.

- **Rendered frame by frame** with headless Chrome through `web/tools/cdp.mjs`: the
  animation is driven by a `--t` variable per frame, never real time, so every frame is
  exact.
- **Alpha:** PNG with transparency, laid over the film.
- **Captions** burned into the web cut, plus SRT files (EN, ES) for YouTube.

### 3. The voice

- **The model: Chatterbox Multilingual** (Resemble AI, **MIT**; weights on Hugging Face).
  It speaks Spanish and English and clones a voice zero-shot from 5–10 s of reference.
  Every file it makes carries Resemble's inaudible Perth watermark; the description says
  the voice is AI.
- **Not the official F5-TTS weights.** F5-TTS's *code* is MIT, but its pretrained
  checkpoints are **CC-BY-NC-4.0**, because their Emilia training set is non-commercial.
  ADR 0042 keeps NC out of Cantus. OpenF5-TTS (Apache-2.0) is alpha and weaker.
- **Fallback: Kokoro-82M** (Apache-2.0), strong in English, thinner in Spanish (its own
  notes say so).
- **Whose voice — the owner's choice:**
  - **(A) His own voice, cloned** from a clean 30–60 s recording he makes. A program's
    author speaking for it is the strongest choice, and it is his consent.
  - **(B) A stock voice** of the model.

  Never a voice copied from anyone else.
- **Takes.** Each line is generated 4–6 times (seeds, exaggeration 0.4–0.6, cfg 0.3–0.5),
  as 24 kHz WAV.
- **The gate, since Claude cannot hear.** Each take is:
  - transcribed back with Whisper (MIT): it must match the script word for word;
  - measured for clipping, DC, and silence at the head and tail;
  - checked against the line's pace window: about 2.3–2.7 words a second in English and
    4.5–5.5 syllables a second in Spanish, a calm presenter's pace, set per line by the
    picture's timing.

  The best two per line go to a small A/B page, in the ABX pages' manner, and **the owner
  picks by ear**.
- **Run locally**, in a Python virtual environment of its own, outside the repository:
  `F:\projects\video-work\`. Downloads go through the corporate proxy: verify each one
  (a `.safetensors` file is not HTML) and wait if it is quarantined; never bypass the proxy.

### 4. The soundtrack: mixed in Cantus

The music is the demo songs themselves (excerpts rendered by `render_demos`), so the video
sounds like Cantus because it is Cantus. The soundtrack is mixed **in Cantus**, a project
of its own:

- the voice-over on one track, the song excerpts on others, cut to the picture's timings;
- the music ducked under the voice by a sidechain (Cedo keyed by the voice, or a compressor
  with the voice as its key), about 6–8 dB, 30 ms attack, 250 ms release;
- the master chain to −14 LUFS integrated and −1 dBTP (the web's targets), checked with
  Cantus's own meters;
- exported as 48 kHz 24-bit WAV.

The description can then say, truthfully, "the soundtrack was mixed and mastered in Cantus."

### 5. The assembly

One committed script, `tools/video/build.sh`, a reproducible ffmpeg filtergraph:

- the scenes in order, with cuts on the beat (the song's tempo map gives the bar times),
  cross-dissolves of 8–12 frames between sections, and push-ins as crops of the 4K frames
  (eased, never more than 1.6×);
- the cards over the film (alpha), the captions, the soundtrack;
- **outputs:**

  | File | Spec | Use |
  |---|---|---|
  | `cantus-<lang>-master.mov` | ProRes 422 HQ or FFV1, 2160p60 | the master, kept |
  | `cantus-<lang>-2160p.mp4` | H.264 High, CRF 16, AAC 320 k | YouTube |
  | `cantus-<lang>-1080p.mp4` and `.webm` | H.264 (CRF 20) and VP9 (or AV1), ≤ 12 MB | the site, with a poster JPG |
  | `cantus-loop.mp4` | 15–20 s, no voice, captions only, muted autoplay | the site's hero |
  | `cantus-<lang>-vertical.mp4` | 1080 × 1920, 30 s | Shorts, Reels |

  The SRTs and a chapters file go beside them.
- **ffmpeg is a tool here,** never shipped with Cantus.
- **Big files stay out of the repository** (ADR 0030): the masters are kept by the owner,
  and the site's cut is uploaded where the owner decides (the site's assets if small
  enough, else a release asset).

### 6. The checks before the owner sees it

- **Every claim against the repository** at the video's commit:
  - the counts from `web/src/units_data.rs` and `counts.rs`;
  - the platforms from `web/src/top.rs` and the last release;
  - the licence from ADR 0047.
- **Every 10th frame and every cut's two sides,** looked at by Claude:
  - no text overlaps;
  - no "no audio device" line;
  - no debug colour;
  - no clipped label;
  - the look is Slate Lapis;
  - no dancer but Onda, and Onda only in its simple, constant groove.
- **The soundtrack:** loudness and true peak, the voice intelligible (the ducking measured:
  music at least 8 dB under the voice in the speech band while it speaks), picture and sound
  in sync (the playhead's position against the audio at three marks, to the frame).
- **The owner watches the draft,** EN and ES, and says what changes. Nothing is published
  until he says so.

## What the owner decides

1. **The voice:** his own, cloned from a recording he makes (recommended), or a stock voice.
2. **The end card:** with Colossians 3:23 (see the script's two endings) or without.
3. **The music:** which demo song carries the video. The script proposes *Aurora* for the
   open and the piano, with a beat-driven song for the mixer and effects.
4. **Where it is published:** the site's hero, YouTube, both.

## Where its work goes

- **A branch `r41/video` from master:** `film.rs`, `tools/video/` (cards, `build.sh`, the
  voice scripts) and this folder's updates.
- **Never committed:** the generated frames, takes and renders. They go in
  `F:\projects\video-work\`.
- The session follows `CLAUDE.md`: explicit staging, no pushing; builds through
  `F:\projects\daw-r40\slot.sh`, never while another session runs a suite or a CPU
  measurement (ask it first).

## Sources

- Chatterbox: https://huggingface.co/ResembleAI/chatterbox (MIT) ·
  https://www.resemble.ai/learn/models/chatterbox-multilingual
- F5-TTS's checkpoint licence (CC-BY-NC-4.0, the code MIT):
  https://huggingface.co/SWivid/F5-TTS
- Kokoro-82M (Apache-2.0): https://huggingface.co/hexgrad/Kokoro-82M ·
  https://huggingface.co/hexgrad/Kokoro-82M/blob/main/VOICES.md
- The video export Cantus already has: ADR 0088.
