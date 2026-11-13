//! Fragor's display: what the chain does to a note at the level playing — one cycle of it
//! going in and coming out (the clipping's shape, the tone stack's tilt, the fuzz's
//! asymmetry, as a scope shows them) and its harmonics, odd and even, with the total
//! distortion. Computed by the DSP itself (a copy of the board on this thread, the same
//! circuits), cached until a setting or the level moves. Also the inspector's strip and
//! the demo set-up the screenshot tool uses.

use super::super::canvas::{self, Ink};
use super::super::frame;
use super::super::host::{DawHost, FxHost};
use super::{State, NAMES};
use crate::app::CantusApp;
use crate::kit::color::with_alpha;
use crate::kit::skin::pedals;
use crate::kit::text::{Role, Text};
use crate::kit::{with_brush, Fill, Style};
use cantus_core::{EffectId, EffectKind, EffectSpec, StripId};
use cantus_dsp::fragor::{telemetry as t, BoardSettings, Fragor};
use cantus_engine::fx::fragor as unit;
use egui::{pos2, vec2, Align2, Pos2, Rect, Sense};

/// The note the display plays through the chain: 200 Hz, 240 samples a cycle at 48 kHz.
const SR: f32 = 48_000.0;
const PERIOD: usize = 240;
/// Cycles played before the ones read (the circuits' coupling settles), and read.
const SETTLE: usize = 20;
const READ: usize = 4;
/// Harmonics shown.
pub const HARMONICS: usize = 12;
/// The level shown at rest: a guitar's note, −12 dBFS peak.
pub const REST_DB: f32 = -12.0;

/// The chain's sound at one level.
#[derive(Clone, Debug, Default)]
pub struct Curves {
    /// One cycle: input (aligned for the latency) and output, volts.
    pub cycle: Vec<(f32, f32)>,
    /// Each harmonic's level against the fundamental, dB (index 0 is the fundamental).
    pub harm: [f32; HARMONICS],
    /// The total harmonic distortion, %.
    pub thd: f32,
    /// The output's peak over the cycle.
    pub peak: f32,
}

/// The display's cache: the board copy and the last curves with what they were for.
#[derive(Default)]
pub struct Cache {
    dsp: Option<Box<Fragor>>,
    key: Option<(BoardSettings, i32)>,
    curves: Curves,
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache").field("key", &self.key).finish()
    }
}

/// The chain of `s` played a 200 Hz sine of peak `level` (linear), mono, dry/wet and gate
/// aside, 2× (the display is a picture of the circuits, not of the anti-aliasing).
pub fn compute(dsp: &mut Fragor, s: &BoardSettings, level: f32) -> Curves {
    let mut b = s.clone();
    b.gate_db = cantus_dsp::fragor::gate::OFF_DB;
    b.mix = 1.0;
    b.out_db = 0.0;
    b.mono = true;
    b.oversampling = 2;
    dsp.set(&b);
    dsp.reset();
    let n = PERIOD * (SETTLE + READ);
    let w = std::f32::consts::TAU / PERIOD as f32;
    let x: Vec<f32> = (0..n)
        .map(|i| level * cantus_dsp::math::sin(w * i as f32))
        .collect();
    let (mut l, mut r) = (x.clone(), x.clone());
    for (a, c) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        dsp.process(a, c);
    }
    let lat = dsp.latency();
    let from = n - PERIOD * READ;
    let mut out = Curves::default();
    for i in n - PERIOD..n {
        out.cycle.push((x[i - lat], l[i]));
        out.peak = out.peak.max(l[i].abs());
    }
    let mut mags = [0.0f64; HARMONICS];
    for (h, m) in mags.iter_mut().enumerate() {
        let wh = w as f64 * (h + 1) as f64;
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, y) in l.iter().enumerate().skip(from) {
            let (sn, cs) = cantus_dsp::math::sin_cos(wh * i as f64);
            re += *y as f64 * cs;
            im += *y as f64 * sn;
        }
        *m = (re * re + im * im).sqrt();
    }
    let f = mags[0].max(1e-12);
    let mut sum = 0.0;
    for (h, m) in mags.iter().enumerate() {
        out.harm[h] = (20.0 * cantus_dsp::math::log10((m / f).max(1e-9))) as f32;
        if h > 0 {
            sum += (m / f) * (m / f);
        }
    }
    out.thd = (sum.sqrt() * 100.0) as f32;
    out
}

