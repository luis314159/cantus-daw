//! The companion window's own pass (ADR 0091 §§2–3): what runs when its viewport repaints.
//! It reads what the main window left in [`Shared`], draws the backdrop, the dancers (each
//! in its box, through the one renderer, only when its pose changed), the visualiser and —
//! on hover, outside streaming mode — a thin frame, a menu button and a resize grip; it
//! turns gestures into the system's own window moves (drag, resize) and its menu's picks
//! into [`Action`]s; and it measures itself. Minimised, it does nothing at all.

use super::companion::{Action, Backdrop, Shared, VisMode};
use super::geom::{self, Area};
use super::{vis_paint, Slot};
use crate::i18n::{t, tf};
use crate::kit::color::with_alpha;
use crate::kit::glyph::{self, Glyph};
use crate::kit::menu::{Check, Item};
use crate::kit::text::Src;
use crate::kit::{with_brush, Fill, Style};
use cantus_dance::pose_at;
use egui::{pos2, vec2, Color32, Id, Rect, Sense, Ui, ViewportClass, ViewportCommand};
use std::sync::Mutex;
use std::time::Instant;

/// How long the frame and buttons linger after the pointer left, seconds.
const LINGER: f64 = 1.2;

/// The window's pass.
pub fn show(ui: &mut Ui, class: ViewportClass, shared: &Mutex<Shared>) {
    let t0 = Instant::now();
    let ctx = ui.ctx().clone();
    let mut sh = shared.lock().unwrap_or_else(|e| e.into_inner());
    let (minimized, close, outer, inner, now) = ctx.input(|i| {
        let v = i.viewport();
        (
            v.minimized == Some(true),
            v.close_requested(),
            v.outer_rect,
            v.inner_rect,
            i.time,
        )
    });
    if close {
        sh.actions.push(Action::Close);
        ctx.request_repaint_of(egui::ViewportId::ROOT);
    }
    sh.minimized = minimized;
    if minimized {
        // Paused: no drawing, no next frame asked for.
        return;
    }
    sh.outer = outer;
    sh.inner = inner;
    let st = sh.settings.clone();
    let s = Style::current();
    let k = s.k;
    let r = ui.max_rect();
    let embedded = class == ViewportClass::EmbeddedWindow;

    // Where the feet stand: just above the visualiser's band, else near the bottom.
    let l = vis_paint::layout(r, &st, k);
    let feet_y = l.band.map_or(r.bottom() - 18.0 * k, |b| b.top() - 10.0 * k);

    // The backdrop.
    // Without alpha (a driver that offers none) a see-through backdrop is the theme's.
    let backdrop = match st.backdrop {
        Backdrop::Transparent if !super::companion::see_through() => Backdrop::Theme,
        b => b,
    };
    with_brush(ui.painter(), |b| match backdrop {
        Backdrop::Transparent => {}
        Backdrop::Theme => super::dock::paint_ground_at(b, r, &s, feet_y),
        c => {
            if let Some([red, g, bl, _]) = c.chroma() {
                b.rect(r, Fill::Solid(Color32::from_rgb(red, g, bl)));
            }
        }
    });

    // The dancers.
    let ppp = ctx.pixels_per_point();
    // A tall, narrow window (a vertical video) keeps the characters within its width.
    let standard = super::companion_paint::standard_of(r.width(), r.height(), &sh.dancers);
    // Over a chroma key no contact shadow: the key stays exact wherever the character is not.
    let area = Area::of(r, ppp, standard).with_shadows(st.backdrop.chroma().is_none());
    let clip_px = Rect::from_min_max(
        (r.min.to_vec2() * ppp).to_pos2(),
        (r.max.to_vec2() * ppp).to_pos2(),
    );
    let fast = sh.frame.playing;
    if !sh.follows {
        // Stopped, the one character breathes on this window's own clock.
        let frame = sh.frame;
        if let Some((ch, pose)) = sh.dancers.first_mut() {
            *pose = pose_at(&frame, ch.as_ref(), now);
            pose.y = ((feet_y - r.top()) / r.height().max(1.0)).clamp(0.0, 1.0);
        }
    }
    let n = sh.dancers.len();
    while sh.slots.len() < n {
        let i = sh.slots.len();
        sh.slots.push(Slot::new(format!("companion-{i}"), 0));
    }
    sh.slots.truncate(n);
    let mut redraws = 0;
    let shared_ref = &mut *sh;
    for (i, (ch, pose)) in shared_ref.dancers.iter().enumerate() {
        let Some(placed) = geom::place(&area, clip_px, ch.as_ref(), pose, ppp) else {
            continue;
        };
        let (tex, drawn) = shared_ref.slots[i].texture(
            &ctx,
            ch.as_ref(),
            &placed.pose,
            &placed.stage,
            super::rate(fast, false),
        );
        redraws += usize::from(drawn.is_some());
        if let Some(tex) = tex {
            let painter = ui.painter().with_clip_rect(r);
            with_brush(&painter, |b| b.foreign(tex, placed.rect(ppp)));
        }
    }
    sh.redraws += redraws as u64;

    // The visualiser.
    if st.vis != VisMode::Off || st.bar_beat || st.song {
        let v = sh.vis.clone();
        with_brush(ui.painter(), |b| vis_paint::paint(b, &l, &v, &st, &s));
    }

    // Gestures: drag the window by anything in it, resize by the corner, the menu.
    let resp = ui.interact(
        r,
        Id::new("cantus-stage-companion"),
        Sense::click_and_drag(),
    );
    let hovered = ui.rect_contains_pointer(r);
    if hovered {
        sh.hover_until = now + LINGER;
    }
    let chrome = !st.streaming && sh.hover_until > now;
    if !st.streaming && resp.drag_started() && !embedded {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
    let grip = Rect::from_min_size(r.max - vec2(22.0 * k, 22.0 * k), vec2(20.0 * k, 20.0 * k));
    // The menu's button at the right edge, half-way down: clear of the chips and the band.
    let more = Rect::from_center_size(
        pos2(r.right() - 22.0 * k, r.center().y),
        vec2(26.0 * k, 26.0 * k),
    );
    let mut more_resp = None;
    if chrome {
        let g = ui.interact(grip, Id::new("cantus-stage-companion-grip"), Sense::drag());
        if g.drag_started() && !embedded {
            ctx.send_viewport_cmd(ViewportCommand::BeginResize(
                egui::viewport::ResizeDirection::SouthEast,
            ));
        }
        let m = ui.interact(more, Id::new("cantus-stage-companion-more"), Sense::click());
        let m = if m.hovered() {
            m.on_hover_text(t("stage.tip.menu"))
        } else {
            m
        };
        let fade = ((sh.hover_until - now) / 0.3).clamp(0.0, 1.0) as f32;
        let (gh, mh) = (g.hovered(), m.hovered());
        with_brush(ui.painter(), |b| {
            b.rrect_stroke(
                r.shrink(1.0 * k),
                14.0 * k,
                1.25 * k,
                with_alpha(s.t.highlight, 0.65 * fade),
            );
            let face = if mh { s.t.menu_hover } else { s.t.menu };
            b.circle(
                more.center(),
                more.width() * 0.5,
                Fill::Solid(with_alpha(face, 0.92 * fade)),
            );
            glyph::paint(
                b,
                more.shrink(6.0 * k),
                Glyph::More,
                with_alpha(s.t.menu_text, fade),
            );
            let gc = if gh { s.t.highlight } else { s.t.text_dim };
            glyph::paint(b, grip.shrink(3.0 * k), Glyph::Resize, with_alpha(gc, fade));
        });
        more_resp = Some(m);
    }
    egui::Popup::context_menu(&resp)
        .frame(crate::views::clips_chrome::menu_frame())
        .show(|ui| menu(ui, &mut sh));
    if let Some(m) = &more_resp {
        egui::Popup::menu(m)
            .frame(crate::views::clips_chrome::menu_frame())
            .show(|ui| menu(ui, &mut sh));
    }
    resp.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Other, true, t("stage.a11y.companion"))
    });

    // Its next frame: the main window asks for them while the song plays; stopped, the
    // breath asks for its own. While the frame lingers, one more when it fades.
    if !fast {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(
            1.0 / crate::stage::IDLE_RATE,
        ));
    }
    if chrome {
        ctx.request_repaint_after(std::time::Duration::from_millis(100));
    }

    // What this pass cost: CPU time, smoothed; frames a second from its passes' spacing.
    let ms = t0.elapsed().as_secs_f32() * 1e3;
    let dt = (now - sh.last_pass) as f32;
    sh.last_pass = now;
    let c = &mut sh.cost;
    c.cpu_ms = if c.frames == 0 {
        ms
    } else {
        c.cpu_ms * 0.9 + ms * 0.1
    };
    if dt > 0.0 && dt < 2.0 {
        c.fps = if c.fps == 0.0 {
            1.0 / dt
        } else {
            c.fps * 0.9 + 0.1 / dt
        };
    }
    c.frames += 1;
}

