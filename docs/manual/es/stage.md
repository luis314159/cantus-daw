# El escenario: dónde baila un bailarín

*[Volver al manual](README.md) · [Read in English](../en/stage.md)*

Ver también: [El bailarín](dancer.md) — sus pasos, sus caras y sus personajes · [Importar un personaje](import-characters.md) · [Exportar un vídeo](video.md) — la canción con sus bailarines (también desde el menú de la ventana de compañía: *Exportar vídeo…*).

Un **bailarín** es una pista cuyo instrumento no suena y dibuja un personaje que baila con
tu canción. Esta página trata de *dónde* baila; sus pasos, sus caras y sus personajes están
en la ventana del propio bailarín.

## Sobre el espacio de trabajo, en la pestaña Escenario, u oculto

Cada bailarín tiene un lugar donde se muestra (el *Dónde* de su ventana):

- **Sobre el espacio de trabajo** — el personaje está justo encima de tu arreglo, tu
  mezclador y tus editores, sin nada alrededor: ni ventana ni marco, transparente. Su **X**
  y su **Y** dicen dónde están sus pies (de 0 a 1, a lo ancho y a lo alto del espacio de
  trabajo) y su **Tamaño** cuánto mide (a 1×, cerca de un tercio de la altura del espacio
  de trabajo). Automatiza X y cruzará la ventana caminando.
- **En la pestaña Escenario** — el personaje baila en un escenario propio, una pestaña del
  espacio de trabajo que se abre bajo el inspector la primera vez. Arrastra la pestaña a
  donde quieras, apílala junto a otras o hazla flotar como cualquier otra área; tus
  disposiciones la conservan.
- **Oculto** — no se muestra en ninguna parte y no cuesta nada.

## Moverlo, cambiar su tamaño, fijarlo

Señala un bailarín y aparece su marco:

- **arrastra su cuerpo** para moverlo (un paso de deshacer por arrastre: *Mover bailarín*);
- **arrastra el tirador redondo** de su esquina superior derecha para hacerlo más grande o
  más pequeño (sus pies se quedan donde están);
- en la **píldora** de encima: **fijar** lo mantiene por encima de los paneles flotantes
  (un mezclador flotante, la ventana de un dispositivo); **acoplar** lo pone en la pestaña
  Escenario (en la pestaña, **flotar** lo devuelve sobre el espacio de trabajo);
  **ocultar** lo oculta.

El marco se desvanece un momento después de que el puntero se va.

## Lo que cuesta

Un bailarín se dibuja solo cuando su postura cambia: como mucho 60 veces por segundo
mientras suena la canción, 15 veces por segundo mientras está parada (una respiración lenta
y un parpadeo), y nunca cuando está oculto o Cantus está minimizado. Parado, cuando pasan
diez segundos sin que ocurra nada (ni el puntero sobre la ventana, ni una tecla, ni
Reproducir), mantiene su postura y no cuesta nada; lo siguiente que hagas lo despierta donde
estaba. Mientras la ventana de compañía está abierta sigue respirando, porque es lo que
transmites. Cada bailarín se dibuja
en una caja justo del tamaño que necesita, con los píxeles reales de tu pantalla, así que es
nítido con cualquier zoom.

## La ventana de compañía

*Ver ▸ Ventana de compañía* (**Alt+Mayús+D**) abre una ventana pequeña propia, **Cantus
Stage**, que flota sobre todo —Cantus, tu navegador, un juego— sin marco y con el fondo
transparente. En ella un personaje baila **con lo que suene**, sin automatización: la
canción mientras suena; un pulso propio al tempo del proyecto cuando el transporte está
parado pero sale sonido (una preescucha, tu instrumento por una entrada); una respiración
lenta en silencio. Baila con más ganas cuanto más fuerte suena la música.

Arrástrala desde cualquier punto para moverla; arrastra su esquina inferior derecha para
cambiar su tamaño. Señálala para ver su botón ⋯, o haz clic derecho en ella, para sus
opciones — cada fila pasa al valor siguiente con un clic:

- **Personaje** — cada personaje de Cantus y luego *Los bailarines del proyecto* (tus
  pistas de bailarín, tal como bailan en el escenario).
- **Tamaño** — tres tamaños de compañía, un cuadrado de 600 × 600, un vertical de
  540 × 960 y uno ancho de 960 × 540.
- **Fondo** — transparente, el escenario del tema, o un verde, azul o magenta croma.
- **Visualizador** — el espectro del máster, su nivel, o apagado; **Compás y pulso** y
  **Nombre y tempo de la canción**, encendidos o apagados.
- **Siempre encima**, **Modo para streaming**, **Copiar la configuración de OBS** y
  **Cerrar**.

El menú muestra también **lo que cuesta**: el tiempo de procesador de cada uno de sus
fotogramas, su frecuencia de fotogramas y la parte de un núcleo que suman. El tiempo de la
tarjeta gráfica no se mide (Cantus no se lo pregunta al controlador), y el menú lo dice.
Minimizada, la ventana no dibuja nada y no cuesta nada. Sigue abierta entre sesiones hasta
que la cierras.

## Modo para streaming

*Ver ▸ Modo para streaming* convierte la ventana de compañía en una fuente de captura para
OBS Studio o Streamlabs: la ventana conserva el mismo título, **Cantus Stage**, en cada
sesión, no muestra nada más que el escenario (ni marco ni botones al señalarla — el clic
derecho sigue abriendo su menú), y su tamaño y su lugar quedan fijos y recordados.
*Copiar la configuración de OBS* en su menú copia los pasos para su fondo.

| Fondo | En OBS | Lo que obtienes |
|---|---|---|
| Transparente | **Captura de ventana**, método *Windows 10 (1903 y posterior)* | El personaje sin nada alrededor |
| Transparente | Captura de ventana, método *BitBlt* | Un fondo negro (usa mejor un fondo croma) |
| Verde, azul o magenta croma | Captura de ventana + un filtro **Clave croma** del mismo color | El personaje recortado; baja la *Similitud* hasta que sus bordes queden limpios |
| El escenario del tema | Captura de ventana | El personaje sobre el escenario de Cantus |

En Windows, la *Captura de juego* de OBS no pudo capturar la ventana en un portátil con dos
chips gráficos (uno dibuja Cantus y el otro ejecuta OBS); la *Captura de ventana* siempre
pudo. En Linux, usa *Captura de ventana (Xcomposite)* o *Captura de pantalla (PipeWire)*;
un escritorio con compositor conserva la transparencia, y sin él elige un fondo croma.

## Atajos

| Tecla | Qué hace |
|---|---|
| Alt+D | Mostrar u ocultar los bailarines en pantalla (*Ver ▸ Escenario*) |
| Alt+Mayús+D | Abrir o cerrar la ventana de compañía |
| Ctrl+Mayús+D (Cmd+Mayús+D en un Mac) | Activar o desactivar el modo para streaming (*Ver ▸ Modo para streaming*, o la paleta de comandos) |
