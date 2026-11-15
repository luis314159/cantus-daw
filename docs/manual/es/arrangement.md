# El arreglo: tiempo, secciones y ripple

*Cantus 0.1.0-beta.15 y posteriores · [Read in English](../en/arrangement.md)*

El arreglo es donde la canción se dispone en el tiempo: una fila por pista, la regla y el
carril de secciones encima, la pista de tempo cuando la muestras. Esta página trata de editar
**el tiempo mismo** —hacer sitio en una canción, quitarle compases, mover una sección entera— y
del **modo ripple**, que hace que la edición de un clip mueva lo que lo sigue.

## Insertar tiempo

Arrastra en la regla para seleccionar el tiempo que quieres añadir (por ejemplo, los compases
17–18) y luego:

- haz clic derecho dentro de la selección (sobre un clip o entre ellos) y elige **Insertar tiempo**, o
- pulsa **Insert**, o
- abre la paleta de comandos (**Ctrl+Mayús+P**) y ejecuta *Insertar tiempo*.

Ese tiempo vacío entra al comienzo de la selección, **en todas las pistas**:

- un clip que cruza ese punto se divide allí (las dos mitades conservan todo el contenido del
  clip, como hace *Dividir*) y todo lo que viene después se mueve más tarde;
- los clips de automatización se mueven con la música, y también los carriles de tomas y los
  comps alternativos;
- la pista de tempo y los cambios de compás también se mueven, y los compases nuevos conservan
  el tempo que la canción tenía allí; dentro de una rampa de tempo, la rampa se corta para que
  los compases de antes y de después suenen exactamente igual;
- los marcadores y las señales del mezclador (sus instantáneas colocadas en la canción)
  posteriores se mueven; una sección que cruza el punto crece con los compases nuevos; el bucle
  y el rango de pinchado se mueven o crecen del mismo modo.

Después quedan seleccionados los compases nuevos, vacíos. Sin tiempo seleccionado, **Insert**
mete un compás en el compás donde está el cursor de reproducción.

## Borrar tiempo

Selecciona el tiempo y elige **Borrar tiempo** en el mismo menú, o pulsa **Ctrl+Supr**
(**Ctrl+Retroceso** hace lo mismo; ⌘⌫ en un Mac). Lo que hay dentro desaparece en todas las
pistas —los clips que cruzan los bordes se cortan allí— y todo lo que sigue se mueve antes para
cerrar el hueco, con su automatización, sus cambios de tempo y de compás, sus marcadores, sus
señales del mezclador y sus secciones. Un marcador o una señal dentro del tiempo borrado se va
con él; un bucle o un rango de pinchado que queda entero dentro se elimina. Una señal dura hasta
la siguiente, así que lo que sigue conserva la escena con la que sonaba: si la última señal
dentro del tiempo borrado seguía en efecto al final, queda una señal de la misma instantánea
donde se cerró el hueco. Mover o copiar una sección funciona igual: sus compases llegan con su
escena, y la música que viene después conserva la suya.

Borrar el tiempo que acabas de insertar te devuelve la canción exacta: los clips que la
inserción dividió vuelven a unirse, la pista de tempo queda como estaba; no queda nada atrás.

Cada una de estas acciones es **un solo paso de deshacer**.

## Arrastrar una sección mueve sus compases

El carril de secciones, encima de las pistas, nombra las partes de la canción («Estrofa · 16
compases»). Arrastra una sección por su cuerpo y **sus compases se mueven en todas las pistas**:

| Mantén | Al soltar (Arrastrar una sección: **Insertar**, lo predeterminado) |
|---|---|
| nada | los compases se sacan y se insertan donde caen; lo que había allí se desplaza: la canción reordenada, sin perder nada |
| **Mayús** | los compases se mueven y reemplazan lo que había donde caen; donde estaban queda en silencio |
| **Ctrl** | una copia de los compases se inserta donde cae; lo que sigue se desplaza (la canción se alarga) |
| **Ctrl+Mayús** | una copia de los compases reemplaza lo que había donde cae |
| **Alt** | (con cualquiera de los anteriores) se sale de las líneas de compás: cae donde está el puntero |

