//! The light visualiser in a video's frames (round 35, ADR 0091 §4): the export's own render
//! analysed, each frame a pure function of its time, drawn through the video's overlay hook
//! (`cantus_video::dance::SongFrames::overlay`) under its title line.

use cantus_core::tempo::TempoMap;
use cantus_core::TimeSignature;
use cantus_dance::render::{Background, Stage};
use cantus_ui::stage::vis::{band_hz, BANDS, FALL_PER_S, RATE};
use cantus_ui::stage::vis_video::{analyse, with_title, Setup, VideoVis};
use cantus_video::dance::{Dancer, SongFrames};
use cantus_video::export::FrameSource;
use tiny_skia::Pixmap;

const SR: u32 = 48_000;

/// `secs` seconds of stereo: a tone at `hz` for the first `on` seconds, then silence.
fn tone(hz: f32, on: f64, secs: f64) -> Vec<f32> {
    let n = (secs * f64::from(SR)) as usize;
    (0..n)
        .flat_map(|i| {
            let t = i as f64 / f64::from(SR);
            let v = if t < on {
                0.5 * (2.0 * std::f64::consts::PI * f64::from(hz) * t).sin() as f32
            } else {
                0.0
            };
            [v, v]
        })
        .collect()
}

fn setup(title_on: bool) -> Setup {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    Setup::from_theme("Morning Light", title_on, false)
}

fn vis(samples: &[f32]) -> VideoVis {
    let tempo = TempoMap::constant(120.0, TimeSignature::default());
    VideoVis::new(setup(true), analyse(samples, 2, SR), tempo, 0.0)
}

fn nearest(hz: f32) -> usize {
    (0..BANDS)
        .min_by(|a, b| {
            (band_hz(*a) - hz)
                .abs()
                .total_cmp(&(band_hz(*b) - hz).abs())
        })
        .unwrap()
}

#[test]
fn the_render_is_analysed_and_a_tone_lights_its_band() {
    let v = vis(&tone(1000.0, 2.0, 3.0));
    let b = v.bands_at(1.0);
    let at = nearest(1000.0);
    assert!(b[at] > 0.7, "the 1 kHz band: {b:?}");
    assert!(b[0] < 0.1 && b[BANDS - 1] < 0.1, "{b:?}");
}

#[test]
fn a_band_falls_at_the_meters_pace_after_the_sound_stops() {
    let v = vis(&tone(1000.0, 1.0, 3.0));
    let at = nearest(1000.0);
    let top = v.bands_at(1.0)[at];
    let later = v.bands_at(1.0 + 0.25)[at];
    assert!(later < top, "it falls ({top} → {later})");
    assert!(
        later >= top - FALL_PER_S * 0.25 - 1.0 / RATE as f32 * FALL_PER_S - 0.02,
        "no faster than the meter ({top} → {later})"
    );
    assert!(v.bands_at(2.5)[at] < 0.05, "and rests");
}

#[test]
fn a_frame_is_a_pure_function_of_its_time() {
    let v = vis(&tone(220.0, 3.0, 3.0));
    let draw = |t: f64| {
        let mut pm = Pixmap::new(540, 960).unwrap();
        v.draw(t, &mut pm);
        pm
    };
    let a = draw(1.234);
    let _ = draw(2.5);
    let b = draw(1.234);
    assert_eq!(a.data(), b.data(), "the same frame twice, the same pixels");
    assert_ne!(a.data(), draw(1.7).data(), "another moment, another frame");
    // The corners are left alone (a transparent video stays transparent there).
    assert_eq!(a.pixel(0, 0).unwrap().alpha(), 0);
    assert_eq!(a.pixel(539, 959).unwrap().alpha(), 0);
}

#[test]
fn the_song_frames_draw_it_through_their_overlay_hook() {
    let samples = tone(440.0, 3.0, 3.0);
    let tempo = TempoMap::constant(120.0, TimeSignature::default());
    let v = VideoVis::new(setup(false), analyse(&samples, 2, SR), tempo, 0.0);
    let frames = |overlay| SongFrames {
        dancers: vec![Dancer {
            character: cantus_dance::cast::default_character(),
            timeline: Vec::new(),
        }],
        stage: Stage::new(720, 720, Background::Transparent),
        start_seconds: 0.0,
        fps: 30,
        overlay,
    };
    let plain = frames(None);
    let with = frames(with_title(Some(v), None));
    let mut a = Pixmap::new(720, 720).unwrap();
    let mut b = Pixmap::new(720, 720).unwrap();
    plain.draw(30, &mut a);
    with.draw(30, &mut b);
    let changed = a
        .pixels()
        .iter()
        .zip(b.pixels())
        .filter(|(x, y)| x != y)
        .count();
    assert!(
        changed > 5_000,
        "the visualiser is in the frame ({changed} pixels)"
    );
    let mut c = Pixmap::new(720, 720).unwrap();
    with.draw(30, &mut c);
    assert_eq!(b.data(), c.data(), "deterministic through the hook");
}

#[test]
fn the_dialogs_choice_is_on_by_default_and_kept() {
    let d = cantus_ui::video_export::VideoSettings::default();
    assert!(d.visualiser);
    let old: cantus_ui::video_export::VideoSettings = serde_json::from_str("{}").unwrap();
    assert!(old.visualiser, "a settings file from before it reads it on");
    let off = cantus_ui::video_export::VideoSettings {
        visualiser: false,
        ..d
    };
    let back: cantus_ui::video_export::VideoSettings =
        serde_json::from_str(&serde_json::to_string(&off).unwrap()).unwrap();
    assert!(!back.visualiser);
}

