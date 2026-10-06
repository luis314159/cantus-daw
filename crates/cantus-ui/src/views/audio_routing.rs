//! Channels and routing (round 20): name the interface's inputs and outputs ("Guitar",
//! "Mic 1", "Monitors L"), choose the pair the master plays on, and send a copy of the
//! master to a second pair for headphones, at its own level. A test tone on any pair
//! finds which jack is which. Everything here belongs to the interface and comes back
//! with it.

use super::audio_parts as parts;
use super::workspace_widgets as w;
use crate::app::CantusApp;
use crate::audio_setup::{self as setup, Hit};
use crate::i18n::{t, tf};
use crate::kit::glyph::Glyph;
use crate::kit::knob::KnobSize;
use crate::kit::meter::MIN_DB;
use crate::kit::text::{self, Elide, Policy, Role, Src, Text};
use crate::kit::widgets::{segmented, Param};
use crate::kit::{with_brush, Style};
use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

/// The window's size at 100 %.
const SIZE: Vec2 = Vec2::new(560.0, 600.0);
const LABEL_W: f32 = 110.0;
/// At most this many channels are listed (a 64-channel ASIO driver scrolls past them).
const MAX_ROWS: usize = 32;

/// "1–2", "3–4": the pair starting at channel `first` (0-based).
pub fn pair_label(first: usize, channels: usize) -> String {
    if first + 1 < channels {
        format!("{}–{}", first + 1, first + 2)
    } else {
        format!("{}", first + 1)
    }
}

/// The window, when open.
pub fn show(app: &mut CantusApp, ctx: &egui::Context) {
    if !app.audio_setup.routing {
        return;
    }
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let size = (SIZE * k).min(screen.size() - Vec2::new(40.0, 100.0));
    let mut open = true;
    let live = setup::live(app, true);
    egui::Window::new("cantus-audio-routing")
        .id(Id::new("cantus-audio-routing"))
        .title_bar(false)
        .frame(egui::Frame::NONE)
        .resizable(false)
        .fixed_size(size)
        .default_pos(Pos2::new(
            screen.center().x - size.x * 0.5,
            screen.top() + 70.0,
        ))
        .constrain(true)
        .show(ctx, |ui| {
            let rect = Rect::from_min_size(ui.max_rect().min, size);
            parts::panel(ui, rect, &s);
            let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 30.0 * k));
            if super::workspace_settings::title(ui, head, &s, Glyph::Send, "devices.routing.title")
            {
                open = false;
            }
            app.audio_setup.hit(
                ui.ctx(),
                Hit::Close,
                Rect::from_min_max(Pos2::new(head.right() - 36.0 * k, head.top()), head.max),
            );
            let body = Rect::from_min_max(
                Pos2::new(rect.left() + 14.0 * k, head.bottom() + 8.0 * k),
                Pos2::new(rect.right() - 8.0 * k, rect.bottom() - 10.0 * k),
            );
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(body));
            egui::ScrollArea::vertical()
                .id_salt("cantus-audio-routing-body")
                .auto_shrink([false, false])
                .show(&mut child, |ui| {
                    ui.set_width(body.width() - 14.0 * k);
                    ui.spacing_mut().item_spacing = Vec2::new(8.0 * k, 6.0 * k);
                    page(app, ui, &live);
                });
        });
    if !open || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.audio_setup.routing = false;
        setup::set_tone(app, None);
    }
}

