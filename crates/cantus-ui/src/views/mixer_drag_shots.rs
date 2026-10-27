//! Pictures of round 25's routing gestures, rendered on the CPU through the whole window
//! (`crate::raster`), for looking at what was built — not a test that can fail.
//!
//! `CANTUS_SHOTS=<dir> cargo test --release -p cantus-ui --lib drag::shots -- --ignored`
//! (an optional `CANTUS_SHOTS_FILTER` keeps the names that contain it).
//!
//! They live beside `mixer_drag`'s tests because a cable starts on a port the desk builder
//! draws: until the band lands, the press on it is seeded as `source` records it.

use super::*;
use crate::views::mixer::{self, Control};
use cantus_core::{Project, Send, SendTap};
use cantus_engine::EngineHandle;
use egui::{Event, Vec2};
use std::cell::RefCell;
use std::rc::Rc;

/// What a picture shows.
#[derive(Copy, Clone, Debug, PartialEq)]
enum Scene {
    /// A cable from strip 2 held over strip 6 (plain: a send).
    DragSend,
    /// The same with Ctrl (the only output).
    DragOutput,
    /// The same with Alt over a strip with a compressor (a key).
    DragKey,
    /// A cable from the bus held over a strip that feeds the bus: refused.
    DragLoop,
    /// A send's end carried off every column: "Release to remove".
    DragRemove,
    /// Strip 2's number cell carried along the desk.
    Reorder,
    /// A group carried by its separator.
    ReorderGroup,
    /// A track (from the arrangement) held over a strip.
    Track,
    /// Strips nothing feeds, with their plates.
    RouteHere,
    /// A return's plate clicked: its menu.
    RouteHereMenu,
    /// A send's route menu with the level submenu.
    RouteMenu,
    /// The add-send list with *New return ▸* open.
    NewReturn,
    /// The inspector of a return: Receives.
    Receives,
    /// The strip menu's routing band (*Where it goes…*, *Its routes*).
    StripMenu,
    /// Click mode: strip 2's port clicked (armed), the pointer over strip 6.
    Armed,
}

/// The project every picture starts from: named channels, a bus, two returns fed by
/// sends (one pre-fader, one doubled), a compressor keyed from the kick.
fn project() -> Project {
    let mut p = crate::starter_project();
    let names = ["Kick", "Bass", "Keys", "Pad", "Lead", "Strings", "Bus"];
    for (i, n) in names.iter().enumerate() {
        p.strips[i].name = (*n).to_string();
    }
    let bus = p.strips[6].id;
    let returns: Vec<StripId> = p
        .strips
        .iter()
        .filter(|s| s.dock == Dock::Right)
        .map(|s| s.id)
        .collect();
    p.strips[2].output = Some(bus);
    p.strips[3].output = Some(bus);
    let mut send = Send::new(returns[0]);
    send.level = 0.5;
    p.strips[1].sends.push(send);
    let mut pre = Send::new(returns[0]);
    pre.level = 0.35;
    pre.tap = SendTap::Pre;
    p.strips[2].sends.push(pre);
    p.strips[6].sends.push(Send::new(returns[1]));
    p.strips[3].sends.push(Send::new(bus));
    let r0 = p
        .strips
        .iter()
        .position(|s| s.id == returns[0])
        .unwrap_or(0);
    p.strips[r0].name = "Reverb".into();
    let r1 = p
        .strips
        .iter()
        .position(|s| s.id == returns[1])
        .unwrap_or(0);
    p.strips[r1].name = "Delay".into();
    p
}

/// The pointer's plan once the desk has been laid out: where it presses, where it goes,
/// and the modifiers held.
#[derive(Clone, Debug, Default)]
struct Plan {
    press: Option<(Pos2, Option<Source>)>,
    to: Option<Pos2>,
    mods: Modifiers,
    release: bool,
    /// A menu row to rest the pointer on (its submenu opens), by its key.
    hover: Option<&'static str>,
}

const WARM: usize = 4;
const TRAVEL: usize = 8;

