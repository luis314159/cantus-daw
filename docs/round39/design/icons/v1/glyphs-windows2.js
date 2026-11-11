// Cantus icon family — the window keys, part 2: mixer, browser, plug-ins, inspector.
var G = typeof G === 'undefined' ? {} : G;

// Mixer: three fader slots (ink-2) and their caps (ink-1) at three levels; from 24 px each
// cap carries its grip line (a pixel of the key's well).
G.mixer = {
  16: { c: 16, px: [E16, E16,
    '...O...O...O....',
    '...O...O...O....',
    '...O..###..O....',
    '...O..###..O....',
    '...O...O...O....',
    '...O...O...O....',
    '..###..O...O....',
    '..###..O...O....',
    '...O...O..###...',
    '...O...O..###...',
    '...O...O...O....',
    '...O...O...O....', E16, E16] },
  20: { c: 20, px: [E20, E20, E20,
    '....O....O....O.....',
    '....O....O....O.....',
    '....O...###...O.....',
    '....O...###...O.....',
    '....O...###...O.....',
    '....O....O....O.....',
    '....O....O....O.....',
    '...###...O....O.....',
    '...###...O....O.....',
    '...###...O...###....',
    '....O....O...###....',
    '....O....O...###....',
    '....O....O....O.....',
    '....O....O....O.....', E20, E20, E20] },
  24: { c: 24, svg: mixerSvg([6, 12, 18], 2, 3, 21, 4, 5, [12, 6, 14], 0.75) },
  30: { c: 30, svg: mixerSvg([8.5, 15.5, 22.5], 1, 4, 27, 5, 6, [15, 8, 18], 1) },
  36: { c: 36, svg: mixerSvg([9, 18, 27], 2, 5, 31, 6, 7, [18, 9, 21], 1) },
};

// Mixer per size: slot centres, slot width, top and bottom, cap width and height, cap tops.
function mixerSvg(xs, sw, top, bottom, cw, ch, caps, rx) {
  let s = '';
  xs.forEach(x => {
    s += `<rect class="f2" x="${x - sw / 2}" y="${top}" width="${sw}" height="${bottom - top}"/>`;
  });
  xs.forEach((x, i) => {
    const y = caps[i];
    s += `<rect class="f1" x="${x - cw / 2}" y="${y}" width="${cw}" height="${ch}" rx="${rx}"/>`;
    s += `<rect x="${x - cw / 2 + 1}" y="${y + Math.floor(ch / 2)}" width="${cw - 2}" height="1" fill="var(--well)"/>`;
  });
  return s;
}

// Browser: a folder (ink-1) and the files under it on a tree (ink-2 lines, ink-1 files).
G.browser = {
  16: { c: 16, at: [1, 1], px: [E16,
    '..###...........',
    '..#######.......',
    '..#######.......',
    '..#######.......',
    '...O............',
    '...O............',
    '...O...######...',
    '...OOO.######...',
    '...O............',
    '...O...######...',
    '...OOO.######...', E16, E16, E16, E16] },
  20: { c: 20, px: [E20, E20, E20,
    '...####.............',
    '...#########........',
    '...#########........',
    '...#########........',
    '...#########........',
    '.....O..............',
    '.....O..............',
    '.....O...########...',
    '.....OOO.########...',
    '.....O..............',
    '.....O..............',
    '.....O...########...',
    '.....OOO.########...', E20, E20, E20, E20] },
  24: { c: 24, svg: '<path class="f1" d="M3 3 H8 L9 5 H14 V11 H3 Z"/><rect class="f2" x="5" y="12" width="1" height="8"/><rect class="f2" x="5" y="14" width="4" height="1"/><rect class="f2" x="5" y="19" width="4" height="1"/><rect class="f1" x="10" y="13" width="11" height="3" rx="1"/><rect class="f1" x="10" y="18" width="11" height="3" rx="1"/>' },
  30: { c: 30, svg: '<path class="f1" d="M4 4 H10.5 L12 6.5 H18 V14 H4 Z"/><rect class="f2" x="6" y="15" width="1" height="11"/><rect class="f2" x="6" y="18" width="5" height="1"/><rect class="f2" x="6" y="25" width="5" height="1"/><rect class="f1" x="13" y="17" width="13" height="3" rx="1"/><rect class="f1" x="13" y="24" width="13" height="3" rx="1"/>' },
  36: { c: 36, svg: '<path class="f1" d="M5 5 H12.5 L14.5 8 H22 V17 H5 Z"/><rect class="f2" x="7" y="18" width="2" height="13"/><rect class="f2" x="7" y="21" width="7" height="2"/><rect class="f2" x="7" y="29" width="7" height="2"/><rect class="f1" x="16" y="20" width="15" height="4" rx="1.5"/><rect class="f1" x="16" y="28" width="15" height="4" rx="1.5"/>' },
};

