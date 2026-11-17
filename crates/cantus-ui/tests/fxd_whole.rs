//! fxd — the X-Y pad's and the formula controller's windows (ADRs 0147, 0148; design sheet
//! v1) never cut a word, shorten it or paint it past its card or the window (the kit's
//! clip-aware probe), in Slate English and Paper Spanish at 75 to 200 %, in every colourway;
//! and a language whose words take more rows grows the window, never the display.

use cantus_core::modulator::{FormulaSource, RouteRange, XyPath, XySource};
use cantus_ui::i18n::{self, Locale};
use cantus_ui::kit::text::probe;
use cantus_ui::views::fx::{fxd, fxd_formula, fxd_xy};

fn context(dark: bool) -> egui::Context {
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let theme = if dark {
        cantus_ui::theme::slate()
    } else {
        cantus_ui::theme::paper()
    };
    cantus_ui::theme::apply(&theme, &ctx);
    ctx
}

fn links() -> Vec<fxd_xy::Link> {
    vec![
        fxd_xy::Link {
            name: format!("{} · Cribrum", i18n::t("kit.param.cutoff")),
            axis: 0,
            range: Some(RouteRange {
                from: -0.5,
                to: 0.5,
                curve: 0.2,
            }),
            depth: 1.0,
        },
        fxd_xy::Link {
            name: format!("{} · Keys bus", i18n::t("auto.target.width")),
            axis: 1,
            range: None,
            depth: -0.5,
        },
    ]
}

fn size(_ctx: &egui::Context, formula: bool) -> egui::Vec2 {
    // Measured on a context of its own: a shot hands its fonts' atlas to the first raster
    // only, and the window's shot must have it.
    let mut size = egui::Vec2::ZERO;
    cantus_ui::raster::shoot(&context(true), egui::vec2(64.0, 64.0), 1.0, 1, None, |ui| {
        size = fxd::designed_size(ui, formula);
    });
    size
}

/// Every word not read whole in one window.
fn probe_window(formula: bool, look: u8, k: f32, dark: bool, error: bool) -> Vec<String> {
    let ctx = context(dark);
    fxd::shell_of(formula, look).set_scale(&ctx, k);
    let sz = size(&ctx, formula) * k;
    let context = i18n::t("fxd.sheet.context");
    // The longest well a window shows: the custom word (a preset's name is shorter).
    let preset = i18n::t("fxd.preset.custom");
    probe::start_clipped();
    cantus_ui::raster::shoot(&ctx, sz, 1.0, 3, None, |ui| {
        if formula {
            let src = FormulaSource {
                text: "a*Sin(SongTime*Pi*(Int(b*10)))+c".into(),
                inputs: [0.4, 0.3, 0.5],
                bipolar: false,
                links: Vec::new(),
                look,
            };
            let live = fxd_formula::Live {
                beats: 6.0,
                tempo: 120.0,
                draft: error.then(|| "a*Sin(SongTime*Pi*b".to_string()),
                caret: error.then_some(19),
                focused: error,
                links_first: 0,
            };
            let c = fxd::Chrome {
                preset: Some(&preset),
                ..fxd::Chrome::sheet(&context, true, &[])
            };
            fxd::formula(ui, &c, &src, 1.0, &links(), &live);
        } else {
            let src = XySource {
                inertia_ms: 300.0,
                path: Some(XyPath {
                    points: vec![[0.2, 0.2], [0.8, 0.5], [0.4, 0.9]],
                    beats: 4.0,
                    smooth: true,
                }),
                play_path: true,
                look,
                ..XySource::default()
            };
            let live = fxd_xy::Live {
                puck: [0.6, 0.4],
                heads: [0.7, 0.45],
                trail: vec![[0.3, 0.3], [0.45, 0.35], [0.6, 0.4]],
                path_at: Some(0.3),
                ..fxd_xy::Live::default()
            };
            let c = fxd::Chrome {
                preset: Some(&preset),
                ..fxd::Chrome::sheet(&context, true, &[])
            };
            fxd::xy(ui, &c, &src, 1.0, &links(), &live);
        }
    });
    let shorts = probe::take_shorts();
    let mut out: Vec<String> = probe::take()
        .into_iter()
        .chain(shorts)
        .map(|c| format!("{:?} -> {:?} ({})", c.full, c.shown, c.how))
        .collect();
    out.sort();
    out.dedup();
    out
}

#[test]
fn the_controllers_windows_never_cut_or_clip_a_word() {
    let mut bad = Vec::new();
    for (lang, dark) in [(Locale::En, true), (Locale::Es, false)] {
        i18n::set_locale(lang);
        for formula in [false, true] {
            for k in [0.75f32, 1.0, 1.5, 2.0] {
                for c in probe_window(formula, 0, k, dark, false) {
                    bad.push(format!("{formula} {lang:?} {k}: {c}"));
                }
            }
            for look in [1u8, 2] {
                for c in probe_window(formula, look, 1.0, dark, false) {
                    bad.push(format!("{formula} look {look} {lang:?}: {c}"));
                }
            }
        }
        for c in probe_window(true, 0, 1.0, dark, true) {
            bad.push(format!("formula error {lang:?}: {c}"));
        }
    }
    i18n::set_locale(Locale::En);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

/// Spanish words are longer: the window may take another row of cards, never a smaller
/// display (the owner's rule: the window grows rather than the display shrinking).
#[test]
fn a_longer_language_grows_the_window_not_the_display() {
    let ctx = context(true);
    for formula in [false, true] {
        i18n::set_locale(Locale::En);
        let en = size(&ctx, formula);
        i18n::set_locale(Locale::Es);
        let es = size(&ctx, formula);
        assert_eq!(en.x, es.x, "the width is the display's and the side's");
        assert!(es.y >= en.y);
    }
    i18n::set_locale(Locale::En);
    const { assert!(fxd_xy::PAD >= 440.0 && fxd_formula::SCREEN_W >= 520.0) };
}
