// Cantus icon family — set 6, the final pass over the whole library (round 39, icons-final).
// Loaded after every other glyphs-t-*.js file and before glyphs-t-families.js.
//
// 1. Redraws of the weak glyphs, each with 2–3 variants (suffix A, B, C) drawn on the 24-unit
//    sheet and built per size like the rest (`track`, `sil`, `F`, `CUT`, `STK`, `C1`, `RR`; the
//    additions' helpers `window.ADDA`, `bShape`, `aShape`, `pegHead`). The chosen variant takes
//    the plain id at the end of the file; spec-tracks.md says why.
// 2. The consistency rules the pass applied in the shared helpers (glyphs-t-tracks3.js:
//    `lineW`, `cutW`): a drawn line is 1 device px below 30 and 2 px from 30, a cut is 1 px;
//    the plucked instruments that lie down lie at 45°, body forward.
// Everything sits in one function scope so no name collides with the other files.
(() => {
  const N = window.N;
  N.theremin0 = N.theremin;
  const { E, G, P, rod, halo, CYM, CYMdown, tomShell } = window.ADDA;
  // A rod drawn as a stroke in sheet units (it thickens with the size, as a mass does).
  const TUBE = (d, w, c = 's1') => `<path class="${c}" fill="none" stroke-width="${w}" stroke-linecap="round" stroke-linejoin="round" d="${d}"/>`;
  const HALO = (t, d, w) => `<path class="scut" fill="none" stroke-width="${w}" stroke-linecap="round" stroke-linejoin="round" d="${d}"/>`;
  window.FINAL = { TUBE, HALO };

  // ============================================================ Drums
  // Snare: a shallow shell, its head in the second ink, lugs cut in the shell; two sticks
  // crossed over it (the set-6 snare was an outline, the only drum not drawn as a silhouette).
  const STICK = (x0, y0, x1, y1, c = 'f1') => F(rod(x0, y0, x1, y1, 1.9, 1.3), c) + C1(x1, y1, 1.2, c);
  track('snareA', t => sil(t, tomShell(2.4, 21.6, 13, 19.4, 2.6) + (t.D ? CUT(t, 'M6.4 16.4 V20.4 M12 16.8 V21.4 M17.6 16.4 V20.4', 1) : '') +
    STICK(20.8, 2, 10.4, 11.6, 'f2') + HALO(t, 'M3.2 2 L13.6 11.6', t.S >= 30 ? 3.6 : 3) + STICK(3.2, 2, 13.6, 11.6)));
  track('snareB', t => sil(t, tomShell(2.4, 21.6, 10, 18.8, 2.8) + (t.D ? CUT(t, 'M6.4 13.4 V19.6 M12 13.8 V20.6 M17.6 13.4 V19.6', 1) : '') +
    CUT(t, 'M3.6 19.8 C8 21.6 16 21.6 20.4 19.8', 1) + STK(t, 'M4.6 21.6 C8.6 23 15.4 23 19.4 21.6', 1, 's2') +
    STICK(20.8, 1.4, 11.2, 8.4, 'f2') + HALO(t, 'M3.2 1.4 L12.8 8.4', t.S >= 30 ? 3.6 : 3) + STICK(3.2, 1.4, 12.8, 8.4)));
  // Tom: a deep shell alone, the rack and floor toms carry their mounts.
  track('tomA', t => sil(t, tomShell(3.4, 20.6, 6.6, 19, 2.8) + (t.D ? CUT(t, 'M7.4 10.6 V20.4 M12 11 V21.6 M16.6 10.6 V20.4', 1) : CUT(t, 'M12 10.8 V21.4', 1))));
  // Tambourine: the half-moon frame with its jingles in their slots (the round frame drum
  // with a head is the pandeiro; set 6's ring read as a circle mark).
  const MOON = 'M2 13.6 A10 10 0 0 1 22 13.6 L17.6 13.6 A5.6 5.6 0 0 0 6.4 13.6 Z';
  const jingles = (t, n) => (n === 3 ? [150, 90, 30] : [140, 105, 75, 40]).map(a => G(`rotate(${a - 90} 12 13.6)`,
    RR(10.4, 4.2, 3.2, 3.6, 0.4, 'cut') + RR(10.7, 4.9, 2.6, 2.2, 1.1, 'f2'))).join('');
  track('tambourineA', t => sil(t, G('rotate(-18 12 12)', F(MOON) + jingles(t, t.D === 2 ? 4 : 3) + (t.D ? RR(10.6, 12.4, 2.8, 4.4, 1.2) : RR(10.4, 12.4, 3.2, 4.6, 1.4)))));
  track('tambourineB', t => sil(t, F(MOON) + jingles(t, t.D === 2 ? 4 : 3) + RR(10.4, 13.6, 3.2, 7.6, 1.4)));

  // ============================================================ Cymbals
  // Hi-hat: the instrument — the pair closed at the top of its stand, the tripod, the pedal.
  track('hihatA', t => sil(t, F(CYM(12, 6.8, 9.4, 2.2, 1.9, 1.5)) + CYMdown(12, 9.8, 9.4, 2, 1.8, 1.3) + STK(t, 'M12 1.4 V3.4', 1) +
    STK(t, 'M12 11.4 V21.6', 1, 's2') + (t.D ? STK(t, 'M12 17.6 L6.6 22.2 M12 17.6 L17.4 22.2', 1, 's2') : STK(t, 'M7.6 21.6 H12', 1, 's2')) +
    F('M13 22.8 L21.4 19.6 C22 19.4 22.6 20 22.2 20.6 L13.8 23.8 Z')));
  // Cymbal: one cymbal, level, on its stand and tripod (the crash tilts and sparks, the ride
  // has its stick, the splash is small).
  track('cymbalA', t => sil(t, F(CYM(12, 8.8, 10.6, 2.6, 2.2, 2)) + STK(t, 'M12 10.6 V21.6', 1, 's2') +
    (t.D ? STK(t, 'M12 17.4 L6.4 22.2 M12 17.4 L17.6 22.2', 1, 's2') : STK(t, 'M8 21.6 H16', 1, 's2'))));
  // Open hi-hat: the top cymbal lifted clear and tilted; the stack (below) sits bell to bell.
  track('hihatOpenA', t => sil(t, G('rotate(-12 12 6)', F(CYM(12, 6, 9.4, 2.2, 1.9, 1.5))) + CYMdown(12, 14.4, 9.4, 2, 1.8, 1.3) +
    STK(t, 'M12 1 V2.6', 1) + STK(t, 'M12 8.2 V12.6', 1) + STK(t, 'M12 16 V21.6', 1, 's2') + (t.D ? STK(t, 'M8 21.6 H16', 1, 's2') : '')));
  // Stacked cymbals: a small cymbal upside down sitting on a big one, the wing nut on top.
  track('cymbalStackA', t => sil(t, CYMdown(12, 9.4, 5.6, 2, 1.7, 1.2) + F(CYM(12, 13.4, 10.6, 2.4, 2, 1.5)) +
    STK(t, 'M12 5.4 V9.4', 1) + RR(9.8, 4.6, 4.4, 1.2, 0.6) + STK(t, 'M12 14.8 V21.6', 1, 's2') + (t.D ? STK(t, 'M8 21.6 H16', 1, 's2') : '')));
})();

