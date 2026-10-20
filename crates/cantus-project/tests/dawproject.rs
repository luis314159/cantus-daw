//! DAWproject (ADR 0021): what is exported is valid against the official schemas (vendored
//! in `tests/fixtures/dawproject`, MIT, Bitwig), a Cantus project comes back whole, and a
//! file from another application comes in with a report of everything it could not keep.

mod support;
use cantus_core::*;
use cantus_project::dawproject::{self, zip::ZipWriter};
#[allow(unused_imports)]
use cantus_project::Severity;
use support::xsd::Schema;

fn schemas() -> (Schema, Schema) {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/dawproject/");
    let read = |f: &str| std::fs::read_to_string(format!("{dir}{f}")).unwrap();
    (
        Schema::parse(&read("Project.xsd")),
        Schema::parse(&read("MetaData.xsd")),
    )
}

fn strip_of(p: &mut Project, track: usize) -> &mut MixerStrip {
    let id = p.tracks[track].strip.expect("the track feeds a strip");
    p.strip_mut(id).unwrap()
}

/// Everything the exporter has a place for, and a few things it does not.
fn rich_project() -> Project {
    let mut p = Project::new("Rich");
    p.tempo_bpm = 128.0;
    p.time_signature = TimeSignature::new(3, 4);
    p.master.gain_db = -1.0;
    p.master.pan = 0.1;

    let mut lead = p.make_track("Lead");
    lead.instrument.firmus_mut().unwrap().cutoff_hz = 1234.5;
    let mut c = p.make_clip("Verse", Ticks::ZERO, Ticks::beats(6));
    c.color = Some(Color::rgb(10, 20, 30));
    for i in 0..6 {
        c.notes.push(p.make_note(
            Ticks::beats(i) + Ticks(7 * i),
            Ticks::EIGHTH + Ticks(i),
            60 + i as u8,
            40 + 13 * i as u8,
        ));
    }
    let mut muted = p.make_note(Ticks::beats(5), Ticks::QUARTER, 90, 100);
    muted.muted = true;
    c.notes.push(muted);
    lead.clips.push(c);
    let mut looped = p.make_clip("Loop", Ticks::beats(6), Ticks::beats(9));
    looped.offset = Ticks::beats(1);
    looped.content_length = Some(Ticks::beats(2));
    looped
        .notes
        .push(p.make_note(Ticks::beats(1) + Ticks(5), Ticks(333), 67, 99));
    lead.clips.push(looped);
    let mut silent = p.make_clip("Muted", Ticks::beats(15), Ticks::beats(3));
    silent.muted = true;
    silent
        .notes
        .push(p.make_note(Ticks::ZERO, Ticks::QUARTER, 72, 80));
    lead.clips.push(silent);
    p.tracks.push(lead);

    let mut bass = p.make_track("Bass");
    bass.gain_db = -2.0;
    bass.pan = 0.3;
    let mut c = p.make_clip("Line", Ticks::beats(3), Ticks::beats(6));
    c.notes
        .push(p.make_note(Ticks::ZERO, Ticks::beats(2), 36, 110));
    bass.clips.push(c);
    p.tracks.push(bass);

    for (name, pad) in [("Kick", DrumPad::kick()), ("Snare", DrumPad::snare())] {
        let mut t = p.make_track(name);
        let pitch = pad.pitch;
        t.instrument = InstrumentSpec::Pulsus(PulsusParams::single(pad));
        let mut c = p.make_clip(name, Ticks::ZERO, Ticks::beats(3));
        c.notes
            .push(p.make_note(Ticks::ZERO, Ticks::SIXTEENTH, pitch, 120));
        t.clips.push(c);
        p.tracks.push(t);
    }

    // The mixer: the lead's strip holds an EQ, a compressor keyed from the kick, and a
    // send to a return; the bass's strip has its channel EQ set; kick and snare share a
    // named bus with a limiter on it.
    let bus = p.strips[5].id;
    let ret = p.strips[15].id;
    let kick_strip = p.tracks[2].strip.unwrap();
    let mut eq = p.make_effect(EffectKind::Eq);
    eq.spec.set("b0.gain", 4.5);
    let mut comp = p.make_effect(EffectKind::Compressor);
    comp.sidechain = Some(kick_strip);
    comp.bypass = true;
    {
        let s = strip_of(&mut p, 0);
        s.gain_db = -3.0;
        s.pan = -0.25;
        s.effects.push(eq);
        s.effects.push(comp);
        s.sends.push(Send {
            to: ret,
            level: 0.4,
            enabled: true,
            tap: cantus_core::SendTap::Post,
        });
        s.name = "Lead strip".into();
    }
    strip_of(&mut p, 1).eq.set(1, EqField::Gain, -4.0);
    // Schema 23: a pre-fader send, which DAWproject carries as `type="pre"`.
    strip_of(&mut p, 1).sends.push(Send {
        level: 0.25,
        tap: cantus_core::SendTap::Pre,
        ..Send::new(ret)
    });
    strip_of(&mut p, 2).output = Some(bus);
    strip_of(&mut p, 3).output = Some(bus);
    let lim = p.make_effect(EffectKind::Limiter);
    let rev = p.make_effect(EffectKind::Reverb);
    {
        let b = p.strip_mut(bus).unwrap();
        b.name = "Drum bus".into();
        b.effects.push(lim);
        b.muted = true;
        b.color = Some(Color::rgb(200, 100, 50));
    }
    {
        let r = p.strip_mut(ret).unwrap();
        r.name = "Return".into();
        r.effects.push(rev);
        r.gain_db = -6.0;
        r.soloed = true;
    }
    p
}

