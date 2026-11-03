# Editar un clip con un plug-in ARA

*[Volver al manual](README.md) · [Read in English](../en/ara.md)*

**ARA 2** (Audio Random Access, de Celemony) permite que un plug-in vea una grabación entera
y los clips que la reproducen, en lugar de solo el sonido que pasa por él. Así se hacen los
editores de afinación y tiempo, los editores espectrales y las herramientas de alineación.
Cantus aloja plug-ins ARA 2 en formato **VST3**.

## Vincular un clip

1. Instala el plug-in y deja que la búsqueda de plug-ins lo encuentre (*Ajustes ▸ Plug-ins ▸ Buscar
   plug-ins*; Cantus vuelve a revisar por sí solo, una vez, los VST3 que había revisado antes
   de conocer ARA).
2. Clic derecho en un **clip de audio** ▸ en la sección **Plug-in ARA**, *Vincular a* seguido del nombre del
   plug-in.

El clip suena ahora con el render que el plug-in hace de él, en lugar de su archivo. La
ganancia del clip, los fundidos, la pista y la mezcla se aplican como a cualquier clip; la
exportación, el bounce y los stems también oyen el render. *Deshacer* quita el vínculo en un
paso.

La sección siempre lo dice: **los plug-ins ARA se ejecutan dentro de Cantus, sin
aislamiento.** Otros plug-ins pueden ejecutarse en un proceso auxiliar, para que uno que se
cuelgue no se lleve Cantus consigo; un plug-in ARA comparte la grabación directamente con
Cantus, así que se ejecuta en el propio proceso de Cantus. Guarda a menudo mientras pruebas
uno nuevo.

## Lo que el plug-in conserva

- **Sus ediciones se guardan con el proyecto.** Al guardar, Cantus le pide al plug-in su
  estado de cada clip vinculado y lo guarda en el archivo del proyecto; al abrirlo, se lo
  devuelve.
- **Mover, recortar o dividir un clip** se lo dice al plug-in, que vuelve a renderizarlo.
- **Borrar un clip, o soltarlo,** conserva el trabajo del plug-in para *Deshacer*: el clip
  vuelve con todo lo que el plug-in le había hecho.
- **Una copia de un clip vinculado** empieza con lo que el plug-in le había hecho al
  original.

## Soltar un clip

Clic derecho en el clip ▸ *Soltar* el plug-in. El clip vuelve a sonar con su archivo.
Vincularlo de nuevo empieza limpio; *Vincular a … con sus ediciones anteriores* recupera lo
que el plug-in le había hecho.

## Cuando el plug-in no está

Un proyecto abierto en una computadora sin el plug-in conserva el vínculo y el estado
guardado del plug-in; el clip suena con **su propio archivo**, y su menú dice por qué (*no
está instalado*, o lo que respondió el plug-in). Instala el plug-in y el clip se vuelve a
renderizar.

## Abrir el editor del plug-in

Clic derecho en el clip vinculado ▸ *Abrir* el plug-in. Su propia ventana se abre sobre el
clip. Cada edición que hagas ahí es **un paso de deshacer** en Cantus (*Editar ▸ Deshacer* la
revierte también en el plug-in), y el clip se vuelve a renderizar en segundo plano: sigues
oyendo el último render hasta que el nuevo está listo, y el cuerpo del clip muestra cuánto
falta. Solo se vuelve a renderizar la parte que el plug-in dice que cambió.

## Si un plug-in se cuelga o deja de responder

Si Cantus se cierra mientras un plug-in ARA trabaja, el siguiente arranque abre tu proyecto
con los clips de ese plug-in sonando con sus fuentes, y el menú del clip dice por qué: elige
*Reintentar* para volver a ejecutarlo. Un render que tarda demasiado detiene el plug-in
durante la sesión; Cantus nunca lo espera. *Solo fuentes (plug-ins ARA apagados)*, en el mismo
menú, apaga todos los plug-ins ARA: cada clip vinculado suena con su propio archivo.

## Lo que todavía no está

- Un clip vinculado no puede además estar invertido, tener warp ni las ediciones de
  afinación propias de Cantus: el plug-in se encarga de la afinación y el tiempo. Quítalos
  antes de vincularlo.
- El plug-in se ejecuta dentro del proceso de Cantus; ejecutarlo aislado llega en una
  versión posterior.

Cantus se prueba con un plug-in ARA de prueba construido con Cantus (y, donde se ha
construido, con el plug-in de referencia de Celemony); cuéntanos cómo se comportan tus
plug-ins ARA ([reportar un error](README.md#15-reportar-un-error)).
