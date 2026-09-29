//! The inspector (ADR 0024 §7): it follows what was clicked last, it never shows a track's
//! instrument for a strip that track does not feed, the strip's gain and pan appear once,
//! its size keeps to its range, no two texts collide in English or Spanish at 75 % and
//! 200 %, and a still frame costs no allocation of its own.

mod support;
use cantus_core::{Command, EffectKind, Send};
use cantus_ui::views::inspector::{self, Inspector, Subject};
use cantus_ui::{welcome_project, CantusApp};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use support::*;

struct Counting;
static COUNTING: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static THIS_THREAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) && THIS_THREAD.with(|t| t.get()) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if COUNTING.load(Ordering::Relaxed) && THIS_THREAD.with(|t| t.get()) {
            ALLOCS.fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Counting = Counting;

/// The welcome song with Keys' strip carrying three inserts, two sends and an EQ that is
/// not flat, and a bus nothing feeds. Returns (Keys' strip, the bus).
fn scene(a: &mut CantusApp) -> (cantus_core::StripId, cantus_core::StripId) {
    let strip = a.project().tracks[0].strip.unwrap();
    for k in [EffectKind::Eq, EffectKind::Compressor, EffectKind::Space] {
        a.add_effect(strip, k);
    }
    a.exec(Command::SetStripEq {
        id: strip,
        band: 1,
        field: cantus_core::EqField::Gain,
        value: -3.5,
    });
    a.exec(Command::SetStripGain {
        id: strip,
        gain_db: -2.4,
    });
    let mut ids = Vec::new();
    for name in ["Reverb", "Delay", "Bus"] {
        let s = a.bus.new_strip(name);
        ids.push(s.id);
        let index = a.project().strips.len();
        a.exec(Command::InsertStrip {
            index,
            strip: Box::new(s),
        });
    }
    a.set_strip_send(strip, Send::new(ids[0]));
    a.set_strip_send(strip, Send::new(ids[1]));
    a.end_gesture();
    (strip, ids[2])
}

/// One or more frames of the inspector in a panel `w` wide; the text of the last.
fn frames(
    ctx: &egui::Context,
    a: &mut CantusApp,
    w: f32,
    n: usize,
) -> Vec<egui::epaint::ClippedShape> {
    let mut last = Vec::new();
    for i in 0..n {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(w, 1400.0),
            )),
            time: Some(i as f64 * 0.016),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| inspector::show(a, ui));
        });
        out.textures_delta.clear();
        last = out.shapes;
    }
    last
}

#[test]
fn a_strip_click_shows_the_channel_and_a_track_click_the_track() {
    let mut a = app(welcome_project());
    let (strip, _) = scene(&mut a);
    let keys = a.project().tracks[0].id;
    let ctx = ctx();
    a.select_track(keys);
    assert_eq!(inspector::subject(&a), Subject::Track(keys));
    let text = painted_text(&frames(&ctx, &mut a, 300.0, 2));
    assert!(
        text.contains(&cantus_ui::i18n::t("inspector.ctx.track")),
        "{text}"
    );
    assert!(
        text.contains(&cantus_ui::i18n::t("inspector.track_level")),
        "{text}"
    );
    // The same things selected, but the strip was clicked last: its channel alone.
    a.select_strip(strip);
    assert_eq!(inspector::subject(&a), Subject::Strip(strip));
    let text = painted_text(&frames(&ctx, &mut a, 300.0, 2));
    assert!(
        text.contains(&cantus_ui::i18n::t("kit.mixer.title")),
        "{text}"
    );
    assert!(
        !text.contains(&cantus_ui::i18n::t("inspector.track_level")),
        "{text}"
    );
    assert!(
        text.contains(&cantus_ui::i18n::t(EffectKind::Eq.name_key())),
        "{text}"
    );
    // A clip: its rows, above the channel.
    let clip = a.project().tracks[0].clips[0].id;
    a.select_clip(keys, clip);
    assert_eq!(inspector::subject(&a), Subject::Clip(keys, clip));
}

#[test]
fn a_strip_no_track_feeds_never_shows_the_last_tracks_instrument() {
    let mut a = app(welcome_project());
    let (_, bus) = scene(&mut a);
    let keys = a.project().tracks[0].id;
    let ctx = ctx();
    a.select_track(keys);
    a.select_strip(bus);
    assert_eq!(inspector::subject(&a), Subject::Strip(bus));
    let text = painted_text(&frames(&ctx, &mut a, 300.0, 2));
    let instrument = cantus_ui::i18n::t(a.project().tracks[0].instrument.name_key());
    assert!(
        !text.contains(&instrument),
        "a bus showed Keys' instrument: {text}"
    );
    assert!(
        !text.contains(&cantus_ui::i18n::t("inspector.title")),
        "{text}"
    );
    assert!(text.contains("Bus"), "the title names the bus: {text}");
}

#[test]
fn the_strips_gain_and_pan_appear_once() {
    let mut a = app(welcome_project());
    let (strip, _) = scene(&mut a);
    let keys = a.project().tracks[0].id;
    let ctx = ctx();
    for pick in [0, 1] {
        if pick == 0 {
            a.select_track(keys);
        } else {
            a.select_strip(strip);
        }
        let text = painted_text(&frames(&ctx, &mut a, 300.0, 2));
        // −2.4 dB is the strip's fader, readout in the pinned column only.
        assert_eq!(text.matches("\u{2212}2.4").count(), 1, "{text}");
    }
}