/// The level the display shows: the input's peak while playing (in 2 dB steps, so the
/// picture does not chase every block), else a guitar's note.
fn level_db(tel: &[f32; 64]) -> i32 {
    let p = tel[t::IN_PEAK];
    let db = if p > -60.0 { p } else { REST_DB };
    ((db / 2.0).round() * 2.0) as i32
}

fn curves<'a>(c: &'a mut Cache, s: &BoardSettings, db: i32) -> &'a Curves {
    let key = (s.clone(), db);
    if c.key.as_ref() != Some(&key) {
        let dsp = c.dsp.get_or_insert_with(|| Box::new(Fragor::new(SR)));
        c.curves = compute(dsp, s, cantus_dsp::math::db_to_gain(db as f32));
        c.key = Some(key);
    }
    &c.curves
}

pub fn display(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    d: Rect,
    tel: &[f32; 64],
) {
    let spec = host.spec().clone();
    let settings = unit::settings(&spec);
    let db = level_db(tel);
    let playing = tel[t::IN_PEAK] > -60.0;
    let cv = curves(&mut st.curves, &settings, db).clone();
    let k = s.k;
    let ink = Ink::of(s);
    let odd = ink.voice(s.hue, false);
    let even = ink.voice(s.hue, true);
    let empty = settings.chain.iter().all(Option::is_none);
    // The transfer on the left (square-ish), the harmonics on the right.
    let tw = (d.height() * 2.0).min(d.width() * 0.46);
    let tr = Rect::from_min_size(d.min, vec2(tw, d.height()));
    let hr = Rect::from_min_max(pos2(tr.right(), d.top()), d.max);
    with_brush(&ui.painter_at(d), |b| {
        canvas::ground_b(b, d, &ink, s.hue_tok);
        b.vline(
            b.snap_mid(tr.right()),
            d.top() + 6.0 * k,
            d.bottom() - 6.0 * k,
            b.px(),
            ink.grid,
        );
        // ---- one cycle: the note going in (faint) and coming out ----
        let plot = Rect::from_min_max(
            tr.min + vec2(14.0 * k, 24.0 * k),
            tr.max - vec2(14.0 * k, 22.0 * k),
        );
        let lvl = cantus_dsp::math::db_to_gain(db as f32).max(1e-6);
        let ymax = cv.peak.max(1e-6) * 1.1;
        let n = cv.cycle.len().max(2);
        let px = |i: usize| plot.left() + plot.width() * i as f32 / (n - 1) as f32;
        let py = |y: f32, m: f32| plot.center().y - plot.height() * 0.5 * (y / m).clamp(-1.1, 1.1);
        for f in [0.25f32, 0.5, 0.75] {
            let x = plot.left() + plot.width() * f;
            b.vline(b.snap_mid(x), plot.top(), plot.bottom(), b.px(), ink.grid);
        }
        for f in [-1.0f32, 1.0] {
            let y = plot.center().y - plot.height() * 0.5 * f / 1.1;
            b.hline(plot.left(), plot.right(), b.snap_mid(y), b.px(), ink.grid);
        }
        b.hline(
            plot.left(),
            plot.right(),
            b.snap_mid(plot.center().y),
            b.px(),
            ink.grid_strong,
        );
        let input: Vec<Pos2> = cv
            .cycle
            .iter()
            .enumerate()
            .map(|(i, (x, _))| pos2(px(i), py(*x, lvl * 1.1 / 1.1)))
            .collect();
        b.polyline(&input, 1.3 * k, with_alpha(ink.dim, 0.75));
        let output: Vec<Pos2> = cv
            .cycle
            .iter()
            .enumerate()
            .map(|(i, (_, y))| pos2(px(i), py(*y, ymax)))
            .collect();
        canvas::trace(b, &output, 2.0 * k, odd, &ink);
        if playing {
            if let Some((i, (_, y))) = cv
                .cycle
                .iter()
                .enumerate()
                .max_by(|a, c| a.1 .1.total_cmp(&c.1 .1))
            {
                let p = pos2(px(i), py(*y, ymax));
                b.circle(p, 5.0 * k, Fill::Solid(with_alpha(odd, 0.35)));
                b.circle(p, 3.0 * k, Fill::Solid(odd));
            }
        }
        let word = |b: &mut crate::kit::Brush, at: Pos2, al: Align2, key: &str| {
            let t = Text::key(key, Role::Micro, ink.dim).caps(true);
            canvas::put(b, at, al, &t, k, None);
        };
        word(
            b,
            pos2(plot.left(), d.top() + 12.0 * k),
            Align2::LEFT_CENTER,
            "fragor.cycle",
        );
        canvas::putf(
            b,
            pos2(plot.right(), d.top() + 12.0 * k),
            Align2::RIGHT_CENTER,
            Role::Micro,
            ink.text,
            k,
            None,
            format_args!("{db} dBFS"),
        );
        // The legend under the plot: what each line is.
        let ly = plot.bottom() + 11.0 * k;
        let r_out = canvas::put(
            b,
            pos2(plot.right(), ly),
            Align2::RIGHT_CENTER,
            &Text::key("fragor.wave.out", Role::Micro, ink.text).caps(true),
            k,
            None,
        );
        b.hline(
            r_out.left() - 16.0 * k,
            r_out.left() - 4.0 * k,
            ly,
            2.0 * k,
            odd,
        );
        let r_in = canvas::put(
            b,
            pos2(r_out.left() - 24.0 * k, ly),
            Align2::RIGHT_CENTER,
            &Text::key("fragor.wave.in", Role::Micro, ink.dim).caps(true),
            k,
            None,
        );
        b.hline(
            r_in.left() - 16.0 * k,
            r_in.left() - 4.0 * k,
            ly,
            1.3 * k,
            with_alpha(ink.dim, 0.75),
        );
        // ---- the harmonics ----
        let hp = Rect::from_min_max(
            hr.min + vec2(18.0 * k, 24.0 * k),
            hr.max - vec2(64.0 * k, 22.0 * k),
        );
        word(
            b,
            pos2(hp.left(), d.top() + 12.0 * k),
            Align2::LEFT_CENTER,
            "fragor.harmonics",
        );
        let floor = 72.0f32;
        for db_line in [0.0f32, -24.0, -48.0, -72.0] {
            let y = hp.top() + hp.height() * (-db_line / floor);
            b.hline(hp.left(), hp.right(), b.snap_mid(y), b.px(), ink.grid);
            canvas::putf(
                b,
                pos2(hp.right() + 6.0 * k, y),
                Align2::LEFT_CENTER,
                Role::Micro,
                ink.dim,
                k,
                None,
                format_args!("{db_line:.0}"),
            );
        }
        let bw = hp.width() / HARMONICS as f32;
        for h in 0..HARMONICS {
            let v = cv.harm[h].clamp(-floor, 0.0);
            let x0 = hp.left() + bw * h as f32 + bw * 0.18;
            let x1 = hp.left() + bw * (h as f32 + 1.0) - bw * 0.18;
            let top = hp.top() + hp.height() * (-v / floor);
            let col = if (h + 1) % 2 == 1 { odd } else { even };
            if v > -floor + 0.5 {
                b.rect(
                    Rect::from_min_max(pos2(x0, top), pos2(x1, hp.bottom())),
                    Fill::V(col, with_alpha(col, 0.35)),
                );
            }
            canvas::putf(
                b,
                pos2((x0 + x1) * 0.5, hp.bottom() + 10.0 * k),
                Align2::CENTER_CENTER,
                Role::Micro,
                ink.dim,
                k,
                None,
                format_args!("{}", h + 1),
            );
        }
        // The legend and the total.
        let lx = hp.right() - 4.0 * k;
        let ly = d.top() + 12.0 * k;
        let r1 = canvas::put(
            b,
            pos2(lx, ly),
            Align2::RIGHT_CENTER,
            &Text::key("fragor.even", Role::Micro, ink.text).caps(true),
            k,
            None,
        );
        b.rect(
            Rect::from_center_size(pos2(r1.left() - 7.0 * k, ly), vec2(7.0 * k, 7.0 * k)),
            Fill::Solid(even),
        );
        let r2 = canvas::put(
            b,
            pos2(r1.left() - 18.0 * k, ly),
            Align2::RIGHT_CENTER,
            &Text::key("fragor.odd", Role::Micro, ink.text).caps(true),
            k,
            None,
        );
        b.rect(
            Rect::from_center_size(pos2(r2.left() - 7.0 * k, ly), vec2(7.0 * k, 7.0 * k)),
            Fill::Solid(odd),
        );
        if empty {
            let t = Text::key("fragor.empty", Role::Label, ink.text);
            canvas::put(b, d.center(), Align2::CENTER_CENTER, &t, k, Some(ink.plate));
        } else {
            canvas::putf(
                b,
                pos2(hr.center().x, d.top() + 12.0 * k),
                Align2::CENTER_CENTER,
                Role::Value,
                ink.text,
                k,
                None,
                format_args!("THD {:.1} %", cv.thd),
            );
        }
    });
}

