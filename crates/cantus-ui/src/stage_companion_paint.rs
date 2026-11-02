//! The companion window drawn by tiny-skia into one pixmap of its exact pixel size (round
//! 35, ADR 0091 §2): what Windows shows through a layered window with true per-pixel
//! alpha (`cantus_rt::window::Layered`), and what the tests and the shots read.
//!
//! The same parts and look as the companion drawn by egui elsewhere: the backdrop
//! (nothing, the theme's stage, a chroma key), the dancers through the one renderer's
//! placement, the visualiser — glass chips along the top, the spectrum or the level along
//! the bottom with the feet just above it — and, on hover outside streaming mode, a thin
//! frame, the ⋯ menu button and the resize grip.

use super::companion::{Backdrop, StageSettings, VisMode, VisSnap, STANDARD};
use super::vis_video::{circle, fill, mix, rect, rrect, stroke, with, Painter, Setup};
use cantus_dance::character::Character;
use cantus_dance::pose::Pose;
use cantus_dance::render::{placement, Background, Canvas, Stage};
use std::sync::Arc;
use tiny_skia::{Color, Pixmap, Rect};

/// The chips' plates and words, drawn once and kept until they change (a song's name, the
/// bar's number, the window's size): a frame lays the kept pixels over and draws only what
/// moves (the beat dots, the bars, the dancers).
#[derive(Default)]
pub struct ChipCache {
    key: Option<(String, i64, u32, u32, bool, bool, u32)>,
    pm: Option<Pixmap>,
}

/// What one drawing of the window needs.
pub struct Scene<'a> {
    pub settings: &'a StageSettings,
    pub setup: &'a Setup,
    pub dancers: &'a [(Arc<dyn Character>, Pose)],
    /// The dancers' feet are placed by the window (the chosen character); else they keep
    /// their own `y` (the project's dancers).
    pub place_feet: bool,
    pub vis: &'a VisSnap,
    pub hover: bool,
    /// Physical pixels per logical pixel (the monitor's scale).
    pub scale: f32,
}

/// Where the parts went (pixels): the ⋯ button, if drawn.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Drawn {
    pub button: Option<(i32, i32, i32, i32)>,
}

/// Draw `scene` into `pm` (cleared first).
pub fn draw(pm: &mut Pixmap, scene: &Scene<'_>) -> Drawn {
    draw_cached(pm, scene, &mut ChipCache::default())
}

