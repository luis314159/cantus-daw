//! Chorda's scene in the Play view (round 37, ADR 0104): the card that holds it, its two
//! pickers (the piano and its finish — the look, never the sound), the line that says what
//! it shows for the screen reader, and the tuning's progress while one is designed.

use super::scene::{self, Inputs};
use crate::app::CantusApp;
use crate::i18n::t;
use crate::kit::text::{self as ktext, Policy, Role, Src, Text};
use crate::kit::{well, with_brush, Function, State as KitState, Style};
use crate::views::fx::controls;
use crate::views::fx::frame::radio;
use crate::views::inst::organ::parts::{paint_card, Card};
use cantus_core::inst::chorda_look::{self as look, Body, Finish};
use cantus_core::{NativeSpec, TrackId};
use egui::{Align2, Pos2, Rect, Sense, Vec2};

/// The pickers' widths (points at 100 %).
const BODY_W: f32 = 178.0;
const FINISH_W: f32 = 132.0;

/// The card's words, made again only when what they say changes (the look and controls,
/// the language): a frame at rest makes none.
#[derive(Debug, Default)]
pub struct Words {
    key: Option<(Inputs, crate::i18n::Locale)>,
    said: String,
    hint: String,
    body: String,
    body_name: String,
    finish: String,
    finish_name: String,
}

impl Words {
    fn update(&mut self, inputs: &Inputs) {
        let key = (*inputs, crate::i18n::locale());
        if self.key == Some(key) {
            return;
        }
        self.key = Some(key);
        self.said = scene::caption(inputs);
        self.hint = t("chorda.look.hint");
        self.body = t(inputs.look.body.label_key());
        self.body_name = t("chorda.look.body");
        self.finish = t(inputs.look.finish.label_key());
        self.finish_name = t("chorda.look.finish");
    }
}

/// The scene's card in `rect`: the title, the scene filling the display, the pickers over
/// its top-left corner.
pub fn scene_card(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    rect: Rect,
    s: &Style,
    busy: Option<&str>,
) {
    let f = Function::Space;
    let sf = s.function(f);
    let display = (rect.height() / s.k - 24.0 - 1.0 - 6.0).max(40.0);
    let cr = paint_card(ui, rect, s, &Card::new("chorda.scene", f).display(display));
    let Some(d) = cr.display else {
        return;
    };
    let inputs = Inputs::of(spec, s.t.dark);
    // This Chorda's own scene: another's piano is never shown on it, nor glided from.
    let (scene, words) = app.inst.chorda.scenes.entry(track.0).or_default();
    words.update(&inputs);
    let now = ui.input(|i| i.time);
    let painter = ui.painter_at(d);
    // A window's sizing pass (its first frame back, unseen) lays out at a passing size:
    // it asks for no bake and moves no glide.
    if !ui.is_sizing_pass() && scene.paint(&painter, d, inputs, now) {
        ui.ctx().request_repaint();
    }
    let sd = crate::views::inst::face::on_display(&sf);
    with_brush(&painter, |b| crate::views::inst::face::cover(b, d, &sd));
    let resp = ui.interact(d, ui.id().with("chorda-scene"), Sense::hover());
    let words = &app.inst.chorda.scenes[&track.0].1;
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, &words.said));
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &words.said,
            &words.hint,
            crate::kit::hint::Caps::NONE,
        );
    }
    pickers(app, ui, track, spec, d, &sf);
    if let Some(busy) = busy {
        chip(ui, d, &sf, busy);
    }
}

/// The two pickers at the display's top-left.
fn pickers(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    d: Rect,
    s: &Style,
) {
    let shown = look::of(spec);
    let pad = s.s(8.0);
    let row = Rect::from_min_size(
        Pos2::new(d.left() + pad, d.top() + pad),
        Vec2::new(s.s(BODY_W + FINISH_W + 6.0), s.s(controls::ROW)),
    );
    if row.right() > d.right() - pad || row.bottom() > d.bottom() - pad {
        return;
    }
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(row)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.spacing_mut().item_spacing.x = s.s(6.0);
    let id = egui::Id::new(("chorda-look-body", track.0));
    let w = &app.inst.chorda.scenes[&track.0].1;
    let at = controls::field(&mut child, s, id, &w.body, &w.body_name, BODY_W);
    if let Some(at) = at {
        let items: [_; 4] = std::array::from_fn(|i| {
            radio(
                Src::Key(Body::ALL[i].label_key()),
                Body::ALL[i] == shown.body,
            )
        });
        if let Some(i) = controls::list(&mut child, s, id, at, BODY_W + 40.0, &items) {
            set(app, track, look::BODY_KEY, Body::ALL[i].key());
        }
    }
    let id = egui::Id::new(("chorda-look-finish", track.0));
    let w = &app.inst.chorda.scenes[&track.0].1;
    let at = controls::field(&mut child, s, id, &w.finish, &w.finish_name, FINISH_W);
    if let Some(at) = at {
        let items: [_; 4] = std::array::from_fn(|i| {
            radio(
                Src::Key(Finish::ALL[i].label_key()),
                Finish::ALL[i] == shown.finish,
            )
        });
        if let Some(i) = controls::list(&mut child, s, id, at, FINISH_W + 40.0, &items) {
            set(app, track, look::FINISH_KEY, Finish::ALL[i].key());
        }
    }
}

/// One look key set, as one undo step (named in the history as the look).
pub fn set(app: &mut CantusApp, track: TrackId, key: &str, word: &str) {
    let now = app
        .project()
        .track(track)
        .and_then(|t| t.instrument.native())
        .and_then(|n| n.text(key).map(str::to_string));
    if now.as_deref() != Some(word) {
        app.set_inst_text(track, key, Some(word.to_string()));
    }
}

/// A small plate at the display's bottom-left with `text` (the tuning being designed).
fn chip(ui: &egui::Ui, d: Rect, s: &Style, text: &str) {
    let pad = s.s(8.0);
    let h = s.s(20.0);
    let w = (d.width() - 2.0 * pad).min(s.s(360.0));
    let r = Rect::from_min_size(
        Pos2::new(d.left() + pad, d.bottom() - pad - h),
        Vec2::new(w, h),
    );
    with_brush(&ui.painter_at(d), |b| {
        well::paint(b, r, s, KitState::REST, None);
        let _ = ktext::paint(
            b,
            r.shrink2(Vec2::new(s.s(8.0), 0.0)),
            &Text::new(text, Role::Label, s.t.text)
                .caps(false)
                .align(Align2::LEFT_CENTER)
                .policy(Policy::Elide(ktext::Elide::End)),
            s.tk(),
        );
    });
}
