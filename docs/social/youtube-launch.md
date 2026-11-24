# The launch video on YouTube: the upload package

*Round 42, 2026-11-22. For the channel **Cantus DAW (@cantusdaw)** ([`accounts.md`](accounts.md),
[`setup.md`](setup.md)). The files are in `F:\projects\video-work\final\` (the film, made by
[`tools/video/`](../../tools/video/README.md)) and `F:\projects\video-work\social\` (the
thumbnails, made by `tools/social/render.mjs`).*

**When:** the video shows round 41 (12 instruments with Fides, 43 effects with Nitor), so it
goes public **only after beta.17 is released and the site that lists them is deployed**
(`docs/next-session.md`). Upload earlier as **Private**, check everything, then publish.

## Two videos, not one with dubbed audio — the recommendation

Upload the English and the Spanish films as **two videos**, each with its own title,
description, captions and thumbnail, linked to each other. Why:

1. **The picture speaks a language.** The cards are drawn in each language ("12
   instrumentos", "Gratis · código abierto", "Descarga gratis") and the Spanish cut burns
   nothing else in. A Spanish audio track over the English picture would show English cards
   under a Spanish voice.
2. **Multi-language audio is not open to a new channel.** YouTube's help page (answer
   13338784, read 2026-11-22) says it is "currently available to creators with access to
   Advanced features" — a channel gets those with enough history, or by verifying with an ID
   or a video, which not every creator is offered. A new channel cannot count on it on day one.
3. **Each language gets its own thumbnail, title and search terms**, and its own place in
   search and suggestions in that language.

What is lost: the views are split in two counts. Acceptable at launch. If the channel later
has Advanced features and a language-neutral picture, one video with audio tracks is the
better shape for tutorials (the picture of a tutorial is the app, whose language can be
shown in both).

## The files

| What | English video | Spanish video |
|---|---|---|
| Video | `final\cantus-en-2160p.mp4` (3840 × 2160, 60 fps, chapters, no burned captions) | `final\cantus-es-2160p.mp4` |
| Thumbnail | `social\youtube-thumb-en.png` (1280 × 720, 477 KB) — or `youtube-thumb-en-4k.jpg` (3840 × 2160, 0.8 MB; YouTube now recommends 4K, and a desktop upload takes up to 50 MB) | `social\youtube-thumb-es.png` / `youtube-thumb-es-4k.jpg` |
| Captions | `final\cantus-en.srt`, language *English* | `final\cantus-es.srt`, language *Spanish (Latin America)* |
| Chapters | `final\chapters-en.txt` (in the description below) | `final\chapters-es.txt` |
| Short | `final\cantus-en-vertical.mp4` (1080 × 1920, the first 30 s, captions under the picture) | `final\cantus-es-vertical.mp4` |

The thumbnails are the app itself (the README's window, *Neon Tide* in Slate Lapis) with
Chorda's piano, two big words and two plain facts. A custom thumbnail needs the channel's
phone verification first (`setup.md` §4).

## The English video

**Title** (72 / 100): `Cantus — a free, open-source DAW: 12 instruments, 43 effects, no account`

**Description** (paste as is; the chapters need the first at 0:00, at least three, each
10 s or longer — these are five):

```text
Cantus is a free, open-source DAW for Windows, Linux and macOS (preview). 80 seconds of what it does: twelve instruments made from the ground up — like Chorda, a modelled grand piano, and Tympanum, an acoustic drummer — forty-three effects, a mixer you wire with cables, recording, clips launched live, CLAP and VST3 plug-ins, and mastering on the master channel.

No account, no ads, no subscription, nothing behind a paywall.

Download free: https://cantusdaw.com/download/
Hear every instrument and effect in your browser: https://cantusdaw.com
Source code: https://github.com/luis314159/cantus-daw
Questions, ideas, songs made with Cantus: https://github.com/luis314159/cantus-daw/discussions
En español: <link to the Spanish video>

Chapters
0:00 Cantus
0:14 Instruments: Chorda and Tympanum
0:31 The mixer and 43 effects
0:44 Record, play live, master
1:00 Free, for everyone

Music: "With All Your Heart", "Morning Light", "Sunday Strut" and "Neon Tide", Cantus's demo songs, made in Cantus. Narration: a synthetic voice (ElevenLabs).
Cantus is a beta: save often and keep backups of your projects.
Made by Luis Ángel Almazán — whatever you do, work at it with all your heart.

#DAW #musicproduction #opensource
```

**Tags** (298 / 500, counted as YouTube counts them: commas, and 2 for a tag with a space):

```text
Cantus, Cantus DAW, free DAW, open source DAW, DAW, music production, home studio, free music software, Rust, Linux audio, piano roll, mixing, mastering, free VST, CLAP plugins, VST3, modelled piano, Chorda, Tympanum, drum machine, DAW gratis, producción musical, software libre, FL Studio
```

## The Spanish video

**Title** (73 / 100): `Cantus — un DAW libre y gratuito: 12 instrumentos, 43 efectos, sin cuenta`

**Description:**

```text
Cantus es un DAW libre y gratuito para Windows, Linux y macOS (vista previa). 80 segundos de lo que hace: doce instrumentos hechos desde cero — como Chorda, un piano de cola modelado, y Tympanum, un baterista acústico —, cuarenta y tres efectos, una consola que se cablea, grabación, clips lanzados en vivo, plug-ins CLAP y VST3, y masterización en el canal master.

Sin cuenta, sin anuncios, sin suscripción, nada detrás de un muro de pago.

