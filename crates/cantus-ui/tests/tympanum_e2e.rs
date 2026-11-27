//! Tympanum end to end (round 40 phase 3, ADR 0124, ADR 0123): through
//! `CantusApp::assemble` — the boot path, which installs the kit loader — and the real
//! engine, a recorded multi-mic kit installed as a pack plays a groove into its mics; each
//! mic sounds on its own; *Each mic to its own strip* (one undo step) makes a strip per mic
//! and the split kit plays the unsplit kit **to the bit**, offline at 512 and 333 frames
//! and live (offline = real time); the modelled studio kit does the same. And Gogodze Phu
//! Vol. II's published program files (`tests/fixtures/sfz/gogodze_phu_vol_ii`, CC0, byte
//! for byte) read into the kit the engine plays: seven mics by their folders, the hi-hat's
//! four levels on CC4, every strike heard by every mic. Its own binary: it points
//! `CANTUS_DATA_DIR` at a temporary folder.

use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::inst::tympanum::{KitRef, KIT_KEY};
use cantus_core::inst_out::{self, MicRule, OutputPlan};
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
        write_test_kit(&d.path().join("Content").join("test-kit"));
        d
    })
    .path()
}

/// A 16-bit mono WAV of `frames` frames of a decaying tone.
fn wav(path: &Path, hz: f32, decay: f32, level: f32, frames: usize) {
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
            * level;
        b.extend_from_slice(&((x * 32_000.0) as i16).to_le_bytes());
    }
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b).unwrap();
}

/// A small kit as a library ships one: one folder per mic, a kick and a snare with two
/// round robins, a hi-hat closed and open on CC4, and its receipt as an install writes it.
fn write_test_kit(dir: &Path) {
    let mics = [("kick_mic", 1.0f32), ("snare_mic", 0.6), ("oh_mic", 0.4)];
    let sounds = [
        ("kick", 60.0f32, 0.25),
        ("snare", 210.0, 0.15),
        ("hc", 3_500.0, 0.05),
        ("ho", 3_100.0, 0.4),
    ];
    let mut sfz =
        String::from("<control>\nset_cc4=0\n<global>\nloop_mode=one_shot\nseq_length=2\n");
    for (mi, (mic, level)) in mics.iter().enumerate() {
        sfz.push_str("<master>\n");
        for (si, (s, hz, decay)) in sounds.iter().enumerate() {
            let (key, cc) = match si {
                0 => (36, ""),
                1 => (38, ""),
                2 => (46, " hicc4=63 group=1 off_by=2"),
                _ => (46, " locc4=64 group=2 off_by=1"),
            };
            for rr in 1..=2 {
                let f = format!("Samples/{mic}/{s}_rr{rr}.wav");
                wav(
                    &dir.join(&f),
                    hz * (1.0 + 0.13 * mi as f32) * (1.0 + 0.01 * rr as f32),
                    *decay,
                    *level,
                    (decay * 4.0 * 48_000.0) as usize,
                );
                sfz.push_str(&format!(
                    "<region> key={key}{cc} seq_position={rr} sample=../{f}\n"
                ));
            }
        }
    }
    let progs = dir.join("Programs");
    std::fs::create_dir_all(&progs).unwrap();
    std::fs::write(progs.join("kit.sfz"), sfz).unwrap();
    std::fs::write(dir.join("LICENSE"), "CC0 1.0 Universal (a test kit)").unwrap();
    let receipt = serde_json::json!({
        "id": "test-kit", "version": 1,
        "name": {"en": "Test kit", "es": "Batería de prueba"},
        "licence": "CC0-1.0",
        "licence_url": "https://creativecommons.org/publicdomain/zero/1.0/legalcode",
        "licence_file": "LICENSE", "credits": [],
        "instruments": [{"id": "kit", "name": {"en": "Test kit", "es": "Batería de prueba"},
            "file": "Programs/kit.sfz", "shelf": "drums", "gain_db": 0.0, "plays": "tympanum"}],
        "installed_at": 0, "files": 0, "bytes": 0
    });
    std::fs::write(dir.join("cantus-pack.json"), receipt.to_string()).unwrap();
}

