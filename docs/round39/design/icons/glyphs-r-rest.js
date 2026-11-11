// Cantus icon family — set 5, the rest of glyph.rs: glyphs that sit in keys and panels,
// drawn for keys 16, 20, 24 (1-px lines) and 30, 36 (2-px lines), set 2's grammar.
var N = window.N || (window.N = {});
const s1 = (d, w = 1, extra = '') => `<path class="s1" stroke-width="${w}" ${extra} d="${d}"/>`;
const s2r = (d) => s1(d, 2, 'stroke-linecap="round" stroke-linejoin="round"');
const box = (x, y, w, h, c = 'f1', rx = 0) => `<rect class="${c}" x="${x}" y="${y}" width="${w}" height="${h}"${rx ? ` rx="${rx}"` : ''}/>`;

// Help: a question mark, no ring (a ring made it a diagram at 16 px).
N.help = {
  16: { svg: s1('M6 5.5 A2.5 2.5 0 1 1 8.5 8 V9.5') + box(8, 11, 1, 1) },
  20: { svg: s1('M7.5 7.5 A3 3 0 1 1 10.5 10.5 V12') + box(10, 14, 1, 1) },
  24: { svg: s1('M9 8.5 A3.5 3.5 0 1 1 12.5 12 V14') + '<circle class="f1" cx="12.5" cy="16.5" r="0.9"/>' },
  30: { svg: s2r('M10.5 11 A4.5 4.5 0 1 1 15 15.5 V18') + '<circle class="f1" cx="15" cy="22" r="1.4"/>' },
  36: { svg: s2r('M12.5 13 A5.5 5.5 0 1 1 18 18.5 V21') + '<circle class="f1" cx="18" cy="26" r="1.6"/>' },
};

// Warning: a triangle and its mark (lit in solo's amber).
N.warning = {
  16: { svg: s1('M8.5 3.5 L13.5 12.5 H3.5 Z', 1, 'stroke-linejoin="round"') + box(8, 6, 1, 3) + box(8, 10, 1, 1) },
  20: { svg: s1('M10.5 3.5 L16.5 15.5 H4.5 Z', 1, 'stroke-linejoin="round"') + box(10, 7, 1, 4) + box(10, 13, 1, 1) },
  24: { svg: s1('M12.5 4.5 L19.5 18.5 H5.5 Z', 1, 'stroke-linejoin="round"') + box(12, 8, 1, 5) + box(12, 15, 1, 1) },
  30: { svg: s2r('M15 5 L24 22 H6 Z') + s2r('M15 10.5 V15.5') + '<circle class="f1" cx="15" cy="19" r="1.2"/>' },
  36: { svg: s2r('M18 6 L29 27 H7 Z') + s2r('M18 12.5 V19') + '<circle class="f1" cx="18" cy="23" r="1.4"/>' },
};

// Agent: a sparkle and a small one — the AI area.
const spark = (cx, cy, R) => `<path class="f1" d="M${cx} ${cy - R} Q${cx} ${cy} ${cx + R} ${cy} Q${cx} ${cy} ${cx} ${cy + R} Q${cx} ${cy} ${cx - R} ${cy} Q${cx} ${cy} ${cx} ${cy - R} Z"/>`;
N.agent = {
  16: { svg: spark(7, 9, 5) + spark(12, 4, 2) },
  20: { svg: spark(9, 11, 6) + spark(15, 5, 2.5) },
  24: { svg: spark(11, 13, 7) + spark(18, 6, 3) },
  30: { svg: spark(13.5, 16.5, 9) + spark(22.5, 7.5, 3.5) },
  36: { svg: spark(16, 20, 11) + spark(27, 9, 4) },
};

// Device: a wave in a circle.
N.device = {
  16: { px: [ring(10, 3, 3)], svg: s1('M5 8 Q6.5 5.5 8 8 T11 8') },
  20: { svg: '<circle class="s1" stroke-width="1" cx="10" cy="10" r="5.5"/>' + s1('M6.5 10 Q8.25 7 10 10 T13.5 10') },
  24: { svg: '<circle class="s1" stroke-width="1" cx="12" cy="12" r="6.5"/>' + s1('M7.5 12 Q9.75 8.5 12 12 T16.5 12') },
  30: { svg: '<circle class="s1" stroke-width="2" cx="15" cy="15" r="8"/>' + s2r('M10 15 Q12.5 11 15 15 T20 15') },
  36: { svg: '<circle class="s1" stroke-width="2" cx="18" cy="18" r="10"/>' + s2r('M12 18 Q15 13 18 18 T24 18') },
};

// Keyboard (the typing keyboard plays notes): a computer keyboard — two rows of keys, a space bar.
const dots = (xs, y, w = 1, h = 1) => xs.map(x => box(x, y, w, h)).join('');
N.keyboard = {
  16: { svg: s1('M2.5 4.5 H13.5 V11.5 H2.5 Z') + dots([4, 6, 8, 10, 12], 6) + dots([5, 7, 9, 11], 8) + box(5, 10, 6, 1) },
  20: { svg: s1('M3.5 5.5 H16.5 V14.5 H3.5 Z') + dots([5, 7, 9, 11, 13, 15], 7) + dots([6, 8, 10, 12, 14], 9) + box(7, 12, 6, 1) },
  24: { svg: s1('M4.5 6.5 H19.5 V17.5 H4.5 Z') + dots([6, 8, 10, 12, 14, 16, 18], 9) + dots([7, 9, 11, 13, 15, 17], 11) + box(8, 14, 8, 1) },
  30: { svg: s1('M5 8 H25 V22 H5 Z', 2, 'stroke-linejoin="round"') + dots([8, 11, 14, 17, 20], 11, 2, 2) + dots([9.5, 12.5, 15.5, 18.5].map(Math.floor), 14, 2, 2) + box(10, 17, 10, 2) },
  36: { svg: s1('M6 9 H30 V27 H6 Z', 2, 'stroke-linejoin="round"') + dots([9, 13, 17, 21, 25], 13, 2, 2) + dots([11, 15, 19, 23], 17, 2, 2) + box(12, 21, 12, 2) },
};

