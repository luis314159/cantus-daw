# Faders VCA e instantáneas de la mesa

*Cantus 0.1.0-beta.11 · [Read in English](../en/mixing-studio.md) · volver al [manual](README.md)*

Dos herramientas de una mesa de estudio, en el mezclador: los **faders VCA**, que mueven un
grupo entero de pistas a la vez sin tocar sus propios faders, y las **instantáneas**, que
guardan la mesa y la devuelven, entera o canal por canal.

## Faders VCA

Un VCA es una franja **sin audio**: un fader, un silencio y un solo. Las franjas que lo
**siguen** reciben su nivel sumado al suyo, en decibelios, en la etapa de ganancia, justo donde
actúa el fader de cada franja. Sus faders, su automatización y sus moduladores no se reescriben:
una franja a −3 dB que sigue a un VCA a −6 dB suena a −9 dB y sigue mostrando −3 dB. Un VCA a
0 dB no cambia nada, ni una sola muestra.

**Crear uno.** En el menú ▸ del mezclador, **Añadir VCA**; o clic derecho en una franja (o en
una selección) → **Seguir un VCA ▸ Nuevo VCA con esta franja**. Un VCA se dibuja como una
franja con una placa «VCA» de color donde iría el medidor, y cuántas franjas lo siguen. Cada
franja que un VCA alcanza lleva una barra del color del VCA bajo su número; pasa el ratón sobre
el número para ver cuál.

**Seguir uno.** Clic derecho en una franja → **Seguir un VCA ▸** y marca los VCA que sigue (con
una selección, todas las seleccionadas lo siguen juntas). Una franja puede seguir varios VCA:
sus decibelios se suman, **cada uno contado una vez** aunque un VCA llegue a la franja por dos
caminos (un VCA puede seguir a otro VCA). Un ciclo se rechaza. **Un grupo puede seguir a un
VCA**: clic derecho en el separador del grupo → **VCA del grupo**.

**El silencio y el solo siguen.** Un VCA en silencio silencia todo lo que alcanza; un VCA en
solo pone en solo todo lo que alcanza (y sus rutas, como el solo de FL).

**Automatización.** El fader del VCA es un fader normal: dibuja o graba automatización en él, o
ponle un modulador. La automatización de cada franja se queda y las dos se suman. Un VCA no
puede tener inserciones, envíos, salida ni pistas, y un VCA que alguien sigue no se puede
quitar.

## Instantáneas

La fila de título del mezclador tiene un campo **Instantáneas**. **Guardar instantánea** guarda
la mesa —el fader, el panorama, la anchura, la polaridad, el silencio, el solo, el intercambio,
el interruptor de inserciones y la EQ de cada franja; el nivel, el interruptor y el punto de
toma de cada envío; el bypass, el seco/mojado y los parámetros nativos de cada inserción; y el
máster— con un nombre; **Guardar instantánea de las franjas seleccionadas** guarda solo esas.
Una instantánea guarda *estado*, nunca *estructura*: el encaminamiento, qué inserciones existen
y el estado propio de un plugin alojado no están en ella, y recuperarla nunca crea, quita ni
reencamina nada.

El submenú de cada instantánea:

- **Recuperar** devuelve todos los canales en un solo paso de deshacer. **Recuperar las franjas
  seleccionadas** devuelve solo esas; **Recuperar todo, también las franjas a salvo** alcanza
  también a las franjas marcadas a salvo.
- **A/B con la mesa** intercambia la mesa con la instantánea y vuelve, para oír la diferencia.
- **Actualizar con la mesa** la guarda de nuevo; **Renombrar**; **Quitar**.
- **Señalar en el cursor** (más abajo).

**Franjas a salvo.** Clic derecho en una franja → **A salvo de recuperar instantáneas**: al
recuperar todo, la franja se queda como está (una recuperación que la nombra sí la alcanza). **A
salvo del solo** mantiene una franja sonando cuando otra está en solo (un clic, una referencia),
salvo que esté en silencio.

**Señales: instantáneas en la línea de tiempo.** Una señal recupera una instantánea en un
compás —la escena entera—: desde ahí se mantienen sus faders, panoramas, anchuras, niveles de
envío y parámetros nativos de efectos, y se conmutan sus silencios, solos, polaridad,
intercambio, interruptores de inserciones, bypass y EQ de franja (con un fundido de 16 muestras,
nunca un corte). Un nivel se mantiene hasta otra señal o hasta que empiece tu siguiente clip de
automatización de ese control; un clip que sonaba a través del compás de la señal se calla desde
ella. Las señales se colocan en el cursor y se listan en el menú Instantáneas, donde se quita
cada una; añadir y quitar una señal es un paso de deshacer. En la vista de sesión, los clips de
una escena llevan sus propias envolventes (una escena se lanza tocando, no en un compás).

## Qué puede hacer un agente

`create_vca`, `assign_vca`, `take_snapshot`, `recall_snapshot`, `get_mix_snapshots`,
`remove_snapshot`, `add_mix_cue`, `remove_mix_cue` y `set_strip_safe` — véase
[Qué puede hacer un agente de IA en Cantus](ai-tools.md).
