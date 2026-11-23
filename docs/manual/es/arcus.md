# Arcus, la cuerda frotada

*Cantus 0.1.0-beta.18 · [Read in English](../en/arcus.md) · volver al [manual](README.md)*

Arcus es un chelo y un contrabajo que **se tocan, no se reproducen**: nada está grabado. Una
cuerda con su longitud, su tensión y su masa reales la frota un arco con una fuerza en
newtons, a una velocidad y a una distancia del puente; la fricción de la resina sujeta la
cuerda y la suelta cientos de veces por segundo (el movimiento de Helmholtz, pegar y
deslizar), y el cuerpo del instrumento radia lo que la cuerda canta. Así una nota responde a
cómo la tocas — suave o fuerte, ligada o con arco nuevo, cerca del puente o sobre el diapasón
— y no hay dos notas que sean la misma grabación.

Su nombre es el latín del arco.

**Añádelo** desde el selector de dispositivos (el **+** sobre las cabeceras de pista, *Otros
instrumentos*). Un Arcus nuevo es un chelo que liga las notas que solapas.

## Tocarlo desde un teclado

Arcus **nunca se queda en silencio** por falta de un controlador: la velocidad sola lo toca.

- **La velocidad** fija la dinámica de la nota (de pianísimo a fortísimo: lo rápido que va el
  arco, lo fuerte que aprieta y lo cerca del puente que toca, a la vez, como hacen los
  músicos) y su ataque (una nota más fuerte acelera el arco antes y aprieta más — una suave
  habla un poco más tarde, como una cuerda real, una cuerda grave más tarde que una aguda).
- **Solapa dos notas** y la segunda se **liga**: el arco sigue, el dedo se mueve — al
  instante, sin retardo añadido (el tiempo de *Dedo* es el movimiento del dedo, 3 ms). **Deja
  un hueco** y se toca con **arco nuevo** en la otra dirección (détaché), la cuerda aún
  sonando. El **Modo** dice cuál: *Legato* (los solapes ligan — por defecto), *Mono* (cada nota
  con arco nuevo), *Poli* (cada nota su cuerda: dobles cuerdas).
- **CC1** (la rueda de modulación) mueve la dinámica de forma continua en cuanto la mueves —
  crece una nota sostenida y el arco acelera y aprieta con ella. **CC11**, el **soplo** (CC2)
  y la **presión** también (*Dinámica desde*: *Auto* toma el último que moviste de CC1, soplo
  y presión, con CC11 escalándolo; o nombra uno; o *Velocidad* sola).
- **CC21** fija la profundidad del vibrato (o CC2, o la presión: *Vibrato desde*). Hasta que
  se mueve, el vibrato llega solo, un poco después de empezar la nota.
- **CC65** enciende el portamento (un deslizamiento entre notas ligadas); si no, no hay —
  solo cuando lo pides (*Portamento*, en ms).
- **MPE**: la presión de una nota es su propia dinámica; su deslizamiento mueve dónde toca el
  arco, hacia el puente o el diapasón; su afinación la dobla.

La **articulación** propia de una nota (el carril de articulación del piano roll) se oye como
dice: *sostenido* le da arco nuevo, *legato* la liga, *staccato* para el arco sobre la cuerda,
*spiccato* lo hace saltar, *pizzicato* la pulsa, *trémolo* agita el arco; su **arco** (abajo
o arriba) elige la dirección. Sin ella, el arco alterna solo (*Arco*: *Alterno*, o siempre
abajo, o siempre arriba), y empieza abajo tras un silencio.

## La vista Tocar

**El dibujo** (izquierda) es la cuerda tal como se frota: el puente a la izquierda, el dedo
donde la nota pisa la cuerda, el diapasón y la cejilla; **el arco** cruzándola en su punto de
contacto, tan grueso como aprieta, su flecha tan larga como se mueve y en su dirección.

Debajo, **el diagrama de Schelleng**: la fuerza del arco (arriba) contra su distancia al
puente (a lo ancho), ambas en escala logarítmica. La región ámbar es donde la cuerda mantiene
su movimiento de Helmholtz — el tono limpio, que canta. Encima el arco aprieta demasiado para
su velocidad (un crujido áspero); debajo, demasiado poco (un sonido de superficie, soplado,
silbante); la región se estrecha hacia el puente, como la dibujó Schelleng en 1973. El punto
granate es el arco ahora. **Arrastra el punto** para mover el contacto (a lo ancho) y la
fuerza (arriba y abajo).

