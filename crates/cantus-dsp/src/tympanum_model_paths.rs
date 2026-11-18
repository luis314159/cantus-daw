//! The modelled kit's paths: built from the geometry, rendered from the rings.

use super::*;

/// Catmull-Rom's weights for `y0 … y3` at `f` between `y1` and `y2`.
#[inline]
fn catmull_rom(f: f32) -> [f32; 4] {
    let (f2, f3) = (f * f, f * f * f);
    [
        0.5 * (-f + 2.0 * f2 - f3),
        1.0 + 0.5 * (-5.0 * f2 + 3.0 * f3),
        0.5 * (f + 4.0 * f2 - 3.0 * f3),
        0.5 * (-f2 + f3),
    ]
}

/// A settled path's chunk: `acc[t] += (k · cubic(x, t) [+ side · cubic(y, t)]) · g` — the
/// window `x` (and `y`, the side's, when `side` is not 0) read in one run, `c + 3` samples
/// from the sample before the first.
#[inline(never)]
fn settled_read(acc: &mut [f32], x: &[f32], w: [f32; 4], k: f32, g: f32, side: f32, y: &[f32]) {
    let c = acc.len();
    let (x0, x1, x2, x3) = (&x[..c], &x[1..c + 1], &x[2..c + 2], &x[3..c + 3]);
    if side != 0.0 {
        let (y0, y1, y2, y3) = (&y[..c], &y[1..c + 1], &y[2..c + 2], &y[3..c + 3]);
        for t in 0..c {
            let mut v = k * (w[0] * x0[t] + w[1] * x1[t] + w[2] * x2[t] + w[3] * x3[t]);
            v += side * (w[0] * y0[t] + w[1] * y1[t] + w[2] * y2[t] + w[3] * y3[t]);
            acc[t] += v * g;
        }
    } else {
        for t in 0..c {
            let v = k * (w[0] * x0[t] + w[1] * x1[t] + w[2] * x2[t] + w[3] * x3[t]);
            acc[t] += v * g;
        }
    }
}

/// One group of paths in a batch (a piece × mic × channel): its paths, its colour, and the
/// sample of this chunk from which its colour rests.
#[derive(Copy, Clone, Default)]
struct Group {
    from: usize,
    to: usize,
    colour: usize,
    piece: usize,
    mic: usize,
    ch: u8,
    rest: usize,
}

/// A drum head's highs leave along its normal (drums-gen, round 40): a piston of radius `a`
/// is heard off its axis at angle θ only below about `c / (π a sin θ)` — the kick's and
/// toms' highs reach the snare's, the toms' and the overheads' mics far duller than their
/// own (the recordings' bleed is; ours, a copy of the whole head in every mic, filled the
/// kick's first 30 ms with 8-16 kHz 13-25 dB above DRSKit's and combed the snare's highs
/// into lines). The radius is the piece's (`PieceGeom::radius`), or by its kind when not
/// given (its dipole share, which the unit sets per kind: a kick 0.55, a snare or tom 0.7):
/// a 22″ kick, a 14″ drum. A cymbal (0.85) is no piston: open. `c`: the cosine between the
/// head's normal and the way to the mic. The cut-off in Hz (`f32::MAX`: open).
fn head_beam(g: PieceGeom, c: f32) -> f32 {
    if g.dipole >= 0.8 {
        return f32::MAX;
    }
    let a = if g.radius > 0.0 {
        g.radius
    } else if g.dipole < 0.6 {
        0.28
    } else {
        0.18
    };
    let c = c.clamp(-1.0, 1.0);
    let s = (1.0 - c * c).max(0.0).sqrt().max(1e-3);
    2.0 * C / (std::f32::consts::PI * a * s)
}