#[test]
fn the_width_keeps_its_range_snaps_resets_and_folds() {
    let mut i = Inspector::default();
    assert_eq!(i.width, inspector::WIDTH);
    i.set_width(100.0);
    assert_eq!(i.width, inspector::MIN_WIDTH);
    i.set_width(900.0);
    assert_eq!(i.width, inspector::MAX_WIDTH);
    i.set_width(inspector::WIDTH + 6.0);
    assert_eq!(i.width, inspector::WIDTH, "a drag near 280 lands on it");
    i.set_width(330.0);
    assert_eq!(i.width, 330.0);
    let mut a = app(welcome_project());
    a.inspector.toggle_collapsed();
    assert_eq!(inspector::desired_width(&a), inspector::RAIL);
    a.inspector.toggle_collapsed();
    // A host wider than the range: the panel keeps to 420 and says so.
    let ctx = ctx();
    frames(&ctx, &mut a, 1000.0, 1);
    assert_eq!(inspector::desired_width(&a), inspector::MAX_WIDTH);
}

/// The visible part of every text the frame painted (what its clip leaves of it),
/// unrotated ones only.
fn text_rects(shapes: &[egui::epaint::ClippedShape]) -> Vec<(egui::Rect, String)> {
    let mut out = Vec::new();
    fn walk(s: &egui::Shape, clip: egui::Rect, out: &mut Vec<(egui::Rect, String)>) {
        match s {
            egui::Shape::Text(t) if t.angle == 0.0 && !t.galley.text().is_empty() => {
                let r = t.galley.rect.translate(t.pos.to_vec2()).intersect(clip);
                if r.is_positive() {
                    out.push((r, t.galley.text().to_string()))
                }
            }
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, clip, out)),
            _ => {}
        }
    }
    for s in shapes {
        walk(&s.shape, s.clip_rect, &mut out);
    }
    out
}

#[test]
fn no_two_texts_collide_in_either_language_at_75_and_200_percent() {
    for locale in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
        for zoom in [0.75f32, 2.0] {
            for width in [220.0f32, 280.0, 420.0] {
                let mut a = app(welcome_project());
                let (strip, _) = scene(&mut a);
                let keys = a.project().tracks[0].id;
                let ctx = ctx();
                cantus_ui::i18n::default_thread_locale(locale);
                ctx.set_pixels_per_point(zoom);
                for pick in [0, 1] {
                    if pick == 0 {
                        a.select_track(keys);
                    } else {
                        a.select_strip(strip);
                    }
                    let shapes = frames(&ctx, &mut a, width, 3);
                    let rects = text_rects(&shapes);
                    // A number is never cut short: its well is wide enough for it.
                    for (_, t) in &rects {
                        let numeric =
                            t.starts_with(|c: char| c.is_ascii_digit() || "+\u{2212}-".contains(c));
                        assert!(
                            !(numeric && t.contains('\u{2026}')),
                            "the value {t:?} was cut at {width} pt, {zoom}×, {locale:?}"
                        );
                    }
                    for (i, (r, t)) in rects.iter().enumerate() {
                        for (q, u) in &rects[i + 1..] {
                            let o = r.intersect(*q);
                            assert!(
                                o.width() <= 1.0 || o.height() <= 1.0,
                                "{t:?} {r:?} and {u:?} {q:?} collide at {width} pt, {zoom}×, {locale:?}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a_still_frame_allocates_nothing_of_its_own() {
    let mut a = app(welcome_project());
    let (strip, _) = scene(&mut a);
    a.select_strip(strip);
    let ctx = ctx();
    let run = |a: &mut CantusApp, count: bool| {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(300.0, 1400.0),
            )),
            ..Default::default()
        };
        let mut n = 0;
        let mut out = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ui, |ui| {
                    THIS_THREAD.with(|t| t.set(true));
                    ALLOCS.store(0, Ordering::Relaxed);
                    COUNTING.store(count, Ordering::Relaxed);
                    inspector::show(a, ui);
                    COUNTING.store(false, Ordering::Relaxed);
                    THIS_THREAD.with(|t| t.set(false));
                    n = ALLOCS.load(Ordering::Relaxed);
                });
        });
        out.textures_delta.clear();
        n
    };
    for _ in 0..8 {
        run(&mut a, false);
    }
    let n = run(&mut a, true);
    eprintln!("inspector: {n} allocations in a still frame");
    // How long a whole frame of it takes (egui included), for the report; the budget is
    // asserted by the workspace's frame test.
    let t0 = std::time::Instant::now();
    for _ in 0..100 {
        run(&mut a, false);
    }
    eprintln!(
        "inspector: {:.3} ms a frame (egui's pass included)",
        t0.elapsed().as_secs_f64() * 10.0
    );
    // egui's own bookkeeping for the widgets the rows sense is all that may remain.
    assert!(n <= 40, "a still inspector frame allocated {n} times");
}
