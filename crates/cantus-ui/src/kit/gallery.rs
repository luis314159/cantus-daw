//! The gallery (ADR 0024 §14): every part × every state, painted on the surface it
//! normally sits on. `tests/kit_gallery.rs` renders each sheet through the software
//! renderer in both default themes at 1×, 1.5× and 2× and compares it with its golden
//! image; `examples/proof.rs --gallery` writes them for review.

use super::band::{self, CardLook};
use super::brush::{Brush, Fill};
use super::cap::{self, CapLook, KeyFace};
use super::cell::{self, CellLook, Control};
use super::chrome;
use super::fader::{self, FaderLook};
use super::glyph::Glyph;
use super::knob::{self, KnobLook, KnobSize};
use super::led;
use super::menu::{self, Check, Item};
use super::meter::{self, Levels};
use super::segmented;
use super::text::{self, Role, Src, Text};
use super::tile::{self, TileLook};
use super::well;
use super::{Function, State, Style, Tok};
use egui::{Color32, Pos2, Rect, Vec2};

/// Every part of the kit (§4), one sheet each.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Part {
    Cap,
    Well,
    Tile,
    Led,
    Knob(KnobSize),
    Fader,
    Meter,
    Cell,
    Segmented,
    LedButton,
    MenuField,
    SectionBand,
    Card,
    ToolbarKey,
    Tab,
    Splitter,
    Scrollbar,
    Menu,
    /// A routing cable (round 25): every kind, faint, lit and selected.
    Cable,
}

/// Every sheet, with the name its golden image carries.
pub const PARTS: [(&str, Part); 23] = [
    ("cap", Part::Cap),
    ("well", Part::Well),
    ("tile", Part::Tile),
    ("led", Part::Led),
    ("knob_mini", Part::Knob(KnobSize::Mini)),
    ("knob_s", Part::Knob(KnobSize::S)),
    ("knob_m", Part::Knob(KnobSize::M)),
    ("knob_l", Part::Knob(KnobSize::L)),
    ("knob_xl", Part::Knob(KnobSize::XL)),
    ("fader", Part::Fader),
    ("meter", Part::Meter),
    ("cell", Part::Cell),
    ("segmented", Part::Segmented),
    ("led_button", Part::LedButton),
    ("menu_field", Part::MenuField),
    ("section_band", Part::SectionBand),
    ("card", Part::Card),
    ("toolbar_key", Part::ToolbarKey),
    ("tab", Part::Tab),
    ("splitter", Part::Splitter),
    ("scrollbar", Part::Scrollbar),
    ("menu", Part::Menu),
    ("cable", Part::Cable),
];

/// The room one state of a part takes on its sheet.
pub fn slot(part: Part) -> Vec2 {
    match part {
        Part::Cap => Vec2::new(44.0, 44.0),
        Part::Well => Vec2::new(96.0, 36.0),
        Part::Tile => Vec2::new(180.0, 30.0),
        Part::Led => Vec2::new(28.0, 28.0),
        Part::Knob(k) => Vec2::splat(k.diameter() + 20.0),
        Part::Fader => Vec2::new(40.0, 150.0),
        Part::Meter => Vec2::new(28.0, 130.0),
        Part::Cell => cell::size(KnobSize::M) + Vec2::splat(12.0),
        Part::Segmented => Vec2::new(170.0, 36.0),
        Part::LedButton => Vec2::new(96.0, 32.0),
        Part::MenuField => Vec2::new(170.0, 36.0),
        Part::SectionBand => Vec2::new(170.0, 32.0),
        Part::Card => Vec2::new(200.0, 150.0),
        Part::ToolbarKey => Vec2::new(40.0, 40.0),
        Part::Tab => Vec2::new(110.0, 34.0),
        Part::Splitter => Vec2::new(40.0, 80.0),
        Part::Scrollbar => Vec2::new(150.0, 24.0),
        Part::Menu => Vec2::new(210.0, 196.0),
        Part::Cable => Vec2::new(150.0, 150.0),
    }
}

/// A sheet: the six states side by side, a caption row under them.
pub fn sheet_size(part: Part) -> Vec2 {
    let s = slot(part);
    Vec2::new(
        8.0 + (s.x + 8.0) * State::ALL.len() as f32,
        8.0 + s.y + 18.0,
    )
}

/// The surface a part normally sits on.
pub fn ground_tok(part: Part) -> Tok {
    match part {
        Part::Fader | Part::Meter | Part::Knob(KnobSize::Mini) => Tok::Strip,
        Part::Tile => Tok::PanelLight,
        Part::ToolbarKey => Tok::Toolbar,
        Part::Tab | Part::Splitter | Part::Scrollbar | Part::Menu | Part::SectionBand => Tok::Panel,
        Part::Cable => Tok::Strip,
        Part::Card => Tok::DeviceGround,
        Part::Cap | Part::Led => Tok::Strip,
        _ => Tok::Deck,
    }
}

