//! The song built as a user builds it, in a running `CantusApp`: four tracks added from the
//! instrument picker (Arcus), each given its factory preset from the preset browser, named,
//! coloured, panned and sent to a concert hall (Aula, from the effect picker, on a return);
//! the tempo track with its two ritardandos, the key, the section markers; then each part's
//! clip, its notes' articulations (`SetNoteArt`, the piano roll's lane) and its CC1 lane —
//! every change one command, as the interface sends it.
//!
//! r42 arcus-expr: the same build plays a [`Piece`] — the quartet, or one of the solos — in
//! a [`Style`]: pass 6's (the notes, their slurs and marks, the written CC1) or performed
//! (`perform.rs`: the bows, the attacks, CC1/CC11/CC21/CC65, the intonation, the timing and
//! the rubato, all as a user enters them).

use super::perform::{self, Ctx, Role};
use super::score::{self, Part, Section, Who, BAR};
use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::model::note_expr::{ExprKind, ExprPoint, NoteExpr};
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
fn desk(who: Who, solo: bool) -> (f32, f32, f32, Color) {
    if solo {
        let (_, _, _, c) = desk(who, false);
        return (0.0, 0.0, -13.0, c);
    }
    match who {
        // r42:arcus-p5: the violin's and the bass's faders back at 0 dB — the presets'
        // gains are fitted on a musical phrase now (`cantus_ui::arcus_level`); before, the
        // demo lifted the violin 7 dB and lowered the bass 4 to sit them with the others.
        Who::Violin => (-0.38, 0.0, -13.0, Color::rgb(0xD9, 0xA4, 0x41)),
        Who::Viola => (0.40, 0.5, -13.0, Color::rgb(0xC2, 0x7A, 0x3A)),
        Who::Cello => (0.12, 0.0, -13.0, Color::rgb(0x9C, 0x5A, 0x33)),
        Who::Bass => (0.22, 0.0, -11.0, Color::rgb(0x6E, 0x45, 0x2C)),
    }
}

/// How the notes are played.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Style {
    /// Pass 6's build: the notes as written with their slurs and marks, a velocity per
    /// part with a fixed wobble, the written CC1 (the quartet), or one velocity and a flat
    /// CC1 (a solo's plain take).
    Plain,
    /// The player's (`perform.rs`).
    Performed,
}