/// [`draw`], the chips kept in `cache` between frames.
pub fn draw_cached(pm: &mut Pixmap, scene: &Scene<'_>, cache: &mut ChipCache) -> Drawn {
    let (w, h) = (pm.width() as f32, pm.height() as f32);
    let k = scene.scale.max(0.5);
    let st = scene.settings;
    let s = scene.setup;
    let m = 12.0 * k;
    let painter = Painter { setup: s };

    // The visualiser's places first: the feet stand just above the band.
    let band = (st.vis != VisMode::Off).then(|| {
        let bh = (h * 0.16).clamp(40.0 * k, 120.0 * k);
        let bh = if st.vis == VisMode::Level {
            bh.min(22.0 * k)
        } else {
            bh
        };
        rect(m, h - m - bh, w - 2.0 * m, bh)
    });
    let feet_y = band.map_or(h - 18.0 * k, |b| b.top() - 10.0 * k);

    // The backdrop.
    match st.backdrop {
        Backdrop::Transparent => pm.fill(Color::TRANSPARENT),
        Backdrop::Theme => ground(pm, s, feet_y),
        c => {
            let [r, g, b, _] = c.chroma().unwrap_or([0, 0, 0, 255]);
            pm.fill(Color::from_rgba8(r, g, b, 255));
        }
    }

    // The dancers, through the renderer's own placement.
    let standard = standard_of(w, h, scene.dancers);
    let stage = Stage {
        width: pm.width(),
        height: pm.height(),
        background: Background::Transparent,
        standard_height: standard,
        // A contact shadow over a chroma key would be keyed as a smudge.
        shadows: st.backdrop.chroma().is_none(),
    };
    for (ch, pose) in scene.dancers {
        let mut p = *pose;
        if scene.place_feet {
            p.y = (feet_y / h.max(1.0)).clamp(0.0, 1.0);
        }
        if !p.visible {
            continue;
        }
        let (t, ppu) = placement(&stage, ch.as_ref(), &p);
        let mut canvas = Canvas {
            pixmap: pm,
            transform: t,
            px_per_unit: ppu,
            hue: p.hue,
            shadow: stage.shadows,
        };
        ch.draw(&p, &mut canvas);
    }

    // The visualiser.
    let v = scene.vis;
    if let Some(b) = band {
        match st.vis {
            VisMode::Spectrum => painter.spectrum(pm, b, k, &v.bands),
            VisMode::Level => level(&painter, pm, b, k, v.level),
            VisMode::Off => {}
        }
    }
    let chip_h = 40.0 * k;
    let bar_w = 108.0 * k;
    let bar_rect = rect(w - m - bar_w, m, bar_w, chip_h);
    let key = (
        format!("{}|{:.1}", v.song, v.bpm),
        v.bar,
        pm.width(),
        pm.height(),
        st.song,
        st.bar_beat,
        k.to_bits(),
    );
    if cache.key.as_ref() != Some(&key) || cache.pm.is_none() {
        let mut layer = Pixmap::new(pm.width(), pm.height());
        if let Some(l) = layer.as_mut() {
            if st.bar_beat {
                painter.bar_chip_still(l, bar_rect, k, v.bar);
            }
            if st.song {
                let right = if st.bar_beat {
                    bar_rect.left() - 8.0 * k
                } else {
                    w - m
                };
                let song_w = (right - m).min(260.0 * k);
                if song_w >= 60.0 * k {
                    painter.song_chip(l, rect(m, m, song_w, chip_h), k, f64::from(v.bpm));
                }
            }
        }
        cache.pm = layer;
        cache.key = Some(key);
    }
    if let Some(l) = cache.pm.as_ref() {
        pm.draw_pixmap(
            0,
            0,
            l.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::identity(),
            None,
        );
    }
    if st.bar_beat {
        painter.bar_dots(pm, bar_rect, k, v.beat, v.beats_per_bar, v.beat_phase);
    }

    // On hover: a thin frame, the menu's button, the grip.
    let mut drawn = Drawn::default();
    if scene.hover && !st.streaming {
        let frame = rect(k, k, w - 2.0 * k, h - 2.0 * k);
        stroke(pm, &rrect(frame, 14.0 * k), s.accent, 1.25 * k);
        let r = 13.0 * k;
        let (cx, cy) = (w - 22.0 * k, h * 0.5);
        fill(pm, &circle(cx, cy, r), with(s.menu, 0.95));
        for dx in [-5.0f32, 0.0, 5.0] {
            fill(pm, &circle(cx + dx * k, cy, 1.6 * k), with(s.text, 1.0));
        }
        drawn.button = Some((
            (cx - r) as i32,
            (cy - r) as i32,
            (2.0 * r) as i32,
            (2.0 * r) as i32,
        ));
        // The grip: three short diagonals in the corner, on a small plate — on a
        // see-through backdrop a pixel of alpha 0 lets the pointer through to the window
        // under it, so the whole corner the system sizes from has to be drawn.
        let g = 22.0 * k;
        fill(
            pm,
            &rrect(rect(w - g - k, h - g - k, g, g), 7.0 * k),
            with(s.menu, 0.9),
        );
        for i in 1..=3 {
            let d = i as f32 * 4.0 * k;
            let mut pb = tiny_skia::PathBuilder::new();
            pb.move_to(w - 4.0 * k - d, h - 4.0 * k);
            pb.line_to(w - 4.0 * k, h - 4.0 * k - d);
            if let Some(path) = pb.finish() {
                stroke(pm, &path, s.dim, 1.4 * k);
            }
        }
    }
    drawn
}

