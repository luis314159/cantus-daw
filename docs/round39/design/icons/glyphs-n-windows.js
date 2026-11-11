// Cantus icon family, second pass — the window keys, as icons rather than diagrams: one
// idea each (clips as pills, a piano's keys, rows of steps, faders, a folder, a plug, a
// panel with its sidebar).
var N = window.N || (window.N = {});
const R = (x, y, w, h, c = 'f1', rx = 0) => `<rect class="${c}" x="${x}" y="${y}" width="${w}" height="${h}"${rx ? ` rx="${rx}"` : ''}/>`;

// Arrangement: three clips on three lanes, staggered.
N.arrangement = {
  16: { svg: R(3, 4, 7, 2, 'f1', 0.5) + R(6, 7, 7, 2, 'f1', 0.5) + R(3, 10, 5, 2, 'f1', 0.5) },
  20: { svg: R(4, 5, 8, 2, 'f1', 1) + R(8, 9, 8, 2, 'f1', 1) + R(4, 13, 5, 2, 'f1', 1) },
  24: { svg: R(5, 6, 9, 2, 'f1', 1) + R(10, 11, 9, 2, 'f1', 1) + R(5, 16, 6, 2, 'f1', 1) },
  30: { svg: R(7, 8, 10, 3, 'f1', 1.5) + R(12, 14, 11, 3, 'f1', 1.5) + R(7, 20, 7, 3, 'f1', 1.5) },
  36: { svg: R(7, 10, 13, 4, 'f1', 2) + R(14, 16, 15, 4, 'f1', 2) + R(7, 22, 9, 4, 'f1', 2) },
};

// Piano roll: a piano's keys from above — three white keys, two black ones.
N.pianoRoll = {
  16: { svg: '<path class="s1" stroke-width="1" d="M3.5 4.5 H12.5 V11.5 H3.5 Z"/>' + R(6, 8, 1, 3) + R(9, 8, 1, 3) + R(5, 5, 2, 3) + R(9, 5, 2, 3) },
  20: { svg: '<path class="s1" stroke-width="1" d="M3.5 5.5 H15.5 V14.5 H3.5 Z"/>' + R(7, 10, 1, 4) + R(11, 10, 1, 4) + R(6, 6, 3, 4) + R(10, 6, 3, 4) },
  24: { svg: '<path class="s1" stroke-width="1" d="M4.5 6.5 H19.5 V17.5 H4.5 Z"/>' + R(9, 12, 1, 5) + R(14, 12, 1, 5) + R(8, 7, 3, 5) + R(13, 7, 3, 5) },
  30: { svg: '<path class="s1" stroke-width="2" stroke-linejoin="round" d="M6 7 H24 V23 H6 Z"/>' + R(11, 16, 2, 7) + R(17, 16, 2, 7) + R(10, 7, 4, 9) + R(16, 7, 4, 9) },
  36: { svg: '<path class="s1" stroke-width="2" stroke-linejoin="round" d="M6 8 H30 V28 H6 Z"/>' + R(13, 18, 2, 10) + R(21, 18, 2, 10) + R(12, 8, 4, 10) + R(20, 8, 4, 10) },
};

// Channel rack: two rows — a name and its steps, lit (main ink) or not (second ink).
const rackRows = (name, steps, size, rows, pattern, rx) => rows.map((y, r) =>
  R(name[0], y + name[2], name[1], name[3], 'f1', rx ? 0.5 : 0) +
  steps.map((x, i) => R(x, y, size, size, pattern[r][i] === '1' ? 'f1' : 'f2', rx)).join('')).join('');
N.rack = {
  16: { svg: rackRows([2, 2, 1, 1], [5, 8, 11], 2, [5, 9], ['101', '010'], 0) },
  20: { svg: rackRows([3, 2, 1, 1], [6, 10, 14], 3, [6, 11], ['101', '010'], 0) },
  24: { svg: rackRows([5, 3, 1, 1], [9, 13, 17], 3, [7, 14], ['101', '010'], 0.5) },
  30: { svg: rackRows([6, 3, 1, 2], [11, 16, 21], 4, [9, 17], ['101', '010'], 1) },
  36: { svg: rackRows([7, 4, 1, 2], [14, 20, 26], 4, [11, 21], ['101', '010'], 1) },
};

// Mixer: three faders — thin slots (second ink) and solid caps (main ink) at three levels.
const faders = (xs, sw, top, bottom, cw, ch, caps, rx) =>
  xs.map(x => R(x, top, sw, bottom - top, 'f2')).join('') +
  xs.map((x, i) => R(x + sw / 2 - cw / 2, caps[i], cw, ch, 'f1', rx)).join('');
