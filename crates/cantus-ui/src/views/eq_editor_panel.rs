//! The panels under the display: the selected band's controls (and, for a dynamic band,
//! its last seconds against its threshold), or EQ match while it is open.

use super::*;

fn used(slot: &EffectSlot, band: usize) -> bool {
    slot.spec.get(&eq_key(band, "used")) >= 0.5
}

/// A knob bound to one field of one band; a drag is one undo step.
#[allow(clippy::too_many_arguments)]
fn knob(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    band: usize,
    field: &'static str,
    value: f32,
) -> egui::Response {
    let Some(&(_, label, min, max, default, unit, log, _)) =
        EQ_BAND_FIELDS.iter().find(|f| f.0 == field)
    else {
        return ui.label(field);
    };
    let mut v = value;
    let slope = field == "slope";
    let r = ui.add(
        Knob::new(&mut v, min..=max, default)
            .diameter(space::GRID * 11.0)
            .log(log)
            .unit(unit)
            .label(t(label))
            .format(move |x| match unit {
                "dB/oct" => format!("{:.0} dB/oct", (x / 6.0).round() * 6.0),
                ":1" => format!("{x:.1}:1"),
                "" => format!("{x:.2}"),
                u => crate::widgets::knob::readout(
                    x,
                    if u == "Hz" {
                        "Hz"
                    } else if u == "ms" {
                        "ms"
                    } else {
                        "dB"
                    },
                ),
            }),
    );
    if r.changed() {
        let v = if slope { (v / 6.0).round() * 6.0 } else { v };
        app.set_band(strip, effect, band, &[(field, v)], true);
    }
    if r.drag_stopped() || r.double_clicked() {
        app.end_gesture();
    }
    r
}

/// Two or more choices side by side, the chosen one filled.
fn segmented(ui: &mut egui::Ui, current: usize, labels: &[String], tip: &str) -> Option<usize> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (i, l) in labels.iter().enumerate() {
            if chip(ui, current == i, l).on_hover_text(tip).clicked() && current != i {
                picked = Some(i);
            }
        }
    });
    picked
}

