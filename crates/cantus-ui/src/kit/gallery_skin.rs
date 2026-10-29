//! The skins' sheets in the gallery (round 32, ADR 0074): every knob shape, switch shape,
//! material, display look and plate the kit draws, and the ornaments — each on a sheet of
//! its own, in every state, through the same painters a unit's window uses. Each is worn
//! by a skin that changes only that one thing (no tint, no accent), so every colour still
//! resolves from a token and `tests/kit_tokens.rs` proves inspect mode names them all.

use crate::kit::band::{self, CardLook};
use crate::kit::brush::{Brush, Fill};
use crate::kit::knob::{self, KnobLook, KnobSize};
use crate::kit::led;
use crate::kit::shell;
use crate::kit::skin::{
    self, Case, Display, KnobShape, Lettering, Material, Ornaments, Plate, Skin, SwitchShape,
    Weight,
};
use crate::kit::text::Src;
use crate::kit::well;
use crate::kit::{State, Style, Tok};
use egui::{Pos2, Rect, Vec2};

/// A skin variant with a sheet of its own.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SkinPart {
    /// `KnobShape::ALL[i]`.
    Knob(u8),
    /// `SwitchShape::ALL[i]`.
    Switch(u8),
    Material(Material),
    Display(Display),
    Plate(Plate),
    Ornaments,
}

/// How many sheets the skins add.
pub const COUNT: usize = 6 + 4 + 9 + 6 + 4 + 1;

use super::Part;

/// The sheets, with the names their golden images carry.
pub const PARTS: [(&str, Part); COUNT] = [
    ("skin_knob_arc", Part::Skin(SkinPart::Knob(1))),
    ("skin_knob_skirted", Part::Skin(SkinPart::Knob(2))),
    ("skin_knob_chicken_head", Part::Skin(SkinPart::Knob(3))),
    ("skin_knob_soft", Part::Skin(SkinPart::Knob(4))),
    ("skin_knob_cap", Part::Skin(SkinPart::Knob(5))),
    ("skin_knob_dial", Part::Skin(SkinPart::Knob(6))),
    ("skin_switch_bat", Part::Skin(SkinPart::Switch(1))),
    ("skin_switch_rocker", Part::Skin(SkinPart::Switch(2))),
    ("skin_switch_pill", Part::Skin(SkinPart::Switch(3))),
    ("skin_switch_lamp", Part::Skin(SkinPart::Switch(4))),
    (
        "skin_material_matte",
        Part::Skin(SkinPart::Material(Material::Matte)),
    ),
    (
        "skin_material_brushed",
        Part::Skin(SkinPart::Material(Material::Brushed)),
    ),
    (
        "skin_material_anodised",
        Part::Skin(SkinPart::Material(Material::Anodised)),
    ),
    (
        "skin_material_lacquer",
        Part::Skin(SkinPart::Material(Material::Lacquer)),
    ),
    (
        "skin_material_wood",
        Part::Skin(SkinPart::Material(Material::Wood)),
    ),
    (
        "skin_material_vinyl",
        Part::Skin(SkinPart::Material(Material::Tolex)),
    ),
    (
        "skin_material_glass",
        Part::Skin(SkinPart::Material(Material::Glass)),
    ),
    (
        "skin_material_paper",
        Part::Skin(SkinPart::Material(Material::Paper)),
    ),
    (
        "skin_material_carbon",
        Part::Skin(SkinPart::Material(Material::Carbon)),
    ),
    (
        "skin_display_phosphor",
        Part::Skin(SkinPart::Display(Display::Phosphor)),
    ),
    (
        "skin_display_lcd",
        Part::Skin(SkinPart::Display(Display::Lcd)),
    ),
    (
        "skin_display_vfd",
        Part::Skin(SkinPart::Display(Display::Vfd)),
    ),
    (
        "skin_display_glass",
        Part::Skin(SkinPart::Display(Display::Glass)),
    ),
    (
        "skin_display_paper",
        Part::Skin(SkinPart::Display(Display::Paper)),
    ),
    (
        "skin_display_scope",
        Part::Skin(SkinPart::Display(Display::Scope)),
    ),
    (
        "skin_plate_engraved",
        Part::Skin(SkinPart::Plate(Plate::Engraved)),
    ),
    (
        "skin_plate_badge",
        Part::Skin(SkinPart::Plate(Plate::Badge)),
    ),
    (
        "skin_plate_stripe",
        Part::Skin(SkinPart::Plate(Plate::Stripe)),
    ),
    (
        "skin_plate_legend",
        Part::Skin(SkinPart::Plate(Plate::Legend)),
    ),
    ("skin_ornaments", Part::Skin(SkinPart::Ornaments)),
];

