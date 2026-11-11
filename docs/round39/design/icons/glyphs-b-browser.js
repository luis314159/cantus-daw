// Cantus icon family — the browser set, at des-views' sizes: 12 and 14 in the rows (22 high),
// 16 for the unit marks, 10–12 for the small marks; 150 %: 15, 18, 21, 24. Same grammar.
var N = window.N || (window.N = {});
const sp = (d, w = 1, c = 's1') => `<path class="${c}" stroke-width="${w}" d="${d}"/>`;

N.folder = {
  12: { svg: sp('M1.5 2.5 H4.5 L5.5 3.5 H10.5 V9.5 H1.5 Z M2 4.5 H10') },
  14: { svg: sp('M1.5 3.5 H5.5 L6.5 4.5 H12.5 V11.5 H1.5 Z M2 5.5 H12') },
  18: { svg: sp('M2.5 4.5 H7.5 L8.5 5.5 H15.5 V14.5 H2.5 Z M3 7.5 H15') },
  21: { svg: sp('M2.5 5.5 H8.5 L9.5 6.5 H18.5 V16.5 H2.5 Z M3 8.5 H18') },
};
N.folderOpen = {
  12: { svg: sp('M1.5 9.5 V2.5 H4.5 L5.5 3.5 H9.5 V5') + sp('M1.5 9.5 L3 5.5 H11.5 L10 9.5 Z') },
  14: { svg: sp('M1.5 11.5 V3.5 H5.5 L6.5 4.5 H11.5 V6.5') + sp('M1.5 11.5 L3.5 6.5 H13.5 L11.5 11.5 Z') },
  18: { svg: sp('M2.5 14.5 V4.5 H7.5 L8.5 5.5 H14.5 V8.5') + sp('M2.5 14.5 L5 8.5 H16.5 L14 14.5 Z') },
  21: { svg: sp('M2.5 16.5 V5.5 H8.5 L9.5 6.5 H16.5 V9.5') + sp('M2.5 16.5 L5.5 9.5 H19.5 L16.5 16.5 Z') },
};

// File: a page; its folded corner in the second ink.
N.file = {
  12: { svg: sp('M2.5 1.5 H7 L9.5 4 V10.5 H2.5 Z') + sp('M7 1.5 V4 H9.5', 1, 's2') },
  14: { svg: sp('M3.5 1.5 H8.5 L11.5 4.5 V12.5 H3.5 Z') + sp('M8.5 1.5 V4.5 H11.5', 1, 's2') },
  18: { svg: sp('M4.5 2.5 H10.5 L14.5 6.5 V15.5 H4.5 Z') + sp('M10.5 2.5 V6.5 H14.5', 1, 's2') },
  21: { svg: sp('M5.5 2.5 H12.5 L16.5 6.5 V18.5 H5.5 Z') + sp('M12.5 2.5 V6.5 H16.5', 1, 's2') },
};

// Audio: a waveform — bars every other pixel, heights from a sound, centred on the axis.
const wave = (x0, heights, mid) => heights.map((h, i) => `<rect class="f1" x="${x0 + 2 * i}" y="${mid - h / 2}" width="1" height="${h}"/>`).join('');
N.audio = {
  12: { svg: wave(1, [2, 6, 10, 6, 4, 2], 6) },
  14: { svg: wave(2, [4, 8, 12, 6, 8, 2], 7) },
  16: { svg: wave(2, [2, 6, 10, 12, 6, 8, 2], 8) },
  18: { svg: wave(3, [4, 8, 12, 14, 8, 10, 4], 9) },
  21: { svg: wave(3, [4, 8, 12, 16, 10, 12, 6, 2], 10) },
  24: { svg: wave(4, [4, 8, 12, 16, 18, 10, 14, 6, 2], 12) },
};

// MIDI: two beamed notes.
const notes = (beam, s1, s2, h1, h2, rx) => `<rect class="f1" x="${beam[0]}" y="${beam[1]}" width="${beam[2]}" height="${beam[3]}"/>` +
  `<rect class="f1" x="${s1[0]}" y="${s1[1]}" width="1" height="${s1[2]}"/><rect class="f1" x="${s2[0]}" y="${s2[1]}" width="1" height="${s2[2]}"/>` +
  `<rect class="f1" x="${h1[0]}" y="${h1[1]}" width="${h1[2]}" height="${h1[3]}" rx="${rx}"/><rect class="f1" x="${h2[0]}" y="${h2[1]}" width="${h2[2]}" height="${h2[3]}" rx="${rx}"/>`;
