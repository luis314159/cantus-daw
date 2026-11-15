//! Round 40, `look`: the owner's answers on the look as he judges them — the whole window
//! on the song the program opens on (the Chords track chosen, playing in a loop), at
//! 1920 × 1080, before (round 39's choices: `--old`) and after (the new defaults).
//!
//! `cargo run --release -p cantus-ui --example shoot_look40 -- <dir> <case> <theme> <zoom>
//! [--old] [--es] [--extras] [--grey]` writes `<case>-<theme>-<zoom>[-old][-es].png`.
//! `<dir> --all` shoots the round's standard set (each change at 100 and 150 %, Night and
//! Paper, before and after).
//!
//! Cases: `arrangement`, `mixer` (Wide, the arrangement folded, two strips muted),
//! `maximise` (the arrangement maximised), `inspector` (the mixer with a strip chosen, the
//! inspector's values), `topbar` (the arrangement with the bar's extras), `settings`
//! (Settings ▸ Appearance at *The window*), `menu` (the bar's Edit menu open), `dialog` (the
//! unsaved-changes question). Themes: a preset id, `r39_bordeaux` / `r39_pewter` (the
//! pair as round 39 left them), or `slate_paper@<k>` — Slate Paper with its room's greys
//! keeping `k` of their chroma (R2-B11, the owner's "less blue": `@1` today, `@0` neutral).
//! `<dir> --paper` shoots the arrangement and the mixer at 1, 0.5 and 0.

use cantus_engine::EngineHandle;
use cantus_ui::app::{CantusApp, CentralView};
use cantus_ui::kit::appearance::Mode;
use cantus_ui::raster;
use cantus_ui::views::mixer::layout::WidthMode;
use cantus_ui::views::owner_look::{DockPlace, LookSettings, MaximiseKeeps, MuteOn, ValueStyle};
use cantus_ui::views::workspace::Pane;
use cantus_ui::views::workspace_settings::{open, SettingsPage};
use egui::vec2;
use std::path::{Path, PathBuf};

#[path = "shots_common/themes_r39.rs"]
mod themes_r39;

const SIZE: (f32, f32) = (1920.0, 1080.0);

struct Shot<'a> {
    case: &'a str,
    theme: &'a str,
    zoom: f32,
    old: bool,
    es: bool,
    extras: bool,
    grey: bool,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let rest: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    let flag = |f: &str| rest.iter().any(|a| a == f);
    if flag("--all") {
        all(&dir);
        return;
    }
    if flag("--paper") {
        for case in ["arrangement", "mixer"] {
            for theme in ["slate_paper@1", "slate_paper@0.5", "slate_paper@0"] {
                let shot = Shot {
                    case,
                    theme,
                    zoom: 1.0,
                    old: false,
                    es: false,
                    extras: false,
                    grey: false,
                };
                println!("{}", window(&dir, &shot).display());
            }
        }
        return;
    }
    let pos: Vec<&String> = rest.iter().filter(|a| !a.starts_with("--")).collect();
    let shot = Shot {
        case: pos.first().map_or("arrangement", |s| s.as_str()),
        theme: pos.get(1).map_or("slate_night", |s| s.as_str()),
        zoom: pos.get(2).and_then(|z| z.parse().ok()).unwrap_or(1.0),
        old: flag("--old"),
        es: flag("--es"),
        extras: flag("--extras"),
        grey: flag("--grey"),
    };
    println!("{}", window(&dir, &shot).display());
}

/// The round's standard set.
fn all(dir: &Path) {
    let mut n = 0;
    let mut go = |s: Shot<'_>| {
        println!("{}", window(dir, &s).display());
        n += 1;
    };
    for theme in ["slate_night", "slate_paper"] {
        for zoom in [1.0, 1.5] {
            for case in ["mixer", "maximise", "inspector", "topbar", "menu", "dialog"] {
                for old in [true, false] {
                    let extras = case == "topbar" && !old;
                    go(Shot {
                        case,
                        theme,
                        zoom,
                        old,
                        es: false,
                        extras,
                        grey: false,
                    });
                }
            }
        }
        // Settings ▸ Appearance's new section, and the bar with both extras in Spanish.
        go(Shot {
            case: "settings",
            theme,
            zoom: 1.0,
            old: false,
            es: false,
            extras: false,
            grey: false,
        });
        go(Shot {
            case: "topbar",
            theme,
            zoom: 1.0,
            old: false,
            es: true,
            extras: true,
            grey: false,
        });
    }
    // Bordeaux and Pewter, before and after, at 100 %.
    for (before, after) in [("r39_bordeaux", "bordeaux"), ("r39_pewter", "slate")] {
        for case in ["arrangement", "mixer"] {
            for theme in [before, after] {
                go(Shot {
                    case,
                    theme,
                    zoom: 1.0,
                    old: false,
                    es: false,
                    extras: false,
                    grey: false,
                });
            }
        }
    }
    println!("{n} shots");
}

