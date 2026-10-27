//! The application's side of round 31's editing (ADRs 0070, 0071): crossfades between
//! overlapping clips and clip gain envelopes as edits (each one undo step), the settings
//! that steer them, and effects inserted on their starting preset.
//!
//! The arrangement's gestures (`views::arrangement_audio*`) call these; so do the menus
//! and the tests. The envelope's point operations are pure functions over an
//! [`AudioRegion`], like `audio_clips::with_*`.

use crate::app::CantusApp;
use crate::clips::ClipRef;
use cantus_core::clip_gain::{GainEnvelope, GainPoint, ENV_MAX_DB, ENV_MIN_DB};
use cantus_core::crossfade::{self, XfadeCurve};
use cantus_core::{AudioRegion, Clip, Command, EffectKind, EffectSlot, MergeKey, Shape, Ticks};

// ---- settings and inserts ------------------------------------------------------------

impl CantusApp {
    /// Tell the command bus what the settings say of automatic crossfades (ADR 0070): at
    /// start (`assemble`, and an app built from parts) and whenever the setting changes.
    pub fn apply_editing_settings(&mut self) {
        self.bus
            .set_auto_crossfades(self.settings.auto_crossfades());
    }

    /// Automatic crossfades on or off (Settings ▸ Editing, the timeline's menu).
    pub fn set_auto_crossfade(&mut self, on: bool) {
        self.settings.auto_crossfade = on;
        self.apply_editing_settings();
        self.store_settings();
    }

    /// The curve automatic crossfades take.
    pub fn set_auto_crossfade_curve(&mut self, curve: XfadeCurve) {
        self.settings.crossfade_curve = curve;
        self.apply_editing_settings();
        self.store_settings();
    }

    /// *Insert effects at their neutral settings* (ADR 0071).
    pub fn set_insert_neutral(&mut self, on: bool) {
        self.settings.insert_neutral = on;
        self.store_settings();
    }

    /// Show or hide the audio clips' gain envelopes in the arrangement.
    pub fn toggle_clip_gain(&mut self) {
        self.settings.show_clip_gain = !self.settings.show_clip_gain;
        self.store_settings();
    }

    /// A new insert of `kind` as it starts (ADR 0071): on its starting preset, unless the
    /// user asked for neutral inserts. Every way an effect is inserted from the interface
    /// (a slot's menu, the picker, a return, a swap) takes its slot from here.
    pub fn new_insert(&mut self, kind: EffectKind) -> EffectSlot {
        let mut slot = self.bus.new_effect(kind);
        if !self.settings.insert_neutral {
            cantus_core::starting::apply(&mut slot.spec);
        }
        slot
    }
}

// ---- crossfades ------------------------------------------------------------------------

/// The overlap `r` (a clip, or any piece of a split one) crosses into: the earlier
/// window's last piece and the later window's first — the two whose edges bound it, the
/// second carrying its curve.
pub fn pair(app: &CantusApp, r: ClipRef) -> Option<(Clip, Clip)> {
    let t = app.project().track(r.0)?;
    let o = crossfade::overlap_into(t, r.1)?;
    Some((t.clip(o.left_last)?.clone(), t.clip(o.right_first)?.clone()))
}

impl CantusApp {
    /// Give the overlap `r` starts in a crossfade on `curve`, or take it away (`None`):
    /// one undo step.
    pub fn set_crossfade(&mut self, r: ClipRef, curve: Option<XfadeCurve>) -> bool {
        let Some((_, right)) = pair(self, r) else {
            return false;
        };
        let Some(mut a) = right.audio.clone() else {
            return false;
        };
        if a.cross == curve {
            return false;
        }
        a.cross = curve;
        self.exec(Command::Batch {
            label_key: "cmd.clipfx.crossfade".into(),
            commands: vec![Command::SetClipAudio {
                track: r.0,
                clip: right.id,
                audio: Some(a),
            }],
        })
    }

    /// Cross every overlap among the selected clips on `curve` (or take every one's away):
    /// one undo step. Returns how many changed.
    pub fn crossfade_selection(&mut self, curve: Option<XfadeCurve>) -> usize {
        let commands: Vec<Command> = self
            .clip_selection()
            .into_iter()
            .filter_map(|r| {
                let (_, right) = pair(self, r)?;
                let mut a = right.audio.clone()?;
                (a.cross != curve).then(|| {
                    a.cross = curve;
                    Command::SetClipAudio {
                        track: r.0,
                        clip: right.id,
                        audio: Some(a),
                    }
                })
            })
            .collect();
        // A split's pieces in the selection name one overlap: set once.
        let mut seen = std::collections::HashSet::new();
        let commands: Vec<Command> = commands
            .into_iter()
            .filter(|c| match c {
                Command::SetClipAudio { track, clip, .. } => seen.insert((*track, *clip)),
                _ => true,
            })
            .collect();
        let n = commands.len();
        if n > 0 {
            self.exec(Command::Batch {
                label_key: "cmd.clipfx.crossfade".into(),
                commands,
            });
        }
        n
    }

