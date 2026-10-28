//! The kit's promises, checked on what it actually paints (ADR 0024 §3, §4, §13).

use cantus_ui::i18n::{default_thread_locale, set_thread_locale, Locale};
use cantus_ui::kit::brush::Brush;
use cantus_ui::kit::gallery;
use cantus_ui::kit::knob::{self, KnobLook, KnobSize};
use cantus_ui::kit::meter::{Levels, MeterBatch};
use cantus_ui::kit::text::{self, Elide, Policy, Role, Src, Text};
use cantus_ui::kit::{cache, hint, shell, widgets, Fill, State, Style, Tokens};
use egui::{Color32, Pos2, Rect, Vec2};

/// Run one frame of `f` with a brush, in a context with the bundled fonts.
fn frame<R>(ctx: &egui::Context, f: impl FnOnce(&mut Brush, &mut egui::Ui) -> R) -> R {
    let mut out = None;
    let mut f = Some(f);
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 900.0))),
        ..Default::default()
    };
    ctx.run_ui(input, |ui| {
        let mut b = Brush::default();
        b.begin(ui.ctx());
        if let Some(f) = f.take() {
            out = Some(f(&mut b, ui));
        }
        b.flush(ui.painter());
    })
    .textures_delta
    .clear();
    out.expect("the frame ran")
}

#[test]
fn boxed_text_never_leaves_its_box() {
    default_thread_locale(Locale::En);
    let ctx = gallery::offscreen_context();
    let s = Style::new(Tokens::slate());
    for lang in [Locale::En, Locale::Es] {
        set_thread_locale(lang);
        frame(&ctx, |b, _| {
            let srcs = [
                Src::Str("Ghost Kick Layer (hosted)"),
                Src::Str("Resonancia del filtro"),
                Src::Key("kit.param.key_tracking"),
                Src::Key("kit.inspector.delay"),
            ];
            let policies = [
                Policy::Elide(Elide::End),
                Policy::Elide(Elide::Middle),
                Policy::Abbreviate,
            ];
            for role in [
                Role::Micro,
                Role::Label,
                Role::Body,
                Role::Title,
                Role::Value,
                Role::Section,
            ] {
                for src in srcs {
                    for policy in policies {
                        for w in (6..200).step_by(9) {
                            let bx = Rect::from_min_size(
                                Pos2::new(10.0, 10.0),
                                Vec2::new(w as f32, 22.0),
                            );
                            let t = Text {
                                src,
                                role,
                                caps: role.caps(),
                                align: egui::Align2::CENTER_CENTER,
                                policy,
                                color: s.t.text,
                            };
                            if let Some(p) = text::paint(b, bx, &t, 1.0) {
                                assert!(
                                    p.rect.left() >= bx.left() - 0.51
                                        && p.rect.right() <= bx.right() + 0.51,
                                    "{src:?} as {role:?} with {policy:?} left its {w}-pt box: {:?}",
                                    p.rect
                                );
                            }
                        }
                    }
                }
            }
        });
    }
}

fn fitted(ctx: &egui::Context, t: &Text<'_>, w: f32) -> String {
    frame(ctx, |_, _| {
        cache::with(ctx, |c| text::fit(ctx, c, t, 1.0, w))
            .map(|(g, _)| g.text().to_string())
            .unwrap_or_default()
    })
}

#[test]
fn middle_elision_keeps_both_ends_of_a_name() {
    default_thread_locale(Locale::En);
    let ctx = gallery::offscreen_context();
    let t =
        Text::new("GHOST KICK", Role::Label, Color32::WHITE).policy(Policy::Elide(Elide::Middle));
    let s = fitted(&ctx, &t, 40.0);
    assert!(s.contains('…'), "{s}");
    assert!(
        s.starts_with("GH") && s.ends_with('K'),
        "FL's GHO…ICK: got {s}"
    );
    let e = fitted(
        &ctx,
        &Text {
            policy: Policy::Elide(Elide::End),
            ..t
        },
        40.0,
    );
    assert!(e.ends_with('…') && e.starts_with("GHO"), "{e}");
    // Room enough: untouched.
    assert_eq!(fitted(&ctx, &t, 200.0), "GHOST KICK");
}

#[test]
fn abbreviate_uses_the_short_form_in_each_language() {
    let ctx = gallery::offscreen_context();
    let t = Text::key("kit.param.attack", Role::Label, Color32::WHITE)
        .caps(true)
        .policy(Policy::Abbreviate);
    set_thread_locale(Locale::En);
    assert_eq!(fitted(&ctx, &t, 30.0), "ATK");
    assert_eq!(fitted(&ctx, &t, 200.0), "ATTACK");
    set_thread_locale(Locale::Es);
    assert_eq!(fitted(&ctx, &t, 30.0), "ATQ");
    assert_eq!(fitted(&ctx, &t, 200.0), "ATAQUE");
    set_thread_locale(Locale::En);
}

#[test]
fn a_rotated_name_stays_in_its_column() {
    default_thread_locale(Locale::En);
    let ctx = gallery::offscreen_context();
    frame(&ctx, |b, _| {
        let col = Rect::from_min_size(Pos2::new(100.0, 100.0), Vec2::new(18.0, 64.0));
        let t = Text::new("Lead Vocal Double", Role::Label, Color32::WHITE)
            .align(egui::Align2::CENTER_BOTTOM)
            .policy(Policy::Rotate90(Elide::Middle));
        let p = text::paint(b, col, &t, 1.0).expect("painted");
        assert!(p.shortened, "a long name in a short column is elided");
        assert!(
            col.expand(0.6).contains_rect(p.rect),
            "{:?} outside {col:?}",
            p.rect
        );
    });
}

