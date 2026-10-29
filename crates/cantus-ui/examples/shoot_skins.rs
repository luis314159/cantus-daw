//! Every unit's face, without a screen (round 32, ADR 0074).
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_skins -- [out-dir] [unit…] [--plain] [--gallery] [--quick]
//! ```
//!
//! - **Shots**: each unit's editor in its device frame, in Slate and in Paper, at the
//!   window scales 100 % and 200 %, named `<unit>-<theme>-<scale>.png` (`aequalis-slate-100.png`).
//!   Give unit names (`aequalis spatium unda pulsus`) to shoot only those.
//! - `--plain`: one more shot of each, Slate at 100 %, with *Units wear their own faces*
//!   off (`<unit>-slate-100-plain.png`) — the kit's plain look, to compare.
//! - `--gallery`: the kit's skin sheets (every material, display look, knob and switch
//!   shape, plate and the ornaments) in both themes at 2×, `gallery-<sheet>-<theme>.png`.
//! - `--quick`: Slate at 100 % only.
//! - `--try <sample>`: every unit shot wears one of the sample sheets below instead of its
//!   own (`studio`: FabFilter-like matte with value rings; `vintage`: wood, a VU-era
//!   phosphor, skirted knobs and bat toggles; `machine`: vinyl amp covering, an LCD, soft knobs and lamp
//!   buttons; `lab`: brushed metal, a scope, dial knobs, mono readouts) — named
//!   `<unit>-<theme>-<scale>-<sample>.png`. For trying a look on a real editor.
//!
//! PNGs go to `target/shots/skins` by default. The engine is the dummy one; each editor
//! is fed what a playing engine would by its own `demo_feed`.

use cantus_core::inst::InstrumentKind;
use cantus_core::EffectKind;
use cantus_engine::EngineHandle;
use cantus_ui::kit::color::Hsl;
use cantus_ui::kit::gallery::{self, Part, PARTS};
use cantus_ui::kit::shell::{self, Shell};
use cantus_ui::kit::skin::{
    self, Case, Display, KnobShape, Lettering, Material, Ornaments, Pair, Plate, Skin, SwitchShape,
    Tint, Weight,
};

const fn hsl(h: f32, s: f32, l: f32) -> Hsl {
    Hsl { h, s, l }
}

/// Sample sheets, to try on any unit (`--try`).
static SAMPLES: [(&str, Skin); 4] = [
    (
        "studio",
        Skin {
            surface: Tint {
                hue: 215.0,
                sat: 0.18,
                amount: 0.5,
            },
            accent: Some(Pair {
                dark: hsl(48.0, 0.92, 0.58),
                light: hsl(42.0, 0.9, 0.40),
            }),
            accent2: None,
            material: Material::Matte,
            display: Display::Glass,
            knob: KnobShape::Arc,
            switch: SwitchShape::Pill,
            lettering: Lettering {
                case: Case::AsIs,
                tracking: 0.02,
                weight: Weight::SemiBold,
                condensed: false,
                mono_values: false,
            },
            plate: Plate::Plain,
            ornaments: Ornaments {
                screws: false,
                vents: false,
                rules: false,
                rack_ears: false,
            },
            ..Skin::plain("sample.studio")
        },
    ),
    (
        "vintage",
        Skin {
            surface: Tint {
                hue: 30.0,
                sat: 0.30,
                amount: 0.55,
            },
            accent: Some(Pair {
                dark: hsl(34.0, 0.88, 0.58),
                light: hsl(28.0, 0.85, 0.40),
            }),
            accent2: Some(Pair::both(hsl(160.0, 0.6, 0.5))),
            material: Material::Wood,
            display: Display::Phosphor,
            knob: KnobShape::Skirted,
            switch: SwitchShape::Bat,
            lettering: Lettering {
                case: Case::Upper,
                tracking: 0.16,
                weight: Weight::Regular,
                condensed: false,
                mono_values: false,
            },
            plate: Plate::Engraved,
            ornaments: Ornaments {
                screws: true,
                vents: true,
                rules: true,
                rack_ears: false,
            },
            ..Skin::plain("sample.vintage")
        },
    ),
    (
        "machine",
        Skin {
            surface: Tint {
                hue: 0.0,
                sat: 0.04,
                amount: 0.8,
            },
            accent: Some(Pair {
                dark: hsl(8.0, 0.85, 0.58),
                light: hsl(6.0, 0.8, 0.45),
            }),
            accent2: None,
            material: Material::Tolex,
            display: Display::Lcd,
            knob: KnobShape::Soft,
            switch: SwitchShape::Lamp,
            lettering: Lettering {
                case: Case::Upper,
                tracking: 0.06,
                weight: Weight::SemiBold,
                condensed: true,
                mono_values: false,
            },
            plate: Plate::Badge,
            ornaments: Ornaments {
                screws: true,
                vents: false,
                rules: false,
                rack_ears: true,
            },
            ..Skin::plain("sample.machine")
        },
    ),
    (
        "lab",
        Skin {
            surface: Tint {
                hue: 200.0,
                sat: 0.10,
                amount: 0.6,
            },
            accent: Some(Pair {
                dark: hsl(186.0, 0.8, 0.55),
                light: hsl(190.0, 0.85, 0.35),
            }),
            accent2: None,
            material: Material::Brushed,
            display: Display::Scope,
            knob: KnobShape::Dial,
            switch: SwitchShape::Rocker,
            lettering: Lettering {
                case: Case::Upper,
                tracking: 0.12,
                weight: Weight::Medium,
                condensed: true,
                mono_values: true,
            },
            plate: Plate::Legend,
            ornaments: Ornaments {
                screws: true,
                vents: false,
                rules: true,
                rack_ears: false,
            },
            ..Skin::plain("sample.lab")
        },
    ),
];
use cantus_ui::kit::{Style, Tokens};
use cantus_ui::raster::shoot;
use cantus_ui::{views, welcome_project, CantusApp};
use std::path::{Path, PathBuf};

