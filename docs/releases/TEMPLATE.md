<!--
The release notes' template (ADR 0067). Copy it to docs/releases/v<version>.md, replace
every {placeholder}, and delete this comment: release.yml publishes that file as the
release's text, and the app shows its first 80 lines in the update window — what is new
comes first. Keep the English and the Spanish halves saying the same things.
-->
**Cantus {version} — {one line on what this release is}.** A free, open-source digital
audio workstation for Windows, macOS and Linux, made by Luis Ángel Almazán
([lalmazan.com](https://lalmazan.com)).
*"Whatever you do, work heartily, as for the Lord" — Colossians 3:23.*
[Español abajo](#cantus-{version-anchor}--{spanish-title-anchor}).

## What is new

- **{Feature}**: {what it does for the user, in their words}.

## Fixed

- {What was wrong, now right.}

## Install

| System | File | |
|---|---|---|
| Windows 11, x64 | `Cantus-{version}-windows-x64-setup.exe` | Run it; no administrator rights. Also a portable zip. |
| macOS, Apple silicon and Intel | `Cantus-{version}-macos-universal.dmg` | Open it and drag Cantus onto *Applications*. |
| Linux x86-64 (Ubuntu 22.04 or newer, or alike) | `Cantus-{version}-linux-x86_64.AppImage` | `chmod +x` it and open it; nothing to install. |

Each file is also published under a name without the version (`Cantus-windows-x64-setup.exe`,
`Cantus-macos-universal.dmg`, `Cantus-linux-x86_64.AppImage`), the names the website's
button links to. Check a download against `SHA256SUMS.txt` if you like:
`Get-FileHash <file>` on Windows, `shasum -a 256 <file>` on a Mac, `sha256sum <file>` on
Linux. {Signing: say which files are signed — e.g. "The Windows installer is not
code-signed yet: choose *More info ▸ Run anyway*. The disk image is notarised by Apple." or
"is not notarised yet: the first time, right-click Cantus ▸ *Open*."}

**Updating from an earlier version:** Cantus tells you when this version is out. On
Windows it installs when you close Cantus; an AppImage replaces itself (the old one is kept
beside it, ending in `.old`, until the new one starts); on a Mac the disk image opens for
you to drag the new Cantus over the old one. A copy from a distribution's package or
Flatpak updates with it.

Start with the [quick-start manual](https://github.com/luis314159/cantus-daw/blob/master/docs/manual/en/README.md)
(also in *Help ▸ Manual*).

## Known issues

- {Issue, and what to do meanwhile.}

## Feedback

[Open an issue](https://github.com/luis314159/cantus-daw/issues) — say what you did, what
you expected and what happened, and your system.

---

## Cantus {version} — {título en español}

### Qué hay de nuevo

- **{Función}**: {qué hace por quien la usa}.

### Corregido

- {Lo que estaba mal y ya está bien.}

### Instalación

| Sistema | Archivo | |
|---|---|---|
| Windows 11, x64 | `Cantus-{version}-windows-x64-setup.exe` | Ejecútalo; sin permisos de administrador. También hay un zip portátil. |
| macOS, Apple silicon e Intel | `Cantus-{version}-macos-universal.dmg` | Ábrelo y arrastra Cantus a *Aplicaciones*. |
| Linux x86-64 (Ubuntu 22.04 o más nuevo, o similar) | `Cantus-{version}-linux-x86_64.AppImage` | Hazlo ejecutable (`chmod +x`) y ábrelo; nada que instalar. |

Cada archivo se publica también con un nombre sin la versión
(`Cantus-windows-x64-setup.exe`, `Cantus-macos-universal.dmg`,
`Cantus-linux-x86_64.AppImage`), al que apunta el botón de la web. Si quieres, compara la
descarga con `SHA256SUMS.txt`: `Get-FileHash <archivo>` en Windows, `shasum -a 256
<archivo>` en Mac, `sha256sum <archivo>` en Linux. {Firma: di qué archivos están firmados.}

**Actualizar desde una versión anterior:** Cantus te avisa cuando sale esta versión. En
Windows se instala al cerrar Cantus; un AppImage se reemplaza a sí mismo (el anterior se
guarda a su lado, terminado en `.old`, hasta que arranca el nuevo); en Mac se abre la
imagen de disco para que arrastres el Cantus nuevo sobre el anterior. Una copia del paquete
de una distribución o de Flatpak se actualiza con él.

Empieza con el [manual de inicio rápido](https://github.com/luis314159/cantus-daw/blob/master/docs/manual/es/README.md)
(también en *Ayuda ▸ Manual*).

### Problemas conocidos

- {Problema, y qué hacer mientras tanto.}

### Opiniones

[Abre un issue](https://github.com/luis314159/cantus-daw/issues) — cuenta qué hiciste, qué
esperabas, qué pasó y tu sistema.
