//! The instruments' pictures in their display's look (round 32, ADR 0074): what the
//! characteristic displays of Unda, Calor, Modulus, Exemplar, Gravis, Lamina, Organum,
//! Chorda and Pulsus paint with when their unit's face gives the display a look of its own
//! (a phosphor, an LCD, a VFD, glass, printed paper, a scope).
//!
//! The pictures were written against the kit's tokens — the text, the labels, the grid,
//! the ink and the well, the fills of the things drawn in them (an operator's node, a
//! voice card, a bass's body, a routing box), the device hue, and whether the theme is
//! dark. [`on_display`] hands them a style whose tokens *are* the display's: words in the
//! look's text (AA on its ground and on every fill), the grid in its rules, the fills shades
//! of its ground on the display's side of light, `dark` the display's own — so a light
//! LCD in a dark theme is painted as a light surface, and printed paper in Slate too. On
//! the kit's plain look (a plain sheet, or the user's switch off) it is the style unchanged.
//!
//! A display's colours are made once per unit and theme (on the unit's tone hue, whatever
//! card hue the picture's card wears) and kept here beside the style made from them, so
//! [`voice2`] and [`cover`] never make them again from the mapped style.

use crate::kit::cache;
use crate::kit::color::{contrast, ensure_contrast, ensure_contrast_all, mix};
use crate::kit::skin::{self, Display, DisplayInk};
use crate::kit::{Brush, Style, Tok};
use egui::{Color32, Rect};
use std::cell::Cell;

thread_local! {
    /// The displays' colours behind the styles [`on_display`] made, newest first.
    static INKS: Cell<[Option<(u64, DisplayInk)>; 8]> = const { Cell::new([None; 8]) };
}

/// A mapped style's identity: its skin, its theme, and the colours mapping gave it.
fn mapped_key(o: &Style) -> u64 {
    cache::key((
        o.skin.key,
        o.t.generation,
        o.t.dark,
        cache::ckey(o.t.ink),
        cache::ckey(o.t.text),
        cache::ckey(o.t.deck),
    ))
}

fn remember(key: u64, d: DisplayInk) {
    INKS.with(|c| {
        let last = c.get();
        if last.iter().flatten().any(|(k, _)| *k == key) {
            return;
        }
        let mut next = [None; 8];
        next[0] = Some((key, d));
        next[1..].copy_from_slice(&last[..7]);
        c.set(next);
    });
}

fn recalled(o: &Style) -> Option<DisplayInk> {
    let key = mapped_key(o);
    INKS.with(|c| {
        c.get()
            .iter()
            .flatten()
            .find(|(k, _)| *k == key)
            .map(|e| e.1)
    })
}

/// The display's colours for `s` — a unit's card style, or a style [`on_display`] made.
pub fn ink(s: &Style) -> DisplayInk {
    if let Some(d) = recalled(s) {
        return d;
    }
    // One per unit and theme: the unit's tone hue, not the card's function hue.
    let mut tone = *s;
    tone.hue = tone.t.hue_tone;
    tone.hue_tok = Some(Tok::HueTone);
    skin::display_ink(&tone)
}

/// The fills and words of a style painted in the display `d`: the kit's display style
/// (`skin::display_style`: every fill a shade of the ground, the display's polarity, the
/// modulation hue kept apart, the theme's floor run — round 32, the review of delivery 2
/// asked for one implementation), with the instruments' one addition: the words also read
/// on a body mixed from the ground and a raised fill (Gravis's bass).
fn mapped(s: &Style, d: &DisplayInk) -> Style {
    let mut tone = *s;
    tone.hue = tone.t.hue_tone;
    tone.hue_tok = Some(Tok::HueTone);
    let mut o = skin::display_style(&tone);
    let t = &mut o.t;
    let body = mix(t.ink, t.panel_raised, 0.45);
    let fills = [
        t.ink,
        t.well,
        t.deck,
        t.deck_title,
        t.panel,
        t.panel_raised,
        t.panel_light,
        body,
    ];
    t.text = ensure_contrast_all(t.text, &fills, 4.5);
    t.label = ensure_contrast_all(t.label, &fills, 4.5);
    t.text_dim = ensure_contrast_all(t.text_dim, &fills, 3.0);
    t.value_pill_text = ensure_contrast(t.text, t.value_pill, 4.5);
    t.grid_bar = d.grid_strong;
    o.hue = d.trace;
    o
}

