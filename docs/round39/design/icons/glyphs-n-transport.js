// Cantus icon family, second pass — the transport, drawn per key height (16/20/24/30/36).
// Filled primitives, about 0.4 of the key: play 6/8/10/12/14 tall, stop a little smaller
// (it weighs more), record a little larger (a circle weighs less). The triangle's back edge
// sits on a whole pixel and its centre of mass on the key's centre.
var N = window.N || (window.N = {});
const P = (x, y, rows) => ({ at: [x, y], rows });

N.play = {
  16: { svg: '<path class="f1" d="M6 5 L11 8 L6 11 Z"/>' },
  20: { svg: '<path class="f1" d="M8 6 L15 10 L8 14 Z"/>' },
  24: { svg: '<path class="f1" d="M9 7 L18 12 L9 17 Z"/>' },
  30: { svg: '<path class="f1" d="M12 9 L22 15 L12 21 Z"/>' },
  36: { svg: '<path class="f1" stroke="var(--main)" stroke-width="1" stroke-linejoin="round" d="M14.5 11.8 L25.2 18 L14.5 24.2 Z"/>' },
};

N.pause = {
  16: { svg: '<rect class="f1" x="5" y="5" width="2" height="6"/><rect class="f1" x="9" y="5" width="2" height="6"/>' },
  20: { svg: '<rect class="f1" x="6" y="6" width="3" height="8"/><rect class="f1" x="11" y="6" width="3" height="8"/>' },
  24: { svg: '<rect class="f1" x="8" y="7" width="3" height="10"/><rect class="f1" x="13" y="7" width="3" height="10"/>' },
  30: { svg: '<rect class="f1" x="10" y="9" width="3" height="12" rx="0.5"/><rect class="f1" x="17" y="9" width="3" height="12" rx="0.5"/>' },
  36: { svg: '<rect class="f1" x="12" y="11" width="4" height="14" rx="1"/><rect class="f1" x="20" y="11" width="4" height="14" rx="1"/>' },
};

N.stop = {
  16: { svg: '<rect class="f1" x="5" y="5" width="5" height="5"/>' },
  20: { svg: '<rect class="f1" x="7" y="7" width="6" height="6"/>' },
  24: { svg: '<rect class="f1" x="8" y="8" width="8" height="8" rx="0.5"/>' },
  30: { svg: '<rect class="f1" x="10" y="10" width="10" height="10" rx="1"/>' },
  36: { svg: '<rect class="f1" x="12" y="12" width="12" height="12" rx="1.5"/>' },
};

N.record = {
  16: { svg: '<circle class="fr" cx="8" cy="8" r="3.5"/>' },
  20: { svg: '<circle class="fr" cx="10" cy="10" r="4.5"/>' },
  24: { svg: '<circle class="fr" cx="12" cy="12" r="5.5"/>' },
  30: { svg: '<circle class="fr" cx="15" cy="15" r="6.5"/>' },
  36: { svg: '<circle class="fr" cx="18" cy="18" r="8"/>' },
};

// Return to start: the start line and the arrow, one ink (the first pass put the line in
// the second ink, and it read as disabled).
N.toStart = {
  16: { svg: '<rect class="f1" x="4" y="5" width="1" height="6"/><path class="f1" d="M11 5 L6 8 L11 11 Z"/>' },
  20: { svg: '<rect class="f1" x="5" y="6" width="2" height="8"/><path class="f1" d="M15 6 L8 10 L15 14 Z"/>' },
  24: { svg: '<rect class="f1" x="6" y="7" width="2" height="10"/><path class="f1" d="M18 7 L10 12 L18 17 Z"/>' },
  30: { svg: '<rect class="f1" x="8" y="9" width="2" height="12"/><path class="f1" d="M22 9 L12 15 L22 21 Z"/>' },
  36: { svg: '<rect class="f1" x="9" y="11" width="3" height="14" rx="0.5"/><path class="f1" d="M27 11 L14 18 L27 25 Z"/>' },
};

