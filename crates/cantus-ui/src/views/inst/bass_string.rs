//! The String engine's display: the bass drawn from above, to scale, and the harmonics of
//! the note it plays with the combs its geometry cuts — drawn with the kit's brush in the
//! Source hue.
//!
//! **The drawing is the editor** (the research's "take" from MODO's electronics page, and
//! further): the pluck marker and both pickups are handles — drag them along the string,
//! read them in millimetres from the bridge on dimension lines, double-click to reset. The
//! fretting finger shows where the note is played (string and fret, from the engine), and
//! the part of the string that vibrates is drawn in the hue.
//!
//! **The consequence of a position**, which no product shows: beside the drawing, the
//! note's harmonics as the pickups hear them — the pluck's comb `|sin nπβ|` times the
//! pickups' — with every notch marked (▽ the pluck's, ▼ the pickups'), moving as you drag,
//! and the stiffness stretch printed in cents.

use super::{micro, note_name, Grab};
use crate::app::CantusApp;
use crate::i18n::{t, tf};
use crate::kit::color::{mix, shade, with_alpha};
use crate::kit::{self, Brush, Fill, Style, Tok};
use cantus_core::{NativeSpec, TrackId};
use cantus_dsp::gravis::string::{b_open, comb, fret_mm, open_strings};
use cantus_engine::inst::bass::tel;
use egui::{Align2, Color32, Pos2, Rect, Sense, Vec2};

/// Harmonics drawn in the strip.
const PARTIALS: usize = 24;
/// Behind the bridge and past the nut, mm.
const TAIL_MM: f32 = 60.0;
const HEAD_MM: f32 = 40.0;
/// The tokens the display paints with, beside the hue (for inspect mode).
const TOKENS: &[Tok] = &[
    Tok::Ink,
    Tok::Grid,
    Tok::GridStrong,
    Tok::Text,
    Tok::TextDim,
    Tok::PanelRaised,
    Tok::Panel,
];

/// Everything a frame draws from.
struct View {
    scale: f32,
    strings: usize,
    open: [f32; 6],
    frets: usize,
    fretless: bool,
    pluck: f32,
    pu: [f32; 2],
    pu_level: [f32; 2],
    humbucker: bool,
    piezo: f32,
    stiffness: f32,
    kind: f32,
}

impl View {
    fn of(spec: &NativeSpec) -> Self {
        let strings = spec.get("str.strings").round().clamp(4.0, 6.0) as usize;
        View {
            scale: spec.get("str.scale"),
            strings,
            open: open_strings(strings, spec.get("str.tuning").round() as usize),
            frets: spec.get("str.frets").round() as usize,
            fretless: spec.get("str.fretless") >= 0.5,
            pluck: spec.get("str.pluck"),
            pu: [spec.get("str.pu1"), spec.get("str.pu2")],
            pu_level: [spec.get("str.pu1_level"), spec.get("str.pu2_level")],
            humbucker: spec.get("str.pu_type") >= 0.5,
            piezo: spec.get("str.piezo"),
            stiffness: spec.get("str.stiffness"),
            kind: spec.get("str.type").round(),
        }
    }
}

#[derive(Copy, Clone)]
struct Geo {
    rect: Rect,
    scale: f32,
}

impl Geo {
    /// x of a distance from the bridge in mm (the bridge at the left, the nut at the right).
    fn x(&self, mm: f32) -> f32 {
        let span = self.scale + TAIL_MM + HEAD_MM;
        self.rect.left() + (mm + TAIL_MM) / span * self.rect.width()
    }
    fn mm(&self, x: f32) -> f32 {
        let span = self.scale + TAIL_MM + HEAD_MM;
        (x - self.rect.left()) / self.rect.width() * span - TAIL_MM
    }
}

/// Where the drawing, the fingerboard and the harmonic strip go in `rect` at scale `k`.
fn layout(rect: Rect, v: &View, k: f32) -> (Rect, Rect, f32) {
    let strip_w = (rect.width() * 0.3).max(200.0 * k);
    let draw = Rect::from_min_max(
        rect.min + Vec2::new(10.0 * k, 26.0 * k),
        Pos2::new(rect.right() - strip_w - 34.0 * k, rect.bottom() - 34.0 * k),
    );
    let geo = Geo {
        rect: draw,
        scale: v.scale,
    };
    let neck_h = draw.height() * 0.5;
    let neck = Rect::from_min_max(
        Pos2::new(
            geo.x(fret_mm(v.scale as f64, v.frets as f64 + 0.5) as f32),
            draw.center().y - neck_h / 2.0,
        ),
        Pos2::new(geo.x(v.scale), draw.center().y + neck_h / 2.0),
    );
    (draw, neck, strip_w)
}

