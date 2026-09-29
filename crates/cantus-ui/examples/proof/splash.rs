//! (d) The splash, like FL's: logo #2 as designed, the wordmark, the version and a
//! progress line. (e) The ten logos, in their original colours and tinted by the theme,
//! with #2's small masters at 16, 24 and 32 px.

use cantus_ui::kit::color::{shade, with_alpha};
use cantus_ui::kit::logo::{self, Logo, Tint};
use cantus_ui::kit::text::{self, Elide, Policy, Role, Src, Text};
use cantus_ui::kit::{Brush, Fill, Style};
use egui::{Align2, Color32, Painter, Pos2, Rect, Vec2};

pub const SPLASH: Vec2 = Vec2::new(520.0, 320.0);
pub const SHEET: Vec2 = Vec2::new(1096.0, 404.0);

pub fn splash(b: &mut Brush, _p: &Painter, s: &Style, size: Vec2) {
    let t = &s.t;
    let r = Rect::from_min_size(Pos2::ZERO, size);
    // A full-bleed card: the deepest ground, lit a little from the top left.
    let (top, bottom) = if t.dark {
        (shade(t.ink, 0.04), shade(t.ink, -0.05))
    } else {
        (shade(t.panel, 0.06), shade(t.panel, -0.02))
    };
    b.rect(r, Fill::V(top, bottom));
    b.rrect_soft(
        Rect::from_center_size(Pos2::new(150.0, 150.0), Vec2::splat(300.0)),
        150.0,
        Fill::Solid(with_alpha(t.brand, if t.dark { 0.035 } else { 0.05 })),
        220.0,
    );
    let l = Rect::from_min_size(Pos2::new(36.0, 44.0), Vec2::splat(184.0));
    logo::paint(b, l, Logo::DEFAULT, Tint::Original, t, true);
    // The wordmark and the version.
    let ink = t.text;
    let wm = Rect::from_min_size(Pos2::new(244.0, 96.0), Vec2::new(250.0, 56.0));
    let _ = text::paint(
        b,
        wm,
        &Text::new("Cantus", Role::Section, ink)
            .caps(true)
            .align(Align2::LEFT_CENTER),
        3.2,
    );
    let v = Rect::from_min_size(Pos2::new(248.0, 150.0), Vec2::new(240.0, 18.0));
    let _ = text::paint(
        b,
        v,
        &Text::new("0.1.0", Role::Value, t.text_dim).align(Align2::LEFT_CENTER),
        1.0,
    );
    // Progress: the step, and a thin bar.
    let p = Rect::from_min_size(Pos2::new(248.0, 238.0), Vec2::new(236.0, 16.0));
    let _ = text::paint(
        b,
        p,
        &Text::key("kit.splash.midi", Role::Body, t.text_dim).align(Align2::LEFT_CENTER),
        1.0,
    );
    let bar = Rect::from_min_size(Pos2::new(248.0, 260.0), Vec2::new(236.0, 4.0));
    b.rrect(
        b.snap_rect(bar),
        2.0,
        Fill::Solid(with_alpha(t.text_dim, 0.25)),
    );
    let done = Rect::from_min_size(bar.min, Vec2::new(bar.width() * 0.42, bar.height()));
    b.rrect(b.snap_rect(done), 2.0, Fill::Solid(t.brand));
    b.rrect_stroke(
        b.snap_rect(r).shrink(b.px() * 0.5),
        0.0,
        b.px(),
        with_alpha(Color32::WHITE, if t.dark { 0.08 } else { 0.5 }),
    );
}

pub fn sheet(b: &mut Brush, _p: &Painter, s: &Style, size: Vec2) {
    let t = &s.t;
    b.rect(Rect::from_min_size(Pos2::ZERO, size), Fill::Solid(t.panel));
    let side = 88.0;
    let pitch = 106.0;
    for (row, (tint, key)) in [
        (Tint::Original, "kit.logo.tint.original"),
        (Tint::Theme, "kit.logo.tint.theme"),
    ]
    .iter()
    .enumerate()
    {
        let y = 14.0 + row as f32 * 150.0;
        let band = Rect::from_min_size(Pos2::new(0.0, y), Vec2::new(size.x, 20.0));
        let _ = text::paint(
            b,
            band.shrink2(Vec2::new(16.0, 0.0)),
            &Text::key(key, Role::Section, t.label).align(Align2::LEFT_CENTER),
            1.0,
        );
        for logo in Logo::all() {
            let x = 16.0 + (logo.0 - 1) as f32 * pitch;
            let r = Rect::from_min_size(
                Pos2::new(x + (pitch - side) * 0.5 - 8.0, y + 26.0),
                Vec2::splat(side),
            );
            logo::paint(b, r, logo, *tint, t, true);
            let lr = Rect::from_min_size(
                Pos2::new(x - 6.0, r.bottom() + 4.0),
                Vec2::new(pitch - 6.0, 16.0),
            );
            let name = Text {
                src: Src::Key(logo.name_key()),
                role: Role::Label,
                caps: false,
                align: Align2::CENTER_CENTER,
                policy: Policy::Elide(Elide::Middle),
                color: t.text_dim,
            };
            let _ = text::paint(b, lr, &name, 1.0);
        }
    }
    // #2's small masters, at their exact sizes, on the toolbar and on the ink.
    let y = 320.0;
    let _ = text::paint(
        b,
        Rect::from_min_size(Pos2::new(16.0, y), Vec2::new(400.0, 20.0)),
        &Text::new("#2 · 16 · 24 · 32 · 48 px", Role::Section, t.label).align(Align2::LEFT_CENTER),
        1.0,
    );
    let mut x = 16.0;
    for ground in [t.toolbar, t.ink] {
        let g = Rect::from_min_size(Pos2::new(x, y + 24.0), Vec2::new(170.0, 60.0));
        b.rrect(b.snap_rect(g), 4.0, Fill::Solid(ground));
        let mut lx = g.left() + 12.0;
        for px in [16.0, 24.0, 32.0, 48.0] {
            let r = Rect::from_min_size(Pos2::new(lx, g.center().y - px * 0.5), Vec2::splat(px));
            logo::paint(b, r, Logo::DEFAULT, Tint::Original, t, true);
            lx += px + 12.0;
        }
        x += 186.0;
    }
}
