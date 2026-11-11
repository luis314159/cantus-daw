// Cantus icon family — set 6, the track icons: the user's own mark for a track, beside its
// type icon. Sizes: arrangement header 14/16 (21/24 at 150 %), mixer strip head 12/14
// (18/21), the picker 24 (36), and the picker at 125 % (15, 18, 20, 30).
//
// Construction: each glyph is designed on a 24-unit sheet and built for each size, never
// scaled: straight edges snap to that size's pixels (fills to whole pixels, 1-px lines to
// half pixels), the line is 1 px below 30 px and 2 px from 30, and each size keeps the detail
// it can hold (D0 at 12–15: silhouette; D1 at 16–21: + inner detail; D2 at 24+: full).
// The 12- and 14-px drawings were then read at 4x and corrected where the construction
// failed (see the overrides at the end of glyphs-t-tracks2.js).
var N = window.N || (window.N = {});
const TSIZES = [12, 14, 15, 16, 18, 20, 21, 24, 30, 36];

function tk(S) {
  const k = S / 24, w = S >= 30 ? 2 : 1, o = w % 2 ? 0.5 : 0;
  const D = S <= 15 ? 0 : S <= 21 ? 1 : 2;
  const f = v => Math.round(v * k);
  const l = v => Math.round(v * k - o) + o;
  const n = v => +(v * k).toFixed(2);
  const sc = d => d.replace(/-?\d+(\.\d+)?/g, m => n(+m));
  const cap = w > 1 ? ' stroke-linecap="round" stroke-linejoin="round"' : ' stroke-linejoin="round"';
  return {
    S, k, w, o, D, f, l, n,
    R: (x0, y0, x1, y1, c = 'f1', rx = 0) => `<rect class="${c}" x="${f(x0)}" y="${f(y0)}" width="${Math.max(1, f(x1) - f(x0))}" height="${Math.max(1, f(y1) - f(y0))}"${rx ? ` rx="${n(rx)}"` : ''}/>`,
    L: (x0, y0, x1, y1, c = 's1') => `<path class="${c}" stroke-width="${w}"${cap} d="M${l(x0)} ${l(y0)} L${l(x1)} ${l(y1)}"/>`,
    Poly: (pts, c = 's1') => `<path class="${c}" stroke-width="${w}"${cap} fill="none" d="M${pts.map(([x, y]) => `${l(x)} ${l(y)}`).join(' L')}"/>`,
    Box: (x0, y0, x1, y1, c = 's1', rx = 0) => `<rect class="${c}" stroke-width="${w}" x="${l(x0)}" y="${l(y0)}" width="${l(x1) - l(x0)}" height="${l(y1) - l(y0)}"${rx ? ` rx="${n(rx)}"` : ''}/>`,
    Ps: (d, c = 's1') => `<path class="${c}" stroke-width="${w}"${cap} fill="none" d="${sc(d)}"/>`,
    Pf: (d, c = 'f1', op) => `<path class="${c}"${op ? ` fill-opacity="${op}"` : ''} d="${sc(d)}"/>`,
    C: (cx, cy, r, c = 's1') => c[0] === 's' ? `<circle class="${c}" stroke-width="${w}" cx="${n(cx)}" cy="${n(cy)}" r="${Math.max(0.5, n(r) - (w === 1 ? 0 : 0))}"/>`
      : `<circle class="${c}" cx="${n(cx)}" cy="${n(cy)}" r="${Math.max(0.6, n(r))}"/>`,
    E: (cx, cy, rx, ry, rot, c = 'f1') => `<ellipse class="${c}"${c[0] === 's' ? ` stroke-width="${w}"` : ''} cx="${n(cx)}" cy="${n(cy)}" rx="${n(rx)}" ry="${n(ry)}" transform="rotate(${rot} ${n(cx)} ${n(cy)})"/>`,
  };
}
// Build N[id] for every size from a construction.
const track = (id, build) => { N[id] = {}; TSIZES.forEach(S => { N[id][S] = { svg: build(tk(S)) }; }); };