#[test]
fn the_exported_project_is_valid_against_the_official_schemas() {
    let (project_xsd, metadata_xsd) = schemas();
    for p in [rich_project(), Project::default(), Project::new("Empty")] {
        let parts = dawproject::write::parts(&p, "0.1.0");
        let errors = project_xsd.validate(&parts.project_xml);
        assert!(errors.is_empty(), "project.xml:\n{}", errors.join("\n"));
        let errors = metadata_xsd.validate(&parts.metadata_xml);
        assert!(errors.is_empty(), "metadata.xml:\n{}", errors.join("\n"));
        // Every state file the XML names is in the archive.
        let doc = roxmltree::Document::parse(&parts.project_xml).unwrap();
        for s in doc.descendants().filter(|n| n.has_tag_name("State")) {
            let path = s.attribute("path").unwrap();
            assert!(
                parts.files.iter().any(|(f, _)| f == path),
                "{path} is missing"
            );
        }
        let archive = dawproject::zip::read(&dawproject::export(&p, "0.1.0").bytes).unwrap();
        assert_eq!(archive[0].0, "project.xml");
        assert_eq!(archive[1].0, "metadata.xml");
    }
}

#[test]
fn the_validator_refuses_what_the_schema_refuses() {
    let (xsd, _) = schemas();
    let good = dawproject::write::parts(&rich_project(), "0.1.0").project_xml;
    assert!(xsd.validate(&good).is_empty());
    let breakages = [
        // A required attribute gone.
        (good.replacen(" unit=\"bpm\"", "", 1), "required attribute"),
        // An enumeration broken.
        (
            good.replacen("role=\"master\"", "role=\"bus\"", 1),
            "is not one of",
        ),
        // An element the schema does not have.
        (
            good.replacen("<Transport>", "<Transport><Swing/>", 1),
            "do not match",
        ),
        // Out of order: a channel's volume before its mute.
        (
            good.replacen(
                "<Mute value=\"false\"",
                "<Volume unit=\"linear\" value=\"1\"/><Mute value=\"false\"",
                1,
            ),
            "do not match",
        ),
        // A destination that names nothing.
        (
            good.replacen("destination=\"id", "destination=\"nowhere", 1),
            "names no ID",
        ),
        // An id twice.
        (good.replacen("id=\"id2\"", "id=\"id1\"", 1), "unique"),
        // Not a number where one belongs.
        (
            good.replacen("time=\"0.0\"", "time=\"soon\"", 1),
            "xs:double",
        ),
    ];
    for (bad, why) in breakages {
        assert_ne!(bad, good, "the breakage for {why:?} changed nothing");
        let errors = xsd.validate(&bad);
        assert!(
            errors.iter().any(|e| e.contains(why)),
            "expected an error about {why:?}, got {errors:?}"
        );
    }
}

/// A clip without ids and without the notes DAWproject cannot mute.
fn clip_view(c: &Clip) -> String {
    let mut notes: Vec<_> = c
        .notes
        .iter()
        .filter(|n| !n.muted)
        .map(|n| (n.start.0, n.length.0, n.pitch, n.velocity))
        .collect();
    notes.sort();
    format!(
        "{} {:?} {} {} {} {:?} {} {:?}",
        c.name, c.color, c.start.0, c.length.0, c.offset.0, c.content_length, c.muted, notes
    )
}