// ============================================================ Latin, world, tuned percussion
(() => {
  const { E, G, P, rod, halo } = window.ADDA;
  const { TUBE, HALO } = window.FINAL;
  // Cuíca: (A) the drum lying on its side, the head at the left with the knot of the stick,
  // the open back at the right where the bamboo stick comes out, the squeal before the head;
  // (B) the same seen a little from the head, the head a large ellipse.
  track('cuicaA', t => sil(t, RR(7.4, 6.6, 12, 10.8) + E(19.4, 12, 1.8, 5.4) + E(19.4, 12, 1, 4.2, 'cut') +
    E(7.4, 12, 2.6, 5.4) + E(7.4, 12, 1.7, 4.3, 'f2') + C1(7.6, 12, 0.95) + (t.D ? CUT(t, 'M11.4 6.8 V17.2 M15.4 6.8 V17.2', 1) : '') +
    TUBE('M8.4 12 H23', 1.3) + STK(t, t.D ? 'M3.4 7 C1.8 8.6 4 10.2 2.4 12 C0.8 13.8 3 15.4 1.4 17' : 'M3.2 8.2 C1.4 10 3.6 12 1.8 14.2', 1, 's2')));
  track('cuicaB', t => sil(t, F('M9 5 H18.6 C20.6 5 21.8 8.2 21.8 12 C21.8 15.8 20.6 19 18.6 19 H9 Z') + E(9, 12, 4.8, 7) + E(9, 12, 3.8, 6, 'f2') + C1(9.2, 12, 1.2) +
    (t.D ? CUT(t, 'M13.6 5.4 V18.6 M17.4 5.4 V18.6', 1) : '') + STK(t, t.D ? 'M2.6 3 C1.4 4.2 2.4 5.6 1.4 7' : 'M2.4 3.4 C1.4 4.6 2.4 6 1.4 7.2', 1, 's2')));
  // Vibraslap: (A) the box low at the left, the bent rod rising in a J to the ball at the top
  // right; (B) the rod as a U, box and ball both up.
  track('vibraslapA', t => sil(t, G('rotate(-16 6 18.6)', RR(1, 15.4, 10.4, 6.4, 1.4) + (t.D ? CUT(t, 'M3 18.6 H9.4', 1) : '')) +
    TUBE('M10 15.4 C15.6 13.6 18.4 10.4 18.6 7.2', t.S >= 30 ? 2 : 1.7) + C1(18.8, 5, 3.4)));
  track('vibraslapB', t => sil(t, G('rotate(-20 6 6)', RR(1, 2.6, 10.4, 6.6, 1.4) + (t.D ? CUT(t, 'M3 5.9 H9.4', 1) : '')) +
    TUBE('M7.2 9.8 C7.2 19.8 12.6 22.2 16.4 20.6 C19.6 19.2 20.2 15.2 19.4 11.2', t.S >= 30 ? 2 : 1.7) + C1(19.2, 7.8, 3.4)));
  // Crotales: (A) three thick discs, graduated, each with its boss, in a row on their rail;
  // (B) one disc large, its boss, the brass mallet on its edge and the ting;
  // (C) the chromatic set, two rows of discs offset like a keyboard's, and the mallet.
  const crot = (t, x, y, r, c = 'f1') => E(x, y + 0.9, r, r * 0.42, c) + RR(x - r, y, 2 * r, 0.9, 0, c) + E(x, y, r, r * 0.42, c) +
    (t.D ? E(x, y - 0.1, r * 0.62, r * 0.26, 'cut') : '') + E(x, y - 0.5, r * 0.34, r * 0.3, c);
  const cutRim = (x, y, r) => E(x, y + 0.4, r + 0.8, r * 0.42 + 0.8, 'cut');
  track('crotalesA', t => sil(t, crot(t, 5.6, 9, 3.6) + cutRim(11.6, 13.2, 4.4) + crot(t, 11.6, 13.2, 4.4) + cutRim(17.4, 17.8, 5.2) + crot(t, 17.4, 17.8, 5.2) +
    STK(t, 'M2 12.2 L22.4 21.8', 1, 's2')));
  track('crotalesB', t => sil(t, crot(t, 10.4, 15.4, 8.6) + HALO(t, 'M14.2 12.6 L22.2 2.6', t.S >= 30 ? 3.4 : 3) + F(rod(22.2, 2.6, 14.6, 12.2, 1.4, 1.4)) + C1(14.2, 12.6, 1.8) +
    STK(t, t.D ? 'M18 15.4 L21.8 14.2 M17.4 18.2 L20.6 19.8' : 'M18 15.6 L21.6 14.6', 1)));
  track('crotalesC', t => sil(t, [4.6, 12, 19.4].map(x => crot(t, x, 16.6, 3.2)).join('') + [8.3, 15.7].map(x => crot(t, x, 9.4, 3.2)).join('') +
    (t.D ? STK(t, 'M14 3 L21.4 1', 1, 's2') + C1(13.4, 3.2, 1.3, 'f2') : '')));
})();
(() => {
  const { E, G } = window.ADDA;
  const { TUBE } = window.FINAL;
  // Cuíca (C): B's drum seen from the head, the bamboo stick coming out of its open back,
  // two arcs of the squeal before the head.
  track('cuicaC', t => sil(t, F('M8.4 5.4 H17.4 C19.2 5.4 20.4 8.4 20.4 12 C20.4 15.6 19.2 18.6 17.4 18.6 H8.4 Z') + E(8.4, 12, 4.4, 6.6) + E(8.4, 12, 3.3, 5.5, 'f2') + C1(8.6, 12, 1.1) +
    (t.D ? CUT(t, 'M14.4 5.8 V18.2', 1) : '') + E(19.2, 12, 1, 3.6, 'cut') + TUBE('M18.6 12 L23.4 13.6', t.S >= 30 ? 1.6 : 1.4) +
    (t.D ? STK(t, 'M2 7.6 C0.8 10 0.8 14 2 16.4', 1, 's2') : '')));
  // Crotales (D): the chromatic set from above, two rows of discs offset like a keyboard's,
  // each with its boss.
  const disc = (t, x, y, r) => C1(x, y, r) + (t.D ? `<circle class="scut" fill="none" stroke-width="${px(t, 1)}" cx="${x}" cy="${y}" r="${(r * 0.55).toFixed(2)}"/>` + C1(x, y, 0.8) : C1(x, y, 1, 'cut'));
  track('crotalesD', t => sil(t, [8, 16].map(x => disc(t, x, 7.6, 3.6)).join('') + [4, 12, 20].map(x => disc(t, x, 15.6, 3.6)).join('') +
    STK(t, 'M1.6 21.8 H22.4', 1, 's2')));
})();

