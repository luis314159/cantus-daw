# Separar stems

*[Volver al manual](README.md) · [Read in English](../en/stems.md)*

Cantus puede dividir una grabación en sus partes — **batería, bajo, voces y el resto**, o
con el modelo de 6 stems también **guitarra y piano** — con un modelo que corre **en tu
ordenador**. No se sube nada a ninguna parte.

## Separar un clip, una pista o una selección

- Clic derecho en un **clip de audio** ▸ *Separar stems…* (con varios clips de una pista
  seleccionados, se separan juntos; los de varias pistas se separan pista por pista, porque
  las partes de cada pista van bajo un bus propio), o
- clic derecho en el **nombre de una pista** ▸ *Separar stems…* (todos sus clips de audio).

Elige **4 stems** o **6 stems**, **Rápida** o **La mejor** (cuatro pasadas promediadas: algo
más limpia, cuatro veces más lenta) y, en un Mac, *Usar la tarjeta gráfica cuando se
pueda* (en Windows y Linux el panel dice que no está disponible con el runtime que
descarga Cantus: el modelo corre en el procesador). Pulsa **Separar**. El panel muestra el avance; *Cancelar* la detiene (no queda
nada), *Ocultar* cierra el panel y la separación sigue.

¿Qué tan buena es? En las canciones de prueba que usan los investigadores (MUSDB18-HQ), el
modelo de 4 pistas logra 8.80 dB de SDR en promedio (batería 10.05, bajo 9.78, otros 6.42,
voces 8.93) — los números que publicaron sus autores, medidos de nuevo en Cantus. Las
mezclas electrónicas densas son las más difíciles.

Al terminar:

- cada parte es una **pista de audio nueva justo debajo de la pista separada**, llamada
  *Batería*, *Bajo*, *Otros*, *Voces*…, cada una en su canal del mezclador;
- los canales van a **un bus nuevo**, en un grupo del mezclador con el nombre de la pista
  (*Stems de Mix*), y el bus va adonde iba la pista original — la canción suena igual, ahora en
  partes;
- cada clip nuevo está **exactamente donde está su clip original**, muestra a muestra, con
  la misma ventana, fundidos, ganancia y warp;
- un clip que habías silenciado sigue silenciado en cada parte, y se conservan los fundidos
  cruzados y las costuras de las tomas compuestas;
- las partes suenan al **nivel del original**: sus canales empiezan al nivel que compensa
  la ley de paneo, así que silenciar el original y oír las partes suena igual (si cambias
  la ley de paneo después, se ajustan con ella, en el mismo paso de deshacer);
- los **clips originales quedan silenciados**, no se borran;
- es **un solo paso de Deshacer**.

Los archivos nuevos van a la carpeta *Audio/Stems* del proyecto. Sus clips llevan una
pequeña insignia **IA**: cada archivo recuerda qué modelo lo hizo, su versión y licencia,
cuándo y de qué archivo (se guarda en el proyecto).

Sirve un archivo a cualquier frecuencia de muestreo: se convierte a los 44,1 kHz del modelo
y las partes vuelven a la frecuencia y la duración del archivo.

## Los modelos (*Ajustes ▸ Modelos*)

La primera vez, Cantus muestra el tamaño del modelo, su **licencia** y la del runtime de
inferencia, quién lo hizo y con qué se entrenó. *Aceptar la licencia y descargar* los baja:

| Modelo | Partes | Descarga | Licencia |
|---|---|---|---|
| HT-Demucs — 4 stems (Meta) | batería, bajo, otros, voces | 84 MB + el runtime | MIT |
| HT-Demucs — 6 stems (Meta) | y guitarra, piano (su piano es flojo, dicen sus autores) | 55 MB + el runtime | MIT |

El runtime es **ONNX Runtime** de Microsoft (MIT; 15 MB en Windows, 11 MB en Linux, 37 MB
en Apple silicon), se descarga una vez para todos los modelos y con su telemetría apagada.
Cantus no aloja los pesos: descarga el archivo de Meta, comprueba cada byte y reconstruye
el modelo a partir de él. Una descarga cortada sigue donde se quedó.

*Quitar* borra un modelo (y el runtime con el último); espera mientras corre una
separación. Antes de empezar, Cantus comprueba que haya espacio en el disco (*La mejor*
necesita espacio para cuatro pasadas) y lo dice si no lo hay. Una grabación más larga de lo
que cabe en un archivo WAV como stems (4 GB: unos 45 minutos a 192 kHz) se rechaza con un
aviso. Abrir otro proyecto cancela la separación en curso.

## Cuánto tarda

Alrededor de un tercio de la duración de la canción en el procesador de un portátil
reciente (una canción de 3 minutos en unos 65 segundos), cuatro veces más con *La mejor*. El
modelo corre en un proceso auxiliar propio, `cantus-ml`: si falla, Cantus lo dice y sigue.

## Para agentes de IA

Un agente conectado a Cantus puede llamar a `separate_stems` (una pista o clips; 4 o 6
partes; rápida o la mejor). Te pregunta antes, como cualquier edición, y el modelo tiene que
estar instalado — su licencia la aceptas tú.
