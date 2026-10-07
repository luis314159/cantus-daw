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
