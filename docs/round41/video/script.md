# The Cantus launch video: the script

*Round 41, 2026-11-20. 80 seconds; English and Spanish. Each line is timed to the picture
and written to be spoken at a calm presenter's pace: about 2.4 words a second in English,
about 5 syllables a second in Spanish. The Spanish is written as Spanish, not translated
word for word.*

**Before recording, check every bracketed fact** against the repository at the video's
commit. The video ships with the release that carries round 41 (beta.17, with Fides and Nitor), so
its counts are `r41/integration`'s (2026-11-21, `8a24a2cea`), counted as the site and the README
count them: **12 instruments** (`INSTRUMENTS`, 11 cards with Fides, plus Firmus,
`web/src/counts.rs`) and **43 effects** (`EFFECTS`, `web/src/units_data.rs`: the app's
`EffectKind::offered`, 49 kinds less the old Reverb and the five board-only Fragor pedals).
The first count, at master `7b287c39`, was 11 and 42.
The release is 0.1.0-beta.15, for Windows and Linux, with macOS held for beta.16.

**The music:**
- ***With All Your Heart*** (cinematic pop) opens and closes the video. Its name is the
  motto's own words (Colossians 3:23).
- ***Morning Light*** (pop ballad) carries the piano: Chorda's warm grand from its first
  bar (`demos/ballad.rs`).
- ***Neon Tide*** (house) carries the mixer and the effects.
- ***Sunday Strut*** (funk) carries the drums: its sixteenth-note groove with ghost notes
  played by **Tympanum**, loaded on its drum track as a user loads it.

Checked against the code at `74f202b1` (step 1, `report.md`): *Aurora* has no piano (its
instruments are Unda, Calor, Modulus, Exemplar and Gravis), and Chorda plays only in *With
All Your Heart* and *Morning Light*. **No demo uses Tympanum**: every demo's drums are
Pulsus II kits (*Midnight Static*'s is the trap kit). So the piano moved to *Morning
Light*, and the drum shot loads Tympanum on *Sunday Strut*, whose studio-kit groove suits an
acoustic drummer, with *Each mic to its own strip* (ADR 0124) so the mics stand on strips.

## The cut

| # | Time | Picture (the real app, `film.rs`) | Sound | Card |
|---|---|---|---|---|
| 1 | 0:00–0:06 | Black, then the arrangement of *With All Your Heart* fades in, playing; a slow push-in on the playhead crossing a bar line | the song's opening bars, no voice for 2 s | logo #03 draws on, gold rule |
| 2 | 0:06–0:14 | the whole window, the song playing; a pull-back to show arrangement, mixer, browser — **Onda** in the Stage, its steady groove | voice 1 | — |
| 3 | 0:14–0:24 | the instrument picker, then Chorda's window, *Morning Light*'s piano part playing; push-in on the keys moving | voice 2 | "[12] instruments" |
| 4 | 0:24–0:31 | Tympanum's kit view playing *Sunday Strut*'s drums; the mics on strips | voice 3 | — |
| 5 | 0:31–0:44 | *Neon Tide*'s mixer (FL-shaped desk); the cursor drags an effect onto a strip; its face opens; one knob turns, the sound changes | voice 4 | "[43] effects" |
| 6 | 0:44–0:52 | three fast shots, one bar each: recording (an armed track, its meter); the session view launching clips; a hosted CLAP/VST3 plug-in's slot | voice 5 | lower third per shot |
| 7 | 0:52–1:00 | the master channel's mastering chain on *With All Your Heart*; the loudness meter landing | voice 6 | — |
| 8 | 1:00–1:08 | a slow glide across the desk, the song building; it ends on **Onda** in the Stage, framed close, keeping the beat | voice 7 | "Free · open source" |
| 9 | 1:08–1:20 | end card over the song's last bars: logo, cantusdaw.com, "Windows · Linux — macOS next", the download button drawn; **Onda** small at the card's side, still in its groove (its frames from the film, keyed out of the Stage, or the app's own transparent video layer, ADR 0088) | voice 8, then the song alone | end card (A or B) |

### Onda, the one dancer (the owner, 2026-11-20)

The owner chose **Onda**, the figure drawn by an oscilloscope's amber trace with a Lissajous
loop for a head, and asked for "algo muy simple y constante".

- **What it dances:** one steady groove on the beat for the whole video (a sway, a nod, the
  limbs rippling with the sound), with **none of its drop moves** (the wave snapping square)
  and no spins or jumps. Choose it from what the app's dancer already does: a calm
  move-set, a low energy, or the one move held. If the app offers no way to keep it that
  simple, say so in the report and ask the owner, rather than drawing a special Onda for
  the film.
- **Where:**
  - shot 2, in the Stage, beside the window's work;
  - shot 8's last beats, framed close;
  - the end card, small, at the side.

  It never covers a control the narration names.
- **On the beat to the frame:** its feed follows the same engine clock as the picture and
  the sound.

## The narration

### English (about 130 words)

1. *(0:06)* "This is Cantus — a digital audio workstation that's free, open source, and built
   to be the best there is."
2. *(0:14)* "Play it the way you hear it. [Twelve] instruments made from the ground up — like
   Chorda, a modelled grand piano, tuned string by string."
3. *(0:24)* "And Tympanum: a full acoustic drummer, every microphone on its own channel."
4. *(0:31)* "Mix on a desk that feels at home if you've used FL Studio. Drag in any of [forty-three]
   effects, each with its own face — and hear it as you add it."
5. *(0:44.5)* "Record your voice and your guitar. Launch ideas live. Bring your CLAP and VST3
   plug-ins."
6. *(0:52)* "Then master it, right on the master channel."
7. *(1:00)* "No subscription. No paywall. Every feature, for everyone."
8. *(1:08)* "Cantus. Download it free, at cantusdaw.com."

### Español (unas 120 palabras)

1. *(0:06)* «Esto es Cantus: una estación de trabajo de audio gratuita, de código abierto, y
   hecha para ser la mejor que existe.»
2. *(0:14)* «Toca como lo escuchas. [Doce] instrumentos hechos desde cero, como Chorda: un
   piano de cola modelado, afinado cuerda por cuerda.»
3. *(0:24)* «Y Tympanum: un baterista acústico completo, cada micrófono en su propio canal.»
4. *(0:31)* «Mezcla en una consola que se siente como en casa si vienes de FL Studio. Arrastra
   cualquiera de sus [cuarenta y tres] efectos, cada uno con su propia cara, y suena desde
   que lo agregas.»
5. *(0:44.5)* «Graba tu voz y tu guitarra. Lanza ideas en vivo. Trae tus plug-ins CLAP y VST3.»
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
- **"every microphone on its own channel":** Tympanum's *Each mic to its own strip*
  (ADR 0124, Outputs). It is a choice, not the default, so the drum shot sets it and the
  picture shows what the line says.
- **"hear it as you add it":** effects load a starting preset on insert (the owner's
  2026-10-16 decision).
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
