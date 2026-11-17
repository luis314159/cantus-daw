//! Vinculum end to end (fx side round, wave B, ADR 0142 provisional), on the boot path: an
//! app made by `CantusApp::assemble` with the real engine handle; a band (a chord and a
//! line) on two tracks routed into a bus strip, and a bass line on a third track straight to
//! the master (the key). Vinculum is inserted on the bus as the picker inserts it — on its
//! starting preset, Glue, no latency:
//!
//! - **the reduction heard on the bus**: with the threshold under the band, the bus comes
//!   out quieter than the dry band, and Glue as inserted reduces its peaks;
//! - **an external key from another strip drives it**: with the threshold over the band,
//!   the band passes untouched on its own key; the bass strip routed as the slot's key
//!   presses it down while the bass plays;
//! - **the high-pass changes the key**: the side chain's high-pass at 500 Hz takes the
//!   bass's low end out of the key, and the band is pressed less;
//! - an edit heard and undone to the bit; live in odd blocks equal to the offline render;
//!   and at its defaults, the dry master to the bit (the key still routed).
//!
//! The band alone is the master less the bass alone (the master sums linearly).
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder. // fxb:vinculum

use cantus_core::*;
use cantus_engine::{render, EffectUnit, EngineHandle, Processor, RenderOptions};
use cantus_ui::app::Files;
use cantus_ui::CantusApp;
use std::path::Path;
use std::sync::Arc;

const SR: usize = 48_000;

fn data() -> &'static Path {
    static DIR: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        let d = tempfile::tempdir().unwrap();
        std::env::set_var("CANTUS_DATA_DIR", d.path());
        d
    })
    .path()
}

fn assembled() -> (CantusApp, Processor) {
    let files = Files::read(data(), Some(cantus_ui::i18n::Locale::En));
    let ctx = egui::Context::default();
    let (engine, proc_) = EngineHandle::manual(48_000.0);
    let app = CantusApp::assemble(&ctx, files, None, engine, None, None, None, false);
    (app, proc_)
}

/// Eight beats at 120 BPM (four seconds), interleaved stereo.
fn rendered(app: &mut CantusApp) -> Vec<f32> {
    let p = app.project().clone();
    let snap = app.engine.compile_for_render(&p, 1);
    let opts = RenderOptions {
        end: Some(Ticks::beats(8)),
        tail_seconds: 0.0,
        ..Default::default()
    };
    render(snap, &opts).unwrap().samples
}

/// Mean square of both channels from `a` to `b` seconds.
fn power(x: &[f32], a: f32, b: f32) -> f64 {
    let (i, j) = (
        (a * SR as f32) as usize * 2,
        ((b * SR as f32) as usize * 2).min(x.len()),
    );
    x[i..j]
        .iter()
        .map(|v| f64::from(*v) * f64::from(*v))
        .sum::<f64>()
        / (j - i) as f64
}

fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0f32, |m, v| m.max(v.abs()))
}

fn db(p: f64) -> f64 {
    10.0 * p.max(1e-30).log10()
}

fn peak_db(x: &[f32]) -> f32 {
    20.0 * peak(x).max(1e-9).log10()
}

fn minus(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b).map(|(x, y)| x - y).collect()
}

/// A track playing `notes` (pitch, start beat, beats, velocity).
fn line(p: &mut Project, name: &str, notes: &[(u8, i64, i64, u8)]) -> Track {
    let mut track = p.make_track(name);
    let mut clip = p.make_clip(name, Ticks::ZERO, Ticks::beats(8));
    for &(pitch, at, len, vel) in notes {
        let n = p.make_note(Ticks::beats(at), Ticks::beats(len), pitch, vel);
        clip.notes.push(n);
    }
    track.clips.push(clip);
    track
}

