# Exportar un video con baile

*[Volver al manual](README.md) · [Read in English](../en/video.md)* · Ver también: [el
bailarín](dancer.md) · [el escenario](stage.md) · [importar personajes](import-characters.md)

*Archivo ▸ Exportar video…* hace un video de tus bailarines con la canción: un clip vertical
para TikTok, Reels o Shorts, uno cuadrado, uno panorámico para YouTube, o una capa
transparente para un editor de video. Lo que ves en la vista previa es lo que tendrá el
archivo, imagen a imagen: los bailarines se mueven exactamente como en pantalla, con su
automatización.

## La ventana

- **Vista previa** (izquierda): la imagen en cualquier momento de la canción. Arrastra la
  barra de abajo para recorrerla. El fondo y el título se dibujan como quedarán en el
  archivo; un fondo transparente se ve como un tablero. Para mostrar a los bailarines como quedarán en el
  archivo, la vista previa renderiza unos segundos de la canción alrededor de ese momento,
  con automatización, notas y moduladores (los plug-ins de otros fabricantes quedan fuera de
  ella; en el archivo están).
- **Qué bailarines**: todos los del proyecto salvo uno que no se muestra en ningún sitio
  (oculto). Un personaje importado que aún se está leyendo se espera ("Cargando los
  personajes…"); uno que no se puede leer detiene la exportación y dice cuál.
- **Formato**:
  - **MP4** — H.264 y AAC, el formato que aceptan todas las plataformas (YouTube, TikTok,
    Instagram, Facebook, X). Se hace en segundos.
  - **WebM** — AV1 y Vorbis, libre de regalías; lo aceptan YouTube y TikTok. Tarda más en
    hacerse: la estimación bajo las opciones dice cuánto.
  - **MOV + alfa** — imágenes PNG sin pérdidas con fondo transparente, para Final Cut,
    Kdenlive, Shotcut y otros editores.
  - **Imágenes PNG** — una carpeta de archivos PNG numerados y la canción como WAV: la
    importa cualquier editor.
- **Forma**: vertical 1080 × 1920, cuadrado 1080 × 1080, panorámico 1920 × 1080, u otro
  tamaño (de 720 × 1280 a 4K, y el 1080 × 1350 de Instagram).
- **Imágenes/s**: 30 o 60 por segundo. **Calidad**: Buena, Alta (la que recomiendan las
  plataformas) o Máxima.
- **Fondo**: el color del escenario del tema, transparente (solo MOV y PNG), un croma
  (verde, azul o magenta) o negro. Un croma en MP4 o WebM se difumina un poco en los bordes
  del personaje (esos formatos guardan el color a media resolución): para un croma limpio,
  MOV + alfa.
- **Título**: el nombre y el tempo de la canción al pie de la imagen.
- **Rango**: toda la canción, la selección de tiempo o el bucle.
- **Codificadores**: qué codificador hace la imagen y el sonido en este equipo, dicho antes
  de empezar.

El pie dice cuánto debería tardar y cuánto ocupará el archivo; durante la exportación,
cuánto lleva y cuánto falta. *Detener* la para y no deja nada a medias.

## Sincronizado, a la muestra

Cada imagen se dibuja en su momento exacto y el sonido empieza en la misma muestra: el paso
de cada tiempo cae en su tiempo en cualquier reproductor y plataforma. Una excepción que
quizá notes al comparar muy de cerca: **el reproductor de Windows (Reproductor multimedia,
Películas y TV) reproduce el sonido de cualquier MP4 con AAC unos 21 ms tarde** — también
los de otros programas — porque ignora la instrucción que recorta el inicio del
codificador. Lo que subas va exacto.

## De dónde salen los codificadores

Cantus no incluye un codificador de video propio (H.264 y AAC se licencian por copia):

- **Windows** usa los codificadores H.264 y AAC del propio Windows. Las ediciones *Windows
  N* no los traen: añade el *Media Feature Pack* gratuito (Configuración ▸ Aplicaciones ▸
  Características opcionales), o usa OpenH264 de Cisco (abajo).
- **macOS** usa los codificadores del propio macOS (VideoToolbox y AudioToolbox).
- **Linux** usa **OpenH264 de Cisco**: actívalo en la ventana y pulsa *Descargar de Cisco*
  (una vez, unos 0,6 MB). Es la compilación de Cisco, comprobada byte a byte, y rige su
  licencia (*OpenH264 Video Codec provided by Cisco Systems, Inc.*); puedes desactivarlo
  cuando quieras. El sonido es AAC si está instalado FFmpeg o GStreamer (con gst-libav) —
  la ventana dice cuál —, y si no MP3, que suena en todas partes pero Instagram rechaza:
  instala FFmpeg (`sudo apt install ffmpeg`) para tener AAC.
- **WebM, MOV y PNG** no necesitan nada: los hace Cantus en todos los sistemas.

## Para agentes

Un asistente conectado por MCP puede exportar un video con `export_video` (formato, forma,
imágenes por segundo, calidad, fondo, título, rango, archivo) y seguirlo con
`get_video_export`. Como toda herramienta que escribe archivos, se rechaza hasta que
permitas archivos en *Ajustes ▸ IA*, y siempre se te pregunta.
