// Grammar sheet, part 2: the inks, the rules, the states. Continues grammar.html's page.
(function () {
  let y = window.__y, b = window.__b;
  const t = new URLSearchParams(location.search).get('t') || 'night';
  const k = INK[t];
  const W = 1240, P = 24;
  const T = (x, yy, s, o = {}) => { b += label(t, x, yy, s, o); };

  function lum(hex) {
    const n = parseInt(hex.slice(1), 16);
    const c = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map(v => {
      v /= 255;
      return v <= 0.03928 ? v / 12.92 : Math.pow((v + 0.055) / 1.055, 2.4);
    });
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
  }
  const ratio = (a, b2) => {
    const [l1, l2] = [lum(a), lum(b2)].sort((p, q) => q - p);
    return ((l1 + 0.05) / (l2 + 0.05)).toFixed(1);
  };

  // 3. Inks
  T(P, y + 14, '3 · The inks (tokens; every direction sets its own values — these are the sheet\'s)', { size: 14, weight: 600 });
  y += 28;
  const inks = [
    ['ink-1', 'ink1', 'the body: what the glyph is about (the act, the subject)'],
    ['ink-2', 'ink2', 'edges, structure and context (slots, keys, frames, the start line, a wake)'],
    ['ink-on', 'inkOn', 'ink-1 when the key is on'],
    ['ink-on-dim', 'inkOnDim', 'ink-2 when the key is on'],
    ['ink-off', 'inkOff', 'both inks when the key is disabled'],
    ['ink-rec', 'inkRec', 'record at rest: a muted red, the only colour at rest'],
    ['ink-rec-on', 'inkRecOn', 'record armed or recording'],
  ];
  inks.forEach(([name, key, use], i) => {
    const yy = y + i * 24;
    b += `<rect x="${P}" y="${yy}" width="40" height="18" rx="3" fill="${k.well}" stroke="${k.edge}"/>`;
    b += `<rect x="${P + 10}" y="${yy + 5}" width="20" height="8" rx="1" fill="${k[key]}"/>`;
    T(P + 52, yy + 13, name, { size: 12, weight: 600 });
    T(P + 150, yy + 13, k[key], { size: 11.5 });
    T(P + 230, yy + 13, `${ratio(k[key], k.well)} : 1 on the key`, { size: 11.5, dim: true });
    T(P + 370, yy + 13, use, { size: 11.5 });
  });
  y += inks.length * 24 + 16;

  // 4. Rules
  T(P, y + 14, '4 · The rules', { size: 14, weight: 600 });
  y += 26;
  const rules = [
    ['Proportion', 'A solid primitive is a third of the key (play 8 in 24, 12 in 36); stop is a quarter, so it weighs the same to the eye. A line glyph keeps inside half the key. The rest is air — that air is what reads as clean.'],
    ['Two inks', 'ink-1 is the subject (the arrow, the rod, the clips, the notes, the poles that snap); ink-2 is its structure (the start line, the body of the magnet, the fader slots, the keys of the keyboard, frames). A glyph never uses more than two inks and the record red.'],
    ['Filled or outlined', 'Transport primitives are filled. Tools are lines with filled heads and masses. Window keys are small diagrams of filled content on ink-2 structure — never a box drawn inside the key\'s box.'],
    ['Lines and terminals', 'Lines are whole device pixels (weights in the table). Terminals are cut square; arrowheads are solid triangles, never open chevrons, except the follow key\'s chevron, which is a motion mark. At 16 and 20 px a 45° line is drawn pixel by pixel as a clean staircase.'],
    ['Corners', 'Solids have square corners up to 20 px and a 1–1.5 px radius from 24 px. Clips and notes take a 1 px radius from 24 px. Curves (loop, undo, zoom) use true arcs from 24 px and hand anti-aliasing below.'],
    ['Air cut', 'Where two parts meet, one pixel of the key\'s well separates them: the magnet\'s poles from its body, the speaker\'s body from its cone, a fader cap\'s grip. It is what keeps a 16-px glyph from turning to a blot.'],
    ['Optical centring', 'The canvas is centred in the key and floored to a whole pixel; a triangle sits a little right of the box centre (its centre of mass is a third from its base).'],
    ['On', 'On is shown by the key and the inks together: ink-1 → ink-on, ink-2 → ink-on-dim, the well tinted (13 %) with a one-pixel edge in the on colour. No glyph changes shape to be on; colour is spent only on state.'],
  ];
  rules.forEach(([h, txt]) => {
    T(P, y + 13, h, { size: 12, weight: 600 });
    // wrap the text by hand at ~150 characters
    const words = txt.split(' ');
    let line = '', yy = y + 13;
    words.forEach(w => {
      if ((line + ' ' + w).length > 150) { T(P + 150, yy, line, { size: 11.5 }); line = w; yy += 16; }
      else line = line ? line + ' ' + w : w;
    });
    T(P + 150, yy, line, { size: 11.5 });
    y = yy + 12;
  });
  y += 10;

  // 5. States at 1:1
  T(P, y + 14, '5 · States at 1:1 — rest, on, disabled (loop, snap, mixer, record)', { size: 14, weight: 600 });
  y += 28;
  let x = P;
  [['loop', 24], ['snap', 20], ['mixer', 20], ['record', 24], ['loop', 36], ['snap', 30]].forEach(([id, c]) => {
    ['rest', 'on', 'off'].forEach(st => {
      b += iconInKey(t, G[id], c, x, y, st);
      x += KEY[c] + 6;
    });
    x += 18;
  });
  y += 54 + 24;

  const H = Math.ceil(y);
  document.body.style.background = k.page;
  document.getElementById('out').innerHTML = svgDoc(t, W, H, `<rect width="${W}" height="${H}" fill="${k.page}"/>` + b);
  document.body.setAttribute('data-w', W);
  document.body.setAttribute('data-h', H);
})();
