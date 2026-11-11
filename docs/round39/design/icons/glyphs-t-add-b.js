// Cantus icon family — set 6 additions, part B (icons-b, 2026-11-09): guitars and basses, the
// electric and acoustic guitar shapes, Mexican and Latin strings, strings of the orchestra and
// the world, winds and brass, keys. Silhouettes on the 24-unit sheet, built per size through
// `track`, `sil`, `F`, `CUT`, `STK`, `C1` and `RR` (glyphs-t-tracks.js, -tracks3.js, -lib.js).
// Variants carry a capital suffix (A, B, C); the chosen ones are named at the end of the file.
//
// The guitar shapes are GENERIC stylised outlines (ADR-free note for the kit: never trace a
// maker's body or headstock); their labels are generic, the makers' model names are only
// search aliases (spec-tracks-add-b.md).
var N = window.N || (window.N = {});
const bRing = (t, cx, cy, r, w, c = 'scut') => `<circle class="${c}" fill="none" stroke-width="${px(t, w)}" cx="${cx}" cy="${cy}" r="${r}"/>`;
const bG = (tr, inner) => `<g transform="${tr}">${inner}</g>`;
const bPegs = (x0, x1, ys, h) => ys.map(y => `M${x0} ${y} H${x1} V${y + h} H${x0} Z`).join(' ');
const bDots = (pts, r) => pts.map(([x, y]) => C1(x, y, r, 'cut')).join('');
// A cut outline (the key's surface) of a path: lifts a front shape off the one behind it.
const bLift = (t, d, w = 1, s = 1) => `<path class="scut" fill="none" stroke-width="${(px(t, w) / s).toFixed(3)}" stroke-linejoin="round" d="${d}"/>`;
// A guitar-family instrument: body, neck, head, inner detail; upright before `rot`.
const bInst = (t, body, o = {}) => sil(t, (o.back || '') + F(body) + RR(o.nx ?? 11.25, o.n0 ?? 2.6, o.nw ?? 1.5, (o.n1 ?? 12) - (o.n0 ?? 2.6), 0, o.nc || 'f1') +
  (o.head ?? F('M10.8 0.2 H13.2 L13 3 H11 Z')) + (o.inner || ''), o.rot ?? 45);
const HOLE = (t, y, r = 1.8) => C1(12, y, t.D ? r : r + 0.2, 'cut');
const BRIDGE = (t, y) => t.D ? CUT(t, `M10.2 ${y} H13.8`, 1) : '';
const PICKUPS = (t, ys) => t.D ? CUT(t, ys.map(y => `M10.4 ${y} H13.6`).join(' '), 1) : '';

// ======== Electric guitar shapes (generic outlines; at 14 px only the outline survives) ========
// The shapes are drawn body-forward: 45°, the body scaled up about its centre and the neck
// short, so the outline (the only thing 14 px keeps) is as large as the key allows.
const bShape = (t, body, o = {}) => {
  const s = o.s ?? 1.22, cy = o.cy ?? 16.4, dy = o.dy ?? -1.2;
  const grow = inner => `<g transform="translate(0 ${dy}) translate(12 ${cy}) scale(${s}) translate(-12 -${cy})">${inner}</g>`;
  return sil(t, RR(o.nx ?? 11.25, o.n0 ?? 1.6, o.nw ?? 1.5, (o.n1 ?? 9) - (o.n0 ?? 1.6)) + (o.head ?? F('M10.8 -0.6 H13.2 L13 2 H11 Z')) +
    grow((o.pre ? o.pre(s) : '') + (o.bt ? `<g transform="${o.bt}">${F(body)}</g>` : F(body)) + (o.inner ? o.inner(s) : '')) + (o.top || ''), o.rot ?? 45);
};
const PU = (t, ys) => s => t.D === 2 ? CUT(t, ys.map(y => `M10.4 ${y} H13.6`).join(' '), 1 / s) : '';

// ======== Electric guitar shapes (generic outlines; at 14 px only the outline survives) ========
// S-type: two horns, the bass-side horn longer; contoured waist.
const E_S = 'M13 12.6 C13.8 11.4 15 9.8 16 9.6 C17 9.4 17.4 10.4 17 11.6 C16.6 12.8 16 13.8 16.4 14.8 C16.8 15.8 18.6 16.8 18.6 19.4 C18.6 22.2 15.8 23.6 12 23.6 C8.2 23.6 5.4 22 5.4 19.4 C5.4 17 7.2 16 7.4 15 C7.6 13.8 6.6 12 6.6 9.6 C6.6 8 7.6 7.4 8.6 8 C9.6 8.6 10.2 10.6 11 12.4 Z';
// T-type: a slab, one rounded shoulder, the treble side cut straight in to the neck.
const E_T = 'M11 10.4 C9 10 7 10 6.6 11.4 C6.2 12.8 7 14.2 6.6 15.4 C6.2 16.6 5.2 17.2 5.2 19.6 C5.2 22.6 8.2 23.6 12 23.6 C15.8 23.6 18.8 22.6 18.8 19.6 C18.8 17.2 17.8 16.4 17.8 15 C17.8 13.8 17.4 13 16.2 13 L13 13 V10.4 Z';
// Single cut, carved top: a round body, a scooped cutaway leaving one short horn.
const E_SC = 'M11 10 C8.8 10 7.2 11.2 7.2 13.2 C7.2 14.6 8 15.4 7.6 16.2 C7 17.2 5.6 17.8 5.6 20 C5.6 22.4 8.4 23.6 12 23.6 C15.6 23.6 18.4 22.4 18.4 19.8 C18.4 17.8 17.2 16.8 17.2 15.2 C17.2 13.6 17.6 11 16.8 9.2 C15.6 9.6 15.4 12.4 14.6 13.4 C13.8 13.8 13.2 13.2 13 12.6 V10 Z';
// Double-horn solid body: two sharp, near-symmetric horns, deep cutaways, a slim body.
const E_DH = 'M13 13.6 C14 12.4 14.8 10.2 16.2 8.2 C17.2 10 17.4 12.4 17 14 C16.6 15.4 18.2 16.4 18.2 19 C18.2 22 15.6 23.6 12 23.6 C8.4 23.6 5.8 22 5.8 19 C5.8 16.4 7.4 15.4 7 14 C6.6 12.4 6.8 10 7.8 8.2 C9.2 10.2 10 12.4 11 13.6 Z';
// Superstrat: long, thin, pointed horns; a sharper outline.
const E_SS = 'M13 12.8 C14.2 11.2 16 8.8 18.2 7.4 C18 9.8 16.8 12.6 16.6 14.2 C16.4 15.4 18.4 16.6 18.4 19.2 C18.4 22 15.6 23.6 12 23.6 C8.4 23.6 5.6 22 5.6 19.4 C5.6 16.8 7.4 16 7.4 14.4 C7.4 12.2 6.2 8.8 5.2 6.4 C7.8 7.4 10 10.4 11 12.8 Z';
// Semi-hollow: wide, symmetric, round horns; f-holes.
const E_SH = 'M13 12.6 C13.6 11.2 14.8 9.6 16.4 9.6 C18 9.6 18.6 11.2 18.4 12.8 C18.2 14 17.8 14.6 18.6 15.4 C19.6 16.4 20 17.6 20 19 C20 21.8 16.6 23.6 12 23.6 C7.4 23.6 4 21.8 4 19 C4 17.6 4.4 16.4 5.4 15.4 C6.2 14.6 5.8 14 5.6 12.8 C5.4 11.2 6 9.6 7.6 9.6 C9.2 9.6 10.4 11.2 11 12.6 Z';
// Offset: the waist and the lower bout skewed to the bass side, a long bass horn.
const E_OF = 'M13 12.4 C13.6 11.6 14.6 10.4 15.6 10.2 C16.6 10 17 10.8 16.8 11.8 C16.6 12.8 16 13.8 16.4 14.8 C16.8 16 18.8 17.2 18.6 19.8 C18.4 22.4 15.6 23.6 12 23.6 C8.2 23.6 5 22 5 19 C5 16.8 7 15.8 7.2 14.6 C7.4 13.2 6.2 11 6.2 8.6 C6.2 6.8 7.4 6.4 8.4 7.2 C9.6 8.2 10.2 10.4 11 12.4 Z';
// V-shape: two wings with rounded tips and a shallow notch (generic, not a maker's outline).
const E_V = 'M11 10.2 L4.8 21.4 C4.2 22.6 5 23.6 6.4 23.2 L10.4 22 L12 19.6 L13.6 22 L17.6 23.2 C19 23.6 19.8 22.6 19.2 21.4 L13 10.2 Z';
// Pointed body: four sharp points, straight edges (generic).
const E_PT = 'M13 12.4 L17.8 7.8 L16.2 14.4 L19.8 19.8 L13.4 18.4 L12 24 L10.6 18.4 L4.2 19.8 L7.8 14.4 L6.2 7.8 L11 12.4 Z';
track('eStype', t => bShape(t, E_S, { inner: PU(t, [15.6, 17.6, 19.8]) }));
track('eTtype', t => bShape(t, E_T, { inner: s => t.D === 2 ? CUT(t, 'M10.4 19.8 H13.6 M15.2 18.4 L16.8 20.8 M10.6 15.4 H13.4', 1 / s) : '' }));
track('eSingleCut', t => bShape(t, E_SC, { inner: PU(t, [15.8, 19.2]) }));
track('eDoubleHorn', t => bShape(t, E_DH, { inner: PU(t, [16.4, 19.4]) }));
track('eSuperstrat', t => bShape(t, E_SS, { inner: PU(t, [15.6, 19.6]) }));
track('eSemiHollow', t => bShape(t, E_SH, { inner: s => (t.D ? CUT(t, 'M7.4 15.8 C6.8 17.2 8.2 18.4 7.4 20.2 M16.6 15.8 C17.2 17.2 15.8 18.4 16.6 20.2', 1 / s) : CUT(t, 'M7.6 16 V19.8 M16.4 16 V19.8', 1 / s)) + PU(t, [15.8, 19.2])(s) }));
track('eOffset', t => bShape(t, E_OF, { bt: 'rotate(-13 12 17.4)', inner: PU(t, [15.8, 19.2]) }));
track('eVshape', t => bShape(t, E_V, { inner: PU(t, [14.4, 16.6]) }));
track('ePointed', t => bShape(t, E_PT, { inner: PU(t, [14.4, 16.6]) }));