fn shoot(
    dir: &std::path::Path,
    name: &str,
    scene: Scene,
    size: Vec2,
    ppp: f32,
    dark: bool,
    width: Option<usize>,
) {
    let ctx = egui::Context::default();
    crate::design::install_fonts(&ctx);
    crate::design::install_style(&ctx);
    let theme = if dark {
        crate::theme::slate()
    } else {
        crate::theme::paper()
    };
    crate::theme::apply(&theme, &ctx);
    let mut app = CantusApp::from_parts(Some(project()), EngineHandle::dummy(), false);
    app.central = crate::app::CentralView::Mixer;
    if let Some(w) = width {
        mixer::set_width_mode(&mut app, mixer::layout::WidthMode::from_index(w));
    }
    let ids: Vec<StripId> = app.project().strips.iter().map(|s| s.id).collect();
    let ret = app
        .project()
        .strips
        .iter()
        .find(|s| s.dock == Dock::Right)
        .map(|s| s.id)
        .unwrap_or(ids[0]);
    match scene {
        Scene::DragKey => {
            let _ = app.duck_strip_from(ids[5], ids[0]);
            app.select_strip(ids[1]);
        }
        Scene::Receives => app.select_strip(ret),
        Scene::ReorderGroup => {
            app.create_strip_group(&[ids[2], ids[3]]);
            app.select_strip(ids[1]);
        }
        Scene::RouteHere | Scene::RouteHereMenu => {
            app.select_strip(ids[4]);
        }
        _ => app.select_strip(ids[1]),
    }
    let plan = Rc::new(RefCell::new(Plan::default()));
    let frames = WARM + 2 + TRAVEL + 3;
    let path = dir.join(format!("{name}.png"));
    let plan_ui = plan.clone();
    let mut n = 0usize;
    crate::raster::shoot_events(
        &ctx,
        size / ppp,
        ppp,
        frames,
        Some(&path),
        |frame, ctx| {
            let plan = plan.borrow();
            let mut ev = events(&plan, frame);
            if let (Some(row), true) = (plan.hover, frame > WARM) {
                if let Some(r) = crate::views::route_menu::row_rect(ctx, mixer::route_owner(), row)
                {
                    ev.push(Event::PointerMoved(r.center()));
                }
            }
            ev
        },
        |root| {
            if n == WARM - 1 {
                *plan_ui.borrow_mut() = plan_for(&mut app, root.ctx(), scene, &ids, ret);
            }
            if n == WARM + 1 {
                // The press frame: the port's press, as `source` records it.
                if let Some((at, Some(src))) = plan_ui.borrow().press {
                    app.mixer.drag.press = Some(Press {
                        src,
                        origin: at,
                        anchor: Rect::from_center_size(at, vec2(8.0, 8.0)),
                    });
                }
                if scene == Scene::Track {
                    let t = app.project().tracks[0].id;
                    egui::DragAndDrop::set_payload(root.ctx(), TrackDrag(t));
                }
            }
            n += 1;
            let mut frame = eframe::Frame::_new_kittest();
            eframe::App::ui(&mut app, root, &mut frame);
        },
    );
    println!("wrote {}", path.display());
}

fn at(ctx: &egui::Context, strip: Option<StripId>, c: Control) -> Pos2 {
    mixer::control_rect(ctx, strip, c).map_or(Pos2::ZERO, |r| r.center())
}

/// Where the pointer goes for `scene` (the desk is laid out by now); menus that open
/// without a pointer are opened here.
fn plan_for(
    app: &mut CantusApp,
    ctx: &egui::Context,
    scene: Scene,
    ids: &[StripId],
    ret: StripId,
) -> Plan {
    let meter = |s: StripId| at(ctx, Some(s), Control::Meter);
    // The real port in the cable band (round 25's integration: the desk wires it).
    let port = |s: StripId| at(ctx, Some(s), Control::Port);
    let fader = |s: StripId| at(ctx, Some(s), Control::Fader);
    let head = |s: StripId| at(ctx, Some(s), Control::Head);
    let mut p = Plan::default();
    match scene {
        Scene::DragSend | Scene::DragOutput => {
            p.press = Some((port(ids[1]), None));
            p.to = Some(fader(ids[5]));
            if scene == Scene::DragOutput {
                p.mods = Modifiers::COMMAND;
            }
        }
        Scene::DragKey => {
            p.press = Some((port(ids[0]), None));
            p.to = Some(fader(ids[5]));
            p.mods = Modifiers::ALT;
        }
        Scene::DragLoop => {
            p.press = Some((port(ids[6]), None));
            p.to = Some(fader(ids[2]));
        }
        Scene::DragRemove => {
            let route = Route::send(ids[1], ret);
            let a = meter(ret);
            p.press = Some((
                a,
                Some(Source::End {
                    route,
                    end: End::Destination,
                }),
            ));
            // Up over the title row: off every column.
            let title = head(ids[8]).y - 34.0;
            p.to = Some(pos2(a.x - 220.0, title.max(8.0)));
        }
        Scene::Reorder => {
            p.press = Some((head(ids[1]), None));
            p.to = Some(head(ids[5]) + vec2(14.0, 0.0));
        }
        Scene::ReorderGroup => {
            let gid = app.project().strip(ids[2]).and_then(|s| s.group);
            let sep = gid
                .and_then(|g| ctx.read_response(Id::new(("mixer-group", g.0))))
                .map_or(Pos2::ZERO, |r| r.rect.center());
            p.press = Some((sep, None));
            p.to = Some(head(ids[8]) + vec2(14.0, 0.0));
        }
        Scene::Track => {
            p.press = Some((fader(ids[9]) - vec2(300.0, 0.0), None));
            p.to = Some(fader(ids[9]));
        }
        Scene::RouteHere => {
            let plate = ctx
                .read_response(crate::views::route_menu::plate_id(ids[9]))
                .map_or(Pos2::ZERO, |r| r.rect.center());
            p.to = Some(plate);
        }
        Scene::RouteHereMenu => {
            // The last return: nothing feeds it.
            let empty = app
                .project()
                .strips
                .iter()
                .rev()
                .find(|s| s.dock == Dock::Right)
                .map_or(ret, |s| s.id);
            let plate = ctx
                .read_response(crate::views::route_menu::plate_id(empty))
                .map_or(Pos2::ZERO, |r| r.rect.center());
            p.press = Some((plate, None));
            p.release = true;
        }
        Scene::RouteMenu => {
            let route = Route::send(ids[2], ret);
            let a = fader(ids[2]);
            crate::views::route_menu::open(ctx, mixer::route_owner(), route, a);
            p.hover = Some("routing.menu.level");
        }
        Scene::NewReturn => {
            let a = head(ids[1]) + vec2(10.0, 20.0);
            crate::views::route_menu::open_add_send(ctx, mixer::route_owner(), ids[1], a);
            p.hover = Some("routing.menu.new_return");
        }
        Scene::Receives => {}
        Scene::StripMenu => {
            mixer::open_menu(app, Some(ids[2]), head(ids[2]) + vec2(6.0, 10.0));
        }
        Scene::Armed => {
            p.press = Some((port(ids[1]), None));
            p.release = true;
            p.to = Some(fader(ids[5]));
        }
    }
    p
}

