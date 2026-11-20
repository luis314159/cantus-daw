# The launch video — the session's report

*Branch `r41/video` from `r41/video-plan` (`74f202b1`), worktree `F:/projects/daw-r41/video`.
Generated files live in `F:\projects\video-work\`, never in the repository.*

| Step | State |
|---|---|
| 1. Facts | done (`ae88ebae`); script corrected |
| 2. The voice | **the A/B page is ready for the owner's ear**: `F:\projects\video-work\voice\ab.html`, two gated takes per line in English and Spanish (below, "Step 2, rounds 2–3") |
| 3. The film | not started; cargo is allowed again (the build slot, `-j 3`), and it waits for the takes the owner picks |
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

## What nobody has watched or heard yet

No one has heard any take. Claude measured them, and Whisper transcribed them. The card
stills have been looked at by Claude only.

*(Written by the integrator from the video agent's hand-back, 2026-11-20: the subagent's tool
policy refused it a report file. Two of its claims were checked against the code: no demo
uses Tympanum, and Chorda plays only in `ballad` and `heart`.)*
