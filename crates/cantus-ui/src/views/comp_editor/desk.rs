//! Where Comprimo reaches the rest of Cantus: the inspector's small curve, the strip's
//! gain-reduction lane and "Duck this strip from…" on the desk — and the demo the
//! screenshots and `--editor comp` open it with.

use super::*;
use crate::design::space;
use crate::views::fx::canvas::{self, Ink};
use cantus_dsp::comprimo::effective_gain_reduction_db;

/// `4.0:1`, `∞:1`.
struct Ratio(f32);

impl std::fmt::Display for Ratio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.0 >= RATIO_INF - 0.05 {
            f.write_str("\u{221e}:1")
        } else {
            write!(f, "{:.1}:1", self.0)
        }
    }
}

/// A small, read-only transfer curve for the inspector's effect card, with the numbers
/// that matter at rest: threshold, ratio, style (starred when its engine is custom), and
/// the auto gain and lookahead if on.
pub fn mini_curve(ui: &mut egui::Ui, spec: &EffectSpec) -> egui::Response {
    let s = Style::current().function(frame::hue_of(EffectKind::Compressor));
    let ink = Ink::of(&s);
    let cs = cantus_engine::comp_settings(spec);
    let h = space::GRID * 14.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::click());
    let sq = Rect::from_min_size(rect.min, vec2(h, h)).shrink(space::S1);
    let lo = -60.0f32;
    let x = |d: f32| sq.left() + ((d - lo) / -lo).clamp(0.0, 1.0) * sq.width();
    let y = |d: f32| sq.bottom() - ((d - lo) / -lo).clamp(0.0, 1.0) * sq.height();
    with_brush(&ui.painter_at(rect), |b| {
        canvas::ground_b(b, rect, &ink, s.hue_tok);
        b.segment(
            pos2(x(lo), y(lo)),
            pos2(x(0.0), y(0.0)),
            b.px(),
            ink.grid_strong,
            false,
        );
        let mut pts = [Pos2::ZERO; 61];
        for (i, p) in pts.iter_mut().enumerate() {
            let d = lo + -lo * i as f32 / 60.0;
            *p = pos2(x(d), y(d - cs.mix * effective_gain_reduction_db(d, &cs)));
        }
        b.polyline(&pts, 1.5, ink.hue);
        let words = Rect::from_min_max(pos2(sq.right() + space::S2, rect.top()), rect.max);
        let style = frame::tr(STYLE_KEYS[cs.style.index().min(8)]);
        let star = if cs.engine.is_some() { "*" } else { "" };
        let auto = if cs.auto_gain {
            frame::tr("comp.auto")
        } else {
            ""
        };
        let sep = if cs.auto_gain { " \u{b7} " } else { "" };
        if cs.lookahead_ms > 0.0 {
            canvas::line_in(
                b,
                words,
                Role::Body,
                ink.text,
                s.tk(),
                format_args!(
                    "{:+.0} dB \u{b7} {} \u{b7} {style}{star}{sep}{auto} \u{b7} {:.1} ms",
                    cs.threshold_db,
                    Ratio(cs.ratio),
                    cs.lookahead_ms
                ),
            );
        } else {
            canvas::line_in(
                b,
                words,
                Role::Body,
                ink.text,
                s.tk(),
                format_args!(
                    "{:+.0} dB \u{b7} {} \u{b7} {style}{star}{sep}{auto}",
                    cs.threshold_db,
                    Ratio(cs.ratio),
                ),
            );
        }
        let edge = if resp.hovered() { ink.hue } else { ink.edge };
        canvas::edge_b(b, rect, &Ink { edge, ..ink });
    });
    if resp.hovered() {
        hint::set_text(ui.ctx(), frame::tr("comp.open_tip"), "", Caps::NONE);
    }
    resp
}

