# Sesión con un Mac prestado — lista de comprobación

Todo lo de macOS está hecho y probado sin Mac (ADR 0066): compila para Apple silicon e
Intel, la batería de pruebas corre en el Mac de GitHub (`macos.yml`) y el `.dmg` se
construye allí. Esta sesión solo **verifica** lo que una máquina virtual no puede: sonido
de verdad, ventanas de verdad, Gatekeeper, el trackpad y tus oídos. Unas 2–3 horas.

## Antes de la sesión (sin el Mac)

1. **Cuenta de Apple Developer** (99 USD al año): <https://developer.apple.com/programs/>.
   Con ella, en *Certificates, Identifiers & Profiles*, crea un certificado **Developer
   ID Application** (la solicitud CSR puede hacerse en el Mac prestado con *Acceso a
   Llaveros ▸ Asistente de certificados*, o con `openssl` en Windows). Expórtalo con su
   clave como `.p12` con contraseña.
2. En <https://appleid.apple.com>, *Contraseñas específicas de app*: crea una para
   `notarytool`.
3. En GitHub, `luis314159/cantus-daw` ▸ *Settings ▸ Secrets and variables ▸ Actions*,
   añade los cinco secretos: `APPLE_CERT_P12_BASE64` (el `.p12` en base64:
   `certutil -encode cert.p12 cert.b64` en Windows, quitando las líneas BEGIN/END),
   `APPLE_CERT_PASSWORD`, `APPLE_TEAM_ID` (10 caracteres, en *Membership*), `APPLE_ID`
   (tu correo de Apple) y `APPLE_APP_PASSWORD`.
4. Descarga a una memoria USB: el `.dmg` del último `macos.yml` (artefacto
   `cantus-macos-dmg`) **y** el de la release anterior (para probar la actualización), un
   plug-in gratuito en CLAP o VST3 (por ejemplo Surge XT y Vital) y un proyecto tuyo.

## En el Mac

Anota versión de macOS, modelo (M1/M2/M3… o Intel), interfaz de audio y auriculares.

