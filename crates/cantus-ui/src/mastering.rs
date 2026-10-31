//! The mastering assistant (round 34, ADR 0084): it listens to the mix as it reaches the
//! master, and proposes a chain of Cantus's own effects — EQ, glue compression, stereo,
//! a true-peak limiter — with every setting, so the master lands on a platform's loudness
//! under its ceiling with the balance of a genre's curve or of a reference track. The
//! chain is ordinary inserts on the master, applied as one undo step and editable by hand
//! afterwards; nothing is hidden in it.
//!
//! **How it decides** (ideas of reference mastering — Matchering's: match the reference's
//! loudness, its frequency balance and its width, then limit — re-implemented here from
//! their description, with our own measurements and our own units; no code taken):
//!
//! 1. *Balance.* The mix's octave spectrum against the target's, both relative to their
//!    mean over the octaves, corrected by an EQ of a cut under 20 Hz, a low shelf, eight
//!    octave-wide bells and a high shelf — in closed loop: the EQ is run over the mix, the
//!    result measured, the gains moved, three times. A genre curve is a guide: half the
//!    difference, at most ±3 dB a band; a reference track is matched closer: ±6 dB.
//! 2. *Glue.* A loudness range wider than the genre keeps gets a slow compressor (2:1 or
//!    1.5:1) whose threshold sits under the loud passages' short-term level.
//! 3. *Width and mono.* A mix that folds badly to mono (more than 2 LU lost, or a window
//!    under −0.2 correlation) is narrowed a little and its bass made mono under 120 Hz; one
//!    much narrower than the target is widened, by at most 2 dB of side.
//! 4. *Loudness.* A true-peak limiter at the platform's ceiling, its gain found by trial
//!    over the whole mix (the chain run on it, the loudness measured, the gain moved — a
//!    secant search to within 0.1 LU), a target under the mix's own level reached by the
//!    EQ's output trim instead.
//!
//! The trials run the chain over the rendered mix (`cantus_engine::listen::run_chain`); the
//! numbers the user is given at the end come from a real render with the chain in place.

use super::targets::{Curve, Platform};
use cantus_core::{eq_key, EffectKind, EffectSlot};
use cantus_dsp::speculum::ear::{Ear, Reading, OCTAVES};

/// What the master aims at.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub platform: &'static Platform,
    /// The balance to reach, dB per octave against the 1 kHz octave.
    pub curve: [f32; OCTAVES],
    /// A genre's id, or `None` for a reference track's balance.
    pub curve_id: Option<&'static str>,
    pub lra: (f32, f32),
    pub width_db: f32,
}

impl Target {
    /// A platform with a genre's curve.
    pub fn genre(platform: &'static Platform, curve: &'static Curve) -> Target {
        Target {
            platform,
            curve: curve.octaves,
            curve_id: Some(curve.id),
            lra: curve.lra,
            width_db: curve.width_db,
        }
    }

    /// A platform with a reference track's balance and width (as read by an [`Ear`]).
    pub fn reference(platform: &'static Platform, reference: &Reading) -> Target {
        let k1 = reference.octaves_db[5];
        let curve = std::array::from_fn(|o| {
            let d = reference.octaves_db[o] - k1;
            if d.is_finite() {
                d
            } else {
                -30.0
            }
        });
        let lra = reference.range_lu.unwrap_or(8.0) as f32;
        Target {
            platform,
            curve,
            curve_id: None,
            lra: ((lra - 2.0).max(1.0), lra + 2.0),
            width_db: if reference.width_db.is_finite() {
                reference.width_db
            } else {
                -30.0
            },
        }
    }

    fn eq_cap(&self) -> (f32, f32) {
        match self.curve_id {
            Some(_) => (0.5, 3.0),
            None => (0.8, 6.0),
        }
    }
}

/// One thing the assistant did and why: a key in the `listen` area and its arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct Why {
    pub kind: EffectKind,
    pub key: &'static str,
    pub args: Vec<(&'static str, String)>,
}