// Score: a staff (second ink) and a note on it.
const staff = (ys, x0, x1) => ys.map(y => box(x0, y, x1 - x0, 1, 'f2')).join('');
N.score = {
  16: { svg: staff([5, 8, 11], 2, 14) + box(6, 8, 3, 2, 'f1', 0.8) + box(8, 3, 1, 6) },
  20: { svg: staff([6, 9, 12, 15], 3, 17) + box(7, 11, 4, 3, 'f1', 1.2) + box(10, 4, 1, 8) },
  24: { svg: staff([7, 10, 13, 16], 4, 20) + box(8, 12, 5, 3, 'f1', 1.4) + box(12, 5, 1, 8) },
  30: { svg: staff([9, 13, 17, 21], 5, 25) + '<ellipse class="f1" cx="12" cy="17" rx="3.2" ry="2.3" transform="rotate(-20 12 17)"/>' + box(14, 6, 2, 11) },
  36: { svg: staff([11, 16, 21, 26], 6, 30) + '<ellipse class="f1" cx="14.5" cy="21" rx="3.8" ry="2.7" transform="rotate(-20 14.5 21)"/>' + box(17, 7, 2, 14) },
};

// Mouse menu: a mouse whose right button is lit — "has a right-click menu".
N.mouseMenu = {
  16: { svg: s1('M5.5 5.5 A3 3 0 0 1 11.5 5.5 V9.5 A3 3 0 0 1 5.5 9.5 Z M6 7.5 H11 M8.5 3 V7') + box(9, 3, 2, 4) },
  20: { svg: s1('M6.5 7 A3.5 3.5 0 0 1 13.5 7 V12 A3.5 3.5 0 0 1 6.5 12 Z M7 9.5 H13 M10.5 4 V9') + box(11, 4, 2, 5) },
  24: { svg: s1('M7.5 8.5 A4.5 4.5 0 0 1 16.5 8.5 V14.5 A4.5 4.5 0 0 1 7.5 14.5 Z M8 11.5 H16 M12.5 4.5 V11') + box(13, 5, 3, 6) },
  30: { svg: s1('M9 11 A6 6 0 0 1 21 11 V19 A6 6 0 0 1 9 19 Z M9 14 H21 M15 5 V14', 2, 'stroke-linejoin="round"') + box(16, 6, 4, 7, 'f1', 1) },
  36: { svg: s1('M11 13 A7 7 0 0 1 25 13 V23 A7 7 0 0 1 11 23 Z M11 16 H25 M18 6 V16', 2, 'stroke-linejoin="round"') + box(19, 7, 5, 8, 'f1', 1) },
};

// Automate: an envelope — straight segments and their points.
N.automate = {
  16: { svg: cells(seg(2, 11, 6, 7).concat(seg(6, 7, 9, 10), seg(9, 10, 13, 6))) + box(5, 6, 2, 2) + box(8, 9, 2, 2) },
  20: { svg: cells(seg(3, 14, 8, 9).concat(seg(8, 9, 12, 13), seg(12, 13, 17, 8))) + box(7, 8, 3, 3, 'f1', 0.8) + box(11, 12, 3, 3, 'f1', 0.8) },
  24: { svg: cells(seg(4, 16, 10, 10).concat(seg(10, 10, 14, 14), seg(14, 14, 20, 8))) + box(9, 9, 3, 3, 'f1', 0.8) + box(13, 13, 3, 3, 'f1', 0.8) },
  30: { svg: s2r('M6 21 L13 13 L18 18 L24 11') + '<circle class="f1" cx="13" cy="13" r="2.4"/><circle class="f1" cx="18" cy="18" r="2.4"/>' },
  36: { svg: s2r('M7 25 L16 15 L21 20 L29 12') + '<circle class="f1" cx="16" cy="15" r="2.8"/><circle class="f1" cx="21" cy="20" r="2.8"/>' },
};

// Filter: a funnel.
N.filter = {
  16: { svg: s1('M2.5 3.5 H13.5 L9.5 8.5 V12.5 L6.5 11 V8.5 Z', 1, 'stroke-linejoin="round"') },
  20: { svg: s1('M3.5 4.5 H16.5 L11.5 10.5 V15.5 L8.5 14 V10.5 Z', 1, 'stroke-linejoin="round"') },
  24: { svg: s1('M4.5 5.5 H19.5 L13.5 12.5 V18.5 L10.5 17 V12.5 Z', 1, 'stroke-linejoin="round"') },
  30: { svg: s2r('M6 7 H24 L17 15.5 V23 L13 21 V15.5 Z') },
  36: { svg: s2r('M7 8 H29 L21 18 V27 L15 24.5 V18 Z') },
};
