//! The mixer desk, screenshotted without a screen: the **whole window** (transport,
//! arrangement, inspector and the desk in the lower zone), as a user sees it, through
//! the software renderer in `cantus_ui::raster`.
//!
//! `cargo run --release -p cantus-ui --example shoot_mixer -- <out-dir> [filter…]`
//!
//! Writes `mixer-<theme>-<w>x<h>-<strips>[-<variant>].png` for Slate and Paper, at
//! 1280 × 720 (the laptop where the old desk lost its bottom 170 px) and 1920 × 1080,
//! with 16 and 64 strips, the transport playing so the meters move. Variants: `es`
//! (Spanish), `75` and `200` (UI scale), `wide` and `xl` (width modes, `xl-75`,
//! `xl-200`), `cables-pick` (a pre-fader send's cable selected: its tap chip), `master` (a
//! mastering chain on the master, the master chosen so the inspector shows it, Extra
//! large) and `master-menu` (its menu open). A filter keeps only the files whose name
//! contains it.

use cantus_core::{Color, Project, Send, Ticks};
use cantus_engine::EngineHandle;
use cantus_ui::raster::shoot;
use cantus_ui::views::mixer::layout::CableView;
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

/// Routing on the grown desk (round 25): the last two strips are the Reverb and Delay
/// returns, docked at the right; strip 9 is a drum bus fed by strips 3, 4 and 6. The
/// selected strip (3) goes to the bus, sends pre-fader to the reverb, has a switched-off
/// send to the delay and a doubled send into the bus (its output already reaches it), and
/// keys a compressor on strip 2. Other strips send to the reverb; with 64 strips, strip 3
/// also sends to strip 40 (scrolled out of view in Wide: its edge chip).
fn routed(app: &mut CantusApp) {
    use cantus_core::{Command, Dock, EffectKind, SendTap};
    let ids: Vec<_> = app.project().strips.iter().map(|s| s.id).collect();
    let n = ids.len();
    let (rev, del, bus, sel) = (ids[n - 2], ids[n - 1], ids[8], ids[2]);
    let named = [
        (rev, "Reverb", Color::PALETTE[5]),
        (del, "Delay", Color::PALETTE[6]),
        (bus, "Drum Bus", Color::PALETTE[3]),
    ];
    for (id, name, color) in named {
        app.exec(Command::SetStripName {
            id,
            name: name.into(),
        });
        app.exec(Command::SetStripColor {
            id,
            color: Some(color),
        });
    }
    for id in [rev, del] {
        app.exec(Command::SetStripDock {
            id,
            dock: Dock::Right,
        });
    }
    for id in [sel, ids[3], ids[5]] {
        app.exec(Command::SetStripOutput {
            id,
            output: Some(bus),
        });
    }
    let send = |to, level: f32, enabled, tap| Send {
        to,
        level,
        enabled,
        tap,
    };
    app.set_strip_send(sel, send(rev, 0.5, true, SendTap::Pre));
    app.set_strip_send(sel, send(del, 0.35, false, SendTap::Post));
    app.set_strip_send(sel, send(bus, 0.3, true, SendTap::Post));
    if n >= 50 {
        app.set_strip_send(sel, send(ids[39], 0.6, true, SendTap::Post));
    }
    for (k, level) in [(0usize, 0.25f32), (4, 0.4), (6, 0.3), (7, 0.2)] {
        app.set_strip_send(ids[k], send(rev, level, true, SendTap::Post));
    }
    app.set_strip_send(ids[6], send(del, 0.3, true, SendTap::Post));
    app.add_effect(ids[1], EffectKind::Compressor);
    if let Some(fx) = app
        .project()
        .strip(ids[1])
        .and_then(|s| s.effects.last())
        .map(|e| e.id)
    {
        app.exec(Command::SetEffectSidechain {
            strip: ids[1],
            effect: fx,
            source: Some(sel),
        });
    }
    app.add_effect(sel, EffectKind::Eq);
    app.add_effect(sel, EffectKind::Saturator);
    app.add_effect(sel, EffectKind::Compressor);
    app.end_gesture();
}