    /// Seal a crossfade length's drag (its frames were one step).
    pub fn end_gesture_if_xfade(&mut self, r: ClipRef) {
        self.bus
            .end_gesture_if(MergeKey::of(&("xfade-length", r.0 .0, r.1 .0)));
    }

    /// Make the crossfade `r` starts with `len` long, centred where it is (the two clips'
    /// edges move, each only as far as its audio reaches). `merge`: a drag's frames, one
    /// step.
    pub fn set_crossfade_length(&mut self, r: ClipRef, len: Ticks, merge: bool) -> bool {
        let Some(base) = xfade_base(self, r) else {
            return false;
        };
        self.set_crossfade_length_from(r, &base, len, merge)
    }

    /// [`Self::set_crossfade_length`] from where a gesture began (`base`): every frame of a
    /// drag or a knob's turn sets the clips from that state, so the result depends on where
    /// the pointer is, not on how many frames it took (review 1: from the clips already
    /// resized, a clamped side crept).
    pub fn set_crossfade_length_from(
        &mut self,
        r: ClipRef,
        base: &XfadeBase,
        len: Ticks,
        merge: bool,
    ) -> bool {
        let (left, right) = (&base.left, &base.right);
        let commands = crossfade::resize(r.0, left, right, len, base.room).unwrap_or_else(|| {
            // Back to where it began: the gesture's last frame sets that too.
            vec![
                Command::SetClipBounds {
                    track: r.0,
                    clip: left.id,
                    start: left.start,
                    length: left.length,
                },
                Command::SetClipOffset {
                    track: r.0,
                    clip: left.id,
                    offset: left.offset,
                },
                Command::SetClipBounds {
                    track: r.0,
                    clip: right.id,
                    start: right.start,
                    length: right.length,
                },
                Command::SetClipOffset {
                    track: r.0,
                    clip: right.id,
                    offset: right.offset,
                },
            ]
        });
        let cmd = Command::Batch {
            label_key: "cmd.clipfx.xfade_length".into(),
            commands,
        };
        if merge {
            self.exec_merging(cmd, MergeKey::of(&("xfade-length", r.0 .0, r.1 .0)))
        } else {
            self.exec(cmd)
        }
    }
}

/// What a crossfade's length is resized from: its two clips and how far each can grow,
/// as they were when the gesture began.
#[derive(Clone, Debug, PartialEq)]
pub struct XfadeBase {
    pub left: Clip,
    pub right: Clip,
    pub room: (Ticks, Ticks),
}

impl XfadeBase {
    /// The crossfade's length now.
    pub fn len(&self) -> Ticks {
        let (a, b) = crossfade::overlap(&self.left, &self.right);
        b - a
    }

    /// The longest it can be: each clip grown as far as its audio and its other edge
    /// allow (review 1: the editor's range comes from here, not a fixed cap).
    pub fn span(&self) -> (Ticks, Ticks) {
        let earliest = (self.right.start - self.room.0).max(self.left.start + Ticks(1));
        let latest = (self.left.end() + self.room.1).min(self.right.end());
        (earliest, latest.max(earliest + Ticks(1)))
    }
}

/// The base of the crossfade `r` starts in, as the clips are now.
pub fn xfade_base(app: &CantusApp, r: ClipRef) -> Option<XfadeBase> {
    let (left, right) = pair(app, r)?;
    let room = crossfade::room(app.project(), &left, &right);
    Some(XfadeBase { left, right, room })
}

// ---- gain envelopes ----------------------------------------------------------------------

/// The envelope a region shows: its own, or a flat one at 0 dB (what a first point is
/// added to).
pub fn envelope_of(a: &AudioRegion) -> GainEnvelope {
    a.gain_env.as_deref().cloned().unwrap_or_default()
}

/// `a` with `env` as its envelope (`None` when it has no points left).
pub fn with_envelope(a: &AudioRegion, env: GainEnvelope) -> AudioRegion {
    AudioRegion {
        gain_env: (!env.points.is_empty()).then(|| Box::new(env)),
        ..a.clone()
    }
}

/// A point added at content tick `tick` on the line (at the line's value there, or `db`):
/// the region and the new point's index.
pub fn add_point(a: &AudioRegion, tick: Ticks, db: Option<f32>) -> (AudioRegion, usize) {
    let mut env = envelope_of(a);
    let value = db
        .unwrap_or_else(|| env.db_at(tick.0 as f64))
        .clamp(ENV_MIN_DB, ENV_MAX_DB);
    // The segment it lands in keeps its shape on both sides of the new point.
    let i = env.points.partition_point(|p| p.tick <= tick);
    let mut p = GainPoint::new(tick, value);
    if let Some(prev) = i.checked_sub(1).and_then(|k| env.points.get(k)) {
        p.shape = prev.shape;
        p.tension = prev.tension;
    }
    env.points.insert(i, p);
    (with_envelope(a, env), i)
}