/// The events of `frame`: after the warm-up, move to the press, press, travel, hold (or
/// release, for a click).
fn events(plan: &Plan, frame: usize) -> Vec<Event> {
    let mut ev = vec![Event::ModifiersChanged(plan.mods)];
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: plan.mods,
    };
    if frame < WARM {
        return ev;
    }
    let k = frame - WARM;
    match (plan.press, plan.to) {
        (Some((a, _)), to) => {
            if k == 0 {
                ev.push(Event::PointerMoved(a));
            } else if k == 1 {
                ev.push(button(a, true));
            } else if plan.release && k == 2 {
                ev.push(button(a, false));
            } else if let Some(z) = to {
                let u = ((k - 1) as f32 / TRAVEL as f32).min(1.0);
                ev.push(Event::PointerMoved(a + (z - a) * u));
            }
        }
        (None, Some(z)) => ev.push(Event::PointerMoved(z)),
        (None, None) => {}
    }
    ev
}

#[test]
#[ignore = "pictures: CANTUS_SHOTS=<dir> cargo test --release -p cantus-ui --lib drag::shots -- --ignored"]
fn shots() {
    let Some(dir) = std::env::var_os("CANTUS_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    std::fs::create_dir_all(&dir).expect("the pictures' folder");
    let filter = std::env::var("CANTUS_SHOTS_FILTER").unwrap_or_default();
    let scenes = [
        ("drag-send", Scene::DragSend),
        ("drag-output", Scene::DragOutput),
        ("drag-key", Scene::DragKey),
        ("drag-loop", Scene::DragLoop),
        ("drag-remove", Scene::DragRemove),
        ("reorder", Scene::Reorder),
        ("reorder-group", Scene::ReorderGroup),
        ("track", Scene::Track),
        ("routehere", Scene::RouteHere),
        ("routehere-menu", Scene::RouteHereMenu),
        ("route-menu", Scene::RouteMenu),
        ("new-return", Scene::NewReturn),
        ("receives", Scene::Receives),
        ("strip-menu", Scene::StripMenu),
        ("armed", Scene::Armed),
    ];
    let size = vec2(1600.0, 900.0);
    for (name, scene) in scenes {
        for (suffix, locale, ppp, dark, width) in [
            ("", crate::i18n::Locale::En, 1.0, true, None),
            ("-es", crate::i18n::Locale::Es, 1.0, true, None),
            ("-paper", crate::i18n::Locale::En, 1.0, false, None),
            ("-75", crate::i18n::Locale::En, 0.75, true, None),
            ("-200", crate::i18n::Locale::En, 2.0, true, None),
            ("-wide", crate::i18n::Locale::En, 1.0, true, Some(1)),
            ("-xl", crate::i18n::Locale::En, 1.0, true, Some(2)),
        ] {
            let full = format!("routing-{name}{suffix}");
            let wanted = if filter.is_empty() {
                suffix.is_empty()
            } else {
                full.contains(&filter)
            };
            if !wanted {
                continue;
            }
            crate::i18n::default_thread_locale(locale);
            crate::i18n::set_thread_locale(locale);
            let px = if ppp == 2.0 { size * 1.5 } else { size };
            shoot(&dir, &full, scene, px, ppp, dark, width);
        }
    }
}
