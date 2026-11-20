# The launch video — the session's report

*Branch `r41/video` from `r41/video-plan` (`74f202b1`), worktree `F:/projects/daw-r41/video`.
Generated files live in `F:\projects\video-work\`, never in the repository.*

| Step | State |
|---|---|
| 1. Facts | done (`ae88ebae`); script corrected |
| 2. The voice | the owner chose the **stock voice** (2026-11-20); environment, models and tools ready; no take made yet (the machine is held for the integrator's suites) |
| 3. The film | not started (needs the build slot after "suites done") |
| 4. The cards | sources written (`tools/video/cards/`), both end cards A and B; **not rendered, not looked at** |
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

## What nobody has watched or heard yet

Everything.

*(Written by the integrator from the video agent's hand-back, 2026-11-20: the subagent's tool
policy refused it a report file. Two of its claims were checked against the code: no demo
uses Tympanum, and Chorda plays only in `ballad` and `heart`.)*
