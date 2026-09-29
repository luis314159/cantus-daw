//! What Organum's and Lamina's editor tests share (`mod inst_keys;`): a frame of the open
//! editor at a size, the text it painted and where, gestures, and the audit ADR 0024 §14 asks
//! for — rendered at 75 % and 200 % and in Spanish, no text of the editor's own area is
//! elided and no two texts overlap.
#![allow(dead_code)]

use cantus_core::TrackId;
use cantus_ui::CantusApp;
use egui::{Pos2, Rect, Vec2};

/// A piece of text a frame painted: what, where (its visual bounds) and its clip.
#[derive(Clone, Debug)]
pub struct Painted {
    pub text: String,
    pub rect: Rect,
    pub clip: Rect,
    pub angle: f32,
}

/// Room for the largest window (1040 × 680 at 200 %).
pub const SCREEN: Vec2 = Vec2::new(2200.0, 1500.0);

fn collect(shapes: &[egui::epaint::ClippedShape]) -> Vec<Painted> {
    fn walk(shape: &egui::Shape, clip: Rect, out: &mut Vec<Painted>) {
        match shape {
            egui::Shape::Text(t) => out.push(Painted {
                text: t.galley.text().to_string(),
                rect: t.visual_bounding_rect(),
                clip,
                angle: t.angle,
            }),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in shapes {
        walk(&s.shape, s.clip_rect, &mut out);
    }
    out
}

/// One frame of the open instrument editor with `events`; the text it painted.
pub fn frame(a: &mut CantusApp, ctx: &egui::Context, events: Vec<egui::Event>) -> Vec<Painted> {
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)),
        events,
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        cantus_ui::views::inst::window_in(a, ui);
    });
    out.textures_delta.clear();
    collect(&out.shapes)
}

/// All the painted text joined, as `support::painted_text` does.
pub fn joined(p: &[Painted]) -> String {
    let mut s = String::new();
    for t in p {
        s.push_str(&t.text);
        s.push_str(" | ");
    }
    s
}

fn button(pos: Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

/// A press at `from`, a drag in five steps to `to`, a release.
pub fn drag(a: &mut CantusApp, ctx: &egui::Context, from: Pos2, to: Pos2) {
    frame(a, ctx, vec![egui::Event::PointerMoved(from)]);
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(from), button(from, true)],
    );
    for i in 1..=5 {
        let at = from + (to - from) * (i as f32 / 5.0);
        frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    }
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(to), button(to, false)],
    );
    frame(a, ctx, vec![]);
}

/// A modulation source dropped at `at` (as if dragged there from its chip).
pub fn drop_source(a: &mut CantusApp, ctx: &egui::Context, tag: &'static str, at: Pos2) {
    frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    egui::DragAndDrop::set_payload(ctx, tag);
    frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    frame(
        a,
        ctx,
        vec![egui::Event::PointerMoved(at), button(at, false)],
    );
    frame(a, ctx, vec![]);
}

/// Show the editor of `track` at scale `k` (the shell's per-window scale).
pub fn set_scale(ctx: &egui::Context, track: TrackId, k: f32) {
    cantus_ui::kit::shell::Shell::new(("inst", track.0)).set_scale(ctx, k);
}

/// What is wrong with the text painted inside `view` (the editor's visible area): a text
/// elided ("…") or two texts overlapping.
pub fn audit(texts: &[Painted], view: Rect) -> Vec<String> {
    let mine: Vec<&Painted> = texts
        .iter()
        .filter(|p| !p.text.trim().is_empty() && view.contains(p.rect.center()))
        .collect();
    let mut problems = Vec::new();
    for p in &mine {
        if p.text.contains('\u{2026}') {
            problems.push(format!("elided: {:?} at {:?}", p.text, p.rect.min));
        }
    }
    for (i, a) in mine.iter().enumerate() {
        for b in &mine[i + 1..] {
            if a.angle != 0.0 || b.angle != 0.0 {
                continue;
            }
            let o = a.rect.shrink(1.0).intersect(b.rect.shrink(1.0));
            if o.is_positive() {
                problems.push(format!(
                    "overlap: {:?} {:?} and {:?} {:?}",
                    a.text, a.rect, b.text, b.rect
                ));
            }
        }
    }
    problems
}

