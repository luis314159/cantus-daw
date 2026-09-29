//! Exemplar through the engine: files decoded and mip-mapped by the sample cache, zones
//! configured from the spec's flat keys, the unit rendering them — bit-exact at unity,
//! the same at every block size, silent for a missing file, within a CPU budget.

mod support;
use cantus_core::inst::sampler::{zf, Zone};
use cantus_core::*;
use cantus_engine::inst as einst;
use cantus_engine::inst::sampler::tel;
use cantus_engine::*;
use std::path::Path;
use std::sync::Arc;

fn write_wav(path: &Path, channels: u16, rate: u32, frames: &[f32]) {
    let spec = hound::WavSpec {
        channels,
        sample_rate: rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for s in frames {
        w.write_sample(*s).unwrap();
    }
    w.finalize().unwrap();
}

fn noise(n: usize, seed: u32) -> Vec<f32> {
    let mut s = seed.max(1);
    (0..n)
        .map(|_| {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            (s as f32 / u32::MAX as f32) - 0.5
        })
        .collect()
}

fn sine(freq: f64, rate: f64, n: usize, amp: f32) -> Vec<f32> {
    (0..n)
        .map(|i| (std::f64::consts::TAU * freq * i as f64 / rate).sin() as f32 * amp)
        .collect()
}

/// A spec from zones.
fn spec_of(zones: &[Zone]) -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::Sampler);
    for (n, z) in zones.iter().enumerate() {
        for (k, v) in z.values_for(n) {
            s.set(&k, v).unwrap();
        }
        for (k, t) in z.texts_for(n) {
            s.set_text(&k, Some(t)).unwrap();
        }
    }
    s
}

/// A one-track project hosting `spec`, with notes `(start beat, beats, key, velocity)`.
fn project(spec: &NativeSpec, notes: &[(u32, u32, u8, u8)]) -> Project {
    let mut p = Project::new("exemplar");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Exemplar");
    t.instrument = InstrumentSpec::Native(spec.clone());
    let mut clip = p.make_clip("Notes", Ticks::ZERO, Ticks::beats(16));
    for (start, len, key, vel) in notes {
        clip.notes.push(p.make_note(
            Ticks::beats(*start as i64),
            Ticks::beats(*len as i64),
            *key,
            *vel,
        ));
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

/// The project's snapshot with its files decoded and mip-mapped, as the backend makes it.
fn snapshot(p: &Project, cache: &mut SampleCache) -> Arc<Snapshot> {
    let mut s = Snapshot::compile(p, 1);
    cache.attach(&mut s);
    Arc::new(s)
}

fn render_at(p: &Project, block: usize) -> Vec<f32> {
    let mut cache = SampleCache::new();
    let opts = RenderOptions {
        tail_seconds: 1.0,
        block_frames: block,
        ..Default::default()
    };
    render(snapshot(p, &mut cache), &opts).unwrap().samples
}

/// The unit alone, configured from a snapshot's first track.
fn unit_for(p: &Project, cache: &mut SampleCache) -> Box<dyn InstrumentUnit> {
    let snap = snapshot(p, cache);
    let mut u = einst::make(InstrumentKind::Sampler, 48_000.0);
    let spec = snap.tracks[0].instrument.native().unwrap().clone();
    u.configure(&spec, &snap.tracks[0].assets);
    u
}

fn play(u: &mut dyn InstrumentUnit, frames: usize) -> (Vec<f32>, Vec<f32>) {
    let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
    for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        u.process(a, b);
    }
    (l, r)
}

fn on(key: u8, velocity: f32, id: u64) -> einst::NoteOn {
    einst::NoteOn {
        key,
        velocity,
        id,
        channel: 0,
    }
}

#[test]
fn a_stereo_wav_at_its_root_plays_back_bit_for_bit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("take.wav");
    let data = noise(2 * 9_000, 3);
    write_wav(&path, 2, 48_000, &data);
    let zone = Zone::new(Some(path.to_string_lossy().into()), 0, 127, 60);
    let p = project(&spec_of(&[zone]), &[]);
    let mut u = unit_for(&p, &mut SampleCache::new());
    u.note_on(on(60, 1.0, 1));
    let (l, r) = play(u.as_mut(), 8_000);
    for i in 0..8_000 {
        assert_eq!(l[i].to_bits(), data[2 * i].to_bits(), "left {i}");
        assert_eq!(r[i].to_bits(), data[2 * i + 1].to_bits(), "right {i}");
    }
}

