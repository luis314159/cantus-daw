//! The splash (ADR 0024 §12), like FL's: the window **starts as the splash** — 520 × 320,
//! no decorations, centred — showing the logo, the wordmark, the version and a progress
//! line, while the application opens behind it; then the same window turns into the main
//! one.
//!
//! - **What runs where.** The files (settings, the look, the layouts, the keymap), the
//!   project's parsing and the plug-in database are read on a thread; so is the MIDI scan
//!   (701 ms measured with no ports at all). The fonts and the audio stream open on the
//!   main thread, one stage per frame, so the splash keeps drawing.
//! - **The logo.** The first launch shows #2 as designed, baked into the executable at
//!   build time at 1× and 2× (`crates/cantus-app/build.rs`), so the first frame needs no
//!   rendering. From the second launch on the user's chosen logo and tint are shown, from
//!   the images the running application leaves in its cache ([`remember_logo`]).
//! - **The application** is `enum Shell { Booting, Running }`: the same `eframe::App`
//!   paints the splash, then the application.

use crate::app::{self, CantusApp, Files};
use crate::kit::color::{shade, with_alpha};
use crate::kit::logo::{self, LogoChoice};
use crate::kit::text::{self, Elide, Policy, Role, Text};
use crate::kit::{with_brush, Fill, Style, Tokens};
use crate::{i18n, LaunchOptions};
use cantus_core::Project;
use cantus_engine::{EngineHandle, MidiInputs};
use egui::{Align2, ColorImage, Rect, TextureHandle, TextureOptions, Vec2};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

/// The splash window's size, in points.
pub const SIZE: Vec2 = Vec2::new(520.0, 320.0);
/// The logo's side on the splash, in points (baked at 1× and 2× of this).
pub const LOGO: f32 = 184.0;

/// An image baked into the executable: its side in pixels and its RGBA bytes.
#[derive(Copy, Clone, Debug)]
pub struct Baked {
    pub side: u32,
    /// Premultiplied RGBA for the splash; straight RGBA for the icon.
    pub rgba: &'static [u8],
}

/// What the executable brings baked in (ADR 0024 §12).
#[derive(Copy, Clone, Debug)]
pub struct Art {
    /// Logo #2 at 1× and 2× of [`LOGO`].
    pub logo_1x: Baked,
    pub logo_2x: Baked,
    /// #2's small master at 32 px: the window's and the taskbar's icon.
    pub icon: Baked,
}

/// The application: the splash while it opens, then itself.
#[allow(clippy::large_enum_variant)]
pub enum Shell {
    Booting(Boot),
    Running(Box<CantusApp>),
}

/// What the thread reads.
struct Read {
    files: Files,
    project: Option<(Project, PathBuf)>,
    plugins: cantus_host::PluginDb,
}

/// One step of opening, one per frame.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Stage {
    Start,
    Fonts,
    Files,
    Audio,
    Midi,
    Assemble,
}

impl Stage {
    fn key(self) -> &'static str {
        match self {
            Stage::Start | Stage::Fonts | Stage::Files => "workspace.splash.files",
            Stage::Audio => "kit.splash.audio",
            Stage::Midi => "kit.splash.midi",
            Stage::Assemble => "kit.splash.song",
        }
    }

    fn progress(self) -> f32 {
        match self {
            Stage::Start => 0.05,
            Stage::Fonts => 0.15,
            Stage::Files => 0.3,
            Stage::Audio => 0.5,
            Stage::Midi => 0.7,
            Stage::Assemble => 0.9,
        }
    }
}

/// The splash's state.
pub struct Boot {
    options: LaunchOptions,
    stage: Stage,
    fonts: bool,
    read: Option<Receiver<Read>>,
    got: Option<Read>,
    engine: Option<EngineHandle>,
    midi: Option<Receiver<MidiInputs>>,
    midi_got: Option<Option<MidiInputs>>,
    logo: Option<TextureHandle>,
    owner: Option<raw_window_handle::RawWindowHandle>,
    started: Instant,
}