/// The companion's standard height (a share of its height) for `dancers` in a `w` × `h`
/// window. A tall narrow one keeps every character within its width: how wide it reaches
/// (Trino's arms up are wider than he is tall) sets the scale there, not the height. A
/// character's bounds hold every move with room to spare (a spin, a jump); a pose itself
/// reaches about 0.6 of their width.
pub(crate) fn standard_of(w: f32, h: f32, dancers: &[(Arc<dyn Character>, Pose)]) -> f32 {
    let reach = dancers
        .iter()
        .map(|(ch, p)| 0.6 * ch.bounds().width() * p.size.max(0.1))
        .fold(1.0f32, f32::max);
    (STANDARD * h).min(0.92 * w / reach) / h.max(1.0)
}

/// The theme's stage: its deepest ground, a soft spotlight from above in the accent, a
/// floor line fading to the sides where the feet are.
fn ground(pm: &mut Pixmap, s: &Setup, floor_y: f32) {
    let (w, h) = (pm.width() as f32, pm.height() as f32);
    pm.fill(Color::from_rgba8(s.ink[0], s.ink[1], s.ink[2], 255));
    let spot = tiny_skia::RadialGradient::new(
        tiny_skia::Point::from_xy(w * 0.5, -h * 0.15),
        0.0,
        tiny_skia::Point::from_xy(w * 0.5, -h * 0.15),
        h * 1.05,
        vec![
            tiny_skia::GradientStop::new(0.0, with(s.accent, 0.16)),
            tiny_skia::GradientStop::new(0.55, with(s.accent, 0.06)),
            tiny_skia::GradientStop::new(1.0, with(s.accent, 0.0)),
        ],
        tiny_skia::SpreadMode::Pad,
        tiny_skia::Transform::identity(),
    );
    if let Some(shader) = spot {
        let p = tiny_skia::Paint {
            shader,
            ..tiny_skia::Paint::default()
        };
        if let Some(r) = Rect::from_xywh(0.0, 0.0, w, h) {
            pm.fill_rect(r, &p, tiny_skia::Transform::identity(), None);
        }
    }
    let floor = mix(s.floor, s.accent, 0.25);
    let n = 24;
    for i in 0..n {
        let x0 = w * i as f32 / n as f32;
        let mid = ((i as f32 + 0.5) / n as f32 - 0.5).abs() * 2.0;
        let a = (1.0 - mid * mid) * 0.8;
        if let Some(r) = Rect::from_xywh(x0, floor_y, w / n as f32 + 0.5, (h / 600.0).max(1.0)) {
            let mut p = tiny_skia::Paint::default();
            p.set_color(with(floor, a));
            pm.fill_rect(r, &p, tiny_skia::Transform::identity(), None);
        }
    }
}