#[test]
fn a_hairline_is_one_physical_pixel_at_1_5x() {
    let bg = Color32::from_rgb(40, 40, 40);
    let line = Color32::from_rgb(220, 220, 220);
    let c = gallery::render(Vec2::new(40.0, 20.0), 1.5, |b, _| {
        b.rect(
            Rect::from_min_size(Pos2::ZERO, Vec2::new(40.0, 20.0)),
            Fill::Solid(bg),
        );
        b.hline(0.0, 40.0, 10.3, b.px(), line);
    });
    let lit: Vec<usize> = (0..c.height).filter(|&y| c.pixel(20, y)[0] > 60).collect();
    assert_eq!(lit.len(), 1, "rows lit: {lit:?}");
    assert_eq!(
        c.pixel(20, lit[0])[0],
        220,
        "the line is at full strength, not smeared"
    );
}

#[test]
fn many_knobs_share_one_rasterised_body() {
    default_thread_locale(Locale::En);
    let ctx = gallery::offscreen_context();
    let s = Style::new(Tokens::slate());
    let (verts, stats) = frame(&ctx, |b, _| {
        let before = cache::with(&ctx, |c| c.stats);
        for i in 0..256 {
            let c = Pos2::new(30.0 + (i % 16) as f32 * 44.0, 30.0 + (i / 16) as f32 * 44.0);
            knob::paint(
                b,
                c,
                &s,
                State::REST,
                &KnobLook::new(KnobSize::M, i as f32 / 255.0, 0.0),
            );
        }
        let after = cache::with(&ctx, |c| c.stats);
        (b.vertex_count(), (before, after))
    });
    let (before, after) = stats;
    assert_eq!(
        after.layers_rasterised - before.layers_rasterised,
        1,
        "one body for 256 knobs of one size"
    );
    // (Repeats are answered by the brush's own memo, without asking the cache again.)
    let per_knob = verts / 256;
    assert!(
        per_knob < 420,
        "a cached knob should be light: {per_knob} vertices"
    );
}

#[test]
fn all_the_meters_of_a_view_are_one_mesh() {
    let ctx = gallery::offscreen_context();
    let s = Style::new(Tokens::slate());
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 300.0))),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        let mut b = Brush::default();
        b.begin(ui.ctx());
        let mut batch = MeterBatch::new(&s);
        for i in 0..64 {
            let r = Rect::from_min_size(
                Pos2::new(8.0 + i as f32 * 20.0, 10.0),
                Vec2::new(14.0, 200.0),
            );
            batch.paint(
                &mut b,
                r,
                &Levels {
                    level: [-6.0 - i as f32 * 0.5, -8.0],
                    hold: [-3.0, -4.0],
                },
            );
        }
        assert!(
            b.vertex_count() / 64 <= 60,
            "{} vertices a meter",
            b.vertex_count() / 64
        );
        b.flush(ui.painter());
    });
    out.textures_delta.clear();
    let meshes = out
        .shapes
        .iter()
        .filter(|s| matches!(s.shape, egui::Shape::Mesh(_)))
        .count();
    assert_eq!(meshes, 1, "64 meters, one mesh");
}

#[test]
fn a_hovered_knob_writes_its_hint() {
    default_thread_locale(Locale::En);
    let ctx = gallery::offscreen_context();
    let mut v = 0.5f32;
    let run = |ctx: &egui::Context, v: &mut f32, events: Vec<egui::Event>| {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 200.0))),
            events,
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            widgets::Param::new(v, 0.0..=1.0, 0.5)
                .name("Cutoff")
                .format(|x, b| {
                    use std::fmt::Write as _;
                    let _ = write!(b, "{:.0} %", x * 100.0);
                })
                .show(ui);
        })
        .textures_delta
        .clear();
    };
    run(&ctx, &mut v, vec![]);
    run(
        &ctx,
        &mut v,
        vec![egui::Event::PointerMoved(Pos2::new(20.0, 20.0))],
    );
    let h = hint::current(&ctx).expect("the knob wrote a hint");
    assert_eq!(h.name, "Cutoff");
    assert_eq!(h.value, "50 %");
    assert!(h.caps.automatable);
}

#[test]
fn the_shell_frames_the_body_between_header_and_footer() {
    default_thread_locale(Locale::En);
    cantus_ui::kit::tokens::install(Tokens::slate());
    let ctx = gallery::offscreen_context();
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, shell::EFFECT)),
        ..Default::default()
    };
    let mut got = None;
    ctx.run_ui(input, |ui| {
        let h = shell::Header {
            name: "Aequalis",
            context: "(Insert 1)",
            hue: cantus_ui::kit::Function::Tone,
            preset: Some(shell::Preset {
                name: "Init",
                modified: false,
                favourite: false,
            }),
            ab: Some(shell::Ab::A),
            undo: Some((false, false)),
            views: None,
            bypassed: Some(false),
            help: true,
            name_w: 0.0,
            window: None,
        };
        let f = shell::Footer {
            chips: &[],
            meter: None,
        };
        let mut body_seen = None;
        let out = shell::Shell::new("t").show(ui, &h, &f, |_, _, body| body_seen = Some(body));
        got = Some((out, body_seen));
    })
    .textures_delta
    .clear();
    let (out, body) = got.expect("shown");
    assert_eq!(out.header.height(), shell::HEADER);
    assert_eq!(out.footer.height(), shell::FOOTER);
    assert_eq!(Some(out.body), body);
    assert_eq!(out.body.top(), out.header.bottom());
    assert_eq!(out.body.bottom(), out.footer.top());
    assert!(out.events.is_empty());
    assert_eq!(out.k, 1.0);
}
