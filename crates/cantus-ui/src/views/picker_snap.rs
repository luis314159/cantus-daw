//! Real-look thumbnails (ADR 0029 §3): a native device's **own editor**, drawn once off
//! screen and shrunk to a picture — what FL Studio's plug-in picker shows, made from the
//! very painters the device uses, so a thumbnail can never drift from the device.
//!
//! The editor is drawn in a private egui context of its own, around a scratch application
//! (`CantusApp::from_parts` with the dummy engine) holding one track with the device set up
//! as its screenshot is (`demo_setup`, `demo_feed`). That part runs on the UI thread,
//! because the application is not `Send`; it costs a few frames of one editor. The
//! expensive part — filling the triangles on the CPU (`crate::raster`), cropping and
//! shrinking — is a pure function of the tessellated frame and runs on a worker
//! ([`finish`]).

use crate::app::CantusApp;
use crate::raster::{Canvas, Textures};
use cantus_core::{EffectKind, InstrumentKind, InstrumentSpec, NativeSpec};
use egui::epaint::ClippedPrimitive;
use egui::{ColorImage, Context, Rect, Vec2};

/// The thumbnails' proportions, width over height.
pub const ASPECT: f32 = 1.6;

/// What a real-look thumbnail can be made of: a device with an editor of its own.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Subject {
    Instrument(InstrumentKind),
    Pulsus,
    Effect(EffectKind),
}

impl Subject {
    /// Whether the device has an editor to picture (Delay, Reverb, Firmus and hosted
    /// plug-ins get a drawn card instead).
    pub fn of_effect(kind: EffectKind) -> Option<Subject> {
        kind.has_editor()
            .then_some(Subject::Effect(kind))
            .filter(|_| kind != EffectKind::External)
    }

    /// The name `CantusApp::demo_editor` knows an effect by.
    fn effect_name(kind: EffectKind) -> &'static str {
        match kind {
            EffectKind::Compressor => "comp",
            EffectKind::Limiter => "limiter",
            EffectKind::Space => "space",
            EffectKind::Multiband => "multiband",
            EffectKind::DeEsser => "deesser",
            EffectKind::Gate => "gate",
            EffectKind::Stereo => "stereo",
            EffectKind::Maximizer => "maximizer",
            EffectKind::Saturator => "saturator",
            _ => "eq",
        }
    }
}

/// A frame of an editor, tessellated and ready to be filled off the UI thread.
pub struct Frame {
    pub prims: Vec<ClippedPrimitive>,
    pub textures: Textures,
    pub ppp: f32,
    /// The canvas, in physical pixels.
    pub size: [usize; 2],
    /// The editor's rect inside the canvas, in physical pixels.
    pub crop: [usize; 4],
    pub background: egui::Color32,
}

/// Set a scratch application up so that `subject`'s editor is open, as its screenshot is.
fn stage(subject: Subject) -> CantusApp {
    let project = crate::app::starter_project();
    let mut app = CantusApp::from_parts(Some(project), cantus_engine::EngineHandle::dummy(), false);
    let track = app.project().tracks[0].id;
    app.selected_track = Some(track);
    match subject {
        Subject::Instrument(kind) => {
            app.set_instrument(track, InstrumentSpec::Native(NativeSpec::new(kind)));
            crate::views::inst::demo_setup(&mut app, track, kind);
            crate::views::inst::demo_feed(&mut app, kind);
            app.open_inst_editor(track);
        }
        Subject::Pulsus => crate::views::pulsus_editor::demo(&mut app, "pulsus"),
        Subject::Effect(kind) => {
            app.demo_editor(Subject::effect_name(kind));
            // Pinned to the top-left corner, so its rect is known without asking egui.
            app.fx.pos = Some(egui::Pos2::ZERO);
            if feeds(kind) {
                crate::views::fx::demo_feed(&mut app, kind);
            }
        }
    }
    app
}

/// Whether an effect's demo feed is cheap enough to run on the UI thread (some run the
/// real DSP over seconds of audio to fill a history; those are pictured idle).
fn feeds(kind: EffectKind) -> bool {
    !matches!(
        kind,
        EffectKind::Limiter | EffectKind::Maximizer | EffectKind::Space | EffectKind::DeEsser
    )
}

/// A context that draws as `outer` does: the fonts, the style, the light or dark visuals.
fn twin(outer: &Context) -> Context {
    let ctx = Context::default();
    crate::design::install_fonts(&ctx);
    crate::design::install_style(&ctx);
    let theme = outer.theme();
    ctx.set_theme(theme);
    ctx.set_visuals_of(theme, outer.style_of(theme).visuals.clone());
    ctx
}

