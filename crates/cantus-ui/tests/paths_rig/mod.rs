//! The whole window driven as a person drives it (round 39, ADR 0110): `CantusApp::assemble`
//! over a data folder of its own, real pointer and key events through egui's input, and a
//! count of what each path costs — **clicks** (every press of a mouse button: a double-click
//! is two, a drag is one), **keys** (every chord or character typed: Ctrl+S is one) and
//! **pointer travel** (the distance the pointer moved, in points, at 1600 × 900, from where
//! the path began).
//!
//! Things are found the way a person finds them: by the words painted on screen
//! ([`Rig::text`]), or, for a control that shows a glyph and no word, by pointing at each
//! widget of a region and reading the help line's name for it ([`Rig::control`]). Probing
//! moves no counted pointer: the count is the path's, not the search's.
#![allow(dead_code)]

use cantus_core::Project;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, Rect, Vec2};
use std::path::{Path, PathBuf};

/// The window's size: a laptop's 1600 × 900, as the other whole-window tests use.
pub const SIZE: Vec2 = Vec2::new(1600.0, 900.0);

/// What a path costs.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Count {
    pub clicks: u32,
    pub keys: u32,
    /// Points.
    pub travel: f32,
}

impl Count {
    /// Clicks and keys together: the actions a person takes.
    pub fn actions(&self) -> u32 {
        self.clicks + self.keys
    }
}

impl std::fmt::Display for Count {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} clicks, {} keys, {:.0} pt",
            self.clicks, self.keys, self.travel
        )
    }
}

/// One painted text: its words, where it lies, and the part of it that can be seen.
#[derive(Clone, Debug)]
pub struct Painted {
    pub text: String,
    pub rect: Rect,
    pub seen: Rect,
    /// Cut short by its room (the galley says so).
    pub elided: bool,
}

pub struct Rig {
    pub app: CantusApp,
    pub ctx: egui::Context,
    pub dir: PathBuf,
    _tmp: Option<tempfile::TempDir>,
    t: f64,
    pub pointer: Pos2,
    pub mods: Modifiers,
    pub texts: Vec<Painted>,
    pub count: Count,
    tex: cantus_ui::raster::Textures,
    shapes: Vec<egui::epaint::ClippedShape>,
}

/// A data folder of the test binary's own (anything that reads `CANTUS_DATA_DIR`), with a
/// configured copy: no first-run audio setup.
pub fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        configure(d.path());
        d
    })
    .path()
}

/// A data folder as a configured install leaves it: the audio set up, no start page.
pub fn configure(dir: &Path) {
    let path = dir.join("settings.json");
    if !path.exists() {
        std::fs::write(
            &path,
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false,"plugin_scan_at_start":false}"#,
        )
        .unwrap();
    }
}

impl Rig {
    /// The app assembled over a fresh data folder with `project` open.
    pub fn new(project: Project) -> Rig {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();
        let mut r = Rig::in_dir(&dir, Some(project));
        r._tmp = Some(tmp);
        r
    }

    /// The app assembled over `dir` (a second session over the first one's folder).
    pub fn in_dir(dir: &Path, project: Option<Project>) -> Rig {
        let _ = data();
        configure(dir);
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let files = Files::read(dir, Some(cantus_ui::i18n::Locale::En));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let app = CantusApp::assemble(
            &ctx,
            files,
            project,
            EngineHandle::dummy(),
            None,
            None,
            None,
            false,
        );
        let mut r = Rig {
            app,
            ctx,
            dir: dir.to_path_buf(),
            _tmp: None,
            t: 0.0,
            pointer: Pos2::new(SIZE.x * 0.4, SIZE.y * 0.3),
            mods: Modifiers::NONE,
            texts: Vec::new(),
            count: Count::default(),
            tex: Default::default(),
            shapes: Vec::new(),
        };
        r.idle(4);
        r
    }

