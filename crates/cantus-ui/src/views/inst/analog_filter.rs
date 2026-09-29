//! The filter display: the model's response as the DSP computes it (`va::response`), the
//! cutoff with its note, the live cutoff of the newest voice, the self-oscillation mark,
//! the bass a four-pole loses, and the level going into the filter — drawn with the kit's
//! brush in the Tone hue, bleeding to the card's edges. The handle *is* the parameter pair:
//! drag across for the cutoff and up for the resonance (one undo step; Shift is finer);
//! click to put the cutoff there; double-click to reset both.

use super::*;
use crate::kit::color::{shade, with_alpha};
use crate::kit::meter::{self, Levels};
use crate::kit::text::{Elide, Policy, Role, Src, Text};
use crate::kit::{self, light, Brush, Fill, Style, Tok};
use cantus_dsp::va::response;
use cantus_dsp::va::{FilterSettings, Model, Revision};
use egui::{Align2, Pos2, Rect, Sense, Vec2};

const F_MIN: f32 = 20.0;
const F_MAX: f32 = 20_000.0;
const DB_TOP: f32 = 30.0;
const DB_BOT: f32 = -48.0;
/// The response is drawn at Good's rate.
const FS: f32 = 96_000.0;
/// The tokens the display paints with, beside the Tone hue (for inspect mode).
const TOKENS: &[Tok] = &[
    Tok::Ink,
    Tok::Grid,
    Tok::GridStrong,
    Tok::Text,
    Tok::TextDim,
    Tok::ValuePill,
    Tok::ValuePillText,
];

/// The filter a spec describes, for drawing.
pub fn settings(spec: &NativeSpec) -> FilterSettings {
    FilterSettings {
        model: Model::from_index(spec.get("vcf.model")),
        reso: spec.get("vcf.reso"),
        poles: spec.get("vcf.slope").round() as u8,
        morph: spec.get("vcf.morph"),
        band: spec.get("vcf.mid") >= 0.5,
        bass_comp: spec.get("vcf.bass_comp"),
        drive: 1.0,
        rev: Revision::from_index(spec.get("vcf.rev")),
    }
}

#[derive(Copy, Clone)]
struct Geo {
    r: Rect,
}

impl Geo {
    fn x(&self, f: f32) -> f32 {
        self.r.left() + self.r.width() * (f.max(F_MIN) / F_MIN).ln() / (F_MAX / F_MIN).ln()
    }
    fn hz(&self, x: f32) -> f32 {
        F_MIN * (F_MAX / F_MIN).powf(((x - self.r.left()) / self.r.width()).clamp(0.0, 1.0))
    }
    fn y(&self, db: f32) -> f32 {
        self.r.top() + self.r.height() * (DB_TOP - db.clamp(DB_BOT, DB_TOP)) / (DB_TOP - DB_BOT)
    }
}

/// Where the plot sits in a display: right of the input meter, above the frequencies.
fn plot_of(d: Rect, s: &Style) -> Rect {
    Rect::from_min_max(
        Pos2::new(d.left() + s.s(24.0), d.top() + s.s(8.0)),
        Pos2::new(d.right() - s.s(10.0), d.bottom() - s.s(15.0)),
    )
}

/// A frequency as a note: `A4 +12 ct`.
pub fn note_of(hz: f32) -> String {
    let n = 69.0 + 12.0 * (hz / 440.0).log2();
    let k = n.round().clamp(0.0, 127.0);
    let cents = ((n - k) * 100.0).round();
    let name = cantus_core::pitch_name(k as u8);
    if cents.abs() < 1.0 {
        name
    } else {
        format!("{name} {cents:+.0} ct")
    }
}

/// Hz as a musician reads it.
pub fn hz_text(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.2} kHz", hz / 1000.0)
    } else {
        format!("{hz:.0} Hz")
    }
}

