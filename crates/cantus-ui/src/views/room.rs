//! The room the views paint in (round 39, `des-views`): the design's names for a theme's
//! tokens — gutter, base, surface, raised, well, ink 1–4, the accent — as the piano roll,
//! the channel rack, the browser and the clips' content spend them, and the colours the
//! design added (the keyboard's ivory and ebony), derived from the tokens so every theme
//! has them and none is spelled out (`docs/round39/design/des-views/README.md`).
//!
//! In Slate the derivations land on the design's values (the tests hold them to a step of
//! a few levels); in the other families they follow those families' own tokens.

use crate::kit::color::{mix, opaque, over};
use crate::kit::tokens::Tokens;
use egui::Color32;

/// The design's names, resolved from a theme's tokens.
#[derive(Copy, Clone, Debug)]
pub struct Room {
    pub dark: bool,
    /// Between panels; the darkest value.
    pub gutter: Color32,
    /// Panel bodies, lanes, the roll's white-key rows.
    pub base: Color32,
    /// Bars, heads, rows of the rack.
    pub surface: Color32,
    /// Lifted pills, name plates.
    pub raised: Color32,
    /// Inside a key or field.
    pub well: Color32,
    /// A well under the pointer.
    pub well_hi: Color32,
    /// A well's 1-px dark edge and its 1-px light lip.
    pub edge: Color32,
    pub lip: Color32,
    /// The grid's bar and beat lines (design `grid_bar`, `grid_beat`) and hairlines.
    pub grid_bar: Color32,
    pub grid_beat: Color32,
    pub line: Color32,
    pub ink1: Color32,
    pub ink2: Color32,
    pub ink3: Color32,
    pub ink4: Color32,
    pub accent: Color32,
    pub play: Color32,
    pub solo: Color32,
    pub auto: Color32,
    pub white: Color32,
    pub black: Color32,
}

impl Room {
    pub fn of(t: &Tokens) -> Room {
        let dark = t.dark;
        let solid = |c: Color32, under: Color32| opaque(over(c, under));
        Room {
            dark,
            gutter: t.ink,
            base: t.panel,
            surface: t.toolbar,
            raised: mix(t.toolbar, t.menu, if dark { 0.5 } else { 1.0 }),
            well: t.well,
            well_hi: mix(t.well, t.toolbar, 0.5),
            edge: solid(t.inner_shadow, t.well),
            lip: solid(t.lip, t.toolbar),
            grid_bar: t.grid_strong,
            grid_beat: t.grid,
            line: t.grid_bar,
            ink1: t.text,
            ink2: t.text_dim,
            ink3: mix(t.text_dim, t.placeholder, 0.5),
            ink4: mix(t.panel, t.edge, if dark { 0.46 } else { 0.52 }),
            accent: t.selection,
            play: t.led,
            solo: t.meter_high,
            auto: t.hue_mod,
            white: t.light,
            black: t.dark_shade,
        }
    }

    /// The darker of the two inks a light ground needs (the gutter in a dark theme, the
    /// primary text in a light one).
    pub fn deep(&self) -> Color32 {
        if self.dark {
            self.gutter
        } else {
            self.ink1
        }
    }
}

/// The keyboard's materials (design: `sep`, `ivory`, `ivory_hi`, `ebony`, `ebony_top`,
/// `ebony_front`) and its label inks.
#[derive(Copy, Clone, Debug)]
pub struct Piano {
    /// The bed the white keys lie on; it shows in the 1-px gap between them.
    pub bed: Color32,
    pub ivory: Color32,
    pub ivory_hi: Color32,
    pub ebony: Color32,
    pub ebony_top: Color32,
    pub ebony_front: Color32,
    /// The black key's lit upper side and front end.
    pub ebony_lit: Color32,
    /// Middle C's name, and every other C's.
    pub c4: Color32,
    pub c: Color32,
    /// A white key's name from 16-px rows, quieter.
    pub name: Color32,
}

impl Piano {
    pub fn of(t: &Tokens) -> Piano {
        let r = Room::of(t);
        let dark = t.dark;
        let ivory = mix(t.key_white, t.panel, if dark { 0.07 } else { 0.33 });
        let ink = r.deep();
        let ebony_front = mix(t.key_black, t.light, if dark { 0.14 } else { 0.22 });
        Piano {
            bed: if dark {
                mix(t.edge, t.ink, 0.33)
            } else {
                mix(t.edge, ivory, 0.41)
            },
            ivory,
            ivory_hi: mix(ivory, t.light, 0.4),
            ebony: mix(t.key_black, t.dark_shade, if dark { 0.15 } else { 0.10 }),
            ebony_top: mix(t.key_black, t.light, 0.065),
            ebony_front,
            ebony_lit: mix(ebony_front, t.light, if dark { 0.12 } else { 0.18 }),
            c4: mix(ivory, ink, 0.92),
            c: mix(ivory, ink, if dark { 0.72 } else { 0.64 }),
            name: mix(ivory, ink, if dark { 0.50 } else { 0.45 }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: Color32, hex: u32, tol: i32) -> bool {
        let [r, g, b, _] = a.to_array();
        let want = [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8];
        [r, g, b]
            .iter()
            .zip(want)
            .all(|(&x, w)| (x as i32 - w as i32).abs() <= tol)
    }

    /// In Slate the derived materials land on the design's values (des-views README,
    /// *Keyboard*), within a few levels.
    #[test]
    fn slate_s_keyboard_is_the_design_s() {
        let n = Piano::of(&crate::theme_slate::slate_night_tokens());
        let p = Piano::of(&crate::theme_slate::slate_paper_tokens());
        for (got, want, what) in [
            (n.ivory, 0xcfd5db, "night ivory"),
            (n.ivory_hi, 0xe2e6ea, "night ivory_hi"),
            (n.bed, 0x59636e, "night sep"),
            (n.ebony, 0x161a1f, "night ebony"),
            (n.ebony_top, 0x262d34, "night ebony_top"),
            (n.ebony_front, 0x3a434c, "night ebony_front"),
            (p.ivory, 0xfbfcfd, "paper ivory"),
            (p.bed, 0xa3adb8, "paper sep"),
            (p.ebony, 0x262c33, "paper ebony"),
            (p.ebony_top, 0x39414a, "paper ebony_top"),
            (p.ebony_front, 0x58626c, "paper ebony_front"),
        ] {
            assert!(near(got, want, 8), "{what}: {got:?} against #{want:06x}");
        }
        let rn = Room::of(&crate::theme_slate::slate_night_tokens());
        assert!(near(rn.ink3, 0x909ca8, 6), "{:?}", rn.ink3);
        assert!(near(rn.ink4, 0x47515b, 6), "{:?}", rn.ink4);
        assert!(near(rn.well_hi, 0x20262d, 3), "{:?}", rn.well_hi);
    }

    /// The keys' names read on ivory in every shipped theme: middle C at 7:1, the other
    /// Cs at 4.5:1.
    #[test]
    fn the_c_names_read_on_the_ivory_in_every_theme() {
        use crate::kit::color::contrast;
        for (id, make) in crate::kit::appearance::PRESETS {
            let t = make().kit_tokens();
            let k = Piano::of(&t);
            assert!(contrast(k.c4, k.ivory) >= 7.0, "{id}: C4");
            assert!(contrast(k.c, k.ivory) >= 4.5, "{id}: C");
        }
    }
}
