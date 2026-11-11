// Cantus icon family — set 6, the instruments redrawn after the owner's review (2026-11-09):
// silhouettes of real instruments, filled, details cut out of them (the key's surface shows
// through), designed on the 24-unit sheet. Variants first; the chosen ones replace the
// first drawings at the end of this file.
var N = window.N || (window.N = {});

// A silhouette group: 24-unit paths, built for the size (the group carries the size's
// factor; a stroke inside is given in device pixels through `px`).
const sil = (t, inner, rot = 0) => `<g transform="scale(${t.k}) rotate(${rot} 12 12)">${inner}</g>`;
const px = (t, n) => (n / t.k).toFixed(3);
// One weight per size (the final pass, icons-final): a drawn line is 1 device px below 30 and
// 2 px from 30; a cut (the surface showing through) is 1 px at every size. A width under 1 is
// a device pixel already divided by an inner group's scale and keeps that ratio; a width over
// 1.6 is a mass (a ring, a halo), kept, and rounded to whole pixels below 30.
const lineW = (t, n) => n <= 1 ? n * (t.S >= 30 ? 2 : 1) : n <= 1.6 ? (t.S >= 30 ? 2 : 1) : (t.S >= 30 ? n : Math.round(n));
const cutW = (t, n) => n <= 1 ? n : n <= 1.6 ? 1 : (t.S >= 30 ? n : Math.round(n));
const F = (d, c = 'f1') => `<path class="${c}" d="${d}"/>`;
const CUT = (t, d, w) => `<path class="scut" fill="none" stroke-width="${px(t, cutW(t, w))}" stroke-linecap="round" d="${d}"/>`;
const STK = (t, d, w, c = 's1') => `<path class="${c}" fill="none" stroke-width="${px(t, c === 'scut' ? cutW(t, w) : lineW(t, w))}" stroke-linecap="round" stroke-linejoin="round" d="${d}"/>`;

// ---- Guitars (diagonal: body below left, head above right)
const STRAT = 'M12 13.2 C11 13.2 10.6 12.4 10 11.4 C9.4 10.4 8.2 10 7.7 11.1 C7.2 12.3 7.8 13.6 8 14.6 C8.2 15.6 6.4 16.6 6.4 18.8 C6.4 21.6 9 23 12 23 C15 23 17.6 21.6 17.6 19 C17.6 16.8 15.9 16 16 14.8 C16.1 13.8 16.8 13 16.3 12.1 C15.8 11.4 14.8 11.9 14.2 12.6 C13.6 13.1 13 13.2 12 13.2 Z';
const LP = 'M12 13.4 C10.4 13.4 8 13.2 7.2 15.4 C6.6 17 6.2 17.6 6.2 19.2 C6.2 21.8 8.8 23.2 12 23.2 C15.2 23.2 17.8 21.8 17.8 19.2 C17.8 17.6 17 16.8 16.6 15.6 C16.4 14.4 17.2 13.6 16.4 12.6 C15.6 11.8 14.6 12.6 14 13 C13.4 13.4 13 13.4 12 13.4 Z';
track('eguitarA', t => sil(t, F(STRAT) + F('M11.2 2.4 H12.8 V13.6 H11.2 Z') + F('M11 3 L11.2 0.4 C12.6 0 14.2 0.3 14.8 1.1 C15.2 1.8 14.1 2.4 12.8 2.8 V3.2 Z') +
  (t.D ? CUT(t, 'M10.5 16.2 H13.5 M10.5 18.2 H13.5', 1) : '') + (t.D === 2 ? CUT(t, 'M10.2 20.4 H13.8', 1) : ''), 45));
track('eguitarB', t => sil(t, F(LP) + F('M11.2 2.4 H12.8 V13.6 H11.2 Z') + F('M10.4 0.2 H13.6 L13.2 3 H10.8 Z') + (t.D ? CUT(t, 'M10.4 17 H13.6 M10.4 19.4 H13.6', 1) : ''), 45));
const ACOUSTIC = 'M12 8.7 C14.4 8.7 16 10.3 16 12.4 C16 13.6 15.4 14.4 15.3 15 C16.6 15.8 17.2 17 17.2 18.6 C17.2 21.6 14.8 23.6 12 23.6 C9.2 23.6 6.8 21.6 6.8 18.6 C6.8 17 7.4 15.8 8.7 15 C8.6 14.4 8 13.6 8 12.4 C8 10.3 9.6 8.7 12 8.7 Z';
track('aguitarA', t => sil(t, F(ACOUSTIC) + F('M11.25 1.8 H12.75 V9.4 H11.25 Z') + F('M10.6 0 H13.4 V2.6 H10.6 Z') +
  `<circle class="cut" cx="12" cy="14.2" r="${t.D ? 1.8 : 2}"/>` + (t.D ? CUT(t, 'M10.2 19.4 H13.8', 1) : ''), 45));