const fn with_knob(key: &'static str, knob: KnobShape) -> Skin {
    Skin {
        knob,
        ..Skin::plain(key)
    }
}

const fn with_switch(key: &'static str, switch: SwitchShape) -> Skin {
    Skin {
        switch,
        ..Skin::plain(key)
    }
}

const fn with_material(key: &'static str, material: Material) -> Skin {
    Skin {
        material,
        ..Skin::plain(key)
    }
}

const fn with_display(key: &'static str, display: Display) -> Skin {
    Skin {
        display,
        lettering: Lettering {
            mono_values: true,
            ..Lettering::PLAIN
        },
        ..Skin::plain(key)
    }
}

const fn with_plate(key: &'static str, plate: Plate, lettering: Lettering) -> Skin {
    Skin {
        plate,
        lettering,
        ..Skin::plain(key)
    }
}

static KNOBS: [Skin; 7] = [
    with_knob("gallery.knob.plain", KnobShape::Plain),
    with_knob("gallery.knob.arc", KnobShape::Arc),
    with_knob("gallery.knob.skirted", KnobShape::Skirted),
    with_knob("gallery.knob.chicken_head", KnobShape::ChickenHead),
    with_knob("gallery.knob.soft", KnobShape::Soft),
    with_knob("gallery.knob.cap", KnobShape::Cap),
    with_knob("gallery.knob.dial", KnobShape::Dial),
];

static SWITCHES: [Skin; 5] = [
    with_switch("gallery.switch.plain", SwitchShape::Plain),
    with_switch("gallery.switch.bat", SwitchShape::Bat),
    with_switch("gallery.switch.rocker", SwitchShape::Rocker),
    with_switch("gallery.switch.pill", SwitchShape::Pill),
    with_switch("gallery.switch.lamp", SwitchShape::Lamp),
];

static MATERIALS: [Skin; 10] = [
    with_material("gallery.material.plain", Material::Plain),
    with_material("gallery.material.matte", Material::Matte),
    with_material("gallery.material.brushed", Material::Brushed),
    with_material("gallery.material.anodised", Material::Anodised),
    with_material("gallery.material.lacquer", Material::Lacquer),
    with_material("gallery.material.wood", Material::Wood),
    with_material("gallery.material.vinyl", Material::Tolex),
    with_material("gallery.material.glass", Material::Glass),
    with_material("gallery.material.paper", Material::Paper),
    with_material("gallery.material.carbon", Material::Carbon),
];

static DISPLAYS: [Skin; 7] = [
    with_display("gallery.display.plain", Display::Plain),
    with_display("gallery.display.phosphor", Display::Phosphor),
    with_display("gallery.display.lcd", Display::Lcd),
    with_display("gallery.display.vfd", Display::Vfd),
    with_display("gallery.display.glass", Display::Glass),
    with_display("gallery.display.paper", Display::Paper),
    with_display("gallery.display.scope", Display::Scope),
];

