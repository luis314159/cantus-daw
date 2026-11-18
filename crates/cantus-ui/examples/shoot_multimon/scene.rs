//! `multimon`: the design pictures for several screens (ADR 0151, proposed — phase 1).
//!
//! The real application, through `CantusApp::assemble`, fed as egui-winit feeds a window on
//! a screen of its own (sizes in points are the window's pixels over the system's scale
//! times the zoom, the zoom the screen-following rule picks for that screen): a laptop
//! 2 880 × 1 800 at the system's 200 % holding the main window, and a 1 920 × 1 080 screen
//! at 100 % holding a window of its own with the mixer. What does not exist yet — the rows
//! of the tab's menu, the View menu's band, a window's bar, the screens map — is mocked with
//! the kit's own pieces (its menus, its panels, its text and the top bar's own groups), so
//! the picture is what the build would draw. The desk picture puts both on two screens.
//! `CANTUS_SHOOT_LANG=es` for Spanish.

use cantus_engine::EngineHandle;
use cantus_ui::i18n::{t, tf};
use cantus_ui::kit::menu::{Check, Item};
use cantus_ui::kit::text::{self as ktext, Role, Src, Text};
use cantus_ui::kit::{with_brush, Fill, Style};
use cantus_ui::views::transport::layout::Group;
use cantus_ui::views::workspace::{Pane, Workspace};
use cantus_ui::CantusApp;
use egui::{pos2, vec2, Align2, Color32, Event, Pos2, Rect, Vec2};
use std::path::{Path, PathBuf};

/// A screen the rig feeds: its pixels, the system's scale, and the taskbar it leaves.
#[derive(Copy, Clone)]
struct Screen {
    px: Vec2,
    native: f32,
    taskbar: f32,
}

const LAPTOP: Screen = Screen {
    px: Vec2::new(2880.0, 1800.0),
    native: 2.0,
    taskbar: 48.0,
};
const EXTERNAL: Screen = Screen {
    px: Vec2::new(1920.0, 1080.0),
    native: 1.0,
    taskbar: 48.0,
};

/// The interface's scale the screen-following rule (ADR 0111 §7, ADR 0149) picks for each
/// screen: 0.9 on the laptop at 200 %, 1.25 on the 1 920-px screen at 100 % — the scale each
/// window keeps on its own screen (ADR 0151 §5).
const LAPTOP_ZOOM: f32 = 0.9;
const EXTERNAL_ZOOM: f32 = 1.25;

struct Rig {
    app: CantusApp,
    ctx: egui::Context,
    t: f64,
    tex: cantus_ui::raster::Textures,
    shapes: Vec<egui::epaint::ClippedShape>,
    size: Vec2,
    pointer: Pos2,
    _dir: tempfile::TempDir,
}

