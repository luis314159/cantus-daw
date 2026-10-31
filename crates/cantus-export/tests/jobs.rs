//! Whole exports from a project: the mix through the engine's render, stems in every
//! format (FLAC stems to the bit of the engine's stems), and a stop at any moment leaves
//! nothing behind.

mod common;

use cantus_core::*;
use cantus_engine::{stem_file_name, RenderOptions, StemOptions, StemSource, StemSpec};
use cantus_export::process::quantise;
use cantus_export::{
    export_mix, export_rendered, export_stems, Control, Depth, Dither, ExportError, ExportSettings,
    Format, MixJob, Rate, Scope, StemsJob, Tags,
};
use std::sync::Arc;

/// A lead and a bass, each on a strip of its own, eight beats at 128 BPM.
fn project() -> Project {
    let mut p = Project::new("Export");
    p.tempo_bpm = 128.0;
    for (name, base, len) in [
        ("Lead", 60u8, Ticks::EIGHTH),
        ("Bass", 36u8, Ticks::QUARTER),
    ] {
        let mut t = p.make_track(name);
        let mut c = p.make_clip(name, Ticks::ZERO, Ticks::beats(8));
        for i in 0..8 {
            let n = p.make_note(len * i, len - Ticks(40), base + (i as u8 % 3) * 4, 100);
            c.notes.push(n);
        }
        t.clips.push(c);
        p.tracks.push(t);
    }
    p
}

fn snapshot() -> Arc<Snapshot> {
    Arc::new(Snapshot::compile(&project(), 1))
}

fn render_opts() -> RenderOptions {
    RenderOptions {
        tail_seconds: 0.5,
        ..Default::default()
    }
}

fn specs(p: &Project) -> Vec<StemSpec> {
    let mut v: Vec<StemSpec> = (0..p.strips.len())
        .map(|i| StemSpec {
            source: StemSource::Strip(i),
            file_name: stem_file_name(i + 1, &format!("Strip {}", i + 1)),
        })
        .collect();
    v.push(StemSpec {
        source: StemSource::Master,
        file_name: "00 Master.wav".into(),
    });
    v
}

fn files_in(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn the_mix_is_the_engines_render_encoded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Export.flac");
    let s = ExportSettings {
        format: Format::Flac,
        depth: Depth::Int24,
        dither: Dither::Off,
        rate: Rate::Session,
        ..Default::default()
    };
    let job = MixJob {
        snapshot: snapshot(),
        render: render_opts(),
        units: Vec::new(),
        instruments: Vec::new(),
    };
    export_mix(job, &s, &Tags::default(), &path, &Control::new()).unwrap();
    let reference = cantus_engine::render(snapshot(), &render_opts()).unwrap();
    let (_, _, _, ints, _) = common::decode_flac(&path);
    assert!(ints == quantise(&reference.samples, 2, 24, Dither::Off));
}

#[test]
fn stems_in_every_format_and_flac_stems_to_the_bit() {
    let p = project();
    let specs = specs(&p);
    let opts = StemOptions {
        render: render_opts(),
        ..Default::default()
    };
    let reference = cantus_engine::render_stems(snapshot(), &opts, &specs).unwrap();
    for format in [Format::Flac, Format::Mp3, Format::Ogg, Format::Wav] {
        let dir = tempfile::tempdir().unwrap();
        let s = ExportSettings {
            scope: Scope::Stems,
            format,
            depth: Depth::Int24,
            dither: Dither::Off,
            rate: Rate::Session,
            ..Default::default()
        };
        let job = StemsJob {
            snapshot: snapshot(),
            options: opts.clone(),
            specs: specs.clone(),
            units: Vec::new(),
            instruments: Vec::new(),
        };
        let done = export_stems(job, &s, &Tags::default(), dir.path(), &Control::new()).unwrap();
        let ext = format.extension();
        let mut want: Vec<String> = reference
            .iter()
            .filter(|st| !st.silent)
            .map(|st| st.spec.file_name.replace(".wav", &format!(".{ext}")))
            .collect();
        want.sort();
        assert_eq!(
            files_in(dir.path()),
            want,
            "{format:?}: the stems and nothing else"
        );
        assert_eq!(done.files.len(), want.len());
        assert_eq!(
            done.silent.len(),
            reference.iter().filter(|s| s.silent).count()
        );
        if format == Format::Flac {
            for st in reference.iter().filter(|s| !s.silent) {
                let path = dir.path().join(st.spec.file_name.replace(".wav", ".flac"));
                let (.., ints, tags) = common::decode_flac(&path);
                assert!(
                    ints == quantise(&st.samples, 2, 24, Dither::Off),
                    "{}",
                    st.spec.file_name
                );
                assert!(tags.iter().any(|(k, _)| k == "TITLE"));
            }
        }
    }
}

