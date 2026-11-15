//! Tympanum's hybrid voice end to end (round 40, `drums-hyb`; ADR 0124's third amendment):
//! through `CantusApp::assemble` — the boot path, which installs the kits' loader and the
//! residuals' — and the real engine. A song whose jazz kit is *Hybrid* before its pack is
//! on disk plays the generated kit, to the bit (never silence); the pack, built as the fit
//! tool writes it and served from a file through the content manager (the manifest's own
//! entry, its source swapped for the file), installs the way the kit card's *Get* does;
//! then the hybrid sounds the residual at the pieces' sources, the same bits in blocks of
//! 1, 64, 333 and 512, live as offline; *Generated* (one command, one undo step) is the
//! model alone to the bit, and its undo is the hybrid again. Its own binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder.

use cantus_content::manifest::{FileEntry, Source};
use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::inst::tympanum::voice::{set_voice, Voice, HYBRID, RESIDUAL_FILE};
use cantus_core::inst::tympanum::{KIT_KEY, VOICE_KEY};
use cantus_core::*;
use cantus_dsp::tympanum_residual::{model_hash, Art, Layer, Residual, Take};
use cantus_engine::{render, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::path::Path;

const PACK: &str = "tympanum-hybrid-jazz";

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

/// A Tympanum on the jazz kit, two seconds: kick, snare, hats closing and opening by CC4, a
/// crash and the ride.
fn song(hybrid: bool) -> Project {
    let mut p = Project::new("Drums");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Drums");
    let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
    spec.set_text(KIT_KEY, Some("model:jazz".into())).unwrap();
    if hybrid {
        spec.set_text(VOICE_KEY, Some(HYBRID.into())).unwrap();
    }
    t.instrument = InstrumentSpec::Native(spec);
    let mut c = p.make_clip("Groove", Ticks::ZERO, Ticks::beats(4));
    for step in 0..16i64 {
        let at = Ticks(step * PPQ / 4);
        if step % 8 == 0 || step % 8 == 5 {
            c.notes.push(p.make_note(at, Ticks(PPQ / 4), 36, 104));
        }
        if step % 8 == 4 {
            c.notes.push(p.make_note(at, Ticks(PPQ / 4), 38, 112));
        } else if step % 4 == 3 {
            c.notes.push(p.make_note(at, Ticks(PPQ / 4), 38, 34));
        }
        if step % 2 == 0 {
            c.notes
                .push(p.make_note(at, Ticks(PPQ / 4), 46, 70 + step as u8 * 3));
        }
        if step == 2 || step == 10 {
            c.notes.push(p.make_note(at, Ticks(PPQ / 4), 51, 90));
        }
    }
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks(PPQ / 4), 49, 120));
    let pt = |b: f64, v: f32| automation::AutoPoint::new(Ticks((b * PPQ as f64) as i64), v);
    c.lanes.push(ClipLane::new(
        LaneKind::Cc(4),
        vec![pt(0.0, 0.05), pt(1.99, 0.05), pt(2.0, 0.9), pt(3.5, 0.9)],
    ));
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// The pack's residual as the fit writes it: every key the song plays, two layers of two
/// takes of decaying noise (80–150 ms), the hat in three states.
fn residual() -> Residual {
    let mut data = Vec::new();
    let mut x: u32 = 0x9E37_79B9;
    let mut arts = Vec::new();
    for (key, open) in [
        (36u8, 255u8),
        (38, 255),
        (46, 10),
        (46, 50),
        (46, 95),
        (49, 255),
        (51, 255),
    ] {
        let mut layers = Vec::new();
        for (l, vel) in [40u8, 110].iter().enumerate() {
            let mut takes = Vec::new();
            for k in 0..2u32 {
                let len = 3_840 + 1_440 * k + 480 * u32::from(key % 3);
                let start = data.len() as u32;
                for i in 0..len {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    let f = 1.0 - i as f32 / len as f32;
                    data.push((((x >> 16) as i32 - 32_768) as f32 * f * f) as i16);
                }
                takes.push(Take {
                    start,
                    len,
                    scale: 0.05 * (l + 1) as f32 / 32_768.0,
                });
            }
            layers.push(Layer {
                vel: *vel,
                rho: 0.1,
                takes,
            });
        }
        arts.push(Art { key, open, layers });
    }
    let mut r = Residual {
        rate: 48_000,
        model: model_hash(),
        arts,
        data,
        soft: Vec::new(),
    };
    r.finish();
    r
}

fn boot(p: Project) -> (CantusApp, Processor) {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, proc) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, Some(p), engine, None, None, None, false);
    (app, proc)
}

fn offline(app: &mut CantusApp, p: &Project, frames: usize, block: usize) -> Vec<f32> {
    let snap = app.engine.compile_for_render(p, 1);
    let mut r = render(
        snap,
        &RenderOptions {
            tail_seconds: 1.0,
            block_frames: block,
            ..Default::default()
        },
    )
    .unwrap()
    .samples;
    r.truncate(frames * 2);
    r
}

fn play(app: &mut CantusApp, proc: &mut Processor, frames: usize) -> Vec<f32> {
    app.sync_engine();
    app.seek(Ticks::ZERO);
    app.toggle_play();
    app.sync_engine();
    let mut out = Vec::with_capacity(frames * 2);
    let sizes = [512usize, 256, 333, 1024];
    let mut i = 0;
    while out.len() < frames * 2 {
        let n = sizes[i % sizes.len()].min(frames - out.len() / 2);
        i += 1;
        let mut buf = vec![0f32; n * 2];
        proc.process(&mut buf, 2);
        out.extend_from_slice(&buf);
    }
    app.toggle_play();
    app.sync_engine();
    out
}