impl Rig {
    fn new() -> Rig {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false,"plugin_scan_at_start":false}"#,
        )
        .unwrap();
        let files = cantus_ui::app::Files::read(dir.path(), Some(cantus_ui::i18n::locale()));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let p = cantus_ui::demos::house_without_dancer();
        let mut app = CantusApp::assemble(
            &ctx,
            files,
            Some(p),
            EngineHandle::dummy(),
            None,
            None,
            None,
            false,
        );
        crate::studio::studio(&mut app, 256);
        Rig {
            app,
            ctx,
            t: 0.0,
            tex: Default::default(),
            shapes: Vec::new(),
            size: Vec2::ZERO,
            pointer: pos2(-10.0, -10.0),
            _dir: dir,
        }
    }

    /// One pass for a maximised window on `screen`, drawn by `draw`.
    fn pass(&mut self, screen: Screen, draw: &mut dyn FnMut(&mut CantusApp, &mut egui::Ui)) {
        self.t += 1.0 / 60.0;
        let ppp = self.ctx.zoom_factor() * screen.native;
        let window = vec2(screen.px.x, screen.px.y - screen.taskbar * screen.native);
        self.size = window / ppp;
        let rect = Rect::from_min_size(Pos2::ZERO, self.size);
        let mut input = egui::RawInput {
            screen_rect: Some(rect),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events: vec![Event::PointerMoved(self.pointer)],
            ..Default::default()
        };
        let vp = input.viewports.entry(egui::ViewportId::ROOT).or_default();
        vp.native_pixels_per_point = Some(screen.native);
        vp.monitor_size = Some(screen.px / ppp);
        vp.inner_rect = Some(rect);
        vp.outer_rect = Some(rect);
        vp.maximized = Some(true);
        vp.focused = Some(true);
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| draw(app, ui));
        self.tex.update(&out.textures_delta);
        out.textures_delta.clear();
        self.shapes = out.shapes;
    }

    fn passes(
        &mut self,
        screen: Screen,
        n: usize,
        draw: &mut dyn FnMut(&mut CantusApp, &mut egui::Ui),
    ) {
        for _ in 0..n {
            self.pass(screen, draw);
        }
    }

    /// Where `words` were painted last pass, if they were.
    fn text_at(&self, words: &str) -> Option<Rect> {
        fn walk(s: &egui::Shape, words: &str, out: &mut Option<Rect>) {
            match s {
                egui::Shape::Text(t) if t.galley.text() == words => {
                    out.get_or_insert(t.galley.rect.translate(t.pos.to_vec2()));
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, words, out)),
                _ => {}
            }
        }
        let mut out = None;
        for c in &self.shapes {
            walk(&c.shape, words, &mut out);
        }
        out
    }

    fn shoot(&self, path: &Path) {
        let ppp = self.ctx.pixels_per_point();
        let prims = self.ctx.tessellate(self.shapes.clone(), ppp);
        let bg = self.ctx.global_style().visuals.panel_fill;
        let mut c = cantus_ui::raster::Canvas::new(
            (self.size.x * ppp).round() as usize,
            (self.size.y * ppp).round() as usize,
            bg,
        );
        c.paint(&prims, &self.tex, ppp);
        let _ = c.write_png(path);
        println!("{}", path.display());
    }
}

