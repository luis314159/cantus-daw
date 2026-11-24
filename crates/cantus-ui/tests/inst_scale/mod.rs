//! The instrument editors read whole at every scale and at the narrowest window: the host and
//! the checks `instruments_scale_e2e` (Chorda, Tympanum) and `instruments_narrow_e2e` (every
//! other kind) share (r42 fix4-narrow — Exemplar's Edit view cut six cells and a card's
//! title at the narrowest window at 150 %, in Spanish, and no test looked).
//!
//! The app made by `CantusApp::assemble` (`paths_rig::Rig`); its editor hosted at a size, as
//! a window sized to it shows it:
//!
//! - every word painted whole at 100 % is painted whole at 150 % and 200 %;
//! - none is cut — elided, dropped, or painted past its card (`kit::text::probe`'s clipped
//!   probe, at the view's whole size where nothing scrolls);
//! - no two overlap, and none runs under a button (its own label inside it aside);
//! - at the shell's own size, and at the narrowest window the editor allows at each scale
//!   (three quarters of the shell, `inst::window`'s `min_size`), no word is elided or
//!   dropped and none overlaps;
//! - at that narrowest width and the view's whole height — what the narrowest window shows
//!   as it scrolls down, the kit painting no word out of sight — none is cut either, nor
//!   painted past its card where the view does not scroll sideways (a view that keeps the
//!   width it was laid out for, `inst::keeps_designed`, scrolls there by design);
//!
//! in English and in Spanish. A word in its designed short form (`<key>.short`) is read as
//! the word whole: the kit's own abbreviation, not a cut.
#![allow(dead_code)]

use crate::paths_rig::Rig;
use cantus_core::{InstrumentKind, TrackId};
use cantus_ui::kit::shell::{self, Shell};
use cantus_ui::kit::text::probe;
use cantus_ui::views::inst::{self, View};
use egui::{Pos2, Rect, Vec2};
use std::collections::BTreeSet;

/// A word painted: what, where, and the part seen.
pub struct Word {
    pub text: String,
    pub rect: Rect,
    pub seen: Rect,
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

/// A binary's tests take turns (beta.15's AppImage job): the theme is the process's
/// (`theme::apply` installs its palette and the kit's tokens for every thread), so a test
/// assembling its app — the default theme, Slate Lapis — while another measured Chorda at
/// 150 % repainted that one's knob labels in another lettering, and its words "went
/// missing". The app has one interface thread; only tests run several.
pub fn turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

/// A binary's tests take turns (docs/RELEASING.md L5): `theme::apply` installs its palette
/// and the kit's tokens for the whole process, so a test applying a theme repainted another's
/// words mid-measure (beta.16's rehearsal, macOS: fx_limiter's dark demo cut "GAIN REDUCTION").
/// The app has one interface thread; only tests run several.
pub fn theme_turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    // Every test starts in the app's own look, whatever the test before it applied.
    cantus_ui::theme::apply(
        &cantus_ui::theme::default_theme(),
        &egui::Context::default(),
    );
    turn
}

/// The editor's own host: a context of its own, in the app's look, and the textures it
/// has sent (the fonts' atlas is sent once).
pub struct Host {
    pub ctx: egui::Context,
    tex: cantus_ui::raster::Textures,
}

