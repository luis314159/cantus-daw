# Pulsus III: pads en canales propios, una cadena por pad, kits de donde sea

*Cantus 0.1.0-beta.12 · [Read in English](../en/drum-kits.md) · volver al [manual](README.md)*

Pulsus es el instrumento de batería de Cantus: hasta 32 pads, cada uno un tambor sintetizado
o muestras. Este capítulo trata de tres cosas que una batería necesita en una mezcla: cada
pad en **un canal propio del mezclador**, **efectos en un solo pad** antes de que el kit se
sume, y **kits hechos con tus propias muestras**: una carpeta de golpes sueltos o un kit SFZ.

## Separar en canales

Abre el kit (doble clic en su pista) → **⋯ ▸ Separar en canales**, o clic derecho en el canal
del kit en el mezclador → **Separar en canales**. Cada pad que suena recibe un canal propio,
con el nombre del pad, justo después del canal del kit, en el orden de los pads. Cada pad
tiene ahora su fader, paneo, inserciones, envíos, silencio y solo. Un solo paso de deshacer.

Los canales de los pads suenan **hacia el canal del kit**: el canal del kit es el bus del kit,
así que un compresor o un fader allí sigue actuando sobre todo el kit, y la pista del kit
sigue silenciándolo y poniéndolo en solo entero. Con cada canal nuevo a 0 dB y centrado, el
kit separado suena **exactamente** como sonaba antes — no casi: las mismas muestras, al bit
(con el nivel propio de la pista del kit a 0 dB y su balance centrado; con otro nivel, los dos
coinciden por debajo de −120 dB). El canal de un pad panea como un balance (su ley de paneo es
de 0 dB), así que deja pasar el pad intacto; el canal del kit coloca el kit con la ley de
paneo del proyecto, como antes. El menú del canal dice qué ley sigue (**Ley de paneo ▸**) y
deja cambiarla.

**Unir de nuevo** (en los mismos menús) devuelve cada pad al canal del kit y quita los canales
que hizo la separación — salvo que otra cosa use alguno (un envío hacia él, otra pista). Un
solo paso de deshacer.

Bueno saber:

- Poner en solo el canal de un pad toca ese pad solo, a través del canal del kit.
- Congelar el kit congela el canal del kit, con cada canal de pad dentro. La pista del kit
  sola no se puede congelar mientras está separada (su sonido son varias salidas).
- El modo de grabación renderiza el canal del kit como un solo render.
- Rebotar en el sitio la pista del kit rebota **el kit entero** (cada pad, como suena el kit
  sin separar) hacia el canal del kit; los canales propios de los pads — sus faders e
  inserciones — no entran en el rebote. Para rebotar lo que el mezclador hace a los pads,
  rebota el canal del kit. El canal de un pad no se puede rebotar solo: es una salida del kit.
- Cargar otro kit sobre un kit separado mantiene la separación: cada pad nuevo toma el canal
  del pad viejo en la misma nota (o del mismo tipo de tambor; en los kits de muestras, del
  mismo nombre y luego del mismo lugar). Cambiar el instrumento o borrar la pista quita con
  ella los canales de los pads, en el mismo paso de deshacer.
- Enrutar la pista del kit a otro canal lleva allí el kit entero: los canales y las cadenas
  de los pads que sonaban hacia el canal viejo suenan hacia el nuevo, en el mismo paso de
  deshacer.
- Separar necesita sitio: el motor toca como mucho 256 pistas — las del proyecto, y una más
  por cada pad con canal o cadena propia — y 256 canales. Una separación que pase de ahí se
  rechaza y se dice; quita antes algunas pistas o canales.
- Una plantilla de pista de un kit guarda los canales y las cadenas de sus pads (al
  insertarla son canales nuevos, propios). Explotar las tomas de un kit no da cadenas a las
  pistas de toma: son del kit.
- Stems: la cadena de un pad es parte del sonido del kit — en el stem de pista del kit y, con
  el kit directo al máster, en el stem de lo que va directo al máster.
- DAWproject no tiene salidas para un instrumento: un kit separado se escribe entero, y el
  informe de exportación lo dice.

**Kits anteriores.** Un kit hecho antes de esta versión es *Modelo 2* (o *Modelo 1*). También
se separa, pero se suma a sí mismo solo con un redondeo (unos −135 dB); **Actualizar a Pulsus
III** en la cabecera del kit lo hace exacto. Nada más cambia.

## Una cadena por pad

