# When the window misbehaves — flickering, jumping, black

*(Español abajo.)*

Cantus draws its window with OpenGL (eframe's `glow` renderer) on Windows, Linux and macOS.
If the window flickers, keeps changing size, jumps, or stays black, the steps below tell
us — and you — where it comes from. They are in the order to try them.

## 0. In beta.14 and earlier: press Ctrl + 0

The flicker reported in GitHub issue #1 (*"the interface keeps flickering along both the
horizontal and vertical axes"*) was Cantus's own: on a fresh installation the interface's
scale follows the screen, and on most screens it measured itself and alternated between two
sizes every other frame. It is fixed in the build after beta.14 (ADR 0146). In beta.14 and
earlier, **press Ctrl + 0** once in the Cantus window (or pick any scale in *Settings ▸
Appearance*, or press Ctrl + / Ctrl −): your own choice of scale stops the following, and
it is remembered. If you cannot get to the window, close Cantus and edit `settings.json`
in the data folder (see below): set `"ui_scale_follows_screen": false`.

## 1. Where the files are

| | Portable zip | Installed |
|---|---|---|
| Program | the unzipped folder, e.g. `Downloads\Cantus-0.1.0-beta.15\cantus.exe` | `%LOCALAPPDATA%\Programs\Cantus\cantus.exe` |
| Data (settings, log) | `<that folder>\data\` | `%LOCALAPPDATA%\Cantus\` |
| **The log** | `<that folder>\data\logs\cantus.log` | `%LOCALAPPDATA%\Cantus\logs\cantus.log` |

The log is written at every start (the start before is kept as `cantus.previous.log`).
Its first lines name the OpenGL driver (`opengl version`, `opengl renderer`, `opengl
vendor`) and the graphics options of the start (`graphics: renderer=glow vsync=on …`). On
Linux the data folder is `~/.local/share/cantus` (or beside a portable copy).

## 2. Start with safe graphics

Open PowerShell in the folder that holds `cantus.exe` (in Explorer: the folder, then
*File ▸ Open Windows PowerShell*, or type `powershell` in the address bar), then:

```powershell
.\cantus.exe --safe-graphics
```

`--safe-graphics` opens the main window at once (no splash that turns into it), at 100 %
interface scale, with vsync on, no multisampling and no transparency. If the window is
fine now, try the parts one at a time to find which one matters:

```powershell
.\cantus.exe --no-splash          # the main window at once, decorated, maximised
.\cantus.exe --scale 1            # our interface scale fixed for this session (1, 1.25, 150 …)
.\cantus.exe --vsync off          # the swap interval off (on by default)
.\cantus.exe --vsync on
.\cantus.exe --msaa 0             # multisampling off (the default); --msaa 4 to try it on
.\cantus.exe --no-splash --window 1600 900   # not maximised, at this size
```

Without a command line (a shortcut, the Start menu), the same choices go in the
`CANTUS_GRAPHICS` variable, comma-separated — a flag on the command line wins over it:

```powershell
$env:CANTUS_GRAPHICS = "safe"; .\cantus.exe
$env:CANTUS_GRAPHICS = "no-splash,vsync=off,scale=1.25"; .\cantus.exe
```

`--scale` is never saved; Ctrl + and Ctrl − still change the scale in that session.

## 3. Record what the window does

```powershell
$env:CANTUS_WINDOW_TRACE = "1"; .\cantus.exe
```

Every change of the window — its size and position in points and in pixels, the system's
scale, our zoom, maximised, full screen, the monitor — is written to the log, one line
each, with `(input)` when a click or a key came with it, and every scale the interface
chose for the screen. Run it for ten seconds, close Cantus, and send `cantus.log`.

Without the variable, a watchdog is always on: if the window's state goes back and forth
six times in a second with no input, it writes `window watchdog: …` with the last sixteen
transitions to the log, stops the interface following the screen, keeps the zoom it has,
and sends the window no more size or maximise commands in that session.

## 4. Things outside Cantus to try (AMD and Intel integrated graphics)

- Update the graphics driver (AMD Adrenalin, or the laptop maker's). After an update, a
  clean install (AMD Cleanup Utility) has fixed OpenGL problems for other programs.
- Set the display scale to 100 % for one try (*Settings ▸ System ▸ Display ▸ Scale*). If
  the flicker stops, tell us your resolution and the scale at which it happens.
- Unplug a second monitor for one try, or move Cantus to the other one (winit has a known
  loop between monitors of different scales, rust-windowing/winit#3040).
- In AMD Software: turn off *Instant Replay* / *Record & Stream*, *FreeSync* for the panel,
  and *Vari-Bright*, one at a time.
- On a laptop with two GPUs, choose the other one for `cantus-x86-64-v3.exe` and
  `cantus-x86-64-v2.exe` in *Settings ▸ System ▸ Display ▸ Graphics*.

## 5. What to send

The log (`cantus.log`, and `cantus.previous.log` if the bad run was the one before), the
display resolution and scale, how many monitors, the graphics driver's version, which of
the options above stopped it, and if you can, a few seconds of video (Win + Alt + R records
the screen).

---

# Cuando la ventana falla — parpadea, salta, se queda negra

Cantus dibuja su ventana con OpenGL (el renderizador `glow` de eframe). Si la ventana
parpadea, cambia de tamaño sin parar, salta o se queda negra, estos pasos nos dicen — y te
dicen — de dónde viene. Están en el orden en que conviene probarlos.

## 0. En beta.14 y anteriores: pulsa Ctrl + 0

El parpadeo del issue #1 (*"la interfaz parpadea en los ejes horizontal y vertical"*) era
de Cantus: en una instalación nueva la escala de la interfaz sigue a la pantalla, y en la
mayoría de las pantallas se medía a sí misma y alternaba entre dos tamaños cada dos
fotogramas. Está corregido en la versión siguiente a beta.14 (ADR 0146). En beta.14 y
anteriores, **pulsa Ctrl + 0** una vez en la ventana de Cantus (o elige cualquier escala en
*Ajustes ▸ Apariencia*, o Ctrl + / Ctrl −): tu propia escala detiene el seguimiento y se
recuerda. Si no llegas a la ventana, cierra Cantus y edita `settings.json` en la carpeta de
datos (abajo): pon `"ui_scale_follows_screen": false`.

## 1. Dónde están los archivos

| | Zip portátil | Instalado |
|---|---|---|
| Programa | la carpeta descomprimida, p. ej. `Descargas\Cantus-0.1.0-beta.15\cantus.exe` | `%LOCALAPPDATA%\Programs\Cantus\cantus.exe` |
| Datos (ajustes, registro) | `<esa carpeta>\data\` | `%LOCALAPPDATA%\Cantus\` |
| **El registro** | `<esa carpeta>\data\logs\cantus.log` | `%LOCALAPPDATA%\Cantus\logs\cantus.log` |

El registro se escribe en cada arranque (el del arranque anterior queda como
`cantus.previous.log`). Sus primeras líneas nombran el controlador OpenGL y las opciones
gráficas del arranque.

## 2. Arranca con gráficos seguros

Abre PowerShell en la carpeta de `cantus.exe` (en el Explorador, escribe `powershell` en la
barra de direcciones) y:

```powershell
.\cantus.exe --safe-graphics
```

`--safe-graphics` abre la ventana principal directamente (sin la pantalla de inicio que se
convierte en ella), con la interfaz al 100 %, vsync activado, sin multimuestreo y sin
transparencia. Si así va bien, prueba las partes una a una: `--no-splash`, `--scale 1`
(o `1.25`, `150`…), `--vsync off`, `--vsync on`, `--msaa 0`, `--no-splash --window 1600 900`.

Sin línea de órdenes (un acceso directo), las mismas opciones van en la variable
`CANTUS_GRAPHICS`, separadas por comas; una opción en la línea de órdenes gana:

```powershell
$env:CANTUS_GRAPHICS = "safe"; .\cantus.exe
$env:CANTUS_GRAPHICS = "no-splash,vsync=off,scale=1.25"; .\cantus.exe
```

`--scale` nunca se guarda.

## 3. Graba lo que hace la ventana

```powershell
$env:CANTUS_WINDOW_TRACE = "1"; .\cantus.exe
```

Cada cambio de la ventana (tamaño y posición, escala del sistema, nuestro zoom, maximizada,
pantalla completa, el monitor) va al registro, una línea cada uno, con `(input)` si vino
con un clic o una tecla. Déjalo diez segundos, cierra Cantus y envía `cantus.log`.

Sin la variable hay un vigilante siempre activo: si el estado de la ventana va y vuelve
seis veces en un segundo sin que nadie toque nada, escribe `window watchdog: …` con las
últimas dieciséis transiciones, deja de seguir a la pantalla, conserva el zoom que tiene y
no envía más órdenes de tamaño ni de maximizar en esa sesión.

## 4. Fuera de Cantus (gráficos integrados AMD e Intel)

- Actualiza el controlador (AMD Adrenalin o el del fabricante del portátil); tras
  actualizar, una instalación limpia (AMD Cleanup Utility).
- Pon la escala de pantalla al 100 % una vez (*Configuración ▸ Sistema ▸ Pantalla ▸
  Escala*); si deja de parpadear, dinos la resolución y la escala en que pasa.
- Desconecta un segundo monitor una vez, o lleva Cantus al otro.
- En AMD Software, desactiva de uno en uno *Instant Replay* / *Grabar y transmitir*,
  *FreeSync* del panel y *Vari-Bright*.
- En un portátil con dos GPU, elige la otra para `cantus-x86-64-v3.exe` y
  `cantus-x86-64-v2.exe` en *Configuración ▸ Sistema ▸ Pantalla ▸ Gráficos*.

## 5. Qué enviar

El registro (`cantus.log`, y `cantus.previous.log` si el arranque malo fue el anterior), la
resolución y la escala de la pantalla, cuántos monitores, la versión del controlador, cuál
de las opciones lo detuvo y, si puedes, unos segundos de vídeo (Win + Alt + R graba la
pantalla).
