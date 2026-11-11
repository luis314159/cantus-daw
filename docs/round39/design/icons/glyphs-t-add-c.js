// Cantus icon family — set 6 additions, part C (icons-c, 2026-11-09): electronic & production,
// voice, notation, sound design & Foley, mix & routing, marks & shapes, letters & numbers.
// The same construction as set 6 (glyphs-t-tracks.js): one design on the 24-unit sheet, built
// for each size (`track(id, t => …)` with `tk(S)`), silhouettes through `sil()` with their
// features cut out, lines in device pixels (1 px, 2 from 30). Each section is its own scope so
// this file can load beside the other additions (-a, -b) without a name clash; the only
// globals it adds are N[id], ADDC (helpers), ADDC_GROUPS and ADDC_WORDS.
var N = window.N || (window.N = {});

window.ADDC = (() => {
  // The line inside a silhouette, in device pixels: a touch over 1 below 30 (as the library's
  // stems and straps), 2 from 30.
  const LW = t => (t.S >= 30 ? 2 : 1.2);
  // Bars on whole pixels, centred (or from x0, in 24 units): each [top, bottom, class] in
  // 24 units; the width and gap are device pixels for the size.
  const bars = (t, spans, o = {}) => {
    const bw = o.bw ? o.bw(t) : Math.max(1, Math.round(t.k * 1.8));
    const gap = o.gap ? o.gap(t) : Math.max(1, Math.round(t.k * 1.3));
    const total = spans.length * bw + (spans.length - 1) * gap;
    const x0 = o.x0 != null ? t.f(o.x0) : Math.floor((t.S - total) / 2);
    return spans.map(([y0, y1, c], i) => `<rect class="${c || 'f1'}" x="${x0 + i * (bw + gap)}" y="${t.f(y0)}" width="${bw}"` +
      ` height="${Math.max(1, t.f(y1) - t.f(y0))}"/>`).join('');
  };
  // A grid of cells on whole pixels, centred: `cell(S)` gives [cols, rows, size, gap];
  // `ink(c, r)` the class of each cell.
  const cells = (t, cell, ink, rx = 0) => {
    const [cols, rows, c, g] = cell(t.S);
    const w = cols * c + (cols - 1) * g, h = rows * c + (rows - 1) * g;
    const x0 = Math.floor((t.S - w) / 2), y0 = Math.floor((t.S - h) / 2);
    let s = '';
    for (let r = 0; r < rows; r++) for (let q = 0; q < cols; q++) {
      const cls = ink(q, r);
      if (cls) s += `<rect class="${cls}" x="${x0 + q * (c + g)}" y="${y0 + r * (c + g)}" width="${c}" height="${c}"${rx && c >= 4 ? ` rx="${rx}"` : ''}/>`;
    }
    return s;
  };
  // A wave as a dense polyline from x0 to x1 (24 units) around y, built at the size (curves are
  // not snapped; they never were in this family).
  const wave = (t, fn, x0, x1, c = 's1', steps = 60) => {
    let d = '';
    for (let i = 0; i <= steps; i++) {
      const x = x0 + (x1 - x0) * i / steps;
      d += (i ? ' L' : 'M') + (x * t.k).toFixed(2) + ' ' + (fn(i / steps) * t.k).toFixed(2);
    }
    return `<path class="${c}" fill="none" stroke-width="${t.w}" stroke-linejoin="round" stroke-linecap="round" d="${d}"/>`;
  };
  return { LW, bars, cells, wave };
})();

// ---- Electronic & production
(() => {
  const { LW, bars, cells, wave } = window.ADDC;
  // Theremin: the cabinet on its legs, the pitch rod standing at the right, the volume loop at
  // the left; from 16 px the hand's waves beside the rod.
  track('theremin', t => sil(t, `<g transform="translate(-0.6 2.4) scale(0.72)">${F('M7 12 H16.6 V17.2 C16.6 20.6 14.4 22.6 11.6 22.6 C8.8 22.6 7 20.6 7 17.8 Z')}` +
    `${RR(7, 4.8, 2.1, 9)}${RR(9.5, 3.2, 2.1, 10, 1)}${RR(12, 3.6, 2.1, 9.6, 1)}${RR(14.5, 5.2, 2.1, 8.4, 1)}<g transform="rotate(-38 6 15)">${RR(4.8, 9.4, 2.3, 7, 1.1)}</g></g>` +
    STK(t, 'M19.8 1.4 V17.6', LW(t)) + RR(14, 17.4, 9.4, 5, 0.8) + (t.D ? STK(t, 'M14.6 6.4 C15.6 7.6 15.6 9.8 14.6 11', 1, 's2') : '') +
    (t.D === 2 ? STK(t, 'M16.8 5 C18.2 6.8 18.2 10.6 16.8 12.4', 1, 's2') : '')));
  // Vocoder: a voice (the capsule) turned into a filter bank's bands.
  track('vocoder', t => sil(t, RR(2.4, 2.6, 6.4, 11, 3.2) + (t.D ? CUT(t, 'M3.8 6 H7.4 M3.8 8.6 H7.4', 0.9) : '') +
    STK(t, 'M5.6 13.6 V20.6', LW(t)) + STK(t, 'M2.8 21 H8.4', LW(t))) +
    bars(t, [[10, 20.6], [5, 20.6], [12.6, 20.6, 'f2'], [7.6, 20.6]], { x0: 11 }));
  // Talk box: the lips, the tube from the box to the mouth, the box with its driver.
  track('talkbox', t => sil(t, `<g transform="translate(0.2 -2.2) scale(0.62)">${F('M2.6 12 C5.6 7.4 8.8 7.8 12 9.4 C15.2 7.8 18.4 7.4 21.4 12 C18.4 16.8 15.2 18 12 18 C8.8 18 5.6 16.8 2.6 12 Z')}` +
    `${t.D ? CUT(t, 'M4.4 12 C8 11 16 11 19.6 12', 1 / 0.62) : ''}</g>` + STK(t, 'M13.8 5.2 C19.4 5.2 20.6 9.6 18.2 14.4', LW(t)) +
    RR(13, 14.4, 9.4, 8, 1) + `<circle class="cut" cx="17.7" cy="18.4" r="${t.D ? 2 : 1.8}"/>` + (t.D === 2 ? C1(17.7, 18.4, 0.8) : '')));
  // Groovebox: one box with its own speaker, a screen and pads.
  track('groovebox', t => sil(t, RR(1.4, 4, 21.2, 16, 1.6) + `<circle class="cut" cx="7.2" cy="12" r="${t.D ? 3.9 : 3.6}"/>` + C1(7.2, 12, t.D ? 1.6 : 1.4) +
    RR(12.4, 6.4, 8, 2.4, 0.3, 'cut') + RR(12.4, 10.6, 3.4, 3, 0.4, 'cut') + RR(17, 10.6, 3.4, 3, 0.4, 'cut') +
    (t.D ? RR(12.4, 14.6, 3.4, 3, 0.4, 'cut') + RR(17, 14.6, 3.4, 3, 0.4, 'cut') : '')));
  // Step sequencer: lanes of steps, lit (ink) and unlit (second ink).
  const PAT = ['10101', '01001', '00110'];
  track('stepseq', t => cells(t, S => S < 16 ? [4, 3, 2, 1] : S < 24 ? [5, 3, 2, 1] : S < 30 ? [5, 3, 3, 1] : S < 36 ? [5, 3, 4, 2] : [5, 3, 5, 2],
    (c, r) => (PAT[r][c] === '1' ? 'f1' : 'f2'), 1));
  // Acid bassline (303-style, no brand): a flat box, a row of knobs, a row of key buttons.
  track('acid', t => sil(t, RR(1.4, 5, 21.2, 14, 1.4) + [4.6, 8, 11.4, 14.8, 18.2].map(x => `<circle class="cut" cx="${x + (t.D ? 0 : 0)}" cy="9" r="${t.D ? 1.2 : 1.3}"/>`).join('') +
    (t.D ? [3.6, 6.4, 9.2, 12, 14.8, 17.6].map((x, i) => RR(x, 13.4, 2, 2.6, 0.3, 'cut')).join('') + RR(20.2, 13.4, 1.4, 2.6, 0.3, 'cut') : CUT(t, 'M4 14.8 H20', 1))));
  // Drum pads: the pad grid alone (the sampler is a box with a screen).
  track('padgrid', t => cells(t, S => S < 16 ? [3, 3, 3, 1] : S < 24 ? [4, 4, 3, 1] : S < 30 ? [4, 4, 4, 1] : S < 36 ? [4, 4, 5, 2] : [4, 4, 6, 2],
    (c, r) => ((c === 1 && r === 1) || (c === 2 && r === 0) ? 'f2' : 'f1'), 1));
  // Laptop: the open lid and the base; from 16 px a waveform on the screen.
  track('laptop', t => sil(t, RR(4, 4.2, 16, 12, 1) + RR(5.6, 5.8, 12.8, 8.8, 0.2, 'cut') + F('M1.4 17.4 H22.6 L21.2 20 H2.8 Z') +
    (t.D ? STK(t, 'M7.4 10.6 L9 8.4 L10.8 12.6 L12.6 7.6 L14.4 11.6 L16.4 9.6', 1, 's1') : '')));
  // DJ controller: two jog wheels and the mixer between them.
  track('djcontroller', t => sil(t, RR(1.2, 6, 21.6, 12, 1.6) + [6.4, 17.6].map(x => `<circle class="cut" cx="${x}" cy="12" r="3.9"/>` + C1(x, 12, t.D ? 1.4 : 1.6)).join('') +
    CUT(t, 'M12 8.2 V15.8', t.D ? 1 : 0.9)));
  // Media player (CDJ): one big jog wheel under a screen.
  track('cdj', t => sil(t, RR(2, 2, 20, 20, 1.6) + RR(4.4, 4, 10.6, 2.8, 0.3, 'cut') + `<circle class="cut" cx="9.6" cy="14" r="5.6"/>` +
    C1(9.6, 14, 3.7) + (t.D ? `<circle class="cut" cx="9.6" cy="14" r="0.9"/>` : '') + CUT(t, 'M18.4 9.6 V19.2', t.S >= 30 ? 2 : 1) + RR(17, 13.2, 2.8, 1.6, 0.3) +
    (t.D ? `<circle class="cut" cx="18.4" cy="5.4" r="1.1"/>` : '')));
  // Cassette: the shell, two reels, the window between them, the head opening below.
  track('cassette', t => sil(t, RR(1.4, 4.4, 21.2, 15.2, 1.4) + RR(4.2, 7.2, 15.6, 6.8, 1, 'cut') + C1(8, 10.6, 2.4) + C1(16, 10.6, 2.4) +
    (t.D ? `<circle class="cut" cx="8" cy="10.6" r="0.9"/><circle class="cut" cx="16" cy="10.6" r="0.9"/>` : '') + (t.D === 2 ? CUT(t, 'M6.4 19.6 L7.6 17 H16.4 L17.6 19.6', 1) : '')));
  // Radio: the antenna, the speaker, the dial.
  track('radio', t => sil(t, STK(t, 'M14.4 6.4 L20.6 1.4', LW(t)) + RR(1.6, 6.4, 20.8, 14.8, 1.6) + `<circle class="cut" cx="7.6" cy="13.8" r="4.1"/>` + C1(7.6, 13.8, 1.6) +
    CUT(t, 'M14.2 10.4 H19.8', 1) + (t.D ? CUT(t, 'M14.2 13.4 H19.8', 1) + `<circle class="cut" cx="17" cy="17.4" r="1.3"/>` : `<circle class="cut" cx="17" cy="16" r="1.5"/>`)));
  // Chiptune / 8-bit: a game controller, its cross and two buttons.
  track('chiptune', t => sil(t, F('M6.4 6.6 H17.6 C20.8 6.6 22.6 9.4 22.8 13 C23 16.6 21.8 19.4 19.6 19.4 C17.8 19.4 16.8 17.8 15.8 16.2 H8.2 C7.2 17.8 6.2 19.4 4.4 19.4 C2.2 19.4 1 16.6 1.2 13 C1.4 9.4 3.2 6.6 6.4 6.6 Z') +
    CUT(t, 'M4.6 12.4 H8.8 M6.7 10.3 V14.5', t.S >= 30 ? 2 : 1.1) + `<circle class="cut" cx="16" cy="13.6" r="1.2"/><circle class="cut" cx="18.6" cy="11" r="1.2"/>`));
  // Glitch: a block torn into bands, slipped sideways; two loose pixels.
  track('glitch', t => t.R(4, 3, 15, 7) + t.R(8, 7, 20, 10) + t.R(3, 10, 13, 13) + t.R(9, 13, 21, 17) + t.R(5, 17, 16, 21) +
    t.R(17, 4, 19.5, 6, 'f2') + t.R(2, 14, 4.5, 16, 'f2') + (t.D ? t.R(18, 19, 20, 21, 'f2') : ''));
  // The wave shapes: one cycle each, from the axis to the axis, the same height.
  track('waveSine', t => wave(t, u => 12 - 7.6 * Math.sin(u * 2 * Math.PI), 2.6, 21.4));
  track('waveSaw', t => t.Poly([[2.6, 12], [12, 4.4], [12, 19.6], [21.4, 12]]));
  track('waveSquare', t => t.Poly([[2.6, 12], [2.6, 4.4], [12, 4.4], [12, 19.6], [21.4, 19.6], [21.4, 12]]));
  track('waveTri', t => t.Poly([[2.6, 12], [7.3, 4.4], [16.7, 19.6], [21.4, 12]]));
})();