    /// One frame with `events` (the pointer's place and the modifiers go first).
    pub fn frame(&mut self, events: Vec<Event>) {
        self.t += 1.0 / 60.0;
        let mut ev = vec![
            Event::ModifiersChanged(self.mods),
            Event::PointerMoved(self.pointer),
        ];
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events: ev,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        self.tex.update(&out.textures_delta);
        out.textures_delta.clear();
        self.texts.clear();
        for c in &out.shapes {
            walk(&c.shape, c.clip_rect, &mut self.texts);
        }
        self.shapes = out.shapes;
        if std::env::var("CANTUS_PATHS_DEBUG").is_ok() {
            let (clicked, hovered) = self.ctx.viewport(|v| {
                (
                    v.interact_widgets.clicked,
                    v.interact_widgets
                        .hovered
                        .iter()
                        .copied()
                        .collect::<Vec<_>>(),
                )
            });
            let dbl = self
                .ctx
                .input(|i| i.pointer.button_double_clicked(PointerButton::Primary));
            println!(
                "frame t={:.3} clicked={clicked:?} dbl={dbl} hovered={hovered:?}",
                self.t
            );
        }
    }

    /// The last frame as a PNG at `path` (for looking at what a path shows).
    pub fn shoot(&self, path: &Path) {
        let prims = self.ctx.tessellate(self.shapes.clone(), 1.0);
        let bg = self.ctx.global_style().visuals.panel_fill;
        let mut c = cantus_ui::raster::Canvas::new(SIZE.x as usize, SIZE.y as usize, bg);
        c.paint(&prims, &self.tex, 1.0);
        let _ = c.write_png(path);
    }

    /// [`Rig::shoot`] into `CANTUS_PATHS_SHOTS/<name>.png` when that folder is set.
    pub fn shot(&self, name: &str) {
        if let Ok(dir) = std::env::var("CANTUS_PATHS_SHOTS") {
            let _ = std::fs::create_dir_all(&dir);
            self.shoot(&Path::new(&dir).join(format!("{name}.png")));
        }
    }

