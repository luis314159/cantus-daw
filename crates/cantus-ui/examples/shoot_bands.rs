//! r40:tymp-ui — bands on the desk (the owner's groups sheets, `design-p3/groups/`): the
//! user group "Voces" (Vox, Choir, Pad) as a band; folded into one strip; its right edge
//! dragged over Guitar; docked right beside the returns; and beside an instrument's family
//! (Drums · Tympanum · 3 mics, with Kick and Snare grouped inside it). Slate Night and
//! Paper, 1280 × 720 points at 100 % and 150 %, English and Spanish.
//!
//! `cargo run -p cantus-ui --example shoot_bands -- <out_dir> [--es] [--zoom z] [case…]`
//! (cases: band, folded, drag, docked, both).

use cantus_core::inst_out::{OutputFamily, OutputId, OutputSlot};
use cantus_core::*;
use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use std::path::PathBuf;

#[derive(Copy, Clone, PartialEq)]
enum Case {
    Band,
    Folded,
    Drag,
    Docked,
    Both,
}

/// The design's Neon Tide (`design-p3/groups/`): eleven tracks, Reverb and Delay docked
/// right — and for *both*, Drums (a Tympanum with Kick In, Kick Out and Snare Top on strips
/// of their own), Bass, Chords, Vox, Choir, Pad and Guitar. No strip beyond the returns.
const NAMES: [&str; 11] = [
    "Kick", "Clap", "Hats", "Bass", "Chords", "Pluck", "Lead", "Vox", "Choir", "Pad", "Guitar",
];
const BOTH: [&str; 7] = ["Drums", "Bass", "Chords", "Vox", "Choir", "Pad", "Guitar"];

