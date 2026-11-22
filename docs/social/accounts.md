# Cantus's own accounts: what goes on each profile

*Round 42, 2026-11-22. The kit the owner fills the profiles from. The step-by-step, in Spanish,
is [`setup.md`](setup.md); the launch video's upload package is
[`youtube-launch.md`](youtube-launch.md); the images come from
[`tools/social/`](../../tools/social/render.mjs) (`node tools/social/render.mjs
F:/projects/video-work/social`), with `sheet.png` showing every one with its crops.*

The accounts belong to **Cantus**, not to a person: display name **Cantus DAW**, handle
**@cantusdaw** everywhere, the address **hola@cantusdaw.com**. The owner's personal accounts
share their posts. Every word below is a claim the site and the README already make (the
counts as `web/src/counts.rs` counts them: 12 instruments with Firmus, 43 effects offered as
inserts). Never "the best DAW" as a fact; the motto appears only in English or as a Spanish
paraphrase, never in the Reina-Valera 1960's words.

**Character counts** were measured with Python's `len` (code points; no emoji, so graphemes
are the same). A line break counts as one.

## Where Cantus posts, and what each account is for

| Platform | Role | Language |
|---|---|---|
| **YouTube** | The videos: the launch video, then instruments, effects, tutorials. | Both: one video per language |
| **Instagram** (Reels) | The same vertical clip as TikTok. | Both: one reel per language |
| **TikTok** | The same vertical clip. | Both: one clip per language |
| **Bluesky** | Optional active: Rust and Linux audio people, release notes. | English first, Spanish too |
| **Mastodon** | Optional active: the same people, the same posts. | English first, Spanish too |
| **X** | Reserved: logo, one line, the link; posts only when there is time. | English |

Cantus's accounts speak as Cantus ("Cantus is…", "made by Luis Ángel Almazán"); the first
person ("I made this", which every community expects from the author) belongs on the owner's
own accounts, when they share the post.

A bilingual account is better than two accounts per platform at this size: every follower
counts once, and each post says its language in its first word (a flag-free "EN /" or
"ES /" prefix is enough when a post is in one language).

## The images (from `F:\projects\video-work\social\`)

| Platform | Profile picture | Header / banner |
|---|---|---|
| YouTube | `avatar-800.png` (shown at 98 px, a circle) | `youtube-banner.jpg` (2560 × 1440, the words inside the 1546 × 423 safe area); watermark `youtube-watermark.png` (150 × 150) |
| Instagram | `avatar-1080.png` (a circle) | — |
| TikTok | `avatar-1080.png` (a circle) | — |
| Bluesky | `avatar-1000.jpg` (under its 1 MB cap) | `bluesky-banner-en.jpg` (3000 × 1000, under 1 MB) |
| Mastodon | `avatar-400.png` | `header-en-1500x500.png` |
| X | `avatar-400.png` | `header-en-1500x500.png` |

The Spanish headers (`header-es-…`, `bluesky-banner-es.jpg`) are there if the owner prefers
Spanish first on a platform. The avatar is logo #03's C and waveform on its lapis, full
bleed, inside a gold ring: a circle, a rounded square or a square crop all keep it whole.

## YouTube

| Field | Value |
|---|---|
| Channel name | **Cantus DAW** |
| Handle | **@cantusdaw** |
| Link | `https://cantusdaw.com` (title *Download Cantus*), then `https://github.com/luis314159/cantus-daw` (*Source code*), `https://github.com/luis314159/cantus-daw/discussions` (*Community*) |
| Contact e-mail | hola@cantusdaw.com |
| Country | Mexico |
| Channel trailer (visitors not subscribed) | the English launch video |
| Featured video (returning subscribers) | the Spanish launch video, until the next video |
| Watermark | `youtube-watermark.png`, shown for the **entire video** |

**Description — English** (894 / 1000 characters; the default language):

