//! DAWproject and round 31's editing (ADR 0070 §9): a crossed overlap is written as the two
//! fades DAWproject has and read back as a crossfade; a gain envelope is the clip's gain
//! expression beside its audio, valid against the official schemas, and comes back point
//! for point; what the format cannot say is reported.

mod support;

use cantus_core::clip_gain::{GainEnvelope, GainPoint};
use cantus_core::crossfade::XfadeCurve;
use cantus_core::*;
use cantus_project::dawproject;
use support::xsd::Schema;

/// Two clips of one file overlapping by a beat, the later crossed on `curve`, the earlier
/// with a gain envelope.
fn project(dir: &std::path::Path, curve: XfadeCurve) -> Project {
    let mut p = Project::new("Edit");
    p.tempo_bpm = 120.0;
    let mut t = p.make_track("Audio");
    std::fs::write(dir.join("take.wav"), b"RIFF take").unwrap();
    let id = AudioSourceId(p.alloc_id());
    p.audio_pool.push(AudioSource {
        id,
        name: "take".into(),
        path: "take.wav".into(),
        sample_rate: 48_000,
        channels: 1,
        frames: 48_000 * 20,
        sha256: None,
        recorded: false,
    });
    let mut a = p.make_audio_clip("a", id, Ticks::beats(4), Ticks::beats(4));
    a.offset = Ticks::beats(2);
    let mut pts = vec![
        GainPoint::new(Ticks::beats(2), 0.0),
        GainPoint::new(Ticks::beats(3), -12.0),
        GainPoint::new(Ticks::beats(4), -12.0),
        GainPoint::new(Ticks::beats(5), 3.0),
    ];
    pts[1].shape = Shape::Hold;
    a.audio.as_mut().unwrap().gain_env = Some(Box::new(GainEnvelope::new(pts)));
    let mut b = p.make_audio_clip("b", id, Ticks::beats(7), Ticks::beats(4));
    b.offset = Ticks::beats(10);
    b.audio.as_mut().unwrap().cross = Some(curve);
    t.clips.extend([a, b]);
    p.tracks.push(t);
    p
}

#[test]
fn a_crossfade_and_an_envelope_are_valid_dawproject() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/dawproject/");
    let xsd = Schema::parse(&std::fs::read_to_string(format!("{dir}Project.xsd")).unwrap());
    let tmp = tempfile::tempdir().unwrap();
    let parts = dawproject::write::parts(&project(tmp.path(), XfadeCurve::Linear), "0.1.0");
    let errors = xsd.validate(&parts.project_xml);
    assert!(errors.is_empty(), "project.xml:\n{}", errors.join("\n"));
    let doc = roxmltree::Document::parse(&parts.project_xml).unwrap();
    let gain = doc
        .descendants()
        .find(|n| n.has_tag_name("Target") && n.attribute("expression") == Some("gain"))
        .expect("the gain expression");
    assert_eq!(gain.parent().unwrap().attribute("unit"), Some("linear"));
    // The overlap's two fades, each a beat.
    let clips: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("Clip") && n.attribute("fadeTimeUnit").is_some())
        .collect();
    let beat = |c: &roxmltree::Node, k: &str| {
        c.attribute(k)
            .and_then(|v| v.parse::<f64>().ok())
            .is_some_and(|v| (v - 1.0).abs() < 1e-9)
    };
    assert!(clips.iter().any(|c| beat(c, "fadeOutTime")));
    assert!(clips.iter().any(|c| beat(c, "fadeInTime")));
}

#[test]
fn a_round_trip_keeps_the_crossfade_and_the_envelope() {
    let tmp = tempfile::tempdir().unwrap();
    let p = project(tmp.path(), XfadeCurve::Linear);
    let out = dawproject::export_with(&p, "0.1.0", Some(tmp.path()));
    let back = dawproject::import(&out.bytes).unwrap();
    let t = &back.project.tracks[0];
    let (a, b) = (&t.clips[0], &t.clips[1]);
    let (ra, rb) = (a.audio.as_ref().unwrap(), b.audio.as_ref().unwrap());
    assert_eq!(
        rb.cross,
        Some(XfadeCurve::Linear),
        "the fades are a crossfade again"
    );
    assert_eq!((ra.fade_out, rb.fade_in), (Ticks::ZERO, Ticks::ZERO));
    let want = p.tracks[0].clips[0]
        .audio
        .as_ref()
        .unwrap()
        .gain_env
        .clone()
        .unwrap();
    let got = ra.gain_env.as_deref().expect("the envelope");
    assert_eq!(got.points.len(), want.points.len());
    for (g, w) in got.points.iter().zip(&want.points) {
        assert_eq!(g.tick, w.tick);
        assert!((g.db - w.db).abs() < 1e-3, "{} vs {}", g.db, w.db);
        assert_eq!(g.shape == Shape::Hold, w.shape == Shape::Hold);
    }
    // Linear and straight: nothing to report of them.
    let keys: Vec<&str> = out.report.items.iter().map(|i| i.key).collect();
    assert!(!keys.contains(&"clipfx.report.xfade_curve"), "{keys:?}");
    assert!(!keys.contains(&"clipfx.report.env_shape"), "{keys:?}");
}

#[test]
fn what_the_format_cannot_say_is_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let mut p = project(tmp.path(), XfadeCurve::EqualPower);
    if let Some(e) = p.tracks[0].clips[0]
        .audio
        .as_mut()
        .and_then(|a| a.gain_env.as_mut())
    {
        e.points[2].shape = Shape::SCurve;
    }
    let out = dawproject::export_with(&p, "0.1.0", Some(tmp.path()));
    let keys: Vec<&str> = out.report.items.iter().map(|i| i.key).collect();
    assert!(keys.contains(&"clipfx.report.xfade_curve"), "{keys:?}");
    assert!(keys.contains(&"clipfx.report.env_shape"), "{keys:?}");
}

/// Another application's clip with its audio and a gain expression in dB, in seconds.
#[test]
fn a_foreign_gain_expression_in_decibels_and_seconds_is_read() {
    let tmp = tempfile::tempdir().unwrap();
    let p = project(tmp.path(), XfadeCurve::Linear);
    let parts = dawproject::write::parts(&p, "0.1.0");
    let doc = roxmltree::Document::parse(&parts.project_xml).unwrap();
    let points = doc
        .descendants()
        .find(|n| n.has_tag_name("Points") && n.attribute("unit") == Some("linear"))
        .unwrap();
    assert!(points.children().any(|c| c.has_tag_name("RealPoint")));
    // Rewritten: dB, seconds (at 120 BPM a beat is half a second).
    let to_ticks = |t: f64| Ticks((t * 2.0 * PPQ as f64).round() as i64);
    let xml = r#"<Points unit="decibel" timeUnit="seconds"><Target expression="gain"/>
        <RealPoint time="1" value="-6" interpolation="linear"/>
        <RealPoint time="1.5" value="0" interpolation="hold"/></Points>"#;
    let d = roxmltree::Document::parse(xml).unwrap();
    let e = dawproject::audio::read_gain(d.root_element(), &to_ticks).unwrap();
    assert_eq!(e.points[0].tick, Ticks::beats(2));
    assert_eq!(e.points[0].db, -6.0);
    assert_eq!(e.points[1].shape, Shape::Hold);
}
