//! Wave D's windows without a screen (the fx side round, ADRs 0147–0148 provisional): the
//! X-Y pad (Tactus) and the formula controller (Calculus) — the design sheets.
//!
//! ```text
//! cargo run -p cantus-ui --example shoot_fxd -- [out-dir]
//! ```
//!
//! Each controller in its states, Slate English and Paper Spanish at 100 %; its first state
//! at 75, 150 and 200 % (Spanish at 75 and 200 %); its other colourways in both. The pad's
//! puck, trail and path come from the model's own path and inertia (`xy::Puck`, stepped as
//! the engine steps it); the plot from the formula's compiled program. Every shot runs the
//! kit's text probe, clip-aware, and prints any word cut. `FXD_ONLY=a,b` shoots only the
//! names holding one of those words.

use cantus_core::modulator::formula::Program;
use cantus_core::modulator::xy::Puck;
use cantus_core::modulator::{presets, FormulaSource, ModSource, RouteRange, XyPath, XySource};
use cantus_ui::i18n::{self, Locale};
use cantus_ui::kit::text::probe;
use cantus_ui::raster::shoot;
use cantus_ui::views::fx::{fxd, fxd_formula, fxd_xy};
use std::path::{Path, PathBuf};

#[derive(Copy, Clone)]
struct Shot {
    unit: &'static str,
    state: &'static str,
    dark: bool,
    lang: Locale,
    k: f32,
    look: u8,
}

fn name(sh: &Shot) -> String {
    let look = match (sh.unit, sh.look) {
        (_, 0) => String::new(),
        ("xy", 1) => "-copper".into(),
        ("xy", _) => "-frost".into(),
        (_, 1) => "-ivory".into(),
        _ => "-ebony".into(),
    };
    format!(
        "{}-{}{look}-{}-{}-k{}.png",
        sh.unit,
        sh.state,
        if sh.dark { "slate" } else { "paper" },
        sh.lang.code(),
        (sh.k * 100.0).round() as u32
    )
}

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

/// The pad of a state, and its links.
fn pad(state: &str, look: u8) -> (XySource, Vec<fxd_xy::Link>, fxd_xy::Live) {
    let t = |k: &str| i18n::t(k);
    let path = XyPath {
        points: vec![
            [0.18, 0.22],
            [0.42, 0.78],
            [0.80, 0.86],
            [0.88, 0.40],
            [0.60, 0.16],
        ],
        beats: 8.0,
        smooth: true,
    };
    let mut src = XySource {
        x: 0.64,
        y: 0.58,
        inertia_ms: 320.0,
        path: Some(path),
        play_path: true,
        look,
        ..XySource::default()
    };
    let mut links = vec![
        fxd_xy::Link {
            name: format!("{} · Cribrum", t("kit.param.cutoff")),
            axis: 0,
            range: Some(RouteRange {
                from: -0.6,
                to: 0.8,
                curve: 0.3,
            }),
            depth: 1.0,
        },
        fxd_xy::Link {
            name: format!("{} · Cribrum", t("kit.param.resonance")),
            axis: 1,
            range: Some(RouteRange {
                from: 0.0,
                to: 0.7,
                curve: 0.0,
            }),
            depth: 1.0,
        },
        fxd_xy::Link {
            name: format!("{} · Keys bus", t("auto.target.width")),
            axis: 1,
            range: None,
            depth: -0.5,
        },
        fxd_xy::Link {
            name: format!("{} · Lead", t("auto.target.pan")),
            axis: 0,
            range: None,
            depth: 0.4,
        },
    ];
    let mut beats = 5.3f64;
    match state {
        "rest" => {
            src = XySource {
                look,
                ..XySource::default()
            };
            links.clear();
        }
        "hand" => {
            src.play_path = false;
            src.inertia_ms = 900.0;
        }
        "oneway" => {
            src.unipolar = true;
            src.inertia_ms = 0.0;
            beats = 2.1;
        }
        "draw" | "record" => {
            src.play_path = false;
            src.path = None;
        }
        "links" => links.extend(many_links()),
        "preset" => {
            if let ModSource::XyPad(x) = (preset_of(false).source)() {
                src = XySource { look, ..x };
            }
        }
        _ => {}
    }
    // The puck as the engine steps it: the path (or the hand) and the inertia, at the
    // control grid's pace (16 samples at 48 kHz, 120 BPM), the last 1.5 s kept as the trail.
    let mut puck = Puck::default();
    let dt = 16.0 / 48_000.0;
    let steps = (beats * 0.5 / dt) as usize;
    let mut trail = Vec::new();
    let hand_from = [0.20f32, 0.25];
    let mut heads = [src.x, src.y];
    for i in 0..=steps {
        let b = i as f64 * dt * 2.0;
        heads = match state {
            "hand" if b < beats - 0.9 => hand_from,
            "hand" => [src.x, src.y],
            "rest" => [0.5, 0.5],
            _ => src.target(b, true, [src.x, src.y]),
        };
        puck.step(heads, src.inertia_ms, dt);
        if i % 150 == 0 || i == steps {
            trail.push(puck.pos);
        }
    }
    let keep = trail.len().saturating_sub(24);
    let trail = trail.split_off(keep);
    let path_at = src
        .path
        .as_ref()
        .filter(|_| src.play_path)
        .map(|p| ((beats / f64::from(p.beats)).fract()) as f32);
    let mut live = fxd_xy::Live {
        puck: puck.pos,
        heads,
        trail: if state == "rest" { Vec::new() } else { trail },
        path_at,
        ..fxd_xy::Live::default()
    };
    match state {
        // Part of a loop drawn: a figure of eight under the hand.
        "draw" | "record" => {
            live.mode = if state == "draw" {
                fxd_xy::PadMode::Draw
            } else {
                fxd_xy::PadMode::Record
            };
            live.stroke = (0..=60)
                .map(|i| {
                    let t = i as f32 / 60.0 * std::f32::consts::PI * 1.4;
                    [0.5 + 0.32 * t.sin(), 0.5 + 0.26 * (2.0 * t).sin()]
                })
                .collect();
            let end = *live.stroke.last().unwrap();
            live.puck = end;
            live.heads = end;
            live.trail.clear();
        }
        "links" => live.links_first = 3,
        _ => {}
    }
    (src, links, live)
}