/// A kit menu at `at`, `width` points wide; returns its rect.
fn menu(ctx: &egui::Context, id: &str, at: Pos2, items: &[Item<'_>], width: f32) -> Rect {
    let r = egui::Area::new(egui::Id::new(("multimon-mock", id)))
        .order(egui::Order::Foreground)
        .fixed_pos(at)
        .show(ctx, |ui| {
            let _ = cantus_ui::kit::widgets::menu_list(ui, items, width);
        });
    r.response.rect
}

/// The centre of row `i` of a menu laid at `rect`.
fn row_centre(rect: Rect, items: &[Item<'_>], i: usize) -> Pos2 {
    let s = Style::current();
    cantus_ui::kit::menu::row_in(rect, items, &s, i).center()
}

fn act(key: &'static str) -> Item<'static> {
    Item::action(Src::Key(key))
}

fn act_str(s: &str) -> Item<'_> {
    Item::action(Src::Str(s))
}

fn with_key<'a>(mut i: Item<'a>, key: &'a str) -> Item<'a> {
    if let Item::Action { shortcut, .. } = &mut i {
        *shortcut = Some(key);
    }
    i
}

fn sub(mut i: Item<'_>) -> Item<'_> {
    if let Item::Action { submenu, .. } = &mut i {
        *submenu = true;
    }
    i
}

fn checked(mut i: Item<'_>, c: Check) -> Item<'_> {
    if let Item::Action { check, .. } = &mut i {
        *check = c;
    }
    i
}

fn disabled(mut i: Item<'_>) -> Item<'_> {
    if let Item::Action { enabled, .. } = &mut i {
        *enabled = false;
    }
    i
}

/// Paint `words` in `rect` in the kit's lettering.
fn words(ui: &egui::Ui, rect: Rect, w: &str, role: Role, color: Color32, align: Align2) {
    let s = Style::current();
    with_brush(ui.painter(), |b| {
        let _ = ktext::paint(
            b,
            rect,
            &Text::new(w, role, color)
                .align(align)
                .policy(ktext::Policy::Elide(ktext::Elide::End)),
            s.tk(),
        );
    });
}

/// The OS's name for a screen (Windows' DisplayConfig, X11's RandR, macOS's
/// `localizedName`), as the build would read it; by number where there is none.
fn screen_name(n: usize) -> &'static str {
    let es = cantus_ui::i18n::locale() == cantus_ui::i18n::Locale::Es;
    match (n, es) {
        (1, false) => "Built-in display",
        (1, true) => "Pantalla integrada",
        _ => "DELL U2723QE",
    }
}

/// A key of the kit's bar: a well with a glyph.
fn bar_key(ui: &egui::Ui, r: Rect, g: cantus_ui::kit::glyph::Glyph, lit: bool) {
    let s = Style::current();
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(r);
        let fill = if lit { s.t.selection } else { s.t.well };
        b.rrect(r, 4.0 * s.k, Fill::Solid(fill));
        let side = (r.height() * 0.5).round();
        let gl = Rect::from_center_size(r.center(), vec2(side, side));
        cantus_ui::kit::glyph::paint(b, gl, g, s.t.text);
    });
}

/// **A window's bar** (ADR 0151 §4): the window's menu and name at the left, then the top
/// bar's own groups the window shows (by default the transport, the position, the tempo and
/// the master meter), back into the main window at the right. Returns the menu key's rect.
fn window_bar(app: &mut CantusApp, ui: &mut egui::Ui, name: &str, menu_lit: bool) -> Rect {
    let s = Style::current();
    let k = s.k;
    let rect = ui.max_rect();
    ui.allocate_rect(rect, egui::Sense::hover());
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(rect);
        b.rect(r, Fill::Solid(s.t.toolbar));
        b.hline(
            r.left(),
            r.right(),
            r.bottom() - b.px() * 0.5,
            b.px(),
            s.t.ink,
        );
    });
    let pad = 8.0 * k;
    let key = (rect.height() - 12.0 * k).round();
    let menu_key = Rect::from_min_size(
        pos2(rect.left() + pad, rect.center().y - key * 0.5),
        vec2(key, key),
    );
    bar_key(ui, menu_key, cantus_ui::kit::glyph::Glyph::More, menu_lit);
    let gl = Rect::from_center_size(
        pos2(menu_key.right() + 16.0 * k, rect.center().y),
        vec2(16.0 * k, 16.0 * k),
    );
    with_brush(ui.painter(), |b| {
        cantus_ui::kit::glyph::paint(b, gl, cantus_ui::kit::glyph::Glyph::Mixer, s.t.highlight)
    });
    let name_r = Rect::from_min_max(
        pos2(gl.right() + 6.0 * k, rect.top()),
        pos2(gl.right() + 120.0 * k, rect.bottom()),
    );
    words(ui, name_r, name, Role::Title, s.t.text, Align2::LEFT_CENTER);
    let ctx = ui.ctx().clone();
    let wants = cantus_ui::views::transport::wants(app, &ctx);
    let mut x = name_r.right() + 12.0 * k;
    for g in [
        Group::Transport,
        Group::Position,
        Group::Tempo,
        Group::Meter,
    ] {
        let w = wants
            .iter()
            .find(|w| w.group == g)
            .map_or(160.0 * k, |w| w.max);
        let r = Rect::from_min_max(pos2(x, rect.top()), pos2(x + w, rect.bottom()));
        cantus_ui::views::transport::draw_group(g, app, ui, r);
        x += w + 10.0 * k;
    }
    let back = Rect::from_min_size(
        pos2(rect.right() - pad - key, rect.center().y - key * 0.5),
        vec2(key, key),
    );
    bar_key(ui, back, cantus_ui::kit::glyph::Glyph::Restore, false);
    menu_key
}

