//! fxd — the controllers' windows (ADRs 0147, 0148; design sheet v1): the kit's unit shell
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

fn header<'a>(name: &'a str, context: &'a str, on: bool, name_w: f32) -> Header<'a> {
    Header {
        name,
        context,
        hue: Function::Mod,
        preset: None,
        ab: None,
        undo: None,
        views: None,
        bypassed: Some(!on),
        help: true,
        name_w,
        window: Some(crate::kit::window_ctl::WindowCtl {
            swap: None,
            minimise: false,
            maximised: None,
            close: true,
        }),
    }
}

/// The X-Y pad's window, filling `ui`.
#[allow(clippy::too_many_arguments)]
pub fn xy(
    ui: &mut egui::Ui,
    context: &str,
    src: &XySource,
    amount: f32,
    on: bool,
    links: &[fxd_xy::Link],
    live: &fxd_xy::Live,
    chips: &[&str],
) -> Vec<fxd_xy::Action> {
    let name = crate::i18n::t("fxd.xy.name");
    let sh = shell_of(false, src.look);
    let mut out = Vec::new();
    sh.show(
        ui,
        &header(&name, context, on, 0.0),
        &Footer { chips, meter: None },
        |ui, s, r| {
            out = fxd_xy::body(ui, s, r, src, amount, links, live);
        },
    );
    out
}

/// The formula controller's window, filling `ui`.
#[allow(clippy::too_many_arguments)]
pub fn formula(
    ui: &mut egui::Ui,
    context: &str,
    src: &FormulaSource,
    amount: f32,
    on: bool,
    links: &[fxd_xy::Link],
    live: &fxd_formula::Live,
    chips: &[&str],
) -> Vec<fxd_formula::Action> {
    let name = crate::i18n::t("fxd.formula.name");
    let sh = shell_of(true, src.look);
    let mut out = Vec::new();
    sh.show(
        ui,
        &header(&name, context, on, 0.0),
        &Footer { chips, meter: None },
        |ui, s, r| {
            out = fxd_formula::body(ui, s, r, src, amount, links, live);
        },
    );
    out
}