```text
Cantus is a free, open-source digital audio workstation for Windows, Linux and macOS (preview), in the spirit of FL Studio. This channel shows what it does and how to use it, in English and Spanish.

What is in it: arrangement, piano roll, channel rack and a mixer you wire with cables; recording with a mode for low buffers; mastering on the master channel; CLAP and VST3 plug-ins. Twelve instruments of its own, among them Chorda, a modelled grand piano, and Tympanum, an acoustic drummer, and forty-three effects, each with its own face.

No account, no ads, no telemetry, nothing behind a paywall. Its own sound packs are free downloads, for good. The code is open (GPL-3.0 and MPL-2.0), so it cannot be taken back.

Made by Luis Ángel Almazán, under one motto: whatever you do, work at it with all your heart.

Download: https://cantusdaw.com
Code: https://github.com/luis314159/cantus-daw
```

**Description — Spanish** (935 / 1000), added as the channel's Spanish translation
(*Customization ▸ Basic info ▸ Add language*), so Spanish-speaking viewers see it:

```text
Cantus es una estación de trabajo de audio digital (DAW) libre y gratuita para Windows, Linux y macOS (vista previa), al estilo de FL Studio. Este canal muestra lo que hace y cómo usarlo, en español y en inglés.

Lo que trae: arreglo, piano roll, channel rack y una consola que se cablea; grabación con un modo para búferes bajos; masterización en el canal master; plug-ins CLAP y VST3. Doce instrumentos propios, entre ellos Chorda, un piano de cola modelado, y Tympanum, un baterista acústico, y cuarenta y tres efectos, cada uno con su propia cara.

Sin cuenta, sin anuncios, sin telemetría, nada detrás de un muro de pago. Sus paquetes de sonidos propios se descargan gratis, para siempre. El código es abierto (GPL-3.0 y MPL-2.0), así que nadie puede quitártelo.

Hecho por Luis Ángel Almazán, bajo un lema: lo que hagas, hazlo de todo corazón.

Descarga: https://cantusdaw.com/es/
Código: https://github.com/luis314159/cantus-daw
```

The channel's translated name stays **Cantus DAW**.

**Channel keywords** (227 / 500 characters; *Settings ▸ Channel ▸ Basic info ▸ Keywords*):

```text
DAW "free DAW" "open source DAW" "music production" "home studio" Rust "Linux audio" "FL Studio" "piano roll" mixing mastering "free plugins" CLAP VST3 Cantus "DAW gratis" "producción musical" "software libre" "estudio en casa"
```

**Playlists** (each public, each with a two-line description in both languages):

| Playlist | What goes in | First items |
|---|---|---|
| **Launch / Lanzamiento** | the launch videos and each release's "what's new" | the EN and ES launch videos, the Short |
| **Instruments / Instrumentos** | one short video per instrument: what it is, three presets heard, one thing to try | Chorda, Tympanum, Fides, then Unda, Calor, Modulus, Exemplar, Gravis, Lamina, Organum, Pulsus II, Firmus |
| **Effects / Efectos** | one short video per effect or per family (EQ, dynamics, space, colour, modulation) | Aequalis, Comprimo, Spatium, Nitor, Fragor and its pedals |
| **Tutorials / Tutoriales** | how to do one task, start to finish, in under ten minutes | install on Windows / Linux; a first beat; record a voice; mix with cables; master on the master channel |

Name each playlist in English with the Spanish after a slash, or (better) give each a
Spanish title translation in Studio, the same way as the channel's description. Each
video goes in its own language's place: the EN and ES versions of a video both go in the
playlist, side by side.

## Instagram

| Field | Value |
|---|---|
| Username | **cantusdaw** |
| Name (searchable, ≤ 30) | **Cantus DAW** |
| Account type | Professional ▸ **Creator** (or Business), category **Software** (or the closest Instagram offers — not verified) |
| Link | `https://cantusdaw.com` |
| Profile picture | `avatar-1080.png` |

**Bio — English** (132 / 150):