// ---- Voice
(() => {
  const { LW } = window.ADDC;
  // A stage mic on its stand: ball head, tapered handle, the pole and its foot.
  const stageMic = (t, x, s, c = 'f1') => `<g transform="translate(${x} 0) scale(${s}) translate(-12 ${24 / s - 24})">` +
    C1(12, 5.2, 3.6, c) + F('M10.2 8.2 H13.8 L12.9 14.2 H11.1 Z', c) + (t.D && c === 'f1' ? CUT(t, 'M9.6 4.4 H14.4 M9.6 6.4 H14.4', 0.8 / s) : '') +
    STK(t, 'M12 14 V21.6', LW(t) / s, c === 'f1' ? 's1' : 's2') + F('M7.6 21.2 H16.4 V22.8 H7.6 Z', c) + '</g>';
  // Lead vocal: the stage mic, and the lead's spark.
  track('leadVocal', t => sil(t, stageMic(t, 10.4, 1) +
    (t.D ? F('M19.4 2 L20.2 4.6 L22.8 5.4 L20.2 6.2 L19.4 8.8 L18.6 6.2 L16 5.4 L18.6 4.6 Z') : F('M19 3 L20 5.4 L22.4 6.4 L20 7.4 L19 9.8 L18 7.4 L15.6 6.4 L18 5.4 Z'))));
  // Backing vocals: two stage mics, the second behind.
  track('backingVocals', t => sil(t, stageMic(t, 7, 0.86, 'f2') + stageMic(t, 15.6, 1)));
  // Rap / MC: a handheld mic, tilted, its cord swinging.
  track('rap', t => sil(t, `<g transform="translate(14.2 8.4) rotate(32)">${C1(0, -3.6, 3.8)}${F('M-2.1 -0.4 H2.1 L1.2 8.6 H-1.2 Z')}` +
    `${t.D ? CUT(t, 'M-2.6 -4.6 H2.6 M-2.6 -2.6 H2.6', 0.8) : ''}</g>` + STK(t, 'M9.6 15.8 C8.2 19.4 3.6 17.6 3.4 20.6 C3.2 22.6 6.6 22.8 8.4 22', 1, 's2')));
  // Harmony: two notes sounding together, ringing.
  const head = (x, y, rx = 3.4, ry = 2.4, c = 'f1') => `<ellipse class="${c}" cx="${x}" cy="${y}" rx="${rx}" ry="${ry}" transform="rotate(-22 ${x} ${y})"/>`;
  track('harmony', t => sil(t, head(8.6, 19.4) + head(8.6, 12.8, 3.4, 2.4, t.D ? 'f1' : 'f1') + STK(t, 'M11.7 18.6 V2', t.S >= 30 ? 2 : 1.3) +
    STK(t, 'M15.4 9 C16.8 10.6 16.8 13.4 15.4 15', 1, 's1') + (t.D ? STK(t, 'M18.4 6.6 C20.8 9.4 20.8 14.6 18.4 17.4', 1, 's2') : '')));
  // Vocal chop: the lips, sliced.
  const LIPS = 'M2.6 12 C5.6 7.4 8.8 7.8 12 9.4 C15.2 7.8 18.4 7.4 21.4 12 C18.4 16.8 15.2 18 12 18 C8.8 18 5.6 16.8 2.6 12 Z';
  track('vocalChop', t => sil(t, `<g transform="translate(0 12) scale(1 1.15) translate(0 -12.6)">${F(LIPS)}${CUT(t, 'M4.4 12 C8 11 16 11 19.6 12', 1 / 1.15)}</g>` +
    CUT(t, 'M8.6 3.6 L6.8 21 M16.6 3 L14.8 20.4', t.S >= 30 ? 3 : t.D ? 1.6 : 1.4)));
  // Ad-lib: a call-out bubble with its exclamation cut out.
  const BUBBLE = 'M12 2.6 C17.6 2.6 21.6 6 21.6 10.4 C21.6 14.8 17.6 18.2 12 18.2 C11 18.2 10 18.1 9.1 17.9 L4.2 21.4 L5.4 16.4 C3.4 15 2.4 12.9 2.4 10.4 C2.4 6 6.4 2.6 12 2.6 Z';
  track('adlib', t => sil(t, F(BUBBLE) + RR(10.9, 5.2, 2.2, 7.2, 1, 'cut') + `<circle class="cut" cx="12" cy="14.8" r="1.25"/>`));
  // Whisper: the bubble drawn in a broken line, three quiet dots in it.
  track('whisper', t => sil(t, `<path class="s1" fill="none" stroke-width="${px(t, t.S >= 30 ? 2 : 1)}" stroke-dasharray="${px(t, t.S >= 30 ? 3 : 2)} ${px(t, t.S >= 30 ? 2 : 1)}" d="${BUBBLE}"/>` +
    [7.8, 12, 16.2].map(x => C1(x, 10.4, t.D ? 1.1 : 1.3, 'f2')).join('')));
  // Voice-over / podcast: a broadcast mic in its mount on a boom arm.
  track('podcast', t => sil(t, RR(3.4, 1.6, 6.6, 11.4, 3.3) + (t.D ? CUT(t, 'M4.4 5 H9 M4.4 7.4 H9', 0.8) : '') +
    STK(t, 'M2 7.6 V9 C2 12.6 11.4 12.6 11.4 9 V7.6', 1) + STK(t, 'M6.7 13.2 V15.2 L14.4 9 L20.8 18.6', LW(t)) + RR(19.2, 18.4, 3.6, 4, 0.6)));
  // Talkback: a desk mic on its gooseneck, the press-to-talk button on its base.
  track('talkback', t => sil(t, RR(2.6, 17.6, 13.6, 4.8, 1) + `<circle class="cut" cx="6.4" cy="20" r="${t.D ? 1.3 : 1.2}"/>` +
    STK(t, 'M12.6 17.6 C12.6 11.4 15 7.6 18.4 6.4', LW(t)) + `<g transform="translate(19.2 5.8) rotate(-62)">${RR(-1.7, -3.8, 3.4, 7.2, 1.7)}</g>` +
    (t.D ? STK(t, 'M14.6 2.4 C13.2 3 12.2 4 11.6 5.4', 1, 's2') : '')));
})();