/// The window of its own with the mixer (and the Routing tab beside it), as a pass on its
/// screen: its bar on top, its own dock under it. Returns the menu key's rect.
fn mixer_window(app: &mut CantusApp, ui: &mut egui::Ui, ws: &mut Workspace, lit: bool) -> Rect {
    std::mem::swap(&mut app.workspace, ws);
    let s = Style::current();
    let h = (cantus_ui::views::transport::HEIGHT * s.t.density).round();
    let name = t("workspace.pane.mixer");
    let mut key = Rect::NOTHING;
    egui::Panel::top(egui::Id::new("multimon-bar"))
        .frame(egui::Frame::NONE)
        .exact_size(h)
        .resizable(false)
        .show(ui, |ui| key = window_bar(app, ui, &name, lit));
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(s.t.panel))
        .show(ui, |ui| cantus_ui::views::workspace::show(app, ui));
    std::mem::swap(&mut app.workspace, ws);
    key
}

/// One screen on the map: its number, where the system puts it (physical pixels), its
/// screen, the interface's scale on it.
type Placed = (usize, Rect, Screen, f32);

/// **The screens map** (ADR 0151 §9): every screen to scale where the system places it,
/// its name, size and scales, and the windows on it. `missing`: the second screen is gone.
fn map_card(ctx: &egui::Context, missing: bool) {
    let s = Style::current();
    let k = s.k;
    let screen = ctx.content_rect();
    let size = vec2(760.0 * k, 430.0 * k).min(screen.size() - vec2(16.0, 16.0));
    let rect = Rect::from_center_size(screen.center(), size);
    egui::Area::new(egui::Id::new("multimon-map"))
        .order(egui::Order::Foreground)
        .fixed_pos(rect.min)
        .show(ctx, |ui| {
            ui.set_min_size(size);
            cantus_ui::views::audio_parts::panel(ui, rect, &s);
            let head = Rect::from_min_size(rect.min, vec2(rect.width(), 32.0 * k));
            let glyph = cantus_ui::kit::glyph::Glyph::Maximise;
            let _ = cantus_ui::kit::dialog::head(ui, head, &s, glyph, "screens.map.title");
            let hint = Rect::from_min_max(
                pos2(rect.left() + 20.0 * k, rect.bottom() - 34.0 * k),
                pos2(rect.right() - 20.0 * k, rect.bottom() - 12.0 * k),
            );
            let dim = s.t.text_dim;
            words(
                ui,
                hint,
                &t("screens.map.hint"),
                Role::Body,
                dim,
                Align2::LEFT_CENTER,
            );
            let body = Rect::from_min_max(
                pos2(rect.left() + 20.0 * k, head.bottom() + 16.0 * k),
                pos2(rect.right() - 20.0 * k, hint.top() - 12.0 * k),
            );
            // The desktop in physical pixels: the laptop at the origin, the screen to its
            // right, a little lower.
            let desk: [Placed; 2] = [
                (
                    1,
                    Rect::from_min_size(Pos2::ZERO, LAPTOP.px),
                    LAPTOP,
                    LAPTOP_ZOOM,
                ),
                (
                    2,
                    Rect::from_min_size(pos2(3000.0, 300.0), EXTERNAL.px),
                    EXTERNAL,
                    EXTERNAL_ZOOM,
                ),
            ];
            let all = desk.iter().fold(Rect::NOTHING, |a, d| a.union(d.1));
            let f = (body.width() / all.width()).min(body.height() / all.height());
            let off = body.center() - all.center().to_vec2() * f;
            let to =
                |r: Rect| Rect::from_min_max(off + r.min.to_vec2() * f, off + r.max.to_vec2() * f);
            for (n, phys, sc, zoom) in desk {
                map_screen(ui, to(phys), n, sc, zoom, missing && n == 2);
            }
        });
}