/// The response under `geo`, one point per three pixels.
fn curve(geo: &Geo, fs: &FilterSettings, fc: f32, out: &mut [Pos2; 400]) -> usize {
    let n = ((geo.r.width() / 3.0) as usize).clamp(16, out.len());
    for (i, p) in out.iter_mut().take(n).enumerate() {
        let x = geo.r.left() + geo.r.width() * i as f32 / (n - 1) as f32;
        let db = response::magnitude_db(fs, fc, geo.hz(x), FS);
        *p = Pos2::new(x, geo.y(db));
    }
    n
}

/// A micro text in a box.
fn micro(b: &mut Brush, r: Rect, text: &str, color: egui::Color32, align: Align2, s: &Style) {
    let t = Text {
        src: Src::Str(text),
        role: Role::Micro,
        caps: false,
        align,
        policy: Policy::Elide(Elide::End),
        color,
    };
    let _ = kit::text::paint(b, r, &t, s.tk());
}

/// The grid: octaves faint, decades strong, every 12 dB, 0 dB strongest; labelled.
fn grid(b: &mut Brush, d: Rect, geo: &Geo, s: &Style) {
    let t = &s.t;
    let plot = geo.r;
    let minor = with_alpha(t.grid, if t.dark { 0.8 } else { 1.0 });
    let major = if t.dark {
        shade(t.grid, 0.07)
    } else {
        t.grid_strong
    };
    for f in [
        30.0f32, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 200.0, 300.0, 400.0, 500.0, 600.0, 700.0,
        800.0, 900.0, 2000.0, 3000.0, 4000.0, 5000.0, 6000.0, 7000.0, 8000.0, 9000.0,
    ] {
        let x = b.snap_mid(geo.x(f));
        b.vline(
            x,
            plot.top(),
            plot.bottom(),
            b.px(),
            with_alpha(minor, 0.55),
        );
    }
    for f in [100.0f32, 1000.0, 10_000.0] {
        b.vline(b.snap_mid(geo.x(f)), d.top(), plot.bottom(), b.px(), major);
    }
    let mut db = 24.0f32;
    while db > DB_BOT {
        let c = if db.abs() < 0.1 {
            shade(major, 0.05)
        } else {
            minor
        };
        b.hline(plot.left(), plot.right(), b.snap_mid(geo.y(db)), b.px(), c);
        db -= 12.0;
    }
    for (f, l) in [
        (50.0f32, "50"),
        (100.0, "100"),
        (200.0, "200"),
        (500.0, "500"),
        (1000.0, "1k"),
        (2000.0, "2k"),
        (5000.0, "5k"),
        (10_000.0, "10k"),
    ] {
        let r = Rect::from_center_size(
            Pos2::new(geo.x(f), plot.bottom() + s.s(8.0)),
            Vec2::new(s.s(30.0), s.s(12.0)),
        );
        micro(b, r, l, t.text_dim, Align2::CENTER_CENTER, s);
    }
    for (db, l) in [
        (24.0f32, "+24"),
        (12.0, "+12"),
        (0.0, "0"),
        (-12.0, "\u{2212}12"),
        (-24.0, "\u{2212}24"),
        (-36.0, "\u{2212}36"),
    ] {
        let r = Rect::from_min_size(
            Pos2::new(plot.left() + s.s(3.0), geo.y(db) - s.s(12.0)),
            Vec2::new(s.s(28.0), s.s(11.0)),
        );
        micro(b, r, l, t.text_dim, Align2::LEFT_CENTER, s);
    }
}