```text
Free, open-source DAW. 12 instruments, 43 effects, every feature for everyone.
No account, no ads.
Windows · Linux · macOS (preview)
```

**Bio — Spanish** (132 / 150):

```text
DAW libre y gratuito. 12 instrumentos, 43 efectos, todo para todos.
Sin cuenta, sin anuncios.
Windows · Linux · macOS (vista previa)
```

Instagram shows one bio; choose by the audience the owner expects first (Mexico and Latin
America: Spanish). **First pinned post:** the launch Reel, `cantus-en-vertical.mp4` and the
next day `cantus-es-vertical.mp4`, both pinned (Instagram pins up to three).

> **EN —** Cantus is a free, open-source DAW made by Luis Ángel Almazán: 12 instruments made from
> scratch (Chorda here is a modelled grand piano, tuned string by string), 43 effects, a
> mixer you wire with cables. No account, no ads. Download it free: link in bio.
> #DAW #musicproduction #homestudio #opensource #freesoftware #producer
>
> **ES —** Cantus es un DAW libre y gratuito hecho por Luis Ángel Almazán: 12 instrumentos hechos
> desde cero (aquí Chorda, un piano de cola modelado, afinado cuerda por cuerda), 43 efectos y
> una consola que se cablea. Sin cuenta, sin anuncios. Descárgalo gratis: enlace en la bio.
> #DAW #produccionmusical #homestudio #softwarelibre #productor

## TikTok

| Field | Value |
|---|---|
| Username | **cantusdaw** |
| Name (≤ 30) | **Cantus DAW** |
| Account | Business account, category *Software & Apps* or the closest (not verified) — a business account can show a link in the bio without 1 000 followers (community reports, not verified) |
| Link | `https://cantusdaw.com` |
| Profile picture | `avatar-1080.png` |

**Bio — English** (70 / 80): `Free, open-source DAW. 12 instruments, 43 effects. No account, no ads.`

**Bio — Spanish** (74 / 80): `DAW libre y gratis. 12 instrumentos, 43 efectos. Sin cuenta, sin anuncios.`

**First pinned post:** the same vertical clips as Instagram, the same captions shortened to
the first sentence and the hashtags.

## Bluesky

