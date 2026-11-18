//! Round 40's instruments read whole at every editor scale (fx/final's site pictures: at
//! 200 % Chorda's and Tympanum's views lost most of their labels). Its own test binary:
//! the scenes paint on this thread.
//!
//! The app made by `CantusApp::assemble`; its editor hosted at its size for the scale, as
//! a window sized to it shows it (and as the site's pictures are taken): Chorda (Play,
//! Mixer, Edit) and Tympanum (Kit, Mixer, Matrix) at the editor's 100 %, 150 % and 200 %,
//! and at the narrowest window the editor allows (three quarters of its size):
//!
//! - every word painted whole at 100 % is painted whole at the other scales;
//! - none is cut — elided, dropped, or painted past its card (`kit::text::probe`'s
//!   clipped probe);
//! - no two overlap, and none runs under a button (its own label inside it aside);
//!
//! in English and in Spanish.

mod paths_rig;

use cantus_core::{InstrumentKind, InstrumentSpec, NativeSpec, Project, TrackId};
use cantus_ui::kit::shell::{self, Shell};
use cantus_ui::kit::text::probe;
use cantus_ui::views::inst::{self, View};
use egui::{Pos2, Rect, Vec2};
use paths_rig::*;
use std::collections::BTreeSet;

fn song() -> (Project, TrackId, TrackId) {
    let mut p = cantus_ui::welcome_project();
    let mut ids = Vec::new();
    for (name, kind) in [
        ("Piano", InstrumentKind::Chorda),
        ("Drums", InstrumentKind::AcousticDrums),
    ] {
        let mut t = p.make_track(name);
        let strip = p.make_strip(name);
        t.strip = Some(strip.id);
        p.strips.push(strip);
        t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
        ids.push(t.id);
        p.tracks.push(t);
    }
    (p, ids[0], ids[1])
}

/// A word painted: what, where, and the part seen.
struct Word {
    text: String,
    rect: Rect,
    seen: Rect,
}

fn walk(s: &egui::Shape, clip: Rect, out: &mut Vec<Word>) {
    match s {
        egui::Shape::Text(t) => {
            let rect = t.galley.rect.translate(t.pos.to_vec2());
            out.push(Word {
                text: t.galley.text().trim().to_string(),
                rect,
                seen: rect.intersect(clip),
            });
        }
        egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
        _ => {}
    }
}

/// The editor's own host: a context of its own, in the app's look, and the textures it
/// has sent (the fonts' atlas is sent once).
struct Host {
    ctx: egui::Context,
    tex: cantus_ui::raster::Textures,
}

impl Host {
    fn new() -> Host {
        let ctx = egui::Context::default();
        cantus_ui::design::install_fonts(&ctx);
        cantus_ui::design::install_style(&ctx);
        cantus_ui::theme::apply(&cantus_ui::theme::slate(), &ctx);
        Host {
            ctx,
            tex: Default::default(),
        }
    }
}

/// A clock that only moves forward (egui's input history asks for it), a frame a call.
fn clock() -> f64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static FRAMES: AtomicU64 = AtomicU64::new(10_000);
    FRAMES.fetch_add(1, Ordering::Relaxed) as f64 / 60.0
}