/// One screen of the map, in `r`.
fn map_screen(ui: &egui::Ui, r: Rect, n: usize, sc: Screen, zoom: f32, gone: bool) {
    let s = Style::current();
    let k = s.k;
    with_brush(ui.painter(), |b| {
        let r = b.snap_rect(r);
        if gone {
            let c = [
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
                r.left_top(),
            ];
            b.dashed(&c, 1.5 * b.px(), s.t.text_dim, 6.0 * k, 4.0 * k);
        } else {
            b.rrect(r, 6.0 * k, Fill::Solid(s.t.well));
            b.rrect_stroke(r, 6.0 * k, b.px(), s.t.edge);
        }
    });
    let line = 18.0 * k;
    let top = Rect::from_min_size(
        r.min + vec2(10.0 * k, 8.0 * k),
        vec2(r.width() - 20.0 * k, line),
    );
    let title = format!(
        "{} · {}",
        tf("screens.map.screen", &[("n", &n)]),
        screen_name(n)
    );
    let ink = if gone { s.t.text_dim } else { s.t.text };
    words(ui, top, &title, Role::Section, ink, Align2::LEFT_CENTER);
    let second = top.translate(vec2(0.0, line));
    let detail = if gone {
        t("screens.map.missing")
    } else {
        let sys = (sc.native * 100.0).round();
        let z = (zoom * 100.0).round();
        format!(
            "{} · {}",
            tf("screens.map.size", &[("w", &sc.px.x), ("h", &sc.px.y)]),
            tf("screens.map.scale", &[("sys", &sys), ("ui", &z)])
        )
    };
    words(
        ui,
        second,
        &detail,
        Role::Label,
        s.t.text_dim,
        Align2::LEFT_CENTER,
    );
    if n == 1 {
        let third = second.translate(vec2(0.0, line));
        let p = t("screens.map.primary");
        words(
            ui,
            third,
            &p,
            Role::Label,
            s.t.text_dim,
            Align2::LEFT_CENTER,
        );
    }
    if gone {
        return;
    }
    // The window on it, inside its work area.
    let (glyph, label) = if n == 1 {
        (
            cantus_ui::kit::glyph::Glyph::Arrangement,
            t("screens.map.main"),
        )
    } else {
        (
            cantus_ui::kit::glyph::Glyph::Mixer,
            t("workspace.pane.mixer"),
        )
    };
    let lines = if n == 1 { 3.6 } else { 2.6 };
    let work = Rect::from_min_max(
        pos2(r.left() + 10.0 * k, r.top() + line * lines),
        pos2(r.right() - 10.0 * k, r.bottom() - 10.0 * k),
    );
    let lit = n == 2;
    with_brush(ui.painter(), |b| {
        let w = b.snap_rect(work);
        let fill = if lit { s.t.selection } else { s.t.panel_raised };
        b.rrect(w, 4.0 * k, Fill::Solid(fill));
        let edge = if lit { s.t.highlight } else { s.t.edge };
        b.rrect_stroke(w, 4.0 * k, b.px(), edge);
        let gl = Rect::from_center_size(w.center() - vec2(0.0, 12.0 * k), vec2(20.0 * k, 20.0 * k));
        let gc = if lit { s.t.highlight } else { s.t.text };
        cantus_ui::kit::glyph::paint(b, gl, glyph, gc);
    });
    let lr = Rect::from_center_size(
        work.center() + vec2(0.0, 12.0 * k),
        vec2(work.width() - 8.0 * k, line),
    );
    words(ui, lr, &label, Role::Body, s.t.text, Align2::CENTER_CENTER);
}

pub fn shots(dir: &Path) {
    // The whole app on a thread of its own: Windows' main thread's 1 MB stack is too small
    // for an assembled app's frame.
    let dir = dir.to_path_buf();
    std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || run(&dir))
        .unwrap()
        .join()
        .unwrap();
}

/// The tab menu's rows for an area in the main window (ADR 0151 §2), and the submenu of
/// *Move to*.
fn tab_rows() -> (Vec<Item<'static>>, Vec<Item<'static>>) {
    let rows = vec![
        act("screens.menu.new_window"),
        sub(act("screens.menu.move_to")),
        act("screens.menu.float"),
        act("ux.panels.collapse"),
        act("ux.panels.maximise"),
        act("workspace.menu.hide"),
        Item::Separator,
        act("layout.reset"),
    ];
    let mixer = Box::leak(
        tf(
            "screens.menu.to_window",
            &[("name", &t("workspace.pane.mixer"))],
        )
        .into_boxed_str(),
    );
    let one = Box::leak(
        tf(
            "screens.menu.to_screen",
            &[("n", &1), ("name", &screen_name(1))],
        )
        .into_boxed_str(),
    );
    let two = Box::leak(
        tf(
            "screens.menu.to_screen",
            &[("n", &2), ("name", &screen_name(2))],
        )
        .into_boxed_str(),
    );
    let subs = vec![
        checked(act("screens.menu.to_main"), Check::Radio(true)),
        act_str(mixer),
        Item::Separator,
        act_str(one),
        act_str(two),
    ];
    (rows, subs)
}

