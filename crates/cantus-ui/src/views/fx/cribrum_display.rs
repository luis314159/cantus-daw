//! Cribrum's display: **what passes**. The music's spectrum as grains on a perforated plate
//! — lit up to the level that comes out, dimmed from there to the level that went in (what
//! the sieve holds back) — under the response of both filters as they move, each filter's
//! handle (dragged: across for the cutoff, up and down for the resonance) with the reach of
//! its movement drawn as a track through it.

use super::super::canvas::{self, Ink};
use super::super::controls;
use super::super::frame;
use super::super::host::FxHost;
use super::State;
use crate::kit::color::with_alpha;
use crate::kit::hint::{self, Caps};
use crate::kit::text::{Role, Src, Text};
use crate::kit::{with_brush, Brush, Fill, Style};
use cantus_core::EffectSpec;
use cantus_dsp::cribrum::filter::{effective, Mode, Voice};
use cantus_dsp::cribrum::{response, t, CribrumSettings, FilterParams};
use cantus_dsp::modfx_clock::tel;
use cantus_dsp::va::filter::Model;
use cantus_dsp::SpectrumAnalyser;
use egui::{pos2, vec2, Align2, Color32, Pos2, Rect, Sense};

/// The spectrum's columns.
const BINS: usize = 72;
/// The response's scale, dB.
const TOP_DB: f32 = 24.0;
const BOT_DB: f32 = -36.0;
/// The spectrum's scale, dBFS (its own: grains are a level, the curve a gain).
const SPEC_TOP: f32 = 12.0;
const SPEC_BOT: f32 = -96.0;
/// How fast a column falls, dB a frame.
const FALL: f32 = 1.2;
const LO: f32 = 20.0;
const HI: f32 = 20_000.0;
/// Points on a response.
const N: usize = 241;

/// A handle being dragged: which filter, its resonance and the pointer's height then.
#[derive(Copy, Clone, Debug)]
struct Drag {
    filter: usize,
    res0: f32,
    y0: f32,
}

/// The display's own memory: the analyser and the columns it last measured.
pub struct View {
    analyser: Option<SpectrumAnalyser>,
    analysed: Option<(usize, u64)>,
    scratch: Vec<f32>,
    spec_in: [f32; BINS],
    spec_out: [f32; BINS],
    drag: Option<Drag>,
}

impl Default for View {
    fn default() -> Self {
        View {
            analyser: None,
            analysed: None,
            scratch: Vec::new(),
            spec_in: [SPEC_BOT; BINS],
            spec_out: [SPEC_BOT; BINS],
            drag: None,
        }
    }
}

impl std::fmt::Debug for View {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("View").field("drag", &self.drag).finish()
    }
}

/// The plot's mapping.
#[derive(Copy, Clone, Debug)]
struct Map {
    plot: Rect,
}

impl Map {
    fn x(&self, hz: f32) -> f32 {
        let u = (hz.clamp(LO, HI) / LO).ln() / (HI / LO).ln();
        self.plot.left() + self.plot.width() * u
    }
    fn hz(&self, x: f32) -> f32 {
        let u = ((x - self.plot.left()) / self.plot.width()).clamp(0.0, 1.0);
        LO * (HI / LO).powf(u)
    }
    fn y(&self, db: f32) -> f32 {
        let v = (TOP_DB - db) / (TOP_DB - BOT_DB);
        self.plot.top() + self.plot.height() * v.clamp(0.0, 1.0)
    }
    fn ys(&self, dbfs: f32) -> f32 {
        let v = (SPEC_TOP - dbfs) / (SPEC_TOP - SPEC_BOT);
        self.plot.top() + self.plot.height() * v.clamp(0.0, 1.0)
    }
}

fn bin_hz(i: usize) -> f32 {
    LO * (HI / LO).powf((i as f32 + 0.5) / BINS as f32)
}

/// A filter's settings as a voice at a cutoff, resonance and vowel.
fn voice(p: &FilterParams, cut: f32, res: f32, vowel: f32) -> Voice {
    Voice {
        model: p.model,
        mode: p.mode,
        slope24: p.slope24,
        cut,
        res,
        drive: cantus_dsp::math::db_to_gain(p.drive_db),
        vowel,
    }
}

