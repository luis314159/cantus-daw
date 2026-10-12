//! Stems from one offline pass (ADR 0021): with the master at unity, the strips that feed
//! the master and the tracks that go to it directly add up to the master **exactly**; a
//! latent insert does not move any stem; the tap changes nothing the mix does; a silent
//! strip is skipped unless it is asked for.

mod support;
use cantus_core::*;
use cantus_dsp::nulltest::peak;
use cantus_engine::stems::render_stems_with;
use cantus_engine::*;
use std::sync::Arc;
use support::*;

/// A pure delay of `n` samples that says so (as in `pdc.rs`).
#[derive(Debug)]
struct PureDelay {
    l: Vec<f32>,
    r: Vec<f32>,
    pos: usize,
}

impl FxUnit for PureDelay {
    fn configure(&mut self, _spec: &EffectSpec) {}
    fn process(&mut self, l: &mut [f32], r: &mut [f32], _key: Option<(&[f32], &[f32])>) {
        for i in 0..l.len() {
            let (ol, or) = (self.l[self.pos], self.r[self.pos]);
            self.l[self.pos] = l[i];
            self.r[self.pos] = r[i];
            self.pos = (self.pos + 1) % self.l.len();
            l[i] = ol;
            r[i] = or;
        }
    }
    fn reset(&mut self) {
        self.l.iter_mut().for_each(|x| *x = 0.0);
        self.r.iter_mut().for_each(|x| *x = 0.0);
    }
    fn latency(&self) -> usize {
        self.l.len()
    }
}

fn opts(block: usize) -> StemOptions {
    StemOptions {
        render: RenderOptions {
            tail_seconds: 0.25,
            block_frames: block,
            ..Default::default()
        },
        keep_silent: false,
        ..Default::default()
    }
}

/// Every strip, the direct-to-master sum and the master.
fn every_stem(p: &Project) -> Vec<StemSpec> {
    let mut specs: Vec<StemSpec> = (0..p.strips.len())
        .map(|i| StemSpec {
            source: StemSource::Strip(i),
            file_name: stem_file_name(i + 1, &format!("Strip {}", i + 1)),
        })
        .collect();
    specs.push(StemSpec {
        source: StemSource::Direct,
        file_name: "00 Direct.wav".into(),
    });
    specs.push(StemSpec {
        source: StemSource::Master,
        file_name: "00 Master.wav".into(),
    });
    specs
}

/// The demo, plus a pad track straight to the master and a reverb return fed by a send.
fn routed_project() -> Project {
    let mut p = demo_project();
    let mut direct = p.make_track("Direct");
    direct.strip = None;
    let mut c = p.make_clip("Stabs", Ticks::beats(1), Ticks::beats(6));
    for i in 0..6 {
        c.notes
            .push(p.make_note(Ticks::beats(i), Ticks::EIGHTH, 72 + i as u8, 90));
    }
    direct.clips.push(c);
    p.tracks.push(direct);
    // A reverb on the last strip, fed by a send from the lead's strip. The plain reverb,
    // not Spatium: Spatium's lines are modulated, so a copy of the song that starts a
    // hundred samples later meets a different modulation phase and cannot be compared to
    // the bit — which is the reverb's business, not the stems'.
    let ret = p.strips[15].id;
    let space = p.make_effect(EffectKind::Reverb);
    p.strips[15].effects.push(space);
    p.strips[15].gain_db = -2.0;
    strip_of(&mut p, 0).sends.push(Send {
        to: ret,
        level: 0.5,
        enabled: true,
        tap: cantus_core::SendTap::Post,
    });
    p
}

fn sum_to_master(p: &Project, stems: &[Stem]) -> (Vec<f32>, Vec<f32>) {
    let snap = Snapshot::compile(p, 1);
    let master = stems
        .iter()
        .find(|s| s.spec.source == StemSource::Master)
        .unwrap()
        .samples
        .clone();
    // In the order the engine mixes: the direct tracks first, then the strips that feed
    // the master, in processing order. f32 addition in the same order is the same sum.
    let mut sum = stems
        .iter()
        .find(|s| s.spec.source == StemSource::Direct)
        .unwrap()
        .samples
        .clone();
    for &i in &snap.strip_order {
        if snap.strips[i].output.is_some() {
            continue;
        }
        let stem = stems
            .iter()
            .find(|s| s.spec.source == StemSource::Strip(i))
            .unwrap();
        for (a, b) in sum.iter_mut().zip(&stem.samples) {
            *a += *b;
        }
    }
    (sum, master)
}