/// The View menu's lower part (ADR 0151 §7): the layouts with their keys, then the band
/// for windows and screens.
fn view_rows() -> Vec<Item<'static>> {
    let needs = Box::leak(tf("screens.layout.needs", &[("n", &3)]).into_boxed_str());
    vec![
        Item::Band(Src::Key("menu.layout")),
        with_key(checked(act("layout.default"), Check::Radio(false)), "Alt+1"),
        with_key(
            checked(act("screens.layout.two_mix"), Check::Radio(true)),
            "Alt+2",
        ),
        with_key(
            checked(act("screens.layout.two_edit"), Check::Radio(false)),
            "Alt+3",
        ),
        with_key(
            disabled(checked(act("screens.layout.three"), Check::Radio(false))),
            needs,
        ),
        act("layout.save_as"),
        act("layout.delete"),
        act("layout.reset"),
        Item::Separator,
        Item::Band(Src::Key("screens.view.band")),
        act("screens.view.new_window"),
        act("screens.view.gather"),
        act("screens.view.dock_all"),
        act("screens.view.map"),
        with_key(act("screens.window.full_screen"), "F11"),
    ]
}

/// The window's own menu (ADR 0151 §4).
fn window_rows() -> Vec<Item<'static>> {
    vec![
        checked(act("screens.window.bar"), Check::On),
        act("screens.window.bar_groups"),
        checked(act("screens.window.on_top"), Check::Off),
        with_key(act("screens.window.full_screen"), "F11"),
        act("screens.window.rename"),
        Item::Separator,
        act("screens.window.back"),
        act("screens.window.close"),
    ]
}

