//! Sectio through the gestures a user makes (round 37, the integrated tree's review): A/B and
//! a saved preset keep a hand-edited styled loop; a style's own mode cuts a new loop; points
//! added or removed leave each slice's settings on its own audio and a style's choke alone;
//! *Apply* keeps them; long loops fit 64 slices; a grid without a tempo borrows the song's;
//! edited points stay on the dump's samples; a loop past 2²⁴ frames is refused; the
//! browser's audition of a style plays what *Load* gives.

use cantus_core::inst::sampler::{zf, Zone};
use cantus_core::sectio::{SliceMode, Slicing};
use cantus_core::*;
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};

const RATE: u32 = 48_000;

/// This binary's own folder, its `CANTUS_DATA_DIR` too (the real app never touches the
/// user's).
fn data() -> PathBuf {
    let d = std::env::temp_dir().join(format!("cantus-sectio-r3-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::env::set_var("CANTUS_DATA_DIR", &d);
    d
}

/// `beats` beats at 120 BPM: a struck tone on every beat and a softer one between
/// (`tone`: a held tone instead, no attacks).
fn wav(name: &str, beats: usize, tone: bool) -> PathBuf {
    let p = data().join(name);
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&p, spec).unwrap();
    let half = RATE as usize / 4;
    for i in 0..beats * half * 2 {
        let v = if tone {
            0.3 * (std::f32::consts::TAU * 220.0 * i as f32 / RATE as f32).sin()
        } else {
            let t = (i % half) as f32 / RATE as f32;
            let g = if (i / half).is_multiple_of(2) {
                0.8
            } else {
                0.4
            };
            g * (-t * 40.0).exp() * (std::f32::consts::TAU * 200.0 * t).sin()
        };
        w.write_sample((v * 30_000.0) as i16).unwrap();
    }
    w.finalize().unwrap();
    p
}

fn app() -> CantusApp {
    let files = Files::read(&data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, _p) = cantus_engine::EngineHandle::manual(RATE as f64);
    let mut a = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    if a.project().tempo_bpm != 120.0 {
        assert!(a.exec(Command::SetTempo { bpm: 120.0 }));
    }
    a
}

fn sliced(a: &mut CantusApp, p: &Path) -> TrackId {
    assert!(cantus_ui::views::samples_browser::slice_from_menu(
        a, p, false
    ));
    a.project().tracks.last().unwrap().id
}

fn style(key: &str) -> NativeSpec {
    let p = cantus_ui::sectio_ui::styles::sampler_presets()
        .iter()
        .find(|p| p.label_key == key)
        .expect("a style");
    cantus_ui::views::inst::preset_spec(InstrumentKind::Sampler, p)
}

fn spec(a: &CantusApp, t: TrackId) -> NativeSpec {
    a.native_of(t).unwrap()
}

fn points(a: &CantusApp, t: TrackId) -> Vec<u32> {
    Slicing::of(&spec(a, t)).unwrap().points
}

fn zone(a: &CantusApp, t: TrackId, n: usize) -> Zone {
    Zone::of(&spec(a, t), n).unwrap()
}

fn set(a: &mut CantusApp, t: TrackId, key: &str, value: f32) {
    assert!(a.exec(Command::SetInstrumentValue {
        track: t,
        key: key.into(),
        value
    }));
}

#[test]
fn ab_and_a_saved_preset_keep_a_hand_edited_styled_loop() {
    let mut a = app();
    let t = sliced(&mut a, &wav("ab.wav", 8, false));
    a.apply_inst_spec(t, &style("slicer.style.break"), "cmd.fx_preset");
    let p0 = points(&a, t);
    assert!(p0.len() > 2);
    assert!(a.add_slice(t, p0[1] + RATE / 20));
    set(&mut a, t, "z2.rev", 1.0);
    let edited = spec(&a, t);
    // A/B once (the current spec applied to itself), and back.
    a.toggle_inst_ab(t);
    assert_eq!(spec(&a, t), edited, "A/B on a fresh pair changes nothing");
    a.toggle_inst_ab(t);
    assert_eq!(spec(&a, t), edited);
    // A saved preset of it, loaded back: its points and every slice's settings.
    a.apply_inst_spec(t, &edited, "cmd.fx_preset");
    assert_eq!(spec(&a, t), edited);
}

#[test]
fn a_new_loop_on_a_styled_exemplar_is_cut_the_styles_way() {
    let mut a = app();
    let t = sliced(&mut a, &wav("first.wav", 4, false));
    a.apply_inst_spec(t, &style("slicer.style.break"), "cmd.fx_preset");
    assert!(a.add_slice(t, RATE / 3)); // a hand edit: the mode reads Manual now
    let second = wav("second.wav", 8, false);
    assert!(a.slice_into_track(t, &second, SliceMode::Markers));
    assert!(
        points(&a, t).len() > 8,
        "transients, the style's: {:?}",
        points(&a, t)
    );
}

#[test]
fn points_added_or_removed_keep_each_slices_settings_on_its_audio_and_its_choke() {
    let mut a = app();
    let t = sliced(&mut a, &wav("carry.wav", 8, false));
    a.apply_inst_spec(t, &style("slicer.style.break"), "cmd.fx_preset");
    let p0 = points(&a, t);
    assert!(p0.len() >= 6);
    set(&mut a, t, "z5.rev", 1.0);
    set(&mut a, t, "z3.excl", 0.0); // the user frees slice 3 from the choke
    let start5 = p0[5];
    assert!(a.add_slice(t, p0[2] + RATE / 20));
    let p1 = points(&a, t);
    let n5 = p1.iter().position(|p| *p == start5).unwrap();
    assert_eq!(
        zone(&a, t, n5).get(zf::REV),
        1.0,
        "the reverse stays on its audio"
    );
    assert_eq!(zone(&a, t, n5 - 1).get(zf::REV), 0.0);
    let n3 = p1.iter().position(|p| *p == p0[3]).unwrap();
    assert_eq!(
        zone(&a, t, n3).get(zf::EXCL),
        0.0,
        "the style does not re-choke it"
    );
    assert!(a.remove_slice(t, 1));
    let p2 = points(&a, t);
    let m5 = p2.iter().position(|p| *p == start5).unwrap();
    assert_eq!(zone(&a, t, m5).get(zf::REV), 1.0);
    // Slice again on a grid: the slice that starts inside the reversed one is reversed.
    assert!(a.reslice(t, SliceMode::Grid, 0.5, 4));
    let s = Slicing::of(&spec(&a, t)).unwrap();
    let inside = s
        .points
        .iter()
        .position(|p| *p >= start5 && *p < p2[m5 + 1])
        .unwrap();
    assert_eq!(zone(&a, t, inside).get(zf::REV), 1.0, "Apply keeps it");
}

#[test]
fn a_long_loop_fits_sixty_four_slices_spread_over_it() {
    let mut a = app();
    let t = sliced(&mut a, &wav("long.wav", 16, false)); // 4 bars
    for key in ["slicer.style.stutter", "slicer.style.break"] {
        a.apply_inst_spec(t, &style(key), "cmd.fx_preset");
        let s = Slicing::of(&spec(&a, t)).unwrap();
        assert!(s.points.len() <= 64, "{key}");
        let spans: Vec<u32> = (0..s.points.len())
            .map(|i| s.span(i).1 - s.span(i).0)
            .collect();
        let (lo, hi) = (spans.iter().min().unwrap(), spans.iter().max().unwrap());
        assert!(*hi <= 4 * lo.max(&1), "{key}: spans {lo}…{hi}");
    }
}

#[test]
fn a_grid_style_on_a_loop_without_a_tempo_borrows_the_songs() {
    let mut a = app();
    let t = sliced(&mut a, &wav("held.wav", 8, true));
    a.apply_inst_spec(t, &style("slicer.style.vocal"), "cmd.fx_preset");
    // 4 s at the song's 120 BPM, a slice a beat.
    assert_eq!(points(&a, t).len(), 8, "{:?}", points(&a, t));
}

#[test]
fn edited_points_stay_on_the_samples_the_dump_plays() {
    let mut a = app();
    let t = sliced(&mut a, &wav("tick.wav", 8, false));
    let p0 = points(&a, t);
    assert!(a.move_slice(t, 2, p0[2] + 337));
    assert!(a.add_slice(t, p0[3] + 1_111));
    let s = Slicing::of(&spec(&a, t)).unwrap();
    assert!(s.bpm > 0.0);
    for p in &s.points {
        let tick = s.tick_of(*p);
        let at = cantus_engine::transport::sample_at(0.0, tick as f64, s.bpm as f64, RATE as f64);
        assert_eq!(*p, at as u32, "a point off the dump's samples");
    }
}

#[test]
fn a_loop_past_two_to_the_twenty_four_frames_is_refused() {
    let p = data().join("huge.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE,
        bits_per_sample: 8,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&p, spec).unwrap();
    for _ in 0..(cantus_ui::sectio_ui::LONGEST as usize + 1) {
        w.write_sample(0i8).unwrap();
    }
    w.finalize().unwrap();
    let e =
        cantus_ui::sectio_ui::analyse_full(&p, SliceMode::Transients, 0.5, 4, 120.0).unwrap_err();
    assert!(e.contains("Too long"), "{e}");
    let _ = std::fs::remove_file(&p);
}

#[test]
fn a_styles_audition_plays_what_load_gives() {
    let mut a = app();
    let t = sliced(&mut a, &wav("aud.wav", 8, false));
    let entry = cantus_ui::preset_catalog::entries(
        cantus_ui::preset_catalog::Unit::Instrument(InstrumentKind::Sampler),
        false,
    )
    .into_iter()
    .find(|e| e.id == "slicer.style.break")
    .expect("the style in the library");
    let prepared = cantus_ui::preset_audition::prepare(
        a.project(),
        cantus_ui::preset_audition::Target::Track(t),
        &entry,
        None,
        120.0,
    );
    let heard = cantus_ui::preset_audition::restyled(&prepared).expect("restyled");
    a.apply_inst_spec(t, &style("slicer.style.break"), "cmd.fx_preset");
    assert_eq!(heard, spec(&a, t), "the audition's instrument is Load's");
}

/// A point moved left keeps every slice's own settings on its own slice (review 4 #1).
#[test]
fn a_point_moved_left_keeps_each_slices_own_settings() {
    let mut a = app();
    let t = sliced(&mut a, &wav("left.wav", 8, false));
    let p0 = points(&a, t);
    set(&mut a, t, "z2.tune", -30.0);
    set(&mut a, t, "z3.tune", 50.0);
    set(&mut a, t, "z3.rev", 1.0);
    assert!(a.move_slice(t, 3, p0[3] - 2_000));
    assert!(points(&a, t)[3] < p0[3]);
    assert_eq!(zone(&a, t, 3).get(zf::TUNE), 50.0, "its own pitch");
    assert_eq!(zone(&a, t, 3).get(zf::REV), 1.0, "its own reverse");
    assert_eq!(
        zone(&a, t, 2).get(zf::TUNE),
        -30.0,
        "the slice before keeps its own"
    );
    assert!(a.move_slice(t, 3, p0[3] + 2_000));
    assert_eq!(zone(&a, t, 3).get(zf::TUNE), 50.0);
}

/// *Slice again*'s preview is what *Apply* makes, on a loop without a tempo (review 4 #2).
#[test]
fn the_preview_is_what_apply_makes_without_a_tempo() {
    let mut a = app();
    let t = sliced(&mut a, &wav("held2.wav", 8, true));
    let song = a.project().tempo_bpm;
    let shown = cantus_ui::sectio_ui::styles::preview(&spec(&a, t), SliceMode::Grid, 0.5, 4, song)
        .expect("a preview");
    assert!(a.reslice(t, SliceMode::Grid, 0.5, 4));
    assert_eq!(shown.points, points(&a, t));
    assert!(shown.points.len() > 1);
}
