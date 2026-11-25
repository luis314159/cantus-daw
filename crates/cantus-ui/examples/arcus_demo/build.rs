//! The song built as a user builds it, in a running `CantusApp`: four tracks added from the
//! instrument picker (Arcus), each given its factory preset from the preset browser, named,
//! coloured, panned and sent to a concert hall (Aula, from the effect picker, on a return);
//! the tempo track with its two ritardandos, the key, the section markers; then each part's
//! clip, its notes' articulations (`SetNoteArt`, the piano roll's lane) and its CC1 lane —
//! every change one command, as the interface sends it.

use super::parts::PARTS;
use super::score::{self, Who, BAR, BPM, SECTIONS};
use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::scale::{Key, ScaleKind};
use cantus_core::tempo::{Marker, MarkerId, TempoPoint};
use cantus_core::{
    AutoPoint, Color, Command, Dock, EffectKind, InstrumentKind, Send, StripId, Ticks, TrackIcon,
    TrackId, PPQ,
};
use cantus_ui::picker::{PickChoice, PickPurpose};
use cantus_ui::preset_audition::Target;
use cantus_ui::CantusApp;
use std::collections::HashMap;

/// Where each instrument sits (a seated quartet seen from the hall: the violin on the
/// left, the cello inside, the viola on the right, the bass behind the cello), its fader
/// and its send to the hall (the bass further back: more of the room).
fn desk(who: Who) -> (f32, f32, f32, Color) {
    match who {
        Who::Violin => (-0.38, 7.0, -13.0, Color::rgb(0xD9, 0xA4, 0x41)),
        Who::Viola => (0.40, 0.5, -13.0, Color::rgb(0xC2, 0x7A, 0x3A)),
        Who::Cello => (0.12, 0.0, -13.0, Color::rgb(0x9C, 0x5A, 0x33)),
        Who::Bass => (0.22, -4.0, -11.0, Color::rgb(0x6E, 0x45, 0x2C)),
    }
}

/// What the build made: each instrument's track and strip.
pub struct Built {
    pub tracks: HashMap<Who, (TrackId, StripId)>,
}

fn exec(app: &mut CantusApp, what: &str, cmd: Command) {
    assert!(app.exec(cmd), "the app refused {what}");
}

/// A tick from a beat (exact: every length in the score is a multiple of 1/16 beat).
pub fn tick(beats: f64) -> Ticks {
    Ticks((beats * PPQ as f64).round() as i64)
}

pub fn bar_tick(bar: u32) -> Ticks {
    tick(f64::from(bar - 1) * BAR)
}