#[test]
fn the_stems_that_feed_the_master_add_up_to_it_exactly_and_the_master_is_the_bounce() {
    let p = routed_project();
    let stems = render_stems(
        Arc::new(Snapshot::compile(&p, 1)),
        &opts(512),
        &every_stem(&p),
    )
    .unwrap();
    let (sum, master) = sum_to_master(&p, &stems);
    assert!(peak(&master) > 0.01, "the mix makes sound");
    assert_eq!(sum.len(), master.len());
    let worst = sum
        .iter()
        .zip(&master)
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    assert_eq!(worst, 0.0, "stems and master differ by {worst}");
    // The master stem is the bounce, to the bit: the tap reads and never writes.
    let bounce = render_of(&p);
    assert_eq!(master, bounce, "a stems pass changes the mix");
    // The return carries the reverb, the direct sum carries the pad track.
    let ret = stems
        .iter()
        .find(|s| s.spec.source == StemSource::Strip(15))
        .unwrap();
    assert!(ret.peak > 0.0 && !ret.silent);
    assert!(
        stems.iter().filter(|s| s.silent).count() >= 12,
        "empty strips are silent"
    );
}

#[test]
fn with_a_bus_the_bus_carries_its_inputs_and_the_master_sum_still_holds() {
    let mut p = routed_project();
    // Lead → strip 3 (a bus) → master.
    let bus = p.strips[2].id;
    strip_of(&mut p, 0).output = Some(bus);
    p.strips[2].gain_db = -1.5;
    let stems = render_stems(
        Arc::new(Snapshot::compile(&p, 1)),
        &opts(333),
        &every_stem(&p),
    )
    .unwrap();
    let (sum, master) = sum_to_master(&p, &stems);
    assert_eq!(sum, master);
    assert_eq!(master, render_with(&p, 333));
}

fn with_delay(p: &Project, latent: &[EffectId], delay: usize, block: usize) -> Vec<Stem> {
    render_stems_with(
        Arc::new(Snapshot::compile(p, 1)),
        &opts(block),
        &every_stem(p),
        &mut |slot, sr| {
            if latent.contains(&slot.id) {
                EffectUnit::from_fx(
                    slot,
                    Box::new(PureDelay {
                        l: vec![0.0; delay],
                        r: vec![0.0; delay],
                        pos: 0,
                    }),
                )
            } else {
                EffectUnit::new(slot, sr)
            }
        },
    )
    .unwrap()
}

#[test]
fn a_latent_insert_moves_no_stem_and_the_sum_still_holds() {
    let reference = routed_project();
    let dry = render_stems(
        Arc::new(Snapshot::compile(&reference, 1)),
        &opts(512),
        &every_stem(&reference),
    )
    .unwrap();
    for delay in [100usize, 16_884] {
        // The same project with a pure delay on the lead's strip: the lead's stem, every
        // other stem and the master all come out where they were, to the bit.
        let mut p = reference.clone();
        let slot = p.make_effect(EffectKind::Delay);
        let id = slot.id;
        strip_of(&mut p, 0).effects.push(slot);
        let wet = with_delay(&p, &[id], delay, 512);
        for (a, b) in dry.iter().zip(&wet) {
            assert_eq!(a.samples.len(), b.samples.len(), "{}", a.spec.file_name);
            assert!(
                a.samples == b.samples,
                "{} moved with a {delay}-sample insert",
                a.spec.file_name
            );
        }
        let (sum, master) = sum_to_master(&p, &wet);
        assert_eq!(sum, master, "delay {delay}");
    }
}

#[test]
fn stems_do_not_depend_on_the_block_size() {
    let p = routed_project();
    let reference = render_stems(
        Arc::new(Snapshot::compile(&p, 1)),
        &opts(64),
        &every_stem(&p),
    )
    .unwrap();
    for block in [7usize, 1000, 4096] {
        let other = render_stems(
            Arc::new(Snapshot::compile(&p, 1)),
            &opts(block),
            &every_stem(&p),
        )
        .unwrap();
        for (a, b) in reference.iter().zip(&other) {
            assert!(
                a.samples == b.samples,
                "{} at block {block}",
                a.spec.file_name
            );
        }
    }
}

