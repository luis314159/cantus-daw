# La afinación de la canción

*Cantus 0.1.0-beta.18 · [Read in English](../en/tuning.md) · volver al [manual](README.md)*

Toda canción empieza en temperamento igual con el La4 a 440 Hz, como toda canción anterior.
Cuando una obra pide otra afinación —un diapasón barroco, un temperamento histórico, la
entonación justa, una escala Scala de 31 notas o la afinación que dirige otro programa— la
pone una vez para la canción, y **todos los instrumentos afinados de Cantus la tocan**:
Chorda, Exemplar y los instrumentos muestreados, los sintetizadores, Fides, Gravis, Organum;
los efectos con altura (Tonus, Saltus, la tónica de Nitor, Loquax) la siguen; los plug-ins
alojados la reciben como afinación por nota donde pueden. Una pista puede seguirla, ignorarla
o tener la suya.

**Ábrala** con *Opciones ▸ Afinación…* o **Ctrl+Alt+T** (la paleta de comandos la encuentra
por "afinación", "temperamento", "Scala" o "MTS-ESP"). Una canción con afinación propia lo dice
en la barra superior, tras su duración: *9 pistas · 64 compases · 2:03 · Werckmeister III en
Eb · A415*. Una canción en la predeterminada no dice nada.

Cada cambio en la ventana es un paso del historial: **Ctrl+Z** devuelve la afinación.

## El sistema

- **Igual** — doce pasos iguales por octava.
- **Temperamento** — una afinación histórica o justa, elegida de la lista: pitagórica,
  mesotónica de un cuarto y de un sexto de coma, Werckmeister III y IV, Kirnberger II y III,
  Vallotti, Young (1799), Kellner, Neidhardt, Bach (Lehman), entonación justa mayor y menor, y
  una afinación "barbershop" de límite 7. **Tónica** la gira para que su tonalidad más pura
  quede donde usted toca (‹ › la mueve un semitono). **Seguir la tonalidad de la canción** la
  gira a la tonalidad de la canción cada vez que esta cambia: ponga antes la tonalidad en la
  barra superior.
- **Scala** — una escala leída de un archivo `.scl` (y su mapa de teclado `.kbm`, si lo
  tiene), con cualquier número de notas y cualquier periodo: 31-EDO, Bohlen–Pierce, el
  slendro de un gamelán.
- **MTS-ESP** — la afinación que dirige otro programa (vea más abajo).

## La altura

**La4** es la nota desde la que se afinan las demás: suena a esta frecuencia sea cual sea el
temperamento, y las demás se mueven con él. Los **preajustes** son las alturas que piden los
músicos: 415 (barroco), 430 (clásico), 432, 440 (de concierto), 442 y 443 (muchas
orquestas). ‹ › lo mueve 1 Hz.

Un **mapa de teclado** nombra su propia referencia —"la tecla 69 es 442 Hz"—, así que con uno
la fila se llama **Diapasón**: 440 toca el mapa tal como está escrito, y cualquier otra altura
sube o baja el mapa entero.

## El círculo de quintas y los grados

El lado derecho muestra lo que oirá antes de oírlo. El **círculo de quintas** recorre Do, Sol,
Re, La … Fa: cada arco del **anillo exterior** es la quinta de su nota a la siguiente, cada arco
del **anillo interior** la tercera mayor sobre su nota, coloreados por **lo rápido que baten**
en el Do central: un intervalo puro no bate (el color bajo de los medidores), uno que bate
rápido lleva su color de saturación. Así comparan los afinadores los temperamentos: en el
temperamento igual cada quinta bate algo menos de una vez por segundo y cada tercera unas diez;
en Werckmeister III las tonalidades cercanas a Do tienen terceras tranquilas y las lejanas más
ásperas; en el mesotónico las terceras son puras y una quinta aúlla (el lobo). Deje el puntero
sobre una nota para leer su quinta y su tercera en batidos por segundo y su quinta en cents
respecto a la pura. El centro nombra la tónica a la que está girada la afinación, y la altura
del La4.

Debajo, **los grados**: los cents de cada nota sobre Do, sus cents **desde el temperamento
igual** (en el La4 elegido: La marca 0) y los batidos de su quinta y su tercera. Una escala que
no es de doce por octava muestra sus grados como los escribió su archivo, en cents, y la tecla
más cercana de un teclado de doce notas con su distancia ("C +38.71": el grado 0 es Do).

## Archivos Scala

**Importar .scl / .kbm…** lee una escala y su mapa de teclado, elegido con ella o junto a ella
con el mismo nombre. La canción guarda la **escala**, no el archivo: envíe la canción a
alguien y la afinación viaja con ella. Un archivo que el formato Scala no admite se rechaza con
el motivo ("No se leyó: 16 pitches of 17"), y nada cambia. **Exportar…** escribe la afinación
como `.scl` (un temperamento o el igual como sus doce notas en cents) y, cuando tiene mapa de
teclado, el `.kbm` a su lado: para Surge XT, Pianoteq, el editor de un sintetizador o el propio
Scala.

