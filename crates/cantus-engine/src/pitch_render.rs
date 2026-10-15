//! The render of a clip's pitch edits: the base's offline shifter (ADR 0053 §2) following
//! the curves the edits make (`cantus_core::pitch::curves`), then each note's gain.
//! Deterministic; the source bit for bit wherever nothing is asked.

use super::TakeAnalysis;
use cantus_core::pitch::curves::{time_points, Curves};
use cantus_core::pitch::PitchEdit;
use cantus_dsp::math;
use cantus_dsp::psola::{render, Plan, TimeMap};

/// Render `edit` over a take's channels (at the rate it was analysed at). Each channel of
/// the result is as long as the take.
pub fn render_edit(channels: &[&[f32]], take: &TakeAnalysis, edit: &PitchEdit) -> Vec<Vec<f32>> {
    let len = channels.iter().map(|c| c.len()).min().unwrap_or(0);
    if !edit.edited() || len == 0 {
        return channels.iter().map(|c| c[..len].to_vec()).collect();
    }
    let curves = Curves::new(edit, &take.frames_view());
    let time = TimeMap::new(time_points(edit)).unwrap_or_default();
    let ratio = |t: f64| {
        let s = curves.shift_at(t);
        if s == 0.0 {
            1.0
        } else {
            math::exp2(s / 12.0)
        }
    };
    let formant = |t: f64| curves.formant_at(t);
    let cut: Vec<&[f32]> = channels.iter().map(|c| &c[..len]).collect();
    let mut out = render(
        &cut,
        &take.analysis,
        &Plan {
            ratio: &ratio,
            formant: &formant,
            time,
            len,
        },
    );
    if curves.has_gain() {
        for ch in &mut out {
            for (t, v) in ch.iter_mut().enumerate() {
                let g = curves.gain_db_at(t as f64);
                if g != 0.0 {
                    *v *= math::db_to_gain(g);
                }
            }
        }
    }
    out
}

fn fnv(h: &mut u64, bytes: &[u8]) {
    for b in bytes {
        *h ^= *b as u64;
        *h = h.wrapping_mul(0x0100_0000_01b3);
    }
}

/// The render's key: the file (as resolved), the edit, and the rate it is played at — so
/// two clips with the same edits share a render, and any change makes a new one.
pub fn key(path: &std::path::Path, edit: &PitchEdit, rate: u32) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    fnv(&mut h, path.to_string_lossy().as_bytes());
    fnv(&mut h, &rate.to_le_bytes());
    fnv(&mut h, &edit.rate.to_le_bytes());
    fnv(&mut h, &edit.frames.to_le_bytes());
    fnv(&mut h, &[edit.key.tonic, edit.key.scale, edit.bypass as u8]);
    for n in &edit.notes {
        for v in [n.src_start, n.src_end, n.start, n.end] {
            fnv(&mut h, &v.to_le_bytes());
        }
        for v in [
            n.sung, n.pitch, n.centre, n.drift, n.vibrato, n.formant, n.gain_db,
        ] {
            fnv(&mut h, &v.to_bits().to_le_bytes());
        }
    }
    h
}