// ======== Acoustic guitar shapes (one scale for all: size is part of the outline) ========
const A_DREAD = 'M12 8.4 C15.4 8.4 17.2 8.6 17.4 10.6 C17.6 12.6 16.6 13.8 16.8 15.2 C17 16.4 18.8 17.4 18.8 19.8 C18.8 22.4 16.2 23.6 12 23.6 C7.8 23.6 5.2 22.4 5.2 19.8 C5.2 17.4 7 16.4 7.2 15.2 C7.4 13.8 6.4 12.6 6.6 10.6 C6.8 8.6 8.6 8.4 12 8.4 Z';
const A_OM = 'M12 9.4 C14.4 9.4 15.8 10.6 15.8 12.6 C15.8 13.8 14.8 14.6 14.8 15.4 C16.8 16.2 17.8 17.6 17.8 19.6 C17.8 22.2 15.4 23.6 12 23.6 C8.6 23.6 6.2 22.2 6.2 19.6 C6.2 17.6 7.2 16.2 9.2 15.4 C9.2 14.6 8.2 13.8 8.2 12.6 C8.2 10.6 9.6 9.4 12 9.4 Z';
const A_JUMBO = 'M12 8.4 C15.2 8.4 17 10.2 17 12.4 C17 13.8 15.6 14.6 15.4 15.2 C17.8 16 19.4 17.6 19.4 19.6 C19.4 22.4 16.2 23.8 12 23.8 C7.8 23.8 4.6 22.4 4.6 19.6 C4.6 17.6 6.2 16 8.6 15.2 C8.4 14.6 7 13.8 7 12.4 C7 10.2 8.8 8.4 12 8.4 Z';
const A_PARLOR = 'M12 11.6 C13.6 11.6 14.6 12.6 14.6 14.2 C14.6 15.2 14 15.8 14 16.4 C15.4 17.2 16 18.4 16 19.9 C16 22.2 14.3 23.6 12 23.6 C9.7 23.6 8 22.2 8 19.9 C8 18.4 8.6 17.2 10 16.4 C10 15.8 9.4 15.2 9.4 14.2 C9.4 12.6 10.4 11.6 12 11.6 Z';
const A_CUT = 'M12 8.6 C12.8 8.6 13.4 8.7 13.4 9.4 C13.4 11.6 14 13 15.6 13.2 C16.6 13.4 16.8 14.4 16.6 15.2 C16.8 16.4 18.6 17.2 18.6 19.8 C18.6 22.4 16.2 23.6 12 23.6 C7.8 23.6 5.4 22.4 5.4 19.8 C5.4 17.2 7.2 16.4 7.4 15.2 C7.6 14 6.8 12.8 7 11 C7.2 9 9 8.6 12 8.6 Z';
const A_HEAD = F('M10.6 -2 H13.4 V0.6 H10.6 Z');
const aShape = (t, body, hy, by, o = {}) => bShape(t, body, Object.assign({ s: 1.1, dy: -0.6, n0: 0.2, n1: 9, head: A_HEAD,
  inner: s => C1(12, hy, (t.D ? 1.8 : 2) / s * 1.05, 'cut') + (t.D ? CUT(t, `M10.2 ${by} H13.8`, 1 / s) : '') }, o));
track('aDread', t => aShape(t, A_DREAD, 13.6, 20.2));
track('aConcert', t => aShape(t, A_OM, 14.2, 20.2));
track('aJumbo', t => aShape(t, A_JUMBO, 13.8, 20.4));
track('aParlor', t => aShape(t, A_PARLOR, 15.2, 20.6, { n1: 11 }));
track('aCutaway', t => aShape(t, A_CUT, 13.8, 20.2));
// Travel / mini: the whole instrument smaller — a small body on a short neck, in the same frame.
track('aTravel', t => aShape(t, A_DREAD, 13.6, 20.2, { s: 0.76, cy: 18, dy: 0.6, n0: 5.2, n1: 11, head: F('M10.7 3.2 H13.3 V5.6 H10.7 Z') }));
// Classical: the slotted headstock (A), or with the rosette in the second ink (B).
const SLOT = t => F('M10.3 -2.4 H13.7 V0.8 H10.3 Z') + (t.D ? CUT(t, 'M11.3 -1.6 V0.1 M12.7 -1.6 V0.1', 1) : CUT(t, 'M12 -1.6 V0.1', 1));
track('classicalA', t => aShape(t, A_OM, 14.2, 20.2, { nx: 11, nw: 2, head: SLOT(t) }));
track('classicalB', t => aShape(t, A_OM, 14.2, 20.2, { nx: 11, nw: 2, head: SLOT(t),
  inner: s => C1(12, 14.2, 2.8, 'cut') + C1(12, 14.2, 2.2, 'f2') + C1(12, 14.2, 1.3, 'cut') + (t.D ? CUT(t, 'M10.2 20.2 H13.8', 1 / s) : '') }));
// 12-string: a jumbo under a long head with pegs on both sides (ticks, or a solid comb).
track('guitar12A', t => aShape(t, A_JUMBO, 13.8, 20.4, { n0: 2, head: F('M10.9 -3 H13.1 V2.4 H10.9 Z') +
  STK(t, (t.D ? [-2.2, -0.6, 1] : [-1.8, 0.6]).map(y => `M9.2 ${y} H10.9 M13.1 ${y} H14.8`).join(' '), 1) }));
track('guitar12B', t => aShape(t, A_JUMBO, 13.8, 20.4, { n0: 2, head: F('M10.2 -3 H13.8 V2.4 H10.2 Z') +
  (t.D ? CUT(t, 'M12 -2.4 V1.8', 1) : '') }));

// ======== Guitars and basses ========
// Archtop / jazz hollow body: a big body, a rounded cutaway, f-holes.
const ARCH = 'M11 8.6 C8.6 8.6 6.6 10 6.6 12.2 C6.6 13.6 7.6 14.4 7.2 15.4 C6.2 16.4 4.6 17.2 4.6 19.6 C4.6 22.4 8 23.8 12 23.8 C16 23.8 19.4 22.4 19.4 19.6 C19.4 17.4 18 16.4 17.6 15.4 C17.2 14.2 17.6 12.6 16.4 11.6 C15.6 11.2 14.6 11.6 14 12.4 C13.4 12 13 11 13 10 V8.6 Z';
track('archtop', t => bShape(t, ARCH, { s: 1.1, dy: -0.6, n0: 0.2, n1: 9.4, head: F('M10.6 -2 C11.4 -2.4 12.6 -2.4 13.4 -2 L13.2 0.6 H10.8 Z'),
  inner: s => (t.D ? CUT(t, 'M8.4 15.4 C7.8 16.8 9.2 18.2 8.4 20.2 M15.6 15.4 C16.2 16.8 14.8 18.2 15.6 20.2', 1 / s) : CUT(t, 'M8.4 15.8 V19.8 M15.6 15.8 V19.8', 1 / s)) + (t.D === 2 ? CUT(t, 'M10.6 19.6 H13.4', 1 / s) : '') }));
// Resonator: the round cover plate over the cone (A: the plate in the second ink; B: rings).
const RES = (t, plate) => bShape(t, A_DREAD, { s: 1.1, dy: -0.6, n0: 0.2, n1: 9, nx: 11.1, nw: 1.8, head: A_HEAD, inner: s => plate(s) + (t.D === 2 ? bDots([[9.6, 12.4], [14.4, 12.4]], 0.9) : '') });
track('resonatorA', t => RES(t, s => C1(12, 18.2, 4, 'cut') + C1(12, 18.2, 3.3, 'f2') + (t.D ? C1(12, 18.2, 1, 'f1') : '')));
track('resonatorB', t => RES(t, s => bRing(t, 12, 18.2, 3.6, 1 / s) + bRing(t, 12, 18.2, 1.6, 1 / s)));
// Lap steel: a flat slab lying down, the steel bar across its neck.
const LAP = t => F('M1.4 9.6 C1.4 7.8 2.6 7 4.2 7 H8.4 C9.6 7 10.2 8.2 11.2 9.4 H20.4 V14.6 H11.2 C10.2 15.8 9.6 17 8.4 17 H4.2 C2.6 17 1.4 16.2 1.4 14.4 Z M20.2 9 H22.4 C22.8 9 23 9.4 23 9.8 V14.2 C23 14.6 22.8 15 22.4 15 H20.2 Z') +
  RR(4.4, 9.4, 1.4, 5.2, 0.3, 'cut') + (t.D ? CUT(t, 'M12.6 9.4 V14.6 M18.6 9.4 V14.6', 1) : '') +
  RR(14.1, 4.9, 2.8, 14.2, 1.4, 'cut') + RR(14.6, 5.4, 1.8, 13.2, 0.9, 'f2');
track('lapsteelA', t => sil(t, LAP(t)));
track('lapsteelB', t => sil(t, LAP(t), -16));
// Pedal steel: a long string table on four legs, the pedals hanging under it.
track('pedalsteel', t => sil(t, RR(1.4, 5.4, 21.2, 6, 0.8) + (t.D ? CUT(t, 'M3.2 7.6 H20.8 M3.2 9.4 H20.8', 1) : CUT(t, 'M3.2 8.4 H20.8', 1)) +
  STK(t, 'M3 11.4 L2 22.6 M21 11.4 L22 22.6', 1.2) + STK(t, 'M8.6 11.4 V19 M12 11.4 V19 M15.4 11.4 V19', 1, 's2') +
  RR(7.4, 18.8, 2.4, 3.6, 0.5) + RR(10.8, 18.8, 2.4, 3.6, 0.5) + RR(14.2, 18.8, 2.4, 3.6, 0.5)));
// Baritone: an S-type body on a long neck (the scale is what makes it a baritone).
track('baritone', t => bShape(t, E_S, { s: 1, cy: 18, dy: 1, n0: -2.2, n1: 13, head: F('M10.8 -4 H13.2 L13 -1.6 H11 Z'), inner: s => t.D ? CUT(t, 'M10.4 16.6 H13.6 M10.4 19.4 H13.6', 1) : '' }));
// Fretless bass: upright, an offset bass body, the smooth fingerboard in the second ink, four
// pegs on one side.
const JB = 'M12 15.2 C11 15.2 10.4 14.2 9.8 13 C9.2 11.8 7.6 11.6 7.2 12.8 C6.8 14.2 7.8 15.6 7.8 16.6 C7.8 17.4 6.6 18.4 6.6 20.4 C6.6 22.8 9 24 12 24 C15 24 17.4 22.8 17.4 20.6 C17.4 18.8 16.2 18 16.2 17.2 C16.2 16.2 17 15.4 16.6 14.6 C16.2 13.8 15.2 14.2 14.4 14.6 C13.6 15 13 15.2 12 15.2 Z';
track('fretlessA', t => sil(t, F(JB) + RR(10.9, 3.4, 2.2, 12.2, 0, 'f2') + F('M10.9 0 H13.1 V3.6 H10.9 Z') + F(bPegs(8.6, 10.9, [0.1, 1, 1.9, 2.8], 0.6)) + (t.D ? CUT(t, 'M10.2 19.4 H13.8', 1) : '')));
track('fretlessB', t => sil(t, F(JB) + RR(10.9, 3.4, 2.2, 12.2, 0, 'f2') + bLift(t, 'M10.9 3.4 V15.6 M13.1 3.4 V15.6') + F(PEGS) + (t.D ? CUT(t, 'M10.2 19.4 H13.8', 1) : '')));
// Acoustic bass guitar: upright, a big acoustic body with its soundhole, a long neck, 2 + 2 pegs.
track('acousticbass', t => sil(t, F('M12 10.4 C14.6 10.4 16.2 11.8 16.2 13.8 C16.2 14.8 15.6 15.4 15.6 16 C17.6 16.8 18.6 18.2 18.6 20 C18.6 22.6 15.8 24 12 24 C8.2 24 5.4 22.6 5.4 20 C5.4 18.2 6.4 16.8 8.4 16 C8.4 15.4 7.8 14.8 7.8 13.8 C7.8 11.8 9.4 10.4 12 10.4 Z') +
  RR(11.2, 3.4, 1.6, 7.6) + F(PEGS) + C1(12, 15.2, t.D ? 1.6 : 1.8, 'cut') + (t.D ? CUT(t, 'M10.2 20.6 H13.8', 1) : '')));
