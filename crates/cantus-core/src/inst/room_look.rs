//! The room's look (round 40 phase 3, ADR 0126 amended, schema 69): the room an
//! instrument's scene stands in, and its floor, walls and rug — four text keys kept with the
//! instrument beside the piano's look (ADR 0104 §2), so a song reopens in the room it was
//! shown in. **Never audible**: text keys, read by no engine. A floor, walls or rug absent
//! (or a word this build does not know) is the room's own; a room absent is the one an older
//! song showed (Chorda's: from its Room size control) — a new Chorda writes the stage.

use super::NativeSpec;

/// The text keys.
pub const FLOOR_KEY: &str = "look.floor";
pub const WALLS_KEY: &str = "look.walls";
pub const RUG_KEY: &str = "look.rug";
pub const ROOM_KEY: &str = "look.room";
pub const KEYS: [&str; 4] = [ROOM_KEY, FLOOR_KEY, WALLS_KEY, RUG_KEY];

macro_rules! words {
    ($(#[$m:meta])* $name:ident $p:literal { $($(#[$vm:meta])* $v:ident = $k:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vm])* $v),+
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$v),+];

            /// The stored word (forever once shipped).
            pub fn key(self) -> &'static str {
                match self {
                    $($name::$v => $k),+
                }
            }

            pub fn from_key(k: &str) -> Option<$name> {
                Self::ALL.iter().copied().find(|x| x.key() == k)
            }

            /// Its name's string key.
            pub fn label_key(self) -> &'static str {
                match self {
                    $($name::$v => concat!($p, $k)),+
                }
            }
        }
    };
}

words!(
    /// The room the scene stands in.
    RoomKind "scene.room." {
        /// A pool of light, the dark around it (the default of a new instrument).
        Stage = "stage",
        Booth = "booth",
        Studio = "studio",
        Hall = "hall",
        Church = "church",
    }
);

words!(
    /// What the floor is laid with.
    Floor "scene.floor." {
        LightOak = "light_oak",
        HoneyOak = "honey_oak",
        DarkWalnut = "dark_walnut",
        StageWalnut = "stage_walnut",
        EbonyStage = "ebony_stage",
        Concrete = "concrete",
        Terrazzo = "terrazzo",
        Flagstone = "flagstone",
    }
);

words!(
    /// What the walls are made of.
    Walls "scene.walls." {
        WarmPlaster = "warm_plaster",
        WarmWood = "warm_wood",
        WalnutPanels = "walnut_panels",
        Plaster = "plaster",
        Charcoal = "charcoal",
        BlueFabric = "blue_fabric",
        GreenFabric = "green_fabric",
        Brick = "brick",
        HallWood = "hall_wood",
        Limestone = "limestone",
        /// The stage's black velvet.
        StageDark = "stage_dark",
    }
);

words!(
    /// The rug under the instrument.
    Rug "scene.rug." {
        Red = "red",
        Blue = "blue",
        Grey = "grey",
        /// No rug.
        Bare = "none",
    }
);

/// The room's look: each `None` the room's own default.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct RoomLook {
    pub room: Option<RoomKind>,
    pub floor: Option<Floor>,
    pub walls: Option<Walls>,
    pub rug: Option<Rug>,
}

/// The room's look a spec shows.
pub fn of(spec: &NativeSpec) -> RoomLook {
    RoomLook {
        room: spec.text(ROOM_KEY).and_then(RoomKind::from_key),
        floor: spec.text(FLOOR_KEY).and_then(Floor::from_key),
        walls: spec.text(WALLS_KEY).and_then(Walls::from_key),
        rug: spec.text(RUG_KEY).and_then(Rug::from_key),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_round_trip_and_are_unique() {
        for f in Floor::ALL {
            assert_eq!(Floor::from_key(f.key()), Some(*f));
        }
        for w in Walls::ALL {
            assert_eq!(Walls::from_key(w.key()), Some(*w));
        }
        for r in Rug::ALL {
            assert_eq!(Rug::from_key(r.key()), Some(*r));
        }
        for r in RoomKind::ALL {
            assert_eq!(RoomKind::from_key(r.key()), Some(*r));
        }
        assert_eq!(Floor::from_key("lava"), None);
    }
}

#[cfg(test)]
mod schema69 {
    use super::*;
    use crate::InstrumentKind;

    #[test]
    fn a_new_chorda_stands_on_the_stage_and_an_old_one_keeps_its_room() {
        let fresh = NativeSpec::new(InstrumentKind::Chorda);
        assert_eq!(of(&fresh).room, Some(RoomKind::Stage));
        // An older song's Chorda has no key: its room comes from its Room size, as before.
        let old: NativeSpec = serde_json::from_str(r#"{"kind":"Chorda","params":{}}"#).unwrap();
        assert_eq!(of(&old), RoomLook::default());
        for key in KEYS {
            let k = InstrumentKind::Chorda;
            assert!(k.is_text_key(key) && k.param(key).is_none(), "{key}");
        }
        assert!(!InstrumentKind::Organ.is_text_key(FLOOR_KEY));
    }

    #[test]
    fn tympanum_keeps_its_room_look_and_a_new_one_shows_its_kits_own_room() {
        let k = InstrumentKind::AcousticDrums;
        for key in KEYS {
            assert!(k.is_text_key(key) && k.param(key).is_none(), "{key}");
        }
        // No room kept: the scene draws the kit's own room (the stage for a modelled kit).
        assert_eq!(of(&NativeSpec::new(k)), RoomLook::default());
    }
}
