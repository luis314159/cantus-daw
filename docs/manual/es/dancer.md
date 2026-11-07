# Saltator: un personaje que baila tu canción

*[Volver al manual](README.md) · [Read in English](../en/dancer.md)*

**Saltator** es un instrumento que no produce sonido: dibuja un personaje que baila tu
música. Ponlo en una pista y baila al ritmo enseguida, sin preparar nada; después hazlo
tuyo: elige el personaje, automatiza sus pasos compás a compás, toca pasos desde un clip
MIDI o tu teclado, deja que reaccione a la batería, llévalo a tu transmisión. Dónde se
muestra (sobre el espacio de trabajo, en la pestaña Escenario, en la ventana de compañía,
el modo para streaming) está en [la página del escenario](stage.md); un clip con el audio
de la canción, en [la exportación de video](video.md); tus propios personajes, en
[importar un personaje](import-characters.md).

## Añadir uno

*Añadir pista* (o el selector de dispositivos, Ctrl+T) ▸ el estante **Visual** ▸
**Saltator**. La pista nueva se llama *Saltator*, tiene de icono una figura que baila, y
baila en ella Fuga, el ninja del reparto. Se abre su ventana.

**El reparto.** Lo encabezan Fuga, el ninja (enmascarado, con una bufanda carmesí rasgada
al viento: volteretas, patadas altas, un aterrizaje agachado, una reverencia marcial), y
Volta, el mecha (armadura de metal oscuro, un visor en V de luz cuyas rendijas muestran su
ánimo, un núcleo altavoz que late con el compás, propulsores que se encienden al saltar:
pop and lock, el salto cohete, una pose de héroe); luego la familia: Trino, el pájaro
cantor; Miso, el gato; Lana, la oveja; Tic, el robot metrónomo; Corchea, la nota; Sol, el
niño. Una canción hecha antes de Fuga conserva su personaje: un bailarín guardado sin
ninguno sigue con Trino.

## La ventana

**Tocar** es un pequeño teatro:

- **El escenario**: el personaje bailando en vivo, exactamente como suena la canción (se
  tiene en cuenta la latencia de la salida), con un foco que lo sigue y se aviva en el
  tiempo. Arriba: el compás y el tiempo, y el paso que baila.
- **Personaje**: cada personaje que trae Cantus, cada uno bailando su propia rutina, y
  cada personaje importado al proyecto. Haz clic en uno para elegirlo (un paso de
  deshacer). Debajo: su nombre, su frase y su licencia. **Importar** trae uno tuyo (un
  Lottie, una hoja de sprites o un personaje de DragonBones: [importar un
  personaje](import-characters.md)); para uno importado, **Editar pasos…** elige qué
  animación suya baila cada paso.
- **Exportar video…** (en la barra de título del escenario): la canción con sus bailarines
  como video: MP4 o WebM, cuadrado, vertical o una capa transparente ([exportar un
  video](video.md)).
- **Pasos**: el mapa de pasos como dieciséis pads, uno por tecla de **C3** a **D#4**. El
  paso que baila ahora se ilumina. Pulsa un pad para bailar ese paso enseguida (una nota en
  vivo, como en el teclado).
- **Baile**: *Paso* (*Al ritmo*, o uno de los dieciséis pasos por su nombre),
  *Desplazamiento* (*En su sitio*, *Camina a la izquierda*, *Camina a la derecha*,
  *Pasea*), *Velocidad* (de ¼× a 4× el tiempo), *Energía*.
- **Cuerpo**: *Altura* (cuánto se separa del suelo), *Orientación* (izquierda, de frente,
  derecha), *Giro* (°), *Tamaño* (×).
- **Aspecto**: *Ánimo* (Contento, Emocionado, Tranquilo, Sorprendido, Dormilón, Genial,
  Tímido, Neutral) y *Color* (gira sus colores por el círculo; ojos y contornos se quedan).
- **Oídos**: *Reacciona a*: lo que escucha. Haz clic en el campo (o en sus ‹ ›) para
  pasar por *Nada*, *Toda la canción* y cada canal del mezclador. *Reacciona* dice cuánta
  de su energía decide el nivel.

**Editar** tiene cada número en una tarjeta, y **Dónde se muestra** (*Flotante*,
*Acoplado*, *Oculto*; *Siempre encima*) y su posición en el escenario.

Cada número es un parámetro corriente: clic derecho para *Automatizar*, *Aprender
controlador MIDI* y los moduladores.

## Cómo baila

- **Al ritmo, por su cuenta.** Con *Paso* en *Al ritmo* baila su propia rutina, un paso
  por compás, cada paso cayendo en el tiempo, sin automatización.
- **Automatización.** Dibuja un clip de automatización de *Paso* y baila el paso elegido
  desde la primera línea de compás del clip hasta donde termina (entre clips el valor se
  mantiene; termina el clip en *Al ritmo* para devolverlo a la rutina). Cada número se
  automatiza igual.
- **Notas.** Una nota en una tecla del mapa de pasos toca ese paso desde su comienzo:
  mantenida, sigue; suelta, termina el paso en curso. La velocidad (fuerza) es lo grande
  que baila. Las notas de un clip de sesión también lo mueven.
- **Moduladores.** Un LFO en *Color* hace girar sus colores; un secuenciador de pasos en
  *Paso* cambia sus pasos en cada paso; aprende la rueda de modulación en *Energía* y baila
  más grande cuanto más la subes.
- **Reaccionar a.** Elige un canal en *Reacciona a*: un seguidor lo escucha y el bailarín
  baila más grande cuando suena fuerte. Es un paso de deshacer; elige *Nada* para parar.
- **Aprender MIDI.** Clic derecho en cualquier número ▸ *Aprender controlador MIDI* y
  mueve un mando; un fader en *Paso* recorre los pasos por su nombre.

## Preajustes

*Al ritmo*, *Fiesta*, *Tranquilo* y *Con estilo* cambian cómo baila. Un preajuste nunca
cambia quién baila ni dónde: el personaje, su sitio, su tamaño y dónde se muestra se
quedan.

## Agentes

Un agente de IA ve Saltator en el catálogo de unidades (`get_unit dancer`: cada número, los
pasos y ánimos por su nombre, el mapa de pasos, los personajes). Puede añadir uno
(`add_track` con kind `dancer`), automatizar su *Paso* y ajustar sus números como los de
cualquier instrumento.
