# Tympanum frente a baterías reales

Ronda 42 · rama `r42/tympanum-real` · 26 de noviembre de 2026

Pediste que Tympanum se compare con grabaciones profesionales y libres de derechos, de
varios géneros. También que se mida dónde y con qué fuerza se golpea cada tambor, y que se
corrija lo que suene lejos de lo real. Este informe dice qué se midió, qué estaba mal, qué
se cambió y qué falta. Las muestras para escuchar están en [escuchar.html](escuchar.html).

## 1. La biblioteca de referencia

Son ocho baterías grabadas con varios micrófonos, todas reproducidas por el mismo motor de
Cantus con su propia mezcla (como las oye un usuario al cargarlas):

- DRSKit: estudio, rock y pop. CC BY 4.0.
- The Aasimonster: death metal, 16 canales. CC BY 4.0.
- Big Rusty Drums: rock de los 80, con escobillas y mazas. CC0.
- Unruly Drums: indie pop, con baquetas y escobillas. CC0.
- Gogodze Phu II: funk y soul de los 70, toms apagados. CC0.
- Virtuosity Drums: jazz. CC0.

Además hay música tocada de verdad:

- El **Groove MIDI Dataset** (Google, CC BY 4.0): 1 150 interpretaciones de diez
  bateristas, en rock, funk, jazz, latino, soul, gospel, punk y hip-hop.
- Grabaciones reales de Freesound (CC0): un jam de 15 minutos, grooves de funk y metal lento.

No se usó nada con licencia NC, ND o SA, ni nada propietario. Las grabaciones viven fuera
del repositorio; el catálogo dice de dónde salió cada una, con su licencia y su crédito.

![A qué fuerza tocan los bateristas reales: la caja (azul) y el bombo (naranja) por género; la barra gruesa es la mitad central de los golpes y el punto blanco la mediana](gmd_levels.png)

**El dato más importante:** el golpe de caja típico de un baterista real es suave
(velocidad 45), y el **41 %** de sus golpes de caja son notas fantasma, por debajo de 40.
En funk llegan al 50 %, en reggae al 64 % y en gospel al 26 %. Lo que más suena de una
batería son los golpes suaves y medios. Ahí es donde un kit generado tiene que ser más
creíble.

## 2. Cómo se midió

Cada pieza se tocó sola a siete fuerzas (16, 32, 48, 64, 80, 100 y 127), tres veces cada una.
En cada golpe se midió:

- el nivel;
- el ataque y el ruido del palo;
- el brillo a lo largo del tiempo;
- la caída de cada banda;
- la afinación y cuánto baja en los golpes fuertes;
- los modos del parche;
- la bordonera;
- el lavado de los platos;
- cuánto cambia un golpe del siguiente;
- la imagen estéreo y la sala.

Después cada medida de Tympanum se colocó sobre el rango de los kits grabados. Las mismas
interpretaciones reales (Groove MIDI) se tocaron con todos los kits y se midieron enteras.

## 3. Lo que estaba mal, y lo que se corrigió

### La dinámica: subir la fuerza a veces bajaba el volumen

Este era el defecto más audible. Tympanum tiene cuatro capas por pieza, grabadas a las
fuerzas 40, 80, 110 y 127, y las mezcla para las fuerzas intermedias. Medido en el motor, esa
mezcla daba niveles que ninguna batería tiene:

- El bombo del kit Studio a fuerza 56 sonaba **más fuerte que a 80**.
- De 100 a 127 el bombo casi no crecía (+0,7 dB), cuando el de DRSKit crece 3 dB.
- El tom de 12″ bajaba 2,8 dB de 80 a 96.
- El ride del kit Jazz sonaba 5,7 dB más bajo a 127 que a 104.

En un groove esto se oye como acentos que no acentúan y notas fantasma que saltan.

**Corrección (ya en esta rama):** cada pieza sigue ahora una ley de nivel medida. El kit
Studio sigue la de DRSKit, la batería de la que se midió. El kit Jazz sigue la suya propia,
sin escalones que bajen. El golpe más fuerte queda donde estaba; solo cambia la forma de la
curva, y cada fuerza conserva su color.

![El bombo: nivel por velocidad (izquierda) y brillo por nivel (derecha); gris los kits grabados, azul Tympanum antes, naranja después](curves_kick.png)

![Los toms](curves_tom.png)

![La caja](curves_snare.png)

### Los toms del kit Jazz: un ataque errático

En los toms de 10″ y 12″ del kit Jazz, el brillo de los primeros 3 ms saltaba de 400 Hz a
10 kHz de un golpe al siguiente, con la misma fuerza. El tom de Virtuosity, del que se
midieron, se queda entre 1,3 y 2,1 kHz. La causa era un chasquido de palo que solo esa tabla
tenía. **Corrección:** se quita ese chasquido; el golpe sordo del palo se queda.