/// What a strip is, named the way the desk names it, with its routes named the same way.
fn strip_view(p: &Project, id: Option<StripId>) -> String {
    let Some(s) = id.and_then(|id| p.strip(id)) else {
        return "master".into();
    };
    let label = |id: StripId| {
        let s = p.strip(id).unwrap();
        if s.name.is_empty() {
            p.tracks_on_strip(id)
                .next()
                .map(|t| format!("<{}>", t.name))
                .unwrap_or_else(|| "?".into())
        } else {
            s.name.clone()
        }
    };
    let fx: Vec<String> = s
        .effects
        .iter()
        .map(|e| {
            format!(
                "{:?} {:?} bypass={} mix={} key={:?}",
                e.spec.kind,
                e.spec.params,
                e.bypass,
                e.mix,
                e.sidechain.map(label)
            )
        })
        .collect();
    let sends: Vec<String> = s
        .sends
        .iter()
        .map(|x| format!("{}@{}:{}:{:?}", label(x.to), x.level, x.enabled, x.tap))
        .collect();
    format!(
        "{} gain={:.3} pan={} mute={} solo={} color={:?} eq={:?} fx={fx:?} sends={sends:?} out={}",
        label(s.id),
        s.gain_db,
        s.pan,
        s.muted,
        s.soloed,
        s.color,
        s.eq,
        s.output.map(label).unwrap_or_else(|| "master".into())
    )
}

#[test]
fn a_cantus_project_comes_back_whole() {
    let before = rich_project();
    let out = dawproject::export(&before, "0.1.0");
    // The export says what it could not carry.
    let muted = out.report.get("interop.report.x_muted_notes").unwrap();
    assert_eq!(muted.arg("n"), Some("1"));
    assert!(out.report.has("interop.report.x_sidechain"));
    assert!(out.report.has("interop.report.x_empty_strips"));

    let back = dawproject::import(&out.bytes).unwrap();
    assert!(
        back.report.is_lossless(),
        "our own file dropped {:?}",
        back.report.dropped_items().collect::<Vec<_>>()
    );
    let after = back.project;
    assert_eq!(after.name, "Rich");
    assert_eq!(after.tempo_bpm, 128.0);
    assert_eq!(after.time_signature, TimeSignature::new(3, 4));
    assert!((after.master.gain_db - -1.0).abs() < 1e-3);
    assert_eq!(after.master.pan, 0.1);
    assert_eq!(after.tracks.len(), before.tracks.len());
    for (a, b) in before.tracks.iter().zip(&after.tracks) {
        assert_eq!(a.name, b.name);
        assert_eq!(a.color, b.color);
        assert_eq!(a.instrument, b.instrument, "{}", a.name);
        assert!((a.gain_db - b.gain_db).abs() < 1e-3, "{}", a.name);
        assert_eq!(a.pan, b.pan, "{}", a.name);
        assert_eq!((a.muted, a.soloed), (b.muted, b.soloed));
        let ca: Vec<_> = a.clips.iter().map(clip_view).collect();
        let cb: Vec<_> = b.clips.iter().map(clip_view).collect();
        assert_eq!(ca, cb, "{}", a.name);
        assert_eq!(
            strip_view(&before, a.strip),
            strip_view(&after, b.strip),
            "the strip {} feeds",
            a.name
        );
    }
    // The return, which no track feeds, came back too.
    let ret = |p: &Project| {
        strip_view(
            p,
            p.strips.iter().find(|s| s.name == "Return").map(|s| s.id),
        )
    };
    assert_eq!(ret(&before), ret(&after));
    assert_eq!(
        after.strips.len(),
        DEFAULT_STRIPS,
        "a full desk to route to"
    );
    // And what the engine would play is the same.
    let played = |p: &Project| -> Vec<Vec<(i64, bool, u8, u8)>> {
        Snapshot::compile(p, 0)
            .tracks
            .iter()
            .map(|t| {
                t.events
                    .iter()
                    .map(|e| (e.tick.0, e.is_on(), e.pitch, e.velocity))
                    .collect()
            })
            .collect()
    };
    assert_eq!(played(&before), played(&after));
}