/// The selected band's controls, always visible: shape and placement, the switches, knobs
/// in real units, then the dynamics when the band has them.
pub(super) fn band_panel(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    bands: &[BandSettings],
    tel: &[f32; 64],
) {
    let pal = theme::p();
    let Some(b) = app.eq_editor.primary.filter(|b| used(slot, *b)) else {
        ui.label(
            RichText::new(t("eq.hint"))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        return;
    };
    let band = bands[b];
    let spec = &slot.spec;
    let others = app.eq_editor.selection.len().saturating_sub(1);
    let soloed = spec.get("solo").round() as usize == b + 1;
    crate::design::Elevation::Card.frame(&pal).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            let (chip, _) = ui.allocate_exact_size(vec2(space::S3, space::S3), Sense::hover());
            ui.painter()
                .circle_filled(chip.center(), chip.width() * 0.5, band_color(b));
            ui.label(
                RichText::new(tf("eq.band_n", &[("n", &(b + 1))])).text_style(design::strong()),
            );
            if others > 0 {
                ui.label(
                    RichText::new(tf("eq.more_selected", &[("n", &others)]))
                        .text_style(design::caption())
                        .color(pal.text_dim),
                );
            }
            egui::ComboBox::from_id_salt(("eq-shape", effect.0, b))
                .selected_text(t(&format!("eq.shape.{}", band.shape.index())))
                .show_ui(ui, |ui| {
                    for s in EqShape::ALL {
                        if ui
                            .selectable_label(
                                band.shape == s,
                                t(&format!("eq.shape.{}", s.index())),
                            )
                            .clicked()
                        {
                            app.set_band(strip, effect, b, &[("shape", s.index() as f32)], false);
                        }
                    }
                });
            egui::ComboBox::from_id_salt(("eq-place", effect.0, b))
                .selected_text(t(&format!("eq.place.{}", band.placement.index())))
                .show_ui(ui, |ui| {
                    for (i, pl) in Placement::ALL.iter().enumerate() {
                        if ui
                            .selectable_label(band.placement == *pl, t(&format!("eq.place.{i}")))
                            .clicked()
                        {
                            app.set_band(strip, effect, b, &[("place", i as f32)], false);
                        }
                    }
                });
            let mut on = spec.get(&eq_key(b, "on")) >= 0.5;
            if ui.checkbox(&mut on, t("fx.eq.on")).changed() {
                app.set_band(
                    strip,
                    effect,
                    b,
                    &[("on", if on { 1.0 } else { 0.0 })],
                    false,
                );
            }
            let mut solo = soloed;
            if toggle(ui, &mut solo, t("eq.solo"))
                .on_hover_text(t("eq.solo_tip"))
                .changed()
            {
                app.set_eq_solo(strip, effect, solo.then_some(b));
            }
            if band.shape.has_gain() {
                let mut dynamic = band.dynamic;
                if ui
                    .checkbox(&mut dynamic, t("fx.eq.dyn"))
                    .on_hover_text(t("eq.dyn_tip"))
                    .changed()
                {
                    app.set_band(
                        strip,
                        effect,
                        b,
                        &[("dyn", if dynamic { 1.0 } else { 0.0 })],
                        false,
                    );
                }
            }
            if band.shape == EqShape::Bell {
                let mut gq = spec.get(&eq_key(b, "gq")) >= 0.5;
                if ui
                    .checkbox(&mut gq, t("fx.eq.gq"))
                    .on_hover_text(t("eq.gq_tip"))
                    .changed()
                {
                    app.set_band(
                        strip,
                        effect,
                        b,
                        &[("gq", if gq { 1.0 } else { 0.0 })],
                        false,
                    );
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(t("eq.remove_band")).clicked() {
                    app.remove_eq_band(strip, effect, b);
                }
                if ui
                    .button(t("eq.split_ms"))
                    .on_hover_text(t("eq.split_tip"))
                    .clicked()
                {
                    app.split_eq_band(strip, effect, b, true);
                }
                if ui
                    .button(t("eq.split_lr"))
                    .on_hover_text(t("eq.split_tip"))
                    .clicked()
                {
                    app.split_eq_band(strip, effect, b, false);
                }
            });
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = space::S1;
            knob(
                app,
                ui,
                strip,
                effect,
                b,
                "freq",
                spec.get(&eq_key(b, "freq")),
            );
            if band.shape.has_gain() {
                knob(
                    app,
                    ui,
                    strip,
                    effect,
                    b,
                    "gain",
                    spec.get(&eq_key(b, "gain")),
                );
            }
            knob(app, ui, strip, effect, b, "q", spec.get(&eq_key(b, "q")));
            if band.shape.has_slope() {
                knob(app, ui, strip, effect, b, "slope", band.slope);
            }
            if band.dynamic && band.shape.has_gain() {
                ui.separator();
                dynamics(app, ui, strip, effect, slot, b, &band, tel);
            }
        });
    });
}