N.midi = {
  12: { svg: notes([4, 2, 7, 1], [4, 3, 6], [10, 3, 5], [2, 8, 3, 2], [8, 7, 3, 2], 0.5) },
  14: { svg: notes([5, 2, 8, 1], [5, 3, 7], [12, 3, 6], [2, 9, 4, 2], [9, 8, 4, 2], 0.8) },
  16: { svg: notes([6, 2, 8, 2], [6, 4, 8], [13, 4, 7], [2, 11, 5, 3], [9, 10, 5, 3], 1.2) },
  18: { svg: notes([7, 3, 9, 2], [7, 5, 8], [15, 5, 7], [3, 12, 5, 3], [11, 11, 5, 3], 1.2) },
  21: { svg: notes([8, 3, 10, 2], [8, 5, 10], [17, 5, 9], [3, 14, 6, 3], [12, 13, 6, 3], 1.4) },
  24: { svg: notes([9, 4, 11, 2], [9, 6, 11], [19, 6, 10], [4, 16, 6, 4], [14, 15, 6, 4], 1.8) },
};

// Preset: a knob — a stored setting.
N.preset = {
  12: { px: [ring(8, 2, 2)], svg: cells(seg(6, 5, 8, 3)) },
  16: { px: [ring(10, 3, 3)], svg: cells(seg(8, 7, 11, 4)) },
  18: { px: [ring(10, 4, 4)], svg: cells(seg(9, 8, 12, 5)) },
  24: { svg: '<circle class="s1" stroke-width="1" cx="12" cy="12" r="6.5"/>' + cells(seg(12, 11, 16, 7)) },
};

// Unit marks: inside des-views' 16-px square — an instrument (a piano's keys, set 2's own
// 16-px drawing) or an effect (a filter's curve).
N.instrument = { 16: N.pianoRoll ? N.pianoRoll[16] : null, 24: N.pianoRoll ? N.pianoRoll[24] : null };
N.effect = {
  16: { svg: sp('M3 6.5 H8 C10 6.5 11 9 12.5 12.5') },
  24: { svg: sp('M4.5 9.5 H12 C15 9.5 16.5 13 18.5 18.5') },
};

// Pack: a box with its lid and its handle slot.
N.pack = {
  12: { svg: sp('M1.5 2.5 H10.5 V4.5 H1.5 Z M2.5 4.5 V10.5 H9.5 V4.5') + '<rect class="f1" x="5" y="6" width="2" height="1"/>' },
  14: { svg: sp('M1.5 2.5 H12.5 V5.5 H1.5 Z M2.5 5.5 V12.5 H11.5 V5.5') + '<rect class="f1" x="5" y="7" width="4" height="1"/>' },
  18: { svg: sp('M2.5 3.5 H15.5 V6.5 H2.5 Z M3.5 6.5 V15.5 H14.5 V6.5') + '<rect class="f1" x="7" y="9" width="4" height="1"/>' },
  21: { svg: sp('M2.5 3.5 H18.5 V7.5 H2.5 Z M3.5 7.5 V18.5 H17.5 V7.5') + '<rect class="f1" x="8" y="10" width="5" height="1"/>' },
};

// Favourite: a star, outlined (not yet) or filled (a favourite, in solo's amber when on).
const star = (cx, cy, R, fill) => {
  const pts = [];
  for (let i = 0; i < 10; i++) {
    const a = -Math.PI / 2 + i * Math.PI / 5, r = i % 2 ? R * 0.43 : R;
    pts.push(`${(cx + r * Math.cos(a)).toFixed(2)} ${(cy + r * Math.sin(a)).toFixed(2)}`);
  }
  return `<path class="${fill ? 'f1' : 's1'}" stroke-width="1" stroke-linejoin="round" d="M${pts.join(' L')} Z"/>`;
};
N.star = { 12: { svg: star(6, 6.4, 5, false) }, 14: { svg: star(7, 7.4, 6, false) }, 18: { svg: star(9, 9.5, 7.5, false) }, 21: { svg: star(10.5, 11, 9, false) } };
N.starFilled = { 12: { svg: star(6, 6.4, 5.2, true) }, 14: { svg: star(7, 7.4, 6.2, true) }, 18: { svg: star(9, 9.5, 7.8, true) }, 21: { svg: star(10.5, 11, 9.3, true) } };

// Search: the magnifier — set 2's zoom, drawn for these pixels.
N.search = {
  12: { ...N.zoom[16], at: [-2, -2] },
  14: { ...N.zoom[16], at: [-1, -1] },
  18: { v1: ['zoom', 16], at: [1, 1] },
  21: { v1: ['zoom', 20], at: [1, 1] },
};
