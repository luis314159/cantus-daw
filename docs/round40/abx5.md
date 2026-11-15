# Round 40 — the listening kit for other listeners (kit 5)

The owner wants Chorda's blind test (Chorda against the Salamander Grand, a recorded Yamaha C5,
CC BY 3.0 by Alexander Holm) taken by other people, not only by him. Kit 4 was made for one
listener who knew what it was; kit 5 is made for strangers. Branch `r40/abx5` (from
`r40/chorda`); the tool is still `chorda_abx` (`crates/cantus-ui/examples/chorda_abx/`,
`chorda_kit/abx.html`, the two readmes) and the check is new
(`chorda_kit/abx_browser_check.mjs`).

## Making a kit

```text
cargo run --release -p cantus-ui --example chorda_abx -- <dir>
    --candidate <spec file> [--partials <csv>] [--levels <csv>] [--name <label>]
    [--downloads <dir>] [--key <dir>] [--wav]
```

The candidate is a parameter (its spec's first line; its per-key tables, as the lab reads
`CHORDA_PARTIALS` / `CHORDA_LEVELS` — the environment's are used when the flags are absent), so
the same command makes a kit with the next candidate. Then zip the folder.
`<dir>` holds only what a listener receives (`index.html`, `audio/`, `LEEME.md`, `README.md`);
`<dir>-key` (never sent) holds `key.json` (which piano every token and file is, the candidate's
files and spec) and `levels.csv` (each file's loudness before and after the match, by piano).

The listeners' kit (`F:\projects\daw-r40\compartir\cantus-chorda-abx5\` and its `.zip`) carries
**candidate 7**:

```text
set V=F:\projects\daw-r40\voicing
cargo run --release -p cantus-ui --example chorda_abx -- F:\projects\daw-r40\compartir\cantus-chorda-abx5 ^
    --candidate %V%\candidate7.txt --partials %V%\candidate6b-partials.csv ^
    --levels %V%\candidate7-levels.csv --name "candidate 7"
```

A candidate with knobs newer than the branch needs `r40/chorda` merged first (candidate 7's
`detune_top`, `hf_top`, `vel_top`, `uni_weight_bass`, `detune_bass`, `itd_ends` came with the
merge of 124ff6ae). Each kit's clips were compared with the plainly named previews rendered by
the voicing builder: candidate 6's kit against `chorda-preview-6` and candidate 7's against
`chorda-preview-7`, every clip of the three pianos the same samples up to one gain (residual
−118 to −141 dB: 24-bit quantisation; the gain 0 dB except where a phrase's group target
differs, ±0.2–0.7 dB). The levels (`-key\levels.csv`): every file at −20 LUFS (middle C −21.1,
the fortissimo chord −20.4, lowered together for a peak), every peak at or under −1.00 dBFS —
the first draw had middle C's candidate at −0.81, the second loudness pass lifting the peak that
had set the target; all three now come down together by the excess. 21 renders + 2 catch copies,
24.0 MB (FLAC 24-bit, 48 kHz; the zip 23.9 MB). This draw's catch questions: the top octave
(recorded) and the bass notes (candidate).

## What changed, and why

1. **The blinding leak.** Kit 4 named a file by an FNV hash of `phrase|piano` with one salt per
   kit; the inputs of a phrase differ in a byte, so its three files shared 15 of 16 hex digits
   (`2c0dba9592e70815` / `2c0dbb9592e702c2`): a glance at the URLs grouped them and showed the
   odd one out. Every name is now 64 bits from a keyed PRF (SipHash under the standard
   library's per-process random key, over a counter): nothing of the phrase or the piano is in
   it. The generator redraws a kit whose names share more than 6 leading or 9 positional
   digits (`names::names_look_random`, ~10⁻⁶ by chance; kit 4's names fail it — a test); the
   browser check measures the shipped kit again. The page no longer carries the pianos' names
   in its data either: it knows each piano by a random 8-digit token, and which token is which
   piano is a veiled table (`REVEAL`, base64 of the JSON XOR a xorshift stream) read only for
   the reveal — not secret from someone reading the code, but nothing at a glance.
2. **The pianos.** Chorda 3 = the candidate (candidate 7 in the kit that goes out; candidate 6,
   `candidate6.txt` + `candidate6b-partials.csv` + `candidate6e-levels.csv`, made the check kit
   `cantus-chorda-abx5-c6check`), through the voicing lab as the previews,
   Chorda 2 = beta.14's defaults through the engine, the Salamander through Cantus's own pack
   code and the engine (as kit 4).
3. **Written for listeners.** The page and the readmes say "three pianos", "A / B", "Clip 1 /
   Clip 2"; no cargo, rounds, voicing, candidates or the owner's name. The credits (the
   Salamander, CC BY 3.0, Alexander Holm, with links) and the names of the pianos appear only
   after the main test, with the reveal. Spanish and English by a toggle, the default from the
   browser's language (Spanish when it is neither).
4. **Shorter, in parts.** Part 1: the ABX of the candidate against the Salamander (16 trials).
   Part 2: 16 blind preferences — every phrase for the candidate against the Salamander and
   against Chorda 2, and two catch questions. The ABX of Chorda 2 against the candidate is an
   optional extra, offered at the end. Progress ("5 de 16", a bar, the minutes left), ~15–20
   minutes said up front; *← Previous* / *Next →* to revisit an answer (a change is counted);
   answers kept in the browser, a reload carries on where it was ("welcome back").
5. **About the listener**, before the start: playback (headphones / studio monitors / computer
   or phone speakers — the one required answer), piano (no / a little / yes, and years), audio
   work (no / hobby / professional), age range (optional, for hearing), a name or nickname
   (optional).
6. **Catch questions.** Two preference questions play one clip under two names (one phrase of
   the Salamander, one of the candidate, phrases drawn per kit; never the first question). The
   expected answer is "no preference"; nothing announces them. The owner's summary counts them
   and pools the listeners who passed both apart.
7. **Collecting results.** Offline: *Download results (JSON)*. Published as a claude.ai page:
   each listener's document `abx/<their id>` is written as they go (kit id, nickname, the form,
   every answer with its time, plays per slot, notes, start / end times, browser) through `db`,
   the download goes through `downloads`, and the owner (`user.isOwner()`) gets a summary:
   every listener's ABX score and p, preferences per pair, catch questions, minutes, notes; and
   the pooled ABX (k / N, p, how many individually significant) and preferences (sign test) for
   everyone, by playback device, by piano experience, and for those who passed both catch
   questions. The summary also imports listeners' JSON files (and works offline at
   `index.html#resumen` on imported files only). A listener's page never reads the collection;
   the rules below make that the server's rule, not only the page's.
8. **Loudness and length as kit 4**: one length per phrase, a 30 ms fade, BS.1770 integrated at
   −20 LUFS, lowered together when a peak would pass −1 dBFS; 24-bit FLAC at 48 kHz.

## The audio, and every control checked

The owner found the loop button dead in earlier kits: it worked, but nothing redrew it — the
page repainted the play buttons and never the loop's state. Kit 5's transport is rebuilt:

- **Over http(s)** (a published page) every clip is decoded once and played through Web Audio:
  switching A / B / X is sample-exact with a 5 ms crossfade, the loop is the source's own, the
  position bar is clickable to jump. **Opened from a folder** (`file://`, where a page may not
  fetch its own files) or where a clip will not decode (an old Safari's FLAC), one `<audio>`
  per slot. Pressing the slot that plays starts it over; any other carries on from the place.
- Keys: `A` `B` `X` (`1` `2` in the preferences), space stops (and plays the last slot again),
  `L` loops; never while typing a note; space on a focused button never presses it.

`node chorda_kit/abx_browser_check.mjs <kit>` (Playwright, `PW_DIR` the folder holding
`playwright-core`) drives the page in **Edge and Chrome (installed) and Firefox (Playwright's
132)**, each over http (Web Audio) and file:// (`<audio>`), clicking as a listener does and
reading the page's state and audio position after each step:

| control | how it was verified (every browser × http and file://) |
|---|---|
| language toggle ES / EN, default from the browser | `es-ES` opens in Spanish, `en-US` in English; each button retitles the page and marks itself pressed; switching while a clip plays keeps it playing |
| the form | Start disabled until a playback device is chosen; every pill, the years box (shown for "a little"/"yes"), the nickname recorded in the state |
| A / B / X | each lights while it plays and the audio position advances; A→B and B→X keep the place (within 0.6 s); pressing the playing slot starts it over; every press counted in `plays` / `heard` |
| Stop | silence, the slot goes dark |
| Loop | lights and reports pressed; a clip played into its end starts over; a jump into its last 0.3 s plays on (Firefox's stall, below); off, unlit, the clip keeps playing and then ends |
| position bar | a click at three quarters lands within 0.35 s of it |
| keys | `A` `B` `X` play; space stops, again plays the last slot; `L` toggles the loop; space on a focused Loop button or answer button presses nothing; `1` typed in the note is text, not a play |
| X es A / X es B | records the answer and its time, moves on, progress "2 de 16"; a changed answer is kept and counted |
| ← Anterior / Siguiente → | disabled where they cannot go; Previous shows the answer given; Next returns |
| Clip 1 / Clip 2 / keys 1 2 / Sin preferencia, the note | each recorded with its note and plays |
| pause | a reload resumes at the same question with every answer, with "Bienvenido de nuevo" |
| extra part, results, reveal | the extra opens from the end screen and ends on the results; the reveal names the pianos per trial and per question; the credits carry the CC BY 3.0 link; two catch questions are among the 16 |
| Download results (JSON) | the browser's download (offline) or `downloads.save` (published): the file holds kit, form, every answer with its time, start / end times |
| Start again | asks once, then erases (the old attempt kept in `earlier`, the form kept) |
| published page (Edge, `window.claude` stubbed) | the listener's own `abx/<id>` written with kit, form, answers and times and no piano name; a listener never subscribes to the collection, not even at `#summary`; a visitor who cannot write is told to download and send the file; the owner's summary shows each listener's score, p and catch questions and the pooled groups (24/32 for the two stubbed listeners), reads the collection, Back returns |

Besides: every file the page names exists, the catch questions' two names hold the same bytes,
no two names share more than 9 of 16 digits (the shipped kit: 5 at most), the page's data never
says chorda / salamander / candidate / beta / round / the owner, nor do LEEME.md and README.md,
and the first screen names no piano. No page errors in any run. The shipped kit (`abx5-54beca1842`): **410 checks, 0 failed**.
Run to run: Firefox's `<audio>` takes up to half a second to seek a file:// clip (the check
waits for it), and one of about a dozen headless Chrome runs, made while three cargo builds
filled the machine, lost its browser process mid-run (no renderer crash or page error logged;
not seen again in six instrumented runs).

What the check found and the page now fixes: the loop button never showed its state (kit 4);
Firefox stalls for good an `<audio>` FLAC sought into its last ~0.6 s while it loops (any
encoder — libsndfile's FLAC too; a WAV, or a FLAC played into its end, wraps), which a switch
near a clip's end would hit in the offline kit: a looping jump there starts the clip over, and a
looping clip whose time stands still for 0.6 s is restarted; a non-finite jump is ignored.

Safari was not run (no Mac here): it plays FLAC in `<audio>` since Safari 11 and decodes it in
Web Audio in current versions; where decoding fails the page falls back to `<audio>` by itself
(the fallback is the file:// path the check runs). The `:has()` styling of the form needs
Safari 15.4+. On an iPhone the ring/silent switch mutes Web Audio.

## Publishing it

claude.ai serves an artifact's media as .mp3, .wav, .ogg, .mp4 or .webm, not .flac. The kit to
publish is made with `--wav` (the same command plus `--wav`: 24-bit WAV, the samples a FLAC kit
carries): about 68 MB, more than one publish takes (64 MB), so its audio goes up in two publishes
to the same artifact (each later publish adds to the files already there). Kit 5 as published
was the FLAC kit's clips converted losslessly to WAV and renamed in `FILES`. The page waits up
to 3 s for `window.claude` (the viewer may attach it after the script's first run) before it
treats itself as offline.

The integrator publishes `index.html` with `audio/*` as the artifact's files, and declares:

```js
capabilities: {
  user: {},
  downloads: true,
  db: { rules: [
    { path: "abx", read: "owner", write: "owner" },
    { path: "abx/{self}", read: "interact", write: "interact" }
  ] }
}
```

Each listener reads and writes only `abx/<their id>`; only the owner reads the collection (and
writes imports to it). Who can save online: members of the owner's organisation at Contributor
or above. Visitors from outside the organisation hold `view` and cannot write anything; the page
tells them to download the JSON and send it, and the owner imports it in the summary.