/// Where a head's beam stops (drums-gen 2): the beam is a far-field pattern — a mic nearer
/// the head than its Rayleigh distance `π a² f / c` hears the part of the head beside it,
/// not the beam, so above `f_nf = r c / (π a²)` the head's highs reach it as on its axis.
/// The close mics, 15-45 cm from their drums, are in that near field above a kilohertz or
/// less: with the far-field beam alone the snare's own mic heard its head through a
/// 1.4-kHz low-pass and the overheads, 3-4 ms later, carried the crack (the recordings'
/// strongest 2-16 kHz arrival is within 1.5 ms of the first sound; ours was 4.5 ms). The
/// beam's low-pass at `fc` rests on a floor at `fc / f_nf` of the input (a one-pole's level
/// at `f_nf`), so the response falls from `fc` and stays flat from `f_nf`. The near field
/// is the column over the head: a mic within 1.4 radii of its axis is in it, one beyond 1.8
/// is not (a kick's highs beside it, in the snare's mic, stay beamed), between they blend.
/// `s`: the sine of the mic's angle off the head's axis. Returns the floor (0: the
/// far-field beam alone; 1: open).
fn near_field(g: PieceGeom, fc: f32, r: f32, s: f32) -> f32 {
    if fc == f32::MAX {
        return 1.0;
    }
    let a = if g.radius > 0.0 {
        g.radius
    } else if g.dipole < 0.6 {
        0.28
    } else {
        0.18
    };
    let f_nf = r * C / (std::f32::consts::PI * a * a);
    let x = ((1.8 - r * s.abs() / a) / 0.4).clamp(0.0, 1.0);
    let column = x * x * (3.0 - 2.0 * x);
    column * (fc / f_nf.max(1.0)).min(1.0)
}

/// A mic nearer a drum than this hears the part of its head beside it (its own close mic);
/// one farther, the whole head (another draw of its highs: the voice's side).
const NEAR_M: f32 = 0.5;

/// A path under this share of the loudest one into its mic is dropped (−40 dB).
const PATH_CULL: f32 = 0.01;

/// The fewest samples a path is delayed (its cubic interpolation reads one ahead).
const MIN_DELAY: f32 = 2.0;

