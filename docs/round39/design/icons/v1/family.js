// Cantus icon family, round 39 — the renderer for the design sheets.
//
// Nothing here draws a glyph. Every glyph is drawn by hand, once per device size, in
// glyphs-*.js. This file only holds the ink tokens, the placeholder key the sheets show the
// glyphs in, and the two notations the drawings are written in:
//
//   1. Pixel notation (16 and 20 px, and any detail that must be exact): rows of characters,
//      one character per device pixel.
//        '#' ink-1   '*' ink-1 at 60 %   '+' ink-1 at 30 %   (hand anti-aliasing)
//        'O' ink-2   'o' ink-2 at 55 %   '-' ink-2 at 25 %
//        'R' ink-rec 'r' ink-rec at 50 % 'q' ink-rec at 22 %
//        '.' nothing
//   2. Snapped vector (24, 30 and 36 px): SVG elements whose straight edges sit on whole
//      pixels (fills) or half pixels (odd strokes). Classes:
//        f1/f2/fr  fill with ink-1 / ink-2 / ink-rec
//        s1/s2     stroke with ink-1 / ink-2 (width given on the element)
//
// A drawing is { c: canvasSize, svg: "<…>" } or { c, px: [rows…] } or both (px drawn last).

const INK = {
  night: {
    bar: '#2b3036', well: '#22262b', edge: '#15181b', hi: 'rgba(255,255,255,0.035)',
    ink1: '#c9d1d8', ink2: '#7e8892', inkOn: '#f2b347', inkOff: '#4c545c',
    inkOnDim: '#9a7338', inkRec: '#c4564c', inkRecOn: '#ff4f3f', onWell: 'rgba(242,179,71,0.13)',
    onEdge: 'rgba(242,179,71,0.55)', text: '#c9d1d8', text2: '#7e8892', page: '#1d2125',
    rule: '#343a41',
  },
  paper: {
    bar: '#e6e8eb', well: '#d9dde1', edge: '#b3b9c0', hi: 'rgba(255,255,255,0.5)',
    ink1: '#2a3138', ink2: '#78828c', inkOn: '#b5600a', inkOff: '#b0b6bd',
    inkOnDim: '#d49a5c', inkRec: '#b8473d', inkRecOn: '#d8291b', onWell: 'rgba(214,128,20,0.16)',
    onEdge: 'rgba(181,96,10,0.55)', text: '#2a3138', text2: '#6b747d', page: '#eef0f2',
    rule: '#cfd4d9',
  },
};

// The device sizes that get their own drawing. 24 serves both "24 at 100 %" and
// "16 at 150 %" (the same device pixels in the same 36-px key).
const SIZES = [16, 20, 24, 30, 36];
// Key height for each canvas: one and a half canvases (24/30/36 at 100 %, 45/54 at 150 %).
const KEY = { 16: 24, 20: 30, 24: 36, 30: 45, 36: 54 };
const RADIUS = { 16: 3, 20: 3, 24: 4, 30: 5, 36: 6 };

const PXMAP = {
  '#': ['ink1', 1], '*': ['ink1', 0.6], '+': ['ink1', 0.3],
  'O': ['ink2', 1], 'o': ['ink2', 0.55], '-': ['ink2', 0.25],
  'R': ['inkRec', 1], 'r': ['inkRec', 0.5], 'q': ['inkRec', 0.22],
};

function pxToSvg(rows) {
  let out = '';
  rows.forEach((row, y) => {
    let x = 0;
    while (x < row.length) {
      const ch = row[x];
      if (ch === '.' || ch === ' ' || !PXMAP[ch]) { x++; continue; }
      let w = 1;
      while (row[x + w] === ch) w++;
      const [ink, a] = PXMAP[ch];
      out += `<rect x="${x}" y="${y}" width="${w}" height="1" fill="var(--${ink})"` +
        (a < 1 ? ` fill-opacity="${a}"` : '') + '/>';
      x += w;
    }
  });
  return out;
}