/// A file as Bitwig Studio 5 writes one (the format's own README example), with what other
/// applications add: a group with a track inside it, a return fed by a pre-fader send, the
/// built-in equaliser and compressor, automation, markers, a tempo change, an audio clip, a
/// drum part in seconds and a clip launcher scene.
const FOREIGN: &str = include_str!("fixtures/dawproject/foreign.xml");

fn foreign_archive() -> Vec<u8> {
    let mut z = ZipWriter::new();
    z.add("project.xml", FOREIGN.as_bytes(), true);
    z.add(
        "metadata.xml",
        b"<?xml version=\"1.0\"?><MetaData><Title>Foreign</Title></MetaData>",
        false,
    );
    z.add(
        "plugins/d19b1f6e-bbb6-42fe-a6c9-54b41d97a05d.clap-preset",
        &[1, 2, 3],
        false,
    );
    z.finish()
}

#[test]
fn a_file_from_bitwig_comes_in_and_everything_it_could_not_keep_is_named() {
    // The fixture is itself valid, so it is a fair stand-in for the real thing.
    let (xsd, _) = schemas();
    let errors = xsd.validate(FOREIGN);
    assert!(errors.is_empty(), "the fixture: {errors:?}");

    let imported = dawproject::import(&foreign_archive()).unwrap();
    let p = &imported.project;
    let r = &imported.report;
    assert_eq!(p.name, "Foreign");
    assert_eq!(p.tempo_bpm, 149.0);
    assert!((p.master.gain_db - -6.0206).abs() < 1e-3);

    // Two tracks with notes; the audio track, the group and the return are strips only.
    let names: Vec<&str> = p.tracks.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["Bass", "Kit"]);
    let bass = &p.tracks[0];
    assert_eq!(bass.color, Color::rgb(0xa2, 0xea, 0xbf));
    assert_eq!(bass.clips.len(), 2);
    assert_eq!(bass.clips[0].notes.len(), 9);
    assert_eq!(bass.clips[0].length, Ticks::beats(8));
    let n = &bass.clips[0].notes;
    assert!(n
        .iter()
        .any(|n| n.start == Ticks::beats(6) && n.length == Ticks::beats(2) && n.pitch == 53));
    assert!(n.iter().all(|n| n.velocity == 100), "0.787402 × 127 = 100");
    assert_eq!(bass.clips[1].offset, Ticks::beats(1));
    assert_eq!(bass.clips[1].content_length, Some(Ticks::beats(4)));
    assert!(
        bass.instrument.firmus().is_some(),
        "Surge XT is not hosted: Firmus"
    );

    // The bass's strip: its volume, pan, EQ and the send, now after the fader.
    let strip = p.strip(bass.strip.unwrap()).unwrap();
    assert!((strip.gain_db - -6.0206).abs() < 1e-3);
    assert_eq!(strip.pan, -0.5);
    assert_eq!(strip.eq.bands[0].freq_hz, 120.0);
    assert_eq!(strip.eq.bands[0].gain_db, -3.0);
    assert_eq!(strip.eq.bands[1].gain_db, 2.5);
    assert_eq!(strip.eq.bands[1].q, 1.4);
    assert_eq!(strip.eq.bands[2].gain_db, 0.0);
    assert_eq!(strip.output, None);
    let fx = p.strips.iter().find(|s| s.name == "FX Reverb").unwrap();
    assert_eq!(fx.dock, Dock::Right);
    assert_eq!(
        strip.sends,
        vec![Send {
            level: 0.5,
            tap: cantus_core::SendTap::Pre,
            ..Send::new(fx.id)
        }],
        "the pre-fader send stays pre-fader (schema 23)"
    );

    // The kit: in the group, a kit because its notes are on channel 10, with the crash
    // the file plays, placed by seconds at 149 BPM.
    let kit = &p.tracks[1];
    let pads = kit.instrument.pulsus().expect("a kit");
    assert!(pads.pads.iter().any(|pad| pad.pitch == 49));
    let group = p.strips.iter().find(|s| s.name == "Drums group").unwrap();
    assert_eq!(group.color, Some(Color::rgb(0x33, 0x66, 0x99)));
    let kit_strip = p.strip(kit.strip.unwrap()).unwrap();
    assert_eq!(
        kit_strip.output,
        Some(group.id),
        "a group's child feeds the group"
    );
    assert_eq!(kit_strip.gain_db, -6.0);
    let at = (2.0 * 149.0 / 60.0 * 960.0_f64).round() as i64;
    assert_eq!(kit.clips[0].start, Ticks(at));
    let drumloop = p.strips.iter().find(|s| s.name == "Drumloop").unwrap();
    assert!(drumloop.muted);

    // And the report names every loss.
    for key in [
        "interop.report.d_instrument",
        "interop.report.d_device",
        "interop.report.d_audio_clips",
        "interop.report.d_automation",
        "interop.report.d_scenes",
        "interop.report.d_loop",
    ] {
        assert!(r.has(key), "the report does not say {key}");
    }
    let arg = |key: &str, name: &str| r.get(key).unwrap().arg(name).map(str::to_string);
    assert_eq!(
        arg("interop.report.d_device", "name").as_deref(),
        Some("Compressor")
    );
    assert_eq!(
        arg("interop.report.d_instrument", "name").as_deref(),
        Some("Surge XT")
    );
    // The arrangement's markers and its tempo automation come in (round 29, ADR 0060).
    assert_eq!(arg("tempo.report.markers_kept", "n").as_deref(), Some("2"));
    assert!(r.has("tempo.report.tempo_kept"));
    assert!(!r.has("interop.report.d_markers") && !r.has("interop.report.d_tempo_automation"));
    let names: Vec<&str> = p.markers.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(names, ["Intro", "Verse"]);
    assert_eq!(p.tempo_points.len(), 2);
    assert!(r.has("interop.report.d_seconds"));
    assert!(
        !r.has("interop.report.d_pre_sends"),
        "a pre-fader send is no loss now"
    );
    assert!(r.has("interop.report.d_foreign"));
}

