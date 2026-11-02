---
name: feedback-dancer-quality
description: "Owner 2026-10-31 (round 35): the dancing characters must look super professional and be really fun — show them GIF previews before a character counts as done"
metadata:
  node_type: memory
  type: feedback
  originSessionId: f40cc6ed-5cdd-4c32-bcba-0c89b1c34150
  modified: 2026-10-31T20:38:42.683Z
---

The owner, seeing the contract's stand-in figure dance (round 35, 2026-10-31): "espero que
el resultado final sea algo super super profesional por favor, algo muy padre y divertido".
Earlier the same day they asked to see a preview as soon as possible ("guarda un preview de
la animación para ver si se puede").

**Why:** the character is what people show on stream and in social videos; a programmer's
drawing would make Cantus look amateur. Same bar as [[ui-quality-bar]].

**How to apply:** treat character art and motion as first-rank craft (palette, shading,
outlines, personality, beat-reactive extras, several draw–look–redraw passes). Render GIFs
with `crates/cantus-dance/examples/preview.rs` (frames) + Pillow (run python without `-I`,
Pillow is in the user site) and show the owner before calling a character done. See
[[round-35]].
