//! The mastering assistant's window: the choices, what it hears against the target, the
//! octave balance drawn, the proposed chain and why, *Apply*, the A/B. Hand-drawn with the
//! kit (ADR 0024); movable, resizable, its body scrolls.

use super::{Done, Side};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::glyph::Glyph;
use crate::kit::text::{self, Role, Src, Text};
use crate::kit::{self, with_brush, Fill, Style};
use crate::views::workspace_settings as ws;
use crate::views::workspace_widgets as w;
use cantus_dsp::speculum::ear::{Reading, OCTAVES};
use egui::{pos2, vec2, Align2, Context, Id, Pos2, Rect, Sense, Ui, Vec2};

use super::super::targets::{CURVES, PLATFORMS};

/// The label column's width at 100 %.
const LABEL_W: f32 = 120.0;
const ROW_H: f32 = 26.0;

pub fn window(app: &mut CantusApp, ctx: &Context) {
    let s = Style::current();
    let screen = ctx.content_rect();
    let size = Vec2::new(620.0 * s.k, 800.0 * s.k).min(screen.size() - Vec2::new(32.0, 80.0));
    let mut open = true;
    egui::Window::new("cantus-mastering")
        .id(Id::new("cantus-mastering"))
        .title_bar(false)
        .frame(egui::Frame::NONE)
        .resizable(true)
        .default_size(size)
        .min_size(Vec2::new(400.0, 300.0).min(size))
        .default_pos(Pos2::new(
            screen.right() - size.x - 24.0,
            screen.top() + 60.0,
        ))
        .constrain(true)
        .show(ctx, |ui| {
            let rect = ui.max_rect();
            ui.expand_to_include_rect(rect);
            ws::frame(ui, rect, &s);
            let header = Rect::from_min_size(rect.min, vec2(rect.width(), 30.0 * s.k));
            if ws::title(ui, header, &s, Glyph::Gear, "listen.panel.title") {
                open = false;
            }
            let body = Rect::from_min_max(
                pos2(rect.left() + 12.0 * s.k, header.bottom() + 8.0 * s.k),
                pos2(rect.right() - 6.0 * s.k, rect.bottom() - 10.0 * s.k),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            let mut area = egui::ScrollArea::vertical()
                .id_salt("cantus-mastering-body")
                .auto_shrink([false, false]);
            if let Some(y) = app.listen.panel.scroll.take() {
                area = area.vertical_scroll_offset(y);
            }
            area.show(&mut child, |ui| {
                ui.set_width(body.width() - 14.0 * s.k);
                ui.spacing_mut().item_spacing = vec2(8.0 * s.k, 6.0 * s.k);
                contents(app, ui, &s);
            });
        });
    list(app, ctx, &s);
    if !open {
        super::stop_ab(app);
        app.listen.panel.open = false;
        app.listen.panel.menu = None;
    }
}

/// A row: its label, then what `f` lays out beside it.
fn row(ui: &mut Ui, key: &str, f: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        w::row_label(ui, key, LABEL_W, ROW_H);
        f(ui);
    });
}

/// A menu field with ‹ › that steps through `n` choices; the field itself opens their list
/// under it (`menu`).
#[allow(clippy::too_many_arguments)]
fn stepper(
    ui: &mut Ui,
    value: &str,
    name: &str,
    i: &mut usize,
    n: usize,
    width: f32,
    pick: super::Pick,
    menu: &mut Option<(super::Pick, Pos2)>,
) {
    let open = menu.is_some_and(|m| m.0 == pick);
    let (resp, c) = kit::widgets::menu_field(ui, value, true, open, name, width);
    if c.prev {
        *i = (*i + n - 1) % n;
    } else if c.next {
        *i = (*i + 1) % n;
    } else if c.open {
        *menu = if open {
            None
        } else {
            Some((pick, resp.rect.left_bottom() + vec2(0.0, 2.0)))
        };
    }
}