/// Where the pluck marker and the two pickups sit in a display drawn in `rect` at 100 %
/// (for tests that drag them).
pub fn handles(rect: Rect, spec: &NativeSpec) -> [Pos2; 3] {
    let v = View::of(spec);
    let (draw, neck, _) = layout(rect, &v, 1.0);
    let geo = Geo {
        rect: draw,
        scale: v.scale,
    };
    [
        Pos2::new(geo.x(v.pluck), neck.top() - 8.0),
        Pos2::new(geo.x(v.pu[0]), neck.center().y),
        Pos2::new(geo.x(v.pu[1]), neck.center().y),
    ]
}

/// The millimetres from the bridge under `x` in a display drawn in `rect` at 100 %.
pub fn mm_at(rect: Rect, spec: &NativeSpec, x: f32) -> f32 {
    let v = View::of(spec);
    let (draw, _, _) = layout(rect, &v, 1.0);
    Geo {
        rect: draw,
        scale: v.scale,
    }
    .mm(x)
}

/// The display in `d`, painted, with its three handles. `s` is in the Source hue.
pub fn display(
    app: &mut CantusApp,
    ui: &mut egui::Ui,
    track: TrackId,
    spec: &NativeSpec,
    d: Rect,
    s: &Style,
) {
    let k = s.k;
    let v = View::of(spec);
    let resp = ui.interact(
        d,
        ui.id().with(("gravis-string", track.0)),
        Sense::click_and_drag(),
    );
    let (draw, neck, strip_w) = layout(d, &v, k);
    let geo = Geo {
        rect: draw,
        scale: v.scale,
    };
    // The played note, from the engine (or the lowest open string at rest).
    let tele = app.inst_telemetry();
    let playing = tele[super::super::TEL_VOICES] > 0.0 && tele[tel::STRING] >= 0.0;
    let (string, fret) = if playing {
        (
            (tele[tel::STRING] as usize).min(v.strings - 1),
            tele[tel::FRET].max(0.0),
        )
    } else {
        (0, 0.0)
    };
    let vib = fret_mm(v.scale as f64, fret as f64) as f32;
    let grab = app.inst.bass.grab;
    // The pickups' coils, for the painter and the gestures.
    let coil_w = (geo.x(18.0) - geo.x(0.0)).max(6.0 * k);
    let coils: &[f32] = if v.humbucker { &[-10.0, 10.0] } else { &[0.0] };
    let pickup = |i: usize| {
        coils.iter().fold(Rect::NOTHING, |all, c| {
            all.union(Rect::from_center_size(
                Pos2::new(geo.x(v.pu[i] + c), neck.center().y),
                Vec2::new(coil_w, neck.height() + 10.0 * k),
            ))
        })
    };
    let pickups = [pickup(0), pickup(1)];
    let px = geo.x(v.pluck);
    let tri_top = neck.top() - 14.0 * k;
    kit::with_brush(&ui.painter_at(d), |b| {
        b.register(d, "gravis.string", TOKENS, &[s.hue_tok]);
        let t = &s.t;
        // The body, the fingerboard, the frets.
        let body = Rect::from_min_max(
            Pos2::new(geo.x(-TAIL_MM + 8.0), draw.top()),
            Pos2::new(geo.x(fret_mm(v.scale as f64, 17.0) as f32), draw.bottom()),
        );
        b.rrect(
            body,
            18.0 * k,
            Fill::Solid(mix(t.ink, t.panel_raised, 0.45)),
        );
        b.rrect_stroke(body, 18.0 * k, b.px(), with_alpha(t.grid_strong, 0.8));
        b.rect(neck, Fill::Solid(mix(t.ink, t.panel, 0.7)));
        for f in 1..=v.frets {
            let x = geo.x(fret_mm(v.scale as f64, f as f64) as f32);
            let (w, c) = if v.fretless {
                (b.px(), t.grid)
            } else {
                (1.5 * k, t.text_dim)
            };
            b.vline(x, neck.top(), neck.bottom(), w, c);
            if [3, 5, 7, 9, 12, 15, 17, 19, 21, 24].contains(&f) {
                let prev = geo.x(fret_mm(v.scale as f64, f as f64 - 1.0) as f32);
                let lr = Rect::from_center_size(
                    Pos2::new(0.5 * (x + prev), neck.bottom() + 8.0 * k),
                    Vec2::new(20.0 * k, 12.0 * k),
                );
                micro(b, lr, &format!("{f}"), t.text_dim, Align2::CENTER_CENTER, s);
            }
        }
        // The nut and the bridge.
        let bridge_x = geo.x(0.0);
        b.vline(geo.x(v.scale), neck.top(), neck.bottom(), 3.0 * k, t.text);
        b.vline(bridge_x, neck.top(), neck.bottom(), 3.0 * k, t.text);
        if v.piezo > 0.0 {
            b.dot(Pos2::new(bridge_x, neck.bottom() + 6.0 * k), 3.0 * k, s.hue);
        }
        // The strings: the lowest at the bottom, as in tablature; the one playing lit from
        // the bridge to the fretting finger.
        let gap = neck.height() / v.strings as f32;
        let string_y = |i: usize| neck.bottom() - gap * (i as f32 + 0.5);
        for i in 0..v.strings {
            let y = string_y(i);
            let w = (2.2 - 1.2 * i as f32 / (v.strings.max(2) - 1) as f32) * k;
            b.segment(
                Pos2::new(bridge_x, y),
                Pos2::new(geo.x(v.scale), y),
                w,
                t.text_dim,
                false,
            );
            let lr = Rect::from_min_size(
                Pos2::new(geo.x(v.scale) + 6.0 * k, y - 6.0 * k),
                Vec2::new(24.0 * k, 12.0 * k),
            );
            micro(
                b,
                lr,
                &note_name(v.open[i]),
                t.text_dim,
                Align2::LEFT_CENTER,
                s,
            );
            if i == string && playing {
                b.segment(
                    Pos2::new(bridge_x, y),
                    Pos2::new(geo.x(vib), y),
                    w + k,
                    s.hue,
                    false,
                );
                if fret > 0.25 {
                    // The finger sits just behind the fret.
                    let behind = fret_mm(v.scale as f64, (fret - 0.35) as f64) as f32;
                    b.dot(Pos2::new(geo.x(behind), y), 5.0 * k, s.hue);
                }
            }
        }
        // The pickups: coils across the strings, handles to drag.
        for (i, mm) in v.pu.iter().enumerate() {
            let hot = grab == Some(Grab::Pickup(i));
            let fill = mix(t.panel_raised, s.hue, 0.25 + 0.55 * v.pu_level[i]);
            for c in coils {
                let r = Rect::from_center_size(
                    Pos2::new(geo.x(*mm + c), neck.center().y),
                    Vec2::new(coil_w, neck.height() + 10.0 * k),
                );
                b.rrect(r, 3.0 * k, Fill::Solid(fill));
                b.rrect_stroke(r, 3.0 * k, if hot { 2.0 * k } else { b.px() }, t.text);
            }
            // Its dimension line from the bridge, as on a drawing.
            let y = draw.bottom() + 14.0 * k + 13.0 * k * i as f32;
            let x1 = geo.x(*mm);
            b.hline(bridge_x, x1, b.snap_mid(y), b.px(), t.text_dim);
            b.vline(bridge_x, y - 3.0 * k, y + 3.0 * k, b.px(), t.text_dim);
            b.vline(x1, y - 3.0 * k, y + 3.0 * k, b.px(), t.text_dim);
            let lr = Rect::from_min_size(
                Pos2::new(x1 + 4.0 * k, y - 6.0 * k),
                Vec2::new(80.0 * k, 12.0 * k),
            );
            let text = tf(
                "bass.str.pu_readout",
                &[("n", &format!("{}", i + 1)), ("mm", &format!("{mm:.0}"))],
            );
            micro(b, lr, &text, t.text, Align2::LEFT_CENTER, s);
        }
        // The pluck marker: a triangle over the strings.
        let hot = grab == Some(Grab::Pluck);
        b.polygon(
            &[
                Pos2::new(px - 7.0 * k, tri_top),
                Pos2::new(px + 7.0 * k, tri_top),
                Pos2::new(px, tri_top + 11.0 * k),
            ],
            if hot { shade(s.hue, 0.12) } else { s.hue },
        );
        b.vline(
            px,
            tri_top + 11.0 * k,
            neck.bottom(),
            b.px(),
            with_alpha(s.hue, 0.6),
        );
        let lr = Rect::from_center_size(
            Pos2::new(px, tri_top - 8.0 * k),
            Vec2::new(110.0 * k, 12.0 * k),
        );
        let text = tf(
            "bass.str.pluck_readout",
            &[("mm", &format!("{:.0}", v.pluck))],
        );
        micro(b, lr, &text, t.text, Align2::CENTER_CENTER, s);
        let lr = Rect::from_min_size(
            d.min + Vec2::new(10.0 * k, 4.0 * k),
            Vec2::new(draw.width(), 13.0 * k),
        );
        micro(
            b,
            lr,
            &t_of("bass.str.caption"),
            t.text_dim,
            Align2::LEFT_CENTER,
            s,
        );

        let strip = Rect::from_min_max(
            Pos2::new(d.right() - strip_w, d.top() + 26.0 * k),
            Pos2::new(d.right() - 12.0 * k, d.bottom() - 34.0 * k),
        );
        harmonics(b, strip, &v, (string, fret, vib), s);
    });

    // Gestures on the drawing.
    let pluck_hit = |pt: Pos2| (pt.x - px).abs() < 10.0 * k && pt.y < neck.top() + 4.0 * k;
    if resp.drag_started() {
        // Where the press began: by the time egui calls it a drag, the pointer has moved.
        let origin = ui.input(|i| i.pointer.press_origin());
        if let Some(pt) = origin.or(resp.interact_pointer_pos()) {
            app.inst.bass.grab = if pluck_hit(pt) {
                Some(Grab::Pluck)
            } else if pickups[0].expand(4.0 * k).contains(pt) {
                Some(Grab::Pickup(0))
            } else if pickups[1].expand(4.0 * k).contains(pt) {
                Some(Grab::Pickup(1))
            } else {
                None
            };
        }
    }
    if resp.dragged() {
        if let Some(pt) = resp.interact_pointer_pos() {
            let mm = geo.mm(pt.x).round();
            match app.inst.bass.grab {
                Some(Grab::Pluck) => app.set_inst_value(track, "str.pluck", mm, true),
                Some(Grab::Pickup(i)) => {
                    app.set_inst_value(track, ["str.pu1", "str.pu2"][i], mm, true)
                }
                _ => {}
            }
        }
    }
    if resp.drag_stopped() {
        app.inst.bass.grab = None;
        app.end_gesture();
    }
    if resp.hovered() {
        crate::kit::hint::set_text(
            ui.ctx(),
            &t("bass.str.pluck"),
            &t("bass.str.caption"),
            crate::kit::hint::Caps::PARAM,
        );
    }
    if resp.double_clicked() {
        if let Some(pt) = resp.interact_pointer_pos() {
            let key = if pluck_hit(pt) {
                Some("str.pluck")
            } else if pickups[0].contains(pt) {
                Some("str.pu1")
            } else if pickups[1].contains(pt) {
                Some("str.pu2")
            } else {
                None
            };
            if let Some((key, info)) = key.and_then(|key| spec.kind.param(key).map(|i| (key, i))) {
                app.set_inst_value(track, key, info.default, false);
            }
        }
    }
}