// ---- Notation
(() => {
  const { LW } = window.ADDC;
  // Treble clef (clave de sol): the tail's ball, the spine up past the top, the curl back down
  // and the spiral round the G line.
  const G_A = 'M9.4 19.6 C9.6 22.2 13.4 22.6 13.4 19.4 L11.4 4 C11.2 1.8 14 0.8 14.6 3.2 C15.4 6.4 6.6 9.2 6.6 14.4 C6.6 17.4 9 18.8 11.6 18.8 C14.6 18.8 16.6 16.8 16.6 14.4 C16.6 12 14.8 10.6 12.6 10.6 C10.6 10.6 9.4 12 9.6 13.6 C9.8 15 11 15.8 12.2 15.6';
  track('trebleClef', t => sil(t, STK(t, G_A, t.S >= 30 ? 2 : 1.25) + C1(9.6, 19.8, t.D ? 1.6 : 1.7)));
  // C clef (clave de do): the heavy bar, the hairline, two bows meeting at the middle line.
  track('cClef', t => sil(t, RR(4, 2.6, 3, 18.8) + STK(t, 'M8.8 2.6 V21.4', 1) +
    STK(t, 'M9.4 12 L11.2 9.4 C12 10.6 13.6 10.8 15.2 10.2 C17.8 9.2 18 4.6 15.6 3.6 C14 3 12.6 3.6 12.4 5.2', LW(t)) + C1(13.4, 5.4, 1.5) +
    STK(t, 'M9.4 12 L11.2 14.6 C12 13.4 13.6 13.2 15.2 13.8 C17.8 14.8 18 19.4 15.6 20.4 C14 21 12.6 20.4 12.4 18.8', LW(t)) + C1(13.4, 18.6, 1.5)));
  // Percussion clef: two bars on the staff.
  track('percClef', t => t.L(2, 6, 22, 6, 's2') + t.L(2, 12, 22, 12, 's2') + t.L(2, 18, 22, 18, 's2') + t.R(7.6, 7.6, 10.4, 16.4) + t.R(13.6, 7.6, 16.4, 16.4));
  // Notes: heads tilted as engraved, stems on the right.
  const head = (x, y, rx = 3.9, ry = 2.7, c = 'f1') => `<ellipse class="${c}" cx="${x}" cy="${y}" rx="${rx}" ry="${ry}" transform="rotate(-22 ${x} ${y})"/>`;
  track('quarterNote', t => sil(t, head(9.4, 18.4) + STK(t, 'M12.9 17.4 V2.4', t.S >= 30 ? 2 : 1.3)));
  track('eighthNotes', t => sil(t, head(6.2, 19, 3.3, 2.4) + head(16.8, 17.4, 3.3, 2.4) + STK(t, 'M9.2 18.2 V4.6 M19.8 16.6 V3', t.S >= 30 ? 2 : 1.2) +
    F('M8.6 3.6 L20.4 1.8 V5.2 L8.6 7 Z')));
  track('wholeNote', t => sil(t, `<ellipse class="f1" cx="12" cy="12" rx="8" ry="5.4"/><ellipse class="cut" cx="12" cy="12" rx="2.9" ry="4.2" transform="rotate(-38 12 12)"/>`));
  // Rest (a quarter rest): the slant, the heavy stroke, the hook.
  track('rest', t => sil(t, STK(t, 'M10.4 2.4 L14.6 7.6 C11.6 10.2 11.4 12 14.6 16 C11.6 14.8 9.4 16.4 12.4 21.6', t.S >= 30 ? 3 : t.D ? 2 : 1.7)));
  track('sharp', t => t.L(9.4, 3.4, 9.4, 21.6) + t.L(14.6, 2.4, 14.6, 20.6) + t.Pf('M5.4 9.4 L18.6 6.6 V9.6 L5.4 12.4 Z') + t.Pf('M5.4 16.4 L18.6 13.6 V16.6 L5.4 19.4 Z'));
  track('flat', t => sil(t, RR(6, 1.8, 2, 20) + F('M7.6 12.6 C10.8 9.8 17.4 9.4 17.4 13.6 C17.4 16.8 13 19.4 7.6 21.8 Z') +
    F('M9.4 15 C11.4 13.2 14.6 12.6 14.6 14.4 C14.6 16 12.4 17.8 9.4 19.2 Z', 'cut')));
  track('fermata', t => sil(t, F('M2.4 17.2 C2.4 4.4 21.6 4.4 21.6 17.2 H19.4 C19.4 8.4 4.6 8.4 4.6 17.2 Z') + C1(12, 15.4, 2)));
  // Tuning fork: the prongs, the bend, the stem; from 24 px it rings.
  track('tuningFork', t => sil(t, F('M7.6 1.8 H10.2 V11.4 C10.2 12.8 13.8 12.8 13.8 11.4 V1.8 H16.4 V11.8 C16.4 14.4 14.6 15.8 13.2 16 V22.4 H10.8 V16 C9.4 15.8 7.6 14.4 7.6 11.8 Z') +
    (t.D === 2 ? STK(t, 'M4.6 3.4 C3.4 5 3.4 7.4 4.6 9 M19.4 3.4 C20.6 5 20.6 7.4 19.4 9', 1, 's2') : '')));
  // Sheet music: a page, its corner folded, a staff and notes on it.
  track('scoreSheet', t => sil(t, `<path class="s1" fill="none" stroke-width="${px(t, t.S >= 30 ? 2 : 1)}" stroke-linejoin="round" d="M4.6 1.8 H14.8 L19.4 6.4 V22.2 H4.6 Z M14.8 1.8 V6.4 H19.4"/>`) +
    t.L(7, 10, 17, 10, 's2') + t.L(7, 13, 17, 13, 's2') + t.L(7, 16, 17, 16, 's2') + (t.D === 2 ? t.L(7, 19, 17, 19, 's2') : '') +
    sil(t, head(11, 17.2, 2.2, 1.6) + STK(t, 'M12.9 16.6 V8.4', t.S >= 30 ? 2 : 1)));
  // Click / metronome: the tapered body, its rod and weight cut out.
  track('metronomeTrack', t => sil(t, F('M8.8 1.8 H15.2 L19.8 21.8 Q19.9 22.4 19.2 22.4 H4.8 Q4.1 22.4 4.2 21.8 Z') + CUT(t, 'M12 18.6 L16.4 5.8', t.S >= 30 ? 2 : 1.1) +
    `<g transform="rotate(19 15.2 9.6)">${RR(13.8, 8.6, 2.8, 2, 0.3, 'cut')}</g>` + CUT(t, 'M6.4 18.8 H17.6', t.S >= 30 ? 2 : 1)));
})();

