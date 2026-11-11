// Cantus icon family — mixer strip and inspector, part 2: routes, sidechain, freeze, VCA,
// routing, the insert list, and the small furniture (grip, mini view, more, add, fold).
var N = window.N || (window.N = {});

// Send: the signal line goes on; a branch leaves it up and to the right.
const send = (base, x1, x2, tip, arm) => {
  const [tx, ty] = tip, x0 = tx - (base - 1 - ty);
  return cells(seg(x1, base, x2, base).concat(seg(x0, base - 1, tx, ty), seg(tx - arm, ty, tx, ty), seg(tx, ty, tx, ty + arm)));
};
N.send = {
  12: { svg: send(9, 1, 10, [10, 2], 3) },
  14: { svg: send(11, 1, 12, [12, 3], 4) },
  16: { svg: send(12, 2, 13, [13, 3], 4) },
  18: { svg: send(13, 3, 14, [14, 4], 4) },
  21: { svg: send(15, 3, 16, [16, 4], 5) },
  24: { svg: send(17, 4, 18, [18, 5], 5) },
};

// Output (to a bus), input (into the strip), return (a send comes back).
N.out = {
  12: { svg: cells(seg(2, 6, 7, 6).concat(head(10, 6, 3, 1))) },
  18: { svg: cells(seg(3, 9, 11, 9).concat(head(15, 9, 4, 1))) },
};
N.into = {
  12: { svg: cells(seg(1, 6, 4, 6).concat(head(7, 6, 3, 1), seg(9, 2, 9, 10))) },
  18: { svg: cells(seg(2, 9, 8, 9).concat(head(12, 9, 4, 1), seg(14, 3, 14, 15))) },
};
N.ret = {
  12: { svg: cells(seg(3, 2, 3, 8).concat(seg(4, 8, 7, 8), head(10, 8, 3, 1))) },
  18: { svg: cells(seg(4, 3, 4, 12).concat(seg(5, 12, 11, 12), head(15, 12, 4, 1))) },
};

// Sidechain key: a key — its bow, its shaft, two teeth.
N.key = {
  12: { px: [ring(5, 1, 4)], svg: cells(seg(6, 6, 10, 6).concat(seg(8, 7, 8, 8), seg(10, 7, 10, 8))) },
  14: { px: [ring(5, 2, 5)], svg: cells(seg(7, 7, 12, 7).concat(seg(10, 8, 10, 9), seg(12, 8, 12, 10))) },
  18: { px: [ring(7, 2, 6)], svg: cells(seg(9, 9, 15, 9).concat(seg(12, 10, 12, 11), seg(15, 10, 15, 12))) },
  21: { px: [ring(8, 2, 7)], svg: cells(seg(10, 10, 18, 10).concat(seg(15, 11, 15, 12), seg(18, 11, 18, 13))) },
};

// Freeze: a snowflake — eight arms; from 18 px the four main arms carry their branches.
const flake = (c, a, tick) => {
  let l = seg(c, c - a, c, c + a).concat(seg(c - a, c, c + a, c), seg(c - a, c - a, c + a, c + a), seg(c - a, c + a, c + a, c - a));
  if (tick) {
    const d = a - 2;
    l = l.concat([[c - 1, c - d - 1], [c + 1, c - d - 1], [c - 1, c + d + 1], [c + 1, c + d + 1],
      [c - d - 1, c - 1], [c - d - 1, c + 1], [c + d + 1, c - 1], [c + d + 1, c + 1]]);
  }
  return cells(l);
};
N.freeze = {
  12: { svg: flake(5, 4, false) },
  14: { svg: flake(6, 5, false) },
  18: { svg: flake(8, 6, true) },
  21: { svg: flake(10, 7, true) },
};

// VCA: a fader with its cap, and two followers ticked where the VCA puts them.
const vca = (s0, cap, fol, top, bot, ty) => `<rect class="f2" x="${s0}" y="${top}" width="1" height="${bot - top}"/>` +
  `<rect class="f1" x="${cap[0]}" y="${cap[1]}" width="${cap[2]}" height="${cap[3]}" rx="${cap[3] > 2 ? 1 : 0.5}"/>` +
  fol.map(x => `<rect class="f2" x="${x}" y="${top}" width="1" height="${bot - top}"/><rect class="f1" x="${x - 1}" y="${ty}" width="3" height="1"/>`).join('');
N.vca = {
  14: { svg: vca(3, [1, 6, 5, 3], [8, 11], 2, 13, 7) },
  16: { svg: vca(4, [2, 7, 5, 3], [10, 13], 2, 15, 8) },
  21: { svg: vca(5, [2, 9, 7, 3], [13, 17], 3, 19, 10) },
  24: { svg: vca(6, [3, 11, 7, 3], [15, 19], 4, 21, 12) },
};