/// Point `i` moved to `tick` and `db`, kept between its neighbours and in range.
pub fn move_point(a: &AudioRegion, i: usize, tick: Ticks, db: f32) -> AudioRegion {
    let mut env = envelope_of(a);
    let n = env.points.len();
    if i >= n {
        return a.clone();
    }
    let lo = i
        .checked_sub(1)
        .map_or(Ticks(i64::MIN / 4), |k| env.points[k].tick);
    let hi = env
        .points
        .get(i + 1)
        .map_or(Ticks(i64::MAX / 4), |q| q.tick);
    let p = &mut env.points[i];
    p.tick = tick.max(lo).min(hi);
    p.db = if db.is_finite() {
        db.clamp(ENV_MIN_DB, ENV_MAX_DB)
    } else {
        p.db
    };
    with_envelope(a, env)
}

/// Point `i` taken away.
pub fn remove_point(a: &AudioRegion, i: usize) -> AudioRegion {
    let mut env = envelope_of(a);
    if i < env.points.len() {
        env.points.remove(i);
    }
    with_envelope(a, env)
}

/// The shape of the segment from point `i`.
pub fn set_shape(a: &AudioRegion, i: usize, shape: Shape) -> AudioRegion {
    let mut env = envelope_of(a);
    if let Some(p) = env.points.get_mut(i) {
        p.shape = shape;
        if !shape.has_tension() {
            p.tension = 0.0;
        }
    }
    with_envelope(a, env)
}

/// The bend of the segment from point `i`, set so the line passes through `db` half-way
/// (automation's tension handle, `automation::edit::tension_for_mid`).
pub fn bend_to(a: &AudioRegion, i: usize, db: f32) -> AudioRegion {
    let mut env = envelope_of(a);
    let (Some(p), Some(q)) = (env.points.get(i).copied(), env.points.get(i + 1).copied()) else {
        return a.clone();
    };
    if (q.db - p.db).abs() < 1e-3 || !p.shape.has_tension() {
        return a.clone();
    }
    let mid = ((db - p.db) / (q.db - p.db)).clamp(0.0, 1.0);
    env.points[i].tension = cantus_core::automation::edit::tension_for_mid(p.shape, mid);
    with_envelope(a, env)
}

/// The envelope as the line's 0 dB again: no points.
pub fn cleared(a: &AudioRegion) -> AudioRegion {
    AudioRegion {
        gain_env: None,
        ..a.clone()
    }
}

/// Which repetition of a looping window `d` ticks into it lies in: where that piece starts
/// (window ticks) and how long it is — the whole window when it does not loop.
fn piece(length: Ticks, period: Option<Ticks>, d: Ticks) -> (Ticks, Ticks) {
    match period.filter(|p| *p > Ticks::ZERO && *p < length) {
        Some(p) => {
            let last = (length.0 - 1).max(0) / p.0;
            let k = (d.0.max(0) / p.0).min(last);
            let at = Ticks(k * p.0);
            (at, (length - at).min(p))
        }
        None => (Ticks::ZERO, length),
    }
}

/// The content tick a clip shows at `d` ticks into its window: a looping window shows its
/// period again in each repetition (as the engine plays it — each piece from `offset`,
/// ADR 0070 review 1), each mirrored on its own when reversed.
pub fn content_at(
    clip_offset: Ticks,
    length: Ticks,
    period: Option<Ticks>,
    reverse: bool,
    d: Ticks,
) -> Ticks {
    let (at, len) = piece(length, period, d);
    let dd = d - at;
    if reverse {
        clip_offset + len - dd
    } else {
        clip_offset + dd
    }
}

/// Every place in a clip's window (ticks from its start) content tick `t` shows: once, or
/// once in each repetition of a looping window that holds it.
pub fn window_ats(
    clip_offset: Ticks,
    length: Ticks,
    period: Option<Ticks>,
    reverse: bool,
    t: Ticks,
) -> Vec<Ticks> {
    let one = |at: Ticks, len: Ticks| -> Option<Ticks> {
        let dd = if reverse {
            clip_offset + len - t
        } else {
            t - clip_offset
        };
        (dd >= Ticks::ZERO && dd <= len).then_some(at + dd)
    };
    match period.filter(|p| *p > Ticks::ZERO && *p < length) {
        Some(p) => {
            let mut out = Vec::new();
            let mut at = Ticks::ZERO;
            while at < length {
                let len = (length - at).min(p);
                out.extend(one(at, len));
                at += p;
            }
            out
        }
        None => vec![one(Ticks::ZERO, length).unwrap_or(if reverse {
            clip_offset + length - t
        } else {
            t - clip_offset
        })],
    }
}

