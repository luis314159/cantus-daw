//! What Tympanum's blind kit plays: eight pieces alone at their dynamics and four grooves,
//! the same MIDI through every voice. Times are in beats of the item's tempo (the pieces' at
//! 60 BPM: a beat a second).

/// Which modelled kit an item compares with the recording it was measured from (ADR 0124):
/// the studio kit with DRSKit (its hybrid pack fitted on DRSKit), the jazz kit with
/// Virtuosity Drums (its pack fitted on Virtuosity).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Pair {
    Studio,
    Jazz,
}

impl Pair {
    /// The modelled kit (`model:<id>`, the id `hybrid_pack` knows).
    pub fn model(self) -> &'static str {
        match self {
            Pair::Studio => "studio",
            Pair::Jazz => "jazz",
        }
    }
    /// The recorded kit (`pack:<pack>/<kit>`).
    pub fn recording(self) -> &'static str {
        match self {
            Pair::Studio => "drskit-tympanum/kit",
            Pair::Jazz => "virtuosity-drums/kit",
        }
    }
}

/// One note: when (beats), the key, the velocity.
pub type Hit = (f64, u8, u8);

pub struct Item {
    /// The page's id (never says which kit or voice).
    pub id: &'static str,
    pub pair: Pair,
    pub bpm: f64,
    pub hits: Vec<Hit>,
    /// The hi-hat pedal (CC4, 0 closed … 1 open) from each time on (beats): the first point
    /// holds from the start.
    pub pedal: Vec<(f64, f32)>,
    /// Where the played range ends (beats); the tail rings after it.
    pub end: f64,
    pub tail_s: f64,
}

// The keys (General MIDI; what both kits of a pair play the same piece with).
const KICK: u8 = 36;
const SIDESTICK: u8 = 37;
const SNARE: u8 = 38;
const RIMSHOT: u8 = 40;
const TOM_HIGH: u8 = 48;
const TOM_MID: u8 = 45;
const TOM_FLOOR: u8 = 41;
const HAT_CLOSED: u8 = 42;
const HAT_PEDAL: u8 = 44;
const HAT_OPEN: u8 = 46;
const CRASH: u8 = 49;
const RIDE: u8 = 51;

/// A piece alone: `keys × vels` (every key at the first velocity, then at the next), one
/// every `gap` seconds, the pedal at `pedal`.
fn piece(id: &'static str, keys: &[u8], vels: &[u8], gap: f64, pedal: f32, tail_s: f64) -> Item {
    let mut hits = Vec::new();
    for &v in vels {
        for &k in keys {
            hits.push((hits.len() as f64 * gap, k, v));
        }
    }
    let end = hits.last().map_or(0.0, |h| h.0) + 0.25;
    Item {
        id,
        pair: Pair::Studio,
        bpm: 60.0,
        hits,
        pedal: vec![(0.0, pedal)],
        end,
        tail_s,
    }
}

/// A beat position: bar, sixteenth (a fraction allowed: 32nds).
fn at(bar: i32, s16: f64) -> f64 {
    bar as f64 * 4.0 + s16 / 4.0
}

/// Pop / worship, 74 BPM: two bars of verse on the closed hats, a lift on looser hats with
/// a crash, a tom fill, the last crash ringing.
fn pop() -> Item {
    let mut h: Vec<Hit> = Vec::new();
    let mut pedal = vec![(0.0, 0.0)];
    for bar in 0..4 {
        let lift = bar >= 2;
        let hats_until = if bar == 3 { 8 } else { 16 };
        for s in (0..hats_until).step_by(2) {
            let v = if s % 4 == 0 { 74 } else { 54 };
            if lift {
                h.push((at(bar, s as f64), HAT_OPEN, v + 8));
            } else {
                h.push((at(bar, s as f64), HAT_CLOSED, v));
            }
        }
        let kicks: &[(i32, u8)] = if lift {
            &[(0, 110), (6, 84), (8, 100), (10, 78)]
        } else {
            &[(0, 100), (8, 90), (10, 70)]
        };
        for &(s, v) in kicks {
            h.push((at(bar, s as f64), KICK, v));
        }
        h.push((at(bar, 4.0), SNARE, if lift { 110 } else { 98 }));
        if bar < 3 {
            h.push((at(bar, 12.0), SNARE, if lift { 112 } else { 102 }));
        }
        if bar == 1 {
            h.push((at(bar, 15.0), SNARE, 30));
        }
    }
    // The lift: a crash on its one, the hats a little open.
    h.push((at(2, 0.0), CRASH, 104));
    pedal.push((at(2, 0.0) - 0.05, 0.35));
    // The fill over bar four's last two beats.
    for (s, k, v) in [
        (8.0, SNARE, 72),
        (9.0, SNARE, 82),
        (10.0, TOM_HIGH, 92),
        (11.0, TOM_HIGH, 86),
        (12.0, TOM_MID, 96),
        (13.0, TOM_MID, 90),
        (14.0, TOM_FLOOR, 106),
        (15.0, TOM_FLOOR, 112),
    ] {
        h.push((at(3, s), k, v));
    }
    pedal.push((at(4, 0.0) - 0.1, 0.0));
    h.push((at(4, 0.0), CRASH, 118));
    h.push((at(4, 0.0), KICK, 114));
    Item {
        id: "pop",
        pair: Pair::Studio,
        bpm: 74.0,
        hits: h,
        pedal,
        end: at(4, 1.0),
        tail_s: 3.5,
    }
}