/// The input and output meters: two bars on a scale from −60 to 0 dBFS, each with its
/// reading.
pub fn meters(ui: &mut egui::Ui, s: &Style, d: Rect, db: [f32; 2]) {
    let k = s.k;
    let ink = Ink::of(s);
    let col = ink.voice(s.hue, false);
    with_brush(&ui.painter_at(d), |b| {
        canvas::ground_b(b, d, &ink, s.hue_tok);
        let left = d.left() + 44.0 * k;
        let right = d.right() - 52.0 * k;
        let x = |v: f32| left + (right - left) * ((v + 60.0) / 60.0).clamp(0.0, 1.0);
        let rows = [("fragor.meter.in", db[0]), ("fragor.meter.out", db[1])];
        let h = (d.height() / 2.0).min(28.0 * k);
        let y0 = d.center().y - h;
        for (i, (key, v)) in rows.iter().enumerate() {
            let cy = y0 + h * (i as f32 + 0.5);
            let t = Text::key(key, Role::Micro, ink.dim).caps(true);
            canvas::put(
                b,
                pos2(d.left() + 8.0 * k, cy),
                Align2::LEFT_CENTER,
                &t,
                k,
                None,
            );
            let track = Rect::from_min_max(pos2(left, cy - 4.0 * k), pos2(right, cy + 4.0 * k));
            b.rrect(track, 2.0 * k, Fill::Solid(ink.grid));
            if *v > -60.0 {
                let fill = Rect::from_min_max(track.min, pos2(x(*v), track.max.y));
                let c = if *v > -0.5 { ink.danger } else { col };
                b.rrect(fill, 2.0 * k, Fill::Solid(c));
            }
            let words = if *v > -60.0 {
                format!("{v:.1}")
            } else {
                "–".to_string()
            };
            let t = Text::new(&words, Role::Value, ink.text);
            canvas::put(
                b,
                pos2(d.right() - 8.0 * k, cy),
                Align2::RIGHT_CENTER,
                &t,
                k,
                None,
            );
        }
        for m in [-48.0f32, -36.0, -24.0, -12.0, 0.0] {
            let xx = b.snap_mid(x(m));
            b.vline(
                xx,
                y0 + 2.0 * h + k,
                y0 + 2.0 * h + 5.0 * k,
                b.px(),
                ink.dim,
            );
        }
    });
}

