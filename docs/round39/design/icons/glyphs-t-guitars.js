// Cantus icon family — set 6, the guitar family redrawn (round 39, icons-guitars). Loaded after
// glyphs-t-final.js and before glyphs-t-families.js; it replaces every guitar, bass and
// Mexican-string drawing. The owner, 2026-11-09, twice: "aún dejan mucho que desear las
// guitarras, busca imágenes en internet para que veas" — at 14–16 px the electric shapes read
// alike. The study is guitars-study.md.
//
// What changed, in one line: the shapes stand UPRIGHT, body forward, as a portrait of the body.
// At 45° every edge of a 10-px body is antialiased and horns, waists and cutaways melt; upright,
// the axis, the horns and the bouts sit on the pixel grid, the body gets the key's whole width,
// and the outlines read apart. The headline electric and acoustic guitars (and the baritone,
// whose long neck is its name) keep the 45° pose of the emoji-like "guitar" sign, so the
// headline is never a twin of a shape.
//
// Construction: 24-unit sheet, body axis at x = 12, built per size (`track`). Unlike the other
// families, every coordinate of a guitar is SNAPPED to the size's device pixels (`snap`): the
// tips of the horns, the widest points of the bouts and the waists land on whole pixels at
// every size, which is what keeps 14 and 16 px crisp. Inner detail is placed in device pixels
// (`PX`), so a pickup or an f-hole is exactly one pixel wide, never a 2-px smear.
// Paths here use only M, L, C and Z (pairs), so they can be snapped and mirrored.
(() => {
  const N = window.N;
  // Keep the drawings this file replaces, for the before/after sheet.
  window.GUITAR_IDS = ['eguitar', 'aguitar', 'bassguitar', 'archtop', 'resonator', 'baritone', 'fretless', 'acousticbass',
    'eStype', 'eTtype', 'eSingleCut', 'eDoubleHorn', 'eSuperstrat', 'eSemiHollow', 'eOffset', 'eVshape', 'ePointed',
    'aDread', 'aConcert', 'aCutaway', 'aJumbo', 'aParlor', 'classical', 'guitar12', 'aTravel',
    'requinto', 'vihuela', 'guitarron', 'bajosexto', 'jarana', 'charango', 'cuatro', 'tres'];
  window.GUITAR_IDS.forEach(id => { N[id + '_v2'] = N[id]; });

  // ---- helpers ------------------------------------------------------------------------
  const sn = (t, v) => Math.round(v * t.k) / t.k;
  const snap = (t, d) => d.replace(/-?\d+(\.\d+)?/g, m => +sn(t, +m).toFixed(3));
  // Map a path's coordinate pairs: f([x, y]) -> [x, y]. Paths are M/L/C/Z only.
  const mapD = (d, f) => {
    const nums = d.match(/-?\d+(\.\d+)?/g).map(Number);
    let i = 0;
    return d.replace(/-?\d+(\.\d+)?[ ,]+-?\d+(\.\d+)?/g, () => { const p = f([nums[i], nums[i + 1]]); i += 2; return `${+p[0].toFixed(3)} ${+p[1].toFixed(3)}`; });
  };
  // Stretch about the axis (sx), and about y = yc (sy): the exaggerations.
  const stretch = (d, sx, sy = 1, yc = 23.4) => mapD(d, ([x, y]) => [12 + (x - 12) * sx, yc + (y - yc) * sy]);
  const mirror = d => mapD(d, ([x, y]) => [24 - x, y]);
  const G = (t, inner) => `<g transform="scale(${t.k})">${inner}</g>`;
  // The body's exaggeration (set by `portrait` while it draws a body and its detail): every
  // position below passes through it; pixel sizes do not.
  let XF = null;
  const xy = (x, y) => XF ? XF([x, y]) : [x, y];
  const Fs = (t, d, c = 'f1') => `<path class="${c}" d="${snap(t, XF ? mapD(d, XF) : d)}"/>`;
  // A rect snapped to whole device pixels (at least one pixel each way), in sheet units.
  const R = (t, x0, y0, x1, y1, c = 'f1') => {
    [x0, y0] = xy(x0, y0); [x1, y1] = xy(x1, y1);
    const X0 = Math.round(x0 * t.k), Y0 = Math.round(y0 * t.k);
    const X1 = Math.max(X0 + 1, Math.round(x1 * t.k)), Y1 = Math.max(Y0 + 1, Math.round(y1 * t.k));
    return `<rect class="${c}" x="${+(X0 / t.k).toFixed(3)}" y="${+(Y0 / t.k).toFixed(3)}" width="${+((X1 - X0) / t.k).toFixed(3)}" height="${+((Y1 - Y0) / t.k).toFixed(3)}"/>`;
  };
  // A rect of w × h device pixels centred on a sheet point (the centre snapped so it is crisp).
  const PX = (t, cx, cy, w, h, c = 'cut') => {
    [cx, cy] = xy(cx, cy);
    const X0 = Math.round(cx * t.k - w / 2), Y0 = Math.round(cy * t.k - h / 2);
    return `<rect class="${c}" x="${+(X0 / t.k).toFixed(3)}" y="${+(Y0 / t.k).toFixed(3)}" width="${+(w / t.k).toFixed(3)}" height="${+(h / t.k).toFixed(3)}"/>`;
  };
  // A device-pixel line between two sheet points (1 px wide), for f-holes and slanted pickups.
  const PL = (t, x0, y0, x1, y1, c = 'scut', w = 1) => {
    const o = w % 2 ? 0.5 : 0;
    [x0, y0] = xy(x0, y0); [x1, y1] = xy(x1, y1);
    const a = v => (Math.round(v * t.k - o) + o) / t.k;
    return `<path class="${c}" fill="none" stroke-width="${+(w / t.k).toFixed(3)}" stroke-linecap="butt" d="M${+a(x0).toFixed(3)} ${+a(y0).toFixed(3)} L${+a(x1).toFixed(3)} ${+a(y1).toFixed(3)}"/>`;
  };
  // A round hole whose diameter is a whole number of device pixels (2 px is a square).
  const HOLE = (t, cx, cy, r, c = 'cut') => {
    [cx, cy] = xy(cx, cy);
    const d = Math.max(2, Math.round(2 * r * t.k));
    const X0 = Math.round(cx * t.k - d / 2), Y0 = Math.round(cy * t.k - d / 2);
    if (d <= 2) return `<rect class="${c}" x="${+(X0 / t.k).toFixed(3)}" y="${+(Y0 / t.k).toFixed(3)}" width="${+(d / t.k).toFixed(3)}" height="${+(d / t.k).toFixed(3)}"/>`;
    return `<circle class="${c}" cx="${+((X0 + d / 2) / t.k).toFixed(3)}" cy="${+((Y0 + d / 2) / t.k).toFixed(3)}" r="${+(d / 2 / t.k).toFixed(3)}"/>`;
  };
  // A ring (rosette, resonator plate edge) one device pixel wide.
  const RING = (t, cx, cy, r, c = 'scut') => {
    [cx, cy] = xy(cx, cy);
    const d = Math.max(3, Math.round(2 * r * t.k));
    const X0 = Math.round(cx * t.k - d / 2), Y0 = Math.round(cy * t.k - d / 2);
    return `<circle class="${c}" fill="none" stroke-width="${+(1 / t.k).toFixed(3)}" cx="${+((X0 + d / 2) / t.k).toFixed(3)}" cy="${+((Y0 + d / 2) / t.k).toFixed(3)}" r="${+((d - 1) / 2 / t.k).toFixed(3)}"/>`;
  };
  // The neck: 2 device px at 14–21 px, wider from 24 (sheet x 10.7–13.3, snapped).
  const NECK = (t, y0, y1, w = 2.8, c = 'f1') => R(t, 12 - w / 2, y0, 12 + w / 2, y1, c);

  // ---- heads (upright; the bass side is the viewer's left) ----------------------------
  const HEADS = {
    // six in line, leaning to the bass side: the double cutaway, slab, offset family
    inline: 'M13.4 4.4 L13.4 2 C13.4 0.9 12.8 0.3 11.8 0.3 L8.4 0.3 C7.4 0.3 7.1 1.3 7.9 2 L10.6 3.8 L10.6 4.4 Z',
    // three a side, flared: the carved, twin-horn, semi-hollow, archtop, V family
    three: 'M10.6 5 L9.4 0.9 C9.3 0.5 9.6 0.2 10 0.2 L14 0.2 C14.4 0.2 14.7 0.5 14.6 0.9 L13.4 5 Z',
    // pointed, leaning to the bass side: the pointed horns and pointed bodies
    pointy: 'M13.4 4.4 L13.8 1.8 L7 0 C7 0.8 7.6 1.4 8.6 1.9 L10.6 3.4 L10.6 4.4 Z',
    // the acoustic's square head
    square: 'M10.6 4.6 L9.6 0.8 C9.5 0.4 9.8 0.2 10.2 0.2 L13.8 0.2 C14.2 0.2 14.5 0.4 14.4 0.8 L13.4 4.6 Z',
  };
  const HEAD = (t, kind, dy = 0, c = 'f1') => Fs(t, mapD(HEADS[kind], ([x, y]) => [x, y + dy]), c);

  // An upright portrait: head, neck down into the body, the body, then its detail.
  // o: { head, hy (head shift), n0, n1 (neck), nw, inner(t) }
  // o.xf: the body's exaggeration (default ELEC: 1.14 wide, 0.98 tall, about the body's foot).
  const xfOf = (sx, sy, yc = 23.4) => ([x, y]) => [12 + (x - 12) * sx, yc + (y - yc) * sy];
  const ELEC = xfOf(1.14, 0.98);
  const portrait = (t, body, o = {}) => {
    const f = o.xf || ELEC;
    const n1 = f([12, o.n1 ?? 12])[1];
    let s = (o.headD ? o.headD(t) : HEAD(t, o.head || 'three', o.hy || 0)) + NECK(t, o.n0 ?? 4, n1, o.nw, o.nc);
    XF = f;
    s += (o.back ? o.back(t) : '') + Fs(t, body) + (o.inner ? o.inner(t) : '');
    XF = null;
    return G(t, s + (o.top ? o.top(t) : ''));
  };

  const ID = xfOf(1, 1);
  // A cut outline round a body (a 2-px stroke: one pixel shows outside), lifting it off what is
  // behind it (the bowl back of a vihuela or a guitarrón).
  const LIFT = (t, d) => `<path class="scut" fill="none" stroke-width="${+(2 / t.k).toFixed(3)}" stroke-linejoin="round" d="${snap(t, XF ? mapD(d, XF) : d)}"/>`;
  // Pegs: w × h device-pixel nubs at sheet points (a head's tuners).
  const PEGS = (t, pts, w = 1, h = 1, c = 'f1') => pts.map(([x, y]) => PX(t, x, y, w, h, c)).join('');
  // A whole instrument at 45°, body forward (the headline guitars and the baritone): drawn
  // upright on a long sheet (head above y = 0), turned about its middle, centred in the key.
  // No snapping: at 45° nothing sits on the grid; the outline is kept big instead.
  const at45 = (t, inner, y0, y1, s = 1) => {
    const cy = (y0 + y1) / 2;
    return `<g transform="scale(${t.k}) translate(12 12) rotate(45) scale(${s}) translate(-12 ${-cy})">${inner}</g>`;
  };
  window.GTR = { ID, LIFT, PEGS, at45, xfOf, ELEC, xy, sn, snap, mapD, stretch, mirror, G, Fs, R, PX, PL, HOLE, RING, NECK, HEADS, HEAD, portrait };
})();
