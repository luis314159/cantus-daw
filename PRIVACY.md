# Privacy

*Cantus 0.1.0-beta.9 · written 2026-10-06, AI added 2026-10-30 · [Español abajo](#privacidad-español)*

**Cantus collects nothing about you.** There is no telemetry, no analytics, no crash
reporting, no account and no advertising. Your projects, recordings and settings stay on
your computer. Cantus connects to the internet in exactly the cases below, and each one
is something you can see and switch off.

## 1. The update check (on by default, switchable)

**When.** An installed or portable copy, with *Settings ▸ Updates ▸ Check daily* on (the
default), asks once a day when it starts — at most once a day after an answer and at most
once an hour after a failure. *Help ▸ Check for updates…* and *Check now* ask at once. A
copy built from source never checks by itself.

**What is sent.** One HTTPS request to GitHub, and nothing else:

| | |
|---|---|
| Address | `GET https://api.github.com/repos/luis314159/cantus-daw/releases/latest` — or, in a pre-release build such as this beta, `GET https://api.github.com/repos/luis314159/cantus-daw/releases?per_page=20` (GitHub's "latest" never names a pre-release, so a beta asks for the recent list to hear of the next beta) |
| Headers | `User-Agent: cantus-update` (the program's name only: no version, no system, no identifier), `Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28`, and on later checks `If-None-Match` with the tag GitHub gave the last answer (the same value for everyone who asked about the same release list) |
| Body | none |

Like any web request, it reaches GitHub from your IP address at a moment in time; GitHub's
own privacy statement covers what GitHub keeps. Cantus sends no version number, no
machine name, no user name, no hardware or plug-in list, no project information and no
identifier of any kind.

**If an update is offered** and you click *Download*, Cantus downloads two files from the
release on github.com (GitHub serves them from its download servers): the installer and
`SHA256SUMS.txt`, with the same headers. They are saved in `%LOCALAPPDATA%\Cantus\updates\`,
checked (size, SHA-256, and the publisher's signature when the build is signed) and deleted
if they do not match. Nothing is installed unless you choose *Install when I close Cantus*
or *Restart and update*.

**What is kept.** `update.json` beside your settings: when the last check happened, the
answer's tag and text (so a repeat costs nothing), and a version you chose to skip.

**To switch it off:** *Settings ▸ Updates ▸ Check daily* → Off — or untick *Check for updates once a day* on the installer's *Privacy* page, which shows this summary before anything is installed. A proxy can be typed in
the same section; otherwise Cantus uses `HTTPS_PROXY` or Windows' proxy setting. HTTPS is
always verified with Windows' own certificate store; verification is never turned off.

## 2. Sound packs (only when you ask)

The sample browser and the *Sound packs* window download a pack **only when you click
*Install*** (or the starter kit's offer). The files come from the addresses listed in
`crates/cantus-content/manifest/content.json` — today `raw.githubusercontent.com` and
`github.com`, the packs' own authors' repositories. The requests carry
`User-Agent: Cantus/0.1.0-beta.9` (the program and its version) and, when an interrupted
download resumes, a `Range` header. Nothing else is sent. Packs are saved in
`%LOCALAPPDATA%\Cantus\Content\` and each file's SHA-256 is checked before it is used.
Cantus never checks for new packs by itself.

## 3. The installer's optional FlexASIO step

The installer offers an unticked component, *FlexASIO driver (downloaded now)*. Only if you
tick it, the installer asks GitHub's API for FlexASIO's latest release
(`api.github.com/repos/dechamps/FlexASIO/releases/latest`), downloads its installer from
github.com through Windows' own web client and proxy settings, checks it and runs it. Cantus
itself never downloads drivers.

## 4. Pages opened in your browser

Some buttons open a web page in your browser: FlexASIO's and ASIO4ALL's pages and the
interface makers' driver pages (*Get a low-latency driver*), a release's page, the online
manual, and GitHub's issue forms (*Help ▸ Report a bug*). Cantus only hands the address to
your browser; what happens there is between you and that site.

## 5. Plug-ins made by others

CLAP and VST3 plug-ins you install run inside Cantus with your permissions. Cantus does not
restrict or watch them; some may connect to the internet (licence checks, content
downloads). Their makers' privacy policies apply.

## 6. AI: agents and models (only when you turn them on)

**Cantus has no AI service of its own and sends nothing to one.** It never asks for, keeps
or uses an API key of yours — not your Claude key, nor any other.

**Agents (MCP).** An assistant you run yourself (Claude Code, Claude Desktop, Cursor, VS
Code, Gemini CLI, LM Studio) can drive Cantus only when you allow it: *Settings ▸ AI ▸ Allow
external agents* is **off by default**. When on, Cantus listens on `127.0.0.1` only — this
computer, never the network — and answers only a client that carries a token kept in
`mcp\` beside your settings, readable by your user alone (*Make a new token* disconnects
every client). The `cantus-mcp` bridge that some clients start talks to that same local
door and nowhere else. What an agent reads from Cantus — track names, notes, settings,
the numbers of a listen — goes to the assistant on your computer; what that assistant then
sends to its own company is governed by its maker's privacy terms, not by Cantus. *AI area:
Off* closes the door and refuses every agent.

**Models (stems and audio to MIDI).** No model is in the installer. The first time you
separate stems or convert audio to MIDI, Cantus shows the model's licence, size and origin
and downloads it **only when you accept**, from its maker:

| What | From |
|---|---|
| HT-Demucs (stems) | Meta: `dl.fbaipublicfiles.com`; its licence from `raw.githubusercontent.com` (Meta's repository) |
| Basic Pitch (notes and chords) | Spotify: `raw.githubusercontent.com` (Spotify's repository, a pinned commit) |
| Onsets and Frames Drums (optional) | Hugging Face: `huggingface.co` (the ONNX conversion of Magenta's model, a pinned revision) |
| ONNX Runtime (runs the models) | Microsoft: its package on `files.pythonhosted.org` for Windows, its releases on `github.com` for Linux and macOS |

The requests carry `User-Agent: Cantus/0.1.0-beta.9` and, when a download resumes, a `Range`
header — nothing else. Each file's SHA-256 is checked before it is used. **ONNX Runtime's
telemetry is turned off**: Microsoft's builds would otherwise send usage events, so Cantus
sets `ORT_DISABLE_TELEMETRY=1` before the library loads and builds its environment with
telemetry disabled. The models run in a helper program (`cantus-ml`) on your computer; your
audio never leaves it. *Settings ▸ Models ▸ Remove* deletes a model.

## What stays on your computer

Everything else. Cantus keeps its files in `%LOCALAPPDATA%\Cantus\` (or in `data\` beside
`cantus.exe` for a portable copy):

- `settings.json`, `keymap.toml`, `layouts.json`, `appearance.json`/`theme.json`,
  `devices.json` (favourites and recent devices), `plugins.json` (the plug-ins the scan
  found), `update.json`;
- `autosave\` (recovery copies of unsaved projects), `recordings\` (takes made before a
  project is saved), `Content\` (sound packs), `cache\` (waveform peaks, thumbnails, logos),
  `updates\` (a downloaded installer), `Models\` (the AI models you downloaded and their
  runtime), `mcp\` (the agents' tokens, readable by you alone).

Your projects, their `.autosave` and `.bak` copies and their `Audio` folders are wherever
you save them. Uninstalling removes this folder only if you tick that option.

A separation's stems are written into your project's `Audio\Stems\` folder; the WAVs an
agent's listen keeps go to `cache\listen\` (the newest eight).

## Questions

Open an issue at <https://github.com/luis314159/cantus-daw/issues>, or write through
[lalmazan.com](https://lalmazan.com). This statement changes only with a release, and the
release notes say so.

---

# Privacidad (español)

**Cantus no recopila nada sobre ti.** No hay telemetría, ni analítica, ni informes de
fallos, ni cuentas, ni publicidad. Tus proyectos, grabaciones y ajustes se quedan en tu
computadora. Cantus se conecta a internet solo en estos casos, y cada uno se puede ver y
apagar.

## 1. La búsqueda de actualizaciones (activada por defecto, se puede apagar)

**Cuándo.** Una copia instalada o portátil, con *Ajustes ▸ Actualizaciones ▸ Buscar a
diario* activado (lo predeterminado), pregunta una vez al día al iniciar — como mucho una
vez al día tras una respuesta y una vez por hora tras un fallo. *Ayuda ▸ Buscar
actualizaciones…* y *Buscar ahora* preguntan al momento. Una copia compilada desde el
código nunca busca por sí sola.

**Qué se envía.** Una petición HTTPS a GitHub y nada más:

| | |
|---|---|
| Dirección | `GET https://api.github.com/repos/luis314159/cantus-daw/releases/latest` — o, en una versión preliminar como esta beta, `GET https://api.github.com/repos/luis314159/cantus-daw/releases?per_page=20` (el "latest" de GitHub nunca nombra una versión preliminar, así que una beta pide la lista reciente para enterarse de la siguiente beta) |
| Cabeceras | `User-Agent: cantus-update` (solo el nombre del programa: sin versión, sin sistema, sin identificador), `Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28` y, en búsquedas posteriores, `If-None-Match` con la etiqueta que GitHub dio a la última respuesta (el mismo valor para todos los que preguntaron por la misma lista) |
| Cuerpo | ninguno |

Como cualquier petición web, llega a GitHub desde tu dirección IP en un momento dado; lo
que GitHub guarda lo cubre su propia declaración de privacidad. Cantus no envía número de
versión, nombre del equipo, nombre de usuario, lista de hardware o de plug-ins, datos de
tus proyectos ni identificador alguno.

**Si se ofrece una actualización** y haces clic en *Descargar*, Cantus descarga dos
archivos de la versión en github.com: el instalador y `SHA256SUMS.txt`, con las mismas
cabeceras. Se guardan en `%LOCALAPPDATA%\Cantus\updates\`, se verifican (tamaño, SHA-256 y
la firma del editor cuando la versión está firmada) y se borran si no coinciden. No se
instala nada a menos que elijas *Instalar al cerrar Cantus* o *Reiniciar y actualizar*.

**Qué se guarda.** `update.json` junto a tus ajustes: cuándo fue la última búsqueda, la
etiqueta y el texto de la respuesta, y una versión que decidiste omitir.

**Para apagarla:** *Ajustes ▸ Actualizaciones ▸ Buscar a diario* → No — o desmarca *Buscar actualizaciones una vez al día* en la página *Privacidad* del instalador, que muestra este resumen antes de instalar nada. En la misma sección
puedes escribir un proxy; si no, Cantus usa `HTTPS_PROXY` o el proxy de Windows. HTTPS se
verifica siempre con el almacén de certificados de Windows; la verificación nunca se apaga.

## 2. Paquetes de sonidos (solo cuando lo pides)

El explorador de muestras y la ventana *Paquetes de sonidos* descargan un paquete **solo
cuando haces clic en *Instalar*** (o en la oferta del kit inicial). Los archivos vienen de
las direcciones de `crates/cantus-content/manifest/content.json` — hoy
`raw.githubusercontent.com` y `github.com`, los repositorios de los propios autores. Las
peticiones llevan `User-Agent: Cantus/0.1.0-beta.9` (el programa y su versión) y, al
reanudar una descarga interrumpida, una cabecera `Range`. No se envía nada más. Los
paquetes se guardan en `%LOCALAPPDATA%\Cantus\Content\` y el SHA-256 de cada archivo se
comprueba antes de usarlo. Cantus nunca busca paquetes nuevos por sí solo.

## 3. El paso opcional de FlexASIO en el instalador

El instalador ofrece un componente sin marcar, *Controlador FlexASIO (se descarga ahora)*.
Solo si lo marcas, el instalador pregunta a la API de GitHub por la última versión de
FlexASIO, descarga su instalador de github.com con el cliente web y el proxy de Windows,
lo verifica y lo ejecuta. Cantus en sí nunca descarga controladores.

## 4. Páginas que se abren en tu navegador

Algunos botones abren una página en tu navegador: las de FlexASIO y ASIO4ALL y las de los
fabricantes de interfaces (*Obtener un controlador de baja latencia*), la página de una
versión, el manual en línea y los formularios de GitHub (*Ayuda ▸ Reportar un error*).
Cantus solo le pasa la dirección a tu navegador.

## 5. Plug-ins de terceros

Los plug-ins CLAP y VST3 que instales se ejecutan dentro de Cantus con tus permisos. Cantus
no los limita ni los vigila; algunos pueden conectarse a internet (licencias, descargas de
contenido). Aplican las políticas de privacidad de sus fabricantes.

## 6. IA: agentes y modelos (solo cuando los activas)

**Cantus no tiene un servicio de IA propio ni envía nada a ninguno.** Nunca te pide, guarda
ni usa una clave de API tuya — ni tu clave de Claude ni ninguna otra.

**Agentes (MCP).** Un asistente que tú ejecutas (Claude Code, Claude Desktop, Cursor, VS
Code, Gemini CLI, LM Studio) solo puede manejar Cantus si lo permites: *Ajustes ▸ IA ▸
Permitir agentes externos* viene **desactivado**. Activado, Cantus escucha solo en
`127.0.0.1` — esta computadora, nunca la red — y solo responde a un cliente que lleve un
token guardado en `mcp\` junto a tus ajustes, que solo tu usuario puede leer (*Crear un
token nuevo* desconecta a todos los clientes). El puente `cantus-mcp` que arrancan algunos
clientes habla con esa misma puerta local y con nada más. Lo que un agente lee de Cantus —
nombres de pistas, notas, ajustes, los números de una escucha — va al asistente en tu
computadora; lo que ese asistente envíe después a su empresa lo rigen las condiciones de
privacidad de su fabricante, no Cantus. *Área de IA: Desactivada* cierra la puerta y
rechaza a todo agente.

**Modelos (stems y audio a MIDI).** Ningún modelo viene en el instalador. La primera vez
que separas stems o conviertes audio a MIDI, Cantus muestra la licencia, el tamaño y el
origen del modelo y lo descarga **solo si aceptas**, de su autor:

| Qué | De dónde |
|---|---|
| HT-Demucs (stems) | Meta: `dl.fbaipublicfiles.com`; su licencia de `raw.githubusercontent.com` (el repositorio de Meta) |
| Basic Pitch (notas y acordes) | Spotify: `raw.githubusercontent.com` (el repositorio de Spotify, en un commit fijo) |
| Onsets and Frames Drums (opcional) | Hugging Face: `huggingface.co` (la conversión a ONNX del modelo de Magenta, en una revisión fija) |
| ONNX Runtime (ejecuta los modelos) | Microsoft: su paquete en `files.pythonhosted.org` para Windows, sus versiones en `github.com` para Linux y macOS |

Las peticiones llevan `User-Agent: Cantus/0.1.0-beta.9` y, al reanudar una descarga, una
cabecera `Range` — nada más. El SHA-256 de cada archivo se comprueba antes de usarlo. **La
telemetría de ONNX Runtime está apagada**: las versiones de Microsoft enviarían eventos de
uso, así que Cantus pone `ORT_DISABLE_TELEMETRY=1` antes de cargar la biblioteca y crea su
entorno con la telemetría desactivada. Los modelos corren en un programa auxiliar
(`cantus-ml`) en tu computadora; tu audio nunca sale de ella. *Ajustes ▸ Modelos ▸ Quitar*
borra un modelo.

## Lo que se queda en tu computadora

Todo lo demás. Cantus guarda sus archivos en `%LOCALAPPDATA%\Cantus\` (o en `data\` junto a
`cantus.exe` en una copia portátil): ajustes, atajos, diseños, apariencia, dispositivos
favoritos y recientes, la lista de plug-ins encontrados, el estado de las actualizaciones;
`autosave\`, `recordings\`, `Content\`, `cache\`, `updates\`, `Models\` (los modelos de IA que
descargaste y su motor) y `mcp\` (los tokens de los agentes, que solo tú puedes leer). Tus proyectos, sus copias
`.autosave` y `.bak` y sus carpetas `Audio` están donde tú los guardes. Al desinstalar,
esta carpeta se borra solo si marcas esa opción.

Los stems de una separación se escriben en la carpeta `Audio\Stems\` de tu proyecto; los WAV
que guarda la escucha de un agente van a `cache\listen\` (los ocho más recientes).

## Preguntas

Abre un issue en <https://github.com/luis314159/cantus-daw/issues> o escribe a través de
[lalmazan.com](https://lalmazan.com). Esta declaración solo cambia con una nueva versión, y
las notas de la versión lo dicen.