N.mixer = {
  16: { svg: faders([3, 7, 11], 1, 3, 13, 3, 2, [8, 4, 10], 0) },
  20: { svg: faders([5, 9, 13], 1, 4, 16, 3, 2, [10, 6, 12], 0) },
  24: { svg: faders([6, 11, 16], 1, 5, 19, 3, 3, [12, 7, 14], 0.5) },
  30: { svg: faders([8, 14, 20], 2, 7, 23, 4, 3, [14, 9, 17], 1) },
  36: { svg: faders([10, 17, 24], 2, 9, 27, 6, 4, [17, 11, 21], 1) },
};

// Browser: a folder.
N.browser = {
  16: { svg: '<path class="s1" stroke-width="1" d="M3.5 4.5 H6.5 L7.5 5.5 H12.5 V11.5 H3.5 Z"/>' },
  20: { svg: '<path class="s1" stroke-width="1" d="M4.5 5.5 H8.5 L9.5 6.5 H15.5 V14.5 H4.5 Z"/>' },
  24: { svg: '<path class="s1" stroke-width="1" d="M5.5 6.5 H10.5 L11.5 7.5 H18.5 V17.5 H5.5 Z"/><path class="s1" stroke-width="1" d="M5.5 9.5 H18.5"/>' },
  30: { svg: '<path class="s1" stroke-width="2" stroke-linejoin="round" d="M7 8 H12 L14 10 H23 V22 H7 Z"/><path class="s1" stroke-width="2" d="M7 13 H23"/>' },
  36: { svg: '<path class="s1" stroke-width="2" stroke-linejoin="round" d="M8 10 H15 L17 12.5 H28 V26 H8 Z"/><path class="s1" stroke-width="2" d="M8 16 H28"/>' },
};

// Plug-ins: a plug. Solid where it is small, drawn in line from 24 px.
N.plugins = {
  16: { px: [P(4, 3, ['.#....#.', '.#....#.', '########', '########', '.######.', '..####..', '...##...', '...##...', '...##...', '...##...'])] },
  20: { px: [P(5, 4, ['..#....#..', '..#....#..', '..#....#..', '##########', '##########', '.########.', '..######..', '...####...', '....##....', '....##....', '....##....', '....##....'])] },
  24: { svg: R(9, 5, 1, 3) + R(14, 5, 1, 3) + '<path class="s1" stroke-width="1" d="M6.5 8.5 H17.5 V10 A5.5 5.5 0 0 1 6.5 10 Z"/>' + R(11, 16, 2, 3) },
  30: { svg: R(11, 6, 2, 4) + R(17, 6, 2, 4) + '<path class="s1" stroke-width="2" stroke-linejoin="round" d="M8 11 H22 V13 A7 7 0 0 1 8 13 Z"/>' + R(14, 20, 2, 4) },
  36: { svg: R(13, 7, 2, 5) + R(21, 7, 2, 5) + '<path class="s1" stroke-width="2" stroke-linejoin="round" d="M10 13 H26 V15.5 A8 8 0 0 1 10 15.5 Z"/>' + R(17, 24, 2, 5) },
};

// Inspector: a panel with its right-hand sidebar open.
N.inspector = {
  16: { svg: '<path class="s1" stroke-width="1" d="M3.5 4.5 H12.5 V11.5 H3.5 Z"/>' + R(9, 5, 3, 6) },
  20: { svg: '<path class="s1" stroke-width="1" d="M4.5 5.5 H15.5 V14.5 H4.5 Z"/><path class="s1" stroke-width="1" d="M10.5 6 V14"/>' + R(12, 8, 2, 1) + R(12, 10, 2, 1) + R(12, 12, 2, 1) },
  24: { svg: '<path class="s1" stroke-width="1" d="M5.5 6.5 H18.5 V17.5 H5.5 Z"/><path class="s1" stroke-width="1" d="M13.5 7 V17"/>' + R(15, 9, 2, 1) + R(15, 11, 2, 1) + R(15, 13, 2, 1) },
  30: { svg: '<path class="s1" stroke-width="2" stroke-linejoin="round" d="M7 8 H23 V22 H7 Z"/><path class="s1" stroke-width="2" d="M17 8 V22"/>' + R(19, 11, 2, 2) + R(19, 14, 2, 2) + R(19, 17, 2, 2) },
  36: { svg: '<path class="s1" stroke-width="2" stroke-linejoin="round" d="M8 10 H28 V26 H8 Z"/><path class="s1" stroke-width="2" d="M21 10 V26"/>' + R(23, 13, 3, 2) + R(23, 17, 3, 2) + R(23, 21, 3, 2) },
};
