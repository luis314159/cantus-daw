//! A recorded kit's pieces from its keys (the General MIDI drum map), and where its pieces
//! and mics stand: a pack says which key is which drum, not where the drum is, so the
//! pieces take the places of the studio kit's (the design's), drum by drum — what the
//! kit's picture draws and the bleed matrix names.

use cantus_core::inst::tympanum::MicPattern;
use cantus_core::inst::tympanum_kit::{
    modelled_kit, Articulation, Openness, PieceDef, PieceKind, Place, RoomDef,
};
use cantus_core::inst_out::OutputFamily;

/// What a key is on the General MIDI drum map: its piece's group (keys of one group are
/// one piece), its kind and name, and the articulation's id and label.
fn gm(key: u8) -> (u16, PieceKind, &'static str, &'static str, &'static str) {
    use PieceKind as K;
    match key {
        35 => (0, K::Kick, "Kick", "kick-2", "Kick 2"),
        36 => (0, K::Kick, "Kick", "center", "Kick"),
        37 => (1, K::Snare, "Snare", "sidestick", "Side stick"),
        38 => (1, K::Snare, "Snare", "center", "Snare"),
        40 => (1, K::Snare, "Snare", "rimshot", "Rim shot"),
        39 => (20, K::Percussion, "Clap", "clap", "Clap"),
        42 => (2, K::HiHat, "Hi-hat", "closed", "Closed"),
        44 => (2, K::HiHat, "Hi-hat", "pedal", "Pedal"),
        46 => (2, K::HiHat, "Hi-hat", "open", "Open"),
        50 => (3, K::RackTom, "High tom", "center", "High tom"),
        48 => (4, K::RackTom, "High-mid tom", "center", "High-mid tom"),
        47 => (5, K::RackTom, "Mid tom", "center", "Mid tom"),
        45 => (6, K::RackTom, "Low tom", "center", "Low tom"),
        43 => (7, K::FloorTom, "Floor tom", "center", "Floor tom"),
        41 => (8, K::FloorTom, "Low floor tom", "center", "Low floor tom"),
        49 => (9, K::Crash, "Crash 1", "edge", "Crash"),
        57 => (10, K::Crash, "Crash 2", "edge", "Crash"),
        51 => (11, K::Ride, "Ride", "bow", "Ride"),
        59 => (11, K::Ride, "Ride", "edge", "Ride edge"),
        53 => (11, K::Ride, "Ride", "bell", "Ride bell"),
        52 => (12, K::China, "China", "edge", "China"),
        55 => (13, K::Splash, "Splash", "edge", "Splash"),
        54 => (14, K::Tambourine, "Tambourine", "hit", "Tambourine"),
        56 => (15, K::Cowbell, "Cowbell", "hit", "Cowbell"),
        _ => (100 + key as u16, K::Percussion, "Percussion", "hit", "Hit"),
    }
}

/// An articulation's piece, its index in the piece, and its openness.
pub type ArtPlace = (u16, u16, Option<f32>);