// ============================================================ Electronic & production, voice, mix
(() => {
  const { G } = window.ADDA;
  const { TUBE } = window.FINAL;
  // Acid bassline (no maker's box): (A) the knobs in a row over a one-octave keyboard;
  // (B) the knobs over the squelch, a saw whose every edge rings.
  const knobs = (t, y) => (t.D ? [4.8, 8.6, 12.4, 16.2, 19.6] : [5.6, 12, 18.4]).map(x => C1(x, y, t.D ? 1.35 : 1.8, 'cut')).join('');
  track('acidA', t => sil(t, RR(1.4, 3.4, 21.2, 17.2, 1.6) + knobs(t, 7.4) + RR(3, 11.2, 18, 7.6, 0, 'cut') +
    (t.D ? [3.6, 6.2, 8.8, 11.4, 14, 16.6, 19.2].map(x => RR(x, 11.8, 2, 6.4, 0, 'f2')).join('') + [5.4, 8, 13.2, 15.8, 18.4].map(x => RR(x, 11.8, 1.4, 3.8)).join('')
      : RR(3.6, 11.8, 16.8, 6.4, 0, 'f2') + [6.6, 10.6, 15, 18.6].map(x => RR(x - 0.8, 11.8, 1.6, 3.8)).join(''))));
  track('acidB', t => sil(t, RR(1.4, 3.4, 21.2, 17.2, 1.6) + knobs(t, 7.4) +
    CUT(t, t.D ? 'M3.6 17.6 L8.6 11.8 C9 15.6 9.8 15.8 10.4 14.4 L11.2 17.6 L16.2 11.8 C16.6 15.6 17.4 15.8 18 14.4 L18.8 17.6 H20.4' : 'M3.6 17.6 L9 11.8 L10.2 17.6 L15.6 11.8 L16.8 17.6 H20.4', 1)));
  // Theremin: (A) the instrument at every size — the cabinet on its stand, the pitch rod up at
  // the right, the volume loop at the left, from 16 px the hand's air by the rod; (B) the hand
  // by the rod from 16 px, the instrument below it.
  const THER = t => RR(4.6, 10.6, 13.4, 5, 1) + STK(t, 'M18 12.6 H19.6 V1.4', 1) + STK(t, 'M4.6 11.6 H3 C1 11.6 1 14.6 3 14.6 H4.6', 1) +
    STK(t, 'M11.3 15.6 V21.8', 1, 's2') + STK(t, 'M7 21.8 H15.6', 1, 's2') + (t.D === 2 ? CUT(t, 'M7 13.1 H11.4', 1) : '');
  track('thereminA', t => sil(t, THER(t) + (t.D ? STK(t, 'M16.8 3.4 C15.6 4.8 15.6 7.2 16.8 8.6', 1, 's2') : '') +
    (t.D === 2 ? STK(t, 'M14.4 2.4 C12.6 4.4 12.6 7.6 14.4 9.6', 1, 's2') : '')));
  track('thereminB', t => t.D ? N.theremin0[t.S].svg : sil(t, THER(t)));
  // Talk box: the lips, the tube out of their corner, down into the driver box with its
  // footswitch. (A) lips at the top left, large; (B) lips over the box, the tube between.
  const LIPS2 = 'M1.6 7.8 C3.4 5 5.4 3.8 7.2 4.8 C8 5.2 8.4 5.8 9 5.8 C9.6 5.8 10 5.2 10.8 4.8 C12.6 3.8 14.6 5 16.4 7.8 C14.6 11 12 12.4 9 12.4 C6 12.4 3.4 11 1.6 7.8 Z';
  track('talkboxA', t => sil(t, F(LIPS2) + CUT(t, 'M3.6 8 C6.4 8.8 11.6 8.8 14.4 8', 1) + TUBE('M15.2 8.2 C20.4 8.2 21.4 11.2 19.4 14.4', t.S >= 30 ? 1.8 : 1.4) +
    RR(13.2, 14.4, 9.8, 8.4, 1.2) + C1(18.1, 18.6, t.D ? 2 : 1.9, 'cut') + (t.D === 2 ? C1(18.1, 18.6, 0.9) : '')));
  track('talkboxB', t => sil(t, `<g transform="translate(3 -1.6)">${F(LIPS2)}${CUT(t, 'M3.6 8 C6.4 8.8 11.6 8.8 14.4 8', 1)}</g>` +
    TUBE('M17.6 6.6 C22 7.6 22 12.4 15 14.4', t.S >= 30 ? 1.8 : 1.4) + RR(6, 14.4, 12, 8.4, 1.2) + C1(12, 18.6, t.D ? 2 : 1.9, 'cut')));
  // Vocal chop: (A) a microphone's head cut in three slices, each knocked aside — a voice cut up.
  track('vocalChopA', t => sil(t, G('translate(1.4 0)', F('M7.4 6.2 C7.4 3.6 9.4 1.6 12 1.6 C14.6 1.6 16.6 3.6 16.6 6.2 Z')) +
    RR(6.2, 7.4, 9.2, 3.4) + G('translate(0.8 0)', F('M7.4 12 H16.6 C16.6 14.8 14.6 16.6 12 16.6 C9.4 16.6 7.4 14.8 7.4 12 Z')) +
    STK(t, 'M12.8 16.8 V21.8 M9 21.8 H16.6', 1, 's2')));
  // Surround: the listener at the centre, sound arriving from every side: (A) five speakers
  // without the ring below 16 px; (B) four arcs closing on the centre.
  track('surroundA', t => (t.D ? t.C(12, 12, 8.4, 's2') : '') + t.C(12, 12, t.D ? 2.4 : 2.6, 'f1') +
    [[12, 3.4], [4.4, 8.6], [19.6, 8.6], [6.6, 19.4], [17.4, 19.4]].map(([x, y]) => t.R(x - 2.3, y - 2.3, x + 2.3, y + 2.3, 'f1', 0.6)).join(''));
  track('surroundB', t => t.C(12, 12, 2.6, 'f1') + [45, 135, 225, 315].map(a => {
    const r = 8.6, s = (a - 28) * Math.PI / 180, e = (a + 28) * Math.PI / 180;
    return t.Ps(`M${(12 + r * Math.cos(s)).toFixed(2)} ${(12 + r * Math.sin(s)).toFixed(2)} A${r} ${r} 0 0 1 ${(12 + r * Math.cos(e)).toFixed(2)} ${(12 + r * Math.sin(e)).toFixed(2)}`);
  }).join('') + (t.D ? [0, 90, 180, 270].map(a => t.C(12 + 9 * Math.cos(a * Math.PI / 180), 12 + 9 * Math.sin(a * Math.PI / 180), 1, 'f2')).join('') : ''));
})();