/// The strip menu's "Duck this strip from…": a key that opens the kit's list of the
/// strips that can key it (never one that would close a loop); choosing one adds a
/// Comprimo keyed from it, set to duck, in one step of undo, and opens its editor.
pub fn duck_menu(app: &mut CantusApp, ui: &mut egui::Ui, strip: StripId) {
    let s = Style::current().function(frame::hue_of(EffectKind::Compressor));
    let id = ui.id().with(("comp-duck", strip.0));
    let open_id = id.with("open");
    let mut open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    let width = (ui.available_width() / s.k).max(160.0);
    let resp = controls::key(
        ui,
        &s,
        Src::Key("comp.duck.menu"),
        frame::tr("comp.duck.menu"),
        width,
        open,
    );
    if resp.hovered() {
        hint::set_text(
            ui.ctx(),
            frame::tr("comp.duck.menu"),
            frame::tr("comp.duck.hint"),
            Caps {
                menu: true,
                ..Caps::NONE
            },
        );
    }
    if resp.clicked() {
        open = !open;
    }
    if open {
        // The rows exist only while the list is open.
        let options: Vec<(StripId, String)> = app
            .project()
            .strips
            .iter()
            .map(|x| x.id)
            .filter(|id| *id != strip && !app.project().routing_would_cycle(*id, Some(strip)))
            .map(|id| {
                (
                    id,
                    app.capture_label(cantus_engine::CaptureTarget::Strip(id)),
                )
            })
            .collect();
        let items: Vec<Item<'_>> = options
            .iter()
            .map(|(_, name)| frame::action(Src::Str(name)))
            .collect();
        let mut close = false;
        let picked = frame::popup_list(
            ui.ctx(),
            id.with("list"),
            resp.rect.right_top(),
            resp.rect,
            &s,
            &items,
            resp.rect.width().max(s.d(180.0)),
            &mut close,
        );
        if let Some((from, _)) = picked.and_then(|i| options.get(i)) {
            if let Some(effect) = app.duck_strip_from(strip, *from) {
                app.open_effect_editor(strip, effect);
            }
            close = true;
            ui.close();
        }
        if close {
            open = false;
        }
    }
    ui.data_mut(|d| d.insert_temp(open_id, open));
}