// Guitar amp: a combo — a handle, the control strip, one speaker.
track('guitaramp', t => sil(t, RR(2, 6.4, 20, 15.4, 1.6) + STK(t, 'M8.4 6.4 V4.4 H15.6 V6.4', 1.2) + CUT(t, 'M2.8 10 H21.2', 1) +
  bDots(t.D ? [[5, 8.2], [7.6, 8.2], [10.2, 8.2], [12.8, 8.2]] : [[5.4, 8.2], [9, 8.2]], t.D ? 0.7 : 0.8) +
  C1(12, 15.8, 4.2, 'cut') + C1(12, 15.8, t.D ? 1.4 : 1.6)));
// Bass amp: a head on a cabinet of four speakers.
track('bassamp', t => sil(t, RR(3.4, 1.2, 17.2, 5.2, 0.8) + bDots(t.D ? [[6.4, 3.8], [9, 3.8], [11.6, 3.8]] : [[6.6, 3.8]], 0.7) + (t.D ? RR(14.4, 3.1, 4.2, 1.4, 0, 'cut') : '') +
  RR(3, 7.6, 18, 15.6, 1) + [[8.2, 11.6], [15.8, 11.6], [8.2, 19.2], [15.8, 19.2]].map(([x, y]) => C1(x, y, 2.7, 'cut') + (t.D ? C1(x, y, 0.9) : '')).join('')));
// Guitar pedal: a stompbox — knobs, the footswitch, a jack each side.
track('pedal', t => sil(t, RR(5, 2.4, 14, 20, 1.8) + RR(3.2, 7, 1.9, 2.6, 0.4) + RR(18.9, 7, 1.9, 2.6, 0.4) +
  bDots(t.D ? [[8.2, 6.2], [12, 6.2], [15.8, 6.2]] : [[8.6, 6.2], [15.4, 6.2]], t.D ? 1.3 : 1.5) + (t.D === 2 ? C1(12, 9.8, 0.6, 'cut') : '') +
  CUT(t, 'M5.8 12.4 H18.2', 1) + C1(12, 17.4, 2.4, 'cut') + C1(12, 17.4, 1.2)));

// ======== Mexican and Latin strings ========
// Each carries the one feature that names it, large enough for 14 px.
const pegHead = (y0, y1, ys, w = 1.6) => F(`M10.7 ${y0} H13.3 V${y1} H10.7 Z ` + bPegs(10.7 - w, 10.7, ys, 0.7) + ' ' + bPegs(13.3, 13.3 + w, ys, 0.7));
// Requinto (jarocho): a slender body, and the long horn plectrum (espiga) that plays it.
const REQ = 'M12 9.2 C13.8 9.2 14.8 10.6 14.8 12.2 C14.8 13.4 14.2 14.2 14.2 15 C15.6 15.8 16.2 17.4 16.2 19.2 C16.2 22 14.4 23.8 12 23.8 C9.6 23.8 7.8 22 7.8 19.2 C7.8 17.4 8.4 15.8 9.8 15 C9.8 14.2 9.2 13.4 9.2 12.2 C9.2 10.6 10.2 9.2 12 9.2 Z';
const ESPIGA = (t, d) => `<path class="f2" d="${d}" transform="scale(${t.k})"/>`;
const REQI = (t, o) => aShape(t, REQ, 13.4, 20.6, Object.assign({ n0: 0.4, head: pegHead(-2.2, 0.8, [-1.8, -0.2]) }, o));
track('requintoA', t => REQI(t, { top: '' }) + ESPIGA(t, 'M2.4 3.6 L3.6 2.4 L9.6 8.2 L8.6 9.4 Z'));
track('requintoB', t => REQI(t, { top: '' }) + ESPIGA(t, 'M14.6 22.6 L15.6 21.4 L22.4 15.8 L22.8 16.6 Z'));
track('requintoC', t => REQI(t, { s: 1.16 }));
// Vihuela: a small, round body with its bowl back (the vaulted back shows beside the top).
const VIH = 'M12 10.2 C14.4 10.2 15.8 11.6 15.8 13.4 C15.8 14.4 15.3 15 15.2 15.6 C16.8 16.4 17.6 17.8 17.6 19.4 C17.6 22 15 23.6 12 23.6 C9 23.6 6.4 22 6.4 19.4 C6.4 17.8 7.2 16.4 8.8 15.6 C8.7 15 8.2 14.4 8.2 13.4 C8.2 11.6 9.6 10.2 12 10.2 Z';
track('vihuelaA', t => aShape(t, VIH, 14.4, 20.4, { n0: 0.6, head: pegHead(-2, 0.9, [-1.6, -0.1], 1.3),
  pre: s => `<g transform="translate(2.2 0.2)">${F(VIH, 'f2')}</g>` + bLift(t, VIH, 2, s) }));
track('vihuelaB', t => aShape(t, VIH, 14.4, 20.4, { n0: 0.6, head: pegHead(-2, 0.9, [-1.6, -0.1], 1.3),
  pre: s => `<g transform="translate(12 16.6) scale(1.22 1.12) translate(-12 -16.6) translate(1 0.6)">${F(VIH, 'f2')}</g>` + bLift(t, VIH, 2, s) }));
// Guitarrón: upright, a huge body over a short neck, the V of its back below.
const GTRN = 'M12 7.6 C15.8 7.6 18.6 9.2 18.6 11.8 C18.6 13.2 17.8 14 17.8 14.8 C20.4 15.8 21.8 17.4 21.8 19.4 C21.8 21.6 19.2 22.6 16 23 L12 23.8 L8 23 C4.8 22.6 2.2 21.6 2.2 19.4 C2.2 17.4 3.6 15.8 6.2 14.8 C6.2 14 5.4 13.2 5.4 11.8 C5.4 9.2 8.2 7.6 12 7.6 Z';
track('guitarron', t => sil(t, F(GTRN) + RR(11.1, 2.8, 1.8, 5.4) + pegHead(0, 3.2, [0.3, 1.9]) + C1(12, 13.2, t.D ? 2 : 2.2, 'cut') +
  (t.D ? CUT(t, 'M8 19.4 C9.6 20.4 14.4 20.4 16 19.4', 1) : '') + (t.D === 2 ? CUT(t, 'M5 21.8 L12 23 L19 21.8', 1) : '')));
// Bajo sexto: upright, a tall body (the neck joins at the 12th fret), a long head with 6 + 6 pegs.
const BSX = 'M12 6.2 C14.8 6.2 16.6 7.8 16.6 10.2 C16.6 11.8 15.8 13 15.8 14 C18 15 19.2 16.8 19.2 19.2 C19.2 22 16 23.8 12 23.8 C8 23.8 4.8 22 4.8 19.2 C4.8 16.8 6 15 8.2 14 C8.2 13 7.4 11.8 7.4 10.2 C7.4 7.8 9.2 6.2 12 6.2 Z';
track('bajosexto', t => sil(t, F(BSX) + RR(11.2, 4.8, 1.6, 2) + F('M10.6 0 H13.4 V5 H10.6 Z') +
  STK(t, (t.D ? [0.6, 2, 3.4] : [1, 3]).map(y => `M9 ${y} H10.6 M13.4 ${y} H15`).join(' '), 1) + C1(12, 13.2, t.D ? 1.8 : 2, 'cut') + (t.D ? CUT(t, 'M10 19.6 H14', 1) : '')));
// Jarana: a small body with a deep, pinched waist, its soundhole low in the lower bout.
const JAR = 'M12 10.6 C13.9 10.6 14.9 11.8 14.9 13.2 C14.9 14.2 13.9 15 13.6 15.6 C15.7 16.2 17 17.6 17 19.4 C17 21.8 14.7 23.4 12 23.4 C9.3 23.4 7 21.8 7 19.4 C7 17.6 8.3 16.2 10.4 15.6 C10.1 15 9.1 14.2 9.1 13.2 C9.1 11.8 10.1 10.6 12 10.6 Z';
track('jarana', t => aShape(t, JAR, 18.4, 21.6, { n0: 1.2, n1: 11, head: pegHead(-1.6, 1.4, [-1.2, 0.4], 1.3),
  inner: s => C1(12, 18, (t.D ? 1.4 : 1.6) / s, 'cut') + (t.D === 2 ? CUT(t, 'M10.6 21.4 H13.4', 1 / s) : '') }));
// Charango: seen from the back — the banded armadillo shell.
const CHAR = 'M12 10.4 C14.2 10.4 15.4 11.6 15.4 13.2 C15.4 14.2 14.8 14.8 14.8 15.4 C16.6 16.2 17.4 17.6 17.4 19.4 C17.4 21.8 15 23.6 12 23.6 C9 23.6 6.6 21.8 6.6 19.4 C6.6 17.6 7.4 16.2 9.2 15.4 C9.2 14.8 8.6 14.2 8.6 13.2 C8.6 11.6 9.8 10.4 12 10.4 Z';
track('charangoA', t => aShape(t, CHAR, 0, 0, { n0: 1, n1: 11, head: pegHead(-2, 1.2, [-1.6, 0], 1.3),
  inner: s => CUT(t, (t.D === 2 ? [12.6, 14.8, 17, 19.2, 21.4] : [13.4, 16.6, 19.8]).map(y => `M5 ${y} H19`).join(' '), 1 / s) }));
track('charangoB', t => aShape(t, CHAR, 0, 0, { n0: 1, n1: 11, head: pegHead(-2, 1.2, [-1.6, 0], 1.3),
  inner: s => CUT(t, (t.D === 2 ? [12.6, 14.8, 17, 19.2, 21.4] : [14.2, 18.2]).map(y => `M5 ${y} H19`).join(' '), (t.D === 2 ? 1 : 1.4) / s) }));