/// The look round 39 left (`--old`), or the new defaults with `extras` / `grey`.
fn look_of(s: &Shot<'_>) -> LookSettings {
    if s.old {
        return LookSettings {
            maximise: MaximiseKeeps::Nothing,
            topbar_automation: false,
            topbar_typing: false,
            mixer_dock: DockPlace::AfterStrips,
            inspector_values: ValueStyle::Slim,
            mute_on: MuteOn::Grey,
        };
    }
    LookSettings {
        topbar_automation: s.extras,
        topbar_typing: s.extras,
        mute_on: if s.grey { MuteOn::Grey } else { MuteOn::Colour },
        ..LookSettings::default()
    }
}

/// The theme a shot wears, as both of its lights' slot: a preset, or round 39's pair.
fn wear(app: &mut CantusApp, theme: &str, dark: bool) {
    let mode = if dark { Mode::Dark } else { Mode::Light };
    let old = match theme {
        "r39_bordeaux" => Some((
            "Bordeaux (round 39)",
            themes_r39::bordeaux_tokens(),
            0xeba487,
        )),
        "r39_pewter" => Some(("Pewter (round 39)", themes_r39::pewter_tokens(), 0xffaa54)),
        _ => None,
    };
    if let Some(k) = theme.strip_prefix("slate_paper@") {
        let k: f32 = k.parse().expect("slate_paper@<chroma kept>");
        let t = paper_kept(k);
        let name = format!("Slate Paper @{k}");
        let mut th = cantus_ui::theme::Theme::from_palette(
            &name,
            &cantus_ui::theme_family::palette_of(&t, t.brand, t.brand_bright),
        )
        .with_kit(&t);
        th.system = t.system;
        app.appearance.assign(mode, format!("user:{name}"));
        app.appearance.themes.push(th);
        return;
    }
    if let Some((name, t, accent)) = old {
        let accent =
            egui::Color32::from_rgb((accent >> 16) as u8, (accent >> 8) as u8, accent as u8);
        let pal = cantus_ui::theme_family::palette_of(&t, accent, accent);
        let mut th = cantus_ui::theme::Theme::from_palette(name, &pal).with_kit(&t);
        th.system = t.system;
        app.appearance.assign(mode, format!("user:{name}"));
        app.appearance.themes.push(th);
        return;
    }
    app.appearance.assign(mode, format!("preset:{theme}"));
}

