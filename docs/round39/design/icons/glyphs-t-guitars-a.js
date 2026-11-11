// Cantus icon family — the guitar family, part A: acoustic shapes, resonator, basses, and the
// headline guitars at 45° (round 39, icons-guitars). Helpers in glyphs-t-guitars.js.
// Acoustic bodies are drawn at their true proportions to one another (size is part of the
// outline: the jumbo is the widest, the parlor the smallest), on one scale, upright.
(() => {
  const { ID, LIFT, PEGS, at45, xfOf, stretch, Fs, R, PX, PL, HOLE, RING, NECK, HEAD, portrait } = window.GTR;
  const E = window.GTR_E;
  const A = window.GTR_A = {};

  // ---- acoustic outlines ---------------------------------------------------------------
  // Dreadnought: square shoulders, a shallow waist, a big lower bout.
  A.DREAD = 'M12 8.2 L15.6 8.2 C17.6 8.2 18.6 9 18.6 10.6 C18.6 12.4 17.6 13.2 17.8 14.4 C18 15.6 20.6 16.6 20.6 19.6 C20.6 22.6 17 23.6 12 23.6 C7 23.6 3.4 22.6 3.4 19.6 C3.4 16.6 6 15.6 6.2 14.4 C6.4 13.2 5.4 12.4 5.4 10.6 C5.4 9 6.4 8.2 8.4 8.2 Z';
  // Concert / OM: a small, round upper bout over a deep waist.
  A.OM = 'M12 9 C15 9 16.4 10.4 16.4 12.4 C16.4 13.8 15.2 14.4 15.2 15.2 C15.2 15.8 19.6 16.8 19.6 19.8 C19.6 22.6 16.4 23.6 12 23.6 C7.6 23.6 4.4 22.6 4.4 19.8 C4.4 16.8 8.8 15.8 8.8 15.2 C8.8 14.4 7.6 13.8 7.6 12.4 C7.6 10.4 9 9 12 9 Z';
  // Jumbo: a round upper bout, a pinched waist, the widest, roundest lower bout.
  A.JUMBO = 'M12 7.4 C16 7.4 18 9.2 18 11.6 C18 13.4 16.4 14 16.2 14.8 C16 15.4 21.6 16.4 21.6 19.4 C21.6 22.6 17.4 23.8 12 23.8 C6.6 23.8 2.4 22.6 2.4 19.4 C2.4 16.4 8 15.4 7.8 14.8 C7.6 14 6 13.4 6 11.6 C6 9.2 8 7.4 12 7.4 Z';
  // Parlor: small, narrow, tall for its width, low on a long neck.
  A.PARLOR = 'M12 11 C14.2 11 15.4 12.2 15.4 14 C15.4 15.2 14.6 15.8 14.6 16.4 C14.6 17 17.2 17.8 17.2 20.2 C17.2 22.6 15 23.6 12 23.6 C9 23.6 6.8 22.6 6.8 20.2 C6.8 17.8 9.4 17 9.4 16.4 C9.4 15.8 8.6 15.2 8.6 14 C8.6 12.2 9.8 11 12 11 Z';
  // Cutaway dreadnought: the dreadnought, its treble shoulder scooped down to the waist.
  A.CUTD = 'M10.6 8.2 L13.4 8.2 C13.6 8.4 13.6 8.8 13.6 9.4 C13.6 11.8 14.8 13.4 16.8 13.2 C17.6 13.1 17.8 13.8 17.8 14.4 C18 15.6 20.6 16.6 20.6 19.6 C20.6 22.6 17 23.6 12 23.6 C7 23.6 3.4 22.6 3.4 19.6 C3.4 16.6 6 15.6 6.2 14.4 C6.4 13.2 5.4 12.4 5.4 10.6 C5.4 9 6.4 8.2 8.4 8.2 Z';
  // Classical: a rounder upper bout than the concert's, a moderate waist.
  A.CLASS = 'M12 9.4 C14.8 9.4 16.6 10.6 16.6 12.8 C16.6 14.2 15.6 14.8 15.6 15.6 C15.6 16.2 19.4 17 19.4 19.8 C19.4 22.6 16.2 23.6 12 23.6 C7.8 23.6 4.6 22.6 4.6 19.8 C4.6 17 8.4 16.2 8.4 15.6 C8.4 14.8 7.4 14.2 7.4 12.8 C7.4 10.6 9.2 9.4 12 9.4 Z';
  // Travel: a narrow paddle, no waist.
  A.PADDLE = 'M12 10.6 C14.4 10.6 16 12.8 16.2 16.4 C16.4 20.6 15 23.6 12 23.6 C9 23.6 7.6 20.6 7.8 16.4 C8 12.8 9.6 10.6 12 10.6 Z';

  // the steel-string's teardrop pickguard, below the hole on the treble side
  const GUARD = (x, y) => `M${x} ${y} C${x + 2.6} ${y - 0.4} ${x + 3.8} ${y + 1.2} ${x + 3.4} ${y + 3.2} C${x + 1.6} ${y + 3.4} ${x} ${y + 2} ${x} ${y} Z`;
  // hole and bridge: the hole two pixels at 12–15, three at 16, more from 24; the bridge one
  // pixel tall from 16 px
  const AC = (hy, by, r = 2) => t => HOLE(t, 12, hy, t.D ? r : r * 1.1) + (t.D ? PX(t, 12, by, t.D === 2 ? 6 : 4, 1) : '');
  const ac = (body, hy, by, o = {}) => t => portrait(t, body, Object.assign({ head: 'square', xf: ID, n0: 4, n1: hy, inner: t => AC(hy, by, o.r)(t) + (o.more ? o.more(t) : '') }, o));
  // A: hole and bridge; B: with the pickguard in the second ink from 16 px; C: the outline
  // pushed further from its neighbours (wider, `stretch`).
  const trio = (id, body, hy, by, gx, o = {}) => {
    track(id + 'A', ac(body, hy, by, o));
    track(id + 'B', ac(body, hy, by, Object.assign({}, o, { more: t => t.D ? Fs(t, GUARD(gx, hy + 1.6), 'f2') : '' })));
    track(id + 'C', ac(stretch(body, o.cx || 1.08), hy, by, o));
  };
  trio('aDread', A.DREAD, 12.6, 19.6, 13.6);
  trio('aConcert', A.OM, 13, 19.8, 13.4);
  trio('aJumbo', A.JUMBO, 12.2, 19.6, 13.8);
  trio('aParlor', A.PARLOR, 14.6, 20.6, 13, { r: 1.6, cx: 0.92 });
  trio('aCutaway', A.CUTD, 12.8, 19.6, 13.6);

  // Classical: the slotted head (one slot to 15 px, two from 16), a wider neck, the rosette.
  const SLOT = t => R(t, 9.6, 0.2, 14.4, 5) + (t.D ? PX(t, 10.9, 2.6, 1, Math.max(2, Math.round(3.2 * t.k)), 'cut') + PX(t, 13.1, 2.6, 1, Math.max(2, Math.round(3.2 * t.k)), 'cut')
    : PX(t, 12, 2.6, 1, 2, 'cut'));
  const ROSE = (hy) => t => t.D === 2 ? RING(t, 12, hy, 3.2) : '';
  track('classicalA', ac(A.CLASS, 13.2, 19.8, { headD: SLOT, nw: 3.4 }));
  track('classicalB', ac(A.CLASS, 13.2, 19.8, { headD: SLOT, nw: 3.4, more: ROSE(13.2) }));
  track('classicalC', ac(A.OM, 13, 19.8, { headD: SLOT, nw: 3.4, more: ROSE(13) }));

  // 12-string: a long head with tuners on both sides — a comb of nubs, one pixel on, one off.
  const COMB = (y0, y1, w = 3.6) => t => {
    const x0 = 12 - w / 2, x1 = 12 + w / 2, n = Math.max(2, Math.floor((y1 - y0) * t.k / 2));
    const ys = Array.from({ length: n }, (_, i) => y0 + (i * 2 + 1.5) / t.k);
    return R(t, x0, y0, x1, y1) + PEGS(t, ys.flatMap(y => [[x0 - 0.5 / t.k - 0.01, y], [x1 + 0.5 / t.k, y]]), 1, 1);
  };
  track('guitar12A', ac(A.DREAD, 12.6, 19.6, { headD: COMB(0.2, 6.6), n0: 6 }));
  track('guitar12B', ac(A.JUMBO, 12.2, 19.6, { headD: COMB(0.2, 6.6), n0: 6 }));
  track('guitar12C', ac(A.DREAD, 12.6, 19.6, { headD: COMB(0.2, 6.6, 4.4), n0: 6, more: t => t.D ? Fs(t, GUARD(13.6, 14.2), 'f2') : '' }));

  // Travel: A, the whole guitar small (a mini dreadnought, its head not reaching the top);
  // B, the narrow paddle body of a travel guitar on a full neck; C, A with a carrying strap.
  const MINI = xfOf(0.74, 0.74, 23.6);
  track('aTravelA', t => portrait(t, A.DREAD, { head: 'square', hy: 5.2, n0: 9, xf: MINI, n1: 12.6, inner: AC(12.6, 19.6) }));
  track('aTravelB', ac(A.PADDLE, 14.4, 20.4, { r: 1.6 }));
  track('aTravelC', t => portrait(t, A.DREAD, { head: 'square', hy: 5.2, n0: 9, xf: MINI, n1: 12.6, inner: AC(12.6, 19.6),
    top: t => t.D ? `<path class="s2" fill="none" stroke-width="${+(1 / t.k).toFixed(3)}" d="M10.6 6.4 C5 6.4 3 12 3.6 19.4"/>` : '' }));

  // Resonator: the square-shouldered body, the round cover plate over its cone in the second
  // ink, ringed by a cut; the screen holes in the upper bout from 24 px.
  const PLATE = (t, r) => HOLE(t, 12, 18.4, r + 0.5) + HOLE(t, 12, 18.4, r, 'f2') + (t.D ? PX(t, 12, 18.4, t.D === 2 ? 2 : 1, t.D === 2 ? 2 : 1, 'f1') : '');
  track('resonatorA', t => portrait(t, A.DREAD, { head: 'square', xf: ID, n1: 12, inner: t => PLATE(t, 4.2) + (t.D === 2 ? HOLE(t, 8.8, 11.4, 1, 'cut') + HOLE(t, 15.2, 11.4, 1, 'cut') : '') }));
  track('resonatorB', t => portrait(t, A.DREAD, { head: 'square', xf: ID, n1: 12, inner: t => RING(t, 12, 18.4, 4.4) + RING(t, 12, 18.4, 1.8) }));
  track('resonatorC', t => portrait(t, A.DREAD, { head: 'square', xf: ID, n1: 12, inner: t => PLATE(t, 3.6) + PX(t, 12, 12.4, t.D === 2 ? 6 : 4, 1) }));

  // ---- basses: upright, a small body low on a long neck, four tuners in line ------------
  // P-type body: an offset double cutaway, the bass horn long.
  A.PB = 'M10.9 15 C10.6 13.6 9.6 12.2 8.4 12.2 C7.2 12.2 6.8 13.2 7.2 14.4 C7.6 15.4 8 16 7.4 17 C6.8 17.8 5.6 18.6 5.6 20.4 C5.6 22.8 8.6 23.8 12 23.8 C15.4 23.8 18.4 22.8 18.4 20.6 C18.4 18.8 16.8 18.2 16.8 17.2 C16.8 16.2 17.4 15.6 17 14.8 C16.6 14 15.6 14.2 14.8 14.8 C14.2 15.2 13.6 15.4 13.1 15.4 Z';
  // J-type body: the waist offset, narrower, the horns longer.
  A.JB = 'M10.9 15.2 C10.6 13.4 9.6 11.6 8.2 11.6 C7 11.6 6.8 12.8 7.4 14.2 C7.8 15.2 8.2 16 7.4 17.2 C6.8 18 5.8 18.8 5.8 20.6 C5.8 22.8 8.6 23.8 12 23.8 C15.4 23.8 18.2 22.8 18.2 20.6 C18.2 18.6 16.6 17.8 16.6 16.6 C16.6 15.6 17.4 14.6 17 13.8 C16.6 13 15.4 13.4 14.6 14.2 C14 14.8 13.6 15.2 13.1 15.2 Z';
  const BHEAD = 'M13.1 5.6 L13.1 2.6 C13.1 1.4 12.4 0.3 11 0.3 L9.8 0.3 C9 0.3 8.8 1 9.2 1.6 L10.9 4.6 L10.9 5.6 Z';
  // the four tuners, nubs on the bass side of the head from 16 px
  const BPEGS = t => t.D ? PEGS(t, [[8.4, 1.2], [8.9, 2.6], [9.4, 4]].map(([x, y]) => [x, y]), 1, 1) : '';
  const bass = (body, o = {}) => t => portrait(t, body, Object.assign({ headD: t => Fs(t, BHEAD) + BPEGS(t), xf: ID, nw: 2.2, n0: 5, n1: 16 }, o));
  // P: the split pickup and the bridge; J: two pickups square to the strings.
  const P_PU = t => t.D ? PX(t, 11, 18, 2, 1) + PX(t, 13, 18.8, 2, 1) + (t.D === 2 ? PX(t, 12, 21.4, 4, 1) : '') : '';
  const J_PU = t => t.D ? PX(t, 12, 18.2, 3, 1) + PX(t, 12, 20.4, 3, 1) : '';
  track('bassguitarA', bass(A.PB, { inner: P_PU }));
  track('bassguitarB', bass(A.JB, { inner: J_PU }));
  track('bassguitarC', bass(stretch(A.PB, 1.1), { inner: P_PU }));
  // Fretless: the fingerboard in the second ink (no frets), A with the P body, B with the J.
  // C: the board a cut line down a solid neck (the lined fretless).
  track('fretlessA', bass(A.JB, { nc: 'f2', inner: J_PU }));
  track('fretlessB', bass(A.PB, { nc: 'f2', inner: P_PU }));
  track('fretlessC', bass(A.JB, { inner: t => J_PU(t) + PL(t, 12, 5.6, 12, 15.2) }));
  // Acoustic bass: a big acoustic body, its hole, the long neck, two tuners a side.
  A.ABASS = 'M12 11.2 C14.6 11.2 15.8 12.4 15.8 14 C15.8 15.2 15 15.8 15 16.4 C15 17 18.6 17.8 18.6 20.4 C18.6 22.8 15.8 23.8 12 23.8 C8.2 23.8 5.4 22.8 5.4 20.4 C5.4 17.8 9 17 9 16.4 C9 15.8 8.2 15.2 8.2 14 C8.2 12.4 9.4 11.2 12 11.2 Z';
  const AB_HEAD = t => R(t, 10.4, 0.3, 13.6, 5) + (t.D ? PEGS(t, [[9.6, 1.6], [9.6, 3.6], [14.4, 1.6], [14.4, 3.6]], 1, 1) : '');
  track('acousticbassA', t => portrait(t, A.ABASS, { headD: AB_HEAD, xf: ID, nw: 2.2, n0: 5, n1: 14, inner: AC(14, 20.6, 1.7) }));
  track('acousticbassB', t => portrait(t, stretch(A.ABASS, 1.12), { headD: AB_HEAD, xf: ID, nw: 2.2, n0: 5, n1: 14, inner: AC(14, 20.6, 1.7) }));
  track('acousticbassC', t => portrait(t, A.CUTD, { headD: AB_HEAD, xf: xfOf(0.86, 0.8, 23.8), nw: 2.2, n0: 5, n1: 12.6, inner: AC(12.8, 19.6, 1.9) }));

  // ---- the headline guitars and the baritone, at 45° -------------------------------------
  // A long neck under the body makes the emoji-like "guitar" sign; the body stays large.
  const neckF = (y0, y1, w = 2.4) => `<rect class="f1" x="${12 - w / 2}" y="${y0}" width="${w}" height="${y1 - y0}"/>`;
  const head = (kind, dy) => F(window.GTR.mapD(window.GTR.HEADS[kind], ([x, y]) => [x, y + dy]));
  const cuts = (t, s, d) => `<path class="scut" fill="none" stroke-width="${+(1 / t.k / s).toFixed(3)}" d="${d}"/>`;
  // electric: the double cutaway, its pickups cut from 16 px; C adds the cable from the jack.
  const EG = (t, s, neck0) => at45(t, head('inline', neck0 - 4.4) + neckF(neck0 - 0.2, 11) + F(E.S1) +
    (t.D ? cuts(t, s, 'M9.8 13.6 H14.2 M9.8 16 H14.2 M9.8 18.6 L14.2 18') : ''), neck0 - 4.6, 23.4, s);
  track('eguitarA', t => EG(t, 0.84, -4));
  track('eguitarB', t => EG(t, 0.94, -1));
  track('eguitarC', t => EG(t, 0.84, -4) + (t.D ? `<g transform="scale(${t.k})"><path class="s1" fill="none" stroke-width="${+(1 / t.k).toFixed(3)}" stroke-linecap="round" d="M7.4 21.6 C9 23.4 12.6 23 14.6 21.6 C16.4 20.4 18.6 20.6 22.4 22.4"/></g>` : ''));
  // acoustic: the dreadnought, hole and bridge; C adds its pickguard in the second ink.
  const AG = (t, s, neck0, guard) => at45(t, head('square', neck0 - 4.6) + neckF(neck0 - 0.2, 12) + F(A.DREAD) +
    (guard && t.D ? F(GUARD(13.6, 14.2), 'f2') : '') + `<circle class="cut" cx="12" cy="12.6" r="${t.D ? 2 : 2.2}"/>` +
    (t.D ? cuts(t, s, 'M9.4 19.6 H14.6') : ''), neck0 - 4.8, 23.6, s);
  track('aguitarA', t => AG(t, 0.84, -4));
  track('aguitarB', t => AG(t, 0.94, -1));
  track('aguitarC', t => AG(t, 0.84, -4, true));
  // baritone: a smaller offset body on a much longer neck (the long scale is its name).
  const BAR = window.GTR.mapD(E.OF1, ([x, y]) => [12 + (x - 12) * 0.86, 23.4 + (y - 23.4) * 0.86]);
  track('baritoneA', t => at45(t, head('inline', -11.4) + neckF(-7.2, 13) + F(BAR) + (t.D ? cuts(t, 0.8, 'M10.2 16.4 H13.8 M10.2 18.8 H13.8') : ''), -11.6, 23.4, 0.8));
  track('baritoneB', t => at45(t, head('inline', -11.4) + neckF(-7.2, 13) + F(window.GTR.mapD(E.S1, ([x, y]) => [12 + (x - 12) * 0.86, 23.4 + (y - 23.4) * 0.86])) + (t.D ? cuts(t, 0.8, 'M10.2 15.6 H13.8 M10.2 18.6 H13.8') : ''), -11.6, 23.4, 0.8));
  // C: the baritone upright, like the shapes, its neck reaching the top over a small body.
  track('baritoneC', t => portrait(t, E.OF1, { head: 'inline', xf: xfOf(0.8, 0.7), n1: 12, inner: t => t.D ? PX(t, 12, 15, 3, 1) + PX(t, 12, 18, 3, 1) : '' }));
})();