#[test]
fn zones_far_above_their_root_get_mip_maps_and_near_ones_do_not() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("low.wav");
    write_wav(&path, 1, 44_100, &noise(44_100, 5));
    let far = Zone::new(Some(path.to_string_lossy().into()), 24, 96, 36);
    let near = Zone::new(Some(path.to_string_lossy().into()), 36, 36, 36);
    let mut s = Snapshot::compile(&project(&spec_of(&[far, near]), &[]), 1);
    let mut cache = SampleCache::new();
    assert!(cache.attach(&mut s).is_empty());
    let keys: Vec<&str> = s.tracks[0].assets.iter().map(|a| a.0.as_str()).collect();
    // 60 semitones + bend + an octave of margin: r ≈ 2^(74/12) → levels 1…6.
    for k in 1..=6 {
        assert!(
            keys.contains(&format!("z0.sample~{k}").as_str()),
            "{keys:?}"
        );
    }
    assert!(
        keys.iter().filter(|k| k.starts_with("z1.sample~")).count() <= 1,
        "a zone played at its root needs at most the margin's level: {keys:?}"
    );
    // The same file's levels are built once and shared.
    let a = s.tracks[0]
        .assets
        .iter()
        .find(|a| a.0 == "z0.sample~1")
        .unwrap();
    let b = s.tracks[0].assets.iter().find(|a| a.0 == "z1.sample~1");
    if let (Some(a), Some(b)) = (a.1.as_ref(), b.and_then(|b| b.1.as_ref())) {
        assert!(Arc::ptr_eq(a, b));
    }
}

#[test]
fn a_missing_file_is_silence_and_the_other_zones_play() {
    let lost = Zone::new(Some("C:/nowhere/lost.wav".into()), 0, 59, 60);
    let mut sine = Zone::new(None, 60, 127, 24);
    sine.set(zf::GEN, 1.0).set(zf::MODE, 2.0);
    let p = project(&spec_of(&[lost, sine]), &[]);
    let mut cache = SampleCache::new();
    let mut u = unit_for(&p, &mut cache);
    u.note_on(on(48, 1.0, 1));
    let (l, _) = play(u.as_mut(), 4_000);
    assert!(
        l.iter().all(|x| *x == 0.0),
        "the lost zone is exact silence"
    );
    u.note_on(on(72, 1.0, 2));
    let (l, _) = play(u.as_mut(), 4_000);
    assert!(l.iter().any(|x| x.abs() > 0.01), "the built-in zone plays");
    assert!(matches!(
        cache.status(Path::new("C:/nowhere/lost.wav")),
        SampleStatus::Failed(_)
    ));
}

#[test]
fn a_multisample_renders_the_same_at_every_block_size() {
    let dir = tempfile::tempdir().unwrap();
    let mut zones = Vec::new();
    for (i, root) in [48u8, 60, 72].iter().enumerate() {
        let path = dir.path().join(format!("s{root}.wav"));
        let f = 440.0 * ((*root as f64 - 69.0) / 12.0).exp2();
        let mut d = sine(f, 44_100.0, 44_100 * 2, 0.3);
        for (j, x) in noise(d.len(), 9 + i as u32).iter().enumerate() {
            d[j] += x * 0.05;
        }
        write_wav(&path, 1, 44_100, &d);
        let mut z = Zone::new(
            Some(path.to_string_lossy().into()),
            root - 6,
            root + 5,
            *root,
        );
        z.set(zf::MODE, 3.0)
            .set(zf::LS, 20_000.0)
            .set(zf::LE, 60_000.0)
            .set(zf::XF, 30.0);
        zones.push(z);
    }
    let mut spec = spec_of(&zones);
    spec.set("amp.release", 120.0);
    spec.set("filter.type", 1.0);
    spec.set("filter.cutoff", 4_000.0);
    spec.set("lfo1.pitch", 20.0);
    let notes = [
        (0, 2, 45, 90),
        (0, 3, 57, 100),
        (1, 2, 66, 60),
        (2, 4, 79, 127),
    ];
    let p = project(&spec, &notes);
    let reference = render_at(&p, 512);
    assert!(reference.iter().any(|x| x.abs() > 0.05));
    for block in [1usize, 64, 1_000] {
        let out = render_at(&p, block);
        assert!(out == reference, "block size {block} differs");
    }
}

#[test]
fn every_default_written_out_changes_no_sample() {
    let empty = NativeSpec::new(InstrumentKind::Sampler);
    let mut explicit = empty.clone();
    for info in InstrumentKind::Sampler.params() {
        explicit.set(info.key, info.default).unwrap();
    }
    let notes = [(0, 1, 60, 100), (1, 2, 64, 80)];
    let a = render_at(&project(&empty, &notes), 256);
    let b = render_at(&project(&explicit, &notes), 256);
    assert!(
        a.iter().any(|x| *x != 0.0),
        "the fresh instrument sounds (its built-in tone)"
    );
    assert!(a == b, "writing every default out changed the render");
}

/// Frequency of a steady tone from its rising zero crossings (interpolated).
fn frequency(x: &[f32], rate: f64) -> f64 {
    let mut times = Vec::new();
    for i in 1..x.len() {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            let frac = -x[i - 1] as f64 / (x[i] - x[i - 1]) as f64;
            times.push(i as f64 - 1.0 + frac);
        }
    }
    (times.len() - 1) as f64 * rate / (times[times.len() - 1] - times[0])
}