/// The inspector's picture: the chain as pills in its pedals' finishes, in order.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let st = Style::current().function(frame::hue_of(EffectKind::Pedal));
    let ink = Ink::of(&st);
    let k = st.k;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 40.0 * k), Sense::click());
    let s = unit::settings(spec);
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, st.hue_tok);
        let chain: Vec<usize> = s.chain.iter().flatten().map(|m| m.index()).collect();
        if chain.is_empty() {
            let t = Text::key("fragor.board_empty", Role::Micro, ink.dim).caps(false);
            canvas::put(b, rect.center(), Align2::CENTER_CENTER, &t, k, None);
            return;
        }
        let gap = 14.0 * k;
        let w = (rect.width() - 12.0 * k - gap * (chain.len() as f32 - 1.0)) / chain.len() as f32;
        for (i, m) in chain.iter().enumerate() {
            let f = &pedals::FRAGOR[*m];
            let r = Rect::from_min_size(
                pos2(
                    rect.left() + 6.0 * k + (w + gap) * i as f32,
                    rect.top() + 7.0 * k,
                ),
                vec2(w, rect.height() - 14.0 * k),
            );
            let on = s.on[*m];
            b.rrect(
                r,
                5.0 * k,
                Fill::Solid(with_alpha(f.body.rgb(), if on { 1.0 } else { 0.4 })),
            );
            let ink_c = crate::kit::color::ensure_contrast(f.ink.rgb(), f.body.rgb(), 4.5);
            let t = Text::key(NAMES[*m], Role::Micro, ink_c).caps(true);
            crate::kit::text::paint(b, r.shrink2(vec2(6.0 * k, 0.0)), &t, k);
            if i + 1 < chain.len() {
                let y = r.center().y;
                b.segment(
                    pos2(r.right() + 2.0 * k, y),
                    pos2(r.right() + gap - 2.0 * k, y),
                    1.5 * k,
                    ink.dim,
                    true,
                );
            }
        }
    });
    resp
}