impl ModelEngine {
    /// The kit's geometry, mics and room (each snapshot; allocation-free). `snap`: take the
    /// new delays and levels at once (a new kit, a reset) rather than gliding to them.
    pub fn set_geometry(
        &mut self,
        pieces: &[PieceGeom],
        mics: &[MicGeom],
        room: RoomGeom,
        snap: bool,
    ) {
        self.pieces = pieces.len().min(MAX_PIECES);
        self.mics = mics.len().min(MAX_MICS);
        self.geom_p[..self.pieces].copy_from_slice(&pieces[..self.pieces]);
        self.geom_m[..self.mics].copy_from_slice(&mics[..self.mics]);
        let room_changed = room != self.room_geom;
        self.room_geom = room;
        if room_changed || snap {
            let size = (room.width * room.depth * room.height).max(1.0);
            let mean = math::cbrt(size);
            self.room.set(mean, room.t60, room.t60 * 0.55);
        }
        let old = self.n_paths;
        let mut n = 0;
        let fs = self.fs;
        let (w, d, h, back) = (room.width, room.depth, room.height, room.back);
        for p in 0..self.pieces {
            let g = self.geom_p[p];
            let s = g.pos;
            let disp = [-g.normal[0], -g.normal[1], -g.normal[2]];
            // The source and its images: direct, floor, ceiling, then the walls.
            let images: [([f32; 3], f32); 7] = [
                (s, 1.0),
                ([s[0], s[1], -s[2]], room.floor_reflect),
                ([s[0], s[1], 2.0 * h - s[2]], room.reflect),
                ([w - s[0], s[1], s[2]], room.reflect),
                ([-w - s[0], s[1], s[2]], room.reflect),
                ([s[0], -2.0 * back - s[1], s[2]], room.reflect),
                ([s[0], 2.0 * (d - back) - s[1], s[2]], room.reflect),
            ];
            for m in 0..self.mics {
                let mic = self.geom_m[m];
                let cell = self.cells[p][m];
                if cell == 0.0 {
                    continue;
                }
                let lvl = cell * mic.level / mic.norm.max(1e-6);
                let caps: &[(u8, f32)] = if mic.spread > 0.0 {
                    &[(0, -0.5), (1, 0.5)]
                } else {
                    &[(2, 0.0)]
                };
                let reach = match mic.reflections {
                    0 => 2,
                    1 => 3,
                    _ => 7,
                };
                for &(ch, side) in caps {
                    let cap = [mic.pos[0] + side * mic.spread, mic.pos[1], mic.pos[2]];
                    for (k, &(img, refl)) in images.iter().take(reach).enumerate() {
                        let v = sub(cap, img);
                        let r = len(v).max(0.03);
                        let u = [v[0] / r, v[1] / r, v[2] / r];
                        // The pattern at the angle the sound arrives from (from the mic
                        // towards the source: −u).
                        let pat = pattern_gain(mic.pattern, -dot(mic.aim, u));
                        // The piece's radiation towards the mic; reflections hear its
                        // average.
                        // A head pushes the air it moves towards and pulls it behind
                        // (a dipole, as strong either side, the sign flipped behind), and
                        // the shell's air goes all round: a mic under a head hears it
                        // inverted, one beside it hears the shell.
                        let rad = if k == 0 {
                            let c = dot(disp, u);
                            let side = if c < 0.0 { -1.0 } else { 1.0 };
                            side * ((1.0 - g.dipole) + g.dipole * c.abs())
                        } else {
                            1.0 - 0.5 * g.dipole
                        };
                        let gain = lvl * refl * pat * rad / r;
                        if gain.abs() < 1e-5 || n >= MAX_PATHS {
                            continue;
                        }
                        // A pair hears every piece's direct sound from its own centre: no
                        // comb between its capsules (the recordings' overheads are never
                        // out of phase); their places still set their levels.
                        let centred = k == 0 && ch != 2;
                        let dist = if centred {
                            len(sub(mic.pos, img)).max(0.03)
                        } else {
                            r
                        };
                        let delay = (dist / C * fs).max(MIN_DELAY);
                        let pth = &mut self.paths[n];
                        let keep = !snap && n < old && pth.piece == p as u8 && pth.mic == m as u8;
                        pth.piece = p as u8;
                        pth.mic = m as u8;
                        pth.ch = ch;
                        pth.side = if g.width <= 0.0 && g.dipole < 0.8 {
                            // A drum's head: the mic beside it hears its highs as the voice's
                            // main has them; a far one (and every reflection) the other draw
                            // (the voice's side: drums-gen 2, `DRUM_SIDE_HZ`).
                            if k == 0 && r < NEAR_M {
                                0.0
                            } else {
                                1.0
                            }
                        } else if ch == 2 && g.width > 0.0 && self.measured[p] {
                            // A measured cymbal in a mono mic (its own close mic, another's
                            // bleed): the plate's other side alone (drums-gen 2, cue 7) — the
                            // overheads' sum hears its main, and one source in both combed
                            // the hat into fixed lines (fine-structure stability 0.41
                            // against DRSKit's 0.23-0.28).
                            1.0
                        } else if ch == 2 || g.width <= 0.0 {
                            0.0
                        } else {
                            side * 2.0 * g.width
                        };
                        pth.main = if ch == 2 && g.width > 0.0 && self.measured[p] {
                            0.0
                        } else {
                            1.0
                        };
                        pth.delay_to = delay.min((self.ring_len - 4) as f32);
                        pth.gain_to = gain;
                        if !keep {
                            pth.delay = pth.delay_to;
                            pth.gain = pth.gain_to;
                        }
                        n += 1;
                        if k == 0 {
                            let c = dot(disp, u).clamp(-1.0, 1.0);
                            let src = head_beam(g, c);
                            let floor = near_field(g, src, r, (1.0 - c * c).max(0.0).sqrt());
                            self.set_colour(p, m, ch, mic, -dot(mic.aim, u), r, src, floor);
                        }
                    }
                }
            }
            // The room hears every piece alike — but a measured piece, whose tables hold
            // the recording's own room, gives it less (a sidestick's tail no longer than
            // the recording's).
            self.room_send[p] = room_send(self.measured[p]);
        }
        // A path 40 dB under the loudest a mic hears (a cymbal in the kick's mic, a far
        // reflection of the hat in a tom's) is gone under what that mic carries: dropped
        // (drums-gen 2, the cost — every path is a cubic read per sample).
        let mut loud = [0.0f32; MAX_MICS];
        for q in &self.paths[..n] {
            let m = q.mic as usize;
            loud[m] = loud[m].max(q.gain_to.abs());
        }
        // (silenced in place, not removed: a path keeps its index, so a configure that moves
        // nothing glides nothing — the same bits as before it)
        for q in &mut self.paths[..n] {
            if q.gain_to.abs() < PATH_CULL * loud[q.mic as usize] {
                q.gain_to = 0.0;
                q.gain = 0.0;
            }
        }
        self.n_paths = n;
    }