// Cuatro: A, the Puerto Rican body with its violin-like corners; B, the Venezuelan small guitar.
const CUATRO = 'M12 8.8 C14.6 8.8 16.2 10 16.2 12 C16.2 13 15.2 13.4 14.9 14.2 C14.8 14.8 16 15.4 16.8 16.2 C17.6 17 17.8 18.2 17.8 19.4 C17.8 21.9 15.4 23.6 12 23.6 C8.6 23.6 6.2 21.9 6.2 19.4 C6.2 18.2 6.4 17 7.2 16.2 C8 15.4 9.2 14.8 9.1 14.2 C8.8 13.4 7.8 13 7.8 12 C7.8 10 9.4 8.8 12 8.8 Z';
track('cuatroA', t => aShape(t, CUATRO, 18.2, 21.2, { head: pegHead(-2, 0.6, [-1.6, -0.2]) }));
track('cuatroB', t => aShape(t, A_OM, 14.2, 20.2, { s: 0.94, cy: 17, dy: 0.2, n0: 2.2, n1: 10, head: pegHead(-0.6, 2.4, [-0.2, 1.4]) }));
// Tres (Cuban): three courses — three string lines over the body (A), or the soundhole with three
// pairs (B).
track('tresA', t => aShape(t, A_OM, 0, 0, { nx: 11, nw: 2, inner: s => CUT(t, 'M10.2 10.4 V21 M12 10.4 V21 M13.8 10.4 V21', 1 / s) + CUT(t, 'M9.6 20.6 H14.4', 1.2 / s), head: pegHead(-2, 0.8, [-1.6, -0.2]) }));
track('tresB', t => aShape(t, A_OM, 14.2, 20.2, { nx: 11, nw: 2, head: pegHead(-2, 0.8, [-1.6, -0.2]),
  inner: s => C1(12, 14.2, 2.2 / s * 1.05, 'cut') + (t.D === 2 ? '' : '') + CUT(t, 'M10.2 15.6 V20 M12 15.6 V20 M13.8 15.6 V20', 1 / s) }));

// ======== Strings: orchestra and world ========
const VPARTS = (c = 'f1') => F(VIO, c) + RR(11.25, 2.4, 1.5, 7, 0, c) + C1(12, 1.9, 1.5, c);
// Viola: the violin family's body held at the shoulder (tilted), A; or tilted with its bow, B.
track('violaA', t => sil(t, `<g transform="translate(12 12.6) scale(1.06) translate(-12 -12.6)">${VPARTS()}</g>` + fholes(t, 15.6), -32));
track('violaB', t => sil(t, VPARTS() + fholes(t, 15.4), -30) + sil(t, STK(t, 'M2.4 15.6 L21.6 6.4', 1, 's2')));
// String section: two violins, the one behind in the second ink.
const VG = (x, y, s, inner) => `<g transform="translate(${x} ${y}) scale(${s}) translate(-12 -12)">${inner}</g>`;
track('stringsection', t => sil(t, VG(8.2, 11.2, 0.8, VPARTS('f2')) + VG(15.4, 12.8, 0.8, bLift(t, VIO, 2, 0.8) + VPARTS() + (t.D ? fholes(t, 15.4) : ''))));
// Erhu: a long pole, two pegs, a small drum at its foot, the bow across.
track('erhu', t => sil(t, RR(11.3, 1.8, 1.4, 21.4) + F('M11.3 2.2 C11.3 0.8 12.4 0.1 13.8 0.5 L13.5 1.6 C12.9 1.4 12.7 1.8 12.7 2.2 Z') +
  RR(8.4, 3.6, 3, 1.2, 0.5) + RR(8.4, 6.2, 3, 1.2, 0.5) + RR(8.2, 16.8, 7.6, 4.8, 1) + (t.D ? CUT(t, 'M14 17.6 V20.8', 1) : '') +
  STK(t, 'M2 14 C8 14.8 16 14.8 22 14', 1, 's2')));
// Koto: a long board lying aslant, its bridges on a diagonal.
track('koto', t => sil(t, F('M0.8 10 C0.8 8.6 1.6 8 2.8 8 H21.2 C22.4 8 23.2 8.6 23.2 10 V14 C23.2 15.4 22.4 16 21.2 16 H2.8 C1.6 16 0.8 15.4 0.8 14 Z') +
  (t.D ? [4.4, 7.6, 10.8, 14, 17.2, 20.4] : [5, 9.6, 14.2, 18.8]).map((x, i, a) => RR(x - 0.6, 13.6 - i * 3.6 / (a.length - 1) - 0.8, 1.2, 1.6, 0.2, 'cut')).join(''), -24));
// Guzheng: A, seen from the side (the arched board, strings over bridges, on a stand); B, from
// above, the bridges on a curve.
track('guzhengA', t => sil(t, F('M0.8 15.6 C2 11.6 7 9.6 12 9.6 C17 9.6 22 11.6 23.2 15.6 V17.2 H0.8 Z') + RR(0.8, 17.2, 22.4, 1.2, 0, 'f2') +
  STK(t, 'M1.6 8.2 H22.4', 1, 's2') + (t.D ? [4.4, 8, 12, 16, 19.6] : [6, 12, 18]).map(x => STK(t, `M${x} 8.6 V${x === 12 ? 9.6 : 10.4}`, 1)).join('') + STK(t, 'M3.4 18.4 V21 M20.6 18.4 V21', 1.2), -8));
track('guzhengB', t => sil(t, F('M1 10.4 C1 8.8 2 8 3.4 8 H20 C21.6 7.6 23 8.8 23 10.6 V13.4 C23 15.2 21.6 16.4 20 16 H3.4 C2 16 1 15.2 1 13.6 Z') +
  (t.D ? [[4.4, 13.6], [7.4, 12.2], [10.4, 11.2], [13.4, 10.6], [16.4, 10.4], [19.4, 10.6]] : [[5, 13.4], [9.6, 11.4], [14.2, 10.4], [18.8, 10.6]]).map(([x, y]) => RR(x - 0.6, y - 0.8, 1.2, 1.6, 0.2, 'cut')).join('') +
  CUT(t, 'M21 9 V15', 1), -24));
// Shamisen: a square drum body, a long neck, three pegs; B adds the large plectrum (bachi).
const SHAM = t => RR(6.2, 13.2, 11.6, 10, 2.2) + RR(7.6, 14.6, 8.8, 7.2, 1.2, 'f2') + RR(11.3, 3, 1.4, 10.4) + F('M11.1 3.4 H12.9 L13.4 0.4 H11.6 Z') +
  RR(9, 0.8, 2.4, 1, 0.4) + RR(9, 2.6, 2.4, 1, 0.4) + RR(13.1, 1.6, 2.4, 1, 0.4);
const BACHI = 'M13.6 20.6 L18.8 15 C19.6 15.4 21.8 17.4 22.2 18.4 L15.6 22.4 Z';
track('shamisenA', t => sil(t, SHAM(t)));
track('shamisenB', t => sil(t, SHAM(t) + bLift(t, BACHI, 2) + F(BACHI)));
// Pipa: a teardrop body flowing into the neck, frets across the neck and the upper body, the
// head bent back; no soundhole.
track('pipa', t => sil(t, F('M12 5.8 C14.2 5.8 18.6 13.2 18.6 17.6 C18.6 21.4 15.6 23.6 12 23.6 C8.4 23.6 5.4 21.4 5.4 17.6 C5.4 13.2 9.8 5.8 12 5.8 Z') +
  RR(11.2, 2.6, 1.6, 4) + F('M11.2 2.8 L10 0.4 C10.8 -0.1 12.4 -0.1 13.2 0.5 L12.8 2.8 Z') + RR(8.6, 1.2, 1.8, 0.9, 0.3) + RR(13.6, 1.2, 1.8, 0.9, 0.3) +
  CUT(t, t.D ? 'M10.6 4.6 H13.4 M10.4 7.4 H13.6 M9.6 9.8 H14.4 M8.8 12.2 H15.2' : 'M10.2 7.6 H13.8 M9 11.2 H15', 1) + (t.D ? CUT(t, 'M9.8 20 H14.2', 1) : '')));
// Oud: a round body, a short fretless neck, the pegbox bent back; one large and two small
// soundholes.
track('oud', t => sil(t, F('M12 7.6 C16 7.6 19.4 12 19.4 16.4 C19.4 20.8 16.2 23.6 12 23.6 C7.8 23.6 4.6 20.8 4.6 16.4 C4.6 12 8 7.6 12 7.6 Z') +
  RR(11.2, 4, 1.6, 4.4) + F('M11 4.8 L12.9 4.3 L9.8 0.2 L7.9 0.9 Z') + C1(12, 13.4, t.D ? 1.8 : 2, 'cut') +
  (t.D ? bDots([[8.8, 16.6], [15.2, 16.6]], t.D === 2 ? 0.9 : 0.8) + CUT(t, 'M10 20.4 H14', 1) : ''), 45));
// Lute: a pear body with its rosette, the pegbox at a right angle to the neck.
track('lute', t => sil(t, F('M12 8.4 C14.8 8.4 18.6 12.6 18.6 16.8 C18.6 20.8 15.6 23.6 12 23.6 C8.4 23.6 5.4 20.8 5.4 16.8 C5.4 12.6 9.2 8.4 12 8.4 Z') +
  RR(11.2, 3, 1.6, 5.8) + F('M12.8 3.6 H6.6 V1.8 H12.8 Z') + (t.D === 2 ? STK(t, 'M7.6 1.8 V0.6 M9.4 1.8 V0.6 M11.2 1.8 V0.6', 1, 's2') : '') +
  (t.D ? C1(12, 14.4, 2.2, 'cut') + C1(12, 14.4, t.D === 2 ? 1 : 0.9) : C1(12, 14.4, 2, 'cut')) + (t.D ? CUT(t, 'M10 20.4 H14', 1) : ''), 45));
// Bouzouki: a small round body on a long neck, a flat head with pegs.
track('bouzouki', t => sil(t, F('M12 12.2 C15 12.2 17.2 14.6 17.2 17.8 C17.2 21 15 23.4 12 23.4 C9 23.4 6.8 21 6.8 17.8 C6.8 14.6 9 12.2 12 12.2 Z') +
  RR(11.3, 2.8, 1.4, 9.8) + pegHead(0, 3.2, [0.3, 1.9], 1.4) + C1(12, 16, t.D ? 1.2 : 1.4, 'cut') + (t.D ? CUT(t, 'M10.4 20.6 H13.6', 1) : ''), 45));
