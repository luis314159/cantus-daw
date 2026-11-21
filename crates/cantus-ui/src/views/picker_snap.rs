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
    /// Whether the device has an editor to picture (the old Reverb, Firmus and hosted
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
            EffectKind::Chorus => "concentus",
            EffectKind::Flanger => "flexus",
            EffectKind::Phaser => "phasis",
            EffectKind::Tremolo => "tremor",
            EffectKind::Vibrato => "vibrans",
            EffectKind::RingMod => "anulus",
            EffectKind::FreqShift => "translatio",
            EffectKind::VolumeShaper => "fluctus",
            EffectKind::Clipper => "tonsor",
            EffectKind::Transient => "impetus",
            EffectKind::TransientSplit => "discrimen",
            EffectKind::Analyzer => "speculum",
            EffectKind::AutoTune => "tonus",
            EffectKind::Aula => "aula",                 // r39:conv
            EffectKind::AutoFilter => "cribrum",        // fx:contract
            EffectKind::Pedal => "fragor",              // fx:contract
            EffectKind::TimeShaper => "lapsus",         // fx:contract
            EffectKind::LoFi => "vetus",                // fx:contract
            EffectKind::Utility => "libra",             // fx:contract
            EffectKind::Board => "tabula",              // fx3:contract
            EffectKind::PitchShift => "saltus",         // fx3:contract
            EffectKind::Rotary => "rota",               // fx3:contract
            EffectKind::Amp => "arca",                  // fx3:contract
            EffectKind::Vocoder => "loquax",            // fxa:contract
            EffectKind::Glitch => "fractus",            // fxa:contract
            EffectKind::Shimmer => "caelum",            // fxa:contract
            EffectKind::BusCompressor => "vinculum",    // fxb:contract
            EffectKind::ResonanceSuppressor => "lenis", // fxb:contract
            EffectKind::Ducker => "cedo",               // fxb:contract
            EffectKind::Repair => "purus",              // fxc:contract
            EffectKind::Reference => "comparatio",      // fxc:contract
            EffectKind::Delay => "delay",               // r39:delay
            EffectKind::Enhancer => "nitor",            // r41:nitor
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

/// Room around an editor on the capture's canvas for its shadow.
pub const SHADOW_ROOM: Vec2 = Vec2::new(60.0, 40.0);

/// The capture's canvas: the largest effect editor at 100 % as it is designed now (the
/// faces' widths included — Comprimo's needle makes it 1260 wide), the instruments'
/// shell, and the room for the shadow.
pub fn canvas() -> Vec2 {
    let largest = cantus_core::EffectKind::ALL
        .iter()
        .map(|k| crate::views::fx::frame::designed_size(*k))
        .fold(crate::kit::shell::INSTRUMENT, |a, b| a.max(b));
    // The instruments open at their own size (Tympanum's 1 219 × 711).
    let largest = cantus_core::inst::InstrumentKind::ALL
        .iter()
        .map(|k| crate::views::inst::editor_size(*k))
        .fold(largest, |a, b| a.max(b));
    largest + SHADOW_ROOM
}