/// `s` for painting inside its unit's display: the tokens the pictures read taken from
/// the display's look. Call it on the style the card was painted with.
pub fn on_display(s: &Style) -> Style {
    if s.skin.display == Display::Plain {
        return *s;
    }
    // Already on the display: as it is (mapping it again would make its colours again).
    if recalled(s).is_some() {
        return *s;
    }
    let d = ink(s);
    let o = mapped(s, &d);
    remember(mapped_key(&o), d);
    o
}

/// Whether `s` paints in a display of a look of its own (not the kit's plain one).
pub fn has_look(s: &Style) -> bool {
    s.skin.display != Display::Plain
}

/// The display's second voice: the face's second accent as the look shows it (3:1 on its
/// ground), or `plain` on the kit's plain look.
pub fn voice2(s: &Style, plain: Color32) -> Color32 {
    if !has_look(s) {
        s.skin.accent2_in(s.t.dark).unwrap_or(plain)
    } else {
        ink(s).trace2
    }
}

/// The face's own accent (a colour of the instrument, not of the display: Chorda's gold
/// frame) — whichever of its two tones reads better on the display's ground, at 3:1 — or
/// `plain` on the plain look.
pub fn accent_on(s: &Style, plain: Color32) -> Color32 {
    let Some(a) = s.skin.accent.filter(|_| has_look(s)) else {
        return plain;
    };
    let g = ink(s).ground;
    let (dk, lt) = (a.dark.rgb(), a.light.rgb());
    let best = if contrast(dk, g) >= contrast(lt, g) {
        dk
    } else {
        lt
    };
    ensure_contrast(best, g, 3.0)
}