fn run(dir: &Path) {
    let lang = match cantus_ui::i18n::locale() {
        cantus_ui::i18n::Locale::Es => "es",
        _ => "en",
    };
    let out = |name: &str| dir.join(format!("{name}-{lang}.png"));
    let mut r = Rig::new();
    let w = r.app.project().content_end();
    let beats = w.0 as f32 / cantus_core::Ticks::beats(1).0 as f32;
    r.app.arrangement.px_per_beat = 1250.0 / beats.max(1.0);
    r.app.arrangement.scroll_beats = 0.0;

    // The main window on the laptop: the arrangement over the piano roll and the rack —
    // the mixer has its own window on the other screen.
    r.app.workspace.hide(Pane::Mixer);
    r.app.workspace.hide(Pane::Routing);
    r.app.workspace.reveal(Pane::PianoRoll);
    r.ctx.set_zoom_factor(LAPTOP_ZOOM);
    let mut main = |app: &mut CantusApp, ui: &mut egui::Ui| app.frame(ui);
    r.passes(LAPTOP, 16, &mut main);
    let main_png = out("02-main-window-laptop-200");
    r.shoot(&main_png);

    // The tab's menu on the piano roll's tab, *Move to* hovered and open.
    let tab = r
        .text_at(&t("workspace.pane.piano_roll"))
        .unwrap_or(Rect::from_min_size(pos2(300.0, 500.0), vec2(60.0, 14.0)));
    let (rows, subs) = tab_rows();
    let s = Style::current();
    let rows_w = cantus_ui::kit::menu::width(&r.ctx, &rows, &s);
    let subs_w = cantus_ui::kit::menu::width(&r.ctx, &subs, &s);
    let at = tab.left_bottom() + vec2(0.0, 6.0);
    let first = std::cell::Cell::new(Rect::NOTHING);
    let mut tab_menu = |app: &mut CantusApp, ui: &mut egui::Ui| {
        app.frame(ui);
        let ctx = ui.ctx().clone();
        let m = menu(&ctx, "tab", at, &rows, rows_w);
        first.set(m);
        let row = cantus_ui::kit::menu::row_in(m, &rows, &Style::current(), 1);
        let _ = menu(
            &ctx,
            "tab-sub",
            pos2(m.right() - 2.0, row.top() - 4.0),
            &subs,
            subs_w,
        );
    };
    r.passes(LAPTOP, 2, &mut tab_menu);
    r.pointer = row_centre(first.get(), &rows, 1);
    r.passes(LAPTOP, 8, &mut tab_menu);
    r.shoot(&out("04-tab-menu-move-to"));
    r.pointer = pos2(-10.0, -10.0);

    // The View menu's lower part, under *View*.
    let view = r
        .text_at(&t("menu.view"))
        .unwrap_or(Rect::from_min_size(pos2(120.0, 10.0), vec2(30.0, 14.0)));
    let vrows = view_rows();
    let vw = cantus_ui::kit::menu::width(&r.ctx, &vrows, &s);
    let mut view_menu = |app: &mut CantusApp, ui: &mut egui::Ui| {
        app.frame(ui);
        let ctx = ui.ctx().clone();
        let _ = menu(
            &ctx,
            "view",
            pos2(view.left() - 8.0, view.bottom() + 10.0),
            &vrows,
            vw,
        );
    };
    r.passes(LAPTOP, 8, &mut view_menu);
    r.shoot(&out("05-view-menu-windows"));

    // The screens map over the main window, both screens there; then with the second gone.
    let mut map = |app: &mut CantusApp, ui: &mut egui::Ui| {
        app.frame(ui);
        map_card(ui.ctx(), false);
    };
    r.passes(LAPTOP, 8, &mut map);
    r.shoot(&out("06-screens-map"));

    // The second screen unplugged: the mixer back in the main window's lower zone, the
    // status line saying so, the map with the screen dashed.
    r.app.workspace.reveal(Pane::Mixer);
    let notice = tf(
        "screens.notice.missing",
        &[
            ("name", &screen_name(2)),
            ("areas", &t("workspace.pane.mixer")),
        ],
    );
    r.app.set_status(notice);
    r.passes(LAPTOP, 12, &mut main);
    r.shoot(&out("07-screen-missing"));
    let mut map_gone = |app: &mut CantusApp, ui: &mut egui::Ui| {
        app.frame(ui);
        map_card(ui.ctx(), true);
    };
    r.passes(LAPTOP, 8, &mut map_gone);
    r.shoot(&out("08-screens-map-missing"));
    r.app.workspace.hide(Pane::Mixer);
    r.app.workspace.reveal(Pane::PianoRoll);

    // The mixer's own window on the 1 920-px screen at 100 %, at that screen's scale.
    r.ctx.set_zoom_factor(EXTERNAL_ZOOM);
    // A screen of its own for the desk: its strips Large.
    {
        use cantus_ui::views::mixer::layout::WidthMode;
        cantus_ui::views::mixer::set_width_mode(&mut r.app, WidthMode::from_index(2));
    }
    let dock = egui_dock::DockState::new(vec![Pane::Mixer, Pane::Routing]);
    let mut ws = Workspace::with_dock(dock);
    let mut win = |app: &mut CantusApp, ui: &mut egui::Ui| {
        let _ = mixer_window(app, ui, &mut ws, false);
    };
    r.passes(EXTERNAL, 16, &mut win);
    let mixer_png = out("03-mixer-window-external-100");
    r.shoot(&mixer_png);

    // Its menu open.
    let wrows = window_rows();
    let ww = cantus_ui::kit::menu::width(&r.ctx, &wrows, &s);
    let mut win_menu = |app: &mut CantusApp, ui: &mut egui::Ui| {
        let key = mixer_window(app, ui, &mut ws, true);
        let _ = menu(
            ui.ctx(),
            "window",
            key.left_bottom() + vec2(0.0, 6.0),
            &wrows,
            ww,
        );
    };
    r.passes(EXTERNAL, 8, &mut win_menu);
    r.shoot(&out("09-window-menu"));

    desk(&main_png, &mixer_png, &out("01-desk-two-screens"));
}

