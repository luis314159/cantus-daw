# El modo de grabación

*[Volver al manual](README.md) · [Read in English](../en/recording-mode.md)*

Una sesión grande puede exigir tanto al ordenador que no puedes bajar el búfer de audio sin
chasquidos. El **modo de grabación** quita esa carga antes de grabar: reproduce cada canal
que alimenta el máster desde una grabación de sí mismo —o, si lo eliges, toda la mezcla
desde una sola grabación—, de modo que mientras grabas el ordenador solo lee audio y lo
suma. El canal en el que grabas sigue en vivo.

**Qué hace y qué no.** No acorta la latencia de ida y vuelta de tu interfaz de audio (el
búfer, el controlador y los conversores). Te deja poner un búfer más pequeño sin cortes,
porque al ordenador le queda poco por calcular, y mantiene la pista en la que tocas libre
del retraso que añaden los plug-ins de otras pistas. La barra muestra el búfer, la latencia
de ida y vuelta (según la informa tu controlador, o medida cuando Cantus la ha medido) y la
carga del procesador antes y desde entonces.

## Entrar

1. **Arma** la pista o las pistas en las que vas a grabar. Las pistas armadas, y los canales
   por los que pasan, siguen en vivo. (Puedes armar otra después: mira más abajo.)
2. Abre las **opciones de grabación** (la flechita junto al botón de grabar) y pulsa
   **Preparar para grabar**.
3. Cantus muestra lo que va a hacer: cuántos canales y pistas renderiza, cuántos renders ya
   están en disco, **hasta cuánto disco ocupan los archivos nuevos** (cada render es la
   canción más una cola de hasta 30 segundos; un canal estéreo de 5 minutos son como mucho
   unos 120 MB) y qué sigue en vivo y por qué (una pista armada, una pista que suena por
   equipo externo, que una grabación hecha sin conexión no puede tocar).
4. Elige el **máster**:
   - **En vivo** (por defecto): la cadena del máster trabaja sobre la suma.
   - **Pre-renderizado**: toda la mezcla —salvo lo que sigue en vivo— es una sola grabación, lo
     más ligero. Nada del máster ni de la mezcla se mueve con el modo activo.
5. Elige cómo se comporta la **mezcla** (con el máster en vivo):
   - **En vivo** (por defecto): el fader, el paneo, el mute y el solo de cada canal siguen
     funcionando, así que "más guitarra en mis audífonos" no exige salir. Un canal que entra
     en un bus pre-renderizado, o que envía después del fader a un retorno de reverb o delay
     pre-renderizado, mantiene su mezcla bloqueada, porque esa grabación ya contiene su fader; el
     diálogo dice cuántos. Un envío antes del fader no contiene el fader: ese canal conserva
     su fader y su paneo.
   - **Bloqueados**: nada de la mezcla se mueve hasta que salgas.
6. Elige qué pasa con un canal que no se puede renderizar (por ejemplo un plug-in que no
   puede renderizar sin conexión): **sigue en vivo y se nombra**, o la preparación **se
   detiene**.
7. Pulsa **Preparar**. Aparece una barra arriba con el progreso y **Cancelar**.

Cada canal se renderiza después de sus efectos y automatización y antes de su fader y
paneo, así que con los controles sin tocar el sonido es **idéntico bit a bit** al que oyes
normalmente, también con el máster pre-renderizado. Un bus es una sola grabación que incluye todo
lo que suma.

El modo no puede empezar mientras el tempo sigue a Ableton Link, al reloj MIDI o al código
de tiempo, ni mientras suena una escena de la sesión: las grabaciones sonarían a otra
velocidad. Si una de esas cosas empieza con el modo activo, se sale del modo (y se detiene
la toma).

## Mientras está activo

Una barra imposible de no ver dice **MODO DE GRABACIÓN** (y *Máster pre-renderizado* cuando lo
está), cuántos canales están pre-renderizados, el búfer, la latencia de ida y vuelta, la carga del
procesador antes y desde entonces, cuántos canales siguen en vivo y qué inserciones deja
fuera la escucha con baja latencia. La escucha con baja latencia está activa mientras dura
el modo (las opciones de grabación dicen *la impone el modo de grabación*). Los canales
pre-renderizados se ven congelados.

Todo lo que cambiaría el sonido se rechaza con *"Modo de grabación: sal de él para
editar"*: notas, clips, efectos, instrumentos, automatización, ruteo, el tempo, añadir o
quitar pistas, el modo patrón, y deshacer o rehacer eso, en todos los canales salvo los
armados, incluso uno que siguió en vivo por otro motivo. Los nombres, los colores, los
marcadores, el bucle y el metrónomo siguen editables; también las pistas armadas y sus
canales, los ajustes de un bailarín (no produce sonido) y el máster mientras está en vivo.

**Armar otra pista** cuyo canal está pre-renderizado te pregunta si quieres ponerlo en vivo: su
propia cadena vuelve a sonar al instante, alineada a la muestra, y no se renderiza nada de
nuevo. Solo vuelven en vivo las grabaciones que contienen esa pista. Con el máster
pre-renderizado, el máster también vuelve a sonar en vivo (contenía esa pista).

Graba como siempre. Tu toma nueva queda en la pista en vivo y suena por su propio canal.

No tienes que hacer nada para proteger tu proyecto: entrar y salir del modo no son pasos de
deshacer ni marcan el proyecto como modificado, y el archivo del proyecto **nunca se
escribe en el estado del modo**. Si el programa o el ordenador se detienen, reabres el
proyecto fuera del modo, y los renders siguen en disco.

## Salir

Pulsa **Salir** en la barra (o en las opciones de grabación). Elige:

- **Salir y conservar archivos**: todo suena en vivo otra vez. Los renders se quedan, así
  que al volver a entrar solo se renderizan los canales que cambiaron.
- **Salir y descartar archivos**: lo mismo, y los renders se borran para liberar disco.
- **Quedarme**.

No puedes salir mientras se graba una toma: detén primero.

## Para un agente de IA

`prepare_to_record` (con `máster`, `mix_controls`, `on_failure`; con `dry_run` solo
responde el plan y el disco que necesita), `get_recording_mode` y `leave_recording_mode`.
Un agente que arma una pista pre-renderizada pone su canal en vivo. Mira [Qué puede hacer un
agente de IA en Cantus](ai-tools.md).
