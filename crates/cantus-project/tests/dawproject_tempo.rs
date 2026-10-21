//! Review 2 of the tempo track in DAWproject (ADR 0060): times given in seconds are placed
//! through the file's own tempo map (findings 6, 19 and 57), a jump written as two points
//! at one time keeps the later (finding 58), and the tempo lane's unit is read (finding
//! 66). Built from the Bitwig-style fixture with the times changed.

use cantus_core::tempo::{TempoMap, TempoPoint};
use cantus_core::*;
use cantus_project::dawproject::{self, zip::ZipWriter};

const FOREIGN: &str = include_str!("fixtures/dawproject/foreign.xml");

const TEMPO: &str = r#"<TempoAutomation id="id90" unit="bpm"><Target parameter="id0"/><RealPoint time="0" value="149"/><RealPoint time="32" value="120"/></TempoAutomation>"#;
const MARKERS: &str = r#"<Markers id="id80"><Marker time="0" name="Intro"/><Marker time="16" name="Verse"/></Markers>"#;
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
        (
            KIT_CLIP,
            r#"<Clip time="20.0" duration="2.0" contentTimeUnit="seconds">"#,
        ),
        (
            MARKERS,
            r#"<Markers id="id80" timeUnit="seconds"><Marker time="0" name="Intro"/><Marker time="16" name="Verse"/></Markers>"#,
        ),
    ]);
    let p = &imported.project;
    assert_eq!(
        p.tempo_points,
        vec![pt(0, 149.0, false), pt(30_720, 120.0, false)]
    );
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
        vec![
            pt(0, 149.0, false),
            pt(30_720, 120.0, false),
            pt(third, 100.0, false)
        ]
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
    let held = pts(
        r#"<RealPoint time="0" value="120" interpolation="linear"/><RealPoint time="16" value="120" interpolation="linear"/><RealPoint time="16" value="140" interpolation="linear"/><RealPoint time="32" value="140"/>"#,
    );
    assert_eq!(
        held,
        vec![
            pt(0, 120.0, false),
            pt(15_360, 140.0, false),
            pt(30_720, 140.0, false)
        ]
    );
    let ramped = pts(
        r#"<RealPoint time="0" value="100" interpolation="linear"/><RealPoint time="16" value="140"/><RealPoint time="16" value="90"/>"#,
    );
    assert_eq!(
        ramped,
        vec![
            pt(0, 100.0, true),
            pt(15_359, 140.0, false),
            pt(15_360, 90.0, false)
        ]
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

/// Findings 10 and 50: an audio clip whose content is in seconds plays its `playStart`
/// second at its start, through the map — its hidden part reaching back across a tempo
/// change (120 to beat 8, then 60; the clip at beat 10, 3 s in).
#[test]
fn a_seconds_audio_clip_starts_at_its_second_through_the_map() {
    let old = r#"<Clip time="0.0" duration="8.0" playStart="0.0" name="Drumfunk3 170bpm">
            <Clips id="id26">
              <Clip time="0.0" duration="8.0" contentTimeUnit="beats" playStart="0.0">
                <Warps contentTimeUnit="seconds" timeUnit="beats" id="id28">
                  <Audio algorithm="stretch" channels="2" duration="2.823541666666667" sampleRate="48000" id="id27">
                    <File path="audio/Drumfunk3 170bpm.wav"/>
                  </Audio>
                  <Warp time="0.0" contentTime="0.0"/>
                  <Warp time="8.0" contentTime="2.823541666666667"/>
                </Warps>
              </Clip>
            </Clips>
          </Clip>"#;
    let new = r#"<Clip time="10.0" duration="2.0" contentTimeUnit="seconds" playStart="3" name="Take">
            <Audio algorithm="raw" channels="2" duration="20" sampleRate="48000" id="id27">
              <File path="audio/Drumfunk3 170bpm.wav"/>
            </Audio>
          </Clip>"#;
    let tempo = r#"<TempoAutomation id="id90" unit="bpm"><Target parameter="id0"/><RealPoint time="0" value="120"/><RealPoint time="8" value="60"/></TempoAutomation>"#;
    let mut xml = FOREIGN.replace('\r', "").replace(TEMPO, tempo);
    let fixture = xml.clone();
    xml = xml.replace(old, new);
    assert_ne!(xml, fixture, "the fixture has the clip");
    let p = import(&xml).project;
    let c = p
        .tracks
        .iter()
        .flat_map(|t| &t.clips)
        .find(|c| c.audio.is_some())
        .expect("the audio clip");
    assert_eq!(c.start, Ticks::beats(10));
    let map = p.tempo_map();
    let slip = f64::from(c.audio.as_ref().unwrap().slip_frames) / 48_000.0;
    let at = cantus_core::audio_time::offset_seconds(&map, c.start, c.offset) + slip;
    assert!((at - 3.0).abs() < 1.0 / 48_000.0, "{at} s at its start");
}

/// Review 4 (finding 52): a looped audio clip in seconds whose loop starts where it plays
/// (its hidden part across the tempo change) reports no loop moved.
#[test]
fn a_looped_seconds_clip_across_a_change_reports_no_loop_moved() {
    let old = r#"<Clip time="0.0" duration="8.0" playStart="0.0" name="Drumfunk3 170bpm">
            <Clips id="id26">
              <Clip time="0.0" duration="8.0" contentTimeUnit="beats" playStart="0.0">
                <Warps contentTimeUnit="seconds" timeUnit="beats" id="id28">
                  <Audio algorithm="stretch" channels="2" duration="2.823541666666667" sampleRate="48000" id="id27">
                    <File path="audio/Drumfunk3 170bpm.wav"/>
                  </Audio>
                  <Warp time="0.0" contentTime="0.0"/>
                  <Warp time="8.0" contentTime="2.823541666666667"/>
                </Warps>
              </Clip>
            </Clips>
          </Clip>"#;
    let new = r#"<Clip time="10.0" duration="4.0" contentTimeUnit="seconds" playStart="3" loopStart="3" loopEnd="5" name="Loop">
            <Audio algorithm="raw" channels="2" duration="20" sampleRate="48000" id="id27">
              <File path="audio/Drumfunk3 170bpm.wav"/>
            </Audio>
          </Clip>"#;
    let tempo = r#"<TempoAutomation id="id90" unit="bpm"><Target parameter="id0"/><RealPoint time="0" value="120"/><RealPoint time="8" value="60"/></TempoAutomation>"#;
    let xml = FOREIGN.replace('\r', "").replace(TEMPO, tempo);
    let with_loop = xml.replace(old, new);
    assert_ne!(with_loop, xml, "the fixture has the clip");
    let imported = import(&with_loop);
    let moved: Vec<&String> = imported
        .report
        .dropped_items()
        .filter(|i| i.key == "interop.report.d_loop")
        .flat_map(|i| i.args.iter().map(|(_, v)| v))
        .collect();
    assert!(
        moved.iter().any(|n| *n == "Loopy"),
        "the fixture's own is still said"
    );
    assert!(
        !moved.iter().any(|n| *n == "Loop"),
        "no loop was moved: {moved:?}"
    );
}

/// Review 5 (findings 13, 26): notes straight on a track's lane, in no clip, get one clip
/// on the file's bar lines through its metre changes — and through a metre at bar 1 that
/// differs from the transport's.
#[test]
fn notes_on_a_lane_get_a_clip_on_the_files_bar_lines() {
    let lane_clips = r#"<Lanes track="id33" id="id70" timeUnit="seconds">
        <Clips id="id71">
          <Clip time="2.0" duration="2.0" contentTimeUnit="seconds">
            <Notes id="id72">
              <Note time="0" duration="0.1" channel="9" key="36" vel="1.0"/>
              <Note time="0.5" duration="0.1" channel="9" key="49" vel="0.8"/>
            </Notes>
          </Clip>
        </Clips>
      </Lanes>"#;
    let clip_of = |last_beat: f64, metres: &str| {
        let lane = format!(
            r#"<Lanes track="id33" id="id70" timeUnit="beats"><Notes id="id72"><Note time="0" duration="0.25" channel="9" key="36" vel="1.0"/><Note time="{}" duration="0.5" channel="9" key="49" vel="0.8"/></Notes></Lanes>"#,
            last_beat - 0.5
        );
        let xml = FOREIGN
            .replace('\r', "")
            .replace(lane_clips, &lane)
            .replace("</Arrangement>", &format!("{metres}</Arrangement>"));
        assert!(
            xml.contains("key=\"49\" vel=\"0.8\"/></Notes></Lanes>"),
            "the lane"
        );
        let p = import(&xml).project;
        let c = p
            .tracks
            .iter()
            .flat_map(|t| &t.clips)
            .find(|c| c.notes.iter().any(|n| n.pitch == 49))
            .expect("the lane's clip")
            .clone();
        (p.tempo_map(), c)
    };
    // 4/4, then 7/8 from beat 4: bar lines 0, 3 840, 7 200, 10 560.
    let (map, c) = clip_of(
        11.0,
        r#"<TimeSignatureAutomation id="id91"><Target parameter="id1"/><TimeSignaturePoint time="0" numerator="4" denominator="4"/><TimeSignaturePoint time="4" numerator="7" denominator="8"/></TimeSignatureAutomation>"#,
    );
    assert_eq!((c.start, c.end()), (Ticks::ZERO, Ticks(10_560)));
    assert_eq!(map.bar_start(c.end()), c.end());
    // The transport's 4/4, the automation's 3/4 from bar 1: bar lines every 2 880.
    let (_, c) = clip_of(
        5.0,
        r#"<TimeSignatureAutomation id="id91"><Target parameter="id1"/><TimeSignaturePoint time="0" numerator="3" denominator="4"/></TimeSignatureAutomation>"#,
    );
    assert_eq!((c.start, c.end()), (Ticks::ZERO, Ticks(5_760)));
}

/// Review 5 (finding 28): tempo points, metres and markers past the song's horizon are
/// left out, and said.
#[test]
fn points_past_the_horizon_are_left_out_and_said() {
    let tempo = r#"<TempoAutomation id="id90" unit="bpm"><Target parameter="id0"/><RealPoint time="0" value="120" interpolation="linear"/><RealPoint time="1e9" value="121"/></TempoAutomation>"#;
    let marker = r#"<Markers id="id80"><Marker time="0" name="Intro"/><Marker time="1e9" name="Never"/></Markers>"#;
    let metres = r#"<TimeSignatureAutomation id="id91"><Target parameter="id1"/><TimeSignaturePoint time="0" numerator="4" denominator="4"/><TimeSignaturePoint time="8" numerator="3" denominator="4"/><TimeSignaturePoint time="2e9" numerator="7" denominator="8"/></TimeSignatureAutomation></Arrangement>"#;
    let imported = with(&[(TEMPO, tempo), (MARKERS, marker), ("</Arrangement>", metres)]);
    let p = &imported.project;
    assert!(p.tempo_points.is_empty(), "{:?}", p.tempo_points);
    assert!(!p.markers.iter().any(|m| m.name == "Never"));
    assert!(
        p.metre_changes.iter().all(|m| m.tick <= cantus_core::tempo::HORIZON),
        "{:?}",
        p.metre_changes
    );
    // One line for all three, summed (review 6: a line each).
    let far: Vec<_> = imported
        .report
        .dropped_items()
        .filter(|i| i.key == "tempo.report.too_far")
        .collect();
    assert_eq!(far.len(), 1, "{far:?}");
    assert_eq!(far[0].arg("n"), Some("3"));
}
