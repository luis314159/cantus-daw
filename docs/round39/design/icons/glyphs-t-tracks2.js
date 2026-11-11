// Cantus icon family — set 6, part 2: orchestra, voice, FX, mix, marks and shapes.
var N = window.N || (window.N = {});

// ---- Orchestra
track('strings', t => (t.D ? t.Ps('M8 3 V21 M11 3 V21 M14 3 V21 M17 3 V21', 's2') : t.Ps('M7 3 V21 M12 3 V21 M17 3 V21', 's2')) + t.Ps('M3 17.5 L21 8') + (t.D ? t.Ps('M3.5 19.5 L21.5 10', 's1') : ''));
track('brass', t => t.Pf('M13 12 L21 6 V18 Z') + t.L(3, 12, 13, 12) + t.Ps('M6.5 7.5 V12 M9.5 7.5 V12', t.D ? 's2' : 's1') + (t.D ? t.Box(5, 12, 12, 16) : ''));
track('woodwind', t => t.Box(10, 2, 14, 18) + t.Pf('M9 18 H15 L17 22 H7 Z') + (t.D ? t.R(11, 5, 13, 6.5, 'f2') + t.R(11, 9, 13, 10.5, 'f2') + t.R(11, 13, 13, 14.5, 'f2') : ''));

// ---- Voice
track('choir', t => t.C(5.5, 9.5, 2.5, 'f2') + t.C(18.5, 9.5, 2.5, 'f2') + t.Pf('M1 21 C1 15.5 10 15.5 10 21 Z M14 21 C14 15.5 23 15.5 23 21 Z', 'f2') + t.C(12, 7.5, 3, 'f1') + t.Pf('M6.5 22 C6.5 14 17.5 14 17.5 22 Z'));
track('mic', t => t.R(9, 2, 15, 13, 'f1', 3) + (t.D ? t.Ps('M9.5 6 H14.5 M9.5 9 H14.5', 'scut') : '') + t.Ps('M6 10 C6 17 18 17 18 10') + t.L(12, 16.5, 12, 21) + t.L(8, 21, 16, 21));
track('voice', t => t.Ps('M4 4.5 H20 V15 H11 L7 19.5 V15 H4 Z') + (t.D ? t.Ps('M7.5 8.5 H16.5 M7.5 11.5 H13.5', 's2') : ''));

// ---- FX
track('riser', t => t.Ps('M3 20 C11 20 16 15 19.5 5') + t.Ps('M16 6.5 L20 3.5 L22 8') + (t.D ? t.C(5, 15.5, 0.9, 'f2') + t.C(9, 12.5, 0.9, 'f2') : ''));
const burst = (t, cx, cy, R, r, nPts) => {
  const pts = [];
  for (let i = 0; i < 2 * nPts; i++) {
    const a = -Math.PI / 2 + i * Math.PI / nPts, rr = i % 2 ? r : R;
    pts.push(`${(cx + rr * Math.cos(a)).toFixed(2)} ${(cy + rr * Math.sin(a)).toFixed(2)}`);
  }
  return t.Pf(`M${pts.join(' L')} Z`);
};
track('impact', t => burst(t, 12, 12, 10.5, 4.6, 8) + (t.D ? t.C(12, 12, 2, 'cut') : ''));
track('noise', t => [[3, 10], [6, 6], [9, 14], [12, 8], [15, 12], [18, 5], [21, 9]].map(([x, h]) => t.R(x - 0.6, 12 - h / 2, x + 0.9, 12 + h / 2, 'f1')).join(''));
track('loop', t => t.Ps('M4 14 V12 C4 9.5 6 8 8.5 8 H18.5') + t.Ps('M16 5.5 L18.5 8 L16 10.5') + t.Ps('M20 10 V12 C20 14.5 18 16 15.5 16 H5.5') + t.Ps('M8 13.5 L5.5 16 L8 18.5'));
track('oneshot', t => [[4, 14], [7, 12], [10, 9], [13, 7], [16, 5], [19, 3], [21.5, 2]].map(([x, h]) => t.R(x - 0.6, 12 - h / 2, x + 0.9, 12 + h / 2, 'f1')).join('') + (t.D ? t.R(3, 19.5, 21, 20.5, 'f2') : ''));

