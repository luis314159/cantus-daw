// Cantus icon family, second pass — metronome, count-in, follow.
var N = window.N || (window.N = {});

// Metronome: a solid tapered body; the rod is cut out of it (the key's well shows through)
// and rises past the top in ink. One silhouette, no outline to turn to hatching.
N.metronome = {
  16: { px: [P(3, 3, [
    '........#.',
    '........#.',
    '...###.#..',
    '...###.#..',
    '..####.#..',
    '..####.#..',
    '.####.###.',
    '.####.###.',
    '.###.####.',
    '.###.####.',
    '##########'])] },
  20: { px: [P(4, 4, [
    '..........#.',
    '..........#.',
    '....####.#..',
    '...#####.#..',
    '...#####....',
    '...#####....',
    '..#####.##..',
    '..#####.##..',
    '..####.###..',
    '.#####.####.',
    '.####.#####.',
    '.####.#####.',
    '############'])] },
  24: { svg: '<path class="f1" d="M10 5 H14 L17.5 19 H6.5 Z"/><path class="scut" stroke-width="1.5" d="M12 17 L14.9 8.6"/><path class="s1" stroke-width="1.5" d="M14.9 8.6 L16.6 3.5"/>' },
  30: { svg: '<path class="f1" stroke="var(--main)" stroke-width="1" stroke-linejoin="round" d="M13 7.5 H17 L21.5 23.5 H8.5 Z"/><path class="scut" stroke-width="2" d="M15 22 L18.7 11.5"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M18.7 11.5 L21 5"/>' },
  36: { svg: '<path class="f1" stroke="var(--main)" stroke-width="1" stroke-linejoin="round" d="M15.5 8.5 H20.5 L25.5 27.5 H10.5 Z"/><path class="scut" stroke-width="2.2" d="M18 26 L22.4 13.5"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M22.4 13.5 L25 6"/>' },
};

// Count-in: the clicks before (second ink), then play (main ink).
N.countIn = {
  16: { svg: '<rect class="f2" x="3" y="7" width="2" height="2"/><rect class="f2" x="6" y="7" width="2" height="2"/><path class="f1" d="M9 5 L13 8 L9 11 Z"/>' },
  20: { svg: '<rect class="f2" x="4" y="9" width="2" height="2"/><rect class="f2" x="7" y="9" width="2" height="2"/><rect class="f2" x="10" y="9" width="2" height="2"/><path class="f1" d="M13 7 L17 10 L13 13 Z"/>' },
  24: { svg: '<rect class="f2" x="5" y="11" width="2" height="2"/><rect class="f2" x="8" y="11" width="2" height="2"/><rect class="f2" x="11" y="11" width="2" height="2"/><path class="f1" d="M14 8 L19 12 L14 16 Z"/>' },
  30: { svg: '<circle class="f2" cx="8" cy="15" r="1.4"/><circle class="f2" cx="11.5" cy="15" r="1.4"/><circle class="f2" cx="15" cy="15" r="1.4"/><path class="f1" d="M18 10 L24 15 L18 20 Z"/>' },
  36: { svg: '<circle class="f2" cx="9" cy="18" r="1.6"/><circle class="f2" cx="13.5" cy="18" r="1.6"/><circle class="f2" cx="18" cy="18" r="1.6"/><path class="f1" d="M22 12 L29 18 L22 24 Z"/>' },
};

// Follow: the playhead, and the view going on with it.
N.follow = {
  16: { svg: '<rect class="f1" x="5" y="6" width="1" height="7"/><rect class="f1" x="8" y="8" width="3" height="1"/>',
    px: [P(3, 3, ['#####', '.###.', '..#..']), P(11, 7, ['#.', '##', '#.'])] },
  20: { svg: '<rect class="f1" x="6" y="7" width="1" height="9"/><rect class="f1" x="9" y="10" width="4" height="1"/>',
    px: [P(4, 4, ['#####', '.###.', '..#..']), P(13, 8, ['#..', '##.', '###', '##.', '#..'])] },
  24: { svg: '<rect class="f1" x="7" y="7" width="1" height="12"/><rect class="f1" x="11" y="12" width="5" height="1"/>',
    px: [P(5, 5, ['#####', '.###.', '..#..']), P(16, 10, ['#..', '##.', '###', '##.', '#..'])] },
  30: { svg: '<path class="f1" d="M6.5 7 H13.5 L10 11.5 Z"/><path class="s1" stroke-width="2" d="M10 10 V24"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M14 15 H22"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M19.5 12.5 L22.5 15 L19.5 17.5"/>' },
  36: { svg: '<path class="f1" d="M8 8 H16 L12 13 Z"/><path class="s1" stroke-width="2" d="M12 12 V28"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M17 18 H26"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M23 15 L26.5 18 L23 21"/>' },
};
