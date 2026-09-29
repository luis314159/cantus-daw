//! Every painted colour resolves from a named token, and inspect mode names them all.
//!
//! For every part of the kit, the gallery sheet is painted once with the Slate tokens,
//! then once per token with only that token changed. A token whose change moves any
//! vertex colour, any text colour or any cached layer's pixels is one the part paints
//! with — and it must be among the tokens the part registered for inspect mode (the
//! eyedropper would otherwise miss it). And every token a part registers must change
//! something (the eyedropper must not list tokens that do nothing there).

use cantus_ui::i18n::{default_thread_locale, Locale};
use cantus_ui::kit::brush::Brush;
use cantus_ui::kit::gallery::{self, PARTS};
use cantus_ui::kit::{cache, inspect, Style, Tok, Tokens};
use egui::{Color32, Pos2, Rect, Shape, TextureId};
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};

fn perturbed(c: Color32) -> Color32 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    let flip = |v: u8| {
        if v < 128 {
            v.saturating_add(90)
        } else {
            v - 90
        }
    };
    Color32::from_rgba_unmultiplied(flip(r), flip(g), flip(b), a.max(40))
}

/// A hash of everything a frame painted: shapes and the pixels of the layers it uploaded.
fn paint_hash(ctx: &egui::Context, t: &Tokens, part: gallery::Part) -> u64 {
    cache::with_raw(ctx, |c| c.clear_layers());
    let size = gallery::sheet_size(part);
    let input = egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size + egui::vec2(8.0, 8.0))),
        ..Default::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        let mut b = Brush::default();
        b.begin(ui.ctx());
        gallery::paint_sheet(&mut b, Pos2::ZERO, &Style::new(*t), part);
        b.flush(ui.painter());
    });
    let mut h = cache::Fx::default();
    fn walk(s: &Shape, h: &mut cache::Fx) {
        match s {
            Shape::Mesh(m) => {
                for v in &m.vertices {
                    v.color.to_array().hash(h);
                    v.pos.x.to_bits().hash(h);
                    v.pos.y.to_bits().hash(h);
                }
            }
            Shape::Text(t) => {
                t.galley.text().hash(h);
                t.fallback_color.to_array().hash(h);
                t.override_text_color.map(|c| c.to_array()).hash(h);
            }
            Shape::Vec(v) => v.iter().for_each(|s| walk(s, h)),
            other => format!("{other:?}").hash(h),
        }
    }
    for s in &out.shapes {
        walk(&s.shape, &mut h);
    }
    for (id, deltas) in &out.textures_delta.set {
        if *id != TextureId::default() {
            for d in deltas.iter() {
                let egui::ImageData::Color(img) = &d.image;
                for p in img.pixels.iter() {
                    p.to_array().hash(&mut h);
                }
            }
        }
    }
    out.textures_delta.clear();
    h.finish()
}

#[test]
fn every_part_registers_exactly_the_tokens_it_paints_with() {
    default_thread_locale(Locale::En);
    let ctx = gallery::offscreen_context();
    inspect::set_enabled(&ctx, true);
    let mut problems = Vec::new();
    for (theme, base) in [("slate", Tokens::slate()), ("paper", Tokens::paper())] {
        for (name, part) in PARTS {
            // Warm up (fonts, layers), then the baseline and what it registered.
            paint_hash(&ctx, &base, part);
            let reference = paint_hash(&ctx, &base, part);
            let items = inspect::items(&ctx);
            let registered: BTreeSet<Tok> = items
                .iter()
                .flat_map(|i| i.all_tokens().collect::<Vec<_>>())
                .collect();
            // The sheet's own part: the first thing registered after the sheet's ground.
            let own = items.iter().find(|i| i.part != "gallery").map(|i| i.part);
            let own_tokens: BTreeSet<Tok> = items
                .iter()
                .filter(|i| Some(i.part) == own)
                .flat_map(|i| i.all_tokens().collect::<Vec<_>>())
                .collect();
            let mut used = BTreeSet::new();
            for tok in Tok::ALL {
                let mut t = base;
                t.set(*tok, perturbed(base.get(*tok)));
                if paint_hash(&ctx, &t, part) != reference {
                    used.insert(*tok);
                }
            }
            let missing: Vec<_> = used.difference(&registered).map(|t| t.key()).collect();
            let idle: Vec<_> = own_tokens.difference(&used).map(|t| t.key()).collect();
            if !missing.is_empty() {
                problems.push(format!(
                    "{name} ({theme}): paints with {missing:?} but does not register them"
                ));
            }
            if !idle.is_empty() {
                problems.push(format!(
                    "{name} ({theme}): registers {idle:?}, which change nothing it paints"
                ));
            }
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn a_click_picks_the_topmost_element_and_its_tokens() {
    default_thread_locale(Locale::En);
    let ctx = gallery::offscreen_context();
    inspect::set_enabled(&ctx, true);
    let part = gallery::Part::Cell;
    paint_hash(&ctx, &Tokens::slate(), part);
    let items = inspect::items(&ctx);
    let cell = items
        .iter()
        .find(|i| i.part == "cell")
        .expect("a cell was registered");
    let knob = items
        .iter()
        .find(|i| i.part == "knob")
        .expect("its knob too");
    // On the knob: the knob (painted after, so on top). On the pill: the cell.
    let on_knob = inspect::hit(&ctx, knob.rect.center()).unwrap();
    assert_eq!(on_knob.part, "knob");
    assert!(on_knob.all_tokens().any(|t| t == Tok::KnobBody));
    assert!(
        on_knob.all_tokens().any(|t| t == Tok::HueTone),
        "the section hue is listed"
    );
    let on_pill = inspect::hit(&ctx, cell.rect.center_top() + egui::vec2(0.0, 12.0)).unwrap();
    assert_eq!(on_pill.part, "cell");
    assert!(on_pill.all_tokens().any(|t| t == Tok::ValuePill));
    // Off: nothing is recorded.
    inspect::set_enabled(&ctx, false);
    assert!(inspect::hit(&ctx, knob.rect.center()).is_none());
}