// ============================================================ Guitars, Mexican & Latin strings, strings
(() => {
  const { G, rod } = window.ADDA;
  const { TUBE, HALO } = window.FINAL;
  const hole = (t, y, s, r = 1.8) => C1(12, y, (t.D ? r : r + 0.2) / s * 1.05, 'cut');
  const bridge = (t, y, s) => t.D ? CUT(t, `M10.2 ${y} H13.8`, 1 / s) : '';
  // Three acoustic outlines made to read apart at 14 px, where only the outline survives:
  // the dreadnought square-shouldered with a shallow waist; the concert / OM a deep waist
  // under a small upper bout; the cutaway dreadnought the same box with its treble shoulder
  // scooped away down to the waist.
  const DREAD2 = 'M12 8.6 C15.8 8.6 17.6 9 17.8 11 C18 13 17.2 14.2 17.4 15.4 C17.6 16.6 19 17.6 19 20 C19 22.6 16.4 23.8 12 23.8 C7.6 23.8 5 22.6 5 20 C5 17.6 6.4 16.6 6.6 15.4 C6.8 14.2 6 13 6.2 11 C6.4 9 8.2 8.6 12 8.6 Z';
  const OM2 = 'M12 10.2 C14.2 10.2 15.4 11.4 15.4 13.2 C15.4 14.4 14.4 15 14.2 15.6 C16.8 16.4 18 17.8 18 19.8 C18 22.4 15.4 23.8 12 23.8 C8.6 23.8 6 22.4 6 19.8 C6 17.8 7.2 16.4 9.8 15.6 C9.6 15 8.6 14.4 8.6 13.2 C8.6 11.4 9.8 10.2 12 10.2 Z';
  const CUT2 = 'M12 8.6 C12.9 8.6 13.4 9 13.4 9.8 C13.4 12.4 14.6 14 16.6 14 C17.6 14 17.6 14.8 17.4 15.4 C17.6 16.6 19 17.6 19 20 C19 22.6 16.4 23.8 12 23.8 C7.6 23.8 5 22.6 5 20 C5 17.6 6.4 16.6 6.6 15.4 C6.8 14.2 6 13 6.2 11 C6.4 9 8.2 8.6 12 8.6 Z';
  const MID = 'M12 9.2 C14.8 9.2 16.4 10.6 16.4 12.6 C16.4 13.8 15.6 14.6 15.4 15.2 C17.4 16 18.6 17.6 18.6 19.8 C18.6 22.4 16 23.8 12 23.8 C8 23.8 5.4 22.4 5.4 19.8 C5.4 17.6 6.6 16 8.6 15.2 C8.4 14.6 7.6 13.8 7.6 12.6 C7.6 10.6 9.2 9.2 12 9.2 Z';
  const ac = (t, body, hy, by, o = {}) => aShape(t, body, hy, by, Object.assign({ inner: s => hole(t, hy, s) + bridge(t, by, s) + (o.more ? o.more(s) : '') }, o));
  track('aDreadA', t => ac(t, DREAD2, 13.8, 20.4));
  track('aConcertA', t => ac(t, OM2, 17, 20.8, { n1: 10.6 }));
  track('aCutawayA', t => ac(t, CUT2, 13.8, 20.4));
  // Acoustic guitar (the generic one): a middle outline and the steel-string's pickguard.
  track('aguitarC', t => ac(t, MID, 14.2, 20.4, { more: s => F('M13.4 15.6 C15.6 15.4 16.8 16.6 16.6 18.4 C15 18.6 13.6 17.6 13.4 15.6 Z', 'f2') }));
  // Electric guitar (the generic one): the double cutaway at the shapes' size and angle, its
  // cable running out of the jack (only an electric has one).
  track('eguitarC', t => bShape(t, E_S, { inner: PU(t, [15.6, 17.6, 19.8]) }) +
    (t.D ? sil(t, STK(t, 'M10.6 21.4 C12 23.4 15.6 23 17.6 21.4 C19.2 20.2 21 20.4 22.4 21.8', 1)) : ''));
  // Jarana: slim, a pinched waist, a wide head for its eight strings (four pegs a side).
  const JAR2 = 'M12 9.8 C13.6 9.8 14.4 11 14.4 12.6 C14.4 13.8 13.6 14.8 13.4 15.4 C15.2 16 16.2 17.6 16.2 19.6 C16.2 22.2 14.4 23.8 12 23.8 C9.6 23.8 7.8 22.2 7.8 19.6 C7.8 17.6 8.8 16 10.6 15.4 C10.4 14.8 9.6 13.8 9.6 12.6 C9.6 11 10.4 9.8 12 9.8 Z';
  track('jaranaA', t => aShape(t, JAR2, 18.4, 21.6, { n0: 0.6, n1: 11, head: pegHead(-3.4, 1.2, t.D ? [-3, -1.8, -0.6, 0.6] : [-2.8, -0.6], 1.8),
    inner: s => C1(12, 18.2, (t.D ? 1.4 : 1.6) / s, 'cut') + (t.D === 2 ? CUT(t, 'M10.6 21.6 H13.4', 1 / s) : '') }));
  // Cuatro: the Puerto Rican body, its violin-like corners pushed out at the waist so they
  // read at 14 px; two pegs a side.
  const CUA2 = 'M12 9 C14.8 9 16.4 10.2 16.4 12.2 C16.4 13.2 15.8 13.8 15.2 14.2 L17.4 14.4 L16.2 15.8 C17.6 16.8 18.4 18 18.4 19.6 C18.4 22.2 15.8 23.8 12 23.8 C8.2 23.8 5.6 22.2 5.6 19.6 C5.6 18 6.4 16.8 7.8 15.8 L6.6 14.4 L8.8 14.2 C8.2 13.8 7.6 13.2 7.6 12.2 C7.6 10.2 9.2 9 12 9 Z';
  track('cuatroC', t => ac(t, CUA2, 18.2, 21.4, { head: pegHead(-2, 0.6, [-1.6, -0.2]) }));
  // Requinto: its body, and the long horn plectrum (espiga) laid across it in the main ink.
  track('requintoD', t => REQI(t, {}) + sil(t, HALO(t, 'M2.8 6.6 L13 16.8', t.S >= 30 ? 3.6 : 3) + F(rod(2.8, 6.6, 13, 16.8, 1.6, 1)) + C1(2.8, 6.6, 0.9)));
  // Guzheng: from the side, the long arched board, the strings straight over the slanted
  // bridges, the curled end at the left; no stand (it read as a table).
  const archY = x => 10.6 + 4 * ((x - 12) / 11.2) ** 2;
  track('guzhengC', t => sil(t, F('M0.8 15 C3 11.6 7.6 10.6 12 10.6 C16.4 10.6 21 11.6 23.2 15 V17 C21 16 16.4 15.4 12 15.4 C7.6 15.4 3 16 0.8 17 Z') +
    F('M0.8 15 C0.2 13.6 0.8 12.4 2.2 12.6 L2.6 14 Z') + STK(t, 'M1.6 7.6 H22.4', 1, 's2') +
    (t.D === 2 ? [4, 7, 10, 13, 16, 19] : t.D ? [5, 9, 13, 17] : [6.4, 12, 17.6]).map(x => F(`M${x - 0.9} ${archY(x).toFixed(2)} L${x} 8 L${x + 0.9} ${archY(x).toFixed(2)} Z`)).join(''), -6));
})();