/// Each plate in a lettering of its own, so every lettering is drawn too.
static PLATES: [Skin; 5] = [
    with_plate("gallery.plate.plain", Plate::Plain, Lettering::PLAIN),
    with_plate(
        "gallery.plate.engraved",
        Plate::Engraved,
        Lettering {
            case: Case::Upper,
            tracking: 0.18,
            weight: Weight::Regular,
            condensed: false,
            mono_values: false,
        },
    ),
    with_plate(
        "gallery.plate.badge",
        Plate::Badge,
        Lettering {
            case: Case::AsIs,
            tracking: 0.02,
            weight: Weight::SemiBold,
            condensed: false,
            mono_values: false,
        },
    ),
    with_plate(
        "gallery.plate.stripe",
        Plate::Stripe,
        Lettering {
            case: Case::Upper,
            tracking: 0.08,
            weight: Weight::Medium,
            condensed: true,
            mono_values: false,
        },
    ),
    with_plate(
        "gallery.plate.legend",
        Plate::Legend,
        Lettering {
            case: Case::Upper,
            tracking: 0.12,
            weight: Weight::Regular,
            condensed: true,
            mono_values: false,
        },
    ),
];

static ORNATE: Skin = Skin {
    ornaments: Ornaments {
        screws: true,
        vents: true,
        rules: true,
        rack_ears: true,
    },
    ..Skin::plain("gallery.ornaments")
};

/// The skin a sheet wears.
pub fn skin_of(sp: SkinPart) -> &'static Skin {
    let at = |m: Material| Material::ALL.iter().position(|x| *x == m).unwrap_or(0);
    let dat = |d: Display| Display::ALL.iter().position(|x| *x == d).unwrap_or(0);
    let pat = |p: Plate| Plate::ALL.iter().position(|x| *x == p).unwrap_or(0);
    match sp {
        SkinPart::Knob(i) => &KNOBS[(i as usize).min(KNOBS.len() - 1)],
        SkinPart::Switch(i) => &SWITCHES[(i as usize).min(SWITCHES.len() - 1)],
        SkinPart::Material(m) => &MATERIALS[at(m)],
        SkinPart::Display(d) => &DISPLAYS[dat(d)],
        SkinPart::Plate(p) => &PLATES[pat(p)],
        SkinPart::Ornaments => &ORNATE,
    }
}

/// The room one state takes.
pub fn slot(sp: SkinPart) -> Vec2 {
    match sp {
        SkinPart::Knob(_) => Vec2::new(118.0, 78.0),
        SkinPart::Switch(_) => Vec2::new(100.0, 54.0),
        SkinPart::Material(_) => Vec2::new(150.0, 110.0),
        SkinPart::Display(_) => Vec2::new(150.0, 96.0),
        SkinPart::Plate(_) => Vec2::new(150.0, 36.0),
        SkinPart::Ornaments => Vec2::new(170.0, 120.0),
    }
}

/// The surface a sheet sits on.
pub fn ground_tok(sp: SkinPart) -> Tok {
    match sp {
        SkinPart::Material(_) | SkinPart::Ornaments => Tok::DeviceGround,
        SkinPart::Plate(_) => Tok::Panel,
        _ => Tok::Deck,
    }
}