// ---- Drums
track('kick', t => t.C(12, 12, 9) + (t.D ? t.C(12, 12, 3.2, 'f2') : t.C(12, 12, 2.4, 'f1')) + (t.D === 2 ? t.C(12, 12, 6, 's2') : ''));
track('snare', t => t.Box(4, 11, 20, 19) + t.Ps('M5 3.5 L15 10 M19 3.5 L9 10') + (t.D ? t.L(8, 11, 8, 19, 's2') + t.L(12, 11, 12, 19, 's2') + t.L(16, 11, 16, 19, 's2') : ''));
track('hihat', t => t.Pf('M3 9 L12 6 L21 9 Z') + t.Pf('M3 11.5 L12 14 L21 11.5 Z') + t.L(12, 14, 12, 21, 's2') + (t.D ? t.L(8, 21, 16, 21, 's2') : ''));
track('cymbal', t => t.Pf('M2.5 10 L20.5 4.5 L21.5 6.5 L3.5 12 Z') + t.C(12, 7.6, 1.8, 'f1') + t.L(12, 9.5, 12, 21, 's2') + (t.D ? t.L(8, 21, 16, 21, 's2') : '') + (t.D === 2 ? t.Ps('M18 12 C19.5 13 20 14.5 20 16', 's2') : ''));
track('tom', t => t.Ps('M4 8 C4 4.5 20 4.5 20 8 C20 11.5 4 11.5 4 8 Z') + t.Ps('M4 8 V17 C4 20.5 20 20.5 20 17 V8') + (t.D ? t.Ps('M8 11 V19 M16 11 V19', 's2') : ''));
track('clap', t => t.E(9.5, 12.5, 3.3, 6.6, 28, 'f2') + t.E(13.5, 14.5, 3.3, 6.6, 28, 'f1') + t.Ps('M16 3.5 L17 5.5 M19.5 5 L21 6 M20.5 9 H22.5'));
track('shaker', t => t.Ps('M8.5 11 L16 21.5', 's2') + t.Ps('M15.5 11 L8 21.5') + t.C(7, 7.5, 4.6, 'f2') + t.C(17, 7.5, 4.6, 'f1') + (t.D ? t.C(16, 6.5, 0.9, 'cut') + t.C(18.2, 8.5, 0.9, 'cut') : ''));
track('drumkit', t => t.C(13, 15, 6.5) + t.C(13, 15, 1.8, 'f1') + t.L(2, 6, 10, 4) + t.L(6, 5, 6, 21, 's2') + (t.D ? t.E(19, 6.5, 3.2, 1.7, 0, 's1') : ''));

// ---- Bass
track('bass', t => t.C(6.5, 8.5, 2.2, 'f1') + t.Ps('M5 8 C5 3.5 15 2.5 16.5 8.5 C17.5 14 11.5 19 4.5 21.5') + t.C(20.5, 7, 1.5, 'f1') + t.C(20.5, 13, 1.5, 'f1'));
track('sub', t => t.Box(4, 3, 20, 21, 's1', 2.5) + t.C(12, 13.5, 5.5) + t.C(12, 13.5, t.D ? 1.8 : 2.2, 'f1') + (t.D ? t.C(12, 6.5, 1.1, 'f2') : ''));

// ---- Guitars, keys
const neck = t => t.Ps('M11 13 L19.5 4.5') + t.Pf('M18.2 3.3 L20.7 1.8 L22.2 3.3 L20.7 5.8 Z');
track('eguitar', t => t.Pf('M4.5 15 C3 18 5.5 22 9.5 20.5 C12.5 19.5 13.5 17 12 15 C13.5 13.5 12.5 10.5 10 11.5 C8 10.5 5.5 12 4.5 15 Z') + neck(t) + (t.D ? t.Ps('M6.5 16.5 L9.5 13.5', 'scut') : ''));
track('aguitar', t => t.Pf('M8.5 22 C4 22 3 18 5.5 16 C4 13.5 6 11 8.5 12 C11 11 13 13.5 11.5 16 C14 18 13 22 8.5 22 Z') + neck(t) + t.C(8.5, 17, t.D ? 1.9 : 1.5, 'cut'));
track('piano', t => t.Pf('M3 17 V9 C3 5 7 4 10 5.5 C12.5 6.5 13.5 9.5 17 10.5 C20 11.5 21 13.5 21 15.5 V17 Z') + t.R(3, 18, 21, 19.5, 'f2') + (t.D ? t.L(5, 19.5, 5, 22, 's2') + t.L(19, 19.5, 19, 22, 's2') : ''));
track('keys', t => t.Box(3, 6, 21, 18) + t.L(9, 13, 9, 18) + t.L(15, 13, 15, 18) + t.R(7, 6, 11, 13) + t.R(13, 6, 17, 13));

// ---- Synths
track('synth', t => t.Poly([[3, 15], [7.5, 15], [7.5, 8], [12, 8], [12, 15], [16.5, 15], [16.5, 8], [21, 8]]) + (t.D ? t.L(3, 19, 21, 19, 's2') : ''));
track('pad', t => t.Pf('M3 18.5 L8.5 7 H15.5 L21 18.5 Z', 'f2', 0.45) + t.Ps('M3 18.5 L8.5 7 H15.5 L21 18.5'));
track('lead', t => t.Poly([[3, 17], [9, 6], [9, 17], [15, 6], [15, 17], [21, 6]]));
track('pluck', t => t.Ps('M3 18.5 L6.5 4 C8 12 12 17 21 18.5') + (t.D ? t.L(3, 20.5, 21, 20.5, 's2') : ''));
track('arp', t => t.R(3, 16, 7, 18.5, 'f1', 0.8) + t.R(8, 12, 12, 14.5, 'f1', 0.8) + t.R(13, 8, 17, 10.5, 'f1', 0.8) + t.R(18, 4, 22, 6.5, 'f1', 0.8) + (t.D ? t.R(3, 20, 22, 21, 'f2') : ''));