function glyphSvg(d) {
  if (!d) return '';
  let s = d.svg || '';
  if (d.px) s += `<g shape-rendering="crispEdges">${pxToSvg(d.px)}</g>`;
  // `at`: a whole-pixel offset, so a pixel drawing can be written compactly and then placed.
  if (d.at) s = `<g transform="translate(${d.at[0]},${d.at[1]})">${s}</g>`;
  return s;
}

const STYLE = `
.f1{fill:var(--ink1)} .f2{fill:var(--ink2)} .fr{fill:var(--inkRec)}
.s1{fill:none;stroke:var(--ink1)} .s2{fill:none;stroke:var(--ink2)}
.on{--ink1:var(--inkOn);--ink2:var(--inkOnDim);--inkRec:var(--inkRecOn)}
.off{--ink1:var(--inkOff);--ink2:var(--inkOff);--inkRec:var(--inkOff)}
`;

function themeVars(t) {
  const k = INK[t];
  return `--bar:${k.bar};--well:${k.well};--edge:${k.edge};--ink1:${k.ink1};` +
    `--ink2:${k.ink2};--inkOn:${k.inkOn};--inkOnDim:${k.inkOnDim};--well:${k.well};--inkOff:${k.inkOff};` +
    `--inkRec:${k.inkRec};--inkRecOn:${k.inkRecOn};`;
}

// The placeholder key: a flat dark well a little darker than the bar, a 1-px edge, a
// barely-there top line. The directions own the real key; this one only shows the glyph.
function keySvg(t, c, x, y, state) {
  const k = INK[t];
  const K = KEY[c];
  const r = RADIUS[c];
  let s = `<g transform="translate(${x},${y})">`;
  s += `<rect x="0.5" y="0.5" width="${K - 1}" height="${K - 1}" rx="${r}" fill="${k.well}" stroke="${k.edge}"/>`;
  s += `<rect x="${r}" y="1" width="${K - 2 * r}" height="1" fill="${k.hi}"/>`;
  if (state === 'on') {
    s += `<rect x="0.5" y="0.5" width="${K - 1}" height="${K - 1}" rx="${r}" fill="${k.onWell}" stroke="${k.onEdge}"/>`;
  }
  s += '</g>';
  return s;
}

// A glyph centred in its key: the canvas origin is floored to a whole pixel.
function iconInKey(t, glyph, c, x, y, state) {
  const K = KEY[c];
  const off = Math.floor((K - c) / 2);
  const d = glyph && glyph[c];
  let s = keySvg(t, c, x, y, state);
  const cls = state === 'on' ? 'on' : state === 'off' ? 'off' : '';
  s += `<g class="${cls}" transform="translate(${x + off},${y + off})">${glyphSvg(d)}</g>`;
  if (!d) {
    s += `<text x="${x + K / 2}" y="${y + K / 2 + 3}" font-size="9" text-anchor="middle" fill="${INK[t].text2}">–</text>`;
  }
  return s;
}

function svgDoc(t, w, h, body) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}"` +
    ` style="${themeVars(t)}display:block"><style>${STYLE}</style>${body}</svg>`;
}

function label(t, x, y, text, opts = {}) {
  const k = INK[t];
  const size = opts.size || 11;
  const fill = opts.dim ? k.text2 : k.text;
  const anchor = opts.anchor || 'start';
  const weight = opts.weight || 400;
  return `<text x="${x}" y="${y}" font-family="Segoe UI, sans-serif" font-size="${size}"` +
    ` font-weight="${weight}" text-anchor="${anchor}" fill="${fill}">${text}</text>`;
}

if (typeof window !== 'undefined') {
  Object.assign(window, { INK, SIZES, KEY, RADIUS, pxToSvg, glyphSvg, keySvg, iconInKey, svgDoc, label });
}