/// A Tympanum on `kit` on its own strip, playing two bars: kick, snare, hats closing and
/// opening by CC4.
fn song(kit: &str) -> Project {
    let mut p = Project::new("Drums");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Drums");
    let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
    spec.set_text(KIT_KEY, Some(kit.to_string())).unwrap();
    t.instrument = InstrumentSpec::Native(spec);
    let mut c = p.make_clip("Groove", Ticks::ZERO, Ticks::beats(8));
    for step in 0..32i64 {
        let at = Ticks(step * PPQ / 4);
        if step % 8 == 0 || step % 8 == 5 {
            c.notes
                .push(p.make_note(at, Ticks(PPQ / 4), 36, 100 + (step % 3) as u8 * 9));
        }
        if step % 8 == 4 {
            c.notes.push(p.make_note(at, Ticks(PPQ / 4), 38, 110));
        } else if step % 4 == 3 {
            c.notes.push(p.make_note(at, Ticks(PPQ / 4), 38, 30));
        }
        if step % 2 == 0 {
            c.notes
                .push(p.make_note(at, Ticks(PPQ / 4), 46, 80 + (step % 4) as u8 * 8));
        }
    }
    // CC4: closed for a bar, then open, closing on the last beat.
    let pt = |b: f64, v: f32| automation::AutoPoint::new(Ticks((b * PPQ as f64) as i64), v);
    c.lanes.push(ClipLane::new(
        LaneKind::Cc(4),
        vec![
            pt(0.0, 0.05),
            pt(3.99, 0.05),
            pt(4.0, 0.9),
            pt(6.99, 0.9),
            pt(7.0, 0.0),
        ],
    ));
    t.clips.push(c);
    p.tracks.push(t);
    p
}

fn boot(p: Project) -> (CantusApp, Processor) {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, proc) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, Some(p), engine, None, None, None, false);
    (app, proc)
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

fn peak(a: &[f32]) -> f32 {
    a.iter().fold(0.0f32, |m, x| m.max(x.abs()))
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

/// The kit `kit` split *Each mic to its own strip* through the app, then heard every way.
fn split_plays_unsplit(kit: &str, mics: usize) {
    let frames = 4 * 48_000;
    let (mut app, mut proc) = boot(song(kit));
    let t = app.project().tracks[0].id;
    let opened = app.project().clone();
    let unsplit = offline(&mut app, &opened, frames, 512);
    assert!(peak(&unsplit) > 0.01, "{kit} sounds: {}", peak(&unsplit));
    same(
        &offline(&mut app, &opened, frames, 333),
        &unsplit,
        "unsplit at 333 frames",
    );
    // Each mic heard alone.
    for m in 0..mics {
        let mut p = opened.clone();
        if let Some(n) = p.tracks[0].instrument.native_mut() {
            n.set(&format!("o{m}.solo"), 1.0);
        }
        assert!(
            peak(&offline(&mut app, &p, frames, 512)) > 1e-4,
            "{kit}: mic {m} sounds"
        );
    }
    // The app's action: one undo step, a strip per mic, into the kit's strip, in order.
    let project = app.project().clone();
    let cmd = inst_out::plan(&project, t, OutputPlan::EachOwn, &mut |n| {
        app.bus.new_strip(n)
    })
    .expect("a plan");
    assert!(app.exec(cmd));
    assert_eq!(app.bus.undo_label_key(), Some("cmd.outputs_each"));
    let outs = app.project().tracks[0].instrument.output_strips();
    assert_eq!(outs.len(), mics, "a strip per mic");
    let kit_strip = app.project().tracks[0].strip;
    assert!(outs
        .iter()
        .all(|s| app.project().strip(*s).unwrap().output == kit_strip));
    let split = app.project().clone();
    same(
        &offline(&mut app, &split, frames, 512),
        &unsplit,
        "split offline at 512",
    );
    same(
        &offline(&mut app, &split, frames, 333),
        &unsplit,
        "split offline at 333",
    );
    let live = play(&mut app, &mut proc, frames);
    same(&live, &unsplit, "split live");
    // *All to Main* brings the kit back in one step, its strips gone.
    let project = app.project().clone();
    let cmd = inst_out::plan(&project, t, OutputPlan::AllToMain, &mut |n| {
        app.bus.new_strip(n)
    })
    .expect("back to Main");
    assert!(app.exec(cmd));
    assert!(app.project().tracks[0]
        .instrument
        .output_strips()
        .is_empty());
    assert_eq!(app.project().strips.len(), opened.strips.len());
}

#[test]
fn a_recorded_kit_plays_into_its_mic_strips_and_the_split_sums_to_the_unsplit_to_the_bit() {
    split_plays_unsplit("pack:test-kit/kit", 3);
}

#[test]
fn the_modelled_studio_kit_plays_into_its_mic_strips_and_the_split_sums_to_the_unsplit() {
    split_plays_unsplit("model:studio", 12);
}

fn gogodze() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sfz/gogodze_phu_vol_ii/Programs/Kit.sfz")
}