### El charles del kit Jazz cambia demasiado de un golpe a otro (diagnosticado)

De un golpe cerrado al siguiente, a la misma fuerza, su espectro cambia 6,1 dB. En los charles
grabados cambia 1,4–2,0 dB, y en el del kit Studio 1,65.

Se probó a reducir el sorteo de nivel de cada golpe, y no cambió nada. La causa es otra: unos
pocos parciales fuertes y muy juntos de su tabla laten de forma distinta en cada golpe. Se
arregla al volver a medir esa tabla. Queda pendiente.

### Dónde golpea el palo (centro y borde)

Se midió sobre las grabaciones mismas. En el tom de Virtuosity, golpeado fuera del centro, la
zona del modo (1,1), alrededor de 1,5 veces la fundamental, suena **7–12 dB más fuerte**
frente a la fundamental que golpeado en el centro. Es la física de una membrana: el centro
casi no mueve los modos con diámetros nodales; el borde, sí. La caja de Big Rusty se mueve en
el mismo sentido, 3–6 dB.

Se probó a subir esos modos con el control de posición (`pos`), pero en el motor no subieron
como se esperaba, así que se retiró. El golpe en el borde necesita que el modelo 6 excite cada
modo según el punto de golpe, no un realce de banda. Queda medido y anotado.

### Lo que la medida descartó

- **El charles cerrado no suena demasiado largo.** En la primera medida parecía durar 2–5
  veces más que los grabados. Era porque se tocaba con el pedal un 10 % abierto. Con el
  pedal cerrado, como lo toca una canción sin CC4, dura 241 ms a 8 kHz frente a 193–224 ms
  de los kits grabados: casi igual.
- **«Más fuerte = más brillante» no vale para la caja.** En cuatro de las siete cajas
  grabadas, el golpe suave es *más* brillante que el fuerte, porque en un golpe suave la
  bordonera pesa más que el parche. En los toms, el bombo y los platos sí sube el brillo con
  la fuerza, y Tympanum ya lo hace.
- **Centro contra borde en la caja de Virtuosity:** con la bordonera puesta o quitada, no da
  una diferencia estable. El cambio de posición se basa en el tom de Virtuosity y en la caja
  de Big Rusty.

## 4. Dónde está Tympanum ahora

Cada medida que cae fuera del rango de los kits grabados suma un peso según lo audible que
es.

| | antes | después |
|---|---|---|
| Studio Maple: peso total fuera del rango | 23,2 | 13,7 |
| Jazz Birch: peso total fuera del rango | 38,1 | 25,7 |
| Ataque de los toms del kit Jazz (0–3 ms, fuerte) | 4 875 Hz | 1 745 Hz (grabados: 549–2 083) |
| Escalones de fuerza que bajan de volumen | 2 piezas | ninguno |

![El bombo, cada medida sobre el rango de los kits grabados](fig_kick.png)

![La caja](fig_snare.png)

![Los toms](fig_tom.png)

![El charles](fig_hihat.png)

![El crash](fig_crash.png)

![El ride](fig_ride.png)

## 4b. El modelo 6: el ataque, y por qué no basta

Después de este informe se hizo el **modelo 6**, que cambia cómo empieza cada golpe de los
tambores. Las canciones que ya tenías siguen sonando exactamente igual con el modelo 5:
una canción guardada antes recuerda el suyo, comprobado muestra a muestra. Los cambios:

- cada modo del parche entra empujado por el contacto del palo (0,8 ms fuerte, 1,8 ms suave; el doble con la maza del bombo). La literatura mide 3–8 ms en una caja o un tom real: este contacto es de 3 a 5 veces demasiado corto, otra razón para no seguir puliendo el modelo;
- la bordonera llega con un retraso, por el aire de dentro del tambor;
- el «knock» es el empuje del parche y ya no un ruido;
- un golpe entre dos capas de velocidad ya no puede anularse a sí mismo: el bombo a 96 caía 3,3 dB bajo el de 80;
- el clic del palo en el charles es el mismo golpe a golpe;
- un tambor golpeado mientras suena nunca queda más débil que el golpe solo.

| | modelo 5 | modelo 6 | grabadas |
|---|---|---|---|
| Studio: peso total fuera del rango | 13,7 | 12,8 | — |
| Jazz: peso total fuera del rango | 25,7 | 24,1 | — |
| Cresta del ritmo de rock, Studio (dB) | 27,7 | 22,8 | 19,1–23,8 |
| Cresta del funk, Studio (dB) | 30,2 | 29,1 | 20,9–27,8 |
| Cresta del jazz, Jazz (dB) | 25,7 | 23,7 | 18,5–20,8 |
| Cresta de los primeros 10 ms, media de los tambores Jazz, sobre la mediana grabada | +2,8 dB | +1,7 dB | 0 |