/// The two voices as the display draws them now: moved (from the telemetry) while the
/// unit plays, as set otherwise.
fn voices(cs: &CribrumSettings, tel: &[f32; 64], live: bool) -> [Voice; 2] {
    if live {
        [
            voice(&cs.a, tel[t::CUT_A_L], tel[t::RES_A], tel[t::VOWEL_A]),
            voice(&cs.b, tel[t::CUT_B_L], tel[t::RES_B], tel[t::VOWEL_B]),
        ]
    } else {
        [
            voice(&cs.a, cs.a.cut, cs.a.res, cs.a.vowel),
            voice(&cs.b, cs.b.cut, cs.b.res, cs.b.vowel),
        ]
    }
}

/// The reach of filter `f`'s movement, octaves below and above its cutoff.
fn reach(cs: &CribrumSettings, f: usize) -> (f32, f32) {
    let (mut lo, mut hi) = (0.0f32, 0.0f32);
    let l = cs.lfo_depth.cut.abs();
    lo -= l;
    hi += l;
    for d in [cs.env_depth.cut, cs.curve_depth.cut] {
        lo += d.min(0.0);
        hi += d.max(0.0);
    }
    let tr = if f == 0 { 1.0 } else { cs.b_track };
    let (a, b) = (lo * tr, hi * tr);
    (a.min(b) - cs.spread * 0.5, a.max(b) + cs.spread * 0.5)
}

/// Whether the unit is playing and its telemetry is the moved filters'.
pub fn live(tel: &[f32; 64]) -> bool {
    tel[tel::BLOCKS] > 0.0 && tel[t::ENGAGED] > 0.5
}

pub fn card(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &mut egui::Ui,
    s: &Style,
    rect: Rect,
    tel: &[f32; 64],
    cs: &CribrumSettings,
) {
    controls::card(
        ui,
        s,
        rect,
        Src::Key("cribrum.card.response"),
        None,
        |ui, cr| {
            let r = cr.body;
            let k = s.k;
            let map = Map {
                plot: Rect::from_min_max(
                    pos2(r.left() + 30.0 * k, r.top() + 30.0 * k),
                    pos2(r.right() - 12.0 * k, r.bottom() - 18.0 * k),
                ),
            };
            st.display = Some(map.plot);
            update_spectrum(&mut st.view, host.pairs(), host.sample_rate());
            let resp = ui.interact(
                map.plot,
                ui.id().with("cribrum-display"),
                Sense::click_and_drag(),
            );
            edit(host, st, ui, &resp, map, cs, k);
            let cs = cantus_engine::fx::cribrum::settings(host.spec());
            paint(ui, s, r, map, st, tel, &cs, host.sample_rate());
            if resp.hovered() {
                hint::set_text(
                    ui.ctx(),
                    frame::tr("cribrum.display_tip"),
                    frame::tr("cribrum.display_how"),
                    Caps::NONE,
                );
            }
            if cs.mix == 0.0 {
                super::super::modfx::idle_note(ui, s, r, "cribrum.idle");
            }
        },
    );
}

/// The spectrum of what the analyser tap brought: the input (first of each pair) and the
/// output, falling slowly. Only when the tap brought something new.
fn update_spectrum(v: &mut View, pairs: &std::collections::VecDeque<(f32, f32)>, sr: f32) {
    let a = v
        .analyser
        .get_or_insert_with(|| SpectrumAnalyser::new(4096));
    let n = a.size();
    let sig = (
        pairs.len(),
        pairs.back().map_or(0, |p| {
            (u64::from(p.0.to_bits()) << 32) | u64::from(p.1.to_bits())
        }),
    );
    if pairs.len() < n || v.analysed == Some(sig) {
        return;
    }
    v.analysed = Some(sig);
    v.scratch.clear();
    for (x, y) in pairs.iter().skip(pairs.len() - n) {
        v.scratch.push(*x);
        v.scratch.push(*y);
    }
    a.push(&v.scratch);
    a.compute();
    for i in 0..BINS {
        let hz = bin_hz(i);
        // A gentle tilt (+1.5 dB an octave from 1 kHz), as the analysers draw music.
        let tilt = 1.5 * (hz / 1_000.0).log2();
        let fin = a.level_db(0, sr, hz, 1.0 / 6.0) + tilt;
        let fout = a.level_db(1, sr, hz, 1.0 / 6.0) + tilt;
        v.spec_in[i] = fin.max(v.spec_in[i] - FALL).max(SPEC_BOT);
        v.spec_out[i] = fout.max(v.spec_out[i] - FALL).max(SPEC_BOT);
    }
}

