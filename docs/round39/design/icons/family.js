// Cantus icon family, round 39, second pass — the renderer for the design sheets.
//
// The glyphs are drawn for Slate's key heights (dir-a, the owner's choice): 16, 20 and 24 px
// at 100 %, and 24, 30 and 36 px at 150 %. The device sizes are 16, 20, 24, 30 and 36, and each
// has its own drawing. A glyph's canvas is its key: coordinates are in the key's pixels.
//
// Notations (the drawings are in glyphs-n-*.js):
//   - Snapped vector: SVG whose straight edges sit on whole pixels (fills, even strokes)
//     or half pixels (odd strokes). Classes f1/f2/fr (fill: main ink, second ink, record)
//     and s1/s2 (stroke: main, second). A cut out of a solid uses fill/stroke var(--cut),
//     which is always the key's own well.
//   - Pixels: rows of characters, one per device pixel, placed with `at`:
//       '#' main  '*' main 60 %  '+' main 30 %   'O' second  'o' second 55 %
//       'R' record  'r' record 50 %  'q' record 22 %   '.' nothing
//   - Reuse: `v1: ['name', size]` takes a first-pass drawing that was made for exactly this
//     many pixels (v1 drew a 16-px canvas for a 24-px key, which is what Slate's 20-px key
//     holds, and so on), placed with `at`.
//
// A drawing is { svg?, px?: [{at:[x,y], rows:[…]}], v1?: [name, size], at? }.

const SLATE = {
  night: {
    surface: '#262d34', base: '#1d2329', well: '#1a1f25', wellHi: '#222930', edge: '#11151a',
    lip: '#2e363e', ink1: '#e3e8ed', ink2: '#a6b0ba', ink3: '#7a8692', ink4: '#47515b',
    accent: '#5cafe9', play: '#5dce8a', rec: '#ee5c5a', solo: '#ebb64d', page: '#1d2329', rule: '#323b44',
  },
  paper: {
    surface: '#e4e9ee', base: '#f4f6f8', well: '#d6dde3', wellHi: '#ccd4db', edge: '#b9c3cc',
    lip: '#eef2f5', ink1: '#17202a', ink2: '#4a5867', ink3: '#6a7684', ink4: '#a9b3bd',
    accent: '#1f6ec2', play: '#1f8f55', rec: '#d33b3b', solo: '#a86e06', page: '#f4f6f8', rule: '#c8d0d8',
  },
};

const KEYS = [16, 20, 24, 30, 36];
const KR = { 16: 3, 20: 3, 24: 4, 30: 5, 36: 6 };

function mix(a, b, t) {
  const p = h => [1, 3, 5].map(i => parseInt(h.slice(i, i + 2), 16));
  const [x, y] = [p(a), p(b)];
  return '#' + x.map((v, i) => Math.round(v + (y[i] - v) * t).toString(16).padStart(2, '0')).join('');
}

// The inks of a key in each state. `kind` is 'play', 'rec' or '' (the accent).
function inks(t, state, kind) {
  const k = SLATE[t];
  const onCol = kind === 'play' ? k.play : kind === 'rec' ? k.rec : kind === 'solo' ? k.solo : k.accent;
  const onWell = mix(k.well, onCol, 0.2);
  switch (state) {
    case 'hover': return { main: k.ink1, second: k.ink2, rec: k.rec, well: k.wellHi };
    case 'on': return { main: onCol, second: mix(onCol, k.well, 0.45), rec: k.rec, well: onWell };
    case 'off': return { main: k.ink4, second: k.ink4, rec: k.ink4, well: k.well };
    // A bare glyph at rest, without a well (des-mixer's polarity row, inline field glyphs).
    case 'quiet': return { main: k.ink3, second: k.ink4, rec: k.rec, well: k.surface };
    case 'bare': return { main: k.ink2, second: k.ink3, rec: k.rec, well: k.surface };
    default: return { main: k.ink2, second: k.ink3, rec: k.rec, well: k.well };
  }
}

const PXMAP = {
  '#': ['main', 1], '*': ['main', 0.6], '+': ['main', 0.3],
  'O': ['second', 1], 'o': ['second', 0.55],
  'R': ['rec', 1], 'r': ['rec', 0.5], 'q': ['rec', 0.22],
};