// Balalaika: the triangular body.
track('balalaika', t => sil(t, F('M12 9.4 C12.6 9.4 13.2 10 13.8 10.8 L20 21.2 C20.6 22.4 20 23.4 18.8 23.4 H5.2 C4 23.4 3.4 22.4 4 21.2 L10.2 10.8 C10.8 10 11.4 9.4 12 9.4 Z') +
  RR(11.3, 3.6, 1.4, 6.4) + F('M10.8 0.4 H13.2 L13 4 H11 Z') + RR(9.2, 1, 1.6, 0.9, 0.3) + RR(9.2, 2.6, 1.6, 0.9, 0.3) + RR(13.2, 1.8, 1.6, 0.9, 0.3) +
  C1(12, 16.4, t.D ? 1.1 : 1.3, 'cut') + (t.D ? CUT(t, 'M10.2 20.8 H13.8', 1) : '')));
// Hammered dulcimer: a trapezoid of strings, two bridges, the two hammers below.
track('hdulcimer', t => sil(t, F('M1.2 16.4 L5.2 3.6 H18.8 L22.8 16.4 Z') + CUT(t, 'M8.8 4.8 L7.4 15.2 M15.2 4.8 L16.6 15.2', 1) + (t.D ? CUT(t, 'M5.6 7.4 H18.4 M4.4 11.2 H19.6', 1) : '') +
  STK(t, 'M4.6 23 L10 18.6 M19.4 23 L14 18.6', 1.2) + C1(10.4, 18.4, 1.3) + C1(13.6, 18.4, 1.3)));
// Mountain (Appalachian) dulcimer: a long hourglass, the fretboard down its middle, a scroll.
track('mdulcimer', t => sil(t, F('M1.4 12 C1.4 8.6 3.8 7.4 6.4 8.2 C8.2 8.8 9.2 9.4 10.8 9.4 C12.4 9.4 13.4 8 15.8 8 C18.2 8 19.8 9.8 19.8 12 C19.8 14.2 18.2 16 15.8 16 C13.4 16 12.4 14.6 10.8 14.6 C9.2 14.6 8.2 15.2 6.4 15.8 C3.8 16.6 1.4 15.4 1.4 12 Z') +
  RR(19.4, 11, 2.6, 2) + C1(22.2, 12, 1.4) + (t.D ? CUT(t, 'M3 11 H19.4 M3 13 H19.4', 1) : CUT(t, 'M3 12 H19', 1)) + (t.D === 2 ? bDots([[6, 9.8], [6, 14.2], [15.8, 9.6], [15.8, 14.4]], 0.6) : '')));
// Zither: a flat box bulging on one side, a round soundhole, many strings.
track('zither', t => sil(t, F('M1.4 5.6 H17.6 C20.6 5.6 22.6 7.6 22.6 10.6 V14 C22.6 17.4 20.4 19.4 17 19.4 H1.4 Z') +
  C1(15.4, 12.6, t.D ? 2.6 : 2.8, 'cut') + CUT(t, (t.D ? [8.2, 10.4, 12.6, 14.8, 17] : [8.6, 12.6, 16.6]).map(y => `M2.6 ${y} H12`).join(' '), 1)));
// Hurdy-gurdy: a body with the crank at its tail, the key box along the neck, the wheel cover.
const HGW = 'M5.8 8.6 C6 5.2 11.4 5.2 11.6 8.4 Z';
track('hurdygurdy', t => sil(t, F('M3.6 12.6 C3.6 8.6 6.6 7 10 7.6 L13.6 8.6 V16.6 L10 17.6 C6.6 18.2 3.6 16.6 3.6 12.6 Z') +
  RR(13.4, 8.6, 8, 8, 0.6) + F('M21.2 9.6 H22.4 C23 9.6 23.4 10 23.4 10.6 V14.6 C23.4 15.2 23 15.6 22.4 15.6 H21.2 Z') +
  (t.D ? CUT(t, 'M15.4 13.4 V16.6 M17.4 13.4 V16.6 M19.4 13.4 V16.6', 1) : CUT(t, 'M16.4 12.6 V16.6', 1)) + bLift(t, HGW, 2) + F(HGW, 'f2') +
  STK(t, 'M3.6 12.6 H1.6 V17', 1.2) + RR(0.6, 16.6, 2, 3.4, 1)));
// Koto, redrawn: the bridges across the whole board, corner to corner, and one string.
track('kotoB', t => sil(t, F('M0.8 10 C0.8 8.6 1.6 8 2.8 8 H21.2 C22.4 8 23.2 8.6 23.2 10 V14 C23.2 15.4 22.4 16 21.2 16 H2.8 C1.6 16 0.8 15.4 0.8 14 Z') +
  (t.D ? [4, 7.2, 10.4, 13.6, 16.8, 20] : [4.6, 9.4, 14.2, 19]).map((x, i, a) => RR(x - 0.6, 14.6 - i * 5.2 / (a.length - 1) - 0.8, 1.2, 1.6, 0.2, 'cut')).join('') +
  (t.D === 2 ? CUT(t, 'M2.6 9.6 H21.4', 1) : ''), -24));
// Hammered dulcimer B: the hammers lie across the board, lifted off it by a cut line.
const HAM = 'M5.4 22.6 L14.6 12.4';
track('hdulcimerB', t => sil(t, F('M1 15.4 L5 2.6 H19 L23 15.4 Z') + CUT(t, 'M8.4 3.8 L7 14.2 M15.6 3.8 L17 14.2', 1) + (t.D ? CUT(t, 'M5.2 6.4 H18.8 M4.2 10 H19.8', 1) : '') +
  [0, 4.2].map(dx => STK(t, `M${5.4 + dx} 22.6 L${14.2 + dx} 12.8`, 3, 'scut') + STK(t, `M${5.4 + dx} 22.6 L${14.2 + dx} 12.8`, 1.2) + C1(14.6 + dx, 12.4, 1.8, 'cut') + C1(14.6 + dx, 12.4, 1.2)).join('')));

// ======== Winds and brass ========
// Piccolo: the flute, half as long, at the flute's angle.
track('piccolo', t => sil(t, RR(5.4, 10.6, 13.2, 2.8, 1.4) + (t.D ? bDots([[7.6, 12]], 0.7) + bDots([[11.6, 12], [13.6, 12], [15.6, 12]], 0.6) : CUT(t, 'M11 12 H16', 1)), -28));
// Recorder: a thick beaked head with its window, a slim body with holes, a flared foot.
track('recorder', t => sil(t, F('M10.1 2 C10.1 0.6 13.9 0.6 13.9 2 L14 7.4 H10 Z M10.4 7.4 H13.6 L13.2 18.6 H10.8 Z M10.7 18.4 H13.3 C13.3 20 14.4 21.2 14.8 23 H9.2 C9.6 21.2 10.7 20 10.7 18.4 Z') +
  CUT(t, 'M10.9 4.8 H13.1', 1) + (t.D ? CUT(t, 'M10 7.6 H14', 1) + bDots([[12, 10], [12, 12.2], [12, 14.4], [12, 16.6]], 0.55) : '')));
// Pan flute: a row of tubes, tops level, lengths falling, a band across.
track('panflute', t => sil(t, (t.D ? [0, 1, 2, 3, 4, 5, 6].map(i => RR(2.4 + i * 2.8, 4, 2.3, 17.4 - i * 2.1, 1.1)) : [0, 1, 2, 3, 4].map(i => RR(2.4 + i * 3.9, 4, 3.2, 17.4 - i * 2.9, 1.5))).join('') +
  CUT(t, 'M1.6 8 H22.4', 1), -8));
// Bassoon: a long pole on a wider boot, the bocal curling out of its side.
track('bassoon', t => sil(t, RR(12.4, 0.6, 2.8, 17, 1.2) + RR(9.8, 5, 2.6, 12.4, 0.8, 'f2') + RR(9.4, 15.8, 6.2, 7.6, 1.8) +
  STK(t, 'M11 5.2 C11 3 9.4 2.2 6.8 2.6', 1.2) + F('M5.2 2.2 L6.9 2 L7 3.2 L5.2 3.2 Z') + (t.D ? CUT(t, 'M12.4 4 H15.2', 1) + bDots([[13.8, 9.4], [13.8, 12]], 0.6) : ''), 16));
// English horn: a slim body ending in a round bulb, the bent crook at its top.
track('englishhorn', t => sil(t, STK(t, 'M12 3.6 C12 1.8 10.8 1 9.2 0.8', 1) + F('M11.1 3.2 H12.9 L13.3 17.6 H10.7 Z') +
  F('M12 16.4 C14.6 16.4 15.6 18.4 15.6 20.2 C15.6 22.2 13.9 23.6 12 23.6 C10.1 23.6 8.4 22.2 8.4 20.2 C8.4 18.4 9.4 16.4 12 16.4 Z') +
  (t.D ? bDots([[12, 7], [12, 9.8], [12, 12.6]], 0.6) + C1(12, 22.6, 0.8, 'cut') : '')));
// Bass clarinet: a long straight body, a bent neck at the top, a small bell turned up at the foot.
track('bassclarinetA', t => sil(t, STK(t, 'M12 5 C12 2.6 10.6 1.4 8.4 1.4', 1.2) + RR(6.4, 0.6, 2.4, 1.6, 0.6) + RR(10.8, 4.6, 2.4, 13.4) +
  F('M10.8 17.4 V18.8 C10.8 21.6 12.6 23.4 15 23.4 C17.2 23.4 18.8 22 19 19.8 L19.8 15.4 H17 L16.8 19.2 C16.6 20.4 15.8 21 15 21 C14 21 13.2 20.2 13.2 18.8 V17.4 Z') +
  (t.D ? bDots([[12, 7.6], [12, 10.2], [12, 12.8]], 0.6) : '')));
track('bassclarinetB', t => sil(t, STK(t, 'M12 5 C12 2.6 10.6 1.4 8.4 1.4', 1.2) + RR(6.4, 0.6, 2.4, 1.6, 0.6) + RR(10.8, 4.6, 2.4, 14.6) +
  F('M10.8 18.4 C10.8 21.8 12.6 23.4 15 23.4 C16.4 23.4 17.4 22.6 18 21.4 L21.4 19.2 L20.2 17.4 L16.8 19.6 C16.4 20.6 15.8 21 15 21 C14 21 13.2 20.2 13.2 18.4 Z') +
  (t.D ? bDots([[12, 7.6], [12, 10.2], [12, 12.8], [12, 15.4]], 0.6) : '')));
// Soprano sax: straight and conical, a flared bell; held aslant.
track('sopranosax', t => sil(t, F('M11.5 0.4 H12.5 L12.6 2 L13.6 16 C13.8 18 15.6 20.4 16.2 22.8 H7.8 C8.4 20.4 10.2 18 10.4 16 L11.4 2 Z') +
  (t.D ? bDots([[12, 6], [12, 8.8], [12.1, 11.6], [12.2, 14.4]], 0.6) : ''), 26));