#[test]
fn vinculum_presses_a_bus_is_keyed_by_another_strip_and_undoes_on_the_boot_path() {
    let (mut app, mut proc_) = assembled();
    let mut p = Project::new("Vinculum");
    p.tempo_bpm = 120.0;
    // The band: a chord held through the eight beats and a line over it.
    let chord = line(
        &mut p,
        "Chord",
        &[(57, 0, 8, 90), (60, 0, 8, 90), (64, 0, 8, 90)],
    );
    p.tracks.push(chord);
    let tune = line(
        &mut p,
        "Line",
        &[(69, 0, 2, 100), (72, 2, 2, 100), (76, 4, 2, 100), (74, 6, 2, 100)],
    );
    p.tracks.push(tune);
    // The key: a loud bass from beat 2 to 6 (one to three seconds).
    let bass = line(&mut p, "Bass", &[(33, 2, 2, 127), (36, 4, 2, 127)]);
    p.tracks.push(bass);
    app.replace_project(p);
    // The band's two strips into a bus strip; the bass straight to the master.
    let bus = {
        let mut p = app.project().clone();
        let s = p.make_strip("Bus");
        let id = s.id;
        p.strips.push(s);
        for t in 0..2 {
            let sid = p.tracks[t].strip.expect("a strip");
            p.strips.iter_mut().find(|s| s.id == sid).unwrap().output = Some(id);
        }
        app.replace_project(p);
        id
    };
    let key_strip = app.project().tracks[2].strip.expect("a strip");
    let band_tracks = [app.project().tracks[0].id, app.project().tracks[1].id];
    let mute = |app: &mut CantusApp, id: TrackId, muted: bool| {
        app.exec(Command::SetTrackMute { id, muted });
    };
    let dry = rendered(&mut app);
    // The bass alone and the band alone, dry.
    for t in band_tracks {
        mute(&mut app, t, true);
    }
    let bass_alone = rendered(&mut app);
    for t in band_tracks {
        mute(&mut app, t, false);
    }
    let band_dry = minus(&dry, &bass_alone);
    let (band_peak, bass_peak) = (peak_db(&band_dry), peak_db(&bass_alone));
    eprintln!("dry: the band peaks {band_peak:.1} dBFS, the bass {bass_peak:.1} dBFS");
    assert!(power(&band_dry, 0.0, 4.0) > 1e-6, "the band sounds");
    assert!(power(&bass_alone, 1.0, 3.0) > 1e-6, "the bass sounds");

    // Inserted as the picker inserts it: Glue, no latency.
    app.add_effect(bus, EffectKind::BusCompressor);
    let effect = app.project().strip(bus).unwrap().effects[0].id;
    let spec = app.project().effect(bus, effect).unwrap().spec.clone();
    assert_eq!(
        (spec.get("model"), spec.get("thr"), spec.get("rel")),
        (0.0, -18.0, 4.0),
        "Glue: the VCA, -18 dB, Auto"
    );
    let slot = app.project().strip(bus).unwrap().effects[0].clone();
    assert_eq!(EffectUnit::new(&slot, 48_000.0).latency(), 0);
    let set = |app: &mut CantusApp, key: &str, value: f32| {
        app.exec(Command::SetEffectParam {
            strip: bus,
            effect,
            key: key.into(),
            value,
        })
    };
    let band_of = |app: &mut CantusApp| minus(&rendered(app), &bass_alone);

    // The reduction heard on the bus: Glue as inserted brings the peaks closer to the body
    // (its make-up and output level the loudness back); with the threshold 20 dB under the
    // band's peak at 10:1 and no make-up, the bus comes out far quieter.
    let glue = band_of(&mut app);
    let crest = |x: &[f32]| f64::from(peak_db(x)) - db(power(x, 0.0, 4.0));
    eprintln!(
        "Glue: crest {:.2} dB against {:.2} dry",
        crest(&glue),
        crest(&band_dry)
    );
    assert!(glue != band_dry, "Glue sounds on the bus");
    set(&mut app, "makeup", 0.0);
    set(&mut app, "out", 0.0);
    set(&mut app, "ratio", 2.0);
    set(&mut app, "thr", band_peak - 20.0);
    let pressed = band_of(&mut app);
    let gr = db(power(&band_dry, 0.0, 4.0)) - db(power(&pressed, 0.0, 4.0));
    eprintln!("the bus pressed: {gr:.1} dB quieter");
    assert!(gr > 6.0, "the reduction is heard on the bus: {gr:.1} dB");

    let play = |proc_: &mut Processor, frames: usize, out: &mut Vec<f32>| {
        let sizes = [256usize, 333, 512, 17, 1024];
        let mut i = 0;
        let mut done = 0;
        while done < frames {
            let n = sizes[i % sizes.len()].min(frames - done);
            i += 1;
            let mut buf = vec![0f32; n * 2];
            proc_.process(&mut buf, 2);
            out.extend_from_slice(&buf);
            done += n;
        }
    };
    // What plays is the offline render: the engine the app drives, played from the top in
    // blocks that do not divide anything.
    app.seek(Ticks::ZERO);
    app.sync_engine();
    proc_.settle();
    app.toggle_play();
    let mut live = Vec::new();
    play(&mut proc_, 2 * SR, &mut live);
    app.toggle_play();
    let offline = {
        let p = app.project().clone();
        let r = render(
            Arc::new(Snapshot::compile(&p, 1)),
            &RenderOptions {
                end: Some(Ticks::beats(8)),
                tail_seconds: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        r.samples[..live.len()].to_vec()
    };
    assert!(power(&live, 0.0, 1.0) > 1e-6, "it plays live");
    assert!(
        live.iter()
            .zip(&offline)
            .all(|(a, b)| a.to_bits() == b.to_bits()),
        "live equals the render"
    );

    // An external key from another strip: the threshold 6 dB over the band's peak, so on its
    // own key the band passes untouched (a gain of exactly one is skipped); the bass routed
    // as the slot's key presses it while the bass plays (1 … 3 s) and not before.
    set(&mut app, "thr", band_peak + 6.0);
    let own = band_of(&mut app);
    assert_eq!(own, band_dry, "on its own key, over the band: untouched");
    assert!(
        bass_peak > band_peak + 9.0,
        "the key is louder than the threshold: bass {bass_peak:.1}, band {band_peak:.1}"
    );
    app.exec(Command::SetEffectSidechain {
        strip: bus,
        effect,
        source: Some(key_strip),
    });
    assert_eq!(
        app.project().strip(bus).unwrap().effects[0].sidechain,
        Some(key_strip),
        "the key is routed"
    );
    let keyed = band_of(&mut app);
    let under_key = db(power(&band_dry, 1.1, 2.9)) - db(power(&keyed, 1.1, 2.9));
    let before = db(power(&band_dry, 0.0, 0.95)) - db(power(&keyed, 0.0, 0.95));
    eprintln!("keyed by the bass: {under_key:.1} dB under it, {before:.2} dB before it");
    assert!(under_key > 3.0, "the key presses the band: {under_key:.1} dB");
    assert!(before.abs() < 0.01, "before the key plays: {before:.2} dB");

    // The high-pass changes the key: at 500 Hz the bass's low end leaves the key, and the
    // band is pressed less.
    set(&mut app, "hpf", 500.0);
    let filtered = band_of(&mut app);
    let under_hpf = db(power(&band_dry, 1.1, 2.9)) - db(power(&filtered, 1.1, 2.9));
    eprintln!("keyed through the 500 Hz high-pass: {under_hpf:.1} dB");
    assert!(
        under_hpf < under_key - 1.0,
        "the high-pass takes the key's lows: {under_hpf:.1} against {under_key:.1} dB"
    );
    set(&mut app, "hpf", 20.0);

    // An edit heard, undone to the bit.
    let base = rendered(&mut app);
    set(&mut app, "ratio", 0.0);
    let moved = rendered(&mut app);
    assert!(
        moved.iter().zip(&base).any(|(a, b)| (a - b).abs() > 1e-3),
        "2:1 for 10:1 is heard"
    );
    app.undo();
    assert_eq!(rendered(&mut app), base, "undone, to the bit");

    // Its neutral defaults: the dry master, to the bit (the key still routed).
    let defaults: Vec<Command> = EffectKind::BusCompressor
        .params()
        .iter()
        .map(|i| Command::SetEffectParam {
            strip: bus,
            effect,
            key: i.key.into(),
            value: i.default,
        })
        .collect();
    app.exec(Command::Batch {
        label_key: "cmd.fx_preset".into(),
        commands: defaults,
    });
    assert_eq!(rendered(&mut app), dry, "the null, to the bit");
}

/// The starting preset is the file's Glue, named in the editor; every factory preset names
/// real keys, is in both languages, and every model is among them with a limit on each.
#[test]
fn the_presets_name_real_keys_in_both_languages_and_every_model() {
    use cantus_ui::views::fx::frame::{read_presets, recognise};
    let file: Vec<_> = cantus_ui::preset_data::presets_of(
        cantus_ui::preset_data::FileUnit::Effect(EffectKind::BusCompressor),
    )
    .collect();
    assert!(file.len() >= 12, "twelve presets or more: {}", file.len());
    let mut seen = [[false; 2]; 3];
    for p in &file {
        let mut spec = EffectSpec::new(EffectKind::BusCompressor);
        for (k, v) in &p.values {
            assert!(spec.set(k, *v).is_some(), "{}: no parameter {k}", p.key);
        }
        let model = spec.get("model") as usize;
        let limit = if model == 0 {
            spec.get("ratio") >= 2.0
        } else {
            spec.get("char") >= 0.5
        };
        seen[model][usize::from(limit)] = true;
        for lang in [cantus_ui::i18n::Locale::En, cantus_ui::i18n::Locale::Es] {
            cantus_ui::i18n::set_thread_locale(lang);
            assert_ne!(cantus_ui::i18n::t(&p.key), p.key, "{} in {lang:?}", p.key);
        }
    }
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    assert_eq!(seen, [[true; 2]; 3], "each model, compressing and limiting");
    let start = cantus_core::starting::starting_spec(EffectKind::BusCompressor);
    let list = read_presets(EffectKind::BusCompressor, false);
    let named = recognise(&start, &list).expect("named");
    assert_eq!(named.name, cantus_ui::i18n::t("preset.vinculum.glue"));
}