function pxRows(rows) {
  let out = '';
  rows.forEach((row, y) => {
    let x = 0;
    while (x < row.length) {
      const ch = row[x];
      if (!PXMAP[ch]) { x++; continue; }
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

function drawing(d) {
  if (!d) return '';
  let s = '';
  if (d.v1) {
    const g = (window.G1 || {})[d.v1[0]];
    const v = g && g[d.v1[1]];
    if (v) s += v1Svg(v);
  }
  (d.sub || []).forEach(x => { s += drawing(x); });
  if (d.svg) s += d.svg;
  (d.px || []).forEach(p => {
    s += `<g shape-rendering="crispEdges" transform="translate(${p.at[0]},${p.at[1]})">${pxRows(p.rows)}</g>`;
  });
  if (d.at) s = `<g transform="translate(${d.at[0]},${d.at[1]})">${s}</g>`;
  return s;
}

// A first-pass drawing, through the first pass's own notation (its '#'/'O' rows are the
// main and second inks here).
function v1Svg(v) {
  let s = v.svg || '';
  if (v.px) s += `<g shape-rendering="crispEdges">${pxRows(v.px)}</g>`;
  if (v.at) s = `<g transform="translate(${v.at[0]},${v.at[1]})">${s}</g>`;
  return s;
}

const STYLE = `
.f1{fill:var(--main)} .f2{fill:var(--second)} .fr{fill:var(--rec)}
.s1{fill:none;stroke:var(--main)} .s2{fill:none;stroke:var(--second)}
.cut{fill:var(--cut)} .scut{fill:none;stroke:var(--cut)}
`;

// Slate's well: a 1-px lip showing under, a 1-px dark edge, the flat well inside.
function key(t, K, x, y, state, kind, w) {
  const k = SLATE[t];
  const W = w || K;
  const r = KR[K];
  const ink = inks(t, state, kind);
  let s = `<g transform="translate(${x},${y})">`;
  s += `<rect width="${W}" height="${K}" rx="${r}" fill="${k.lip}"/>`;
  s += `<rect width="${W}" height="${K - 1}" rx="${r}" fill="${k.edge}"/>`;
  s += `<rect x="1" y="1" width="${W - 2}" height="${K - 2}" rx="${r - 1}" fill="${ink.well}"/>`;
  s += '</g>';
  return s;
}

function iconKey(t, glyph, K, x, y, state, kind, w) {
  const ink = inks(t, state || 'rest', kind);
  const W = w || K;
  const dx = Math.floor((W - K) / 2);
  let s = key(t, K, x, y, state || 'rest', kind, W);
  const vars = `--main:${ink.main};--second:${ink.second};--rec:${ink.rec};--cut:${ink.well}`;
  s += `<g style="${vars}" transform="translate(${x + dx},${y})">${drawing(glyph && glyph[K])}</g>`;
  return s;
}

function svgDoc(t, w, h, body) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}"` +
    ` style="display:block"><style>${STYLE}</style>${body}</svg>`;
}

function label(t, x, y, text, o = {}) {
  const k = SLATE[t];
  return `<text x="${x}" y="${y}" font-family="Segoe UI, sans-serif" font-size="${o.size || 11}"` +
    ` font-weight="${o.weight || 400}" text-anchor="${o.anchor || 'start'}"` +
    ` fill="${o.dim ? k.ink3 : k.ink1}">${text}</text>`;
}

// A glyph without a key, on the surface (state 'quiet', 'bare' or 'on' without the well).
function bareGlyph(t, glyph, C, x, y, state, kind) {
  const ink = inks(t, state || 'quiet', kind);
  const vars = `--main:${ink.main};--second:${ink.second};--rec:${ink.rec};--cut:${SLATE[t].surface}`;
  return `<g style="${vars}" transform="translate(${x},${y})">${drawing(glyph && glyph[C])}</g>`;
}

// Any-size key (the mixer's keys are 12–27 px): Slate's well with a radius for its size.
function anyKey(t, glyph, C, x, y, state, kind, w) {
  const k = SLATE[t];
  const W = w || C, r = C <= 14 ? 3 : C <= 20 ? 3 : C <= 24 ? 4 : 5;
  const ink = inks(t, state || 'rest', kind);
  let s = `<g transform="translate(${x},${y})"><rect width="${W}" height="${C}" rx="${r}" fill="${k.lip}"/>` +
    `<rect width="${W}" height="${C - 1}" rx="${r}" fill="${k.edge}"/>` +
    `<rect x="1" y="1" width="${W - 2}" height="${C - 2}" rx="${r - 1}" fill="${ink.well}"/></g>`;
  const vars = `--main:${ink.main};--second:${ink.second};--rec:${ink.rec};--cut:${ink.well}`;
  s += `<g style="${vars}" transform="translate(${x + Math.floor((W - C) / 2)},${y})">${drawing(glyph && glyph[C])}</g>`;
  return s;
}

Object.assign(window, { bareGlyph, anyKey, SLATE, KEYS, KR, mix, inks, pxRows, drawing, key, iconKey, svgDoc, label });
