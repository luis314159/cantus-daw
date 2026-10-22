Cantus {version}
================

Cantus is a free, open-source digital audio workstation.
https://github.com/luis314159/cantus-daw

Licence: the program is GPL-3.0-or-later (LICENSE.txt); COPYRIGHT.txt says which
parts of the source are MIT OR Apache-2.0. THIRD-PARTY-NOTICES.txt names everything
Cantus is built from and its licence; PRIVACY.txt says what Cantus sends (only an
anonymous update check) and what stays on your computer.

Manual: manual\en\index.html (Help > Manual in Cantus).

The programs
------------
cantus.exe starts Cantus. It looks at the processor and starts the program
built for it: cantus-x86-64-v3.exe on processors with AVX2 (Intel since 2013,
AMD since 2015), cantus-x86-64-v2.exe on older ones. Both make exactly the
same sound; v3 is faster. Set CANTUS_CPU=v2 to start v2 anyway, and run
"cantus.exe --launcher-info" to see what was chosen and why. cantus-scan.exe
reads plug-in files for Cantus, one at a time, so a faulty plug-in cannot crash
it.

Plug-ins
--------
Cantus's effects also run in other DAWs, as CLAP and VST3 plug-ins. Copy the
"Cantus <Name>.clap" files into your CLAP folder and the "Cantus <Name>.vst3"
folders into your VST3 folder:

  CLAP, for you:        %LOCALAPPDATA%\Programs\Common\CLAP
  CLAP, for everyone:   C:\Program Files\Common Files\CLAP
  VST3, for you:        %LOCALAPPDATA%\Programs\Common\VST3
  VST3, for everyone:   C:\Program Files\Common Files\VST3

The installer can put them in the "for you" folders. A DAW that does not scan
those folders can be pointed at them in its plug-in settings.

Portable copy
-------------
A copy with portable.txt beside cantus.exe keeps its settings in the "data"
folder beside it. Delete portable.txt to use your profile instead.

Updates
-------
An installed Cantus checks GitHub Releases at most once a day and asks before
downloading anything. Settings > Updates turns this off. Nothing about you or
your music is sent.

---

Cantus es una estación de trabajo de audio digital libre y de código abierto.
cantus.exe inicia el programa hecho para tu procesador: cantus-x86-64-v3.exe si
tiene AVX2, si no cantus-x86-64-v2.exe (suenan exactamente igual; v3 es más
rápido). Los efectos funcionan también en otros DAW como plug-ins CLAP y VST3 (ver
arriba las carpetas). Una copia con portable.txt junto a cantus.exe guarda sus
ajustes en la carpeta "data". Un Cantus instalado busca actualizaciones en
GitHub como mucho una vez al día y pregunta antes de descargar; se desactiva en
Ajustes > Actualizaciones. No se envía nada sobre ti ni sobre tu música.
Manual: manual\es\index.html (Ayuda > Manual). Avisos de terceros:
THIRD-PARTY-NOTICES.txt. Privacidad: PRIVACY.txt.
