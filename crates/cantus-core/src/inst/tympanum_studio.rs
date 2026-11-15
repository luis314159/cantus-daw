//! Cantus's modelled kits (round 40 phase 3, ADR 0124): the design's studio kit — its
//! pieces and microphones where the approved picture puts them (`design-p3/kit-model.json`,
//! metres there, centimetres here) — played by Pulsus's modal voices and heard through
//! virtual microphones that move. One model: what is drawn is what is heard.

use super::{
    Articulation, KitDef, MicDef, ModelledKit, Openness, PieceDef, PieceKind, Place, RoomDef,
};
use crate::inst::tympanum::{KitRef, MicPattern, RoomChoice};
use crate::inst_out::{OutputFamily, OutputId};

/// Cantus's modelled kits (instant, no download). The owner names them at a checkpoint.
pub fn modelled_kits() -> &'static [ModelledKit] {
    &[
        ModelledKit {
            id: "studio",
            label_key: "tympanum.kit.studio",
        },
        // Round 40: the same kit measured off Virtuosity Drums (CC0) — a modelled kit
        // whose every table comes from a public-domain recording.
        ModelledKit {
            id: "jazz",
            label_key: "tympanum.kit.jazz",
        },
    ]
}

fn art(id: &str, key: u8) -> Articulation {
    Articulation {
        id: id.to_string(),
        label: format!("tympanum.art.{id}"),
        key,
        openness: None,
    }
}

fn hat(id: &str, key: u8, at: f32) -> Articulation {
    Articulation {
        openness: Some(Openness { at }),
        ..art(id, key)
    }
}

#[allow(clippy::too_many_arguments)]
fn piece(
    index: u16,
    name: &str,
    kind: PieceKind,
    inches: f32,
    depth_m: f32,
    centre: [f32; 3],
    axis: [f32; 3],
    arts: Vec<Articulation>,
    choke: Option<u8>,
) -> PieceDef {
    // A kick is struck on its batter, the side facing the drummer (−axis in the design,
    // whose kick axis points at the audience); every other piece from above (+axis).
    let normal = if kind == PieceKind::Kick {
        [-axis[0], -axis[1], -axis[2]]
    } else {
        axis
    };
    let slug: String = name
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    PieceDef {
        index,
        name: name.to_string(),
        label: format!("tympanum.piece.{slug}"),
        kind,
        diameter: inches * 2.54,
        depth: depth_m * 100.0,
        place: Place::from_design(centre, normal),
        articulations: arts,
        choke,
    }
}

#[allow(clippy::too_many_arguments)]
fn mic(
    id: u16,
    name: &str,
    family: OutputFamily,
    pattern: MicPattern,
    p: [f32; 3],
    aim: [f32; 3],
    main_for: &[u16],
    pan: f32,
    level_db: f32,
    spread_cm: f32,
) -> MicDef {
    let n = (aim[0] * aim[0] + aim[1] * aim[1] + aim[2] * aim[2])
        .sqrt()
        .max(1e-6);
    MicDef {
        output: OutputId(id),
        name: name.to_string(),
        family,
        channels: if spread_cm > 0.0 { 2 } else { 1 },
        pattern,
        place: Place::from_design(p, [aim[0] / n, aim[1] / n, aim[2] / n]),
        main_for: main_for.to_vec(),
        movable: true,
        pan,
        level_db,
        spread: spread_cm,
    }
}

// The studio kit's mix (drums-gen 2, cue 1): its close mics carry the drums' attack and the
// overheads and rooms sit under them, as DRSKit's own mix does (through the engine: the
// snare's 2-16 kHz in its first 20 ms 3.6 dB under the whole from its top mic, 6.5 from the
// overheads; the ride's from its own mic). At +5 dB the overheads carried every drum's crack
// 3-4 ms late (the strongest 2-16 kHz arrival 4.5 ms after the first sound; DRSKit's 1 ms).
// The jazz kit keeps the spaced balance Virtuosity Drums was recorded with (`jazz`).
const RIDE_DB: f32 = 1.0;
const OVERHEADS_DB: f32 = -3.0;
const ROOM_NEAR_DB: f32 = -5.0;
const MONO_ROOM_DB: f32 = -9.0;