/// Paint one state of a sheet in `rect`.
pub fn paint(b: &mut Brush, rect: Rect, s: &Style, sp: SkinPart, st: State) {
    let skin = skin_of(sp);
    let _worn = skin::wear(skin);
    let s = &s.skinned(skin);
    let c = rect.center();
    match sp {
        SkinPart::Knob(_) => {
            let v = if st.lit { 0.8 } else { 0.62 };
            let rings = [(0.18, s.t.hue_mod)];
            let big = KnobLook {
                size: KnobSize::L,
                value: v,
                default: 0.0,
                rings: if st.lit { &rings } else { &[] },
                live: st.active.then_some(0.4),
            };
            if !big.rings.is_empty() {
                let r = Rect::from_center_size(
                    Pos2::new(rect.left() + 38.0, c.y),
                    Vec2::splat(KnobSize::L.diameter() + 8.0),
                );
                b.register(r, "modulation", &[], &[Some(Tok::HueMod)]);
            }
            if big.live.is_some() {
                let r =
                    Rect::from_center_size(Pos2::new(rect.left() + 38.0, c.y), Vec2::splat(60.0));
                b.register(r, "live", &[Tok::Panel], &[Some(Tok::HueMod)]);
            }
            knob::paint(b, Pos2::new(rect.left() + 38.0, c.y), s, st, &big);
            let small = KnobLook {
                default: 0.5,
                ..KnobLook::new(KnobSize::S, 1.0 - v, 0.5)
            };
            knob::paint(b, Pos2::new(rect.right() - 24.0, c.y), s, st, &small);
        }
        SkinPart::Switch(_) => {
            let r =
                Rect::from_center_size(Pos2::new(c.x, rect.top() + 16.0), Vec2::new(84.0, 20.0));
            led::button(b, r, s, st, Src::Str("Sync"));
            // The same shape as a card's power switch (the kit's square lamp, hollow when
            // off, for the plain and lamp shapes).
            let at =
                Rect::from_center_size(Pos2::new(c.x, rect.bottom() - 12.0), Vec2::splat(12.0));
            b.register(at, "card_lamp", &[Tok::KnobTrack], &[s.hue_tok]);
            band::lamp(
                b,
                Pos2::new(c.x, rect.bottom() - 12.0),
                s,
                st.lit || st.active,
                s.hue,
                st.alpha(),
            );
        }
        SkinPart::Material(_) => {
            shell::paint_ground(b, rect, s);
            let card = Rect::from_min_max(
                rect.min + Vec2::new(10.0, 14.0),
                rect.max - Vec2::new(10.0, 10.0),
            );
            let look = CardLook {
                title: Src::Str("Filter"),
                switchable: true,
                selector: None,
                menu: false,
                display: None,
            };
            let st2 = State {
                lit: !st.disabled && (st.lit || !st.hovered),
                ..st
            };
            let rr = band::card(b, card, s, st2, &look);
            knob::paint(
                b,
                Pos2::new(rr.body.left() + 20.0, rr.body.center().y),
                s,
                State {
                    disabled: st.disabled,
                    ..State::REST
                },
                &KnobLook::new(KnobSize::M, 0.6, 0.0),
            );
        }
        SkinPart::Display(_) => {
            let d = Rect::from_min_size(
                rect.min + Vec2::new(4.0, 4.0),
                Vec2::new(rect.width() - 8.0, 62.0),
            );
            skin::display_ground(b, d, s);
            let ink = skin::display_ink(s);
            let mut pts = [Pos2::ZERO; 40];
            let phase = if st.lit { 1.4 } else { 0.0 };
            for (i, p) in pts.iter_mut().enumerate() {
                let x = i as f32 / 39.0;
                let y = 0.5 - 0.32 * (x * 9.0 + phase).sin() * (1.0 - x * 0.6);
                *p = Pos2::new(d.left() + d.width() * x, d.top() + d.height() * y);
            }
            skin::displays::trace(b, &pts, 1.5, ink.trace.gamma_multiply(st.alpha()), &ink);
            skin::displays::cover(b, d, &ink);
            let r = Rect::from_min_size(
                Pos2::new(rect.left() + 4.0, d.bottom() + 6.0),
                Vec2::new(rect.width() - 8.0, 22.0),
            );
            well::readout(b, r, s, st, "-12.5 dB", true);
        }
        SkinPart::Plate(_) => {
            b.rect(b.snap_rect(rect), Fill::Solid(s.t.panel));
            skin::plates::stripe(b, rect, s, s.hue);
            let area = rect.shrink2(Vec2::new(6.0, 0.0));
            let name = if st.disabled { "Fervor" } else { "Aequalis" };
            skin::plates::nameplate(b, area, s, name, s.hue);
        }
        SkinPart::Ornaments => {
            shell::paint_ground(b, rect, s);
            let card = Rect::from_min_max(
                rect.min + Vec2::new(8.0, 8.0),
                rect.max - Vec2::new(8.0, 8.0),
            );
            let look = CardLook {
                title: Src::Str("Filter"),
                switchable: false,
                selector: None,
                menu: false,
                display: None,
            };
            band::card(b, card, s, st, &look);
        }
    }
}
