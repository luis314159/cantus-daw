//! Waveforms on the clip browser's audio tiles (round 26): drawn from the peaks the
//! arrangement already reads (`PeakStore`, decoded off the UI thread), cached per clip and
//! revision, so a still frame of the pane allocates no more than one without audio.

mod support;

use cantus_core::*;
use cantus_ui::views::clip_browser;
use cantus_ui::CantusApp;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use support::{app, ctx};

struct Counting;
thread_local! {
    static ON: Cell<bool> = const { Cell::new(false) };
    static COUNT: Cell<u64> = const { Cell::new(0) };
}
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        if ON.with(|o| o.get()) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        if ON.with(|o| o.get()) {
            COUNT.with(|c| c.set(c.get() + 1));
        }
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Counting = Counting;

const PATH: &str = "C:/nowhere/cantus-browser-take.wav";

/// One track, a two-second take (loud first second, quiet second) as an audio clip — or a
/// note clip in its place (`audio: false`) — and three note clips.
fn scene(audio: bool) -> (CantusApp, (TrackId, ClipId)) {
    let mut p = Project::new("waves");
    let mut t = p.make_track("Vocal");
    let src = AudioSource {
        id: AudioSourceId(p.alloc_id()),
        name: "take".into(),
        path: PATH.into(),
        sample_rate: 48_000,
        channels: 1,
        frames: 96_000,
        sha256: None,
        recorded: false,
    };
    let mut c = p.make_clip("Take", Ticks::ZERO, Ticks::beats(4));
    if audio {
        c.audio = Some(AudioRegion::of(src.id));
    } else {
        let n = p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 90);
        c.notes.push(n);
    }
    let r = (t.id, c.id);
    t.clips.push(c);
    for i in 1..4 {
        let mut n = p.make_clip(format!("Pattern {i}"), Ticks::beats(4 * i), Ticks::beats(4));
        let note = p.make_note(Ticks::ZERO, Ticks::QUARTER, 60, 90);
        n.notes.push(note);
        t.clips.push(n);
    }
    p.audio_pool.push(src);
    p.tracks.push(t);
    let a = app(p);
    // The samples, as an import hands them over: reduced on the store's worker.
    let frames: Vec<f32> = (0..96_000)
        .map(|i| {
            // A square wave: full swings, no transcendental needed.
            let x = if (i / 40) % 2 == 0 { 1.0 } else { -1.0 };
            if i < 48_000 {
                0.9 * x
            } else {
                0.1 * x
            }
        })
        .collect();
    a.peaks.insert_data(
        std::path::Path::new(PATH),
        Arc::new(SampleData {
            stream: None,
            rate: 48_000,
            channels: 1,
            frames: frames.into(),
        }),
    );
    (a, r)
}

fn pane(ctx: &egui::Context, a: &mut CantusApp, count: bool) -> u64 {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(220.0, 600.0),
        )),
        time: Some(ctx.input(|i| i.time) + 1.0 / 60.0),
        ..Default::default()
    };
    let mut n = 0;
    let mut out = ctx.run_ui(input, |ui| {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                COUNT.with(|c| c.set(0));
                ON.with(|o| o.set(count));
                clip_browser::show(a, ui);
                ON.with(|o| o.set(false));
                n = COUNT.with(|c| c.get());
            });
    });
    out.textures_delta.clear();
    n
}

/// Frames until the take's waveform is cached (the store decodes on its own thread).
fn until_ready(ctx: &egui::Context, a: &mut CantusApp, r: (TrackId, ClipId)) {
    let t0 = std::time::Instant::now();
    while a.clip_browser.wave_of(r).is_none() {
        assert!(t0.elapsed().as_secs() < 10, "the peaks never arrived");
        pane(ctx, a, false);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
}

#[test]
fn an_audio_tile_draws_its_waveform_from_the_peaks() {
    let (mut a, r) = scene(true);
    let ctx = ctx();
    until_ready(&ctx, &mut a, r);
    let wave = a.clip_browser.wave_of(r).unwrap().to_vec();
    assert!(wave.len() >= 16, "{} columns", wave.len());
    // The loud half, then the quiet half (the clip shows both seconds at 120 bpm).
    let height = |p: &(f32, f32)| p.1 - p.0;
    let half = wave.len() / 2;
    let loud = wave[..half - 1].iter().map(height).fold(0.0f32, f32::max);
    let quiet = wave[half + 1..].iter().map(height).fold(0.0f32, f32::max);
    assert!(loud > 1.5 && quiet < 0.3, "loud {loud}, quiet {quiet}");
    // A change to the clip (its gain) is a new revision: the columns are made again.
    let before = wave.clone();
    let mut region = a
        .project()
        .track(r.0)
        .unwrap()
        .clip(r.1)
        .unwrap()
        .audio
        .clone()
        .unwrap();
    region.reverse = true;
    a.exec(Command::SetClipAudio {
        track: r.0,
        clip: r.1,
        audio: Some(region),
    });
    pane(&ctx, &mut a, false);
    let after = a.clip_browser.wave_of(r).unwrap().to_vec();
    assert_eq!(
        after.iter().rev().copied().collect::<Vec<_>>(),
        before,
        "reversed"
    );
}

#[test]
fn a_still_frame_with_a_waveform_allocates_no_more_than_without() {
    let still = |audio: bool| {
        let (mut a, r) = scene(audio);
        let ctx = ctx();
        if audio {
            until_ready(&ctx, &mut a, r);
        }
        for _ in 0..30 {
            pane(&ctx, &mut a, false);
        }
        pane(&ctx, &mut a, true)
    };
    let without = still(false);
    let with = still(true);
    eprintln!("clip browser, still: {without} allocations without audio, {with} with a waveform");
    assert!(with <= without, "{with} vs {without}");
}