/// A dynamic band's controls: which way it moves, what it listens to, its five numbers,
/// and its last seconds.
#[allow(clippy::too_many_arguments)]
fn dynamics(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    b: usize,
    band: &BandSettings,
    tel: &[f32; 64],
) {
    let pal = theme::p();
    let spec = &slot.spec;
    ui.vertical(|ui| {
        ui.label(
            RichText::new(t("eq.dyn_when"))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        let dir = [t("eq.dir.above"), t("eq.dir.below")];
        if let Some(i) = segmented(ui, band.below as usize, &dir, &t("eq.dir_tip")) {
            app.set_band(strip, effect, b, &[("dir", i as f32)], false);
        }
        ui.label(
            RichText::new(t("eq.dyn_hears"))
                .text_style(design::caption())
                .color(pal.text_dim),
        );
        let src = [t("eq.src.band"), t("eq.src.key")];
        if let Some(i) = segmented(ui, band.external as usize, &src, &t("eq.src_tip")) {
            app.set_band(strip, effect, b, &[("key", i as f32)], false);
        }
        if band.external && slot.sidechain.is_none() {
            ui.label(
                RichText::new(t("eq.src.no_key"))
                    .text_style(design::caption())
                    .color(pal.danger),
            );
        }
    });
    for field in ["thr", "range", "ratio", "att", "rel"] {
        knob(
            app,
            ui,
            strip,
            effect,
            b,
            field,
            spec.get(&eq_key(b, field)),
        );
    }
    history(app, ui, strip, effect, b, band, tel);
}

/// The band's last seconds: its detector's level (a fill), its threshold (a dashed line
/// you drag), and the gain it moved by (a line in its colour, on the range's scale).
#[allow(clippy::too_many_arguments)]
fn history(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    b: usize,
    band: &BandSettings,
    tel: &[f32; 64],
) {
    const LO: f32 = -72.0;
    let pal = theme::p();
    let w = ui
        .available_width()
        .clamp(space::GRID * 40.0, space::GRID * 90.0);
    let h = space::GRID * 19.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click_and_drag());
    let y = |d: f32| rect.bottom() - ((d - LO) / -LO).clamp(0.0, 1.0) * rect.height();
    let d_of = |py: f32| LO + (rect.bottom() - py) / rect.height() * -LO;
    let range = band.range_db.abs().max(1.0);
    let gy = |g: f32| rect.center().y - (g / range).clamp(-1.0, 1.0) * rect.height() * 0.45;
    let p = ui.painter_at(rect);
    p.rect_filled(rect, pal.corner_radius, theme::recess(&pal));
    let hist = &app.eq_editor.history;
    let dx = rect.width() / HISTORY as f32;
    let x0 = rect.right() - dx * hist.len() as f32;
    let mut fill = Vec::with_capacity(hist.len());
    let mut moved = Vec::with_capacity(hist.len());
    for (i, (d, l)) in hist.iter().enumerate() {
        let x = x0 + dx * i as f32;
        fill.push(pos2(x, y(l[b])));
        moved.push(pos2(x, gy(d[b])));
    }
    super::display::fill_to(
        &p,
        &fill,
        |_| rect.bottom(),
        pal.text_dim.gamma_multiply(0.3),
    );
    p.hline(rect.x_range(), gy(0.0), Stroke::new(1.0, pal.grid_bar));
    if moved.len() > 1 {
        p.add(egui::Shape::line(moved, Stroke::new(2.0, band_color(b))));
    }
    let ty = y(band.threshold_db);
    let near = resp.hover_pos().is_some_and(|pt| (pt.y - ty).abs() < 6.0);
    p.add(egui::Shape::dashed_line(
        &[pos2(rect.left(), ty), pos2(rect.right(), ty)],
        Stroke::new(if near { 2.0 } else { 1.25 }, pal.gold),
        6.0,
        4.0,
    ));
    let level = tel[aequalis::telemetry::LEVEL + b];
    let now = hist.back().map(|h| h.0[b]).unwrap_or(0.0);
    p.text(
        pos2(rect.left() + space::S2, rect.top() + space::S1),
        Align2::LEFT_TOP,
        format!(
            "{}  {:+.1} dB   {} {:.0} dB",
            t("eq.dyn_now"),
            now,
            t("eq.dyn_level"),
            if level > -199.0 { level } else { LO }
        ),
        design::paint::ruler(),
        pal.text,
    );
    p.text(
        pos2(rect.right() - space::S2, ty - 2.0),
        Align2::RIGHT_BOTTOM,
        format!("{:+.1} dB", band.threshold_db),
        design::paint::ruler(),
        pal.gold,
    );
    p.rect_stroke(
        rect,
        pal.corner_radius,
        Stroke::new(1.0, pal.grid_strong),
        StrokeKind::Inside,
    );
    if near || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if resp.dragged() {
        if let Some(pt) = resp.interact_pointer_pos() {
            app.set_band(
                strip,
                effect,
                b,
                &[("thr", d_of(pt.y).clamp(-80.0, 0.0))],
                true,
            );
        }
    }
    if resp.drag_stopped() {
        app.end_gesture();
    }
    resp.on_hover_text(t("eq.history_tip"));
}

