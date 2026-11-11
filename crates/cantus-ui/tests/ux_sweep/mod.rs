//! What the help-line walk and the whole-words sweep share (round 39, ADR 0109): the app
//! `CantusApp::assemble` puts together on the demo it opens with, a frame of it with every
//! text painted and where, and every view a user reaches — the main window, each pane,
//! each instrument and effect editor, Settings, the first-run setup, the export dialog,
//! the top bar's menus.
#![allow(dead_code)]

use cantus_core::InstrumentKind;
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::i18n::Locale;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::views::workspace::Pane;
use cantus_ui::CantusApp;
use egui::{vec2, Event, Pos2, Rect, Vec2};

pub fn data() -> &'static std::path::Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        // A configured copy: no first-run audio setup over the window (it is a view of
        // its own here).
        std::fs::write(
            d.path().join("settings.json"),
            r#"{"audio_setup":{"setup_done":true},"start_page_at_start":false}"#,
        )
        .unwrap();
        d
    })
    .path()
}

/// A text painted this frame: what it says, where, and whether egui cut its rows short.
#[derive(Clone, Debug)]
pub struct Painted {
    pub text: String,
    pub rect: Rect,
    pub elided: bool,
}

pub struct Rig {
    pub app: CantusApp,
    pub ctx: egui::Context,
    /// The screen in points.
    pub size: Vec2,
    pub ppp: f32,
    t: f64,
    /// The textures every frame uploaded, and the last frame's shapes: what a shot paints.
    textures: cantus_ui::raster::Textures,
    last: Vec<egui::epaint::ClippedShape>,
}

/// The look a sweep runs in.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Look {
    pub locale: Locale,
    pub dark: bool,
    /// The interface's scale: 1.0 or 1.5 on a 1920 × 1080 screen.
    pub scale: f32,
}

impl Look {
    pub const PLAIN: Look = Look {
        locale: Locale::En,
        dark: true,
        scale: 1.0,
    };

    /// Every look the whole-words sweep runs: both languages, both themes, 100 and 150 %.
    pub fn all() -> Vec<Look> {
        let mut v = Vec::new();
        for locale in [Locale::En, Locale::Es] {
            for dark in [true, false] {
                for scale in [1.0, 1.5] {
                    v.push(Look {
                        locale,
                        dark,
                        scale,
                    });
                }
            }
        }
        v
    }
}

impl Rig {
    pub fn new(look: Look) -> Rig {
        cantus_ui::i18n::set_thread_locale(look.locale);
        let files = Files::read(data(), Some(look.locale));
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        let song = cantus_ui::demos::startup_project();
        let app = CantusApp::assemble(
            &ctx,
            files,
            Some(song),
            EngineHandle::dummy(),
            None,
            None,
            None,
            false,
        );
        cantus_ui::theme::apply(
            &if look.dark {
                cantus_ui::theme::slate()
            } else {
                cantus_ui::theme::paper()
            },
            &ctx,
        );
        // A 1920 × 1080 screen at the interface's scale.
        let size = vec2(1920.0, 1080.0) / look.scale;
        ctx.set_pixels_per_point(look.scale);
        let mut r = Rig {
            app,
            ctx,
            size,
            ppp: look.scale,
            t: 0.0,
            textures: Default::default(),
            last: Vec::new(),
        };
        r.idle(3);
        r
    }