fn page(app: &mut CantusApp, ui: &mut Ui, live: &setup::Live) {
    let Some(key) = setup::current_key(app) else {
        super::workspace_settings::note(ui, &t("devices.chip.none"), true);
        return;
    };
    let s = Style::current();
    super::workspace_settings::note(ui, &tf("devices.routing.for", &[("name", &key)]), false);
    // Outputs: the main pair, the cue, the tone.
    w::band(ui, Src::Key("devices.routing.outputs"));
    let outs = live.out_channels.max(2);
    let pairs: Vec<usize> = (0..outs).step_by(2).collect();
    let labels: Vec<String> = pairs.iter().map(|p| pair_label(*p, outs)).collect();
    let mut prof = app
        .settings
        .audio_setup
        .profile(&key)
        .cloned()
        .unwrap_or_default();
    let before = prof.clone();
    let value_w = (ui.available_width() / s.k - LABEL_W - 10.0).clamp(160.0, 360.0);
    // The main pair.
    let mut sel = pairs
        .iter()
        .position(|p| *p == prof.main as usize)
        .unwrap_or(0);
    let mut seg = Rect::NOTHING;
    ui.horizontal(|ui| {
        w::row_label(ui, "devices.routing.main", LABEL_W, 24.0);
        let src: Vec<Src<'_>> = labels.iter().map(|l| Src::Str(l)).collect();
        seg = segmented(ui, &mut sel, &src, &t("devices.routing.main"), value_w).rect;
    });
    for (i, p) in pairs.iter().enumerate() {
        app.audio_setup.hit(
            ui.ctx(),
            Hit::Main(*p as u16),
            crate::kit::segmented::segment(seg, pairs.len(), i),
        );
    }
    prof.main = pairs[sel.min(pairs.len() - 1)] as u16;
    // The cue: off, or a pair.
    let off = t("devices.routing.off");
    let mut cue_labels = vec![off.clone()];
    cue_labels.extend(labels.iter().cloned());
    let mut sel = prof
        .cue
        .and_then(|c| pairs.iter().position(|p| *p == c as usize))
        .map_or(0, |i| i + 1);
    ui.horizontal(|ui| {
        w::row_label(ui, "devices.routing.cue", LABEL_W, 24.0);
        let src: Vec<Src<'_>> = cue_labels.iter().map(|l| Src::Str(l)).collect();
        seg = segmented(ui, &mut sel, &src, &t("devices.routing.cue"), value_w).rect;
    });
    app.audio_setup.hit(
        ui.ctx(),
        Hit::Cue(None),
        crate::kit::segmented::segment(seg, cue_labels.len(), 0),
    );
    for (i, p) in pairs.iter().enumerate() {
        app.audio_setup.hit(
            ui.ctx(),
            Hit::Cue(Some(*p as u16)),
            crate::kit::segmented::segment(seg, cue_labels.len(), i + 1),
        );
    }
    prof.cue = sel.checked_sub(1).map(|i| pairs[i] as u16);
    if prof.cue.is_some() {
        let mut db = prof.cue_db;
        ui.horizontal(|ui| {
            w::row_label(ui, "devices.routing.cue_level", LABEL_W, 64.0);
            let name = t("devices.routing.cue_level");
            Param::new(&mut db, -60.0..=12.0, 0.0)
                .format(|v, b| {
                    use std::fmt::Write as _;
                    let _ = write!(b, "{v:+.0}");
                })
                .size(KnobSize::S)
                .cell(Src::Key("devices.routing.level"))
                .name(&name)
                .show(ui);
            ui.vertical(|ui| {
                ui.add_space(14.0 * s.k);
                super::workspace_settings::note(ui, &t("devices.routing.cue_hint"), false);
            });
        });
        prof.cue_db = db;
    }
    // The test tone, on any pair.
    let mut tone_labels = vec![off];
    tone_labels.extend(labels.iter().cloned());
    let mut sel = app
        .audio_setup
        .tone
        .and_then(|c| pairs.iter().position(|p| *p == c as usize))
        .map_or(0, |i| i + 1);
    let was = sel;
    ui.horizontal(|ui| {
        w::row_label(ui, "devices.routing.tone", LABEL_W, 24.0);
        let src: Vec<Src<'_>> = tone_labels.iter().map(|l| Src::Str(l)).collect();
        segmented(ui, &mut sel, &src, &t("devices.routing.tone"), value_w);
    });
    if sel != was {
        setup::set_tone(app, sel.checked_sub(1).map(|i| pairs[i] as u16));
    }
    // Names: outputs, then inputs with their meters.
    for c in 0..outs.min(MAX_ROWS) {
        name_row(app, ui, &mut prof, false, c, None);
    }
    w::band(ui, Src::Key("devices.routing.inputs"));
    if live.in_channels == 0 {
        super::workspace_settings::note(ui, &t("devices.inputs.none"), false);
    }
    for c in 0..live.in_channels.min(MAX_ROWS) {
        let lvl = parts::db(live.peaks.get(c).copied().unwrap_or(0.0));
        name_row(app, ui, &mut prof, true, c, Some(lvl));
    }
    if live.live {
        ui.ctx().request_repaint();
    }
    if prof != before {
        *app.settings.audio_setup.profile_mut(&key) = prof;
        setup::apply_routing(app);
        app.store_settings();
    }
}

/// A channel: its number, its name in a field, its meter (inputs).
fn name_row(
    app: &mut CantusApp,
    ui: &mut Ui,
    prof: &mut crate::audio_setup::memory::Profile,
    input: bool,
    c: usize,
    level: Option<f32>,
) {
    let s = Style::current();
    let k = s.k;
    let list = if input {
        &mut prof.inputs
    } else {
        &mut prof.outputs
    };
    let mut name = list.get(c).cloned().unwrap_or_default();
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(Vec2::new(LABEL_W * k, 24.0 * k), Sense::hover());
        let label = if input {
            tf("devices.routing.in_n", &[("n", &(c + 1))])
        } else {
            tf("devices.routing.out_n", &[("n", &(c + 1))])
        };
        with_brush(ui.painter(), |b| {
            let _ = text::paint(
                b,
                r,
                &Text::new(&label, Role::Body, s.t.text_dim)
                    .align(Align2::RIGHT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                s.tk(),
            );
        });
        let hint = if input {
            "devices.routing.name_in"
        } else {
            "devices.routing.name_out"
        };
        let resp = w::text_field(ui, &mut name, hint, 200.0);
        let h = if input {
            Hit::InputName(c)
        } else {
            Hit::OutputName(c)
        };
        app.audio_setup.hit(ui.ctx(), h, resp.rect);
        if let Some(db) = level {
            let (m, _) = ui.allocate_exact_size(
                Vec2::new((ui.available_width() - 8.0 * k).max(40.0 * k), 24.0 * k),
                Sense::hover(),
            );
            with_brush(ui.painter(), |b| {
                parts::level_bar(b, m.shrink2(Vec2::new(0.0, 7.0 * k)), &s, db, MIN_DB);
            });
        }
    });
    if list
        .get(c)
        .map_or(!name.trim().is_empty(), |old| old.trim() != name.trim())
    {
        crate::audio_setup::memory::Profile::set_name(list, c, &name);
    }
}