#[test]
fn track_stems_are_what_each_track_sends_into_its_strip() {
    let mut p = routed_project();
    let specs: Vec<StemSpec> = (0..p.tracks.len())
        .map(|i| StemSpec {
            source: StemSource::Track(i),
            file_name: stem_file_name(i + 1, &p.tracks[i].name),
        })
        .collect();
    let before = render_stems(Arc::new(Snapshot::compile(&p, 1)), &opts(256), &specs).unwrap();
    assert!(before.iter().all(|s| !s.silent));
    // The strips can do anything: a track's stem is taken before them.
    let s = strip_of(&mut p, 1);
    s.gain_db = -12.0;
    s.pan = 0.9;
    s.phase_invert = true;
    let comp = p.make_effect(EffectKind::Compressor);
    strip_of(&mut p, 1).effects.push(comp);
    let after = render_stems(Arc::new(Snapshot::compile(&p, 1)), &opts(256), &specs).unwrap();
    assert_eq!(before, after);
    // The track's own level is after the instrument and before the stem.
    p.tracks[1].gain_db = -6.0;
    let quieter = render_stems(Arc::new(Snapshot::compile(&p, 1)), &opts(256), &specs).unwrap();
    assert!(quieter[1].peak < before[1].peak * 0.51 && quieter[1].peak > before[1].peak * 0.49);
    assert_eq!(quieter[0], before[0]);
}

#[test]
fn a_silent_strip_is_skipped_unless_asked_for_and_files_are_named_nn_name() {
    let p = routed_project();
    let dir = tempfile::tempdir().unwrap();
    let specs: Vec<StemSpec> = (0..p.strips.len())
        .map(|i| StemSpec {
            source: StemSource::Strip(i),
            file_name: stem_file_name(i + 1, &format!("Strip {}", i + 1)),
        })
        .collect();
    let done = render_stems_to_folder(
        Arc::new(Snapshot::compile(&p, 1)),
        &opts(512),
        &specs,
        dir.path(),
        &mut |_| true,
    )
    .unwrap();
    let names: Vec<String> = done
        .written
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        vec!["01 Strip 1.wav", "02 Strip 2.wav", "16 Strip 16.wav"]
    );
    assert_eq!(done.silent.len(), 13);
    assert!(!dir.path().join("03 Strip 3.wav").exists());
    let frames = |path: &std::path::Path| {
        let r = hound::WavReader::open(path).unwrap();
        assert_eq!(r.spec().channels, 2);
        assert_eq!(r.spec().sample_format, hound::SampleFormat::Float);
        r.duration() as usize
    };
    let n = frames(&done.written[0]);
    assert_eq!(n, (done.seconds * 48_000.0).round() as usize);
    assert!(
        done.written.iter().all(|p| frames(p) == n),
        "every stem is as long"
    );

    let kept = tempfile::tempdir().unwrap();
    let done = render_stems_to_folder(
        Arc::new(Snapshot::compile(&p, 1)),
        &StemOptions {
            keep_silent: true,
            ..opts(512)
        },
        &specs,
        kept.path(),
        &mut |_| true,
    )
    .unwrap();
    assert_eq!(done.written.len(), 16);
    assert!(done.silent.is_empty());
    let silent = kept.path().join("03 Strip 3.wav");
    assert_eq!(frames(&silent), n);
    let mut r = hound::WavReader::open(&silent).unwrap();
    assert!(r.samples::<f32>().all(|s| s.unwrap() == 0.0));
}

#[test]
fn a_stopped_export_leaves_nothing_behind() {
    let p = routed_project();
    let dir = tempfile::tempdir().unwrap();
    let mut calls = 0;
    let result = render_stems_to_folder(
        Arc::new(Snapshot::compile(&p, 1)),
        &opts(512),
        &every_stem(&p),
        dir.path(),
        &mut |_| {
            calls += 1;
            calls < 40
        },
    );
    assert!(matches!(result, Err(StemsError::Cancelled)));
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn stem_names_are_numbered_and_safe_on_every_file_system() {
    assert_eq!(stem_file_name(3, "Bass"), "03 Bass.wav");
    assert_eq!(stem_file_name(12, "AC/DC: live?"), "12 AC_DC_ live_.wav");
    assert_eq!(stem_file_name(5, "  "), "05.wav");
    assert_eq!(stem_file_name(0, "Master"), "00 Master.wav");
    assert_eq!(stem_file_name(7, "Pads..."), "07 Pads.wav");
}
