//! Round 39's routing cables in their three looks (des-mixer's `options-cables-*`), the
//! **whole window** headless: the demo song (Neon Tide) playing, the mixer maximised with
//! the Kick selected and the Cables key on, in Slate Night and Slate Paper at 1920 × 1080
//! (125 %).
//!
//! `cargo run --release -p cantus-ui --example shoot_cables39 -- <out-dir> [filter…]`
//!
//! Writes `cables39-<style>-<case>-<theme>.png`. Styles: `patch`, `flow`, `subway`, `jack`. Cases:
//! `sel` (the Kick's cables, in and out), `all` (Show all), `hover` (the pointer on the
//! Kick → Drum bus cable: its route to the master lit, the rest dimmed), `drag` (a cable
//! from the Chords' output jack held over the Drum bus: "Route Chords to Drum bus"),
//! `refused` (from the Drum bus held over the Kick: a loop), `stopped` (Show all, the
//! transport stopped: nothing moves).

use cantus_core::StripId;
use cantus_engine::EngineHandle;
use cantus_ui::app::CentralView;
use cantus_ui::kit::appearance::Mode;
use cantus_ui::settings::CableStyle;
use cantus_ui::views::mixer::{self, layout::CableView, layout::WidthMode, Control};
use cantus_ui::views::workspace::Pane;
use cantus_ui::{raster, CantusApp};
use egui::{vec2, Event, Pos2};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Copy, Clone, PartialEq)]
enum Case {
    Sel,
    All,
    Hover,
    Drag,
    Refused,
    Stopped,
}

impl Case {
    fn name(self) -> &'static str {
        match self {
            Case::Sel => "sel",
            Case::All => "all",
            Case::Hover => "hover",
            Case::Drag => "drag",
            Case::Refused => "refused",
            Case::Stopped => "stopped",
        }
    }
}

fn strip(app: &CantusApp, name: &str) -> StripId {
    let p = app.project();
    p.strips
        .iter()
        .find(|s| s.name == name)
        .map(|s| s.id)
        .or_else(|| {
            p.tracks
                .iter()
                .find(|t| t.name == name)
                .and_then(|t| t.strip)
        })
        .unwrap_or_else(|| panic!("no strip {name}"))
}

const FRAMES: usize = 30;

fn shoot(dir: &Path, style: CableStyle, case: Case, preset: &str, dark: bool) {
    cantus_ui::i18n::set_locale(cantus_ui::i18n::Locale::En);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let mut app = CantusApp::from_parts(Some(cantus_ui::demos::startup_project()), engine, false);
    let tmp = std::env::temp_dir().join("cantus-shoot-cables39");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    let mode = app.appearance.mode;
    app.appearance.assign(mode, format!("preset:{preset}"));
    app.settings.cable_style = Some(style);
    app.attach(&ctx);
    app.show_central(CentralView::Mixer);
    mixer::set_width_mode(&mut app, WidthMode::Wide);
    let kick = strip(&app, "Kick");
    let chords = strip(&app, "Chords");
    let bus = app
        .project()
        .strip(kick)
        .and_then(|s| s.output)
        .expect("the kick goes to the drum bus");
    app.select_strip(match case {
        Case::Drag => chords,
        Case::Refused => bus,
        _ => kick,
    });
    let mut prefs = app.mixer.prefs();
    prefs.cables = if matches!(case, Case::All | Case::Hover | Case::Stopped) {
        CableView::All
    } else {
        CableView::Selected
    };
    app.mixer.set_prefs(prefs);
    app.sync_engine();
    let play = case != Case::Stopped;
    if play {
        app.toggle_play();
    }
    let mut audio = vec![0f32; 800 * 2];
    let zoom = 1.25;
    let points = vec2(1920.0 / zoom, 1080.0 / zoom);
    let file = format!(
        "cables39-{}-{}-{preset}.png",
        style_name(style),
        case.name()
    );
    let path = dir.join(&file);
    // Where the pointer goes: the hovered cable's middle (found in the frame before), or
    // a drag from an output jack to the target's input jack.
    let hover: Rc<Cell<Option<Pos2>>> = Rc::new(Cell::new(None));
    let h2 = hover.clone();
    let (from, to) = match case {
        Case::Drag => (chords, bus),
        _ => (bus, kick),
    };
    let mut frames = 0;
    raster::shoot_events(
        &ctx,
        points,
        zoom,
        FRAMES,
        Some(&path),
        move |frame, ctx| match case {
            Case::Hover => h2.get().map(Event::PointerMoved).into_iter().collect(),
            Case::Drag | Case::Refused => {
                let jack =
                    |id| mixer::control_rect(ctx, Some(id), Control::Port).map(|r| r.center());
                let (Some(a), Some(z)) = (jack(from), jack(to)) else {
                    return Vec::new();
                };
                // The target's input jack: 22 pt left of its output (Wide).
                let z = z - vec2(22.0, 0.0) + vec2(3.0, -3.0);
                let button = |pressed| Event::PointerButton {
                    pos: a,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::CTRL,
                };
                match frame {
                    0..=5 => Vec::new(),
                    6 => vec![Event::PointerMoved(a)],
                    7 => vec![Event::ModifiersChanged(egui::Modifiers::CTRL), button(true)],
                    _ => {
                        let u = ((frame - 7) as f32 / 10.0).min(1.0);
                        vec![
                            Event::ModifiersChanged(egui::Modifiers::CTRL),
                            Event::PointerMoved(a + (z - a) * u),
                        ]
                    }
                }
            }
            _ => Vec::new(),
        },
        |root| {
            frames += 1;
            if frames == 2 {
                app.workspace.toggle_fold(Pane::Arrangement);
            }
            if play {
                for _ in 0..3 {
                    processor.process(&mut audio, 2);
                }
            }
            app.frame(root);
            // The band settles its height in the first frames (Subway's lanes, D's
            // plugs): the pointer follows the cable until then.
            if case == Case::Hover && frames < 20 {
                let d = app
                    .mixer
                    .cables()
                    .iter()
                    .find(|d| d.route.from == kick && d.route.to == bus)
                    .copied();
                if let Some(d) = d {
                    let p = app.mixer.cable_point(&d);
                    hover.set(Some(p));
                }
            }
        },
    );
    println!("{} ({} gleams)", path.display(), app.mixer.cable_gleams());
}

fn style_name(s: CableStyle) -> &'static str {
    match s {
        CableStyle::Patch => "patch",
        CableStyle::Flow => "flow",
        CableStyle::Subway => "subway",
        CableStyle::Jack => "jack",
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shoot-cables39"));
    let only: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("create the output directory");
    let cases = [
        Case::Sel,
        Case::All,
        Case::Hover,
        Case::Drag,
        Case::Refused,
        Case::Stopped,
    ];
    for style in CableStyle::ALL {
        for case in cases {
            for (preset, dark) in [("slate_night", true), ("slate_paper", false)] {
                let tag = format!("{}-{}-{preset}", style_name(style), case.name());
                if !only.is_empty() && !only.iter().all(|o| tag.contains(o.as_str())) {
                    continue;
                }
                shoot(&dir, style, case, preset, dark);
            }
        }
    }
}