/// The open list of platforms or balances, over everything; a click outside closes it.
fn list(app: &mut CantusApp, ctx: &Context, s: &Style) {
    let Some((pick, at)) = app.listen.panel.menu else {
        return;
    };
    let names: Vec<String> = match pick {
        super::Pick::Platform => PLATFORMS.iter().map(|p| p.name.to_string()).collect(),
        super::Pick::Curve => CURVES.iter().map(|c| t(c.name_key)).collect(),
    };
    let sel = match pick {
        super::Pick::Platform => app.listen.panel.platform,
        super::Pick::Curve => app.listen.panel.curve,
    };
    let items: Vec<kit::menu::Item<'_>> = names
        .iter()
        .enumerate()
        .map(|(i, n)| kit::menu::Item::Action {
            label: Src::Str(n),
            check: kit::menu::Check::Radio(i == sel),
            shortcut: None,
            submenu: false,
            enabled: true,
        })
        .collect();
    let mut chosen = None;
    let area = egui::Area::new(Id::new("cantus-mastering-list"))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .constrain(true)
        .show(ctx, |ui| {
            chosen = kit::widgets::menu_list(ui, &items, 220.0).1.clicked;
        });
    let _ = s;
    let p = &mut app.listen.panel;
    if let Some(i) = chosen {
        match pick {
            super::Pick::Platform => p.platform = i,
            super::Pick::Curve => p.curve = i,
        }
        p.menu = None;
    } else if area.response.clicked_elsewhere() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        p.menu = None;
    }
}

fn contents(app: &mut CantusApp, ui: &mut Ui, s: &Style) {
    let has_chain = !app.project().master.effects.is_empty();
    let has_loop = app.project().loop_region.is_some();
    let busy = app.listen.panel.running.is_some();
    w::band(ui, Src::Key("listen.panel.target"));
    {
        let p = &mut app.listen.panel;
        let name = PLATFORMS[p.platform].name;
        row(ui, "listen.panel.platform", |ui| {
            stepper(
                ui,
                name,
                &t("listen.panel.platform"),
                &mut p.platform,
                PLATFORMS.len(),
                220.0,
                super::Pick::Platform,
                &mut p.menu,
            );
        });
        let pl = &PLATFORMS[p.platform];
        let numbers = tf(
            "listen.panel.platform_numbers",
            &[
                ("lufs", &format!("{:.0}", pl.lufs)),
                ("ceiling", &format!("{:.1}", pl.ceiling_dbtp)),
            ],
        );
        row(ui, "listen.panel.aim", |ui| note(ui, &numbers, s, false));
    }
    let reference = app.listen.panel.reference.as_ref().map(|(p, _)| {
        p.file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
    });
    let mut load = false;
    let mut clear = false;
    {
        let p = &mut app.listen.panel;
        row(ui, "listen.panel.balance", |ui| match &reference {
            Some(file) => {
                note(
                    ui,
                    &tf("listen.panel.reference_is", &[("file", file)]),
                    s,
                    false,
                );
                clear = w::button(ui, Src::Key("listen.panel.clear"), 0.0, !busy).clicked();
            }
            None => {
                let words = t(CURVES[p.curve].name_key);
                stepper(
                    ui,
                    &words,
                    &t("listen.panel.balance"),
                    &mut p.curve,
                    CURVES.len(),
                    160.0,
                    super::Pick::Curve,
                    &mut p.menu,
                );
                load = w::button(ui, Src::Key("listen.panel.load"), 0.0, !busy).clicked();
            }
        });
        row(ui, "listen.panel.range", |ui| {
            let labels = [
                Src::Key("listen.panel.whole"),
                Src::Key("listen.panel.loop"),
            ];
            let mut sel = usize::from(p.loop_only && has_loop);
            if kit::widgets::segmented(ui, &mut sel, &labels, &t("listen.panel.range"), 220.0)
                .changed()
            {
                p.loop_only = sel == 1;
            }
        });
        if has_chain {
            row(ui, "listen.panel.chain_now", |ui| {
                let labels = [
                    Src::Key("listen.panel.replace"),
                    Src::Key("listen.panel.append"),
                ];
                let mut sel = usize::from(p.append);
                if kit::widgets::segmented(
                    ui,
                    &mut sel,
                    &labels,
                    &t("listen.panel.chain_now"),
                    260.0,
                )
                .changed()
                {
                    p.append = sel == 1;
                }
            });
        }
    }
    if load {
        load_reference(app);
    }
    if clear {
        app.listen.panel.reference = None;
    }
    ui.add_space(4.0 * s.k);
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W * s.k + 8.0 * s.k);
        match &app.listen.panel.running {
            Some(r) => {
                let f = f32::from_bits(r.progress.load(std::sync::atomic::Ordering::Relaxed));
                note(
                    ui,
                    &tf(
                        "listen.panel.listening",
                        &[("percent", &format!("{:.0}", f * 100.0))],
                    ),
                    s,
                    false,
                );
                if w::button(ui, Src::Key("listen.panel.stop"), 0.0, true).clicked() {
                    r.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
            None => {
                if w::button(ui, Src::Key("listen.panel.listen"), 200.0, true).clicked() {
                    super::start(app);
                }
            }
        }
    });
    if let Some(e) = app.listen.panel.error.clone() {
        let width = ui.available_width();
        ws::wrapped(ui, &e, Role::Label, s.t.danger, width);
    }
    if app.listen.panel.done.is_some() {
        heard(app, ui, s);
    }
}