// ---- Bass guitar: upright, a long neck, a 2 + 2 headstock, a bass body.
const PBASS = 'M12 15 C11 15 10.6 14.2 10 13.4 C9.4 12.6 8.2 12.4 7.8 13.4 C7.4 14.6 8.2 15.8 8.2 16.6 C8.2 17.4 6.8 18.2 6.8 20.2 C6.8 22.6 9.2 23.8 12 23.8 C14.8 23.8 17.2 22.6 17.2 20.4 C17.2 18.6 16 17.8 16 16.8 C16 15.8 16.8 15 16.2 14.4 C15.6 13.8 14.8 14.2 14.2 14.6 C13.6 15 13 15 12 15 Z';
const PEGS = 'M10.8 0.2 H13.2 V3.6 H10.8 Z M8.6 0.6 H10.8 V1.6 H8.6 Z M8.6 2.2 H10.8 V3.2 H8.6 Z M13.2 0.6 H15.4 V1.6 H13.2 Z M13.2 2.2 H15.4 V3.2 H13.2 Z';
track('bassguitarA', t => sil(t, F(PBASS) + F('M11.2 3.4 H12.8 V15.4 H11.2 Z') + F(PEGS) + (t.D ? CUT(t, 'M10.4 18.4 H13.6', 1.2) : '')));
track('bassguitarB', t => sil(t, F(PBASS) + F('M11.2 3.4 H12.8 V15.4 H11.2 Z') + F(PEGS) + (t.D ? CUT(t, 'M10.4 18.4 H13.6', 1.2) : ''), 22));

// ---- Clap: two hands meeting, sparks where they meet.
const MITTEN = 'M-2.6 6.5 V-4 C-2.6 -6.4 2.6 -6.4 2.6 -4 V-1 C3.4 -2.2 5 -2 5 -0.8 C5 0 4 1.2 2.6 3 V6.5 Z';
const hands = () => `<g transform="translate(15.6 14.2) scale(-1 1) rotate(24)">${F(MITTEN, 'f2')}</g><g transform="translate(8.4 14.2) rotate(24)">${F(MITTEN)}</g>`;
track('clapA', t => sil(t, hands() + STK(t, 'M12 1.2 V3.4 M7.4 2.4 L8.6 4.2 M16.6 2.4 L15.4 4.2', 1)));
track('clapB', t => sil(t, `<g transform="translate(9 15) rotate(18)">${F('M-3 6 V-5 C-3 -7 3 -7 3 -5 V6 Z')}</g><g transform="translate(15 15) rotate(-18)">${F('M-3 6 V-5 C-3 -7 3 -7 3 -5 V6 Z', 'f2')}</g>` + STK(t, 'M12 1.5 V4 M8 2.6 L9.2 4.6 M16 2.6 L14.8 4.6', 1)));

// ---- Brass: a trumpet (flared bell, three valves, the tube's loop) or a horn.
track('brassA', t => sil(t, F('M2.6 11.2 H4.6 V12.8 H2.6 Z M4.4 11.4 H15.4 V12.6 H4.4 Z M15 10.6 C17.6 10.4 19.6 7.8 22 6.2 V17.8 C19.6 16.2 17.6 13.6 15 13.4 Z') +
  F('M7.4 6.6 H9 V11.4 H7.4 Z M9.8 6.6 H11.4 V11.4 H9.8 Z M12.2 6.6 H13.8 V11.4 H12.2 Z M7 5.6 H14.2 V6.8 H7 Z') +
  (t.D ? STK(t, 'M6 12.6 V15.6 C6 17 7 17.6 8.2 17.6 H13.4 C14.6 17.6 15.2 16.8 15.2 15.6 V13.6', 1, 's2') : '')));
track('brassB', t => sil(t, STK(t, 'M10 6.8 A6 6 0 1 1 4.6 15.8', t.S >= 30 ? 2 : 1.6) + F('M13.6 15 C15.8 16.2 17.8 18.6 18.6 22.4 H22.6 C21.8 18 19 14.8 15.2 13.2 Z') + STK(t, 'M10 6.8 C10.6 4.6 9.4 3 7.4 2.2', 1)));

// ---- Woodwind: a saxophone (crook, body, bell turned up) or a clarinet with its keys.
track('woodwindA', t => sil(t, F('M5 1.4 C8.6 1 11.6 2.4 12.2 5.2 V16.6 C12.2 18.2 13.2 19.2 14.6 19.2 C16 19.2 16.8 18.2 16.8 16.8 V12.8 C16.8 11 18.2 9.6 20.4 9.4 L20.8 11.8 C19.6 11.9 19.2 12.6 19.2 13.6 V16.8 C19.2 19.8 17.2 21.8 14.6 21.8 C11.8 21.8 9.8 19.8 9.8 16.8 V5.6 C9.6 4 7.8 3.2 5.2 3.4 Z') +
  (t.D ? `<circle class="cut" cx="11" cy="8.4" r="0.8"/><circle class="cut" cx="11" cy="11.2" r="0.8"/><circle class="cut" cx="11" cy="14" r="0.8"/>` : '')));
