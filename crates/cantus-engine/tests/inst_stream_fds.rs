//! Third review #1: streamed files' handles stay within a budget — a quarter of the
//! open-files limit (raised at start) — the least recently read closed and opened again on
//! demand, so a library larger than the budget streams on and the application can still open
//! files. Its own test binary: the budget and the limit are the process's.

use cantus_core::inst::sampler::Zone;
use cantus_core::*;
use cantus_engine::inst as einst;
use cantus_engine::*;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn wav(path: &Path, frames: usize, seed: u32) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    for i in 0..frames {
        let v = (cantus_dsp::math::sin(i as f32 * 0.013 * (1 + seed % 7) as f32) * 9_000.0) as i16;
        w.write_sample(v ^ (seed as i16 & 7)).unwrap();
    }
    w.finalize().unwrap();
}

/// `n` zones, one key each, on files long enough to stream.
fn spec(dir: &Path, n: usize) -> NativeSpec {
    let mut s = NativeSpec::new(InstrumentKind::Sampler);
    for i in 0..n {
        let p = dir.join(format!("s{i:03}.wav"));
        wav(&p, 40_000, i as u32);
        let key = (i % 128) as u8;
        let z = Zone::new(Some(p.to_string_lossy().into()), key, key, key);
        for (k, v) in z.values_for(i) {
            s.set(&k, v).unwrap();
        }
        for (k, t) in z.texts_for(i) {
            s.set_text(&k, Some(t)).unwrap();
        }
    }
    s
}

fn project(spec: &NativeSpec, n: usize) -> Project {
    let mut p = Project::new("fds");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Exemplar");
    t.instrument = InstrumentSpec::Native(spec.clone());
    let mut clip = p.make_clip("Notes", Ticks::ZERO, Ticks::beats(16));
    for i in 0..n {
        let note = p.make_note(
            Ticks::beats(i as i64 % 8),
            Ticks::beats(1),
            (i % 128) as u8,
            100,
        );
        clip.notes.push(note);
    }
    t.clips.push(clip);
    p.tracks.push(t);
    p
}

fn render_with(p: &Project, cache: &mut SampleCache) -> Vec<f32> {
    let mut s = Snapshot::compile(p, 1);
    assert!(cache.attach(&mut s).is_empty());
    let opts = RenderOptions {
        tail_seconds: 0.2,
        ..Default::default()
    };
    render(Arc::new(s), &opts).unwrap().samples
}

#[test]
fn more_streamed_files_than_handles_stream_on_within_the_budget() {
    let dir = tempfile::tempdir().unwrap();
    einst::stream::hub().set_cache_dir(dir.path().join("cache"));
    // The machine's disk may be nearly full: these cache files are small.
    einst::stream::set_free_floor(0);
    // On Unix a low limit, as a Finder-started macOS app has (256), lowered further: the
    // budget follows it. Elsewhere the budget is set small directly.
    #[cfg(unix)]
    {
        assert!(cantus_rt::files::set_limit(128));
        assert_eq!(cantus_rt::files::limit(), 128);
    }
    #[cfg(not(unix))]
    einst::stream::set_handle_budget(8);
    let budget = einst::stream::handles_open().1;
    #[cfg(unix)]
    assert_eq!(budget, 32, "a quarter of the limit");
    // More files than the limit itself: one handle each would run the process out.
    let n = budget * 5;
    let spec = spec(dir.path(), n);
    let p = project(&spec, n);
    let whole = render_with(&p, &mut SampleCache::new());
    let mut cache = SampleCache::new();
    cache.set_streaming(true);
    let first = render_with(&p, &mut cache);
    assert!(first == whole, "the first load: the in-memory samples");
    // Every cache file written and switched to: still within the budget.
    let t = Instant::now();
    let cached = || {
        std::fs::read_dir(dir.path().join("cache"))
            .map(|r| {
                r.flatten()
                    .filter(|e| e.path().extension().is_some_and(|x| x == "cstream"))
                    .count()
            })
            .unwrap_or(0)
    };
    while cached() < n && t.elapsed() < Duration::from_secs(30) {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(cached(), n);
    std::thread::sleep(Duration::from_millis(200));
    let again = render_with(&p, &mut cache);
    assert!(
        again == whole,
        "read from the files, handles closed and opened again"
    );
    let (open, budget) = einst::stream::handles_open();
    assert!(open <= budget + 1, "{open} handles, budget {budget}");
    // The application can still open files.
    let unrelated = dir.path().join("unrelated.txt");
    std::fs::write(&unrelated, b"x").unwrap();
    assert!(std::fs::File::open(&unrelated).is_ok());
    // A fresh session's loads (heads from the cache files) keep to it too.
    drop(cache);
    einst::stream::hub().let_go_unheld();
    let mut fresh = SampleCache::new();
    fresh.set_streaming(true);
    let later = render_with(&p, &mut fresh);
    assert!(later == whole);
    let (open, budget) = einst::stream::handles_open();
    assert!(open <= budget + 1, "{open} handles, budget {budget}");
    assert!(std::fs::File::open(&unrelated).is_ok());
}