// Routing (show the cables): two sockets joined by a patch cable.
N.routing = {
  16: { px: [ring(5, 2, 2), ring(5, 9, 9)], svg: '<path class="s1" stroke-width="1" d="M7 4.5 C9.5 4.5 6.5 11.5 9 11.5"/>' },
  20: { px: [ring(7, 2, 2), ring(7, 11, 11)], svg: '<path class="s1" stroke-width="1" d="M9 5.5 C12.5 5.5 7.5 14.5 11 14.5"/>' },
  24: { px: [ring(8, 3, 3), ring(8, 13, 13)], svg: '<path class="s1" stroke-width="1" d="M11 6.5 C15 6.5 9 17.5 13 17.5"/>' },
  30: { svg: '<circle class="s1" stroke-width="2" cx="8" cy="8" r="3.5"/><circle class="s1" stroke-width="2" cx="22" cy="22" r="3.5"/><path class="s1" stroke-width="2" d="M11.5 8 C17.5 8 12.5 22 18.5 22"/>' },
};

// FX (the insert chain on or off): the insert list — a lamp and a slot per row.
const fx = (dot, bar, rows, h) => rows.map(y => `<rect class="f1" x="${dot}" y="${y}" width="${h}" height="${h}"${h > 2 ? ' rx="1"' : ''}/><rect class="f2" x="${bar[0]}" y="${y}" width="${bar[1]}" height="${h}" rx="${h / 2}"/>`).join('');
N.fx = {
  14: { svg: fx(3, [6, 5], [3, 6, 9], 2) },
  16: { svg: fx(4, [7, 6], [3, 7, 11], 2) },
  21: { svg: fx(5, [9, 7], [4, 9, 14], 3) },
  24: { svg: fx(6, [10, 8], [6, 11, 16], 2) },
};

// The insert row's lamp: on (the play colour, with a halo) or bypassed (a ring).
N.lamp = {
  10: { svg: '<circle class="f1" cx="5" cy="5" r="4.5" fill-opacity="0.18"/><circle class="f1" cx="5" cy="5" r="2.5"/>' },
  15: { svg: '<circle class="f1" cx="7.5" cy="7.5" r="6.5" fill-opacity="0.18"/><circle class="f1" cx="7.5" cy="7.5" r="3.6"/>' },
};
N.lampOff = {
  10: { svg: '<circle class="s1" stroke-width="1" cx="5" cy="5" r="2"/>' },
  15: { svg: '<circle class="s1" stroke-width="1" cx="7.5" cy="7.5" r="3"/>' },
};

// Grip (drag an insert), mini view (open an insert's small view), more, add, fold.
N.grip = {
  12: { svg: [3, 7].map(x => [2, 5, 8].map(y => `<rect class="f1" x="${x}" y="${y}" width="2" height="2"/>`).join('')).join('') },
  18: { svg: [5, 10].map(x => [4, 8, 12].map(y => `<rect class="f1" x="${x}" y="${y}" width="2" height="2"/>`).join('')).join('') },
};
N.miniView = {
  12: { svg: '<path class="s1" stroke-width="1" d="M1.5 2.5 H10.5 V9.5 H1.5 Z M2 4.5 H10"/>' },
  16: { svg: '<path class="s1" stroke-width="1" d="M2.5 3.5 H13.5 V12.5 H2.5 Z M3 6.5 H13"/>' },
  18: { svg: '<path class="s1" stroke-width="1" d="M2.5 3.5 H15.5 V14.5 H2.5 Z M3 6.5 H15"/>' },
  24: { svg: '<path class="s1" stroke-width="1" d="M3.5 5.5 H20.5 V18.5 H3.5 Z M4 9.5 H20"/>' },
};
N.more = {
  12: { svg: [2, 5, 8].map(x => `<rect class="f1" x="${x}" y="5" width="2" height="2"/>`).join('') },
  16: { svg: [3, 7, 11].map(x => `<rect class="f1" x="${x}" y="7" width="2" height="2"/>`).join('') },
  18: { svg: [4, 8, 12].map(x => `<rect class="f1" x="${x}" y="8" width="2" height="2"/>`).join('') },
  24: { svg: [7, 12, 17].map(x => `<circle class="f1" cx="${x}" cy="12" r="1.5"/>`).join('') },
};
N.add = {
  10: { svg: '<rect class="f1" x="4" y="1" width="1" height="7"/><rect class="f1" x="1" y="4" width="7" height="1"/>' },
  12: { svg: '<rect class="f1" x="5" y="2" width="1" height="7"/><rect class="f1" x="2" y="5" width="7" height="1"/>' },
  15: { svg: '<rect class="f1" x="7" y="2" width="1" height="11"/><rect class="f1" x="2" y="7" width="11" height="1"/>' },
  18: { svg: '<rect class="f1" x="8" y="3" width="1" height="12"/><rect class="f1" x="3" y="8" width="12" height="1"/>' },
};
const chev = (n, x, y, dir) => cells(dir === 'down'
  ? seg(x, y, x + n - 1, y + n - 1).concat(seg(x + 2 * n - 2, y, x + n - 1, y + n - 1))
  : seg(x, y, x + n - 1, y + n - 1).concat(seg(x, y + 2 * n - 2, x + n - 1, y + n - 1)));
N.foldOpen = { 8: { svg: chev(4, 0, 2, 'down') }, 10: { svg: chev(4, 1, 3, 'down') }, 12: { svg: chev(5, 1, 3, 'down') }, 15: { svg: chev(6, 2, 4, 'down') } };
N.foldShut = { 8: { svg: chev(4, 2, 0, 'right') }, 10: { svg: chev(4, 3, 1, 'right') }, 12: { svg: chev(5, 3, 1, 'right') }, 15: { svg: chev(6, 4, 2, 'right') } };
