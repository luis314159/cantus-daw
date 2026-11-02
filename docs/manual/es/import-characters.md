# Importar un personaje

*[Volver al manual](README.md) · [Read in English](../en/import-characters.md)*

Ver también: [Saltator, el bailarín](dancer.md) · [El escenario](stage.md) · [Exportar un video con baile](video.md)

Un bailarín puede llevar un personaje tuyo: uno que dibujaste, uno que compraste, uno
compartido con una licencia libre. Cantus lee cuatro tipos de archivo:

| Formato | Los archivos | Hecho con |
|---|---|---|
| **Lottie** | `.json`, o un paquete `.lottie` | After Effects con Bodymovin, Glaxnimate (libre), Synfig (libre), Lottie Creator, LottieFiles |
| **Hoja de sprites de Aseprite** | un `.json` junto a su `.png` | Aseprite, LibreSprite (libre) |
| **Hoja de sprites de TexturePacker** | un `.json` (hash o array) junto a su `.png` | TexturePacker, free-tex-packer (libre) |
| **DragonBones** | `…_ske.json`, `…_tex.json` y `…_tex.png` juntos | DragonBones, LoongBones |

Los archivos de Rive (`.riv`) aún no se leen. Los personajes 3D (glTF, VRM) están
previstos. Los personajes de Spine y Live2D no se pueden leer: sus licencias piden que cada
usuario compre una.

## Importar uno

1. Abre la ventana del bailarín y su selector de personajes, y pulsa **Importar
   personaje…** — o selecciona la pista del bailarín y elige **Archivo ▸ Importar
   personaje…**.
2. Elige el archivo. En DragonBones, elige el esqueleto (`…_ske.json`); su atlas se busca
   a su lado.
3. Cantus revisa el archivo y muestra **la tarjeta**:
   - el personaje **bailando** al tempo de la canción — señala un movimiento de la lista
     para verlo;
   - su **nombre**, **formato** y **autor**;
   - su **licencia**: lo que dice el archivo o un archivo `LICENSE` a su lado, o, si nada
     lo dice, una lista breve para elegir (*Obra mía*, CC0, CC BY, CC BY-SA, la Lottie
     Simple License, *Otra*, *No lo sé*). Debajo se dice lo que la licencia te pide: citar
     al autor, compartir igual, no vender;
   - **lo que tiene el archivo** (animaciones, cuadros, capas o huesos) y **lo que cuesta
     dibujar** un cuadro en un escenario de 600 × 600 (el presupuesto de Cantus es 1,5 ms);
   - **lo que no se usó**, cada cosa con palabras — por ejemplo las expresiones de un
     Lottie, que son scripts y nunca se ejecutan (suenan sus fotogramas clave).
4. **Asigna los movimientos.** Cada uno de los dieciséis movimientos de Cantus — *Reposo,
   Rebote, Paso a la izquierda, Paso a la derecha, Vaivén, Palmada, Saludo, Salto, Pirueta,
   Patada, Shuffle, Señalar, Brazos arriba, Twist, Reverencia, Congelado* — toca una
   animación del archivo. Cantus la sugiere por los nombres, en inglés y en español (un
   marcador o etiqueta llamado `clap`, `palmada` o `Clap_01` es *Palmada*); cada sugerencia
   dice "(sugerido)" y cada fila se puede cambiar. Un movimiento en *Ninguna* toca un
   movimiento afín. **Tiempos**: cuántos tiempos llena la animación al tempo de la canción
   (1, 2, 4 u 8).
5. **Reproducción**: *Píxeles nítidos* para pixel art; *A su propia velocidad* lo toca a sus
   cuadros por segundo en vez de al compás.
6. Pulsa **Usar**. El archivo — con sus imágenes y su archivo de licencia — se copia en la
   carpeta **Characters** del proyecto y el bailarín lo lleva puesto. Un *Deshacer* se lo
   quita.

Las animaciones con nombre de un **ánimo** (`happy`, `sleepy`, `feliz`…) suenan en lugar de
*Reposo* mientras ese ánimo esté elegido.

## Cambiar sus movimientos después

En la ventana del bailarín, **Editar movimientos…** vuelve a abrir la tarjeta con tus
elecciones. **Guardar** las conserva como una copia nueva del personaje, y **cada bailarín
de este proyecto** que lo lleva la sigue — un *Deshacer* los devuelve a todos a los
movimientos de antes (esa copia sigue ahí).

## Dónde vive

- `Characters/<nombre>-<código>/` junto al archivo del proyecto guarda la copia y
  `character.json` (su formato, su licencia, tu asignación). El código sale de todo lo que
  tiene — los archivos y tus elecciones — y una copia nunca se cambia después, así que dos
  proyectos guardados en una carpeta comparten una copia solo si es exactamente la misma.
  Mover o comprimir la carpeta del proyecto lo lleva consigo.
- En un proyecto aún sin guardar, la copia espera en la carpeta propia de Cantus; el primer
  guardado la lleva junto al proyecto. *Guardar como* en otra carpeta también la lleva.
- Si falta el archivo de un personaje, se dibuja el personaje por defecto de Cantus y la
  ventana del bailarín lo dice.

## Seguro por diseño

Un archivo de personaje son datos, nunca un programa: no se ejecuta ningún script, no se
descarga nada, no se lee nada fuera de la carpeta del archivo, y cada tamaño se revisa antes
de darle memoria. Un archivo dañado u hostil se rechaza con palabras que dicen por qué.
