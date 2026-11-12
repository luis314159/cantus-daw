# Prueba de escucha de Chorda

Esta carpeta la escribió Cantus (`cargo run --release -p cantus-ui --example chorda_abx -- <carpeta>`).
Contiene siete frases cortas, cada una tocada por tres pianos con el propio motor de Cantus:

- **Chorda modelo 2** — el piano de cola modelado de Cantus tal como lo toca una pista nueva
  (ronda 37),
- **Chorda modelo 1** — el mismo instrumento tal como lo hizo la ronda 28 (lo que conservan
  las canciones antiguas),
- **el Salamander Grand Piano** — un Yamaha C5 muestreado por Alexander Holm (CC BY 3.0),
  tocado por Exemplar exactamente como lo instala *Paquetes de sonido…*.

Todos los archivos tienen la misma sonoridad (ITU-R BS.1770 integrada, el mismo objetivo para
las tres versiones de una frase) y la misma duración. No se les hizo nada más.

## Cómo hacerla

1. Abre `index.html` en un navegador (doble clic). No necesita red.
2. Usa auriculares o buenos altavoces en un cuarto silencioso. Ajusta el volumen una vez, con
   el primer clip, y no lo toques.
3. **ABX 1** (Chorda 2 / Salamander) y **ABX 2** (Chorda 1 / Chorda 2): 16 ensayos cada una.
   En cada ensayo A y B son los dos pianos (en orden sorteado) y X es uno de ellos, sorteado.
   Reproduce A, B y X cuantas veces quieras — cambiar mientras suena conserva el punto
   (teclas: `A`, `B`, `X`, espacio para parar) — y di cuál es X. Si no lo distingues, adivina.
4. **Preferencia**: 14 preguntas ciegas, cada una con dos clips de la misma frase. Di cuál
   suena más a un buen piano de cola bien grabado, o que no tienes preferencia; una palabra
   del porqué ayuda más que nada.
5. **Resultados** muestra la puntuación de cada ABX y su valor p (la probabilidad de acertar
   tanto adivinando), las preferencias con una prueba de signos y — al terminar las dos ABX —
   qué piano era cada uno en cada ensayo. *Descargar resultados (JSON)* lo guarda todo;
   envía ese archivo.

## Cómo leer el resultado

- La ABX dice si **distingues** dos pianos (p < 0,05: sí). Chorda y el Salamander son pianos
  distintos: distinguirlos es lo esperado y no es un fracaso.
- La **preferencia** es lo que cierra Chorda: el modelo 2 debe preferirse al modelo 1 y estar
  al menos a la par del Salamander (sin preferencia significativa por el Salamander).
- Tus notas — "metálico", "delgado", "apagado", "ataque sintético", "sin cuerpo", "demasiada
  sala" — son a lo que apunta la siguiente pasada de voicing.

Escribe tu nombre arriba: va en los resultados. Las respuestas se guardan en este navegador (almacenamiento local) hasta que empieces de
nuevo; recargar la página continúa donde estabas.