/// The studio kit: maple shells, a 22″ kick, 14″ snare, 10″/12″ racks, 14″/16″ floors,
/// 14″ hats, 16″ and 18″ crashes, a 20″ ride, a splash and a china — the design's kit.
fn studio() -> KitDef {
    use PieceKind as K;
    let pieces = vec![
        piece(
            0,
            "Kick",
            K::Kick,
            22.0,
            0.4,
            [0.0, 0.285, 0.18],
            [0.0, 0.0, 1.0],
            vec![art("center", 36), art("open", 35)],
            None,
        ),
        piece(
            1,
            "Snare",
            K::Snare,
            14.0,
            0.16,
            [0.34, 0.6, -0.06],
            [0.0581, 0.9877, -0.1452],
            vec![
                art("center", 38),
                art("rimshot", 40),
                art("sidestick", 37),
                // A player's figures (round 40 phase 3), on the keys past General MIDI's.
                art("flam", 91),
                art("drag", 92),
                art("buzz", 93),
                art("roll", 94),
            ],
            None,
        ),
        piece(
            2,
            "Tom 1",
            K::RackTom,
            10.0,
            0.2,
            [0.13, 0.8, 0.16],
            [0.0799, 0.9703, -0.2283],
            vec![art("center", 48), art("center2", 50)],
            None,
        ),
        piece(
            3,
            "Tom 2",
            K::RackTom,
            12.0,
            0.22,
            [-0.15, 0.82, 0.16],
            [-0.0799, 0.9703, -0.2283],
            vec![art("center", 45), art("center2", 47)],
            None,
        ),
        piece(
            4,
            "Floor 1",
            K::FloorTom,
            14.0,
            0.34,
            [-0.5, 0.5, -0.04],
            [0.025, 0.9962, -0.0835],
            vec![art("center", 43)],
            None,
        ),
        piece(
            5,
            "Floor 2",
            K::FloorTom,
            16.0,
            0.38,
            [-0.82, 0.48, -0.3],
            [0.039, 0.9962, -0.078],
            vec![art("center", 41)],
            None,
        ),
        piece(
            6,
            "Hi-hat",
            K::HiHat,
            14.0,
            0.0,
            [0.66, 0.92, -0.04],
            [0.0, 0.9994, -0.0349],
            vec![
                hat("closed", 42, 0.1),
                hat("pedal", 44, 0.0),
                hat("open", 46, 0.85),
                // The foot opening and closing the plates: they clash and ring.
                hat("splash", 95, 0.55),
            ],
            Some(1),
        ),
        piece(
            7,
            "Crash 1",
            K::Crash,
            16.0,
            0.0,
            [0.44, 1.38, 0.24],
            [-0.0671, 0.9397, -0.3354],
            vec![art("edge", 49)],
            None,
        ),
        piece(
            8,
            "Crash 2",
            K::Crash,
            18.0,
            0.0,
            [-0.36, 1.46, 0.3],
            [0.0983, 0.9397, -0.3276],
            vec![art("edge", 57)],
            None,
        ),
        piece(
            9,
            "Ride",
            K::Ride,
            20.0,
            0.0,
            [-0.78, 1.1, 0.14],
            [0.1082, 0.9703, -0.2164],
            vec![art("bow", 51), art("bell", 53), art("edge", 59)],
            None,
        ),
        piece(
            10,
            "Splash",
            K::Splash,
            10.0,
            0.0,
            [0.82, 1.28, -0.14],
            [-0.1418, 0.9613, -0.2364],
            vec![art("edge", 55)],
            None,
        ),
        piece(
            11,
            "China",
            K::China,
            18.0,
            0.0,
            [-1.18, 1.4, -0.26],
            [0.1581, 0.9613, -0.2258],
            vec![art("edge", 52)],
            None,
        ),
    ];
    use MicPattern as P;
    use OutputFamily as F;
    let mics = vec![
        mic(
            0,
            "Kick Out",
            F::Kick,
            P::Cardioid,
            [0.09, 0.26, 0.62],
            [0.0, 0.0, -1.0],
            &[0],
            0.0,
            5.0,
            0.0,
        ),
        mic(
            1,
            "Kick Sub",
            F::Kick,
            P::Omni,
            [-0.14, 0.28, 0.58],
            [0.0, 0.0, -1.0],
            &[0],
            0.0,
            -1.0,
            0.0,
        ),
        mic(
            2,
            "Snare Top",
            F::Snare,
            P::Supercardioid,
            [0.26, 0.72, 0.1],
            [0.3, -0.7, -0.6],
            &[1],
            0.0,
            5.0,
            0.0,
        ),
        mic(
            3,
            "Hi-hat",
            F::HiHat,
            P::Cardioid,
            [0.7, 1.12, 0.08],
            [0.0, -1.0, -0.2],
            &[6],
            0.55,
            1.0,
            0.0,
        ),
        mic(
            4,
            "Tom 1",
            F::Toms,
            P::Supercardioid,
            [0.2, 0.92, 0.26],
            [-0.3, -0.6, -0.6],
            &[2],
            0.2,
            3.0,
            0.0,
        ),
        mic(
            5,
            "Tom 2",
            F::Toms,
            P::Supercardioid,
            [-0.22, 0.94, 0.26],
            [0.3, -0.6, -0.6],
            &[3],
            -0.15,
            3.0,
            0.0,
        ),
        mic(
            6,
            "Floor 1",
            F::Toms,
            P::Supercardioid,
            [-0.44, 0.62, 0.16],
            [-0.3, -0.5, -0.6],
            &[4],
            -0.4,
            3.0,
            0.0,
        ),
        mic(
            7,
            "Floor 2",
            F::Toms,
            P::Supercardioid,
            [-0.74, 0.6, -0.06],
            [-0.3, -0.5, -0.6],
            &[5],
            -0.6,
            3.0,
            0.0,
        ),
        mic(
            8,
            "Ride",
            F::Cymbals,
            P::Cardioid,
            [-0.64, 1.24, 0.3],
            [-0.3, -1.0, -0.3],
            &[9],
            -0.5,
            RIDE_DB,
            0.0,
        ),
        mic(
            9,
            "Overheads",
            F::Overheads,
            P::Cardioid,
            [0.0, 1.96, -0.04],
            [0.0, -1.0, 0.0],
            &[],
            0.0,
            OVERHEADS_DB,
            56.0,
        ),
        mic(
            10,
            "Room Near",
            F::Rooms,
            P::Omni,
            [0.0, 1.5, 1.9],
            [0.0, -0.2, -1.0],
            &[],
            0.0,
            ROOM_NEAR_DB,
            230.0,
        ),
        mic(
            11,
            "Mono Room",
            F::Rooms,
            P::Figure8,
            [0.0, 0.92, 1.5],
            [0.0, -0.1, -1.0],
            &[],
            0.0,
            MONO_ROOM_DB,
            0.0,
        ),
    ];
    KitDef {
        kit: KitRef::Modelled("studio".into()),
        name: "Studio".into(),
        modelled: true,
        finish: crate::inst::chorda_look::Finish::Maple,
        pieces,
        mics,
        bleed: Default::default(),
        // The design's live room: 6 × 8 m, the kit 2.7 m in front of the wall behind it.
        room: RoomDef {
            width: 600.0,
            depth: 800.0,
            height: 340.0,
            back: 270.0,
        },
        default_room: RoomChoice::Own,
        licence: None,
        // Its cymbals and drums are synthesised from parameters measured off DRSKit's
        // recordings (CC BY 4.0: credited where credits are shown).
        credit: Some(
            "Measured from “DRSKit” by the DrumGizmo team and Jes Eiler (DRSDrums), CC BY 4.0"
                .into(),
        ),
    }
}