| Field | Value |
|---|---|
| Handle | **@cantusdaw.com** (the domain as the handle: it proves the account is the site's own; `setup.md` §5) — or **@cantusdaw.bsky.social** until the DNS record is in |
| Display name (≤ 64) | **Cantus DAW** |
| Avatar / banner | `avatar-1000.jpg` / `bluesky-banner-en.jpg` |

**Description — English** (244 / 256):

```text
Cantus is a free, open-source DAW written in Rust: 12 instruments and 43 effects of its own, a mixer you wire with cables, CLAP and VST3 plug-ins. Windows, Linux, macOS (preview). No account, no ads, no telemetry.

Posts in English and Spanish.
```

**Description — Spanish** (253 / 256):

```text
Cantus es un DAW libre y gratuito escrito en Rust: 12 instrumentos y 43 efectos propios, una consola que se cablea, plug-ins CLAP y VST3. Windows, Linux, macOS (vista previa). Sin cuenta, sin anuncios, sin telemetría.

Publicaciones en español e inglés.
```

**First pinned post** (262 / 300; with `cantus-en-1080p.mp4`, captions burned in):

> Cantus is a free, open-source DAW in Rust, made by Luis Ángel Almazán. 80 seconds of what it does:
> its own instruments and effects, a mixer you wire with cables, recording, mastering.
> No account, no ads. Windows and Linux, macOS in preview. https://cantusdaw.com

## Mastodon

| Field | Value |
|---|---|
| Server | **mastodon.social** (open, run by the Mastodon non-profit, long-lived) — fosstodon.org is the FOSS people's home but reviews sign-ups and has its own language rules: read them first |
| Handle | **@cantusdaw@mastodon.social** |
| Display name | **Cantus DAW** |
| Avatar / header | `avatar-400.png` / `header-en-1500x500.png` |
| Profile fields (4) | *Website* `https://cantusdaw.com` · *Source* `https://github.com/luis314159/cantus-daw` · *Download* `https://cantusdaw.com/download/` · *Community* `https://github.com/luis314159/cantus-daw/discussions` |
| Bot / discoverable | not a bot; *suggest account to others* on; *include in search* on |

The *Website* field turns green (verified) once cantusdaw.com links back to the profile with
`rel="me"` — the integrator adds that link to the site's footer (`setup.md` §9).

**Bio — English** (496 / 500, the default limit; a link would count 23):

```text
Cantus is a free, open-source DAW written in Rust, in the spirit of FL Studio: arrangement, piano roll, channel rack, a mixer you wire with cables, recording, mastering on the master channel. 12 instruments and 43 effects of its own, CLAP and VST3 hosting. Windows, Linux, macOS (preview).

No account, no ads, no telemetry; free for everyone, for good. Built under one motto: whatever you do, work at it with all your heart.

Posts in English and Spanish.

#RustLang #LinuxAudio #DAW #OpenSource
```

**Bio — Spanish** (486 / 500):

```text
Cantus es un DAW libre y gratuito escrito en Rust, al estilo de FL Studio: arreglo, piano roll, channel rack, una consola que se cablea, grabación y masterización en el canal master. 12 instrumentos y 43 efectos propios, plug-ins CLAP y VST3. Windows, Linux, macOS (vista previa).

Sin cuenta, sin anuncios, sin telemetría; gratis para todos, para siempre. Hecho bajo un lema: lo que hagas, hazlo de todo corazón.

Publica en español e inglés.

#RustLang #LinuxAudio #DAW #SoftwareLibre
```

**First pinned post** (474 / 500 as Mastodon counts, each link 23) — Mastodon's custom is an `#introduction`:

> Hello! Cantus is a free, open-source DAW written in Rust, and this is its own account.
> The engine never allocates on the audio thread (a test counts it), and an offline export
> is bit-identical to what you heard. 12 instruments and 43 effects of its own, CLAP and
> VST3 hosting; Windows, Linux (PipeWire, JACK, ALSA), macOS in preview. No account, no
> ads. https://cantusdaw.com — code: https://github.com/luis314159/cantus-daw
> #introduction #RustLang #LinuxAudio #OpenSource #MusicProduction

Attach `cantus-en-1080p.mp4` with its description for screen readers ("Cantus's launch
video, 80 seconds: the arrangement, the Chorda piano, the Tympanum drummer, the mixer and
its effects, recording, the master channel, the download page").

## X

| Field | Value |
|---|---|
| Handle (≤ 15) | **@cantusdaw** |
| Name (≤ 50) | **Cantus DAW** |
| Location | Mexico |
| Website | `https://cantusdaw.com` |
| Photo / header | `avatar-400.png` / `header-en-1500x500.png` |
| Professional category | *Software* or the closest (not verified) |

**Bio — English** (153 / 160):

```text
A free, open-source DAW written in Rust: 12 instruments, 43 effects, a mixer you wire with cables. Windows · Linux · macOS (preview). No account, no ads.
```

**Bio — Spanish** (158 / 160):

```text
Un DAW libre y gratuito escrito en Rust: 12 instrumentos, 43 efectos y una consola con cables. Windows · Linux · macOS (vista previa). Sin cuenta ni anuncios.
```

**First pinned post** (205 / 280; with `cantus-en-1080p.mp4`):

> Cantus is a free, open-source DAW. 12 instruments of its own, 43 effects, a mixer you
> wire with cables, recording and mastering. No account, no ads, no subscription.
> Download it free: https://cantusdaw.com

## What stays true as Cantus changes

The counts (12, 43) and "macOS (preview)" are this release's. When a release changes them,
change the bios, the YouTube description and the banner's platform line together
(`tools/social/social.js` holds the images' words; re-render and re-upload).
