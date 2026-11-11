// Cantus icon family — the window keys. Each is a small diagram of its window: the content
// it holds in ink-1 (clips, notes, pads, fader caps, files, the open panel), the structure
// around it in ink-2 (keys, slots, tree lines, frames). No window key is a box in a box.
var G = typeof G === 'undefined' ? {} : G;

// Arrangement: three lanes of clips, staggered.
G.arrangement = {
  16: { c: 16, px: [E16, E16, E16,
    '..#####..#####..',
    '..#####..#####..',
    E16, E16,
    '....#######.....',
    '....#######.....',
    E16, E16,
    '..###..#######..',
    '..###..#######..', E16, E16, E16] },
  20: { c: 20, px: [E20, E20, E20, E20,
    '...######..######...',
    '...######..######...',
    '...######..######...',
    E20, E20,
    '.....#########......',
    '.....#########......',
    '.....#########......',
    E20, E20,
    '...####..#########..',
    '...####..#########..',
    '...####..#########..', E20, E20, E20] },
  24: { c: 24, svg: '<rect class="f1" x="3" y="4" width="7" height="4" rx="1"/><rect class="f1" x="12" y="4" width="9" height="4" rx="1"/><rect class="f1" x="6" y="10" width="11" height="4" rx="1"/><rect class="f1" x="3" y="16" width="4" height="4" rx="1"/><rect class="f1" x="9" y="16" width="12" height="4" rx="1"/>' },
  30: { c: 30, svg: '<rect class="f1" x="4" y="5" width="8" height="5" rx="1"/><rect class="f1" x="14" y="5" width="12" height="5" rx="1"/><rect class="f1" x="7" y="13" width="14" height="5" rx="1"/><rect class="f1" x="4" y="21" width="5" height="5" rx="1"/><rect class="f1" x="11" y="21" width="15" height="5" rx="1"/>' },
  36: { c: 36, svg: '<rect class="f1" x="5" y="6" width="10" height="6" rx="1.5"/><rect class="f1" x="17" y="6" width="14" height="6" rx="1.5"/><rect class="f1" x="9" y="15" width="17" height="6" rx="1.5"/><rect class="f1" x="5" y="24" width="6" height="6" rx="1.5"/><rect class="f1" x="13" y="24" width="18" height="6" rx="1.5"/>' },
};

// Piano roll: the keys on the left (ink-2), a rising phrase of notes (ink-1) on their rows.
G.pianoRoll = {
  16: { c: 16, px: ['................',
    '................',
    '................',
    '..OO.....#####..',
    '..OO.....#####..',
    '................',
    '................',
    '..OO...#####....',
    '..OO...#####....',
    '................',
    '................',
    '..OO.#####......',
    '..OO.#####......',
    '................',
    '................',
    '................'] },
  20: { c: 20, px: ['....................',
    '....................',
    '....................',
    '....................',
    '...OOO.....######...',
    '...OOO.....######...',
    '...OOO.....######...',
    '....................',
    '....................',
    '...OOO...######.....',
    '...OOO...######.....',
    '...OOO...######.....',
    '....................',
    '....................',
    '...OOO.######.......',
    '...OOO.######.......',
    '...OOO.######.......',
    '....................',
    '....................',
    '....................'] },
  24: { c: 24, svg: '<rect class="f2" x="3" y="4" width="4" height="4" rx="0.5"/><rect class="f2" x="3" y="10" width="4" height="4" rx="0.5"/><rect class="f2" x="3" y="16" width="4" height="4" rx="0.5"/><rect class="f1" x="9" y="16" width="8" height="4" rx="1"/><rect class="f1" x="11" y="10" width="8" height="4" rx="1"/><rect class="f1" x="13" y="4" width="8" height="4" rx="1"/>' },
  30: { c: 30, svg: '<rect class="f2" x="4" y="6" width="5" height="5" rx="0.75"/><rect class="f2" x="4" y="13" width="5" height="5" rx="0.75"/><rect class="f2" x="4" y="20" width="5" height="5" rx="0.75"/><rect class="f1" x="11" y="20" width="10" height="5" rx="1"/><rect class="f1" x="14" y="13" width="9" height="5" rx="1"/><rect class="f1" x="17" y="6" width="9" height="5" rx="1"/>' },
  36: { c: 36, svg: '<rect class="f2" x="5" y="6" width="6" height="6" rx="1"/><rect class="f2" x="5" y="15" width="6" height="6" rx="1"/><rect class="f2" x="5" y="24" width="6" height="6" rx="1"/><rect class="f1" x="13" y="24" width="12" height="6" rx="1.5"/><rect class="f1" x="16" y="15" width="12" height="6" rx="1.5"/><rect class="f1" x="19" y="6" width="12" height="6" rx="1.5"/>' },
};

// Channel rack: rows — a name (ink-2) and steps, lit (ink-1) or not (ink-2 at half).
G.rack = {
  16: { c: 16, px: [E16, E16, E16,
    '..OOO.##.oo.##..',
    '..OOO.##.oo.##..',
    E16, E16,
    '..OOO.oo.##.oo..',
    '..OOO.oo.##.oo..',
    E16, E16,
    '..OOO.##.##.##..',
    '..OOO.##.##.##..', E16, E16, E16] },
  20: { c: 20, px: [E20, E20, E20, E20,
    '...OOO.###.ooo.###..',
    '...OOO.###.ooo.###..',
    '...OOO.###.ooo.###..',
    E20, E20,
    '...OOO.ooo.###.ooo..',
    '...OOO.ooo.###.ooo..',
    '...OOO.ooo.###.ooo..',
    E20, E20,
    '...OOO.###.###.###..',
    '...OOO.###.###.###..',
    '...OOO.###.###.###..', E20, E20, E20] },
  24: { c: 24, svg: rackSvg([3, 5], [10, 14, 18], 3, [3, 10, 17], 4, ['1o1', 'o1o', '111'], 0.75) },
  30: { c: 30, svg: rackSvg([4, 6], [12, 17, 22], 4, [5, 13, 21], 5, ['1o1', 'o1o', '111'], 1) },
  36: { c: 36, svg: rackSvg([5, 7], [15, 21, 27], 4, [6, 15, 24], 6, ['1o1', 'o1o', '111'], 1) },
};

// The rack's vector sizes: name x and width, step x positions and width, row y positions and
// height, the pattern ('1' lit, 'o' unlit), the corner radius — numbers chosen per size.
function rackSvg(name, steps, sw, rows, h, pattern, rx) {
  let s = '';
  rows.forEach((y, r) => {
    s += `<rect class="f2" x="${name[0]}" y="${y}" width="${name[1]}" height="${h}" rx="${rx}"/>`;
    steps.forEach((x, i) => {
      const lit = pattern[r][i] === '1';
      s += `<rect class="${lit ? 'f1' : 'f2'}" x="${x}" y="${y}" width="${sw}" height="${h}" rx="${rx}"${lit ? '' : ' fill-opacity="0.55"'}/>`;
    });
  });
  return s;
}