/// What a picture is of.
#[derive(Copy, Clone)]
struct Variant {
    suffix: &'static str,
    locale: cantus_ui::i18n::Locale,
    ppp: f32,
    /// The width mode (0 Compact, 1 Wide, 2 Extra large), else the default.
    mode: Option<u8>,
    cables: Option<CableView>,
    /// The grown desk with [`routed`]'s routes, or the house demo's own desk.
    routes: Routes,
}

#[derive(Copy, Clone, PartialEq)]
enum Routes {
    None,
    Grown,
    House,
}

const fn var(
    suffix: &'static str,
    locale: cantus_ui::i18n::Locale,
    ppp: f32,
    mode: Option<u8>,
    cables: Option<CableView>,
    routes: Routes,
) -> Variant {
    Variant {
        suffix,
        locale,
        ppp,
        mode,
        cables,
        routes,
    }
}

fn main() {
    use cantus_ui::i18n::Locale::{En, Es};
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(args.next().unwrap_or_else(|| "shots".into()));
    let filters: Vec<String> = args.collect();
    std::fs::create_dir_all(&dir).expect("output dir");
    let themes = [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ];
    let sizes = [(1280.0f32, 720.0f32), (1920.0, 1080.0)];
    let (sel, all) = (Some(CableView::Selected), Some(CableView::All));
    let variants = [
        var("-master", En, 1.0, Some(2), None, Routes::None),
        var("-master-menu", En, 1.0, Some(2), None, Routes::None),
        var("-menu", En, 1.0, None, None, Routes::None),
        var("-desk-menu", En, 1.0, None, None, Routes::None),
        var("", En, 1.0, None, None, Routes::None),
        var("-es", Es, 1.0, None, None, Routes::None),
        var("-75", En, 0.75, None, None, Routes::None),
        var("-200", En, 2.0, None, None, Routes::None),
        var("-wide", En, 1.0, Some(1), None, Routes::None),
        var("-xl", En, 1.0, Some(2), sel, Routes::Grown),
        var("-xl-es", Es, 1.0, Some(2), sel, Routes::Grown),
        var("-xl-75", En, 0.75, Some(2), sel, Routes::Grown),
        var("-xl-200", En, 2.0, Some(2), sel, Routes::Grown),
        // The pre-fader send to the reverb selected: its pill and its tap chip.
        var("-cables-pick", En, 1.0, Some(1), sel, Routes::Grown),
        var("-cables-pick-75", En, 0.75, Some(1), sel, Routes::Grown),
        var("-cables-pick-es", Es, 1.0, Some(1), sel, Routes::Grown),
        var("-cables", En, 1.0, Some(1), sel, Routes::Grown),
        var("-cables-compact", En, 1.0, Some(0), sel, Routes::Grown),
        var("-cables-all", En, 1.0, Some(1), all, Routes::House),
        var("-cables-all-compact", En, 1.0, Some(0), all, Routes::House),
        var("-cables-allgrown", En, 1.0, Some(0), all, Routes::Grown),
        var("-cables-scrolled", En, 1.0, Some(1), sel, Routes::Grown),
        var("-cables-es", Es, 1.0, Some(1), sel, Routes::Grown),
        var("-cables-75", En, 0.75, Some(1), sel, Routes::Grown),
        var("-cables-200", En, 2.0, Some(1), sel, Routes::Grown),
        var(
            "-cables-off",
            En,
            1.0,
            Some(1),
            Some(CableView::Off),
            Routes::Grown,
        ),
    ];
    for (tn, theme) in &themes {
        for (w, h) in sizes {
            for n in [16usize, 64] {
                for v in variants {
                    // The house demo has its own desk: one picture per size.
                    if v.routes == Routes::House && n != 16 {
                        continue;
                    }
                    let name = format!("mixer-{tn}-{}x{}-{n}{}", w as u32, h as u32, v.suffix);
                    let wanted = if filters.is_empty() {
                        v.suffix.is_empty()
                    } else {
                        filters.iter().any(|f| match f.strip_suffix('$') {
                            // "-cables$": that variant only, not "-cables-all".
                            Some(exact) => name.ends_with(exact),
                            None => name.contains(f.as_str()),
                        })
                    };
                    if !wanted {
                        continue;
                    }
                    shoot_one(&dir, &name, theme, (w, h), n, v);
                }
            }
        }
    }
}