/// Where filter `f`'s handle sits: its cutoff across, its own response there up and down.
fn handle(map: Map, v: &Voice, sr: f32) -> Pos2 {
    let (mode, _) = effective(v.model, v.mode, v.slope24);
    let at = if mode == Mode::Vowel {
        cantus_dsp::cribrum::filter::formants(v.vowel, v.cut, v.res)[0].0
    } else {
        v.cut
    };
    let db = 20.0 * response::response(v, at, sr).abs().max(1e-6).log10() as f32;
    pos2(map.x(at), map.y(db.clamp(BOT_DB + 3.0, TOP_DB - 3.0)))
}

fn edit(
    host: &mut dyn FxHost,
    st: &mut State,
    ui: &egui::Ui,
    resp: &egui::Response,
    map: Map,
    cs: &CribrumSettings,
    k: f32,
) {
    let sr = host.sample_rate();
    let set = voices(cs, &[0.0; 64], false);
    let shown = |f: usize| f == 0 || cs.b_on;
    if resp.drag_started() {
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(p) = origin.or(resp.interact_pointer_pos()) {
            let near = (0..2)
                .filter(|f| shown(*f))
                .map(|f| (f, handle(map, &set[f], sr).distance(p)))
                .filter(|(_, d)| *d <= 16.0 * k)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(f, _)| f);
            let f = near.unwrap_or(st.filter);
            st.filter = f;
            let res0 = if f == 0 { cs.a.res } else { cs.b.res };
            st.view.drag = Some(Drag {
                filter: f,
                res0,
                y0: p.y,
            });
        }
    }
    if let (Some(d), Some(p), true) = (st.view.drag, resp.interact_pointer_pos(), resp.dragged()) {
        let keys = super::FILTER_KEYS[d.filter];
        let fine = ui.input(|i| i.modifiers.shift);
        let cut = map.hz(p.x).clamp(LO, HI);
        let span = if fine { 4.0 } else { 1.25 };
        let res = (d.res0 + (d.y0 - p.y) / map.plot.height() * span).clamp(0.0, 1.0);
        host.set(keys[3], cut, Some("cribrum.handle"));
        host.set(keys[4], res, Some("cribrum.handle"));
    }
    if resp.drag_stopped() {
        st.view.drag = None;
        host.end_gesture();
    }
    // A click takes a filter's handle into the Filter card; a double-click puts its cutoff
    // and resonance back to their defaults (one step).
    if let Some(p) = resp.interact_pointer_pos() {
        let near = (0..2)
            .filter(|f| shown(*f))
            .map(|f| (f, handle(map, &set[f], sr).distance(p)))
            .filter(|(_, d)| *d <= 16.0 * k)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(f, _)| f);
        if let Some(f) = near {
            if resp.clicked() {
                st.filter = f;
            }
            if resp.double_clicked() {
                let keys = super::FILTER_KEYS[f];
                host.apply(
                    &[(keys[3], None), (keys[4], None)],
                    "cmd.cribrum_handle_reset",
                    None,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint(
    ui: &egui::Ui,
    s: &Style,
    r: Rect,
    map: Map,
    st: &State,
    tel: &[f32; 64],
    cs: &CribrumSettings,
    sr: f32,
) {
    let ink = Ink::of(s);
    let k = s.k;
    let plot = map.plot;
    let live = live(tel);
    let vs = voices(cs, tel, live);
    let base = voices(cs, tel, false);
    let b_on = cs.b_on;
    let second = ink.voice(s.hue, true);
    let spectrum = st.view.analysed.is_some();
    with_brush(&ui.painter_at(r), |b| {
        canvas::ground_b(b, r, &ink, s.hue_tok);
        plate(b, map, &ink, k);
        if spectrum {
            grains(b, map, &st.view, &ink, k);
        }
        // The response: as set (faint, dashed) while it moves; as heard now, filled.
        let line = |v: &[Voice; 2]| -> Vec<Pos2> {
            (0..N)
                .map(|j| {
                    let hz = LO * (HI / LO).powf(j as f32 / (N - 1) as f32);
                    let db = response::chain_db(&v[0], b_on.then_some(&v[1]), cs.parallel, hz, sr);
                    pos2(map.x(hz), map.y(db))
                })
                .collect()
        };
        let now = line(&vs);
        if live {
            let set = line(&base);
            b.dashed(&set, 0.8 * k, with_alpha(ink.text, 0.35), 4.0 * k, 3.0 * k);
        }
        b.area(&now, plot.bottom(), ink.fill(0.45));
        if b_on {
            for (f, color) in [(0usize, ink.hue), (1, second)] {
                let one: Vec<Pos2> = (0..N)
                    .map(|j| {
                        let hz = LO * (HI / LO).powf(j as f32 / (N - 1) as f32);
                        let db = 20.0 * response::response(&vs[f], hz, sr).abs().max(1e-6).log10();
                        pos2(map.x(hz), map.y(db as f32))
                    })
                    .collect();
                b.polyline(&one, 0.8 * k, with_alpha(color, 0.55));
            }
        }
        canvas::trace(b, &now, 1.4 * k, ink.hue, &ink);
        // The handles: the reach of the movement, the cutoff as set (a ring) and now (a dot).
        for f in 0..2 {
            if f == 1 && !b_on {
                continue;
            }
            let color = if f == 0 { ink.hue } else { second };
            let at_set = handle(map, &base[f], sr);
            let at_now = handle(map, &vs[f], sr);
            let (lo, hi) = reach(cs, f);
            let chosen = st.filter == f;
            // The reach of the movement, for the filter in hand: a rail through its handle
            // from the lowest cutoff the sources can take it to the highest, ticked at
            // both ends, the stretch it has moved now drawn solid.
            if chosen && hi - lo > 0.01 {
                let c = base[f].cut;
                let (x0, x1) = (map.x(c * 2f32.powf(lo)), map.x(c * 2f32.powf(hi)));
                let y = at_set.y;
                let rail = with_alpha(color, 0.55);
                b.hline(x0, x1, y, 0.8 * k, rail);
                for x in [x0, x1] {
                    b.vline(x, y - 3.5 * k, y + 3.5 * k, 0.8 * k, rail);
                }
                if live {
                    let xn = map.x(vs[f].cut);
                    b.hline(at_set.x.min(xn), at_set.x.max(xn), y, 1.6 * k, color);
                    b.dot(pos2(xn, y), 2.4 * k, color);
                }
            }
            b.circle_stroke(
                at_set,
                6.5 * k,
                if chosen { 1.3 } else { 0.9 } * k,
                with_alpha(color, if chosen { 1.0 } else { 0.7 }),
            );
            if live {
                b.dot(at_now, 2.8 * k, color);
            } else {
                b.dot(at_set, 2.2 * k, color);
            }
            let letter = if f == 0 { "A" } else { "B" };
            let t = Text::new(letter, Role::Micro, ink.text).caps(false);
            canvas::put(
                b,
                pos2(at_set.x, at_set.y - 10.0 * k),
                Align2::CENTER_BOTTOM,
                &t,
                k,
                Some(ink.plate),
            );
        }
        words(b, s, r, map, cs, &vs, &ink, k);
        canvas::edge_b(b, r, &ink);
    });
}

/// The perforated plate: a hole every third of an octave and every 6 dB, the decades and
/// 0 dB ruled, their numbers at the edges.
fn plate(b: &mut Brush, map: Map, ink: &Ink, k: f32) {
    let plot = map.plot;
    let mut db = BOT_DB;
    while db <= TOP_DB + 0.1 {
        let y = map.y(db);
        let mut hz = LO;
        while hz <= HI * 1.01 {
            b.dot(pos2(map.x(hz), y), 0.7 * k, ink.grid);
            hz *= 2f32.powf(1.0 / 3.0);
        }
        db += 6.0;
    }
    for hz in [100.0f32, 1_000.0, 10_000.0] {
        b.vline(
            map.x(hz),
            plot.top(),
            plot.bottom(),
            b.px(),
            ink.grid_strong,
        );
    }
    b.hline(
        plot.left(),
        plot.right(),
        map.y(0.0),
        b.px(),
        ink.grid_strong,
    );
    for (hz, word) in [
        (50.0f32, "50"),
        (100.0, "100"),
        (200.0, "200"),
        (500.0, "500"),
        (1_000.0, "1k"),
        (2_000.0, "2k"),
        (5_000.0, "5k"),
        (10_000.0, "10k"),
    ] {
        let t = Text::new(word, Role::Micro, ink.dim).caps(false);
        canvas::put(
            b,
            pos2(map.x(hz), plot.bottom() + 3.0 * k),
            Align2::CENTER_TOP,
            &t,
            k,
            None,
        );
    }
    for (db, word) in [
        (12.0f32, "+12"),
        (0.0, "0"),
        (-12.0, "\u{2212}12"),
        (-24.0, "\u{2212}24"),
    ] {
        let t = Text::new(word, Role::Micro, ink.dim).caps(false);
        canvas::put(
            b,
            pos2(plot.left() - 5.0 * k, map.y(db)),
            Align2::RIGHT_CENTER,
            &t,
            k,
            None,
        );
    }
}

/// The spectrum as grains: lit up to what comes out; over each column a faint cap where
/// what went in reached — the gap between is what the sieve held back.
fn grains(b: &mut Brush, map: Map, v: &View, ink: &Ink, k: f32) {
    let plot = map.plot;
    let cw = plot.width() / BINS as f32;
    let pitch = (4.0 * k).max(cw * 0.45);
    let size = vec2((cw - 2.0 * k).max(1.0), (pitch - 1.5 * k).max(1.0));
    let lit = with_alpha(ink.hue, if ink.dark { 0.42 } else { 0.34 });
    let cap = with_alpha(ink.text, if ink.dark { 0.30 } else { 0.28 });
    for i in 0..BINS {
        let x = plot.left() + cw * i as f32 + (cw - size.x) * 0.5;
        let top_out = map.ys(v.spec_out[i].min(v.spec_in[i]));
        let mut y = plot.bottom() - pitch;
        while y >= top_out - 0.5 {
            b.rect(
                Rect::from_min_size(pos2(x, y + (pitch - size.y) * 0.5), size),
                Fill::Solid(lit),
            );
            y -= pitch;
        }
        let top_in = map.ys(v.spec_in[i]);
        if top_out - top_in > pitch {
            b.rect(
                Rect::from_min_size(pos2(x, top_in), vec2(size.x, (1.5 * k).max(b.px()))),
                Fill::Solid(cap),
            );
        }
    }
}

/// The corner words: each filter's model, response and cutoff; the vowels across the top
/// when one is a voice.
#[allow(clippy::too_many_arguments)]
fn words(
    b: &mut Brush,
    s: &Style,
    r: Rect,
    map: Map,
    cs: &CribrumSettings,
    vs: &[Voice; 2],
    ink: &Ink,
    k: f32,
) {
    let line = |f: usize| -> String {
        let p = if f == 0 { &cs.a } else { &cs.b };
        let v = &vs[f];
        let (mode, s24) = effective(p.model, p.mode, p.slope24);
        let model = super::model_index(p.model);
        let mut out = String::new();
        out.push_str(if f == 0 { "A  " } else { "B  " });
        out.push_str(frame::tr(super::MODEL_SHORT[model]));
        out.push_str(" \u{b7} ");
        out.push_str(frame::tr(super::MODE_KEYS[mode as usize]));
        if mode != Mode::Vowel {
            out.push_str(if s24 { " 24" } else { " 12" });
            out.push_str("  ");
            controls::write_readout(v.cut, "Hz", &mut out);
        } else {
            out.push_str("  ");
            super::vowel_pill(v.vowel, &mut out);
        }
        out.push_str("  ");
        controls::write_readout(v.res, "%", &mut out);
        out
    };
    let a = line(0);
    canvas::put(
        b,
        pos2(r.left() + 8.0 * k, r.top() + 6.0 * k),
        Align2::LEFT_TOP,
        &Text::new(&a, Role::Value, ink.text).caps(false),
        k,
        Some(ink.plate),
    );
    if cs.b_on {
        let bl = line(1);
        canvas::put(
            b,
            pos2(r.right() - 8.0 * k, r.top() + 6.0 * k),
            Align2::RIGHT_TOP,
            &Text::new(&bl, Role::Value, ink.voice(s.hue, true)).caps(false),
            k,
            Some(ink.plate),
        );
    }
    let vowel_f = (0..2).find(|f| {
        let p = if *f == 0 { &cs.a } else { &cs.b };
        (*f == 0 || cs.b_on) && effective(p.model, p.mode, p.slope24).0 == Mode::Vowel
    });
    if let Some(f) = vowel_f {
        let now = vs[f].vowel;
        let cx = map.plot.center().x;
        for (i, l) in ["A", "E", "I", "O", "U"].iter().enumerate() {
            let near = 1.0 - (now - i as f32).abs().min(1.0);
            let c = if near > 0.0 {
                with_alpha(ink.hue, 0.4 + 0.6 * near)
            } else {
                ink.dim
            };
            let t = Text::new(l, Role::Value, c).caps(false);
            canvas::put(
                b,
                pos2(cx + (i as f32 - 2.0) * 22.0 * k, r.top() + 6.0 * k),
                Align2::CENTER_TOP,
                &t,
                k,
                None,
            );
        }
    }
}

/// A response's shape drawn small on a key: the state-variable's (the vowel: its three
/// formants) at a moderate resonance.
pub fn mode_glyph(b: &mut Brush, r: Rect, mode: Mode, ink: Color32, k: f32) {
    const M: usize = 33;
    let v = Voice {
        model: Model::Svf,
        mode,
        slope24: false,
        cut: 1_000.0,
        res: match mode {
            Mode::Vowel => 0.4,
            Mode::Peak => 0.85,
            _ => 0.55,
        },
        vowel: 0.0,
        ..Voice::default()
    };
    let (lo, hi) = (60.0f32, 16_000.0f32);
    let pts: Vec<Pos2> = (0..M)
        .map(|j| {
            let u = j as f32 / (M - 1) as f32;
            let hz = lo * (hi / lo).powf(u);
            let db = (20.0 * response::response(&v, hz, 48_000.0).abs().max(1e-6).log10()) as f32;
            let vv = ((db + 30.0) / 42.0).clamp(0.0, 1.0);
            pos2(r.left() + r.width() * u, r.bottom() - r.height() * vv)
        })
        .collect();
    b.polyline(&pts, 1.0 * k, ink);
}

/// The inspector's picture: the response as set, and its words.
pub fn preview(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let st = Style::current().function(frame::hue_of(super::KIND));
    let ink = Ink::of(&st);
    let k = st.k;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 56.0 * k), Sense::click());
    let cs = cantus_engine::fx::cribrum::settings(spec);
    let vs = voices(&cs, &[0.0; 64], false);
    let plot = rect.shrink2(vec2(6.0 * k, 8.0 * k));
    let map = Map { plot };
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, st.hue_tok);
        let pts: Vec<Pos2> = (0..121)
            .map(|j| {
                let hz = LO * (HI / LO).powf(j as f32 / 120.0);
                let db = response::chain_db(
                    &vs[0],
                    cs.b_on.then_some(&vs[1]),
                    cs.parallel,
                    hz,
                    48_000.0,
                );
                pos2(map.x(hz), map.y(db))
            })
            .collect();
        b.area(&pts, plot.bottom(), ink.fill(0.4));
        canvas::trace(b, &pts, 1.0 * k, ink.hue, &ink);
        let model = super::model_index(cs.a.model);
        let mut w = String::new();
        w.push_str(frame::tr(super::MODEL_SHORT[model]));
        w.push_str(" \u{b7} ");
        controls::write_readout(cs.a.cut, "Hz", &mut w);
        canvas::put(
            b,
            pos2(rect.left() + 6.0 * k, rect.top() + 4.0 * k),
            Align2::LEFT_TOP,
            &Text::new(&w, Role::Micro, ink.text).caps(false),
            k,
            Some(ink.plate),
        );
        canvas::edge_b(b, rect, &ink);
    });
    resp
}