#[test]
fn gogodze_phu_vol_ii_reads_from_its_published_files_into_seven_mics_and_a_continuous_hat() {
    let kit = KitRef::Pack {
        pack: "gogodze-phu-vol-ii".into(),
        kit: "kit".into(),
    };
    let k = cantus_ui::tympanum_load::read_sfz(&gogodze(), kit, "Gogodze Phu Vol. II").unwrap();
    assert!(
        k.log.iter().any(|l| l.contains("sample folders")),
        "{:?}",
        k.log
    );
    let names: Vec<&str> = k.def.mics.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        names,
        ["Front", "Kick", "OH", "Retro", "Side", "Snare", "Window"]
    );
    use cantus_core::inst_out::OutputFamily as F;
    let fams: Vec<F> = k.def.mics.iter().map(|m| m.family).collect();
    assert_eq!(
        fams,
        [
            F::Rooms,
            F::Kick,
            F::Overheads,
            F::Rooms,
            F::Overheads,
            F::Snare,
            F::Rooms
        ]
    );
    // Kick, snare, hi-hat and three toms.
    assert_eq!(
        k.def.pieces.len(),
        6,
        "{:?}",
        k.def.pieces.iter().map(|p| &p.name).collect::<Vec<_>>()
    );
    // Key 46's four levels on CC4, and 42 and 44 as hats too.
    let mut open: Vec<f32> = k.hits.on_key(46).filter_map(|(_, a)| a.openness).collect();
    open.sort_by(f32::total_cmp);
    assert_eq!(open.len(), 4);
    assert!(open[0] < 0.2 && open[3] > 0.8, "{open:?}");
    // Every strike of every articulation heard by all seven mics (full bleed), from four
    // round robins on the drums and three on the open hat.
    for a in &k.hits.arts {
        for l in &a.layers {
            for t in &l.takes {
                let mut mics: Vec<u8> = t.mics.iter().map(|m| m.mic).collect();
                mics.sort();
                mics.dedup();
                assert_eq!(mics.len(), 7, "key {} layer {}-{}", a.key, l.lo, l.hi);
            }
        }
    }
    let kick = k.hits.on_key(36).next().unwrap().1;
    assert!(kick.layers.iter().all(|l| l.takes.len() == 4));
    // An open hat is silenced by a closed one: SFZ's off_by, read.
    let (ho, _) = k
        .hits
        .on_key(46)
        .max_by(|a, b| a.1.openness.partial_cmp(&b.1.openness).unwrap())
        .unwrap();
    let closed = k.hits.on_key(42).next().unwrap().1;
    assert!(closed.chokes.contains(&(ho as u16)), "{:?}", closed.chokes);
    // The library's tuning knob at its default (63 of 127 over ±12 st): a little flat.
    let cents = kick.layers[0].takes[0].mics[0].cents;
    assert!(
        (cents - (-1200.0 + 2400.0 * 63.0 / 127.0)).abs() < 0.01,
        "{cents}"
    );
    assert_eq!(cantus_core::inst_out::MicRule::Folders, MicRule::Folders);
}

/// Big Rusty Drums' published program files (`tests/fixtures/sfz/big_rusty_drums`, CC0,
/// byte for byte, those `01-full.sfz` includes): its faders are per piece and per mic
/// ("Kick OH", "Snare OH", 32 of them), so its microphones are its sample folders — a few,
/// not thirty.
#[test]
fn big_rusty_drums_reads_its_mics_from_its_folders_not_its_per_piece_faders() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sfz/big_rusty_drums/Programs/01-full.sfz");
    let kit = KitRef::Pack {
        pack: "big-rusty-drums".into(),
        kit: "kit".into(),
    };
    let k = cantus_ui::tympanum_load::read_sfz(&path, kit, "Big Rusty Drums").unwrap();
    let names: Vec<&str> = k.def.mics.iter().map(|m| m.name.as_str()).collect();
    assert!((2..=12).contains(&names.len()), "{names:?} {:?}", k.log);
    assert!(
        k.log.iter().any(|l| l.contains("sample folders")),
        "{:?}",
        k.log
    );
    assert!(k.def.pieces.len() >= 8, "{}", k.def.pieces.len());
}

