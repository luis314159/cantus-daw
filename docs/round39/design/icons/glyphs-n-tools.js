// Cantus icon family, second pass — tools: snap, undo, redo, draw, select, slice.
// Line glyphs fill about 0.6 of the key; 1-px lines at keys 16–24, 2-px at 30–36.
var N = window.N || (window.N = {});

// Snap: a magnet — the U and its two pole caps, cut apart by one pixel of air.
N.snap = {
  16: { px: [P(4, 3, ['##....##', '##....##', '........', '##....##', '##....##', '##....##', '##....##', '*##..##*', '.*####*.'])] },
  20: { px: [P(6, 4, ['##....##', '##....##', '........', '##....##', '##....##', '##....##', '##....##', '##....##', '##....##', '*##..##*', '.*####*.'])] },
  24: { svg: '<rect class="f1" x="7" y="6" width="2" height="3"/><rect class="f1" x="15" y="6" width="2" height="3"/><path class="s1" stroke-width="2" d="M8 10 V13 A4 4 0 0 0 16 13 V10"/>' },
  30: { svg: '<rect class="f1" x="8" y="7" width="3" height="3"/><rect class="f1" x="19" y="7" width="3" height="3"/><path class="s1" stroke-width="3" d="M9.5 11 V15 A5.5 5.5 0 0 0 20.5 15 V11"/>' },
  36: { svg: '<rect class="f1" x="10" y="8" width="3" height="3" rx="0.5"/><rect class="f1" x="23" y="8" width="3" height="3" rx="0.5"/><path class="s1" stroke-width="3" d="M11.5 13 V18 A6.5 6.5 0 0 0 24.5 18 V13"/>' },
};

// Undo / redo: one stroke that goes back and turns under; the head is the stroke's end.
N.undo = {
  16: { svg: '<path class="s1" stroke-width="1" d="M6 6.5 H9.5 A3 3 0 0 1 9.5 12.5 H6"/>', px: [P(3, 4, ['..#', '.##', '###', '.##', '..#'])] },
  20: { v1: ['undo', 16], at: [3, 2] },
  24: { v1: ['undo', 20], at: [3, 2] },
  30: { svg: '<path class="s1" stroke-width="2" stroke-linecap="round" d="M8.5 11 H17 A5 5 0 0 1 17 21 H10"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M11.5 7.5 L8 11 L11.5 14.5"/>' },
  36: { svg: '<path class="s1" stroke-width="2" stroke-linecap="round" d="M9.5 13 H20.5 A6 6 0 0 1 20.5 25 H11.5"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M13 9 L9 13 L13 17"/>' },
};
N.redo = {
  16: { svg: '<path class="s1" stroke-width="1" d="M10 6.5 H6.5 A3 3 0 0 0 6.5 12.5 H10"/>', px: [P(10, 4, ['#..', '##.', '###', '##.', '#..'])] },
  20: { v1: ['redo', 16], at: [2, 2] },
  24: { v1: ['redo', 20], at: [2, 2] },
  30: { svg: '<path class="s1" stroke-width="2" stroke-linecap="round" d="M21.5 11 H13 A5 5 0 0 0 13 21 H20"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M18.5 7.5 L22 11 L18.5 14.5"/>' },
  36: { svg: '<path class="s1" stroke-width="2" stroke-linecap="round" d="M26.5 13 H15.5 A6 6 0 0 0 15.5 25 H24.5"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M23 9 L27 13 L23 17"/>' },
};

// Draw: a hollow pencil at 45 degrees, its wood in the second ink and the lead in the main.
N.draw = {
  16: { px: [P(4, 4, ['......#..', '.....#.#.', '....#...#', '...#...#.', '..#...#..', '..O#.#...', '..OO#....', '+#.......', '.+.......'])] },
  20: { v1: ['draw', 16], at: [2, 3] },
  24: { v1: ['draw', 20], at: [2, 3] },
  30: { v1: ['draw', 24], at: [3, 3] },
  36: { v1: ['draw', 30], at: [2, 2] },
};

// Select: the pointer alone (the first pass added a marquee — a diagram).
const kite = (n, tail) => {
  const rows = [];
  for (let i = 1; i <= n; i++) rows.push('#'.repeat(i));
  tail.forEach(w => rows.push('#'.repeat(w)));
  return rows;
};
N.select = {
  16: { px: [P(5, 4, kite(6, [2, 1]))] },
  20: { px: [P(7, 5, kite(7, [3, 2, 1]))] },
  24: { px: [P(8, 6, kite(8, [3, 2, 1]))] },
  30: { svg: '<path class="f1" stroke="var(--main)" stroke-width="1" stroke-linejoin="round" d="M10.5 8.5 V21.5 L13.7 18.3 H19.5 Z"/>' },
  36: { svg: '<path class="f1" stroke="var(--main)" stroke-width="1" stroke-linejoin="round" d="M12.5 9.5 V25.5 L16.4 21.6 H23.5 Z"/>' },
};

// Slice: scissors — two rings and two blades crossing. At 16–24 px the blades are clean
// 45-degree pixel staircases.
const diag = (x0, y0, n, dy) => Array.from({ length: n }, (_, i) => `<rect x="${x0 + i}" y="${y0 + i * dy}" width="1" height="1"/>`).join('');
N.slice = {
  16: { svg: `<g class="f1" shape-rendering="crispEdges">${diag(6, 6, 7, 1)}${diag(6, 9, 7, -1)}</g>`,
    px: [P(3, 3, ['+#+', '#.#', '+#+']), P(3, 10, ['+#+', '#.#', '+#+'])] },
  20: { svg: `<g class="f1" shape-rendering="crispEdges">${diag(7, 7, 9, 1)}${diag(7, 12, 9, -1)}</g>`,
    px: [P(4, 4, ['.##.', '#..#', '#..#', '.##.']), P(4, 12, ['.##.', '#..#', '#..#', '.##.'])] },
  24: { svg: `<g class="f1" shape-rendering="crispEdges">${diag(9, 9, 10, 1)}${diag(9, 14, 10, -1)}</g>`,
    px: [P(5, 5, ['.###.', '#...#', '#...#', '#...#', '.###.']), P(5, 14, ['.###.', '#...#', '#...#', '#...#', '.###.'])] },
  30: { svg: '<circle class="s1" stroke-width="2" cx="10" cy="10" r="3"/><circle class="s1" stroke-width="2" cx="10" cy="20" r="3"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M12.4 12.2 L24 21.5 M12.4 17.8 L24 8.5"/>' },
  36: { svg: '<circle class="s1" stroke-width="2" cx="11.5" cy="12" r="3.5"/><circle class="s1" stroke-width="2" cx="11.5" cy="24" r="3.5"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M14.3 14.2 L28.5 25.5 M14.3 21.8 L28.5 10.5"/>' },
};