Descarga gratis: https://cantusdaw.com/es/download/
Escucha cada instrumento y efecto en tu navegador: https://cantusdaw.com/es/
Código fuente: https://github.com/luis314159/cantus-daw
Preguntas, ideas y canciones hechas con Cantus: https://github.com/luis314159/cantus-daw/discussions
In English: <link to the English video>

Capítulos
0:00 Cantus
0:14 Instrumentos: Chorda y Tympanum
0:31 La consola y 43 efectos
0:44 Graba, toca en vivo, masteriza
1:00 Gratis, para todos

Música: «With All Your Heart», «Morning Light», «Sunday Strut» y «Neon Tide», canciones de demostración de Cantus, hechas en Cantus. Narración: una voz sintética (ElevenLabs).
Cantus está en beta: guarda seguido y respalda tus proyectos.
Hecho por Luis Ángel Almazán — lo que hagas, hazlo de todo corazón.

#DAW #produccionmusical #softwarelibre
```

**Tags:** the same list (it already carries the Spanish terms).

## Settings (both videos)

| Setting | Value | Why |
|---|---|---|
| Audience | **No, it's not made for kids** | a production tool for adults and teens |
| Age restriction | none | |
| Paid promotion | no | |
| Altered or synthetic content | **No** | the narration is a synthetic voice, but it depicts no real person or event; the rule is for realistic depictions of either. The description credits the voice. Re-read the question at upload: if YouTube's wording has widened to cover any synthetic voice, answer Yes |
| Video language / caption certification | English (Spanish for the other) / *This content has never aired on television in the U.S.* | |
| Category | **Science & Technology** | it shows a piece of software; *Music* is for listening, and its viewers expect songs. Tutorials later may use *Howto & Style* |
| Licence | **Standard YouTube Licence** | Cantus's name and logo are the owner's marks (`TRADEMARKS.md`), and the narration's voice is licensed to the owner by ElevenLabs' terms, not to be re-licensed under CC BY. Press and reviewers use the press kit (`web/assets/press/`), embedding stays on |
| Allow embedding | yes | the site and the README can embed it |
| Publish to subscriptions feed | yes | |
| Shorts remixing | allow video and audio | people making Shorts about Cantus help |
| Comments | on, *hold potentially inappropriate comments for review* | answer every comment in the first hours (`docs/round41/promotion-plan.md`) |
| Recording date / location | the upload day / Mexico | optional |
| Chapters | automatic chapters **off** | the description's chapters are the ones wanted |
| Thumbnail | as above | |

**End screen** (the last 12 s, 1:08 to the end, over the end card): the end card's words are
on the left (logo, cantusdaw.com, the platforms, the button), so put the elements **on the
right half**:

- **Subscribe** (the channel's icon), upper right.
- **Video**: the other language's launch video, lower right. Later: *Best for viewer*.

A *Link* element to cantusdaw.com needs the YouTube Partner Program; until then the link is
the description's first line.

**Cards** (small "i" cards, up to five):

- at **0:06**: the other language's video ("Versión en español" / "In English");
- at **0:14**: the *Instruments* playlist, once it has its first video;
- at **0:31**: the *Effects* playlist, likewise.

**Translations** (*Subtitles ▸ Add language ▸ Title and description*): on the English video,
add a Spanish title and description that say *"Versión en español: <link>"* in the first
line, and on the Spanish video an English one the other way round — a viewer whose YouTube
is in the other language then finds their version first.

## The Short

| | English | Spanish |
|---|---|---|
| File | `cantus-en-vertical.mp4` | `cantus-es-vertical.mp4` |
| Title | `A free DAW with a modelled grand piano — Cantus #shorts` (55) | `Un DAW gratis con piano de cola modelado — Cantus #shorts` (57) |
| Description | `Cantus is a free, open-source DAW: 12 instruments, 43 effects, no account, no ads. Full video and download: cantusdaw.com` | `Cantus es un DAW libre y gratuito: 12 instrumentos, 43 efectos, sin cuenta, sin anuncios. Video completo y descarga: cantusdaw.com` |
| Related video | the English launch video | the Spanish launch video |
| Thumbnail | a frame of Chorda's piano, chosen in the YouTube app | the same |

The *Related video* link sends a Short's viewers to the full film. Post the Shorts a day or
two after the long videos, not the same hour. The same vertical files are the Instagram
Reels and the TikTok clips (`accounts.md`).

## The order of steps

1. The channel is ready: name, handle, picture, banner, watermark, description, links,
   **phone verified** (custom thumbnails), playlists created (`setup.md`).
2. Beta.17 is out and the site shows Fides and Nitor.
3. Upload `cantus-en-2160p.mp4` as **Private**. While it processes: title, description,
   tags, thumbnail, captions (`cantus-en.srt`), settings, the *Launch / Lanzamiento*
   playlist.
4. Upload `cantus-es-2160p.mp4` the same way, as **Private**.
5. Put each video's link in the other's description, add the cards, the end screens and the
   translated titles.
6. Watch both, signed out (an incognito window, after publishing as **Unlisted**): the 4K
   version may take an hour or more to appear after the HD one; chapters, captions, end
   screen, links. Clickable links in descriptions need *Advanced features*: if they are not
   clickable yet, the first line still shows the address, and `setup.md` §4 says how to get
   them.
7. **Publish** both (or schedule them for a weekday morning, Mexico City time), then set the
   **channel trailer** (English video) and the **featured video for returning subscribers**
   (Spanish).
8. Share from the other accounts with each platform's own file (`accounts.md`): X, Bluesky
   and Mastodon take `cantus-en-1080p.mp4` (captions burned in, for muted autoplay);
   Instagram and TikTok the vertical cut.
9. A day or two later: the two Shorts.
10. Write down that day's numbers (`docs/round41/promotion-plan.md`, phase 5).