| # | Qué probar | Cómo | Esperado |
|---|---|---|---|
| 1 | **Gatekeeper** | Abre el `.dmg`, arrastra Cantus a Aplicaciones, ábrelo con doble clic. | Firmado y notarizado: abre sin aviso. Sin firmar: clic derecho ▸ *Abrir* ▸ *Abrir* una vez. |
| 2 | **Micrófono** | Arma una pista y graba. | macOS pregunta por el micrófono **una vez**, en tu idioma; al permitir, el medidor se mueve. |
| 3 | **Dúplex 10 minutos** | Tu interfaz a 128 muestras / 48 kHz; graba 10 minutos tocando mientras suena un proyecto. | Ni un clic ni un corte; la latencia que muestra el chip es razonable (≈ 6–10 ms). Repite a 64 muestras. |
| 4 | **Dispositivo agregado** | *Configuración de Audio MIDI* ▸ *+* ▸ dispositivo agregado (interfaz + salida del Mac). | Aparece en la página del dispositivo y suena. |
| 5 | **Plug-in real** | Instala Surge XT o Vital (CLAP/VST3); *Ajustes ▸ Plug-ins ▸ Buscar*. Ponlo en una pista y abre su editor. | Se encuentra; el editor abre en su ventana, a buen tamaño, nítido, sobre Cantus; responde al ratón; al cerrarlo no queda nada. |
| 6 | **Plug-in aislado** | El mismo plug-in con *Por separado* (sandbox). Abre su editor. | Igual que el 5; el Monitor de Actividad muestra `cantus-sandbox`. Al salir de Cantus, `cantus-sandbox` desaparece. |
| 7 | **Menús y teclas** | Usa la barra de menús de arriba; ⌘S, ⌘Z, ⌘⇧Z, ⌘, (Ajustes), ⌘Q; escribe en un campo de texto y usa ⌘C / ⌘V. | Los menús en tu idioma con sus atajos; ⌘M minimiza; ⌘C/⌘V copian el texto del campo. |
| 8 | **Trackpad** | En el piano roll y la ventana de arreglo: pellizcar, deslizar con dos dedos, ⌘+deslizar. | Zoom suave y desplazamiento natural, sin saltos. |
| 9 | **Retina** | Mira la interfaz de cerca y en una pantalla externa no Retina. | Texto y vectores nítidos en ambas; nada borroso ni desalineado. |
| 10 | **CPU en Apple silicon** | Abre una canción de demostración grande; mira el medidor de CPU de Cantus y el Monitor de Actividad. | Anota el % con 64 y 128 muestras; los hilos de audio en núcleos de rendimiento (P). |
| 11 | **Actualización** | Instala la release **anterior**, ábrela, *Ayuda ▸ Buscar actualizaciones*. | Ofrece la nueva; al cerrar se abre el `.dmg` nuevo; tras arrastrarla, la versión nueva arranca con tus ajustes. |
| 12 | **Arrastrar desde Finder** | Arrastra un `.wav` y un `.mid` a la ventana. | Se importan donde los sueltas. |
| 13 | **Abrir un proyecto desde Finder** | Con Cantus cerrado, doble clic en un `.cantus`; luego, con Cantus abierto y un cambio sin guardar, arrastra otro `.cantus` al icono del Dock. | El primero abre Cantus con ese proyecto; el segundo pregunta *¿Guardar cambios?* y luego lo abre. Finder muestra el icono de Cantus en los `.cantus`. |
| 14 | **Muestras desde disco** | Carga el piano de Salamander (16 capas) y toca un acorde enorme con el pedal, o pega 40 notas a la vez en el piano roll y reprodúcelas. | Ningún hueco ni clic al empezar las notas. En el Mac de GitHub (disco virtual) las 40 voces tardan 150–170 ms en tener su primer bloque tras la cabeza (171 ms); en un Mac real debería ser mucho menos. |
| 15 | **Plazo de un plug-in colgado** (opcional, con Rust instalado: `rustup`, y el código) | En la carpeta de Cantus: `cargo test -p cantus-rt --test ipc_process a_late_wait -- --nocapture` | Pasa: la espera de 5 ms acaba antes de 7 ms (en la máquina virtual de GitHub se permiten 10). Anota la mediana que imprime. |
| 16 | **Reloj MIDI a tiempo** (opcional, con Rust y el código, como el 15) | `cargo test -p cantus-engine --test sync_midi the_out_thread -- --nocapture` | Pasa: la mediana del retraso por debajo de 0,5 ms y el percentil 90 de 5 ms. En la máquina virtual de GitHub los temporizadores despiertan tarde (mediana 6,7–9,6 ms) y se permiten 15 y 35 ms. Anota lo que imprime; si en el Mac real también llega tarde, el hilo de salida MIDI necesita la clase QoS *user-interactive* o enviar con la marca de tiempo de CoreMIDI. |
| 17 | **Separar stems (la IA, ADR 0085)** <!-- r34:stems --> | *Ajustes ▸ Modelos*: acepta la licencia de HT-Demucs e instálalo; en una canción, clic derecho en un clip de audio ▸ *Separar stems…* ▸ *Separar*; luego vuelve a *Separar* con *Usar la tarjeta gráfica* activado. | Descarga el runtime de macOS (Apple silicon: 1.31.1; Intel: 1.23.2) y el modelo; `cantus-ml` aparece en el Monitor de Actividad y **carga la biblioteca de Microsoft** pese al *hardened runtime* (sus derechos quitan la validación de bibliotecas). Cuatro pistas bajo un bus, alineadas. Anota los segundos de una canción de 3 minutos en el procesador y con CoreML (y si CoreML la aceptó: el registro lo dice). |

## Cómo anotar los resultados

Copia esta tabla en `docs/round31/mac-session-results.md` (o pásamela en el chat):

| # | Resultado (✔ / ✘ / ~) | Notas (qué viste, números, captura) |
|---|---|---|
| 1 | | |
| 2 | | |
| 3 | | |
| 4 | | |
| 5 | | |
| 6 | | |
| 7 | | |
| 8 | | |
| 9 | | |
| 10 | | |
| 11 | | |
| 12 | | |
| 13 | | |
| 14 | | |
| 15 | | |
| 16 | | |
| 17 | | |

Con un ✘, guarda lo que puedas: el registro de Cantus (ábrelo desde *Terminal* con
`/Applications/Cantus.app/Contents/MacOS/cantus 2>&1 | tee ~/Desktop/cantus.log` y repite
la prueba) y, si Cantus se cerró solo, el informe de
`~/Library/Logs/DiagnosticReports/cantus-*.ips`. Unas capturas (⌘⇧4) ayudan mucho.