/// The open instrument's editor hosted in `size` for a few frames, its cuts noted on the
/// last: the words painted and the cuts (and its picture, when shots are asked for).
fn host(
    r: &mut Rig,
    h: &mut Host,
    size: Vec2,
    clipped: bool,
    shot: &str,
) -> (Vec<Word>, Vec<probe::Cut>, Vec<Rect>) {
    let (ctx, tex) = (&h.ctx, &mut h.tex);
    let mut shapes = Vec::new();
    let mut cuts = Vec::new();
    let mut buttons = Vec::new();
    for i in 0..10 {
        if i == 9 {
            cantus_ui::kit::widgets::record_buttons(true);
            if clipped {
                probe::start_clipped();
            } else {
                probe::start();
            }
        }
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
            time: Some(clock()),
            predicted_dt: 1.0 / 60.0,
            ..Default::default()
        };
        let app = &mut r.app;
        let mut out = ctx.run_ui(input, |ui| inst::window_in_sized(app, ui, size));
        tex.update(&out.textures_delta);
        out.textures_delta.clear();
        shapes = out.shapes;
        if i == 9 {
            cuts = probe::take();
            buttons = cantus_ui::kit::widgets::take_buttons();
            cantus_ui::kit::widgets::record_buttons(false);
        }
    }
    let mut words = Vec::new();
    for c in &shapes {
        walk(&c.shape, c.clip_rect, &mut words);
    }
    if let Ok(dir) = std::env::var("CANTUS_PATHS_SHOTS") {
        let ppp = ctx.pixels_per_point();
        let prims = ctx.tessellate(shapes, ppp);
        let bg = ctx.global_style().visuals.panel_fill;
        let mut c = cantus_ui::raster::Canvas::new(
            (size.x * ppp).round() as usize,
            (size.y * ppp).round() as usize,
            bg,
        );
        c.paint(&prims, tex, ppp);
        let _ = std::fs::create_dir_all(&dir);
        let _ = c.write_png(&std::path::Path::new(&dir).join(format!("{shot}.png")));
    }
    words.retain(|w| !w.text.is_empty());
    (words, cuts, buttons)
}

/// Ask of one hosting: no cut, no two words overlapping, no word running under a button
/// (but its own label, inside it). The words seen whole.
fn check(
    at: &str,
    words: &[Word],
    cuts: &[probe::Cut],
    buttons: &[Rect],
    bad: &mut Vec<String>,
) -> BTreeSet<String> {
    for c in cuts {
        bad.push(format!(
            "{at}: {:?} -> {:?} ({}) in {:?}",
            c.full, c.shown, c.how, c.rect
        ));
    }
    let whole: Vec<&Word> = words.iter().filter(|w| w.seen == w.rect).collect();
    for (i, a) in whole.iter().enumerate() {
        for b in &whole[i + 1..] {
            let x = a.rect.intersect(b.rect);
            if x.width() > 2.0 && x.height() > 3.0 {
                bad.push(format!(
                    "{at}: “{}” and “{}” overlap at {x:?}",
                    a.text, b.text
                ));
            }
        }
    }
    for w in &whole {
        for bt in buttons {
            let x = w.rect.intersect(*bt);
            let own = bt.expand(1.0).contains_rect(w.rect);
            if !own && x.width() > 1.0 && x.height() > 2.0 {
                bad.push(format!(
                    "{at}: “{}” runs under a button {bt:?} at {x:?}",
                    w.text
                ));
            }
        }
    }
    whole.iter().map(|w| w.text.clone()).collect()
}

/// The editor's size at 100 % where `view` lays itself out whole: the shell's, or more
/// where the view asks for more (below its least size it scrolls, by design — a row half
/// out of sight there is no cut).
fn whole(kind: InstrumentKind, view: View) -> Vec2 {
    let chrome = Vec2::new(0.0, shell::HEADER + shell::FOOTER);
    let least = match (kind, view) {
        (InstrumentKind::Chorda, View::Mixer) => inst::chorda_source::mixer::MIN,
        (InstrumentKind::AcousticDrums, View::Mixer) => {
            Vec2::new(inst::tympanum::MIXER_MIN_W, 560.0)
        }
        (InstrumentKind::AcousticDrums, _) => inst::tympanum::KIT_MIN,
        // Every part with its picture: taller than the window, it scrolls there.
        (InstrumentKind::Chorda, View::Edit) => Vec2::new(0.0, 1250.0),
        // Its source list whole, unscrolled (at the shell's height it scrolls).
        (InstrumentKind::Chorda, View::Play) => Vec2::new(0.0, 800.0),
        _ => Vec2::ZERO,
    };
    shell::INSTRUMENT.max(least + chrome)
}