// Trumpet family: the parts, so the cornet and flugelhorn are built the same way.
const VALVES = (x0, y0, y1, pitch = 2.4) => [0, 1, 2].map(i => `M${x0 + i * pitch} ${y0} H${x0 + i * pitch + 1.6} V${y1} H${x0 + i * pitch} Z`).join(' ') + ` M${x0 - 0.4} ${y0 - 1} H${x0 + 2 * pitch + 2} V${y0 + 0.2} H${x0 - 0.4} Z`;
// Cornet: compact — a short bell, the tube wound in a tight loop under the valves.
track('cornet', t => sil(t, F('M3.8 11.2 H5.6 V12.8 H3.8 Z M5.4 11.4 H14.8 V12.6 H5.4 Z M14.4 10.8 C16.6 10.6 18.4 8.6 20.6 7.2 V16.8 C18.4 15.4 16.6 13.4 14.4 13.2 Z') +
  F(VALVES(8, 6.4, 11.4)) + STK(t, 'M6.6 12.6 V15.8 C6.6 17.6 7.8 18.6 9.6 18.6 H12.6 C14.2 18.6 15 17.6 15 16 V13.2', t.S >= 30 ? 2 : 1.2)));
// Flugelhorn: a long, wide conical bell, the mouthpiece shank angled down.
track('flugelhorn', t => sil(t, STK(t, 'M1.8 15.8 L5.2 12.4', 1.2) + RR(4.6, 11.4, 6.6, 1.4) + F(VALVES(5.6, 5.6, 11.4, 2.2)) +
  F('M10.6 10.8 C14 10.2 17.4 6.6 22.8 3.6 V20.4 C17.4 17.4 14 13.8 10.6 13.2 Z')));
// Euphonium: a small tuba, its bell turned up and forward, valves on top.
track('euphonium', t => sil(t, `<g transform="rotate(26 12 12)">` + F('M6.4 1.6 H17.6 C15.8 3.6 14.6 5.4 14.6 8 H9.4 C9.4 5.4 8.2 3.6 6.4 1.6 Z') +
  F('M9.4 7.8 H14.6 V15.2 C14.6 18.4 13.4 20.6 12 20.6 C10.6 20.6 9.4 18.4 9.4 15.2 Z') + `</g>` +
  RR(4.4, 9, 1.4, 5.6) + RR(6.4, 9, 1.4, 5.6) + RR(3.8, 8.2, 4.6, 1.2) + STK(t, 'M5 14.6 V18 C5 20.4 7 21.6 9.6 21.4', 1.2)));
// Sousaphone: the big ring of tubing, the huge bell facing front above it.
track('sousaphone', t => sil(t, bRing(t, 9.4, 15.2, 6.2, t.S >= 30 ? 2.4 : 1.6, 's1') + C1(15.4, 8.2, 6.8, 'cut') + C1(15.4, 8.2, 6.2) + bRing(t, 15.4, 8.2, 4.2, 1) + (t.D ? C1(15.4, 8.2, 2.2, 'f2') : '')));
// Horn section: two trumpets, the one behind in the second ink.
const TPT = c => F('M2.6 11.2 H4.6 V12.8 H2.6 Z M4.4 11.4 H15.4 V12.6 H4.4 Z M15 10.6 C17.6 10.4 19.6 7.8 22 6.2 V17.8 C19.6 16.2 17.6 13.6 15 13.4 Z', c) +
  F('M7.4 6.6 H9 V11.4 H7.4 Z M9.8 6.6 H11.4 V11.4 H9.8 Z M12.2 6.6 H13.8 V11.4 H12.2 Z M7 5.6 H14.2 V6.8 H7 Z', c);
const TPTO = 'M2.6 11.2 H4.6 V11.4 H15 C17.6 10.4 19.6 7.8 22 6.2 V17.8 C19.6 16.2 17.6 13.6 15 13.4 V12.6 H4.4 V12.8 H2.6 Z M7 5.6 H14.2 V11.4 H7 Z';
track('hornsection', t => sil(t, VG(11, 7.4, 0.84, TPT('f2')) + VG(12.6, 15, 0.84, bLift(t, TPTO, 2, 0.84) + TPT('f1'))));
// Woodwind section: a saxophone behind (second ink), a clarinet in front.
const SAXP = 'M5 1.4 C8.6 1 11.6 2.4 12.2 5.2 V16.6 C12.2 18.2 13.2 19.2 14.6 19.2 C16 19.2 16.8 18.2 16.8 16.8 V12.8 C16.8 11 18.2 9.6 20.4 9.4 L20.8 11.8 C19.6 11.9 19.2 12.6 19.2 13.6 V16.8 C19.2 19.8 17.2 21.8 14.6 21.8 C11.8 21.8 9.8 19.8 9.8 16.8 V5.6 C9.6 4 7.8 3.2 5.2 3.4 Z';
const CLAR = 'M11.2 3.2 L11.6 0.8 H12.4 L12.8 3.2 Z M10.8 3 H13.2 V19 H10.8 Z M10.4 18.8 H13.6 L15.4 22.8 H8.6 Z';
track('woodwindsection', t => sil(t, VG(10, 12, 0.9, F(SAXP, 'f2')) + VG(15.2, 12, 1, bLift(t, CLAR, 2) + F(CLAR))));
// Bagpipes: the bag, three drones rising from it, the chanter below, the blowpipe.
track('bagpipes', t => sil(t, F('M4.6 13 C4.6 9.2 7.6 7.4 11 8 C14.8 8.8 17.6 11.4 17.6 15 C17.6 18.4 14.8 20.2 11.4 20 C7.4 19.8 4.6 16.8 4.6 13 Z') +
  F('M9.4 8.4 L13.6 0.6 L15 1.3 L11 8.9 Z M12.2 8.8 L16.6 3 L17.8 3.9 L13.6 9.8 Z M14.6 10.2 L19.6 6 L20.6 7.1 L16 11.4 Z') +
  F('M6.6 17.2 L8.2 18.2 L5 23.6 L3.4 23 Z M6 9.8 L2.6 4.6 L3.8 3.9 L7.2 9 Z') + (t.D ? F('M13 0.6 H15.6 V1.8 H13 Z', 'f2') : '')));
// Didgeridoo: a long thick tube widening to its far end, dotted.
track('didgeridoo', t => sil(t, F('M0.8 10.8 L16.6 10 C19 9.8 21 9 23 8.6 V15.4 C21 15 19 14.2 16.6 14 L0.8 13.2 Z') +
  (t.D ? bDots([[4.6, 12], [8, 12], [11.4, 12], [14.8, 12], [18.6, 10.8], [18.6, 13.2], [21, 12]], 0.55) : CUT(t, 'M4 12 H11 M14 12 H20', 1)), -30));
// Shakuhachi: thick bamboo, the mouth cut on a slant, the nodes, the root flaring at the foot.
track('shakuhachi', t => sil(t, F('M10.6 2.2 L13.4 0.8 V17.6 C13.4 19.6 14.6 21.2 14 23.2 H10 C9.4 21.2 10.6 19.6 10.6 17.6 Z') +
  CUT(t, t.D ? 'M10.6 7 H13.4 M10.6 12.4 H13.4' + (t.D === 2 ? ' M10.6 17.6 H13.4' : '') : 'M10.6 9.6 H13.4', 1), 10));
// Ocarina: an egg with a mouthpiece spout and finger holes.
track('ocarina', t => sil(t, F('M3.8 12.6 C3.8 8.6 8.2 6.6 12.6 6.8 C17.6 7 21.8 9.6 21.8 13.2 C21.8 16.8 17.8 18.6 12.8 18.4 C7.8 18.2 3.8 16.6 3.8 12.6 Z M0.8 10.4 L5.4 9.8 V13.6 L0.8 13.2 Z') +
  (t.D ? bDots([[10.4, 10.2], [13.4, 9.6], [16.4, 10], [19, 11.4], [12, 15.6]], 0.75) + RR(6.2, 9.8, 1.4, 1.2, 0, 'cut') : bDots([[11.4, 10.4], [16.6, 10.4]], 1)), -10));
// Melodica: a keyboard body, its keys cut in, the hose and mouthpiece.
track('melodica', t => sil(t, RR(6.4, 7, 16.4, 9.6, 2.4) + (t.D ? CUT(t, 'M8.2 12 H21.2', 1) + CUT(t, 'M10 12 V16.6 M12.6 12 V16.6 M15.2 12 V16.6 M17.8 12 V16.6', 1) + bDots([[9.4, 9.4], [11.8, 9.4], [16.4, 9.4], [18.8, 9.4]], 0.55) : CUT(t, 'M8.2 12 H21.2', 1)) +
  STK(t, 'M6.4 12 C3.6 12 2.6 14.4 2.8 18.2', 1) + RR(1.6, 18, 2.4, 3.6, 0.8), -14));
// Tin whistle: a thin tube under a wide mouthpiece with its window, held aslant.
track('tinwhistle', t => sil(t, RR(11.1, 6, 1.8, 17.4, 0.6) + RR(10.4, 0.6, 3.2, 6.4, 1) + CUT(t, 'M10.6 4.6 H13.4', 1) +
  (t.D ? bDots([[12, 9.6], [12, 11.8], [12, 14], [12, 16.2], [12, 18.4], [12, 20.6]], 0.5) : ''), -34));
// Kazoo: the submarine — a tapering tube with its round turret.
track('kazoo', t => sil(t, F('M1.6 11 C1.6 10 2.2 9.6 3 9.6 L16.4 8.6 C19.4 8.6 22.4 9.8 22.4 12.2 C22.4 14.6 19.4 15.8 16.4 15.8 L3 14.8 C2.2 14.8 1.6 14.4 1.6 13.4 Z') +
  RR(13.4, 5.4, 4.4, 4, 0.6) + RR(12.8, 4.6, 5.6, 1.4, 0.6) + (t.D ? CUT(t, 'M13 6.4 H18.2', 1) : ''), -12));
// Bandoneón: square end boxes studded with buttons, the bellows between (A straight, B opened
// in a V).
const BAND = t => (t.D ? [3, 6.4] : [4.6]).map(x => (t.D ? [8.4, 12, 15.6] : [9.4, 14.6]).map(y => C1(x, y, t.D ? 0.75 : 1, 'cut')).join('')).join('');
track('bandoneonA', t => sil(t, RR(8.4, 6.4, 7.2, 11.2, 0, 'f2') + CUT(t, 'M10.2 6.4 V17.6 M12 6.4 V17.6 M13.8 6.4 V17.6', 1) +
  RR(0.8, 5, 7.6, 14, 1.2) + RR(15.6, 5, 7.6, 14, 1.2) + BAND(t) + `<g transform="translate(14.8 0)">${BAND(t)}</g>`));