Si prefieres que el arrastre simple reemplace (como FL Studio mueve el contenido), elige
**Reemplazar** en *Ajustes ▸ Edición ▸ Arrastrar una sección*: entonces el arrastre simple
reemplaza y **Mayús** inserta. Insertar es lo predeterminado porque nunca sobrescribe nada, y es
lo que hacen Logic y Studio One al arrastrar una sección.

Mientras arrastras todavía no cambia nada: la sección viaja como un fantasma, los compases que
reemplazará se tiñen con el color de acento en todas las pistas (o, cuando inserta, una línea de
acento muestra dónde entrará), el hueco que deja se dibuja con trazos, y la línea de ayuda de
abajo dice exactamente qué hará al soltar. Al soltar, es un solo paso de deshacer.

Los arrastres que insertan llevan consigo el tempo y el compás de la sección; los que
reemplazan conservan el reloj de la canción, como al mover un clip. Los marcadores y las
señales del mezclador que hay dentro de la sección viajan con sus compases (una copia tiene las
suyas propias).

Para mover **solo el nombre y el tramo** de una sección, sin su música, selecciona el tiempo
que debe cubrir y elige **Llevar la sección a la selección de tiempo** en el menú de la
sección. Su final se sigue pudiendo arrastrar para cambiar su largo.

## El modo ripple

La tecla con un clip y dos flechas, a la derecha de la fila de herramientas del arreglo junto
a *Seguir*, cambia el modo ripple, o pulsa **Alt+Mayús+P**. Recorre:

- **Apagado** (lo predeterminado, cada vez que Cantus arranca): una edición mueve solo lo que toca.
- **Ripple · esta pista**: borrar un clip cierra su hueco en su pista; mover un clip a lo largo
  de su pista mueve igual los clips que lo siguen en esa pista; estirar o acortar el borde
  derecho de un clip mueve lo que lo sigue; pegar inserta, y lo que había después del punto
  de pegado se desplaza.
- **Ripple · todas las pistas**: lo mismo, en todas las pistas, con los marcadores, las
  señales del mezclador, las secciones y los cambios de tempo posteriores. Los clips de otras
  pistas que empezaban dentro de un tramo cerrado se quedan donde están: el modo ripple nunca
  corta un clip; *Borrar tiempo* sí.

Mientras el modo está encendido, su tecla se tiñe con el acento y dice cuál es. Cada edición,
con lo que movió, es un solo paso de deshacer.

## Teclas

| Acción | Tecla |
|---|---|
| Insertar tiempo | Insert |
| Borrar tiempo | Ctrl+Supr, Ctrl+Retroceso |
| Modo ripple (apagado, esta pista, todas las pistas) | Alt+Mayús+P |

Las teclas del tiempo son del arreglo: actúan mientras es lo que estás editando (lo último en
que hiciste clic) y el puntero no está sobre otra zona o una ventana. Sobre el editor de un
efecto, Ctrl+Supr quita su banda y no toca el tiempo de la canción; Insert en el mezclador no
inserta nada. Cuenta cualquier parte del arreglo —su regla, su carril de secciones, su fila de
herramientas—, también flotando en una ventana propia o maximizado. Si llegaste a él con el
teclado (**Ctrl+F6**, *Zona siguiente*), las teclas funcionan dondequiera que repose el ratón,
hasta que lo pulses. Cuando una tecla de tiempo no hace nada, la línea de estado dice por qué y
cómo llegar.

FL Studio inserta espacio con Ctrl+Insert; la biblioteca de ventanas sobre la que está hecho
Cantus lee esa combinación como *Copiar* antes de que Cantus la vea, así que Cantus usa Insert
sola. Todas las teclas se pueden cambiar en *Ayuda ▸ Atajos de teclado*.
