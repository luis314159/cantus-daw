# Tympanum, el baterista acústico

*Cantus 0.1.0-beta.15 · [Read in English](../en/tympanum.md) · volver al [manual](README.md)*

Tympanum es la batería acústica de Cantus: un kit de pie en su sala, oído por sus micrófonos, y
cada micrófono en un canal del mezclador si lo quieres ahí. Toca dos clases de kit. Un kit
**modelado** se calcula mientras tocas — nada que descargar, y sus micrófonos virtuales se
pueden mover. Un kit **grabado** es un paquete de muestras multimicrófono libre (DRSKit,
CrocellKit, Virtuosity Drums, …) o uno que ya tengas, tocado en la sala donde se grabó. Pulsus
sigue siendo la caja de ritmos de pads de Cantus; Tympanum es el baterista.

**Agrégalo** desde el selector de dispositivos (el **+** sobre las cabeceras de las pistas, su
estante *Batería*): **Tympanum** está junto a Pulsus. Un Tympanum nuevo toca el kit
modelado **Studio Maple** con su voz generada — al instante, sin nada que descargar.

## La vista Kit

La ventana se abre en **Kit**: los kits a la izquierda, el kit en su sala en medio, las piezas
y sus tarjetas debajo.

**La lista de kits** — **Todos · Grabados · Modelados** arriba la filtra, y un cuadro de
búsqueda encuentra un kit por su nombre.
- **Modelados** — *al instante · sin descarga*: **Studio Maple** (rock y pop, doce piezas y
  doce micrófonos), **Jazz Birch** (medido sobre Virtuosity Drums, de dominio público) y los
  demás kits modelados. Un clic toca el kit.
- **Grabados** — *baterías reales · paquetes libres*. Un kit que no está en tu disco dice lo que
  ocupa (**Bajar** y su tamaño); un clic lo descarga de su fuente oficial, con su progreso y un
  clic para cancelar. Algunos vienen **del sitio del autor** (un clic, directo de la página del
  propio autor — Cantus nunca los aloja) y otros son **Manual**: *Abrir la página*, descárgalo
  ahí (un registro, un torrent) y luego *Instalar desde un archivo…*. Un kit en disco dice **En
  disco**.
- **Tus propios kits** — **Instalar desde un archivo…** acepta un SFZ, un kit de DrumGizmo (su
  `.xml`, o el archivo comprimido en que vino) o una carpeta de WAV. Un kit de DrumGizmo se
  construye para Tympanum en segundo plano, un archivo por micrófono y por golpe: la ventana
  sigue respondiendo, su recuadro muestra el progreso (*Construyendo sus micros · 30 %*), y un
  clic en él lo cancela sin dejar nada. El kit que sonaba sigue sonando hasta que el nuevo está
  listo.
- **Kits en disco · Gestionar…** — lo que ocupan; *Gestionar…* abre la ventana de Contenido para
  actualizarlos o quitarlos.

**Cambiar de kit** es un paso de deshacer y conserva tu mezcla: una salida mantiene su canal, su
nivel y su ruta cuando el kit nuevo tiene un micrófono del mismo nombre o de la misma familia
(Bombo, Caja, Toms, Aéreos, Sala…); un canal hecho para un micrófono toma el nombre del nuevo;
los canales que quedan sin nada se van. Un preset que no nombra kit conserva el kit y su mezcla.

**La imagen** — el kit dibujado en su sala (*Dibujadas* o *Fotográficas*: Ajustes ▸ Apariencia ▸
Imágenes de instrumentos). **Al pulsar una pieza suena y queda elegida**; Alt+clic recorre sus
articulaciones; arrastrar arriba o abajo mientras pulsas fija la fuerza. *Ver micros* dibuja los
micrófonos (en un kit modelado se arrastran para moverlos).

**La tarjeta de la sala** — **Sala** es la imagen y el sonido en un solo paso: **Escenario** toca
la sala propia del kit (un kit grabado tal como se grabó; uno modelado seco, solo), **Cabina**,
**Estudio** y **Auditorio** ponen una de las salas de Cantus en los micrófonos de sala.
**Suelo**, **Paredes** y **Alfombra** cambian solo la imagen.

**Debajo de la imagen**:
- la fila de piezas con sus teclas;
- **Sensación** (todo el kit) — **Tiempo** (cada golpe hasta esos milisegundos tarde, nunca
  antes), **Fuerza** (cuánto varía la fuerza de cada golpe respecto a la escrita) y **Nunca el
  mismo golpe dos veces**;
- **Pedal del charles** — su **Controlador de apertura** (el controlador 4 por defecto): abre y
  cierra el charles con un pedal continuo; **Chasquido al cerrarse** toca el *chick* del pie;
