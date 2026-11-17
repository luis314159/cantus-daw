//! Caelum end to end (fx side round, wave A, ADR 0138), on the boot path: an app
//! made by `CantusApp::assemble` with the real engine handle; a chord struck once on its
//! track (half a second, then silence for the tail) and a second track that plays later (the
//! key). Caelum is inserted on the chord's strip as the picker inserts it — on its starting
//! preset, Octave shimmer, no latency:
//!
//! - **the shimmer heard**: the tail's energy an octave and two over the chord grows against
//!   the same space without the shimmer, and rises as the tail ages;
//! - **infinite holds**: the tail's level a second and three seconds after the chord within
//!   a decibel and a half (a finite tail falls far more);
//! - **freeze holds**: thrown live a second in, the level held to the end while the input
//!   plays on (a frozen tank takes nothing);
//! - **ducking keyed by another strip**: the later track as the slot's key steps the tail
//!   aside where, unkeyed, it rings;
//! - an edit heard and undone to the bit; live in odd blocks equal to the offline render;
//!   and at its defaults, the dry master to the bit (the key still routed).
//!
//! Its own test binary: it points `CANTUS_DATA_DIR` at a temporary folder. // fxa:caelum

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

fn left(x: &[f32]) -> Vec<f32> {
    x.iter().step_by(2).copied().collect()
}

/// Mean square of the left channel from `a` to `b` seconds.
fn power(x: &[f32], a: f32, b: f32) -> f64 {
    let l = left(x);
    let (i, j) = (
        (a * SR as f32) as usize,
        ((b * SR as f32) as usize).min(l.len()),
    );
    l[i..j]
        .iter()
        .map(|v| f64::from(*v) * f64::from(*v))
        .sum::<f64>()
        / (j - i) as f64
}

fn db(p: f64) -> f64 {
    10.0 * p.max(1e-30).log10()
}

