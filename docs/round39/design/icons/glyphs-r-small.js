// Cantus icon family — set 5, part 3: the small bare marks (menus, fields, lists, title bars):
// 8–12 px at 100 %, 12–18 at 150 %, and the 12–16-px marks a field or a row carries.
var N = window.N || (window.N = {});
const tri = (tipX, tipY, n, dir) => cells(head(tipX, tipY, n, dir));
const triDown = (tipX, tipY, n) => {
  const out = [];
  for (let r = 0; r < n; r++) for (let c = -r; c <= r; c++) out.push([tipX + c, tipY - r]);
  return cells(out);
};
const mirrorX = (list, w) => list.map(([x, y]) => [w - 1 - x, y]);
const chevCells = (n, x, y) => seg(x, y, x + n - 1, y + n - 1).concat(seg(x, y + 2 * n - 2, x + n - 1, y + n - 1));

// Menu ▸ (a window's or a slot's menu) and Down ▾ (a field that opens a list): solid.
N.menu = { 10: { svg: tri(6, 5, 3, 1) }, 12: { svg: tri(7, 6, 4, 1) }, 15: { svg: tri(9, 7, 4, 1) }, 18: { svg: tri(11, 9, 5, 1) } };
N.down = { 8: { svg: triDown(4, 5, 3) }, 10: { svg: triDown(5, 6, 3) }, 12: { svg: triDown(6, 7, 4) }, 15: { svg: triDown(7, 9, 5) } };

// Prev ‹ and Next ›: 1-px chevrons.
N.next = { 10: { svg: cells(chevCells(4, 3, 1)) }, 12: { svg: cells(chevCells(5, 4, 1)) }, 15: { svg: cells(chevCells(6, 5, 2)) }, 18: { svg: cells(chevCells(7, 6, 2)) } };
N.prev = { 10: { svg: cells(mirrorX(chevCells(4, 3, 1), 10)) }, 12: { svg: cells(mirrorX(chevCells(5, 4, 1), 12)) }, 15: { svg: cells(mirrorX(chevCells(6, 5, 2), 15)) }, 18: { svg: cells(mirrorX(chevCells(7, 6, 2), 18)) } };

// Close ✕ — set 4's clear, plus the title bar's 16 px (24 at 150 %).
N.close = {
  10: N.clear[10], 12: N.clear[12], 15: N.clear[15], 18: N.clear[18],
  16: { svg: cells(seg(4, 4, 11, 11).concat(seg(4, 11, 11, 4))) },
  24: { svg: cells(seg(6, 6, 17, 17).concat(seg(6, 17, 17, 6))) },
};

// The title bar: minimise (a bar at the foot), maximise (a window, its title edge), restore.
N.minimise = { 10: { svg: box(2, 7, 6, 1) }, 12: { svg: box(2, 8, 8, 1) }, 15: { svg: box(3, 10, 9, 1) }, 18: { svg: box(4, 12, 10, 1) } };
N.maximise = {
  10: { svg: s1('M1.5 1.5 H8.5 V8.5 H1.5 Z') + box(1, 1, 8, 2) },
  12: { svg: s1('M1.5 2.5 H10.5 V10.5 H1.5 Z') + box(1, 2, 10, 2) },
  15: { svg: s1('M2.5 2.5 H12.5 V12.5 H2.5 Z') + box(2, 2, 11, 2) },
  18: { svg: s1('M3.5 3.5 H14.5 V14.5 H3.5 Z') + box(3, 3, 12, 2) },
};
N.restore = {
  10: { svg: s1('M1.5 3.5 H6.5 V8.5 H1.5 Z') + box(1, 3, 6, 2) + s1('M3.5 3 V1.5 H8.5 V6.5 H7') },
  12: { svg: s1('M1.5 4.5 H7.5 V10.5 H1.5 Z') + box(1, 4, 7, 2) + s1('M3.5 4 V1.5 H10.5 V8.5 H8') },
  15: { svg: s1('M2.5 5.5 H9.5 V12.5 H2.5 Z') + box(2, 5, 8, 2) + s1('M5.5 5 V2.5 H12.5 V9.5 H10') },
  18: { svg: s1('M3.5 6.5 H11.5 V14.5 H3.5 Z') + box(3, 6, 9, 2) + s1('M6.5 6 V3.5 H14.5 V11.5 H12') },
};

// Resize grip ◢ (a corner you drag): three diagonal lines in the second ink.
N.resizeGrip = {
  10: { svg: cells(seg(8, 4, 4, 8).concat(seg(8, 6, 6, 8), [[8, 8]]), 'f2') },
  12: { svg: cells(seg(10, 4, 4, 10).concat(seg(10, 7, 7, 10), [[10, 10]]), 'f2') },
  15: { svg: cells(seg(12, 5, 5, 12).concat(seg(12, 8, 8, 12), seg(12, 11, 11, 12)), 'f2') },
  18: { svg: cells(seg(15, 5, 5, 15).concat(seg(15, 9, 9, 15), seg(15, 13, 13, 15)), 'f2') },
};

