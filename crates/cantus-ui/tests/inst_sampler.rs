//! Exemplar's editor and importers: it opens in every view and paints its name and its
//! map; an import, a preset and a dropped file are one undo step each; SFZ, Decent
//! Sampler, SoundFont and WAV metadata are read as the formats say; SFZ export round-trips.

mod support;
use cantus_core::inst::sampler::{self as model, gf, zf, Zone};
use cantus_core::*;
use cantus_ui::views::inst::sampler::{self as smp, ds, import, sf2, sfz};
use cantus_ui::views::inst::View;
use std::path::Path;
use support::*;

fn with_sampler() -> (cantus_ui::CantusApp, TrackId) {
    let mut a = app(cantus_ui::starter_project());
    let track = a.project().tracks[0].id;
    a.set_instrument(
        track,
        InstrumentSpec::Native(NativeSpec::new(InstrumentKind::Sampler)),
    );
    (a, track)
}

fn spec(a: &cantus_ui::CantusApp, track: TrackId) -> NativeSpec {
    a.native_of(track).expect("a native instrument")
}

fn paint_inst(app: &mut cantus_ui::CantusApp) -> String {
    let ctx = ctx();
    paint(&ctx, egui::vec2(1400.0, 1000.0), 3, |ui| {
        cantus_ui::views::inst::window_in(app, ui);
    })
}

#[test]
fn the_editor_opens_in_every_view_and_shows_its_map_and_seam() {
    let (mut a, track) = with_sampler();
    cantus_ui::views::inst::demo_setup(&mut a, track, InstrumentKind::Sampler);
    cantus_ui::views::inst::demo_feed(&mut a, InstrumentKind::Sampler);
    a.open_inst_editor(track);
    let name = cantus_ui::i18n::t("instrument.sampler");
    for view in [View::Play, View::Edit, View::Matrix] {
        a.inst.view = view;
        let text = paint_inst(&mut a);
        assert!(
            text.to_uppercase().contains(&name.to_uppercase()),
            "{view:?}: no name in {text}"
        );
        if view == View::Edit {
            let pad = cantus_ui::i18n::t("smp.gen.pad");
            // In full, perhaps on two lines in a narrow zone.
            assert!(
                pad.split(' ').all(|w| text.contains(w)),
                "the map labels its zones: {text}"
            );
            assert!(text.contains("dB"), "the seam prints its error in dB");
        }
    }
    // An empty instrument invites a drop and says what it plays.
    let (mut b, track) = with_sampler();
    b.open_inst_editor(track);
    b.inst.view = View::Edit;
    let text = paint_inst(&mut b);
    assert!(
        text.contains(&cantus_ui::i18n::t("smp.drop_here")),
        "{text}"
    );
}

fn three_zone_import() -> import::Import {
    let mut imp = import::Import::default();
    for (i, root) in [48u8, 60, 72].iter().enumerate() {
        let mut z = Zone::new(Some(format!("C:/lib/s{i}.wav")), root - 6, root + 5, *root);
        z.set(zf::MODE, 3.0).set(zf::GROUP, 1.0);
        imp.zones.push(z);
    }
    let mut g = model::Group::default();
    g.values[gf::RELEASE] = Some(40.0);
    imp.groups.push((1, g));
    imp.values.push(("amp.attack".into(), 5.0));
    imp
}

#[test]
fn an_import_is_one_undo_step_and_replaces_the_zones() {
    let (mut a, track) = with_sampler();
    a.set_inst_value(track, "z9.gen", 2.0, false);
    a.set_inst_value(track, "gain", -3.0, false);
    let before = spec(&a, track);
    smp::apply_import(&mut a, track, three_zone_import());
    let s = spec(&a, track);
    let zones = model::zones(&s);
    assert_eq!(zones.len(), 3, "the old zone went, three came");
    assert_eq!(s.get("g1.release"), 40.0);
    assert_eq!(s.get("amp.attack"), 5.0);
    assert_eq!(s.get("gain"), -3.0, "what the import does not name stays");
    a.undo();
    assert_eq!(spec(&a, track), before);
}

