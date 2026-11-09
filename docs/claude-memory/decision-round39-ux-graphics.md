---
name: decision-round39-ux-graphics
description: Owner 2026-11-08 — round 39 is all UX and graphics, judged on whole windows by the owner; the dancer off at start now
metadata:
  type: project
---

On 2026-11-08, after beta.12, the owner said: "FL aún se ve a años luz de Cantus", "hay botones que se ven super feos en el nuestro", and "la siguiente ronda quiero que nos enfoquemos en todos los puntos de UX y gráficos". They also said of the dancer at start: "cada vez me gusta menos, quítalo de una".

- **Round 39 = UX and graphics only.** It is recorded in BACKLOG §6j, in the r38/chorda worktree until round 38 merges. The method: two or three whole-window directions captured from the real app next to FL; the owner picks one; then the kit changes. The owner's eyes on whole windows are the acceptance test, not crop judges.
- **The dancer goes off at start in round 38.** Sent to the scorecard agent (r38/scorecard) with an e2e test through the boot path.

**Why:** crop-by-crop judging (20–22/26) hid the gestalt that the owner sees. See [[ui-quality-bar]].

**How to apply:** when round 38 closes, write docs/next-session.md for round 39 from §6j. Show the owner directions early and often.

**2026-11-08, later:** the owner, after the button study (BACKLOG §6j): "En la próxima sesión pon un agente a diseñar y hagamos los elementos a mano, no elementos reciclados o haciendo las cosas fáciles — vamos por lo difícil." So round 39 starts with **a dedicated design agent**. It designs every control and icon from a blank sheet, for each size and place: SVG at 1:1 and 4× beside FL. The owner approves the sheets, and only then does a builder put them into the kit. No reused glyphs, no shortcuts. The measured diagnosis: glyph about 30 % of the key, not 72 %; dark flat wells, not glossy caps; light-grey two-tone ink, not white solid; a drawing per size; colour only for state.