#[test]
fn a_damaged_file_is_refused_with_a_reason() {
    assert!(matches!(
        dawproject::import(b"PK\x03\x04 not really a zip archive at all"),
        Err(dawproject::DawProjectError::Zip(_))
    ));
    let one = |name: &str, data: &[u8]| {
        let mut z = ZipWriter::new();
        z.add(name, data, false);
        z.finish()
    };
    assert!(matches!(
        dawproject::import(&one("readme.txt", b"hello")),
        Err(dawproject::DawProjectError::NoProject)
    ));
    assert!(matches!(
        dawproject::import(&one("project.xml", b"<Song/>")),
        Err(dawproject::DawProjectError::NotDawProject(_))
    ));
    assert!(matches!(
        dawproject::import(&one("project.xml", b"<Project")),
        Err(dawproject::DawProjectError::Xml(_))
    ));
}

/// The full `javax.xml.validation` validator over the same files, when a JDK is on the
/// PATH: `cargo test -p cantus-project --test dawproject -- --ignored`.
#[test]
#[ignore = "needs a JDK on the PATH"]
fn the_jdk_validator_agrees() {
    let dir = tempfile::tempdir().unwrap();
    let parts = dawproject::write::parts(&rich_project(), "0.1.0");
    let fixtures = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/dawproject/");
    std::fs::write(dir.path().join("project.xml"), &parts.project_xml).unwrap();
    std::fs::write(dir.path().join("metadata.xml"), &parts.metadata_xml).unwrap();
    let src = format!("{fixtures}Validate.java");
    for (xsd, xml) in [
        ("Project.xsd", dir.path().join("project.xml")),
        ("MetaData.xsd", dir.path().join("metadata.xml")),
        ("Project.xsd", format!("{fixtures}foreign.xml").into()),
    ] {
        let out = std::process::Command::new("java")
            .arg(&src)
            .arg(format!("{fixtures}{xsd}"))
            .arg(&xml)
            .output()
            .expect("java runs");
        assert!(
            out.status.success(),
            "{}: {}",
            xml.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// A project with a Lamina and an Exemplar at their defaults (round 24, ADR 0039).
fn calibrated_project() -> Project {
    let mut p = Project::new("Calibrated");
    for kind in [InstrumentKind::ElectricPiano, InstrumentKind::Sampler] {
        let mut t = p.make_track(format!("{kind:?}"));
        t.instrument = InstrumentSpec::Native(NativeSpec::new(kind));
        p.tracks.push(t);
    }
    p
}

fn zip_parts(parts: &dawproject::Parts) -> Vec<u8> {
    let mut z = ZipWriter::new();
    z.add("project.xml", parts.project_xml.as_bytes(), true);
    z.add("metadata.xml", parts.metadata_xml.as_bytes(), true);
    for (path, data) in &parts.files {
        z.add(path, data, true);
    }
    z.finish()
}

#[test]
fn a_dawproject_from_before_the_calibration_keeps_its_level() {
    let p = calibrated_project();
    // What a build before schema 21 wrote: the instruments' state with no trim at all.
    let mut parts = dawproject::write::parts(&p, "0.1.0");
    for (_, data) in &mut parts.files {
        let mut v: serde_json::Value = serde_json::from_slice(data).unwrap();
        if let Some(params) = v.get_mut("params").and_then(|p| p.as_object_mut()) {
            params.remove(cantus_core::inst::TRIM_KEY);
        }
        *data = serde_json::to_vec_pretty(&v).unwrap();
    }
    let back = dawproject::import(&zip_parts(&parts)).unwrap().project;
    for t in &back.tracks {
        let n = t
            .instrument
            .native()
            .expect("Cantus's own instrument comes back");
        assert!(n.is_stored(cantus_core::inst::TRIM_KEY), "{}", t.name);
        assert_eq!(
            n.get(cantus_core::inst::TRIM_KEY),
            0.0,
            "{}: the old level",
            t.name
        );
    }
    // One exported now comes back as it was: at the calibrated level, the spec unchanged.
    let now = dawproject::import(&dawproject::export(&p, "0.1.0").bytes)
        .unwrap()
        .project;
    for (a, b) in p.tracks.iter().zip(&now.tracks) {
        assert_eq!(a.instrument, b.instrument, "{}", a.name);
        let n = b.instrument.native().unwrap();
        assert_eq!(n.get(cantus_core::inst::TRIM_KEY), n.kind.calibration_db());
    }
}

/// Round 29 (ADR 0060): the tempo track (a ramp as `linear`, a step as `hold`), the metre
/// changes and the markers go out as the arrangement's `TempoAutomation`,
/// `TimeSignatureAutomation` and `Markers` — valid against the official schema — and come
/// back exactly.
#[test]
fn the_tempo_map_round_trips_through_a_dawproject() {
    use cantus_core::tempo::{Marker, MarkerId, MetreChange, TempoPoint};
    let mut p = rich_project();
    p.tempo_points = vec![
        TempoPoint {
            tick: Ticks::ZERO,
            bpm: p.tempo_bpm,
            ramp: true,
        },
        TempoPoint {
            tick: Ticks::beats(8),
            bpm: 140.0,
            ramp: false,
        },
        TempoPoint {
            tick: Ticks::beats(16),
            bpm: 100.0,
            ramp: false,
        },
    ];
    p.metre_changes = vec![
        MetreChange {
            tick: Ticks::ZERO,
            signature: p.time_signature,
        },
        MetreChange {
            tick: Ticks(3 * 2880),
            signature: TimeSignature::new(7, 8),
        },
    ];
    p.markers = vec![
        Marker {
            id: MarkerId(5_000),
            tick: Ticks::ZERO,
            name: "Intro".into(),
            color: None,
        },
        Marker {
            id: MarkerId(5_001),
            tick: Ticks::beats(8),
            name: "Verse".into(),
            color: Some(Color::PALETTE[2]),
        },
    ];
    let (xsd, _) = schemas();
    let parts = dawproject::write::parts(&p, "0.1.0");
    let errors = xsd.validate(&parts.project_xml);
    assert!(
        errors.is_empty(),
        "project.xml:
{}",
        errors.join(
            "
"
        )
    );
    let back = dawproject::import(&dawproject::export(&p, "0.1.0").bytes).unwrap();
    assert!(
        back.report.is_lossless(),
        "{:?}",
        back.report.dropped_items().collect::<Vec<_>>()
    );
    let a = back.project;
    assert_eq!(a.tempo_bpm, p.tempo_bpm);
    assert_eq!(a.tempo_points, p.tempo_points);
    assert_eq!(a.metre_changes, p.metre_changes);
    let m: Vec<(Ticks, &str, Option<Color>)> = a
        .markers
        .iter()
        .map(|m| (m.tick, m.name.as_str(), m.color))
        .collect();
    assert_eq!(
        m,
        [
            (Ticks::ZERO, "Intro", None),
            (Ticks::beats(8), "Verse", Some(Color::PALETTE[2]))
        ]
    );
}