#[test]
fn dragging_a_zone_is_one_undo_step() {
    let (mut a, track) = with_sampler();
    smp::apply_import(&mut a, track, three_zone_import());
    let before = spec(&a, track);
    // A drag: many frames, each moving the zone a key further, merged into one step.
    for dk in 1..=5 {
        let s = spec(&a, track);
        let (lo, hi) = (42.0 + dk as f32, 53.0 + dk as f32);
        smp::set_zone_fields(&mut a, track, &s, 0, &[(zf::LO, lo), (zf::HI, hi)], true);
    }
    a.end_gesture();
    let moved = model::Zone::of(&spec(&a, track), 0).unwrap();
    assert_eq!((moved.lo(), moved.hi()), (47, 58));
    a.undo();
    assert_eq!(spec(&a, track), before, "the whole drag undoes at once");
    // Setting a field back to its default clears it rather than storing it.
    let s = spec(&a, track);
    smp::set_zone_fields(&mut a, track, &s, 0, &[(zf::MODE, 0.0)], false);
    assert!(!spec(&a, track).is_stored("z0.mode"));
}

#[test]
fn every_factory_preset_sets_what_it_names() {
    for p in smp::FACTORY_PRESETS {
        let (mut a, track) = with_sampler();
        let target = cantus_ui::views::inst::preset_spec(InstrumentKind::Sampler, p);
        a.apply_inst_spec(track, &target, "cmd.fx_preset");
        let s = spec(&a, track);
        for (k, v) in p.values {
            assert_eq!(s.get(k), *v, "{}: {k}", p.label_key);
        }
        assert!(!model::zones(&s).is_empty(), "{} has zones", p.label_key);
        assert!(
            s.text("macro1.name").is_some(),
            "{} names its macros",
            p.label_key
        );
        let label = cantus_ui::i18n::t(p.label_key);
        assert_ne!(label, p.label_key, "{} has a name", p.label_key);
    }
    assert!((8..=16).contains(&smp::FACTORY_PRESETS.len()));
}

#[test]
fn dropped_files_become_zones_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let spec_wav = hound::WavSpec {
        channels: 1,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut paths = Vec::new();
    for name in [
        "Harp_C3_mf.wav",
        "Harp_C3_ff.wav",
        "Harp_G3_mf.wav",
        "Harp_G3_ff.wav",
    ] {
        let p = dir.path().join(name);
        let mut w = hound::WavWriter::create(&p, spec_wav).unwrap();
        for i in 0..1_000 {
            w.write_sample(((i as f32 * 0.1).sin() * 8_000.0) as i16)
                .unwrap();
        }
        w.finalize().unwrap();
        paths.push(p);
    }
    let (mut a, track) = with_sampler();
    smp::drop_files(&mut a, track, &paths, 60);
    let zones = model::zones(&spec(&a, track));
    assert_eq!(zones.len(), 4);
    let c3: Vec<&Zone> = zones
        .iter()
        .map(|(_, z)| z)
        .filter(|z| z.root() == 48)
        .collect();
    assert_eq!(c3.len(), 2, "two layers rooted on C3");
    assert!(
        c3.iter().any(|z| z.vhi() < 127) && c3.iter().any(|z| z.vlo() > 1),
        "split by velocity"
    );
    let g3 = zones
        .iter()
        .map(|(_, z)| z)
        .find(|z| z.root() == 55)
        .unwrap();
    assert_eq!(
        (g3.lo(), g3.hi()),
        (52, 127),
        "keys split halfway between roots"
    );
    a.undo();
    assert!(model::zones(&spec(&a, track)).is_empty());
    // One file lands where it is dropped.
    smp::drop_files(&mut a, track, &paths[..1], 64);
    let zones = model::zones(&spec(&a, track));
    assert_eq!((zones[0].1.lo(), zones[0].1.hi()), (64, 64));
}

const SFZ: &str = r#"
// A test instrument.
#define $REL 0.4
<control> default_path=Samples/ note_offset=0
<global> ampeg_release=$REL bend_up=200 cutoff=1500 fil_type=lpf_2p resonance=6
/* a block
   comment */
<group> seq_length=2 amp_veltrack=80
<region> sample=Piano C4 soft.wav lokey=c4 hikey=e4 pitch_keycenter=c4 hivel=80 xfout_lovel=60 xfout_hivel=80 seq_position=1
<region> sample=Piano C4 loud.wav lokey=60 hikey=64 pitch_keycenter=60 lovel=60 xfin_lovel=60 xfin_hivel=80 seq_position=2 volume=-3 tune=-7
<group> trigger=release rt_decay=6 ampeg_release=0.1
<region> sample=rel.wav key=60 group=2 off_by=3 width=50
<region> sample=*saw lokey=70 hikey=90 pitch_keycenter=69 loop_mode=loop_sustain loop_start=10 loop_end=2000 loop_crossfade=0.02 direction=reverse
"#;