// ============================================================ Winds & brass, keys
(() => {
  const { TUBE } = window.FINAL;
  // Euphonium: (A) a small tuba with its bell turned to the upper right, the U of the bow
  // below, the valve block on the left branch and the lead pipe.
  track('euphoniumA', t => sil(t, F('M11.4 1.2 H22.4 C20.4 3 19 5 18.8 7.8 H15 C14.8 5 13.4 3 11.4 1.2 Z') +
    F('M15 7.6 H18.8 V15.8 C18.8 20.2 16.2 22.8 12.2 22.8 C8.2 22.8 6.2 20.2 6.2 16.8 V14.2 H9.4 V16.6 C9.4 18.6 10.6 19.8 12.4 19.8 C14.2 19.8 15 18.6 15 16.6 Z') +
    RR(4.4, 9, 1.3, 5.4) + RR(6.2, 9, 1.3, 5.4) + RR(8, 9, 1.3, 5.4) + RR(3.8, 8, 6.2, 1.3, 0.4) + STK(t, 'M4.4 11.4 C2.4 11.4 1.6 9.6 2 7', 1) +
    (t.D ? CUT(t, 'M15.6 9.6 H18.2', 1) : '')));
  // Piccolo: (A) a short flute, thicker than the flute's line, its lip plate and keys;
  // (B) the same with the bright spark of its octave.
  const PIC = t => RR(4.2, 10.4, 15.6, 3.2, 1.6) + RR(6.2, 9.4, 2.4, 1.2, 0.4) + (t.D ? [11.2, 13.4, 15.6].map(x => C1(x, 12, 0.7, 'cut')).join('') : CUT(t, 'M11 12 H16', 1));
  track('piccoloA', t => sil(t, PIC(t), -28));
  const spark = (cx, cy, r) => F(`M${cx} ${cy - r} L${cx + r * 0.3} ${cy - r * 0.3} L${cx + r} ${cy} L${cx + r * 0.3} ${cy + r * 0.3} L${cx} ${cy + r} L${cx - r * 0.3} ${cy + r * 0.3} L${cx - r} ${cy} L${cx - r * 0.3} ${cy - r * 0.3} Z`, 'f2');
  track('piccoloB', t => sil(t, `<g transform="translate(-1.2 2.2)">${sil({ k: 1 }, PIC(t), -28)}</g>` + spark(18.4, 5.2, t.D ? 3.6 : 4)));
  // Clavinet: (A) from above — the string bed with its two pickups over the keyboard, on
  // legs; (B) the case alone, from above.
  const CLAV = (t, y0) => RR(1.6, y0, 20.8, 13.6, 1.2) + RR(3, y0 + 1.4, 18, 5, 0, 'cut') +
    (t.D ? [y0 + 2.4, y0 + 3.9, y0 + 5.4] : [y0 + 3.9]).map(y => RR(3, y - 0.35, 18, 0.7, 0, 'f2')).join('') +
    RR(7.4, y0 + 1, 1.8, 5.8) + RR(14.8, y0 + 1, 1.8, 5.8) + KEYCUTS(t, 1.6, 22.4, y0 + 8.4, y0 + 13.6, 9) + (t.D ? '' : CUT(t, `M12 ${y0 + 8.4} V${y0 + 13.6}`, 1));
  track('clavinetA', t => sil(t, CLAV(t, 3.4) + STK(t, 'M3.6 17 L2.4 22.6 M20.4 17 L21.6 22.6', 1.2)));
  track('clavinetB', t => sil(t, CLAV(t, 5.2)));
})();