/// The display in `d` (a card's display rect): painted, and the drag that is the cutoff
/// and the resonance. `s` is in the Tone hue.
#[allow(clippy::too_many_arguments)]
pub fn display(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    tel: &cantus_engine::Telemetry,
    d: Rect,
    s: &Style,
) {
    let resp = ui.interact(
        d,
        ui.id().with(("calor-filter", track.0)),
        Sense::click_and_drag(),
    );
    let geo = Geo { r: plot_of(d, s) };
    let fs = settings(spec);
    let fc = spec.get("vcf.cutoff");
    let reso = spec.get("vcf.reso");
    let hot = resp.hovered() || resp.dragged();
    kit::with_brush(&ui.painter_at(d), |b| {
        b.register(d, "calor.filter", TOKENS, &[s.hue_tok]);
        grid(b, d, &geo, s);
        let plot = geo.r;
        let mut pts = [Pos2::ZERO; 400];
        let n = curve(&geo, &fs, fc, &mut pts);
        b.area(&pts[..n], plot.bottom(), super::draw::fill(s));
        b.polyline(&pts[..n], s.s(2.0), s.hue);
        // The cutoff: a line through the plot.
        let x = geo.x(fc);
        b.vline(
            b.snap_mid(x),
            plot.top(),
            plot.bottom(),
            b.px(),
            with_alpha(s.hue, 0.6),
        );
        // The newest voice's cutoff, as it moves: a ring on the curve.
        let live = tel[tel::CUTOFF];
        if live > 0.0 {
            let lx = geo.x(live.clamp(F_MIN, F_MAX));
            let ly = geo.y(response::magnitude_db(&fs, fc, live, FS).min(DB_TOP - 4.0));
            b.circle_stroke(Pos2::new(lx, ly), s.s(6.0), s.s(1.5), s.hue);
            b.dot(Pos2::new(lx, ly), s.s(2.5), s.hue);
        }
        // The handle, on the curve at the cutoff.
        let hy = geo.y(response::magnitude_db(&fs, fc, fc, FS).min(DB_TOP - 2.0));
        let c = Pos2::new(x, hy.max(plot.top() + s.s(8.0)));
        light::glow(b, c, s.s(if hot { 18.0 } else { 14.0 }), s.hue, 0.8);
        b.circle(c, s.s(7.0), Fill::Solid(s.hue));
        b.circle_stroke(c, s.s(7.0), s.s(1.0), shade(s.hue, -0.25));
        // What the handle holds, in a pill beside it.
        let right = x > plot.center().x;
        let text = format!("{} \u{b7} {:.0} %", hz_text(fc), reso * 100.0);
        let note = note_of(fc);
        let w = s.s(118.0);
        let pill = Rect::from_min_size(
            Pos2::new(
                if right {
                    x - s.s(14.0) - w
                } else {
                    x + s.s(14.0)
                },
                plot.top() + s.s(4.0),
            ),
            Vec2::new(w, s.s(30.0)),
        );
        b.rrect(pill, s.r(3.0), Fill::Solid(with_alpha(s.t.ink, 0.85)));
        b.rrect_stroke(pill, s.r(3.0), b.px(), with_alpha(s.hue, 0.7));
        let top = Rect::from_min_size(pill.min, Vec2::new(w, s.s(17.0)));
        let t = Text::new(&text, Role::Value, s.t.text).policy(Policy::Elide(Elide::End));
        let _ = kit::text::paint(b, top.shrink2(Vec2::new(s.s(5.0), 0.0)), &t, s.tk());
        let bottom = Rect::from_min_max(Pos2::new(pill.left(), top.bottom() - s.s(2.0)), pill.max);
        let osc = reso >= 1.0;
        let under = if osc {
            format!("{note} \u{b7} {}", t_of("calor.self_osc"))
        } else {
            note
        };
        micro(
            b,
            bottom.shrink2(Vec2::new(s.s(5.0), 0.0)),
            &under,
            if osc { s.hue } else { s.t.text_dim },
            Align2::LEFT_CENTER,
            s,
        );
        if osc {
            b.dot(Pos2::new(x, plot.top() + s.s(3.0)), s.s(3.5), s.hue);
        }
        // The bass a four-pole loses at this resonance.
        let loss = response::bass_loss_db(&fs);
        if fs.model.four_pole() && loss < -0.5 {
            let r = Rect::from_min_size(
                Pos2::new(plot.left() + s.s(34.0), plot.bottom() - s.s(14.0)),
                Vec2::new(s.s(120.0), s.s(12.0)),
            );
            let text = format!("{} {loss:.1} dB", t_of("calor.bass_loss"));
            micro(b, r, &text, s.t.text_dim, Align2::LEFT_CENTER, s);
        }
        // The level into the filter (what the mixer's levels and drive send it), dBFS.
        let m = Rect::from_min_max(
            Pos2::new(d.left() + s.s(8.0), plot.top()),
            Pos2::new(d.left() + s.s(16.0), plot.bottom()),
        );
        let db = 20.0 * tel[tel::IN_PEAK].max(1e-6).log10();
        meter::paint(
            b,
            m,
            s,
            &Levels {
                level: [db, db],
                hold: [db, db],
            },
        );
        let r = Rect::from_min_size(
            Pos2::new(d.left() + s.s(4.0), plot.bottom() + s.s(2.0)),
            Vec2::new(s.s(34.0), s.s(12.0)),
        );
        micro(
            b,
            r,
            &t_of("calor.in"),
            s.t.text_dim,
            Align2::LEFT_CENTER,
            s,
        );
    });
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t_of("calor.vcf.cutoff"),
            &t_of("calor.filter_tip"),
            crate::kit::hint::Caps::PARAM,
        );
    }
    interact(app, ui, track, spec, &geo, &resp);
}