/// The colour of the surface a part normally sits on.
pub fn ground(part: Part, s: &Style) -> Color32 {
    s.t.get(ground_tok(part))
}

/// Paint a whole sheet with its top-left at `origin`.
pub fn paint_sheet(b: &mut Brush, origin: Pos2, s: &Style, part: Part) {
    let size = sheet_size(part);
    let rect = Rect::from_min_size(origin, size);
    b.register(rect, "gallery", &[Tok::TextDim], &[Some(ground_tok(part))]);
    b.rect(b.snap_rect(rect), Fill::Solid(ground(part, s)));
    let sl = slot(part);
    for (i, (name, st)) in State::ALL.iter().enumerate() {
        let at = origin + Vec2::new(8.0 + (sl.x + 8.0) * i as f32, 8.0);
        paint_part(b, Rect::from_min_size(at, sl), s, part, *st);
        let cap = Rect::from_min_size(at + Vec2::new(0.0, sl.y + 2.0), Vec2::new(sl.x, 14.0));
        let t = Text::new(name, Role::Micro, s.t.text_dim);
        let _ = text::paint(b, cap, &t, 1.0);
    }
}

const TICKS: [f32; 8] = [0.0, 0.14, 0.28, 0.42, 0.56, 0.7, 0.8, 0.9];

