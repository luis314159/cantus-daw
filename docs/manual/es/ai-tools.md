# Lo que un agente de IA puede hacer en Cantus

*[Volver al manual](README.md) · [Read in English](../en/ai-tools.md)*

Cuando dejas que un asistente como Claude se conecte a Cantus (*Ajustes ▸ IA*), trabaja en tu
proyecto mediante **herramientas**: las mismas ediciones que haces con el ratón, cada una
**un solo paso de Deshacer**, llamado "IA: …" en el historial. Leer nunca pregunta; cambiar
pregunta en la ventana de Cantus salvo que hayas permitido las ediciones; quitar o sustituir
tu material siempre pregunta; los archivos se rechazan hasta que los permites, y entonces se
preguntan. Los nombres de tus pistas, clips y marcadores se le muestran al agente como datos,
nunca como instrucciones.

## Las herramientas

| Grupo | Herramientas | Qué hacen |
|---|---|---|
| Leer | `get_project`, `get_track`, `get_clip_notes`, `get_mixer`, `get_device`, `get_history`, `get_selection` | La canción, una pista y sus clips, las notas o los puntos de automatización de un clip, el mezclador, los valores de un dispositivo, el historial, lo que tienes seleccionado y dónde está el cursor de reproducción |
| La canción | `set_tempo`, `set_tempo_point`, `set_song`, `set_marker` | Tempo y cambios de tempo, el compás (y sus cambios), la tonalidad, el bucle, el metrónomo, los marcadores |
| Transporte | `transport` | Reproducir, parar, situar; no es un paso de deshacer |
| Pistas | `add_track`, `set_track`, `set_instrument`, `set_instrument_param` | Una pista de instrumento nueva con su propia franja de mezcla; nombre, color, nivel, panorama, silencio, solo, orden, ruteo; cambiar el instrumento; sus parámetros por clave en sus unidades |
| Clips y notas | `add_clip`, `add_notes`, `replace_notes`, `edit_notes`, `set_clip`, `duplicate_clip` | Clips y notas; transponer, cuantizar, humanizar, mover, velocidad, duración, legato; mover, redimensionar, repetir y copiar clips |
| Batería | `write_drum_pattern` | Filas de pasos (`x...x...`) por pad, con swing y acentos |
| Sesión | `set_session` | Escenas, ranuras, modos de lanzamiento y acciones de seguimiento |
| Mezclador y efectos | `add_strip`, `set_strip`, `set_send`, `add_effect`, `set_effect`, `replace_effect` | Buses y retornos, faders, anchura, el ecualizador de la franja, envíos; cualquier efecto de Cantus en cualquier franja o en el máster, con sus parámetros |
| Movimiento | `automate`, `replace_automation`, `add_modulator`, `set_modulator` | Clips de automatización con las formas de FL, en las unidades del parámetro; LFO, envolventes, seguidores (sidechain), secuencias de pasos, aleatorios |
| Música | `lint_music`, `write_part`, `voice_chords`, `pedal_from_chart` | El corrector musical; una parte entera sobre un cifrado; acordes dispuestos; el pedal derecho de Chorda a partir del cifrado |
| Unidades | `list_units`, `get_unit`, `list_presets`, `apply_preset` | El catálogo de todos los instrumentos y efectos; los preajustes (cargar uno sustituye los ajustes de un dispositivo: siempre pregunta) |
| Tomas | `comp_takes`, `tidy_takes` | Componer con las tomas grabadas; borrar una composición, aplanarla, borrar las tomas sin uso |
| Exportar | `export_audio`, `export_project` | WAV, FLAC, MP3, Ogg, pistas separadas; MIDI y DAWproject, solo en la carpeta del proyecto o en una carpeta que hayas permitido |
| Quitar | `remove_items` | Pistas, clips, notas, efectos, franjas, envíos, moduladores, marcadores, puntos de tempo, escenas, clips de sesión; siempre pregunta |