/// A proposed master chain.
#[derive(Clone, Debug)]
pub struct Proposal {
    pub target: Target,
    /// The chain, in order, ids from the caller's `new_slot`.
    pub chain: Vec<EffectSlot>,
    pub why: Vec<Why>,
    /// The mix as it reached the master.
    pub before: Reading,
    /// The chain's trial over the mix (the master strip after it included).
    pub predicted: Reading,
    /// The master strip after the chain (its fader, balance, width, polarity).
    pub post: Post,
    /// How far under the platform's loudness the master was left to keep its true peak
    /// under the ceiling, LU (`None`: on target).
    pub short_of_target_lu: Option<f64>,
}

/// The master strip, which plays after the master chain: width, then the fader and the
/// balance, then the polarity — what the engine does (`graph/master_run.rs`), so a trial
/// hears what the master will (review 1: only the fader was counted, and a width of 1.5
/// put the true peak over the ceiling).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Post {
    pub gain_db: f32,
    pub pan: f32,
    pub width: f32,
    pub invert: bool,
}

impl Default for Post {
    fn default() -> Self {
        Post {
            gain_db: 0.0,
            pan: 0.0,
            width: 1.0,
            invert: false,
        }
    }
}

impl Post {
    /// The project's master strip.
    pub fn of(master: &cantus_core::model::Master) -> Post {
        Post {
            gain_db: master.gain_db,
            pan: master.pan,
            width: master.width,
            invert: master.phase_invert,
        }
    }

    /// Play the strip over interleaved stereo `audio`.
    pub fn apply(&self, audio: &mut [f32]) {
        if *self == Post::default() {
            return;
        }
        let w = self.width.clamp(0.0, 2.0);
        let g = cantus_core::db_to_linear(self.gain_db);
        let (gl, gr) = cantus_core::balance_gains(self.pan);
        let sign = if self.invert { -1.0 } else { 1.0 };
        for f in audio.chunks_exact_mut(2) {
            let (mut l, mut r) = (f[0], f[1]);
            if self.width != 1.0 {
                let (mid, side) = ((l + r) * 0.5, (l - r) * 0.5 * w);
                l = mid + side;
                r = mid - side;
            }
            f[0] = l * g * gl * sign;
            f[1] = r * g * gr * sign;
        }
    }
}

fn read(audio: &[f32], sr: u32) -> Reading {
    let mut ear = Ear::new(sr as f32, audio.len() as f64 / 2.0 / sr as f64);
    // In blocks: no whole-length copies of the two channels beside the trial's.
    let (mut l, mut r) = (vec![0f32; 4_096], vec![0f32; 4_096]);
    for block in audio.chunks(8_192) {
        let n = block.len() / 2;
        for (i, f) in block.chunks_exact(2).enumerate() {
            l[i] = f[0];
            r[i] = f[1];
        }
        ear.push(&l[..n], &r[..n]);
    }
    ear.whole()
}

/// The octaves relative to their mean over 63 Hz … 8 kHz (the shape, not the level).
fn shape(octaves: &[f32; OCTAVES]) -> [f32; OCTAVES] {
    let fin = |x: f32| if x.is_finite() { x } else { -120.0 };
    let mean = (1..=8).map(|o| fin(octaves[o])).sum::<f32>() / 8.0;
    std::array::from_fn(|o| fin(octaves[o]) - mean)
}

