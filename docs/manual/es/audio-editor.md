# El editor de audio

*Cantus 0.1.0-beta.11 y posteriores · [Read in English](../en/audio-editor.md)*

Quita una tos de una toma, silencia un golpe, haz fundidos en los extremos, lleva la toma a un
nivel, quítale el siseo y los chasquidos, borra el aire muerto entre frases — y escúchalo en el
clip al instante. El editor de audio es el editor de onda que se abre desde un clip. (El editor de
tono, que corrige notas sin tocar el audio, es otra herramienta: doble clic en el clip.)

## Abrirlo y salir

Clic derecho en un clip de audio ▸ **Editar en el editor de audio…** La ventana muestra el
archivo entero del clip. Ciérrala con ✕: el clip suena como lo dejaste. Deshacer y Rehacer
funcionan como en todo lo demás, con la ventana abierta o cerrada; la ventana los sigue. Abrir otro
proyecto la cierra.

**Nada de lo que haces aquí toca el archivo del que partiste.** Cada edición hace un archivo nuevo
(se llama `edit-` y un código de su contenido, en la carpeta *Audio* del proyecto) y apunta el clip
a él: un paso de deshacer cada una, con el original aún en la lista de archivos del proyecto.

También en la barra: **Abrir archivo…** pone un archivo en una pista nueva y lo abre aquí; **Pegar
desde archivo…** pega un archivo en el cursor (a otra frecuencia de muestreo, o estéreo en mono, se
convierte antes — igual que el portapapeles); **A clip nuevo** — o **arrastrar con Alt** la
selección fuera de la ventana — hace de la selección un clip propio en una pista nueva, justo donde
suena ahora.

## Seleccionar al cuadro

- **Arrastra** en la onda o en el espectrograma para seleccionar. El pie de la ventana dice la
  selección al cuadro (`1.23456 s (59259)`). Arrastra un borde para cambiarla, arrastra con
  **Mayús** para extenderla, doble clic para seleccionar todo, clic para un cursor. Escape quita la
  selección.
- La **rueda** acerca y aleja alrededor del puntero; **Mayús + rueda** (o la barra bajo los
  gráficos) desplaza. *Ver todo* y *Acercar a la selección* están en la barra. Con zoom suficiente
  se dibujan las muestras mismas, y el espectrograma se vuelve a dibujar en detalle para lo que ves.
- **Marcador** pone una bandera en el cursor; **Región** nombra la selección; **Puntos de bucle**
  marca la selección como bucle. Se guardan con el archivo, en el proyecto, y se mueven con el audio
  al cortar o pegar. Un clic en una bandera de la regla selecciona hasta la siguiente, o en la barra
  de una región la selecciona. *Quitar marcas* las borra (un paso de deshacer).
- **Reproducir** escucha la selección, o la toma entera (la barra espaciadora la inicia y la para).

## Ediciones

La barra: **Cortar**, **Copiar**, **Pegar** (reemplaza la selección, o entra en el cursor),
**Borrar**, **Recortar**, **Silenciar**, **Invertir**, **Reverso**. Sin selección, Invertir y
Reverso actúan sobre la toma entera. Cortar, Copiar, Pegar y Borrar usan las teclas de siempre
mientras la ventana está abierta. Mientras se hace una edición, la siguiente espera ("Espera: el
editor aún trabaja…").

**Uniones suaves** (activadas por omisión): donde se unen trozos — un corte, un pegado, un silencio
— los bordes van al cruce por cero más cercano y los dos lados se funden en 3 ms, para que nada
chasquee. Desactívalas para un empalme crudo exactamente donde lo pones.

La columna de la derecha:

- **Nivel.** *Aplicar ganancia* (dB); *Normalizar* a un objetivo de **pico**, **RMS** (ambos en
  dBFS) o sonoridad (**LUFS**, integrada; al menos 0,4 s) — de la selección o de la toma entera. Con
  **Techo de pico**, RMS y LUFS se detienen donde los picos llegan al techo (el estado dice qué nivel
  se alcanzó); sin él, el estado avisa cuando los picos pasan de 0 dBFS.
- **Fundidos.** Cinco curvas; *Fundido de entrada* y *de salida* sobre la selección.
- **Reducción de ruido.** Selecciona un tramo de **solo ruido** y pulsa **Aprender ruido** (la
  selección se quita después); **Reducir ruido** limpia la toma entera, o una selección que hagas
  (que se funde con el resto en 10 ms). *Cantidad* al 0 % no cambia nada; *Reducir* es lo máximo que
  se baja cualquier frecuencia; *Fuerza* es cuánto de los picos del ruido atrapa; *Suavizar* ablanda
  el resultado. Para ruido constante — siseo, zumbido, ventiladores.
- **Silencio y DC.** *Quitar silencio* corta los tramos por debajo de *Bajo* que duran más de
  *Mínimo*, dejando *Dejar* milisegundos de cada uno. *Quitar DC* elimina el desplazamiento.
- **Chasquidos.** **Quitar chasquidos** encuentra chasquidos y golpes en la selección (o la toma) y
  reconstruye cada uno con el audio de alrededor; *Sensibilidad* encuentra los más débiles. O
  selecciona un chasquido (hasta 20 ms) y **Reparar selección**.
- **Región espectral.** En **Espectro** o **Ambas**, arrastra un rectángulo sobre el sonido que
  quieres bajar (un silbido, la banda de un zumbido), fija la *Ganancia de la región* y **Aplicar a
  la región**.

Una edición que no cambiaría nada lo dice y no hace archivo; una que no dejaría nada se rechaza.

## Qué conserva el clip

La ganancia, los fundidos, los fundidos cruzados y el reverso se quedan. Las notas del editor de
tono y los marcadores de warp se quedan con las ediciones que no mueven el tiempo del audio
(ganancia, normalizar, invertir, DC, fundidos, silencio, ruido, chasquidos, región) y se descartan —
el estado lo dice — con Reverso, Pegar y las ediciones que cambian la duración. Un clip que mostraba
la toma entera muestra la toma editada entera; los demás clips conservan su lugar. Los clips
enlazados cambian juntos.

## Limpiar

Cada edición guarda un archivo. **Limpiar…** (también en el menú del clip de audio: *Quitar audio sin
usar…*) lista los archivos que ningún clip toca y los archivos del editor a los que nada se refiere,
con sus tamaños, antes de quitar nada; la lista sale del proyecto en un solo paso de deshacer. Los
archivos que pusiste tú en la carpeta nunca se borran, y los que Deshacer aún podría traer de vuelta
se quedan en el disco hasta que reabras el proyecto y vuelvas a limpiar.

## Para los agentes

`edit_audio`, `denoise_clip` y `remove_unused_audio` hacen lo mismo, en un paso de deshacer cada
uno; las ediciones corren en segundo plano. Las posiciones son segundos en el archivo propio del
clip.

## Bueno saber

- Se abren aquí tomas de hasta 30 minutos.
- Grabar directamente en el editor aún no está: graba en el arreglo y abre la toma.