// Resize ⤢: a diagonal with an L head at each end.
const dbl = (a, b, arm) => cells(seg(a + 1, b - 1, b - 1, a + 1).concat(seg(b - arm, a, b, a), seg(b, a, b, a + arm), seg(a, b - arm, a, b), seg(a, b, a + arm, b)));
N.resize = { 12: { svg: dbl(2, 9, 3) }, 16: { svg: dbl(3, 12, 4) }, 18: { svg: dbl(4, 13, 4) }, 24: { svg: dbl(5, 18, 5) } };

// Typed (a value can be typed): the text cursor.
N.typed = {
  12: { svg: box(3, 1, 5, 1) + box(5, 2, 1, 8) + box(3, 10, 5, 1) },
  16: { svg: box(5, 2, 5, 1) + box(7, 3, 1, 10) + box(5, 13, 5, 1) },
  18: { svg: box(6, 2, 5, 1) + box(8, 3, 1, 12) + box(6, 15, 5, 1) },
  24: { svg: box(8, 3, 7, 1) + box(11, 4, 1, 16) + box(8, 20, 7, 1) },
};

// Send open △ (a send that would loop): set 3's send with its branch broken (every other
// pixel) — the route exists but would feed back.
const sendOpen = (base, x1, x2, tip, arm) => {
  const [tx, ty] = tip, x0 = tx - (base - 1 - ty);
  const branch = seg(x0, base - 1, tx, ty).filter((_, i, a) => i % 2 === 0 || i === a.length - 1);
  return cells(seg(x1, base, x2, base).concat(branch, seg(tx - arm, ty, tx, ty), seg(tx, ty, tx, ty + arm)));
};
N.sendOpen = {
  12: { svg: sendOpen(9, 1, 10, [10, 2], 3) },
  16: { svg: sendOpen(12, 2, 13, [13, 3], 4) },
  18: { svg: sendOpen(13, 3, 14, [14, 4], 4) },
  24: { svg: sendOpen(17, 4, 18, [18, 5], 5) },
};

window.REST_ROWS = [
  ['help', 'help', [16, 20, 24], [24, 30, 36], ''], ['warning', 'warning', [16, 20, 24], [24, 30, 36], 'solo'],
  ['agent', 'agent', [16, 20, 24], [24, 30, 36], ''], ['device', 'device', [16, 20, 24], [24, 30, 36], ''],
  ['keyboard', 'typing keyboard', [16, 20, 24], [24, 30, 36], ''], ['score', 'score', [16, 20, 24], [24, 30, 36], ''],
  ['mouseMenu', 'mouse menu', [16, 20, 24], [24, 30, 36], ''], ['automate', 'automate', [16, 20, 24], [24, 30, 36], ''],
  ['filter', 'filter', [16, 20, 24], [24, 30, 36], ''], ['replace', 'replace', [16, 20, 24], [24, 30, 36], ''],
  ['warp', 'warp', [16, 20, 24], [24, 30, 36], ''], ['pluginStopped', 'plug-in stopped', [16, 20, 24], [24, 30, 36], 'rec'],
  ['controller', 'controller', [16, 20, 24], [24, 30, 36], ''], ['lanePencil', 'lane: pencil', [16, 20, 24], [24, 30, 36], ''],
  ['laneLine', 'lane: line', [16, 20, 24], [24, 30, 36], ''], ['laneCurve', 'lane: curve', [16, 20, 24], [24, 30, 36], ''],
  ['session', 'session', [16, 20, 24], [24, 30, 36], ''], ['stage', 'stage', [16, 20, 24], [24, 30, 36], ''],
  ['menu', 'menu ▸', [10, 12], [15, 18], ''], ['down', 'down ▾', [8, 10], [12, 15], ''],
  ['prev', 'prev ‹', [10, 12], [15, 18], ''], ['next', 'next ›', [10, 12], [15, 18], ''],
  ['close', 'close', [10, 12, 16], [15, 18, 24], ''], ['minimise', 'minimise', [10, 12], [15, 18], ''],
  ['maximise', 'maximise', [10, 12], [15, 18], ''], ['restore', 'restore', [10, 12], [15, 18], ''],
  ['resizeGrip', 'resize grip', [10, 12], [15, 18], ''], ['resize', 'resize', [12, 16], [18, 24], ''],
  ['typed', 'typed', [12, 16], [18, 24], ''], ['sendOpen', 'send (would loop)', [12, 16], [18, 24], ''],
];
