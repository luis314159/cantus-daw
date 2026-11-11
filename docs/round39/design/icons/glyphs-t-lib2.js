// Cantus icon family — set 6, the instrument library: keys, percussion, electronic, other.
var N = window.N || (window.N = {});

// ---- Keys (the grand piano is set 6's first `piano`)
track('upright', t => sil(t, RR(3, 3, 18, 11.4, 1) + RR(2, 14.4, 20, 2.8, 0, 'f2') + (t.D ? CUT(t, 'M5 14.4 V15.8 M8 14.4 V15.8 M13 14.4 V15.8 M16 14.4 V15.8 M19 14.4 V15.8', 1) : '') +
  RR(3.6, 17.2, 16.8, 4.4) + RR(2, 21.6, 3.4, 1.2) + RR(18.6, 21.6, 3.4, 1.2)));
track('organ', t => sil(t, [[3.2, 12], [6, 9], [8.8, 6], [11.6, 3], [14.4, 6], [17.2, 9], [20, 12]].map(([x, y]) => RR(x - 1.1, y, 2.2, 17 - y, 0.8)).join('') +
  RR(1.6, 17.4, 20.8, 5, 1) + (t.D ? CUT(t, 'M3 19.4 H21', 1) : '')));
track('epiano', t => sil(t, F('M2 9.4 L4 6.4 H20 L22 9.4 Z', 'f2') + RR(2, 9.4, 20, 5.4) + (t.D ? CUT(t, 'M5 12.6 V14.4 M8 12.6 V14.4 M13 12.6 V14.4 M16 12.6 V14.4 M19 12.6 V14.4', 1) : '') +
  STK(t, 'M4.4 14.8 V22.4 M19.6 14.8 V22.4', 1.2)));
track('synthkeys', t => sil(t, RR(1.6, 6.4, 20.8, 11.2, 1.2) + RR(3, 11.4, 18, 4.8, 0, 'f2') + (t.D ? CUT(t, 'M6.6 11.4 V13.8 M9.6 11.4 V13.8 M14.4 11.4 V13.8 M17.4 11.4 V13.8', 1) : '') +
  [5, 8.2, 11.4].map(x => `<circle class="cut" cx="${x}" cy="8.9" r="${t.D ? 0.9 : 1}"/>`).join('') + RR(15, 8.2, 5.6, 1.4, 0, 'cut')));

// ---- Percussion
const drumBody = (x, top, w, bot) => F(`M${x} ${top} H${x + w} L${x + w - 0.8} ${bot - 2.6} L${x + w - 0.2} ${bot} H${x + 0.2} L${x + 0.8} ${bot - 2.6} Z`);
track('congas', t => sil(t, drumBody(2.6, 7, 8, 22.8) + `<ellipse class="f2" cx="6.6" cy="7" rx="4" ry="1.3"/>` + drumBody(12.6, 4, 8.6, 22.8) + `<ellipse class="f2" cx="16.9" cy="4" rx="4.3" ry="1.3"/>`));
track('timpani', t => sil(t, F('M2.6 9 H21.4 C21.4 15.2 17.4 19 12 19 C6.6 19 2.6 15.2 2.6 9 Z') + `<ellipse class="f2" cx="12" cy="9" rx="9.4" ry="2.2"/>` + STK(t, 'M6.4 17 L4.6 22.6 M17.6 17 L19.4 22.6', 1.2)));
track('tambourine', t => sil(t, STK(t, 'M12 3.4 A8.6 8.6 0 1 1 11.99 3.4', t.S >= 30 ? 3 : 2.2) +
  [[12, 3.4], [20.6, 12], [12, 20.6], [3.4, 12]].map(([x, y]) => C1(x, y, 1.9, 'f2')).join('')));
track('cowbell', t => sil(t, F('M8 6.6 H16 L19.4 21 H4.6 Z') + STK(t, 'M10 6.6 V4.4 C10 2.6 14 2.6 14 4.4 V6.6', 1.3) + (t.D ? CUT(t, 'M7 18 H17', 1) : '')));
track('xylophone', t => sil(t, [[3, 5, 18], [6.4, 6.4, 16.6], [9.8, 7.8, 15.2], [13.2, 9.2, 13.8], [16.6, 10.6, 12.4]].map(([x, y0, y1]) => RR(x, y0, 2.6, y1 - y0 + 3.6, 0.6)).join('') +
  STK(t, 'M14.6 22.4 L21.6 3.6 M18 22.6 L22.4 9.4', 1, 's2') + C1(21.6, 3.6, 1.3) + C1(22.4, 9.4, 1.3)));
track('vibraphone', t => sil(t, RR(2, 4, 20, 4.2, 0.8) + (t.D ? CUT(t, 'M5.4 4 V8.2 M8.8 4 V8.2 M12.2 4 V8.2 M15.6 4 V8.2 M19 4 V8.2', 0.8) : '') +
  STK(t, 'M4 8.2 V20 M7.4 8.2 V18.4 M10.8 8.2 V16.8 M14.2 8.2 V15.2 M17.6 8.2 V13.6 M20.6 8.2 V12', 1.2, 's2') + RR(2, 20.6, 20, 1.6)));
track('glockenspiel', t => sil(t, STK(t, 'M5 2.6 C5 14 8 19 12 19 C16 19 19 14 19 2.6', 1.4) +
  [[7, 4.4, 10], [7.6, 7.2, 8.8], [8.4, 10, 7.2], [9.4, 12.8, 5.2]].map(([x, y, w]) => RR(x, y, w, 1.6, 0.4)).join('') + STK(t, 'M12 19 V23.4', 1.4)));