/// The EQ the assistant uses for octave gains `g`, and the output trim `out` (dB).
fn eq_slot(mut slot: EffectSlot, g: &[f32; OCTAVES], out: f32) -> EffectSlot {
    let s = &mut slot.spec;
    let mut band = 0;
    let mut put = |s: &mut cantus_core::EffectSpec, shape: f32, freq: f32, gain: f32, q: f32| {
        for (field, v) in [
            ("used", 1.0),
            ("on", 1.0),
            ("shape", shape),
            ("freq", freq),
            ("gain", gain),
            ("q", q),
            ("slope", 12.0),
        ] {
            s.set(&eq_key(band, field), v);
        }
        band += 1;
    };
    // EqShape's order: Bell 0, LowShelf 1, HighShelf 2, LowCut 3.
    put(s, 3.0, 20.0, 0.0, 0.71);
    for (o, gain) in g.iter().enumerate() {
        if gain.abs() < 0.3 {
            continue;
        }
        let hz = cantus_dsp::speculum::ear::octave_hz(o);
        match o {
            0 => put(s, 1.0, 45.0, *gain, 0.71),
            9 => put(s, 2.0, 11_000.0, *gain, 0.71),
            _ => put(s, 0.0, hz, *gain, 1.41),
        }
    }
    if out != 0.0 {
        s.set("out", out.clamp(-24.0, 24.0));
    }
    slot
}

/// Run `chain` over `mix`, then the master strip after it.
pub fn trial(chain: &[EffectSlot], mix: &[f32], sr: u32, post: &Post) -> Vec<f32> {
    let mut out = cantus_engine::listen::run_chain(chain, true, sr, mix);
    post.apply(&mut out);
    out
}

fn db_to_gain(db: f32) -> f32 {
    cantus_dsp::math::powf(10f32, db / 20.0)
}

/// Propose a master chain for `mix` (interleaved stereo at `sr`, as it reaches the master:
/// its chain out, its fader at unity) aimed at `target`. `new_slot` makes each insert (its
/// id); `post` is the master strip, which comes after the chain; `cancelled`
/// stops the trials between two of them (`None` back).
pub fn propose(
    mix: &[f32],
    sr: u32,
    target: &Target,
    post: &Post,
    new_slot: &mut dyn FnMut(EffectKind) -> EffectSlot,
    cancelled: &dyn Fn() -> bool,
) -> Option<Proposal> {
    propose_passes(mix, sr, target, post, new_slot, cancelled, 3)
}