/// The pieces a kit's articulations make, and for each articulation (in `keys`' order) its
/// piece, its index in the piece, and its openness (a hi-hat's).
pub fn pieces_for(keys: &[(u8, Option<(u8, u8)>)]) -> (Vec<PieceDef>, Vec<ArtPlace>) {
    let studio = modelled_kit("studio").expect("the studio kit");
    let mut pieces: Vec<PieceDef> = Vec::new();
    let mut group_of: Vec<u16> = Vec::new();
    let mut place = Vec::new();
    let mut toms = 0usize;
    let mut crashes = 0usize;
    for &(key, range) in keys {
        let (group, kind, name, id, label) = gm(key);
        let pi = match group_of.iter().position(|g| *g == group) {
            Some(i) => i,
            None => {
                let template = studio_piece(&studio.pieces, kind, &mut toms, &mut crashes);
                let mut p = PieceDef {
                    index: pieces.len() as u16,
                    name: if group >= 100 {
                        format!("Percussion {key}")
                    } else {
                        name.to_string()
                    },
                    label: String::new(),
                    kind,
                    diameter: template.map_or(30.0, |t| t.diameter),
                    depth: template.map_or(20.0, |t| t.depth),
                    place: template.map_or(Place::default(), |t| t.place),
                    articulations: Vec::new(),
                    choke: (kind == PieceKind::HiHat).then_some(1),
                };
                p.articulations.clear();
                pieces.push(p);
                group_of.push(group);
                pieces.len() - 1
            }
        };
        // A hi-hat level on the openness controller: its id and label by how open it is.
        let openness = match (kind, range) {
            (PieceKind::HiHat, Some((lo, hi))) => Some((lo as f32 + hi as f32) / 2.0 / 127.0),
            (PieceKind::HiHat, None) => Some(if key == 46 { 1.0 } else { 0.0 }),
            _ => None,
        };
        let (aid, alabel) = match (range, openness) {
            (Some((lo, hi)), Some(o)) => (
                format!("open-{lo}-{hi}"),
                match o {
                    o if o < 0.25 => "Closed",
                    o if o < 0.5 => "Loose",
                    o if o < 0.75 => "Half open",
                    _ => "Open",
                }
                .to_string(),
            ),
            _ => (id.to_string(), label.to_string()),
        };
        let p = &mut pieces[pi];
        p.articulations.push(Articulation {
            id: aid,
            label: alabel,
            key,
            openness: openness.map(|at| Openness { at }),
        });
        place.push((pi as u16, (p.articulations.len() - 1) as u16, openness));
    }
    (pieces, place)
}

/// The studio kit's piece a recorded piece of `kind` stands in for (toms and crashes in
/// order).
fn studio_piece<'a>(
    studio: &'a [PieceDef],
    kind: PieceKind,
    toms: &mut usize,
    crashes: &mut usize,
) -> Option<&'a PieceDef> {
    match kind {
        PieceKind::RackTom | PieceKind::FloorTom => {
            let all: Vec<&PieceDef> = studio
                .iter()
                .filter(|p| matches!(p.kind, PieceKind::RackTom | PieceKind::FloorTom))
                .collect();
            let p = all.get((*toms).min(all.len().saturating_sub(1))).copied();
            *toms += 1;
            p
        }
        PieceKind::Crash => {
            let all: Vec<&PieceDef> = studio.iter().filter(|p| p.kind == kind).collect();
            let p = all
                .get((*crashes).min(all.len().saturating_sub(1)))
                .copied();
            *crashes += 1;
            p
        }
        _ => studio.iter().find(|p| p.kind == kind),
    }
}

/// Whether a close mic of `family` is the main mic of a piece of `kind` (its bleed fader
/// leaves that piece alone).
pub fn close_mic_hears(family: OutputFamily, kind: PieceKind) -> bool {
    use OutputFamily as F;
    use PieceKind as K;
    match family {
        F::Kick => kind == K::Kick,
        F::Snare => kind == K::Snare,
        F::HiHat => kind == K::HiHat,
        F::Toms => matches!(kind, K::RackTom | K::FloorTom),
        F::Cymbals => matches!(kind, K::Ride | K::Crash | K::Splash | K::China | K::Stack),
        _ => false,
    }
}

/// A recorded mic's pattern as the picture draws it.
pub fn pattern_of(family: OutputFamily) -> MicPattern {
    match family {
        OutputFamily::Rooms => MicPattern::Omni,
        OutputFamily::Overheads | OutputFamily::Cymbals => MicPattern::Cardioid,
        _ => MicPattern::Supercardioid,
    }
}

/// Where a recorded mic is drawn: the studio kit's mic of its family.
pub fn mic_place(family: OutputFamily, i: usize) -> Place {
    let studio = modelled_kit("studio").expect("the studio kit");
    studio
        .mics
        .iter()
        .find(|m| m.family == family)
        .or_else(|| studio.mics.get(i % studio.mics.len()))
        .map(|m| m.place)
        .unwrap_or_default()
}

/// The room a recorded kit is drawn in: the studio's.
pub fn studio_room() -> RoomDef {
    modelled_kit("studio").expect("the studio kit").room
}