/// More parameters than the list shows: it scrolls.
fn many_links() -> Vec<fxd_xy::Link> {
    let t = |k: &str| i18n::t(k);
    let mut v = Vec::new();
    for (i, (key, on)) in [
        ("auto.target.volume", "Drums"),
        ("auto.target.pan", "Pad"),
        ("kit.param.cutoff", "Bass"),
        ("auto.target.width", "Master"),
        ("kit.param.resonance", "Lead"),
        ("auto.target.level", "Keys"),
    ]
    .into_iter()
    .enumerate()
    {
        v.push(fxd_xy::Link {
            name: format!("{} · {on}", t(key)),
            axis: (i % 2) as u8,
            range: None,
            depth: 0.3 + 0.1 * i as f32,
        });
    }
    v
}

/// The formula of a state.
fn formula(state: &str, look: u8) -> (FormulaSource, Vec<fxd_xy::Link>, fxd_formula::Live) {
    let t = |k: &str| i18n::t(k);
    let mut src = FormulaSource {
        text: "a*Sin(SongTime*Pi*(Int(b*10)))+c".into(),
        inputs: [0.42, 0.25, 0.5],
        bipolar: false,
        links: Vec::new(),
        look,
    };
    let links = vec![
        fxd_xy::Link {
            name: format!("{} · Cribrum", t("kit.param.cutoff")),
            axis: 0,
            range: Some(RouteRange {
                from: 0.1,
                to: 0.9,
                curve: -0.4,
            }),
            depth: 1.0,
        },
        fxd_xy::Link {
            name: format!("{} · Keys bus", t("auto.target.pan")),
            axis: 0,
            range: None,
            depth: -0.6,
        },
    ];
    let mut live = fxd_formula::Live {
        beats: 9.4,
        tempo: 120.0,
        ..fxd_formula::Live::default()
    };
    match state {
        "square" => src.text = "a*Round(Frac(SongTime*2*b)+0.5-c)".into(),
        "bipolar" => {
            src.text = "Fold(SongTime*b)*a + 0.3*tri(beat/4)".into();
            src.bipolar = true;
            src.inputs = [0.7, 0.6, 0.0];
        }
        "error" => {
            let d = "a*Sin(SongTime*Pi*b".to_string();
            live.caret = Some(d.len());
            live.draft = Some(d);
            live.focused = true;
        }
        "preset" => {
            if let ModSource::Formula(f) = (preset_of(true).source)() {
                src = FormulaSource { look, ..f };
            }
        }
        "typing" => {
            src.text = "a*Sin(SongTime*Pi*b)+c".into();
            live.caret = Some("a*Sin(SongTime*Pi*b".len());
            live.focused = true;
        }
        _ => {}
    }
    let mut links = links;
    if state == "links" {
        links.extend(many_links());
        live.links_first = 2;
    }
    debug_assert!(Program::compile(&src.text).is_ok());
    (src, links, live)
}