// ---- Mix
track('bus', t => t.Ps('M3 6 C9 6 8 12 14 12 M3 18 C9 18 8 12 14 12', 's2') + t.L(3, 12, 21, 12) + t.Ps('M18 9.5 L20.5 12 L18 14.5'));
track('reverb', t => t.C(5, 12, 2, 'f1') + t.Ps('M9 7.5 C11 10 11 14 9 16.5') + t.Ps('M13 4.5 C16.5 9 16.5 15 13 19.5', 's2') + (t.D ? t.Ps('M17 2.5 C21.5 8 21.5 16 17 21.5', 's2') : ''));
track('delay', t => t.R(3, 4, 6, 20) + t.R(10, 7, 13, 17, 'f2') + t.R(17, 10, 20, 14, 'f2', 0) + (t.D ? '' : ''));
track('master', t => t.Pf('M3 18 L3.5 7 L8.5 12 L12 4.5 L15.5 12 L20.5 7 L21 18 Z') + (t.D ? t.R(3, 15, 21, 16.2, 'cut') : '') + t.R(3, 19.5, 21, 21, 'f2'));
track('midi', t => t.C(12, 12, 9) + (t.D ? [[6.5, 12], [8.1, 8.1], [12, 6.5], [15.9, 8.1], [17.5, 12]] : [[7, 12], [12, 7], [17, 12]]).map(([x, y]) => t.C(x, y, t.D ? 1.2 : 1.5, 'f1')).join('') + t.R(10.5, 18.5, 13.5, 21, 'f1'));
track('video', t => t.Box(3, 7, 15, 17, 's1', 1) + t.Pf('M15.5 10.5 L21 7 V17 L15.5 13.5 Z'));

// ---- Marks and shapes
const star5 = (t, cx, cy, R) => burst(t, cx, cy, R, R * 0.43, 5);
track('star', t => star5(t, 12, 12.6, 10));
track('heart', t => t.Pf('M12 20.5 C5 15 2.5 11.5 2.5 8 C2.5 5 5 3.5 7.5 3.5 C9.5 3.5 11 4.8 12 6.5 C13 4.8 14.5 3.5 16.5 3.5 C19 3.5 21.5 5 21.5 8 C21.5 11.5 19 15 12 20.5 Z'));
track('flag', t => t.L(5, 3, 5, 21) + t.Pf('M6 4 H19.5 L16.5 8.5 L19.5 13 H6 Z'));
track('circle', t => t.C(12, 12, 8));
track('square', t => t.Box(4, 4, 20, 20, 's1', 1));
track('triangle', t => t.Ps('M12 3.5 L21 19.5 H3 Z'));
track('diamond', t => t.Ps('M12 2.5 L21.5 12 L12 21.5 L2.5 12 Z'));

window.TRACK_IDS = [
  ['Drums', ['kick', 'snare', 'hihat', 'cymbal', 'tom', 'clap', 'shaker', 'drumkit']],
  ['Bass and keys', ['bass', 'sub', 'piano', 'keys']],
  ['Guitars and orchestra', ['eguitar', 'aguitar', 'strings', 'brass', 'woodwind']],
  ['Synths', ['synth', 'pad', 'lead', 'pluck', 'arp']],
  ['Voice', ['mic', 'voice', 'choir']],
  ['Sound and FX', ['riser', 'impact', 'noise', 'loop', 'oneshot', 'midi', 'video']],
  ['Mix', ['bus', 'reverb', 'delay', 'master']],
  ['Marks and shapes', ['star', 'heart', 'flag', 'circle', 'square', 'triangle', 'diamond']],
];
const NAMES = { kick: 'kick', snare: 'snare', hihat: 'hi-hat', cymbal: 'cymbal', tom: 'tom', clap: 'clap', shaker: 'shaker', drumkit: 'drum kit', bass: 'bass', sub: 'sub', piano: 'piano', keys: 'keys / organ', eguitar: 'electric guitar', aguitar: 'acoustic guitar', strings: 'strings', brass: 'brass', woodwind: 'woodwind', synth: 'synth', pad: 'pad', lead: 'lead', pluck: 'pluck', arp: 'arp', mic: 'vocal mic', voice: 'voice / speech', choir: 'choir', riser: 'FX / riser', impact: 'impact', noise: 'noise', loop: 'loop', oneshot: 'sample / one-shot', midi: 'MIDI', video: 'video', bus: 'bus', reverb: 'return / reverb', delay: 'delay', master: 'master', star: 'star', heart: 'heart', flag: 'flag', circle: 'circle', square: 'square', triangle: 'triangle', diamond: 'diamond' };
window.TRACK_NAMES = NAMES;
window.TRACK_ROWS = window.TRACK_IDS.flatMap(([, ids]) => ids.map(id => [id, NAMES[id], [12, 14, 16, 24], [18, 21, 24, 36], '']));