fn t_of(key: &str) -> String {
    t(key)
}

/// The note's harmonics as the pickups hear them, and every notch marked.
fn harmonics(
    b: &mut Brush,
    strip: Rect,
    v: &View,
    (string, fret, vib): (usize, f32, f32),
    s: &Style,
) {
    let k = s.k;
    let t = &s.t;
    let beta_p = (v.pluck / vib).clamp(0.0, 1.0) as f64;
    let beta_u = [(v.pu[0] / vib) as f64, (v.pu[1] / vib) as f64];
    let level = |n: usize| {
        // The pluck's displacement falls as 1/n²; the pickup hears velocity (×n).
        let pluck = comb(n, beta_p) / n as f64;
        let pick: f64 = (0..2)
            .map(|i| {
                if beta_u[i] >= 1.0 {
                    0.0
                } else {
                    v.pu_level[i] as f64 * (n as f64 * std::f64::consts::PI * beta_u[i]).sin()
                }
            })
            .sum();
        pluck * pick.abs()
    };
    let mut levels = [0.0f64; PARTIALS];
    for (i, l) in levels.iter_mut().enumerate() {
        *l = level(i + 1);
    }
    let max = levels.iter().cloned().fold(1e-12, f64::max);
    let floor = -48.0f64;
    let w = strip.width() / PARTIALS as f32;
    let top = strip.top() + 14.0 * k;
    let y_of = |db: f64| {
        strip.bottom() - ((db - floor) / -floor).clamp(0.0, 1.0) as f32 * (strip.bottom() - top)
    };
    b.hline(
        strip.left(),
        strip.right(),
        b.snap_mid(y_of(0.0)),
        b.px(),
        t.grid,
    );
    b.hline(
        strip.left(),
        strip.right(),
        b.snap_mid(y_of(-24.0)),
        b.px(),
        t.grid,
    );
    for (i, l) in levels.iter().enumerate() {
        let n = i + 1;
        let db = 20.0 * (l / max).max(1e-9).log10();
        let x0 = strip.left() + i as f32 * w + k;
        let r = Rect::from_min_max(
            Pos2::new(x0, y_of(db)),
            Pos2::new(x0 + w - 2.0 * k, strip.bottom()),
        );
        b.rect(b.snap_rect(r), Fill::Solid(with_alpha(s.hue, 0.85)));
        if n == 1 || n % 4 == 0 {
            let lr = Rect::from_center_size(
                Pos2::new(x0 + (w - 2.0 * k) * 0.5, strip.bottom() + 8.0 * k),
                Vec2::new(20.0 * k, 12.0 * k),
            );
            micro(b, lr, &format!("{n}"), t.text_dim, Align2::CENTER_CENTER, s);
        }
    }
    // Notches: where n·β is a whole number. Continuous in n, so they move as you drag.
    let x_of = |n: f64| strip.left() + ((n - 0.5) as f32) * w;
    let mark = |b: &mut Brush, n: f64, filled: bool, color: Color32| {
        let x = x_of(n);
        let y = strip.top() + 2.0 * k;
        let pts = [
            Pos2::new(x - 4.0 * k, y),
            Pos2::new(x + 4.0 * k, y),
            Pos2::new(x, y + 7.0 * k),
        ];
        if filled {
            b.polygon(&pts, color);
        } else {
            b.polyline(&[pts[0], pts[1], pts[2], pts[0]], 1.2 * k, color);
        }
    };
    let limit = PARTIALS as f64 + 0.5;
    if beta_p > 0.0 {
        let mut n = 1.0;
        while n / beta_p <= limit {
            mark(b, n / beta_p, false, s.hue);
            n += 1.0;
        }
    }
    for (beta, lv) in beta_u.iter().zip(v.pu_level) {
        if *beta > 0.0 && *beta < 1.0 && lv > 0.0 {
            let mut n = 1.0;
            while n / beta <= limit {
                mark(b, n / beta, true, t.text);
                n += 1.0;
            }
        }
    }
    // What it is: the note, its string and fret, and how far the stiffness stretches it.
    let note = v.open[string] + fret;
    let f0 = 440.0 * 2f32.powf((note - 69.0) / 12.0);
    let type_b = [1.0, 0.85, 0.7, 0.4][(v.kind as usize).min(3)];
    let bb = b_open(v.open[string])
        * (v.scale as f64 / vib as f64).powi(2)
        * v.stiffness as f64
        * type_b;
    let stretch = 1200.0 * ((1.0 + 100.0 * bb) / (1.0 + bb)).sqrt().log2();
    let what = tf(
        "bass.str.harmonics_caption",
        &[
            ("note", &note_name(note)),
            ("hz", &format!("{f0:.1}")),
            ("string", &format!("{}", v.strings - string)),
            ("fret", &format!("{fret:.0}")),
        ],
    );
    let lr = Rect::from_min_size(
        Pos2::new(strip.left(), strip.top() - 22.0 * k),
        Vec2::new(strip.width(), 13.0 * k),
    );
    micro(b, lr, &what, t.text, Align2::LEFT_CENTER, s);
    let st = tf(
        "bass.str.stretch",
        &[
            ("b", &format!("{bb:.1e}")),
            ("ct", &format!("{stretch:+.0}")),
        ],
    );
    let lr = Rect::from_min_size(
        Pos2::new(strip.left(), strip.bottom() + 16.0 * k),
        Vec2::new(strip.width(), 12.0 * k),
    );
    micro(b, lr, &st, t.text_dim, Align2::LEFT_CENTER, s);
}