/// Rock, 116 BPM: a crash in, eighths on the closed hats, the backbeat hard, an open hat
/// closed by the foot, a snare-and-toms fill, the last crash.
fn rock() -> Item {
    let mut h: Vec<Hit> = Vec::new();
    let mut pedal = vec![(0.0, 0.0)];
    h.push((0.0, CRASH, 112));
    for bar in 0..4 {
        let hats_until = if bar == 3 { 8 } else { 16 };
        for s in (0..hats_until).step_by(2) {
            if bar == 1 && s == 14 {
                continue;
            }
            let v = if s % 4 == 0 { 96 } else { 72 };
            h.push((at(bar, s as f64), HAT_CLOSED, v));
        }
        for (s, v) in [(0, 116), (6, 96), (8, 110), (10, 92)] {
            if bar == 3 && s == 10 {
                continue;
            }
            h.push((at(bar, s as f64), KICK, v));
        }
        h.push((at(bar, 4.0), SNARE, 120));
        if bar < 3 {
            h.push((at(bar, 12.0), SNARE, 122));
        }
    }
    // Bar two's last eighth: the hat open, the foot closing it on bar three's one.
    pedal.push((at(1, 14.0) - 0.05, 0.85));
    h.push((at(1, 14.0), HAT_OPEN, 100));
    pedal.push((at(2, 0.0) - 0.02, 0.0));
    h.push((at(2, 0.0), HAT_PEDAL, 80));
    for (s, k, v) in [
        (8.0, SNARE, 96),
        (9.0, SNARE, 86),
        (10.0, SNARE, 102),
        (11.0, SNARE, 108),
        (12.0, TOM_HIGH, 112),
        (13.0, TOM_HIGH, 102),
        (14.0, TOM_MID, 112),
        (15.0, TOM_FLOOR, 120),
    ] {
        h.push((at(3, s), k, v));
    }
    h.push((at(4, 0.0), CRASH, 122));
    h.push((at(4, 0.0), KICK, 120));
    Item {
        id: "rock",
        pair: Pair::Studio,
        bpm: 116.0,
        hits: h,
        pedal,
        end: at(4, 1.0),
        tail_s: 3.5,
    }
}

/// Bossa nova, 126 BPM: the side stick on the clave (2-bar), the kick's dotted pulse, eighths
/// on the closed hats and then the ride, a soft tom phrase, a soft crash.
fn bossa() -> Item {
    let mut h: Vec<Hit> = Vec::new();
    for bar in 0..4 {
        let until = if bar == 3 { 8 } else { 16 };
        for s in (0..until).step_by(2) {
            let accent = s % 4 == 0;
            if bar < 2 {
                h.push((at(bar, s as f64), HAT_CLOSED, if accent { 60 } else { 44 }));
            } else {
                h.push((at(bar, s as f64), RIDE, if accent { 66 } else { 50 }));
            }
        }
        for (s, v) in [(0, 82), (6, 56), (8, 82), (14, 56)] {
            if bar == 3 && s == 14 {
                continue;
            }
            h.push((at(bar, s as f64), KICK, v));
        }
        let clave: &[i32] = if bar % 2 == 0 { &[0, 6, 12] } else { &[4, 10] };
        for &s in clave {
            if bar == 3 && s >= 8 {
                continue;
            }
            h.push((at(bar, s as f64), SIDESTICK, 88));
        }
    }
    for (s, k, v) in [
        (8.0, TOM_HIGH, 72),
        (10.0, TOM_MID, 76),
        (12.0, TOM_FLOOR, 82),
        (14.0, TOM_FLOOR, 88),
    ] {
        h.push((at(3, s), k, v));
    }
    h.push((at(4, 0.0), CRASH, 92));
    h.push((at(4, 0.0), KICK, 88));
    Item {
        id: "bossa",
        pair: Pair::Studio,
        bpm: 126.0,
        hits: h,
        pedal: vec![(0.0, 0.0)],
        end: at(4, 1.0),
        tail_s: 3.0,
    }
}