/// Paint `part` in `st` centred in `rect`, with representative content.
pub fn paint_part(b: &mut Brush, rect: Rect, s: &Style, part: Part, st: State) {
    let c = rect.center();
    let s = &s.function(Function::Tone);
    match part {
        Part::Cap => {
            let r = Rect::from_center_size(c, Vec2::new(18.0, 28.0));
            cap::paint(
                b,
                r,
                s,
                st,
                &CapLook {
                    groove: Some(0.3),
                    ..Default::default()
                },
            );
        }
        Part::Well => well::readout(
            b,
            Rect::from_center_size(c, Vec2::new(84.0, 22.0)),
            s,
            st,
            "-12.5 dB",
            true,
        ),
        Part::Tile => {
            let r = Rect::from_center_size(c, Vec2::new(172.0, tile::HEIGHT));
            let look = TileLook {
                name: if st.disabled {
                    None
                } else {
                    Some(Src::Str("Fruity Reverb 2"))
                },
                placeholder: Src::Str("Slot 5"),
                mix: Some(0.7),
                on: !st.disabled,
            };
            tile::paint(b, r, s, st, &look);
        }
        Part::Led => led::paint(
            b,
            c,
            11.0,
            st.lit || st.active,
            s,
            Tok::Led,
            Tok::Strip,
            st.alpha(),
        ),
        Part::Knob(size) => {
            let v = if st.lit { 0.8 } else { 0.66 };
            let rings = [(0.18, s.t.hue_mod)];
            let look = KnobLook {
                size,
                value: v,
                default: if size == KnobSize::Mini { 0.5 } else { 0.0 },
                rings: if st.lit && size != KnobSize::Mini {
                    &rings
                } else {
                    &[]
                },
                live: None,
            };
            if !look.rings.is_empty() {
                // The ring's colour is a modulation source's: here, the modulation hue.
                let r = Rect::from_center_size(c, Vec2::splat(size.diameter() + 8.0));
                b.register(r, "modulation", &[], &[Some(Tok::HueMod)]);
            }
            knob::paint(b, c, s, st, &look);
        }
        Part::Fader => {
            let r = Rect::from_center_size(c, Vec2::new(30.0, 140.0));
            fader::paint(
                b,
                r,
                s,
                st,
                &FaderLook {
                    pos: 0.62,
                    ticks: &TICKS,
                    unity: Some(0.8),
                },
            );
        }
        Part::Meter => {
            let r = Rect::from_center_size(c, Vec2::new(14.0, 120.0));
            let l = match st {
                s if s.lit => Levels {
                    level: [1.5, -2.0],
                    hold: [2.4, -0.5],
                },
                s if s.disabled => Levels::SILENT,
                s if s.active => Levels {
                    level: [-4.0, -5.0],
                    hold: [-2.0, -3.0],
                },
                _ => Levels {
                    level: [-14.0, -18.0],
                    hold: [-9.0, -11.0],
                },
            };
            meter::paint(b, r, s, &l);
        }
        Part::Cell => {
            let size = cell::size(KnobSize::M);
            let r = Rect::from_center_size(c, size);
            let look = CellLook {
                control: Control::Knob(KnobLook::new(KnobSize::M, 0.62, 0.0)),
                value: "2.40 kHz",
                label: Src::Key("kit.param.cutoff"),
            };
            cell::paint(b, r, s, st, &look);
        }
        Part::Segmented => {
            let r = Rect::from_center_size(c, Vec2::new(160.0, 24.0));
            let labels = [Src::Str("Limit"), Src::Str("Comp"), Src::Str("Gate")];
            let hov = st.hovered.then_some(2);
            segmented::paint(b, r, s, st, &labels, if st.lit { 1 } else { 0 }, hov);
        }
        Part::LedButton => {
            led::button(
                b,
                Rect::from_center_size(c, Vec2::new(84.0, 20.0)),
                s,
                st,
                Src::Str("Sync"),
            );
        }
        Part::MenuField => {
            let r = Rect::from_center_size(c, Vec2::new(160.0, 24.0));
            well::menu_field(b, r, s, st, "Ladder 24 dB", true);
        }
        Part::SectionBand => {
            let r = Rect::from_center_size(c, Vec2::new(164.0, band::BAND));
            band::section(b, r, s, st, Src::Str("Envelope"), Some(st.lit), false);
        }
        Part::Card => {
            let r = Rect::from_center_size(c, Vec2::new(192.0, 140.0));
            let look = CardLook {
                title: Src::Str("Filter"),
                switchable: true,
                selector: Some("Ladder"),
                menu: true,
                display: Some(40.0),
            };
            let st2 = State {
                lit: !st.disabled && (st.lit || !st.hovered),
                ..st
            };
            let rr = band::card(b, r, s, st2, &look);
            if let Some(d) = rr.display {
                let mut pts = [Pos2::ZERO; 24];
                for (i, p) in pts.iter_mut().enumerate() {
                    let x = i as f32 / 23.0;
                    let y = 0.35 + 0.3 * (1.0 / (1.0 + ((x - 0.55) * 9.0).exp()));
                    *p = Pos2::new(d.left() + d.width() * x, d.top() + d.height() * y);
                }
                b.polyline(&pts, 2.0, s.hue.gamma_multiply(st.alpha()));
            }
            let k = KnobLook::new(KnobSize::S, 0.55, 0.0);
            knob::paint(
                b,
                Pos2::new(rr.body.left() + 22.0, rr.body.center().y),
                s,
                State {
                    disabled: st.disabled,
                    ..State::REST
                },
                &k,
            );
        }
        Part::ToolbarKey => cap::toolbar_key(
            b,
            Rect::from_center_size(c, Vec2::splat(28.0)),
            s,
            st,
            KeyFace::Glyph(Glyph::Record),
        ),
        Part::Tab => chrome::tab(
            b,
            Rect::from_center_size(c, Vec2::new(100.0, chrome::TAB)),
            s,
            st,
            Src::Str("Mixer"),
        ),
        Part::Splitter => chrome::splitter(
            b,
            Rect::from_center_size(c, Vec2::new(chrome::SPLITTER, 70.0)),
            s,
            st,
        ),
        Part::Scrollbar => {
            let r = Rect::from_center_size(c, Vec2::new(140.0, chrome::SCROLLBAR));
            chrome::scrollbar(b, r, s, st, 0.3, 0.35);
        }
        Part::Menu => {
            let items = [
                Item::Band(Src::Str("View")),
                Item::Action {
                    label: Src::Str("Compact strips"),
                    check: Check::Radio(true),
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                },
                Item::Action {
                    label: Src::Str("Wide strips"),
                    check: Check::Radio(false),
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                },
                Item::Separator,
                Item::Action {
                    label: Src::Str("Show sends"),
                    check: Check::On,
                    shortcut: Some("S"),
                    submenu: false,
                    enabled: true,
                },
                Item::Action {
                    label: Src::Str("Colourful mixer"),
                    check: Check::None,
                    shortcut: None,
                    submenu: true,
                    enabled: true,
                },
                Item::Action {
                    label: Src::Str("Reset layout"),
                    check: Check::None,
                    shortcut: Some("Ctrl+R"),
                    submenu: false,
                    enabled: !st.disabled,
                },
                Item::Band(Src::Str("Routing")),
                Item::Action {
                    label: Src::Str("Route to this track"),
                    check: Check::None,
                    shortcut: None,
                    submenu: false,
                    enabled: true,
                },
            ];
            let h = menu::height(&items, 1.0);
            let r = Rect::from_min_size(
                rect.min + Vec2::new(4.0, 4.0),
                Vec2::new(rect.width() - 8.0, h),
            );
            let hovered = (st.hovered || st.active).then_some(4);
            menu::paint(b, r, s, &items, hovered);
        }
        Part::Cable => cable_sheet(b, rect, s, st),
    }
}