/// Scroll the view under `at` by `dy` points (negative: down the content) and let the
/// scrolling settle.
pub fn scroll(a: &mut CantusApp, ctx: &egui::Context, at: Pos2, dy: f32) {
    frame(
        a,
        ctx,
        vec![
            egui::Event::PointerMoved(at),
            egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, dy),
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            },
        ],
    );
    for _ in 0..6 {
        frame(a, ctx, vec![egui::Event::PointerMoved(at)]);
    }
}

/// With `CANTUS_KEYS_SHOTS=<dir>`: PNGs of `kind`'s editor as `<file>-<view>-<theme>.png` at
/// 100 % in Slate and Paper, and Slate at 75 % and 200 % and in Spanish — the design review.
pub fn shots(kind: cantus_core::InstrumentKind, file: &str) {
    use cantus_ui::views::inst::View;
    let Some(dir) = std::env::var_os("CANTUS_KEYS_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let views = [
        (View::Play, "play"),
        (View::Edit, "edit"),
        (View::Matrix, "matrix"),
    ];
    let runs = [
        ("slate", 1.0, false),
        ("paper", 1.0, false),
        ("slate-75", 0.75, false),
        ("slate-200", 2.0, false),
        ("slate-es", 1.0, true),
    ];
    for (tag, k, es) in runs {
        let theme = if tag.starts_with("paper") {
            cantus_ui::theme::paper()
        } else {
            cantus_ui::theme::slate()
        };
        for (view, vtag) in views {
            if k != 1.0 && view == View::Matrix {
                continue;
            }
            let mut a = crate::support::app(cantus_ui::starter_project());
            let track = a.project().tracks[0].id;
            a.set_instrument(
                track,
                cantus_core::InstrumentSpec::Native(cantus_core::NativeSpec::new(kind)),
            );
            cantus_ui::views::inst::demo_setup(&mut a, track, kind);
            cantus_ui::views::inst::demo_feed(&mut a, kind);
            a.open_inst_editor(track);
            a.inst.view = view;
            if es {
                cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::Es);
            }
            let c = crate::support::ctx();
            cantus_ui::theme::apply(&theme, &c);
            set_scale(&c, track, k);
            let size = Vec2::new(1040.0, 680.0) * k;
            let path = dir.join(format!("{file}-{vtag}-{tag}.png"));
            cantus_ui::raster::shoot(&c, size, 1.0, 6, Some(&path), |ui| {
                cantus_ui::views::inst::window_in(&mut a, ui);
            });
            if es {
                cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
            }
        }
    }
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &crate::support::ctx());
}

/// What in `files` (under `src/views/inst/`) breaks ADR 0024 §3–4: a stock egui widget, a
/// bare label, a colour spelled out instead of a token, the old palette or type scale.
pub fn lint(files: &[&str]) -> Vec<String> {
    const BANNED: [&str; 14] = [
        "ui.label(",
        "RichText",
        "selectable_label",
        "ui.button(",
        "egui::Button",
        "egui::Slider",
        "DragValue",
        "checkbox(",
        "ComboBox",
        "on_hover_text",
        "Color32::from_",
        "theme::p()",
        "design::",
        "FontId::new",
    ];
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("views")
        .join("inst");
    let mut out = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(dir.join(f)).unwrap_or_default();
        assert!(!text.is_empty(), "{f} was read");
        for (n, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            for bad in BANNED {
                if code.contains(bad) {
                    out.push(format!("{f}:{}: {bad}", n + 1));
                }
            }
        }
    }
    out
}