    /// One frame with these events (the pointer at `pointer` first, when given): every
    /// text painted, with where it lies.
    pub fn frame_at(&mut self, pointer: Option<Pos2>, events: Vec<Event>) -> Vec<Painted> {
        self.t += 1.0 / 60.0;
        let mut ev: Vec<Event> = pointer.map(Event::PointerMoved).into_iter().collect();
        ev.extend(events);
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
            time: Some(self.t),
            predicted_dt: 1.0 / 60.0,
            events: ev,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut out = self.ctx.run_ui(input, |ui| app.frame(ui));
        self.textures.update(&out.textures_delta);
        out.textures_delta.clear();
        fn walk(shape: &egui::Shape, clip: Rect, out: &mut Vec<Painted>) {
            match shape {
                egui::Shape::Text(t) => {
                    let rect = t.galley.rect.translate(t.pos.to_vec2());
                    // Text wholly outside its clip is not seen (a row scrolled away).
                    if rect.intersects(clip) {
                        out.push(Painted {
                            text: t.galley.text().to_string(),
                            rect,
                            elided: t.galley.elided,
                        });
                    }
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
                _ => {}
            }
        }
        let mut texts = Vec::new();
        for c in &out.shapes {
            walk(&c.shape, c.clip_rect, &mut texts);
        }
        self.last = out.shapes;
        texts
    }

    pub fn frame(&mut self, pointer: Option<Pos2>) -> Vec<Painted> {
        self.frame_at(pointer, Vec::new())
    }

    pub fn idle(&mut self, n: usize) -> Vec<Painted> {
        let mut t = Vec::new();
        for _ in 0..n {
            t = self.frame(None);
        }
        t
    }

    pub fn click(&mut self, p: Pos2) {
        self.frame(Some(p));
        for pressed in [true, false] {
            self.frame_at(
                Some(p),
                vec![Event::PointerButton {
                    pos: p,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                }],
            );
        }
    }

    /// Esc, and everything opened put away: editors, dialogs, menus.
    pub fn reset(&mut self) {
        self.frame_at(
            None,
            vec![Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
        );
        self.app.close_fx_editor();
        self.app.inst.open = None;
        self.app.show_settings = false;
        self.app.audio_setup.first_run = None;
        self.app.workflow.start.close();
        // The window as it opens: each view is looked at on the default layout, not on
        // every pane an earlier view brought up.
        self.app.reset_layout();
        cantus_ui::views::export::close(&mut self.app);
        self.idle(2);
    }
}

/// A view a user reaches.
#[derive(Clone, Debug, PartialEq)]
pub enum View {
    /// The window as it opens.
    Main,
    /// A pane brought to the front.
    Pane(Pane),
    Instrument(InstrumentKind),
    /// An effect's editor, by the name `CantusApp::demo_editor` knows it by.
    Effect(&'static str),
    Settings,
    FirstRun,
    /// The start page: templates and recent projects (it opens at launch).
    StartPage,
    Export,
    /// The top bar's menu at this place.
    Menu(usize),
}

/// The effects `demo_editor` opens, one of each kind.
pub const EFFECTS: &[&str] = &[
    "eq",
    "comp",
    "limiter",
    "space",
    "multiband",
    "deesser",
    "gate",
    "stereo",
    "maximizer",
    "saturator",
    "concentus",
    "flexus",
    "phasis",
    "tremor",
    "vibrans",
    "anulus",
    "translatio",
    "fluctus",
    "tonsor",
    "impetus",
    "discrimen",
    "speculum",
    "tonus",
];

/// Every view, in the order a sweep visits them.
pub fn views() -> Vec<View> {
    let mut v = vec![View::Main];
    for p in [
        Pane::Arrangement,
        Pane::Mixer,
        Pane::ChannelRack,
        Pane::PianoRoll,
        Pane::Inspector,
        Pane::Browser,
        Pane::Devices,
        Pane::Clips,
        Pane::Routing,
        Pane::Controllers,
        Pane::History,
        Pane::Score,
        Pane::Session,
    ] {
        v.push(View::Pane(p));
    }
    v.extend(InstrumentKind::ALL.iter().map(|k| View::Instrument(*k)));
    v.extend(EFFECTS.iter().map(|e| View::Effect(e)));
    v.extend([
        View::Settings,
        View::FirstRun,
        View::StartPage,
        View::Export,
    ]);
    v.extend((0..MENUS).map(View::Menu));
    v
}

/// Bring `view` up (after [`Rig::reset`]) and let it settle.
pub fn show(r: &mut Rig, view: &View) {
    match view {
        View::Main => {}
        View::Pane(p) => r.app.show_pane(*p),
        View::Instrument(kind) => {
            r.app.apply_pick(
                PickPurpose::AddInstrument { index: None },
                PickChoice::Instrument(*kind),
            );
            let track = r.app.project().tracks.last().map(|t| t.id);
            if let Some(t) = track.filter(|t| r.app.inst.open != Some(*t)) {
                r.app.open_inst_editor(t);
            }
        }
        View::Effect(name) => {
            {
                if let Some(t) = r.app.project().tracks.first().map(|t| t.id) {
                    r.app.select_track(t);
                }
            }
            r.app.demo_editor(name);
        }
        View::Settings => r.app.show_settings = true,
        View::FirstRun => cantus_ui::audio_setup::arm_first_run(&mut r.app),
        View::StartPage => r.app.workflow.start.open(),
        View::Export => cantus_ui::views::export::open(&mut r.app, None),
        View::Menu(i) => {
            r.idle(2);
            if let Some(at) = widget_rect(&r.ctx, egui::Id::new(("toolbar-menu", *i))) {
                r.click(at.center());
            }
        }
    }
    r.idle(4);
}

/// The top bar's menus (`views::transport_menus`): File, Edit, … — six words.
pub const MENUS: usize = 6;

/// Where the widget `id` was last frame.
pub fn widget_rect(ctx: &egui::Context, id: egui::Id) -> Option<Rect> {
    ctx.viewport(|v| v.prev_pass.widgets.get(id).map(|w| w.interact_rect))
}

/// Every widget a pointer can act on last frame (it senses a click or a drag, is enabled,
/// and lies on the screen), by layer: its id and where it takes the pointer.
pub fn interactive(ctx: &egui::Context, screen: Rect) -> Vec<(egui::Id, egui::LayerId, Rect)> {
    ctx.viewport(|v| {
        let mut out = Vec::new();
        for (layer, ws) in v.prev_pass.widgets.layers() {
            for w in ws {
                let r = w.interact_rect.intersect(screen);
                if (w.sense.senses_click() || w.sense.senses_drag())
                    && w.enabled
                    && r.width() >= 2.0
                    && r.height() >= 2.0
                {
                    out.push((w.id, *layer, r));
                }
            }
        }
        out
    })
}

/// Whether the widget `id` takes the pointer now (nothing over it).
pub fn hovered(ctx: &egui::Context, id: egui::Id) -> bool {
    ctx.viewport(|v| v.interact_widgets.hovered.contains(&id))
}

/// The window as it is now, rendered on the CPU into `path` (a PNG), with the pointer at
/// `pointer` when given.
pub fn shoot(r: &mut Rig, path: &std::path::Path, pointer: Option<Pos2>) {
    r.frame(pointer);
    r.frame(pointer);
    let shapes = std::mem::take(&mut r.last);
    let prims = r.ctx.tessellate(shapes, r.ppp);
    let bg = r.ctx.global_style().visuals.panel_fill;
    let mut canvas = cantus_ui::raster::Canvas::new(
        (r.size.x * r.ppp).round() as usize,
        (r.size.y * r.ppp).round() as usize,
        bg,
    );
    canvas.paint(&prims, &r.textures, r.ppp);
    let _ = canvas.write_png(path);
}
