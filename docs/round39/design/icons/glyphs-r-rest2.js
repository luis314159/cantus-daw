// Cantus icon family — set 5, part 2: replace, warp, plug-in stopped, controller, the lane
// tools, session, stage.
var N = window.N || (window.N = {});

// Replace: two cards and the turn from one to the other.
N.replace = {
  16: { svg: s1('M2.5 8.5 H6.5 V12.5 H2.5 Z M9.5 8.5 H13.5 V12.5 H9.5 Z') + s1('M4.5 7 C4.5 3 11.5 3 11.5 6') + cells([[10, 6], [11, 6], [12, 6], [11, 7]]) },
  20: { svg: s1('M3.5 10.5 H8.5 V15.5 H3.5 Z M11.5 10.5 H16.5 V15.5 H11.5 Z') + s1('M6 9 C6 4 14.5 4 14.5 8') + cells([[13, 8], [14, 8], [15, 8], [14, 9]]) },
  24: { svg: s1('M4.5 12.5 H10.5 V18.5 H4.5 Z M13.5 12.5 H19.5 V18.5 H13.5 Z') + s1('M7.5 11 C7.5 5 16.5 5 16.5 10') + cells([[15, 10], [16, 10], [17, 10], [16, 11]]) },
  30: { svg: s1('M5 15 H13 V23 H5 Z M17 15 H25 V23 H17 Z', 2, 'stroke-linejoin="round"') + s2r('M9 12 C9 5 21 5 21 11') + s2r('M18 9 L21 12 L24 9') },
  36: { svg: s1('M6 18 H15 V28 H6 Z M21 18 H30 V28 H21 Z', 2, 'stroke-linejoin="round"') + s2r('M10.5 14.5 C10.5 6 25.5 6 25.5 13.5') + s2r('M22 11 L25.5 14.5 L29 11') },
};

// Warp: a warp marker over a wave — packed on one side, stretched on the other.
const bars = (list) => list.map(([x, y0, y1, w]) => box(x, y0, w || 1, y1 - y0 + 1, 'f2')).join('');
N.warp = {
  16: { svg: bars([[2, 9, 11], [4, 8, 12], [6, 10, 11], [11, 8, 12], [14, 9, 11]]) + box(8, 7, 1, 7), px: [P(6, 3, ['#####', '#####', '.###.', '..#..'])] },
  20: { svg: bars([[3, 11, 13], [5, 9, 15], [7, 11, 14], [13, 9, 15], [17, 11, 13]]) + box(10, 7, 1, 10), px: [P(8, 3, ['#####', '#####', '.###.', '..#..'])] },
  24: { svg: bars([[4, 13, 16], [6, 11, 18], [8, 12, 17], [10, 14, 15], [15, 11, 18], [19, 13, 16]]) + box(12, 8, 1, 12), px: [P(10, 4, ['#####', '#####', '.###.', '..#..'])] },
  30: { svg: bars([[5, 16, 21, 2], [8, 13, 24, 2], [11, 15, 22, 2], [19, 13, 24, 2], [24, 16, 21, 2]]) + box(14, 10, 2, 15) + '<path class="f1" d="M11 5 H19 V8 L15 11 L11 8 Z"/>' },
  36: { svg: bars([[6, 19, 26, 2], [9.5, 16, 29, 2], [13, 18, 27, 2], [22.5, 16, 29, 2], [29, 19, 26, 2]].map(([x, a, b, w]) => [Math.round(x), a, b, w])) + box(17, 13, 2, 18) + '<path class="f1" d="M13 6 H23 V10 L18 14 L13 10 Z"/>' },
};

// Plug-in stopped: a plug pulled apart — its prongs and cap above, its body below, air between.
N.pluginStopped = {
  16: { px: [P(4, 2, ['.#....#.', '.#....#.', '########', '........', '........', '.######.', '..####..', '...##...', '...##...', '...##...', '...##...'])] },
  20: { px: [P(5, 3, ['..#....#..', '..#....#..', '..#....#..', '##########', '..........', '..........', '.########.', '..######..', '...####...', '....##....', '....##....', '....##....', '....##....'])] },
  24: { svg: box(9, 4, 1, 3) + box(14, 4, 1, 3) + s1('M6.5 7.5 H17.5 V9.5 H6.5 Z') + s1('M6.5 12.5 H17.5 V13 A5.5 5.5 0 0 1 6.5 13 Z') + box(11, 19, 2, 3) },
  30: { svg: box(11, 5, 2, 4) + box(17, 5, 2, 4) + s1('M8 9 H22 V12 H8 Z', 2, 'stroke-linejoin="round"') + s1('M8 15 H22 V16 A7 7 0 0 1 8 16 Z', 2, 'stroke-linejoin="round"') + box(14, 23, 2, 4) },
  36: { svg: box(13, 5, 2, 5) + box(21, 5, 2, 5) + s1('M10 10 H26 V13 H10 Z', 2, 'stroke-linejoin="round"') + s1('M10 17 H26 V18.5 A8 8 0 0 1 10 18.5 Z', 2, 'stroke-linejoin="round"') + box(17, 27, 2, 5) },
};

