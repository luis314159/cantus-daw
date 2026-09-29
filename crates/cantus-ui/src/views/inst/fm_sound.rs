//! The spectrum card: the patch's spectrum — rendered here by the very DSP the engine runs,
//! on the note the engine last played (C4 when nothing has) — with each voice's Carson edge,
//! the host's Nyquist and what the alias guard decided. Nobody else shows where the
//! sidebands land (research §7 #2).

use super::parts;
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::with_alpha;
use crate::kit::text::{Role, Src};
use crate::kit::{with_brush, Brush, Style};
use crate::views::inst::write_readout;
use cantus_core::inst::fm::{GUARDS, QUALITIES};
use cantus_core::{pitch_name, NativeSpec, TrackId};
use cantus_dsp::modulus::{tel, Modulus};
use cantus_dsp::spectrum::fft;
use egui::{Align2, Pos2, Rect, Vec2};

const N: usize = 4096;
const RATE: f32 = 48_000.0;

/// The last spectrum computed, and what it was computed for.
#[derive(Debug, Default)]
pub struct Cache {
    spec: Option<NativeSpec>,
    key: u8,
    pub db: Vec<f32>,
    pub bandwidth: f32,
    pub factor: f32,
    pub tame: f32,
}

/// Render one note of the patch and measure it.
pub fn measure(spec: &NativeSpec, key: u8) -> (Vec<f32>, f32, f32, f32) {
    let patch = cantus_engine::inst::fm::patch_of(spec);
    let mut m = Modulus::new(RATE);
    m.set_patch(&patch);
    m.note_on(key, 1, 0.8);
    let total = (0.12 * RATE) as usize + N;
    let (mut l, mut r) = (vec![0f32; total], vec![0f32; total]);
    for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        m.process(a, b);
    }
    let mut telemetry = [0f32; 64];
    m.telemetry(&mut telemetry);
    let start = total - N;
    let wsum: f32 = (0..N)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / N as f32).cos())
        .sum();
    let mut re: Vec<f32> = (0..N)
        .map(|i| {
            let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / N as f32).cos();
            0.5 * (l[start + i] + r[start + i]) * w
        })
        .collect();
    let mut im = vec![0f32; N];
    fft(&mut re, &mut im);
    let db = (0..N / 2)
        .map(|k| {
            let a = 2.0 * (re[k] * re[k] + im[k] * im[k]).sqrt() / wsum;
            20.0 * a.max(1e-7).log10()
        })
        .collect();
    (
        db,
        telemetry[tel::BANDWIDTH],
        telemetry[tel::FACTOR],
        telemetry[tel::TAME],
    )
}

/// The spectrum's card: the picture bleeding to the edges, the guard's verdict at its foot.
pub(super) fn card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    s: &Style,
    rect: Rect,
) {
    let tel_ = app.inst_telemetry();
    let live = tel_[tel::KEY] > 0.0 && tel_[0] > 0.0;
    let key = if live { tel_[tel::KEY] as u8 } else { 60 };
    let cache = &mut app.inst.fm.spectrum;
    if cache.spec.as_ref() != Some(spec) || cache.key != key || cache.db.is_empty() {
        let (db, bw, factor, tame) = measure(spec, key);
        *cache = Cache {
            spec: Some(spec.clone()),
            key,
            db,
            bandwidth: bw,
            factor,
            tame,
        };
    }
    let (mut bw, mut factor, mut tame) = (cache.bandwidth, cache.factor, cache.tame);
    if live {
        bw = tel_[tel::BANDWIDTH];
        factor = tel_[tel::FACTOR];
        tame = tel_[tel::TAME];
    }
    let disp = rect.height() / s.k - 24.0 - 1.0;
    let cr = parts::card(ui, rect, s, Src::Key("fm.spectrum"), None, None, Some(disp));
    let Some(d) = cr.display else {
        return;
    };
    let folds = bw > 0.45 * RATE;
    let q = spec.get("quality").round() as usize;
    let g = spec.get("guard").round() as usize;
    let verdict = if tame < 0.999 {
        tf(
            "fm.guard.tamed",
            &[("pct", &format!("{:.0}", tame * 100.0))],
        )
    } else if folds && factor <= 1.0 {
        t("fm.guard.folds")
    } else {
        t("fm.guard.clean")
    };
    let warn = tame < 0.999 || (folds && factor <= 1.0);
    let status = format!(
        "{} \u{00d7}{factor:.0} \u{b7} {} \u{b7} {} \u{b7} {verdict}",
        t("fm.sound.rate"),
        t(QUALITIES[q.min(4)]),
        t(GUARDS[g.min(2)])
    );
    let db = &app.inst.fm.spectrum.db;
    with_brush(&ui.painter_at(d), |b| {
        paint(b, d, s, db, bw, folds, key);
        parts::chip(
            b,
            Rect::from_min_size(
                Pos2::new(d.left() + s.s(6.0), d.bottom() - s.s(22.0)),
                Vec2::new(d.width() - s.s(12.0), s.s(16.0)),
            ),
            s,
            &status,
            if warn { s.t.highlight } else { s.t.text_dim },
            Align2::LEFT_CENTER,
        );
    });
    let _ = track;
}