/// What a unit is in the app: an effect, an instrument, the drum machine.
#[derive(Copy, Clone)]
enum What {
    Fx(EffectKind),
    Inst(InstrumentKind),
    Pulsus,
}

/// Every unit with an editor of ours: its name (the shot's) and what `demo_editor` opens.
const UNITS: &[(&str, &str, What)] = &[
    ("aequalis", "eq", What::Fx(EffectKind::Eq)),
    ("comprimo", "comp", What::Fx(EffectKind::Compressor)),
    ("terminus", "limiter", What::Fx(EffectKind::Limiter)),
    ("multiplex", "multiband", What::Fx(EffectKind::Multiband)),
    ("sibilus", "deesser", What::Fx(EffectKind::DeEsser)),
    ("porta", "gate", What::Fx(EffectKind::Gate)),
    ("culmen", "culmen", What::Fx(EffectKind::Maximizer)),
    ("tonsor", "tonsor", What::Fx(EffectKind::Clipper)),
    ("impetus", "impetus", What::Fx(EffectKind::Transient)),
    (
        "discrimen",
        "discrimen",
        What::Fx(EffectKind::TransientSplit),
    ),
    ("fervor", "fervor", What::Fx(EffectKind::Saturator)),
    ("spatium", "space", What::Fx(EffectKind::Space)),
    ("latitudo", "latitudo", What::Fx(EffectKind::Stereo)),
    ("speculum", "speculum", What::Fx(EffectKind::Analyzer)),
    ("tonus", "tonus", What::Fx(EffectKind::AutoTune)),
    ("concentus", "concentus", What::Fx(EffectKind::Chorus)),
    ("flexus", "flexus", What::Fx(EffectKind::Flanger)),
    ("phasis", "phasis", What::Fx(EffectKind::Phaser)),
    ("tremor", "tremor", What::Fx(EffectKind::Tremolo)),
    ("vibrans", "vibrans", What::Fx(EffectKind::Vibrato)),
    ("anulus", "anulus", What::Fx(EffectKind::RingMod)),
    ("translatio", "translatio", What::Fx(EffectKind::FreqShift)),
    ("fluctus", "fluctus", What::Fx(EffectKind::VolumeShaper)),
    ("unda", "unda", What::Inst(InstrumentKind::Wavetable)),
    ("calor", "calor", What::Inst(InstrumentKind::Analog)),
    ("modulus", "modulus", What::Inst(InstrumentKind::Fm)),
    ("exemplar", "exemplar", What::Inst(InstrumentKind::Sampler)),
    ("gravis", "gravis", What::Inst(InstrumentKind::Bass)),
    (
        "lamina",
        "lamina",
        What::Inst(InstrumentKind::ElectricPiano),
    ),
    ("organum", "organum", What::Inst(InstrumentKind::Organ)),
    ("chorda", "chorda", What::Inst(InstrumentKind::Chorda)),
    ("pulsus", "pulsus", What::Pulsus),
];

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

/// The app with `demo`'s editor open and fed.
fn app_with(demo: &str, what: What) -> CantusApp {
    let mut app = CantusApp::from_parts(Some(welcome_project()), EngineHandle::dummy(), false);
    app.demo_editor(demo);
    match what {
        What::Fx(kind) => views::fx::demo_feed(&mut app, kind),
        What::Inst(kind) => views::inst::demo_feed(&mut app, kind),
        What::Pulsus => {}
    }
    app.fx.pos = Some(egui::pos2(0.0, 0.0));
    app
}