/// Stop the export, on a thread of its own, once it is `at` done.
fn cancel_at(
    at: f32,
    run: impl FnOnce(Arc<Control>) -> Result<cantus_export::Done, ExportError>
        + std::marker::Send
        + 'static,
) {
    let ctl = Control::new();
    ctl.stop_at(at);
    let result = std::thread::spawn(move || run(ctl)).join().unwrap();
    assert!(
        matches!(result, Err(ExportError::Cancelled)),
        "stopped at {at}: {result:?}"
    );
}

#[test]
fn a_stopped_export_leaves_no_file_in_any_format() {
    let long = common::music(44_100, 40.0);
    for format in [Format::Wav, Format::Flac, Format::Mp3, Format::Ogg] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(format!("song.{}", format.extension()));
        let s = ExportSettings {
            format,
            ..Default::default()
        };
        let (a, p) = (long.clone(), path.clone());
        cancel_at(0.3, move |ctl| {
            export_rendered(a, &s, &Tags::default(), &p, &ctl)
        });
        assert!(
            files_in(dir.path()).is_empty(),
            "{format:?}: {:?}",
            files_in(dir.path())
        );
    }
}

#[test]
fn a_stopped_stems_export_leaves_nothing_whether_rendering_or_encoding() {
    for at in [0.1, 0.7] {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().to_path_buf();
        let s = ExportSettings {
            scope: Scope::Stems,
            format: Format::Mp3,
            ..Default::default()
        };
        let p = project();
        let job = StemsJob {
            snapshot: snapshot(),
            options: StemOptions {
                render: RenderOptions {
                    tail_seconds: 20.0,
                    ..Default::default()
                },
                ..Default::default()
            },
            specs: specs(&p),
            units: Vec::new(),
            instruments: Vec::new(),
        };
        let f = folder.clone();
        cancel_at(at, move |ctl| {
            export_stems(job, &s, &Tags::default(), &f, &ctl)
        });
        assert!(
            files_in(&folder).is_empty(),
            "stopped at {at}: {:?}",
            files_in(&folder)
        );
    }
}

#[test]
fn a_file_already_there_is_replaced_only_when_the_new_one_is_whole() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.wav");
    std::fs::write(&path, b"the old file").unwrap();
    let a = common::music(44_100, 40.0);
    let p = path.clone();
    cancel_at(0.3, move |ctl| {
        export_rendered(
            a,
            &ExportSettings {
                format: Format::Ogg,
                ..Default::default()
            },
            &Tags::default(),
            &p,
            &ctl,
        )
    });
    assert_eq!(std::fs::read(&path).unwrap(), b"the old file");
    let ok = export_rendered(
        common::music(44_100, 1.0),
        &ExportSettings {
            format: Format::Wav,
            ..Default::default()
        },
        &Tags::default(),
        &path,
        &Control::new(),
    );
    assert!(ok.is_ok());
    assert_ne!(std::fs::read(&path).unwrap(), b"the old file");
}

/// A mix long enough to watch: the project with a long tail.
fn long_mix() -> MixJob {
    MixJob {
        snapshot: snapshot(),
        render: RenderOptions {
            tail_seconds: 30.0,
            ..Default::default()
        },
        units: Vec::new(),
        instruments: Vec::new(),
    }
}

#[test]
fn the_mix_render_fills_the_bar_and_stops_when_asked() {
    // The bar follows the render's frames (round 23): during the render the fraction
    // rises inside the render's share, then a stop ends the render at its next block.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("song.wav");
    let ctl = Control::new();
    let (c, p) = (ctl.clone(), path.clone());
    let job = std::thread::spawn(move || {
        export_mix(
            long_mix(),
            &ExportSettings::default(),
            &Tags::default(),
            &p,
            &c,
        )
    });
    let mut seen = Vec::new();
    let started = std::time::Instant::now();
    while !job.is_finished() && started.elapsed().as_secs() < 60 {
        if ctl.phase() == cantus_export::Phase::Render {
            let f = ctl.fraction();
            if f > 0.0 {
                seen.push(f);
            }
            if f > 0.1 {
                ctl.cancel();
                break;
            }
        }
        std::thread::yield_now();
    }
    let result = job.join().unwrap();
    assert!(
        seen.iter()
            .any(|f| *f > 0.0 && *f < cantus_export::job::MIX_RENDER_SHARE),
        "the render reported its progress: {seen:?}"
    );
    assert!(
        seen.windows(2).all(|w| w[0] <= w[1]),
        "and it only rises: {seen:?}"
    );
    assert!(
        matches!(result, Err(ExportError::Cancelled)),
        "a stop during the render stops it: {result:?}"
    );
    assert!(
        files_in(dir.path()).is_empty(),
        "{:?}",
        files_in(dir.path())
    );
}

