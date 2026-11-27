//! r42 arcus-classical (ADR 0166) — the playback's look-ahead reached as a user reaches it:
//! Arcus added through `CantusApp::assemble` and `apply_pick`, a phrase of two notes inserted
//! (the first followed at once by the second), played through the real engine wiring (a
//! processor driven block by block). The processor hands the first note its length and the
//! next note's distance (`InstrumentUnit::note_len`), so its stroke eases before the second
//! comes — a player's note inside a phrase, not an organ's — and the same song renders the
//! same bits in blocks of 512 and of 97.

use cantus_core::{Command, InstrumentKind, Project, Ticks, PPQ};
use cantus_engine::EngineHandle;
use cantus_ui::app::Files;
use cantus_ui::i18n::Locale;
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::CantusApp;

fn app_with_song(data: &std::path::Path) -> (CantusApp, cantus_engine::Processor) {
    std::env::set_var("CANTUS_DATA_DIR", data);
    std::fs::write(
        data.join("settings.json"),
        r#"{"audio_setup":{"setup_done":true}}"#,
    )
    .unwrap();
    let files = Files::read(data, Some(Locale::En));
    let ctx = egui::Context::default();
    let (engine, proc_) = EngineHandle::manual(48_000.0);
    let song = Project::new("arcus-ahead");
    let app = CantusApp::assemble(&ctx, files, Some(song), engine, None, None, None, false);
    (app, proc_)
}

/// `seconds` of the song from its start, mono, through the processor in blocks of `block`.
fn play(
    app: &mut CantusApp,
    proc_: &mut cantus_engine::Processor,
    seconds: usize,
    block: usize,
) -> Vec<f32> {
    app.sync_engine();
    app.seek(Ticks::ZERO);
    app.toggle_play();
    app.sync_engine();
    let frames = 48_000 * seconds;
    let mut out = Vec::with_capacity(frames * 2);
    let since = std::time::Instant::now();
    while out.len() < frames * 2 {
        let mut buf = vec![0f32; block * 2];
        proc_.process(&mut buf, 2);
        out.extend_from_slice(&buf);
        assert!(since.elapsed().as_secs() < 240, "the phrase never finished");
    }
    app.toggle_play();
    app.sync_engine();
    out.chunks_exact(2).map(|f| 0.5 * (f[0] + f[1])).collect()
}

/// The level (dB) of `x` over `from…to` s.
fn level(x: &[f32], from: f64, to: f64) -> f64 {
    let (a, b) = ((from * 48_000.0) as usize, (to * 48_000.0) as usize);
    let e: f64 = x[a..b]
        .iter()
        .map(|v| (*v as f64) * (*v as f64))
        .sum::<f64>()
        / (b - a) as f64;
    10.0 * cantus_dsp::math::log10(e + 1e-30)
}

/// A fresh app with Arcus and the phrase (60 BPM): a D3 held 1.5 s, an E3 at once after it.
fn session(block: usize) -> Vec<f32> {
    let data = tempfile::tempdir().unwrap();
    let (mut app, mut proc_) = app_with_song(data.path());
    let purpose = PickPurpose::AddInstrument { index: None };
    app.open_picker(purpose.clone());
    app.apply_pick(purpose, PickChoice::Instrument(InstrumentKind::BowedString));
    app.picker = None;
    let track = app.project().tracks.last().unwrap().id;
    let mut p = app.project().clone();
    let mut c = p.make_clip("Phrase", Ticks::ZERO, Ticks::beats(4));
    let first = p.make_note(Ticks::ZERO, Ticks(PPQ * 3 / 2), 50, 90);
    let second = p.make_note(Ticks(PPQ * 3 / 2), Ticks(PPQ), 52, 90);
    c.notes.extend([first, second]);
    assert!(app.exec(Command::InsertClip {
        track,
        clip: Box::new(c)
    }));
    assert!(app.exec(Command::SetTempo { bpm: 60.0 }));
    let out = play(&mut app, &mut proc_, 4, block);
    assert!(out.iter().all(|x| x.is_finite()));
    out
}

#[test]
fn a_note_followed_by_the_next_eases_before_it_through_the_app() {
    let out = session(512);
    let peak = (0..14)
        .map(|i| level(&out, 0.1 * i as f64, 0.1 * i as f64 + 0.06))
        .fold(f64::NEG_INFINITY, f64::max);
    let end = level(&out, 1.44, 1.5);
    println!("the D3's last 60 ms: {:.1} dB under its peak", peak - end);
    // Without the look-ahead the stroke's sag alone left it 3.6–5 dB under (the players' 8–10).
    assert!(peak - end > 6.0, "the end only {:.1} dB under", peak - end);
    // The second note sounds.
    assert!(level(&out, 1.7, 2.3) > end, "the next note is not heard");
    // The same bits at another block size: the look-ahead is read from the song, not the
    // blocks.
    let other = session(97);
    let n = out.len().min(other.len());
    assert!(
        out[..n] == other[..n],
        "the look-ahead depends on the block size"
    );
}