// Controller: a knob beside a fader — MIDI learn, the mappings pane.
N.controller = {
  16: { px: [ring(7, 2, 5)], svg: cells(seg(5, 8, 7, 6)) + box(11, 3, 1, 11, 'f2') + box(10, 6, 3, 2) },
  20: { px: [ring(8, 2, 6)], svg: cells(seg(6, 9, 8, 7)) + box(14, 4, 1, 13, 'f2') + box(13, 8, 3, 2) },
  24: { px: [ring(10, 3, 7)], svg: cells(seg(8, 11, 10, 9)) + box(17, 5, 1, 15, 'f2') + box(15, 9, 5, 2) },
  30: { svg: '<circle class="s1" stroke-width="2" cx="11" cy="15" r="5"/>' + s2r('M11 15 L14 12') + box(21, 6, 2, 18, 'f2') + box(18, 10, 8, 3, 'f1', 1) },
  36: { svg: '<circle class="s1" stroke-width="2" cx="13" cy="18" r="6"/>' + s2r('M13 18 L16.5 14.5') + box(25, 7, 2, 22, 'f2') + box(21, 12, 10, 4, 'f1', 1) },
};

// Lane tools: a pencil drawing a line; a straight segment between two points; a bent one.
N.lanePencil = {
  16: { sub: [{ ...N.draw[16], at: [1, -2] }], svg: s1('M2 13.5 Q3.5 10.5 5.5 11.5') },
  20: { sub: [{ v1: ['draw', 16], at: [3, 1] }], svg: s1('M2.5 16.5 Q4.5 13 6.5 14') },
  24: { sub: [{ v1: ['draw', 20], at: [3, 1] }], svg: s1('M2.5 19.5 Q5 16 7.5 17') },
  30: { sub: [{ v1: ['draw', 24], at: [4, 1] }], svg: s2r('M4 25 Q7 20 10 21.5') },
  36: { sub: [{ v1: ['draw', 30], at: [3, 0] }], svg: s2r('M5 30 Q8.5 24 12 26') },
};
N.laneLine = {
  16: { svg: box(2, 10, 3, 3) + box(11, 3, 3, 3) + cells(seg(5, 9, 10, 4)) },
  20: { svg: box(3, 13, 3, 3) + box(14, 4, 3, 3) + cells(seg(6, 12, 13, 5)) },
  24: { svg: box(4, 16, 3, 3, 'f1', 0.5) + box(17, 5, 3, 3, 'f1', 0.5) + cells(seg(7, 15, 16, 6)) },
  30: { svg: '<circle class="f1" cx="8" cy="22" r="2.6"/><circle class="f1" cx="22" cy="8" r="2.6"/>' + s2r('M10 20 L20 10') },
  36: { svg: '<circle class="f1" cx="9" cy="27" r="3"/><circle class="f1" cx="27" cy="9" r="3"/>' + s2r('M11.5 24.5 L24.5 11.5') },
};
N.laneCurve = {
  16: { svg: box(2, 10, 3, 3) + box(11, 3, 3, 3) + s1('M3.5 10 C3.5 5.5 6.5 4.5 11 4.5') },
  20: { svg: box(3, 13, 3, 3) + box(14, 4, 3, 3) + s1('M4.5 13 C4.5 7 8.5 5.5 14 5.5') },
  24: { svg: box(4, 16, 3, 3, 'f1', 0.5) + box(17, 5, 3, 3, 'f1', 0.5) + s1('M5.5 16 C5.5 8.5 10 6.5 17 6.5') },
  30: { svg: '<circle class="f1" cx="8" cy="22" r="2.6"/><circle class="f1" cx="22" cy="8" r="2.6"/>' + s2r('M8 19.5 C8 11 13 8 19.5 8') },
  36: { svg: '<circle class="f1" cx="9" cy="27" r="3"/><circle class="f1" cx="27" cy="9" r="3"/>' + s2r('M9 24 C9 14 15 9 24 9') },
};

