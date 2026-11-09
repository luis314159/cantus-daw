# Prompt para la próxima sesión (ronda 39)

*Escrito el 2026-11-09, al cerrar la ronda 38 (beta.13). El dueño pidió una ronda 39 **toda de
UX y gráficos**, con un agente dedicado solo a diseñar y los elementos hechos a mano desde una
hoja en blanco: «vamos por lo difícil, a mano, que nos dé los mejores resultados». El voicing
de Chorda (tercera pasada, a medias) pasa a la ronda 40, salvo que el dueño diga otra cosa.
Copia el bloque de abajo tal cual en una sesión nueva de Claude Code abierta en
`F:\projects\cantus`.*

---

Lee primero:
- `CLAUDE.md`;
- `docs/STATUS.md` (la cabecera, «The next session: what round 38 left» y la sección de la ronda 38);
- `docs/BACKLOG-excellence.md` **§6j** (la ronda 39, el diagnóstico de la ventana completa y el estudio de los botones);
- `docs/round38/integration.md` y `docs/scorecard/README.md` (las cifras que el aspecto nuevo no puede empeorar);
- `research/ui/fl-look-2026-11/notes.md` y sus capturas, y `research/ui/owner-crops-2026-11-08/`;
- la memoria: decision-round39-ux-graphics, ui-quality-bar, feedback-work-standards-r36, feedback-no-leftovers, feedback-review-cost, gotcha-boot-path, cantus-parallel-builders, feedback-showcase-sell, round-38.

Busca la excelencia (Colosenses 3:23). Escríbeme en español.

## 0. Antes de empezar

1. Revisa los issues de `cantus-daw` y las corridas de la beta.13 (Release, macOS): si algo
   falló, se arregla primero.
2. Espacio: C: tenía 16 GB libres al cerrar la ronda 38 (la imagen de WSL vive ahí); para
   Linux usa el clon `~/ci-r38-integ` (su `target/` ya está caliente) o uno propio solo si hay
   espacio.

## Ronda 39: UX y gráficos, la ventana completa (BACKLOG §6j)

**Por qué:**
- Dije que «FL aún se ve a años luz de Cantus» y que «hay botones que se ven super feos». Los jueces de la ronda 37 dieron 20 y 22 de 26, pero juzgaron recortes; yo juzgo la ventana completa.
- El estudio ampliado de los botones (§6j) encontró que el defecto es el **método**: íconos tecleados como coordenadas bajo reglas, nunca diseñados.
- Medido contra FL:
  - el ícono de FL ocupa ~30 % del botón, el nuestro 65–75 %;
  - FL usa un pozo oscuro y plano; nosotros, plástico abombado con brillo;
  - la tinta de FL es gris claro a dos tonos; la nuestra, blanca y maciza;
  - FL hace un dibujo para cada tamaño;
  - FL usa color solo para indicar estado.

**Cómo se trabaja.** Rondas pequeñas, **tres agentes como máximo**:

1. **Un agente diseñador, que solo diseña.**
   - Nada reciclado y ningún atajo: cada botón, ícono, perilla, fader, medidor, pestaña, campo, menú y superficie se dibuja a mano desde una hoja en blanco, para su tamaño y su lugar.
   - Entrega hojas SVG a 1:1 y a 4×, junto a la pieza equivalente de FL a la misma escala, en los tamaños donde aparece cada pieza (16, 20 y 24 px, y su 150 %), en Cantus Night y Cantus Paper.
   - **Fase A, la dirección:**
     - 2 o 3 direcciones de **ventana completa**: paleta, escala y familia tipográfica (OFL; revisa la licencia y los avisos de terceros), densidad, espaciado, la familia de botones e íconos, y cómo se ve todo en reposo (nada de «−∞» muerto);
     - capturas de la app real (no del raster de CPU) junto a FL a la misma escala;
     - **yo escojo**.
   - **Fase B, las hojas por elemento**, empezando por lo que más se ve:
     - la barra de transporte y de herramientas, y **el panel que se despliega de la barra** (metrónomo, ventanas, CPU/RAM, «Customise top bar…»: «está horrenda», §6j);
     - los botones del mixer y del inspector;
     - perillas y faders;
     - las teclas del piano roll;
     - los íconos del navegador.
     - **Yo apruebo cada hoja ampliada.** Las reglas medibles salen de lo aprobado, no al revés.
2. **Un constructor del kit** (`crates/cantus-ui/src/kit/`) que solo convierte en código las hojas aprobadas:
   - antes, decide con un ADR cómo se dibuja (operaciones vectoriales propias del kit o SVG rasterizado a cada escala), con el presupuesto de cuadro medido;
   - cada vista hereda del kit: ventana principal, mixer e inspector, arreglo, piano roll, rack, navegador, cada instrumento y efecto, diálogos, Ajustes, la primera configuración, menús y menús contextuales;
   - ambos temas, al 100 % y al 150 %.
3. **Un constructor de UX:**
   - medir y acortar las rutas de clic que uso a diario;
   - **una perilla automatizada lo dice** (en la ronda 38 creí que unas perillas estaban rotas y eran clips de automatización);
   - la línea de ayuda nunca vacía;
   - nada truncado;
   - todo panel redimensionable y movible;
   - alcance por teclado.
   - Cada cosa con su prueba de extremo a extremo.

**Lo que no cambia:**
- Muéstrame avances seguido; **el que acepta soy yo** viendo ventanas completas. Los agentes jueces pueden encontrar defectos, pero no deciden.
- El presupuesto de cuadro se mide: el aspecto nuevo no puede costar cuadros.
- Ninguna función nueva fuera de UX y gráficos esta ronda.

**Lista cuando:**
- apruebo las ventanas completas en ambos temas;
- la suite completa, clippy y fmt pasan en Windows y Linux;
- STATUS, ADRs, el manual y las capturas de la web y el README están rehechos con el aspecto nuevo;
- sale la **beta.14**: tienes mi permiso para aplastar a master, subir, publicar con
  `tools/public/publish.sh` y etiquetar cuando todo esté en verde (como en la beta.13).

---

**Lo que solo puedes hacer tú** (está en `docs/beta-checklist.md`):
- conectar y desconectar la interfaz (el aviso de cambio de dispositivo);
- la ABX de Chorda cuando llegue el kit 4 (ronda 40);
- capturas de los botones que te molesten (van a la lista del diseñador; las que ya mandaste están en `research/ui/owner-crops-2026-11-08/`);
- y lo pendiente de rondas anteriores: la UMC en dúplex, la Mac y la firma de código, la revisión legal, el permiso de la RVR1960 y el registro de la marca.

**Después (propuesta, tú decides).** Ronda 40:
- la tercera pasada del voicing de Chorda y la ABX 4;
- la reverb de convolución (7 de 8 líderes la tienen);
- la edición en cascada / insertar y borrar tiempo (6 de 8).

Y luego, por cuántos líderes lo tienen: grupos de clips, AU y la sesión con la Mac, ARA fuera de proceso, la pista de vídeo, y la memoria del motor (el ADR que propone el scorecard).