/// The menu's size (points), for the window it opens in on Windows.
#[cfg(windows)]
pub(crate) fn menu_size(ctx: &egui::Context) -> egui::Vec2 {
    let _ = ctx;
    let s = Style::current();
    let k = s.d(1.0);
    let rows =
        13.0 * crate::kit::menu::ROW + crate::kit::menu::BAND + 2.0 * crate::kit::menu::SEPARATOR;
    egui::vec2(300.0, rows * k + 12.0 * k)
}

/// The window's menu: one row a choice, its value at the right (a click steps it).
pub(crate) fn menu(ui: &mut Ui, sh: &mut Shared) {
    let st = &sh.settings;
    let size = format!("{} × {}", st.size[0].round(), st.size[1].round());
    let backdrop = t(st.backdrop.label_key());
    let vis = t(st.vis.label_key());
    let c = sh.cost;
    let cost = if c.frames == 0 {
        t("stage.cost.none")
    } else {
        tf(
            "stage.cost.line",
            &[
                ("ms", &format!("{:.2}", c.cpu_ms)),
                ("pct", &format!("{:.1}", c.core_percent())),
                ("fps", &format!("{:.0}", c.fps)),
            ],
        )
    };
    let gpu = t("stage.cost.gpu");
    let check = |on: bool| if on { Check::On } else { Check::Off };
    let name: &str = &sh.character_name;
    let items = [
        Item::Band(Src::Key("stage.companion.title")),
        Item::Action {
            label: Src::Key("stage.menu.character"),
            check: Check::None,
            shortcut: Some(name),
            submenu: false,
            enabled: true,
        },
        Item::Action {
            label: Src::Key("stage.menu.size"),
            check: Check::None,
            shortcut: Some(&size),
            submenu: false,
            enabled: true,
        },
        Item::Action {
            label: Src::Key("stage.menu.backdrop"),
            check: Check::None,
            shortcut: Some(&backdrop),
            submenu: false,
            enabled: true,
        },
        Item::Action {
            label: Src::Key("stage.menu.visualiser"),
            check: Check::None,
            shortcut: Some(&vis),
            submenu: false,
            enabled: true,
        },
        Item::Action {
            label: Src::Key("stage.menu.bar_beat"),
            check: check(st.bar_beat),
            shortcut: None,
            submenu: false,
            enabled: true,
        },
        Item::Action {
            label: Src::Key("stage.menu.song"),
            check: check(st.song),
            shortcut: None,
            submenu: false,
            enabled: true,
        },
        Item::Action {
            label: Src::Key("stage.menu.on_top"),
            check: check(st.on_top),
            shortcut: None,
            submenu: false,
            enabled: true,
        },
        Item::Action {
            label: Src::Key("stage.menu.streaming"),
            check: check(st.streaming),
            shortcut: None,
            submenu: false,
            enabled: true,
        },
        Item::Separator,
        Item::action(Src::Key("stage.menu.copy_obs")),
        Item::action(Src::Key("stage.menu.export_video")),
        Item::Action {
            label: Src::Str(&cost),
            check: Check::None,
            shortcut: None,
            submenu: false,
            enabled: false,
        },
        Item::Action {
            label: Src::Str(&gpu),
            check: Check::None,
            shortcut: None,
            submenu: false,
            enabled: false,
        },
        Item::Separator,
        Item::action(Src::Key("stage.menu.close")),
    ];
    let width = (ui.ctx().content_rect().width() - 16.0).clamp(200.0, 300.0);
    let (_, clicks) = crate::kit::widgets::menu_list(ui, &items, width);
    let pick = clicks.clicked.or(clicks.toggled);
    let action = match pick {
        Some(1) => Some(Action::NextCharacter),
        Some(2) => Some(Action::NextSize),
        Some(3) => Some(Action::NextBackdrop),
        Some(4) => Some(Action::NextVis),
        Some(5) => Some(Action::ToggleBarBeat),
        Some(6) => Some(Action::ToggleSong),
        Some(7) => Some(Action::ToggleOnTop),
        Some(8) => Some(Action::ToggleStreaming),
        Some(10) => Some(Action::CopyObsSetup),
        Some(11) => Some(Action::ExportVideo),
        Some(15) => Some(Action::Close),
        _ => None,
    };
    if let Some(a) = action {
        sh.actions.push(a);
        ui.ctx().request_repaint_of(egui::ViewportId::ROOT);
        // A choice that steps or ticks keeps the menu open (step again, see it change);
        // copying and closing are done with it.
        if matches!(
            a,
            Action::Close | Action::CopyObsSetup | Action::ExportVideo
        ) {
            ui.close();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::companion::StageSettings;
    use super::*;
    use cantus_dance::render::{render, Background, Stage};
    use cantus_dance::DanceFrame;
    use std::sync::{Arc, Mutex};

    /// The stage the window's dancer was last drawn on, over `backdrop`.
    fn drawn_over(backdrop: Backdrop) -> (cantus_dance::pose::Pose, Stage) {
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        let ctx = egui::Context::default();
        crate::design::install_fonts(&ctx);
        crate::design::install_style(&ctx);
        let ch = cantus_dance::cast::default_character();
        let frame = DanceFrame {
            playing: true,
            beats: 9.5,
            seconds: 4.75,
            bar: 2,
            bar_start_beats: 8.0,
            ..Default::default()
        };
        let sh = Shared {
            settings: StageSettings {
                companion_open: true,
                streaming: true,
                backdrop,
                vis: VisMode::Off,
                bar_beat: false,
                song: false,
                ..Default::default()
            },
            frame,
            dancers: vec![(ch.clone(), pose_at(&frame, ch.as_ref(), 0.0))],
            ..Default::default()
        };
        let shared = Arc::new(Mutex::new(sh));
        crate::raster::shoot(&ctx, egui::vec2(400.0, 460.0), 1.0, 3, None, |root| {
            show(root, egui::ViewportClass::Deferred, &shared)
        });
        let sh = shared.lock().unwrap();
        sh.slots[0].last.expect("the dancer was drawn")
    }

    /// Last review #3: the streaming window over a chroma key (the egui window: Linux,
    /// macOS) draws no contact shadow — nothing but the character itself touches the key;
    /// over the theme's stage the shadow is there.
    #[test]
    fn over_a_chroma_key_nothing_but_the_character_touches_the_key() {
        let ch = cantus_dance::cast::default_character();
        let (_, stage) = drawn_over(Backdrop::Theme);
        assert!(stage.shadows, "over the theme's stage, its shadow");
        let (pose, stage) = drawn_over(Backdrop::Green);
        assert!(!stage.shadows, "over a key, none");
        let key = Backdrop::Green.chroma().unwrap();
        let keyed = Stage {
            background: Background::Solid(key),
            ..stage
        };
        let alone = Stage {
            background: Background::Transparent,
            shadows: false,
            ..stage
        };
        let drawn = |s: &Stage| render(s, &[(ch.as_ref(), pose)]).unwrap();
        let (k, a) = (drawn(&keyed), drawn(&alone));
        let stray = k
            .pixels()
            .iter()
            .zip(a.pixels())
            .filter(|(k, a)| {
                a.alpha() == 0 && {
                    let k = k.demultiply();
                    [k.red(), k.green(), k.blue()] != [key[0], key[1], key[2]]
                }
            })
            .count();
        assert_eq!(
            stray, 0,
            "pixels off the key where the character draws nothing"
        );
    }
}