/// A ballad, 66 BPM, on the jazz kit: the ride's eighths, a half-time snare, the foot on two
/// and four, and two crash swells (32nds rising from pp) each landing on a crash.
fn ballad() -> Item {
    let mut h: Vec<Hit> = Vec::new();
    for bar in 0..4 {
        let swell = bar % 2 == 1;
        let until = if swell { 8 } else { 16 };
        for s in (0..until).step_by(2) {
            h.push((at(bar, s as f64), RIDE, if s % 4 == 0 { 72 } else { 54 }));
        }
        // A swell lands on bar three's one (and the last bar's).
        h.push((at(bar, 0.0), KICK, if bar == 2 { 100 } else { 84 }));
        h.push((at(bar, 10.0), KICK, 60));
        if !swell {
            h.push((at(bar, 8.0), SNARE, 82));
        }
        for s in [4.0, 12.0] {
            h.push((at(bar, s), HAT_PEDAL, 56));
        }
        if swell {
            // Sixteen 32nds over beats three and four, pp to f.
            for i in 0..16 {
                let v = 18 + (78 * i / 15) as u8;
                h.push((at(bar, 8.0 + i as f64 * 0.5), CRASH, v));
            }
        }
    }
    h.push((at(2, 0.0), CRASH, 112));
    h.push((at(4, 0.0), CRASH, 120));
    h.push((at(4, 0.0), KICK, 104));
    Item {
        id: "ballad",
        pair: Pair::Jazz,
        bpm: 66.0,
        hits: h,
        pedal: vec![(0.0, 0.0)],
        end: at(4, 1.0),
        tail_s: 4.5,
    }
}

/// Every item, in the page's order.
pub fn all() -> Vec<Item> {
    vec![
        piece("kick", &[KICK], &[45, 85, 125], 1.6, 0.0, 2.0),
        piece("snare", &[SNARE], &[45, 85, 125], 1.6, 0.0, 2.0),
        piece("rimshot", &[RIMSHOT], &[70, 100, 127], 1.6, 0.0, 2.0),
        piece(
            "toms",
            &[TOM_HIGH, TOM_MID, TOM_FLOOR],
            &[80, 120],
            1.3,
            0.0,
            2.5,
        ),
        piece("hat_closed", &[HAT_CLOSED], &[45, 85, 125], 0.9, 0.0, 1.5),
        piece("hat_open", &[HAT_OPEN], &[45, 85, 125], 2.5, 0.95, 3.0),
        piece("crash", &[CRASH], &[50, 90, 125], 4.0, 0.0, 5.0),
        piece("ride", &[RIDE], &[45, 80, 115], 2.5, 0.0, 4.0),
        pop(),
        rock(),
        bossa(),
        ballad(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_item_is_named_plainly_and_once() {
        let items = all();
        let mut ids: Vec<&str> = items.iter().map(|i| i.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), items.len());
        // An id is in the page's data: it never says which kit or voice.
        for i in &items {
            for w in ["jazz", "studio", "drs", "virt", "model", "hyb", "rec", "tymp"] {
                assert!(!i.id.contains(w), "{} says {w}", i.id);
            }
        }
    }

    #[test]
    fn every_hit_is_inside_its_range_and_valid() {
        for i in all() {
            assert!(!i.hits.is_empty());
            for &(t, k, v) in &i.hits {
                assert!(t >= 0.0 && t < i.end, "{} at {t}", i.id);
                assert!((35..=81).contains(&k) && (1..=127).contains(&v), "{}", i.id);
            }
            assert!(i.pedal.first().is_some_and(|p| p.0 == 0.0));
            assert!(i.pedal.windows(2).all(|w| w[0].0 < w[1].0), "{}", i.id);
        }
    }

    #[test]
    fn the_swells_rise() {
        let b = ballad();
        let crash: Vec<u8> = b
            .hits
            .iter()
            .filter(|h| h.1 == CRASH && h.0 < at(2, 0.0))
            .map(|h| h.2)
            .collect();
        assert_eq!(crash.len(), 16);
        assert!(crash.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!((crash[0], crash[15]), (18, 96));
    }
}