/// A piece the build plays.
pub struct Piece {
    pub title: &'static str,
    pub bpm: f64,
    /// The tempo track: `(bar, bpm, ramp to the next)`.
    pub tempo: &'static [(u32, f64, bool)],
    pub sections: &'static [Section],
    pub parts: &'static [Part],
    pub who: &'static [Who],
    /// Semitones each instrument's part is moved by (the solos share one line).
    pub transpose: fn(Who) -> i32,
    /// A part's place in a section's texture.
    pub role: fn(Who, usize) -> Role,
    /// The climax's and the coda's sections.
    pub climax: Option<usize>,
    pub intimate: Option<usize>,
    /// A solo: centred, and its plain take one velocity and a flat CC1.
    pub solo: bool,
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

/// The written tempo at `beat` (from the song's start) of a piece's tempo track.
fn tempo_at(piece: &Piece, beat: f64) -> f64 {
    let pts = piece.tempo;
    let at = |k: usize| f64::from(pts[k].0 - 1) * BAR;
    let mut bpm = piece.bpm;
    for k in 0..pts.len() {
        if beat < at(k) {
            break;
        }
        bpm = pts[k].1;
        if pts[k].2 && k + 1 < pts.len() && beat < at(k + 1) {
            let u = (beat - at(k)) / (at(k + 1) - at(k));
            return pts[k].1 + (pts[k + 1].1 - pts[k].1) * u;
        }
    }
    bpm
}

/// The tempo track: the written points, and in the performed style the melody's rubato
/// around them (`perform::rubato`), away from the written ramps.
fn tempo_points(piece: &Piece, style: Style) -> Vec<TempoPoint> {
    let mut pts: Vec<(f64, f64, bool)> = piece
        .tempo
        .iter()
        .map(|&(bar, bpm, ramp)| (f64::from(bar - 1) * BAR, bpm, ramp))
        .collect();
    if style == Style::Performed {
        let mut keep: Vec<(f64, f64)> = Vec::new();
        for k in 0..pts.len() {
            keep.push((pts[k].0 - 1.0, pts[k].0 + 1.0));
            if pts[k].2 && k + 1 < pts.len() {
                keep.push((pts[k].0 - 2.0, pts[k + 1].0 + 1.0));
            }
        }
        let mut phrases = Vec::new();
        for part in piece.parts {
            if (piece.role)(part.who, part.section) != Role::Melody {
                continue;
            }
            let sec = &piece.sections[part.section];
            let base = f64::from(sec.bar - 1) * BAR;
            let (w, _) = score::parse_t(part, (piece.transpose)(part.who));
            let ph = perform::phrases(&w);
            for (k, p) in ph.iter().enumerate() {
                let next = ph.get(k + 1).map_or(f64::INFINITY, |q| w[q.first].at);
                let pk = &w[p.peak];
                phrases.push((
                    base + w[p.first].at,
                    base + pk.at,
                    base + pk.at + pk.len,
                    base + w[p.last].at + w[p.last].len,
                    base + next,
                ));
            }
        }
        pts.extend(perform::rubato(&phrases, |b| tempo_at(piece, b), &keep));
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    pts.into_iter()
        .map(|(b, bpm, ramp)| TempoPoint {
            tick: tick(b),
            bpm,
            ramp,
        })
        .collect()
}

pub fn build(app: &mut CantusApp, piece: &Piece, style: Style) -> Built {
    exec(
        app,
        "the name",
        Command::SetProjectName {
            name: piece.title.to_string(),
        },
    );
    exec(app, "the tempo", Command::SetTempo { bpm: piece.bpm });
    // The quartet: a ritardando into the climax's cadence (bar 37 → 38), a calmer a tempo
    // after it, and the coda held back to the last chord (bar 43 → 46).
    exec(
        app,
        "the tempo track",
        Command::SetTempoTrack {
            bpm: piece.bpm,
            points: tempo_points(piece, style),
        },
    );
    exec(
        app,
        "the key",
        Command::SetProjectKey {
            key: Some(Key::new(2, ScaleKind::Major)),
        },
    );
    for s in piece.sections {
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
    for &who in piece.who {
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
        let (pan, mut gain, send, color) = desk(who, piece.solo);
        // r42 arcus-expr: measured (the HSD quartet's viola sits 1 dB under the four's mean,
        // pass 6's 7), the performed viola 2.5 dB up.
        if style == Style::Performed && who == Who::Viola && !piece.solo {
            gain += 2.5;
        }
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
    for part in piece.parts {
        let (track, _) = tracks[&part.who];
        let sec = &piece.sections[part.section];
        let (mut written, beats) = score::parse_t(part, (piece.transpose)(part.who));
        let length = f64::from(sec.bars) * BAR;
        assert!(
            beats <= length + 1e-9,
            "{:?} in section {} runs {beats} beats into a {length}-beat section",
            part.who,
            part.section
        );
        // A solo's plain take: one velocity, no dynamic but the flat one.
        if style == Style::Plain && piece.solo {
            for w in written.iter_mut() {
                w.vel = part.vel;
            }
        }
        let perf = (style == Style::Performed).then(|| {
            let ctx = Ctx {
                bpm: tempo_at(piece, f64::from(sec.bar - 1) * BAR),
                climax: piece.climax == Some(part.section),
                intimate: piece.intimate == Some(part.section),
                seed: 0xA5C0_5EED ^ ((part.who as u64) << 8) ^ part.section as u64,
            };
            let role = (piece.role)(part.who, part.section);
            perform::perform(part, &written, length, role, &ctx)
        });
        // Ids from the project's counter, as the piano roll takes them.
        let mut scratch = app.project().clone();
        let mut clip = scratch.make_clip(sec.name, bar_tick(sec.bar), tick(length));
        let mut arts = Vec::new();
        match &perf {
            None => {
                for w in &written {
                    let n = scratch.make_note(tick(w.at), tick(w.len), w.key, w.vel);
                    if w.art.is_some() {
                        arts.push((n.id, w.art));
                    }
                    clip.notes.push(n);
                }
            }
            Some(p) => {
                for w in &p.notes {
                    let mut n = scratch.make_note(tick(w.at), tick(w.len), w.key, w.vel);
                    if w.bend != 0.0 {
                        // The intonation: a per-note bend, as the piano roll's MPE lane
                        // draws it.
                        n.expr = NoteExpr {
                            bend: vec![ExprPoint(0, ExprKind::Bend.store(w.bend))],
                            ..NoteExpr::default()
                        }
                        .boxed();
                    }
                    if w.art.is_some() {
                        arts.push((n.id, w.art));
                    }
                    clip.notes.push(n);
                }
            }
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
        let lane = |kind: LaneKind, pts: &[(f64, f32)]| {
            ClipLane::new(
                kind,
                pts.iter()
                    .map(|(b, v)| AutoPoint::new(tick(*b), *v))
                    .collect(),
            )
        };
        let lanes = match &perf {
            None if piece.solo => vec![lane(LaneKind::Cc(1), &[(0.0, 0.5)])],
            None => vec![lane(LaneKind::Cc(1), part.cc1)],
            Some(p) => {
                let mut l = vec![
                    lane(LaneKind::Cc(1), &p.cc1),
                    lane(LaneKind::Cc(11), &p.cc11),
                    lane(LaneKind::Cc(21), &p.cc21),
                ];
                if !p.cc65.is_empty() {
                    l.push(lane(LaneKind::Cc(65), &p.cc65));
                }
                l
            }
        };
        exec(
            app,
            "the controller lanes",
            Command::SetClipLanes {
                track,
                clip: id,
                lanes,
            },
        );
    }
    Built { tracks }
}
