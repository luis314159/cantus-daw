// Cantus icon family — the mixer strip and inspector set, drawn for the sizes des-mixer
// places them at: 12 (inline in fields and rows), 14, 16, 18 (strip keys and the polarity row)
// and their 150 %: 18, 21, 24, 27. Same grammar as set2: 1-px lines up to 24 px, 2 px at 27;
// 45-degree lines and small rings are pixels; heads grow from their strokes.
var N = window.N || (window.N = {});

// Pixel helpers: a straight or 45-degree run of cells, and ring templates.
function seg(x0, y0, x1, y1) {
  const n = Math.max(Math.abs(x1 - x0), Math.abs(y1 - y0));
  const sx = Math.sign(x1 - x0), sy = Math.sign(y1 - y0);
  return Array.from({ length: n + 1 }, (_, i) => [x0 + i * sx, y0 + i * sy]);
}
function cells(list, cls = 'f1') {
  return `<g class="${cls}" shape-rendering="crispEdges">` +
    list.map(([x, y]) => `<rect x="${x}" y="${y}" width="1" height="1"/>`).join('') + '</g>';
}
const RING = {
  5: ['.###.', '#...#', '#...#', '#...#', '.###.'],
  7: ['..###..', '.#...#.', '#.....#', '#.....#', '#.....#', '.#...#.', '..###..'],
  8: ['..####..', '.#....#.', '#......#', '#......#', '#......#', '#......#', '.#....#.', '..####..'],
  10: ['...####...', '.##....##.', '.#......#.', '#........#', '#........#', '#........#', '#........#', '.#......#.', '.##....##.', '...####...'],
};
const ring = (n, x, y) => P(x, y, RING[n]);
const head = (tipX, tipY, n, dir) => {
  // A solid pixel arrowhead of n columns pointing left/right, tip at (tipX, tipY).
  const out = [];
  for (let c = 0; c < n; c++) for (let r = -c; r <= c; r++) out.push([tipX - dir * c, tipY + r]);
  return out;
};

// Polarity Ø: a ring and the stroke through it, a little past the ring.
N.phase = {
  12: { px: [ring(8, 2, 2)], svg: cells(seg(1, 10, 10, 1)) },
  16: { px: [ring(10, 3, 3)], svg: cells(seg(2, 13, 13, 2)) },
  18: { px: [ring(10, 4, 4)], svg: cells(seg(3, 14, 14, 3)) },
  24: { svg: '<circle class="s1" stroke-width="1" cx="12" cy="12" r="6.5"/>' + cells(seg(4, 19, 19, 4)) },
  27: { svg: '<circle class="s1" stroke-width="2" cx="13.5" cy="13.5" r="7"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M21.5 5.5 L5.5 21.5"/>' },
};

// Swap L/R: two arrows passing each other.
N.swap = {
  12: { svg: cells(seg(2, 3, 7, 3).concat(head(9, 3, 2, 1), seg(4, 8, 9, 8), head(2, 8, 2, -1))) },
  16: { svg: cells(seg(3, 5, 10, 5).concat(head(13, 5, 3, 1), seg(5, 10, 12, 10), head(2, 10, 3, -1))) },
  18: { svg: cells(seg(4, 6, 11, 6).concat(head(14, 6, 3, 1), seg(6, 11, 13, 11), head(3, 11, 3, -1))) },
  24: { svg: cells(seg(5, 8, 15, 8).concat(head(18, 8, 3, 1), seg(8, 15, 18, 15), head(5, 15, 3, -1))) },
  27: { svg: '<path class="s1" stroke-width="2" stroke-linecap="round" d="M7 10 H19.5 M20 17 H7.5"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M16.5 7 L20 10 L16.5 13 M10.5 14 L7 17 L10.5 20"/>' },
};

// Mono: one ring with its centre — one source.
N.mono = {
  12: { px: [ring(8, 2, 2)], svg: '<rect class="f1" x="5" y="5" width="2" height="2"/>' },
  16: { px: [ring(10, 3, 3)], svg: '<rect class="f1" x="7" y="7" width="2" height="2"/>' },
  18: { px: [ring(10, 4, 4)], svg: '<rect class="f1" x="8" y="8" width="2" height="2"/>' },
  24: { svg: '<circle class="s1" stroke-width="1" cx="12" cy="12" r="6.5"/><circle class="f1" cx="12" cy="12" r="1.7"/>' },
  27: { svg: '<circle class="s1" stroke-width="2" cx="13.5" cy="13.5" r="7"/><circle class="f1" cx="13.5" cy="13.5" r="2.2"/>' },
};

// Link (stereo pair, linked controls): two links of a chain, overlapping.
N.link = {
  12: { svg: '<rect class="s1" stroke-width="1" x="1.5" y="3.5" width="6" height="5" rx="2.5"/><rect class="s1" stroke-width="1" x="4.5" y="3.5" width="6" height="5" rx="2.5"/>' },
  16: { svg: '<rect class="s1" stroke-width="1" x="2.5" y="5.5" width="7" height="5" rx="2.5"/><rect class="s1" stroke-width="1" x="6.5" y="5.5" width="7" height="5" rx="2.5"/>' },
  18: { svg: '<rect class="s1" stroke-width="1" x="2.5" y="6.5" width="8" height="5" rx="2.5"/><rect class="s1" stroke-width="1" x="7.5" y="6.5" width="8" height="5" rx="2.5"/>' },
  24: { svg: '<rect class="s1" stroke-width="1" x="3.5" y="8.5" width="10" height="7" rx="3.5"/><rect class="s1" stroke-width="1" x="10.5" y="8.5" width="10" height="7" rx="3.5"/>' },
  27: { svg: '<rect class="s1" stroke-width="2" x="4" y="10" width="11" height="7" rx="3.5"/><rect class="s1" stroke-width="2" x="12" y="10" width="11" height="7" rx="3.5"/>' },
};

// Arm: the record dot (in the record colour when armed; in ink at rest).
N.arm = {
  14: { svg: '<circle class="f1" cx="7" cy="7" r="2.8"/>' },
  16: { svg: '<circle class="f1" cx="8" cy="8" r="3.2"/>' },
  18: { svg: '<circle class="f1" cx="9" cy="9" r="3.6"/>' },
  21: { svg: '<circle class="f1" cx="10.5" cy="10.5" r="4.2"/>' },
  24: { svg: '<circle class="f1" cx="12" cy="12" r="4.8"/>' },
  27: { svg: '<circle class="f1" cx="13.5" cy="13.5" r="5.4"/>' },
};

// Enable (an insert, a strip, a device): power — a ring open at the top and its stem.
const power = (c, r, top, len) => {
  const a = 55 * Math.PI / 180;
  const x1 = (c + r * Math.sin(a)).toFixed(2), x2 = (c - r * Math.sin(a)).toFixed(2), y = (c - r * Math.cos(a)).toFixed(2);
  return `<path class="s1" stroke-width="1" d="M${x1} ${y} A${r} ${r} 0 1 1 ${x2} ${y}"/><rect class="f1" x="${c - 0.5}" y="${top}" width="1" height="${len}"/>`;
};
N.enable = {
  12: { svg: power(6.5, 3.5, 2, 5) },
  14: { svg: power(7.5, 4, 2, 6) },
  16: { svg: power(8.5, 4.5, 3, 6) },
  18: { svg: power(9.5, 5.5, 3, 7) },
  21: { svg: power(10.5, 6, 3, 8) },
  24: { svg: power(12.5, 7, 4, 9) },
};