/// The editor's size at scale `k`, and set the scale where its shell keeps it.
fn scaled(app: &CantusApp, ctx: &egui::Context, what: What, k: f32) -> egui::Vec2 {
    match what {
        What::Fx(kind) => {
            views::fx::frame::shell_of(kind).set_scale(ctx, k);
            views::fx::frame::designed_size(kind) * k
        }
        What::Inst(_) => {
            if let Some(t) = app.inst.open {
                Shell::new(("inst", t.0)).set_scale(ctx, k);
            }
            shell::INSTRUMENT * k
        }
        What::Pulsus => {
            if let Some(t) = app.pulsus.open {
                Shell::new(("pulsus", t.0)).set_scale(ctx, k);
            }
            shell::INSTRUMENT * k
        }
    }
}

fn draw(app: &mut CantusApp, ui: &mut egui::Ui, what: What) {
    match what {
        What::Fx(_) => {
            let ctx = ui.ctx().clone();
            views::eq_editor::window(app, &ctx);
            views::comp_editor::window(app, &ctx);
            views::fx::window(app, &ctx);
        }
        What::Inst(_) => views::inst::window_in(app, ui),
        What::Pulsus => views::pulsus_editor::window_in(app, ui),
    }
}

/// Shoot one unit in one theme at one scale.
#[allow(clippy::too_many_arguments)]
fn shoot_unit(
    dir: &Path,
    name: &str,
    demo: &str,
    what: What,
    dark: bool,
    k: f32,
    faces: bool,
    sample: Option<&'static (&'static str, Skin)>,
) {
    skin::set_enabled(faces);
    skin::set_override(sample.map(|s| &s.1));
    let ctx = context(dark);
    let mut app = app_with(demo, what);
    let size = scaled(&app, &ctx, what, k);
    let theme = if dark { "slate" } else { "paper" };
    let scale = (k * 100.0).round() as u32;
    let file = if let Some((s, _)) = sample {
        format!("{name}-{theme}-{scale}-{s}.png")
    } else if faces {
        format!("{name}-{theme}-{scale}.png")
    } else {
        format!("{name}-{theme}-{scale}-plain.png")
    };
    let path = dir.join(file);
    shoot(&ctx, size, 1.0, 8, Some(&path), |ui| {
        draw(&mut app, ui, what)
    });
    skin::set_enabled(true);
    skin::set_override(None);
    println!("{}", path.display());
}

/// The kit's skin sheets.
fn shoot_gallery(dir: &Path) {
    for (theme, tokens) in [("slate", Tokens::slate()), ("paper", Tokens::paper())] {
        let style = Style::new(tokens);
        for (name, part) in PARTS {
            if !matches!(part, Part::Skin(_)) {
                continue;
            }
            let canvas = gallery::render_sheet(&style, part, 2.0);
            let path = dir.join(format!("gallery-{name}-{theme}.png"));
            let png = gallery::encode_png(&canvas).expect("encode");
            std::fs::write(&path, png).expect("write the shot");
            println!("{}", path.display());
        }
    }
}

fn main() {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    let mut rest = args.iter().filter(|a| !a.starts_with("--"));
    let dir = rest.next().map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/shots/skins")
    });
    let only: Vec<&String> = rest.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    if flag("--gallery") {
        shoot_gallery(&dir);
        if only.is_empty() {
            return;
        }
    }
    let quick = flag("--quick");
    let tried =
        args.iter()
            .position(|a| a == "--try")
            .and_then(|i| args.get(i + 1))
            .map(|n| {
                SAMPLES.iter().find(|(s, _)| s == n).unwrap_or_else(|| {
                    panic!("no sample sheet {n}: studio, vintage, machine or lab")
                })
            });
    let only: Vec<&String> = only
        .into_iter()
        .filter(|o| tried.is_none_or(|(s, _)| o.as_str() != *s))
        .collect();
    for (name, demo, what) in UNITS {
        if !only.is_empty() && !only.iter().any(|o| o.as_str() == *name) {
            continue;
        }
        for dark in [true, false] {
            if quick && !dark {
                continue;
            }
            for k in [1.0, 2.0] {
                if quick && k > 1.0 {
                    continue;
                }
                shoot_unit(&dir, name, demo, *what, dark, k, true, tried);
            }
        }
        if flag("--plain") {
            shoot_unit(&dir, name, demo, *what, true, 1.0, false, None);
        }
    }
}