/// [`propose`] with at most `passes` searches of the limiter's gain (each after its
/// ceiling was lowered by what the master strip after it still added).
pub(crate) fn propose_passes(
    mix: &[f32],
    sr: u32,
    target: &Target,
    post: &Post,
    new_slot: &mut dyn FnMut(EffectKind) -> EffectSlot,
    cancelled: &dyn Fn() -> bool,
    passes: usize,
) -> Option<Proposal> {
    let passes = passes.max(1);
    let before = read(mix, sr);
    let mut why = Vec::new();
    let arg = |x: f64| format!("{x:.1}");

    // 1. Balance, in closed loop over the octaves.
    let (k, cap) = target.eq_cap();
    let want = shape(&target.curve);
    let mut g = [0f32; OCTAVES];
    let eq_base = new_slot(EffectKind::Eq);
    let mut measured = shape(&before.octaves_db);
    for _ in 0..3 {
        if cancelled() {
            return None;
        }
        for o in 0..OCTAVES {
            g[o] = (g[o] + k * (want[o] - measured[o])).clamp(-cap, cap);
        }
        let eq = eq_slot(eq_base.clone(), &g, 0.0);
        measured = shape(&read(&trial(&[eq], mix, sr, &Post::default()), sr).octaves_db);
    }
    let moved: Vec<String> = (0..OCTAVES)
        .filter(|o| g[*o].abs() >= 0.3)
        .map(|o| {
            let name = cantus_dsp::speculum::ear::OCTAVE_NAMES[o];
            match name.strip_suffix('k') {
                Some(k) => format!("{k} kHz {:+.1} dB", g[o]),
                None => format!("{name} Hz {:+.1} dB", g[o]),
            }
        })
        .collect();
    why.push(Why {
        kind: EffectKind::Eq,
        key: if moved.is_empty() {
            "listen.why.eq_flat"
        } else {
            "listen.why.eq"
        },
        args: vec![("bands", moved.join(", "))],
    });

    // 2. Glue, by the loudness range.
    let after_eq = read(
        &trial(
            &[eq_slot(eq_base.clone(), &g, 0.0)],
            mix,
            sr,
            &Post::default(),
        ),
        sr,
    );
    let lra = after_eq.range_lu.unwrap_or(0.0) as f32;
    let loud = after_eq.short_term_max_lufs.unwrap_or(-20.0) as f32;
    let comp = if lra > target.lra.1 {
        let (ratio, under) = if lra > target.lra.1 + 3.0 {
            (2.0, 8.0)
        } else {
            (1.5, 6.0)
        };
        let mut c = new_slot(EffectKind::Compressor);
        for (key, v) in [
            ("thr", (loud - under).clamp(-60.0, -1.0)),
            ("ratio", ratio),
            ("knee", 6.0),
            ("att", 30.0),
            ("rel", 150.0),
            ("auto_rel", 1.0),
            ("makeup", 0.0),
            ("auto_gain", 0.0),
            ("mix", 1.0),
        ] {
            c.spec.set(key, v);
        }
        why.push(Why {
            kind: EffectKind::Compressor,
            key: "listen.why.glue",
            args: vec![
                ("lra", arg(lra as f64)),
                ("most", arg(target.lra.1 as f64)),
                ("ratio", arg(ratio as f64)),
            ],
        });
        Some(c)
    } else {
        None
    };

    // 3. Width and mono.
    let folds_badly = before.mono_loss_lu.is_some_and(|m| m < -2.0)
        || before.correlation_min.is_some_and(|c| c < -0.2);
    let low_side = (0..2).any(|o| before.side_octaves_db[o] - before.octaves_db[o] > -10.0);
    let narrow = before.width_db.is_finite() && before.width_db < target.width_db - 3.0;
    let stereo = if folds_badly || low_side || narrow {
        let mut s = new_slot(EffectKind::Stereo);
        if folds_badly || low_side {
            s.spec.set("mono", 120.0);
        }
        let width = if folds_badly {
            0.85
        } else if narrow {
            db_to_gain((target.width_db - before.width_db).min(2.0))
        } else {
            1.0
        };
        s.spec.set("width", width);
        why.push(Why {
            kind: EffectKind::Stereo,
            key: if folds_badly {
                "listen.why.mono"
            } else if narrow {
                "listen.why.widen"
            } else {
                "listen.why.bass_mono"
            },
            args: vec![
                ("width", arg(before.width_db as f64)),
                ("loss", arg(before.mono_loss_lu.unwrap_or(0.0))),
            ],
        });
        Some(s)
    } else {
        None
    };

    // 4. Loudness: the limiter at the ceiling, its gain by trial — and the ceiling lowered
    //    by what the master strip after it adds to the peaks (its fader, its width), found
    //    the same way: the trial's true peak over the platform's, taken off, at most thrice.
    let mut ceiling = (target.platform.ceiling_dbtp - post.gain_db.max(0.0)).clamp(-30.0, 0.0);
    let mut lim = new_slot(EffectKind::Limiter);
    for (key, v) in [
        ("tp", 1.0),
        (
            "style",
            if target.platform.lufs > -11.0 {
                2.0
            } else {
                0.0
            },
        ),
        ("look", 2.0),
    ] {
        lim.spec.set(key, v);
    }
    let build = |gain: f32, ceiling: f32| -> Vec<EffectSlot> {
        let mut l = lim.clone();
        l.spec.set("gain", gain.clamp(0.0, 30.0));
        l.spec.set("ceiling", ceiling);
        let mut chain = vec![eq_slot(eq_base.clone(), &g, gain.min(0.0))];
        chain.extend(comp.clone());
        chain.extend(stereo.clone());
        chain.push(l);
        chain
    };
    let loudness = |gain: f32, ceiling: f32| -> f64 {
        read(&trial(&build(gain, ceiling), mix, sr, post), sr)
            .integrated_lufs
            .unwrap_or(-70.0)
    };
    let mut x1 = 0f32;
    for pass in 0..passes {
        let mut x0 = 0f32;
        let mut y0 = loudness(x0, ceiling);
        x1 = (x0 + (target.platform.lufs - y0) as f32).clamp(-24.0, 30.0);
        let mut y1 = loudness(x1, ceiling);
        for _ in 0..6 {
            let err = target.platform.lufs - y1;
            if err.abs() < 0.1 || cancelled() {
                break;
            }
            let slope = if (x1 - x0).abs() > 1e-3 {
                ((y1 - y0) / (x1 - x0) as f64).clamp(0.2, 1.5)
            } else {
                1.0
            };
            let x2 = (x1 + (err / slope) as f32).clamp(-24.0, 30.0);
            (x0, y0) = (x1, y1);
            x1 = x2;
            y1 = loudness(x1, ceiling);
        }
        let tp = read(&trial(&build(x1, ceiling), mix, sr, post), sr).true_peak_dbtp;
        let over = tp - target.platform.ceiling_dbtp;
        // Lowered only when another search follows (review 2: the last pass lowered it
        // and kept the gain found for the higher one).
        if over <= 0.0 || cancelled() || ceiling <= -30.0 || pass + 1 == passes {
            break;
        }
        ceiling = (ceiling - over - 0.05).max(-30.0);
    }
    if cancelled() {
        return None;
    }
    // Never over the ceiling (review 3: when the searches ran out still over, the proposal
    // shipped a true peak above it): the gain comes down by the excess until the peak is
    // under — and the proposal says how far under the target's loudness that leaves it.
    let mut lowered = false;
    for _ in 0..8 {
        let r = read(&trial(&build(x1, ceiling), mix, sr, post), sr);
        if r.true_peak_dbtp <= target.platform.ceiling_dbtp || cancelled() {
            break;
        }
        x1 -= r.true_peak_dbtp - target.platform.ceiling_dbtp + 0.05;
        lowered = true;
    }
    let chain = build(x1, ceiling);
    why.push(Why {
        kind: EffectKind::Limiter,
        key: "listen.why.limit",
        args: vec![
            ("gain", arg(x1 as f64)),
            ("lufs", arg(target.platform.lufs)),
            // The limiter's own ceiling (review 2), under the platform's when the master
            // strip after it adds to the peaks.
            ("ceiling", arg(ceiling as f64)),
            ("platform", target.platform.name.to_string()),
        ],
    });
    let predicted = read(&trial(&chain, mix, sr, post), sr);
    let short_of_target_lu = lowered
        .then(|| target.platform.lufs - predicted.integrated_lufs.unwrap_or(-70.0))
        .filter(|lu| *lu > 0.1);
    Some(Proposal {
        target: target.clone(),
        chain,
        why,
        before,
        predicted,
        post: *post,
        short_of_target_lu,
    })
}

/// The commands that put `proposal`'s chain on the master as one undo step: the master's
/// own inserts out first when `replace`, the chain after what stays.
pub fn apply_command(
    project: &cantus_core::Project,
    proposal: &Proposal,
    replace: bool,
) -> cantus_core::Command {
    use cantus_core::{Command, StripId};
    let mut commands = Vec::new();
    if replace {
        for e in &project.master.effects {
            commands.push(Command::RemoveEffect {
                strip: StripId::MASTER,
                effect: e.id,
            });
        }
    }
    let base = if replace {
        0
    } else {
        project.master.effects.len()
    };
    for (i, slot) in proposal.chain.iter().enumerate() {
        commands.push(Command::InsertEffect {
            strip: StripId::MASTER,
            index: base + i,
            slot: Box::new(slot.clone()),
        });
    }
    if !project.master.fx_enabled {
        commands.push(Command::SetMasterFxEnabled { enabled: true });
    }
    Command::Batch {
        label_key: "cmd.master_assist".into(),
        commands,
    }
}