// ---- Sound design & Foley
(() => {
  const { LW, bars, wave } = window.ADDC;
  // Downlifter: the riser turned over — the fall and its head.
  track('downlifter', t => t.Ps('M3 4 C11 4 16 9 19.5 19') + t.Ps('M16 17.5 L20 20.5 L22 16') + (t.D ? t.C(5, 8.5, 0.9, 'f2') + t.C(9, 11.5, 0.9, 'f2') : ''));
  // Sweep: a tone whose pitch rises (a chirp).
  track('sweep', t => wave(t, u => 12 - 7.4 * Math.sin(2 * Math.PI * (0.8 * u + 1.5 * u * u)), 2.4, 21.6, 's1', 120));
  // Whoosh: three air lines passing, the long one in ink.
  track('whoosh', t => t.Ps('M2.4 18 C9.6 18 15.4 13.6 21.6 4.6') + t.Ps('M7.4 21.4 C12.6 21.4 17 18.8 20.4 14.4', 's2') +
    t.Ps('M2.4 12.6 C7 12.6 10.6 10.4 13.4 6.6', 's2'));
  // Sub drop: the fall, and the slow wave it lands in.
  track('subDrop', t => sil(t, F('M10.4 1.6 H13.6 V8.4 H17.4 L12 14 L6.6 8.4 H10.4 Z') +
    STK(t, 'M1.8 19.2 C5.4 15.2 8.6 15.2 12 19.2 C15.4 23.2 18.6 23.2 22.2 19.2', t.S >= 30 ? 2 : 1.3)));
  // Reverse: a sound that swells to its cut, and the arrow back.
  track('reverse', t => t.Poly([[7.4, 8.8], [3.2, 12], [7.4, 15.2]]) + t.L(3.6, 12, 9.4, 12) +
    bars(t, [[10.6, 13.4], [9, 15], [7, 17], [4.6, 19.4], [2.4, 21.6]], { x0: 11.6, bw: t => Math.max(1, Math.round(t.k * 1.6)), gap: t => Math.max(1, Math.round(t.k * 0.9)) }));
  // Stutter: the same slice, three times.
  track('stutter', t => [2.6, 9.6, 16.6].map(x => t.R(x, 5, x + 2, 19) + t.R(x + 3, 9, x + 5, 15, 'f2')).join(''));
  // Tape stop: the reel, and the pitch falling away from it.
  track('tapeStop', t => sil(t, C1(9, 9.4, 7.2) + [90, 210, 330].map(a => `<circle class="cut" cx="${(9 + 3.8 * Math.cos(a * Math.PI / 180)).toFixed(2)}" cy="${(9.4 - 3.8 * Math.sin(a * Math.PI / 180)).toFixed(2)}" r="1.5"/>`).join('') +
    `<circle class="cut" cx="9" cy="9.4" r="0.9"/>` + STK(t, 'M16.6 9.4 C19.2 10.6 20.8 14.4 21 20.6', LW(t)) + STK(t, 'M18.6 18.4 L21 21 L23.2 18.2', LW(t))));
  // Vinyl crackle: a record and its dust.
  track('vinyl', t => sil(t, C1(10, 14, 8.6) + `<circle class="scut" fill="none" stroke-width="${px(t, 1)}" cx="10" cy="14" r="5.6"/>` +
    `<circle class="cut" cx="10" cy="14" r="2.4"/>` + C1(10, 14, 0.8) + C1(20.4, 3.4, 1.2) + C1(22.4, 8.2, 0.9) + (t.D ? C1(16.4, 1.8, 0.8) : '')));
  // Scratch: a record and the hand's back-and-forth.
  track('scratch', t => sil(t, C1(9.4, 14.6, 8.2) + `<circle class="cut" cx="9.4" cy="14.6" r="2.4"/>` + C1(9.4, 14.6, 0.8) +
    (t.D ? `<circle class="scut" fill="none" stroke-width="${px(t, 1)}" cx="9.4" cy="14.6" r="5.2"/>` : '') +
    STK(t, 'M13.6 2.6 C18.4 2.6 21.4 5.6 21.4 10.4', LW(t)) + STK(t, 'M15.8 0.6 L13.6 2.6 L15.8 4.6 M19.4 8.2 L21.4 10.4 L23.4 8.2', LW(t))));
  // Footsteps / Foley: two prints, one ahead.
  const print = (x, y, r) => `<g transform="translate(${x} ${y}) rotate(${r})"><ellipse class="f1" cx="0" cy="-2.6" rx="2.8" ry="4.4"/><ellipse class="f1" cx="0" cy="4.6" rx="2.2" ry="2.3"/></g>`;
  track('footsteps', t => sil(t, print(7.2, 14.4, -10) + print(16.8, 8.8, 10)));
  // Ambience: a landscape — the hills, and the sun over them.
  track('ambience', t => sil(t, F('M1.4 20.6 L8.4 9.2 L12.4 14.8 L15.4 11.2 L22.6 20.6 Z') + C1(17.8, 5, 2.6) + (t.D ? CUT(t, 'M6.4 12.6 L8.4 9.4 L10.2 12', 1) : '')));
  // Rain: a cloud and its drops.
  const CLOUD = 'M6.6 14 C3.8 14 2.2 12.2 2.2 10.2 C2.2 8 4 6.6 6.2 6.8 C6.8 4.2 9 2.6 11.8 2.6 C14.8 2.6 16.8 4.6 17.2 7 C19.8 6.8 21.8 8.6 21.8 10.6 C21.8 12.6 20.2 14 17.8 14 Z';
  track('rain', t => sil(t, F(CLOUD) + STK(t, 'M7.6 17 L6.2 20.8 M12.6 17 L11.2 20.8 M17.6 17 L16.2 20.8', LW(t))));
  // Wind: the air's lines, curling.
  track('wind', t => t.Ps('M2.6 8.6 H14.4 C17 8.6 18 6.6 17.4 5 C16.8 3.4 14.6 3.2 13.6 4.6') + t.Ps('M2.6 12.6 H18.6 C21 12.6 22.2 14.6 21.4 16.4 C20.6 18 18.4 18.2 17.4 16.8') +
    t.Ps('M2.6 16.6 H11', 's2'));
  // Water: a drop and its light.
  track('water', t => sil(t, F('M12 1.6 C12 1.6 4.6 10.4 4.6 15 C4.6 19.2 8 22.4 12 22.4 C16 22.4 19.4 19.2 19.4 15 C19.4 10.4 12 1.6 12 1.6 Z') +
    (t.D ? CUT(t, 'M8.4 15.2 C8.4 17.4 9.6 18.8 11.4 19.2', t.S >= 30 ? 2 : 1.1) : '')));
  // Fire: the flame and its heart.
  track('fire', t => sil(t, F('M12 1.4 C12.8 5.8 18.8 8.4 18.8 15 C18.8 19.4 15.6 22.4 12 22.4 C8.4 22.4 5.2 19.4 5.2 15 C5.2 11.6 7.2 9.6 8.2 7.4 C9.4 9.4 9.8 10.8 10.6 11.6 C12 8.8 12.6 5.6 12 1.4 Z') +
    (t.D ? F('M12 21 C10.2 21 8.8 19.8 8.8 17.8 C8.8 15.6 10.6 14.6 11.4 12.6 C12.6 14.6 15.2 15.8 15.2 18 C15.2 19.8 13.8 21 12 21 Z', 'cut') : '')));
  // Birds: two gulls, one farther off.
  const GULL = 'M-5 0.6 C-3.6 -2.2 -1.6 -2.4 0 0.8 C1.6 -2.4 3.6 -2.2 5 0.6 C3.4 -0.6 1.6 -0.2 0 2.6 C-1.6 -0.2 -3.4 -0.6 -5 0.6 Z';
  track('birds', t => sil(t, `<g transform="translate(9.4 9) scale(2.1 2.3)">${F(GULL)}</g><g transform="translate(18.2 17.2) scale(1.3 1.5)">${F(GULL, 'f2')}</g>`));
  // Crowd / applause: the audience, one cheering with both arms up.
  const person = (x, y, s, c) => `<g transform="translate(${x} ${y}) scale(${s})">${C1(0, -4.2, 2.4, c)}${F('M-4 5 C-4 0.6 -2.4 -1.2 0 -1.2 C2.4 -1.2 4 0.6 4 5 Z', c)}</g>`;
  track('crowd', t => sil(t, person(4.2, 17.4, 0.9, 'f2') + person(19.8, 17.4, 0.9, 'f2') + person(12, 17.4, 1.05, 'f1') +
    STK(t, 'M9 16 L6.6 6.4 M15 16 L17.4 6.4', LW(t)) + C1(6.4, 5.6, 1.3) + C1(17.6, 5.6, 1.3)));
  // Siren: the beacon on its base, and its light.
  track('siren', t => sil(t, F('M6.4 18 V12.6 C6.4 9 9 6.8 12 6.8 C15 6.8 17.6 9 17.6 12.6 V18 Z') + RR(4, 18.2, 16, 4, 0.8) +
    (t.D ? CUT(t, 'M9.4 15.6 V12.8 C9.4 11.2 10.2 10.2 11.4 9.8', 1) : '') + STK(t, 'M12 1.4 V3.8 M4.2 4.6 L5.8 6.2 M19.8 4.6 L18.2 6.2 M1.4 12.2 H3.6 M20.4 12.2 H22.6', LW(t))));
  // Explosion: the bomb, its fuse lit.
  track('explosion', t => sil(t, C1(10.2, 14.4, 7.6) + `<g transform="rotate(45 15.6 8.8)">${RR(13.8, 7.2, 3.6, 3.2, 0.4)}</g>` +
    STK(t, 'M17 7.6 C17.8 5.4 19.2 4.6 20.2 3.8', LW(t)) + STK(t, 'M21.4 0.6 V1.8 M21.4 4.6 V5.8 M18.8 3.2 H20 M22.8 3.2 H24', 1) +
    (t.D ? CUT(t, 'M5.8 13.6 C5.8 11.4 7.2 9.8 9.2 9.4', t.S >= 30 ? 2 : 1.1) : '')));
  // Phone / ringtone: the phone, ringing.
  track('phone', t => sil(t, RR(7, 2, 10, 20, 2) + (t.D ? RR(8.6, 4.6, 6.8, 12.8, 0.4, 'cut') : RR(8.8, 4.6, 6.4, 12.4, 0.4, 'cut')) +
    `<circle class="cut" cx="12" cy="19.6" r="${t.D ? 0.9 : 1}"/>` +
    STK(t, 'M19.6 7.6 C20.8 8.8 20.8 11.2 19.6 12.4 M4.4 7.6 C3.2 8.8 3.2 11.2 4.4 12.4', LW(t))));
  // Field recording: the handheld recorder, its two capsules crossed.
  track('fieldRec', t => sil(t, RR(6.4, 10.6, 11.2, 12, 1.6) + RR(8.4, 12.8, 7.2, 3.6, 0.4, 'cut') + `<circle class="cut" cx="12" cy="19.6" r="1.2"/>` +
    F('M6.4 10.2 C6.4 4.6 8.8 1.8 12 1.8 C15.2 1.8 17.6 4.6 17.6 10.2 Z') + CUT(t, t.D ? 'M8.6 4.8 L15.4 9.2 M15.4 4.8 L8.6 9.2' : 'M9 6 L15 9.6 M15 6 L9 9.6', t.S >= 30 ? 2 : 1)));
  // Bell / notification: the bell and its clapper.
  track('bell', t => sil(t, F('M12 2.2 C12.8 2.2 13.4 2.8 13.4 3.6 C16.4 4.4 18 7 18 10.2 V14.4 L20.2 17.6 H3.8 L6 14.4 V10.2 C6 7 7.6 4.4 10.6 3.6 C10.6 2.8 11.2 2.2 12 2.2 Z') +
    C1(12, 19.8, 2.2) + (t.D ? CUT(t, 'M8.8 13.6 V10.4 C8.8 8.8 9.6 7.6 10.8 7', 1) : '')));
})();

