# Chorda: el piano modelado, los pianos grabados y el mezclador del piano

*Cantus 0.1.0-beta.15 · [Read in English](../en/chorda-pianos.md) · volver al [manual](README.md)*

Chorda es el piano de Cantus. Su sonido propio es **modelado**: cuerdas, martillos y tabla
armónica calculados mientras tocas, nada que descargar. También puede tocar un **piano
grabado** — un paquete de muestras libre (el Salamander Grand, el Steinway D de bitKlavier,
Upright Piano KW, …) o uno que compraste — con la misma ventana, las mismas notas y los mismos
canales del mezclador. Este capítulo trata de elegir la fuente, conseguir los pianos, mezclar
sus micrófonos y lo que añade un piano grabado: **Añadir nuestra sala** y el **retardo** de un
micrófono.

## Elegir la fuente

Abre Chorda (doble clic en su pista). La lista **Fuente** está a la izquierda de la vista
**Tocar**; **Todos · Grabado · Modelado** arriba la filtra.

- **Modelado** — *Chorda 2*, el modelo de hoy. *Modelos anteriores* despliega *Chorda 1*,
  guardado para que las canciones antiguas suenen como sonaban.
- **Grabado** — los pianos libres que Cantus conoce, primero el Salamander Grand. Un piano que
  no está en tu disco dice lo que ocupa (**Obtener · 742 MB**); un clic lo descarga de su fuente
  oficial, con su progreso y *Cancelar*. Uno en disco dice **En disco**; el que suena, **Sonando**.
  *Piano in 162* y *Helsinki 1967 Pro* solo se obtienen en la página de su autor (un torrent,
  una cuenta): **Abrir página**, descárgalo allí y luego *Instalar desde un archivo…*.
- **Tus pianos** — lo que instalaste desde un archivo.

Un clic en un piano que está en disco cambia la fuente: **un paso de deshacer**. Tus notas,
tu automatización, tus canales y las rutas del mezclador se quedan; volver al modelo toca sus
ajustes exactamente como estaban.

### Instalar desde un archivo…

El recuadro de trazos al final de la lista acepta un **SFZ**, un preset de **Decent Sampler**,
un **SF2** o un **zip / tar.gz** con uno. La descarga de un piano que Cantus conoce (reconocida
por su tamaño y su suma de comprobación, se llame como se llame) se instala como ese piano;
cualquier otra cosa pasa a *Tus pianos* y suena donde está (un archivo comprimido se
descomprime en la carpeta de contenido de Cantus). La lista de kits de Tympanum tiene el mismo
recuadro para kits de batería.

## La vista Tocar

Como la dibuja el diseño: el piano en su sala (el selector del piano y su acabado sobre la
imagen; la sala, el suelo, las paredes y la alfombra en la tarjeta de su esquina — el aspecto,
nunca el sonido), el teclado debajo, las tarjetas **Martillo**, **Resonancia** y **Salida**, y
**El piano** a la derecha:

*(La imagen se dibuja **Dibujadas** — ligera, por defecto — o **Fotográficas**, con reflejos
trazados y sombras suaves: Ajustes ▸ Apariencia ▸ Imágenes de instrumentos. El sonido nunca
cambia.)*

- **Tapa** — cerrada, vara corta, vara larga.
- **Perspectiva · Dónde escuchas** — de sobre los martillos a la sala; debajo, los micros del
  mezclador a los que llega, y **Abrir el mezclador ›** lleva allí.
- **Pedales** — *Derecho* y *Suave* dicen dónde está el pie del pianista (controladores 64 y
  67) o dónde los pones; *Apagadores* fija con qué firmeza paran las cuerdas; la curva de abajo
  muestra los apagadores según el pedal, sombreada la zona de medio pedal.

Cambiado en esta versión: el nivel de **Sala** de Chorda es el fader del canal *Sala* del
Mezclador, la **Anchura** está en el canal *Piano* del Mezclador (Chorda 1 la conserva en
Salida), **Apagadores** es la fila bajo Pedales y las **macros** están en la vista Editar. La
primera vez que abres la nueva vista Tocar, la línea de estado lo dice.