    /// The window's close box: the frame in which the system asks the window to close
    /// (what is kept on exit is written then).
    pub fn close_window(&mut self) {
        self.t += 1.0 / 60.0;
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SIZE)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events: vec![Event::PointerMoved(self.pointer)],
            ..Default::default()
        };
        input
            .viewports
            .entry(egui::ViewportId::ROOT)
            .or_default()
            .events
            .push(egui::ViewportEvent::Close);
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        out.textures_delta.clear();
    }

    /// The rect of the dock's leaf that holds `pane` (as laid out last frame).
    pub fn leaf_of(&self, pane: cantus_ui::views::workspace::Pane) -> Option<Rect> {
        let d = &self.app.workspace.dock;
        let path = d.find_tab(&pane)?;
        d.leaf(path.node_path()).ok().map(|l| l.rect)
    }

    /// Where `pane`'s tab is: its title in its leaf's tab bar.
    pub fn tab(&self, pane: cantus_ui::views::workspace::Pane) -> Rect {
        let leaf = self.leaf_of(pane).expect("the area is in the dock");
        let bar = Rect::from_min_size(
            leaf.min - egui::vec2(0.0, 2.0),
            egui::vec2(leaf.width(), 30.0),
        );
        let title = cantus_ui::i18n::t(pane.title_key());
        self.must_in(&title, bar)
    }

    /// A wheel's notches over `p` (with `mods`): one click counted per notch.
    pub fn wheel(&mut self, p: Pos2, notches: f32, mods: Modifiers) {
        self.hover(p);
        let was = self.mods;
        self.mods = mods;
        self.frame(vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, notches),
            phase: egui::TouchPhase::Move,
            modifiers: mods,
        }]);
        self.mods = was;
        self.settle();
    }

    pub fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.frame(Vec::new());
        }
    }

    /// A fifth of a second: what an action opens has faded in.
    pub fn settle(&mut self) {
        self.idle(12);
    }

    /// Start counting a path here: the pointer where a person works, the count at zero.
    pub fn begin(&mut self, home: Pos2) {
        self.pointer = home;
        self.idle(2);
        self.count = Count::default();
    }

    /// The pointer moves to `p` (counted) and rests there a frame.
    pub fn hover(&mut self, p: Pos2) {
        let d = (p - self.pointer).length();
        self.count.travel += d;
        // A hand takes half a second to reach a new target (and egui counts clicks closer
        // than 0.6 s as one triple-click).
        if d > 2.0 {
            self.t += 0.5;
        }
        self.pointer = p;
        self.idle(1);
    }

    fn button(&mut self, p: Pos2, button: PointerButton, pressed: bool) {
        self.frame(vec![Event::PointerButton {
            pos: p,
            button,
            pressed,
            modifiers: self.mods,
        }]);
    }

    /// A click of `button` at `p`.
    pub fn press(&mut self, p: Pos2, button: PointerButton) {
        self.hover(p);
        self.count.clicks += 1;
        self.button(p, button, true);
        self.button(p, button, false);
        self.settle();
    }

    pub fn click(&mut self, p: Pos2) {
        self.press(p, PointerButton::Primary);
    }

    pub fn right_click(&mut self, p: Pos2) {
        self.press(p, PointerButton::Secondary);
    }

    /// Two clicks in quick succession: two clicks counted.
    pub fn double_click(&mut self, p: Pos2) {
        self.hover(p);
        self.count.clicks += 2;
        // A person's double-click: about a tenth of a second between the two.
        self.button(p, PointerButton::Primary, true);
        self.button(p, PointerButton::Primary, false);
        self.idle(4);
        self.button(p, PointerButton::Primary, true);
        self.button(p, PointerButton::Primary, false);
        self.settle();
    }

    /// Press at `a`, move to `b` in steps, let go: one click, the travel of both legs.
    pub fn drag(&mut self, a: Pos2, b: Pos2) {
        self.hover(a);
        self.count.clicks += 1;
        self.button(a, PointerButton::Primary, true);
        let steps = 8;
        for i in 1..=steps {
            let p = a + (b - a) * (i as f32 / steps as f32);
            self.count.travel += ((b - a) / steps as f32).length();
            self.pointer = p;
            self.idle(1);
        }
        self.button(b, PointerButton::Primary, false);
        self.settle();
    }

    /// A key (with `mods` held): one key counted.
    pub fn key(&mut self, key: Key, mods: Modifiers) {
        self.count.keys += 1;
        let was = self.mods;
        self.mods = mods;
        for pressed in [true, false] {
            self.frame(vec![Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers: mods,
            }]);
        }
        self.mods = was;
        self.settle();
    }

    /// Words typed: a key per character.
    pub fn type_text(&mut self, s: &str) {
        for ch in s.chars() {
            self.count.keys += 1;
            let key = Key::from_name(&ch.to_ascii_uppercase().to_string());
            let mut ev = Vec::new();
            if let Some(k) = key {
                ev.push(Event::Key {
                    key: k,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                });
            }
            ev.push(Event::Text(ch.to_string()));
            self.frame(ev);
            if let Some(k) = key {
                self.frame(vec![Event::Key {
                    key: k,
                    physical_key: None,
                    pressed: false,
                    repeat: false,
                    modifiers: Modifiers::NONE,
                }]);
            }
        }
        self.idle(2);
    }

    /// Where the words `s` are painted and seen (the topmost, painted last), if anywhere.
    pub fn text(&self, s: &str) -> Option<Rect> {
        self.texts
            .iter()
            .rev()
            .find(|p| p.text.trim() == s && p.seen.is_positive())
            .map(|p| p.seen)
    }

    /// Where painted words starting with `s` are seen (the topmost).
    pub fn text_starting(&self, s: &str) -> Option<Rect> {
        self.texts
            .iter()
            .rev()
            .find(|p| p.text.trim().starts_with(s) && p.seen.is_positive())
            .map(|p| p.seen)
    }

    /// [`Rig::text`], or a failure that lists what is painted.
    pub fn must(&self, s: &str) -> Rect {
        self.text(s).unwrap_or_else(|| {
            let all: Vec<&str> = self.texts.iter().map(|p| p.text.as_str()).collect();
            panic!("“{s}” is not on screen; painted: {all:?}")
        })
    }

    /// [`Rig::must`] among the words seen inside `region`.
    pub fn must_in(&self, s: &str, region: Rect) -> Rect {
        self.texts
            .iter()
            .rev()
            .find(|p| p.text.trim() == s && p.seen.is_positive() && region.contains_rect(p.seen))
            .map(|p| p.seen)
            .unwrap_or_else(|| panic!("“{s}” is not on screen in {region:?}"))
    }

    /// The help line's name for the control under `p` (the pointer is put back after,
    /// uncounted).
    pub fn hint_at(&mut self, p: Pos2) -> Option<String> {
        let was = self.pointer;
        self.pointer = p;
        self.idle(1);
        let name = cantus_ui::kit::hint::current(&self.ctx).map(|h| h.name);
        self.pointer = was;
        name
    }

    /// The interactive widgets drawn last frame whose centre is inside `region`, smallest
    /// first.
    pub fn widgets_in(&self, region: Rect) -> Vec<Rect> {
        let mut v: Vec<Rect> = self.ctx.viewport(|vp| {
            vp.prev_pass
                .widgets
                .layers()
                .flat_map(|(_, ws)| ws.iter())
                .filter(|w| w.sense.interactive() && region.contains(w.interact_rect.center()))
                .map(|w| w.interact_rect)
                .collect()
        });
        v.sort_by(|a, b| a.area().total_cmp(&b.area()));
        v.dedup();
        v
    }

    /// The control in `region` whose hint names `want` (contains it, any case): where it
    /// is. Each widget is pointed at once, uncounted.
    pub fn control(&mut self, region: Rect, want: &str) -> Option<Rect> {
        let want = want.to_lowercase();
        for r in self.widgets_in(region) {
            if let Some(name) = self.hint_at(r.center()) {
                if name.to_lowercase().contains(&want) {
                    return Some(r);
                }
            }
        }
        None
    }

    /// Every widget in `region` with its hint (pointed at once each, uncounted).
    pub fn hints_in(&mut self, region: Rect) -> Vec<(Rect, Option<String>)> {
        let mut out = Vec::new();
        for r in self.widgets_in(region) {
            let h = self.hint_at(r.center());
            out.push((r, h));
        }
        out
    }

    /// The control in `region` whose hint name is exactly `want`.
    pub fn control_named(&mut self, region: Rect, want: &str) -> Rect {
        for r in self.widgets_in(region) {
            if self.hint_at(r.center()).as_deref() == Some(want) {
                return r;
            }
        }
        let seen = self.hints_in(region);
        panic!("no control named “{want}” in {region:?}; found {seen:?}")
    }

    /// [`Rig::control`], or a failure that lists the hints found there.
    pub fn must_control(&mut self, region: Rect, want: &str) -> Rect {
        if let Some(r) = self.control(region, want) {
            return r;
        }
        let mut seen = Vec::new();
        for r in self.widgets_in(region) {
            seen.push((r, self.hint_at(r.center())));
        }
        panic!("no control “{want}” in {region:?}; found {seen:?}")
    }
}

fn walk(shape: &egui::Shape, clip: Rect, out: &mut Vec<Painted>) {
    match shape {
        egui::Shape::Text(t) => {
            let rect = t.galley.rect.translate(t.pos.to_vec2());
            out.push(Painted {
                text: t.galley.text().to_string(),
                rect,
                seen: rect.intersect(clip),
                elided: t.galley.elided,
            });
        }
        egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
        _ => {}
    }
}

pub const NONE: Modifiers = Modifiers::NONE;
/// Ctrl as Windows and Linux report it: `ctrl` and egui's `command` both set.
pub const CTRL: Modifiers = Modifiers {
    alt: false,
    ctrl: true,
    shift: false,
    mac_cmd: false,
    command: true,
};
pub const SHIFT: Modifiers = Modifiers::SHIFT;
pub const ALT: Modifiers = Modifiers::ALT;

/// Ctrl+Shift (⌘⇧ on a Mac).
pub fn ctrl_shift() -> Modifiers {
    CTRL | Modifiers::SHIFT
}
