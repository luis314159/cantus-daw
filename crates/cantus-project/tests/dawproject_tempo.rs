//! Review 2 of the tempo track in DAWproject (ADR 0060): times given in seconds are placed
//! through the file's own tempo map (findings 6, 19 and 57), a jump written as two points
//! at one time keeps the later (finding 58), and the tempo lane's unit is read (finding
//! 66). Built from the Bitwig-style fixture with the times changed.

use cantus_core::tempo::{TempoMap, TempoPoint};
use cantus_core::*;
use cantus_project::dawproject::{self, zip::ZipWriter};

const FOREIGN: &str = include_str!("fixtures/dawproject/foreign.xml");

const TEMPO: &str = r#"<TempoAutomation id="id90" unit="bpm"><Target parameter="id0"/><RealPoint time="0" value="149"/><RealPoint time="32" value="120"/></TempoAutomation>"#;
const MARKERS: &str =
    r#"<Markers id="id80"><Marker time="0" name="Intro"/><Marker time="16" name="Verse"/></Markers>"#;
const KIT_CLIP: &str = r#"<Clip time="2.0" duration="2.0" contentTimeUnit="seconds">"#;

fn import(xml: &str) -> cantus_project::dawproject::Imported {
    let mut z = ZipWriter::new();
    z.add("project.xml", xml.as_bytes(), true);
    z.add(
        "metadata.xml",
        b"<?xml version=\"1.0\"?><MetaData><Title>Foreign</Title></MetaData>",
        false,
    );
    dawproject::import(&z.finish()).unwrap()
}

fn with(replace: &[(&str, &str)]) -> cantus_project::dawproject::Imported {
    let mut xml = FOREIGN.to_string();
    for (a, b) in replace {
        assert!(xml.contains(a), "the fixture has {a}");
        xml = xml.replace(a, b);
    }
    import(&xml)
}

fn pt(tick: i64, bpm: f64, ramp: bool) -> TempoPoint {
    TempoPoint {
        tick: Ticks(tick),
        bpm,
        ramp,
    }
}

fn map_of(p: &Project) -> TempoMap {
    p.tempo_map()
}