track('bandoneonB', t => sil(t, F('M8.2 7.6 L15.8 7.6 L15.6 16.6 L8.4 16.6 Z', 'f2') + CUT(t, 'M10.2 7.6 V16.6 M12 7.6 V16.6 M13.8 7.6 V16.6', 1) +
  `<g transform="rotate(-12 8.4 12)">${RR(0.8, 5, 7.6, 14, 1.2) + BAND(t)}</g><g transform="rotate(12 15.6 12)">${RR(15.6, 5, 7.6, 14, 1.2)}<g transform="translate(14.8 0)">${BAND(t)}</g></g>`));

// ======== Keys ========
const KEYCUTS = (t, x0, x1, y0, y1, n) => t.D ? CUT(t, Array.from({ length: n }, (_, i) => `M${(x0 + (i + 1) * (x1 - x0) / (n + 1)).toFixed(2)} ${y0} V${y1}`).join(' '), 1) : '';
// Harpsichord: A, seen from above — a long straight-sided wing, the keyboard across its foot;
// B, from the side — an angular case, the lid raised on its stick.
track('harpsichordA', t => sil(t, F('M3.4 17.6 V1.2 H6.6 C8 6.4 12.6 9 17.4 11 C20 12.2 20.8 14.2 20.8 17.6 Z') + RR(2.6, 18.6, 19, 3.4, 0.4, 'f2') +
  KEYCUTS(t, 2.6, 21.6, 18.6, 22, 7) + (t.D ? C1(10.4, 13.4, 1, 'cut') : '') + (t.D === 2 ? CUT(t, 'M5 4 L9 16.4 M7.4 8.4 L10.6 16.4', 1) : '')));
track('harpsichordB', t => sil(t, F('M1.6 12 H22.4 V15.6 H1.6 Z') + F('M2 11 L21.6 4.6 L22 5.8 L3.2 11 Z') + STK(t, 'M14.4 11 L13.4 7.4', 1) +
  RR(1.2, 15.6, 6, 1.6, 0, 'f2') + STK(t, 'M3.4 15.6 V22.4 M20.6 15.6 V22.4 M12 15.6 V22.4', 1.2)));
// Clavinet: a slanted box, its row of rocker tabs, the keys.
track('clavinet', t => sil(t, F('M2 10.4 L3.8 5.6 H20.2 L22 10.4 V17.2 H2 Z') + CUT(t, 'M2.8 11.6 H21.2', 1) +
  (t.D ? [4.2, 6.2, 8.2, 10.2].map(x => RR(x, 6.8, 1.2, 2, 0.2, 'cut')).join('') : RR(4.4, 7, 5.4, 1.8, 0.2, 'cut')) + KEYCUTS(t, 2, 22, 11.6, 14.4, 6) +
  RR(2, 17.2, 20, 1.6, 0, 'f2') + RR(2.8, 18.8, 2.4, 1.6) + RR(18.8, 18.8, 2.4, 1.6)));
// Celesta: a small cabinet with an arched top, its bell plates showing, on two legs.
track('celesta', t => sil(t, F('M4.4 4.4 C4.4 2.4 6 1.6 12 1.6 C18 1.6 19.6 2.4 19.6 4.4 V12.6 H4.4 Z') + RR(3, 12.6, 18, 3, 0.4, 'f2') + KEYCUTS(t, 3, 21, 12.6, 15.6, 5) +
  CUT(t, t.D ? 'M7 5 H17 M7.6 7.2 H16.4 M8.4 9.4 H15.6' : 'M7 5.6 H17 M8.4 9 H15.6', 1) + STK(t, 'M5.6 15.6 V22.6 M18.4 15.6 V22.6', 1.2) + (t.D === 2 ? STK(t, 'M12 15.6 V21.4', 1, 's2') : '')));
// Mellotron / tape keys: a cabinet whose sloped top shows two tape reels, the keyboard below.
track('mellotron', t => sil(t, F('M2 10.4 L4.4 3.2 H19.6 L22 10.4 V22.4 H2 Z') + C1(8.4, 6.8, 2.4, 'cut') + C1(15.6, 6.8, 2.4, 'cut') + (t.D ? C1(8.4, 6.8, 0.8) + C1(15.6, 6.8, 0.8) + CUT(t, 'M8.4 4.4 H15.6', 1) : '') +
  CUT(t, 'M2.8 11.6 H21.2', 1) + RR(2.8, 12.6, 18.4, 3.6, 0, 'f2') + KEYCUTS(t, 2.8, 21.2, 12.6, 16.2, 6)));
// Tonewheel organ: two manuals on a cabinet, the drawbars pulled out above them.
track('tonewheel', t => sil(t, (t.D ? [[4, 3.4], [5.6, 5.4], [7.2, 2.6], [8.8, 4.6], [10.4, 6]] : [[4.4, 3.6], [7.2, 2.4], [10, 5]]).map(([x, y]) => STK(t, `M${x} ${y} V9`, 1, 's2') + RR(x - 0.7, y - 0.6, 1.4, 1.6, 0.3)).join('') +
  RR(1.6, 9.4, 20.8, 3, 0.4) + RR(1, 13.2, 22, 3, 0.4) + KEYCUTS(t, 1.6, 22.4, 10.6, 12.4, 6) + KEYCUTS(t, 1, 23, 14.4, 16.2, 7) +
  RR(2.4, 16.8, 19.2, 3, 0, 'f2') + RR(3, 19.8, 2, 3.2) + RR(19, 19.8, 2, 3.2)));
// Electric pianos. Tine: the suitcase — keys on a speaker cabinet. Reed: the compact case with
// its rounded lid on splayed legs.
track('tinepiano', t => sil(t, F('M2 8.6 C2 6.6 3 5.6 4.6 5.6 H19.4 C21 5.6 22 6.6 22 8.6 V9.6 H2 Z') + RR(2, 9.6, 20, 2.8, 0, 'f2') + KEYCUTS(t, 2, 22, 9.6, 12.4, 7) +
  RR(3, 13.2, 18, 9.6, 0.8) + C1(8.2, 18, 2.8, 'cut') + C1(15.8, 18, 2.8, 'cut') + (t.D ? C1(8.2, 18, 0.9) + C1(15.8, 18, 0.9) : '')));
track('reedpiano', t => sil(t, F('M3 10.6 C3 7.4 5 5.4 8.4 5.4 H17.6 C20.2 5.4 21 7 21 9 V15.4 H3 Z') + CUT(t, 'M3.8 11.4 H20.2', 1) + KEYCUTS(t, 3, 21, 11.4, 13.8, 6) +
  STK(t, 'M5.6 15.4 L3.6 22.8 M18.4 15.4 L20.4 22.8 M5.6 15.4 L7.8 22.8 M18.4 15.4 L16.2 22.8', 1.2)));
// Toy piano: A, a little upright with an arched top, chunky keys, bun feet; B, a little grand.
track('toypianoA', t => sil(t, F('M4.6 17.6 V9.4 C4.6 5.8 7.6 4 12 4 C16.4 4 19.4 5.8 19.4 9.4 V17.6 Z') + RR(3.6, 12.4, 16.8, 3.6, 0.4, 'f2') +
  CUT(t, 'M7.8 12.4 V16 M12 12.4 V16 M16.2 12.4 V16', 1) + (t.D ? C1(12, 8, 1.4, 'cut') : '') + C1(7, 19.6, 1.8) + C1(17, 19.6, 1.8)));
track('toypianoB', t => sil(t, F('M3.6 15.6 V10.4 C3.6 7.4 6.2 6.2 8.8 7 C10.6 7.6 11.6 9.6 14.4 10.2 C17.4 10.8 20.4 11.8 20.4 14 V15.6 Z') + RR(3.6, 16.4, 16.8, 2.4, 0, 'f2') +
  C1(6, 20.4, 1.8) + C1(18, 20.4, 1.8) + CUT(t, 'M8 16.4 V18.8 M12 16.4 V18.8 M16 16.4 V18.8', 1)));
// Keytar: a keyboard slung like a guitar — the body with its keys, the neck reaching up.
const KTB = 'M1.6 14 C1.6 11.4 3.2 10 5.4 10 H15.6 L20.6 4.8 C21.4 4 22.8 4.8 22.4 6 L17.6 12.2 V15.2 C17.6 17 16.4 18 14.6 18 H5.4 C3.2 18 1.6 16.6 1.6 14 Z';
track('keytarA', t => sil(t, F(KTB) + RR(3.2, 13.4, 13, 3, 0, 'f2') + KEYCUTS(t, 3.2, 16.2, 13.4, 16.4, 6) + (t.D ? bDots([[5, 11.6], [7.4, 11.6]], 0.6) : ''), -14));
track('keytarB', t => sil(t, F(KTB) + RR(3.2, 13.4, 13, 3, 0, 'f2') + KEYCUTS(t, 3.2, 16.2, 13.4, 16.4, 6) + (t.D ? bDots([[5, 11.6], [7.4, 11.6]], 0.6) : ''), -32));
// MIDI controller keyboard: keys, a row of pads, two wheels, and its cable.
track('midikeyboard', t => sil(t, RR(1.6, 8.4, 20.8, 11.4, 1.2) + RR(2.8, 14, 18.4, 4.6, 0, 'f2') + KEYCUTS(t, 2.8, 21.2, 14, 18.6, 7) +
  (t.D ? [10.6, 13.4, 16.2, 19].map(x => RR(x, 9.6, 2.2, 3, 0.4, 'cut')).join('') : RR(10.4, 9.6, 2.8, 3, 0.4, 'cut') + RR(15.4, 9.6, 2.8, 3, 0.4, 'cut')) +
  RR(3.4, 9.6, 1.2, 3.2, 0.6, 'cut') + RR(5.8, 9.6, 1.2, 3.2, 0.6, 'cut') + STK(t, 'M12 8.4 C12 5 14.6 4 17.4 4.4 C19.6 4.6 20.8 3.4 21.4 1.4', 1, 's2')));

// ======== The chosen variants (picked at 14, 16 and 24 px, 1:1 and 4x; see spec-tracks-add-b.md) ========
Object.assign(N, {
  classical: N.classicalB, guitar12: N.guitar12B, resonator: N.resonatorA, lapsteel: N.lapsteelA, fretless: N.fretlessA,
  requinto: N.requintoA, vihuela: N.vihuelaB, charango: N.charangoA, cuatro: N.cuatroA, tres: N.tresA,
  viola: N.violaA, koto: N.kotoB, guzheng: N.guzhengA, shamisen: N.shamisenA, hdulcimer: N.hdulcimerB,
  bassclarinet: N.bassclarinetB, bandoneon: N.bandoneonA, harpsichord: N.harpsichordA, toypiano: N.toypianoA, keytar: N.keytarB,
});