/// EQ match: listen to this strip and the key strip, then fit the difference into bands —
/// with an amount, a band budget and limits, and the residual stated.
pub(super) fn match_panel(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    strip: StripId,
    effect: EffectId,
    slot: &EffectSlot,
    tel: &[f32; 64],
) {
    let pal = theme::p();
    let sr = app.eq_sample_rate();
    let key_name = slot
        .sidechain
        .map(|k| app.capture_label(cantus_engine::CaptureTarget::Strip(k)));
    let key_on = tel[aequalis::telemetry::KEY_ON] >= 0.5;
    // Refit when the options change, and every half second of listening.
    if let Some(m) = app.eq_editor.matching.as_mut() {
        let stamp = m.frames / 30 * 1_000
            + m.budget * 7
            + (m.amount * 100.0) as usize * 131
            + (m.max_boost * 10.0) as usize * 17
            + (m.max_cut * 10.0) as usize * 19;
        if stamp != m.fitted_at {
            m.fitted_at = stamp;
            m.fit = m
                .target()
                .map(|target| aequalis::fit::fit(&target, &m.options(), sr));
        }
    }
    let mut apply = None;
    let mut close = false;
    crate::design::Elevation::Card.frame(&pal).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let Some(m) = app.eq_editor.matching.as_mut() else {
            return;
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(t("eq.match")).text_style(design::strong()));
            match &key_name {
                Some(name) => {
                    ui.label(
                        RichText::new(tf("eq.match_to", &[("strip", name)])).color(pal.text_dim),
                    );
                }
                None => {
                    ui.label(RichText::new(t("eq.match_no_key")).color(pal.danger));
                }
            }
            let secs = m.frames as f32 / 60.0;
            let listening = if m.listening {
                t("eq.match_listening")
            } else {
                t("eq.match_paused")
            };
            if chip(ui, m.listening, listening)
                .on_hover_text(t("eq.match_listen_tip"))
                .clicked()
            {
                m.listening = !m.listening;
            }
            ui.label(
                RichText::new(tf("eq.match_heard", &[("s", &format!("{secs:.1}"))]))
                    .text_style(design::caption())
                    .color(if key_on { pal.text } else { pal.text_dim }),
            );
            if ui.button(t("eq.match_reset")).clicked() {
                let keep = MatchState {
                    amount: m.amount,
                    budget: m.budget,
                    max_boost: m.max_boost,
                    max_cut: m.max_cut,
                    ..MatchState::default()
                };
                *m = keep;
            }
        });
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = space::S1;
            let mut amount = m.amount;
            if ui
                .add(
                    Knob::new(&mut amount, 0.0..=1.0, 1.0)
                        .diameter(space::GRID * 11.0)
                        .unit("%")
                        .label(t("eq.match_amount")),
                )
                .changed()
            {
                m.amount = amount;
            }
            let mut budget = m.budget as f32;
            if ui
                .add(
                    Knob::new(&mut budget, 1.0..=12.0, 6.0)
                        .diameter(space::GRID * 11.0)
                        .stepped(true)
                        .label(t("eq.match_bands"))
                        .format(|x| format!("{x:.0}")),
                )
                .changed()
            {
                m.budget = budget.round() as usize;
            }
            let mut boost = m.max_boost;
            if ui
                .add(
                    Knob::new(&mut boost, 0.0..=18.0, 9.0)
                        .diameter(space::GRID * 11.0)
                        .unit("dB")
                        .label(t("eq.match_boost")),
                )
                .changed()
            {
                m.max_boost = boost;
            }
            let mut cut = m.max_cut;
            if ui
                .add(
                    Knob::new(&mut cut, 0.0..=24.0, 12.0)
                        .diameter(space::GRID * 11.0)
                        .unit("dB")
                        .label(t("eq.match_cut"))
                        .format(|x| format!("-{x:.1} dB")),
                )
                .changed()
            {
                m.max_cut = cut;
            }
            ui.separator();
            ui.vertical(|ui| {
                match &m.fit {
                    Some(fit) => {
                        ui.label(
                            RichText::new(tf(
                                "eq.match_result",
                                &[
                                    ("n", &fit.bands.len()),
                                    ("rms", &format!("{:.1}", fit.rms_db)),
                                    ("max", &format!("{:.1}", fit.max_db)),
                                ],
                            ))
                            .text_style(design::mono_small()),
                        );
                    }
                    None => {
                        ui.label(
                            RichText::new(t("eq.match_wait"))
                                .text_style(design::caption())
                                .color(pal.text_dim),
                        );
                    }
                }
                ui.label(
                    RichText::new(t("eq.match_hint"))
                        .text_style(design::caption())
                        .color(pal.text_dim),
                );
                ui.horizontal(|ui| {
                    let ready = m.fit.as_ref().is_some_and(|f| !f.bands.is_empty());
                    if ui
                        .add_enabled(ready, egui::Button::new(t("eq.match_apply")))
                        .clicked()
                    {
                        apply = m.fit.clone();
                    }
                    if ui.button(t("layout.cancel")).clicked() {
                        close = true;
                    }
                });
            });
        });
    });
    if let Some(fit) = apply {
        app.apply_eq_fit(strip, effect, &fit);
        close = true;
    }
    if close {
        app.eq_editor.matching = None;
    }
}