pub fn build(app: &mut CantusApp) -> Built {
    exec(
        app,
        "the name",
        Command::SetProjectName {
            name: super::TITLE.to_string(),
        },
    );
    exec(app, "the tempo", Command::SetTempo { bpm: BPM });
    // Ritardando into the climax's cadence (bar 37 → 38), a calmer a tempo after it, and
    // the coda held back to the last chord (bar 43 → 46).
    let pt = |bar: u32, bpm: f64, ramp: bool| TempoPoint {
        tick: bar_tick(bar),
        bpm,
        ramp,
    };
    exec(
        app,
        "the tempo track",
        Command::SetTempoTrack {
            bpm: BPM,
            points: vec![
                pt(1, BPM, false),
                pt(37, BPM, true),
                pt(38, 64.0, false),
                pt(39, 70.0, false),
                pt(43, 70.0, true),
                pt(46, 58.0, false),
            ],
        },
    );
    exec(
        app,
        "the key",
        Command::SetProjectKey {
            key: Some(Key::new(2, ScaleKind::Major)),
        },
    );
    for s in &SECTIONS {
        let id = MarkerId(app.bus.alloc_id());
        exec(
            app,
            "a marker",
            Command::AddMarker {
                marker: Marker {
                    id,
                    tick: bar_tick(s.bar),
                    name: s.name.to_string(),
                    color: None,
                },
            },
        );
    }

    // ---- the hall: a return on the right of the desk, Aula's concert hall on it ----
    let hall = app
        .project()
        .strips
        .iter()
        .find(|s| s.dock == Dock::Right && s.name.is_empty())
        .map(|s| s.id)
        .expect("a free return");
    exec(
        app,
        "the hall's name",
        Command::SetStripName {
            id: hall,
            name: "Sala de conciertos".to_string(),
        },
    );
    let purpose = PickPurpose::AddEffect { strip: hall };
    app.open_picker(purpose.clone());
    app.apply_pick(purpose, PickChoice::Effect(EffectKind::Aula));
    app.picker = None;
    let verb = app
        .project()
        .strip(hall)
        .and_then(|s| s.effects.last())
        .map(|e| e.id)
        .expect("Aula on the return");
    // Its concert hall, all wet on a return; the lows that would muddy the bass and the
    // highs a hall swallows kept out; a short pre-delay so the attacks stay clear.
    for (key, value) in [
        ("ir", 7.0),
        ("mix", 1.0),
        ("predelay", 18.0),
        ("low", 140.0),
        ("high", 9_000.0),
        ("level", 0.0),
    ] {
        exec(
            app,
            "a hall setting",
            Command::SetEffectParam {
                strip: hall,
                effect: verb,
                key: key.to_string(),
                value,
            },
        );
    }

    // ---- the four instruments ----
    let mut tracks = HashMap::new();
    for who in Who::ALL {
        let purpose = PickPurpose::AddInstrument { index: None };
        app.open_picker(purpose.clone());
        app.apply_pick(purpose, PickChoice::Instrument(InstrumentKind::BowedString));
        app.picker = None;
        let t = app.project().tracks.last().expect("the new track");
        let (track, strip) = (t.id, t.strip.expect("the track's strip"));
        app.open_preset_browser(Target::Track(track));
        assert!(
            app.apply_preset_id(who.preset()),
            "the preset {}",
            who.preset()
        );
        app.close_preset_browser();
        let (pan, gain, send, color) = desk(who);
        let name = who.name().to_string();
        exec(
            app,
            "a name",
            Command::SetTrackName {
                id: track,
                name: name.clone(),
            },
        );
        exec(
            app,
            "a strip name",
            Command::SetStripName { id: strip, name },
        );
        exec(app, "a colour", Command::SetTrackColor { id: track, color });
        exec(
            app,
            "a strip colour",
            Command::SetStripColor {
                id: strip,
                color: Some(color),
            },
        );
        exec(
            app,
            "an icon",
            Command::SetTrackIcon {
                id: track,
                icon: Some(TrackIcon::Strings),
            },
        );
        exec(app, "a pan", Command::SetStripPan { id: strip, pan });
        exec(
            app,
            "a fader",
            Command::SetStripGain {
                id: strip,
                gain_db: gain,
            },
        );
        let mut s = Send::new(hall);
        s.level = cantus_core::db_to_linear(send);
        exec(
            app,
            "a send",
            Command::InsertStripSend {
                id: strip,
                index: 0,
                send: s,
            },
        );
        tracks.insert(who, (track, strip));
    }

    // ---- the parts ----
    for part in PARTS {
        let (track, _) = tracks[&part.who];
        let sec = &SECTIONS[part.section];
        let (written, beats) = score::parse(part);
        let length = f64::from(sec.bars) * BAR;
        assert!(
            beats <= length + 1e-9,
            "{:?} in section {} runs {beats} beats into a {length}-beat section",
            part.who,
            part.section
        );
        // Ids from the project's counter, as the piano roll takes them.
        let mut scratch = app.project().clone();
        let mut clip = scratch.make_clip(sec.name, bar_tick(sec.bar), tick(length));
        let mut arts = Vec::new();
        for w in &written {
            let n = scratch.make_note(tick(w.at), tick(w.len), w.key, w.vel);
            if w.art.is_some() {
                arts.push((n.id, w.art));
            }
            clip.notes.push(n);
        }
        app.bus.reserve_ids(scratch.next_id);
        let id = clip.id;
        exec(
            app,
            "a clip",
            Command::InsertClip {
                track,
                clip: Box::new(clip),
            },
        );
        if !arts.is_empty() {
            exec(
                app,
                "the articulations",
                Command::SetNoteArt {
                    track,
                    clip: id,
                    arts,
                },
            );
        }
        let points = part
            .cc1
            .iter()
            .map(|(b, v)| AutoPoint::new(tick(*b), *v))
            .collect();
        exec(
            app,
            "the CC1 lane",
            Command::SetClipLanes {
                track,
                clip: id,
                lanes: vec![ClipLane::new(LaneKind::Cc(1), points)],
            },
        );
    }
    Built { tracks }
}