/// Every cable kind in one state, one under another, from a port at the left to one at
/// the right: rest is *All* mode's faint cable, active the selected one (its halo),
/// disabled a switched-off send, the others lit; the last row a doubled send.
fn cable_sheet(b: &mut Brush, rect: Rect, s: &Style, st: State) {
    use super::cable::{self, CableKind, CableLook};
    let kinds = [
        CableKind::Output,
        CableKind::Send {
            on: !st.disabled,
            pre: false,
        },
        CableKind::Send {
            on: !st.disabled,
            pre: true,
        },
        CableKind::Key,
        CableKind::Master,
        CableKind::Send {
            on: true,
            pre: false,
        },
    ];
    // On the desk a cable wears its strip's colour; here the space hue stands in for it.
    let color = s.t.hue_space;
    b.register(rect, "cable", &[], &[Some(Tok::HueSpace)]);
    let rows = kinds.len() as f32;
    let h = rect.height() / rows;
    for (i, kind) in kinds.iter().enumerate() {
        let y = rect.top() + 4.0 + i as f32 * h;
        let (from, to) = (
            Pos2::new(rect.left() + 10.0, y),
            Pos2::new(rect.right() - 10.0 - i as f32 * 8.0, y),
        );
        let look = CableLook {
            kind: *kind,
            color,
            lit: st.hovered || st.active || st.lit || st.focused || st.disabled,
            selected: st.active,
            doubled: i == kinds.len() - 1,
            mark: Some(cable::path(from, to, h - 7.0).mark),
            related: false,
        };
        cable::paint(b, s, from, to, h - 7.0, &look);
        for p in [from, to] {
            b.circle_stroke(p, s.s(3.5), s.s(1.4), color);
        }
    }
}

/// Encode a rendered canvas as a compressed PNG.
pub fn encode_png(c: &crate::raster::Canvas) -> Option<Vec<u8>> {
    let mut pix = tiny_skia::Pixmap::new(c.width as u32, c.height as u32)?;
    for y in 0..c.height {
        for x in 0..c.width {
            let [r, g, b, _] = c.pixel(x, y);
            pix.pixels_mut()[y * c.width + x] =
                tiny_skia::PremultipliedColorU8::from_rgba(r, g, b, 255)?;
        }
    }
    pix.encode_png().ok()
}

/// Decode a PNG into (width, height, RGBA pixels).
pub fn decode_png(bytes: &[u8]) -> Option<(usize, usize, Vec<[u8; 4]>)> {
    let pix = tiny_skia::Pixmap::decode_png(bytes).ok()?;
    let px = pix
        .pixels()
        .iter()
        .map(|p| {
            let c = p.demultiply();
            [c.red(), c.green(), c.blue(), c.alpha()]
        })
        .collect();
    Some((pix.width() as usize, pix.height() as usize, px))
}

/// A context ready to render the kit offscreen: the bundled fonts and the type scale.
pub fn offscreen_context() -> egui::Context {
    let ctx = egui::Context::default();
    crate::design::install_fonts(&ctx);
    crate::design::install_style(&ctx);
    ctx
}

/// Render `paint` into a canvas of `size` points at `ppp`, through the software renderer,
/// in a context of its own (the renderer keeps no textures between renders, so every
/// image starts from a fresh font atlas and layer atlas — and is deterministic).
pub fn render(
    size: Vec2,
    ppp: f32,
    mut paint: impl FnMut(&mut Brush, &egui::Painter),
) -> crate::raster::Canvas {
    let ctx = offscreen_context();
    crate::raster::shoot(&ctx, size, ppp, 3, None, |ui| {
        let painter = ui.painter().clone();
        super::with_brush(&painter, |b| paint(b, &painter));
    })
}

/// Render a whole `Ui` (a widget such as the device shell) offscreen.
pub fn render_ui(
    size: Vec2,
    ppp: f32,
    mut add: impl FnMut(&mut egui::Ui),
) -> crate::raster::Canvas {
    let ctx = offscreen_context();
    crate::raster::shoot(&ctx, size, ppp, 3, None, |ui| add(ui))
}

/// Render one part's sheet.
pub fn render_sheet(s: &Style, part: Part, ppp: f32) -> crate::raster::Canvas {
    render(sheet_size(part), ppp, |b, _| {
        paint_sheet(b, Pos2::ZERO, s, part)
    })
}