// ============================================================ The chosen (by looking at 1:1 and 4×)
(() => {
  const N = window.N;
  window.TRACK_VARIANTS_FINAL = [
    ['snare', ['snareA', 'snareB'], 'snareA', 'a silhouette like every other drum; A (sticks crossed over a shallow shell) over B (the snares under it, which 14 px loses)'],
    ['tom', ['tomA'], 'tomA', 'the outline tom made a silhouette with its lugs'],
    ['tambourine', ['tambourineA', 'tambourineB'], 'tambourineA', 'the ring read as the circle mark; the half-moon with jingles, tilted, over the upright one (a fan)'],
    ['hihat', ['hihatA'], 'hihatA', 'the instrument on its stand with tripod and pedal, apart from the closed and open articulations'],
    ['cymbal', ['cymbalA'], 'cymbalA', 'one level cymbal on a tripod at the size of the other cymbals (set 6 had a thin tilted bar)'],
    ['hihatOpen', ['hihatOpenA'], 'hihatOpenA', 'the top cymbal lifted further and tilted'],
    ['cymbalStack', ['cymbalStackA'], 'cymbalStackA', 'a small cymbal on a big one with its wing nut — read as the open hi-hat before'],
    ['cuica', ['cuicaA', 'cuicaB', 'cuicaC'], 'cuicaC', 'the drum seen from its head with the stick out of the open back; A read as a lantern, B as a can'],
    ['vibraslap', ['vibraslapA', 'vibraslapB'], 'vibraslapB', 'the U with box and ball, larger and with a heavier rod; the J lay flat at 14 px'],
    ['crotales', ['crotalesA', 'crotalesB', 'crotalesC', 'crotalesD'], 'crotalesD', 'the set from above, two rows like a keyboard; side views were thin smears at 14 px'],
    ['acid', ['acidA', 'acidB'], 'acidB', 'knobs over the squelch: a box with keys read as every other box'],
    ['theremin', ['thereminA', 'thereminB'], 'thereminB', 'below 16 px the instrument (cabinet, rod, loop); the hand by the rod from 16 px, where it reads'],
    ['talkbox', ['talkboxA', 'talkboxB'], 'talkboxA', 'lips twice as large, the tube out of their corner'],
    ['vocalChop', ['vocalChopA'], 'vocalChopA', 'a microphone head cut in three; the chopped lips read as a ball'],
    ['surround', ['surroundA', 'surroundB'], 'surroundA', 'five speakers and the listener, the ring only from 16 px'],
    ['aDread', ['aDreadA'], 'aDreadA', 'square shoulders and a shallow waist'],
    ['aConcert', ['aConcertA'], 'aConcertA', 'a deep waist under a small upper bout'],
    ['aCutaway', ['aCutawayA'], 'aCutawayA', 'the dreadnought with its treble shoulder scooped to the waist'],
    ['aguitar', ['aguitarC'], 'aguitarC', 'at the shapes\' size and 45°, with the steel-string\'s pickguard'],
    ['eguitar', ['eguitarC'], 'eguitarC', 'at the shapes\' size and 45°, with its cable from 16 px'],
    ['jarana', ['jaranaA'], 'jaranaA', 'slim, pinched, a wide head of eight strings'],
    ['cuatro', ['cuatroC'], 'cuatroC', 'the Puerto Rican corners pushed out'],
    ['requinto', ['requintoD'], 'requintoD', 'the espiga across the body in the main ink'],
    ['guzheng', ['guzhengC'], 'guzhengC', 'from the side without its stand (it read as a table): arch, strings, slanted bridges'],
    ['euphonium', ['euphoniumA'], 'euphoniumA', 'a small tuba with its bell to the side and the valve block (it read as a saxophone)'],
    ['piccolo', ['piccoloA', 'piccoloB'], 'piccoloB', 'a thicker short flute with the spark of its octave; alone it was a dash at 14 px'],
    ['clavinet', ['clavinetA', 'clavinetB'], 'clavinetA', 'from above: strings and two pickups over the keys, on legs'],
  ];
  window.TRACK_VARIANTS_FINAL.forEach(([id, , pick]) => { N[id] = N[pick]; });
})();