- **Kit** (todas las piezas) — **Afinación**, **Resonancia** y **Nivel**;
- **el panel de la pieza elegida** — su tamaño, **Oír** (mantenlo para un redoble), sus
  **Articulaciones** (cada una en su tecla, un clic para oírla), **Dónde se golpea** (un clic en
  el parche lo fija y toca ahí; el borde suena más brillante), **Dispersión**, **Sonido**, **La
  oyen** (qué micrófonos la oyen) y **Abrir el mezclador ›**.

**Voz: Generada · Híbrida** (un kit modelado, bajo el kit encendido en la lista). *Generada* es
el modelo de Cantus solo. *Híbrida* toca, debajo del modelo, los primeros instantes de los
golpes de una batería real — el ataque de cada pieza tal como se grabó — por los mismos
micrófonos virtuales, la misma fuga y la misma sala. Sus ataques grabados son un paquete libre
de unos 5 MB (el del kit de jazz, de Virtuosity Drums, CC0; el del kit de estudio, de DRSKit,
CC BY 4.0): **Bajar · 5 MB** una vez y después suena sin conexión. Mientras el paquete no esté
en disco, *Híbrida* suena exactamente como *Generada*.

## Tocarlo

Tympanum sigue el mapa de batería de General MIDI (bombo en 36, caja en 38, charles cerrado
42, abierto 46, crash 49, ride 51, …). Algunas notas fuera del mapa tocan figuras de baterista
en la caja: **flam** en 91, **drag** 92, **buzz** 93 (ocho rebotes rápidos y suaves) y un
**redoble sostenido** en 94 (golpes cada 55 ms mientras la nota siga pulsada); el **splash de
pie** del charles en 95. Un kit grabado toca las grabaciones de sus articulaciones que trae su
paquete. Todos los micrófonos de un golpe tocan la misma toma, y una toma nunca repite la
anterior en esa tecla.

## La vista Mezclador

**Mezclador**, arriba en la ventana, muestra los micrófonos del kit:

- **La banda de micrófonos** (un kit modelado): el **Patrón** del micrófono elegido (cardioide,
  súper, híper, omni), su **Distancia**, su **Apunte** y su **Fuga**; **Lo que oye** — cada pieza
  en ese micrófono frente a la suya propia; **Todos los micros** — cada pieza en cada micrófono,
  en dB; un clic en una celda la apaga (los micrófonos de un kit grabado se quedan donde se
  grabaron).
- **Salidas** — **Todo a Principal** (cada micrófono en el canal propio de Tympanum), **Piezas
  a canales** (un canal por familia — bombo, caja, toms, platillos, aéreos, sala — con sus
  micrófonos dentro) y **Cada micro a su canal** (un canal por micrófono, en orden, que suena
  hacia el canal de Tympanum). Un paso de deshacer cada una; a ganancia unidad el kit separado
  suena exactamente como sin separar.
- **Los canales** — cada micrófono con su medidor, polaridad (ø), paneo, **Retardo** (hasta
  50 ms para alinear un micrófono lejano con uno cercano; arrástralo de lado o haz doble clic y
  escríbelo; 0 es *como se grabó*), silencio, solo, fader y **Salida**: *Principal*, su propio
  canal o cualquier canal de la mesa. Un retardo cambiado mientras el kit suena se desliza hasta
  su nuevo tiempo.

En el mezclador de Cantus los canales del kit quedan bajo una sola **banda** (*Batería ·
Tympanum · 12 micros*) que se pliega en un canal; *Cada micro a su canal* los agrupa por familia
dentro de ella, y un bus de batería alimentado solo por ellos puede unirse a la banda — ver
*Bandas en la mesa* en [el mezclador](README.md#6-mezclar-el-mezclador-inserciones-y-envíos).

## Los kits grabados

| Kit | Micrófonos | Licencia | En disco |
|---|---|---|---|
| Gogodze Phu Vol. II | 7 | CC0 | 0,5 GB |
| DRSKit, todos los micrófonos | 13 | CC BY 4.0 | 1,9 GB |
| CrocellKit, todos los micrófonos | 15 | CC BY 4.0 | 5,9 GB |
| MuldjordKit, todos los micrófonos | 13 | CC BY 4.0 | 2,7 GB |
| Virtuosity Drums | 7 | CC0 | 1,5 GB |
| Big Rusty Drums | 6 | CC0 | 0,7 GB |

La licencia y el crédito de cada paquete se muestran antes de bajarlo y en *Créditos* de la
ventana Acerca de; el crédito de un kit CC BY acompaña tu música donde la licencia lo pide
(MuldjordKit pide, con las palabras de su autor, *"Drum samples provided by DrumGizmo.org"* en
los créditos de un disco). Las muestras largas se leen del disco por la caché de muestras de
Cantus, y un kit carga solo los micrófonos encendidos.

## Lo que cuesta

Un kit modelado se calcula mientras suena: en el portátil donde se mide Cantus, el modelo solo
toma cerca de una décima parte de un núcleo del procesador en un groove cargado, y cada
micrófono en su propio canal cerca de una quinta parte. Un kit grabado cuesta lo que cuesta leer
sus voces. Nada trabaja mientras el kit calla.