#[test]
fn the_built_in_sine_is_in_tune_across_the_keyboard() {
    let mut z = Zone::new(None, 0, 127, 24);
    z.set(zf::GEN, 1.0).set(zf::MODE, 2.0);
    let p = project(&spec_of(&[z]), &[]);
    let mut cache = SampleCache::new();
    let mut worst = 0.0f64;
    for key in [24u8, 36, 45, 57, 69, 81, 93, 105] {
        let mut u = unit_for(&p, &mut cache);
        u.note_on(on(key, 1.0, key as u64));
        let (l, _) = play(u.as_mut(), 48_000);
        let f = frequency(&l[4_000..], 48_000.0);
        let want = 440.0 * ((key as f64 - 69.0) / 12.0).exp2();
        let cents = 1200.0 * (f / want).log2();
        worst = worst.max(cents.abs());
        assert!(
            cents.abs() < 0.1,
            "key {key}: {f:.4} Hz for {want:.4}, {cents:.4} cents"
        );
    }
    println!("pitch error, worst across C1…A7: {worst:.4} cents");
}

#[test]
fn sixteen_transposed_stereo_voices_render_faster_than_real_time() {
    // A stereo sample played a fifth up (resampled, High), 4 seconds — a generous ceiling
    // on a machine shared by a dozen builds; the per-voice cost is printed for the report.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wide.wav");
    let mut d = noise(2 * 48_000 * 6, 11);
    for x in d.iter_mut() {
        *x *= 0.2;
    }
    write_wav(&path, 2, 48_000, &d);
    let zone = Zone::new(Some(path.to_string_lossy().into()), 0, 127, 53);
    let mut spec = spec_of(&[zone]);
    // 16 under a real-time ceiling holds even with the whole workspace's tests running in
    // parallel on a shared machine (measured: 2 % of a core per voice alone, 3.8 % there).
    let voices = 16u8;
    spec.set("voices", voices as f32);
    let p = project(&spec, &[]);
    let mut u = unit_for(&p, &mut SampleCache::new());
    for i in 0..voices {
        u.note_on(on(60, 0.8, i as u64 + 1));
    }
    assert_eq!(u.active_voices(), voices as usize);
    let secs = 4.0;
    let start = std::time::Instant::now();
    let (l, _) = play(u.as_mut(), (48_000.0 * secs) as usize);
    let took = start.elapsed().as_secs_f64();
    assert!(l.iter().any(|x| *x != 0.0));
    let per_voice = took / secs / voices as f64 * 100.0;
    println!(
        "{voices} stereo voices, +7 st, High: {took:.3} s for {secs} s of audio \
         ({per_voice:.3} % of a core per voice)"
    );
    assert!(
        took < secs,
        "{voices} voices took {took:.2} s for {secs} s of audio"
    );
}

#[test]
fn memory_per_minute_of_stereo_audio_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("minute.wav");
    write_wav(&path, 2, 48_000, &noise(2 * 48_000 * 60, 13));
    // Played up to two octaves above its root: levels 1 and 2 (plus the margin's).
    let zone = Zone::new(Some(path.to_string_lossy().into()), 60, 84, 60);
    let p = project(&spec_of(&[zone]), &[]);
    let mut cache = SampleCache::new();
    let u = unit_for(&p, &mut cache);
    let mib = u.telemetry()[tel::MEMORY_MIB];
    let base = 48_000.0 * 60.0 * 2.0 * 4.0 / 1_048_576.0;
    println!("one minute of 48 kHz stereo: {base:.2} MiB as f32, {mib:.2} MiB with its mip-maps");
    assert!((base..base * 2.0).contains(&mib), "{mib} MiB");
    assert_eq!(u.telemetry()[tel::ZONES], 1.0);
}

#[test]
fn telemetry_names_the_zones_that_sound() {
    let mut a = Zone::new(None, 0, 59, 24);
    a.set(zf::GEN, 2.0).set(zf::MODE, 2.0);
    let mut b = Zone::new(None, 60, 127, 24);
    b.set(zf::GEN, 3.0).set(zf::MODE, 2.0);
    let p = project(&spec_of(&[a, b]), &[]);
    let mut u = unit_for(&p, &mut SampleCache::new());
    u.note_on(on(64, 1.0, 1));
    play(u.as_mut(), 512);
    let t = u.telemetry();
    assert_eq!(t[0], 1.0, "one voice");
    assert_eq!(t[tel::LAYERS], 1.0);
    assert_eq!(t[tel::PLAYING], 1.0);
    assert_eq!(t[tel::FIRST_PAIR], 1.0, "zone 1 plays");
    assert!(t[tel::FIRST_PAIR + 1] >= 0.0);
}
