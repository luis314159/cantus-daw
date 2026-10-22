# Cantus en Linux — instalación y configuración del audio

[English](../en/linux.md) · [Volver al manual](README.md)

## Qué necesitas

- Un PC de 64 bits (x86-64) con un Linux de 2022 o posterior: Ubuntu 22.04, Linux Mint
  21, Debian 12, Fedora 36, openSUSE Leap 15.5, Arch — o cualquiera más nuevo (glibc 2.35
  o posterior).
- Un escritorio con X11 o Wayland (en Wayland Cantus funciona a través de XWayland, que
  todos los escritorios Wayland traen; mira [Ventanas de los plug-ins](#ventanas-de-los-plug-ins)).
- Unos 300 MB de disco.

## Instalar: la AppImage

1. Descarga `Cantus-<versión>-linux-x86_64.AppImage` de la
   [página de versiones](https://github.com/luis314159/cantus-daw/releases) — o de
   [cantusdaw.com](https://cantusdaw.com).
2. Hazla ejecutable: en tu gestor de archivos, *Propiedades ▸ Permisos ▸ Permitir
   ejecutar el archivo como un programa*; o en una terminal: `chmod +x Cantus-*.AppImage`.
3. Haz doble clic en ella (o `./Cantus-*.AppImage`).

La AppImage es el programa entero en un solo archivo: no se instala nada en el sistema ni
se pide contraseña de administrador. Al arrancar por primera vez pone **Cantus en tu menú
de aplicaciones** — con su icono, y *Abrir con Cantus* para los proyectos `.cantus` —
apuntando al archivo donde está; si lo mueves, el siguiente arranque actualiza la
entrada. Para dejarlo fuera del menú, arráncalo con `CANTUS_NO_DESKTOP_ENTRY=1`.

Un buen sitio para guardarla: `~/Applications`. Las **actualizaciones** sustituyen ese
mismo archivo.

**Desinstalar**: borra la AppImage; quita la entrada del menú con
`rm ~/.local/share/applications/com.cantusdaw.Cantus.desktop`. Tus ajustes están en
`~/.config/cantus` y tus preajustes, grabaciones y autoguardados en `~/.local/share/cantus`
(las carpetas `XDG_CONFIG_HOME` y `XDG_DATA_HOME`, si las defines): bórralos solo si
quieres que desaparezcan. Tus proyectos nunca se tocan.

**Flatpak**: hay un Flatpak (`com.cantusdaw.Cantus`) preparado para Flathub; hasta que se
publique allí, usa la AppImage.

## Audio: el controlador

Abre *Ajustes ▸ Audio* (o el chip de audio de la barra superior). La lista
**Controlador** ofrece lo que tiene tu sistema, el mejor primero:

| Controlador | Cuándo | Latencia |
|---|---|---|
| **PipeWire** | Ubuntu 22.10 y posteriores, Fedora 34 y posteriores, Debian 12, Mint 22, Arch con PipeWire — la mayoría de escritorios hoy. Cantus habla con PipeWire directamente. | Baja: el búfer que eliges es al que PipeWire hace funcionar a Cantus (64–256 muestras van bien en la mayoría de equipos). |
| **JACK** | Cuando hay un servidor JACK en marcha (JACK2, o el JACK de PipeWire). | Baja: el periodo del servidor. |
| **ALSA** | Siempre. Por el dispositivo predeterminado del sistema (que en un sistema con PipeWire o PulseAudio pasa por ellos), o directo a una tarjeta de sonido (dispositivos `hw:`) cuando nada más la usa. | Más alta por el predeterminado; baja directo a la tarjeta. |

Elige el controlador, el **Dispositivo de salida** y el **Dispositivo de entrada**, la **Frecuencia de
muestreo** y el **Tamaño del búfer** (muestras), y *Aplicar*. Cantus recuerda la elección para cada
interfaz. No hay nada que instalar: si falta la biblioteca de PipeWire o de JACK,
simplemente no se ofrecen esos controladores.

**Menos latencia con PipeWire.** PipeWire hace funcionar a todos los programas con un
mismo *quantum* (su búfer). Cantus pide el búfer que elegiste; si otro programa pide uno
mayor, PipeWire puede usar ese. Para fijarlo: `pw-metadata -n settings 0
clock.force-quantum 128` (para volver a lo normal: `… 0`).

**JACK a través de PipeWire.** En un sistema con PipeWire sin servidor JACK, instala el
paquete JACK de PipeWire de tu distribución (`pipewire-jack`) para usar también ahí el
controlador JACK; el controlador propio de PipeWire es igual de rápido.

## Prioridad de tiempo real

En *Ajustes ▸ Audio*, la línea **Prioridad de tiempo real** dice qué dio el sistema a los
hilos de audio de Cantus: por ejemplo `SCHED_FIFO priority 50` (directa), `SCHED_RR
priority 20 via rtkit`, o `refused: …` con el motivo. El hilo de audio de PipeWire y de
JACK lo ponen ellos en tiempo real (`driver-owned thread: SCHED_FIFO`); Cantus eleva sus
propios hilos.

La mayoría de escritorios la conceden mediante **rtkit** sin hacer nada. Si la línea dice
*refused*, instala y arranca `rtkit` (`sudo apt install rtkit`), o da a tu usuario el
derecho a pedirla directamente — el clásico grupo audio:

```text
sudo usermod -aG audio $USER
echo '@audio - rtprio 95' | sudo tee /etc/security/limits.d/95-audio.conf
echo '@audio - memlock unlimited' | sudo tee -a /etc/security/limits.d/95-audio.conf
```

y cierra la sesión y vuelve a entrar. (Ubuntu Studio y Fedora Jam ya lo traen hecho.)

## Plug-ins

Cantus encuentra los plug-ins CLAP y VST3 en las carpetas estándar de Linux:

| Formato | Carpetas |
|---|---|
| CLAP | `~/.clap`, `/usr/lib/clap`, `/usr/local/lib/clap`, y las carpetas de `CLAP_PATH` |
| VST3 | `~/.vst3`, `/usr/lib/vst3`, `/usr/local/lib/vst3`, y las carpetas de `VST3_PATH` |

Instala un plug-in como diga su fabricante (normalmente: copia el archivo `.clap` o
la carpeta `.vst3` en `~/.clap` o `~/.vst3`), y luego *Ajustes ▸ Plug-ins ▸ Buscar
plug-ins*. Cada archivo se lee en un proceso de examen aparte, así que un plug-in que
falla al leerse solo aparece como *bloqueado*. Un plug-in también puede funcionar en
un proceso propio (*Cuando un plug-in falla*, en el manual): un fallo ahí se captura,
Cantus sigue y puedes volver a cargarlo.

Los efectos propios de Cantus también vienen en la AppImage como plug-ins CLAP y
VST3, para otros programas: extráelos con `./Cantus-*.AppImage --appimage-extract` y copia
`squashfs-root/usr/share/cantus/plugins/clap/*` en `~/.clap` y `…/plugins/vst3/*` en
`~/.vst3`.

### Ventanas de los plug-ins

El editor propio de un plug-in se abre en una ventana suya, que se mantiene encima de
la de Cantus. En Linux esa ventana es de **X11**, porque los plug-ins solo dibujan en
ventanas X11; en un escritorio Wayland pasa por XWayland, y la ventana de Cantus también,
así que las dos van juntas. (Arrancar Cantus con `CANTUS_WAYLAND=1` le da una ventana
Wayland nativa; los editores de los plug-ins se abren entonces como ventanas normales
que no se mantienen encima.)

## Archivos, diálogos y fuentes

- Los diálogos de abrir y guardar son los de tu escritorio (mediante el portal de
  escritorio XDG; con `zenity` donde no hay portal). En un gestor de ventanas sin ninguno
  de los dos, instala `xdg-desktop-portal-gtk` o `zenity`.
- El texto en chino, japonés, coreano, hindi o tailandés usa las fuentes que tu sistema
  tiene para ello (se las pregunta a fontconfig): instala `fonts-noto-cjk` (Debian/Ubuntu)
  o `google-noto-sans-cjk-fonts` (Fedora) si esos caracteres salen como cuadros.

## Si algo falla

| Qué ves | Qué hacer |
|---|---|
| No hay sonido y la lista de controladores solo tiene ALSA | PipeWire o JACK no están en marcha para tu usuario. Prueba ALSA con el dispositivo *default*, o arranca el servicio de sonido de tu escritorio. |
| Chasquidos y cortes | Sube el búfer (*Ajustes ▸ Audio ▸ Tamaño del búfer*); mira la línea **Prioridad de tiempo real** (arriba); cierra programas que usen mucho el procesador. |
| La ventana de un plug-in queda vacía o no se abre | Cantus funciona con `CANTUS_WAYLAND=1`, o el plug-in necesita una biblioteca que falta en tu sistema: arranca Cantus desde una terminal y lee lo que imprime el plug-in. |
| La AppImage no arranca (*FUSE*) | Las AppImage muy antiguas necesitaban `libfuse2`; la de Cantus no. Si tu sistema bloquea las AppImage, ejecuta `./Cantus-*.AppImage --appimage-extract` y arranca `squashfs-root/AppRun`. |