/// A strip's gain-reduction lane on the desk, beside its level meter: shown on a strip
/// that carries a Comprimo, hanging from the top in the danger colour over 0–24 dB (ticks
/// every 6 dB), so a strip that is working reads at a glance and the faders stay in line.
pub fn strip_gr_meter(app: &CantusApp, ui: &mut egui::Ui, strip: StripId, height: f32) {
    let Some(gr) = app.comp_gr_on_strip(strip) else {
        return;
    };
    let tok = Style::current().t;
    let (rect, resp) = ui.allocate_exact_size(vec2(space::GRID * 1.5, height), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 1.0, tok.meter_well);
    if let Some(g) = gr {
        let h = (g / 24.0).clamp(0.0, 1.0) * rect.height();
        if h > 0.5 {
            // One filled bar in the danger colour (the desk's test counts it).
            p.rect_filled(
                Rect::from_min_size(rect.min, vec2(rect.width(), h)).shrink(0.5),
                0.5,
                tok.danger,
            );
        }
    }
    for db in [6.0f32, 12.0, 18.0] {
        let y = rect.top() + db / 24.0 * rect.height();
        p.hline(rect.x_range(), y, egui::Stroke::new(1.0, tok.grid_bar));
    }
    p.rect_stroke(
        rect,
        1.0,
        egui::Stroke::new(1.0, tok.danger.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
    if resp.hovered() {
        let text = match gr {
            Some(g) => tf("comp.strip_gr", &[("db", &format!("{g:.1}"))]),
            None => t("comp.strip_gr_idle"),
        };
        hint::set_text(ui.ctx(), frame::tr("comp.meter_gr"), &text, Caps::NONE);
    }
}

/// `--editor comp` and the screenshots: a compressor doing a job — a Punch style gently
/// customised, lookahead, auto gain, and a key that ignores the low end — with the engine
/// card open, so every part of the editor has something to show.
pub fn demo_setup(app: &mut CantusApp, strip: StripId, effect: EffectId) {
    DawHost::new(app, strip, effect, "comp").set_many(
        &[
            ("style", 1.0),
            ("thr", -26.0),
            ("ratio", 4.0),
            ("knee", 9.0),
            ("att", 6.0),
            ("rel", 140.0),
            ("hold", 4.0),
            ("look", 2.0),
            ("auto_gain", 1.0),
            ("custom", 1.0),
            ("det_rms", 0.1),
            ("det_fb", 0.4),
            ("det_prog", 0.0),
            ("det_dual", 0.35),
            ("det_curve", 0.0),
            ("sc0.used", 1.0),
            ("sc0.shape", EqShape::LowCut.index() as f32),
            ("sc0.freq", 90.0),
            ("sc0.slope", 24.0),
            ("sc1.used", 1.0),
            ("sc1.freq", 2_500.0),
            ("sc1.gain", 4.0),
            ("sc1.q", 1.2),
        ],
        "cmd.fx_preset",
    );
    let ed = &mut app.comp_editor;
    ed.engine_open = true;
    // A stand-in transport: a beat every 30 frames, as the screenshot's drum loop.
    ed.demo = Some(DemoClock {
        tick: 0.0,
        step: cantus_core::PPQ as f64 / 30.0,
    });
}

/// What a playing engine would report, for screenshots and tests with no engine: the
/// real DSP run over a drum loop with the effect's settings — its telemetry, its
/// loudness blocks, the analyser's pairs, and (when nothing has been fed yet) the history.
pub fn demo_feed(app: &mut CantusApp) {
    let Some((strip, effect)) = app.comp_editor.open else {
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
    let mut c = cantus_dsp::Comprimo::new(DISPLAY_SR);
    c.set(&cantus_engine::comp_settings(&spec));
    let frame = 800;
    let mut seed = 5u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let ed = &mut app.comp_editor;
    let fill_history = ed.history.is_empty();
    let mut pairs = Vec::with_capacity(8_192 * 2);
    let mut last_hops = 0.0;
    let frames = 600;
    for f in 0..frames {
        let mut l = Vec::with_capacity(frame);
        for i in 0..frame {
            // A kick every 30 frames, a snare between, hats, and a bass line under it.
            let n = f * frame + i;
            let beat = (n % (30 * frame)) as f32 / DISPLAY_SR;
            let kick = (-beat * 18.0).exp() * (std::f32::consts::TAU * 55.0 * beat).sin();
            let off = ((n + 15 * frame) % (30 * frame)) as f32 / DISPLAY_SR;
            let snare = if (n / (30 * frame)) % 2 == 1 {
                (-off * 25.0).exp() * noise() * 0.6
            } else {
                0.0
            };
            let hat =
                (-(((n % (15 * frame / 2)) as f32) / DISPLAY_SR) * 60.0).exp() * noise() * 0.15;
            let bass = 0.12 * (std::f32::consts::TAU * 82.0 * n as f32 / DISPLAY_SR).sin();
            l.push(0.8 * kick + snare + hat + bass);
        }
        let dry = l.clone();
        let mut r = l.clone();
        c.process(&mut l, &mut r, None);
        let m = c.meter();
        if fill_history {
            ed.feed(m.input, m.output, m.gr_db);
        }
        let tel = c.telemetry();
        let hops = tel[tele::HOPS];
        let new = ((hops - last_hops) as usize).min(cantus_dsp::comprimo::BLOCKS_REPORTED);
        for i in (0..new).rev() {
            ed.blocks.push(tel[tele::BLOCKS + i]);
        }
        last_hops = hops;
        if f >= frames - 11 {
            for (a, b) in dry.iter().zip(&l) {
                pairs.push(*a);
                pairs.push(*b);
            }
        }
    }
    let an = ed
        .analyser
        .get_or_insert_with(|| SpectrumAnalyser::new(ANALYSER_N));
    an.push(&pairs);
    an.compute();
    // The bell on the key is selected, so its numbers are on screen.
    if ed.key_band.is_none() {
        ed.key_band = Some(1);
    }
    let m = c.meter();
    ed.now = (db(m.input), db(m.input) - m.gr_db);
    app.fx.injected = Some(c.telemetry());
}
