//! The mixer desk, screenshotted without a screen: the **whole window** (transport,
//! arrangement, inspector and the desk in the lower zone), as a user sees it, through
//! the software renderer in `cantus_ui::raster`.
//!
//! `cargo run --release -p cantus-ui --example shoot_mixer -- <out-dir> [filter…]`
//!
//! Writes `mixer-<theme>-<w>x<h>-<strips>[-<variant>].png` for Slate and Paper, at
//! 1280 × 720 (the laptop where the old desk lost its bottom 170 px) and 1920 × 1080,
//! with 16 and 64 strips, the transport playing so the meters move. Variants: `es`
//! (Spanish), `75` and `200` (UI scale), `wide` and `xl` (width modes), `master` (a
//! mastering chain on the master, the master chosen so the inspector shows it, Extra
//! large) and `master-menu` (its menu open). A filter keeps only the files whose name
//! contains it.

use cantus_core::{Color, Project, Send, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::{welcome_project, CantusApp};
use std::path::PathBuf;

/// The welcome project grown to `n` strips, every one of them fed by a playing track
/// with a name, a colour and a clip — the "mixer 64" of `rendering-performance.md` §2.
pub fn grown(n: usize) -> Project {
    let mut p = welcome_project();
    while p.strips.len() < n {
        let s = p.make_strip("");
        p.strips.push(s);
    }
    const NAMES: [&str; 16] = [
        "Bass",
        "Sub",
        "Pad Wide",
        "Lead Vocal",
        "Ghost Kick Layer 2",
        "Strings",
        "Brass",
        "Pluck",
        "Arp",
        "FX Riser",
        "Choir",
        "Guitar L",
        "Guitar R",
        "Organ",
        "Perc",
        "Clap",
    ];
    let mut i = 0;
    while p.tracks.len() < n.min(p.strips.len()) {
        let name = NAMES[i % NAMES.len()];
        i += 1;
        let mut t = p.make_track(name);
        t.color = Color::PALETTE[(i * 5) % Color::PALETTE.len()];
        let mut clip = p.make_clip(name, Ticks::ZERO, Ticks::beats(16));
        for k in 0..16 {
            let pitch = 48 + ((k * 7 + i * 3) % 24) as u8;
            let note = p.make_note(
                Ticks::beats(k as i64),
                Ticks::QUARTER - Ticks(60),
                pitch,
                70 + (k % 4) as u8 * 12,
            );
            clip.notes.push(note);
        }
        t.clips.push(clip);
        if t.strip.is_none() {
            break;
        }
        p.tracks.push(t);
    }
    // A little life on the desk: a few faders off unity, a pan, a muted and a soloed
    // strip, a send, an insert.
    for (k, s) in p.strips.iter_mut().enumerate() {
        s.gain_db = [0.0, -3.5, -6.0, -1.5, -9.0, -4.0][k % 6];
        s.pan = [0.0, -0.3, 0.25, 0.0, 0.5, -0.6][k % 6];
    }
    if p.strips.len() > 6 {
        p.strips[4].muted = true;
        let to = p.strips[p.strips.len() - 2].id;
        p.strips[1].sends.push(Send::new(to));
    }
    p
}

/// A mastering chain on the master (ADR 0026): an EQ, a compressor (bypassed), the
/// limiter, and its mini views open in the inspector.
fn master_chain(app: &mut CantusApp) {
    use cantus_core::{Command, EffectKind, StripId};
    let m = StripId::MASTER;
    for kind in [EffectKind::Eq, EffectKind::Compressor, EffectKind::Limiter] {
        app.add_effect(m, kind);
    }
    let ids: Vec<_> = app.project().master.effects.iter().map(|e| e.id).collect();
    app.exec(Command::SetEffectBypass {
        strip: m,
        effect: ids[1],
        bypass: true,
    });
    app.exec(Command::SetEffectMix {
        strip: m,
        effect: ids[0],
        mix: 0.75,
    });
    app.inspector.toggle_mini(ids[0]);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    let themes = [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ];
    let sizes = [(1280.0f32, 720.0f32), (1920.0, 1080.0)];
    // (suffix, locale, ppp, width mode 0/1/2)
    let variants: [(&str, cantus_ui::i18n::Locale, f32, Option<u8>); 10] = [
        ("-master", cantus_ui::i18n::Locale::En, 1.0, Some(2)),
        ("-master-menu", cantus_ui::i18n::Locale::En, 1.0, Some(2)),
        ("-menu", cantus_ui::i18n::Locale::En, 1.0, None),
        ("-desk-menu", cantus_ui::i18n::Locale::En, 1.0, None),
        ("", cantus_ui::i18n::Locale::En, 1.0, None),
        ("-es", cantus_ui::i18n::Locale::Es, 1.0, None),
        ("-75", cantus_ui::i18n::Locale::En, 0.75, None),
        ("-200", cantus_ui::i18n::Locale::En, 2.0, None),
        ("-wide", cantus_ui::i18n::Locale::En, 1.0, Some(1)),
        ("-xl", cantus_ui::i18n::Locale::En, 1.0, Some(2)),
    ];
    for (tn, theme) in &themes {
        for (w, h) in sizes {
            for n in [16usize, 64] {
                for (suffix, locale, ppp, mode) in variants {
                    let name = format!("mixer-{tn}-{}x{}-{n}{suffix}", w as u32, h as u32);
                    let wanted = if filters.is_empty() {
                        suffix.is_empty()
                    } else {
                        filters.iter().any(|f| name.contains(f.as_str()))
                    };
                    if !wanted {
                        continue;
                    }
                    cantus_ui::i18n::set_locale(locale);
                    let ctx = egui::Context::default();
                    cantus_ui::design::install_fonts(&ctx);
                    cantus_ui::design::install_style(&ctx);
                    cantus_ui::theme::apply(theme, &ctx);
                    let (engine, mut processor) = EngineHandle::manual(48_000.0);
                    let mut app = CantusApp::from_parts(Some(grown(n)), engine, false);
                    app.central = cantus_ui::app::CentralView::Mixer;
                    if let Some(m) = mode {
                        cantus_ui::views::mixer::set_width_mode(
                            &mut app,
                            cantus_ui::views::mixer::layout::WidthMode::from_index(m as usize),
                        );
                    }
                    let pick = app.project().strips[2].id;
                    app.select_strip(pick);
                    if suffix.starts_with("-master") {
                        master_chain(&mut app);
                        app.select_master();
                    }
                    if suffix == "-master-menu" {
                        let at = egui::pos2(95.0, h * 0.56);
                        let master = Some(cantus_core::StripId::MASTER);
                        cantus_ui::views::mixer::open_menu(&mut app, master, at);
                    } else if suffix == "-menu" {
                        let at = egui::pos2(w * 0.22, h * 0.62);
                        cantus_ui::views::mixer::open_menu(&mut app, Some(pick), at);
                    } else if suffix == "-desk-menu" {
                        let at = egui::pos2(8.0, h * 0.47);
                        cantus_ui::views::mixer::open_menu(&mut app, None, at);
                    }
                    app.sync_engine();
                    app.toggle_play();
                    let mut audio = vec![0f32; 800 * 2];
                    let path = dir.join(format!("{name}.png"));
                    // The size in points is the window's pixels over the UI scale.
                    let size = egui::vec2(w / ppp, h / ppp);
                    shoot(&ctx, size, ppp, 24, Some(&path), |root| {
                        processor.process(&mut audio, 2);
                        let mut frame = eframe::Frame::_new_kittest();
                        eframe::App::ui(&mut app, root, &mut frame);
                    });
                    println!("wrote {}", path.display());
                }
            }
        }
    }
}