    /// fix-e — the bleed cells changed and nothing else: the paths again from the geometry
    /// as set (their gains glide to the new cells, as [`Self::set_geometry`] makes them).
    pub fn regain(&mut self) {
        let (pieces, mics, room) = (self.geom_p, self.geom_m, self.room_geom);
        let (np, nm) = (self.pieces, self.mics);
        self.set_geometry(&pieces[..np], &mics[..nm], room, false);
    }

    #[allow(clippy::too_many_arguments)]
    fn set_colour(
        &mut self,
        p: usize,
        m: usize,
        ch: u8,
        mic: MicGeom,
        cos: f32,
        r: f32,
        src_fc: f32,
        floor: f32,
    ) {
        let fs = self.fs;
        let one_pole = |hz: f32| 1.0 - math::exp(-2.0 * std::f32::consts::PI * hz / fs);
        // Off axis the highs fall (a pattern's back is duller); an omni stays open.
        let off = if mic.pattern == 0 {
            1.0
        } else {
            ((1.0 + cos) * 0.5).clamp(0.0, 1.0)
        };
        let fc_mic = 3_500.0 + 16_500.0 * off * off;
        // The head's beam binds only where it cuts below the mic's own off-axis fall; in the
        // head's near field it rests on a floor (`near_field`), open at 1.
        let (fc, floor) = if floor >= 1.0 || fc_mic <= src_fc {
            (fc_mic.min(0.45 * fs), 0.0)
        } else {
            (src_fc.min(0.45 * fs), floor)
        };
        // Close to a directional mic the bass rises (the proximity effect).
        let directional = match mic.pattern {
            0 => 0.0,
            1 => 0.5,
            2 => 0.63,
            3 => 0.75,
            _ => 1.0,
        };
        let boost_db = 9.0 * directional * ((0.25 - r) / 0.2).clamp(0.0, 1.0);
        for c in 0..2 {
            if ch != 2 && c != ch as usize {
                continue;
            }
            let col = &mut self.colour[(p * MAX_MICS + m) * 2 + c];
            col.lp_a = if fc >= 0.45 * fs - 1.0 {
                1.0
            } else {
                one_pole(fc)
            };
            col.floor = floor;
            col.shelf_a = one_pole(150.0);
            col.shelf_k = math::db_to_gain(boost_db) - 1.0;
        }
    }

    /// A strike from the sequencer (`ordinal`: its key's count on the track) or live.
    pub fn note_on(&mut self, key: u8, velocity: u8, note: u64, ordinal: Option<u32>) {
        let open = self.pulsus.hat_openness(key);
        self.hybrid.strike(key, velocity, ordinal, open); // r40:drums-hyb
        match ordinal {
            Some(o) => self.pulsus.trigger(
                key,
                velocity,
                crate::pulsus::Hit {
                    id: note,
                    tick: i64::from(o),
                    ordinal: o,
                    sequenced: true,
                },
            ),
            None => self.pulsus.note_on(key, velocity, note),
        }
        self.quiet = 0;
    }

    /// Whether nothing rings: no voice, every ring delivered, the room silent.
    pub fn at_rest(&self) -> bool {
        self.pulsus.active_voices() == 0
            && self.quiet >= self.ring_len
            && self.room.at_rest()
            && self.hybrid.at_rest()
    }