fn parse_sfz(text: &str) -> import::Import {
    let none = |_: &str| None;
    let meta = |_: &str| None;
    sfz::parse(text, Path::new("C:/lib"), &none, &meta)
}

#[test]
fn sfz_maps_the_priority_opcodes_and_logs_the_rest() {
    let imp = parse_sfz(SFZ);
    assert_eq!(imp.zones.len(), 4);
    let soft = &imp.zones[0];
    assert_eq!(
        soft.sample.as_deref(),
        Some("C:/lib/Samples/Piano C4 soft.wav"),
        "spaces survive"
    );
    assert_eq!((soft.lo(), soft.hi(), soft.root()), (60, 64, 60));
    assert_eq!((soft.vlo(), soft.vhi(), soft.get(zf::VFOUT)), (1, 80, 20.0));
    assert_eq!((soft.get(zf::SEQ), soft.get(zf::SEQLEN)), (1.0, 2.0));
    let loud = &imp.zones[1];
    assert_eq!((loud.vlo(), loud.get(zf::VFIN)), (60, 20.0));
    assert_eq!((loud.get(zf::GAIN), loud.get(zf::TUNE)), (-3.0, -7.0));
    let rel = &imp.zones[2];
    assert_eq!(rel.get(zf::TRIG), 1.0);
    assert_eq!(
        (rel.get(zf::RTDECAY), rel.get(zf::EXCL), rel.get(zf::OFFBY)),
        (6.0, 2.0, 3.0)
    );
    let saw = &imp.zones[3];
    assert_eq!(saw.get(zf::GEN), 2.0, "*saw is the built-in saw");
    assert_eq!(
        (saw.get(zf::MODE), saw.get(zf::LS), saw.get(zf::LE)),
        (3.0, 10.0, 2000.0)
    );
    assert!((saw.get(zf::XF) - 20.0).abs() < 1e-3 && saw.get(zf::REV) == 1.0);
    // Groups carry the envelopes: the define reached the release.
    let g0 = &imp.groups[soft.get(zf::GROUP) as usize].1;
    assert!((g0.values[gf::RELEASE].unwrap() - 400.0).abs() < 1e-3);
    assert_eq!(g0.values[gf::VEL], Some(80.0));
    let g1 = &imp.groups[rel.get(zf::GROUP) as usize].1;
    assert!((g1.values[gf::RELEASE].unwrap() - 100.0).abs() < 1e-3);
    // The instrument-wide opcodes.
    let v = |k: &str| imp.values.iter().find(|(key, _)| key == k).map(|(_, v)| *v);
    assert_eq!(v("bend"), Some(2.0));
    assert_eq!(
        (v("filter.type"), v("filter.cutoff")),
        (Some(1.0), Some(1500.0))
    );
    assert!(imp.log.iter().any(|l| l.contains("width")), "{:?}", imp.log);
}

#[test]
fn sfz_export_round_trips() {
    let imp = parse_sfz(SFZ);
    let (mut a, track) = with_sampler();
    smp::apply_import(&mut a, track, imp);
    let s = spec(&a, track);
    // Relative to the library, as a real export writes them: `C:/lib/…` is absolute on
    // Windows only, so written whole it would be joined to the library again on Linux.
    let text = sfz::export(&s, &|p: &str| {
        p.strip_prefix("C:/lib/").unwrap_or(p).to_string()
    });
    let back = parse_sfz(&text);
    let (mut b, track_b) = with_sampler();
    smp::apply_import(&mut b, track_b, back);
    let original: Vec<Zone> = model::zones(&s).into_iter().map(|(_, z)| z).collect();
    let again: Vec<Zone> = model::zones(&spec(&b, track_b))
        .into_iter()
        .map(|(_, z)| z)
        .collect();
    assert_eq!(original.len(), again.len());
    for (x, y) in original.iter().zip(&again) {
        assert_eq!(x.sample, y.sample);
        for f in [
            zf::LO,
            zf::HI,
            zf::ROOT,
            zf::VLO,
            zf::VHI,
            zf::TUNE,
            zf::GAIN,
            zf::MODE,
            zf::LS,
            zf::LE,
            zf::TRIG,
            zf::SEQ,
            zf::SEQLEN,
            zf::EXCL,
            zf::OFFBY,
            zf::VFIN,
            zf::VFOUT,
            zf::GEN,
        ] {
            assert!(
                (x.get(f) - y.get(f)).abs() < 1e-3,
                "field {} differs: {} vs {}",
                model::zone_field_name(f),
                x.get(f),
                y.get(f)
            );
        }
    }
}

