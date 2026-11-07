//! Sectio's card in Exemplar's Play view (round 37, ADR 0099): it draws a sliced loop with
//! its slices and its gestures, in both languages, the gestures' names whole; screenshots
//! in both themes with `CANTUS_KEYS_SHOTS`.

mod support;
use cantus_core::sectio::{SliceMode, Slicing};
use cantus_core::*;
use cantus_ui::views::inst::View;

fn sliced_app(dir: &std::path::Path) -> (cantus_ui::CantusApp, TrackId) {
    // A loop of four struck tones at 120 BPM, 48 kHz.
    let path = dir.join("view-loop.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for i in 0..96_000usize {
        let t = (i % 24_000) as f32 / 48_000.0;
        let v = (-t * 12.0).exp() * (std::f32::consts::TAU * 220.0 * t).sin();
        w.write_sample((v * 20_000.0) as i16).unwrap();
    }
    w.finalize().unwrap();
    let s = Slicing {
        mode: SliceMode::Transients,
        sens: 0.5,
        bpm: 120.0,
        grid: 4,
        len: 96_000,
        rate: 48_000,
        points: vec![0, 24_000, 48_000, 72_000],
    };
    let mut a = support::app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    let spec = cantus_ui::sectio_ui::spec_of(&path.to_string_lossy(), &s, None);
    a.set_instrument(track, InstrumentSpec::Native(spec));
    a.open_inst_editor(track);
    a.inst.view = View::Play;
    (a, track)
}

#[test]
fn the_slices_card_reads_whole_in_both_languages() {
    let dir = tempfile::tempdir().unwrap();
    for (locale, want, count) in [
        (
            cantus_ui::i18n::Locale::En,
            "Dump to piano roll",
            "4 slices",
        ),
        (
            cantus_ui::i18n::Locale::Es,
            "Volcar al piano roll",
            "4 cortes",
        ),
    ] {
        cantus_ui::i18n::set_thread_locale(locale);
        let (mut a, _) = sliced_app(dir.path());
        let c = support::ctx();
        let text = support::paint(&c, egui::vec2(1040.0, 680.0), 3, |ui| {
            cantus_ui::views::inst::window_in(&mut a, ui);
        });
        // The presses read in capitals, each whole (an elided one would end in "…").
        let up = text.to_uppercase();
        let whole = format!("| {} |", want.to_uppercase());
        assert!(up.contains(&whole), "{locale:?}: {text}");
        assert!(text.contains(count), "{locale:?}: the count: {text}");
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
}

#[test]
fn screenshots_for_the_design_review() {
    let Some(out) = std::env::var_os("CANTUS_KEYS_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    for (tag, theme) in [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ] {
        let (mut a, _) = sliced_app(dir.path());
        // The engine's whole file for the waveform: the card asks, the sample cache reads.
        for _ in 0..50 {
            a.sync_engine();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let c = support::ctx();
        cantus_ui::theme::apply(&theme, &c);
        let path = out.join(format!("sectio-play-{tag}.png"));
        // One shot of a few frames (a second shot on the same context would find its font
        // atlas already uploaded and paint the text as blocks).
        cantus_ui::raster::shoot(&c, egui::vec2(1040.0, 680.0), 1.0, 8, Some(&path), |ui| {
            cantus_ui::views::inst::window_in(&mut a, ui);
        });
    }
    cantus_ui::theme::apply(&cantus_ui::theme::default_theme(), &support::ctx());
}
