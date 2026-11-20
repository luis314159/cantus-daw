# tools/video — the launch video, made again from nothing

The tools that make the Cantus launch video (`docs/round41/video/`: the plan, the script
and the report). They stay in the repository for the next version, for tests, and for this
video alike (the owner, 2026-11-20). **Everything they generate goes in a work folder
outside the repository**, `F:\projects\video-work\` on the owner's PC (written `$WORK`
below): environments, models, takes, frames and renders. Big files never go in git
(ADR 0030).

Steps run in order; each is checked before the next. Downloaded models are untrusted data:
each in its own folder, checked against its SHA-256, read by Python run with `-I`. Behind
the corporate proxy a download may come back as an HTML page for ~10 minutes: check what
arrived (a `.pt` file starts with `PK`, a `.safetensors` with its JSON header), wait, retry.
Never bypass the proxy.

## 0. What a machine needs

- Python 3.12 (`py -3.12`), Node 22 or newer, Chrome or Edge, ffmpeg (a tool here, never
  shipped), Rust as `docs/DEVELOPING.md` says.
- An NVIDIA GPU makes the voice fast (an RTX 4060 makes a take in ~10 s). A CPU works, slowly.

## 1. The facts

`python -I tools/video/voice/pace.py docs/round41/video/script.md`: every narration line's
estimated length against its window (words a second in English, syllables in Spanish).
Every bracketed fact in `script.md` is checked against the code by hand; the report says how.

## 2. The voice

The environment (pins in `voice/requirements.txt`):

```text
py -3.12 -m venv $WORK/env/tts
$WORK/env/tts/Scripts/python -m pip install torch==2.6.0 torchaudio==2.6.0 --index-url https://download.pytorch.org/whl/cu124
$WORK/env/tts/Scripts/python -m pip install -r tools/video/voice/requirements.txt
```

Set `PIP_CACHE_DIR` and `HF_HOME` under `$WORK/cache` first if the system disk is small.

The models (files, sizes and SHA-256s in `voice/models.txt`; check every one with
`sha256sum`):

```text
# Chatterbox Multilingual (MIT), revision 5bb1f6ee58e50c3b8d408bc82a6d3740c2db6e18
for f in ve.pt t3_mtl23ls_v2.safetensors s3gen.pt grapheme_mtl_merged_expanded_v1.json conds.pt Cangjie5_TC.json; do
  curl -sSL -o $WORK/models/chatterbox/$f https://huggingface.co/ResembleAI/chatterbox/resolve/5bb1f6ee58e50c3b8d408bc82a6d3740c2db6e18/$f
done
# Whisper large-v3 (MIT), the gate's ear
curl -sSL -o $WORK/models/whisper/large-v3.pt https://openaipublic.azureedge.net/main/whisper/models/e5b1a55b89c1367dacf97e3e19bfd829a01529dbfdeefa8caeb59b3f1b81dadb/large-v3.pt
```

On its first run the package also fetches spacy-pkuseg's Chinese segmentation model to
`~/.pkuseg` (unused for English and Spanish).

Not the F5-TTS checkpoints: they are CC-BY-NC, which ADR 0042 keeps out.

The takes, the gate and the page (`PY=$WORK/env/tts/Scripts/python`):

```text
$PY -I tools/video/voice/takes.py --script docs/round41/video/script.md --lang en --models $WORK/models/chatterbox --out $WORK/voice/en
$PY -I tools/video/voice/takes.py --script docs/round41/video/script.md --lang es --models $WORK/models/chatterbox --out $WORK/voice/es
$PY -I tools/video/voice/gate.py --takes $WORK/voice/en --whisper $WORK/models/whisper
$PY -I tools/video/voice/gate.py --takes $WORK/voice/es --whisper $WORK/models/whisper
$PY -I tools/video/voice/abpage.py --takes $WORK/voice/en --takes $WORK/voice/es --out $WORK/voice/ab.html
```

- `takes.py` makes six takes a line from a fixed grid of seed, exaggeration, cfg weight
  and temperature, so any take can be made again exactly. Without `--ref` it uses the
  model's stock voice (version 1, the owner's choice). With `--ref <wav>` it clones a voice,
  only the owner's own, cut with `ref.py`.
- `gate.py` checks every take: the words, transcribed back by Whisper; clipping; DC; the
  head and tail silence; the pace; whether it fits its window. It also reports loudness.
  The result goes in `gate.json`.
- `abpage.py` offers the two passing takes nearest the pace's middle, per line, for the
  owner's ear. He picks. Claude cannot hear, so it never picks.

## 4. The cards

```text
node tools/video/cards/render.mjs $WORK/cards/stills --lang en --scale 1 --card end-a --still 6   # one still
$PY -I tools/video/cards/view.py $WORK/cards/stills                                                # on a plate, to look at
node tools/video/cards/render.mjs $WORK/cards/frames --lang en                                     # every card, 3840 x 2160, 60 fps
```

- The cards are `cards/cards.html`, `.css` and `.js`, in the site's tokens. Each is moved
  by one time value per frame.
- `render.mjs` serves the repository's root to headless Chrome (through
  `web/tools/cdp.mjs`) and writes transparent PNGs.
- The names of the cards are in `cards.js`'s `CARDS`.

## 3, 5, 6, 7. The film, the soundtrack, the assembly, the checks

Each section gets its commands here as its tools land: `film.rs`
(`crates/cantus-ui/examples/`), the soundtrack project, `build.sh`, and the checks
(loudness, sync, captions, the frames' look).