## La vista Mezclador

**Mezclador**, arriba en la ventana, muestra el mezclador propio del piano:

- La imagen y una tarjeta de la **salida elegida**: su nombre como lo dice el paquete, quién
  lo grabó y su licencia. Para bitKlavier, **Imágenes en disco** — cuántas de sus nueve
  imágenes de micrófono tienes, lo que ocupan y **Obtener las otras n**.
- **Salidas** — **Todo a Principal** devuelve cada salida al canal del piano; **Cada micro a su
  canal** da a cada salida un canal propio en el mezclador de Cantus, en orden, que suena dentro
  del canal del piano. Un paso de deshacer cada uno. A ganancia unidad el piano separado suena
  exactamente como sin separar.
- Los canales — **Piano** a la izquierda (el nivel de todo el piano; la Anchura del modelo),
  las salidas bajo sus familias (*Perspectivas*, *Ruidos*; una cabecera pliega sus canales) y
  **Sala** a la derecha. Cada salida tiene polaridad (ø), paneo, silencio, solo, su fader y su
  celda **Salida**: *Principal*, su propio canal o cualquier canal del mezclador.
- **Las imágenes de bitKlavier.** Cada imagen de micrófono se descarga aparte (unos 1,5 GB en
  disco cada una). Una imagen que no está en disco es un canal de trazos con su descarga; una
  que está en disco se carga o se quita de memoria desde su celda de arriba (*como se grabó* ·
  *sin cargar*): una imagen sin cargar no ocupa memoria.
- Las cinco salidas del modelo (Martillos, Pianista, Público, Sala, Mecánica) aparecen; hasta
  que el modelo las genere por separado suenan juntas en Principal, y la fila lo dice.

## Añadir nuestra sala

Un piano grabado lleva la sala donde se grabó (sus micros lejanos la oyen). Un piano grabado de
cerca o en seco puede pedir más sala: en el canal **Sala**, **Añadir nuestra sala** recorre
*No · Sala pequeña · Estudio · Auditorio* — las salas donde suenan los kits de Tympanum.
Encendida, el fader del canal es el nivel de la sala; suena en el canal del piano. **«No» no
añade nada**: no se crea sala, no se ocupa memoria, el piano suena exactamente como antes.

## El retardo de un micrófono

Cada salida tiene un retardo (hasta 50 ms) para alinear su micrófono con otro — un par lejano
que llega después que el cercano — o para atrasarlo a propósito. Es la celda **Retardo** de cada
canal de salida, bajo ø y el paneo: arrástrala de lado (0,05 ms por punto, `Mayús` 0,01) o haz
doble clic y escribe un valor (`Intro` lo deja, `Esc` lo deja como estaba); 0 es *como se grabó*
y no añade nada. Cada cambio es un paso de deshacer, y se puede dibujar como automatización. Un
retardo que cambia mientras el micrófono suena **se desliza** hasta su nuevo tiempo (sin
chasquido ni silencio); en reposo la salida se retrasa exactamente las muestras enteras que
pusiste. Cantus construye una línea de retardo solo para las salidas que tienen un retardo o un
carril de automatización sobre él; la primera vez que una salida recibe uno, las notas que suenan
en ese momento se cortan.

## Disco y memoria

- Los paquetes se guardan como **FLAC** al instalarse: las imágenes de bitKlavier ocupan unos
  1,5 GB cada una en vez de 3,8 GB; cada archivo se comprueba muestra a muestra antes de borrar
  el original.
- Las muestras largas se **leen del disco** a través de la caché de muestras de Cantus
  (*Caché de muestras…* en los ajustes). Varias imágenes de bitKlavier a la vez la llenan deprisa (unos 4 GB cada
  una); sube su límite si tocas cuatro o más juntas.
- Una imagen cuesta un 6 % de un núcleo del procesador; cuatro imágenes, un 25 %.