// Plug-ins: a plug — prongs (ink-2), body and cable (ink-1).
G.plugins = {
  16: { c: 16, px: [E16, E16,
    '.....OO..OO.....',
    '.....OO..OO.....',
    '.....OO..OO.....',
    '...##########...',
    '...##########...',
    '....########....',
    '.....######.....',
    '......####......',
    '.......##.......',
    '.......##.......',
    '.......##.......',
    '.......##.......', E16, E16] },
  20: { c: 20, px: [E20, E20, E20,
    '......OO....OO......',
    '......OO....OO......',
    '......OO....OO......',
    '......OO....OO......',
    '....############....',
    '....############....',
    '.....##########.....',
    '......########......',
    '.......######.......',
    '........####........',
    '.........##.........',
    '.........##.........',
    '.........##.........',
    '.........##.........', E20, E20, E20] },
  24: { c: 24, svg: '<rect class="f2" x="8" y="3" width="2" height="5"/><rect class="f2" x="14" y="3" width="2" height="5"/><path class="f1" d="M5 8 H19 V10 C19 14 16 16.5 12 16.5 C8 16.5 5 14 5 10 Z"/><rect class="f1" x="11" y="16" width="2" height="6"/>' },
  30: { c: 30, svg: '<rect class="f2" x="10" y="4" width="3" height="6"/><rect class="f2" x="18" y="4" width="3" height="6"/><path class="f1" d="M6.5 10 H24.5 V12.5 C24.5 17.5 20.5 20.5 15.5 20.5 C10.5 20.5 6.5 17.5 6.5 12.5 Z"/><rect class="f1" x="14" y="20" width="3" height="7"/>' },
  36: { c: 36, svg: '<rect class="f2" x="12" y="5" width="3" height="7"/><rect class="f2" x="21" y="5" width="3" height="7"/><path class="f1" d="M8 12 H28 V15 C28 21 23.5 24.5 18 24.5 C12.5 24.5 8 21 8 15 Z"/><rect class="f1" x="16" y="24" width="4" height="8"/>' },
};

// Inspector: a frame (ink-2) with its right-hand panel open (ink-1 rows).
G.inspector = {
  16: { c: 16, px: [E16, E16, E16,
    '..OOOOOOOOOOOO..',
    '..O.....####.O..',
    '..O.....####.O..',
    '..O..........O..',
    '..O.....####.O..',
    '..O.....####.O..',
    '..O..........O..',
    '..O.....####.O..',
    '..O.....####.O..',
    '..OOOOOOOOOOOO..', E16, E16, E16] },
  20: { c: 20, px: [E20, E20, E20, E20,
    '...OOOOOOOOOOOOOO...',
    '...O............O...',
    '...O.......####.O...',
    '...O.......####.O...',
    '...O............O...',
    '...O.......####.O...',
    '...O.......####.O...',
    '...O............O...',
    '...O.......####.O...',
    '...O.......####.O...',
    '...O............O...',
    '...OOOOOOOOOOOOOO...', E20, E20, E20, E20] },
  24: { c: 24, svg: '<rect class="s2" stroke-width="1" x="3.5" y="4.5" width="17" height="15" rx="1.5"/><rect class="f1" x="14" y="7" width="5" height="2" rx="0.5"/><rect class="f1" x="14" y="11" width="5" height="2" rx="0.5"/><rect class="f1" x="14" y="15" width="5" height="2" rx="0.5"/>' },
  30: { c: 30, svg: '<rect class="s2" stroke-width="1" x="4.5" y="5.5" width="22" height="20" rx="2"/><rect class="f1" x="18" y="9" width="6" height="3" rx="0.75"/><rect class="f1" x="18" y="14" width="6" height="3" rx="0.75"/><rect class="f1" x="18" y="19" width="6" height="3" rx="0.75"/>' },
  36: { c: 36, svg: '<rect class="s2" stroke-width="2" x="6" y="7" width="24" height="22" rx="2.5"/><rect class="f1" x="21" y="11" width="7" height="3" rx="1"/><rect class="f1" x="21" y="17" width="7" height="3" rx="1"/><rect class="f1" x="21" y="23" width="7" height="3" rx="1"/>' },
};