/// Draw `subject`'s editor off screen at `ppp` pixels per point and tessellate it (UI
/// thread). `None` when the editor did not open.
pub fn capture(outer: &Context, subject: Subject, ppp: f32) -> Option<Frame> {
    let ctx = twin(outer);
    ctx.set_pixels_per_point(ppp);
    let mut app = stage(subject);
    // Room for the largest editor at 100 % (Aequalis, Culmen) and its shadow.
    let size = Vec2::new(1240.0, 820.0);
    let mut textures = Textures::default();
    let mut last = None;
    let mut body: Option<Rect> = None;
    for frame in 0..4 {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(frame as f64 * 0.25),
            predicted_dt: 0.25,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |root| {
            let c = root.ctx().clone();
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root, |ui| match subject {
                    Subject::Instrument(_) => {
                        body = Some(Rect::from_min_size(
                            ui.max_rect().min,
                            crate::kit::shell::INSTRUMENT,
                        ));
                        crate::views::inst::window_in(&mut app, ui);
                    }
                    Subject::Pulsus => {
                        body = Some(Rect::from_min_size(
                            ui.max_rect().min,
                            crate::kit::shell::INSTRUMENT,
                        ));
                        crate::views::pulsus_editor::window_in(&mut app, ui);
                    }
                    Subject::Effect(_) => {}
                });
            if let Subject::Effect(_) = subject {
                crate::views::eq_editor::window(&mut app, &c);
                crate::views::comp_editor::window(&mut app, &c);
                crate::views::fx::window(&mut app, &c);
            }
        });
        textures.update(&out.textures_delta);
        out.textures_delta.clear();
        last = Some(out);
    }
    let out = last?;
    let rect = match subject {
        Subject::Effect(kind) => {
            let k = crate::views::fx::frame::shell_of(kind).scale(&ctx);
            Rect::from_min_size(
                egui::Pos2::ZERO,
                crate::views::fx::frame::designed_size(kind) * k,
            )
        }
        _ => body?,
    };
    // The picture is 16:10 and keeps the top-left corner — the header with the device's
    // name — cutting the right side or the bottom (the help line) to fit.
    let rect = if rect.width() / rect.height() > ASPECT {
        Rect::from_min_size(rect.min, Vec2::new(rect.height() * ASPECT, rect.height()))
    } else {
        Rect::from_min_size(rect.min, Vec2::new(rect.width(), rect.width() / ASPECT))
    };
    let ppp = out.pixels_per_point;
    let prims = ctx.tessellate(out.shapes, ppp);
    let w = (size.x * ppp).round() as usize;
    let h = (size.y * ppp).round() as usize;
    let px = |v: f32, max: usize| ((v * ppp).round().max(0.0) as usize).min(max);
    let crop = [
        px(rect.left(), w),
        px(rect.top(), h),
        px(rect.right(), w),
        px(rect.bottom(), h),
    ];
    if crop[2] <= crop[0] + 8 || crop[3] <= crop[1] + 8 {
        return None;
    }
    Some(Frame {
        prims,
        textures,
        ppp,
        size: [w, h],
        crop,
        background: outer.style_of(outer.theme()).visuals.panel_fill,
    })
}

/// Fill a captured frame, crop it to the editor and shrink it to `width` pixels (keeping
/// its proportions) with a box filter, its corners rounded by `radius` pixels — safe to
/// run on any thread.
pub fn finish(frame: Frame, width: usize, radius: f32) -> ColorImage {
    let mut canvas = Canvas::new(frame.size[0], frame.size[1], frame.background);
    canvas.paint(&frame.prims, &frame.textures, frame.ppp);
    let [x0, y0, x1, y1] = frame.crop;
    let mut img = shrink(&canvas, x0, y0, x1 - x0, y1 - y0, width);
    round_corners(&mut img, radius);
    img
}

/// Make the corners outside a radius-`r` rounding transparent, anti-aliased over a pixel.
pub fn round_corners(img: &mut ColorImage, r: f32) {
    let [w, h] = img.size;
    if r <= 0.0 || w == 0 || h == 0 {
        return;
    }
    for y in 0..h {
        for x in 0..w {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let cx = px.clamp(r, w as f32 - r);
            let cy = py.clamp(r, h as f32 - r);
            let d = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();
            let cover = (r + 0.5 - d).clamp(0.0, 1.0);
            if cover < 1.0 {
                let c = &mut img.pixels[y * w + x];
                *c = c.gamma_multiply(cover);
            }
        }
    }
}

/// The region `(x0, y0, w, h)` of `canvas`, averaged down to `width` pixels across.
pub fn shrink(
    canvas: &Canvas,
    x0: usize,
    y0: usize,
    w: usize,
    h: usize,
    width: usize,
) -> ColorImage {
    let width = width.clamp(1, w.max(1));
    let height = ((h as f32 * width as f32 / w.max(1) as f32).round() as usize).max(1);
    let mut px = Vec::with_capacity(width * height * 4);
    for dy in 0..height {
        let sy0 = y0 + dy * h / height;
        let sy1 = (y0 + (dy + 1) * h / height).max(sy0 + 1);
        for dx in 0..width {
            let sx0 = x0 + dx * w / width;
            let sx1 = (x0 + (dx + 1) * w / width).max(sx0 + 1);
            let mut acc = [0u32; 4];
            let mut n = 0u32;
            for y in sy0..sy1 {
                for x in sx0..sx1 {
                    let p = canvas.pixel(x, y);
                    for c in 0..4 {
                        acc[c] += p[c] as u32;
                    }
                    n += 1;
                }
            }
            for a in acc {
                px.push((a / n.max(1)) as u8);
            }
        }
    }
    ColorImage::from_rgba_premultiplied([width, height], &px)
}