fn shoot_one(
    dir: &std::path::Path,
    name: &str,
    theme: &cantus_ui::theme::Theme,
    (w, h): (f32, f32),
    n: usize,
    v: Variant,
) {
    cantus_ui::i18n::set_locale(v.locale);
    let ctx = egui::Context::default();
    cantus_ui::design::install_fonts(&ctx);
    cantus_ui::design::install_style(&ctx);
    cantus_ui::theme::apply(theme, &ctx);
    let (engine, mut processor) = EngineHandle::manual(48_000.0);
    let project = match v.routes {
        Routes::House => (cantus_ui::demos::find("house").expect("house demo").build)(),
        _ => grown(n),
    };
    let mut app = CantusApp::from_parts(Some(project), engine, false);
    app.central = cantus_ui::app::CentralView::Mixer;
    if let Some(m) = v.mode {
        cantus_ui::views::mixer::set_width_mode(
            &mut app,
            cantus_ui::views::mixer::layout::WidthMode::from_index(m as usize),
        );
    }
    if let Some(c) = v.cables {
        let mut p = app.mixer.prefs();
        p.cables = c;
        app.mixer.set_prefs(p);
    }
    if v.routes == Routes::Grown {
        routed(&mut app);
    }
    let pick = if v.routes == Routes::House {
        // The chords: sidechained from the kick, sent to both returns.
        app.project()
            .strips
            .iter()
            .find(|s| s.effects.iter().any(|e| e.sidechain.is_some()))
            .map_or(app.project().strips[2].id, |s| s.id)
    } else {
        app.project().strips[2].id
    };
    app.select_strip(pick);
    if v.suffix.starts_with("-cables-pick") {
        let rev = app
            .project()
            .strips
            .iter()
            .find(|s| s.name == "Reverb")
            .map(|s| s.id);
        if let Some(rev) = rev {
            let route = cantus_core::routing::Route::send(pick, rev);
            app.mixer.select_cable(Some(route));
        }
    }
    if v.suffix.starts_with("-master") {
        master_chain(&mut app);
        app.select_master();
    }
    if v.suffix == "-master-menu" {
        let at = egui::pos2(95.0, h * 0.56);
        let master = Some(cantus_core::StripId::MASTER);
        cantus_ui::views::mixer::open_menu(&mut app, master, at);
    } else if v.suffix == "-menu" {
        let at = egui::pos2(w * 0.22, h * 0.62);
        cantus_ui::views::mixer::open_menu(&mut app, Some(pick), at);
    } else if v.suffix == "-desk-menu" {
        let at = egui::pos2(8.0, h * 0.47);
        cantus_ui::views::mixer::open_menu(&mut app, None, at);
    }
    app.sync_engine();
    app.toggle_play();
    let mut audio = vec![0f32; 800 * 2];
    let path = dir.join(format!("{name}.png"));
    // The size in points is the window's pixels over the UI scale.
    let size = egui::vec2(w / v.ppp, h / v.ppp);
    shoot(&ctx, size, v.ppp, 24, Some(&path), |root| {
        processor.process(&mut audio, 2);
        let mut frame = eframe::Frame::_new_kittest();
        eframe::App::ui(&mut app, root, &mut frame);
    });
    println!("wrote {}", path.display());
}