impl Boot {
    pub fn new(cc: &eframe::CreationContext<'_>, options: LaunchOptions) -> Boot {
        let owner = {
            use raw_window_handle::HasWindowHandle;
            cc.window_handle().ok().map(|h| h.as_raw())
        };
        // The splash's own look, whatever the theme: Slate, FL's dark.
        crate::kit::tokens::install(Tokens::slate());
        let logo = splash_image(&cc.egui_ctx, options.art.as_ref());
        Boot {
            options,
            stage: Stage::Start,
            fonts: false,
            read: None,
            got: None,
            engine: None,
            midi: None,
            midi_got: None,
            logo,
            owner,
            started: Instant::now(),
        }
    }

    /// Take one step. Returns the application once everything is in.
    fn step(&mut self, ctx: &egui::Context) -> Option<CantusApp> {
        match self.stage {
            Stage::Start => {
                let (tx, rx) = channel();
                let dir = app::config_dir();
                let locale = self
                    .options
                    .locale
                    .as_deref()
                    .and_then(i18n::Locale::from_code);
                let path = self.options.project.clone();
                let ctx2 = ctx.clone();
                std::thread::spawn(move || {
                    let files = Files::read(&dir, locale);
                    let project = path.and_then(|p| match cantus_project::load(&p, true) {
                        Ok(l) => Some((l.project, p)),
                        Err(e) => {
                            log::error!("could not open {}: {e}", p.display());
                            None
                        }
                    });
                    let plugins =
                        cantus_host::PluginDb::load(&app::data_dir().join("plugins.json"));
                    let _ = tx.send(Read {
                        files,
                        project,
                        plugins,
                    });
                    ctx2.request_repaint();
                });
                self.read = Some(rx);
                self.stage = Stage::Fonts;
            }
            Stage::Fonts => {
                crate::design::install_fonts(ctx);
                crate::design::install_style(ctx);
                self.fonts = true;
                self.stage = Stage::Files;
            }
            Stage::Files => {
                if let Some(r) = self.read.as_ref().and_then(|rx| rx.try_recv().ok()) {
                    self.got = Some(r);
                    self.stage = Stage::Audio;
                }
            }
            Stage::Audio => {
                let settings = &self.got.as_ref()?.files.settings;
                let engine = app::start_engine(settings);
                if engine.is_live() {
                    let (tx, rx) = channel();
                    let sender = engine.midi_sender();
                    let ctx2 = ctx.clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(MidiInputs::open_all(sender));
                        ctx2.request_repaint();
                    });
                    self.midi = Some(rx);
                } else {
                    self.midi_got = Some(None);
                }
                self.engine = Some(engine);
                self.stage = Stage::Midi;
            }
            Stage::Midi => {
                if self.midi_got.is_none() {
                    if let Some(m) = self.midi.as_ref().and_then(|rx| rx.try_recv().ok()) {
                        self.midi_got = Some(Some(m));
                    }
                }
                if self.midi_got.is_some() {
                    self.stage = Stage::Assemble;
                }
            }
            Stage::Assemble => {
                let read = self.got.take()?;
                let engine = self.engine.take()?;
                let midi = self.midi_got.take().flatten();
                let (project, path) = match read.project {
                    Some((p, path)) => (Some(p), Some(path)),
                    // Nothing to open: the program opens on a demo (round 19), so the first
                    // thing anybody hears is what Cantus can do.
                    None if !self.options.stress => (Some(crate::demos::startup_project()), None),
                    None => (None, None),
                };
                let mut app = CantusApp::assemble(
                    ctx,
                    read.files,
                    project,
                    engine,
                    midi,
                    Some(read.plugins),
                    self.owner,
                    self.options.stress,
                );
                if path.is_some() {
                    app.path = path;
                }
                self.options.apply_flags(&mut app);
                log::info!(
                    "opened in {:.0} ms",
                    self.started.elapsed().as_secs_f64() * 1000.0
                );
                return Some(app);
            }
        }
        None
    }
}