/// The preset the preset shots show: the figure of eight, the square.
fn preset_of(formula: bool) -> &'static presets::ControllerPreset {
    presets::by_id(if formula {
        "formula.square"
    } else {
        "xy.figure_eight"
    })
    .expect("the preset")
}

/// The preset well's name in the preset shots.
fn preset_name(formula: bool) -> String {
    i18n::t(&format!("fxd.preset.{}", preset_of(formula).id))
}

fn take(dir: &Path, sh: &Shot) {
    i18n::set_locale(sh.lang);
    let ctx = context(sh.dark);
    let f = sh.unit == "formula";
    fxd::shell_of(f, sh.look).set_scale(&ctx, sh.k);
    // Measured on a context of its own: a shot hands its fonts' atlas to the first raster
    // only, and the window's shot must have it.
    let mut size = egui::Vec2::ZERO;
    shoot(
        &context(sh.dark),
        egui::vec2(64.0, 64.0),
        1.0,
        1,
        None,
        |ui| {
            size = fxd::designed_size(ui, f);
        },
    );
    let path = dir.join(name(sh));
    let context = i18n::t("fxd.sheet.context");
    probe::start_clipped();
    let preset = (sh.state == "preset").then(|| preset_name(f));
    shoot(&ctx, size * sh.k, 1.0, 6, Some(&path), |ui| {
        if f {
            let (src, links, live) = formula(sh.state, sh.look);
            let chips = [i18n::t("fxd.sheet.chip.formula")];
            let chips: Vec<&str> = chips.iter().map(String::as_str).collect();
            let mut c = fxd::Chrome::sheet(&context, true, &chips);
            c.preset = preset.as_deref();
            fxd::formula(ui, &c, &src, 1.0, &links, &live);
        } else {
            let (src, links, live) = pad(sh.state, sh.look);
            let chips = [i18n::t("fxd.sheet.chip.xy")];
            let chips: Vec<&str> = chips.iter().map(String::as_str).collect();
            let mut c = fxd::Chrome::sheet(&context, true, &chips);
            c.preset = preset.as_deref();
            fxd::xy(ui, &c, &src, 0.85, &links, &live);
        }
    });
    let cuts = probe::take();
    println!("{}", path.display());
    for c in cuts {
        println!("  CUT {:?} -> {:?} ({})", c.full, c.shown, c.how);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dir = args
        .first()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shots/fxd"));
    std::fs::create_dir_all(&dir).expect("output dir");
    let only = std::env::var("FXD_ONLY").ok();
    let sh = |unit, state, dark, lang, k, look| Shot {
        unit,
        state,
        dark,
        lang,
        k,
        look,
    };
    let states: [(&str, &[&str]); 2] = [
        (
            "xy",
            &[
                "start", "hand", "oneway", "rest", "draw", "record", "preset", "links",
            ],
        ),
        (
            "formula",
            &[
                "start", "square", "bipolar", "error", "typing", "preset", "links",
            ],
        ),
    ];
    let mut shots = Vec::new();
    for (unit, list) in states {
        for state in list {
            shots.push(sh(unit, state, true, Locale::En, 1.0, 0));
            shots.push(sh(unit, state, false, Locale::Es, 1.0, 0));
        }
        for k in [0.75, 1.5, 2.0] {
            shots.push(sh(unit, "start", true, Locale::En, k, 0));
        }
        for k in [0.75, 2.0] {
            shots.push(sh(unit, "start", false, Locale::Es, k, 0));
        }
        for look in [1u8, 2] {
            shots.push(sh(unit, "start", true, Locale::En, 1.0, look));
            shots.push(sh(unit, "start", false, Locale::Es, 1.0, look));
        }
    }
    for s in &shots {
        let n = name(s);
        if only
            .as_ref()
            .is_none_or(|o| o.split(',').any(|p| n.contains(p)))
        {
            take(&dir, s);
        }
    }
}