## MTS-ESP: seguir a otro programa y dirigirlos

MTS-ESP (de ODDSound) es como viaja la afinación entre programas y plug-ins en un ordenador.
Cantus lo usa cuando está instalado —lo instalan el **MTS-ESP Mini** gratuito de ODDSound,
**Surge XT** o **Pianoteq**; Cantus no lo incluye—. Sin él, la opción MTS-ESP aparece atenuada y
la ventana dice por qué.

- **Seguir**: elija *MTS-ESP* como sistema y cada instrumento de la canción toca la tabla que
  publica el programa maestro, en vivo (una exportación toma la tabla que encuentra al empezar).
  Sin maestro, toca el temperamento igual en el La4.
- **Dirigir**: **Ser el maestro de MTS-ESP** publica la afinación de la canción, y la vuelve a
  publicar cada vez que la cambia: Surge XT, Vital, Pianoteq y cualquier otro plug-in de
  MTS-ESP —dentro de Cantus o en otro programa— tocan en la afinación de la canción. Solo un
  programa puede dirigir a la vez: mientras otro lo hace, Cantus lo dice y lo deja estar. Si ese
  programa se cerró o falló y dejó su lugar ocupado, **Reiniciar MTS-ESP** lo libera (también
  desconecta a un maestro que siga en marcha). Una canción que sigue a MTS-ESP no puede además
  dirigirlo.

## La afinación propia de una pista

En el inspector de una pista, **Afinación** dice lo que toca y abre una lista:

- **Sigue al proyecto** — lo predeterminado.
- **Temperamento igual** — esta pista ignora la afinación de la canción: un bus de batería, un
  bucle muestreado, un bajo que quiere llano.
- **La suya…** — abre la misma ventana para esta pista: todo lo de arriba, solo para ella.
  *Seguir al proyecto*, en su pie, se la devuelve.

## La escucha

**Tocar una cadencia en …** toca I–IV–V–I (i–iv–V–i en una canción menor) en la tónica de la
afinación, en la pista que edita la ventana, en la pista seleccionada o en la primera pista de
instrumento: así oye el color del temperamento antes que la canción.

## Chorda: una afinación propia, desde dentro de Chorda

Chorda puede tocar en una afinación propia mientras el resto de la canción sigue como está. Su
vista *Editar* tiene una tarjeta **Temperamento**:

- **Suena en**: *La del proyecto* (lo predeterminado) o *La suya*.
- **Afinación**: los preajustes, en grupos — *Igual* (De concierto, estirada al afinar; De
  estudio, ajustada; Brillante, estiramiento amplio), *Históricas* y *Entonación justa* (cada
  una en la **Tónica** de abajo: Werckmeister III en E, mesotónica en D …), *Carácter*
  (Honky-tonk, Vertical de salón antiguo, De cantina, Un poco desafinado, Barroca La 415, Verdi
  La 432) y *Suyas*. Elegir una da a Chorda su afinación propia y ajusta el estiramiento y los
  unísonos que necesita, en un solo paso del historial (**Ctrl+Z** lo devuelve todo).
- **Tónica**: dónde se asienta una afinación histórica o justa (‹ › un semitono).
- **Suyas**: *Guardar como mía* guarda la afinación actual como un archivo Scala suyo (en la
  carpeta `Tuning` de los datos), que aparece en *Suyas*; *Importar…* y *Exportar…* leen y
  escriben `.scl` / `.kbm`.
- **Cadencia**: óigala antes de tocar.
- El **círculo de quintas** y las **terceras mayores más puras y más ásperas** dicen qué
  tonalidades suenan tranquilas y cuáles ásperas.

El **La4** sigue una regla, para que nada antiguo cambie: mientras la pista no toca ninguna
afinación, la perilla A4 de Chorda es la altura, como en toda canción anterior; cuando toca la
del proyecto, la perilla muestra el La4 del proyecto, atenuado ("la del proyecto"); con una
afinación propia, ajusta el La4 de esa afinación. La línea bajo el estiramiento dice qué
afinación toca el piano; un clic en ella abre *Proyecto ▸ Afinación*. El **estiramiento**, la
**rigidez** y los **unísonos** de Chorda siguen siendo suyos.

## Conviene saber

- Un mapa de teclado puede dejar teclas sin asignar: no suenan, como en Scala.
- Organum (una rueda sirve a varias teclas) y Lamina (teclas plegadas por octavas) siguen la
  clase de altura de una escala que no es de doce por octava.
- Pulsus y Tympanum no tienen altura: la afinación no los toca.