track('woodwindB', t => sil(t, F('M11.2 3.2 L11.6 0.8 H12.4 L12.8 3.2 Z M10.8 3 H13.2 V19 H10.8 Z M10.4 18.8 H13.6 L15.4 22.8 H8.6 Z') +
  (t.D ? `<circle class="cut" cx="12" cy="6.2" r="0.7"/><circle class="cut" cx="12" cy="8.8" r="0.7"/><circle class="cut" cx="12" cy="11.4" r="0.7"/>` + STK(t, 'M13.2 13.6 H15 M8.8 15.6 H10.8', 1) : STK(t, 'M13.2 9 H14.8 M9.2 13 H10.8', 1))));

// ---- Kick: the front head, its hoop, the pedal and the spurs.
track('kickA', t => sil(t, `<circle class="f2" cx="12" cy="10.6" r="7.6"/>` + STK(t, 'M12 2.4 A8.2 8.2 0 1 1 11.99 2.4', t.S >= 30 ? 2 : 1.4) +
  F('M10.4 19.8 H13.6 V22.6 H10.4 Z') + STK(t, 'M5.6 16 L3.4 22.2 M18.4 16 L20.6 22.2', 1)));
track('kickB', t => sil(t, F('M12 2 A8.6 8.6 0 1 1 11.99 2 Z M12 4.2 A6.4 6.4 0 1 0 12.01 4.2 Z') + `<circle class="f2" cx="12" cy="10.6" r="5.2"/>` +
  F('M10.6 19.6 H13.4 V22.8 H10.6 Z') + STK(t, 'M5.2 16.4 L3.2 22.4 M18.8 16.4 L20.8 22.4', 1)));

// ---- Drum kit: the kick, a tom on it, a cymbal on its stand.
track('drumkitA', t => sil(t, `<circle class="f2" cx="10.4" cy="16.6" r="5.4"/>` + STK(t, 'M10.4 10.6 A6 6 0 1 1 10.39 10.6', 1.2) +
  F('M6.8 6.8 H13.8 V10.2 H6.8 Z') + `<ellipse class="f1" cx="10.3" cy="6.8" rx="3.5" ry="1.2"/>` +
  `<ellipse class="f1" cx="18.6" cy="4.6" rx="4.6" ry="1" transform="rotate(-14 18.6 4.6)"/>` + STK(t, 'M18.8 5.2 V22.6', 1, 's2')));

// ---- Strings: a violin, f-holes cut out; or a cello on its endpin.
const VIOLIN = 'M12 9 C14.6 9 16 10.2 16 12.2 C16 13.2 15 13.8 14.8 14.6 C14.7 15.2 15.4 15.6 16.2 16.4 C17 17.2 17.2 18.4 17.2 19.4 C17.2 21.8 15 23.4 12 23.4 C9 23.4 6.8 21.8 6.8 19.4 C6.8 18.4 7 17.2 7.8 16.4 C8.6 15.6 9.3 15.2 9.2 14.6 C9 13.8 8 13.2 8 12.2 C8 10.2 9.4 9 12 9 Z';
track('stringsA', t => sil(t, F(VIOLIN) + F('M11.25 2.4 H12.75 V9.4 H11.25 Z') + `<circle class="f1" cx="12" cy="1.9" r="1.5"/>` +
  (t.D ? CUT(t, 'M9.9 15.4 C9.4 16.6 10.5 17.6 9.9 18.8 M14.1 15.4 C14.6 16.6 13.5 17.6 14.1 18.8', 1) : CUT(t, 'M10 16 V18.4 M14 16 V18.4', 1))));
track('stringsB', t => sil(t, `<g transform="translate(0 -1.2) scale(1 0.92)">${F(VIOLIN)}</g>` + F('M11.25 1.6 H12.75 V8 H11.25 Z') + `<circle class="f1" cx="12" cy="1.4" r="1.3"/>` +
  STK(t, 'M12 21 V23.6', 1) + (t.D ? CUT(t, 'M9.9 14 C9.4 15.2 10.5 16.2 9.9 17.4 M14.1 14 C14.6 15.2 13.5 16.2 14.1 17.4', 1) : CUT(t, 'M10 14.6 V17 M14 14.6 V17', 1)) + STK(t, 'M3 9 L21 20', 1, 's2')));
