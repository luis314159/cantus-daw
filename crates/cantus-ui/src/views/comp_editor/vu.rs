//! Comprimo's needle (round 32, ADR 0074): the gain reduction on a backlit VU face, as a
//! FET rack compressor reads it — the needle rests at 0 and falls left by the decibels
//! taken away.

use super::*;
use crate::views::fx::canvas::{self, Ink};

/// The card's width at 100 %.
pub(super) const WIDTH: f32 = 136.0;

/// The face's height over its width (the needle's arc needs about 1.5 : 1).
const ASPECT: f32 = 0.62;

/// Whether the needle card shows: in Comprimo's face; never in the kit's plain look (the
/// user's switch off), which keeps the editor exactly as it was.
pub(super) fn shown(s: &Style) -> bool {
    !s.skin.is_plain()
}

/// The editor's designed width: 1120 in the plain look, 1260 with the needle card.
pub fn designed_width() -> f32 {
    if crate::kit::skin::of_effect(cantus_core::EffectKind::Compressor).is_plain() {
        1120.0
    } else {
        1260.0
    }
}

/// Where the needle points for `gr` dB of reduction (positive): 0 VU at none, −20 at 20 dB
/// or more.
pub(super) fn needle_vu(gr: f32) -> f32 {
    (-gr.max(0.0)).clamp(-20.0, 3.0)
}

/// The needle's time constant: a one-pole toward the reduction with τ = 65 ms reaches 99 %
/// of a step in 300 ms (4.6 τ), a VU meter's rise time (IEC 60268-17) — slow enough to
/// read as a needle, quick enough to follow a compressor's release.
pub(super) const TAU_S: f32 = 0.065;

/// How long the peak lamp stays lit after an over, whatever the frame rate.
pub(super) const LAMP_S: f64 = 0.4;

/// Where the needle is and until when its lamp is lit.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) struct Needle {
    /// The reduction it reads, dB (positive).
    pub gr: f32,
    /// The time (s, the UI clock) its lamp goes out.
    pub over_until: f64,
}

impl Needle {
    pub const REST: Needle = Needle {
        gr: 0.0,
        over_until: f64::NEG_INFINITY,
    };

    /// One frame of `dt` seconds at time `now`: the needle moves toward `target` (dB of
    /// reduction) as a one-pole with [`TAU_S`] (`a = 1 − e^(−dt/τ)`, so the frame rate
    /// does not change its motion), the lamp lit for [`LAMP_S`] after any over. Returns
    /// whether the lamp is lit now.
    pub fn step(&mut self, target: f32, over: bool, dt: f32, now: f64) -> bool {
        let a = 1.0 - cantus_dsp::math::exp(-dt.max(0.0) / TAU_S);
        self.gr += (target.max(0.0) - self.gr) * a;
        if over {
            self.over_until = now + LAMP_S;
        }
        now < self.over_until
    }

    /// Whether it still moves or glows (the window repaints while it does).
    pub fn busy(&self, target: f32, now: f64) -> bool {
        (self.gr - target.max(0.0)).abs() > 0.02 || now < self.over_until
    }
}

/// Paint the needle in the card's display `rect`, centred. Its numbers are the meters
/// card's (the reduction under its bar, its last ten seconds): the needle is the reading at
/// a glance, as on the hardware, and costs no text of its own each frame.
pub(super) fn show(st: &mut State, ui: &mut egui::Ui, s: &Style, rect: Rect) {
    let k = s.k;
    let ink = Ink::of(s);
    let target = st.live.gr.max(0.0);
    let (dt, now) = ui.input(|i| (i.stable_dt, i.time));
    // An over: the output's peak this frame at or above full scale.
    let lit = st.needle.step(target, st.live.out >= 0.0, dt, now);
    if st.needle.busy(target, now) {
        ui.ctx().request_repaint();
    }
    let gr = st.needle.gr;
    let pad = 6.0 * k;
    let face_w = rect.width() - 2.0 * pad;
    let face_h = (face_w * ASPECT)
        .min(rect.height() - 2.0 * pad)
        .max(24.0 * k);
    let face_w = face_w.min(face_h / ASPECT);
    let face = Rect::from_center_size(rect.center(), vec2(face_w, face_h));
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        crate::kit::skin::displays::needle_meter(b, face, s, needle_vu(gr), lit);
        canvas::edge_b(b, rect, &ink);
    });
}

#[cfg(test)]
mod tests {
    use super::{designed_width, needle_vu, shown, Needle};

    /// The needle's ballistics do not depend on the frame rate: a step to 10 dB, read
    /// over 300 ms at 60 and at 144 frames a second, lands within 0.1 dB at both, and the
    /// first frame is not there yet (a needle, not a jump).
    #[test]
    fn the_needle_moves_alike_at_any_frame_rate_and_never_jumps() {
        let run = |fps: f32| {
            let dt = 1.0 / fps;
            let mut n = Needle::REST;
            let mut first = None;
            let frames = (0.3 * fps).round() as usize;
            for i in 0..frames {
                n.step(10.0, false, dt, f64::from(i as f32 * dt));
                first.get_or_insert(n.gr);
            }
            (first.unwrap(), n.gr)
        };
        let (f60, at60) = run(60.0);
        let (f144, at144) = run(144.0);
        assert!((at60 - at144).abs() < 0.1, "{at60} vs {at144}");
        assert!(at60 > 9.5, "{at60}");
        assert!(f60 < 5.0 && f144 < 5.0, "{f60} {f144}");
    }

    /// An over of one frame lights the lamp for 0.4 s, at any frame rate.
    #[test]
    fn a_one_frame_over_lights_the_lamp_long_enough_to_see() {
        for fps in [60.0f32, 144.0] {
            let dt = 1.0 / fps;
            let mut n = Needle::REST;
            let mut lit_for = 0.0f64;
            for i in 0..(fps as usize) {
                let now = f64::from(i as f32 * dt);
                if n.step(0.0, i == 0, dt, now) {
                    lit_for = now;
                }
            }
            assert!(lit_for >= 0.3, "{fps} fps: lit for {lit_for} s");
            assert!(lit_for < 0.45, "{fps} fps: lit for {lit_for} s");
        }
    }

    /// The needle is Comprimo's face: with faces off (the plain look) the editor is the
    /// one it was — 1120 wide, no needle card.
    #[test]
    fn faces_off_keep_comprimo_as_it_was() {
        use crate::kit::skin;
        let base = crate::kit::Tokens::for_theme(&crate::theme::slate());
        let worn = crate::kit::Style::new(base)
            .skinned(skin::of_effect(cantus_core::EffectKind::Compressor));
        assert!(shown(&worn));
        assert_eq!(designed_width(), 1260.0);
        skin::set_enabled(false);
        let plain = crate::kit::Style::new(base)
            .skinned(skin::of_effect(cantus_core::EffectKind::Compressor));
        let (w, needle) = (designed_width(), shown(&plain));
        skin::set_enabled(true);
        assert_eq!(w, 1120.0);
        assert!(!needle);
    }

    #[test]
    fn the_needle_rests_at_zero_and_falls_left_with_the_reduction() {
        assert_eq!(needle_vu(0.0), 0.0);
        assert_eq!(needle_vu(-3.0), 0.0);
        assert_eq!(needle_vu(6.0), -6.0);
        assert_eq!(needle_vu(40.0), -20.0);
    }
}
