# The launch video — the session's report

*Branch `r41/video` from `r41/video-plan` (`74f202b1`), worktree `F:/projects/daw-r41/video`.
Generated files live in `F:\projects\video-work\`, never in the repository.*

| Step | State |
|---|---|
| 1. Facts | done (`ae88ebae`); script corrected |
| 2. The voice | **the A/B page is ready for the owner's ear**: `F:\projects\video-work\voice\ab.html`, two gated takes per line in English and Spanish (below, "Step 2, rounds 2–3") |
| 3. The film | **draft ready for the owner**: `F:\projects\video-work\film\draft-1080p30.mp4` (1080p30, no voice); the 2160p60 final waits for his approval |
| 4. The cards | sources written, both end cards A and B; **stills rendered in EN and ES and looked at**; three fixes made from them |

## Step 2, round 1 (2026-11-20): 3 of 96 takes passed

Six takes a line, stock voice, exaggeration 0.4–0.6, cfg 0.35–0.5. What the gate found:

- **Too fast.** English ran at 3.5–4 words a second, against a calm 2.3–2.7. Spanish ran at
  6–7 syllables a second.
- **Words.** The model read "FL" as "Val" or "Fel", "VST3" as "Vistie Street", and the
  address as "cantus.com", dropping "daw". "Cantus" was heard as "Candice" on some takes.
- **Babble after the line**, on several takes (e.g. EN 05-5: 40 s of invented speech).
- **Flagged as clipped:** these were single float samples at 1.005–1.026, overshoots, not
  flat tops.

The fixes, committed:

- The model is given spoken spellings ("F.L. Studio", "V.S.T. three", "cantus-daw dot
  com"; in Spanish "efe ele", "uve ese te tres", "cantus daw punto com").
- A calmer grid: eight takes, exaggeration 0.3–0.45, cfg 0.2–0.3, temperature 0.6–0.75.
- The gate:
  - matches names within two letters with the same first sound;
  - measures pace with the line's own pauses, 2.0–3.0 and 4.0–6.0;
  - allows a 10 ms head;
  - counts a clip only as a flat top of 3 or more samples;
  - reads the audio as an array, because Whisper wanted ffmpeg for files.
- Round 1 is kept in `F:\projects\video-work\voice\round1`.

## Step 4: what the stills showed (looked at, 1920 × 1080, on a slate plate)

`F:\projects\video-work\cards\stills\view\{en,es}-<card>.png`. Three fixes came from them:

- the ™ floated high and apart, and now sits at cap height;
- the count's panel resized as the figure grew, and is now two figures wide;
- a dot showed before a stroke drew, and no longer does.

Checked at mid-animation: the logo draws on (the border's stroke, the C, the flourishes),
and a count reads 35 on its way to 42.
| 5–8 | not started |

## The owner's answers (2026-11-20)

1. **The voice:** the model's stock voice (`conds.pt`) for this version. His own cloned voice
   comes in a later version, so there is no reference recording, and `takes.py` runs without
   `--ref`.
2. **The songs, as step 1 changed them:** approved. *With All Your Heart* opens and closes,
   *Morning Light* has the piano, Tympanum plays on *Sunday Strut*, and *Neon Tide* has the
   mixer.
3. **The shorter line 4, in English and Spanish:** approved.
4. **Line 7 stays as written.** Cantus's own sound packs will always be free, forever. A
   third party selling its own packs later does not change that.

## Step 1 — the facts, against the code at `74f202b1` (master `7b287c39`'s code)

What holds:

- **11 instruments**: the site's `INSTRUMENTS` has 10, and `counts::instruments_with_firmus()`
  adds Firmus. The app's `InstrumentKind::ALL` has 10 kinds, one of them the dancer, which
  makes no sound.
- **42 effects**: `EffectKind::ALL` has 43, and `offered` leaves out only the old Reverb.
- **Windows and Linux at beta.15, macOS next.** The site's download shelf reads the latest
  release's files; change the card if beta.16 ships macOS first.
- **The licence**: ADR 0047 and `CLA.md` §3.
- **"Tuned string by string"**: all 254 loops measured alone and in tolerance.
- **"Hear it as you add it"**: an effect loads a starting preset when it is inserted.
- **Session view, host, master and mastering**: ADRs 0076, 0019/0073, 0026 and 0084.
- **Tympanum, every microphone on its own channel**: *Each mic to its own strip* (ADR 0124)
  is a choice, not the default; the drum shot sets it.
- **Onda**: held on `Bounce`, with no `Jump` and no `Shy` (both are its square).
- **End card A's verses**: the app's WEB and RV1909 texts, both public domain.

Fixed in the script:

1. Aurora has no piano, so shot 3 is *Morning Light*.
2. No demo uses Tympanum, so shot 4 loads it on *Sunday Strut*'s drum track, as a user would.
3. Line 4 was cut in English and Spanish to fit its window. Shot 5 now runs to 0:44, and
   line 5 starts at 0:44.5.
4. The word counts now say about 130 in English and about 120 in Spanish.

Asked the owner: the voice, the changed songs, the shorter line 4, and line 7's "No paywall".

## Step 2 — the voice (prepared, not run)

- **Environment**: `F:\projects\video-work\env\tts`, Python 3.12, torch 2.6.0+cu124,
  chatterbox-tts 0.1.7 and openai-whisper 20250625 (`tools/video/voice/requirements.txt`).
- **Models**: ResembleAI/chatterbox at rev `5bb1f6ee`, and Whisper large-v3. Their SHA-256s
  are in `tools/video/voice/models.txt`, every one checked.
- **Tools**: `lines.py`, `pace.py`, `takes.py`, `gate.py`, `abpage.py` and `ref.py`. None has
  run on audio yet.

## Step 4 — the cards (sources only, not rendered)

`tools/video/cards/`:

- **`cards.html`, `cards.css` and `cards.js`.** One page with every card, chosen with
  `?card=<name>&lang=en|es`.
  - It is laid out at 1920 × 1080 CSS px, on a transparent page, inside a 5 % safe area.
  - It uses the site's tokens (Slate Night, the lapis stage, gold only in rules, ticks and
    the verse), served from `web/assets/fonts`.
  - Logo #03 is fetched from the app's own `crates/cantus-ui/assets/logos/03.svg` and
    inlined, so its parts can move.
  - There is no CSS animation. `window.cardSetTime(t)` sets every moving value for time t,
    with cubic easing, so each frame is exact.
- **The cards:**

  | Card | Shot | Length | What it shows |
  |---|---|---|---|
  | `title` | 1 | 6 s | Logo #03 draws on (tile, gold border, C, the waveform's bars, flourishes), then "Cantus™" and the gold rule |
  | `count-instruments` | 3 | 4.5 s | A lower panel; the figure counts up to 11 ("instruments" / "instrumentos") |
  | `count-effects` | 5 | 4.5 s | The same panel, counting up to 42 ("effects" / "efectos") |
  | `lower-record`, `lower-session`, `lower-plugins` | 6 | 2.6 s each (`--dur`) | "Recording", "Session view", "CLAP and VST3 plug-ins" (ES: "Grabación", "Vista de sesión", "Plug-ins CLAP y VST3") |
  | `free` | 8 | 5 s | "Free · open source" / "Gratis · código abierto" over a low scrim |
  | `end-a`, `end-b` | 9 | 12 s | The lapis ground, the logo with "Cantus™", cantusdaw.com, "Windows · Linux — macOS next" / "— macOS en camino", the download button drawn ("Download free" / "Descarga gratis"); A adds the verse |

  - End card A's verse is the app's own text, its first clause, upright (the site ships no
    italic Barlow): the WEB in English and the RV1909 in Spanish.
  - On both end cards, Onda's place is kept free at the right, 420 × 560 px, where
    `build.sh` lays the keyed-out Stage.
- **`render.mjs`.** It drives the site's `web/tools/cdp.mjs` and serves the repository's
  root with its own server: no crash on a missing file, nothing outside the root.
  - It writes transparent PNGs at device scale 2 (3840 × 2160), 60 a second, with a `.json`
    per card.
  - `--still <s>` writes one frame per card, to look at.
- **Checked so far:** `node --check` on both scripts. Nothing has been rendered or looked
  at. The logo's part selectors, the line breaks and the spacing are unverified until the
  first stills.

## Step 2, rounds 2–3 and the A/B page (2026-11-20)

- **Round 2:** whole lines, the calmer grid, the spoken spellings, 8 takes a line. Under
  the gate as it then stood, 6 of 64 English takes passed and 8 of 64 Spanish. Every long
  line still ran fast: 3.5–4.5 words a second in English, 6–7.5 syllables in Spanish.
- **Round 3, phrase by phrase:** stopped after two takes. The model babbles after a short
  input: line 1 came out at 12.9 s and 14.7 s. This is kept behind `takes.py --phrases`,
  off.
- **What stands:**
  - New whole-line takes for lines 4 and 5 (`F-L`, `vee-ess-tee three`).
  - `breathe.py` over every round-2 and round-3 take. It opens the take's own pauses to
    180 and 320 ms with silence, and never stretches the voice.
  - The gate, revised:
    - A take's end is judged by its level: louder than −30 dB under its loudest 20 ms
      means a word was cut off; up to 700 ms of trailing sound is allowed.
    - The pace is reported, not failed.
- **Passed, per line:**

  | Language | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
  |---|---|---|---|---|---|---|---|---|
  | English | 7/8 | 6/8 | 5/8 | 8/16 | 5/16 | 6/8 | 4/8 | 3/8 |
  | Spanish | 4/8 | 2/8 | 4/8 | 9/16 | 5/16 | 6/8 | 3/8 | 5/8 |

- **The page:** `F:\projects\video-work\voice\ab.html`, 32 takes in all.
  - It shows the two passing takes per line nearest a calm pace, in take-name order.
  - It shows each take's length and pace, and says "faster than a calm read" where it is.
    That is English lines 1, 2, 4 and 8, and Spanish lines 1, 2 and 4.
  - **Checked:** every audio link resolves to a file. A headless-Chrome screenshot of the
    page was looked at: the layout, the notes and the picks bar.
- **Whisper's spellings on the takes shown** (the gate accepts a name within two letters,
  same first sound):
  - "Qantas" for Cantus on both English line-1 takes;
  - "cantusda.com" on both English line-8 takes;
  - "Timpanum" on Spanish line 3, take r2-03-4.

  The owner's ear decides whether the name was said right.

## Step 2, the owner's verdict and Qwen3-TTS (2026-11-20)

- **His verdict on Chatterbox:** low quality, sounds like AI, in both languages. Of the
  pairs, he preferred the A takes' cadence. The Spanish must sound Latin American.
- **His decision:** try Qwen3-TTS; if it fails, ElevenLabs (his account).
- **The names in Latin's hard consonants:** Chorda KOR-da, Cantus KAN-tus, Tympanum
  TIM-pa-num. The TTS is given Korda, Kantus and Timpanum (in Spanish, Korda and
  Tímpanum). The gate accepts Chorda, Korda and Corda, no longer "Qantas", and lists the
  names it heard in each take.
- **Qwen3-TTS:** code and weights Apache-2.0. Models pinned in `models.txt`, SHA-256s
  checked. Its own environment at `env\qwen`, pinned in `requirements-qwen.txt`.
- **Voices, nobody's:**
  - English: the built-in Aiden and Ryan, steered by a calm-narrator delivery.
  - Spanish: two voices designed from descriptions, a man and a woman, "acento
    latinoamericano neutro, mexicano". Each is held steady across lines: a reference clip is
    made by VoiceDesign, and the Base model speaks every line in that synthetic voice.
- **The run:** `tools/video/voice/qwen_run.sh`, with one model in memory at a time. The
  page is `F:\projects\video-work\voice\ab-qwen.html`. It puts each line's preferred
  Chatterbox A beside the best gated take of each Qwen voice.
- **Where it paused** (2026-11-20, the integrator's order: C: had under 2 GB free):
  - **Done:**
    - the Spanish references, all eight passing the gate (references `es-latam-4` and
      `es-latam-f-4` chosen, the nearest 5 syllables a second);
    - the English takes, 80, with the names re-made in their spoken spellings, and gated;
    - the Spanish takes, 80.
  - **Not done:** the Spanish gate and the page.
  - **Resume:** `bash tools/video/voice/qwen_run.sh /f/projects/video-work 4`.
- **The English gate, passed per line** (Aiden / Ryan, of 5 each):

  | Voice | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
  |---|---|---|---|---|---|---|---|---|
  | Aiden | 5 | 4 | 2 | 2 | 4 | 5 | 5 | 5 |
  | Ryan | 1 | 4 | 3 | 2 | 1 | 5 | 3 | 4 |

  Pace is 2.4–3.1 words a second on most lines, near the calm window without any
  breathing pass. Line 4 runs 2.7–3.4.
- **The page, built (2026-11-21):** `F:\projects\video-work\voice\ab-qwen.html`.
  - Its audio links resolve, and a headless screenshot of it was looked at.
  - **The gate:** Whisper large-v3-turbo, loaded straight onto the GPU in half precision.
    The first run with large-v3 loaded on the CPU grew C:'s pagefile until C: had 2 GB
    left, and it was stopped.
  - **New rule:** a long lead no longer fails a take, because the assembly trims it.
    Qwen's Base model leaves 0.4–0.9 s before the speech. "Fits" measures the speech
    itself.
  - `gate.py --recheck` re-applied the rules to the measures already in `gate.json`.
- **Passed per line** (lines 1–8, of 5 each):

  | Voice | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | Total |
  |---|---|---|---|---|---|---|---|---|---|
  | EN Aiden | 5 | 0 | 2 | 3 | 4 | 5 | 5 | 4 | 28/40 |
  | EN Ryan | 1 | 4 | 3 | 2 | 1 | 5 | 4 | 3 | 23/40 |
  | ES man (Latin American) | 5 | 5 | 5 | 4 | 4 | 4 | 5 | 5 | 37/40 |
  | ES woman (Latin American) | 5 | 5 | 4 | 5 | 4 | 5 | 4 | 4 | 36/40 |

- **Aiden's line 2:** Qwen read "modelled" as "mottled". The model is now given
  "modeled". The remake stopped on low RAM after 3 of Aiden's 5 takes. Those three are not
  gated, so the page shows no Aiden take for line 2.
- **Spanish pace:** 4.5–5.5 syllables a second on every line but 7, which is slower and
  is marked on the page. Nobody but the owner can judge the accent.

## Step 2, ElevenLabs (2026-11-21)

- **The owner's verdict:** Qwen sounds horrible and is dropped. He prefers Chatterbox's A
  takes.
- **His plan:** ElevenLabs Starter, 40 000 characters a month. The key stays in Windows
  Credential Manager (`cantus-elevenlabs`) and is used only through `eleven-api.ps1`.
- **The model:** `eleven_multilingual_v2`, which the API itself describes as best for
  voice-overs. Output is WAV, 48 kHz, 16-bit, as the API gives it.
- **Voices,** ElevenLabs library voices added to the account:

  | Language | Voice | Id |
  |---|---|---|
  | English | Jon, Warm & Grounded Storyteller (American) | `MFZUKuGQUsGJPQjTS4wC` |
  | English | Victoria, Warm, Trustworthy (American) | `qSeXEcewz7tA0Q0qk9fH` |
  | Spanish | Alejandro, Expressive and Polished (Mexican narrator) | `NWqMOQLlMBaUbjKYdhbW` |
  | Spanish | Gabriela, Warm, Rich and Balanced (Mexican, narrative) | `hHjbwzYZW17oh0p05AKv` |

- **Takes:** 3 per line per voice, each with its own seed and settings (stability
  0.45–0.65, speed 0.92–0.95).
  - The names are spelled as they are said: Kantus, Korda, Timpanum.
  - In Spanish, "ve ese te tres", as Latin America says the letters.
- **Characters spent: 8 724**, by the ledger (`F:\projects\video-work\voice\eleven\ledger.json`).
  The budget was 20 000.
- **The English gate,** passed per line out of 3:

  | Voice | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | Total |
  |---|---|---|---|---|---|---|---|---|---|
  | Jon | 2 | 3 | 3 | 3 | 3 | 3 | 3 | 3 | 23/24 |
  | Victoria | 0 | 0 | 2 | 0 | 1 | 1 | 2 | 0 | 6/24 |

  - Victoria's files start within 5–10 ms and often end loud (−14 to −30 dB): cut short,
    by the gate's rules.
  - Victoria said "Cantus" so that Whisper wrote "Countess" and "Contas" on line 8.
  - Whisper hears "modeled" as "mottled" for every engine. In American English the two
    are said alike, so the gate now takes "mottled" as the word.
- **The Spanish gate** was stopped when C: fell to 4.7 GB, a fall that went on after my
  processes ended. The page is English only for now:
  `F:\projects\video-work\voice\ab-eleven.html`.

## Step 3, the film draft (2026-11-20; built by a builder agent, reviewed)

- **The file:** `F:\projects\video-work\film\draft-1080p30.mp4`.
  - 1920 × 1080, 30 fps, 2400 frames, 80.0 s, H.264 and AAC, 23 MB.
  - The sound is each scene's own engine output, with no narration and no mastering
    (−12.4 LUFS integrated).
  - The cards are the English set, with end card B.
- **The program:** `crates/cantus-ui/examples/film.rs`, with `tools/video/film/draft.sh`
  and `sheet.sh`. ffmpeg is pinned in `tools/video/film/ffmpeg.txt` (BtbN n8.1.3, SHA-256
  checked against the release's checksums).
  - Every scene opens through `CantusApp::assemble` over a data folder of its own.
  - The manual engine is stepped 48 000 / fps samples a frame, and its output is kept as a
    WAV.
  - The look is Slate Lapis Night.
  - The cursor's clicks are real UI events.
- **The scenes:**

  | Scene | Cut | Song | What it shows |
  |---|---|---|---|
  | s1 | 0:00–0:06 | With All Your Heart | The arrangement playing; a push-in; the title card |
  | s2 | 0:06–0:14 | With All Your Heart | A pull-back to the whole window; Onda in the Stage |
  | s3 | 0:14–0:24 | Morning Light | The picker, Chorda's window, and the cursor playing four keys |
  | s4 | 0:24–0:31 | Sunday Strut | Tympanum's kit, then its mixer with 12 mics, each "Own strip" |
  | s5 | 0:31–0:44 | Neon Tide | Cribrum inserted from a strip's empty slot; its face; the cutoff dragged |
  | s6a | 0:44–0:46.7 | — | An armed guitar track with its input meter |
  | s6b | 0:46.7–0:49.3 | — | Two session clips launched |
  | s6c | 0:49.3–0:52 | — | A hosted CLAP, the repository's test gain, on a strip |
  | s7 | 0:52–1:00 | With All Your Heart | Speculum on the master, its loudness view |
  | s8 | 1:00–1:08 | With All Your Heart | A glide across the desk, ending on Onda |
  | s9 | 1:08–1:20 | With All Your Heart | End card B, with Onda's Stage inset |

- **Substitutes, and what is not real yet:**
  - The mixer has no drop target for effects, so an effect is a click on the empty slot,
    then the picker.
  - Chorda's keys light only for notes held with the mouse, so the cursor plays them.
  - The recording take is not real: the manual engine has no input stream. The track is
    armed, and the song's own guitar, rendered alone, is fed to its input.
  - The session clips are made by the arrangement drag's own command.
  - The plug-in is the repository's test gain, with no editor window (the CPU renderer
    cannot show one).
  - Onda's end-card inset is a rectangle, not keyed out.
  - The camera's zooms are soft at 1080p; they will be sharp at scale 2.
  - Hard cuts only.
  - The Surprised mood also squares Onda; the film holds Happy.
- **Looked at:**
  - The builder looked at contact sheets of every scene, full frames of each, and both
    sides of every cut.
  - The session (Claude) looked at a sheet of 11 frames across the cut, taken from the
    mp4. No overlap, no "no audio device", Lapis Night everywhere, and Onda in shots 2, 8
    and 9 only.
- **Not measured:** the frame render time. Renders ran at below-normal priority on a
  loaded machine.

## What nobody has watched or heard yet

No one has heard any take or watched the draft. Claude measured the takes, Whisper
transcribed them, and Claude looked at the frames and card stills.

*(Written by the integrator from the video agent's hand-back, 2026-11-20: the subagent's tool
policy refused it a report file. Two of its claims were checked against the code: no demo
uses Tympanum, and Chorda plays only in `ballad` and `heart`.)*