const DSPRESET: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<DecentSampler minVersion="1.0.0">
  <groups volume="-6dB" globalPan="10" attack="0.01" release="0.5">
    <group name="Sustain" groupVolume="0.5" groupPan="-20" seqMode="round_robin" seqLength="2" tags="sus">
      <sample path="Samples/c4_1.wav" rootNote="60" loNote="58" hiNote="62" seqPosition="1" volume="-1dB" pan="5" loopEnabled="true" loopStart="100" loopEnd="900" loopCrossfade="441"/>
      <sample path="Samples/c4_2.wav" rootNote="C4" loNote="58" hiNote="62" seqPosition="2"/>
    </group>
    <group name="Release" trigger="release" silencedByTags="sus" release="0.2">
      <sample path="Samples/rel.wav" rootNote="60" loNote="0" hiNote="127"/>
    </group>
  </groups>
  <ui><tab><labeled-knob/></tab></ui>
</DecentSampler>"#;

#[test]
fn decent_sampler_multiplies_volume_adds_pan_and_maps_tags() {
    let meta = |_: &str| None;
    let imp = ds::parse(DSPRESET, Path::new("C:/lib"), &meta).unwrap();
    assert_eq!(imp.zones.len(), 3);
    let a = &imp.zones[0];
    // −6 dB × 0.5 (−6.02 dB) × −1 dB.
    assert!(
        (a.get(zf::GAIN) - (-6.0 - 6.0206 - 1.0)).abs() < 0.01,
        "{}",
        a.get(zf::GAIN)
    );
    assert_eq!(a.get(zf::PAN), 10.0 - 20.0 + 5.0, "pans add");
    assert_eq!((a.get(zf::SEQ), a.get(zf::SEQLEN)), (1.0, 2.0));
    assert_eq!(
        (a.get(zf::MODE), a.get(zf::LS), a.get(zf::LE)),
        (2.0, 100.0, 900.0)
    );
    assert!(
        (a.get(zf::XF) - 10.0).abs() < 0.01,
        "441 frames at 44.1 kHz: {}",
        a.get(zf::XF)
    );
    assert_eq!(imp.zones[1].root(), 60, "note names work too");
    let rel = &imp.zones[2];
    assert_eq!(rel.get(zf::TRIG), 1.0);
    assert_eq!(
        rel.get(zf::OFFBY),
        a.get(zf::EXCL),
        "silencedByTags names the tag's class"
    );
    assert!(a.get(zf::EXCL) > 0.0);
    let g = |i: usize| imp.groups.iter().find(|(m, _)| *m == i).unwrap().1.clone();
    assert!(
        (g(0).values[gf::RELEASE].unwrap() - 500.0).abs() < 1e-3,
        "inherited from <groups>"
    );
    assert!((g(1).values[gf::RELEASE].unwrap() - 200.0).abs() < 1e-3);
    assert!(imp.log.iter().any(|l| l.contains("ui")));
}

fn riff(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = id.to_vec();
    v.extend((body.len() as u32).to_le_bytes());
    v.extend(body);
    if body.len() % 2 == 1 {
        v.push(0);
    }
    v
}

fn list(kind: &[u8; 4], chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut body = kind.to_vec();
    for c in chunks {
        body.extend(c);
    }
    riff(b"LIST", &body)
}

fn name20(s: &str) -> Vec<u8> {
    let mut v = s.as_bytes().to_vec();
    v.resize(20, 0);
    v
}