/// r40:fix-f (review of 40a27bb7, finding 4): a user's DrumGizmo kit — DRSKit's published
/// kit files (`cantus-content/tests/fixtures/drskit_minimal`, CC BY 4.0, byte for byte),
/// a short 13-channel hit written for every file they name — installed with *Install from
/// a file…* through the app is built for Tympanum and plays a groove on every one of its
/// eleven mics (it used to install and never play: its reader was a stub).
#[test]
fn a_users_drumgizmo_kit_installs_from_a_file_and_plays_on_its_mics() {
    let dir = tempfile::tempdir().unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cantus-content/tests/fixtures/drskit_minimal");
    let to = dir.path().join("DRSKit");
    // The destination first: ext4 lists a folder in a hash's order, so the kit's XML can
    // come before any sub-folder that would have made it (beta.15's release job, 22.04).
    std::fs::create_dir_all(&to).unwrap();
    let mut stack = vec![src.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            let rel = p.strip_prefix(&src).unwrap().to_path_buf();
            if p.is_dir() {
                std::fs::create_dir_all(to.join(&rel)).unwrap();
                stack.push(p);
            } else {
                std::fs::copy(&p, to.join(&rel)).unwrap();
            }
        }
    }
    let kit = cantus_content::drumgizmo::read_kit(&to, "DRSKit_minimal.xml").unwrap();
    for f in cantus_content::drumgizmo::files(&kit) {
        let path = cantus_content::drumgizmo::path_of(&to, &f);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let spec = hound::WavSpec {
            channels: 13,
            sample_rate: 48_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        for i in 0..2_400 {
            for c in 0..13 {
                let x = ((i as f32) * (0.02 + 0.01 * c as f32)).sin()
                    * (1.0 - i as f32 / 2_400.0)
                    * 0.3;
                w.write_sample((x * 32_000.0) as i16).unwrap();
            }
        }
        w.finalize().unwrap();
    }
    let frames = 2 * 48_000;
    let (mut app, _proc) = boot(song("model:studio"));
    let t = app.project().tracks[0].id;
    cantus_ui::views::inst::tympanum::kits::install_kit_path(
        &mut app,
        t,
        &to.join("DRSKit_minimal.xml"),
    );
    app.wait_user_installs(std::time::Duration::from_secs(120));
    let n = app.native_of(t).unwrap();
    let KitRef::Pack { pack, .. } = KitRef::of(&n) else {
        panic!("the user's kit plays: {:?}", n.text(KIT_KEY));
    };
    assert!(pack.starts_with("user-"), "{pack}");
    let p = app.project().clone();
    let all = offline(&mut app, &p, frames, 512);
    assert!(peak(&all) > 0.01, "the kit sounds: {}", peak(&all));
    let mics = inst_out::definition(&p, t);
    assert_eq!(mics.len(), 11, "13 channels, two pairs: {mics:?}");
    for m in &mics {
        let mut solo = p.clone();
        if let Some(n) = solo.tracks[0].instrument.native_mut() {
            n.set(&format!("o{}.solo", m.id.0), 1.0);
        }
        assert!(
            peak(&offline(&mut app, &solo, frames, 512)) > 1e-4,
            "mic {} sounds",
            m.name
        );
    }
}