// ---- Mix & routing
(() => {
  const { LW, wave } = window.ADDC;
  // Group / folder: a folder holding its tracks.
  track('groupFolder', t => sil(t, F('M2 5.4 C2 4.6 2.6 4 3.4 4 H9.2 L11.2 6.4 H20.6 C21.4 6.4 22 7 22 7.8 V18.6 C22 19.4 21.4 20 20.6 20 H3.4 C2.6 20 2 19.4 2 18.6 Z') +
    CUT(t, t.D ? 'M5.4 11 H18.6 M5.4 14.6 H18.6' : 'M5.4 11.4 H18.6 M5.4 15.2 H18.6', t.S >= 30 ? 2 : 1)));
  // Aux: the signal runs on; a branch leaves it for the aux bus.
  track('aux', t => t.L(2, 18, 22, 18, 's2') + t.Ps('M5.6 18 C5.6 11.4 8.4 8.4 13 8.4') + t.C(17.4, 8.4, 4.6, 'f1') + (t.D ? t.C(17.4, 8.4, 1.4, 'cut') : ''));
  // Sidechain key: the key — its bow, its shaft, two teeth.
  track('sidechain', t => sil(t, C1(7, 12, 5) + `<circle class="cut" cx="7" cy="12" r="2.1"/>` + RR(11.4, 10.9, 10.8, 2.2) + RR(16.6, 13, 2, 3.6) + RR(20.2, 13, 2, 5)));
  // Compressor: the transfer curve, bending over at the threshold.
  track('compressor', t => t.Poly([[3, 2.6], [3, 21], [21.4, 21]], 's2') + t.Ps('M3 21 L11.6 12.4 C13.6 10.4 16.2 9.4 21.4 8.2') + (t.D === 2 ? t.C(11.6, 12.4, 1.3, 'f1') : ''));
  // EQ: a bell and a cut on the curve, their nodes.
  track('eq', t => (t.D ? t.L(2.4, 12, 21.6, 12, 's2') : '') + t.Ps('M2.4 15 C6 15 7 6.4 10 6.4 C13 6.4 13.4 17.6 16.6 17.6 C19 17.6 19.6 12.6 21.6 12.6') +
    (t.D ? t.C(10, 6.4, 1.6, 'f1') + t.C(16.6, 17.6, 1.6, 'f1') : ''));
  // Limiter: the ceiling, and the wave flattened under it.
  track('limiter', t => t.R(2, 3, 22, 5.4) + t.Ps('M2.4 16 C3.4 11.6 4.4 8 6 8 H8.8 C10.2 8 11.4 12 12 15 C12.8 18.8 13.6 21.2 15 21.2 C16.4 21.2 17.2 18.8 18 15 C18.6 12 19.8 8 21 8 H21.6'));
  // Saturator / distortion: a valve, its filament.
  track('saturator', t => sil(t, F('M7.4 19 V9.2 C7.4 4.6 9.6 1.8 12 1.8 C14.4 1.8 16.6 4.6 16.6 9.2 V19 Z') + RR(5.8, 19.2, 12.4, 3.2, 0.6) +
    CUT(t, t.D ? 'M10 16.6 V11.2 L12 8.4 L14 11.2 V16.6' : 'M12 8.4 V16.4', t.S >= 30 ? 2 : 1)));
  // Filter: a low-pass with its resonance.
  track('filterFx', t => t.Ps('M2.4 9.6 H10.4 C12.8 9.6 13.4 5.4 15.2 5.4 C16.8 5.4 17.6 12 20.8 20.6') + (t.D ? t.L(2.4, 20.6, 21.6, 20.6, 's2') : ''));
  // Chorus: one voice and its two detuned copies.
  track('chorus', t => wave(t, u => 8.4 - 4.4 * Math.sin(2 * Math.PI * (u - 0.08)), 2.4, 21.6, 's2') + (t.D ? wave(t, u => 15.6 - 4.4 * Math.sin(2 * Math.PI * (u + 0.08)), 2.4, 21.6, 's2') : '') +
    wave(t, u => 12 - 4.4 * Math.sin(2 * Math.PI * u), 2.4, 21.6, 's1'));
  // Phaser / flanger: the comb — notches through the spectrum.
  track('phaser', t => t.Poly([[2.4, 6], [4.2, 6], [5.8, 18], [7.4, 6], [10.2, 6], [11.8, 18], [13.4, 6], [16.2, 6], [17.8, 18], [19.4, 6], [21.6, 6]]));
  // Mastering: the meter — its scale and its needle.
  track('mastering', t => sil(t, RR(1.8, 4, 20.4, 16, 1.6) + CUT(t, 'M5.4 12.6 C8.4 8.4 15.6 8.4 18.6 12.6', t.S >= 30 ? 2 : 1) +
    (t.D ? CUT(t, 'M5.6 9.4 L6.4 10.4 M18.4 9.4 L17.6 10.4 M12 6.6 V7.8', 1) : '') + CUT(t, 'M12 17.2 L15.4 8.6', t.S >= 30 ? 2 : 1.2) + `<circle class="cut" cx="12" cy="17.2" r="1.5"/>`));
  // Stems: one song, split into its parts.
  track('stems', t => t.R(2.4, 10.6, 7.6, 13.4) + t.Ps('M7.6 12 C9.6 12 9.6 5.4 11.6 5.4 M7.6 12 H11.6 M7.6 12 C9.6 12 9.6 18.6 11.6 18.6', 's2') +
    t.R(12.4, 4, 21.6, 6.8) + t.R(12.4, 10.6, 21.6, 13.4) + t.R(12.4, 17.2, 21.6, 20));
  // Reference track: the target.
  track('reference', t => t.C(12, 12, 9.4) + t.C(12, 12, 5.4) + t.C(12, 12, 2, 'f1'));
  // Cue / guide: a signpost.
  track('cue', t => sil(t, RR(10.9, 1.6, 2.2, 20.8) + RR(7.6, 21.2, 8.8, 1.4, 0.4) + F('M4.6 3.4 H18.2 L21.4 6.6 L18.2 9.8 H4.6 Z') +
    F('M19.4 11.6 H5.8 L2.6 14.8 L5.8 18 H19.4 Z', 'f2')));
  // Headphones: the band and the cups.
  track('headphones', t => sil(t, STK(t, 'M4.6 15 V12 C4.6 7.4 8 4 12 4 C16 4 19.4 7.4 19.4 12 V15', LW(t)) + RR(2.4, 13, 5, 8.6, 1.8) + RR(16.6, 13, 5, 8.6, 1.8)));
  // Speaker / monitor: the cabinet, the woofer and the tweeter.
  track('monitor', t => sil(t, RR(4.8, 1.6, 14.4, 20.8, 1.6) + `<circle class="cut" cx="12" cy="14.4" r="4.6"/>` + C1(12, 14.4, 1.7) + `<circle class="cut" cx="12" cy="6.2" r="1.8"/>`));
  // Mono: one ring with its centre — one source. Stereo: two rings, overlapping.
  track('monoMix', t => t.C(12, 12, 8.4) + t.C(12, 12, 2.2, 'f1'));
  track('stereoMix', t => t.C(8.4, 12, 6.4) + t.C(15.6, 12, 6.4));
  // Surround: the listener among five speakers.
  track('surround', t => t.C(12, 12, 8.4, 's2') + t.C(12, 12, t.D ? 2.4 : 2.2, 'f1') +
    [[12, 3.4], [4.6, 7.8], [19.4, 7.8], [6.6, 19], [17.4, 19]].map(([x, y]) => t.R(x - 2.2, y - 2.2, x + 2.2, y + 2.2, 'f1', 0.6)).join(''));
})();