/// Paint the splash over `ui`'s whole rect: the logo, and — once the fonts are in — the
/// wordmark, the version and the step `key`; the progress bar at `progress` (0…1).
fn paint(ui: &mut egui::Ui, logo: Option<&TextureHandle>, key: &str, progress: f32, fonts: bool) {
    let rect = ui.max_rect();
    let s = Style::new(Tokens::slate());
    let t = s.t;
    let k = (rect.width() / SIZE.x).min(rect.height() / SIZE.y).max(0.1);
    let at = |x: f32, y: f32| rect.min + Vec2::new(x, y) * k;
    with_brush(ui.painter(), |b| {
        b.rect(rect, Fill::V(shade(t.ink, 0.04), shade(t.ink, -0.05)));
        b.rrect_soft(
            Rect::from_center_size(at(150.0, 150.0), Vec2::splat(300.0 * k)),
            150.0 * k,
            Fill::Solid(with_alpha(t.brand, 0.035)),
            220.0 * k,
        );
        let l = Rect::from_min_size(at(36.0, 44.0), Vec2::splat(LOGO * k));
        if let Some(tex) = logo {
            b.foreign(tex.id(), b.snap_rect(l));
        }
        if fonts {
            let wm = Rect::from_min_size(at(244.0, 96.0), Vec2::new(250.0, 56.0) * k);
            let _ = text::paint(
                b,
                wm,
                &Text::new("Cantus", Role::Section, t.text)
                    .caps(true)
                    .align(Align2::LEFT_CENTER),
                3.2 * k,
            );
            let v = Rect::from_min_size(at(248.0, 150.0), Vec2::new(240.0, 18.0) * k);
            let _ = text::paint(
                b,
                v,
                &Text::new(crate::APP_VERSION, Role::Value, t.text_dim).align(Align2::LEFT_CENTER),
                k,
            );
            let p = Rect::from_min_size(at(248.0, 238.0), Vec2::new(236.0, 16.0) * k);
            let _ = text::paint(
                b,
                p,
                &Text::key(key, Role::Body, t.text_dim)
                    .align(Align2::LEFT_CENTER)
                    .policy(Policy::Elide(Elide::End)),
                k,
            );
        }
        let bar = b.snap_rect(Rect::from_min_size(
            at(248.0, 260.0),
            Vec2::new(236.0, 4.0) * k,
        ));
        b.rrect(bar, 2.0 * k, Fill::Solid(with_alpha(t.text_dim, 0.25)));
        let done = Rect::from_min_size(
            bar.min,
            Vec2::new(bar.width() * progress.clamp(0.0, 1.0), bar.height()),
        );
        b.rrect(b.snap_rect(done), 2.0 * k, Fill::Solid(t.brand));
        b.rrect_stroke(
            b.snap_rect(rect).shrink(b.px() * 0.5),
            0.0,
            b.px(),
            with_alpha(t.text, 0.08),
        );
    });
}

/// The splash as the screenshots show it: the logo rendered on the spot, the MIDI scan
/// under way.
pub fn paint_splash(root: &mut egui::Ui, progress: f32) {
    let ctx = root.ctx().clone();
    let id = egui::Id::new("cantus-splash-shot");
    let tex = ctx.data(|d| d.get_temp::<TextureHandle>(id)).or_else(|| {
        let t = splash_image(&ctx, None)?;
        ctx.data_mut(|d| d.insert_temp(id, t.clone()));
        Some(t)
    });
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(root, |ui| {
            paint(ui, tex.as_ref(), "kit.splash.midi", progress, true)
        });
}

impl eframe::App for Shell {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        match self {
            Shell::Running(app) => app.frame(root),
            Shell::Booting(boot) => {
                let (key, progress, fonts) = (boot.stage.key(), boot.stage.progress(), boot.fonts);
                let logo = boot.logo.clone();
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(root, |ui| paint(ui, logo.as_ref(), key, progress, fonts));
                if let Some(app) = boot.step(&ctx) {
                    become_main_window(&ctx, &boot.options);
                    *self = Shell::Running(Box::new(app));
                }
                ctx.request_repaint_after(Duration::from_millis(16));
            }
        }
    }
}