// ============================================================ Electric shapes (the owner, 2026-11-09:
// "la guitarra single cut y la offset se ven muy raras y feas") — generic outlines, drawn from
// blank sheets, body forward at 45° with the other shapes (`bShape`).
(() => {
  const PU2 = (t, ys) => s => t.D === 2 ? CUT(t, ys.map(y => `M10.4 ${y} H13.6`).join(' '), 1 / s) : '';
  // Single cut, carved top: a round lower bout, a clear waist, a smaller upper bout, and one
  // cutaway, on the treble side only, leaving a short horn.
  const SC_A = 'M11 10.4 C8.6 10.2 7 11.4 7 13 C7 14.2 7.9 14.8 7.8 15.6 C7.6 16.6 5.4 17.6 5.4 20 C5.4 22.5 8.2 23.8 12 23.8 C15.8 23.8 18.6 22.5 18.6 20 C18.6 17.6 16.6 16.8 16.4 15.8 C16.2 14.8 17 14 17 12.4 C17 11.4 16.8 10.6 16.2 10.4 C15.6 10.4 15.4 11.2 15 11.8 C14.4 12.6 13.6 12.8 13 12.6 V10.4 Z';
  const SC_B = 'M11 10.2 C8.4 10.2 6.8 11.6 6.8 13.4 C6.8 14.6 7.8 15.2 7.6 16 C7.4 16.8 5.2 17.8 5.2 20.2 C5.2 22.6 8 23.8 12 23.8 C16 23.8 18.8 22.6 18.8 20.2 C18.8 17.8 16.6 16.8 16.4 15.8 C16.2 14.8 17.4 13.6 17.6 11.6 C17.7 10.6 17.6 9.8 17.4 9.4 C16.4 10.4 15.8 11.8 14.8 12.4 C14.2 12.8 13.4 12.8 13 12.4 V10.2 Z';
  const SC_C = SC_A;
  track('eSingleCutA', t => bShape(t, SC_A, { inner: PU2(t, [16.2, 19.6]) }));
  track('eSingleCutB', t => bShape(t, SC_B, { inner: PU2(t, [16.2, 19.6]) }));
  // C: A with the carved top's binding showing as an inner contour from 24 px.
  track('eSingleCutC', t => bShape(t, SC_C, { inner: s => (t.D === 2 ? `<path class="scut" fill="none" stroke-width="${(px(t, 1) / s).toFixed(3)}" d="M7.6 19.8 C7.6 21.8 9.4 22.6 12 22.6 C14.6 22.6 16.6 21.8 16.6 19.8"/>` : '') + PU2(t, [16, 19])(s) }));
  // Offset: two horns of unequal length (the bass horn long), the waist lower on the treble
  // side than on the bass side, the lower bout fuller on the bass side — the body looks
  // offset without being turned.
  const OF_A = 'M13 12.4 C13.6 11.4 14.4 10.4 15.4 10.4 C16.4 10.4 16.8 11.2 16.6 12.4 C16.4 13.8 16 15 16.6 16.2 C17.2 17.4 18.8 18.4 18.8 20.4 C18.8 22.6 16.2 23.8 12.2 23.8 C8 23.8 5 22.4 5 19.6 C5 17.6 6.8 16.6 7 15 C7.2 13.6 6.2 11.4 6.2 9.4 C6.2 7.6 7.2 6.8 8.2 7.4 C9.4 8 10.4 10.4 11 12.4 Z';
  const OF_B = 'M13 12.6 C13.6 11.8 14.6 11 15.6 11 C16.6 11 17 11.8 16.8 12.8 C16.6 14 15.8 15.4 16.6 16.6 C17.4 17.8 19 18.6 19 20.6 C19 22.8 16.4 23.8 12.2 23.8 C7.8 23.8 4.8 22.2 4.8 19.4 C4.8 17.2 6.6 16 6.8 14.4 C7 12.8 5.6 10.6 5.6 8.6 C5.6 6.8 6.8 6.2 7.8 6.8 C9.2 7.6 10.4 10.4 11 12.6 Z';
  const OF_C = 'M13 12.4 C13.8 11.6 15 11.2 16 11.6 C16.8 12 16.8 12.8 16.4 13.8 C16 15 16.2 16 17.2 16.8 C18.4 17.8 19.2 18.8 19 20.6 C18.8 22.8 16.2 23.8 12.4 23.8 C8.2 23.8 5 22.6 5 19.8 C5 17.8 6.4 16.6 6.6 15.2 C6.8 13.8 5.4 12 5.4 9.8 C5.4 7.6 6.6 6.8 7.8 7.4 C9.2 8.2 10.2 10.4 11 12.4 Z';
  track('eOffsetA', t => bShape(t, OF_A, { inner: PU2(t, [16.2, 19.6]) }));
  track('eOffsetB', t => bShape(t, OF_B, { inner: PU2(t, [16.4, 19.8]) }));
  track('eOffsetC', t => bShape(t, OF_C, { inner: PU2(t, [16.4, 19.8]) }));
})();
(() => {
  const PU2 = (t, ys) => s => t.D === 2 ? CUT(t, ys.map(y => `M10.4 ${y} H13.6`).join(' '), 1 / s) : '';
  // Single cut D: A with a deeper waist and a rounder bass shoulder.
  const SC_D = 'M11 10.2 C8.2 10.2 6.6 11.4 6.6 13.2 C6.6 14.4 7.8 15 7.8 15.8 C7.8 16.8 5.2 17.6 5.2 20.1 C5.2 22.6 8.2 23.8 12 23.8 C15.8 23.8 18.8 22.6 18.8 20.1 C18.8 17.6 16.2 16.8 16.2 15.8 C16.2 14.8 17.2 13.8 17.2 12.2 C17.2 11.2 16.9 10.4 16.3 10.3 C15.6 10.3 15.3 11.3 14.9 11.9 C14.4 12.6 13.6 12.8 13 12.6 V10.2 Z';
  track('eSingleCutD', t => bShape(t, SC_D, { inner: PU2(t, [16.2, 19.6]) }));
  // Offset D: the bass horn long and broad, the treble side only a rounded shoulder; the waist
  // high and shallow on the bass side, low and deep on the treble side; the lower bout
  // fuller on the treble side — offset without turning the body.
  const OF_D = 'M13 12.6 C13.8 12 15.2 11.8 16 12.4 C16.6 12.9 16.4 13.8 15.8 14.6 C15.4 15.4 15.6 16.4 16.6 17 C18.2 17.8 19.2 19 19.2 20.6 C19.2 22.8 16.6 23.8 12.4 23.8 C8 23.8 4.8 22.6 4.8 19.6 C4.8 17.4 6.4 16 6.6 14.6 C6.8 13 5.4 11.2 5.6 9 C5.8 7 7.2 6.4 8.4 7.2 C9.8 8.2 10.4 10.4 11 12.6 Z';
  track('eOffsetD', t => bShape(t, OF_D, { inner: PU2(t, [16.6, 20]) }));
})();
(() => {
  const N = window.N;
  // The owner's two electric shapes: keep the additions' drawings as the "before".
  N.eSingleCut0 = N.eSingleCut; N.eOffset0 = N.eOffset;
  const more = [
    ['eSingleCut', ['eSingleCutA', 'eSingleCutB', 'eSingleCutC', 'eSingleCutD'], 'eSingleCutB', 'the owner found it odd and ugly: a round lower bout, a clear waist, one cutaway leaving a short pointed horn on the treble side; B\'s horn reads at 14 px where A\'s and D\'s melt into the shoulder, C\'s binding line read as a crack'],
    ['eOffset', ['eOffsetA', 'eOffsetB', 'eOffsetC', 'eOffsetD'], 'eOffsetD', 'the owner found it odd and ugly (it was a double cutaway turned 13°): the bass horn long and broad, the treble side only a shoulder, the waist high on the bass side and low on the treble side; A–C still read as the double cutaway'],
  ];
  window.TRACK_VARIANTS_FINAL.unshift(...more);
  more.forEach(([id, , pick]) => { N[id] = N[pick]; });
})();