// ---- Marks & shapes (fire and bell are the Foley drawings)
(() => {
  const { LW } = window.ADDC;
  // Hexagon: point up, its sides on whole pixels.
  track('hexagon', t => { const p = [0, 1, 2, 3, 4, 5, 0].map(i => [12 + 9.8 * Math.cos((-90 + 60 * i) * Math.PI / 180), 12.2 + 9.8 * Math.sin((-90 + 60 * i) * Math.PI / 180)]); return t.Poly(p); });
  track('bolt', t => t.Pf('M14 1.6 L4.8 13.6 H11.2 L9.6 22.4 L19.2 9.8 H12.8 Z'));
  // Sun: the disc and eight rays.
  track('sun', t => t.C(12, 12, t.D ? 4.4 : 4.2, 'f1') + [0, 45, 90, 135, 180, 225, 270, 315].map(a => {
    const c = Math.cos(a * Math.PI / 180), s = Math.sin(a * Math.PI / 180), r0 = t.D ? 7 : 6.8, r1 = 10;
    return t.L(12 + r0 * c, 12 + r0 * s, 12 + r1 * c, 12 + r1 * s);
  }).join(''));
  // (Arcs go through sil(): tk's Pf scales every number, an arc's flags too.)
  track('moon', t => sil(t, F('M13.4 2.4 A9.8 9.8 0 1 0 21.6 15.6 A8 8 0 0 1 13.4 2.4 Z')));
  // Leaf: the blade, its midrib cut out, the stalk.
  track('leaf', t => sil(t, F('M3.6 20.4 C3.6 10.4 9.6 3.6 20.8 3.2 C20.6 14.2 14 20.4 3.6 20.4 Z') + CUT(t, 'M5 19 C8.8 15.2 12.8 11 17 7', t.S >= 30 ? 2 : 1) +
    STK(t, 'M3.8 20.2 L1.8 22.2', LW(t))));
  // Cross: the Christian cross, a Latin cross — the upright and the beam on whole pixels,
  // centred, the beam a quarter of the way down.
  track('cross', t => {
    const S = t.S, m = Math.round(S / 12), h = S - 2 * m;
    const near = (v, par) => { let a = Math.round(v); if ((a - par) % 2) a += (v > a ? 1 : -1); return Math.max(par % 2 ? 1 : 2, a); };
    const bw = near(S * 0.145, S % 2), cw = near(h * 0.64, S % 2);
    const x = (S - bw) / 2, by = m + Math.round(h * 0.24);
    return `<rect class="f1" x="${x}" y="${m}" width="${bw}" height="${h}"/><rect class="f1" x="${(S - cw) / 2}" y="${by}" width="${cw}" height="${bw}"/>`;
  });
  // Check: one confident stroke.
  track('checkMark', t => `<path class="s1" fill="none" stroke-width="${t.S >= 30 ? 3.4 : t.S >= 20 ? 2.4 : 2}" stroke-linecap="round" stroke-linejoin="round" d="M${t.n(3.8)} ${t.n(12.8)} L${t.n(9.4)} ${t.n(18.2)} L${t.n(20.4)} ${t.n(6)}"/>`);
  // Music note: an eighth note with its flag.
  track('musicNote', t => sil(t, `<ellipse class="f1" cx="8.6" cy="18.4" rx="3.9" ry="2.8" transform="rotate(-22 8.6 18.4)"/>` + STK(t, 'M12.2 17.4 V2.4', t.S >= 30 ? 2 : 1.3) +
    F('M11.6 1.8 H12.8 C13.2 5.4 18.6 6.6 17.6 13.2 C17 10.8 15.2 9.2 12.8 8.8 V9.4 H11.6 Z')));
  track('bookmark', t => t.Pf('M6 2.2 H18 V21.8 L12 16.8 L6 21.8 Z'));
  // Pin: the map pin, its hole.
  track('pin', t => sil(t, F('M12 22.4 C12 22.4 4.4 14.2 4.4 9.4 C4.4 5.2 7.8 2 12 2 C16.2 2 19.6 5.2 19.6 9.4 C19.6 14.2 12 22.4 12 22.4 Z') +
    `<circle class="cut" cx="12" cy="9.4" r="${t.D ? 2.8 : 3}"/>`));
  // Eye: the lids, the iris, its light.
  track('eye', t => t.Ps('M1.8 12 C5.6 5.6 18.4 5.6 22.2 12 C18.4 18.4 5.6 18.4 1.8 12 Z') + t.C(12, 12, t.D ? 4 : 3.6, 'f1') + (t.D ? t.C(13.4, 10.6, 1.2, 'cut') : ''));
  // Lock: the shackle and the body, its keyhole.
  track('lock', t => sil(t, STK(t, 'M7.8 11 V7.8 C7.8 4.8 9.6 2.6 12 2.6 C14.4 2.6 16.2 4.8 16.2 7.8 V11', t.S >= 30 ? 2.4 : 1.6) + RR(4.6, 10.4, 14.8, 11.8, 1.6) +
    `<circle class="cut" cx="12" cy="15.2" r="1.7"/>` + RR(11.2, 15.4, 1.6, 3.6, 0.4, 'cut')));
  // Smile: the face, its eyes and smile cut out.
  track('smile', t => sil(t, C1(12, 12, 10) + `<circle class="cut" cx="8.6" cy="9.4" r="1.5"/><circle class="cut" cx="15.4" cy="9.4" r="1.5"/>` +
    CUT(t, 'M7.2 14 C9 17.6 15 17.6 16.8 14', t.S >= 30 ? 2.4 : 1.4)));
})();