fn same(a: &[f32], b: &[f32], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}: lengths");
    if let Some(i) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.to_bits() != y.to_bits())
    {
        panic!("{what} differs at {i}: {} vs {}", a[i], b[i]);
    }
}

fn energy(a: &[f32], b: &[f32]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| f64::from(x - y) * f64::from(x - y))
        .sum()
}

/// The manifest's own entry for the jazz kit's hybrid, its source swapped for the files
/// the fit wrote (served from `file://`).
fn manifest(dir: &Path) -> cantus_content::Manifest {
    let mut m =
        cantus_content::Manifest::parse(include_str!("../../cantus-content/manifest/content.json"))
            .expect("the manifest reads");
    let p = m
        .packs
        .iter_mut()
        .find(|p| p.id == PACK)
        .expect("the manifest offers the jazz kit's hybrid");
    assert_eq!(p.licence, "CC0-1.0", "a credit-free default");
    let mut entries = Vec::new();
    for (name, bytes) in [
        (RESIDUAL_FILE, residual().encode()),
        ("LICENSE.txt", b"CC0 1.0".to_vec()),
    ] {
        let f = dir.join(name);
        std::fs::write(&f, &bytes).unwrap();
        entries.push(FileEntry {
            url: cantus_content::fetch::file_url(&f),
            to: name.into(),
            size: bytes.len() as u64,
            sha256: cantus_content::fetch::sha256_file(&f).unwrap(),
        });
    }
    p.source = Source::Files(entries);
    p.licence_file = "LICENSE.txt".into();
    m
}

#[test]
fn a_hybrid_kit_plays_generated_until_its_pack_is_in_then_its_attacks_and_generated_is_the_model_to_the_bit(
) {
    let frames = 2 * 48_000;
    let (mut app, mut proc) = boot(song(true));
    // Before its pack: the generated kit, to the bit, and never silence.
    let gen = offline(&mut app, &song(false), frames, 512);
    assert!(gen.iter().any(|x| x.abs() > 1e-4), "the kit sounds");
    same(
        &offline(&mut app, &song(true), frames, 512),
        &gen,
        "hybrid without its pack",
    );

    // The pack, downloaded as the kit card's Get does: the content manager, the manifest's
    // entry (its file served locally).
    let src = tempfile::tempdir().unwrap();
    let content = data().join("Content");
    let m = manifest(src.path());
    app.samples.manifest = Some(m.clone());
    app.samples
        .set_content(cantus_content::Manager::new(&content, m));
    app.samples.content().install(PACK);
    let t0 = std::time::Instant::now();
    while !matches!(
        app.samples.content().status(PACK),
        cantus_content::Status::Installed { .. }
    ) {
        // The manager hears its download thread when polled (the app polls each frame).
        for f in app.samples.content().poll() {
            if let Err(e) = &f.result {
                panic!("the install failed: {e:?}");
            }
        }
        assert!(
            t0.elapsed().as_secs() < 60,
            "{:?}",
            app.samples.content().status(PACK)
        );
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(content.join(PACK).join(RESIDUAL_FILE).is_file());

    // Now the hybrid adds the residual at the pieces' sources.
    let hyb = offline(&mut app, &song(true), frames, 512);
    let e = energy(&hyb, &gen);
    assert!(e > 1e-3, "the residual is heard: {e}");
    // The same bits however the blocks are cut.
    for block in [1usize, 64, 333] {
        same(
            &offline(&mut app, &song(true), frames, block),
            &hyb,
            &format!("hybrid in blocks of {block}"),
        );
    }
    // Live as offline (the song the app holds is the hybrid one).
    let live = play(&mut app, &mut proc, frames);
    same(&live, &hyb, "live = offline");

    // Generated, through the command the kit card runs: the model alone, to the bit; its
    // undo is the hybrid again.
    let track = app.project().tracks[0].id;
    let cmd = set_voice(app.project(), track, Voice::Generated).expect("a change");
    assert!(app.exec(cmd));
    assert_eq!(
        Voice::of(app.project().tracks[0].instrument.native().unwrap()),
        Voice::Generated
    );
    let p = app.project().clone();
    same(&offline(&mut app, &p, frames, 512), &gen, "generated");
    app.undo();
    let p = app.project().clone();
    same(
        &offline(&mut app, &p, frames, 512),
        &hyb,
        "undo: hybrid again",
    );
}

/// The published pack itself (r40 integration): the manifest's entry as it ships — the
/// public `content-r40` release — downloaded by the content manager the kit card's *Get*
/// runs (its size and SHA-256 checked by the manager), unpacked, and its residual read for
/// the model this build plays. Network: `cargo test -p cantus-ui --test tympanum_hybrid_e2e
/// -- --ignored the_published`.
#[test]
#[ignore]
fn the_published_hybrid_pack_downloads_matches_its_hash_and_fits_this_model() {
    let m =
        cantus_content::Manifest::parse(include_str!("../../cantus-content/manifest/content.json"))
            .expect("the manifest reads");
    let content = tempfile::tempdir().unwrap();
    let mut mgr = cantus_content::Manager::new(content.path(), m);
    mgr.install(PACK);
    let t0 = std::time::Instant::now();
    while !matches!(mgr.status(PACK), cantus_content::Status::Installed { .. }) {
        for f in mgr.poll() {
            if let Err(e) = &f.result {
                panic!("the download failed: {e:?}");
            }
        }
        assert!(t0.elapsed().as_secs() < 600, "{:?}", mgr.status(PACK));
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let bytes = std::fs::read(content.path().join(PACK).join(RESIDUAL_FILE)).unwrap();
    let r = Residual::decode(&bytes).expect("the residual reads");
    assert_eq!(
        r.model,
        model_hash(),
        "fitted for the model this build plays"
    );
    assert!(!r.arts.is_empty());
}
