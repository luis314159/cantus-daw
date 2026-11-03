# Equipo externo y superficies de control

*[Volver al manual](README.md) · [Read in English](../en/external-gear.md)*

Dos cosas viven en **Ajustes ▸ MIDI**, bajo los puertos: el **equipo externo** del proyecto (un
sintetizador o un procesador de rack que es parte de una pista) y las **superficies de control**
de tu estudio (una Mackie Control o una HUI con faders motorizados).

## Un instrumento externo: un sintetizador fuera de la computadora

1. Haz clic derecho en la pista y elige **Instrumento externo…** (o selecciónala y pulsa **Instrumento externo** en Ajustes ▸ MIDI). Sus notas salen ahora por un puerto MIDI
   hacia tu sintetizador, y el sonido del sintetizador vuelve por un par de entradas de tu interfaz
   de audio y suena en la pista (el instrumento propio de la pista calla: el sintetizador está
   afuera).
2. En la fila *Conexión* elige el **puerto de salida MIDI**, el **canal** y las **entradas**
   (*Ent 1-2* son las entradas 1 y 2 de la interfaz).
3. Haz que el sintetizador suene, detén el transporte y pulsa **Ping**.

Cantus toca el do central cuatro veces y escucha el primer sonido que vuelve. Dice lo que
encontró — *612 muestras, 12.8 ms (variación 0)* — y lo guarda con el proyecto. Desde ahí las
demás pistas esperan ese tiempo, así que tu sintetizador queda **a tiempo** con el resto de la
canción, al reproducir, al mezclar en el sitio y en el archivo exportado (usan el número guardado
en el proyecto, no una medición nueva). **Medir de nuevo** cuando cambies el búfer de la interfaz,
el ataque del sonido o un cable. Si el sonido tiene un ataque lento, corrígelo con **Ajuste** (en
milisegundos, ±500).

Si cambias el búfer de la interfaz, Cantus ajusta el número guardado con lo que la propia interfaz
informa y lo dice (*medido con búfer 256, ajustado a 512*); con otra interfaz te pide medir de
nuevo. Arrancar el transporte detiene una medición; una que no vuelve se abandona a los 15 segundos
y se avisa.

**Renders.** Congelar, mezclar en el sitio o exportar no pueden tocar un sintetizador o un rack que
está fuera de la computadora: Cantus los rechaza mientras una unidad suena y dice qué hacer — grabar
el retorno del equipo en una pista (la grabación se coloca con la latencia medida) o apagar la
unidad.

## Un efecto externo: un procesador de rack

**Efecto externo** da a la pista un *envío* (un par de salidas de la interfaz) y un *retorno* (un
par de entradas). **Ping** envía una ráfaga corta de ruido por el envío y la busca en el retorno;
necesita el cable conectado, y avisa si no volvió nada, si solo volvió zumbido o si las mediciones
no coinciden. El tiempo medido se guarda y el resto de la mezcla lo espera. El sonido de la pista sale por el par de envío (usa las salidas 3-4 o más: las 1-2 son la
master) y lo que vuelve por el retorno suena en su lugar, con el nivel, los inserts y los envíos
de la pista.

## Lo que dice la pantalla

Cada unidad dice si puede sonar ahora: *lista*, *apagada*, *no hay dispositivo de audio abierto*,
*hay otra interfaz de audio abierta*, *la interfaz no tiene esos canales*, *el puerto MIDI no
está*, *su pista ya no existe*. Cantus nunca abre un dispositivo ni un puerto para una unidad que
no está disponible, ni le hace ping. **Activo** conserva la configuración y toca la pista como si
la unidad no estuviera. **Quitar** la saca y devuelve a la pista la entrada que tenía — deshacer la devuelve. Quitar una unidad (o apagarla, o cambiarle el puerto o el canal) primero suelta toda nota que tenga sonando en el sintetizador.

Todo se puede deshacer y cada paso tiene nombre en el historial (*añadir equipo externo*, *medir
equipo externo*…). Un agente puede hacerlo todo (`add_external_unit`, `ping_external_unit`,
`list_external_units`).

## Superficies de control: Mackie Control y HUI

Pulsa **Añadir superficie**, elige el tipo — *Mackie Control*, *Extensión Mackie 1/2* (más canales
a la derecha de la unidad principal) o *HUI* — y sus puertos MIDI de entrada y salida (la salida
es por defecto la del mismo dispositivo). ¿Sin equipo todavía? **Añadir superficie virtual**
conecta una unidad virtual a Cantus, para probar la configuración.

- Los **faders** mueven los de los canales (y los siguen: los motores van a donde está el
  mezclador, nunca mientras tocas uno). Un toque hasta soltarlo es un solo paso de deshacer.
- Los **V-pots** panoramizan; pulsarlos centra; **Shift** los hace diez veces más finos. **Flip**
  cambia faders y V-pots (el fader panoramiza, el V-pot fija la ganancia).
- Los botones **Rec**, **Solo**, **Mute** y **Select** y sus luces siguen al mezclador.
- **Bank ◂ ▸** mueve ocho canales (dieciséis con una extensión), **canal ◂ ▸** uno. El display de
  dos dígitos muestra la página.
- Las **tiras de nombre** muestran el nombre de cada canal y debajo su ganancia; **Name/Value**
  muestra el pan. Los **medidores** muestran niveles; el display grande, la posición.
- **Play, Stop, Record, Cycle, Click, Rewind, Fast forward, Scrub, la rueda jog, Marker ◂ ▸, Save y
  Undo** (Shift + Undo es Rehacer) manejan el transporte y el proyecto.

El puerto de una superficie es de la superficie: sus botones nunca tocan notas ni llegan a la
asignación genérica de controladores, que sigue igual para los demás dispositivos. Los últimos
mensajes de cada superficie aparecen bajo la lista (en la HUI, cualquier interruptor que Cantus no
asigna se nombra por zona y puerto: mándanoslo y lo añadimos).

**Faders táctiles.** Toda Mackie Control y HUI sabe cuándo hay un dedo en un fader. Algunos
controladores en modo Mackie no: apaga *Faders táctiles* para ellos, y un fader que mueves se
mantiene 0.6 s antes de que los motores sigan al mezclador — si no, los motores pelearían con tu
mano.

*La HUI está asignada para sus ocho canales, la selección de canal y banco, shift, deshacer y
guardar, las flechas y scrub, el transporte, el bucle y la vuelta al inicio, y la rueda jog; las
demás zonas se leen y se ignoran. Cantus le hace ping cada segundo y avisa si deja de responder.*