/// The settings `--editor fragor` and the screenshot tool open it with: three pedals —
/// the green overdrive pushing the germanium fuzz, the triangle muff last.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "fragor").set_many(
        &[
            ("s1", 1.0),
            ("s2", 4.0),
            ("s3", 5.0),
            ("od.drive", 0.35),
            ("od.tone", 0.6),
            ("od.level", 0.7),
            ("ge.fuzz", 0.75),
            ("ge.bias", 0.8),
            ("ge.vol", 0.45),
            ("mf.sustain", 0.55),
            ("mf.tone", 0.4),
            ("mf.vol", 0.5),
            ("gate", -62.0),
        ],
        "cmd.fx_preset",
    );
}

/// What a playing engine would report, for screenshots with no engine: a plucked note at
/// a guitar's level through the board as set.
pub fn demo_feed(app: &mut CantusApp) {
    demo_feed_at(app, 0.35);
}

/// [`demo_feed`] with a note of peak `amp` (0: silence — the unit at rest).
pub fn demo_feed_at(app: &mut CantusApp, amp: f32) {
    let Some((strip, effect, _)) = app.fx.open else {
        return;
    };
    let Some(slot) = app.project().effect(strip, effect).cloned() else {
        return;
    };
    use cantus_engine::FxUnit;
    let mut u = unit::Unit::new(SR);
    u.configure(&slot.spec);
    let n = 9_600;
    let w = std::f32::consts::TAU * 196.0 / SR;
    let (mut l, mut r): (Vec<f32>, Vec<f32>) = (0..n)
        .map(|i| {
            let v = amp * cantus_dsp::math::sin(w * i as f32);
            (v, v)
        })
        .unzip();
    for (a, c) in l.chunks_mut(480).zip(r.chunks_mut(480)) {
        u.process(a, c, None);
    }
    app.fx.injected = Some(u.telemetry());
}
