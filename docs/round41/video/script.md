# The Cantus launch video: the script

*Round 41, 2026-11-20. 80 seconds; English and Spanish. Each line is timed to the picture
and written to be spoken at a calm presenter's pace: about 2.4 words a second in English,
about 5 syllables a second in Spanish. The Spanish is written as Spanish, not translated
word for word.*

**Before recording, check every bracketed fact** against the repository at the video's
commit. On 2026-11-20 (master `7b287c39`) the site counts **11 instruments** (`INSTRUMENTS`
plus Firmus, `web/src/counts.rs`) and **42 effects** (`EFFECTS`, `web/src/units_data.rs`).
The release is 0.1.0-beta.15, for Windows and Linux, with macOS held for beta.16.

**The music:**
- ***With All Your Heart*** (cinematic pop) opens and closes the video. Its name is the
  motto's own words (Colossians 3:23).
- ***Aurora*** (cinematic) carries the piano.
- ***Neon Tide*** (house) carries the mixer and the effects.
- ***Midnight Static*** (trap) carries the drums.

Check which demo uses which instrument, and swap a song if another shows the unit better.

## The cut

| # | Time | Picture (the real app, `film.rs`) | Sound | Card |
|---|---|---|---|---|
| 1 | 0:00–0:06 | Black, then the arrangement of *With All Your Heart* fades in, playing; a slow push-in on the playhead crossing a bar line | the song's opening bars, no voice for 2 s | logo #03 draws on, gold rule |
| 2 | 0:06–0:14 | the whole window, the song playing; a pull-back to show arrangement, mixer, browser | voice 1 | — |
| 3 | 0:14–0:24 | the instrument picker, then Chorda's window, *Aurora*'s piano part playing; push-in on the keys moving | voice 2 | "[11] instruments" |
| 4 | 0:24–0:31 | Tympanum's kit view playing *Midnight Static*'s drums; the mics on strips | voice 3 | — |
| 5 | 0:31–0:43 | *Neon Tide*'s mixer (FL-shaped desk); the cursor drags an effect onto a strip; its face opens; one knob turns, the sound changes | voice 4 | "[42] effects" |
| 6 | 0:43–0:52 | three fast shots, one bar each: recording (an armed track, its meter); the session view launching clips; a hosted CLAP/VST3 plug-in's slot | voice 5 | lower third per shot |
| 7 | 0:52–1:00 | the master channel's mastering chain on *With All Your Heart*; the loudness meter landing | voice 6 | — |
| 8 | 1:00–1:08 | a slow glide across the desk, the song building | voice 7 | "Free · open source" |
| 9 | 1:08–1:20 | end card over the song's last bars: logo, cantusdaw.com, "Windows · Linux — macOS next", the download button drawn | voice 8, then the song alone | end card (A or B) |

## The narration

### English (about 175 words)

1. *(0:06)* "This is Cantus — a digital audio workstation that's free, open source, and built
   to be the best there is."
2. *(0:14)* "Play it the way you hear it. [Eleven] instruments made from the ground up — like
   Chorda, a modelled grand piano, tuned string by string."
3. *(0:24)* "And Tympanum: a full acoustic drummer, every microphone on its own channel."
4. *(0:31)* "Mix on a desk that feels at home if you've used FL Studio. Drag in any of [forty-two]
   effects — each with a face of its own, and a sound you can hear the moment you add it."
5. *(0:43)* "Record your voice and your guitar. Launch ideas live. Bring your CLAP and VST3
   plug-ins."
6. *(0:52)* "Then master it, right on the master channel."
7. *(1:00)* "No subscription. No paywall. Every feature, for everyone."
8. *(1:08)* "Cantus. Download it free, at cantusdaw.com."

### Español (unas 190 palabras)

1. *(0:06)* «Esto es Cantus: una estación de trabajo de audio gratuita, de código abierto, y
   hecha para ser la mejor que existe.»
2. *(0:14)* «Toca como lo escuchas. [Once] instrumentos hechos desde cero, como Chorda: un
   piano de cola modelado, afinado cuerda por cuerda.»
3. *(0:24)* «Y Tympanum: un baterista acústico completo, cada micrófono en su propio canal.»
4. *(0:31)* «Mezcla en una consola que se siente como en casa si vienes de FL Studio. Arrastra
   cualquiera de sus [cuarenta y dos] efectos: cada uno con su propia cara, y un sonido que
   oyes desde el momento en que lo agregas.»
5. *(0:43)* «Graba tu voz y tu guitarra. Lanza ideas en vivo. Trae tus plug-ins CLAP y VST3.»
6. *(0:52)* «Y luego masterízalo, ahí mismo, en el canal master.»
7. *(1:00)* «Sin suscripción. Sin muros de pago. Todo, para todos.»
8. *(1:08)* «Cantus. Descárgalo gratis en cantusdaw.com.»

### The end card: the owner chooses

- **(A) With the motto.** Under the logo and the address, small, in the gold rule's
  colour, in the translations the app itself uses (`crates/cantus-ui/locales/verses/README.md`):
  - EN, the World English Bible (public domain): *"And whatever you do, work heartily, as
    for the Lord" — Colossians 3:23*;
  - ES, the Reina-Valera 1909 (public domain): *«Y todo lo que hagáis, hacedlo de ánimo,
    como al Señor» — Colosenses 3:23*.

  The Reina-Valera 1960's wording (*«hacedlo de corazón»*) is © Sociedades Bíblicas Unidas.
  The app does not ship it until written permission arrives (`verse::RVR1960_PERMITTED`).
  In the video it is used only if the owner chooses it, with the publisher's credit line as
  its quotation terms ask.

  The song is named *With All Your Heart*, so the card closes the circle without a word of
  narration.
- **(B) Without.** The logo, the address and the platforms only.

Either way the narration stays as above. The faith is shown, not argued, as in the app's
start-up verse.

## Words to check before recording

- **"the best there is":** the project's stated goal (`CLAUDE.md`, BACKLOG §0). Said as an
  aim ("built to be"), never as a measured fact in this video.
- **"tuned string by string":** true on master since round 41 (`docs/round41/chorda-strings.md`):
  every string measured alone and in tolerance.
- **"feels at home if you've used FL Studio":** FL is the stated reference (`CLAUDE.md`).
  It is a mark of Image-Line, named as a comparison, not an endorsement.
- **"a sound you can hear the moment you add it":** effects load a starting preset on
  insert (the owner's 2026-10-16 decision).
- **"Launch ideas live":** the session view (round 33). **"CLAP and VST3 plug-ins":** the
  host (round 17); check that the shot's plug-in is free to show (the test plug-ins, or one
  with a licence that allows it).
- **"master it on the master channel":** the master chain and its mastering (round 19,
  round 34's mastering).
- **"No subscription. No paywall. Every feature, for everyone":** the licence (GPL app, MPL
  engine, ADR 0047) and the free-forever promise in `CLA.md`. The sound packs are free
  downloads (ADR 0030).
- **"Windows · Linux — macOS next":** true at beta.15. Change the card if macOS has shipped
  by then.

## Captions

Burned into the site's cut and given as `cantus-en.srt` and `cantus-es.srt`: the narration
line by line, at most 42 characters a line and two lines a caption, on the line's own
timing. The site's muted loop (`cantus-loop.mp4`) uses only the cards, no captions.
