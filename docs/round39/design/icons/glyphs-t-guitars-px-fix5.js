// Hand corrections to the pixel fit, part 5 (notation in glyphs-t-guitars-px-fix.js):
// - the archtop redrawn so it never reads as the semi-hollow: a taller dome on the bass side
//   and one deep, round cutaway with a low, round horn (the semi-hollow has two ears);
// - 12 px where the fit lost the feature (the charango's bands, the tres's courses);
// - 15 px (the 12-px places at 125 %) is the hand-drawn 14 placed one pixel down, so the
//   decisive pixels are the same ones; the 45° guitars and the basses keep their fit.
(() => {
  if (/mode=fit/.test(location.search)) return;
  const N = window.N;
  const P = window.GTR_PX;
  P('archtop', 16, [
    '......####......',
    '......####......',
    '.......##.......',
    '...*##*##.......',
    '..*######.......',
    '..#######.......',
    '..#######..*#*..',
    '..#######*####..',
    '..############..',
    '...##########...',
    '.*############*.',
    '##.##########.##',
    '##.##########.##',
    '##.##########.##',
    '.##############.',
    '...*########*...',
  ]);
  P('archtop', 14, [
    '.....####.....',
    '.....####.....',
    '..*##*##......',
    '.*######......',
    '.#######......',
    '.#######.*#*..',
    '.############.',
    '..##########..',
    '.############.',
    '##.########.##',
    '##.########.##',
    '##.########.##',
    '.############.',
    '...*######*...',
  ]);
  P('charango', 12, [
    '...######...',
    '....####....',
    '.....##.....',
    '.....##.....',
    '...*####*...',
    '...OOOOOO...',
    '....####....',
    '..########..',
    '..OOOOOOOO..',
    '..########..',
    '...OOOOOO...',
    '....*##*....',
  ]);
  P('tres', 12, [
    '...######...',
    '....####....',
    '.....##.....',
    '...*####*...',
    '...O#OO#O...',
    '...O#OO#O...',
    '....#OO#....',
    '..#O#OO#O#..',
    '.##O#OO#O##.',
    '.##O#OO#O##.',
    '..########..',
    '...*####*...',
  ]);
  const HAND14 = ['eStype', 'eTtype', 'eSingleCut', 'eDoubleHorn', 'eSuperstrat', 'eSemiHollow', 'eOffset', 'eVshape', 'ePointed', 'archtop',
    'aDread', 'aConcert', 'aCutaway', 'aJumbo', 'aParlor', 'classical', 'guitar12', 'aTravel', 'resonator',
    'requinto', 'vihuela', 'guitarron', 'bajosexto', 'jarana', 'charango', 'cuatro', 'tres'];
  HAND14.forEach(id => { N[id][15] = { px: [{ at: [1, 1], rows: N[id][14].px[0].rows }] }; });
})();