/// The splash turns into the main window: decorations back, resizable, the saved size or
/// maximised.
fn become_main_window(ctx: &egui::Context, options: &LaunchOptions) {
    use egui::ViewportCommand as C;
    ctx.send_viewport_cmd(C::Title(i18n::t("app.title")));
    ctx.send_viewport_cmd(C::Decorations(true));
    ctx.send_viewport_cmd(C::Resizable(true));
    ctx.send_viewport_cmd(C::MinInnerSize(Vec2::new(960.0, 600.0)));
    match options.window {
        Some((w, h)) => ctx.send_viewport_cmd(C::InnerSize(Vec2::new(w, h))),
        None => {
            ctx.send_viewport_cmd(C::InnerSize(Vec2::new(1280.0, 800.0)));
            ctx.send_viewport_cmd(C::Maximized(true));
        }
    }
}

/// Where the running application leaves the splash's images of the chosen logo.
fn cache_path(scale: u32) -> PathBuf {
    app::config_dir()
        .join("cache")
        .join(format!("splash-{scale}x.png"))
}

/// The splash's logo: the user's, from the cache the application left; else the one baked
/// into the executable; else rendered now (tests, and builds without the art).
fn splash_image(ctx: &egui::Context, art: Option<&Art>) -> Option<TextureHandle> {
    let two = ctx.pixels_per_point() > 1.25;
    let scale = if two { 2 } else { 1 };
    let img = read_png(&cache_path(scale))
        .or_else(|| {
            art.map(|a| {
                let baked = if two { a.logo_2x } else { a.logo_1x };
                let side = baked.side as usize;
                ColorImage::from_rgba_premultiplied([side, side], baked.rgba)
            })
        })
        .or_else(|| {
            let px = (LOGO * scale as f32) as u32;
            let t = Tokens::slate();
            let svg = logo::svg_for(logo::Logo::DEFAULT, logo::Tint::Original, &t, px);
            logo::render_svg(&svg, px)
        })?;
    Some(ctx.load_texture("cantus-splash-logo", img, TextureOptions::LINEAR))
}

fn read_png(path: &Path) -> Option<ColorImage> {
    let bytes = std::fs::read(path).ok()?;
    let pix = tiny_skia::Pixmap::decode_png(&bytes).ok()?;
    Some(ColorImage::from_rgba_premultiplied(
        [pix.width() as usize, pix.height() as usize],
        pix.data(),
    ))
}

/// Leave the chosen logo where the next launch's splash finds it (on a worker: the
/// detailed logos take tens of milliseconds to render). The default, as designed, needs
/// nothing: it is baked into the executable.
pub fn remember_logo(choice: LogoChoice, tokens: &Tokens) {
    let dir = app::config_dir().join("cache");
    if choice.is_default() {
        for scale in [1, 2] {
            let _ = std::fs::remove_file(cache_path(scale));
        }
        return;
    }
    let t = *tokens;
    std::thread::spawn(move || {
        let _ = std::fs::create_dir_all(&dir);
        for scale in [1u32, 2] {
            let px = (LOGO * scale as f32) as u32;
            let svg = logo::svg_for(choice.logo(), choice.tint, &t, px);
            let Some(img) = logo::render_svg(&svg, px) else {
                continue;
            };
            let Some(mut pix) = tiny_skia::Pixmap::new(px, px) else {
                continue;
            };
            for (d, s) in pix.pixels_mut().iter_mut().zip(&img.pixels) {
                if let Some(c) =
                    tiny_skia::PremultipliedColorU8::from_rgba(s.r(), s.g(), s.b(), s.a())
                {
                    *d = c;
                }
            }
            if let Ok(bytes) = pix.encode_png() {
                let _ = std::fs::write(cache_path(scale), bytes);
            }
        }
    });
}

/// The window's icon (the taskbar's too), from the art baked into the executable.
pub fn icon(art: Option<&Art>) -> Option<egui::IconData> {
    let a = art?;
    Some(egui::IconData {
        rgba: a.icon.rgba.to_vec(),
        width: a.icon.side,
        height: a.icon.side,
    })
}