    /// fx/final — every path's delay and level at its target at once (the playhead jumped:
    /// a start, a seek, a loop's wrap), as a new kit takes them. The rings, the colours and
    /// the room are kept.
    pub fn snap(&mut self) {
        for p in &mut self.paths[..self.n_paths] {
            p.delay = p.delay_to;
            p.gain = p.gain_to;
        }
    }

    pub fn reset(&mut self) {
        self.pulsus.reset();
        self.hybrid.reset(); // r40:drums-hyb
        self.ring.iter_mut().for_each(|x| *x = 0.0);
        self.ring_s.iter_mut().for_each(|x| *x = 0.0);
        for c in self.colour.iter_mut() {
            c.lp = 0.0;
            c.shelf = 0.0;
        }
        self.room.clear();
        self.quiet = usize::MAX / 2;
        self.pquiet = [usize::MAX / 2; MAX_PIECES];
    }

    /// Render `c ≤ LANE` frames into the mics' lanes (begun by the caller).
    pub fn render(&mut self, c: usize, outs: &mut Outputs) {
        let rl = self.ring_len;
        // The pieces, dry: each articulation's voices on its piece's bus.
        let pieces = self.pieces;
        self.src[..MAX_PIECES * LANE].fill(0.0);
        self.src_s[..MAX_PIECES * LANE].fill(0.0);
        self.touched = [false; MAX_PIECES];
        {
            let (src, src_s, touched) = (&mut self.src, &mut self.src_s, &mut self.touched);
            self.pulsus.process_buses(c, &mut |bus, at, a, b| {
                let p = (bus as usize).min(MAX_PIECES - 1);
                touched[p] = true;
                for (i, (x, y)) in a.iter().zip(b).enumerate() {
                    src[p * LANE + at + i] += 0.5 * (x + y);
                    // A mono voice's side is exactly nothing.
                    src_s[p * LANE + at + i] += 0.5 * (x - y);
                }
            });
        }
        // r40:drums-hyb — the hybrid's residual, at each piece's source (the mics hear it).
        self.hybrid
            .render(c, &mut self.src, LANE, &mut self.touched);
        let any = self.touched[..pieces].iter().any(|t| *t);
        let quiet_start = self.quiet;
        if any {
            self.quiet = 0;
        } else if self.quiet >= rl && self.room.at_rest() {
            // Nothing in the rings, nothing in the room: exact silence, nothing touched.
            self.ring_pos = (self.ring_pos + c) & (rl - 1);
            return;
        } else {
            self.quiet = self.quiet.saturating_add(c);
        }
        // Each piece's silence, counted from its last sample that was not zero — the same
        // whatever the chunks: a piece silent a ring's length gives nothing on any path.
        let stride = rl + MIRROR;
        let mut pq = [0usize; MAX_PIECES];
        let mut live = [false; MAX_PIECES];
        for p in 0..pieces {
            pq[p] = self.pquiet[p];
            let (s, ss) = (
                &self.src[p * LANE..p * LANE + c],
                &self.src_s[p * LANE..p * LANE + c],
            );
            match (0..c).rev().find(|&t| s[t] != 0.0 || ss[t] != 0.0) {
                Some(t) => {
                    live[p] = true;
                    self.pquiet[p] = c - 1 - t;
                }
                None => self.pquiet[p] = self.pquiet[p].saturating_add(c),
            }
            if live[p] || pq[p] < rl {
                for t in 0..c {
                    let q = (self.ring_pos + t) & (rl - 1);
                    let at = p * stride + q;
                    let (x, y) = (self.src[p * LANE + t], self.src_s[p * LANE + t]);
                    self.ring[at] = x;
                    self.ring_s[at] = y;
                    // (the mirror past the ring's end: a window read in one run)
                    if q < MIRROR {
                        self.ring[at + rl] = x;
                        self.ring_s[at + rl] = y;
                    }
                }
            }
        }
        // The paths, grouped by piece × mic × channel, each group coloured then added — up
        // to `BATCH` groups at once, their colours side by side, added in group order (the
        // same sums as one group after another).
        let mut i = 0;
        let mut batch = [Group::default(); BATCH];
        let mut nb = 0;
        while i < self.n_paths || nb > 0 {
            if i < self.n_paths {
                let (p, m, ch) = (self.paths[i].piece, self.paths[i].mic, self.paths[i].ch);
                let mut j = i;
                while j < self.n_paths
                    && self.paths[j].piece == p
                    && self.paths[j].mic == m
                    && self.paths[j].ch == ch
                {
                    j += 1;
                }
                let cidx = (p as usize * MAX_MICS + m as usize) * 2 + (ch as usize & 1);
                let pi = p as usize;
                let settled = self.paths[i..j]
                    .iter()
                    .all(|q| q.delay == q.delay_to && q.gain == q.gain_to);
                let at_rest = self.colour[cidx].lp == 0.0 && self.colour[cidx].shelf == 0.0;
                // A piece a ring's length silent, its paths settled and its colour at rest:
                // all it would give is zeros — skipped (the same bits as rendering them). So
                // is a group whose every path is dropped (`PATH_CULL`), at rest.
                let culled = self.paths[i..j]
                    .iter()
                    .all(|q| q.gain == 0.0 && q.gain_to == 0.0);
                if at_rest && settled && ((!live[pi] && pq[pi] >= rl) || culled) {
                    i = j;
                    continue;
                }
                // The sample from which the colour comes to rest: every ring delivered, the
                // kit's or this piece's.
                let kit = if any {
                    usize::MAX
                } else {
                    rl.saturating_sub(quiet_start)
                };
                let own = if live[pi] {
                    usize::MAX
                } else {
                    rl.saturating_sub(pq[pi])
                };
                batch[nb] = Group {
                    from: i,
                    to: j,
                    colour: cidx,
                    piece: pi,
                    mic: m as usize,
                    ch,
                    rest: kit.min(own),
                };
                nb += 1;
                i = j;
                if nb < BATCH && i < self.n_paths {
                    continue;
                }
            }
            if nb == 0 {
                break;
            }
            // The batch's paths, each group into its own row.
            for (g, grp) in batch[..nb].iter().enumerate() {
                let base = grp.piece * stride;
                let acc = &mut self.scratch[g * LANE..g * LANE + c];
                acc.fill(0.0);
                for pth in &mut self.paths[grp.from..grp.to] {
                    if pth.gain == 0.0 && pth.gain_to == 0.0 {
                        // a path dropped (`PATH_CULL`): nothing to read
                        continue;
                    }
                    if pth.delay == pth.delay_to && pth.gain == pth.gain_to {
                        // Settled: one set of weights for the whole chunk, its window read in
                        // one run (the ring's mirror past its end).
                        let di = pth.delay.floor();
                        let frac = pth.delay - di;
                        let (back, f) = if frac > 0.0 {
                            (1, 1.0 - frac)
                        } else {
                            (0, 0.0)
                        };
                        let w = catmull_rom(f);
                        let start = self.ring_pos as i64 - di as i64 - back;
                        let s0 = base + ((start - 1) as usize & (rl - 1));
                        let (x, y) = (&self.ring[s0..s0 + c + 3], &self.ring_s[s0..s0 + c + 3]);
                        if pth.main != 0.0 {
                            settled_read(acc, x, w, pth.main, pth.gain, pth.side, y);
                        } else if pth.side != 0.0 {
                            settled_read(acc, y, w, pth.side, pth.gain, 0.0, y);
                        }
                        continue;
                    }
                    for (t, a) in acc.iter_mut().enumerate() {
                        if pth.delay != pth.delay_to {
                            pth.delay += self.glide * (pth.delay_to - pth.delay);
                            if (pth.delay - pth.delay_to).abs() < 1e-4 {
                                pth.delay = pth.delay_to;
                            }
                        }
                        if pth.gain != pth.gain_to {
                            pth.gain += self.glide * (pth.gain_to - pth.gain);
                            if (pth.gain - pth.gain_to).abs() < 1e-7 {
                                pth.gain = pth.gain_to;
                            }
                        }
                        // Whole and fractional delay apart: the fraction never depends on
                        // where the ring stands (an f32 position would round with its size).
                        let di = pth.delay.floor();
                        let frac = pth.delay - di;
                        let head = (self.ring_pos + t) as i64 - di as i64;
                        let (k, f) = if frac > 0.0 {
                            (head - 1, 1.0 - frac)
                        } else {
                            (head, 0.0)
                        };
                        // Cubic (Catmull-Rom) between k and k + 1 — the settled path's own weights.
                        let w = catmull_rom(f);
                        let cubic = |ring: &[f32]| {
                            let at = |o: i64| ring[base + ((k + o) as usize & (rl - 1))];
                            w[0] * at(-1) + w[1] * at(0) + w[2] * at(1) + w[3] * at(2)
                        };
                        let mut v = if pth.main != 0.0 {
                            pth.main * cubic(&self.ring)
                        } else {
                            0.0
                        };
                        if pth.side != 0.0 {
                            v += pth.side * cubic(&self.ring_s);
                        }
                        *a += v * pth.gain;
                    }
                }
            }
            // Their colours side by side (each a one-pole low-pass resting on its floor, then
            // the proximity shelf).
            let mut st = [[0.0f32; BATCH]; 6];
            let mut rest = [usize::MAX; BATCH];
            for (g, grp) in batch[..nb].iter().enumerate() {
                let col = &self.colour[grp.colour];
                for (k, v) in [
                    col.lp,
                    col.shelf,
                    col.lp_a,
                    col.floor,
                    col.shelf_a,
                    col.shelf_k,
                ]
                .into_iter()
                .enumerate()
                {
                    st[k][g] = v;
                }
                rest[g] = grp.rest;
            }
            let [mut lp, mut sh, la, fl, sa, sk] = st;
            let rows = &mut self.scratch[..BATCH * LANE];
            for t in 0..c {
                for g in 0..BATCH {
                    let x = rows[g * LANE + t];
                    // Every ring delivered — the kit's, or this piece's — the colour comes to
                    // rest at an exact sample, so a unit (or a piece) at rest is as good as new
                    // whatever the chunks were.
                    let keep = if t < rest[g] { 1.0 } else { 0.0 };
                    lp[g] *= keep;
                    sh[g] *= keep;
                    lp[g] += la[g] * (x - lp[g]);
                    let v = lp[g] + fl[g] * (x - lp[g]);
                    sh[g] += sa[g] * (v - sh[g]);
                    rows[g * LANE + t] = v + sk[g] * sh[g];
                }
            }
            // Into the mics' lanes, group after group.
            for (g, grp) in batch[..nb].iter().enumerate() {
                let col = &mut self.colour[grp.colour];
                col.lp = lp[g];
                col.shelf = sh[g];
                let mic = self.geom_m[grp.mic];
                let (pl, pr) = if grp.ch == 2 {
                    pan_law(mic.pan)
                } else {
                    (1.0, 1.0)
                };
                let (l, r) = outs.lane(grp.mic, c);
                let y = &self.scratch[g * LANE..g * LANE + c];
                match grp.ch {
                    0 => l.iter_mut().zip(y).for_each(|(a, y)| *a += y),
                    1 => r.iter_mut().zip(y).for_each(|(a, y)| *a += y),
                    _ => {
                        for ((a, b), y) in l.iter_mut().zip(r.iter_mut()).zip(y) {
                            *a += y * pl;
                            *b += y * pr;
                        }
                    }
                }
            }
            nb = 0;
        }
        // The room's late field, into every mic by how far into the room it listens.
        if any || !self.room.at_rest() {
            for t in 0..c {
                let mut x = 0.0;
                for p in 0..pieces {
                    x += self.src[p * LANE + t] * self.room_send[p];
                }
                let (tl, tr) = self.room.tick(x);
                for m in 0..self.mics {
                    let mic = self.geom_m[m];
                    if mic.room == 0.0 {
                        continue;
                    }
                    let g = mic.room * mic.level;
                    let (l, r) = outs.lane(m, c);
                    if mic.spread > 0.0 {
                        l[t] += tl * g;
                        r[t] += tr * g;
                    } else {
                        let (pl, pr) = pan_law(mic.pan);
                        let mono = 0.5 * (tl + tr) * g;
                        l[t] += mono * pl;
                        r[t] += mono * pr;
                    }
                }
            }
        }
        self.ring_pos = (self.ring_pos + c) & (rl - 1);
    }
}

