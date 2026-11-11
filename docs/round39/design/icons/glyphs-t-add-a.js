// Cantus icon family — set 6 additions, designer icons-a (round 39): cymbals and kit pieces,
// Latin and Mexican percussion, world percussion (with the tuned-percussion additions the
// owner's list places there). Silhouettes on the 24-unit sheet, built per size through
// `track()` and `sil()` (glyphs-t-tracks.js, glyphs-t-tracks3.js); details cut out of the
// silhouette or drawn in the second ink; lines given in device pixels.
// Everything here sits inside one function so its helpers never collide with the other
// designers' files (a top-level `const` twice on one page stops the second script).
(() => {
  const N = window.N;
  const E = (cx, cy, rx, ry, c = 'f1', rot = 0) => `<ellipse class="${c}" cx="${cx}" cy="${cy}" rx="${rx}" ry="${ry}"${rot ? ` transform="rotate(${rot} ${cx} ${cy})"` : ''}/>`;
  const G = (tr, inner) => `<g transform="${tr}">${inner}</g>`;
  const P = pts => 'M' + pts.map(p => p.map(v => +v.toFixed(2)).join(' ')).join(' L') + ' Z';
  // A tapered rod (stick, mallet shaft) from (x0,y0) to (x1,y1), w0 wide at the start and w1 at the end.
  const rod = (x0, y0, x1, y1, w0, w1) => {
    const L = Math.hypot(x1 - x0, y1 - y0), nx = -(y1 - y0) / L, ny = (x1 - x0) / L;
    return P([[x0 + nx * w0 / 2, y0 + ny * w0 / 2], [x1 + nx * w1 / 2, y1 + ny * w1 / 2], [x1 - nx * w1 / 2, y1 - ny * w1 / 2], [x0 - nx * w0 / 2, y0 - ny * w0 / 2]]);
  };
  // A cut halo along a rod, so a rod laid over a mass stays apart from it (w in device px).
  const halo = (t, x0, y0, x1, y1, w) => CUT(t, `M${x0} ${y0} L${x1} ${y1}`, w);
  // A cymbal seen edge-on: flat underside, a shallow cone, the bell on top (24-unit sheet).
  const CYM = (cx, cy, r, h = 2.6, bw = 2.1, bh = 2) => `M${cx - r} ${cy} L${cx - bw} ${+(cy - h).toFixed(2)} C${cx - bw} ${+(cy - h - bh * 1.3).toFixed(2)} ${cx + bw} ${+(cy - h - bh * 1.3).toFixed(2)} ${cx + bw} ${+(cy - h).toFixed(2)} L${cx + r} ${cy} L${cx + r - 0.5} ${cy + 1.4} L${cx - r + 0.5} ${cy + 1.4} Z`;
  // The same cymbal turned over (bell down), mirrored about y = cy.
  const CYMdown = (cx, cy, r, h, bw, bh) => G(`translate(0 ${2 * cy}) scale(1 -1)`, F(CYM(cx, cy, r, h, bw, bh)));
  const cymTh = 1.4; // the cymbal's edge thickness, for spacing two of them
  // A stand: the rod (second ink) and, from 16 px, its foot.
  const stand = (t, x, y0, y1 = 21.4) => STK(t, `M${x} ${y0} V${y1}`, 1, 's2') + (t.D ? STK(t, `M${x - 4} ${y1} H${x + 4}`, 1, 's2') : '');
  const hexP = (cx, cy, r, sy = 0.72) => P([0, 1, 2, 3, 4, 5].map(i => [cx + r * Math.cos(i * Math.PI / 3), cy + r * sy * Math.sin(i * Math.PI / 3)]));
  

  // ================================================================ Cymbals and kit pieces
  // Crash: steeply tilted, struck on the edge — sparks off the high edge.
  track('crash', t => sil(t, G('rotate(-20 11 10)', F(CYM(11, 10, 8.6))) + stand(t, 11, 10.4) +
    STK(t, t.D ? 'M19.4 3.6 L21.4 1.8 M20.4 7 H23 M19.8 10 L22 11.4' : 'M19.6 3.4 L21.6 1.8 M20.2 7 H22.8', 1)));
  // Ride: the biggest, nearly flat, a big bell and the stick's tip on it.
  track('ride', t => sil(t, G('rotate(-5 11 11)', F(CYM(11, 11.4, 10.2, 2.2, 2.7, 2.3))) + stand(t, 11, 12) +
    F(rod(22.6, 0.6, 13.4, 5.6, 1.6, 1.2)) + C1(13.2, 5.7, 1.2) + (t.D === 2 ? STK(t, 'M17.6 9.4 L19.4 8.6', 1, 's2') : '')));
  // Splash: small, high on its stand, a splash all around it.
  track('splash', t => sil(t, F(CYM(12, 11.4, 6.2, 2, 1.6, 1.5)) + stand(t, 12, 12.2) +
    STK(t, t.D ? 'M4.4 9.4 L2.6 8 M19.6 9.4 L21.4 8 M7.4 5.4 L6.4 3.6 M16.6 5.4 L17.6 3.6 M12 4.4 V2.4' : 'M4.6 9.6 L2.6 8.2 M19.4 9.6 L21.4 8.2 M12 4.6 V2.2', 1)));
  // China: upside down, its edge flanged up, the bell hanging under; the rod through it.
  const CHINA = 'M1.8 6.6 L4.4 9.2 L9.4 10 H14.6 L19.6 9.2 L22.2 6.6 L21.6 9 L20 11 L14.6 11.8 C14.6 14.6 9.4 14.6 9.4 11.8 L4 11 L2.4 9 Z';
  track('china', t => sil(t, F(CHINA) + STK(t, 'M12 7.4 V9.6', 1) + stand(t, 12, 14)));
  // Hi-hat, open: the top cymbal lifted and tilted, the rod between.
  track('hihatOpen', t => sil(t, G('rotate(-9 12 7.4)', F(CYM(12, 7.4, 9, 2.2))) + CYMdown(12, 15, 9, 2, 1.9, 1.5) +
    STK(t, 'M12 2.4 V4.4', 1) + STK(t, 'M12 9.2 V12.6', 1) + stand(t, 12, 17.6)));
  // Hi-hat, closed: the two cymbals shut, a hairline between.
  track('hihatClosed', t => sil(t, F(CYM(12, 10.2, 9.6, 2.4)) + CYMdown(12, +(10.2 + 2 * cymTh + 1 / t.k).toFixed(2), 9.6, 2.2, 1.9, 1.5) +
    STK(t, 'M12 2.4 V4.6', 1) + stand(t, 12, 17.4)));
  // Stacked cymbals: a small one turned over on a big one, bell to bell, on one rod.
  track('cymbalStack', t => sil(t, CYMdown(12, 7.4, 6.6, 1.8, 1.6, 1.2) + F(CYM(12, 15, 10, 2.4, 1.9, 1.5)) +
    (t.D ? CUT(t, 'M9.6 10.7 H14.4', 1) : '') + STK(t, 'M12 4.4 V7', 1) + stand(t, 12, 15.6)));

  window.ADDA = { E, G, P, rod, halo, CYM, CYMdown, stand, hexP, tomShell: (...a) => tomShell(...a) };

  // Rack tom: a tom tilted on its arm, the kick's rim under it.
  const tomShell = (x0, x1, y0, y1, ry) => {
    const cx = (x0 + x1) / 2, rx = (x1 - x0) / 2;
    return E(cx, y1, rx, ry) + RR(x0, y0, x1 - x0, y1 - y0) + E(cx, y0, rx, ry) + E(cx, y0, rx - 1.3, ry - 0.75, 'f2');
  };
  track('rackTom', t => sil(t, G('rotate(-14 12 10)', tomShell(4.2, 19.8, 5.4, 13.4, 2.4) + (t.D ? CUT(t, 'M8 9.2 V12.6 M12 9.6 V13.6 M16 9.2 V12.6', 1) : '')) +
    RR(11.2, 15, 1.6, 6) + RR(10.2, 19.4, 3.6, 1.6, 0.4) + STK(t, 'M2.6 23.4 C6 21.2 18 21.2 21.4 23.4', 1, 's2')));
  // Floor tom: deep, on three legs.
  track('floorTom', t => sil(t, tomShell(4.4, 19.6, 4.8, 16.2, 2.4) + (t.D ? CUT(t, 'M8 9 V16.6 M12 9.4 V17.4 M16 9 V16.6', 1) : '') +
    STK(t, 'M4.8 10 L2.6 23 M19.2 10 L21.4 23', 1.2) + STK(t, 'M12 18.6 V23', 1.2)));
  // Side-stick: a snare, the stick laid across its head and over the rim, a click at the rim.
  track('sideStick', t => sil(t, tomShell(2.6, 19.4, 12.4, 18.6, 2.6) + halo(t, 7.4, 14.6, 21.6, 6.8, t.S >= 30 ? 6 : 3.4) +
    F(rod(7.4, 14.6, 21.6, 6.8, 1.3, 2)) + (t.D ? CUT(t, 'M6.6 16 V19.6 M11 16.4 V20.6 M15.4 16 V19.6', 1) : '') +
    STK(t, t.D ? 'M15.4 6 L14.6 3.8 M18.6 3.8 L19.2 1.8 M12.6 8.2 L10.6 7' : 'M15.6 5.8 L14.8 3.6 M12.8 8 L10.6 7', 1)));
  // Drumsticks: two sticks crossed, beaded tips up.
  const STICK = (x0, y0, x1, y1, c) => F(rod(x0, y0, x1, y1, 2.8, 1.5), c) + C1(x1, y1, 1.55, c) + C1(x0, y0, 1.4, c);
  track('sticks', t => sil(t, STICK(19.4, 21.8, 5, 2.6, 'f2') + halo(t, 4.6, 21.8, 19, 2.6, t.S >= 30 ? 7 : 4) + STICK(4.6, 21.8, 19, 2.6, 'f1')));
  // Brushes: a handle, its ferrule and the fan of wires (A: one; B: a crossed pair).
  const fan = (t, n) => {
    const xs = n === 3 ? [-5.2, 0, 5.2] : n === 5 ? [-6, -3, 0, 3, 6] : [-6.4, -4.3, -2.1, 0, 2.1, 4.3, 6.4];
    return STK(t, xs.map(dx => `M12 11.6 L${12 + dx} ${1.8 + Math.abs(dx) * 0.28}`).join(' '), 1);
  };
  track('brushesA', t => sil(t, G('rotate(32 12 12)', RR(11, 13.6, 2, 9.6, 1) + RR(10.4, 10.4, 3.2, 3.8, 0.6) + fan(t, t.D === 2 ? 7 : t.D ? 5 : 3))));
  const oneBrush = (t, c, n) => RR(11.1, 14.4, 1.8, 8.8, 0.9, c) + RR(10.6, 11.6, 2.8, 3.2, 0.5, c) +
    STK(t, (n === 3 ? [-3.4, 0, 3.4] : [-4.2, -2.1, 0, 2.1, 4.2]).map(dx => `M12 12.4 L${12 + dx} ${2.4 + Math.abs(dx) * 0.2}`).join(' '), 1, c === 'f2' ? 's2' : 's1');
  track('brushesB', t => sil(t, G('rotate(-24 12 12)', oneBrush(t, 'f2', t.D ? 5 : 3)) + G('rotate(24 12 12)', oneBrush(t, 'f1', t.D ? 5 : 3))));
  // Electronic drums: (A) hexagonal pads on a rack; (B) round mesh pads and the module.
  track('edrumsA', t => sil(t, STK(t, 'M3 10.6 H21 M4 10.6 V22.6 M20 10.6 V22.6', 1, 's2') +
    F(hexP(6.6, 6.4, 4.4)) + F(hexP(17.4, 6.4, 4.4)) + F(hexP(12, 15.6, 5.2)) +
    (t.D ? F(hexP(6.6, 6.4, 2.6), 'cut') + F(hexP(17.4, 6.4, 2.6), 'cut') + F(hexP(12, 15.6, 3.2), 'cut') + F(hexP(6.6, 6.4, 1.6), 'f1') + F(hexP(17.4, 6.4, 1.6), 'f1') + F(hexP(12, 15.6, 2.1), 'f1') : '')));
  track('edrumsB', t => sil(t, STK(t, 'M2.6 11.4 H21.4 M4 11.4 V22.6 M20 11.4 V22.6', 1, 's2') +
    E(6.4, 8, 4, 2.2) + E(17.6, 8, 4, 2.2) + E(12, 17.4, 5.2, 2.6) + RR(9.6, 2, 4.8, 3.6, 0.6) + RR(10.6, 3, 2.8, 1.4, 0, 'cut') +
    (t.D ? E(6.4, 8, 2.6, 1.1, 'cut') + E(17.6, 8, 2.6, 1.1, 'cut') + E(12, 17.4, 3.6, 1.4, 'cut') : '')));
  // Kick pedal, seen from the side: the footboard, the post, the beater up on its shaft.
  track('kickPedal', t => sil(t, RR(2.4, 21.2, 19.6, 1.8, 0.6) + RR(16, 9.6, 2.2, 11.8) + F('M2.8 21.2 L3.6 19.6 L15.6 13.4 L16.6 15 L5.2 21.2 Z') +
    C1(17.1, 9.4, 2) + (t.D ? C1(17.1, 9.4, 0.8, 'cut') : '') + F(rod(16.4, 8.6, 10.8, 3.8, 1.2, 1)) + G('rotate(-40 9.4 2.8)', RR(6.6, 1, 5.6, 3.6, 1.4)) +
    (t.D ? STK(t, 'M15.6 13.4 C16 12 16.4 11.4 16.6 11', 1, 's2') : '')));
})();

