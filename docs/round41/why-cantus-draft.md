# "Why I'm building Cantus" — a draft for the owner

**Status: draft, not published.** Nothing below is on the site or in the README. It is
written in the first person for **Luis to approve, rewrite or reject**; until he does, no
word of it goes live under his name. Each sentence comes from something the repository
already says he said or decided (sources at the end). Where the draft needed a link between
two facts, the sentence is marked **[check]** — those are the ones most likely to need his
own words.

The reviewer's point was not to soften the faith but to give it context: a visitor who meets
"Christ is King" in the footer and Colossians 3:23 beside the download should be able to
read, in his words, why it is there. The faith stays exactly where it is.

---

## English

### Why I'm building Cantus

I make Cantus because I believe a musician should be able to finish a song without paying a
subscription, creating an account or wondering whether their tools will still be theirs next
year. So the whole DAW is free, it sends nothing about you anywhere, and its source is open —
the application under the GPL, the engine under the MPL — so it cannot be taken back.

I also want it to be good, not good *for a free program*. The goal written at the top of the
project is plain: to become, undeniably and measurably, the best DAW in the world, free or
commercial — in features, performance, quality and stability. That is why the site shows
numbers measured against FL Studio and REAPER, and why it shows the row Cantus loses as
plainly as the ones it wins. **[check: the tone of claiming "the best" in public]**

I am a Christian, and Cantus is built under one verse: *"Whatever you do, work heartily, as
for the Lord and not for men"* (Colossians 3:23, World English Bible). For me that
means doing this work with excellence and honesty — finishing what is started, testing what
is claimed, and studying humbly those who did it well before us. It is why the footer says
"Christ is King". **[check: whether he wants to say more, or less, about his faith]**

Cantus never asks you to share that belief. A verse greets you when it starts, in your
language, and one switch turns it off for good. Everyone gets the same program, whatever they
believe, whatever they give — or nothing at all.

— Luis Ángel Almazán

---

## Español

### Por qué construyo Cantus

Hago Cantus porque creo que un músico debería poder terminar una canción sin pagar una
suscripción, sin crear una cuenta y sin preguntarse si sus herramientas seguirán siendo suyas
el año que viene. Por eso el DAW completo es gratis, no envía nada sobre ti a ningún lado y su
código es abierto —la aplicación bajo la GPL, el motor bajo la MPL—, así que nadie puede
quitártelo.

También quiero que sea bueno, no «bueno para ser gratis». La meta escrita al principio del
proyecto es clara: llegar a ser, de forma innegable y medible, el mejor DAW del mundo, libre o
comercial, en funciones, rendimiento, calidad y estabilidad. Por eso el sitio muestra cifras
medidas frente a FL Studio y REAPER, y muestra la fila que Cantus pierde con la misma claridad
que las que gana. **[revisar: el tono de decir «el mejor» en público]**

Soy cristiano, y Cantus se construye bajo un versículo: *«Y todo lo que hagáis, hacedlo de
ánimo, como al Señor, y no á los hombres»* (Colosenses 3:23, Reina-Valera 1909). Para mí eso
significa hacer este trabajo con excelencia y honestidad: terminar lo que se empieza, probar lo
que se afirma y estudiar con humildad a quienes lo hicieron bien antes que nosotros. Por eso el
pie de página dice «Viva Cristo Rey». **[revisar: si quiere decir más, o menos, sobre su fe]**

Cantus nunca te pide compartir esa creencia. Un versículo te saluda al arrancar, en tu idioma,
y un interruptor lo apaga para siempre. Todos reciben el mismo programa, crean lo que crean y
den lo que den, o nada.

— Luis Ángel Almazán

---

## Where it would go (once approved)

1. **The site, home page, chapter 6 (*Free, and meant to stay that way*,
   `web/src/body.rs`, `Free`).** The verse already sits there as a figure beside the
   promises. The approved text would replace the figure's one-line caption ("Colossians
   3:23 — the motto Cantus is built under") with the first sentence of the faith paragraph
   and a link, *Why I'm building Cantus →*, to the whole text.
2. **The whole text on its own page**, `/why/` and `/es/why/` — a new `Route::Why` in
   `web/src/route.rs` (title, description, both languages; `Page::all` puts it in the
   sitemap and the pre-renderer), rendered by a small component with the four paragraphs.
   The footer's "Christ is King" line (`web/src/foot.rs`) could link to it too.
3. **The README**: one short paragraph under *Why Cantus* on the first screen (the faith
   paragraph's first two sentences) and a link to `docs/WHY.md` holding both languages.

The quotations are public domain: the **World English Bible** in English (the verse file
Cantus shows at start, `locales/verses/en.txt`) and the **Reina-Valera 1909** in Spanish, the
same words the site already shows. (The site's English line, "Whatever you do, work at it with
all your heart", is the project's own short form of the motto.) The **RVR1960** must not be quoted here or anywhere published: it is kept
privately until its publisher grants permission (`LICENSE`, `crates/cantus-ui/locales/verses/README.md`),
and the public repository's publishing script removes it.

## Sources

- `CLAUDE.md` — "made by Luis Ángel Almazán with Christian values (motto: Colossians 3:23 —
  'whatever you do, work at it with all your heart')"; "the best DAW in the world — free or
  commercial — in features, performance, quality and stability"; "Christian values,
  respected choices: a Reina-Valera verse … greets the user at start in the interface's
  language — on by default, with an off switch for those who do not share the belief".
- `crates/cantus-ui/locales/verses/en.txt` and `es-1909.txt`, line 123 — Colossians 3:23
  in the World English Bible and the Reina-Valera 1909.
- `idea.md` — "excellence in execution, the humility to study
  those who did it well before us"; the principles *User freedom* (no lock-in, no hidden
  telemetry) and *Data safety above all*.
- `docs/BACKLOG-excellence.md` §0 — the goal and its scorecard.
- `web/src/body.rs` (*Free*) — "The whole DAW is free", "No account, no telemetry", the
  licences, "Everyone gets the same program, whatever they give, or nothing at all"; the
  verse in English and in Reina-Valera 1909.
- `web/src/foot.rs` — "Christ is King" / "Viva Cristo Rey".
- `crates/cantus-ui/locales/verses/README.md` — the verse at start, its off switch.
