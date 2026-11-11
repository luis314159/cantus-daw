// Cantus icon family — tools, part 3: zoom, eyedropper, settings.
var G = typeof G === 'undefined' ? {} : G;

// Zoom: a lens (ink-1 ring, an ink-2 glint inside) and its handle.
G.zoom = {
  16: { c: 16, px: [E16, E16,
    '.....###........',
    '...##...##......',
    '...#.o...#......',
    '..#.o.....#.....',
    '..#.......#.....',
    '..#.......#.....',
    '...#.....#......',
    '...##...##......',
    '.....###.##.....',
    '..........##....',
    '...........##...',
    '............##..', E16, E16] },
  20: { c: 20, px: [E20, E20, E20,
    '.......###..........',
    '.....##...##........',
    '....#.......#.......',
    '....#.o.....#.......',
    '...#.o.......#......',
    '...#.........#......',
    '...#.........#......',
    '....#.......#.......',
    '....#.......#.......',
    '.....##...##........',
    '.......###..##......',
    '.............##.....',
    '..............##....',
    '...............##...', E20, E20, E20] },
  24: { c: 24, svg: '<circle class="s1" stroke-width="2" cx="10" cy="10" r="6"/><path class="s2" stroke-width="1" d="M6.5 10 A3.5 3.5 0 0 1 10 6.5"/><path class="s1" stroke-width="3" d="M14.5 14.5 L20 20"/>' },
  30: { c: 30, svg: '<circle class="s1" stroke-width="2" cx="12.5" cy="12.5" r="7.5"/><path class="s2" stroke-width="1.5" d="M8 12.5 A4.5 4.5 0 0 1 12.5 8"/><path class="s1" stroke-width="3.5" d="M18 18 L25 25"/>' },
  36: { c: 36, svg: '<circle class="s1" stroke-width="3" cx="15" cy="15" r="9"/><path class="s2" stroke-width="2" d="M9.5 15 A5.5 5.5 0 0 1 15 9.5"/><path class="s1" stroke-width="4.5" d="M21.5 21.5 L30 30"/>' },
};

// Eyedropper: the bulb and collar (ink-1), the glass tube (ink-2), the drop at its tip (ink-1).
G.pick = {
  16: { c: 16, px: [E16, E16,
    '...........##...',
    '..........####..',
    '..........####..',
    '........#..##...',
    '.........#......',
    '.......OO.#.....',
    '......OO........',
    '.....OO.........',
    '....OO..........',
    '...OO...........',
    '..#*............', E16, E16, E16] },
  20: { c: 20, px: [E20, E20, E20,
    '..............##....',
    '.............####...',
    '.............####...',
    '..........#...##....',
    '...........#........',
    '............#.......',
    '.........OOO.#......',
    '........OOO.........',
    '.......OOO..........',
    '......OOO...........',
    '.....OOO............',
    '....OOO.............',
    '...#*...............', E20, E20, E20, E20] },
  24: { c: 24, svg: '<g transform="rotate(-45 12 12)"><rect class="f1" x="15.5" y="9" width="6" height="6" rx="2.5"/><rect class="f1" x="13" y="8" width="1.5" height="8"/><rect class="f2" x="5" y="10.75" width="7" height="2.5"/><path class="f1" d="M2.5 12 L5 10.75 V13.25 Z"/></g>' },
  30: { c: 30, svg: '<g transform="rotate(-45 15.5 15.5)"><rect class="f1" x="19.5" y="12" width="7.5" height="7" rx="3"/><rect class="f1" x="16.5" y="10.5" width="2" height="10"/><rect class="f2" x="6.5" y="14" width="9" height="3"/><path class="f1" d="M3 15.5 L6.5 14 V17 Z"/></g>' },
  36: { c: 36, svg: '<g transform="rotate(-45 18 18)"><rect class="f1" x="23.5" y="14" width="9" height="8" rx="3.5"/><rect class="f1" x="19.5" y="12.5" width="2.5" height="11"/><rect class="f2" x="7.5" y="16.25" width="11" height="3.5"/><path class="f1" d="M3.5 18 L7.5 16.25 V19.75 Z"/></g>' },
};

// Settings: a gear with eight teeth, drawn as pixels at 16 and 20 px (where a vector gear
// turns to mush) and with an ink-2 hub from 24 px up.
G.gear = {
  16: { c: 16, at: [2, 2], px: [
    '.....##.....',
    '..#..##..#..',
    '.##########.',
    '..########..',
    '..##+..+##..',
    '####....####',
    '####....####',
    '..##+..+##..',
    '..########..',
    '.##########.',
    '..#..##..#..',
    '.....##.....'] },
  20: { c: 20, at: [3, 3], px: [
    '.....####.....',
    '.....####.....',
    '..##.####.##..',
    '..##########..',
    '...###..###...',
    '#####....#####',
    '####......####',
    '####......####',
    '#####....#####',
    '...###..###...',
    '..##########..',
    '..##.####.##..',
    '.....####.....',
    '.....####.....'] },
  24: { c: 24, svg: gearSvg(12, 5, 3, 3, 3, 6.5, 1.5) },
  30: { c: 30, svg: gearSvg(15.5, 6.25, 3.5, 4, 3.5, 8, 2) },
  36: { c: 36, svg: gearSvg(18, 7.5, 4, 4.5, 4.5, 9.25, 2.5) },
};

// The gear's vector sizes, each with its own numbers: centre, ring radius and weight, tooth
// width and length, where the teeth start, the hub's radius.
function gearSvg(c, r, w, tw, tl, t0, hub) {
  let s = `<circle class="s1" stroke-width="${w}" cx="${c}" cy="${c}" r="${r}"/>`;
  for (let k = 0; k < 8; k++) {
    s += `<rect class="f1" x="${c - tw / 2}" y="${c - t0 - tl}" width="${tw}" height="${tl + 0.5}" transform="rotate(${k * 45} ${c} ${c})"/>`;
  }
  s += `<circle class="f2" cx="${c}" cy="${c}" r="${hub}"/>`;
  return s;
}