/// A seconds lane's clip and seconds markers after the beat-32 change (149 → 120 BPM)
/// land where they sound: at the map's tick for their second, the clip's notes at their
/// content seconds at 120 BPM.
#[test]
fn seconds_content_after_a_tempo_change_lands_where_it_sounds() {
    let imported = with(&[
        (KIT_CLIP, r#"<Clip time="20.0" duration="2.0" contentTimeUnit="seconds">"#),
        (
            MARKERS,
            r#"<Markers id="id80" timeUnit="seconds"><Marker time="0" name="Intro"/><Marker time="16" name="Verse"/></Markers>"#,
        ),
    ]);
    let p = &imported.project;
    assert_eq!(p.tempo_points, vec![pt(0, 149.0, false), pt(30_720, 120.0, false)]);
    let map = map_of(p);
    let at = |s: f64| Ticks(map.tick_at_seconds(s).round() as i64);
    // By hand: beat 32 is 32·60/149 s in; 120 BPM after it.
    let b32: f64 = 32.0 * 60.0 / 149.0;
    let by_hand = |s: f64| Ticks(((32.0 + (s - b32) * 2.0) * 960.0).round() as i64);
    assert_eq!(at(20.0), by_hand(20.0));
    let kit = &p.tracks[1];
    assert_eq!(kit.clips[0].start, by_hand(20.0));
    assert_eq!(kit.clips[0].length, Ticks(3840), "2 s at 120");
    let starts: Vec<Ticks> = kit.clips[0].notes.iter().map(|n| n.start).collect();
    assert_eq!(starts, vec![Ticks(0), Ticks(960)], "0 and 0.5 s at 120");
    let verse = p.markers.iter().find(|m| m.name == "Verse").unwrap();
    assert_eq!(verse.tick, by_hand(16.0));
    assert!(imported.report.has("tempo.report.seconds_map"));
}

/// Tempo points given in seconds are placed one after another, each through the map the
/// earlier ones make.
#[test]
fn tempo_points_in_seconds_are_placed_one_after_another() {
    let b32: f64 = 32.0 * 60.0 / 149.0;
    let tempo = format!(
        r#"<TempoAutomation id="id90" unit="bpm" timeUnit="seconds"><Target parameter="id0"/><RealPoint time="0" value="149"/><RealPoint time="{b32}" value="120"/><RealPoint time="20" value="100"/></TempoAutomation>"#
    );
    let imported = with(&[(TEMPO, &tempo)]);
    let p = &imported.project;
    let third = (30_720.0 + (20.0 - b32) * 2.0 * 960.0).round() as i64;
    assert_eq!(
        p.tempo_points,
        vec![pt(0, 149.0, false), pt(30_720, 120.0, false), pt(third, 100.0, false)]
    );
}

/// A jump written as two points at one time: the later holds from there (as
/// `TempoMap::new` has it); a ramp into the earlier ends a tick before the jump.
#[test]
fn a_jump_written_as_two_points_keeps_the_later() {
    let pts = |body: &str| {
        let tempo = format!(
            r#"<TempoAutomation id="id90" unit="bpm"><Target parameter="id0"/>{body}</TempoAutomation>"#
        );
        with(&[(TEMPO, &tempo)]).project.tempo_points
    };
    let held = pts(r#"<RealPoint time="0" value="120" interpolation="linear"/><RealPoint time="16" value="120" interpolation="linear"/><RealPoint time="16" value="140" interpolation="linear"/><RealPoint time="32" value="140"/>"#);
    assert_eq!(
        held,
        vec![pt(0, 120.0, false), pt(15_360, 140.0, false), pt(30_720, 140.0, false)]
    );
    let ramped = pts(r#"<RealPoint time="0" value="100" interpolation="linear"/><RealPoint time="16" value="140"/><RealPoint time="16" value="90"/>"#);
    assert_eq!(
        ramped,
        vec![pt(0, 100.0, true), pt(15_359, 140.0, false), pt(15_360, 90.0, false)]
    );
}

/// The tempo lane's unit: `normalized` through the transport tempo's range (20…666), an
/// unknown unit dropped with a report.
#[test]
fn the_tempo_lanes_unit_is_read() {
    let norm = r#"<TempoAutomation id="id90" unit="normalized"><Target parameter="id0"/><RealPoint time="0" value="0.5"/><RealPoint time="32" value="0.25"/></TempoAutomation>"#;
    let p = with(&[(TEMPO, norm)]).project;
    assert_eq!(
        p.tempo_points,
        vec![pt(0, 343.0, false), pt(30_720, 20.0 + 0.25 * 646.0, false)]
    );
    let odd = r#"<TempoAutomation id="id90" unit="percent"><Target parameter="id0"/><RealPoint time="0" value="50"/><RealPoint time="32" value="60"/></TempoAutomation>"#;
    let imported = with(&[(TEMPO, odd)]);
    assert!(imported.project.tempo_points.is_empty());
    assert_eq!(imported.project.tempo_bpm, 149.0, "the transport's tempo");
    assert!(imported.report.has("tempo.report.tempo_unit"));
}

/// Two metre points at one time: the later wins, as for the tempo.
#[test]
fn two_metre_points_at_one_time_keep_the_later() {
    let metres = r#"<TimeSignatureAutomation id="id91"><Target parameter="id1"/><TimeSignaturePoint time="0" numerator="4" denominator="4"/><TimeSignaturePoint time="8" numerator="3" denominator="4"/><TimeSignaturePoint time="8" numerator="7" denominator="8"/></TimeSignatureAutomation></Arrangement>"#;
    let p = with(&[("</Arrangement>", metres)]).project;
    let sigs: Vec<(Ticks, (u8, u8))> = p
        .metre_changes
        .iter()
        .map(|m| (m.tick, (m.signature.numerator, m.signature.denominator)))
        .collect();
    assert_eq!(sigs, vec![(Ticks(0), (4, 4)), (Ticks::beats(8), (7, 8))]);
}