#[test]
fn chorda_and_tympanum_read_whole_at_every_scale_and_the_narrowest_window() {
    inst::chorda::scene::set_blocking(true);
    let (p, chorda, tymp) = song();
    let mut r = Rig::new(p);
    r.idle(4);
    let mut h = Host::new();
    let mut bad: Vec<String> = Vec::new();
    let mut read = 0;
    for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_thread_locale(lang);
        for (track, kind, name, views) in [
            (
                chorda,
                InstrumentKind::Chorda,
                "chorda",
                [View::Play, View::Mixer, View::Edit],
            ),
            (
                tymp,
                InstrumentKind::AcousticDrums,
                "tympanum",
                [View::Play, View::Mixer, View::Matrix],
            ),
        ] {
            r.app.open_inst_editor(track);
            for view in views {
                r.app.inst.view = view;
                let mut base: Option<BTreeSet<String>> = None;
                for k in [1.0f32, 1.5, 2.0] {
                    let at = format!("{lang:?} {name} {view:?} {:.0} %", k * 100.0);
                    Shell::new(("inst", track.0)).set_scale(&h.ctx, k);
                    let shot = format!("{name}-{view:?}-{:.0}-{}", k * 100.0, lang.code());
                    let (words, cuts, buttons) =
                        host(&mut r, &mut h, whole(kind, view) * k, true, &shot);
                    let set = check(&at, &words, &cuts, &buttons, &mut bad);
                    read += set.len();
                    match &base {
                        None => base = Some(set),
                        Some(b) => {
                            let missing: Vec<&String> = b.difference(&set).collect();
                            if !missing.is_empty() {
                                bad.push(format!(
                                "{at}: {} of the {} words at 100 % are not painted: {missing:?}",
                                missing.len(),
                                b.len()
                            ));
                            }
                        }
                    }
                }
                // The shell's own size, where a view taller or wider scrolls: no word cut or
                // overlapping there either.
                Shell::new(("inst", track.0)).set_scale(&h.ctx, 1.0);
                let (words, cuts, buttons) = host(
                    &mut r,
                    &mut h,
                    shell::INSTRUMENT,
                    false,
                    &format!("{name}-{view:?}-default-{}", lang.code()),
                );
                let at = format!("{lang:?} {name} {view:?} at the shell's size");
                check(&at, &words, &cuts, &buttons, &mut bad);
                // The narrowest window the editor allows: three quarters of its size (a view
                // with a least size scrolls there, so only its words' cuts are asked).
                Shell::new(("inst", track.0)).set_scale(&h.ctx, 1.0);
                let at = format!("{lang:?} {name} {view:?} narrowest");
                let (words, cuts, buttons) = host(
                    &mut r,
                    &mut h,
                    shell::INSTRUMENT * 0.75,
                    false,
                    &format!("{name}-{view:?}-narrowest-{}", lang.code()),
                );
                // The header is the shell's, every instrument's (its name gives way to the
                // tools at this width: noted apart, not this view's).
                let body: Vec<probe::Cut> = cuts
                    .into_iter()
                    .filter(|c| c.rect.top() >= shell::HEADER - 0.5)
                    .collect();
                check(&at, &words, &body, &buttons, &mut bad);
            }
            r.app.close_inst_editor();
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    assert!(
        read > 300,
        "only {read} words were read:\n{}",
        bad.join("\n")
    );
    assert!(
        bad.is_empty(),
        "{} problems:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// Each instrument's editor opened fresh in the assembled app opens at its own size, its
/// default view uncut at the window's edge (fx/final: Tympanum's Kit at the shell's
/// 1 040 × 680 ran past the window's right and foot). A list inside it may scroll.
#[test]
fn an_instrument_opens_at_its_own_size_its_view_uncut() {
    inst::chorda::scene::set_blocking(true);
    let (p, chorda, tymp) = song();
    let mut r = Rig::new(p);
    r.idle(4);
    let mut bad = Vec::new();
    for (track, kind) in [
        (chorda, InstrumentKind::Chorda),
        (tymp, InstrumentKind::AcousticDrums),
    ] {
        r.app.open_inst_editor(track);
        r.idle(12);
        let key = inst::editor_id(track).value();
        let Some((ed, _)) = cantus_ui::views::workspace_device::shown(&r.ctx, key) else {
            bad.push(format!("{kind:?}: the editor is not drawn"));
            continue;
        };
        r.shot(&format!("opened-{kind:?}"));
        let want = inst::editor_size(kind);
        if ed.width() + 0.5 < want.x || ed.height() + 0.5 < want.y {
            bad.push(format!(
                "{kind:?}: opened at {:?}, not its {want:?}",
                ed.size()
            ));
        }
        // No word cut — at the editor's edge, or by the view scrolling because the window
        // is short of its whole size. Only the lists at the left (Chorda's sources,
        // Tympanum's kits) scroll by design.
        let list = Rect::from_min_size(ed.min, egui::vec2(260.0, ed.height()));
        for t in &r.texts {
            if t.text.trim().is_empty() || !t.rect.intersects(ed) || t.seen == t.rect {
                continue;
            }
            let in_list = list.contains_rect(t.rect);
            if !ed.contains_rect(t.rect) || !in_list {
                bad.push(format!(
                    "{kind:?}: “{}” cut at the editor's edge: {:?} of {:?}",
                    t.text.trim(),
                    t.seen,
                    t.rect
                ));
            }
        }
        r.app.close_inst_editor();
        r.idle(4);
    }
    assert!(
        bad.is_empty(),
        "{} problems:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// The shell's header never cuts an instrument's name: at the narrowest window the editor
/// allows, every instrument's name is painted whole, in English and Spanish — the
/// track's part and the other secondary items give way first.
#[test]
fn the_header_keeps_every_instruments_name_whole_at_the_narrowest_window() {
    inst::chorda::scene::set_blocking(true);
    let mut p = cantus_ui::welcome_project();
    let mut tracks = Vec::new();
    for kind in InstrumentKind::ALL {
        let mut t = p.make_track("A long track name");
        let strip = p.make_strip("A long track name");
        t.strip = Some(strip.id);
        p.strips.push(strip);
        t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
        tracks.push((kind, t.id));
        p.tracks.push(t);
    }
    let mut r = Rig::new(p);
    r.idle(4);
    let mut h = Host::new();
    let mut bad = Vec::new();
    for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_thread_locale(lang);
        for &(kind, track) in &tracks {
            r.app.open_inst_editor(track);
            Shell::new(("inst", track.0)).set_scale(&h.ctx, 1.0);
            let shot = format!("header-{kind:?}-narrowest-{}", lang.code());
            let (words, cuts, _) = host(&mut r, &mut h, shell::INSTRUMENT * 0.75, false, &shot);
            let name = cantus_ui::i18n::t(kind.name_key());
            let whole = words.iter().any(|w| {
                w.text.eq_ignore_ascii_case(&name)
                    && w.seen == w.rect
                    && w.rect.top() < shell::HEADER
            });
            if !whole {
                let seen: Vec<&str> = words
                    .iter()
                    .filter(|w| w.rect.top() < shell::HEADER)
                    .map(|w| w.text.as_str())
                    .collect();
                bad.push(format!(
                    "{lang:?} {kind:?}: “{name}” not whole in the header: {seen:?}"
                ));
            }
            for c in cuts.iter().filter(|c| c.rect.top() < shell::HEADER - 0.5) {
                bad.push(format!(
                    "{lang:?} {kind:?}: the header cut {:?} -> {:?} ({})",
                    c.full, c.shown, c.how
                ));
            }
            r.app.close_inst_editor();
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    assert!(
        bad.is_empty(),
        "{} problems:\n{}",
        bad.len(),
        bad.join("\n")
    );
}
