// Cantus icon family — the browser set, part 2: history, clear, sounding, installed, get.
var N = window.N || (window.N = {});

// History: a clock whose ring runs back (an arrowhead where it opens) — what was used.
const hist = (c, r, head, hands) => {
  const a1 = 200 * Math.PI / 180, a2 = 160 * Math.PI / 180;
  const p = a => `${(c + r * Math.cos(a)).toFixed(2)} ${(c + r * Math.sin(a)).toFixed(2)}`;
  return `<path class="s1" stroke-width="1" d="M${p(a1)} A${r} ${r} 0 1 1 ${p(a2)}"/><path class="f1" d="${head}"/>` +
    hands.map(h => `<rect class="f1" x="${h[0]}" y="${h[1]}" width="${h[2]}" height="${h[3]}"/>`).join('');
};
N.history = {
  12: { svg: hist(6.5, 4, 'M0.8 7.4 L4.6 7.4 L2.7 9.8 Z', [[6, 4, 1, 3], [6, 6, 2, 1]]) },
  14: { svg: hist(7.5, 5, 'M0.9 8.8 L4.7 8.8 L2.8 11.4 Z', [[7, 4, 1, 4], [7, 7, 3, 1]]) },
  18: { svg: hist(9.5, 6.5, 'M1.2 11.2 L5.6 11.2 L3.4 14.2 Z', [[9, 5, 1, 5], [9, 9, 4, 1]]) },
  21: { svg: hist(10.5, 7.5, 'M1 12.4 L5.9 12.4 L3.45 15.8 Z', [[10, 5, 1, 6], [10, 10, 4, 1]]) },
};

// Clear (a search, a chip): a cross, pixels.
N.clear = {
  10: { svg: cells(seg(1, 1, 7, 7).concat(seg(1, 7, 7, 1))) },
  12: { svg: cells(seg(2, 2, 8, 8).concat(seg(2, 8, 8, 2))) },
  15: { svg: cells(seg(3, 3, 11, 11).concat(seg(3, 11, 11, 3))) },
  18: { svg: cells(seg(4, 4, 12, 12).concat(seg(4, 12, 12, 4))) },
};

// Sounding (the row being previewed): a speaker and two waves.
const wavesAt = (cx, cy, rs) => rs.map(r => {
  const dx = (r * Math.cos(Math.PI / 4)).toFixed(2), dy = (r * Math.sin(Math.PI / 4)).toFixed(2);
  return `<path class="s1" stroke-width="1" d="M${(cx + +dx).toFixed(2)} ${(cy - dy).toFixed(2)} A${r} ${r} 0 0 1 ${(cx + +dx).toFixed(2)} ${(cy + +dy).toFixed(2)}"/>`;
}).join('');
N.speaker = {
  12: { svg: '<rect class="f1" x="1" y="5" width="2" height="3"/><rect class="f1" x="4" y="4" width="1" height="5"/><rect class="f1" x="5" y="3" width="1" height="7"/>' + wavesAt(5.5, 6.5, [2.5, 4.5]) },
  18: { svg: '<rect class="f1" x="2" y="7" width="3" height="4"/><rect class="f1" x="6" y="6" width="1" height="6"/><rect class="f1" x="7" y="5" width="1" height="8"/><rect class="f1" x="8" y="4" width="1" height="10"/>' + wavesAt(8.5, 9, [3.5, 6]) },
};

// Installed: a check, pixels.
N.check = {
  10: { svg: cells(seg(1, 5, 3, 7).concat(seg(4, 6, 8, 2))) },
  12: { svg: cells(seg(2, 6, 4, 8).concat(seg(5, 7, 10, 2))) },
  15: { svg: cells(seg(2, 8, 5, 11).concat(seg(6, 10, 12, 4))) },
  18: { svg: cells(seg(3, 9, 7, 13).concat(seg(8, 12, 15, 5))) },
};

// Get (download a pack): an arrow down into a tray.
const getArrow = (x, top, stemEnd, headRows, tray) => {
  let l = seg(x, top, x, stemEnd);
  headRows.forEach(([y, w]) => { l = l.concat(seg(x - w, y, x + w, y)); });
  const [x0, y0, x1, y1] = tray;
  l = l.concat(seg(x0, y0, x0, y1), seg(x0, y1, x1, y1), seg(x1, y0, x1, y1));
  return cells(l);
};
N.download = {
  12: { svg: getArrow(5, 1, 6, [[7, 2], [8, 1], [9, 0]], [1, 8, 9, 10]) },
  14: { svg: getArrow(6, 1, 6, [[7, 2], [8, 1], [9, 0]], [1, 10, 11, 12]) },
  18: { svg: getArrow(8, 2, 9, [[10, 3], [11, 2], [12, 1], [13, 0]], [2, 12, 14, 15]) },
  21: { svg: getArrow(10, 2, 10, [[11, 4], [12, 3], [13, 2], [14, 1], [15, 0]], [2, 15, 18, 18]) },
};

// The browser set's rows for the sheet: id, name, sizes at 100 %, at 150 %, the on colour.
window.BROWSER_ROWS = [
  ['folder', 'folder', [12, 14], [18, 21], ''], ['folderOpen', 'folder, open', [12, 14], [18, 21], ''],
  ['file', 'file', [12, 14], [18, 21], ''], ['audio', 'audio', [12, 14, 16], [18, 21, 24], ''],
  ['midi', 'MIDI', [12, 14, 16], [18, 21, 24], ''], ['preset', 'preset', [12, 16], [18, 24], ''],
  ['instrument', 'unit mark: instrument', [16], [24], ''], ['effect', 'unit mark: effect', [16], [24], ''],
  ['pack', 'pack', [12, 14], [18, 21], ''], ['star', 'favourite (not)', [12, 14], [18, 21], 'solo'],
  ['starFilled', 'favourite', [12, 14], [18, 21], 'solo'], ['search', 'search', [12, 14], [18, 21], ''],
  ['history', 'history', [12, 14], [18, 21], ''], ['clear', 'clear', [10, 12], [15, 18], ''],
  ['speaker', 'sounding', [12], [18], ''], ['check', 'installed', [10, 12], [15, 18], 'play'],
  ['download', 'get', [12, 14], [18, 21], ''],
];