Un agente nunca toma tu material en silencio: dibujar automatización sobre puntos tuyos,
una parte donde están tus notas o un preajuste sobre tus ajustes preguntan antes (o se
rechazan).

Las posiciones se dan en **pulsos** (negras desde el principio) o como **compás:pulso**
(`9:1`, `9:2.5`), y Cantus responde con las dos. Los valores van en la unidad de cada
parámetro (dB, Hz, ms, semitonos…); un valor fuera de su rango se rechaza diciendo el rango,
nunca se cambia a escondidas.

El agente tiene además las herramientas propias del servidor (ADR 0081): esperar o cancelar un
trabajo largo, agrupar un turno de su trabajo en un solo paso de deshacer
(`begin_change`/`end_change`), deshacer su último paso y listar lo que hizo.

## El catálogo de unidades

`list_units` y `get_unit` (y el recurso `cantus://units`) describen todos los instrumentos y
efectos de Cantus: la clave de cada parámetro, su nombre, su grupo (*Banda 4*), unidad, rango,
valor por defecto y escala, los nombres de las opciones de un selector, los preajustes de
fábrica y lo que ajustan, el preajuste con el que un efecto entra al insertarlo, su latencia y
para qué sirve la unidad. Se genera de las mismas tablas que usan los mandos, así que siempre
está completo y es exacto.

## El corrector musical

`lint_music` lee las notas como escucha un revisor, y dice dónde y por qué:

- **una nota que sigue sonando en el acorde siguiente** sin pertenecerle: un *error* cuando
  queda a un semitono del acorde nuevo (el caso clásico: el La♭ de un acorde sonando aún sobre
  el Sol de Mi♭/Sol);
- **un choque**: una nota sostenida bajo un acorde a un semitono *por encima* de una de sus
  notas (la oncena sobre la tercera de un acorde mayor, la sexta menor sobre la quinta); una
  novena, una trecena o una séptima mayor son color;
- **una nota doble**: la misma altura escrita dos veces a la vez en una pista;
- **fuera de la tonalidad** y del acorde (*información*: a menudo una nota de paso, a veces un
  error);
- **fuera del instrumento**: por debajo del La0 de un piano, una nota de batería sin pad.

Dale los acordes (el cifrado) para las comprobaciones armónicas; sin ellos comprueba las notas
dobles, la tonalidad y el registro. No cambia nada.

## Cifrados: acordes por compases

Los escritores y el corrector leen los acordes como los escribe una partitura de jazz: un
compás entre barras `|`, y los acordes de un compás se lo reparten por igual:

```text
Fm9 | Dbmaj7 | Abadd9 | Eb Eb/G
C . . G7 | %                       Do tres pulsos, Sol7 uno; luego el compás otra vez
```

`.` alarga el acorde anterior una parte más, `%` repite el compás, `N.C.` es sin acorde. Vale
cualquier símbolo (en notación anglosajona: C es Do): `C`, `Cm`, `C7`, `Cmaj7` (`CM7`, `CΔ`),
`Cm7b5` (`Cø`), `Cdim7`, `Csus4`, `C6/9`, `Cadd9`, `C9`, `C13`, `C7b9`, `C7#11`, `C7alt`, y
bajos con barra (`Eb/G`).

`write_part` escribe una parte sobre un cifrado (acordes plaqué, pulsos, arpegios o una línea
de bajo, a un ritmo fijo o con un patrón de pasos `x..x..x.`, con swing y un toque humano),
dispuesta para moverse lo menos posible de acorde a acorde, y **ninguna nota suena dentro del
acorde siguiente**. `voice_chords` escribe los acordes sostenidos; `pedal_from_chart` dibuja el
pedal derecho de Chorda: arriba en cada cambio y abajo otra vez un instante después, para que
cada acorde resuene y nunca el anterior.

## Indicaciones

Un cliente ofrece peticiones listas: *Explícame la mezcla* (no cambia nada), *Escribe una
parte*, *Arregla una introducción*, *Ordena el proyecto* (pregunta antes de quitar nada) y
*Enséñame esta unidad*, en el idioma de tu interfaz.