/// The energy of `x` (one channel) in the band `lo … hi` Hz, by Goertzel's probes 2 Hz apart
/// over the window `a … b` seconds.
fn band(x: &[f32], a: f32, b: f32, lo: f32, hi: f32) -> f64 {
    let (i, j) = ((a * SR as f32) as usize, (b * SR as f32) as usize);
    let w = &x[i..j];
    let mut sum = 0.0f64;
    let mut f = lo;
    while f <= hi {
        let c = 2.0 * (std::f64::consts::TAU * f64::from(f) / SR as f64).cos();
        let (mut s1, mut s2) = (0.0f64, 0.0f64);
        for v in w {
            let s0 = f64::from(*v) + c * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        sum += s1 * s1 + s2 * s2 - c * s1 * s2;
        f += 2.0;
    }
    sum
}

/// The share of the tail's energy over the chord's own octave (an octave and more above its
/// root, 440 Hz … 4 kHz) against the chord's octave (220 … 440 Hz), dB.
fn climbed_db(x: &[f32], a: f32, b: f32) -> f64 {
    let l = left(x);
    db(band(&l, a, b, 450.0, 4_000.0)) - db(band(&l, a, b, 200.0, 440.0))
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
fn caelum_shimmers_holds_freezes_ducks_and_undoes_on_the_boot_path() {
    let (mut app, mut proc_) = assembled();
    let mut p = Project::new("Caelum");
    p.tempo_bpm = 120.0;
    // The chord: A3 C#4 E4 for a beat (half a second), then the tail alone.
    let chord = line(
        &mut p,
        "Chord",
        &[(57, 0, 1, 100), (61, 0, 1, 100), (64, 0, 1, 100)],
    );
    p.tracks.push(chord);
    // The key: a loud line from beat 2 to the end.
    let key = line(
        &mut p,
        "Key",
        &[(45, 2, 2, 127), (48, 4, 2, 127), (52, 6, 2, 127)],
    );
    p.tracks.push(key);
    app.replace_project(p);
    let strip = app.project().tracks[0].strip.expect("a strip");
    let key_strip = app.project().tracks[1].strip.expect("a strip");
    assert_ne!(strip, key_strip, "a strip each");
    let dry = rendered(&mut app);
    assert!(power(&dry, 0.0, 0.5) > 1e-6, "the chord sounds");

    // Inserted as the picker inserts it: Octave shimmer, no latency.
    app.add_effect(strip, EffectKind::Shimmer);
    let effect = app.project().strip(strip).unwrap().effects[0].id;
    let spec = app.project().effect(strip, effect).unwrap().spec.clone();
    assert_eq!(
        (spec.get("shim"), spec.get("pa"), spec.get("mix")),
        (0.55, 12.0, 0.35),
        "Octave shimmer"
    );
    let slot = app.project().strip(strip).unwrap().effects[0].clone();
    assert_eq!(EffectUnit::new(&slot, 48_000.0).latency(), 0);
    let set = |app: &mut CantusApp, key: &str, value: f32| {
        app.exec(Command::SetEffectParam {
            strip,
            effect,
            key: key.into(),
            value,
        })
    };
    // The key track muted while the chord's tail is measured (it plays again as the key).
    let key_track = app.project().tracks[1].id;
    app.exec(Command::SetTrackMute {
        id: key_track,
        muted: true,
    });

    // The shimmer heard: against the same space without it, the tail two to three and a half
    // seconds on holds more above the chord's octave — and it climbs as the tail ages.
    let shimmer = rendered(&mut app);
    set(&mut app, "shim", 0.0);
    let plain = rendered(&mut app);
    app.undo();
    let (early, late) = (
        climbed_db(&shimmer, 0.6, 1.4),
        climbed_db(&shimmer, 2.0, 3.5),
    );
    let plain_late = climbed_db(&plain, 2.0, 3.5);
    eprintln!(
        "above the chord's octave: shimmer {early:.1} dB early, {late:.1} dB late; without it {plain_late:.1} dB late"
    );
    assert!(
        late > plain_late + 4.0,
        "the shimmer's octaves are heard: {late:.1} dB against {plain_late:.1} dB"
    );
    assert!(
        late > early + 3.0,
        "the light climbs as the tail ages: {early:.1} → {late:.1} dB"
    );

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

    // Infinite holds (the shimmer off: an infinite shimmer fades as it climbs, the owner's
    // default): a second and three seconds after the chord within 1.5 dB; the same space
    // finite falls by far more.
    set(&mut app, "shim", 0.0);
    let finite = rendered(&mut app);
    set(&mut app, "inf", 1.0);
    let endless = rendered(&mut app);
    let held = db(power(&endless, 3.0, 4.0)) - db(power(&endless, 1.0, 2.0));
    let fell = db(power(&finite, 3.0, 4.0)) - db(power(&finite, 1.0, 2.0));
    eprintln!("infinite: {held:+.2} dB over two seconds; finite {fell:+.1} dB");
    assert!(held.abs() < 1.5, "infinite holds: {held:+.2} dB");
    assert!(fell < -8.0, "a finite tail falls: {fell:+.1} dB");
    set(&mut app, "inf", 0.0);
    set(&mut app, "shim", 0.55);

    // Freeze holds: thrown live a second in (the chord over, its tail in the tank), the level
    // held to the end.
    app.seek(Ticks::ZERO);
    app.sync_engine();
    proc_.settle();
    app.toggle_play();
    let mut frozen = Vec::new();
    play(&mut proc_, SR, &mut frozen);
    set(&mut app, "freeze", 1.0);
    app.sync_engine();
    play(&mut proc_, 3 * SR, &mut frozen);
    app.toggle_play();
    let kept = db(power(&frozen, 3.0, 4.0)) - db(power(&frozen, 1.2, 2.0));
    let open = db(power(&shimmer, 3.0, 4.0)) - db(power(&shimmer, 1.2, 2.0));
    eprintln!("frozen: {kept:+.2} dB from 1.2–2 s to 3–4 s (open: {open:+.1} dB)");
    assert!(kept.abs() < 1.0, "freeze holds: {kept:+.2} dB");
    assert!(open < -6.0, "an open tank falls: {open:+.1} dB");
    set(&mut app, "freeze", 0.0);

    // Ducking keyed by another strip: the key unmuted and routed to the slot, a deep range;
    // the chord's tail (two seconds on, while the key plays) steps aside where, unkeyed, it
    // rings. The wet alone is the master less the master with the mix at 0 (the null).
    app.exec(Command::SetTrackMute {
        id: key_track,
        muted: false,
    });
    set(&mut app, "duck", 30.0);
    set(&mut app, "dthr", -40.0);
    let wet_of = |app: &mut CantusApp| {
        let out = rendered(app);
        let mix = app.project().effect(strip, effect).unwrap().spec.get("mix");
        app.exec(Command::SetEffectParam {
            strip,
            effect,
            key: "mix".into(),
            value: 0.0,
        });
        let base = rendered(app);
        app.exec(Command::SetEffectParam {
            strip,
            effect,
            key: "mix".into(),
            value: mix,
        });
        out.iter()
            .zip(&base)
            .map(|(a, b)| a - b)
            .collect::<Vec<f32>>()
    };
    let unkeyed = wet_of(&mut app);
    app.exec(Command::SetEffectSidechain {
        strip,
        effect,
        source: Some(key_strip),
    });
    let keyed = wet_of(&mut app);
    let stepped = db(power(&keyed, 1.5, 3.5)) - db(power(&unkeyed, 1.5, 3.5));
    eprintln!("ducked by the key: {stepped:+.1} dB");
    assert!(stepped < -10.0, "the key ducks the tail: {stepped:+.1} dB");
    assert_eq!(
        app.project().strip(strip).unwrap().effects[0].sidechain,
        Some(key_strip),
        "the key is routed"
    );
    // Keyed, it listens to the key alone: before the key plays (0.6 … 0.95 s, the chord just
    // over) the tail is not ducked by the chord itself, as the unkeyed one still is.
    let before = db(power(&keyed, 0.6, 0.95)) - db(power(&unkeyed, 0.6, 0.95));
    eprintln!("before the key: keyed {before:+.1} dB over the self-ducked tail");
    assert!(
        before > 6.0,
        "keyed: not ducked by its own input: {before:+.1} dB"
    );
    set(&mut app, "duck", 0.0);

    // An edit heard, undone to the bit.
    let base = rendered(&mut app);
    set(&mut app, "pa", 7.0);
    let moved = rendered(&mut app);
    assert!(
        moved.iter().zip(&base).any(|(a, b)| (a - b).abs() > 1e-3),
        "the first voice a fifth is heard"
    );
    app.undo();
    assert_eq!(rendered(&mut app), base, "undone, to the bit");

    // Its neutral defaults: the dry master, to the bit (the key still routed).
    let defaults: Vec<Command> = EffectKind::Shimmer
        .params()
        .iter()
        .map(|i| Command::SetEffectParam {
            strip,
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

/// The factory spaces the DSP measured are the file's first eleven, as written (the file's
/// output level aside, which the fit sets), and the starting preset is the first of them,
/// named in the editor.
#[test]
fn the_file_holds_the_dsp_s_factory_spaces_and_the_starting_preset_is_named() {
    use cantus_ui::views::fx::frame::{read_presets, recognise};
    let file: Vec<_> = cantus_ui::preset_data::presets_of(
        cantus_ui::preset_data::FileUnit::Effect(EffectKind::Shimmer),
    )
    .collect();
    let dsp = cantus_dsp::caelum::presets::factory();
    assert!(file.len() >= 12, "twelve presets or more: {}", file.len());
    for (i, want) in dsp.iter().enumerate() {
        let mut spec = EffectSpec::new(EffectKind::Shimmer);
        for (k, v) in &file[i].values {
            let _ = spec.set(k, *v);
        }
        assert_eq!(
            cantus_engine::fx::caelum::settings(&spec),
            want.settings,
            "{} is {}",
            file[i].key,
            want.name
        );
    }
    let start = cantus_core::starting::starting_spec(EffectKind::Shimmer);
    let list = read_presets(EffectKind::Shimmer, false);
    let named = recognise(&start, &list).expect("named");
    assert_eq!(
        named.name,
        cantus_ui::i18n::t("preset.caelum.octave_shimmer")
    );
    // No factory preset starts frozen: it would hold silence.
    for p in &file {
        assert_ne!(p.values.get("freeze").copied(), Some(1.0), "{}", p.key);
    }
}