/// The picture: grid, the loudest bin per pixel column filled, the axes' numbers, the
/// Carson edge (the "now" colour when it folds past Nyquist), Nyquist, the note.
fn paint(b: &mut Brush, d: Rect, s: &Style, db: &[f32], bw: f32, folds: bool, key: u8) {
    let plot = Rect::from_min_max(
        d.min + Vec2::new(s.s(8.0), s.s(20.0)),
        d.max - Vec2::new(s.s(8.0), s.s(40.0)),
    );
    let (f_lo, f_hi) = (30.0f32, RATE / 2.0);
    let x = |f: f32| plot.left() + (f.max(f_lo) / f_lo).ln() / (f_hi / f_lo).ln() * plot.width();
    let y = |v: f32| plot.top() + (-v.clamp(-96.0, 0.0)) / 96.0 * plot.height();
    for f in [100.0, 1_000.0, 10_000.0] {
        b.vline(
            b.snap_mid(x(f)),
            plot.top(),
            plot.bottom(),
            b.px(),
            s.t.grid,
        );
    }
    for v in [-24.0, -48.0, -72.0] {
        b.hline(
            plot.left(),
            plot.right(),
            b.snap_mid(y(v)),
            b.px(),
            s.t.grid,
        );
    }
    if !db.is_empty() {
        let bin_hz = RATE / N as f32;
        let cols = (plot.width() / s.s(1.5)).max(1.0) as usize;
        let mut top: Vec<Pos2> = Vec::with_capacity(cols);
        for c in 0..cols {
            let f0 = f_lo * (f_hi / f_lo).powf(c as f32 / cols as f32);
            let f1 = f_lo * (f_hi / f_lo).powf((c + 1) as f32 / cols as f32);
            let (b0, b1) = (
                (f0 / bin_hz) as usize,
                ((f1 / bin_hz) as usize).max((f0 / bin_hz) as usize + 1),
            );
            let v = db[b0.min(db.len() - 1)..b1.min(db.len())]
                .iter()
                .fold(-120f32, |m, x| m.max(*x));
            top.push(Pos2::new(
                plot.left() + plot.width() * c as f32 / cols as f32,
                y(v),
            ));
        }
        b.area(
            &top,
            plot.bottom(),
            with_alpha(s.hue, if s.t.dark { 0.30 } else { 0.22 }),
        );
        b.polyline(&top, s.s(1.5), s.hue);
    }
    for (f, l) in [(100.0, "100"), (1_000.0, "1k"), (10_000.0, "10k")] {
        parts::say(
            b,
            Rect::from_min_size(
                Pos2::new(x(f) + s.s(2.0), plot.bottom() + s.s(1.0)),
                Vec2::new(s.s(30.0), s.s(12.0)),
            ),
            s,
            l,
            Role::Micro,
            s.t.text_dim,
            Align2::LEFT_CENTER,
        );
    }
    for v in [-24.0, -48.0, -72.0] {
        parts::say(
            b,
            Rect::from_min_size(
                Pos2::new(plot.left() + s.s(2.0), y(v) - s.s(13.0)),
                Vec2::new(s.s(48.0), s.s(12.0)),
            ),
            s,
            &format!("{v:.0} dB"),
            Role::Micro,
            s.t.text_dim,
            Align2::LEFT_CENTER,
        );
    }
    if bw > 0.0 {
        let ex = x(bw.min(f_hi));
        let c = if folds { s.t.highlight } else { s.t.text };
        let mut y0 = plot.top();
        while y0 < plot.bottom() {
            let y1 = (y0 + s.s(4.0)).min(plot.bottom());
            b.segment(Pos2::new(ex, y0), Pos2::new(ex, y1), s.s(1.5), c, false);
            y0 += s.s(7.0);
        }
        let mut hz = String::new();
        write_readout(bw, "Hz", &mut hz);
        let label = tf("fm.carson", &[("hz", &hz)]);
        let w = s.s(110.0);
        let (lx, align) = if ex > plot.center().x {
            (ex - w - s.s(3.0), Align2::RIGHT_CENTER)
        } else {
            (ex + s.s(3.0), Align2::LEFT_CENTER)
        };
        parts::say(
            b,
            Rect::from_min_size(Pos2::new(lx, d.top() + s.s(3.0)), Vec2::new(w, s.s(14.0))),
            s,
            &label,
            Role::Micro,
            c,
            align,
        );
    }
    b.vline(
        b.snap_mid(plot.right()),
        plot.top(),
        plot.bottom(),
        b.px(),
        s.t.text_dim,
    );
    parts::say(
        b,
        Rect::from_min_size(
            Pos2::new(plot.right() - s.s(62.0), plot.top() + s.s(2.0)),
            Vec2::new(s.s(58.0), s.s(14.0)),
        ),
        s,
        &t("fm.nyquist"),
        Role::Micro,
        s.t.text_dim,
        Align2::RIGHT_CENTER,
    );
    parts::say(
        b,
        Rect::from_min_size(
            d.min + Vec2::new(s.s(6.0), s.s(3.0)),
            Vec2::new(s.s(40.0), s.s(14.0)),
        ),
        s,
        &pitch_name(key),
        Role::Micro,
        s.t.text,
        Align2::LEFT_CENTER,
    );
}