impl Host {
    pub fn new() -> Host {
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

thread_local! {
    /// The designed short forms (`<key>.short`) the last hosting painted.
    static SHORTS: std::cell::RefCell<Vec<probe::Cut>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// `set` with the last hosting's designed short forms read as their words whole ("Últ."
/// painted for "Última"): a word read in its short form at one scale and whole at another
/// is read at both.
fn shorts_whole(set: &mut BTreeSet<String>) {
    SHORTS.with(|s| {
        for c in s.borrow().iter() {
            set.remove(c.shown.trim());
            set.insert(c.full.trim().to_string());
        }
    })
}

/// What one hosting painted: the words, the kit's cuts, the buttons.
pub type Hosted = (Vec<Word>, Vec<probe::Cut>, Vec<Rect>);

/// The open instrument's editor hosted in `size` for a few frames, its cuts noted on the
/// last: the words painted and the cuts (and its picture, when `CANTUS_PATHS_SHOTS` names a
/// folder).
pub fn host(r: &mut Rig, h: &mut Host, size: Vec2, clipped: bool, shot: &str) -> Hosted {
    const FRAMES: usize = 10;
    let (ctx, tex) = (&h.ctx, &mut h.tex);
    let mut shapes = Vec::new();
    let mut cuts = Vec::new();
    let mut buttons = Vec::new();
    for i in 0..FRAMES {
        let last = i == FRAMES - 1;
        if last {
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
        if last {
            cuts = probe::take();
            SHORTS.with(|s| *s.borrow_mut() = probe::take_shorts());
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
pub fn check(
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
    // Whole: seen to within a point — a line's box (its leading, past the letters' ink)
    // clipped by a fraction of a point at another scale is no cut.
    let whole: Vec<&Word> = words
        .iter()
        .filter(|w| w.seen.expand(1.0).contains_rect(w.rect))
        .collect();
    for (i, a) in whole.iter().enumerate() {
        for b in &whole[i + 1..] {
            // One word painted in layers — an engraved or embossed plate's name, its light
            // and its shadow a point apart — is one word, not two overlapping.
            let layered = a.text == b.text
                && (a.rect.min - b.rect.min).length() <= 2.0
                && (a.rect.max - b.rect.max).length() <= 2.0;
            if layered {
                continue;
            }
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

/// One instrument to read: its track, its kind, a short name for the pictures, its views.
pub struct Case {
    pub track: TrackId,
    pub kind: InstrumentKind,
    pub name: &'static str,
    pub views: Vec<View>,
}

/// The views the editor's header offers for `kind` (`inst::frame::views_of`).
pub fn views_of(kind: InstrumentKind) -> Vec<View> {
    match kind {
        InstrumentKind::Dancer => vec![View::Play, View::Edit],
        InstrumentKind::AcousticDrums => vec![View::Play, View::Mixer, View::Matrix],
        InstrumentKind::Chorda => vec![View::Play, View::Mixer, View::Edit, View::Matrix],
        _ => vec![View::Play, View::Edit, View::Matrix],
    }
}

/// Read every case's views in English and Spanish: at `whole(kind, view)` (the editor's
/// size at 100 % where the view lays itself out whole) times 100 %, 150 % and 200 % with the
/// clipped probe, then at the shell's own size, then at the narrowest window the editor
/// allows at each of the three scales. Returns the words read; the problems go to `bad`.
pub fn read_views(
    r: &mut Rig,
    h: &mut Host,
    cases: &[Case],
    whole: impl Fn(InstrumentKind, View) -> Vec2,
    bad: &mut Vec<String>,
) -> usize {
    let mut read = 0;
    for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        cantus_ui::i18n::set_thread_locale(lang);
        for case in cases {
            let (track, name) = (case.track, case.name);
            r.app.open_inst_editor(track);
            for &view in &case.views {
                r.app.inst.view = view;
                let mut base: Option<BTreeSet<String>> = None;
                for k in [1.0f32, 1.5, 2.0] {
                    let at = format!("{lang:?} {name} {view:?} {:.0} %", k * 100.0);
                    Shell::new(("inst", track.0)).set_scale(&h.ctx, k);
                    let shot = format!("{name}-{view:?}-{:.0}-{}", k * 100.0, lang.code());
                    let size = whole(case.kind, view) * k;
                    let (words, cuts, buttons) = host(r, h, size, true, &shot);
                    let mut set = check(&at, &words, &cuts, &buttons, bad);
                    read += set.len();
                    shorts_whole(&mut set);
                    match &base {
                        None => base = Some(set),
                        Some(b) => {
                            // A paragraph may break its lines at other words at another
                            // scale (fx/final: Chorda's Spanish mixer note at 1 119 points):
                            // a line is missing only when one of its words is.
                            let each: BTreeSet<&str> =
                                set.iter().flat_map(|l| l.split_whitespace()).collect();
                            let missing: Vec<&String> = b
                                .difference(&set)
                                .filter(|l| l.split_whitespace().any(|w| !each.contains(w)))
                                .collect();
                            if !missing.is_empty() {
                                // Where each one is painted, if it is, and the part seen.
                                let seen: Vec<String> = missing
                                    .iter()
                                    .filter_map(|m| words.iter().find(|w| &&w.text == m))
                                    .map(|w| format!("{:?} seen {:?}", w.rect, w.seen))
                                    .collect();
                                bad.push(format!(
                                    "{at}: {} of the {} words at 100 % are not painted: \
                                     {missing:?} {seen:?}",
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
                    r,
                    h,
                    shell::INSTRUMENT,
                    false,
                    &format!("{name}-{view:?}-default-{}", lang.code()),
                );
                let at = format!("{lang:?} {name} {view:?} at the shell's size");
                check(&at, &words, &cuts, &buttons, bad);
                // The narrowest window the editor allows at each scale: three quarters of the
                // shell's size (`inst::window`'s least size; a view with a least size scrolls
                // there, so only its words' cuts are asked).
                for k in [1.0f32, 1.5, 2.0] {
                    Shell::new(("inst", track.0)).set_scale(&h.ctx, k);
                    let at = format!("{lang:?} {name} {view:?} narrowest at {:.0} %", k * 100.0);
                    let (words, cuts, buttons) = host(
                        r,
                        h,
                        shell::INSTRUMENT * 0.75 * k,
                        false,
                        &format!("{name}-{view:?}-narrowest-{:.0}-{}", k * 100.0, lang.code()),
                    );
                    // The header is the shell's, every instrument's (its name gives way to
                    // the tools at this width: noted apart, not this view's).
                    let body: Vec<probe::Cut> = cuts
                        .into_iter()
                        .filter(|c| c.rect.top() >= shell::HEADER * k - 0.5)
                        .collect();
                    check(&at, &words, &body, &buttons, bad);
                    // The same width, as tall as the view is whole: what the narrowest
                    // window shows as it scrolls down (the kit paints no word out of
                    // sight, so the window alone never sees its lower cards — r42
                    // fix4-narrow: Exemplar's lower cards were cut only there).
                    let tall = Vec2::new(
                        shell::INSTRUMENT.x * 0.75,
                        whole(case.kind, view).y.max(shell::INSTRUMENT.y * 0.75),
                    ) * k;
                    let at = format!(
                        "{lang:?} {name} {view:?} narrowest, scrolled whole, at {:.0} %",
                        k * 100.0
                    );
                    // A view with a least width of its own scrolls sideways there (Chorda's
                    // and Tympanum's mixers): its words half out of sight are no cut.
                    let wide = whole(case.kind, view).x > shell::INSTRUMENT.x + 0.5
                        || inst::keeps_designed(case.kind, view).0;
                    let (words, cuts, buttons) = host(
                        r,
                        h,
                        tall,
                        !wide,
                        &format!(
                            "{name}-{view:?}-narrowest-tall-{:.0}-{}",
                            k * 100.0,
                            lang.code()
                        ),
                    );
                    let body: Vec<probe::Cut> = cuts
                        .into_iter()
                        .filter(|c| c.rect.top() >= shell::HEADER * k - 0.5)
                        .collect();
                    check(&at, &words, &body, &buttons, bad);
                }
                Shell::new(("inst", track.0)).set_scale(&h.ctx, 1.0);
            }
            r.app.close_inst_editor();
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    read
}
