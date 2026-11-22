# Fides, el resonador

*Cantus 0.1.0-beta.17 · [Read in English](../en/fides.md) · volver al [manual](README.md)*

Fides suena como suenan los instrumentos acústicos: algo **excita** un objeto — un dedo lo
pulsa, una púa lo engancha, un arco lo frota, una baqueta lo golpea, un ruido lo sopla — y el
objeto **resuena**: una cuerda, una barra, una lengüeta, una placa, una membrana, un tubo, una
campana, un cristal. Un **cuerpo** irradia lo que suena. Nada está grabado: cada nota se calcula
con la física mientras tocas, así que responde a cuán fuerte, dónde y cuánto tiempo la tocas, y
nunca repite una grabación. Marimba, vibráfono, kalimba, steel pan, arpa, guitarra de nylon, un
chelo con arco y cristal son sus primeros presets; la familia va más allá (campanas, gong,
timbales, un salterio, una flauta de pan, colchones con arco, lluvia sobre cristal).

Su nombre es el latín de las cuerdas de una lira — un instrumento de cuerda — y de la fe.

**Añádelo** desde el selector de dispositivos (el **+** sobre las cabeceras de pista, sus estantes
*Teclados* y *Otros instrumentos*). Un Fides nuevo toca una cuerda de nylon pulsada en el cuerpo de una guitarra.

## La vista Tocar

**El dibujo** (a la izquierda) es el instrumento tal como está ajustado: el resonador dibujado
como un plano — una cuerda entre su puente (a la izquierda) y su cejilla, una barra con los nodos
de su primer modo punteados, una lengüeta desde su sujeción, una placa, un parche visto desde
arriba con el radio por el que se golpea, un tubo (cerrado a la derecha cuando lo es), una
campana, un cristal, un steel pan — el excitador dibujado donde toca y el anillo del micrófono
donde se escucha un objeto modal. **Arrastra sobre él** para mover dónde se toca: los parciales
de abajo lo siguen.

Debajo, **los parciales**: cada uno una línea en su frecuencia sobre la fundamental (×1, ×2,
×4 … en un eje logarítmico), tan alta como el golpe la excita donde se toca. Los de una cuerda son
sus armónicos con los huecos que corta la posición (tócala a un quinto de su largo y falta cada
quinto armónico); los de una barra, sus modos estirados (×2,76, ×5,40 …); los de una barra de
marimba están afinados a dos octavas y a tres octavas y una tercera. Mientras suena una nota se
encienden con lo que suena la voz más reciente.

**El excitador** (a la derecha): sus seis tipos, las perillas de ese tipo y dónde toca.

| Excitador | Qué es | Sus perillas |
|---|---|---|
| **Pulsar** | la yema de un dedo al soltar: los parciales caen como 1/n desde el primero | Contacto (ms: una yema 0,6–1,5, la carne del pulgar 2–3), Tono por velocidad, Ruido del golpe (una uña), Tono, Nivel |
| **Púa** | un plectro: un cuarto del contacto, con su roce | las mismas |
| **Arco** | un arco que frota, por una curva de fricción: la cuerda se pega y se suelta (el movimiento de Helmholtz) | Presión, Velocidad del arco, Ataque, Levantar, Nivel |
| **Baqueta** | el pulso de fuerza de un resorte de ley de potencia (el contacto de Hunt–Crossley, p = 2,5): más fuerte es más corto y más brillante | Contacto (una baqueta de lana 1,5–3 ms, de goma 0,5–1, de latón 0,2), Tono por velocidad, Ruido del golpe (el fieltro), Tono, Nivel |
| **Ruido** | ruido filtrado con su propia envolvente — aliento, viento, lluvia (Densidad por debajo del 100 % lo vuelve granos) | Color, Anchura, Densidad, Ataque, Liberación |
| **Entrada** | el audio de otro canal en el resonador, mientras se mantiene una nota: se elige en *Entrada de*, en su tarjeta, como la clave de un compresor (cualquier canal después de su fader, un sidechain — un bus de batería que hace sonar una marimba, una voz que mueve una cuerda); sin ninguno elegido no suena, y lo dice | Nivel, Ataque, Levantar, Tono |

**Posición** es dónde se toca, como fracción del largo desde el extremo que el dibujo pone a la
izquierda: el puente de una cuerda, el extremo de una barra, la punta de una lengüeta, el borde
de una membrana, el labio de una campana. Cerca del extremo es delgado y brillante; el centro es
redondo. **Ruido** pone el generador de ruido bajo cualquier excitador (la resina de un arco, el
fieltro de una baqueta).

**El resonador** (abajo): doce tipos.

