# Cantus — manual de inicio rápido

*Para Cantus 0.1.0-beta.1 en Windows · [Read in English](../en/README.md)*

Cantus es una estación de trabajo de audio digital libre y de código abierto: graba,
compón, mezcla y masteriza una canción, de la primera nota al archivo terminado. Este
manual te lleva hasta ahí en una tarde. Todo lo que nombra está en el programa tal como es
hoy; donde la beta aún tiene huecos, el [capítulo 14](#14-problemas-conocidos-y-limitaciones)
lo dice.

![Cantus al iniciar: el arreglo, el mezclador abajo, el inspector a la derecha](../img/startup.png)

**Contenido**

1. [Instalación](#1-instalación)
2. [Primer inicio: configura tu audio](#2-primer-inicio-configura-tu-audio)
3. [El espacio de trabajo](#3-el-espacio-de-trabajo)
4. [Grabar audio y MIDI](#4-grabar-audio-y-midi)
5. [Editar: clips, piano roll, automatización](#5-editar-clips-piano-roll-automatización)
6. [Mezclar: el mezclador, inserciones y envíos](#6-mezclar-el-mezclador-inserciones-y-envíos)
7. [Masterizar: la cadena del máster, Fervor, Latitudo, Culmen](#7-masterizar-la-cadena-del-máster-fervor-latitudo-culmen)
8. [Exportar](#8-exportar)
9. [Sonidos: el explorador y los paquetes de sonidos](#9-sonidos-el-explorador-y-los-paquetes-de-sonidos)
10. [Instrumentos y efectos: el selector de dispositivos](#10-instrumentos-y-efectos-el-selector-de-dispositivos)
11. [Proyectos, autoguardado y ajustes](#11-proyectos-autoguardado-y-ajustes)
12. [Atajos de teclado](#12-atajos-de-teclado)
13. [Privacidad y actualizaciones](#13-privacidad-y-actualizaciones)
14. [Problemas conocidos y limitaciones](#14-problemas-conocidos-y-limitaciones)
15. [Reportar un error](#15-reportar-un-error)

## 1. Instalación

Necesitas Windows 11 de 64 bits (Windows 10 sin probar) y
unos 200 MB de disco.

1. Descarga `Cantus-0.1.0-beta.1-windows-x64-setup.exe` de la
   [página de versiones](https://github.com/luis314159/cantus/releases).
2. Ejecútalo. Se instala solo para ti, en `%LOCALAPPDATA%\Programs\Cantus`, y no pide
   permisos de administrador. Esta beta aún no está firmada, así que Windows puede mostrar
   *"Windows protegió su PC"*: elige *Más información ▸ Ejecutar de todas formas*.
3. Elige los componentes: el programa (obligatorio), *Abrir los proyectos .cantus con
   Cantus* (activado), un acceso directo en el escritorio (desactivado), los plug-ins CLAP
   y VST3 para otros DAW (activados) y *Controlador FlexASIO (se descarga ahora)*
   (desactivado — márcalo si tu tarjeta de sonido no tiene controlador ASIO propio; ver el
   [capítulo 2](#2-primer-inicio-configura-tu-audio)).
4. La página *Privacidad* dice qué envía Cantus por la red (solo una pregunta diaria por
   versiones nuevas, y las descargas que tú pidas). *Buscar actualizaciones una vez al día
   (recomendado)* viene marcada; desmárcala para apagar la búsqueda (también después, en
   *Ajustes ▸ Actualizaciones*).

**Copia portátil.** Descomprime `Cantus-0.1.0-beta.1-windows-x64-portable.zip` donde
quieras (una memoria USB sirve). Por el archivo `portable.txt` junto a `cantus.exe`, esa
copia guarda sus ajustes en la carpeta `data` de al lado y no escribe nada en tu perfil.

**Desinstalar** desde *Aplicaciones* de Windows. Tus ajustes, presets y autoguardados — y
la caché de muestras (hasta 16 GB, ver el [capítulo 11](#11-proyectos-autoguardado-y-ajustes)) —
solo se borran si marcas esa casilla; tus proyectos nunca se tocan.

## 2. Primer inicio: configura tu audio

La primera vez que Cantus arranca, se abre la ventana **Configura tu audio** sobre la
música.

![La configuración de audio del primer inicio](../img/first-run.png)

1. **Elige tu interfaz.** Cada dispositivo de audio tiene una tarjeta: *Interfaz de
   audio*, *Este equipo*, *Audífonos*… Haz clic en la que usas para tocar y escuchar.
   Cantus le elige controlador y búfer: **ASIO** a 128 muestras para una interfaz con
   controlador ASIO propio; si no, **WASAPI** (el audio de Windows) a 256.
2. **Escucha.** Haz clic en *Tono de prueba*: deberías oír un tono suave en las salidas
   1–2.
3. **Toca o habla — el medidor debe moverse.** Rasguea, canta o habla en lo que tengas
   conectado; la entrada que se mueve es la que vas a grabar.
4. **Latencia.** Cantus dice cuánto tarda el sonido de la cuerda a la bocina y si es lo
   bastante justo para tocar a través de él. Para tomas exactas, conecta con un cable la
   salida 1 a la entrada 1 y haz clic en *Medir latencia*; Cantus colocará cada toma a la
   muestra.

Haz clic en **Grabar en …** y Cantus añade una pista de audio en esa entrada, armada y
lista: pulsa `R` para grabar. *Omitir* cierra la ventana; no vuelve a abrirse sola (el
botón *Asistente…* del chip de audio la trae de vuelta).

### El chip de audio

El chip de la barra superior (o de su menú ⋯ cuando la ventana es estrecha) muestra el
dispositivo, el controlador, la
frecuencia, el búfer y la latencia — por ejemplo `UMC204HD · ASIO · 48 k · 128 · 6.1 ms`.
Su luz es verde cuando el audio funciona, ámbar tras un corte y roja cuando el dispositivo
se perdió o el controlador se detuvo. **Haz clic en él** para cambiar cualquier cosa; cada
cambio se aplica al instante y se recuerda para esa interfaz:

![La ventana del chip de audio](../img/audio-popover.png)

- **Interfaz**, **Controlador** (WASAPI o ASIO, con la lista *Controlador ASIO*),
  **Dispositivo de entrada**, **Frecuencia**;
- **Búfer · ida y vuelta**: de 64 a 2048 muestras, cada una con su latencia. Más pequeño
  es más justo para tocar; más grande es más seguro para mezclas grandes. Si la barra
  superior cuenta cortes, sube al siguiente tamaño;
- los problemas aparecen arriba con un botón que los arregla (*Reiniciar audio*, *Usar el
  audio de este equipo*, *Usar 48 kHz*…);
- *Tono de prueba*, *Canales y ruteo…* (nombra tus entradas y salidas, elige un par para
  la mezcla de audífonos), *Asistente…* y el engrane con todos los ajustes.

**¿Qué controlador?** Usa el controlador ASIO de tu interfaz si lo tiene (Behringer,
Focusrite, PreSonus, Steinberg, MOTU, RME…). Para el sonido integrado de una laptop, usa
WASAPI, o instala **FlexASIO** para menos latencia. *Obtener ASIO…* en la ventana del chip
abre otra con la página de descarga de FlexASIO, la de ASIO4ALL y las de los fabricantes;
tras instalar un controlador, haz clic en *Volver a buscar*. Cantus nunca instala
controladores por su cuenta.

Al conectar una interfaz, Cantus ofrece cambiar a ella; al desconectarla, ofrece el audio
de tu equipo y vuelve a ella cuando regresa. Si un controlador deja de responder, Cantus
reinicia el audio una vez por sí solo. No se puede cambiar el dispositivo mientras grabas.

## 3. El espacio de trabajo

Cantus abre con una canción — la segunda subida de la demo *Neon Tide*, en bucle — para
que lo primero que oigas sea música. Pulsa `Espacio` para reproducirla.

- **La barra superior** (ver [más abajo](#la-barra-superior)): los menús sobre el nombre
  del proyecto y el panel de pistas, y luego el transporte, el tempo, el metrónomo, la
  posición, los medidores y más: un grupo por función.
- **El arreglo** (centro): las pistas y sus clips sobre una línea de tiempo.
- **El navegador de clips** (`Alt+F5`, *Ver ▸ Navegador de clips*): todos los clips de la
  canción como mosaicos, junto al arreglo (ver el [capítulo 5](#los-clips-en-el-arreglo)).
- **La zona inferior**: el Mezclador, el Piano roll y el Rack de canales en pestañas.
- **El inspector** (derecha, `I`): todo sobre la pista, el canal o el clip seleccionado.
- **La barra de estado** (abajo): lo que hay bajo el puntero, la latencia de los plug-ins,
  las muestras que se cargan, la octava del teclado y los dispositivos minimizados.

Cada área se muestra u oculta con su tecla de ventana o su tecla de función (`F5` arreglo,
`Alt+F5` navegador de clips, `F6` rack de canales, `F7` piano roll, `F9` mezclador,
`Alt+F8` explorador, `I` inspector). Arrastra los divisores para cambiar tamaños; clic derecho en una pestaña para
que flote en su propia ventana; guarda la forma de la ventana en *Ver ▸ Disposición ▸
Guardar disposición…*. La interfaz se amplía del 75 % al 200 % (`Ctrl++`, `Ctrl+-`,
`Ctrl+0`). Doce temas — Cantus Noche (el predeterminado), Cantus Papel, Luz de día, Hora
azul, alto contraste oscuro y claro y más, cada uno con su vista previa —, los colores, el
logo y el color del icono de la barra de tareas están en *Ver ▸ Apariencia*.

**Escribir un nombre o un valor** funciona igual en todas partes — renombrar una pista, un
clip, un pad o un canal, una búsqueda, un nivel: el campo se abre con su texto
seleccionado, así que lo que escribes lo reemplaza; `Enter` o un clic fuera conserva lo
escrito; `Esc` deja lo que había. Mientras escribes, ningún atajo se dispara. Los nombres
pueden ir en cualquier escritura — japonés, chino, coreano, hindi, tailandés, árabe,
hebreo…: Cantus usa para ellos las fuentes de tu sistema (ver los
[problemas conocidos](#14-problemas-conocidos-y-limitaciones) sobre las escrituras de
derecha a izquierda).

**Canciones de demostración.** *Archivo ▸ Canciones de demostración…* (`Ctrl+Shift+N`)
abre seis canciones hechas solo con los instrumentos y efectos de Cantus: Neon Tide
(house), Morning Light (balada pop), Sunday Strut (funk y soul), Paper Lanterns (hip-hop
lo-fi), Aurora (cinemática) y Midnight Static (trap). Abre una, escúchala, desármala.

### La barra superior

La barra superior es la de FL Studio, un grupo por función y una línea fina entre ellos:

- **Los menús y el panel de pistas**: *Archivo, Edición, Clip, Ver, Ayuda* sobre el nombre
  del proyecto y la pista de lo que hay bajo el puntero. **Haz clic en el nombre** para
  cambiarlo ahí mismo (`Intro` lo guarda, `Esc` lo deja); el título de la ventana lo
  sigue, con `•` mientras haya cambios sin guardar.
- **PATRÓN / CANCIÓN** (`L`): el *modo canción* reproduce el arreglo; el *modo patrón*
  reproduce el clip que editas en el piano roll o en el rack de canales —con lo que el
  rack muestra de las otras pistas en ese tramo— solo y en bucle, y el reloj cuenta desde
  el inicio del patrón. Elegir otro clip cambia lo que suena. Al salir del modo patrón el
  cursor vuelve a donde estaba la canción. El modo patrón no graba (una toma pertenece a
  la canción: pasa antes al modo canción) y la exportación siempre renderiza la canción.
  Se guarda con el proyecto.
- **Transporte**: ir al inicio, reproducir, detener, grabar y sus opciones ▾, bucle.
- **Tempo**: los BPM (arrastra, usa la rueda, doble clic para 120); **TAP** (`Shift+T`):
  pulsa al menos dos veces —el tempo es la media de tus últimas pulsaciones, una pausa
  empieza de nuevo y cada tanda es un solo paso de deshacer—; el **compás**: haz clic
  para los tiempos (‹ ›), la figura y los compases habituales.
- **Metrónomo** (`Ctrl+M`): su interruptor, la **cuenta** previa (no, 1, 2, 4 compases) y
  el **nivel** del clic (arrastra, rueda, doble clic para 0 dB). Su ▾ guarda el
  **sonido** (*Clásico*, *Caja china*, *Pitido*), el **acento** en el primer tiempo de
  cada compás y **cuándo** suena (siempre, al grabar, durante la cuenta). El sonido y el
  acento se guardan con la canción.
- **Posición**: compases.tiempos.ticks y el tiempo, con el compás al lado; al grabar, el
  tiempo de la toma.
- **Rejilla y ajuste**: la rejilla y la luz IMÁN del editor en el que trabajas —el arreglo,
  el piano roll o el rack de canales (siempre sobre su rejilla)—. Los mismos valores que
  la barra del propio editor.
- **Deshacer** y **rehacer**, y las **teclas de ventana** (explorador, arreglo, rack, piano
  roll, mezclador, inspector).
- **CPU, RAM, voces, cortes**: la carga del audio (su histograma en la información
  emergente), la memoria que usa Cantus, las voces que suenan y los cortes contados (la
  causa del último en la información emergente, y dicha en la barra de estado cuando
  ocurre).
- **Medidor maestro**: *Pico* muestra los picos izquierdo y derecho y retiene el más alto
  de los dos últimos segundos (clic en el número para soltarlo); *LUFS* muestra la
  sonoridad del maestro —la momentánea arriba, la de corto plazo debajo, con una marca en
  −14 LUFS (el nivel de los servicios de streaming)— y debajo la de corto plazo, la
  integrada desde que pulsaste reproducir y el pico real.
- **Entradas**: el teclado del ordenador (`Ctrl+T`), la luz **MIDI** (se enciende cuando
  llegan notas; el número son las entradas abiertas) y **Armadas** (cuántas pistas
  grabarán; clic para armar la seleccionada).
- **El chip de audio** (ver [el chip de audio](#el-chip-de-audio)) y las **herramientas**:
  el selector de elementos, la página de apariencia y el zoom de la interfaz.

**Con la ventana estrecha** los grupos menos importantes se pliegan —enteros— en el menú
**⋯** del extremo derecho, donde funcionan igual que en la barra; los menús se pliegan en
una sola tecla antes de que lo haga el transporte. Un punto en ⋯ avisa de que hay algo
plegado (rojo si es el chip de audio y el audio necesita atención). **Personalizar la
barra superior…** (en ⋯, en *Ver* o con clic derecho en la barra): marca un grupo para
mostrarlo u ocultarlo, arrástralo por su asa para moverlo, *Restablecer* para volver al
orden por defecto. Tu disposición se recuerda.

## 4. Grabar audio y MIDI

### Una pista de audio

1. Añade una pista de audio: `Ctrl+Shift+A`, o clic derecho en el **+** sobre las
   cabeceras de pista ▸ *Añadir pista de audio*. (El *Grabar en …* del primer inicio te
   crea una.)
2. **Ármala**: haz clic en la luz roja redonda de su cabecera (o `Alt+R`). Mayús+clic arma
   solo esa pista. Una pista armada muestra el medidor de su entrada.
3. **Elige su entrada**: el chip de entrada de la cabecera (*Ent 1*, *Ent 1-2*…; aparece
   cuando la fila de la pista es lo bastante alta), la sección *Grabación* del inspector
   o el menú de la pista.
4. **Escucha**: la tecla del altavoz de la cabecera alterna *No*, *Auto* (oyes la entrada
   mientras la pista está armada — lo predeterminado) y *Sí*. El inspector tiene además
   *Ajuste* (−24 a +24 dB antes de grabar).
5. Pulsa **`R`** (o la tecla de grabar). Graban todas las pistas armadas; pulsa `R` o
   `Espacio` para parar. Toda la pasada es un solo paso de deshacer.

Las tomas se escriben como archivos WAV de 32 bits flotantes: en la carpeta `Audio` del
proyecto una vez guardado; antes, en `%LOCALAPPDATA%\Cantus\recordings\`; al guardar se
mueven al proyecto. Cada toma se adelanta la latencia medida, para que cuadre con lo que
oíste.

### Opciones de grabación

La ▾ junto a la tecla de grabar (o clic derecho sobre ella) abre las **Opciones de
grabación**:

![Opciones de grabación](../img/record-options.png)

- **Claqueta**: no, 1, 2 o 4 compases antes de la toma (`Alt+C`). La claqueta suena
  aunque el **Metrónomo** (`Ctrl+M`) esté apagado.
- **Pinchazo**: graba solo dentro de un tramo. *Desde la selección* toma la selección de
  tiempo; `Alt+P` usa la selección, si no el bucle, si no dos compases desde el cursor. El
  tramo aparece como una llave roja bajo la regla: arrastra sus extremos. Todo lo tocado
  se conserva en el archivo; el pinchazo solo fija la ventana del clip.
- **Tomas en bucle**: con el bucle activado, cada vuelta es una toma.
- **Modo de toma**: *Apilar* (los clips anteriores pasan a carriles de tomas — lo
  predeterminado), *Reemplazar* (corta lo que cubre la toma) o *Sobregrabar* (MIDI: añade
  notas al clip).
- El clic —su nivel, su sonido, su acento y cuándo suena— es del metrónomo, en la
  [barra superior](#la-barra-superior).

### Tomas y comping

Cada toma se guarda en un **carril de tomas** bajo la pista: ábrelos con el pequeño
desplegable al pie de la celda de color de la pista, `Alt+T` o *Mostrar tomas* del menú de
la pista. **Arrastra sobre una toma** para pasar ese tramo a la pista; haz clic en una toma
para usarla entera. Clic derecho en un carril: *Usar esta toma*, *Borrar tomas sin usar*
(que también quita las grabaciones que ya nada reproduce), *Separar tomas en pistas
nuevas* y *Ocultar tomas*.

### MIDI

Arma una **pista de instrumento** (una sin entrada de audio) y pulsa `R`: se graban en un
clip las notas de todos los teclados MIDI que Cantus encontró (la barra de estado dice
*Entradas MIDI: n*). Sin nada armado, `R` graba en la pista seleccionada. ¿Sin teclado?
`Ctrl+T` activa el **teclado del ordenador**: `A`–`L` tocan notas, `Z`/`X` cambian de
octava. Mientras está activo, los atajos de letra (incluida la `R`) se apagan — usa
`Espacio` o la tecla de grabar.

## 5. Editar: clips, piano roll, automatización

### Gestos rápidos

Como en FL Studio, el arreglo abre las cosas sin buscar en los menús:

- **Doble clic en la cabecera de una pista** (en cualquier sitio menos su nombre): se abre su
  **instrumento** — la ventana del instrumento (Unda, Pulsus II…) o la ventana propia de un
  plug-in CLAP/VST3. Firmus y las pistas de audio, que no tienen ventana, muestran el
  inspector.
- **Clic en el nombre de una pista** — sobre las letras mismas (el puntero se vuelve un
  cursor de texto) — para **renombrarla** allí mismo: el nombre queda seleccionado, escribe
  el nuevo, `Enter` lo guarda (un paso de deshacer), `Esc` devuelve el anterior. Un clic en
  otro lugar de la cabecera solo selecciona la pista, y escribir con una pista seleccionada
  nunca la renombra. (Los canales del mezclador y los pads del rack se renombran con doble
  clic.)
- **Doble clic en un clip**: se abre el **piano roll** en él, con el clip entero a la vista.
  Un patrón de pasos que el rack muestra sin perder nada — cada nota un golpe corto en un
  paso, un tono por canal (una batería puede usar todos sus pads; una melodía en un
  sintetizador se abre en el piano roll, como en FL) — se abre en el **rack de canales**,
  en la rejilla del propio patrón y desplazado hasta su pista. Un clip de automatización se
  abre en su editor grande; un clip de audio se abre en el **editor de afinación** (su
  ganancia, fundidos y archivo siguen en el inspector).
- **Abrir editor** — en el inspector, los menús del mezclador o el marco de un instrumento —
  trae una ventana ya abierta de la bandeja (o de la barra de tareas, si es de un
  instrumento o un efecto plug-in) y la pone delante; mientras la ventana de un plug-in está
  abierta, su botón dice *Mostrar*. `F8` y el botón de canal del rack hacen lo mismo cuando
  la ventana está minimizada, detrás de otra pestaña o bajo otra ventana, y solo la cierran
  cuando ya está delante. (Cantus no puede traer delante un plug-in que abre su propia
  ventana flotante: su botón dice *Cerrar*.)

### Los clips en el arreglo

- **Crear**: doble clic en un lugar vacío de una pista (un clip de 4 compases), o graba.
- **Seleccionar**: clic; `Ctrl+clic` añade; arrastra sobre el vacío para un recuadro;
  `Esc` limpia.
- **Mover**: arrastra (también a otra pista). `Alt+arrastrar` copia; `Ctrl` al arrastrar
  ignora la rejilla. `Izquierda`/`Derecha` desplazan según la rejilla, con `Shift` un
  compás.
- **Recortar**: arrastra el borde de un clip — su contenido se queda donde está.
  **Desplazar el contenido**: `Shift+arrastrar` lo mueve dentro del clip.
- **Cortar** en el cursor: `Ctrl+E`. **Unir**: `Ctrl+J`. **Duplicar**: `Ctrl+D` (púlsalo
  de nuevo para seguir en fila). **Silenciar**: `0`. **Renombrar**: `F2`. **Color**:
  `Ctrl+Shift+C`.
- **Selección de tiempo**: arrastra en el carril inferior de la regla; la franja de arriba
  es el bucle. `Shift+L` pone el bucle en la selección, `Z` la amplía.
- **Los clips de audio** tienen tiradores de fundido de entrada, de salida y de ganancia
  (doble clic en uno lo restablece; clic derecho en un fundido para su curva) y un menú con
  *Normalizar*, *Invertir*, *Restablecer ganancia* y *Quitar fundidos*.
- **Importar audio**: arrastra archivos WAV, AIFF, FLAC u Ogg al arreglo (caen donde
  sueltas), o *Archivo ▸ Importar audio…*. Un archivo MIDI añade sus pistas.
- **Copias enlazadas**: pegar, `Ctrl+D` o `Alt+arrastrar` crean *el mismo clip* otra vez —
  editas uno y todas las copias lo siguen (las notas, o la ganancia, los fundidos y la
  inversión de un clip de audio). Lo muestran una marca de enlace y la cuenta ×N. *Hacer
  único* (`Alt+U`) saca la selección de su grupo; *Seleccionar enlazados* (`Shift+Alt+L`);
  *Enlazar la selección a este clip* (`Alt+L`); *Enlazar a…* (`Alt+N`) elige, por nombre o
  por pista, el grupo enlazado o el clip propio en que se convierte. En el inspector, la
  fila *Enlace* de un clip enlazado dice cuántas copias tiene y ofrece *Enlazar a…*;
  renombrarlo en la fila *Nombre* renombra todas las copias. Clic derecho en un lugar vacío
  del arreglo ▸ *Las copias quedan enlazadas* para hacer copias independientes.
- **El navegador de clips** (`Alt+F5`, o *Ver ▸ Navegador de clips*) muestra una vez cada
  grupo enlazado y cada clip propio, como mosaicos del color del clip con una miniatura de
  lo que contiene; escribe para filtrar por nombre o pista. **Arrastra un mosaico** al
  arreglo para colocar ahí una copia enlazada. **Haz clic en un mosaico** para elegirlo para
  la **herramienta de pintar**: con el arreglo enfocado, `B` (o `2`) la saca — un clic
  coloca una copia enlazada, arrastrar pinta copias seguidas a lo largo de la pista,
  arrastrar con el botón derecho (o `Alt`+arrastrar) borra solo las copias de ese clip;
  cada trazo es un paso de deshacer; `Esc` la desactiva. Doble clic en un mosaico
  selecciona todas sus copias.

La **rejilla** (barra del arreglo) empieza en un compás; elige valores más finos para el
detalle. Desplázate con la rueda, `Shift+rueda` en el tiempo, `Ctrl+rueda` para el zoom.

### El piano roll

Selecciona un clip MIDI y muestra el piano roll (`F7`, o su pestaña bajo el arreglo):
edita las notas de ese clip.

![El piano roll](../img/piano-roll.png)

| Herramienta | Tecla | Qué hace |
|---|---|---|
| Dibujar | `P` o `1` | Clic añade una nota, arrastrar mueve, arrastrar el borde derecho cambia la duración, clic derecho borra |
| Pintar | `B` o `2` | Arrastra sobre la rejilla para pintar notas |
| Seleccionar | `E` o `3` | Arrastra para seleccionar; arrastra notas para moverlas |
| Borrar | `D` o `4` | Clic o arrastre sobre notas para borrarlas |
| Silenciar | `T` o `5` | Clic en una nota para silenciarla (se queda, sin sonar) |
| Cortar | `C` o `6` | Arrastra una línea para dividir notas |
| Escuchar | `Y` o `7` | Clic en notas o filas para oírlas |

`Arriba`/`Abajo` transportan la selección (`Shift`: una octava), `Izquierda`/`Derecha` la
desplazan, `Q` cuantiza, `Ctrl+C`/`Ctrl+V` copian y pegan. Las velocidades son tallos en
el carril bajo la rejilla: arrástralos, o `Alt+rueda` sobre una nota. Una batería muestra
una fila con nombre por pad.

El **rack de canales** (`F6`) muestra cada pista como una fila de pasos sobre los compases
del clip seleccionado: clic enciende un paso, clic derecho lo apaga, arrastrar pinta. Un
paso es una nota normal.

### El editor de afinación

Doble clic en un clip de audio — o su menú ▸ *Editar afinación*, o *Editar afinación* en el
inspector — y su toma se abre en el lugar del piano roll, a la manera de Melodyne: cada
**nota** es una mancha en la fila de la nota que suena, tan gruesa como fuerte es la voz,
con la **curva de afinación** atravesándola (lo que se cantó, tenue, bajo lo que sonará).
Las sibilantes y las respiraciones se marcan abajo. La toma se analiza junto a la ventana
la primera vez (un segundo o así por minuto de audio; una toma grabada se analiza en
cuanto llega) y el resultado se guarda en la caché de Cantus, así que la próxima vez se
abre al instante. El archivo nunca cambia: el clip suena con un render de tus ediciones,
hecho junto a la ventana, y mientras un render está en camino se oye la toma tal como fue
(nunca silencio). La exportación espera a todos los renders.

- **Herramienta Editar**: haz clic en una nota para seleccionarla (`Mayús` añade; arrastra
  en el fondo para varias). Arrástrala **arriba o abajo** para mover su afinación — a las
  notas de la tonalidad (*Ajuste: Escala*), a semitonos o a cualquier punto (*Libre*;
  mantén `Alt` para cents). Arrástrala **a los lados** para moverla en el tiempo, o
  arrastra uno de sus **extremos** para estirarla: solo ceden los huecos a su lado, sus
  vecinas quedan exactamente como se cantaron. **Doble clic** en una nota lleva su centro a
  su nota (otra vez para soltarlo). `Arriba`/`Abajo` mueven la selección un paso (`Mayús`:
  una octava, `Alt`: 10 cents), `Ctrl+A` selecciona todas, `Ctrl+J` une, `Q` cuantiza,
  `Esc` deselecciona.
- **Herramienta Dividir**: haz clic dentro de una nota para cortarla en dos; **Unir** junta
  vecinas.
- **La nota** (el panel de abajo): lo que la selección *suena* y lo que se *cantó*, y su
  **centro** (cuánto se lleva su centro a su nota, conservando su deriva y su vibrato
  naturales), **deriva** (el vaivén lento: 100 % como se cantó, 0 % plano), **vibrato**
  (0–200 %), **formante** (el color de la voz, arriba o abajo, sin mover la afinación) y
  **ganancia**.
- **Las macros** actúan sobre la selección, o sobre todas las notas si no hay ninguna
  seleccionada: **Corregir afinación** según una cantidad (una nota fuera de la tonalidad va
  a su nota más cercana), **Corregir deriva** según una cantidad, y **Cuantizar** el inicio
  de cada nota a la rejilla de la barra.
- **Tonalidad**: hallada a partir de las notas; cámbiala en la barra (las escalas de
  Tonus). **Original** compara con la toma tal como se cantó (A/B) — el inspector tiene el
  mismo interruptor. **Restablecer** deshace todas las ediciones de la selección. **▶**
  reproduce desde la primera nota seleccionada. Cada edición es un paso de deshacer.
- **MIDI**: arrástralo a una pista de instrumento del arreglo y la melodía cae allí, en el
  lugar de la voz; haz clic para una pista nueva. **Doblar**: una segunda voz una tercera o
  una quinta por encima en la tonalidad, de la misma toma, en su propia pista.

Los formantes se conservan al mover una nota (medido: dentro de 3 dB a ±3 semitonos).
Trabaja sobre una voz o una línea de instrumento a la vez; los acordes no se separan en
notas.

### Automatización

Clic derecho en cualquier perilla, fader o control de un efecto ▸ **Crear clip de
automatización**, o mueve un control y pulsa **`Alt+A`** (automatiza lo último que
tocaste). El clip cubre la selección de tiempo (si no, el bucle; si no, cuatro compases)
en un carril bajo su pista. En el carril: arrastra los puntos, arrastra el anillo entre
dos puntos para curvar, clic en la línea añade un punto, doble clic en un punto lo borra,
clic derecho para su forma (curva simple o doble, mantener, escalera, pulso, onda). Doble
clic en el clip abre el editor grande, con herramienta de dibujo y articuladores
(invertir, suavizar, azar, escala, LFO). Un control automatizado muestra un punto.

## 6. Mezclar: el mezclador, inserciones y envíos

![El mezclador](../img/mixer.png)

Cada pista alimenta un **canal** del mezclador (`F9`); los canales van al **máster**, a la
izquierda, o a otro canal (un bus). Un canal tiene su medidor, silencio y solo, paneo,
fader, fase e intercambio, anchura, EQ de tres bandas, sus inserciones y sus envíos. Clic
selecciona, `Ctrl+clic` añade, `Shift+clic` toma un tramo; mover un fader de una selección
mueve todos. Con el foco en el mezclador, `M` silencia, `S` pone solo,
`Alt+Izquierda`/`Alt+Derecha` mueven el canal. `Ctrl+rueda` cambia el ancho de los canales;
en *Extra grande* cada canal muestra un resumen de su cadena en una línea (un clic abre el
inspector).

- **Inserciones** (hasta diez por canal): clic en una ranura vacía para elegir un efecto
  en el selector, o clic derecho para la lista rápida. En una ranura ocupada, clic en la
  luz la puentea y en el nombre abre su ventana; clic derecho para cambiarla, moverla o
  quitarla. En el inspector, cada inserción tiene un anillo de **Mezcla** (seco/procesado).
- **Envíos**: selecciona un canal y mira la fila *Envíos* de los demás — haz clic en el ▲
  del canal al que quieres enviar (un retorno de reverb, un compresor en paralelo); una
  perilla fija entonces el nivel del envío. Cada envío toma la señal **después del fader
  (POST, por defecto) o antes (PRE)** — clic en su chip PRE/POST; un envío PRE sigue sonando
  al bajar el fader.
- **Cables**: cada canal tiene un puerto. Arrastra desde él a otro canal, o haz clic en él y
  luego en el destino (`Esc` cancela): sin tecla es un envío a 0 dB, con `Ctrl` la única
  salida del canal, con `Alt` una llave de sidechain. Los cables del canal seleccionado (o
  todos, o ninguno — el interruptor sobre la mesa, con un filtro de tipos) se dibujan en
  una banda bajo la mesa; clic en un cable lo selecciona, arrastrar su extremo lo mueve,
  `Supr` lo quita. Un camino doble se permite y se marca. Cada cambio es un paso de deshacer.
- **Ruteo**: clic derecho en un canal ▸ *Enrutado* ▸ *Salida* o *Enviar a*, `Shift+F10`
  para el menú de ruta del canal seleccionado, o selecciona varios canales y *Crear
  submezcla con los seleccionados*. En el menú de ruta de un cable o un envío, *Nivel ▸
  Escribir un nivel…* toma un nivel en dB (de −∞ a 0; `Enter` o un clic fuera lo aplica,
  en un paso de deshacer; `Esc` cancela). **La matriz de ruteo** (`Alt+F9`) muestra cada origen
  frente a cada destino: clic en una celda para enrutar, arrastrar para el nivel (`Shift`
  para afinar), `Ctrl+arrastrar` copia hacia abajo en una columna. Los canales y grupos se
  mueven arrastrando o con `Alt+Izquierda`/`Alt+Derecha`; una cabecera de pista soltada
  sobre un canal suena por él.
- **Las ventanas de los dispositivos** tienen *Cambiar*, minimizar (a la bandeja de la
  barra de estado), maximizar y cerrar. `F8` abre el instrumento de la pista seleccionada
  (o lo trae delante, o lo cierra si ya está delante),
  `Esc` cierra el último dispositivo que usaste y `F12` cierra todos los flotantes.

**El solo** sigue el ruteo: un canal en solo se oye con lo que lo alimenta y aquello a lo
que alimenta.

## 7. Masterizar: la cadena del máster, Fervor, Latitudo, Culmen

El **máster** tiene su propia cadena de hasta diez inserciones, aplicada a toda la mezcla
antes del fader del máster. Haz clic en la cabecera del máster para ver su cadena en el
inspector (*La mezcla completa* → las inserciones → *Salida de audio*), o clic derecho en
su nombre ▸ *Efectos* ▸ *+ Añadir efecto*.

![Culmen en el máster](../img/master.png)

Las demos masterizan en este orden, y es un buen comienzo:

1. **Aequalis** (EQ): un pasa-altos bajo el sub, un poco de aire.
2. **Multiplex** o **Comprimo**: un pegamento suave.
3. **Latitudo** (estéreo): el preset *Máster: graves más firmes* o *Máster: agudos más
   anchos, graves en mono* deja los graves en mono y los agudos anchos. Vigila su medidor
   de correlación: debe quedar por encima de cero.
4. **Culmen** (maximizador): elige un **objetivo** — *Streaming −14* (−14 LUFS, −1 dBTP),
   *Apple −16*, *CD −9*, *Club −7*, *Emisión −23* — reproduce la parte más fuerte de la
   canción y haz clic en **Aprender empuje**: Culmen escucha, ajusta el empuje, vuelve a
   escuchar y se detiene en el objetivo, en un solo paso de deshacer. La tarjeta
   *Sonoridad* muestra la sonoridad momentánea, de corto plazo e integrada y el pico real;
   el veredicto dice *En el objetivo* o cuántos LU sobran o faltan. Culmen trabaja en tres
   bandas más la banda completa, hacia un muro de pico real en el techo; su *Carácter*
   (Limpio, Pegada, Fuerte, Bus, Seguro) y sus diez presets cubren casi cualquier máster.

**Fervor** (saturación) añade color: *Cinta*, *Válvula*, *Overdrive*, *Fuzz*, *Recorte*,
*Pliegue* o una *Curva* dibujada, con sobremuestreo, tono, mezcla e *Igualar nivel*, para
que lo más fuerte nunca te engañe. En el máster, los presets *Recorte de máster* y
*Válvula en el medio* son los suaves.

## 8. Exportar

*Archivo ▸ Exportar audio…* (`Ctrl+Shift+E` con las teclas por defecto) abre la ventana de
exportación: los presets a la izquierda, las opciones a la derecha y una línea que dice
exactamente qué se va a escribir.

- **Presets**: *Streaming* (MP3 320 kbps, 44.1 kHz, −14 LUFS, −1 dBTP), *CD* (WAV 16 bits,
  44.1 kHz, dither con modelado), *Stems para masterizar* (cada canal, WAV 24 bits a la
  frecuencia de la sesión, sin normalizar) y *Podcast* (MP3 mono 128 kbps, −16 LUFS,
  −1 dBTP). Si cambias cualquier opción se ilumina la tarjeta *Personalizado*.
- **Qué**: la canción (del inicio al final del último clip, más la cola que elijas), la
  selección de tiempo, o **stems** — un archivo por canal del mezclador (después de su
  fader) o por pista (antes de su canal), todos desde el primer tiempo de la canción con la
  compensación de latencia incluida, y opcionalmente *a través de la cadena del máster*.
- **Formatos**: **WAV** de 16, 24 bits o 32 bits flotantes; **FLAC** de 16 o 24 bits; **MP3**
  CBR de 128 a 320 kbps o VBR V0/V2/V4/V6 (LAME), sin huecos; **Ogg Vorbis** calidad 3–10.
  Frecuencia: la de la sesión o 44.1, 48, 88.2, 96, 192 kHz (MP3 usa 44.1 o 48). Mono o
  estéreo.
- **Dither** (apagado, TPDF, con modelado) al escribir 16 o 24 bits.
- **Loudness**: apagado, o un objetivo (−9, −11, −14, −16, −19, −23 LUFS) con un techo de
  pico real (−0.1, −0.3, −1, −2 dBTP); el limitador de Culmen sostiene el techo cuando la
  ganancia lo necesita. Los stems nunca se normalizan.
- **Etiquetas**: título, artista, álbum, año, comentario y el crédito de codificador
  *Cantus*.

La exportación corre en segundo plano con una barra de progreso; *Detener* no deja archivos
a medias. Se recuerdan tus últimas opciones.

- **MIDI**: *Archivo ▸ Exportar MIDI…* — la canción entera, la pista seleccionada o el clip
  seleccionado, como archivo de tipo 1 con el tempo, el compás y los nombres de pista.
- **DAWproject**: *Archivo ▸ Exportar DAWproject…* para llevar un proyecto a Bitwig, Studio
  One y otros DAW que lo leen (un informe dice lo que no pudo viajar).

## 9. Sonidos: el explorador y los paquetes de sonidos

El **explorador** (`Alt+F8`, la primera tecla de ventana) se acopla a la izquierda. Lista
los paquetes de sonidos que instalaste y *Mis carpetas* (haz clic en **+** para añadir
cualquier carpeta de muestras).

![El explorador de muestras](../img/browser.png)

- **Clic** en un sonido para oírlo; otro clic lo detiene. Escribe en el campo de búsqueda
  para encontrar sonidos en todos los paquetes y carpetas.
- **Arrastra** un sonido a un **pad de Pulsus** (el pad lo toca), al mapa de teclas de
  **Exemplar** (una zona en esa tecla), al **rack de canales** (un canal de muestra nuevo
  con su propio canal del mezclador) o al **arreglo** (un clip de audio donde lo sueltas).
  Cada arrastre es un solo paso de deshacer.
- Sin nada instalado, el explorador ofrece **Empieza con una batería**: *Descargar* trae la
  *Batería inicial de Cantus* (81 golpes, 33 MB). *Paquetes de sonidos…* (o *Más
  paquetes…*) lista los demás — las baterías de Karoryfer, la percusión de VCSL y VSCO 2,
  un TX81Z para Exemplar y el **piano de cola Salamander** (un Yamaha C5, 16 capas de
  velocidad) — con su tamaño, licencia y créditos; *Instalar*, *Actualizar*, *Quitar*,
  mostrar la carpeta e *Instalar desde un archivo…* para equipos sin internet.

Todos los paquetes son gratuitos. Casi todos son **CC0** (sin tener que citar); el piano
Salamander es **CC BY 3.0**: su crédito aparece en su página de *Paquetes de sonidos…* y en
*Ayuda ▸ Créditos*, y se guarda en la carpeta del paquete. En los dos casos, la música que
haces con ellos es tuya. Se guardan en `%LOCALAPPDATA%\Cantus\Content\`; cada archivo se
verifica con su SHA-256.

**Instrumentos de un paquete.** Un paquete instalado que trae instrumentos (el Salamander:
con 8 capas de velocidad, unos 120 MB de memoria, o con las 16, unos 185 MB) los muestra en
el estante *Teclados* del selector de dispositivos y en su página de *Paquetes de sonidos…*
(*Pista nueva con …*); cada uno suena en Exemplar. Las muestras largas se **leen del
disco**: en memoria queda solo su comienzo. La primera vez, Cantus descodifica las muestras
una sola vez en su **caché de muestras** (1 GB para las 8 capas del Salamander, 2 GB para
16), junto a la ventana, con *Cargando muestras… n de m* en la barra de estado; las cargas
siguientes tardan una fracción de segundo. Si el disco no llega a tiempo, la barra de estado
cuenta *Disco lento: n*.

**Un paquete que falta.** Un proyecto que toca un paquete que este equipo no tiene abre una
tarjeta con el paquete y sus pistas en silencio: *Instalar en Paquetes de sonidos…*, *Usar
el paquete instalado* (cuando el proyecto apunta a otra copia, por ejemplo de otro equipo)
o *Buscar la carpeta…* (cuando se movieron archivos de un paquete instalado). El resto del
proyecto suena con normalidad.

## 10. Instrumentos y efectos: el selector de dispositivos

El **selector de dispositivos** muestra todos los instrumentos y efectos — los de Cantus y
los plug-ins CLAP y VST3 instalados en tu equipo — como mosaicos.

![El selector de dispositivos](../img/picker.png)

Ábrelo con el **+** sobre las cabeceras de pista (una pista de instrumento nueva), con un
clic en el chip de instrumento de una pista (cambiar el instrumento), con *Cambiar* en
cualquier ventana de dispositivo, con una ranura de inserción vacía o con *Ver ▸ Selector
de dispositivos…*. Busca por nombre, fabricante o tipo; filtra por estante (*Favoritos*,
*Recientes*, una categoría, o *Cantus / CLAP / VST3*); las flechas se mueven, `Enter`
elige, `Ctrl+D` marca un favorito, `Esc` cierra. Los instrumentos de los paquetes
instalados (el piano Salamander) están en *Teclados*. Cambiar un instrumento conserva sus
notas, clips, canal del mezclador y efectos; cambiar un efecto conserva su ranura, su
mezcla y su puenteo.

| Instrumento | Qué es |
|---|---|
| Firmus | El sintetizador sencillo con el que empieza cada pista nueva |
| Pulsus II | Caja de ritmos: tambores sintetizados y pads de muestras, grupos de corte |
| Unda | Sintetizador wavetable: tres osciladores, unísono, dos filtros, LFO dibujados |
| Calor | Polisintetizador analógico |
| Modulus | Sintetizador FM de ocho operadores; lee bancos DX7 |
| Exemplar | Sampler: SFZ, Decent Sampler y SoundFont |
| Gravis | Bajo: 808, sintetizado y eléctrico modelado |
| Lamina | Piano eléctrico |
| Organum | Órgano de ruedas fónicas con barras y altavoz rotatorio |

Los efectos son Aequalis (EQ), Comprimo (compresor), Porta (puerta y expansor), Sibilus
(de-esser), Multiplex (dinámica multibanda), Terminus (limitador), Culmen (maximizador),
Spatium (reverb), Fervor (saturación), Latitudo (estéreo), Delay y Reverb.

### Los efectos de modulación

Ocho efectos mueven el sonido en el tiempo: **Concentus** (chorus), **Flexus** (flanger),
**Phasis** (phaser), **Tremor** (trémolo y autopaneo), **Vibrans** (vibrato), **Anulus**
(modulador en anillo), **Translatio** (desplazador de frecuencia) y **Fluctus** (modelador
de volumen). Cada uno se inserta **sin hacer nada** — su *Mezcla* o su *Profundidad* empieza
en 0 y el sonido pasa intacto —, así que sube ese control o elige un preset de fábrica en la
lista del marco.

| Efecto | Qué hace | Conviene saber |
|---|---|---|
| Concentus | Engorda y ensancha | *Juno I, II, I+II* son el chorus del Juno-60 tal como barre su circuito; *Dimension 1–4* ensanchan sin vaivén; *Voces libres* son de 1 a 4 voces con tu propio LFO. La **prueba en mono** dice si la anchura sobrevive a un altavoz mono. |
| Flexus | El peine del avión a reacción | **Por cero** retrasa el sonido seco 5 ms (el pie lo dice y Cantus lo compensa) para que la copia en movimiento lo atraviese, como hacían dos magnetófonos. El barrido sigue al LFO, a lo fuerte que tocas (*Envolvente*) o a una perilla (*Manual*). *El peine ahora* dibuja las muescas que oyes. |
| Phasis | Muescas que barren | De 2 a 12 etapas (una muesca por cada dos), realimentación en ambos sentidos; el Phase 90 y el Small Stone son presets. |
| Tremor | Nivel o panorama a tempo | *Trémolo*, *Autopaneo* (de igual potencia) y *Armónico* (graves contra agudos, como los amplis marrones). Las formas cuadrada y de sierra tienen un **borde** suave: nunca hacen clic. |
| Vibrans | Una afinación que ondula | La profundidad va en **cents**, sea cual sea la velocidad; la **Entrada** deja que el vibrato llegue después de cada nota, como el de un cantante. |
| Anulus | Campanas, metal, robots | La portadora se muestra como nota con sus cents; *En qué se convierte un tono de 1 kHz* dibuja los parciales que crea. |
| Translatio | Mueve cada parcial los mismos hercios | No es un transpositor: los armónicos dejan de serlo. *Arriba*, *Abajo* o *Ambas* (una por lado); la realimentación hace las espirales de barbería. Su otra banda lateral queda más de 60 dB por debajo de 20 Hz a 20 kHz. |
| Fluctus | Bombeo de sidechain sin clave | Dibuja el nivel en un ciclo (una figura de 4/1 a 1/16): arrastra puntos (Mayús: fuera de la rejilla), doble clic para añadir o quitar uno, arrastra un tramo para curvarlo; las formas de los tramos son las del editor de automatización. |

**Al tempo de la canción.** Cada LFO puede seguir a la canción (*Al tempo*, una figura con
puntillos y tresillos, swing donde tiene sentido) o ir libre en Hz. Un LFO al tempo está
atado a la posición de la canción, no al momento en que pulsaste Reproducir: empieza en el
compás 5 y está donde habría estado; un bucle que vuelve lo devuelve a su sitio; la
exportación suena exactamente como la reproducción. La pantalla de cada efecto dibuja lo que
hará el LFO en los dos ciclos siguientes, con un cabezal donde está ahora.

**En otro DAW.** Las versiones CLAP y VST3 exportadas leen el tempo y la posición del
anfitrión, así que un Tremor o un Fluctus al tempo siguen también la canción de ese DAW.

**Tus plug-ins.** Cantus revisa las carpetas estándar — `%LOCALAPPDATA%\Programs\Common\CLAP`
y `…\VST3`, `C:\Program Files\Common Files\CLAP` y `…\VST3` — y las que añadas en *Ajustes
▸ Plug-ins* (*Buscar plug-ins*, *Volver a leer todo*). Cada archivo se revisa en un
proceso aparte, así que un plug-in que falla al revisarse queda bloqueado, sin tumbar
Cantus. Los efectos de Cantus se instalan también en tus carpetas CLAP y VST3, para otros
DAW.

### Tonus: corrección de afinación para la voz

**Tonus** (en el estante *Afinación · voz* del selector) afina una voz cantada mientras entra:
para el efecto duro, a saltos; para una corrección natural; o para escucharte mientras
grabas (añade 2,7 ms, que la compensación de retardo tiene en cuenta).

1. Pon Tonus en la pista de la voz. Elige la **tónica** y la **escala**; en el teclado,
   haz clic en una nota para quitarla (×) o devolverla (●), y clic derecho para dejarla
   sin corregir (○).
2. **Velocidad**: 0 ms (*Dura*) salta de nota en nota; 50–100 ms corrige y conserva la
   vida de la voz. **Cantidad** fija cuánto se aplica.
3. Para afinar a lo que tocas: **Notas MIDI** (las notas que suenan en una pista, en
   cualquier octava) o **Melodía MIDI** (la voz canta exactamente la última nota); elige
   la pista bajo el selector: cuentan sus clips y lo que tocas en ella en vivo.
4. **Más** guarda humanizar (las notas largas conservan su movimiento), margen (las
   desviaciones pequeñas no se tocan), vibrato (el del cantante, quitado o doblado; o uno
   creado), los formantes y la **garganta** (más brillante o más oscura sin cambiar la
   altura), la transposición y el La4 de referencia.

La pantalla muestra lo que cantaste (tenue) y lo que sale (brillante), y la nota que
cantas con su distancia en cents. Preajustes: *Natural*, *Ajustada*, *Afinación dura*,
*Garganta arriba*, *Garganta abajo*, *Melodía MIDI* y más. Detalles:
`docs/plugins/tonus.md`.

### Dinámica y análisis: Tonsor, Impetus, Discrimen, Speculum

Cuatro efectos moldean los picos y los ataques de un sonido y muestran lo que hay en él.
Como los demás, se insertan **sin hacer nada**: uno recién puesto no cambia ninguna muestra
hasta que lo tocas.

| Efecto | Qué hace | Conviene saber |
|---|---|---|
| Tonsor | Un clipper: recorta los picos | **Techo**, **Empuje**, una **Rodilla** y tres hombros (*Dura*, *Suave*, *Analógica*). El **sobremuestreo** hasta 16× evita que los armónicos que crea se plieguen hacia abajo; **True peak** mantiene el techo también entre las muestras (cerca de 1 ms de anticipación), así que el −1 dBTP de un servicio de streaming es de verdad −1. **Igualar nivel** quita el empuje después de la curva, para que lo más fuerte nunca gane. El medidor dice cuánto (la reducción más honda), cuán a menudo (la parte de las muestras recortadas en el último segundo), los sobrepicos entre muestras de la entrada y el pico real de la salida. |
| Impetus | Más o menos ataque y sostenido | Trabaja con **diferencias de nivel, no con un umbral**: un golpe suave y uno fuerte se moldean igual. **Rapidez** es lo que dura un ataque; **Duración**, cuánto cuenta una cola como tal. *Agudos* o *Graves* moldean solo un lado de un cruce (chasquido sin que bombeen los graves, o el golpe de un bombo); un **sidechain** deja que los golpes de otra pista lo muevan. *Un golpe a través* dibuja lo que hace antes de tocar. |
| Discrimen | Separa cada golpe en su **transitorio** y su **cuerpo** | Cada parte tiene su **ganancia**, un **compresor** de una perilla y una **inclinación**; *Escuchar* toca una parte sola. Sin tocarlas, las dos suman la entrada exactamente. *Sin latencia* no cuesta nada; *Anticipación* (2 ms, compensados) atrapa cada ataque desde su primera muestra. |
| Speculum | Un analizador que nunca toca el sonido | Pestañas: *Resumen*, *Espectro* (promedio, pendiente, retención de picos, medio/lados/izquierda/derecha), *Espectrograma*, *Osciloscopio* (libre, con disparo o **en el pulso**), *Vectorial* (y la **correlación**: por debajo de 0 hay problemas en mono), *Sonoridad* (LUFS momentánea, a corto plazo e integrada, rango, pico real, frente a un objetivo: las lecturas de EBU R128). ⋯ ▸ **Acoplar en el espacio de trabajo** lo vuelve una pestaña que puedes redimensionar, dividir o sacar como ventana; acopla varios y míralos a la vez. |

Páginas: `docs/plugins/tonsor.md`, `impetus.md`, `discrimen.md`, `speculum.md`.

## 11. Proyectos, autoguardado y ajustes

- *Archivo ▸ Nuevo proyecto* (`Ctrl+N`), *Abrir…* (`Ctrl+O`), *Guardar* (`Ctrl+S`),
  *Guardar como…* (`Ctrl+Shift+S`). Un proyecto es un archivo `.cantus`; al guardar, el
  anterior queda como `.cantus.bak`.
- **Los archivos de audio** se reúnen al guardar: las grabaciones pasan a la carpeta
  `Audio` del proyecto y los archivos importados se copian ahí, así que la carpeta viaja
  completa. *Reunir audio y guardar* lo hace cuando quieras. Un archivo que falta muestra
  **FALTA** en su clip; clic derecho ▸ *Buscar archivo…*.
- **El autoguardado** escribe una copia de recuperación cada minuto si algo cambió
  (*Ajustes ▸ Guardado*, de 10 s a 10 min): `<proyecto>.cantus.autosave` junto a un
  proyecto guardado, o en `%LOCALAPPDATA%\Cantus\autosave\` para uno sin guardar. Nunca
  sobrescribe tu archivo, y escribirla nunca congela la ventana (se hace en segundo plano).
- **Recupera tu trabajo**: si Cantus se cierra de forma inesperada —o lo cierras sin
  guardar—, el siguiente inicio te ofrece lo que quedó: los cambios autoguardados y las tomas
  grabadas que no están en ningún archivo guardado. *Restaurar* los devuelve como un solo
  paso de deshacer (`Ctrl+Z` muestra el último guardado); *Descartar* no borra nada. *Nuevo*
  y *Abrir* preguntan antes de reemplazar trabajo sin guardar.

**Ajustes** (*Ver ▸ Ajustes…*): *General* (el versículo al iniciar, sí o no, y la Biblia en
español), *Audio* (controlador, dispositivos, frecuencia, búfer, latencia y la corrección
de grabación), *MIDI*, *Apariencia* (idioma — English o Español —, tamaño de la interfaz,
colores, atajos de teclado), *Guardado*, *Caché de muestras* (su tamaño máximo — 16 GB
de forma predeterminada; cuando se llena, salen primero las muestras que llevan más tiempo
sin usarse — y *Vaciar caché*; vive en `%LOCALAPPDATA%\Cantus\cache\samples`, o en la
carpeta `data` de una copia portátil), *Actualizaciones* y *Plug-ins*.

## 12. Atajos de teclado

*Ayuda ▸ Atajos de teclado* lista todos los atajos y permite cambiarlos, o cargar los
presets de FL Studio, Ableton Live o Bitwig Studio. Los predeterminados:

| Teclas | Acción |
|---|---|
| `Espacio` | Reproducir / pausar (también detiene una grabación) |
| `R` | Grabar |
| `Inicio` | Ir al principio |
| `Ctrl+L` · `Ctrl+M` | Bucle sí/no · metrónomo sí/no |
| `L` · `Shift+T` | Modo patrón / canción · marcar el tempo |
| `Alt+R` · `Alt+C` · `Alt+P` · `Alt+T` | Armar la pista seleccionada · claqueta · pinchazo · mostrar tomas |
| `Ctrl+Shift+A` | Añadir una pista de audio |
| `Ctrl+N` · `Ctrl+O` · `Ctrl+S` · `Ctrl+Shift+S` | Nuevo · abrir · guardar · guardar como |
| `Ctrl+Shift+N` | Canciones de demostración |
| `Ctrl+Shift+E` | Exportar audio |
| `Ctrl+Z` · `Ctrl+Y` | Deshacer · rehacer |
| `Ctrl+X` · `Ctrl+C` · `Ctrl+V` · `Ctrl+D` | Cortar · copiar · pegar · duplicar |
| `Ctrl+A` · `Supr` · `Esc` | Seleccionar todo · eliminar · vaciar la selección |
| `Ctrl+E` · `Ctrl+J` · `0` · `F2` | Cortar el clip · unir · silenciar el clip · renombrar |
| `Izquierda` `Derecha` (`Shift`: un compás) | Desplazar |
| `Arriba` `Abajo` (`Shift`: una octava) | Transportar notas |
| `Q` | Cuantizar |
| `P` `B` `E` `D` `T` `C` `Y` o `1`–`7` | Herramientas del piano roll; en el arreglo, `B` o `2` es la herramienta de pintar |
| `Shift+L` · `Z` | Bucle en la selección de tiempo · ampliarla |
| `Alt+A` | Automatizar el último parámetro que moviste |
| `F5` `F6` `F7` `F9` · `Alt+F8` · `I` | Arreglo, rack de canales, piano roll, mezclador · explorador · inspector |
| `Alt+F5` | Navegador de clips |
| `F8` · `F12` | El instrumento de la pista seleccionada · cerrar los dispositivos flotantes |
| `M` · `S` · `Alt+Izquierda`/`Alt+Derecha` | Mezclador: silenciar · solo · mover el canal |
| `Alt+F9` · `Shift+F10` | La matriz de ruteo · el menú de ruta del canal seleccionado |
| `Alt+U` · `Alt+L` · `Shift+Alt+L` · `Alt+N` | Hacer único · enlazar la selección a este clip · seleccionar enlazados · enlazar a… |
| `Ctrl+T` | El teclado del ordenador toca notas |
| `Ctrl++` · `Ctrl+-` · `Ctrl+0` | Zoom de la interfaz |

## 13. Privacidad y actualizaciones

Cantus **no tiene telemetría**. Una copia instalada o portátil pregunta a GitHub una vez al
día si hay una versión nueva — una petición anónima, sin versión, equipo ni datos tuyos —
y te pregunta antes de descargar nada; *Ajustes ▸ Actualizaciones ▸ Buscar a diario* la
apaga. Los paquetes de sonidos se descargan solo cuando haces clic en *Instalar*. Todo lo
demás se queda en tu equipo. Los detalles están en [PRIVACY.md](../../../PRIVACY.md); las
licencias de todo aquello con lo que está hecho Cantus, en
[THIRD-PARTY-NOTICES.md](../../../THIRD-PARTY-NOTICES.md) (también en *Ayuda ▸ Acerca de
Cantus*).

## 14. Problemas conocidos y limitaciones

Esto es una beta. Lo que ya sabemos:

- **Un plug-in de terceros que falla cierra Cantus** (los plug-ins se ejecutan dentro del
  programa; aislarlos viene después). Guarda seguido.
- **No se pueden importar MP3** (sí WAV, AIFF, FLAC y Ogg).
- **Sin estiramiento de tiempo**: los clips de audio no siguen los cambios de tempo; un
  tempo y un compás por proyecto. Sin fundidos cruzados ni envolventes de ganancia por
  clip.
- Los parámetros de los plug-ins de terceros aún no se pueden automatizar; la
  automatización se dibuja, no se graba (sin latch ni touch). En los stems exportados *a
  través de la cadena del máster*, un efecto de terceros en el máster no entra en los
  stems (la mezcla completa sí lo lleva).
- Cortar un clip **en bucle** silencia su mitad derecha: en su lugar, selecciona el rango
  de tiempo que necesitas, cópialo y pégalo (la parte pegada conserva el bucle).
- Los clips de automatización solo se pegan dentro del proyecto del que vienen, mientras
  Cantus siga abierto; un primer guardado o un *Guardar como* que copia el audio importado
  a la carpeta del proyecto puede detener la ventana mientras se copian los archivos; Ctrl+V sin
  nada copiado no hace nada ni lo dice.
- **Lamina** suena un poco más bajo que los demás instrumentos (unos 4 dB) con el mismo
  fader, y el **piano Salamander** unos 5 dB más bajo.
- La **primera carga del Salamander** descodifica sus muestras en la caché de muestras
  (1–2 GB en disco; unos 10 s), junto a la ventana; una nota tocada antes de que lleguen
  sus muestras no suena. Las cargas siguientes son rápidas.
- Los nombres en otras escrituras usan las fuentes de tu sistema (se leen cuando hacen
  falta; Cantus no las distribuye). **El árabe y el hebreo se ven de izquierda a derecha y
  con las letras sin unir**, y las escrituras que unen o reordenan letras (devanagari,
  tailandés…) se muestran sin hacerlo: la biblioteca de la interfaz aún no da forma al
  texto.
- **Audio**: el controlador ASIO integrado de Realtek no da sonido en algunas laptops (usa
  WASAPI o FlexASIO); ASIO4ALL puede tardar varios segundos en arrancar; muchos
  controladores ASIO fijan el búfer en su propio panel; conecta la interfaz *antes* de
  abrir Cantus. No se ofrece WASAPI exclusivo.
- **Rendimiento**: en equipos modestos, un proyecto denso con búfer pequeño puede cortarse;
  sube el búfer si la barra de estado cuenta cortes.
- No se puede cambiar el dispositivo de audio mientras grabas; el explorador aún no se navega con el teclado; los proyectos
  guardan las muestras de los paquetes con su ruta completa.
- **Fuera de esta beta**: macOS y Linux, warping, MIDI learn y superficies de control,
  funciones de IA, varios arreglos por proyecto.

## 15. Reportar un error

*Ayuda ▸ Reportar un error…* abre el formulario de errores en GitHub (necesitas una cuenta
gratuita de GitHub). Di qué hiciste, qué esperabas y qué pasó; incluye tu versión de
Cantus (*Ayuda ▸ Acerca de Cantus*), tu versión de Windows, tu interfaz de audio, el
controlador, la frecuencia y el búfer (todo está en el chip de audio); adjunta un proyecto
pequeño que muestre el problema si puedes. Cuéntanos de tu interfaz de audio aunque
funcione, con el [reporte de hardware](https://github.com/luis314159/cantus/issues/new?template=hardware_report.yml),
y manda ideas con *Ayuda ▸ Sugerir una función…*. En español o en inglés, como prefieras.

*Cantus lo hace Luis Ángel Almazán — [lalmazan.com](https://lalmazan.com) — y es software
libre bajo la GPL de GNU, versión 3 o posterior.*