// ======== The picker's families (the set-6 icons of each family first, then the new ones) ========
window.TRACK_ADD_B = [
  ['Guitars & basses', ['eguitar', 'aguitar', 'bassguitar', 'archtop', 'resonator', 'lapsteel', 'pedalsteel', 'baritone', 'fretless', 'acousticbass', 'guitaramp', 'bassamp', 'pedal']],
  ['Electric guitar shapes', ['eStype', 'eTtype', 'eSingleCut', 'eDoubleHorn', 'eSuperstrat', 'eSemiHollow', 'eOffset', 'eVshape', 'ePointed']],
  ['Acoustic guitar shapes', ['aDread', 'aConcert', 'aJumbo', 'aParlor', 'aCutaway', 'classical', 'guitar12', 'aTravel']],
  ['Mexican & Latin strings', ['requinto', 'vihuela', 'guitarron', 'bajosexto', 'jarana', 'charango', 'cuatro', 'tres']],
  ['Strings', ['violin', 'viola', 'cello', 'doubleBass', 'stringsection', 'harp', 'banjo', 'mandolin', 'ukulele', 'sitar', 'erhu', 'koto', 'guzheng', 'shamisen', 'pipa', 'oud', 'lute', 'bouzouki', 'balalaika', 'hdulcimer', 'mdulcimer', 'zither', 'hurdygurdy']],
  ['Winds & brass', ['sax', 'sopranosax', 'trumpet', 'cornet', 'flugelhorn', 'trombone', 'horn', 'euphonium', 'tuba', 'sousaphone', 'hornsection', 'flute', 'piccolo', 'clarinet', 'bassclarinet', 'oboe', 'englishhorn', 'bassoon', 'woodwindsection',
    'recorder', 'tinwhistle', 'panflute', 'shakuhachi', 'ocarina', 'didgeridoo', 'bagpipes', 'harmonica', 'melodica', 'kazoo', 'accordion', 'bandoneon']],
  ['Keys', ['piano', 'upright', 'harpsichord', 'celesta', 'toypiano', 'organ', 'tonewheel', 'epiano', 'tinepiano', 'reedpiano', 'clavinet', 'mellotron', 'synthkeys', 'midikeyboard', 'keytar', 'keys']],
];
// id: [English, Spanish, other search words]. Maker and model names appear only here, as
// search aliases, never as labels.
const WORDS_B = {
  archtop: ['archtop guitar', 'guitarra archtop', 'jazz guitar, hollow body, guitarra de jazz, hueca'],
  resonator: ['resonator guitar', 'guitarra resonadora', 'dobro, resophonic, resofónica, slide'],
  lapsteel: ['lap steel', 'lap steel', 'steel guitar, hawaiian, guitarra hawaiana, slide'],
  pedalsteel: ['pedal steel', 'pedal steel', 'steel guitar, country, guitarra de pedales'],
  baritone: ['baritone guitar', 'guitarra barítono', 'baritone, barítono'],
  fretless: ['fretless bass', 'bajo sin trastes', 'fretless, bajo fretless'],
  acousticbass: ['acoustic bass guitar', 'bajo acústico', 'acoustic bass, bajo electroacústico'],
  guitaramp: ['guitar amp', 'amplificador de guitarra', 'amp, combo, ampli'],
  bassamp: ['bass amp', 'amplificador de bajo', 'amp, cabinet, stack, ampli, gabinete'],
  pedal: ['guitar pedal', 'pedal de efectos', 'stompbox, effects pedal, pedalera, distortion, distorsión'],
  eStype: ['S-type double cutaway', 'tipo S, doble cutaway', 'strat, double cutaway, doble corte'],
  eTtype: ['T-type single cutaway', 'tipo T, un cutaway', 'tele, single cutaway, un corte'],
  eSingleCut: ['single-cut carved top', 'cutaway simple tallada', 'les paul, single cut, tapa tallada'],
  eDoubleHorn: ['double-horn solid body', 'sólida de dos cuernos', 'sg, horns, cuernos'],
  eSuperstrat: ['superstrat', 'superstrat', 'superstrat, pointed horns, cuernos puntiagudos, shred, metal'],
  eSemiHollow: ['semi-hollow', 'semihueca', '335, f-holes, efes, semiacústica'],
  eOffset: ['offset body', 'cuerpo offset', 'offset, asimétrica, surf, indie'],
  eVshape: ['V-shape', 'forma de V', 'v, metal'],
  ePointed: ['pointed body', 'cuerpo puntiagudo', 'pointy, metal, picos'],
  aDread: ['dreadnought', 'dreadnought', 'dread, western, folk, acústica'],
  aConcert: ['concert / OM', 'concierto / OM', 'orchestra model, om, auditorium, auditorio'],
  aJumbo: ['jumbo', 'jumbo', 'big body, cuerpo grande'],
  aParlor: ['parlor', 'parlor', 'parlour, salón, blues, pequeña'],
  aCutaway: ['cutaway dreadnought', 'dreadnought con cutaway', 'cutaway, electroacústica'],
  classical: ['classical guitar', 'guitarra clásica', 'nylon, spanish guitar, guitarra española, criolla, flamenco'],
  guitar12: ['12-string guitar', 'guitarra de 12 cuerdas', 'twelve string, doce cuerdas, 12 string'],
  aTravel: ['travel / mini guitar', 'guitarra de viaje', 'mini, 3/4, baby, niño, small'],
  requinto: ['requinto', 'requinto', 'requinto jarocho, espiga, son jarocho, requinto romántico, trío'],
  vihuela: ['vihuela', 'vihuela', 'mariachi, vihuela mexicana'],
  guitarron: ['guitarrón', 'guitarrón', 'guitarron, mariachi, bass, bajo'],
  bajosexto: ['bajo sexto', 'bajo sexto', 'bajo quinto, norteño, tejano, conjunto, 12 string bass'],
  jarana: ['jarana', 'jarana', 'jarana jarocha, son jarocho, huasteca'],
  charango: ['charango', 'charango', 'andean, andino, quirquincho, armadillo'],
  cuatro: ['cuatro', 'cuatro', 'cuatro puertorriqueño, cuatro venezolano, llanero'],
  tres: ['tres (Cuban)', 'tres cubano', 'tres, son cubano'],
  viola: ['viola', 'viola', 'alto, viola da braccio'],
  stringsection: ['string section', 'sección de cuerdas', 'strings, ensemble, orchestra, orquesta, cuerdas'],
  erhu: ['erhu', 'erhu', 'chinese fiddle, violín chino, huqin'],
  koto: ['koto', 'koto', 'japanese zither, cítara japonesa'],
  guzheng: ['guzheng', 'guzheng', 'zheng, chinese zither, cítara china'],
  shamisen: ['shamisen', 'shamisen', 'sanshin, japanese, japonés'],
  pipa: ['pipa', 'pipa', 'chinese lute, laúd chino'],
  oud: ['oud', 'laúd árabe', 'ud, oud, arabic, árabe'],
  lute: ['lute', 'laúd', 'renaissance lute, renacentista, barroco'],
  bouzouki: ['bouzouki', 'buzuki', 'irish, greek, griego, irlandés'],
  balalaika: ['balalaika', 'balalaica', 'russian, rusa'],
  hdulcimer: ['hammered dulcimer', 'salterio', 'santur, santoor, cimbalom, yangqin, dulcémele'],
  mdulcimer: ['mountain dulcimer', 'dulcémele apalache', 'appalachian, lap dulcimer, apalaches'],
  zither: ['zither', 'cítara', 'concert zither, autoharp, autoarpa'],
  hurdygurdy: ['hurdy-gurdy', 'zanfona', 'zanfoña, viella, organistrum'],
  piccolo: ['piccolo', 'flautín', 'octavín'],
  recorder: ['recorder', 'flauta dulce', 'flauta de pico, block flute'],
  panflute: ['pan flute', 'zampoña', 'flauta de pan, siku, antara, panpipes'],
  bassoon: ['bassoon', 'fagot', 'fagotto'],
  englishhorn: ['English horn', 'corno inglés', 'cor anglais'],
  bassclarinet: ['bass clarinet', 'clarinete bajo', 'clarón'],
  sopranosax: ['soprano sax', 'saxofón soprano', 'straight sax, saxo recto'],
  cornet: ['cornet', 'corneta', 'cornetín'],
  flugelhorn: ['flugelhorn', 'fliscorno', 'flügelhorn, fiscorno'],
  euphonium: ['euphonium', 'bombardino', 'eufonio, baritone horn, barítono'],
  sousaphone: ['sousaphone', 'sousafón', 'helicón, tuba de banda, banda sinaloense'],
  hornsection: ['horn section', 'sección de metales', 'brass section, metales, horns, vientos'],
  woodwindsection: ['woodwind section', 'sección de maderas', 'winds, maderas, vientos'],
  bagpipes: ['bagpipes', 'gaita', 'gaita gallega, highland pipes, dudelsack'],
  didgeridoo: ['didgeridoo', 'didgeridú', 'didjeridu, yidaki'],
  shakuhachi: ['shakuhachi', 'shakuhachi', 'bamboo flute, flauta de bambú'],
  ocarina: ['ocarina', 'ocarina', ''],
  melodica: ['melodica', 'melódica', 'pianica, armónica de teclado'],
  tinwhistle: ['tin whistle', 'flauta irlandesa', 'penny whistle, whistle, silbato irlandés'],
  kazoo: ['kazoo', 'kazoo', 'mirlitón'],
  bandoneon: ['bandoneon', 'bandoneón', 'tango, concertina'],
  harpsichord: ['harpsichord', 'clavecín', 'clave, cembalo, clavicémbalo'],
  clavinet: ['clavinet', 'clavinet', 'clavi, funk keys'],
  celesta: ['celesta', 'celesta', 'celeste'],
  mellotron: ['mellotron / tape keys', 'teclado de cintas', 'mellotron, tape, cintas'],
  tonewheel: ['tonewheel organ', 'órgano de ruedas fónicas', 'drawbars, hammond, b3, órgano eléctrico, jazz organ'],
  tinepiano: ['tine electric piano', 'piano eléctrico de púas', 'rhodes, suitcase, ep, tines, e-piano'],
  reedpiano: ['reed electric piano', 'piano eléctrico de lengüetas', 'wurlitzer, wurli, ep, reeds, e-piano'],
  toypiano: ['toy piano', 'piano de juguete', 'toy, juguete, kids'],
  keytar: ['keytar', 'keytar', 'shoulder keyboard, teclado colgante'],
  midikeyboard: ['MIDI controller', 'controlador MIDI', 'midi keyboard, teclado midi, controller, pads'],
};
window.TRACK_WORDS = Object.assign(window.TRACK_WORDS || {}, WORDS_B);
window.TRACK_ADD_B_NEW = Object.keys(WORDS_B);