// ---- the line's scale on screen --------------------------------------------------------

/// Where 0 dB sits on the envelope's scale, from the top (0…1): above it the headroom to
/// +12 dB, below it a square-root scale to the floor (−∞), so the dB a mix is made of —
/// 0 to −20 — have most of the height.
pub const ZERO_AT: f32 = 0.25;

/// The height fraction (0 top … 1 bottom) of `db` on the envelope's scale.
pub fn env_v(db: f32) -> f32 {
    if db >= 0.0 {
        ZERO_AT * (1.0 - (db / ENV_MAX_DB).min(1.0))
    } else {
        ZERO_AT + (1.0 - ZERO_AT) * (-db / -ENV_MIN_DB).min(1.0).sqrt()
    }
}

/// The dB at height fraction `v` (the inverse of [`env_v`]).
pub fn env_db(v: f32) -> f32 {
    let v = v.clamp(0.0, 1.0);
    if v <= ZERO_AT {
        ENV_MAX_DB * (1.0 - v / ZERO_AT)
    } else {
        let u = (v - ZERO_AT) / (1.0 - ZERO_AT);
        ENV_MIN_DB * u * u
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scale_inverts_and_puts_zero_where_it_says() {
        assert_eq!(env_v(0.0), ZERO_AT);
        assert_eq!(env_v(ENV_MAX_DB), 0.0);
        assert_eq!(env_v(ENV_MIN_DB), 1.0);
        for db in [-60.0, -24.0, -6.0, -0.5, 0.0, 3.0, 12.0] {
            assert!((env_db(env_v(db)) - db).abs() < 1e-3, "{db}");
        }
    }

    #[test]
    fn points_keep_their_order_and_their_range() {
        let a = AudioRegion::of(cantus_core::AudioSourceId(1));
        let (a, i) = add_point(&a, Ticks(100), Some(-6.0));
        assert_eq!(i, 0);
        let (a, j) = add_point(&a, Ticks(300), None);
        assert_eq!((j, envelope_of(&a).points[1].db), (1, -6.0));
        let (a, k) = add_point(&a, Ticks(200), Some(99.0));
        assert_eq!((k, envelope_of(&a).points[1].db), (1, ENV_MAX_DB));
        // Moved past a neighbour: held at it.
        let a = move_point(&a, 1, Ticks(1_000), -3.0);
        assert_eq!(envelope_of(&a).points[1].tick, Ticks(300));
        assert!(envelope_of(&a).is_valid());
        let a = remove_point(&remove_point(&remove_point(&a, 0), 0), 0);
        assert!(a.gain_env.is_none(), "no points, no envelope");
    }

    #[test]
    fn a_bend_passes_the_line_through_the_middle_asked() {
        let a = AudioRegion::of(cantus_core::AudioSourceId(1));
        let (a, _) = add_point(&a, Ticks(0), Some(-12.0));
        let (a, _) = add_point(&a, Ticks(100), Some(0.0));
        let b = bend_to(&a, 0, -9.0);
        let mid = envelope_of(&b).db_at(50.0);
        assert!((mid - -9.0).abs() < 0.2, "{mid}");
    }

    #[test]
    fn content_and_window_ticks_mirror_on_a_reversed_clip() {
        let (o, l) = (Ticks(1_000), Ticks(400));
        for rev in [false, true] {
            for d in [0, 1, 250, 400] {
                let t = content_at(o, l, None, rev, Ticks(d));
                assert_eq!(window_ats(o, l, None, rev, t), vec![Ticks(d)]);
            }
        }
    }

    /// Review 1 (ADR 0070): a looping window shows its period again in each repetition,
    /// as the engine plays it — content tick and window tick agree in every one.
    #[test]
    fn a_looping_windows_line_repeats_with_its_audio() {
        let (o, l, p) = (Ticks(500), Ticks(1_000), Some(Ticks(300)));
        for rev in [false, true] {
            for d in [0, 299, 300, 450, 899, 900, 999] {
                let t = content_at(o, l, p, rev, Ticks(d));
                assert!(t >= o && t <= o + Ticks(300), "{rev} {d}: {t:?}");
                assert!(window_ats(o, l, p, rev, t).contains(&Ticks(d)), "{rev} {d}");
            }
        }
        // Bar 3 of a 1-bar clip looped to 4 shows the bar's own content.
        assert_eq!(
            content_at(
                Ticks(0),
                Ticks(4 * 3840),
                Some(Ticks(3840)),
                false,
                Ticks(2 * 3840 + 960)
            ),
            Ticks(960)
        );
    }
}