Escucha el modelo 6 en [m6/index.html](m6/index.html). Los gráficos de abajo comparan ahora el
modelo 5 (azul) con el modelo 6 (naranja) sobre el rango de las grabadas.

**Tu oído tenía razón.** Al rock con el modelo 6 lo llamaste «horroroso», y a todo lo anterior
«un desastre, no se parece nada a algo real». Las medidas mejoraron, pero no lo bastante para
sonar real. El análisis pieza a pieza ([analisis-elementos/index.html](analisis-elementos/index.html))
dice por qué:

- al bombo Studio le faltan todos los agudos después del ataque (12–39 dB);
- charles, crash y toms tienen graves de más (6–12 dB a 100–200 Hz);
- a la campana del ride le falta su nota;
- la caja Studio está hueca en 500–800 Hz y tiene un tono de 200 Hz demasiado largo.

Varias de estas cosas se pueden corregir en las tablas. Pero la bordonera, el lavado de los
platos y el roce de los charles son ruidos caóticos que una muestra trae tal cual y que el
modelo solo imita en promedio.

**Por eso cambia el plan.** Las baterías principales de Tympanum pasan a ser **muestreadas**,
desde las mejores grabadas libres: DRSKit, Virtuosity, Gogodze, Big Rusty, Unruly, MuldjordKit.
Tendrán todas sus capas, round robin sin repetir, sus micrófonos y su fuga entre micrófonos, y
un cruce entre capas que no se anula. El modelo queda para lo que hace mejor que una muestra:
afinar, apagar, variar sin fin, la bordonera simpática, la posición del golpe y la sala. Las
baterías modeladas quedan como una familia aparte, con su nombre. Es el plan de la ronda 43.

## 5. Lo que falta (en orden de lo que más se oye)

1. **Los picos (diagnosticado, sin corregir todavía).** En los grooves, Tympanum tiene
   2–10 dB más de factor de cresta (pico sobre sonoridad) que los kits grabados. La causa
   está medida en la caja, banda por banda:
   - En DRSKit y en Virtuosity todas las bandas llegan a su máximo entre 5 y 11 ms después
     del primer sonido; los graves, los últimos.
   - En Tympanum los agudos llegan al máximo a los 0,5–1,4 ms, los medios a los 2–3 ms y los
     graves a los 4–5 ms.

   El parche real tarda en moverse:
   - el palo lo toca ~1 ms;
   - el parche se desplaza a lo largo de sus primeros medios ciclos (2,5 ms cada uno en una
     caja de 14″);
   - el parche de abajo y la bordonera responden 1–3 ms después, a través del aire de dentro.

   Tympanum arranca todos los modos de golpe. La corrección es el arranque del modelo 6: los
   modos empujados a través del contacto del palo, y la bordonera con el retraso de la
   cavidad. No un limitador.
2. **Los medios.** En el kit Studio, cada pieza está dentro del rango a 1 kHz; el exceso del
   groove viene del equilibrio de la mezcla. En el kit Jazz, la caja y los toms sí están por
   encima, por su «golpe sordo» de palo. Se corrige con el punto 1.
3. **Escobillas.** Tympanum no tiene escobillas, y el jazz y las baladas las necesitan. El
   plan está escrito:
   - Las referencias: las escobillas de Big Rusty, Unruly y Swirly, y dos sets de Freesound.
   - Lo que hay que medir: la excitación de un toque (muchas cerdas en 5–30 ms en lugar de un
     contacto de 1 ms); el barrido según la velocidad de la mano; la bordonera.
   - Lo que hay que construir: toques y barridos sostenidos.
5. **Kits por género.** Hay referencias de metal (Aasimonster), funk de los 70 (Gogodze) y
   rock de los 80 (Big Rusty). Un kit Tympanum para cada uno exige medir sus tablas como se
   hizo con DRSKit y Virtuosity. Es trabajo de la próxima ronda, y solo contra estas
   grabaciones.
6. **Dónde golpea el palo, en la caja.** Con la bordonera puesta o quitada, la caja de
   Virtuosity no da una diferencia estable en esta medida: su fundamental se lee mal bajo el
   ruido. Hace falta una medida modo por modo.

## 6. Cómo demostraríamos que es la mejor

Ninguna batería generada se ha medido así. Para demostrarlo con números, cada instrumento
pasaría las mismas tres pruebas:

1. Las medidas de este informe contra los mismos kits grabados.
2. Una prueba ciega MUSHRA con una grabación real escondida.
3. «¿Grabación o síntesis?», escuchando cada clip solo.

Los competidores que se podrían medir sin comprar nada:

- **MODO DRUM CS** de IK, que es gratis y es el rival modelado más cercano.
- La prueba de 15 días de **Chromaphone 3**.
- Las versiones gratuitas de los kits de muestras: SSD5 Free, BFD Player y la prueba de
  90 días de Logic.

Para eso hace falta tu máquina y tus cuentas.