// Session: a grid of clip cells, one playing (main ink), the rest waiting (second ink).
const grid = (xs, s, rx, lit) => xs.map((y, r) => xs.map((x, c) => box(x, y, s, s, r === lit[0] && c === lit[1] ? 'f1' : 'f2', rx)).join('')).join('');
N.session = {
  16: { svg: grid([3, 7, 11], 2, 0, [1, 0]) },
  20: { svg: grid([4, 9, 14], 3, 0, [1, 0]) },
  24: { svg: grid([4, 10, 16], 4, 0.8, [1, 0]) },
  30: { svg: grid([6, 13, 20], 4, 1, [1, 0]) },
  36: { svg: grid([7, 15, 23], 6, 1.2, [1, 0]) },
};

// Stage: a figure dancing — one arm up, one out, a kick.
N.stage = {
  16: { svg: box(7, 2, 2, 2) + cells(seg(8, 5, 8, 8).concat(seg(7, 5, 5, 3), seg(9, 6, 11, 8), seg(7, 9, 5, 11), [[5, 12]], seg(9, 9, 12, 12))) },
  20: { svg: '<rect class="f1" x="8" y="2" width="3" height="3" rx="1"/>' + cells(seg(9, 6, 9, 11).concat(seg(8, 6, 5, 3), seg(10, 7, 13, 10), seg(8, 12, 6, 14), [[6, 15], [6, 16]], seg(10, 12, 14, 16))) },
  24: { svg: '<circle class="f1" cx="12.5" cy="4.5" r="2"/>' + cells(seg(12, 7, 12, 13).concat(seg(11, 8, 7, 4), seg(13, 9, 16, 12), seg(11, 14, 8, 17), [[8, 18], [8, 19]], seg(13, 14, 18, 19))) },
  30: { svg: '<circle class="f1" cx="15" cy="6" r="2.8"/>' + s2r('M15 10 V18 M15 11 L10 6 M15 12 L20 16 M15 18 L11 23 L11 25 M15 18 L21 24') },
  36: { svg: '<circle class="f1" cx="18" cy="7.5" r="3.2"/>' + s2r('M18 12 V22 M18 13 L12 7 M18 14 L24 19 M18 22 L13 28 L13 30 M18 22 L25 29') },
};

// Sliders (round 39 close-out, lo-mixer: the More panel's foot, *Customise top bar…*, as
// el-topbar's sheet draws it): three tracks, each cut by one pixel round its knob, the
// knobs at three heights so the mark reads as settings to set, not a gear.
N.sliders = {
  16: { svg: s1('M2 4.5 H8 M12 4.5 H14 M2 8.5 H4 M8 8.5 H14 M2 12.5 H7 M11 12.5 H14') + box(9, 3, 2, 3) + box(5, 7, 2, 3) + box(8, 11, 2, 3) },
  20: { svg: s1('M3 5.5 H10 M14 5.5 H17 M3 10.5 H5 M9 10.5 H17 M3 15.5 H8 M12 15.5 H17') + box(11, 4, 2, 3) + box(6, 9, 2, 3) + box(9, 14, 2, 3) },
  24: { svg: s1('M4 6.5 H12 M16 6.5 H20 M4 12.5 H6 M10 12.5 H20 M4 18.5 H10 M14 18.5 H20') + box(13, 5, 2, 3) + box(7, 11, 2, 3) + box(11, 17, 2, 3) },
  30: { svg: s2r('M6 8 H12.5 M20.5 8 H24 M6 15 H8.5 M16.5 15 H24 M6 22 H14.5 M22.5 22 H24') + box(15, 5, 3, 6, 'f1', 1) + box(11, 12, 3, 6, 'f1', 1) + box(17, 19, 3, 6, 'f1', 1) },
  36: { svg: s2r('M7 10 H16.5 M25.5 10 H29 M7 18 H9.5 M18.5 18 H29 M7 26 H13.5 M22.5 26 H29') + box(19, 7, 4, 6, 'f1', 1) + box(12, 15, 4, 6, 'f1', 1) + box(16, 23, 4, 6, 'f1', 1) },
};