#[cfg(test)]
mod beam_tests {
    use super::*;

    #[test]
    fn a_head_beams_its_highs_along_its_normal_and_a_cymbal_does_not() {
        let kick = PieceGeom {
            dipole: 0.55,
            radius: 0.28,
            ..Default::default()
        };
        // On its axis open; beside it (a snare or tom mic) below a kilohertz.
        assert!(head_beam(kick, 1.0) > 20_000.0);
        let side = head_beam(kick, 0.0);
        assert!(side > 500.0 && side < 1_000.0, "{side} Hz");
        // A smaller head beams less: a 10-inch tom beside it keeps more highs.
        let tom = PieceGeom {
            radius: 0.127,
            ..kick
        };
        assert!(head_beam(tom, 0.0) > 2.0 * side);
        // No radius given: the kind's (a kick's dipole share: 22 inches).
        let unset = PieceGeom {
            radius: 0.0,
            ..kick
        };
        assert_eq!(head_beam(unset, 0.0), side);
        let cymbal = PieceGeom {
            dipole: 0.85,
            radius: 0.25,
            ..Default::default()
        };
        assert_eq!(head_beam(cymbal, 0.0), f32::MAX);
    }

    #[test]
    fn a_close_mic_in_the_heads_near_field_hears_its_highs_and_a_far_one_the_beam() {
        // The studio kit's snare (14 inches) and its own mic: 21.5 cm away, 65 degrees off
        // the axis, over the rim — in the column over the head and inside its Rayleigh
        // distance above 740 Hz: open (the far-field beam alone cut it at 1.35 kHz).
        let snare = PieceGeom {
            dipole: 0.7,
            radius: 0.178,
            ..Default::default()
        };
        let (r, s) = (0.215f32, 0.906f32);
        let fc = head_beam(snare, (1.0 - s * s).sqrt());
        assert!(fc > 1_200.0 && fc < 1_500.0, "{fc} Hz");
        assert_eq!(near_field(snare, fc, r, s), 1.0);
        // The overheads, 1.4 m off and 2.7 radii off its axis: outside the column over the
        // head, the far-field beam alone.
        let (r, s) = (1.40f32, 0.34f32);
        let fc = head_beam(snare, (1.0 - s * s).sqrt());
        assert!(fc > 3_000.0 && fc < 4_500.0, "{fc} Hz");
        assert_eq!(near_field(snare, fc, r, s), 0.0);
        // A mic just over the rim but farther up (40 cm, 1.4 radii off the axis): open
        // above its Rayleigh frequency, the beam's floor its level there.
        let (r, s) = (0.40f32, 0.62f32);
        let fc = head_beam(snare, (1.0 - s * s).sqrt());
        let floor = near_field(snare, fc, r, s);
        assert!(floor > 0.5 && floor <= 1.0, "{floor}");
        // A kick's highs beside it, in the snare's mic (1.8 radii off its axis): beamed.
        let kick = PieceGeom {
            dipole: 0.55,
            radius: 0.28,
            ..Default::default()
        };
        let fc = head_beam(kick, 0.1);
        assert_eq!(near_field(kick, fc, 0.51, 0.99), 0.0);
        // A cymbal is open whatever the distance.
        let cymbal = PieceGeom {
            dipole: 0.85,
            ..Default::default()
        };
        assert_eq!(near_field(cymbal, head_beam(cymbal, 0.0), 2.0, 1.0), 1.0);
    }
}