fn note(ui: &mut Ui, words: &str, s: &Style, warn: bool) {
    let color = if warn { s.t.danger } else { s.t.text_dim };
    let tx = Text::new(words, Role::Body, color).align(Align2::LEFT_CENTER);
    let width = ui
        .available_width()
        .min(text::measure(ui.ctx(), &tx, s.tk()).x + 4.0 * s.k);
    let (rect, _) = ui.allocate_exact_size(vec2(width, ROW_H * s.k), Sense::hover());
    with_brush(ui.painter(), |b| {
        let _ = text::paint(b, rect, &tx, s.tk());
    });
}

fn load_reference(app: &mut CantusApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter(
            t("listen.panel.audio_files"),
            &["wav", "flac", "mp3", "ogg", "aif", "aiff"],
        )
        .pick_file()
    else {
        return;
    };
    match cantus_engine::samples::assets::decode_any(&path) {
        Ok(data) => {
            let mut ear = cantus_dsp::speculum::ear::Ear::new(data.rate as f32, 0.0);
            if data.channels >= 2 {
                let (l, r): (Vec<f32>, Vec<f32>) =
                    data.frames.chunks_exact(2).map(|f| (f[0], f[1])).unzip();
                ear.push(&l, &r);
            } else {
                ear.push(&data.frames, &data.frames);
            }
            app.listen.panel.reference = Some((path, ear.whole()));
        }
        Err(e) => app.listen.panel.error = Some(e),
    }
}

/// A level as the table shows it.
fn num(x: Option<f64>, unit: &str) -> String {
    match x {
        Some(v) if v.is_finite() => format!("{v:.1} {unit}"),
        _ => "—".into(),
    }
}

/// The table's rows: label key, then mix, master, target.
fn table_rows(d: &Done) -> Vec<(&'static str, String, String, String)> {
    let (a, b): (&Reading, &Reading) = (&d.proposal.before, &d.proposal.predicted);
    let tg = &d.proposal.target;
    let f = |x: f32| Some(x as f64);
    vec![
        (
            "listen.panel.integrated",
            num(a.integrated_lufs, "LUFS"),
            num(b.integrated_lufs, "LUFS"),
            num(Some(tg.platform.lufs), "LUFS"),
        ),
        (
            "listen.panel.true_peak",
            num(f(a.true_peak_dbtp), "dBTP"),
            num(f(b.true_peak_dbtp), "dBTP"),
            format!("≤ {:.1} dBTP", tg.platform.ceiling_dbtp),
        ),
        (
            "listen.panel.lra",
            num(a.range_lu, "LU"),
            num(b.range_lu, "LU"),
            format!("{:.0}–{:.0} LU", tg.lra.0, tg.lra.1),
        ),
        (
            "listen.panel.short_term",
            num(a.short_term_max_lufs, "LUFS"),
            num(b.short_term_max_lufs, "LUFS"),
            String::new(),
        ),
        (
            "listen.panel.width",
            num(f(a.width_db), "dB"),
            num(f(b.width_db), "dB"),
            num(f(tg.width_db), "dB"),
        ),
        (
            "listen.panel.mono",
            num(a.mono_loss_lu, "LU"),
            num(b.mono_loss_lu, "LU"),
            String::new(),
        ),
        (
            "listen.panel.correlation",
            a.correlation_min.map_or("—".into(), |c| format!("{c:.2}")),
            b.correlation_min.map_or("—".into(), |c| format!("{c:.2}")),
            String::new(),
        ),
    ]
}

