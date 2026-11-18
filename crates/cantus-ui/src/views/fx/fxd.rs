//! fxd — the controllers' windows (ADRs 0144, 0145; design sheet v1): the kit's unit shell
//! in the controller's own face and colourway — its name engraved in the header, the host
//! it lives on beside it, the on lamp, the window keys — over the face's body, and a footer
//! saying what it drives. Opened from the modulator's row in the inspector (step 3).

use super::{fxd_formula, fxd_xy};
use crate::kit::shell::{self, Footer, Header, Shell};
use crate::kit::skin::{self, Skin};
use crate::kit::tokens::Function;
use cantus_core::modulator::{FormulaSource, XySource};
use egui::Vec2;

/// The face of a controller in colourway `look`.
pub fn skin_of(formula: bool, look: u8) -> &'static Skin {
    let ways = if formula {
        &skin::FORMULA_COLOURWAYS
    } else {
        &skin::XY_COLOURWAYS
    };
    ways[usize::from(look).min(ways.len() - 1)].sheet
}

/// The window's shell.
pub fn shell_of(formula: bool, look: u8) -> Shell {
    Shell::new(("cantus-fxd", formula)).skin(skin_of(formula, look))
}

/// The window's size at 100 % in the current language: header, body, footer.
pub fn designed_size(ui: &egui::Ui, formula: bool) -> Vec2 {
    let body = if formula {
        fxd_formula::body_size(ui)
    } else {
        fxd_xy::body_size(ui)
    };
    body + Vec2::new(0.0, shell::HEADER + shell::FOOTER)
}

/// What the window's frame shows around a face: the host words, the on lamp, the preset
/// well's name, the footer's chips, the window's keys.
#[derive(Copy, Clone, Debug)]
pub struct Chrome<'a> {
    /// "(Modulator 2 · Master)".
    pub context: &'a str,
    pub on: bool,
    /// The preset well's name (a preset the settings match, or the custom word); `None`
    /// shows no well (the design sheets).
    pub preset: Option<&'a str>,
    pub chips: &'a [&'a str],
    /// The window's keys (minimise, maximise, close); `None` is close only.
    pub window: Option<crate::kit::window_ctl::WindowCtl>,
}

impl<'a> Chrome<'a> {
    /// The design sheets' frame: no preset well, close only.
    pub fn sheet(context: &'a str, on: bool, chips: &'a [&'a str]) -> Chrome<'a> {
        Chrome {
            context,
            on,
            preset: None,
            chips,
            window: None,
        }
    }
}

/// A face shown in its window: what the user did on the face, where its parts were, and
/// what the frame reported (moves, the bypass, presets, the window's keys).
pub struct Shown<A, R> {
    pub actions: Vec<A>,
    pub rects: R,
    pub shell: shell::Output,
    /// Where the header's parts were (the preset well opens its list under it).
    pub header: shell::HeaderRects,
}

/// Where the header's parts were drawn.
fn header_rects(sh: &Shell, out: &shell::Output, head: &Header<'_>) -> shell::HeaderRects {
    let s = crate::kit::Style::plain()
        .scaled(out.k)
        .function(Function::Mod)
        .skinned(sh.skin);
    shell::header_rects(out.header, &s, head)
}

fn header<'a>(ui: &egui::Ui, formula: bool, look: u8, name: &'a str, c: &Chrome<'a>) -> Header<'a> {
    let look_skin = skin_of(formula, look);
    let name_w = if c.preset.is_some() {
        super::frame::header_name_w(ui.ctx(), look_skin, name, c.context)
    } else {
        0.0
    };
    Header {
        name,
        context: c.context,
        hue: Function::Mod,
        preset: c.preset.map(|p| shell::Preset {
            name: p,
            modified: false,
            favourite: false,
        }),
        ab: None,
        undo: None,
        views: None,
        views_w: 0.0,
        bypassed: Some(!c.on),
        help: true,
        name_w,
        window: Some(c.window.unwrap_or(crate::kit::window_ctl::WindowCtl {
            swap: None,
            minimise: false,
            maximised: None,
            close: true,
        })),
    }
}

/// The X-Y pad's window, filling `ui`.
pub fn xy(
    ui: &mut egui::Ui,
    c: &Chrome<'_>,
    src: &XySource,
    amount: f32,
    links: &[fxd_xy::Link],
    live: &fxd_xy::Live,
) -> Shown<fxd_xy::Action, fxd_xy::Rects> {
    let name = crate::i18n::t("fxd.xy.name");
    let sh = shell_of(false, src.look);
    let mut actions = Vec::new();
    let mut rects = fxd_xy::Rects::default();
    let head = header(ui, false, src.look, &name, c);
    let shell = sh.show(
        ui,
        &head,
        &Footer {
            chips: c.chips,
            meter: None,
        },
        |ui, s, r| {
            (actions, rects) = fxd_xy::body(ui, s, r, src, amount, links, live);
        },
    );
    let header = header_rects(&sh, &shell, &head);
    Shown {
        actions,
        rects,
        shell,
        header,
    }
}

/// The formula controller's window, filling `ui`.
pub fn formula(
    ui: &mut egui::Ui,
    c: &Chrome<'_>,
    src: &FormulaSource,
    amount: f32,
    links: &[fxd_xy::Link],
    live: &fxd_formula::Live,
) -> Shown<fxd_formula::Action, fxd_formula::Rects> {
    let name = crate::i18n::t("fxd.formula.name");
    let sh = shell_of(true, src.look);
    let mut actions = Vec::new();
    let mut rects = fxd_formula::Rects::default();
    let head = header(ui, true, src.look, &name, c);
    let shell = sh.show(
        ui,
        &head,
        &Footer {
            chips: c.chips,
            meter: None,
        },
        |ui, s, r| {
            (actions, rects) = fxd_formula::body(ui, s, r, src, amount, links, live);
        },
    );
    let header = header_rects(&sh, &shell, &head);
    Shown {
        actions,
        rects,
        shell,
        header,
    }
}