/// The desk: the laptop with the main window and the screen with the mixer's window, side
/// by side as they stand (`tiny-skia`, from the two pictures).
pub fn desk(main: &Path, mixer: &Path, to: &PathBuf) {
    use tiny_skia::{
        Color, FillRule, FilterQuality, Paint, PathBuilder, Pixmap, PixmapPaint, Transform,
    };
    let (Ok(a), Ok(b)) = (std::fs::read(main), std::fs::read(mixer)) else {
        return;
    };
    let (Ok(a), Ok(b)) = (Pixmap::decode_png(&a), Pixmap::decode_png(&b)) else {
        return;
    };
    let mut c = Pixmap::new(2400, 1080).unwrap();
    c.fill(Color::from_rgba8(38, 40, 46, 255));
    let fill = |c: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, r: f32, rgb: (u8, u8, u8)| {
        let mut p = PathBuilder::new();
        p.move_to(x + r, y);
        p.line_to(x + w - r, y);
        p.quad_to(x + w, y, x + w, y + r);
        p.line_to(x + w, y + h - r);
        p.quad_to(x + w, y + h, x + w - r, y + h);
        p.line_to(x + r, y + h);
        p.quad_to(x, y + h, x, y + h - r);
        p.line_to(x, y + r);
        p.quad_to(x, y, x + r, y);
        p.close();
        let mut paint = Paint::default();
        paint.set_color_rgba8(rgb.0, rgb.1, rgb.2, 255);
        paint.anti_alias = true;
        if let Some(path) = p.finish() {
            c.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    };
    let q = PixmapPaint {
        quality: FilterQuality::Bicubic,
        ..Default::default()
    };
    // The laptop: a 2 880 × 1 800 panel shown 1 100 px wide.
    let (lx, ly, lw) = (90.0, 160.0, 1100.0);
    let lh = lw * 1800.0 / 2880.0;
    fill(
        &mut c,
        lx - 22.0,
        ly - 22.0,
        lw + 44.0,
        lh + 44.0,
        18.0,
        (18, 19, 22),
    );
    fill(&mut c, lx, ly, lw, lh, 2.0, (12, 13, 15));
    let f = lw / a.width() as f32;
    c.draw_pixmap(
        0,
        0,
        a.as_ref(),
        &q,
        Transform::from_row(f, 0.0, 0.0, f, lx, ly),
        None,
    );
    fill(
        &mut c,
        lx,
        ly + lh - 48.0 * f * 2.0,
        lw,
        48.0 * f * 2.0,
        0.0,
        (24, 26, 30),
    );
    fill(
        &mut c,
        lx - 90.0,
        ly + lh + 22.0,
        lw + 180.0,
        26.0,
        10.0,
        (70, 73, 80),
    );
    // The screen: 1 920 × 1 080 shown 1 000 px wide, on its stand.
    let (sx, sy, sw) = (1320.0, 210.0, 1000.0);
    let sh = sw * 1080.0 / 1920.0;
    fill(
        &mut c,
        sx + sw * 0.5 - 40.0,
        sy + sh,
        80.0,
        190.0,
        6.0,
        (55, 58, 64),
    );
    fill(
        &mut c,
        sx + sw * 0.5 - 170.0,
        sy + sh + 180.0,
        340.0,
        22.0,
        10.0,
        (62, 65, 72),
    );
    fill(
        &mut c,
        sx - 16.0,
        sy - 16.0,
        sw + 32.0,
        sh + 32.0,
        12.0,
        (18, 19, 22),
    );
    fill(&mut c, sx, sy, sw, sh, 2.0, (12, 13, 15));
    let g = sw / b.width() as f32;
    c.draw_pixmap(
        0,
        0,
        b.as_ref(),
        &q,
        Transform::from_row(g, 0.0, 0.0, g, sx, sy),
        None,
    );
    fill(
        &mut c,
        sx,
        sy + sh - 48.0 * g,
        sw,
        48.0 * g,
        0.0,
        (24, 26, 30),
    );
    if let Ok(bytes) = c.encode_png() {
        let _ = std::fs::write(to, bytes);
        println!("{}", to.display());
    }
}