fn window(dir: &Path, s: &Shot<'_>) -> PathBuf {
    let locale = if s.es {
        cantus_ui::i18n::Locale::Es
    } else {
        cantus_ui::i18n::Locale::En
    };
    cantus_ui::i18n::set_locale(locale);
    // Round 39's Paper sheets for the "before" shots.
    cantus_ui::kit::dialog::set_paper_r39(s.old);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let project = cantus_ui::demos::startup_project();
    let mut app = CantusApp::from_parts(Some(project), engine, false);
    let tmp = std::env::temp_dir().join("cantus-shoot-look40");
    app.settings_path = tmp.join("settings.json");
    app.layouts_path = tmp.join("layouts.json");
    app.keymap_path = tmp.join("keymap.toml");
    app.appearance_path = tmp.join("appearance.json");
    let dark = !(s.theme.contains("paper") || s.theme == "print" || s.theme == "parchment");
    app.appearance.mode = if dark { Mode::Dark } else { Mode::Light };
    wear(&mut app, s.theme, dark);
    app.attach(&ctx);
    cantus_ui::views::owner_look::set(&mut app, look_of(s));
    // The Chords track chosen, its first clip selected.
    let chords = app.project().tracks.iter().find_map(|t| {
        if !t.name.to_lowercase().contains("chord") {
            return None;
        }
        t.clips
            .iter()
            .find(|c| !c.notes.is_empty())
            .map(|c| (t.id, c.id))
    });
    if let Some((t, c)) = chords {
        app.select_clip(t, c);
    }
    let bar = cantus_core::PPQ * 4;
    app.loop_span(cantus_core::Ticks(bar * 4), cantus_core::Ticks(bar * 8));
    app.engine
        .seek(cantus_core::Ticks(bar * 5 + cantus_core::PPQ * 2));
    let mut fold = false;
    let mut maximise = false;
    let mut menu = false;
    match s.case {
        "mixer" | "inspector" => {
            app.show_central(CentralView::Mixer);
            cantus_ui::views::mixer::set_width_mode(&mut app, WidthMode::Wide);
            fold = true;
            // Two strips muted (Mute's colour), and in `inspector` the Chords strip
            // chosen with its Level section open.
            let ids: Vec<_> = app
                .project()
                .strips
                .iter()
                .skip(2)
                .take(2)
                .map(|x| x.id)
                .collect();
            for id in ids {
                app.exec(cantus_core::Command::SetStripMute { id, muted: true });
            }
        }
        "maximise" => maximise = true,
        "settings" => open(&mut app, SettingsPage::Appearance),
        "menu" => menu = true,
        "dialog" => app.unsaved_prompt = Some(cantus_ui::views::unsaved::Then::New),
        _ => {}
    }
    app.sync_engine();
    app.toggle_play();
    let mut audio = vec![0f32; 800 * 2];
    let points = vec2(SIZE.0 / s.zoom, SIZE.1 / s.zoom);
    let mut frames = 0;
    let mut name = format!("{}-{}-{}", s.case, s.theme, (s.zoom * 100.0).round() as u32);
    if s.old {
        name.push_str("-old");
    }
    if s.es {
        name.push_str("-es");
    }
    if s.grey {
        name.push_str("-grey");
    }
    let path = dir.join(format!("{name}.png"));
    let settings_scroll = s.case == "settings";
    raster::shoot_events(
        &ctx,
        points,
        s.zoom,
        30,
        Some(&path),
        |f, _| {
            let mut ev = Vec::new();
            if menu && (12..=14).contains(&f) {
                // A click on the bar's *Edit* (its words at the left, the same place in
                // points at every size): the menu with its checks and shortcuts.
                let at = egui::pos2(85.0, 14.0);
                ev.push(egui::Event::PointerMoved(at));
                if f > 12 {
                    ev.push(egui::Event::PointerButton {
                        pos: at,
                        button: egui::PointerButton::Primary,
                        pressed: f == 13,
                        modifiers: egui::Modifiers::NONE,
                    });
                }
            }
            if menu && f > 14 {
                // The pointer on a row, for its hover.
                ev.push(egui::Event::PointerMoved(egui::pos2(110.0, 92.0)));
            }
            if settings_scroll && (8..14).contains(&f) {
                // Settings ▸ Appearance wheeled down to *The window*.
                ev.push(egui::Event::PointerMoved(egui::pos2(
                    points.x * 0.55,
                    points.y * 0.5,
                )));
                ev.push(egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Line,
                    delta: vec2(0.0, -4.0),
                    phase: egui::TouchPhase::Move,
                    modifiers: egui::Modifiers::NONE,
                });
            }
            ev
        },
        |root| {
            frames += 1;
            if frames == 2 && fold {
                app.workspace.toggle_fold(Pane::Arrangement);
            }
            if frames == 2 && maximise {
                app.workspace.reveal(Pane::Arrangement);
                app.workspace.toggle_maximise(Pane::Arrangement);
            }
            if frames == 3 && s.case == "inspector" {
                cantus_ui::views::inspector::unfold(root.ctx(), "inspector.level");
                cantus_ui::views::inspector::unfold(root.ctx(), "inspector.sends");
                if let Some(id) = app
                    .project()
                    .strips
                    .iter()
                    .find(|x| x.name.to_lowercase().contains("chord"))
                    .map(|x| x.id)
                {
                    app.select_strip(id);
                }
            }
            for _ in 0..3 {
                processor.process(&mut audio, 2);
            }
            app.frame(root);
        },
    );
    path
}

/// Slate Paper with the greys of its room keeping `k` of their chroma (R2-B11). Each grey
/// moves toward the neutral grey of its own luminance in linear light, so every contrast
/// the theme makes stays where it was (to the 8-bit step); the accents, the selection, the
/// playhead, the meters and the function hues are left as they are.
fn paper_kept(k: f32) -> cantus_ui::kit::tokens::Tokens {
    use cantus_ui::kit::tokens::Tok;
    const KEEP: &[&str] = &[
        "text_select",
        "brand",
        "brand_bright",
        "highlight",
        "highlight_band",
        "highlight_glyph",
        "selection",
        "focus",
        "led",
        "danger",
        "fx_on",
        "meter_low",
        "meter_high",
        "meter_clip",
        "meter_mid",
        "step_odd_lit",
        "step_even_lit",
        "playhead",
        "menu_hover",
        "menu_text_hover",
        "hue_source",
        "hue_tone",
        "hue_dynamics",
        "hue_mod",
        "hue_space",
        "hue_output",
        "auto",
        "modulated",
        "destructive",
        "primary_ink",
        "light",
        "dark_shade",
        "readout",
        "readout_soft",
        "selection_tint",
        "loop_edge",
        "sheen",
        "glow",
        "glow_cool",
        "specular",
    ];
    let lin = |v: u8| {
        let v = v as f32 / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    let srgb = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        let e = if v <= 0.003_130_8 {
            v * 12.92
        } else {
            1.055 * v.powf(1.0 / 2.4) - 0.055
        };
        (e * 255.0).round() as u8
    };
    let base = cantus_ui::theme_slate::slate_paper_tokens();
    let mut t = base;
    for &tok in Tok::ALL {
        if KEEP.contains(&tok.field()) {
            continue;
        }
        let [cr, cg, cb, ca] = base.get(tok).to_srgba_unmultiplied();
        let (r, g, b) = (lin(cr), lin(cg), lin(cb));
        let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
        let m = |v: f32| srgb(y + k * (v - y));
        t.set(
            tok,
            egui::Color32::from_rgba_unmultiplied(m(r), m(g), m(b), ca),
        );
    }
    t
}