/// The master's level as one bar with round ends, its colour running along it.
fn level(painter: &Painter<'_>, pm: &mut Pixmap, r: Rect, k: f32, v: f32) {
    painter.plate(pm, r, k);
    let s = painter.setup;
    let inner = rect(
        r.left() + 8.0 * k,
        r.top() + 6.0 * k,
        r.width() - 16.0 * k,
        r.height() - 12.0 * k,
    );
    let rad = inner.height() * 0.5;
    fill(pm, &rrect(inner, rad), with(s.meter_well, 1.0));
    let lw = inner.width() * v.clamp(0.0, 1.0);
    if lw <= 0.5 {
        return;
    }
    let colour = |t: f32| {
        if t < 0.75 {
            mix(s.meter_low, s.meter_high, t / 0.75)
        } else {
            mix(s.meter_high, s.meter_clip, (t - 0.75) / 0.25)
        }
    };
    let n = 24;
    let full = rect(
        inner.left(),
        inner.top(),
        lw.max(inner.height()),
        inner.height(),
    );
    for i in 0..n {
        let a = full.left() + full.width() * i as f32 / n as f32;
        let z = full.left() + full.width() * (i + 1) as f32 / n as f32;
        let c = colour((z - inner.left()) / inner.width());
        let slice = rect(a, full.top(), z - a + 0.5, full.height());
        let path = if i == 0 || i == n - 1 {
            rrect(slice, rad.min(slice.width() * 0.5))
        } else {
            tiny_skia::PathBuilder::from_rect(slice)
        };
        fill(pm, &path, with(c, 1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage::companion::VisSnap;
    use cantus_dance::{cast, pose_at, DanceFrame};

    /// A vertical window (a phone's stream) scales the character by its reach, not its
    /// height: Trino with his arms up stays inside it; a square one keeps the standard.
    #[test]
    fn a_tall_narrow_window_keeps_the_character_within_its_width() {
        let ch = cast::default_character();
        let dancers = vec![(ch.clone(), Pose::default())];
        let (w, h) = (540.0, 960.0);
        let units = standard_of(w, h, &dancers) * h;
        assert!(0.6 * ch.bounds().width() * units <= 0.92 * w + 0.01);
        assert!(units < STANDARD * h);
        let square = standard_of(400.0, 460.0, &dancers);
        assert!((square - STANDARD).abs() < 1e-6);
    }

    fn scene_draw(backdrop: Backdrop, hover: bool) -> (Pixmap, Drawn) {
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        let ch = cast::default_character();
        let frame = DanceFrame {
            playing: true,
            beats: 9.5,
            seconds: 4.75,
            bar: 2,
            bar_start_beats: 8.0,
            ..DanceFrame::default()
        };
        let pose = pose_at(&frame, ch.as_ref(), 0.0);
        let st = StageSettings {
            companion_open: true,
            backdrop,
            ..StageSettings::default()
        };
        let setup = Setup::from_theme("Morning Light", false, backdrop.chroma().is_some());
        let mut vis = VisSnap::default();
        for (i, b) in vis.bands.iter_mut().enumerate() {
            *b = 0.2 + 0.6 * ((i % 5) as f32 / 4.0);
        }
        vis.playing = true;
        let dancers = vec![(ch, pose)];
        let scene = Scene {
            settings: &st,
            setup: &setup,
            dancers: &dancers,
            place_feet: true,
            vis: &vis,
            hover,
            scale: 1.25,
        };
        let mut pm = Pixmap::new(500, 575).unwrap();
        let d = draw(&mut pm, &scene);
        (pm, d)
    }

    #[test]
    fn see_through_its_corners_are_clear_and_its_soft_edges_are_soft() {
        let (pm, d) = scene_draw(Backdrop::Transparent, false);
        for (x, y) in [(0, 0), (499, 0), (0, 574), (499, 574)] {
            assert_eq!(pm.pixel(x, y).unwrap().alpha(), 0, "corner {x},{y}");
        }
        let soft = pm
            .pixels()
            .iter()
            .filter(|c| (8..=247).contains(&c.alpha()))
            .count();
        let solid = pm.pixels().iter().filter(|c| c.alpha() == 255).count();
        assert!(
            soft > 2_000,
            "soft shadows, glass and glows are partial alpha ({soft})"
        );
        assert!(
            solid > 20_000,
            "the character and the chips are drawn ({solid})"
        );
        assert!(d.button.is_none(), "no menu button without the pointer");
    }

    #[test]
    fn on_a_chroma_key_the_key_is_exact_and_on_hover_the_button_shows() {
        let (pm, _) = scene_draw(Backdrop::Green, false);
        let [r, g, b, _] = Backdrop::Green.chroma().unwrap();
        let c = pm.pixel(250, 300).unwrap();
        let corner = pm.pixel(2, 300).unwrap();
        assert_eq!(
            (corner.red(), corner.green(), corner.blue(), corner.alpha()),
            (r, g, b, 255)
        );
        assert!(c.alpha() == 255);
        let (a, d) = scene_draw(Backdrop::Theme, true);
        let (x, y, w, h) = d.button.expect("the menu's button on hover");
        assert!(w > 0 && h > 0 && x + w <= 500 && y + h <= 575);
        let (b2, _) = scene_draw(Backdrop::Theme, true);
        assert_eq!(a.data(), b2.data(), "the same scene, the same pixels");
    }
}
