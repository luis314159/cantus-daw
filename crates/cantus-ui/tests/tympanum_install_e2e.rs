//! fix-e (round 40, review of 40a27bb7, finding 5): a song whose Tympanum names a kit this
//! machine does not have plays silence — and once the kit is installed it **sounds**, in
//! the same session, without the user choosing it again. Through `CantusApp::assemble`
//! (the boot path, which installs the kit loader), *Install from a file…* (the kits list's
//! own path, a user pack) and the real engine, offline and live. Its own binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::inst::tympanum::KIT_KEY;
use cantus_core::*;
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::path::{Path, PathBuf};

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        std::fs::create_dir_all(d.path().join("Content")).unwrap();
        d
    })
    .path()
}

/// A 16-bit mono WAV of a decaying tone.
fn wav(path: &Path, hz: f32, decay: f32, frames: usize) {
    let mut b: Vec<u8> = Vec::new();
    let data = (frames * 2) as u32;
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&48_000u32.to_le_bytes());
    b.extend_from_slice(&96_000u32.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for t in 0..frames {
        let x = (t as f32 / 48_000.0 * hz * std::f32::consts::TAU).sin()
            * (-(t as f32) / (decay * 48_000.0)).exp()
            * 0.6;
        b.extend_from_slice(&((x * 32_000.0) as i16).to_le_bytes());
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b).unwrap();
}

/// A small kit as a library ships one, not installed: two mics, a kick and a snare.
fn write_kit(dir: &Path) -> PathBuf {
    let mut sfz = String::from("<global>\nloop_mode=one_shot\n");
    for (mi, mic) in ["kick_mic", "oh_mic"].iter().enumerate() {
        sfz.push_str("<master>\n");
        for (key, s, hz) in [(36, "kick", 60.0f32), (38, "snare", 210.0)] {
            let f = format!("Samples/{mic}/{s}.wav");
            wav(&dir.join(&f), hz * (1.0 + 0.2 * mi as f32), 0.2, 24_000);
            sfz.push_str(&format!("<region> key={key} sample=../{f}\n"));
        }
    }
    let progs = dir.join("Programs");
    std::fs::create_dir_all(&progs).unwrap();
    let path = progs.join("kit.sfz");
    std::fs::write(&path, sfz).unwrap();
    path
}

/// A Tympanum on `kit` playing a bar of kick and snare.
fn song(kit: &str) -> Project {
    let mut p = Project::new("Drums");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Drums");
    let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
    spec.set_text(KIT_KEY, Some(kit.to_string())).unwrap();
    t.instrument = InstrumentSpec::Native(spec);
    let mut c = p.make_clip("Groove", Ticks::ZERO, Ticks::beats(4));
    for beat in 0..4i64 {
        let key = if beat % 2 == 0 { 36 } else { 38 };
        c.notes
            .push(p.make_note(Ticks(beat * PPQ), Ticks(PPQ / 4), key, 110));
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn boot(p: Project) -> (CantusApp, Processor, egui::Context) {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, proc) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, Some(p), engine, None, None, None, false);
    (app, proc, ctx)
}

fn offline(app: &mut CantusApp) -> f32 {
    let p = app.project().clone();
    let snap = app.engine.compile_for_render(&p, 1);
    let r = render(
        snap,
        &RenderOptions {
            tail_seconds: 0.5,
            block_frames: 512,
            ..Default::default()
        },
    )
    .unwrap()
    .samples;
    r.iter().fold(0.0f32, |m, x| m.max(x.abs()))
}

/// Play from the start until something sounds or `seconds` of audio went by (the kit's
/// files load in the background: the snapshot follows each one).
fn live(app: &mut CantusApp, proc: &mut Processor, seconds: f32) -> f32 {
    app.sync_engine();
    app.seek(Ticks::ZERO);
    app.toggle_play();
    app.sync_engine();
    let mut peak = 0.0f32;
    let mut done = 0usize;
    let started = std::time::Instant::now();
    while done < (seconds * 48_000.0) as usize && peak == 0.0 {
        let mut buf = vec![0f32; 512 * 2];
        proc.process(&mut buf, 2);
        peak = buf.iter().fold(peak, |m, x| m.max(x.abs()));
        done += 512;
        app.sync_engine();
        if done % (48_000 * 2) < 512 {
            // The bar again, the loaded files in.
            app.seek(Ticks::ZERO);
            app.sync_engine();
        }
        if started.elapsed().as_secs() > 60 {
            break;
        }
    }
    app.toggle_play();
    app.sync_engine();
    peak
}

#[test]
fn a_kit_the_machine_lacks_sounds_once_it_is_installed_without_choosing_it_again() {
    let downloads = data().join("downloads").join("Late Kit");
    let sfz = write_kit(&downloads);
    // What *Install from a file…* will name it: the user pack of that file, its kit.
    let pack = cantus_content::user_pack::user_id(&sfz);
    let text = format!("pack:{pack}/kit");
    let (mut app, mut proc, ctx) = boot(song(&text));
    let track = app.project().tracks[0].id;
    // Not installed: silence, and the engine says why.
    assert_eq!(offline(&mut app), 0.0, "a kit not installed plays nothing");
    assert!(
        cantus_engine::inst::tympanum::cache::refused(&text).is_some(),
        "the kit was asked for and refused"
    );
    app.poll_samples(&ctx);
    // Installed from the file: the song already names it, so nothing in it changes.
    cantus_ui::views::inst::tympanum::kits::install_kit_path(&mut app, track, &sfz);
    // r40 integration — fix-f builds a user kit in the background: its install lands
    // when that job is done.
    app.wait_user_installs(std::time::Duration::from_secs(120));
    assert_eq!(
        cantus_engine::inst::tympanum::cache::kit_text(
            app.project().tracks[0].instrument.native().unwrap()
        ),
        text
    );
    // The next frame sees the packs changed: the refusal is forgotten.
    app.poll_samples(&ctx);
    assert!(cantus_engine::inst::tympanum::cache::refused(&text).is_none());
    assert!(offline(&mut app) > 0.05, "the installed kit plays offline");
    assert!(
        live(&mut app, &mut proc, 8.0) > 0.05,
        "and live, in the same session"
    );
}
