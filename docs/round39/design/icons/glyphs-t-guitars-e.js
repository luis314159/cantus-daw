// Cantus icon family — the guitar family, part E: the electric shapes, upright (round 39,
// icons-guitars). Helpers in glyphs-t-guitars.js (`window.GTR`). Variants: A, the outline with
// its pickups cut; B, the same outline with its pickguard in the second ink; C, a second,
// more exaggerated outline. The chosen ones are named in glyphs-t-guitars-pick.js.
// Generic outlines drawn from blank sheets after studying photographs (guitars-study.md); no
// maker's outline is traced, the labels are generic.
(() => {
  const { stretch, mirror, Fs, R, PX, PL, HOLE, portrait } = window.GTR;
  const E = window.GTR_E = {};
  // the neck over a pickguard, inside the body's exaggeration (so it keeps the neck's width)
  const NK = (t, y1) => R(t, 12 - 1.4 / 1.14, 4, 12 + 1.4 / 1.14, y1);

  // ---- double cutaway (S-type): two rounded horns, the bass horn longer; a contoured waist.
  E.S1 = 'M10.7 10.6 C10.4 8.4 9.2 6 7.4 5.6 C6 5.4 5.2 6.4 5.4 8.2 C5.6 10 6.6 11.4 6.2 13 C5.8 14.4 4.2 15.6 4.2 18.4 C4.2 21.8 7.6 23.4 12 23.4 C16.4 23.4 19.8 21.8 19.8 18.6 C19.8 16 18 15.2 17.8 13.6 C17.6 12.2 18.4 10.8 18 9.2 C17.6 7.6 16.2 7.4 15.2 8.2 C14.4 8.8 13.8 9.8 13.3 10.6 Z';
  // C: the horns longer and the waist deeper.
  E.S2 = 'M10.7 10.8 C10.4 8 9 5 7 4.4 C5.4 4 4.6 5.4 4.8 7.4 C5 9.6 6.4 11.4 6 13.2 C5.6 14.6 3.8 15.6 3.8 18.4 C3.8 21.8 7.4 23.4 12 23.4 C16.6 23.4 20.2 21.8 20.2 18.6 C20.2 15.8 18.2 15 18 13.4 C17.8 11.8 18.8 10.2 18.4 8.2 C18 6.6 16.4 6.4 15.2 7.4 C14.2 8.2 13.6 9.6 13.3 10.8 Z';
  const S_GUARD = 'M10.7 9.6 C9.2 9 7.6 9.4 7.4 10.8 C7.2 12.6 8.4 13.4 7.6 15.2 C7 16.6 6.4 17.6 7.6 18.8 C9 20 11.4 20.4 13.6 19.6 L13.3 9.6 Z';
  const S_PU = t => t.D ? PL(t, 9.6, 13.2, 14.4, 13.2) + PL(t, 9.6, 15.8, 14.4, 15.8) + PL(t, 9.6, 18.6, 14.4, 18) : '';
  track('eStypeA', t => portrait(t, E.S1, { head: 'inline', n1: 12, inner: S_PU }));
  track('eStypeB', t => portrait(t, E.S1, { head: 'inline', n1: 12, inner: t => t.D ? Fs(t, S_GUARD, 'f2') + NK(t, 12) + (t.D === 2 ? S_PU(t) : '') : '' }));
  track('eStypeC', t => portrait(t, E.S2, { head: 'inline', n1: 12, inner: S_PU }));

  // ---- single cutaway slab (T-type): a flat ledge beside the neck on the bass side, a squared
  // shoulder, nearly straight sides; on the treble side one cutaway and a blunt horn.
  E.T1 = 'M10.7 9.6 L7.8 9.6 C6.2 9.6 5.2 10.4 5.2 12 C5.2 13.6 5.6 14.4 5.2 15.6 C4.8 16.8 4.2 17.6 4.2 19.6 C4.2 22.4 7.2 23.4 12 23.4 C16.8 23.4 19.8 22.4 19.8 19.6 C19.8 17.6 19.2 16.8 18.8 15.6 C18.4 14.4 18.8 13.4 18.8 12.4 C18.8 11.2 18 10.4 17.2 10.6 C16.2 10.8 15.6 12.6 14.4 13 C13.8 13.2 13.4 12.8 13.3 12.2 Z';
  // C: the ledge and shoulder square, the sides straight, the horn lower and blunter.
  E.T2 = 'M10.7 9.2 L6.8 9.2 C5.4 9.2 4.6 10 4.6 11.4 L4.6 15.4 C4.6 16.4 4 17.4 4 19.6 C4 22.4 7 23.4 12 23.4 C17 23.4 20 22.4 20 19.6 C20 17.4 19.4 16.4 19.4 15.4 L19.4 13 C19.4 11.6 18.4 11 17.6 11.4 C16.6 11.8 16 13.6 14.4 13.8 C13.8 13.8 13.4 13.4 13.3 12.8 Z';
  const T_GUARD = 'M10.7 10.4 L7.8 10.4 C6.6 10.4 6 11 6 12.2 C6 13.6 6.6 14.6 7.6 15.6 C8.6 16.6 9.4 17.4 10.2 17.4 L13.3 17.4 L13.3 10.4 Z';
  // the slanted bridge pickup in its plate, and the control plate on the treble side
  const T_DET = t => t.D ? PL(t, 10, 19.4, 13.8, 18.4) + (t.D === 2 ? PX(t, 12, 14.6, 3, 1) : '') : '';
  track('eTtypeA', t => portrait(t, E.T1, { head: 'inline', n1: 12, inner: T_DET }));
  track('eTtypeB', t => portrait(t, E.T1, { head: 'inline', n1: 12, inner: t => t.D ? Fs(t, T_GUARD, 'f2') + NK(t, 17) + T_DET(t) : '' }));
  track('eTtypeC', t => portrait(t, E.T2, { head: 'inline', n1: 12.6, inner: T_DET }));

  // ---- carved single cut: a round dome of a bass shoulder, a narrower upper bout over a deep
  // waist, a full lower bout; one cutaway leaving a sharp horn that points up.
  E.SC1 = 'M10.7 10 C10 9.2 9 8.8 8 9 C6.4 9.4 5.6 10.6 5.8 12.2 C6 13.4 6.8 14 6.4 15 C6 15.8 4.2 16.8 4.2 19.4 C4.2 22.2 7.4 23.4 12 23.4 C16.6 23.4 19.8 22.2 19.8 19.4 C19.8 17 18 16.2 17.6 15 C17.2 13.8 18 12.4 17.8 10.6 C17.7 9.6 17.4 8.6 16.9 8.2 C16.2 9.2 16 11.6 15 12.6 C14.4 13.2 13.6 13 13.3 12.4 Z';
  // C: the horn taller and sharper, the waist deeper, the dome rounder.
  E.SC2 = 'M10.7 10.2 C10 9 8.8 8.4 7.6 8.6 C5.8 9 5 10.6 5.4 12.4 C5.6 13.6 6.8 14.2 6.6 15.2 C6.4 16 4 16.8 4 19.4 C4 22.2 7.4 23.4 12 23.4 C16.6 23.4 20 22.2 20 19.4 C20 16.8 17.8 16.2 17.6 15 C17.4 13.6 18.4 12.2 18.2 10 C18.1 8.8 17.8 7.6 17.2 7 C16.2 8.4 16.2 11.4 15 12.6 C14.4 13.2 13.6 13 13.3 12.4 Z';
  const SC_GUARD = 'M13.8 13.6 C15 13.2 15.8 12.6 16.2 11.6 C16.6 13.4 16.4 15.4 15.4 16.8 C14.6 16.4 14 15.2 13.8 13.6 Z';
  // two humbuckers: fat, two pixels tall from 24 px
  const SC_PU = t => t.D ? PX(t, 12, 15.2, t.D === 2 ? 5 : 3, t.D === 2 ? 2 : 1) + PX(t, 12, 18.8, t.D === 2 ? 5 : 3, t.D === 2 ? 2 : 1) : '';
  track('eSingleCutA', t => portrait(t, E.SC1, { n1: 13, inner: SC_PU }));
  track('eSingleCutB', t => portrait(t, E.SC1, { n1: 13, inner: t => (t.D ? Fs(t, SC_GUARD, 'f2') : '') + SC_PU(t) }));
  track('eSingleCutC', t => portrait(t, E.SC2, { n1: 13, inner: SC_PU }));

  // ---- twin horn: two sharp, symmetric horns that spread outward like a lyre; deep, round
  // cutaways; a slim body with a clear waist.
  E.DH1 = 'M10.7 11.6 C10.2 9.6 8.4 7.6 5.6 6.4 C5.2 8.2 5.6 10.4 6.2 11.8 C6.6 12.8 6.2 13.8 5.6 14.8 C4.8 16.2 4.6 17.4 4.6 19.2 C4.6 22 7.6 23.4 12 23.4 C16.4 23.4 19.4 22 19.4 19.2 C19.4 17.4 19.2 16.2 18.4 14.8 C17.8 13.8 17.4 12.8 17.8 11.8 C18.4 10.4 18.8 8.2 18.4 6.4 C15.6 7.6 13.8 9.6 13.3 11.6 Z';
  // C: the horns longer and further apart, the waist deeper (the lower body a tulip).
  E.DH2 = 'M10.7 12 C10 9.6 7.6 7.2 4.4 5.8 C4 8 4.8 10.6 5.8 12.2 C6.4 13.2 6.4 14 5.6 15.2 C4.8 16.4 4.4 17.6 4.4 19.4 C4.4 22.2 7.6 23.4 12 23.4 C16.4 23.4 19.6 22.2 19.6 19.4 C19.6 17.6 19.2 16.4 18.4 15.2 C17.6 14 17.6 13.2 18.2 12.2 C19.2 10.6 20 8 19.6 5.8 C16.4 7.2 14 9.6 13.3 12 Z';
  const DH_GUARD = 'M10.7 12.4 C9.4 12.6 8.2 13.6 8.4 15 C8.6 16.4 7.4 17.2 8 18.2 C9.4 18.6 10.6 17.8 11.2 17 L13.3 17 L13.3 12.4 Z';
  track('eDoubleHornA', t => portrait(t, E.DH1, { n1: 13, inner: SC_PU }));
  track('eDoubleHornB', t => portrait(t, E.DH1, { n1: 13, inner: t => (t.D ? Fs(t, DH_GUARD, 'f2') + Fs(t, mirror(DH_GUARD), 'f2') : '') + SC_PU(t) }));
  track('eDoubleHornC', t => portrait(t, E.DH2, { n1: 13, inner: SC_PU }));

  // ---- pointed double cut: needle horns pointing straight up, the bass horn far longer, deep
  // cutaways; a pointed head.
  E.SS1 = 'M10.7 12 C10.4 9.4 8.8 6.2 6 4.2 C5.6 7.4 6.4 10 6.4 11.8 C6.4 13.4 4.2 15.4 4.2 18.4 C4.2 21.8 7.6 23.4 12 23.4 C16.6 23.4 19.8 21.6 19.8 18.6 C19.8 16.2 17.8 15 17.8 13.2 C17.8 11.4 18.4 9 18.4 6.6 C16 8 13.8 9.8 13.3 12 Z';
  // C: the horns thinner and longer still, the lower bout angled (a sharper tail).
  E.SS2 = 'M10.7 12.4 C10.4 9.4 8.6 5.4 5.6 3.2 C5 6.8 6.2 9.8 6.2 11.8 C6.2 13.6 3.8 15.6 3.8 18.6 C3.8 21.6 6.8 23.4 11 23.4 L13 23.4 C17.2 23.4 20.2 21.6 20.2 18.8 C20.2 16 17.6 15 17.6 13 C17.6 11 18.6 8.2 18.6 5.4 C16 7 13.8 9.8 13.3 12.4 Z';
  const SS_PU = t => t.D ? PX(t, 12, 14.4, t.D === 2 ? 5 : 3, t.D === 2 ? 2 : 1) + PL(t, 9.8, 17.2, 14.2, 17.2) + PX(t, 12, 20, t.D === 2 ? 5 : 3, t.D === 2 ? 2 : 1) : '';
  track('eSuperstratA', t => portrait(t, E.SS1, { head: 'pointy', n1: 13, inner: SS_PU }));
  track('eSuperstratB', t => portrait(t, E.SS1, { head: 'pointy', n1: 13, inner: t => (t.D ? Fs(t, 'M10.7 12 C9.6 12 8.6 12.8 8.6 14 L8.6 17.6 C9.6 18.6 12 18.6 13.3 18.2 L13.3 12 Z', 'f2') : '') + (t.D === 2 ? SS_PU(t) : '') }));
  track('eSuperstratC', t => portrait(t, E.SS2, { head: 'pointy', n1: 13, inner: SS_PU }));

  // ---- semi-hollow: the widest electric; two round, symmetric horns like ears; f-holes.
  E.SH1 = 'M10.7 11.8 C10.4 10 9.2 8.6 7.6 8.6 C5.6 8.6 4.6 10.2 4.8 12 C5 13.4 5.8 13.8 5 15 C4 16.2 3.4 17.4 3.4 19 C3.4 22 7.2 23.4 12 23.4 C16.8 23.4 20.6 22 20.6 19 C20.6 17.4 20 16.2 19 15 C18.2 13.8 19 13.4 19.2 12 C19.4 10.2 18.4 8.6 16.4 8.6 C14.8 8.6 13.6 10 13.3 11.8 Z';
  // C: wider still, the ears rounder, the waist a little deeper.
  E.SH2 = stretch(E.SH1, 1.08);
  const FH = t => t.D ? PL(t, 7.2, 14.6, 6.8, 19.2) + PL(t, 16.8, 14.6, 17.2, 19.2) + (t.D === 2 ? PX(t, 6.8, 14.2, 2, 1) + PX(t, 7.2, 19.6, 2, 1) + PX(t, 17.2, 14.2, 2, 1) + PX(t, 16.8, 19.6, 2, 1) : '')
    : PL(t, 7, 15, 7, 19) + PL(t, 17, 15, 17, 19);
  const SH_PU = t => t.D === 2 ? PX(t, 12, 14.6, 5, 2) + PX(t, 12, 18.2, 5, 2) : '';
  track('eSemiHollowA', t => portrait(t, E.SH1, { n1: 13, inner: t => FH(t) + SH_PU(t) }));
  track('eSemiHollowB', t => portrait(t, E.SH1, { n1: 13, inner: t => FH(t) + (t.D ? Fs(t, 'M13.6 13.4 C15 13.2 15.6 14 15.4 15.6 C15.2 16.8 14.4 17.6 13.6 17.4 Z', 'f2') : '') + SH_PU(t) }));
  track('eSemiHollowC', t => portrait(t, E.SH2, { n1: 13, inner: t => FH(t) + SH_PU(t) }));

  // ---- offset: the body skewed — a long, broad bass horn and a short treble horn; the waist
  // high on the bass side and low on the treble side; the lower bout fuller on the treble side,
  // its lowest point to the right of the axis.
  E.OF1 = 'M10.7 10.8 C10.4 8.6 9.4 6.6 7.8 6.2 C6.2 5.8 4.8 7 5 9.2 C5.2 11 6.2 11.8 6.2 13 C6.2 14.4 4.6 15.4 4.6 17.8 C4.6 21.2 7.2 22.8 10.6 23.2 C15.6 23.8 20.2 22.4 20.2 18.6 C20.2 16.6 18.6 16 18.2 14.6 C17.8 13.2 18.4 12.2 18 11 C17.6 9.8 16.4 9.8 15.6 10.6 C14.8 11.4 13.8 11.6 13.3 11.6 Z';
  // C: the skew stronger — the bass waist higher, the treble bout lower and fuller.
  E.OF2 = 'M10.7 10.6 C10.4 8.2 9.2 5.8 7.4 5.4 C5.6 5 4.4 6.6 4.8 8.8 C5.1 10.6 6.4 11.2 6.4 12.2 C6.4 13.6 4.8 14.6 4.8 17.2 C4.8 20.8 7.2 22.6 10.6 23.2 C15.8 24 20.6 22.6 20.6 18.6 C20.6 16.4 18.8 15.8 18.4 14.4 C18 13 18.6 12.4 18.2 11.4 C17.8 10.4 16.6 10.4 15.8 11 C15 11.6 14 11.8 13.3 11.8 Z';
  const OF_GUARD = 'M10.7 9.4 C9.4 8.6 7.4 8.4 7 10.2 C6.8 11.6 7.8 12.6 7.4 14.2 C7 15.6 6.4 16.8 7.6 18 C9.2 19.4 13.4 19.2 15.8 17.4 C17 16.4 17 14.4 16.4 13.4 C15.8 12.6 14.4 12.6 13.3 12.6 L13.3 9.4 Z';
  // two wide, flat pickups (soapbars), set square to the strings
  const OF_PU = t => t.D ? PX(t, 12, 14.4, t.D === 2 ? 5 : 3, t.D === 2 ? 2 : 1) + PX(t, 12, 17.6, t.D === 2 ? 5 : 3, t.D === 2 ? 2 : 1) : '';
  track('eOffsetA', t => portrait(t, E.OF1, { head: 'inline', n1: 12, inner: OF_PU }));
  track('eOffsetB', t => portrait(t, E.OF1, { head: 'inline', n1: 12, inner: t => (t.D ? Fs(t, OF_GUARD, 'f2') + NK(t, 12.6) : '') + (t.D === 2 ? OF_PU(t) : '') }));
  track('eOffsetC', t => portrait(t, E.OF2, { head: 'inline', n1: 12, inner: OF_PU }));

  // ---- V body: two wings with rounded tips, a notch between them.
  E.V1 = 'M10.7 9 L4 21.6 C3.6 22.6 4.4 23.6 5.6 23.2 L9.8 21.4 L12 18.8 L14.2 21.4 L18.4 23.2 C19.6 23.6 20.4 22.6 20 21.6 L13.3 9 Z';
  E.V2 = 'M10.7 8.2 L3.6 21.4 C3.2 22.4 3.8 23.4 5 23.2 L9.6 21.6 L12 17.8 L14.4 21.6 L19 23.2 C20.2 23.4 20.8 22.4 20.4 21.4 L13.3 8.2 Z';
  const V_PU = t => t.D ? PX(t, 12, 12.6, t.D === 2 ? 4 : 2, t.D === 2 ? 2 : 1) + PX(t, 12, 15.6, t.D === 2 ? 4 : 2, t.D === 2 ? 2 : 1) : '';
  track('eVshapeA', t => portrait(t, E.V1, { n1: 11, inner: V_PU }));
  track('eVshapeB', t => portrait(t, E.V1, { n1: 11, inner: t => t.D ? Fs(t, 'M10.4 11 L7.4 18.4 L9.6 19.2 L12 16.8 L14.4 19.2 L16.6 18.4 L13.6 11 Z', 'f2') + NK(t, 15.6) : '' }));
  track('eVshapeC', t => portrait(t, E.V2, { n1: 10.4, inner: V_PU }));

  // ---- pointed body: an asymmetric body of straight edges and sharp points — a long bass horn
  // to the upper left, a short treble point, a long point to the lower right.
  E.PT1 = 'M10.7 11.4 L4.4 5.6 L6.6 13.8 L4 19.8 L10.6 21 L12.6 23.6 L20.4 21.4 L17.6 15.6 L19.2 10.6 L13.3 12.6 Z';
  // C: fewer, bolder points: the bass horn, the treble point, the lower-right tail.
  E.PT2 = 'M10.7 11.8 L3.8 5 L6.2 15.4 L5.4 21.4 L20.8 23.2 L17.8 15.2 L19.8 9.8 L13.3 12.8 Z';
  const PT_PU = t => t.D ? PX(t, 12, 15, t.D === 2 ? 5 : 3, t.D === 2 ? 2 : 1) + PX(t, 12, 18.6, t.D === 2 ? 5 : 3, t.D === 2 ? 2 : 1) : '';
  track('ePointedA', t => portrait(t, E.PT1, { head: 'pointy', n1: 13, inner: PT_PU }));
  track('ePointedB', t => portrait(t, E.PT1, { head: 'pointy', n1: 13, inner: t => t.D ? Fs(t, 'M9 16.4 L7.4 20 L13.6 21 L15.4 18.6 L13.3 16.4 Z', 'f2') + (t.D === 2 ? PT_PU(t) : '') : '' }));
  track('ePointedC', t => portrait(t, E.PT2, { head: 'pointy', n1: 13, inner: PT_PU }));

  // ---- archtop: the big hollow body, one round (Venetian) cutaway, f-holes (the trapeze tail
  // was cut as a line and split the body in two at 24 px; it is left out).
  E.AR1 = 'M10.7 9.4 C9.8 8.2 8.4 7.8 7 8.2 C5.2 8.8 4.4 10.4 4.6 12.2 C4.8 13.6 5.6 14.2 4.6 15.4 C3.6 16.6 3.2 17.8 3.2 19.4 C3.2 22.2 7.2 23.6 12 23.6 C16.8 23.6 20.8 22.2 20.8 19.4 C20.8 17.8 20.2 16.6 19.4 15.4 C18.6 14.2 19.2 13.4 19.2 12 C19.2 10.6 18.4 9.8 17.4 10 C16.4 10.2 16.2 11.4 15.6 12.2 C14.8 13 13.8 13 13.3 12.4 Z';
  E.AR2 = stretch(E.AR1, 1.06);
  const AR_DET = t => FH(t) + (t.D === 2 ? PX(t, 12, 15.4, 4, 2) : '');
  track('archtopA', t => portrait(t, E.AR1, { n1: 13, inner: AR_DET }));
  track('archtopB', t => portrait(t, E.AR1, { n1: 13, inner: t => AR_DET(t) + (t.D ? Fs(t, 'M13.8 13.2 C15.4 13 16.4 14 16.2 15.8 C16 17.2 15 18 14 17.8 Z', 'f2') : '') }));
  track('archtopC', t => portrait(t, E.AR2, { n1: 13, inner: AR_DET }));
})();
