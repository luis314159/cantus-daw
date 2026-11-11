// Cantus icon family — the guitar family, part MX: Mexican and Latin strings, upright like the
// acoustic shapes (round 39, icons-guitars). Helpers in glyphs-t-guitars.js. Each carries the
// one feature that names it, large enough for 14 px: the requinto its espiga, the vihuela and
// the guitarrón their bowl backs (small body on a long neck; a huge body on a short one), the
// bajo sexto its tall pear and its comb of twelve tuners, the jarana its pinched waist and low
// hole, the charango its banded shell, the cuatro its corners, the tres its three courses.
(() => {
  const { ID, LIFT, PEGS, mapD, stretch, Fs, R, PX, PL, HOLE, portrait } = window.GTR;
  const M = window.GTR_M = {};
  const AC = (hy, by, r = 1.8) => t => HOLE(t, 12, hy, t.D ? r : r * 1.1) + (t.D ? PX(t, 12, by, t.D === 2 ? 5 : 3, 1) : '');
  // a head with n nubs a side (from 16 px), w sheet units wide
  const pegHead = (y1, n, w = 3.4) => t => R(t, 12 - w / 2, 0.2, 12 + w / 2, y1) +
    (t.D ? PEGS(t, Array.from({ length: n }, (_, i) => 0.2 + (y1 - 0.2) * (i + 0.5) / n).flatMap(y => [[12 - w / 2 - 0.5 / t.k, y], [12 + w / 2 + 0.5 / t.k, y]]), 1, 1) : '');
  const COMB = (y1, w = 3.6) => t => {
    const n = Math.max(2, Math.floor((y1 - 0.2) * t.k / 2));
    return R(t, 12 - w / 2, 0.2, 12 + w / 2, y1) + PEGS(t, Array.from({ length: n }, (_, i) => 0.2 + (i * 2 + 1.5) / t.k).flatMap(y => [[12 - w / 2 - 0.5 / t.k, y], [12 + w / 2 + 0.5 / t.k, y]]), 1, 1);
  };
  const up = (body, o) => t => portrait(t, body, Object.assign({ xf: ID, head: 'square' }, o));
  // The small instruments are drawn large (a small instrument drawn small is a smudge at 14 px):
  // their bodies grow about the foot; the long neck still tells their size.
  const BIG = window.GTR.xfOf(1.3, 1.12, 23.6);

  // ---- requinto (jarocho): a slender body on a long neck, and the espiga, the long horn
  // plectrum that plays it, laid across it (lifted by a cut halo).
  M.REQ = 'M12 10.8 C13.8 10.8 14.8 12 14.8 13.6 C14.8 14.8 14 15.4 14 16 C14 16.6 16.4 17.4 16.4 19.8 C16.4 22.4 14.6 23.6 12 23.6 C9.4 23.6 7.6 22.4 7.6 19.8 C7.6 17.4 10 16.6 10 16 C10 15.4 9.2 14.8 9.2 13.6 C9.2 12 10.2 10.8 12 10.8 Z';
  const ESPIGA = (x0, y0, x1, y1, w0, w1) => {
    const a = Math.atan2(y1 - y0, x1 - x0), nx = -Math.sin(a), ny = Math.cos(a);
    const p = (x, y, w, s) => `${+(x + nx * w * s).toFixed(2)} ${+(y + ny * w * s).toFixed(2)}`;
    return `M${p(x0, y0, w0 / 2, 1)} L${p(x1, y1, w1 / 2, 1)} L${p(x1, y1, w1 / 2, -1)} L${p(x0, y0, w0 / 2, -1)} Z`;
  };
  const halo = (t, d) => `<path class="scut" fill="none" stroke-width="${+(3 / t.k).toFixed(3)}" stroke-linejoin="round" d="${d}"/>`;
  const REQ = (o = {}) => up(M.REQ, Object.assign({ xf: BIG, headD: pegHead(5, 2), n0: 4, n1: 14, nw: 2.4, inner: AC(14, 21) }, o));
  // A: the espiga standing beside the body, leaning, in the main ink (a bar across the body
  // read as a "no guitar" sign); B: the same in the second ink; C: across the foot only.
  const ESP_A = ESPIGA(2.2, 8.2, 3.8, 23.2, 1.4, 2.8);
  track('requintoA', t => REQ({ top: t => halo(t, ESP_A) + `<path class="f1" d="${ESP_A}"/>` })(t));
  const ESP_B = ESP_A;
  track('requintoB', t => REQ({ top: t => `<path class="f2" d="${ESP_B}"/>` })(t));
  const ESP_C = ESPIGA(1.4, 15.4, 9.6, 23.4, 1.2, 2.4);
  track('requintoC', t => REQ({ top: t => halo(t, ESP_C) + `<path class="f1" d="${ESP_C}"/>` })(t));

  // ---- vihuela: a small, round body, its vaulted back showing beside it in the second ink.
  M.VIH = 'M12 11 C14.4 11 15.6 12.2 15.6 14 C15.6 15 15 15.6 15 16.2 C15 16.8 17.6 17.6 17.6 20 C17.6 22.6 15.2 23.6 12 23.6 C8.8 23.6 6.4 22.6 6.4 20 C6.4 17.6 9 16.8 9 16.2 C9 15.6 8.4 15 8.4 14 C8.4 12.2 9.6 11 12 11 Z';
  const bowl = (d, dx, dy, s = 1) => mapD(d, ([x, y]) => [12 + (x - 12) * s + dx, 17.4 + (y - 17.4) * s + dy]);
  const BOWL = (d, dx, dy, s) => t => Fs(t, bowl(d, dx, dy, s), 'f2') + LIFT(t, d);
  track('vihuelaA', up(M.VIH, { xf: BIG, headD: pegHead(4.6, 2), n1: 14, nw: 2.4, back: BOWL(M.VIH, 2.4, 0.2, 1.04), inner: AC(14.2, 20.6) }));
  track('vihuelaB', up(M.VIH, { xf: BIG, headD: pegHead(4.6, 2), n1: 14, nw: 2.4, back: BOWL(M.VIH, 1.4, 0.4, 1.16), inner: AC(14.2, 20.6) }));
  // C: the bowl's V, seen below the body (from a little under).
  track('vihuelaC', up(M.VIH, { xf: BIG, headD: pegHead(4.6, 2), n1: 14, nw: 2.4, back: t => Fs(t, 'M7.4 21.6 L12 24 L16.6 21.6 Z', 'f2') + LIFT(t, M.VIH), inner: AC(14.2, 20.6) }));

  // ---- guitarrón: the huge body over a short neck, its bowl back beside it.
  M.GTR = 'M12 6.6 C16.2 6.6 18.4 8.2 18.4 10.6 C18.4 12.4 17.2 13 17.2 13.8 C17.2 14.4 21.8 15.4 21.8 18.6 C21.8 21.8 17.6 23.4 12 23.4 C6.4 23.4 2.2 21.8 2.2 18.6 C2.2 15.4 6.8 14.4 6.8 13.8 C6.8 13 5.6 12.4 5.6 10.6 C5.6 8.2 7.8 6.6 12 6.6 Z';
  const GTRN = (o) => up(o.body || M.GTR, Object.assign({ headD: pegHead(4, 2, 4.2), n0: 3.8, n1: 8, nw: 2.8, inner: AC(11.4, 19.4, 2.2) }, o));
  track('guitarronA', GTRN({ body: mapD(M.GTR, ([x, y]) => [x - 0.8, y - 0.2]), back: BOWL(mapD(M.GTR, ([x, y]) => [x - 0.8, y - 0.2]), 1.8, 0.4, 0.98) }));
  track('guitarronB', GTRN({ back: t => Fs(t, 'M4 21 L12 24 L20 21 Z', 'f2') + LIFT(t, M.GTR) }));
  track('guitarronC', GTRN({}));

  // ---- bajo sexto: a tall pear (the neck meets it at the twelfth fret, high), a comb of
  // twelve tuners on a long head.
  M.BSX = 'M12 7.2 C14.4 7.2 15.6 8.6 15.6 10.6 C15.6 12.4 14.6 13.2 14.8 14.2 C15 15.2 20 16.2 20 19.8 C20 22.6 16.6 23.8 12 23.8 C7.4 23.8 4 22.6 4 19.8 C4 16.2 9 15.2 9.2 14.2 C9.4 13.2 8.4 12.4 8.4 10.6 C8.4 8.6 9.6 7.2 12 7.2 Z';
  track('bajosextoA', up(M.BSX, { headD: COMB(6.2), n0: 6, n1: 11.6, nw: 2.8, inner: AC(11.6, 20.4, 1.7) }));
  track('bajosextoB', up(stretch(M.BSX, 1.1), { headD: COMB(6.2, 4.2), n0: 6, n1: 11.6, nw: 2.8, inner: AC(11.6, 20.4, 1.7) }));
  track('bajosextoC', up(M.BSX, { headD: COMB(6.2), n0: 6, n1: 11.6, nw: 2.8, inner: t => AC(11.6, 20.4, 1.7)(t) + (t.D ? PX(t, 12, 17.2, 1, 3, 'cut') : '') }));

  // ---- jarana: slim and small, a deep pinched waist, the hole low in the lower bout, a wide
  // head for its eight strings.
  M.JAR = 'M12 11.4 C13.8 11.4 14.6 12.6 14.6 14.2 C14.6 15.4 13.6 16.2 13.4 16.8 C15.4 17.4 16.6 18.8 16.6 20.6 C16.6 22.8 14.6 23.8 12 23.8 C9.4 23.8 7.4 22.8 7.4 20.6 C7.4 18.8 8.6 17.4 10.6 16.8 C10.4 16.2 9.4 15.4 9.4 14.2 C9.4 12.6 10.2 11.4 12 11.4 Z';
  track('jaranaA', up(M.JAR, { xf: BIG, headD: pegHead(5.4, 3, 4.6), n0: 5, n1: 14, nw: 2.4, inner: t => HOLE(t, 12, 19.8, t.D ? 1.5 : 1.6) + (t.D === 2 ? PX(t, 12, 22.2, 4, 1) : '') }));
  track('jaranaB', up(M.JAR, { xf: BIG, headD: pegHead(5.4, 3, 4.6), n0: 5, n1: 14, nw: 2.4, inner: AC(14.2, 21.4, 1.4) }));
  track('jaranaC', up(stretch(M.JAR, 1.1), { xf: BIG, headD: COMB(5.4, 4.6), n0: 5, n1: 14, nw: 2.4, inner: t => HOLE(t, 12, 19.8, t.D ? 1.5 : 1.6) }));

  // ---- charango: seen from the back, the banded armadillo shell.
  M.CHA = M.VIH;
  const bands = (ys) => t => ys(t).map(y => PL(t, 5, y, 19, y)).join('');
  track('charangoA', up(M.CHA, { xf: BIG, headD: pegHead(5, 2), n1: 14, nw: 2.4, inner: bands(t => t.D === 2 ? [13, 15.2, 17.4, 19.6, 21.8] : t.D ? [13.6, 16.4, 19.2, 21.8] : [14.4, 18.4]) }));
  track('charangoB', up(M.CHA, { xf: BIG, headD: pegHead(5, 2), n1: 14, nw: 2.4, inner: bands(t => t.D ? [14.4, 18, 21.4] : [16.6]) }));
  track('charangoC', up(stretch(M.CHA, 1.1), { xf: BIG, headD: pegHead(5, 2), n1: 14, nw: 2.4, inner: bands(t => t.D === 2 ? [13, 15.2, 17.4, 19.6, 21.8] : t.D ? [13.6, 16.4, 19.2, 21.8] : [14.4, 18.4]) }));

  // ---- cuatro (Puerto Rican): the violin-like corners pushed out at the waist.
  M.CUA = 'M12 9.6 C14.6 9.6 16 10.8 16 12.6 C16 13.6 15.4 14.2 15 14.6 L17.6 14.4 L16.4 16 C17.8 17 18.6 18.2 18.6 19.8 C18.6 22.4 15.8 23.6 12 23.6 C8.2 23.6 5.4 22.4 5.4 19.8 C5.4 18.2 6.2 17 7.6 16 L6.4 14.4 L9 14.6 C8.6 14.2 8 13.6 8 12.6 C8 10.8 9.4 9.6 12 9.6 Z';
  track('cuatroA', up(M.CUA, { xf: BIG, headD: pegHead(4.6, 2), n1: 12.4, nw: 2.4, inner: AC(12.4, 20.4, 1.6) }));
  track('cuatroB', up(stretch(M.CUA, 1.12), { xf: BIG, headD: pegHead(4.6, 2), n1: 12.4, nw: 2.4, inner: AC(12.4, 20.4, 1.6) }));
  track('cuatroC', up(M.CUA, { xf: BIG, headD: pegHead(4.6, 2), n1: 12.4, nw: 2.4, inner: AC(19, 21.6, 1.6) }));

  // ---- tres (Cuban): three courses — three string lines down the body.
  M.TRES = 'M12 9.6 C14.6 9.6 15.8 10.8 15.8 12.6 C15.8 13.8 14.8 14.4 14.8 15.2 C14.8 15.8 18.8 16.8 18.8 19.6 C18.8 22.4 15.8 23.6 12 23.6 C8.2 23.6 5.2 22.4 5.2 19.6 C5.2 16.8 9.2 15.8 9.2 15.2 C9.2 14.4 8.2 13.8 8.2 12.6 C8.2 10.8 9.4 9.6 12 9.6 Z';
  const THREE = t => [-1, 0, 1].map(i => PL(t, 12 + i * 2 / t.k, 10.4, 12 + i * 2 / t.k, 21)).join('');
  track('tresA', up(M.TRES, { xf: BIG, headD: pegHead(4.6, 3), n1: 12, nw: 2.4, inner: THREE }));
  track('tresB', up(M.TRES, { xf: BIG, headD: pegHead(4.6, 3), n1: 12, nw: 2.4, inner: t => AC(13, 21, 1.8)(t) + (t.D ? [-1, 0, 1].map(i => PL(t, 12 + i * 2 / t.k, 15.4, 12 + i * 2 / t.k, 20.4)).join('') : '') }));
  track('tresC', up(stretch(M.TRES, 1.1), { xf: BIG, headD: pegHead(4.6, 3), n1: 12, nw: 2.4, inner: THREE }));
})();