**La tarjeta del instrumento** (derecha): *Chelo* o *Contrabajo*; *Arco*, *Pizzicato* o
*Trémolo*; y los cuatro números que gira un músico — **Dinámica** (la dinámica sin
controlador ni parte de la velocidad), **Contacto** (−1 sobre el diapasón, sul tasto, … +1
cerca del puente), **Fuerza** (una octava más ligera o más pesada que la de la dinámica),
**Vel. arco**.

Debajo: **Vibrato** (profundidad, frecuencia, demora), **Cuerpo y cuerda** (cuánto cuerpo se
oye, su tamaño, cuánto resuena) y **Salida** (*Resonancia*: cuánto suena una nota cuando el
arco se ha levantado y la tecla está arriba; la ganancia).

## La vista Editar

Cada parte con su tarjeta: la **dinámica** y lo que la mueve; el **arco** (su contacto,
fuerza, velocidad, ataque, alzada, el tiempo del cambio de arco, el ruido de las cerdas, el
deslizamiento MPE, y la ley de fricción: *Hiperbólica* — la curva clásica de la resina, por
defecto — o *Térmica*, un modelo en el que el agarre de la resina depende de lo caliente que
esté el contacto, una opción para comparar); la **dirección** del arco, la frecuencia del
**trémolo** y el punto, el dedo y la caída del **pizzicato**; el **vibrato** (su demora y su
entrada, su vida — cuánto varían su frecuencia y su profundidad de ciclo en ciclo — y su
controlador); el **cuerpo y la cuerda** (la caída de la cuerda, la resonancia); la **salida**
(cortes bajo y alto); la **voz** (modo, voces, tiempos de dedo y portamento, rango de bend,
octava, transposición, fino, la semilla de sus pequeñas variaciones).

## De qué está hecho

- **Las cuerdas** — Do2 Sol2 Re3 La3 del chelo sobre 690 mm, Mi1 La1 Re2 Sol2 del contrabajo
  sobre 1 060 mm (la afinación de orquesta), cada una con su tensión (los márgenes de los
  fabricantes), así que cada una tiene su impedancia bajo el arco; una cuerda pisada suena
  más corta que una al aire. Una tecla suena en la cuerda más aguda cuyo tono al aire está en
  ella o por debajo.
- **El arco** — la unión de fricción de McIntyre, Schumacher y Woodhouse (1983), resuelta
  exactamente en cada muestra, con la curva de la resina de la literatura; la **torsión** de
  la cuerda bajo el arco forma parte del contacto, que es lo que deja empezar limpio el
  movimiento de Helmholtz; el **ruido de las cerdas** es una capa propia (*Ruido arco*).
- **El cuerpo** — el cuerpo de chelo de Fides (sus modos de firma); el del contrabajo es el
  del chelo **escalado a su tamaño** (no se encontró un cuerpo de contrabajo medido: dicho
  claramente).
- **La afinación** — una cuerda frotada no suena la altura de su longitud libre (la fricción
  la mueve); Arcus la corrige con una tabla medida en el propio modelo, así que una nota
  frotada queda a menos de un cent de su tecla; una pulsada, a una décima.
- **El vibrato** mueve la longitud de la cuerda; el volumen de los parciales se mueve al
  barrer las resonancias del cuerpo — que es como se oye un vibrato real (Mellody y
  Wakefield, 2000).
- **La afinación** del proyecto (Proyecto ▸ Afinación) le llega como a todo instrumento con
  altura.

## Presets

Veinticuatro, del chelo y del contrabajo: las líneas llanas y cantabile, sul tasto y junto al
puente, sin vibrato, détaché y marcato, portamento, dobles cuerdas, pizzicato de orquesta y
resonante (y el pizzicato de jazz del contrabajo), trémolo, un arco lento, MPE. Sus cuatro
macros: la dinámica, el vibrato, el contacto, el cuerpo.

El violín y la viola están en el mismo motor pero aún no se ofrecen: esperan la misma puerta
medida que pasaron el chelo y el contrabajo.