// ---- Letters & numbers: small badges, the character cut out of them. Drawn on pixels: a
// 5 x 7 face at 12-16 px, a 7 x 11 face at 18-24 px, and the 5 x 7 face doubled at 30 and 36
// (the family's 2-px line). The badge is sized so the character sits centred on whole pixels.
(() => {
  const SMALL = {
    A: ['.###.', '#...#', '#...#', '#####', '#...#', '#...#', '#...#'], B: ['####.', '#...#', '#...#', '####.', '#...#', '#...#', '####.'],
    C: ['.###.', '#...#', '#....', '#....', '#....', '#...#', '.###.'], D: ['####.', '#...#', '#...#', '#...#', '#...#', '#...#', '####.'],
    E: ['#####', '#....', '#....', '####.', '#....', '#....', '#####'], F: ['#####', '#....', '#....', '####.', '#....', '#....', '#....'],
    G: ['.###.', '#...#', '#....', '#.###', '#...#', '#...#', '.###.'], H: ['#...#', '#...#', '#...#', '#####', '#...#', '#...#', '#...#'],
    1: ['..#..', '.##..', '..#..', '..#..', '..#..', '..#..', '.###.'], 2: ['.###.', '#...#', '....#', '...#.', '..#..', '.#...', '#####'],
    3: ['.###.', '#...#', '....#', '..##.', '....#', '#...#', '.###.'], 4: ['...#.', '..##.', '.#.#.', '#..#.', '#####', '...#.', '...#.'],
    5: ['#####', '#....', '####.', '....#', '....#', '#...#', '.###.'], 6: ['..##.', '.#...', '#....', '####.', '#...#', '#...#', '.###.'],
    7: ['#####', '....#', '...#.', '..#..', '.#...', '.#...', '.#...'], 8: ['.###.', '#...#', '#...#', '.###.', '#...#', '#...#', '.###.'],
    9: ['.###.', '#...#', '#...#', '.####', '....#', '...#.', '.##..'],
  };
  const LARGE = {
    A: ['..###..', '.#...#.', '#.....#', '#.....#', '#.....#', '#######', '#.....#', '#.....#', '#.....#', '#.....#', '#.....#'],
    B: ['######.', '#.....#', '#.....#', '#.....#', '######.', '#.....#', '#.....#', '#.....#', '#.....#', '#.....#', '######.'],
    C: ['.#####.', '#.....#', '#......', '#......', '#......', '#......', '#......', '#......', '#......', '#.....#', '.#####.'],
    D: ['#####..', '#....#.', '#.....#', '#.....#', '#.....#', '#.....#', '#.....#', '#.....#', '#.....#', '#....#.', '#####..'],
    E: ['#######', '#......', '#......', '#......', '#......', '######.', '#......', '#......', '#......', '#......', '#######'],
    F: ['#######', '#......', '#......', '#......', '#......', '######.', '#......', '#......', '#......', '#......', '#......'],
    G: ['.#####.', '#.....#', '#......', '#......', '#......', '#...###', '#.....#', '#.....#', '#.....#', '#.....#', '.#####.'],
    H: ['#.....#', '#.....#', '#.....#', '#.....#', '#.....#', '#######', '#.....#', '#.....#', '#.....#', '#.....#', '#.....#'],
    1: ['...#...', '..##...', '.#.#...', '...#...', '...#...', '...#...', '...#...', '...#...', '...#...', '...#...', '.#####.'],
    2: ['.#####.', '#.....#', '......#', '......#', '.....#.', '....#..', '...#...', '..#....', '.#.....', '#......', '#######'],
    3: ['.#####.', '#.....#', '......#', '......#', '......#', '..####.', '......#', '......#', '......#', '#.....#', '.#####.'],
    4: ['.....#.', '....##.', '...#.#.', '..#..#.', '.#...#.', '#....#.', '#######', '.....#.', '.....#.', '.....#.', '.....#.'],
    5: ['#######', '#......', '#......', '#......', '######.', '......#', '......#', '......#', '......#', '#.....#', '.#####.'],
    6: ['..####.', '.#.....', '#......', '#......', '######.', '#.....#', '#.....#', '#.....#', '#.....#', '#.....#', '.#####.'],
    7: ['#######', '......#', '......#', '.....#.', '....#..', '...#...', '...#...', '...#...', '...#...', '...#...', '...#...'],
    8: ['.#####.', '#.....#', '#.....#', '#.....#', '#.....#', '.#####.', '#.....#', '#.....#', '#.....#', '#.....#', '.#####.'],
    9: ['.#####.', '#.....#', '#.....#', '#.....#', '#.....#', '.######', '......#', '......#', '......#', '.....#.', '.####..'],
  };
  //                 S:  badge, face, scale, radius
  const PLAN = { 12: [11, 's', 1, 2], 14: [13, 's', 1, 2], 15: [13, 's', 1, 2], 16: [13, 's', 1, 2], 18: [15, 'l', 1, 3], 20: [17, 'l', 1, 3],
    21: [17, 'l', 1, 3], 24: [19, 'l', 1, 3], 30: [24, 's', 2, 4], 36: [28, 's', 2, 5] };
  const badge = (ch, S) => {
    const [b, face, z, r] = PLAN[S], rows = (face === 's' ? SMALL : LARGE)[ch];
    const w = rows[0].length * z, h = rows.length * z, bx = Math.floor((S - b) / 2), by = Math.floor((S - b) / 2);
    const x0 = bx + (b - w) / 2, y0 = by + (b - h) / 2;
    let s = `<rect class="f1" x="${bx}" y="${by}" width="${b}" height="${b}" rx="${r}"/>`;
    rows.forEach((row, y) => {
      let x = 0;
      while (x < row.length) {
        if (row[x] !== '#') { x++; continue; }
        let n = 1; while (row[x + n] === '#') n++;
        s += `<rect class="cut" x="${x0 + x * z}" y="${y0 + y * z}" width="${n * z}" height="${z}"/>`;
        x += n;
      }
    });
    return `<g shape-rendering="crispEdges">${s}</g>`;
  };
  const mk = (id, ch) => { N[id] = {}; TSIZES.forEach(S => { N[id][S] = { svg: badge(ch, S) }; }); };
  'ABCDEFGH'.split('').forEach(c => mk('letter' + c, c));
  '123456789'.split('').forEach(c => mk('num' + c, c));
})();

// ==== SECTIONS-END (new sections go above this line)