Elige un pad y abre **Edición**: la tarjeta **Cadena** lista las inserciones propias del pad,
en orden. **Añadir inserción ▾** pone cualquier efecto de Cantus al final (con su preset
inicial); cada inserción tiene su interruptor, **Editar** (su editor) y **Quitar**. El sonido
del pad pasa por su cadena antes de unirse al kit — o antes de ir a su propio canal, cuando el
kit está separado; separar y unir mantienen la cadena en su pad. Cuando el pad ya tiene
cadena, **Más…** abre el selector de dispositivos sobre ella: todos los efectos, plug-ins
incluidos.

Una cadena no cuesta nada mientras su pad calla: Cantus no ejecuta sus inserciones hasta que
el pad vuelve a sonar, y lo que sale es exactamente lo que daría ejecutarlas.

Una cadena sigue a su kit: enruta la pista del kit a otro canal y cada cadena va con ella.
**Añadir inserción ▾** ofrece lo que ofrece un canal (Spatium es la reverb).

## Kits con tus muestras

**Una carpeta.** Arrastra una carpeta de golpes sueltos desde el explorador de archivos sobre
los pads de un kit: pasa a ser ese kit. Suéltala en cualquier otro lugar de la ventana: pasa a
ser una pista de kit nueva. Cantus lee los nombres, en español o en inglés:

| Un nombre con | Va a |
|---|---|
| bombo, kick, bd, bass drum | Bombo (nota 36) |
| aro, rim, rimshot, side stick | Aro (37) |
| caja, redoblante, snare, sd | Caja (38) |
| palmas, clap | Palmas (39) |
| charles / hh / hihat / hat + cerrado, closed | Charles cerrado (42) |
| … + pie, pedal, foot | Charles de pedal (44) |
| … + abierto, open | Charles abierto (46) — los tres se cortan entre sí |
| tom + grave/floor/low, tom, tom + agudo/high | Toms (41…50) |
| crash, splash, china · ride · cencerro, cowbell | 49 · 51 · 56 |
| pandereta, shaker, conga, bongó, clave, maraca, perc… | Percusión (54, 58, 60…) |

Los archivos que solo se diferencian **por un número** (*Bombo 1, Bombo 2*) pasan a ser un pad
que los toca por turnos (round robin) — salvo un número que nombra el tambor: *Tom 1, Tom 2,
Tom 3* (de agudo a base), *Crash 1, Crash 2* y uno al principio (*808 Kick*, *909 Kick*) son
pads propios. Los que se diferencian **por una dinámica** (*suave,
fuerte, soft, hard, pp, mf, ff, vl1…vl4*) pasan a ser capas de velocidad de un pad, la más
suave para los golpes más suaves. Un nombre que no dice nada va al siguiente pad libre, en el
orden de la carpeta. Hasta 32 pads y 16 capas por pad; lo que no cabe se nombra en el registro.
Un solo paso de deshacer. La carpeta se lee en segundo plano; una de más de 512 sonidos (o
4 GB) no es un kit y se rechaza, y una carpeta soltada fuera de los pads de un kit debe tener
algún archivo con nombre de tambor.

**Un kit SFZ.** Arrastra un archivo `.sfz` igual: un pad por tecla, sus regiones como capas
(rangos de velocidad, afinación, nivel, paneo), con sus round robins y elecciones al azar, y
los grupos exclusivos del kit (los de un charles) como un grupo de corte. Probado con el kit
libre *Virtuosity Drums*.

## Los kits de fábrica

La cabecera del kit lista los dieciséis kits de fábrica: Pulsus clásico, Estudio, 808, 909,
Boom bap, Electro, Percusión, Trap, House, Lo-fi, Rock, Pop, R&B, Reguetón, Cinemática y Jazz
con escobillas — todos sintetizados por el propio Pulsus, sin muestras. Cada uno trae su **Nivel del kit**
(la tarjeta del kit), calibrado; 0 dB en un kit tuyo no cambia nada. **⋯ ▸ Explorar
presets…** abre el navegador de presets sobre el kit: filtra por estilo, escucha un kit antes
de elegirlo y cárgalo en un solo paso de deshacer.

## Para agentes

`split_kit_outputs` (y `merge`), `set_pad_chain` (añadir o quitar una inserción en un pad) y
`make_kit_from_folder` (una carpeta o un `.sfz`, solo dentro de la carpeta del proyecto o de
una carpeta que permitiste en Ajustes ▸ IA, con el acceso a archivos activado) hacen lo mismo, un paso de deshacer cada
uno — ver [Lo que un agente de IA puede hacer](ai-tools.md).