| Resonador | Qué es |
|---|---|
| **Cuerda** | una cuerda de guía de onda (las matemáticas de afinación de Gravis): su pérdida concentrada en el puente, su rigidez como pasa-todos — *Inarmonicidad* estira sus parciales como los de un piano |
| **Barra** | una barra libre en sus dos extremos (la de un xilófono sin afinar, la de un glockenspiel): modos en 1, 2,76, 5,40, 8,93 … |
| **Barra de marimba** | una barra rebajada para afinar sus parciales a 1 : 4 : 9,2 |
| **Lengüeta** | una barra sujeta por un extremo (la de una kalimba, una caja de música, una celesta): 1, 6,27, 17,5 |
| **Placa** | los modos de una placa rectangular (la de un gong, una lámina de trueno) |
| **Membrana** | un parche ideal: los ceros de Bessel (1, 1,59, 2,14, 2,30 …) |
| **Parche** | el parche de un timbal, afinado por el aire bajo él (1, 1,5, 2, 2,44 …) |
| **Tubo** | un tubo cerrado por un extremo (el de una flauta de pan): armónicos impares |
| **Caño** | un tubo abierto en sus dos extremos: todos los armónicos, sus graves irradiados menos |
| **Campana** | los parciales de una campana de iglesia afinada (hum, prima, tercera, quinta, nominal …) |
| **Cristal** | los modos de anillo de una copa de vino |
| **Steel pan** | una nota de steel pan: fundamental, octava y duodécima afinadas |

**Caída** es cuánto suena la fundamental (T60, el tiempo en bajar 60 dB). **Material** es cómo
mueren los agudos: los de la madera de inmediato, los del cristal y el metal tanto como la
fundamental. **Inarmonicidad** estira (o comprime) los parciales: en una cuerda, en 0 son armónicos, por encima los de una cuerda rígida (los de un piano al 100 %), por debajo se recogen otro tanto — cada paso los mueve. **Escucha** es desde dónde se
oye un objeto modal (el anillo del micrófono).

**El cuerpo**: Ninguno, Guitarra (su modo de aire cerca de 98 Hz y los de su tapa), Violín (sus
modos característicos y la colina del puente cerca de 2,5 kHz), Chelo, Arpa (su caja y su tabla),
Caja (la de una kalimba), Casco (la falda de un tambor o de un steel pan) — con su **mezcla**, su
**tamaño** (un cuerpo mayor baja sus modos) y su **resonancia**.

**Salida**: Liberación (qué tan rápido se apagan los resonadores al soltar: 60 s los deja sonar
como las barras de una marimba), Anchura (las voces repartidas en el estéreo por tecla) y Ganancia.

## La vista Editar

Cada parte como una tarjeta: el dibujo y el excitador; el **Resonador A** y el **Resonador B** (la
luz de B lo enciende), cada uno con todas sus perillas — *Caída por tecla* (cuánto más rápido
muere una nota una octava arriba), *Parciales* (cuántos modos toca un objeto modal: menos es más
ligero para la CPU), *Afinación* y *Fino* (B frente a A), *Seguimiento de tecla* (0 % mantiene la
altura del resonador sea cual sea la tecla: un tambor); el **acoplamiento** — *Paralelo* (el
excitador en los dos) o *A hacia B* (A excita a B, como la barra de una marimba al tubo bajo
ella) y lo que se oye de cada uno; el **arco y MPE**; el **ruido** con su envolvente; el
**cuerpo**; el **vibrato** (profundidad en cents, frecuencia, un retardo antes de empezar) y el
**trémolo** (su fase el reloj de la canción, como el motor de un vibráfono); la **salida** con sus
cortes de graves y agudos; la **voz** — Polifónico, Mono o Legato, *Golpear de nuevo* (encendido:
una tecla tocada mientras suena golpea el mismo resonador, como una barra o una cuerda reales;
apagado: una voz nueva), Voces, Portamento, Rango de bend, Octava, Transponer, Fino, La4,
Velocidad, Liberación y la Semilla de su ruido.

## Tocarlo

- **La velocidad** decide cuán fuerte es el golpe: más fuerte y, por *Tono por velocidad*, un
  contacto más corto (más brillante), como el de una baqueta más rápida.
- **MPE**: la **presión** de una nota aprieta el arco (*Presión MPE*: cuánto; con Ruido o Entrada
  sube el nivel), su **deslizamiento** mueve dónde se toca (*Deslizamiento MPE*), su **bend** la
  dobla. El aftertouch, la rueda y la expresión de un controlador sin MPE son fuentes de
  modulación.
- **La afinación** es exacta: los lazos y filtros de la guía de onda se resuelven para la
  fundamental, y los modos se fijan por sus ángulos — medida dentro de 0,002 cent con cualquier
  bend.
- **El pedal** sostiene las notas soltadas, como la barra apagadora de un vibráfono.

## Los presets

Marimba · Vibráfono · Kalimba · Steel pan · Arpa · Guitarra de nylon · Chelo con arco · Cristal
golpeado · Armónica de cristal · Xilófono · Glockenspiel · Celesta · Caja de música · Campana ·
Timbales · Pandero · Salterio · Banjo · Pizzicato · Colchón de cuerdas con arco · Vibráfono con
arco · Flauta de pan · Lluvia sobre cristal · Gong · Pulsado de cristal. Cada uno tiene cuatro
macros: **1** cuán fuerte (el contacto, la presión del arco, el color del ruido), **2** la caída,
**3** el cuerpo, **4** dónde se toca.

## Lo que cuesta

Una cuerda pulsada o frotada son unas pocas decenas de operaciones por muestra; un objeto modal,
una multiplicación compleja por parcial (48 como mucho). Sus voces se reparten entre los hilos
del motor con los bits de uno (ADR 0038). La perilla *Parciales* es el precio de una voz modal.