fn t_of(key: &str) -> String {
    crate::i18n::t(key)
}

fn interact(
    app: &mut CantusApp,
    ui: &egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    geo: &Geo,
    resp: &egui::Response,
) {
    let fine = if ui.input(|i| i.modifiers.shift) {
        0.2
    } else {
        1.0
    };
    if resp.drag_started() {
        app.inst.analog.drag = Some((spec.get("vcf.cutoff"), spec.get("vcf.reso")));
    }
    if resp.dragged() {
        let d = resp.drag_delta() * fine;
        let cut = spec.get("vcf.cutoff") * (F_MAX / F_MIN).powf(d.x / geo.r.width());
        let reso = spec.get("vcf.reso") - 1.2 * d.y / geo.r.height();
        set_filter(app, track, cut.clamp(F_MIN, F_MAX), reso.clamp(0.0, 1.2));
    }
    if resp.drag_stopped() {
        app.inst.analog.drag = None;
        app.end_gesture();
    }
    if resp.double_clicked() {
        app.end_gesture();
        let defaults: Vec<(&str, f32)> = ["vcf.cutoff", "vcf.reso"]
            .iter()
            .filter_map(|k| spec.kind.param(k).map(|i| (*k, i.default)))
            .collect();
        set_values(app, track, "filter-reset", &defaults);
        app.end_gesture();
    } else if resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            app.set_inst_value(track, "vcf.cutoff", geo.hz(p.x), false);
        }
    }
}

/// The inspector's picture: the curve, small, with the model's name.
pub fn preview(ui: &mut egui::Ui, spec: &NativeSpec) -> egui::Response {
    let s = Style::current().function(crate::kit::Function::Tone);
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(192.0, 64.0), Sense::hover());
    kit::with_brush(&ui.painter_at(rect), |b| {
        b.register(rect, "calor.preview", TOKENS, &[s.hue_tok]);
        b.rrect(rect, s.r(4.0), Fill::Solid(s.t.ink));
        let geo = Geo {
            r: rect.shrink2(Vec2::new(4.0, 6.0)),
        };
        let mut pts = [Pos2::ZERO; 400];
        let n = curve(&geo, &settings(spec), spec.get("vcf.cutoff"), &mut pts);
        b.area(&pts[..n], geo.r.bottom(), super::draw::fill(&s));
        b.polyline(&pts[..n], 1.5, s.hue);
        let model = edit::MODELS[spec.get("vcf.model").round() as usize % 5];
        let t = Text::key(model, Role::Micro, s.t.text)
            .caps(true)
            .align(Align2::LEFT_TOP)
            .policy(Policy::Elide(Elide::End));
        let _ = kit::text::paint(b, rect.shrink(4.0), &t, 1.0);
    });
    resp
}