/// What lies over a display, painted after its content: the glass's reflection, a CRT's
/// darker edges. Nothing on the plain look.
pub fn cover(b: &mut Brush, rect: Rect, s: &Style) {
    if has_look(s) {
        skin::displays::cover(b, rect, &ink(s));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kit::Tokens;

    fn sheets() -> Vec<&'static skin::Skin> {
        skin::Unit::all()
            .into_iter()
            .filter(|u| !matches!(u, skin::Unit::Effect(_)))
            .map(skin::of)
            .collect()
    }

    /// Every fill a picture draws on, mapped to the display's side of light, with the words
    /// AA on each — in every instrument's face, in every theme preset (the review's
    /// finding: dark letters on dark operator nodes on Modulus's LCD in Slate, light text on
    /// Calor's voice cards in Paper, Gravis's body a light slab on its scope).
    #[test]
    fn a_display_s_fills_and_words_follow_its_own_light_in_every_theme() {
        let mut bad = Vec::new();
        for (name, make) in crate::kit::appearance::PRESETS {
            let base = Tokens::for_theme(&make());
            for sheet in sheets() {
                let s = Style::new(base).skinned(sheet);
                let d = skin::display_ink(&s);
                let o = on_display(&s);
                if o.t.dark != d.dark {
                    bad.push(format!(
                        "{name} {}: dark {} on a display that is {}",
                        sheet.key, o.t.dark, d.dark
                    ));
                }
                for (what, fill) in [
                    ("ground", o.t.ink),
                    ("deck", o.t.deck),
                    ("deck_title", o.t.deck_title),
                    ("panel_raised", o.t.panel_raised),
                    ("body", mix(o.t.ink, o.t.panel_raised, 0.45)),
                ] {
                    let c = contrast(o.t.text, fill);
                    if c < 4.5 {
                        bad.push(format!("{name} {}: text on {what} {c:.2}", sheet.key));
                    }
                }
                // The colours behind the mapped style are the card's, never made again.
                if voice2(&o, Color32::RED) != d.trace2 {
                    bad.push(format!("{name} {}: the second voice drifted", sheet.key));
                }
            }
        }
        assert!(bad.is_empty(), "{}", bad.join("\n"));
    }

    /// Round 32, the review of delivery 2: one way to draw inside a display. The
    /// instruments' style is the kit's (`skin::display_style`): knobs, the toolbar, caps,
    /// meter wells and the device ground are shades of the display's ground, and the
    /// modulation hue is kept apart from the trace — in every instrument face and preset.
    #[test]
    fn the_instruments_display_style_is_the_kit_s() {
        let mut bad = Vec::new();
        for kind in cantus_core::inst::InstrumentKind::ALL {
            let sheet = skin::of(skin::Unit::Instrument(kind));
            if sheet.display == Display::Plain {
                continue;
            }
            for (name, make) in crate::kit::appearance::PRESETS {
                let s = Style::new(Tokens::for_theme(&make())).skinned(sheet);
                let o = on_display(&s);
                let mut tone = s;
                tone.hue = tone.t.hue_tone;
                tone.hue_tok = Some(Tok::HueTone);
                let k = skin::display_style(&tone);
                for (what, a, b) in [
                    ("knob body", o.t.knob_body, k.t.knob_body),
                    ("toolbar", o.t.toolbar, k.t.toolbar),
                    ("cap", o.t.cap, k.t.cap),
                    ("meter well", o.t.meter_well, k.t.meter_well),
                    ("device ground", o.t.device_ground, k.t.device_ground),
                    ("modulation hue", o.t.hue_mod, k.t.hue_mod),
                ] {
                    if a != b {
                        bad.push(format!("{name} {}: its {what} is not the kit's", sheet.key));
                    }
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{}",
            bad.join(
                "
"
            )
        );
    }

    /// One display's colours per unit and theme, whatever card hue the picture's card
    /// wears, and none made again from the styles mapped from them.
    #[test]
    fn a_unit_s_displays_make_their_colours_once() {
        let base = Tokens::for_theme(&crate::kit::appearance::PRESETS[0].1());
        let s = Style::new(base).skinned(skin::of(skin::Unit::Instrument(
            cantus_core::inst::InstrumentKind::Fm,
        )));
        let _ = on_display(&s);
        let before = skin::counts().2;
        for f in crate::kit::Function::ALL {
            let o = on_display(&s.function(f));
            let _ = voice2(&o, Color32::RED);
            let _ = accent_on(&o, Color32::RED);
            let _ = on_display(&o);
        }
        assert_eq!(skin::counts().2, before);
    }

    /// The colours of every vertex `grand` paints in `s`.
    fn painted(s: &crate::kit::Style) -> Vec<egui::Color32> {
        let keys = cantus_dsp::chorda::tables::keyboard(440.0, 1.0, 1.0);
        let now = crate::views::inst::chorda::draw::Now {
            key: 60,
            playing: false,
            velocity: 0.0,
            pedal: 0.0,
            soft: 0.0,
        };
        let ctx = egui::Context::default();
        crate::design::install_fonts(&ctx);
        let rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(640.0, 320.0));
        let input = egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            let mut b = crate::kit::Brush::default();
            b.begin(ui.ctx());
            crate::views::inst::chorda::draw::grand(&mut b, rect, s, &keys, &now, 1.0, None);
            b.flush(ui.painter());
        });
        out.textures_delta.clear();
        let mut v = Vec::new();
        fn walk(s: &egui::Shape, v: &mut Vec<egui::Color32>) {
            match s {
                egui::Shape::Mesh(m) => v.extend(m.vertices.iter().map(|x| x.color)),
                egui::Shape::Vec(xs) => xs.iter().for_each(|x| walk(x, v)),
                _ => {}
            }
        }
        for c in &out.shapes {
            walk(&c.shape, &mut v);
        }
        v
    }

    /// With *Units wear their own faces* off, Chorda's piano from above is the kit's plain
    /// picture (the review of round 32): no soundboard, no gold rim, no felt row, and the
    /// shown hammer in the device hue — never the dynamics hue or a face's colour.
    #[test]
    fn the_plain_look_draws_the_piano_as_before_and_the_face_adds_its_wood_and_felt() {
        use crate::kit::{skin, Style, Tokens};
        let t = Tokens::for_theme(&crate::theme::slate());
        let plain = Style::new(t);
        let colours = painted(&plain);
        assert!(
            !colours.contains(&t.hue_dynamics),
            "a dynamics-hue fill on the plain look"
        );
        assert!(
            colours.contains(&plain.hue),
            "the shown hammer in the device hue"
        );
        let faced = Style::new(t).skinned(skin::of(skin::Unit::Instrument(
            cantus_core::inst::InstrumentKind::Chorda,
        )));
        let gold = crate::views::inst::face::accent_on(&faced, egui::Color32::RED);
        let felt = crate::views::inst::face::voice2(&faced, egui::Color32::RED);
        let fc = painted(&faced);
        assert!(fc.contains(&felt), "the shown hammer in felt");
        assert!(
            fc.len() > colours.len(),
            "the soundboard, its rim and the felt row"
        );
        assert!(!colours.contains(&gold) && !colours.contains(&felt));
    }

    /// A card title painted beside a tail control (`organ_parts::card`: Organum, Chorda,
    /// Lamina, Gravis…) follows the user's text size like every other word of the editor —
    /// faces off exactly as before the lettering fix, and faces on in the face's lettering.
    #[test]
    fn a_tail_card_s_title_follows_the_text_size_with_faces_on_and_off() {
        use crate::kit::text::{self as ktext, Role, Text};
        use crate::kit::{Function, Tokens};
        crate::i18n::set_thread_locale(crate::i18n::Locale::En);
        let ctx = egui::Context::default();
        crate::design::install_fonts(&ctx);
        let mut t = Tokens::for_theme(&crate::theme::slate());
        t.text_scale = 1.4;
        let organum = skin::of(skin::Unit::Instrument(
            cantus_core::inst::InstrumentKind::Organ,
        ));
        for sheet in [&skin::PLAIN, organum] {
            let s = Style::new(t).skinned(sheet);
            let rect = Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420.0, 160.0));
            let card = crate::views::inst::organ::parts::Card::new("organ.vibrato", Function::Mod)
                .tail(80.0);
            let mut out = ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(rect.expand(8.0)),
                    ..Default::default()
                },
                |ui| {
                    let mut b = crate::kit::Brush::default();
                    b.begin(ui.ctx());
                    let _ = crate::views::inst::organ::parts::card(&mut b, rect, &s, &card);
                    b.flush(ui.painter());
                },
            );
            out.textures_delta.clear();
            let (face, caps) = s.skin.lettering.title();
            let want = ktext::measure_face(
                &ctx,
                &Text::key("organ.vibrato", Role::Section, s.t.label).caps(caps),
                s.tk(),
                face,
            );
            let word = crate::i18n::t("organ.vibrato");
            let word = if caps { word.to_uppercase() } else { word };
            let mut got = None;
            fn walk(sh: &egui::Shape, word: &str, got: &mut Option<f32>) {
                match sh {
                    egui::Shape::Text(t) if t.galley.text() == word => {
                        *got = Some(t.galley.size().y)
                    }
                    egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, word, got)),
                    _ => {}
                }
            }
            for c in &out.shapes {
                walk(&c.shape, &word, &mut got);
            }
            let got = got.unwrap_or_else(|| panic!("{}: the title {word:?} painted", sheet.key));
            assert!(
                (got - want.y).abs() < 0.5,
                "{}: the title at {got} high, the text size asks {}",
                sheet.key,
                want.y
            );
        }
    }
}