/// Draw `subject`'s editor off screen at `ppp` pixels per point and tessellate it, waiting
/// for its scenes' pictures (the shoots; the picker uses [`Capture`] a frame at a time).
/// `None` when the editor did not open.
pub fn capture(outer: &Context, subject: Subject, ppp: f32) -> Option<Frame> {
    let mut c = Capture::start(outer, subject, ppp);
    loop {
        if let Some(done) = c.step() {
            return done;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

/// The frames an editor is drawn before its picture is taken (its layout settles).
const FRAMES: u32 = 4;
/// How long a capture waits for its scenes' pictures before taking what is there.
const SCENE_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// An editor being drawn off screen (UI thread): [`FRAMES`] frames at once, then one a
/// call while a scene's picture — Chorda's piano, Tympanum's kit, built on the scenes'
/// worker — is on its way, so the UI never waits for it. Round 40: their thumbnails were
/// taken after four frames, before the pictures came, and showed an empty stage.
pub struct Capture {
    subject: Subject,
    ctx: Context,
    app: Box<CantusApp>,
    textures: Textures,
    frames: u32,
    /// Frames in a row that ended with no picture on its way.
    still: u32,
    body: Option<Rect>,
    since: std::time::Instant,
    background: egui::Color32,
}

impl Capture {
    /// Stage `subject`'s editor in a scratch application drawn as `outer` draws.
    pub fn start(outer: &Context, subject: Subject, ppp: f32) -> Capture {
        let ctx = twin(outer);
        ctx.set_pixels_per_point(ppp);
        Capture {
            subject,
            ctx,
            app: Box::new(stage(subject)),
            textures: Textures::default(),
            frames: 0,
            still: 0,
            body: None,
            since: std::time::Instant::now(),
            background: outer.style_of(outer.theme()).visuals.panel_fill,
        }
    }

    /// The device being pictured.
    pub fn subject(&self) -> Subject {
        self.subject
    }

    /// Whether a scene of the staged editor has a picture on its way.
    fn building(&self) -> bool {
        self.app.inst.chorda.scenes_building()
            || self.app.inst.tympanum_scenes.values().any(|s| s.building())
    }

    /// Draw the next frames: `None` while a picture is on its way, `Some` once the capture
    /// is done (`Some(None)`: the editor did not open). It is taken after a whole frame
    /// with nothing on its way — the frame a picture lands in already draws it.
    pub fn step(&mut self) -> Option<Option<Frame>> {
        loop {
            let out = self.run();
            self.frames += 1;
            self.still = if self.building() { 0 } else { self.still + 1 };
            let waited = self.since.elapsed() >= SCENE_WAIT;
            if self.frames >= FRAMES && (self.still >= 2 || waited) {
                return Some(self.take(out));
            }
            if self.frames >= FRAMES {
                return None;
            }
        }
    }

    /// One frame of the editor, its textures kept.
    fn run(&mut self) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, canvas())),
            time: Some(f64::from(self.frames) * 0.25),
            predicted_dt: 0.25,
            ..Default::default()
        };
        let (subject, app, body) = (self.subject, &mut self.app, &mut self.body);
        let mut out = self.ctx.run_ui(input, |root| {
            let c = root.ctx().clone();
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(root, |ui| match subject {
                    Subject::Instrument(kind) => {
                        *body = Some(Rect::from_min_size(
                            ui.max_rect().min,
                            crate::views::inst::editor_size(kind),
                        ));
                        crate::views::inst::window_in(app, ui);
                    }
                    Subject::Pulsus => {
                        *body = Some(Rect::from_min_size(
                            ui.max_rect().min,
                            crate::kit::shell::INSTRUMENT,
                        ));
                        crate::views::pulsus_editor::window_in(app, ui);
                    }
                    Subject::Effect(_) => {}
                });
            if let Subject::Effect(_) = subject {
                crate::views::eq_editor::window(app, &c);
                crate::views::comp_editor::window(app, &c);
                crate::views::fx::window(app, &c);
            }
        });
        self.textures.update(&out.textures_delta);
        out.textures_delta.clear();
        out
    }

    /// The last frame, cropped to the editor and tessellated.
    fn take(&mut self, out: egui::FullOutput) -> Option<Frame> {
        let (ctx, size) = (&self.ctx, canvas());
        let rect = match self.subject {
            Subject::Effect(kind) => {
                let k = crate::views::fx::frame::shell_of(kind).scale(ctx);
                Rect::from_min_size(
                    egui::Pos2::ZERO,
                    crate::views::fx::frame::designed_size(kind) * k,
                )
            }
            _ => self.body?,
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
            textures: std::mem::take(&mut self.textures),
            ppp,
            size: [w, h],
            crop,
            background: self.background,
        })
    }
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

#[cfg(test)]
mod tests {
    /// Every effect's editor, as designed now (Comprimo's face is 1260 wide), fits the
    /// capture's canvas with its shadow — its thumbnail is never clipped.
    #[test]
    fn every_editor_fits_the_capture_canvas() {
        let c = super::canvas();
        for kind in cantus_core::EffectKind::ALL {
            let d = crate::views::fx::frame::designed_size(kind) + super::SHADOW_ROOM;
            assert!(d.x <= c.x && d.y <= c.y, "{kind:?}: {d:?} in {c:?}");
        }
        assert!(c.x >= 1260.0 + super::SHADOW_ROOM.x, "{c:?}");
    }
}
