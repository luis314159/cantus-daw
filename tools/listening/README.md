# listening — blind tests and listens for the owner and for strangers

Round 40's listening tools (round 42: one page for every instrument, Tympanum's kit): where each one lives, the exact commands, how a kit is published
as a claude.ai page, and how the results come back. The measures decide what is *tried*; these
decide what *ships* — the owner's ear outranks every number.

| what | tool | for |
|---|---|---|
| the blind kits' one page | `crates/cantus-ui/examples/abx_kit/` (`abx.html`, `kit.rs`, `names.rs`); each instrument's subject file (`chorda_kit/abx_subject.json`, `tympanum_abx/subject.json`) | every kit below |
| Chorda's blind test (ABX + preferences, three pianos) | `cantus-ui` example `chorda_abx` | the owner and other listeners |
| Tympanum's blind test (ABX + preferences: generated, hybrid, recorded) | `cantus-ui` example `tympanum_abx` | the owner and other listeners |
| a listener's results | `tools/listening/abx_read.py` (per item too) | reading what came back |
| the page's browser check | `crates/cantus-ui/examples/abx_kit/abx_browser_check.mjs` | before sending a kit |
| Chorda's named previews | `tools/chorda-voicing/preview.py` | the owner, each candidate (not blind) |
| Tympanum's listens (previous · now · recording) | `tools/tympanum-fit/make_listen.py`, `make_b4.py` | the owner, each drum iteration |
| Tympanum's representation ladder ("escalera": MUSHRA + ABX) | `tools/tympanum-fit/ladder_kit.py` | the owner, before a model 6 |

## Chorda's blind test (`chorda_abx`)

```bash
cargo run --release -p cantus-ui --example chorda_abx -- <kit dir> \
    --candidate tools/chorda-voicing/candidates/c16/spec.txt \
    --partials tools/chorda-voicing/candidates/c16/partials.csv \
    --levels tools/chorda-voicing/candidates/c16/levels.csv \
    --name "candidate 16" --downloads <downloads dir> [--wav | --flac]
```