fn song(case: Case) -> (Project, StripId) {
    let mut p = Project::new("Neon Tide");
    let both = case == Case::Both;
    // Strip index of each track: in *both* the Drums' mics take strips 1–3.
    let names: Vec<&str> = if both { BOTH.to_vec() } else { NAMES.to_vec() };
    for (i, name) in names.iter().enumerate() {
        let at = if both && i > 0 { i + 3 } else { i };
        let mut t = p.make_track(*name);
        t.strip = Some(p.strips[at].id);
        t.color = Color::PALETTE[i % Color::PALETTE.len()];
        p.tracks.push(t);
    }
    let used = if both { names.len() + 3 } else { names.len() };
    p.strips.truncate(used + 2);
    for (k, name) in ["Reverb", "Delay"].into_iter().enumerate() {
        p.strips[used + k].name = name.into();
        p.strips[used + k].dock = Dock::Right;
    }
    if both {
        let mut n = NativeSpec::new(InstrumentKind::AcousticDrums);
        let kit_strip = p.strips[0].id;
        for (i, (id, name, fam)) in [
            (0u16, "Kick In", OutputFamily::Kick),
            (1, "Kick Out", OutputFamily::Kick),
            (3, "Snare Top", OutputFamily::Snare),
        ]
        .into_iter()
        .enumerate()
        {
            let s = &mut p.strips[i + 1];
            s.name = name.into();
            s.kind = cantus_core::vca::StripKind::Pad;
            s.output = Some(kit_strip);
            n.outputs.push(OutputSlot {
                id: OutputId(id),
                name: name.into(),
                family: fam,
                strip: p.strips[i + 1].id,
            });
        }
        p.tracks[0].instrument = InstrumentSpec::Native(n);
    }
    // Voces: Vox, Choir, Pad.
    let mut g = p.make_strip_group("Voces", Color::PALETTE[7]);
    g.collapsed = case == Case::Folded;
    let gid = g.id;
    p.strip_groups.push(g);
    let voices: Vec<StripId> = ["Vox", "Choir", "Pad"]
        .iter()
        .filter_map(|n| p.tracks.iter().find(|t| t.name == *n).and_then(|t| t.strip))
        .collect();
    for id in &voices {
        if let Some(s) = p.strip_mut(*id) {
            s.group = Some(gid);
            if case == Case::Docked {
                s.dock = Dock::Right;
            }
        }
    }
    if case == Case::Docked {
        // Beside the returns: the group's strips just before them.
        let moved: Vec<MixerStrip> = p
            .strips
            .iter()
            .filter(|s| voices.contains(&s.id))
            .cloned()
            .collect();
        p.strips.retain(|s| !voices.contains(&s.id));
        let at = p
            .strips
            .iter()
            .position(|s| s.name == "Reverb")
            .unwrap_or(p.strips.len());
        for (k, s) in moved.into_iter().enumerate() {
            p.strips.insert(at + k, s);
        }
    }
    if both {
        // Kick In and Kick Out grouped inside the family.
        let mut k = p.make_strip_group("Kick", Color::PALETTE[0]);
        k.collapsed = false;
        let kid = k.id;
        p.strip_groups.push(k);
        for i in [1usize, 2] {
            p.strips[i].group = Some(kid);
        }
    }
    // The design's inspector shows Vox's strip.
    let vox = voices[0];
    (p, vox)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/shoot-bands"));
    let rest: Vec<String> = args.collect();
    let es = rest.iter().any(|a| a == "--es");
    let zooms: Vec<f32> = match rest
        .iter()
        .position(|a| a == "--zoom")
        .and_then(|i| rest.get(i + 1))
        .and_then(|z| z.parse().ok())
    {
        Some(z) => vec![z],
        None => vec![1.0, 1.5],
    };
    let only: Vec<&String> = rest
        .iter()
        .enumerate()
        .filter(|(i, a)| !a.starts_with("--") && (*i == 0 || rest[i - 1] != "--zoom"))
        .map(|(_, a)| a)
        .collect();
    std::fs::create_dir_all(&dir).expect("the output folder");
    cantus_ui::i18n::set_locale(if es {
        cantus_ui::i18n::Locale::Es
    } else {
        cantus_ui::i18n::Locale::En
    });
    let tmp = std::env::temp_dir().join("cantus-shoot-bands");
    for (case, label) in [
        (Case::Band, "band"),
        (Case::Folded, "folded"),
        (Case::Drag, "drag"),
        (Case::Docked, "docked"),
        (Case::Both, "both"),
    ] {
        if !only.is_empty() && !only.iter().any(|o| o.as_str() == label) {
            continue;
        }
        for (preset, dark) in [("slate_night", true), ("slate_paper", false)] {
            for &zoom in &zooms {
                let ctx = egui::Context::default();
                cantus_ui::design::install_fonts(&ctx);
                cantus_ui::design::install_style(&ctx);
                let (project, first) = song(case);
                let mut app = CantusApp::from_parts(Some(project), EngineHandle::dummy(), false);
                app.settings_path = tmp.join("settings.json");
                app.layouts_path = tmp.join("layouts.json");
                app.keymap_path = tmp.join("keymap.toml");
                app.appearance_path = tmp.join("appearance.json");
                app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
                let mode = app.appearance.mode;
                app.appearance.assign(mode, format!("preset:{preset}"));
                app.attach(&ctx);
                app.show_central(CentralView::Mixer);
                // The design's window: the mixer and the inspector, no arrangement.
                app.workspace
                    .hide(cantus_ui::views::workspace::Pane::Arrangement);
                app.workspace.impose_sizes(246.0, 0.0);
                cantus_ui::views::mixer::set_width_mode(
                    &mut app,
                    cantus_ui::views::mixer::layout::WidthMode::Wide,
                );
                app.select_strip(first);
                let theme = preset.trim_start_matches("slate_");
                let mut name = format!("grp-{label}-{theme}-{:.0}", zoom * 100.0);
                if es {
                    name.push_str("-es");
                }
                let path = dir.join(format!("{name}.png"));
                let size = egui::vec2(1280.0, 720.0);
                if case == Case::Drag {
                    // Hold the band's right edge and carry it over Guitar.
                    let pad = app
                        .project()
                        .tracks
                        .iter()
                        .find(|t| t.name == "Pad")
                        .and_then(|t| t.strip);
                    let guitar = app
                        .project()
                        .tracks
                        .iter()
                        .find(|t| t.name == "Guitar")
                        .and_then(|t| t.strip);
                    let ctx2 = ctx.clone();
                    let rect = move |id| {
                        cantus_ui::views::mixer::control_rect(
                            &ctx2,
                            id,
                            cantus_ui::views::mixer::Control::Head,
                        )
                    };
                    raster::shoot_events(
                        &ctx,
                        size,
                        zoom,
                        16,
                        Some(&path),
                        |frame, _| {
                            let (Some(a), Some(b)) = (rect(pad), rect(guitar)) else {
                                return Vec::new();
                            };
                            let from = egui::pos2(a.right() + 1.0, a.top() - 9.0);
                            let to = egui::pos2(b.right() - 6.0, a.top() - 9.0);
                            let press = |pressed| egui::Event::PointerButton {
                                pos: from,
                                button: egui::PointerButton::Primary,
                                pressed,
                                modifiers: egui::Modifiers::NONE,
                            };
                            match frame {
                                6 => vec![egui::Event::PointerMoved(from)],
                                7 => vec![press(true)],
                                8..=14 => {
                                    let k = (frame - 7) as f32 / 7.0;
                                    vec![egui::Event::PointerMoved(from + (to - from) * k)]
                                }
                                _ => Vec::new(),
                            }
                        },
                        |root| app.frame(root),
                    );
                } else {
                    raster::shoot(&ctx, size, zoom, 8, Some(&path), |root| app.frame(root));
                }
                println!("{}", path.display());
            }
        }
    }
}