fn heard(app: &mut CantusApp, ui: &mut Ui, s: &Style) {
    let k = s.k;
    w::band(ui, Src::Key("listen.panel.heard"));
    let Some(d) = app.listen.panel.done.as_ref() else {
        return;
    };
    // The numbers: a label column, then mix | proposed master | target.
    let rows = table_rows(d);
    let width = ui.available_width();
    let h = ROW_H * k * (rows.len() + 1) as f32;
    let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
    let col = (width - LABEL_W * k) / 3.0;
    with_brush(ui.painter(), |b| {
        b.rrect(rect, s.r(4.0), Fill::Solid(s.t.strip));
        let cell = |i: usize, c: usize| {
            let x = if c == 0 {
                rect.left()
            } else {
                rect.left() + LABEL_W * k + col * (c - 1) as f32
            };
            let wdt = if c == 0 { LABEL_W * k } else { col };
            Rect::from_min_size(
                pos2(x, rect.top() + ROW_H * k * i as f32),
                vec2(wdt, ROW_H * k),
            )
            .shrink2(vec2(8.0 * k, 0.0))
        };
        for (c, key) in [
            "",
            "listen.panel.mix",
            "listen.panel.master",
            "listen.panel.goal",
        ]
        .iter()
        .enumerate()
        {
            if !key.is_empty() {
                let _ = text::paint(
                    b,
                    cell(0, c),
                    &Text::key(key, Role::Label, s.t.label).align(Align2::RIGHT_CENTER),
                    s.tk(),
                );
            }
        }
        for (i, (key, a, m, g)) in rows.iter().enumerate() {
            let r = i + 1;
            if r % 2 == 0 {
                b.rect(
                    Rect::from_min_size(
                        pos2(rect.left(), rect.top() + ROW_H * k * r as f32),
                        vec2(width, ROW_H * k),
                    ),
                    Fill::Solid(kit::color::with_alpha(s.t.text, 0.04)),
                );
            }
            let _ = text::paint(
                b,
                cell(r, 0),
                &Text::key(key, Role::Body, s.t.text_dim).align(Align2::LEFT_CENTER),
                s.tk(),
            );
            for (c, v, ink) in [(1, a, s.t.text_dim), (2, m, s.t.text), (3, g, s.t.text_dim)] {
                let _ = text::paint(
                    b,
                    cell(r, c),
                    &Text::new(v, Role::Value, ink).align(Align2::RIGHT_CENTER),
                    s.tk(),
                );
            }
        }
    });
    // The octave balance: the mix's bars, the proposed master's line, the target's dots.
    ui.add_space(4.0 * k);
    let (chart, _) = ui.allocate_exact_size(vec2(width, 150.0 * k), Sense::hover());
    spectrum(ui, chart, d, s);
    // The chain and why.
    w::band(ui, Src::Key("listen.panel.chain"));
    let lines: Vec<(String, String)> = d
        .proposal
        .why
        .iter()
        .map(|why| {
            let args: Vec<(&str, &dyn std::fmt::Display)> = why
                .args
                .iter()
                .map(|(k, v)| (*k, v as &dyn std::fmt::Display))
                .collect();
            (t(why.kind.name_key()), tf(why.key, &args))
        })
        .collect();
    for (unit, why) in lines {
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(LABEL_W * k, ROW_H * k), Sense::hover());
            with_brush(ui.painter(), |b| {
                let _ = text::paint(
                    b,
                    r,
                    &Text::new(&unit, Role::Body, s.t.text).align(Align2::RIGHT_CENTER),
                    s.tk(),
                );
            });
            // Its own column, so the lines it breaks into stack under each other.
            let width = ui.available_width();
            ui.vertical(|ui| ws::wrapped(ui, &why, Role::Body, s.t.text_dim, width));
        });
    }
    let replace = d.replace;
    let d_short = d.proposal.short_of_target_lu;
    let source = format!(
        "{} ({})",
        d.proposal.target.platform.source, d.proposal.target.platform.checked
    );
    ui.add_space(6.0 * k);
    let playing = app.listen.panel.ab.map(|(side, _)| side);
    // Replacing says what it takes out (review 1): a count on the key, the names in full
    // under the row (review 2: six names overflowed the key).
    let removed = super::super::run::removed_names(app.project(), replace);
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W * k + 8.0 * k);
        let words = if !removed.is_empty() {
            crate::i18n::tf_count("listen.panel.apply_replace", removed.len(), &[])
        } else if replace {
            t("listen.panel.apply")
        } else {
            t("listen.panel.apply_append")
        };
        if w::button(ui, Src::Str(&words), 0.0, true).clicked() {
            super::apply(app);
        }
        let labels = [
            Src::Key("listen.panel.ab_off"),
            Src::Key("listen.panel.ab_mix"),
            Src::Key("listen.panel.ab_master"),
        ];
        let mut sel = match playing {
            None => 0,
            Some(Side::Mix) => 1,
            Some(Side::Master) => 2,
        };
        if kit::widgets::segmented(ui, &mut sel, &labels, &t("listen.panel.ab"), 240.0).changed() {
            match sel {
                1 => super::play_ab(app, Side::Mix),
                2 => super::play_ab(app, Side::Master),
                _ => super::stop_ab(app),
            }
        }
    });
    let width = ui.available_width();
    if let Some(lu) = d_short {
        ws::wrapped(
            ui,
            &tf("listen.panel.short", &[("lu", &format!("{lu:.1}"))]),
            Role::Label,
            s.t.danger,
            width,
        );
    }
    if !removed.is_empty() {
        ws::wrapped(
            ui,
            &tf("listen.panel.removes", &[("effects", &removed.join(", "))]),
            Role::Label,
            s.t.text_dim,
            width,
        );
    }
    if !super::super::run::post_moves(app.project()).is_empty() {
        ws::wrapped(
            ui,
            &t("listen.panel.post_moves"),
            Role::Label,
            s.t.danger,
            width,
        );
    }
    ws::wrapped(
        ui,
        &t("listen.panel.ab_note"),
        Role::Label,
        s.t.text_dim,
        width,
    );
    ws::wrapped(
        ui,
        &tf("listen.panel.source", &[("source", &source)]),
        Role::Label,
        s.t.text_dim,
        width,
    );
}