N.skipFwd = {
  16: { svg: '<path class="f1" d="M4 5 L8 8 L4 11 Z M8 5 L12 8 L8 11 Z"/>' },
  20: { svg: '<path class="f1" d="M5 6 L10 10 L5 14 Z M10 6 L15 10 L10 14 Z"/>' },
  24: { svg: '<path class="f1" d="M6 7 L12 12 L6 17 Z M12 7 L18 12 L12 17 Z"/>' },
  30: { svg: '<path class="f1" d="M8 9 L15 15 L8 21 Z M15 9 L22 15 L15 21 Z"/>' },
  36: { svg: '<path class="f1" d="M10 11 L18 18 L10 25 Z M18 11 L26 18 L18 25 Z"/>' },
};

N.skipBack = {
  16: { svg: '<path class="f1" d="M12 5 L8 8 L12 11 Z M8 5 L4 8 L8 11 Z"/>' },
  20: { svg: '<path class="f1" d="M15 6 L10 10 L15 14 Z M10 6 L5 10 L10 14 Z"/>' },
  24: { svg: '<path class="f1" d="M18 7 L12 12 L18 17 Z M12 7 L6 12 L12 17 Z"/>' },
  30: { svg: '<path class="f1" d="M22 9 L15 15 L22 21 Z M15 9 L8 15 L15 21 Z"/>' },
  36: { svg: '<path class="f1" d="M26 11 L18 18 L26 25 Z M18 11 L10 18 L18 25 Z"/>' },
};

// Loop: two opposed arrows on one stadium — rotationally symmetric, so it rests in its key.
// The heads grow out of the stroke: pixel heads where the stroke is 1 px, chevrons of the
// stroke's own weight where it is 2 px.
N.loop = {
  16: { svg: '<path class="s1" stroke-width="1" d="M3.5 9 V8 A2.5 2.5 0 0 1 6 5.5 H10"/><path class="s1" stroke-width="1" d="M12.5 7 V8 A2.5 2.5 0 0 1 10 10.5 H6"/>',
    px: [P(10, 4, ['#.', '##', '#.']), P(4, 9, ['.#', '##', '.#'])] },
  20: { svg: '<path class="s1" stroke-width="1" d="M5.5 12 V10.5 A3 3 0 0 1 8.5 7.5 H12"/><path class="s1" stroke-width="1" d="M14.5 8 V9.5 A3 3 0 0 1 11.5 12.5 H8"/>',
    px: [P(12, 5, ['#..', '##.', '###', '##.', '#..']), P(5, 10, ['..#', '.##', '###', '.##', '..#'])] },
  24: { svg: '<path class="s1" stroke-width="1" d="M6.5 14 V12 A3.5 3.5 0 0 1 10 8.5 H15"/><path class="s1" stroke-width="1" d="M17.5 10 V12 A3.5 3.5 0 0 1 14 15.5 H9"/>',
    px: [P(15, 6, ['#..', '##.', '###', '##.', '#..']), P(6, 13, ['..#', '.##', '###', '.##', '..#'])] },
  30: { svg: '<path class="s1" stroke-width="2" stroke-linecap="round" d="M8 16 V14 A4 4 0 0 1 12 10 H21"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M18.5 7.5 L21.5 10 L18.5 12.5"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M22 14 V16 A4 4 0 0 1 18 20 H9"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M11.5 17.5 L8.5 20 L11.5 22.5"/>' },
  36: { svg: '<path class="s1" stroke-width="2" stroke-linecap="round" d="M9 19 V17 A5 5 0 0 1 14 12 H26"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M23 9 L26.5 12 L23 15"/><path class="s1" stroke-width="2" stroke-linecap="round" d="M27 17 V19 A5 5 0 0 1 22 24 H10"/><path class="s1" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M13 21 L9.5 24 L13 27"/>' },
};