/// The modelled kit's crash, then its hat closed, open and on the foot (one bar at 120).
fn cymbal_song() -> Project {
    let mut p = Project::new("Cymbals");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Drums");
    let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
    spec.set_text(KIT_KEY, Some("model:studio".to_string()))
        .unwrap();
    t.instrument = InstrumentSpec::Native(spec);
    let mut c = p.make_clip("Cymbals", Ticks::ZERO, Ticks::beats(12));
    for (beat, key, vel) in [
        (0, 49, 110u8),
        (4, 42, 100),
        (5, 46, 100),
        (6, 44, 90),
        (7, 42, 60),
        (8, 51, 100),
        (10, 53, 100),
    ] {
        c.notes
            .push(p.make_note(Ticks(beat * PPQ), Ticks(PPQ / 4), key, vel));
    }
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// Spectral flatness of `x` between `lo` and `hi` Hz (a Hann window, bins every 4 Hz).
fn flatness(x: &[f32], lo: f32, hi: f32) -> f32 {
    let n = x.len();
    let w: Vec<f32> = (0..n)
        .map(|i| {
            let s = (std::f32::consts::PI * i as f32 / n as f32).sin();
            x[i] * s * s
        })
        .collect();
    let mut p = Vec::new();
    let mut f = lo;
    while f < hi {
        let k = std::f32::consts::TAU * f / 48_000.0;
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for (i, v) in w.iter().enumerate() {
            let a = k * i as f32;
            re += v * a.cos();
            im += v * a.sin();
        }
        p.push((re * re + im * im).max(1e-30));
        f += 4.0;
    }
    let geo = (p.iter().map(|v| v.ln()).sum::<f32>() / p.len() as f32).exp();
    geo / (p.iter().sum::<f32>() / p.len() as f32)
}

#[test]
fn the_modelled_kits_crash_and_hats_are_measured_noise_over_partials_not_glass() {
    // Model 5 (round 40): the crash and the hats synthesised from tables measured off
    // DRSKit — through the boot path and the real engine. Model 4's crash measured a 4–12
    // kHz flatness of 0.002 (a few pure partials, analysis-drums/report.md), DRSKit's
    // 0.12–0.24; the live and offline renders agree whatever the block.
    let p = cymbal_song();
    let (mut app, mut proc) = boot(p.clone());
    let frames = 48_000 * 6;
    let live = play(&mut app, &mut proc, frames);
    assert!(peak(&live) > 1e-3, "the cymbals sound");
    let a = offline(&mut app, &p, frames, 512);
    let b = offline(&mut app, &p, frames, 333);
    same(&a, &b, "offline at 512 and 333");
    same(&live, &a, "live and offline");
    let mono: Vec<f32> = a.chunks(2).map(|s| 0.5 * (s[0] + s[1])).collect();
    let crash = &mono[(0.15 * 48_000.0) as usize..(0.4 * 48_000.0) as usize];
    let f = flatness(crash, 4_000.0, 12_000.0);
    assert!(
        f > 0.03,
        "the crash's wash is dense, not a few partials: flatness {f}"
    );
    // The closed hat is all but noise.
    let hat = &mono[(2.01 * 48_000.0) as usize..(2.13 * 48_000.0) as usize];
    let f = flatness(hat, 4_000.0, 12_000.0);
    assert!(f > 0.15, "the closed hat: flatness {f}");
    // The ride's bow and bell sound (model 5 too).
    let at = |s: f32| (s * 96_000.0) as usize;
    assert!(peak(&a[at(4.0)..at(5.0)]) > 1e-3, "the ride");
    assert!(peak(&a[at(5.0)..at(6.0)]) > 1e-3, "the bell");
    // The crash's tail between the kit's two sides: never out of phase (the recordings'
    // overheads +0.1 … +0.4; the spaced pair's comb gave −0.3).
    let (mut lr, mut ll, mut rr) = (0.0f64, 0.0f64, 0.0f64);
    for s in a[at(0.3)..at(1.8)].chunks(2) {
        let (l, r) = (s[0] as f64, s[1] as f64);
        lr += l * r;
        ll += l * l;
        rr += r * r;
    }
    let corr = lr / (ll * rr).sqrt().max(1e-30);
    assert!(corr > 0.1, "the crash's tail L/R correlation {corr}");
}

#[test]
fn the_jazz_kit_plays_its_own_measured_voices() {
    // Round 40: the studio kit's pieces on tables measured off Virtuosity Drums (CC0) —
    // through the boot path, it sounds, live = offline, and it is not the studio kit.
    let jazz = |kit: &str| {
        let mut p = cymbal_song();
        if let InstrumentSpec::Native(n) = &mut p.tracks[0].instrument {
            n.set_text(KIT_KEY, Some(kit.to_string())).unwrap();
        }
        p
    };
    let frames = 48_000 * 6;
    let p = jazz("model:jazz");
    let (mut app, mut proc) = boot(p.clone());
    let live = play(&mut app, &mut proc, frames);
    assert!(peak(&live) > 1e-3, "the jazz kit sounds");
    let a = offline(&mut app, &p, frames, 512);
    same(&live, &a, "live and offline");
    let s = offline(&mut app, &jazz("model:studio"), frames, 512);
    assert!(a != s, "its own voices, not the studio kit's");
}

/// The modelled kits' level law (r42:tympanum-real, `DSP_VERSION` 19), through the boot path
/// and the real engine: every piece of the studio and jazz kits grows with the velocity at
/// every step a drummer plays, for every seed — the drums a second apart, each struck while its
/// last hit rings: model 6 (a new Tympanum's `model` 2) never lets one hit's own draw cancel its
/// level (model 5's blend did: the studio kick at 96 3.3 dB under its 80 for one seed). The
/// hat 4 s apart and the cymbals 12 s: a cymbal struck while it rings blooms as a harder hit by
/// design (ADR 0124's second amendment) — a ride rings for 7–10 s, and its 112 a few seconds after
/// its 96 already plays as its 127.
#[test]
fn every_modelled_piece_grows_with_the_velocity_through_the_app() {
    const VELS: [u8; 9] = [16, 32, 48, 56, 64, 80, 96, 112, 127];
    for kit in ["model:studio", "model:jazz"] {
        for key in [36u8, 38, 45, 41, 42, 49, 51] {
            let beats = match key {
                49 | 51 => 12,
                42 => 4,
                _ => 1,
            };
            for seed in [0.0f32, 1.0, 2.0] {
                let mut p = Project::new("Law");
                p.tempo_bpm = 60.0;
                let mut t = p.make_track("Drums");
                let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
                spec.set_text(KIT_KEY, Some(kit.to_string())).unwrap();
                spec.set("seed", seed);
                t.instrument = InstrumentSpec::Native(spec);
                // a drum's hit every second (a beat at 60 BPM): each struck while it rings
                let n = VELS.len() as i64;
                let mut c = p.make_clip("Hits", Ticks::ZERO, Ticks::beats(beats * n + 1));
                for (i, v) in VELS.iter().enumerate() {
                    let at = Ticks::beats(beats * i as i64);
                    c.notes.push(p.make_note(at, Ticks(PPQ / 8), key, *v));
                }
                t.clips.push(c);
                p.tracks.push(t);
                let (mut app, _proc) = boot(p.clone());
                let step = beats as usize * 96_000;
                let x = offline(
                    &mut app,
                    &p,
                    (beats as usize * VELS.len() + 1) * 48_000,
                    512,
                );
                let lv: Vec<f32> = (0..VELS.len())
                    .map(|i| {
                        let w = &x[i * step..i * step + 2 * 3_840];
                        let e = w.iter().map(|s| s * s).sum::<f32>() / w.len() as f32;
                        10.0 * cantus_dsp::math::log10(e.max(1e-20))
                    })
                    .collect();
                for (j, w) in lv.windows(2).enumerate() {
                    assert!(
                        w[1] > w[0] - 1.0,
                        "{kit} key {key} seed {seed}: velocity {} → {} falls ({lv:?})",
                        VELS[j],
                        VELS[j + 1]
                    );
                }
            }
        }
    }
}

/// Model 6 through the app (r42:tympanum-real, ADR 0124's seventh amendment; schema 86): a
/// new Tympanum stores `model` 2 and its drums play model 6, so its snare and kick are not
/// what a song made before (`model` 1, model 5) plays; its crash plays the same bits under
/// either model. (Model 6's onset is held at the voice, over every drum and seed:
/// cantus-dsp `a_model_6_drum_comes_in_through_the_contact`.)
#[test]
fn a_new_tympanum_plays_the_drums_onset_and_an_old_song_keeps_its_own() {
    let render_with = |model: Option<f32>, key: u8| {
        let mut p = Project::new("Onset");
        p.tempo_bpm = 60.0;
        let mut t = p.make_track("Drums");
        let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
        assert_eq!(spec.get("model"), 2.0, "a new instance stores model 2");
        spec.set_text(KIT_KEY, Some("model:studio".to_string()))
            .unwrap();
        if let Some(m) = model {
            spec.set("model", m);
        }
        t.instrument = InstrumentSpec::Native(spec);
        let mut c = p.make_clip("Hit", Ticks::ZERO, Ticks::beats(2));
        c.notes
            .push(p.make_note(Ticks::ZERO, Ticks(PPQ / 8), key, 110));
        t.clips.push(c);
        p.tracks.push(t);
        let (mut app, _proc) = boot(p.clone());
        offline(&mut app, &p, 48_000, 512)
    };
    for key in [38u8, 36] {
        let (old, new) = (render_with(Some(1.0), key), render_with(None, key));
        assert!(peak(&new) > 1e-3 && peak(&old) > 1e-3);
        assert!(old != new, "key {key}: model 2 plays model 6, not model 5");
    }
    same(
        &render_with(Some(1.0), 49),
        &render_with(None, 49),
        "the crash",
    );
}