/// The octave chart (see [`heard`]): every value relative to its own mean over 63 Hz …
/// 8 kHz, ±12 dB.
fn spectrum(ui: &Ui, rect: Rect, d: &Done, s: &Style) {
    let k = s.k;
    let shape = |o: &[f32; OCTAVES]| {
        let mean = (1..=8).map(|i| o[i]).filter(|x| x.is_finite()).sum::<f32>() / 8.0;
        std::array::from_fn::<f32, OCTAVES, _>(
            |i| if o[i].is_finite() { o[i] - mean } else { -40.0 },
        )
    };
    let mix = shape(&d.proposal.before.octaves_db);
    let master = shape(&d.proposal.predicted.octaves_db);
    let target = shape(&d.proposal.target.curve);
    let plot = Rect::from_min_max(
        pos2(rect.left() + 40.0 * k, rect.top() + 8.0 * k),
        pos2(rect.right() - 8.0 * k, rect.bottom() - 22.0 * k),
    );
    let y = |db: f32| plot.center().y - db.clamp(-12.0, 12.0) / 12.0 * plot.height() * 0.5;
    let colw = plot.width() / OCTAVES as f32;
    let x = |o: usize| plot.left() + colw * (o as f32 + 0.5);
    with_brush(ui.painter(), |b| {
        b.rrect(rect, s.r(4.0), Fill::Solid(s.t.strip));
        for db in [-12.0f32, -6.0, 0.0, 6.0, 12.0] {
            let c = if db == 0.0 { s.t.grid_strong } else { s.t.grid };
            b.hline(plot.left(), plot.right(), y(db), b.px(), c);
            let lr = Rect::from_min_max(
                pos2(rect.left(), y(db) - 8.0 * k),
                pos2(plot.left() - 4.0 * k, y(db) + 8.0 * k),
            );
            let _ = text::paint(
                b,
                lr,
                &Text::new(&format!("{db:+.0}"), Role::Micro, s.t.text_dim)
                    .align(Align2::RIGHT_CENTER),
                s.tk(),
            );
        }
        #[allow(clippy::needless_range_loop)] // `o` is also the band's place
        for o in 0..OCTAVES {
            let bar = Rect::from_min_max(
                pos2(x(o) - colw * 0.3, y(mix[o].max(0.0)).min(y(0.0))),
                pos2(x(o) + colw * 0.3, y(mix[o].min(0.0)).max(y(0.0))),
            );
            b.rect(bar, Fill::Solid(kit::color::with_alpha(s.t.text_dim, 0.45)));
            let words = cantus_dsp::speculum::ear::OCTAVE_NAMES[o];
            let lr =
                Rect::from_center_size(pos2(x(o), plot.bottom() + 11.0 * k), vec2(colw, 16.0 * k));
            let _ = text::paint(b, lr, &Text::new(words, Role::Micro, s.t.text_dim), s.tk());
        }
        let pts: Vec<Pos2> = (0..OCTAVES).map(|o| pos2(x(o), y(master[o]))).collect();
        b.polyline(&pts, 2.0 * k, s.t.brand_bright);
        for (o, v) in target.iter().enumerate() {
            b.circle_stroke(pos2(x(o), y(*v)), 3.5 * k, 1.5 * k, s.t.highlight);
        }
    });
    // The legend, under the title row of the chart.
    let legend = Rect::from_min_size(
        pos2(plot.left() + 6.0 * k, plot.top()),
        vec2(plot.width(), 14.0 * k),
    );
    with_brush(ui.painter(), |b| {
        let words = t("listen.panel.legend");
        let _ = text::paint(
            b,
            legend,
            &Text::new(&words, Role::Micro, s.t.text_dim).align(Align2::LEFT_CENTER),
            s.tk(),
        );
    });
}