// ================================================================ Latin and Mexican percussion
(() => {
  const { E, G, P, rod, halo, stand, tomShell } = window.ADDA;
  // Bongós: two short drums, the bigger on the left, joined by their block.
  track('bongos', t => sil(t, F('M1.8 8.6 L3.2 18 C3.2 19.2 9.8 19.2 9.8 18 L11.2 8.6 Z') + E(6.5, 8.6, 4.7, 1.7) + E(6.5, 8.6, 3.5, 1.05, 'f2') +
    F('M13.2 10 L14.4 18 C14.4 19.2 20.2 19.2 20.2 18 L21.4 10 Z') + E(17.3, 10, 4.1, 1.5) + E(17.3, 10, 3, 0.9, 'f2') + RR(10.2, 12.6, 4, 3.4, 0.4) +
    (t.D ? CUT(t, 'M4.8 11.6 V15.6 M8.2 11.6 V15.6 M15.8 12.8 V15.8 M18.8 12.8 V15.8', 1) : '')));
  // Timbales: two shallow shells side by side on a stand, the cowbell on top.
  track('timbales', t => sil(t, F('M10.2 1.8 H13.8 L15 6.6 H9 Z') + stand(t, 12, 6.6, 22) +
    tomShell(1.4, 11, 9.6, 13.6, 1.6) + tomShell(13, 22.6, 9.6, 13.6, 1.6) + STK(t, 'M8 15.2 L12 17.6 L16 15.2', 1, 's2')));
  // Güiro: the long gourd with its grooves, the scraper beside it.
  const grooves = (t, step) => {
    let d = '';
    for (let x = 5; x <= 19.01; x += step) { const h = 3.8 * Math.sqrt(Math.max(0, 1 - ((x - 12) / 10) ** 2)) - 1.2; d += `M${x.toFixed(2)} ${(12 - h).toFixed(2)} V${(12 + h).toFixed(2)} `; }
    return CUT(t, d, 1);
  };
  track('guiro', t => sil(t, G('rotate(-30 12 12)', E(12, 12, 10.4, 4) + grooves(t, t.D === 2 ? 1.75 : t.D ? 2.33 : 3.5) + STK(t, 'M5 5.6 H19', t.S >= 30 ? 2 : 1.2, 's2'))));
  // Claves: one clave held across, the other striking it, a click where they meet.
  track('claves', t => sil(t, F(rod(2.8, 19.2, 18.4, 15.6, 3, 3)) + C1(2.8, 19.2, 1.5) + C1(18.4, 15.6, 1.5) +
    halo(t, 11.4, 15, 21, 3.4, t.S >= 30 ? 7 : 4) + F(rod(11.4, 15, 21, 3.4, 3, 3)) + C1(11.4, 15, 1.5) + C1(21, 3.4, 1.5) +
    STK(t, t.D ? 'M8.6 12.6 L7 10.8 M7.6 15.4 H5.4 M11 10.8 L10.4 8.6' : 'M8.4 12.6 L6.8 10.8 M7.4 15.4 H5.2', 1)));
  // Quijada: the jawbone, its teeth along the top.
  const JAW = 'M2.6 4.6 C2.6 3 4.6 2.4 5.4 3.8 L6.2 7 C7 6 8.4 6.2 8.6 7.6 L8.8 11.2 H21 C22.6 11.2 23.2 12.8 22.2 13.8 C18.4 17.6 11.4 19.8 6.2 20 C3.8 20.1 2.6 18.6 2.6 16.6 Z';
  const molars = t => (t.D ? [10.4, 13.1, 15.8, 18.5] : [10.8, 14.4, 18]).map(x => RR(x - (t.D ? 1.1 : 1.4), 7.6, t.D ? 2.2 : 2.8, 4, 0.7)).join('');
  track('quijadaA', t => sil(t, G('rotate(-10 12 12)', F(JAW) + molars(t) + (t.D ? CUT(t, 'M9.4 11.2 H21.4', 1) : ''))));
  const ARCH = 'M5.2 2.6 C5.2 15 7.6 21.4 12 21.4 C16.4 21.4 18.8 15 18.8 2.6';
  track('quijadaB', t => sil(t, `<path class="s1" fill="none" stroke-width="4.6" stroke-linecap="round" d="${ARCH}"/>` +
    CUT(t, (t.D ? [6, 9.4, 12.6, 15.6, 18.4] : [7.4, 12.6, 17]).map(y => `M2.6 ${y} H8 M16 ${y} H21.4`).join(' ') + (t.D ? ' M10.6 17.6 L9.4 21.6 M13.4 17.6 L14.6 21.6' : ' M12 18 V22.4'), 1)));
  // Cabasa: the bead-wrapped cylinder on its handle.
  const beads = t => {
    if (!t.D) return CUT(t, 'M6.6 7.6 V16.4 M10.4 7.6 V16.4', 1);
    const xs = t.D === 2 ? [5.4, 7.9, 10.4, 12.9] : [5.8, 8.8, 11.8], ys = t.D === 2 ? [8.6, 10.8, 13, 15.2] : [8.8, 12, 15.2];
    return xs.map(x => ys.map(y => `<circle class="cut" cx="${x}" cy="${y}" r="${t.D === 2 ? 0.75 : 0.9}"/>`).join('')).join('');
  };
  track('cabasaA', t => sil(t, G('rotate(-35 12 12)', E(3.6, 12, 1.6, 5.6) + RR(3.6, 6.4, 10.8, 11.2, 0) + E(14.4, 12, 1.6, 5.6) + RR(15, 10.4, 7.8, 3.2, 1.6) + beads(t))));
  const chains = t => (t.D === 2 ? [5.2, 7.6, 10, 12.4] : t.D ? [5.4, 8.2, 11] : [5.6, 9.4]).map(x => RR(x - (t.D === 2 ? 0.9 : 1.1), 5.6, t.D === 2 ? 1.8 : 2.2, 12.8, 1)).join('');
  track('cabasaB', t => sil(t, G('rotate(-35 12 12)', RR(3.4, 7.6, 10.4, 8.8, 1, 'f2') + chains(t) + RR(14.2, 10.4, 8.6, 3.2, 1.6))));
  // Shekere: the gourd with its neck, a net of beads over it.
  const GOURD = 'M10.6 1.8 H13.4 V4.8 C13.4 5.8 14.2 6.6 15.4 7.2 C19 8.8 20.8 11.8 20.8 15.2 C20.8 19.8 16.8 23 12 23 C7.2 23 3.2 19.8 3.2 15.2 C3.2 11.8 5 8.8 8.6 7.2 C9.8 6.6 10.6 5.8 10.6 4.8 Z';
  track('shekere', t => sil(t, F(GOURD) + CUT(t, t.D ? 'M4.6 11.2 L14.8 23 M7.6 8.4 L20 22 M12 7.6 L21.4 17.6 M19.4 11.2 L9.2 23 M16.4 8.4 L4 22 M12 7.6 L2.6 17.6' : 'M5.6 10 L16 23 M12 7.4 L21.4 17.6 M18.4 10 L8 23 M12 7.4 L2.6 17.6', 1)));
  // Agogô: two bells, big and small, on one bent rod.
  track('agogo', t => sil(t, F('M2.4 3.4 H10.6 L8.6 14.2 C7.6 14.6 5.4 14.6 4.4 14.2 Z') + E(6.5, 3.4, 4.1, 1.2, 'f2') +
    F('M13.8 6.6 H20.8 L19.2 15 C18.4 15.4 16.6 15.4 15.6 15 Z') + E(17.3, 6.6, 3.5, 1, 'f2') +
    STK(t, 'M6.5 14.4 V16.6 C6.5 20.6 17.4 20.6 17.4 16.6 V15', 1.2) + STK(t, 'M12 19.6 V23.2', 1.2)));
  // Cuíca: the drum on its side, its head to the left with the stick's knot, a squeak.
  track('cuica', t => sil(t, RR(8, 6.4, 13, 11.2, 0) + E(21, 12, 1.6, 5.6) + E(8, 12, 2.8, 5.6) + E(8, 12, 1.9, 4.4, 'f2') + C1(8, 12, 0.95) +
    STK(t, 'M11 6.4 C12.6 2 17.4 2 19 6.4', 1, 's2') + (t.D ? CUT(t, 'M12.4 8 V16 M16.6 8 V16', 1) : '') +
    STK(t, t.D ? 'M2.6 7.6 C1.4 9 3.6 10.4 2.4 12 C1.2 13.6 3.4 15 2.2 16.4' : 'M2.6 8.4 C1.2 10.2 3.6 12 2.2 14', 1, 's2')));
  // Surdo: the deep bass drum with its hoops and rods, the soft mallet beside it.
  track('surdo', t => sil(t, tomShell(2, 17.4, 5, 19.6, 2.4) + (t.D ? CUT(t, 'M2.6 8.2 H16.8 M2.6 17.4 H16.8 M6.4 8.2 V17.4 M9.7 8.8 V18 M13 8.2 V17.4', 1) : CUT(t, 'M2.6 8.4 H16.8 M2.6 17.2 H16.8', 1)) +
    F(rod(21.8, 22.6, 20.2, 5.4, 1.4, 1.2)) + C1(20, 4, 2.4)));
  // Pandeiro: the frame, its head, the jingles in their slots.
  const slots = (t, n) => [...Array(n).keys()].map(i => G(`rotate(${-90 + i * 360 / n} 12 12)`, RR(19.9, 10.5, 1.8, 3, 0, 'cut'))).join('');
  track('pandeiro', t => sil(t, C1(12, 12, 10.4) + C1(12, 12, 7.8, 'cut') + C1(12, 12, 7, 'f2') + slots(t, t.D ? 5 : 4)));
  // Woodblock: the block with its slits, a stick on it.
  track('woodblock', t => sil(t, RR(2.2, 9.4, 19.6, 9.2, 1.8) + CUT(t, t.D ? 'M4.6 12 H19.4 M4.6 16 H19.4' : 'M4.6 12.4 H19.4', t.S >= 30 ? 2 : 1) +
    F(rod(22.4, 1, 15.2, 7, 1.4, 1.4)) + C1(14.8, 7.4, 1.6)));
  // Castanets: (A) the two shells open, hinged at the cord; (B) the pair face on, the cord between.
  track('castanetsA', t => sil(t, G('rotate(-16 3.6 12)', F('M3.6 11.2 H20.6 C20.6 6.6 16.8 3.8 12.2 3.8 C7.6 3.8 3.6 6.6 3.6 11.2 Z')) +
    G('rotate(12 3.6 12)', F('M3.6 12.8 H20.6 C20.6 17.4 16.8 20.2 12.2 20.2 C7.6 20.2 3.6 17.4 3.6 12.8 Z', 'f2')) + STK(t, 'M3.4 12 C1.2 10.4 1.4 7 3.6 6', 1)));
  const SHELL = 'M0 -8 C1.6 -8 2.6 -7 2.6 -5.8 C5.6 -4.8 7.4 -2.2 7.4 1.2 C7.4 5.4 4 8.2 0 8.2 C-4 8.2 -7.4 5.4 -7.4 1.2 C-7.4 -2.2 -5.6 -4.8 -2.6 -5.8 C-2.6 -7 -1.6 -8 0 -8 Z';
  track('castanetsB', t => sil(t, G('translate(15.2 13.4)', F(SHELL, 'f2')) + G('translate(9.2 14.6)', `<path class="scut" fill="none" stroke-width="${px(t, 2)}" d="${SHELL}"/>` + F(SHELL) + (t.D ? `<ellipse class="f2" cx="0.4" cy="2" rx="5" ry="4.6"/>` : `<ellipse class="f2" cx="0.4" cy="2.2" rx="4.2" ry="3.8"/>`)) +
    `<circle class="cut" cx="9.2" cy="8.4" r="0.9"/>` + STK(t, 'M9.2 6.4 C9.2 2 15.2 1.6 15.2 5.2', 1)));
  // Vibraslap: the box at one end, the ball at the other, the bent rod between.
  track('vibraslap', t => sil(t, G('rotate(-24 6 5.4)', RR(1.2, 2, 9.6, 6.6, 1.2) + CUT(t, 'M3.2 5.3 H8.8', 1)) +
    STK(t, 'M7 8.8 C7 18.6 12.4 22 16.4 20.4 C19.8 19 20.6 14.8 19.8 10.4', t.S >= 30 ? 2 : 1.2) + C1(19.6, 7.4, 3)));
  // Marimba: the bars on their frame, the resonators hanging under them, graduated; legs.
  const resos = () => [4.2, 7, 9.8, 12.6, 15.4, 18.2].map((x, i) => RR(x, 9.6, 2, 11.4 - i * 1.5, 1, 'f2')).join('');
  const bars = t => RR(1.4, 5.8, 21.2, 3.8) + CUT(t, t.D ? 'M4.4 5.8 V9.6 M7.4 5.8 V9.6 M10.4 5.8 V9.6 M13.4 5.8 V9.6 M16.4 5.8 V9.6 M19.4 5.8 V9.6' : 'M6 5.8 V9.6 M10.6 5.8 V9.6 M15.2 5.8 V9.6', 1);
  const legs = t => RR(1.6, 9.6, 1.6, 13.6) + RR(20.8, 9.6, 1.6, 13.6) + STK(t, 'M2.4 21.8 H21.6', 1);
  track('marimbaA', t => sil(t, bars(t) + resos() + legs(t) + C1(8.4, 3.6, 1.5) + C1(15.6, 3.6, 1.5) + STK(t, 'M8.4 3.6 L4.6 0.6 M15.6 3.6 L19.4 0.6', 1)));
  track('marimbaB', t => sil(t, bars(t) + resos() + legs(t)));
})();