// ---- The picker's families (set6-additions.md order) and every icon's names: id: [English,
// Spanish, other search words]. Icons listed in two families are one drawing (`bass`,
// `metronomeTrack`, `fire`, `bell`).
window.ADDC_GROUPS = [
  ['Electronic & production', ['theremin', 'vocoder', 'talkbox', 'groovebox', 'stepseq', 'acid', 'padgrid', 'laptop', 'djcontroller', 'cdj', 'cassette', 'radio', 'chiptune', 'glitch', 'waveSine', 'waveSaw', 'waveSquare', 'waveTri']],
  ['Voice', ['leadVocal', 'backingVocals', 'harmony', 'rap', 'vocalChop', 'adlib', 'whisper', 'podcast', 'talkback']],
  ['Notation', ['trebleClef', 'bass', 'cClef', 'percClef', 'quarterNote', 'eighthNotes', 'wholeNote', 'rest', 'sharp', 'flat', 'fermata', 'tuningFork', 'scoreSheet', 'metronomeTrack']],
  ['Sound design & Foley', ['downlifter', 'sweep', 'whoosh', 'subDrop', 'reverse', 'stutter', 'tapeStop', 'vinyl', 'scratch', 'footsteps', 'ambience', 'rain', 'wind', 'water', 'fire', 'birds', 'crowd', 'siren', 'explosion', 'phone', 'fieldRec', 'bell']],
  ['Mix & routing', ['groupFolder', 'aux', 'sidechain', 'compressor', 'eq', 'limiter', 'saturator', 'filterFx', 'chorus', 'phaser', 'mastering', 'stems', 'reference', 'metronomeTrack', 'cue', 'headphones', 'monitor', 'monoMix', 'stereoMix', 'surround']],
  ['Marks & shapes', ['hexagon', 'bolt', 'fire', 'sun', 'moon', 'leaf', 'cross', 'checkMark', 'musicNote', 'bookmark', 'pin', 'eye', 'lock', 'smile', 'bell']],
  ['Letters & numbers', ['letterA', 'letterB', 'letterC', 'letterD', 'letterE', 'letterF', 'letterG', 'letterH', 'num1', 'num2', 'num3', 'num4', 'num5', 'num6', 'num7', 'num8', 'num9']],
];
window.ADDC_GROUPS = window.ADDC_GROUPS.map(([g, ids]) => [g, ids.filter(id => N[id])]);
window.ADDC_WORDS = {
  // Electronic & production
  theremin: ['theremin', 'theremín', 'etherphone, termenvox'], vocoder: ['vocoder', 'vocoder', 'robot voice, voz robótica'],
  talkbox: ['talk box', 'talk box', 'talkbox, tubo, voice box'], groovebox: ['groovebox', 'groovebox', 'caja de ritmos, all-in-one, portable'],
  stepseq: ['step sequencer', 'secuenciador por pasos', 'steps, pasos, pattern, patrón, sequencer'],
  acid: ['acid bassline', 'bajo acid', 'acid, 303, squelch, bassline'], padgrid: ['drum pads', 'pads de batería', 'pads, mpc, finger drumming, pad grid'],
  laptop: ['laptop', 'portátil', 'computer, ordenador, computadora, notebook'], djcontroller: ['DJ controller', 'controlador DJ', 'dj, mixer, mezcladora, decks'],
  cdj: ['media player', 'reproductor DJ', 'cdj, cd, player, jog'], cassette: ['cassette', 'casete', 'tape, cinta, lo-fi'],
  radio: ['radio', 'radio', 'transistor, am, fm, broadcast'], chiptune: ['chiptune', 'chiptune', '8-bit, 8 bits, game, videojuego, gamepad, consola'],
  glitch: ['glitch', 'glitch', 'error, fallo, broken'],
  waveSine: ['sine wave', 'onda senoidal', 'sine, seno, sinusoidal, waveform'], waveSaw: ['saw wave', 'onda de sierra', 'saw, sawtooth, sierra, diente de sierra'],
  waveSquare: ['square wave', 'onda cuadrada', 'square, pulse, pulso'], waveTri: ['triangle wave', 'onda triangular', 'triangle, tri'],
  // Voice
  leadVocal: ['lead vocal', 'voz principal', 'lead, vocals, singer, cantante, voz líder'], backingVocals: ['backing vocals', 'coros', 'bv, backing, voces de fondo, segundas voces'],
  harmony: ['harmony', 'armonía', 'harmonies, voces, dueto, terceras'], rap: ['rap / MC', 'rap / MC', 'mc, hip-hop, hip hop, rapero, verse'],
  vocalChop: ['vocal chop', 'corte vocal', 'chop, chops, cortes de voz, slice'], adlib: ['ad-lib', 'ad-lib', 'adlib, shout, grito, yeah'],
  whisper: ['whisper', 'susurro', 'breath, aliento, quiet, soft'], podcast: ['voice-over / podcast', 'locución / pódcast', 'vo, podcast, broadcast mic, narración, locutor'],
  talkback: ['talkback', 'talkback', 'talk back, intercom, comunicación, control room'],
  // Notation (`bass` is set 6's bass clef; listed here too, with the clef's names)
  bass: ['bass (synth)', 'bajo (sinte)', 'bass clef, clave de fa, fa, F clef, clef, clave'],
  trebleClef: ['treble clef', 'clave de sol', 'sol, treble, G clef, clef, clave'], cClef: ['C clef', 'clave de do', 'do, alto, tenor, viola clef, C clef, clave'],
  percClef: ['percussion clef', 'clave de percusión', 'neutral clef, drums, clef, clave'], quarterNote: ['quarter note', 'negra', 'crotchet, nota, note'],
  eighthNotes: ['eighth notes', 'corcheas', 'quavers, beamed, notas, notes'], wholeNote: ['whole note', 'redonda', 'semibreve, nota, note'],
  rest: ['rest', 'silencio', 'quarter rest, pausa, silencio de negra'], sharp: ['sharp', 'sostenido', '#, accidental, alteración'],
  flat: ['flat', 'bemol', 'b, accidental, alteración'], fermata: ['fermata', 'calderón', 'hold, pause, corona'],
  tuningFork: ['tuning fork', 'diapasón', 'tuner, afinador, 440, a4'], scoreSheet: ['sheet music', 'partitura', 'score, notation, notación, sheet'],
  metronomeTrack: ['click / metronome', 'clic / metrónomo', 'click, clic, metronome, metrónomo, tempo, guide'],
  // Sound design & Foley
  downlifter: ['downlifter', 'bajada', 'fall, caída, fx, drop'], sweep: ['sweep', 'barrido', 'chirp, filter sweep, fx'],
  whoosh: ['whoosh', 'whoosh', 'swoosh, pass-by, paso, zumbido, swish'], subDrop: ['sub drop', 'sub drop', 'drop, boom, bajo, caída grave'],
  reverse: ['reverse', 'reverso', 'reversed, al revés, rewind, reverse cymbal'], stutter: ['stutter', 'tartamudeo', 'repeat, repetición, beat repeat, gate'],
  tapeStop: ['tape stop', 'parada de cinta', 'slow down, frenado, power down'], vinyl: ['vinyl crackle', 'crujido de vinilo', 'crackle, dust, lo-fi, polvo, vinilo'],
  scratch: ['scratch', 'scratch', 'scratching, dj, rayado'], footsteps: ['footsteps / Foley', 'pasos / Foley', 'foley, steps, pisadas, huellas'],
  ambience: ['ambience', 'ambiente', 'atmos, room tone, atmósfera, paisaje, landscape'], rain: ['rain', 'lluvia', 'storm, tormenta, weather'],
  wind: ['wind', 'viento', 'air, aire, breeze, brisa'], water: ['water', 'agua', 'drop, gota, liquid, líquido'],
  fire: ['fire', 'fuego', 'flame, llama, hot'], birds: ['birds', 'pájaros', 'bird, aves, nature, naturaleza'],
  crowd: ['crowd / applause', 'público / aplausos', 'audience, applause, gente, multitud, aplauso'], siren: ['siren', 'sirena', 'alarm, alarma, emergency, police'],
  explosion: ['explosion', 'explosión', 'bomb, bomba, blast, boom'], phone: ['phone / ringtone', 'teléfono / tono', 'ring, mobile, móvil, celular, ringtone'],
  fieldRec: ['field recording', 'grabación de campo', 'recorder, grabadora, handy, field, location'],
  bell: ['bell / notification', 'campana / aviso', 'notification, notificación, alert, alerta, ding'],
  // Mix & routing
  groupFolder: ['group / folder', 'grupo / carpeta', 'group, folder, carpeta, stack'], aux: ['aux', 'auxiliar', 'aux send, envío, send, effects return'],
  sidechain: ['sidechain key', 'llave de sidechain', 'sidechain, key, ducking, clave'], compressor: ['compressor', 'compresor', 'comp, dynamics, dinámica'],
  eq: ['EQ', 'ecualizador', 'equalizer, eq, ecualización'], limiter: ['limiter', 'limitador', 'brickwall, ceiling, techo, clip'],
  saturator: ['saturator / distortion', 'saturador / distorsión', 'tube, válvula, drive, distortion, overdrive, warm'],
  filterFx: ['filter', 'filtro', 'low-pass, lowpass, high-pass, resonance, resonancia'], chorus: ['chorus', 'chorus', 'coro, ensemble, doubling'],
  phaser: ['phaser / flanger', 'phaser / flanger', 'flanger, comb, modulation, modulación'], mastering: ['mastering', 'masterización', 'master, meter, vu, loudness, sonoridad'],
  stems: ['stems', 'stems', 'pistas, split, separación, multitrack'], reference: ['reference track', 'pista de referencia', 'reference, referencia, a/b, target'],
  cue: ['cue / guide', 'cue / guía', 'guide, guía, scratch, cue, pauta'], headphones: ['headphones', 'auriculares', 'cascos, audífonos, phones, cue mix'],
  monitor: ['speaker / monitor', 'altavoz / monitor', 'speaker, monitor, bocina, parlante, studio monitor'],
  monoMix: ['mono', 'mono', 'monaural, one channel, un canal'], stereoMix: ['stereo', 'estéreo', 'stereo, two channels, l/r, dos canales'],
  surround: ['surround', 'envolvente', '5.1, 7.1, atmos, immersive, inmersivo'],
  // Marks & shapes
  hexagon: ['hexagon', 'hexágono', 'shape, forma'], bolt: ['lightning bolt', 'rayo', 'bolt, flash, energy, energía'],
  sun: ['sun', 'sol (astro)', 'day, día, bright'], moon: ['moon', 'luna', 'night, noche'], leaf: ['leaf', 'hoja', 'nature, naturaleza, organic'],
  cross: ['cross', 'cruz', 'christian, cristiano, faith, fe, church, iglesia, worship, alabanza'],
  checkMark: ['check', 'visto', 'done, hecho, ok, tick, palomita'], musicNote: ['music note', 'nota musical', 'note, nota, music, música'],
  bookmark: ['bookmark', 'marcador', 'save, guardar, marca'], pin: ['pin', 'chincheta', 'location, ubicación, marker, alfiler'],
  eye: ['eye', 'ojo', 'watch, ver, visible'], lock: ['lock', 'candado', 'locked, bloqueado, closed'],
  smile: ['smile', 'sonrisa', 'happy, feliz, face, cara, emoji'],
};
// Letters and numbers: "letter A" / "letra A", "number 1" / "número 1", found by the bare
// character too.
'ABCDEFGH'.split('').forEach(c => { window.ADDC_WORDS['letter' + c] = [`letter ${c}`, `letra ${c}`, `${c}, badge, insignia, take, toma, letter, letra`]; });
'123456789'.split('').forEach(c => { window.ADDC_WORDS['num' + c] = [`number ${c}`, `número ${c}`, `${c}, badge, insignia, take, toma, number, número`]; });
// The picker's search finds these too.
if (window.TRACK_WORDS) Object.assign(window.TRACK_WORDS, window.ADDC_WORDS);