/// A one-preset, one-instrument, one-sample SoundFont.
fn tiny_sf2(frames: usize) -> Vec<u8> {
    let mut smpl = Vec::new();
    for i in 0..frames + 46 {
        let v = if i < frames {
            ((i as f32 * 0.2).sin() * 10_000.0) as i16
        } else {
            0
        };
        smpl.extend(v.to_le_bytes());
    }
    let gen = |op: u16, amount: i16| {
        let mut v = op.to_le_bytes().to_vec();
        v.extend(amount.to_le_bytes());
        v
    };
    let phdr = |name: &str, bag: u16| {
        let mut v = name20(name);
        v.extend(0u16.to_le_bytes());
        v.extend(0u16.to_le_bytes());
        v.extend(bag.to_le_bytes());
        v.extend([0u8; 12]);
        v
    };
    let inst = |name: &str, bag: u16| {
        let mut v = name20(name);
        v.extend(bag.to_le_bytes());
        v
    };
    let bag = |g: u16| {
        let mut v = g.to_le_bytes().to_vec();
        v.extend(0u16.to_le_bytes());
        v
    };
    let key_range = (48u16 | (72u16 << 8)) as i16;
    let igens = [
        gen(43, key_range),
        gen(58, 60),
        gen(54, 1),
        gen(48, 100),
        gen(38, 0),
        gen(53, 0),
        gen(0, 0),
    ]
    .concat();
    let mut shdr = name20("Sine");
    for v in [0u32, frames as u32, 100, 612, 44_100] {
        shdr.extend(v.to_le_bytes());
    }
    shdr.extend([60u8, 0]);
    shdr.extend(0u16.to_le_bytes());
    shdr.extend(1u16.to_le_bytes());
    shdr.extend(name20("EOS"));
    shdr.extend([0u8; 26]);
    let pdta = list(
        b"pdta",
        &[
            riff(b"phdr", &[phdr("Pad", 0), phdr("EOP", 1)].concat()),
            riff(b"pbag", &[bag(0), bag(1)].concat()),
            riff(b"pmod", &[0u8; 10]),
            riff(b"pgen", &[gen(41, 0), gen(0, 0)].concat()),
            riff(b"inst", &[inst("Inst", 0), inst("EOI", 1)].concat()),
            riff(b"ibag", &[bag(0), bag(6)].concat()),
            riff(b"imod", &[0u8; 10]),
            riff(b"igen", &igens),
            riff(b"shdr", &shdr),
        ],
    );
    let info = list(
        b"INFO",
        &[riff(b"ifil", &[2, 0, 1, 0]), riff(b"INAM", b"Tiny\0\0")],
    );
    let sdta = list(b"sdta", &[riff(b"smpl", &smpl)]);
    let mut body = b"sfbk".to_vec();
    body.extend(info);
    body.extend(sdta);
    body.extend(pdta);
    riff(b"RIFF", &body)
}

#[test]
fn a_soundfont_preset_becomes_zones_and_wav_files() {
    let sf = sf2::parse(&tiny_sf2(2_000)).unwrap();
    assert_eq!(sf.name, "Tiny");
    assert_eq!(
        (sf.presets.len(), sf.instruments.len(), sf.samples.len()),
        (1, 1, 1)
    );
    let dir = tempfile::tempdir().unwrap();
    let imp = sf2::to_import(&sf, 0, dir.path()).unwrap();
    assert_eq!(imp.zones.len(), 1);
    let z = &imp.zones[0];
    assert_eq!((z.lo(), z.hi(), z.root()), (48, 72, 60));
    assert_eq!(
        (z.get(zf::MODE), z.get(zf::LS), z.get(zf::LE)),
        (2.0, 100.0, 611.0),
        "endloop is exclusive in SF2"
    );
    assert!(
        (z.get(zf::GAIN) + 4.0).abs() < 1e-4,
        "100 cB at FluidSynth's 0.4: {}",
        z.get(zf::GAIN)
    );
    let g = &imp.groups[0].1;
    assert!(
        (g.values[gf::RELEASE].unwrap() - 1_000.0).abs() < 0.01,
        "0 timecents is one second"
    );
    let wav = hound::WavReader::open(z.sample.as_ref().unwrap()).unwrap();
    assert_eq!((wav.spec().sample_rate, wav.len()), (44_100, 2_000));
}

