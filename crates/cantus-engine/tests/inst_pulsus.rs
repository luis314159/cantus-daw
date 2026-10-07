//! Pulsus II through the whole engine: the promises that only hold end to end.
//!
//! - **An old kit renders exactly as it did.** A kit written before Pulsus II has no
//!   `version` and loads as model 1, and model 1 is bit-identical to the Pulsus that wrote
//!   it — pinned here by the SHA-256 of a busy render recorded *before* Pulsus II existed.
//! - A new kit (model 2) is position-seeded: a render that starts at bar 5 equals bars 5…8
//!   of a render from the top, noise, round robin and all.

use cantus_dsp::math;

mod support;
use cantus_core::*;
use cantus_engine::*;
use std::sync::Arc;

/// A kit exactly as Pulsus I wrote it to a project file: no `version`, no pad ids, no
/// field Pulsus II added. Every voice, a choke group, pans, drive and a sample pad.
const V1_KIT: &str = r#"{
  "type": "Pulsus",
  "pads": [
    {"pitch": 36, "name": "Kick", "voice": "Kick", "tune": -2.0, "decay": 0.5, "pitch_drop": 18.0,
     "pitch_decay": 0.05, "click": 0.6, "drive": 0.5, "noise_decay": 0.2, "tone": 2400.0, "mix": 0.5,
     "hp_cutoff": 7000.0, "level_db": 0.0, "pan": -0.2, "choke": 0, "muted": false, "sample": null},
    {"pitch": 37, "name": "Rim", "voice": "Snare", "tune": 14.0, "decay": 0.035, "pitch_drop": 15.5,
     "pitch_decay": 0.04, "click": 0.35, "drive": 0.2, "noise_decay": 0.03, "tone": 3200.0, "mix": 0.25,
     "hp_cutoff": 7000.0, "level_db": -5.0, "pan": 0.3, "choke": 0, "muted": false, "sample": null},
    {"pitch": 38, "name": "Snare", "voice": "Snare", "tune": 1.0, "decay": 0.13, "pitch_drop": 15.5,
     "pitch_decay": 0.04, "click": 0.35, "drive": 0.2, "noise_decay": 0.22, "tone": 2500.0, "mix": 0.7,
     "hp_cutoff": 7000.0, "level_db": 0.0, "pan": 0.0, "choke": 0, "muted": false, "sample": null},
    {"pitch": 39, "name": "Clap", "voice": "Sample", "tune": 3.0, "decay": 4.0, "pitch_drop": 15.5,
     "pitch_decay": 0.04, "click": 0.35, "drive": 0.2, "noise_decay": 0.2, "tone": 1900.0, "mix": 0.5,
     "hp_cutoff": 7000.0, "level_db": -3.0, "pan": 0.1, "choke": 2, "muted": false, "sample": "@CLAP@"},
    {"pitch": 42, "name": "Closed hat", "voice": "HiHat", "tune": 0.0, "decay": 0.055, "pitch_drop": 15.5,
     "pitch_decay": 0.04, "click": 0.35, "drive": 0.2, "noise_decay": 0.2, "tone": 9500.0, "mix": 0.5,
     "hp_cutoff": 7000.0, "level_db": -3.0, "pan": 0.25, "choke": 1, "muted": false, "sample": null},
    {"pitch": 45, "name": "Low tom", "voice": "Snare", "tune": -9.0, "decay": 0.42, "pitch_drop": 15.5,
     "pitch_decay": 0.04, "click": 0.35, "drive": 0.2, "noise_decay": 0.2, "tone": 1800.0, "mix": 0.06,
     "hp_cutoff": 7000.0, "level_db": -2.0, "pan": -0.4, "choke": 0, "muted": false, "sample": null},
    {"pitch": 46, "name": "Open hat", "voice": "HiHat", "tune": 0.5, "decay": 0.8, "pitch_drop": 15.5,
     "pitch_decay": 0.04, "click": 0.35, "drive": 0.2, "noise_decay": 0.2, "tone": 9000.0, "mix": 0.5,
     "hp_cutoff": 6500.0, "level_db": -4.0, "pan": 0.25, "choke": 1, "muted": false, "sample": null},
    {"pitch": 49, "name": "Crash", "voice": "HiHat", "tune": -5.0, "decay": 1.6, "pitch_drop": 15.5,
     "pitch_decay": 0.04, "click": 0.35, "drive": 0.2, "noise_decay": 0.2, "tone": 7000.0, "mix": 0.5,
     "hp_cutoff": 4000.0, "level_db": -6.0, "pan": -0.3, "choke": 2, "muted": false, "sample": null}
  ]
}"#;