// ================================================================ World percussion (and the tuned additions)
(() => {
  const { E, G, P, rod, halo } = window.ADDA;
  // Djembe: the goblet, widest at its head, its rope lacing cut in a zigzag.
  track('djembe', t => sil(t, RR(3.2, 2.2, 17.6, 2.6, 0.8) + F('M3.8 4.6 H20.2 C20.2 9.6 16.4 12 14.4 13.6 L16.6 22.6 H7.4 L9.6 13.6 C7.6 12 3.8 9.6 3.8 4.6 Z') +
    CUT(t, t.D ? 'M5.6 5.6 L7.8 10.6 L10 5.6 L12 10.6 L14 5.6 L16.2 10.6 L18.4 5.6' : 'M6.4 5.6 L9 10.2 L12 5.6 L15 10.2 L17.6 5.6', 1) + (t.D ? CUT(t, 'M9.4 15.6 H14.6', 1) : '')));
  // Darbuka: a round bowl under a smaller head, the narrow neck, a band; (A) upright, (B) lying as it is held.
  const DARB = 'M6.8 3.6 H17.2 C20.4 5.2 21.2 9 19.6 11.6 C18.4 13.4 15.6 14.2 14.2 15 L15 20.4 C15.4 21.4 16.6 22 17 22.8 H7 C7.4 22 8.6 21.4 9 20.4 L9.8 15 C8.4 14.2 5.6 13.4 4.4 11.6 C2.8 9 3.6 5.2 6.8 3.6 Z';
  const darb = t => RR(6.4, 1.8, 11.2, 2.2, 0.8) + F(DARB) + CUT(t, t.D ? 'M4.2 8.4 C9 9.8 15 9.8 19.8 8.4 M4.8 11 C9.4 12.2 14.6 12.2 19.2 11' : 'M4.2 9.2 C9 10.6 15 10.6 19.8 9.2', 1);
  track('darbukaA', t => sil(t, darb(t)));
  track('darbukaB', t => sil(t, G('translate(0.6 0.4) scale(0.92)', darb(t)), -62));
  // Tabla: the round bayan and the tall dayan, each head with its black spot.
  track('tabla', t => sil(t, F('M1.6 11.8 H12.4 C12.4 18.4 10.2 21.8 7 21.8 C3.8 21.8 1.6 18.4 1.6 11.8 Z') + E(7, 11.8, 5.4, 1.9) + E(7, 11.8, 4.3, 1.25, 'f2') + E(8.2, 11.8, 1.5, 0.55) +
    F('M13.6 6.6 H21.8 C22.4 10.6 22.4 17.4 21.6 21.8 H13.8 C13 17.4 13 10.6 13.6 6.6 Z') + E(17.7, 6.6, 4.1, 1.5) + E(17.7, 6.6, 3.1, 0.95, 'f2') + E(17.7, 6.6, 1.4, 0.45) +
    (t.D ? CUT(t, 'M15.4 9.4 V19.6 M17.7 9.8 V20.4 M20 9.4 V19.6', 1) : '')));
  // Frame drum / bodhrán: the wide frame seen a little turned, its head, the tipper across.
  track('frameDrum', t => sil(t, E(10.4, 12, 8.6, 10.4) + E(11.6, 12, 7.2, 9.2, 'f2') +
    halo(t, 12.8, 21.6, 22.4, 5.4, t.S >= 30 ? 7 : 4) + F(rod(12.8, 21.6, 22.4, 5.4, 1.4, 1.4)) + C1(12.8, 21.6, 1.6) + C1(22.4, 5.4, 1.6)));
  // Taiko: the barrel on its side, its rows of tacks, on the crossed stand.
  const tacks = t => t.D ? [6.2, 17.8].map(x => [7.6, 9.7, 11.8, 13.9, 16].map(y => `<circle class="cut" cx="${x}" cy="${y}" r="0.7"/>`).join('')).join('') : CUT(t, 'M6.2 8 V16 M17.8 8 V16', 1);
  track('taiko', t => sil(t, F('M3.6 6 C9 3.8 15 3.8 20.4 6 V17.2 C15 19.4 9 19.4 3.6 17.2 Z') + E(3.6, 11.6, 1.7, 5.6) + E(20.4, 11.6, 1.7, 5.6) + tacks(t) +
    STK(t, 'M7 17.6 L16.4 23 M17 17.6 L7.6 23', t.S >= 30 ? 2 : 1.2, 's2')));
  // Handpan: the two shells, a saucer, the ding on top, tone fields round it, the seam.
  track('handpan', t => sil(t, F('M1.4 13.2 C1.4 9.6 6.2 7.6 12 7.6 C17.8 7.6 22.6 9.6 22.6 13.2 C22.6 16.8 17.8 19.2 12 19.2 C6.2 19.2 1.4 16.8 1.4 13.2 Z') + E(12, 8.2, 3.2, 1.9) +
    CUT(t, 'M1.8 13.6 C5.6 16.2 18.4 16.2 22.2 13.6', 1) +
    (t.D ? `<g class="scut" fill="none" stroke-width="${px(t, 1)}">${[[6.2, 11.6], [17.8, 11.6]].concat(t.D === 2 ? [[9, 13.2], [15, 13.2]] : []).map(([x, y]) => `<ellipse cx="${x}" cy="${y}" rx="1.9" ry="0.9"/>`).join('')}</g>` : '')));
  // Steel pan: (A) the drum, its notes in the bowl, the stick; (B) seen from above, the notes as petals.
  track('steelpanA', t => sil(t, F('M3 7.6 V17.4 C3 20.6 21 20.6 21 17.4 V7.6 Z') + E(12, 7.6, 9, 3.6) + E(12, 7.8, 7.6, 2.7, 'f2') +
    (t.D ? `<g class="scut" fill="none" stroke-width="${px(t, 1)}"><ellipse cx="8.2" cy="7.8" rx="1.8" ry="0.8"/><ellipse cx="15.8" cy="7.8" rx="1.8" ry="0.8"/><ellipse cx="12" cy="6.4" rx="1.6" ry="0.6"/></g>` : '') +
    (t.D ? CUT(t, 'M3.4 13.6 C8 15.6 16 15.6 20.6 13.6', 1) : '') + halo(t, 22.6, 0.6, 15, 6.6, t.S >= 30 ? 6 : 3.4) + F(rod(22.6, 0.6, 15.4, 6.4, 1.2, 1.2)) + C1(14.8, 6.8, 1.5)));
  const petals = (t, n, r, rx, ry) => [...Array(n).keys()].map(i => `<ellipse class="cut" cx="12" cy="${12 - r}" rx="${rx}" ry="${ry}" transform="rotate(${i * 360 / n} 12 12)"/>`).join('');
  track('steelpanB', t => sil(t, C1(12, 12, 10.6) + C1(12, 12, 9, 'f2') + petals(t, t.D ? 8 : 6, 6.2, 1.3, 2) + (t.D ? `<circle class="cut" cx="12" cy="12" r="1.8"/>` : '')));
  // Kalimba: the box, its tines fanned in a V over the bridge, the sound hole.
  const tines = t => {
    const xs = t.D === 2 ? [6, 8, 10, 12, 14, 16, 18] : [5.6, 8.8, 12, 15.2, 18.4];
    return CUT(t, xs.map(x => `M${x} 13.4 V${(5.2 + Math.abs(x - 12) * 0.7).toFixed(2)}`).join(' ') + ' M4.6 13.6 H19.4', 1);
  };
  track('kalimba', t => sil(t, RR(2.8, 2.6, 18.4, 20.4, 2.6) + tines(t) + C1(12, 18.2, t.D ? 2 : 2.3, 'cut')));
  // Rainstick: the long tube leaning, its spiral of pins, the grains falling.
  track('rainstick', t => sil(t, G('rotate(34 12 12)', RR(9.8, 1.4, 4.4, 21.2, 1.6) + RR(9.2, 0.6, 5.6, 2.8, 0.9) + RR(9.2, 20.6, 5.6, 2.8, 0.9) +
    CUT(t, (t.D ? [5.4, 8, 10.6, 13.2, 15.8, 18.4] : [6.4, 10.4, 14.4, 18]).map(y => `M10.6 ${y + 0.8} L13.4 ${y - 0.8}`).join(' '), 1)) +
    STK(t, t.D ? 'M4 14.4 V15.6 M6.4 17.6 V18.8 M3.2 19.6 V20.8' : 'M4 15 V16.4 M6 18.4 V19.8', 1, 's2')));
  // Wind chimes: the ring, the disc, the tubes hanging round the clapper and its sail.
  track('windChimes', t => sil(t, STK(t, 'M12 2.6 A1.1 1.1 0 1 1 11.99 2.6', 1) + STK(t, 'M12 4.4 V5', 1) + E(12, 5.8, 8.6, 1.5) +
    [[4.6, 9.4], [8.2, 12.6], [15.8, 12.6], [19.4, 9.4]].map(([x, l]) => RR(x - 1, 8.2, 2, l, 0.8)).join('') + STK(t, 'M4.6 7 V8.2 M8.2 7 V8.2 M15.8 7 V8.2 M19.4 7 V8.2 M12 7 V13.4', 1, 's2') +
    E(12, 14.2, 2.2, 0.9) + F('M12 16.6 L14.2 19.6 L12 23 L9.8 19.6 Z', 'f2')));
  // Tubular bells: the frame, the tubes graduated, each with its cap.
  track('tubularBells', t => sil(t, RR(1.6, 2, 20.8, 1.8, 0.4) + RR(2.2, 2, 1.6, 19.8) + RR(20.2, 2, 1.6, 19.8) + RR(0.8, 21.2, 4.4, 1.6, 0.6) + RR(18.8, 21.2, 4.4, 1.6, 0.6) +
    [[6.6, 19.6], [10.2, 17.8], [13.8, 16], [17.4, 14.2]].map(([x, y1]) => RR(x - 1.1, 5.4, 2.2, y1 - 5.4, 0.6) + RR(x - 1.5, 5, 3, 1.6, 0.4)).join('') +
    (t.D ? CUT(t, 'M5.4 7.2 H7.8 M9 7.2 H11.4 M12.6 7.2 H15 M16.2 7.2 H18.6', 1) : '') + STK(t, 'M6.6 3.8 V5 M10.2 3.8 V5 M13.8 3.8 V5 M17.4 3.8 V5', 1, 's2')));
  // Gamelan gong: (A) the bossed gong hung in its carved frame; (B) a bonang's row of kettle gongs, each with its knob.
  track('gamelanGongA', t => sil(t, F('M1 1.8 C2.4 3.2 4 3.6 5.6 3.6 H18.4 C20 3.6 21.6 3.2 23 1.8 L22.4 5.4 H1.6 Z') + RR(2.6, 5, 1.6, 17.6) + RR(19.8, 5, 1.6, 17.6) + RR(1, 21.6, 22, 1.4, 0.6) +
    STK(t, 'M9.4 5.4 L10.4 7.6 M14.6 5.4 L13.6 7.6', 1, 's2') + C1(12, 13.8, 6.8) + `<circle class="scut" fill="none" stroke-width="${px(t, 1)}" cx="12" cy="13.8" r="${t.D ? 3.4 : 3.6}"/>` + C1(12, 13.8, 1.7, 'f2')));
  const kettle = (cx, by, r) => E(cx, by - 2.4, r, 1.9) + RR(cx - r, by - 2.4, 2 * r, 2.4) + RR(cx - 0.6, by - 5, 1.2, 1.4) + C1(cx, by - 5.6, 1.45);
  track('gamelanGongB', t => sil(t, kettle(4.6, 13.6, 3.3) + kettle(12, 13.6, 3.3) + kettle(19.4, 13.6, 3.3) +
    RR(1, 14.2, 22, 3.4, 0.6) + RR(2.4, 17.6, 1.8, 5.2) + RR(19.8, 17.6, 1.8, 5.2) + (t.D ? CUT(t, 'M2.6 15.9 H21.4', 1) : '') +
    (t.D ? CUT(t, 'M1.6 11.2 H7.6 M9 11.2 H15 M16.4 11.2 H22.4', 1) : '')));
  // Crotales: (A) the small discs in two rows on their board, each with its boss; (B) two thick discs on a post.
  const disc = (t, x, y, r) => E(x, y, r + 0.7, (r + 0.7) * 0.55, 'cut') + E(x, y, r, r * 0.55) + (t.D ? E(x, y - 0.2, r * 0.38, r * 0.24, 'f2') : '');
  track('crotalesA', t => sil(t, F('M1.4 19.4 H22.6 L20.6 6.6 H3.4 Z', 'f2') + disc(t, 8.3, 9.6, 2.4) + disc(t, 15.7, 9.6, 2.4) + disc(t, 4.6, 15.6, 2.8) + disc(t, 12, 15.6, 2.8) + disc(t, 19.4, 15.6, 2.8)));
  const thick = (cx, cy, r) => F(`M${cx - r} ${cy} H${cx + r} V${cy + 1.8} H${cx - r} Z`) + F(`M${cx - 2.6} ${cy} C${cx - 2.6} ${cy - 3.2} ${cx + 2.6} ${cy - 3.2} ${cx + 2.6} ${cy} Z`);
  track('crotalesB', t => sil(t, thick(12, 8.6, 6.4) + thick(12, 16.2, 8) + STK(t, 'M12 3.6 V22.6', 1, 's2') + (t.D ? STK(t, 'M8 22.6 H16', 1, 's2') : '')));
})();