#[test]
fn wav_metadata_gives_root_fine_tune_and_loop() {
    let mut fmt = Vec::new();
    fmt.extend(1u16.to_le_bytes());
    fmt.extend(1u16.to_le_bytes());
    fmt.extend(48_000u32.to_le_bytes());
    fmt.extend(96_000u32.to_le_bytes());
    fmt.extend(2u16.to_le_bytes());
    fmt.extend(16u16.to_le_bytes());
    let mut smpl = vec![0u8; 36];
    smpl[12..16].copy_from_slice(&62u32.to_le_bytes());
    smpl[16..20].copy_from_slice(&(u32::MAX / 4).to_le_bytes());
    smpl[28..32].copy_from_slice(&1u32.to_le_bytes());
    let mut lp = vec![0u8; 24];
    lp[8..12].copy_from_slice(&1_000u32.to_le_bytes());
    lp[12..16].copy_from_slice(&5_000u32.to_le_bytes());
    smpl.extend(lp);
    let mut body = b"WAVE".to_vec();
    body.extend(riff(b"fmt ", &fmt));
    body.extend(riff(b"smpl", &smpl));
    body.extend(riff(b"data", &vec![0u8; 20_000]));
    let m = import::meta_of_bytes(&riff(b"RIFF", &body)).unwrap();
    assert_eq!((m.rate, m.channels, m.frames), (48_000, 1, 10_000));
    assert_eq!(m.root, Some(62));
    assert!((m.fine_ct - 25.0).abs() < 0.01);
    assert_eq!(m.sustain_loop, Some((1_000, 5_000, false)));
    assert_eq!(import::parse_note("c#4"), Some(61));
    assert_eq!(import::parse_note("Bb-1"), Some(10));
    assert_eq!(import::name_hints("Violin_A3_ff_rr2").0, Some(57));
}

/// Round 26: reading a file's metadata skips its audio — a `smpl` chunk after a large
/// `data` chunk is still found, and a FLAC (no chunks) is answered from its first bytes.
#[test]
fn file_metadata_skips_the_audio_and_finds_chunks_after_it() {
    let mut fmt = Vec::new();
    fmt.extend(1u16.to_le_bytes());
    fmt.extend(2u16.to_le_bytes());
    fmt.extend(44_100u32.to_le_bytes());
    fmt.extend(176_400u32.to_le_bytes());
    fmt.extend(4u16.to_le_bytes());
    fmt.extend(16u16.to_le_bytes());
    let mut smpl = vec![0u8; 36];
    smpl[12..16].copy_from_slice(&50u32.to_le_bytes());
    smpl[28..32].copy_from_slice(&1u32.to_le_bytes());
    let mut lp = vec![0u8; 24];
    lp[8..12].copy_from_slice(&700u32.to_le_bytes());
    lp[12..16].copy_from_slice(&90_000u32.to_le_bytes());
    smpl.extend(lp);
    let mut body = b"WAVE".to_vec();
    body.extend(riff(b"fmt ", &fmt));
    body.extend(riff(b"data", &vec![0u8; 400_000]));
    body.extend(riff(b"smpl", &smpl));
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("late-smpl.wav");
    std::fs::write(&wav, riff(b"RIFF", &body)).unwrap();
    let m = import::meta_of(&wav).unwrap();
    assert_eq!((m.rate, m.channels, m.frames), (44_100, 2, 100_000));
    assert_eq!(m.root, Some(50));
    assert_eq!(m.sustain_loop, Some((700, 90_000, false)));
    let flac = dir.path().join("a.flac");
    std::fs::write(&flac, [&b"fLaC"[..], &[0u8; 64][..]].concat()).unwrap();
    assert!(import::meta_of(&flac).is_none());
}

// ---- the look (ADR 0024): words in their boxes, a drag on a cell -----------------------------