/// The jazz kit: the studio kit's pieces and microphones, its voices measured off
/// Virtuosity Drums (CC0 1.0: nothing to credit, credited all the same).
fn jazz() -> KitDef {
    let mut k = studio();
    k.kit = KitRef::Modelled("jazz".into());
    k.name = "Jazz".into();
    k.credit = Some("Measured from “Virtuosity Drums” (CC0 1.0)".into());
    k.finish = crate::inst::chorda_look::Finish::Walnut;
    // Virtuosity Drums is two mics and a pair: its overheads carry the kit (the toms
    // 9 dB over the snare mic), so the jazz kit's overheads and rooms stay up front.
    for m in &mut k.mics {
        m.level_db = match m.name.as_str() {
            "Ride" => -3.0,
            "Overheads" => 5.0,
            "Room Near" => 1.0,
            "Mono Room" => -5.0,
            _ => m.level_db,
        };
    }
    k
}

/// The definition of a modelled kit (off the audio thread: it allocates).
pub fn modelled_kit(id: &str) -> Option<KitDef> {
    match id {
        "studio" => Some(studio()),
        "jazz" => Some(jazz()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_studio_kit_matches_the_design_and_names_every_output_once() {
        let k = modelled_kit("studio").expect("the studio kit");
        assert_eq!(k.pieces.len(), 12);
        assert_eq!(k.mics.len(), 12);
        let mut keys = std::collections::HashSet::new();
        for p in &k.pieces {
            for a in &p.articulations {
                assert!(keys.insert(a.key), "key {} twice", a.key);
            }
        }
        for (i, m) in k.mics.iter().enumerate() {
            assert_eq!(m.output, OutputId(i as u16));
        }
        // The design's kick (0, 0.285, 0.18) m: x 0, y 18 cm towards the audience, z 28.5.
        let kick = k.pieces[0].place;
        assert!((kick.y - 18.0).abs() < 1e-3 && (kick.z - 28.5).abs() < 1e-3);
        // Its batter faces the drummer; the overheads point down.
        assert!(k.pieces[0].place.dir()[1] < -0.99);
        assert!(k.mics[9].place.dir()[2] < -0.99);
        assert_eq!(k.mics[9].channels, 2);
    }

    #[test]
    fn the_jazz_kit_is_the_studio_kit_on_virtuositys_tables() {
        let (s, j) = (
            modelled_kit("studio").unwrap(),
            modelled_kit("jazz").unwrap(),
        );
        assert_eq!(j.kit, KitRef::Modelled("jazz".into()));
        assert_eq!(j.pieces.len(), s.pieces.len());
        assert_eq!(j.mics.len(), s.mics.len());
        assert!(modelled_kits().iter().any(|k| k.id == "jazz"));
        assert_eq!(crate::inst::tympanum_kit::measured_set("jazz"), 16);
        assert_eq!(crate::inst::tympanum_kit::measured_set("studio"), 0);
        // Each kit says whose recordings its voices were measured from.
        assert!(s.credit.as_deref().is_some_and(|c| c.contains("DRSKit")));
        assert!(j
            .credit
            .as_deref()
            .is_some_and(|c| c.contains("Virtuosity")));
    }
}
