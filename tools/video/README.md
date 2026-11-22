# tools/video — the launch video, from a clean machine to the finished film

These are the tools that make Cantus's launch video. Round 41's plan, script and report are
in `docs/round41/video/`. The owner wants every script the video uses kept here, for the
next version, for tests, and for this film alike (2026-11-20, 2026-11-21).

**Everything generated stays outside the repository,** in a work folder written `$WORK`
below (`F:\projects\video-work\` on the owner's PC): environments, models, takes, frames,
mixes and cuts. Big files never go in git (ADR 0030). Commands run from the repository's
root, in Git Bash on Windows, unless a step says PowerShell.

## The owner's decisions this pipeline encodes

| Decision | Where it lives |
|---|---|
| Slate Lapis Night in every shot. Onda is the only dancer, on its `Bounce` (never `Jump`, `Shy` or `Surprised`), only in shots 2 and 8. **No dancer on the end card.** | `crates/cantus-ui/examples/film.rs` |
| The songs: *With All Your Heart* opens and closes, *Morning Light* has the piano, Tympanum plays on *Sunday Strut*, *Neon Tide* has the mixer. | `film.rs`, `docs/round41/video/script.md` |
| **The narration is ElevenLabs:** English **Jon** (`MFZUKuGQUsGJPQjTS4wC`), Spanish **Alejandro** (`NWqMOQLlMBaUbjKYdhbW`), each line that voice's **first take** on `ab-eleven.html`. | `voice/choose.py --voice jon` / `alejandro` |
| The names in Latin's hard consonants: Cantus KAN-tus, Chorda KOR-da, Tympanum TIM-pa-num. | `voice/takes.py`'s `SPEAK` (given to every engine), `voice/gate.py`'s `NAMES` |
| **One song under the whole film,** *With All Your Heart*, with one 2 s crossfade (bar 12 to bar 56) and its last chord at the end. | `crates/cantus-ui/examples/soundtrack.rs` |
| **The voice's band is carved out of the music** (Aequalis's dynamic bells keyed by the voice, 200 Hz–5 kHz), not a full-band duck. The voice stays ≥ 10 dB over the music in the speech band on every line. | `soundtrack.rs` |
| The song is rendered with **round 41's Chorda** (DSP_VERSION 11, `r41/integration`). | `film/int_worktree.sh`, `film/draft3.sh` |
| −14 LUFS integrated, −1 dBTP. The mp4s come from a −2 dBTP export, so AAC stays under −1. | `soundtrack.rs`, `film/draft.sh` |
| **End card B** (without the verse). The platform line reads **"Windows · Linux · macOS (preview)"** / "(vista previa)". The end card is not centred. | `cards/cards.js` (`--os preview`, the default), `film/draft.sh` |
| Nothing is published without his word. | — |

## 0. A clean machine

- **Software:**
  - Rust, as `docs/DEVELOPING.md` says;
  - Python 3.12 (`py -3.12`);
  - Node 22 or newer;
  - Chrome or Edge;
  - Git Bash;
  - PowerShell 7 (`pwsh`), for the ElevenLabs helper.
- **ffmpeg** is a tool here and never ships with Cantus. `film/ffmpeg.txt` has the pinned
  build, its URL and its SHA-256. Unzip it to `$WORK/tools/ffmpeg/`.
- **A GPU** makes Whisper fast (an NVIDIA RTX 4060 laptop GPU). A CPU works, slowly.
- **The ElevenLabs key:** the owner's Starter plan, kept in Windows Credential Manager (a
  generic credential, target `cantus-elevenlabs`, user `api`).
  - `voice/eleven-api.ps1` reads it at the moment of use.
  - It is never printed, never on a command line, never in a file.
- **Downloads are untrusted data.** Each goes in its own folder, is checked against its
  SHA-256, and is read only by Python run with `-I`.
  - Behind the corporate proxy, a download may arrive as an HTML page for about 10 minutes.
    `fetch_models.sh` detects this and waits.
  - Never bypass the proxy or TLS.
- **On a shared machine,** every heavy job goes through `film/gently.sh`.
  - It waits until RAM has 6 GB free and C: has 6.5 GB.
  - It runs at below-normal priority.
  - `film/watchdog.sh` stops it when RAM falls under 4 GB or C: under 6 GB.
  - Cargo goes through `tools/build/slot.sh` with `-j 2`.

## 1. The facts and the script

Every bracketed fact in `docs/round41/video/script.md` is checked against the code by hand;
the report says how it was done at `74f202b1`. Then check each narration line's estimated
length against its window:

```text
py -3.12 -I tools/video/voice/pace.py docs/round41/video/script.md
```

## 2. The voice

**The environment.** One venv carries Whisper (the gate) and the Qwen tools;
`requirements.txt` is the Chatterbox venv's, for re-running those rounds.

```text
py -3.12 -m venv $WORK/env/qwen
$WORK/env/qwen/Scripts/python -m pip install torch==2.6.0 torchaudio==2.6.0 --index-url https://download.pytorch.org/whl/cu124
$WORK/env/qwen/Scripts/python -m pip install -r tools/video/voice/requirements-qwen.txt
bash tools/video/voice/fetch_models.sh $WORK whisper     # the gate's ear; add chatterbox qwen to re-run those
PY=$WORK/env/qwen/Scripts/python
```

**The ElevenLabs takes** (the narration the film uses): three takes per line per voice,
counted against a character budget.

```text
pwsh -File tools/video/voice/eleven_voices.ps1 -Out $WORK/voice/eleven -Search -Add    # the library voices, into the account
$PY -I tools/video/voice/takes_eleven.py --lang en --voice MFZUKuGQUsGJPQjTS4wC --name Jon       --out $WORK/voice/eleven/en
$PY -I tools/video/voice/takes_eleven.py --lang en --voice qSeXEcewz7tA0Q0qk9fH --name Victoria  --out $WORK/voice/eleven/en
$PY -I tools/video/voice/takes_eleven.py --lang es --voice NWqMOQLlMBaUbjKYdhbW --name Alejandro --out $WORK/voice/eleven/es
$PY -I tools/video/voice/takes_eleven.py --lang es --voice hHjbwzYZW17oh0p05AKv --name Gabriela  --out $WORK/voice/eleven/es
```

- The ledger is `$WORK/voice/eleven/ledger.json`. It counts every character before each
  call, stops at `--budget` (20 000), and never pays twice for a take that exists.
  2026-11-21 spent 8 724 characters.
- The audio is WAV, 48 kHz 16-bit, from `eleven_multilingual_v2`.

**The gate** (Whisper on the GPU) and the comparison page:

```text
for L in en es; do $PY -I tools/video/voice/gate.py --takes $WORK/voice/eleven/$L --whisper $WORK/models/whisper --model large-v3-turbo; done
$PY -I tools/video/voice/compare.py --engine eleven --per-voice 2 \
    --pair en $WORK/voice/pool/en $WORK/voice/eleven/en --pair es $WORK/voice/pool/es $WORK/voice/eleven/es --out $WORK/voice/ab-eleven.html
bash tools/video/voice/page_shot.sh $WORK/voice/ab-eleven.html $WORK/voice/ab-eleven.png 6400    # look before the owner does
```

`gate.py` checks every take and writes `gate.json`:
- **The words:** transcribed back by Whisper. A name may be spelled within two letters of
  the script with the same first sound; in Spanish, b and v count as one sound, and so do
  a vowel with or without its written accent.
- **The signal:** no flat-top clipping and no DC.
- **The edges:** a head of at least 10 ms. An end at least 30 dB under the loudest 20 ms
  (louder means a word was cut off), with at most 700 ms of trailing sound.
- **The window:** the speech fits its window.
- **Reported, not gated:** pace and loudness.

`gate.py --recheck` re-applies the rules to `gate.json` without running Whisper.

**The chosen voice track** (the owner's decision) is the film's input:

```text
py -3.12 -I tools/video/voice/choose.py --takes $WORK/voice/eleven/en --voice jon       --out $WORK/voice/chosen/en
py -3.12 -I tools/video/voice/choose.py --takes $WORK/voice/eleven/es --voice alejandro --out $WORK/voice/chosen/es
```

- It writes `chosen/<lang>/01.wav … 08.wav` and `chosen.json`.
- Per line, the manifest gives: the source take, the text, the line's start and window,
  where the speech begins, its length, its pace, what Whisper heard, and the voice and its id.
- `--pick <line>=<take>` overrides one line.

**Engines tried and set aside,** kept for the next version (the report has each round):

- **Chatterbox Multilingual** (MIT), the stock voice.
  - `takes.py --ref` clones a voice, only ever the owner's own, cut by `ref.py`.
  - `--phrases` is off, because the model babbles after short inputs.

  ```text
  py -3.12 -m venv $WORK/env/tts && $WORK/env/tts/Scripts/python -m pip install torch==2.6.0 torchaudio==2.6.0 --index-url https://download.pytorch.org/whl/cu124 && $WORK/env/tts/Scripts/python -m pip install -r tools/video/voice/requirements.txt
  bash tools/video/voice/fetch_models.sh $WORK chatterbox
  T=$WORK/env/tts/Scripts/python
  $T -I tools/video/voice/takes.py --lang en --models $WORK/models/chatterbox --out $WORK/voice/takes/en
  $T -I tools/video/voice/breathe.py --from $WORK/voice/takes/en --to $WORK/voice/pool/en --tag t     # the take's own pauses opened with silence
  $PY -I tools/video/voice/gate.py --takes $WORK/voice/pool/en --whisper $WORK/models/whisper
  $PY -I tools/video/voice/abpage.py --takes $WORK/voice/pool/en --takes $WORK/voice/pool/es --out $WORK/voice/ab.html
  ```

- **Qwen3-TTS** (Apache-2.0): built-in English voices, and designed Latin American Spanish
  voices held steady through the Base model. The owner found it the worst of the three.

  ```text
  bash tools/video/voice/fetch_models.sh $WORK qwen
  bash tools/video/voice/qwen_run.sh $WORK
  ```

- `ab.html`, `ab-qwen.html` and `ab-eleven.html` are the three listening pages.

## 3. The cards

```text
node tools/video/cards/render.mjs $WORK/cards/stills --lang en --scale 1 --card end-b --still 6    # one still to look at
$PY -I tools/video/cards/view.py $WORK/cards/stills                                                # stills on a plate
bash tools/video/film/draft.sh cards en                                                            # every card the cut uses, 1080p30
```

- **The sources:** `cards/cards.html`, `.css` and `.js`, in the site's tokens. Logo #03 is
  inlined from `crates/cantus-ui/assets/logos/03.svg`; Barlow Semi Condensed comes from
  `web/assets/fonts`.
- **The motion:** `cardSetTime(t)` sets every value, so each frame is exact.
- **The renderer:** `render.mjs` serves the repository's root to headless Chrome through
  `web/tools/cdp.mjs` and writes transparent PNGs.
- **Options:** `--fps`, `--scale`, `--card`, `--dur <card>=<s>`, `--still <s>`, and
  `--os preview|all` for the end card's platform line.

## 4. The film's picture

`crates/cantus-ui/examples/film.rs` is the director's script in code.
- Each scene opens through `CantusApp::assemble` over a data folder of its own.
- The manual engine is stepped `48 000 / fps` samples a frame.
- The cursor is drawn, and its clicks are real UI events. The look is Lapis Night.
- It writes `$WORK/film/<scene>/00000.png…` and `<scene>.json`.

```text
bash tools/video/film/draft.sh render                 # build (release, through slot.sh) and render every scene
target/release/examples/film.exe --out $WORK/film --scene s7-master     # one scene again (--every N, --limit N to look quickly)
bash tools/video/film/sheet.sh $WORK/film/s7-master $WORK/film/s7.png   # a contact sheet of a scene
```

- `draft.sh render` also builds the test plug-in that shot 6c hosts
  (`crates/cantus-test-plugins`).
- The final is the same program with `--fps 60 --scale 2` (3840 × 2160). It is made only
  after the owner approves the draft.

## 5. The soundtrack, mixed in Cantus (one project per language)

`crates/cantus-ui/examples/soundtrack.rs` builds a real Cantus project and renders it
through Cantus's own export:

1. *With All Your Heart* is rendered by `cantus_export::export_mix` to `heart.wav`.
2. **The music track:** the Heart from bar 1, with one 2 s equal-power crossfade from
   bar 12 into bar 56 (ADR 0070's overlap), and the last chord ringing to the end.
3. **The voice track:** the eight chosen lines at their starts, each lead trimmed to 60 ms.
4. **The carve:** Aequalis on the music strip, with four dynamic bells keyed by the voice
   strip.
   - The bells sit at 320 Hz, 850 Hz and 2.1 kHz (Q 1.0) and 4.3 kHz (Q 1.2).
   - Threshold −60 dB, ratio 20, attack 40 ms, release 450 ms.
   - It is deepened until every line stands `--margin` (10) dB over the music in the speech
     band.
   - The bed stays at −3 dB and is lowered only if the deepest carve cannot reach the margin.
5. **The master:** the export's BS.1770-4 stage and Terminus's true-peak limiter, to
   −14 LUFS and −1 dBTP, 48 kHz 24-bit. A −2 dBTP twin is made for the lossy encode.

Round 41's Chorda is on `r41/integration`, so the soundtrack is built there, in a throwaway
worktree that keeps `r41/video`'s history its own:

```text
bash tools/video/film/int_worktree.sh make      # r41/video-int from r41/integration at F:/projects/daw-r41/video-int, r41/video merged in
bash tools/video/film/int_worktree.sh build     # the soundtrack example, release, -j 2 (its log prints DSP_VERSION)
bash tools/video/film/int_worktree.sh merge     # after a change on r41/video
```

**Its output,** in `$WORK/soundtrack3/` (draft 3, DSP 11; draft 2's DSP-10 mix stays in `$WORK/soundtrack/`):
- `heart.wav`;
- the project, `soundtrack-<lang>.cantus`;
- `soundtrack-<lang>.wav` and `-web.wav`;
- the stems `-music.wav` and `-voice.wav`;
- `soundtrack-<lang>.json`: the edit, the margins per line, the music's drop at each entry,
  the loudness as Cantus measured it, and the DSP_VERSION.

## 6. The cut

```text
bash tools/video/film/draft3.sh en      # the DSP-11 soundtrack, end-b re-rendered, the cut, the checks
bash tools/video/film/draft3.sh es
```

- `draft3.sh` runs each heavy step through `gently.sh`.
- `draft.sh cut <lang>` with `DRAFT=<name>` does the cut itself:
  - it re-encodes only the scenes rendered since the last cut;
  - it joins them in the script's order and lays the cards over;
  - it puts the web soundtrack under;
  - it writes `$WORK/film/<DRAFT>-<lang>-1080p30.mp4`.

## 7. The checks

```text
bash tools/video/film/check_audio.sh $WORK/soundtrack3/soundtrack-en.wav $WORK/film/draft3-en-1080p30.mp4   # LUFS, LRA, true peak (ffmpeg)
bash tools/video/film/cut_frames.sh $WORK/film/draft3-en-1080p30.mp4 $WORK/film/cuts-en.png               # both sides of every cut, and the end card
bash tools/video/film/review_sheet.sh $WORK/film/draft3-en-1080p30.mp4 $WORK/film/sheet-en.png             # chosen frames as one sheet
bash tools/video/film/heart_diff.sh $WORK/soundtrack/heart.wav $WORK/soundtrack3/heart.wav 0 16.55       # the intro's Chorda, DSP 10 against 11
```

**What to look for:**
- **The frames:** no overlap, no clipped label, no "no audio device", Lapis Night
  throughout, Onda only in shots 2 and 8, the end card alone.
- **The sound:** −14 LUFS, nothing over −1 dBTP in the mp4.
- **The carve:** every line ≥ 10 dB over the music, and the music's full-band drop small
  (`$WORK/soundtrack3/soundtrack-<lang>.json`).

## 8. The finished files (the owner approved draft 3 on 2026-11-21)

`tools/video/final/` makes the finished files. The 4K render is heavy: it is held while a
test suite's timing tests or a CPU measurement run.

```text
bash tools/video/final/build.sh captions          # cantus-<lang>.srt, chapters-<lang>.txt / .ffmeta (final/captions.py)
bash tools/video/final/build.sh render            # the scenes at 3840 x 2160, 60 fps, into $WORK/film4k (both languages share them)
bash tools/video/final/build.sh cards en          # the cards at scale 2, 60 fps; then es
bash tools/video/final/build.sh all en            # master, YouTube, site, vertical, then the loop; then: all es
py -3.12 -I tools/video/final/checks.py --work $WORK   # files, loudness, sync at three marks, the carve, the captions
```

`DRY=1` prints a step's commands without running them.

**The files,** in `$WORK/final/`:

| File | What |
|---|---|
| `cantus-<lang>-master.mkv` | 3840 × 2160, 60 fps, FFV1 (lossless), 48 kHz 24-bit PCM from the −1 dBTP master, chapters. Kept by the owner. |
| `cantus-<lang>-2160p.mp4` | YouTube: H.264 High CRF 16, 60 fps, AAC 320 k from the −2 dBTP export, chapters. |
| `cantus-<lang>-1080p.mp4` / `.webm` | The site: H.264 and VP9, two passes to at most 12 MB, 30 fps, captions burned in (Barlow Semi Condensed on a translucent slate box), chapters. |
| `cantus-poster.jpg` | The title card drawn, at 3.5 s: the same in both languages. |
| `cantus-loop.mp4` / `.webm` | The site's hero: 0:31–0:49, the mixer at work, muted, no captions, faded at both ends. |
| `cantus-<lang>-vertical.mp4` | 1080 × 1920, the first 30 s: the frame's middle three quarters over itself blurred, captions under it. |
| `cantus-<lang>.srt`, `chapters-<lang>.txt` | For YouTube: captions and description chapters. |

**How the captions are made:** they follow the speech as placed. Each line's speech begins at
its `start`, so the time is shared by the characters each caption holds. Each line is at most
42 characters and each caption at most two lines, broken at sentences, then at clauses. Lines
are balanced, never start with a dash, and never end on a small word.

**What `checks.py` checks:**
- every file's size, length, picture and sound against its spec;
- −14 LUFS ± 0.5 and true peak ≤ −1 dBTP (ffmpeg's ebur128);
- sync at three marks, s1 at 3 s, s7 at 55 s, s8 at 64 s: the song's place the picture shows
  against the place the soundtrack plays, and their measured cross-correlation, both within a
  frame;
- every line's margin and the music's drop at its entry;
- the captions' rules.

Nothing is published without the owner's word. The site's deploy and YouTube go through the
integrator session.
