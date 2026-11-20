# Cantus's community channel: GitHub Discussions

*Round 41, 2026-11-20. The owner chose GitHub Discussions on cantus-daw, and asked that it
be set up for him. Its links are already in the site's footer, the README's *Community*
section and the issue forms' chooser (`.github/ISSUE_TEMPLATE/config.yml`). They answer
only once Discussions is switched on, so switch it on before the site or the README is
published.*

## 1. Switch it on (the owner, about a minute)

The session could not do this step: it needs the owner's GitHub login, and reading his stored
credential was refused by the tool's permission rules, rightly.

1. <https://github.com/luis314159/cantus-daw/settings> → **General** → **Features** → tick
   **Discussions**. GitHub creates its six default categories: Announcements, General, Ideas,
   Polls, Q&A, Show and tell.
2. On the **Discussions** tab, click the pencil beside **Categories** and edit the
   descriptions to the bilingual ones below. Delete *Polls* if it is not wanted.
3. Post the welcome message (below) in **Announcements** and **pin** it.

If he prefers that a session does steps 2–3, `gh auth login` in a terminal of his own (GitHub
CLI) gives a session a login it may use. The category descriptions can be set by the API; a
category cannot be created by the API, only in the page.

## 2. The categories

| Category | Description (EN / ES) |
|---|---|
| 📣 Announcements | New versions and what is coming. Only the maintainer posts. / Versiones nuevas y lo que viene. Solo publica el mantenedor. |
| 🙋 Q&A | How do I…? Answers get marked. / ¿Cómo hago…? Las respuestas se marcan. |
| 💡 Ideas | Features you would like, and why. / Funciones que te gustaría tener, y por qué. |
| 🎵 Show and tell | Songs and sounds made with Cantus. / Canciones y sonidos hechos con Cantus. |
| 💬 General | Anything else about Cantus. / Cualquier otra cosa sobre Cantus. |

Bugs stay in **issues**, where the bug and hardware forms ask for what a fix needs. A
discussion that turns out to be a bug can be converted to an issue from its page.

## 3. The welcome message

Post it in **Announcements** and pin it. It is the owner's to post under his name: change
anything.

**Title:** Welcome to Cantus's community / Bienvenidos a la comunidad de Cantus

> **Welcome!** This is the place to ask how to do something, share what you make with Cantus,
> and suggest what it should do next — in English or in Spanish.
>
> Cantus is a free, open-source DAW, completely free for everyone. Its ambition is to become
> the best DAW on the market, and that takes the people who use it.
>
> - **Questions** go in *Q&A*; when an answer solves yours, mark it.
> - **Songs and sounds** made with Cantus go in *Show and tell*. I would love to hear them.
> - **Ideas** go in *Ideas*: say what you were trying to do, not only the feature.
> - **Bugs** go in [issues](https://github.com/luis314159/cantus-daw/issues/new/choose),
>   where the form asks for what a fix needs.
>
> Be kind and be specific. Thank you for being here. — Luis
>
> ---
>
> **¡Bienvenidos!** Este es el lugar para preguntar cómo hacer algo, compartir lo que haces con
> Cantus y proponer lo que debería hacer después, en español o en inglés.
>
> Cantus es un DAW libre y de código abierto, completamente gratis para todos. Su ambición es
> llegar a ser el mejor DAW del mercado, y para eso hace falta la gente que lo usa.
>
> - Las **preguntas** van en *Q&A*; cuando una respuesta resuelva la tuya, márcala.
> - Las **canciones y sonidos** hechos con Cantus van en *Show and tell*. Me encantará
>   escucharlos.
> - Las **ideas** van en *Ideas*: cuenta qué intentabas hacer, no solo la función.
> - Los **errores** van en [issues](https://github.com/luis314159/cantus-daw/issues/new/choose),
>   donde el formulario pide lo que hace falta para arreglarlos.
>
> Sé amable y concreto. Gracias por estar aquí. — Luis

## 4. Discord, later

Not now. A Discord with three members reads as empty, and Discussions is searchable, kept
with the code, and needs no new account for anyone who reports a bug. Open one when
Discussions has steady daily traffic or people ask for live chat. Its plan, then: #announcements,
#help, #share-your-music, #ideas, #español, and a bot posting each new release.