track('triangleInst', t => sil(t, STK(t, 'M10.4 5.2 L3.4 19 H20.6 L13.8 6.4', t.S >= 30 ? 2 : 1.6) + STK(t, 'M12 4.6 V1.4', 1, 's2') + STK(t, 'M15.6 13.6 L22 9', 1.2)));
track('gong', t => sil(t, STK(t, 'M3 23 V3 H21 V23', 1.2, 's2') + C1(12, 13, 7.2) + C1(12, 13, 2.4, 'cut') + STK(t, 'M9 3 L9.6 6.4 M15 3 L14.4 6.4', 1, 's2')));
track('cajon', t => sil(t, RR(5, 2.4, 14, 20.4, 1) + C1(12, 8.6, 2.5, 'cut') + (t.D ? CUT(t, 'M6.4 4 H17.6', 1) : '')));

// ---- Electronic (brand-free)
track('drummachine', t => sil(t, RR(1.6, 5, 20.8, 14, 1.4) + RR(3.4, 6.8, 6.4, 2.4, 0, 'cut') + [12.6, 15.6, 18.6].map(x => `<circle class="cut" cx="${x}" cy="8" r="1"/>`).join('') +
  [3.4, 5.8, 8.2, 10.6, 13, 15.4, 17.8].map(x => RR(x, 13.8, 1.8, 3, 0.3, 'cut')).join('')));
track('sampler', t => sil(t, RR(2.6, 2.6, 18.8, 18.8, 1.6) + RR(4.4, 4.4, 15.2, 3.2, 0, 'cut') +
  [0, 1, 2, 3].map(r => [0, 1, 2, 3].map(c => RR(4.6 + c * 3.8, 9.4 + r * 2.9, 3, 2.3, 0.4, 'cut')).join('')).join('')));
track('turntable', t => sil(t, `<rect class="s1" fill="none" stroke-width="${px(t, lineW(t, 1))}" x="2.4" y="3.4" width="19.2" height="17.2" rx="1.4"/>` + C1(10.4, 12, 6.2) + C1(10.4, 12, 1.6, 'cut') +
  STK(t, 'M19 5.4 V13.4 L15.6 16.6', 1.2)));
track('modular', t => sil(t, `<rect class="s1" fill="none" stroke-width="${px(t, lineW(t, 1))}" x="2" y="3" width="20" height="18" rx="1.2"/>` +
  [[6.4, 8], [12, 8], [17.6, 8], [6.4, 16], [12, 16], [17.6, 16]].map(([x, y]) => `<circle class="s1" fill="none" stroke-width="${px(t, 1)}" cx="${x}" cy="${y}" r="1.7"/>`).join('') +
  STK(t, 'M6.4 8 C6.4 13 12 11 12 16', 1.4, 's1') + STK(t, 'M17.6 8 C17.6 12 12 10 12 8', 1.2, 's2')));
track('boom808', t => sil(t, STK(t, 'M2 12 C2.6 3 5.4 3 6 12 C6.8 20 9.4 20 10.4 12 C11.4 6.4 13.6 6.4 14.6 12 C15.6 16 17.6 16 18.6 12 C19.6 9.8 21 9.8 22 12', 1.3) + (t.D ? RR(2, 21.2, 20, 0.8, 0, 'f2') : '')));

// ---- World and other
const BELL = 'M-3 3.4 C-3 -3 -2 -5 0 -5 C2 -5 3 -3 3 3.4 L4.4 5 H-4.4 Z';
track('bells', t => sil(t, `<g transform="translate(8.2 12) rotate(-14)">${F(BELL, 'f2')}<circle class="f2" cx="0" cy="6.6" r="1.2"/></g><g transform="translate(15.6 13) rotate(12)">${F(BELL)}<circle class="f1" cx="0" cy="6.6" r="1.2"/></g>` +
  STK(t, 'M8.2 6.6 V3.6 M15.6 7.6 V4.6', 1)));
track('musicbox', t => sil(t, RR(2.6, 10.6, 14.6, 10.4, 1) + F('M2.6 10.6 L5.6 4 H20 L17.2 10.6 Z', 'f2') + STK(t, 'M17.2 15.6 H21.2 V12.4', 1.2) + (t.D ? CUT(t, 'M4.4 15.6 H15.4', 1) : '')));
track('whistle', t => sil(t, F('M1.8 8.6 H13.6 C18.4 8.6 22.2 11.6 22.2 15.4 C22.2 19.2 19 22 15.4 22 C11.6 22 9 19.2 8.8 15.6 V12.4 H1.8 Z') + RR(9.6, 8.6, 2.4, 2.6, 0, 'cut') + STK(t, 'M4 8.6 C4 5.6 6.6 4 9 5', 1, 's2')));
track('beatbox', t => sil(t, F('M2.6 12 C5.6 7.4 8.8 7.8 12 9.4 C15.2 7.8 18.4 7.4 21.4 12 C18.4 16.8 15.2 18 12 18 C8.8 18 5.6 16.8 2.6 12 Z') + CUT(t, 'M4.4 12 C8 11 16 11 19.6 12', 1) +
  (t.D ? STK(t, 'M20.6 5.6 C21.8 6.6 22.4 7.6 22.6 8.8', 1, 's2') : '')));