/// A clap-like stereo WAV at 44.1 kHz, so the old sample path (linear interpolation,
/// rate conversion, choke) is inside the golden.
fn write_clap(dir: &std::path::Path) -> String {
    let wav = dir.join("clap.wav");
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&wav, spec).unwrap();
    let mut seed = 12_345u32;
    for i in 0..9_000 {
        let t = i as f32 / 44_100.0;
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let n = seed as f32 / u32::MAX as f32 * 2.0 - 1.0;
        let env = math::exp(-t * 18.0);
        let l = (math::sin(t * 1_300.0) * 0.3 + n * 0.5) * env;
        let r = (math::sin(t * 1_700.0) * 0.3 + n * 0.45) * env;
        w.write_sample((l * 6_000_000.0) as i32).unwrap();
        w.write_sample((r * 6_000_000.0) as i32).unwrap();
    }
    w.finalize().unwrap();
    wav.to_string_lossy().replace('\\', "/")
}

/// Four bars at 132 BPM that hit every pad: hats on the eighths with an open hat choked
/// by the next closed one, snare and clap on the backbeat (the clap choked by the crash
/// in group 2), a tom fill, rims, and velocities from 40 to 127.
fn v1_project(kit_json: &str) -> Project {
    let mut p = Project::new("pulsus v1");
    p.tempo_bpm = 132.0;
    let mut t = p.make_track("Drums");
    t.instrument = serde_json::from_str(kit_json).expect("a Pulsus I kit parses");
    let mut clip = p.make_clip("Beat", Ticks::ZERO, Ticks::beats(16));
    let mut hit = |p: &mut Project, at: Ticks, pitch: u8, vel: u8| {
        clip.notes
            .push(p.make_note(at, Ticks::SIXTEENTH, pitch, vel));
    };
    for bar in 0..4i64 {
        let b = Ticks::beats(bar * 4);
        hit(&mut p, b, 36, 127);
        hit(&mut p, b + Ticks::beats(2) + Ticks::EIGHTH, 36, 96);
        hit(&mut p, b + Ticks::beats(1), 38, 112);
        hit(&mut p, b + Ticks::beats(3), 38, 120);
        hit(&mut p, b + Ticks::beats(3), 39, 100);
        for e in 0..8i64 {
            let at = b + Ticks::EIGHTH * e;
            let open = e == 3 || e == 7;
            hit(&mut p, at, if open { 46 } else { 42 }, 50 + (e as u8 * 9));
        }
        hit(&mut p, b + Ticks::SIXTEENTH * 7, 37, 70);
        if bar == 3 {
            for (i, pitch) in [45u8, 45, 38, 45].iter().enumerate() {
                hit(
                    &mut p,
                    b + Ticks::beats(2) + Ticks::SIXTEENTH * i as i64,
                    *pitch,
                    90,
                );
            }
        }
        if bar % 2 == 1 {
            hit(&mut p, b + Ticks::beats(3) + Ticks::SIXTEENTH, 49, 110);
        }
    }
    hit(&mut p, Ticks::beats(15) + Ticks::SIXTEENTH * 3, 39, 40);
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn sha(samples: &[f32]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for s in samples {
        h.update(s.to_le_bytes());
    }
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// With `CANTUS_GOLDEN_DUMP=<folder>`, writes a golden's samples there (`<name>.f32`, raw
/// little-endian) so a reviewed `DSP_VERSION` bump can state how far each golden moved
/// (`docs/round21/math.md`).
fn golden_dump(name: &str, samples: &[f32]) {
    if let Some(dir) = std::env::var_os("CANTUS_GOLDEN_DUMP") {
        let bytes: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        std::fs::write(
            std::path::Path::new(&dir).join(format!("{name}.f32")),
            bytes,
        )
        .expect("write the golden's samples");
    }
}

fn render_attached(p: &Project, opts: &RenderOptions) -> Rendered {
    let mut snap = Snapshot::compile(p, 1);
    let mut cache = SampleCache::new();
    let errors = cache.attach(&mut snap);
    assert!(errors.is_empty(), "sample failed to load: {errors:?}");
    render(Arc::new(snap), opts).expect("render")
}

/// The promise Pulsus II was built under: **an old kit sounds exactly as it did**. The hash
/// was recorded from this render on 2026-09-24, before the first line of Pulsus II.
#[test]
fn a_pulsus_one_kit_renders_exactly_as_it_did() {
    let dir = tempfile::tempdir().unwrap();
    let clap = write_clap(dir.path());
    let p = v1_project(&V1_KIT.replace("@CLAP@", &clap));
    let r = render_attached(
        &p,
        &RenderOptions {
            tail_seconds: 1.0,
            block_frames: 512,
            ..Default::default()
        },
    );
    assert!(r.peak > 0.1 && r.peak < 1.0, "peak {}", r.peak);
    golden_dump("v1_kit", &r.samples);
    assert_eq!(
        sha(&r.samples),
        GOLDEN_V1_KIT,
        "a Pulsus I kit no longer renders as it did (peak {})",
        r.peak
    );
}

/// The historical fixture's kit, through the file loader and its migrations.
#[test]
fn the_v14_fixture_kit_renders_exactly_as_it_did() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../cantus-project/fixtures/v14.cantus");
    let text = std::fs::read_to_string(&path).unwrap();
    let loaded = cantus_project::parse(&text, true).unwrap();
    let mut project = loaded.file.project;
    // Only the kit: the other tracks are other instruments' business.
    project.tracks.retain(|t| t.instrument.pulsus().is_some());
    assert_eq!(
        project.tracks.len(),
        1,
        "the fixture stopped carrying a kit"
    );
    let snap = Arc::new(Snapshot::compile(&project, 1));
    let r = render(
        snap,
        &RenderOptions {
            tail_seconds: 0.5,
            block_frames: 512,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(r.peak > 0.05);
    golden_dump("v14_kit", &r.samples);
    assert_eq!(sha(&r.samples), GOLDEN_V14_KIT, "peak {}", r.peak);
}

/// Recorded on 2026-09-24 from Pulsus I (commit e20e3f4), before any Pulsus II change.
/// Re-blessed for `DSP_VERSION` 2 (round 21, libm transcendentals): max |Δ| 3.8e-5 (−88.5 dBFS).
const GOLDEN_V1_KIT: &str = "0749b097ff0d24febc9c54a5c5bd11a7d3704466feae692e36099c026911c0a6";
/// Recorded on 2026-09-24 from Pulsus I (commit e20e3f4), before any Pulsus II change.
/// Re-blessed for `DSP_VERSION` 2 (round 21, libm transcendentals): max |Δ| 3.0e-8 (−150.5 dBFS).
const GOLDEN_V14_KIT: &str = "20142bf8f3bbc7dfbd4fffdb55033257f5e3f39529b80ea2510063401bd56c13";

/// Model 1 keeps even its defect: its noise is seeded by a hit count that a seek resets,
/// so a render that starts at bar 2 is pinned too — as Pulsus I rendered it.
#[test]
fn a_pulsus_one_kit_rendered_from_bar_two_is_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let clap = write_clap(dir.path());
    let p = v1_project(&V1_KIT.replace("@CLAP@", &clap));
    let r = render_attached(
        &p,
        &RenderOptions {
            start: Ticks::beats(4),
            tail_seconds: 0.5,
            block_frames: 333,
            ..Default::default()
        },
    );
    golden_dump("v1_from_bar_2", &r.samples);
    assert_eq!(sha(&r.samples), GOLDEN_V1_FROM_BAR_2, "peak {}", r.peak);
}

/// Recorded on 2026-09-24 from Pulsus I (commit e20e3f4), before any Pulsus II change.
/// Re-blessed for `DSP_VERSION` 2 (round 21, libm transcendentals): max |Δ| 3.8e-5 (−88.5 dBFS).
const GOLDEN_V1_FROM_BAR_2: &str =
    "7cad1abd2d1e992b42c9a8a1febf6fbb8e04049df48133e41be6112b59f10c05";

// ---- Pulsus II through the engine ---------------------------------------------------------

/// The full Pulsus II kit, eight bars of a busy groove with velocity-zoned, round-robin
/// sample layers on the snare and a random round robin on a sampled shaker.
fn studio_project(dir: &std::path::Path) -> Project {
    let mut p = Project::new("pulsus ii");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Kit");
    let mut kit = PulsusParams::studio();
    let wav = |name: &str, hz: f32| -> String {
        let path = dir.join(name);
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48_000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut w = hound::WavWriter::create(&path, spec).unwrap();
        let mut seed = hz as u32 | 1;
        for i in 0..6_000 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let t = i as f32 / 48_000.0;
            let n = seed as f32 / u32::MAX as f32 - 0.5;
            let v =
                (math::sin(t * hz * std::f32::consts::TAU) * 0.4 + n * 0.3) * math::exp(-t * 25.0);
            w.write_sample(v).unwrap();
        }
        w.finalize().unwrap();
        path.to_string_lossy().replace('\\', "/")
    };
    let snare = kit.index_of_pitch(38).unwrap();
    let layers = &mut kit.pads[snare].layers;
    for (i, hz) in [310.0f32, 330.0, 350.0].iter().enumerate() {
        let mut l = PadLayer::new(wav(&format!("soft{i}.wav"), *hz));
        l.vel_hi = 80;
        l.xfade = 10;
        layers.push(l);
        let mut l = PadLayer::new(wav(&format!("hard{i}.wav"), *hz * 1.5));
        l.vel_lo = 81;
        l.xfade = 10;
        layers.push(l);
    }
    kit.pads[snare].round_robin = RoundRobin::NoRepeat;
    let mut shaker = DrumPad::new(70, "Shaker", PadVoice::Sample);
    shaker.layers = (0..4)
        .map(|i| PadLayer::new(wav(&format!("sh{i}.wav"), 2_000.0 + i as f32 * 300.0)))
        .collect();
    shaker.round_robin = RoundRobin::Random;
    shaker.id = kit.next_id();
    kit.pads.push(shaker);
    t.instrument = InstrumentSpec::Pulsus(kit);
    let mut clip = p.make_clip("Groove", Ticks::ZERO, Ticks::beats(32));
    for bar in 0..8i64 {
        let b = Ticks::beats(bar * 4);
        for s in 0..16i64 {
            let at = b + Ticks::SIXTEENTH * s;
            let v = 60 + ((bar * 7 + s * 13) % 60) as u8;
            if s % 4 == 0 {
                clip.notes.push(p.make_note(at, Ticks::SIXTEENTH, 36, 120));
            }
            if s % 8 == 4 {
                clip.notes
                    .push(p.make_note(at, Ticks::SIXTEENTH, 38, v + 10));
            }
            if s % 2 == 0 {
                clip.notes.push(p.make_note(
                    at,
                    Ticks::SIXTEENTH,
                    if s == 14 { 46 } else { 42 },
                    v,
                ));
            }
            clip.notes
                .push(p.make_note(at, Ticks::SIXTEENTH, 70, 40 + v / 2));
            if s == 11 {
                clip.notes.push(p.make_note(at, Ticks::SIXTEENTH, 39, v));
            }
        }
        clip.notes.push(p.make_note(
            b + Ticks::beats(3),
            Ticks::SIXTEENTH,
            [45, 47, 50, 56][bar as usize % 4],
            100,
        ));
        if bar % 4 == 0 {
            clip.notes.push(p.make_note(b, Ticks::beats(1), 49, 110));
        }
        // The ride rings for three seconds; bars 3 and 4 leave it out so nothing from
        // before bar 5 is still ringing half a second into it.
        if bar != 2 && bar != 3 {
            clip.notes
                .push(p.make_note(b + Ticks::beats(2), Ticks::SIXTEENTH, 51, 90));
        }
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

/// The fix for Pulsus I's hit counter, end to end: a render that starts at bar 5 equals
/// bars 5… of a render from the top — noise, round robin (random and no-repeat) and all.
#[test]
fn a_pulsus_two_render_from_bar_five_equals_the_same_bars_from_the_top() {
    let dir = tempfile::tempdir().unwrap();
    let p = studio_project(dir.path());
    let opts = |start: Ticks| RenderOptions {
        start,
        end: Some(Ticks::beats(32)),
        tail_seconds: 0.5,
        block_frames: 480,
        ..Default::default()
    };
    let whole = render_attached(&p, &opts(Ticks::ZERO));
    let late = render_attached(&p, &opts(Ticks::beats(16)));
    // Bar 5 starts 8 s in at 120 BPM. Tails of bar 4 ring into bar 5 in the whole render
    // only, so compare from half a second into bar 5, when every one of them has ended.
    let from = |seconds: f32| (seconds * 48_000.0) as usize * 2;
    let offset = from(8.0);
    let a = &whole.samples[offset + from(0.5)..offset + from(7.5)];
    let b = &late.samples[from(0.5)..from(7.5)];
    let diff = a
        .iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0f32, f32::max);
    eprintln!("pulsus: bars 5-8 from bar 5 against from the top: max difference {diff:e}");
    // Not close: identical. Every hit's seed and round-robin choice is computed from the
    // note, and nothing from before bar 5 is still ringing.
    assert_eq!(
        diff, 0.0,
        "bars 5… differ by {diff} between a render from the top and one from bar 5"
    );
}

/// Headroom: the default kit with every pad at full velocity at once (Pulsus I's test,
/// on the model 2 kit), the studio kit's busy groove, and every factory pad alone.
#[test]
fn the_kits_keep_their_headroom() {
    let all_at_once = |kit: PulsusParams| {
        let mut p = Project::new("headroom");
        let mut t = p.make_track("Kit");
        let pitches: Vec<u8> = kit.pads.iter().map(|d| d.pitch).collect();
        t.instrument = InstrumentSpec::Pulsus(kit);
        let mut clip = p.make_clip("All", Ticks::ZERO, Ticks::beats(4));
        for pitch in &pitches {
            clip.notes
                .push(p.make_note(Ticks::ZERO, Ticks::SIXTEENTH, *pitch, 127));
        }
        t.clips.push(clip);
        p.tracks.push(t);
        (p, pitches)
    };
    let (p, _) = all_at_once(PulsusParams::default());
    let r = render(
        Arc::new(Snapshot::compile(&p, 1)),
        &RenderOptions::default(),
    )
    .unwrap();
    assert!(r.peak < 0.95, "the default kit at once peaks at {}", r.peak);
    let dir = tempfile::tempdir().unwrap();
    let groove = render_attached(&studio_project(dir.path()), &RenderOptions::default());
    eprintln!("pulsus: studio groove peaks at {:.3}", groove.peak);
    assert!(
        groove.peak < 0.95,
        "the studio groove peaks at {}",
        groove.peak
    );
    for factory in PulsusParams::factory() {
        let (p, pitches) = all_at_once(factory.kit);
        for pitch in pitches {
            let mut solo = p.clone();
            solo.tracks[0].clips[0].notes.retain(|n| n.pitch == pitch);
            let r = render(
                Arc::new(Snapshot::compile(&solo, 1)),
                &RenderOptions::default(),
            )
            .unwrap();
            assert!(
                r.peak < 0.6,
                "{}: pad {pitch} alone peaks at {}",
                factory.label_key,
                r.peak
            );
            assert!(
                r.peak > 0.02,
                "{}: pad {pitch} alone is inaudible: {}",
                factory.label_key,
                r.peak
            );
        }
    }
}

/// A Pulsus II field set on a pad reaches the sound through the snapshot, and a field at
/// its default changes nothing: a model 2 kit with every new field at its default renders
/// exactly as the same kit serialised and read back.
#[test]
fn pulsus_two_fields_reach_the_sound_and_their_defaults_change_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let p = studio_project(dir.path());
    let opts = RenderOptions {
        end: Some(Ticks::beats(8)),
        tail_seconds: 0.2,
        ..Default::default()
    };
    let base = render_attached(&p, &opts);
    let json = serde_json::to_string(&p.tracks[0].instrument).unwrap();
    let mut again = p.clone();
    again.tracks[0].instrument = serde_json::from_str(&json).unwrap();
    assert_eq!(
        sha(&render_attached(&again, &opts).samples),
        sha(&base.samples)
    );
    let mut filtered = p.clone();
    let kit = filtered.tracks[0].instrument.pulsus_mut().unwrap();
    kit.pads[0].filter = 1;
    kit.pads[0].cutoff = 200.0;
    let changed = render_attached(&filtered, &opts);
    assert_ne!(
        sha(&changed.samples),
        sha(&base.samples),
        "the filter did nothing"
    );
}

/// Counts heap allocations on the thread that asks (ADR 0005: the audio thread never
/// allocates). Installed for this test binary only.
struct Counting;
static ALLOCATIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
thread_local! {
    static COUNTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
unsafe impl std::alloc::GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        if COUNTING.with(|c| c.get()) {
            ALLOCATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        unsafe { std::alloc::System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        if COUNTING.with(|c| c.get()) {
            ALLOCATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
}
#[global_allocator]
static GLOBAL: Counting = Counting;

/// A full pool — every model, sample layers, round robin, the pedal, chokes and drive —
/// configured, triggered and rendered without one allocation or free.
#[test]
fn pulsus_two_never_allocates_on_the_audio_thread() {
    let dir = tempfile::tempdir().unwrap();
    let p = studio_project(dir.path());
    let mut snap = Snapshot::compile(&p, 1);
    SampleCache::new().attach(&mut snap);
    let kit = snap.tracks[0].instrument.pulsus().unwrap().clone();
    let mut drums = cantus_dsp::Pulsus::new(48_000.0);
    let mut l = vec![0.0f32; 512];
    let mut r = vec![0.0f32; 512];
    COUNTING.with(|c| c.set(true));
    cantus_engine::pulsus_kit::configure(&mut drums, &kit, &snap.tracks[0].samples);
    for block in 0..400u32 {
        for (i, pad) in kit.pads.iter().enumerate() {
            if (block as usize + i).is_multiple_of(5) {
                let hit = cantus_dsp::pulsus::Hit {
                    id: block as u64 * 64 + i as u64,
                    tick: block as i64 * 96,
                    ordinal: block,
                    sequenced: true,
                };
                drums.trigger(pad.pitch, 40 + (block % 88) as u8, hit);
            }
        }
        if block % 7 == 0 {
            drums.controller(4, (block % 11) as f32 / 10.0);
        }
        drums.process_add(&mut l, &mut r);
    }
    COUNTING.with(|c| c.set(false));
    assert!(drums.active_voices() > 0);
    assert_eq!(
        ALLOCATIONS.load(std::sync::atomic::Ordering::Relaxed),
        0,
        "Pulsus allocated on the audio thread"
    );
}