#[test]
fn a_finished_mix_says_its_render_is_done() {
    let dir = tempfile::tempdir().unwrap();
    let ctl = Control::new();
    let job = MixJob {
        render: render_opts(),
        ..long_mix()
    };
    export_mix(
        job,
        &ExportSettings::default(),
        &Tags::default(),
        &dir.path().join("song.mp3"),
        &ctl,
    )
    .unwrap();
    assert_eq!(ctl.render.fraction(), 1.0);
    assert_eq!(ctl.fraction(), 1.0);
}

#[test]
fn a_stopped_stems_export_keeps_the_stems_of_the_last_one() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().to_path_buf();
    let s = ExportSettings {
        scope: Scope::Stems,
        format: Format::Wav,
        ..Default::default()
    };
    let p = project();
    let job = |tail: f64| StemsJob {
        snapshot: snapshot(),
        options: StemOptions {
            render: RenderOptions {
                tail_seconds: tail,
                ..Default::default()
            },
            ..Default::default()
        },
        specs: specs(&p),
        units: Vec::new(),
        instruments: Vec::new(),
    };
    let first = export_stems(job(0.5), &s, &Tags::default(), &folder, &Control::new()).unwrap();
    assert!(first.files.len() >= 2, "{:?}", first.files);
    let before: Vec<(String, Vec<u8>)> = files_in(&folder)
        .into_iter()
        .map(|n| {
            let bytes = std::fs::read(folder.join(&n)).unwrap();
            (n, bytes)
        })
        .collect();
    // Stopped while encoding the second stem (past the render's share of the bar).
    let (j, f, s2) = (job(20.0), folder.clone(), s.clone());
    cancel_at(0.85, move |ctl| {
        export_stems(j, &s2, &Tags::default(), &f, &ctl)
    });
    let after: Vec<(String, Vec<u8>)> = files_in(&folder)
        .into_iter()
        .map(|n| {
            let bytes = std::fs::read(folder.join(&n)).unwrap();
            (n, bytes)
        })
        .collect();
    assert_eq!(after, before, "the earlier stems are there, untouched");
}

/// A stem of the last export held open by another program (a player, without delete
/// sharing): the new export fails, and the folder is the last export's, every file of it.
#[cfg(windows)]
#[test]
fn a_stems_export_that_cannot_replace_one_file_replaces_none() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().to_path_buf();
    let s = ExportSettings {
        scope: Scope::Stems,
        format: Format::Wav,
        ..Default::default()
    };
    let p = project();
    let job = |tail: f64| StemsJob {
        snapshot: snapshot(),
        options: StemOptions {
            render: RenderOptions {
                tail_seconds: tail,
                ..Default::default()
            },
            ..Default::default()
        },
        specs: specs(&p),
        units: Vec::new(),
        instruments: Vec::new(),
    };
    let first = export_stems(job(0.5), &s, &Tags::default(), &folder, &Control::new()).unwrap();
    assert!(first.files.len() >= 2);
    let read_all = || -> Vec<(String, Vec<u8>)> {
        files_in(&folder)
            .into_iter()
            .map(|n| {
                let b = std::fs::read(folder.join(&n)).unwrap();
                (n, b)
            })
            .collect()
    };
    let before = read_all();
    // The last stem in the list is held open, read-only sharing: it cannot be moved.
    let held = first.files.last().unwrap().clone();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1) // FILE_SHARE_READ
        .open(&held)
        .unwrap();
    let r = export_stems(job(1.0), &s, &Tags::default(), &folder, &Control::new());
    assert!(r.is_err(), "the export says it failed");
    drop(lock);
    assert_eq!(
        read_all(),
        before,
        "the folder is the last export's, untouched"
    );
}

/// Fix review 3 #10 (and review 2 #2): an export told never to replace refuses, at its
/// commit, a file that appeared after it started — the file left as it was, no part left;
/// the same export with leave replaces it whole.
#[test]
fn never_replacing_refuses_a_file_that_appeared_and_leaves_it_whole() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mix.wav");
    let job = || MixJob {
        snapshot: snapshot(),
        render: render_opts(),
        units: Vec::new(),
        instruments: Vec::new(),
    };
    let mut s = ExportSettings {
        format: Format::Wav,
        no_replace: true,
        ..Default::default()
    };
    // The file appears between the agent's check and the commit (here, before the call).
    std::fs::write(&path, b"the user's").unwrap();
    let r = export_mix(job(), &s, &Tags::default(), &path, &Control::new());
    assert!(
        matches!(r, Err(ExportError::Io(ref e)) if e.kind() == std::io::ErrorKind::AlreadyExists),
        "{r:?}"
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"the user's");
    assert_eq!(
        files_in(dir.path()),
        vec!["mix.wav".to_string()],
        "no part left"
    );
    s.no_replace = false;
    export_mix(job(), &s, &Tags::default(), &path, &Control::new()).unwrap();
    assert_eq!(&std::fs::read(&path).unwrap()[..4], b"RIFF");
}