// ---- The dialog's preview (review: its visualiser was always empty) ----

mod preview {
    use super::*;
    use cantus_core::{Project, Snapshot, Ticks};
    use cantus_engine::{EngineHandle, RenderOptions, RenderProgress};
    use cantus_ui::app::CantusApp;
    use cantus_ui::video_export as vx;

    fn app(p: Project, folder: &std::path::Path) -> CantusApp {
        cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
        let mut a = CantusApp::from_parts(Some(p), EngineHandle::dummy(), false);
        a.settings_path = folder.join("settings.json");
        a.path = Some(folder.join("song.cantus"));
        a
    }

    /// The preview at `scrub` seconds of the video, once the moment's own render is in.
    fn previewed(a: &mut CantusApp, scrub: f64) -> (vx::VideoSettings, VideoVis) {
        vx::open(a);
        let d = a.video.dialog.clone().expect("the dialog");
        assert!(d.visualiser, "on by default");
        a.video.scrub = scrub;
        let t0 = std::time::Instant::now();
        loop {
            match vx::preview_timelines(a, &d) {
                vx::PreviewState::Ready(_) => break,
                vx::PreviewState::Failed(e) => panic!("{e}"),
                vx::PreviewState::Rendering => {}
            }
            assert!(t0.elapsed().as_secs() < 60, "rendered in time");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let v = cantus_ui::stage::vis_video::preview_vis(a, &d).expect("the visualiser");
        (d, v)
    }

    /// The export's own visualiser: the range rendered from its start, analysed.
    fn exported(a: &CantusApp, d: &vx::VideoSettings, to_seconds: f64) -> VideoVis {
        let p = a.project();
        let (from, _) = vx::range(a, d.range).expect("a range");
        let map = p.tempo_map();
        let end = Ticks(map.tick_at_seconds(to_seconds).ceil() as i64);
        let snapshot = std::sync::Arc::new(Snapshot::compile(p, a.bus.revision()));
        let opts = RenderOptions {
            sample_rate: SR,
            channels: 2,
            start: from,
            end: Some(end),
            tail_seconds: 0.0,
            compensate_latency: true,
            ..Default::default()
        };
        let r = cantus_engine::render_with_progress(
            snapshot,
            &opts,
            Vec::new(),
            Vec::new(),
            &RenderProgress::default(),
        )
        .expect("rendered");
        let audio = cantus_export::Audio {
            rate: r.sample_rate,
            channels: r.channels,
            samples: r.samples,
        };
        let setup = cantus_ui::stage::vis_video::setup_for(p, d, d.title).expect("on");
        cantus_ui::stage::vis_video::for_render(Some(setup), &audio, &map, from).expect("on")
    }

    /// At a moment the song sounds, the preview's bars are drawn — from the moment's own
    /// render, on the export's instants — and are the export's.
    #[test]
    fn the_preview_draws_the_files_own_spectrum_where_the_song_sounds() {
        let folder = tempfile::tempdir().unwrap();
        let mut a = app(cantus_ui::welcome_project(), folder.path());
        let scrub = 6.3; // bar 4 of the welcome song
        let (d, v) = previewed(&mut a, scrub);
        let shown = v.bands_at(scrub);
        let lit = shown.iter().filter(|b| **b > 0.2).count();
        assert!(lit >= 6, "bars drawn where the song sounds: {shown:?}");
        let export = exported(&a, &d, scrub + 0.2);
        // The frame's visualiser as the file draws it, pixel for pixel.
        let mut x = Pixmap::new(1080, 1920).unwrap();
        let mut y = Pixmap::new(1080, 1920).unwrap();
        v.draw(scrub, &mut x);
        export.draw(scrub, &mut y);
        assert!(x.data() == y.data(), "the file's own frame");
        let file = export.bands_at(scrub);
        let worst = shown
            .iter()
            .zip(&file)
            .map(|(p, f)| (p - f).abs())
            .fold(0.0f32, f32::max);
        assert!(
            worst <= 1e-6,
            "{worst}: the export's bands: {file:?}\nthe preview's: {shown:?}"
        );
    }

    /// At a moment nothing sounds, the box is the empty one (as before the render is in).
    #[test]
    fn a_silent_moment_draws_the_empty_box() {
        let folder = tempfile::tempdir().unwrap();
        let mut p = cantus_ui::welcome_project();
        for t in &mut p.tracks {
            for c in &mut t.clips {
                c.muted = true;
            }
        }
        let mut a = app(p, folder.path());
        let (d, v) = previewed(&mut a, 6.3);
        assert!(v.bands_at(6.3).iter().all(|b| *b == 0.0));
        // Drawn, it is the box before any render: its baseline alone.
        let p = a.project();
        let (from, _) = vx::range(&a, d.range).expect("a range");
        let map = p.tempo_map();
        let start = map.seconds_at(from.0 as f64);
        let setup = cantus_ui::stage::vis_video::setup_for(p, &d, d.title).expect("on");
        let empty = VideoVis::new(setup, Vec::new(), map, start);
        let mut x = Pixmap::new(360, 640).unwrap();
        let mut y = Pixmap::new(360, 640).unwrap();
        v.draw(6.3, &mut x);
        empty.draw(6.3, &mut y);
        assert!(x.data() == y.data(), "the empty box");
    }
}
