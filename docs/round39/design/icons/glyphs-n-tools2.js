// Cantus icon family, second pass — tools: erase, mute, zoom, eyedropper, settings.
// The 16-px key gets new drawings; the larger keys take the first pass's drawings that were
// made for exactly their pixel size (see family.js), centred on the new key.
var N = window.N || (window.N = {});

N.erase = {
  16: { px: [P(4, 4, ['.....O...', '....O.O..', '...O...O.', '..##....O', '.####..O.', '######O..', '.#####...', '..###....', '...#.....'])] },
  20: { v1: ['erase', 16], at: [2, 2] },
  24: { v1: ['erase', 20], at: [2, 2] },
  30: { v1: ['erase', 24], at: [3, 3] },
  36: { v1: ['erase', 30], at: [2, 2] },
};

// Mute (the tool): a speaker — body and cone cut apart by one pixel — and a cross.
N.muteTool = {
  16: { svg: '<rect class="f1" x="3" y="7" width="2" height="3"/><rect class="f1" x="6" y="6" width="1" height="5"/><rect class="f1" x="7" y="5" width="1" height="7"/>',
    px: [P(9, 6, ['O...O', '.O.O.', '..O..', '.O.O.', 'O...O'])] },
  20: { v1: ['muteTool', 16], at: [2, 3] },
  24: { v1: ['muteTool', 20], at: [2, 2] },
  30: { v1: ['muteTool', 24], at: [3, 3] },
  36: { v1: ['muteTool', 30], at: [2, 2] },
};

N.zoom = {
  16: { px: [P(3, 3, ['..###..', '.#...#.', '#.....#', '#.....#', '#.....#', '.#...#.', '..###..']), P(9, 9, ['##..', '.##.', '..##', '...#'])] },
  20: { v1: ['zoom', 16], at: [2, 2] },
  24: { v1: ['zoom', 20], at: [2, 2] },
  30: { v1: ['zoom', 24], at: [3, 3] },
  36: { v1: ['zoom', 30], at: [2, 2] },
};

N.pick = {
  16: { px: [P(4, 3, ['.......+#+', '.......###', '.......+#+', '.....#....', '......#...', '.....O.#..', '....O.....', '...O......', '..O.......', '.#........', '*.........'])] },
  20: { v1: ['pick', 16], at: [2, 3] },
  24: { v1: ['pick', 20], at: [2, 3] },
  30: { v1: ['pick', 24], at: [3, 3] },
  36: { v1: ['pick', 30], at: [2, 2] },
};

// Settings: six teeth at 16 px (eight do not fit in ten pixels), eight from 20 px up.
N.gear = {
  16: { px: [P(3, 3, ['....##....', '...####...', '.########.', '.###..###.', '###....###', '###....###', '.###..###.', '.########.', '...####...', '....##....'])] },
  20: { v1: ['gear', 16], at: [2, 2] },
  24: { v1: ['gear', 20], at: [2, 2] },
  30: { v1: ['gear', 24], at: [3, 3] },
  36: { v1: ['gear', 30], at: [2, 2] },
};