Three pianos: Chorda at the candidate ("Chorda 3") and Chorda 2 at beta.14's voicing (both
through the lab: model 2's DSP at a spec, beta.14's being the empty one), and the Salamander
Grand through the engine (`--downloads` is where it is installed, through Cantus's own pack
code — pass it: the built-in default is a round-37 folder). Every phrase brought to
−20 LUFS (lowered together if a peak would pass −1 dBFS), 30 ms fades, file names 64 random bits
(no two share structure: the generator redraws until they do not), two catch questions (one clip
under two names). `<kit dir>` holds only what a listener receives (`index.html`, `audio/`,
`LEEME.md`, `README.md`); **`<kit dir>-key`** (never sent) holds `key.json` (which piano each
token and file is, the candidate's files) and `levels.csv`. The method and the checks:
`docs/round40/abx5.md`.

- **Offline kit** (default, `--flac`): 24-bit FLAC, zip the folder and send it; the page runs from
  `file://`, keeps the answers in the browser and offers them as a JSON file to send back.
- **Published kit** (`--wav`): claude.ai serves an artifact's media as .mp3/.wav/.ogg/.mp4/.webm,
  **not .flac** — make the online kit with `--wav` (the same samples as 24-bit WAV).

### Publishing it as a claude.ai page

Publish `index.html` as the page with `audio/*` as its files, declaring:

```js
capabilities: {
  user: {},
  downloads: true,
  db: { rules: [
    { path: "abx",        read: "owner",    write: "owner" },
    { path: "abx/{self}", read: "interact", write: "interact" }
  ] }
}
```

- Each listener reads and writes only `abx/<their id>`; only the owner reads the collection
  (the page's `#summary`: each listener's score, p, catch questions, the pooled groups) and
  imports files sent by people who could not save online. Members of the owner's organisation
  at Contributor or above save online; visitors from outside hold `view` and are told to
  download the JSON and send it.
- **One publish takes at most 64 MB.** A WAV kit is ~68 MB: publish the page with part of
  `audio/` first, then publish again to the same artifact with the rest (a later publish adds
  to the files already there).
- The page waits up to 3 s for `window.claude` (the viewer may attach it after the script's
  first run) before it treats itself as offline — keep that wait if the page is edited.
- Before sending: `PW_DIR=<folder with node_modules/playwright-core> node
  crates/cantus-ui/examples/abx_kit/abx_browser_check.mjs <kit dir>` (Edge, Chrome and
  Firefox, over http and file://, plus the published mode against a stub of `window.claude`).

### One page, a subject per instrument (round 42)

The page (`abx_kit/abx.html`) knows nothing of pianos or drums: a generator fills `__SUBJECT__`
with its instrument's subject file — the texts laid over the page's own (the lead, the help of
each part, the sources' names `voice_<id>`, the credits), the items' names, the pairs the
preferences count, the trials of each ABX, the form's instrument question (`play`: its answer is
`about.<play>`), the seconds a trial and a question take. Part 1's trials draw every item as
often as the others. `abx_kit/kit.rs` holds what the generators share (the loudness match, the
page's fill, the writers). The browser check reads the counts, names and credits from the kit's
`SUBJECT`, and what must stay hidden from its own table by the subject's `id`; with
`--mock <abx.html> <subject.json>` it builds a stand-in kit of tones for any subject.

## Tympanum's blind test (`tympanum_abx`)

```bash
cargo run --release -p cantus-ui --example tympanum_abx -- <kit dir> \
    [--downloads <dir>] [--key <dir>] [--wav | --flac]
```

Tympanum as it ships — the modelled kit's generated voice and its *Hybrid* voice — against the
recording each modelled kit was measured from (the studio kit and DRSKit, the jazz kit and
Virtuosity Drums), the same MIDI through each, through the engine's offline render, each kit's
own mix of its microphones. `--downloads` (default `F:/projects/daw-r42/downloads/tympanum`)
holds `content/`, where `drskit-tympanum`, `virtuosity-drums`, `tympanum-hybrid-studio` and
`tympanum-hybrid-jazz` are installed through Cantus's own pack code unless they are there at
the manifest's version (the recorded kits are 1.9 and 1.5 GB: a junction to an earlier
download at the same version saves fetching them again). The tool prints each hybrid residual's
model hash against the build's and refuses a kit whose hybrid is not heard. Twelve items (eight
pieces alone at two or three velocities, four grooves: pop/worship, rock, bossa nova, a ballad
on the jazz kit with crash swells), −23 LUFS (lowered together for a −1 dBFS peak), 100 ms
fades, 80 ms of silence before the first hit. Part 1: ABX generated vs recording (24 trials,
each item twice); part 2: 26 preferences (every item generated vs recording and hybrid vs
recording, two catch questions); extra: ABX hybrid vs recording (24). `<kit dir>-key` holds
`key.json` (tokens, files, items, packs, residuals) and `levels.csv` (loudness before and after,
the stereo balance and width, the first onset, per file). Zip the folder to send it; publish a
`--wav` kit as Chorda's. The method and what was checked: `docs/round42/tymp-abx.md`.

### Reading results

```bash
python -I tools/listening/abx_read.py <results.json> [<kit dir>-key/key.json]
```

The form, the ABX score with its one-sided binomial p, every trial (plays, changes) and every
preference with its note; with the key, tokens become piano names. **Results files are personal
data** (a listener's answers and what they typed): keep them out of the repository — record
the scores, not the files, in the round's notes.

## Chorda's named previews

`tools/chorda-voicing/preview.py <out dir> "<label>=<wav dir>" …` — every listening phrase from
each source at −20 LUFS, 24-bit FLAC, named plainly (`<phrase> - <label>.flac`); with a
`NOTA.md` in Spanish for the owner (what changed, what to listen for) and a zip. The round's
labels: `1 Chorda 2 (beta.14)`, `2 candidato N-1`, `3 candidato N`, `4 Salamander` (the
analysts' scripts read those labels).

## Tympanum's listens

- `make_listen.py <out> <prev hits> <new hits> <recorded hits> <prev groove> <new groove>
  <recorded groove> <kit tag> [pieces]` — each piece at four velocities and the groove, three ways
  (`-1-antes`, `-2-nuevo`, `-3-grabado`), at one loudness. The hits come from
  `tympanum_measure hits`, the grooves from `tympanum_measure ab` (`tools/tympanum-fit/README.md`).
- The ladder's blind kit: `python -I tools/tympanum-fit/ladder_kit.py build <ladder dir> <kit dir>`
  (the key in `<kit dir>-key`), `ladder_kit.py score <key dir> <results.json>`;
  `docs/round40/ladder.md` has the method. Offline only (no online saving).