/// Every straight text a frame painted: its string, where its letters are, and the clip it
/// was painted under.
fn texts(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Rect, egui::Rect)> {
    fn walk(
        shape: &egui::Shape,
        clip: egui::Rect,
        out: &mut Vec<(String, egui::Rect, egui::Rect)>,
    ) {
        match shape {
            egui::Shape::Text(t) if t.angle == 0.0 => {
                let r = egui::Rect::from_min_size(t.pos, t.galley.size());
                out.push((t.galley.text().to_string(), r, clip));
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for s in shapes {
        walk(&s.shape, s.clip_rect, &mut out);
    }
    out
}

/// Run frames of the editor with `events` in them, and return the last frame's shapes.
fn run(
    ctx: &egui::Context,
    app: &mut cantus_ui::CantusApp,
    size: egui::Vec2,
    frames: Vec<Vec<egui::Event>>,
) -> Vec<egui::epaint::ClippedShape> {
    let mut shapes = Vec::new();
    for (i, events) in frames.into_iter().enumerate() {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
            time: Some(i as f64 * 0.05),
            events,
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            cantus_ui::views::inst::window_in(app, ui);
        });
        out.textures_delta.clear();
        shapes = out.shapes;
    }
    shapes
}

/// Nothing elided, nothing cut by its clip, no two words on top of each other: the texts
/// whose centre lies in `area`.
fn assert_fits(texts: &[(String, egui::Rect, egui::Rect)], area: egui::Rect, what: &str) {
    let inside: Vec<&(String, egui::Rect, egui::Rect)> = texts
        .iter()
        .filter(|(s, r, clip)| {
            !s.trim().is_empty() && area.contains(r.center()) && clip.contains(r.center())
        })
        .collect();
    assert!(
        inside.len() > 20,
        "{what}: only {} texts painted",
        inside.len()
    );
    for (s, r, clip) in &inside {
        // A label, a title or a value that did not fit loses its end; a name is elided in
        // the middle by design (ADR 0024 §3), and may be.
        assert!(
            !s.ends_with('\u{2026}') || opens_a_dialog(s),
            "{what}: \"{s}\" was cut short"
        );
        assert!(
            r.left() >= clip.left() - 0.5 && r.right() <= clip.right() + 0.5,
            "{what}: \"{s}\" at {r:?} is cut by its clip {clip:?}"
        );
    }
    for (i, (a, ra, _)) in inside.iter().enumerate() {
        for (b, rb, _) in &inside[i + 1..] {
            let o = ra.intersect(*rb);
            assert!(
                !(o.width() > 0.5 && o.height() > 0.5),
                "{what}: \"{a}\" {ra:?} runs into \"{b}\" {rb:?}"
            );
        }
    }
}

fn demo_at_origin(view: View) -> (cantus_ui::CantusApp, TrackId) {
    let (mut a, track) = with_sampler();
    cantus_ui::views::inst::demo_setup(&mut a, track, InstrumentKind::Sampler);
    cantus_ui::views::inst::demo_feed(&mut a, InstrumentKind::Sampler);
    a.open_inst_editor(track);
    a.inst.view = view;
    (a, track)
}

#[test]
fn every_word_fits_its_box_at_three_quarters_double_size_and_in_spanish() {
    use cantus_ui::i18n::{set_thread_locale, Locale};
    for (locale, k) in [
        (Locale::En, 1.0),
        (Locale::En, 0.75),
        (Locale::En, 2.0),
        (Locale::Es, 1.0),
        (Locale::Es, 0.75),
    ] {
        for view in [View::Play, View::Edit] {
            let (mut a, track) = demo_at_origin(view);
            let c = ctx();
            set_thread_locale(locale);
            cantus_ui::theme::apply(&cantus_ui::theme::slate(), &c);
            cantus_ui::kit::shell::Shell::new(("inst", track.0)).set_scale(&c, k);
            let size = cantus_ui::kit::shell::INSTRUMENT * k + egui::vec2(40.0, 40.0);
            let shapes = run(&c, &mut a, size, vec![Vec::new(); 3]);
            // The instrument's own area: under the header, above the bands.
            let bands = if view == View::Play {
                60.0 + 92.0
            } else {
                92.0
            };
            let area = egui::Rect::from_min_max(
                egui::pos2(0.0, 36.0 * k),
                egui::pos2(1040.0 * k, (680.0 - 32.0 - bands) * k),
            );
            assert_fits(
                &texts(&shapes),
                area,
                &format!("{locale:?} {view:?} at {k}"),
            );
            set_thread_locale(Locale::En);
        }
    }
}

#[test]
fn a_drag_on_a_cell_is_one_undo_step() {
    let (mut a, track) = demo_at_origin(View::Play);
    let before = spec(&a, track);
    let c = ctx();
    let size = egui::vec2(1100.0, 720.0);
    let shapes = run(&c, &mut a, size, vec![Vec::new(); 3]);
    let attack = cantus_ui::i18n::t("smp.attack").to_uppercase();
    let (_, r, _) = texts(&shapes)
        .into_iter()
        .find(|(s, _, _)| *s == attack)
        .expect("the amplitude card's attack cell is painted");
    let p = r.center();
    let mut frames = vec![
        vec![egui::Event::PointerMoved(p)],
        vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    ];
    for i in 1..=8 {
        frames.push(vec![egui::Event::PointerMoved(
            p - egui::vec2(0.0, 6.0 * i as f32),
        )]);
    }
    frames.push(vec![egui::Event::PointerButton {
        pos: p - egui::vec2(0.0, 48.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    }]);
    frames.push(Vec::new());
    run(&c, &mut a, size, frames);
    assert!(
        spec(&a, track).get("amp.attack") > before.get("amp.attack"),
        "dragging the cell up lengthens the attack"
    );
    a.undo();
    assert_eq!(spec(&a, track), before, "the whole drag undoes at once");
}

/// A drag on a zone's cell moves the zone's number and is one undo step too.
#[test]
fn a_drag_on_a_zone_cell_is_one_undo_step() {
    let (mut a, track) = demo_at_origin(View::Edit);
    let before = spec(&a, track);
    let n = a.inst.sampler.selected.expect("the demo selects a zone");
    let c = ctx();
    let size = egui::vec2(1100.0, 720.0);
    let shapes = run(&c, &mut a, size, vec![Vec::new(); 3]);
    let gain = cantus_ui::i18n::t("smp.gain").to_uppercase();
    // The zone card is the first card with a GAIN cell from the top.
    let (_, r, _) = texts(&shapes)
        .into_iter()
        .filter(|(s, _, _)| *s == gain)
        .min_by(|a, b| a.1.top().total_cmp(&b.1.top()))
        .expect("the zone card's gain cell is painted");
    let p = r.center();
    let mut frames = vec![
        vec![egui::Event::PointerMoved(p)],
        vec![egui::Event::PointerButton {
            pos: p,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        }],
    ];
    for i in 1..=6 {
        frames.push(vec![egui::Event::PointerMoved(
            p - egui::vec2(0.0, 5.0 * i as f32),
        )]);
    }
    frames.push(vec![egui::Event::PointerButton {
        pos: p - egui::vec2(0.0, 30.0),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    }]);
    frames.push(Vec::new());
    run(&c, &mut a, size, frames);
    let z = model::Zone::of(&spec(&a, track), n).unwrap();
    assert!(
        z.get(zf::GAIN) > 0.0,
        "dragging the zone's gain up raises it"
    );
    a.undo();
    assert_eq!(spec(&a, track), before, "the whole drag undoes at once");
}

/// A label that ends in "…" by the convention for an action that opens a dialog
/// ("Export SFZ…", "+ Layer…"), as the locale spells it — not a cut.
fn opens_a_dialog(s: &str) -> bool {
    [
        "smp.export_sfz",
        "smp.import_samples",
        "smp.import_folder",
        "smp.import_instrument",
        "pulsus.layer.add",
        "pad.sample_load",
    ]
    .iter()
    .map(|k| cantus_ui::i18n::t(k))
    .any(|l| l == s || l.to_uppercase() == s)
}

/// With `CANTUS_INST_SHOTS=<dir>`: the whole Edit view, unscrolled, in Slate and Paper —
/// the part of the design review the window's scroll hides.
#[test]
fn the_whole_edit_view_for_the_design_review() {
    let Some(dir) = std::env::var_os("CANTUS_INST_SHOTS").map(std::path::PathBuf::from) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    for (name, theme) in [
        ("slate", cantus_ui::theme::slate()),
        ("paper", cantus_ui::theme::paper()),
    ] {
        let (mut a, track) = demo_at_origin(View::Edit);
        let c = ctx();
        cantus_ui::theme::apply(&theme, &c);
        let path = dir.join(format!("exemplar-edit-full-{name}.png"));
        cantus_ui::raster::shoot(&c, egui::vec2(1040.0, 1180.0), 1.0, 4, Some(&path), |ui| {
            let s = cantus_ui::kit::Style::current().function(cantus_ui::kit::Function::Source);
            let spec = a.native_of(track).unwrap();
            egui::CentralPanel::default().show(ui, |ui| {
                cantus_ui::views::inst::sampler::body(&mut a, ui, track, &spec, &s);
            });
        });
    }
}

/// Round 28 (sound): an instrument file's round robins mean a strict cycle (SFZ's
/// `seq_length`, Decent Sampler's round robin, a folder's `rr` takes), so an import into an
/// Exemplar saved before round 28 (round robin seeded, `rr.order` = 0) plays them in order
/// — and the undo gives the seeded order back.
#[test]
fn an_import_plays_its_round_robins_in_order_even_on_an_old_exemplar() {
    let (mut a, track) = with_sampler();
    a.set_inst_value(track, "rr.order", 0.0, false);
    smp::apply_import(&mut a, track, three_zone_import());
    assert_eq!(spec(&a, track).get("rr.order"), 1.0);
    a.undo();
    assert_eq!(spec(&a, track).get("rr.order"), 0.0);
}
