//! Culmen's picture in the inspector, and the demo the screenshot tool opens it with.

use super::bands::band_ink;
use super::{State, BAND_KEYS};
use crate::app::CantusApp;
use crate::design::space;
use crate::kit::hint::{self, Caps};
use crate::kit::text::Role;
use crate::kit::{with_brush, Fill, Style};
use crate::views::fx::canvas::{self, Ink};
use crate::views::fx::frame;
use crate::views::fx::host::{DawHost, FxHost as _};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::culmen::Culmen;
use egui::{pos2, vec2, Rect, Sense};

/// The inspector's picture: the bands as blocks (lit where they act), the full band, and
/// the drive into the ceiling in words.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let st = Style::current().function(frame::hue_of(EffectKind::Maximizer));
    let ink = Ink::of(&st);
    let ms = cantus_engine::fx::maximizer::settings(spec);
    let h = space::GRID * 14.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let blocks = Rect::from_min_size(rect.min, vec2(h * 1.6, h)).shrink(space::S1);
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, st.hue_tok);
        let used: Vec<usize> = (0..4).filter(|x| ms.band_in_use(*x)).collect();
        let w = blocks.width() / used.len().max(1) as f32;
        for (i, band) in used.iter().enumerate() {
            let r = Rect::from_min_size(
                pos2(blocks.left() + i as f32 * w, blocks.top()),
                vec2(w - 2.0, blocks.height()),
            );
            let bs = ms.bands[*band];
            let acts = bs.on && (bs.ratio != 1.0 || bs.ratio_lo != 1.0 || bs.gain_db != 0.0);
            let color = band_ink(*band, &st);
            b.rect(
                r,
                Fill::Solid(Ink::a(color, if acts { 0.55 } else { 0.12 })),
            );
            canvas::put_key(
                b,
                r.center(),
                egui::Align2::CENTER_CENTER,
                BAND_KEYS[*band],
                Role::Micro,
                ink.text,
                st.tk(),
            );
        }
        let words = Rect::from_min_max(pos2(blocks.right() + space::S2, rect.top()), rect.max);
        canvas::line_in(
            b,
            words,
            Role::Body,
            ink.text,
            st.tk(),
            format_args!(
                "{} {:+.1} dB \u{b7} {} {:.1} dB \u{b7} {} {:.0} LUFS",
                frame::tr("fx.max.gain"),
                ms.drive_db,
                frame::tr("fx.max.ceiling"),
                ms.limiter.ceiling_db,
                frame::tr("fx.max.target"),
                spec.get("target"),
            ),
        );
        let edge = if resp.hovered() { ink.hue } else { ink.edge };
        canvas::edge_b(b, rect, &Ink { edge, ..ink });
    });
    if resp.hovered() {
        hint::set_text(ui.ctx(), frame::tr("fx.max.gain_tip"), "", Caps::NONE);
    }
    resp
}

/// The settings `--editor maximizer` and the screenshot tool open it with: the classic
/// three-band master, its full band lifting the quiet parts, driven toward −11 LUFS.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "maximizer").set_many(
        &[
            ("gain", 7.0),
            ("style", 1.0),
            ("target", -14.0),
            ("lo.thr", -22.0),
            ("lo.ratio", 2.5),
            ("mid.thr", -20.0),
            ("mid.ratio", 1.8),
            ("hi.thr", -26.0),
            ("hi.ratio", 2.0),
            ("all.thr", -14.0),
            ("all.ratio", 1.5),
            ("all.thr_lo", -36.0),
            ("all.ratio_lo", 1.6),
        ],
        "cmd.fx_preset",
    );
}

/// A mix-like signal at 120 BPM: kick, snare, hats over a bass and a pad — stereo,
/// interleaved per channel as two vectors.
pub fn demo_signal(seconds: f32, sr: f32) -> (Vec<f32>, Vec<f32>) {
    use std::f32::consts::TAU;
    let n = (seconds * sr) as usize;
    let mut seed = 5u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed as f32 / u32::MAX as f32 * 2.0 - 1.0
    };
    let beat = (sr * 0.5) as usize;
    let mut l = Vec::with_capacity(n);
    let mut r = Vec::with_capacity(n);
    let mut hat_lp = 0.0f32;
    for i in 0..n {
        let tb = (i % beat) as f32 / sr;
        let bar_beat = (i / beat) % 4;
        let t = i as f32 / sr;
        let kick = 0.7 * (-tb * 14.0).exp() * (TAU * (48.0 + 90.0 * (-tb * 40.0).exp()) * tb).sin();
        let nz = noise();
        let snare = if bar_beat % 2 == 1 {
            0.35 * (-tb * 20.0).exp() * (0.6 * nz + 0.4 * (TAU * 190.0 * tb).sin())
        } else {
            0.0
        };
        hat_lp += (nz - hat_lp) * 0.5;
        let th = (i % (beat / 2)) as f32 / sr;
        let hat = 0.08 * (nz - hat_lp) * (-th * 45.0).exp();
        let bass = 0.22 * (TAU * 55.0 * t).sin() * (0.6 + 0.4 * (TAU * 0.5 * t).sin());
        let pad = 0.06 * ((TAU * 220.0 * t).sin() + (TAU * 277.2 * t).sin());
        let swell = 0.5 + 0.5 * (TAU * t / seconds.max(1.0)).sin().abs();
        let common = kick + snare + bass;
        l.push(common + (hat * 1.2 + pad * 0.8) * swell);
        r.push(common + (hat * 0.8 + pad * 1.2) * swell);
    }
    (l, r)
}

/// What a playing engine would feed the editor, for screenshots with no engine: the demo
/// mix through the real Culmen, its telemetry ingested column by column.
pub fn demo_feed(app: &mut CantusApp) {
    let Some((strip, effect, _)) = app.fx.open else {
        return;
    };
    let Some(spec) = app
        .project()
        .strip(strip)
        .and_then(|s| s.effect(effect))
        .map(|e| e.spec.clone())
    else {
        return;
    };
    let sr = 48_000.0;
    let mut c = Culmen::new(sr);
    c.set(&cantus_engine::fx::maximizer::settings(&spec));
    c.reset();
    let (l, r) = demo_signal(9.0, sr);
    let st = &mut app.fx.maximizer;
    *st = State {
        effect: Some(effect),
        ..State::default()
    };
    let mut t = [0.0f32; 64];
    let block = 480;
    let mut pairs = Vec::with_capacity(l.len() * 2);
    for (a, b) in l.chunks(block).zip(r.chunks(block)) {
        let (mut x, mut y) = (a.to_vec(), b.to_vec());
        c.process(&mut x, &mut y);
        t = c.telemetry();
        st.ingest(&t);
        for (i, o) in a.iter().zip(&x) {
            pairs.push(*i);
            pairs.push(*o);
        }
    }
    app.fx.feed(&pairs);
    app.fx.injected = Some(t);
}
