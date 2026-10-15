//! The drums, from text grids: a heartbeat (a kick's *lub-dub*) opens the song and comes
//! back in the breakdown and the outro; the verse starts half-time; the pre-chorus drops
//! the kick for a snare roll and a silent last beat; the choruses go four on the floor.

use super::{Band, CLAP, CRASH, FINAL, HAT, KICK, OPEN, RIM, SHAKER, SNARE, TOM_HI, TOM_LO, TOM_MID};
use crate::demos::parts::{bars, drums, swell};
use crate::demos::song::{section as name_of, Song};
use crate::demos::theory::Ev;

const Q: f64 = 0.25;

/// One bar of a snare roll that tightens bar by bar into the drop: eighths, then
/// sixteenths, louder all the way.
fn roll(stage: usize) -> Vec<Ev> {
    let grid = if stage == 0 {
        "x.x.x.x.x.x.x.x."
    } else {
        "xxxxxxxxxxxx...."
    };
    let from = 0.5 + stage as f64 * 0.25;
    swell(&drums(&[(grid, SNARE)], Q), 4.0, from, from + 0.25)
}

fn clip(s: &mut Song, part: crate::demos::song::Part, name: &str, at: f64, len: f64, v: &[Ev]) {
    if !v.is_empty() {
        s.clip(part, name, at, len, v);
    }
}

pub fn section(s: &mut Song, b: &Band, key: &str, at: f64, len: f64) {
    let n = len as usize;
    let name = name_of(key);
    let last = at >= FINAL;
    let heart = |loud: bool| {
        drums(
            &[(if loud { "X..o............" } else { "o..g............" }, KICK)],
            Q,
        )
    };

    let kick: Vec<Ev> = bars(n, 4.0, |i| match key {
        "intro" if i >= 4 => heart(false),
        "verse" if i < 8 => drums(&[("X......x..x.....", KICK)], Q),
        "verse" if i == n - 1 => drums(&[("X.....x.X...x.x.", KICK)], Q),
        "verse" => drums(&[("X.....x.X.......", KICK)], Q),
        "prechorus" if i < 6 => drums(&[("X...x...X...x...", KICK)], Q),
        "chorus" => drums(&[("X...x...X...x...", KICK)], Q),
        "break" if i >= 4 => heart(true),
        "outro" if i < 4 => drums(&[("X......x........", KICK)], Q),
        "outro" if i < 7 => heart(false),
        _ => vec![],
    });
    clip(s, b.kick, &name, at, len, &kick);

    let snare: Vec<Ev> = bars(n, 4.0, |i| match key {
        "verse" if i < 8 => drums(&[("........o.......", SNARE)], Q),
        "verse" => drums(
            &[("....o.......o...", SNARE), ("....g.......g...", CLAP)],
            Q,
        ),
        "prechorus" if i >= 6 => roll(i - 6),
        "prechorus" => drums(
            &[("....x.......x...", SNARE), ("....o.......o...", CLAP)],
            Q,
        ),
        "chorus" => drums(
            &[
                (
                    if i % 4 == 3 {
                        "....X.......X..g"
                    } else {
                        "....X.......X..."
                    },
                    SNARE,
                ),
                ("....x.......x...", CLAP),
            ],
            Q,
        ),
        "break" if i >= 6 => roll(i - 6),
        "outro" if i < 4 => drums(&[("........o.......", SNARE)], Q),
        _ => vec![],
    });
    clip(s, b.snare, &name, at, len, &snare);

    let hats: Vec<Ev> = bars(n, 4.0, |i| match key {
        "verse" if i < 8 => drums(&[("o.g.o.g.o.g.o.g.", HAT)], Q),
        "verse" => drums(&[("x.o.x.oox.o.x.og", HAT)], Q),
        "prechorus" if i < 4 => drums(&[("x.o.x.o.x.o.x.o.", HAT)], Q),
        "prechorus" if i == n - 1 => drums(&[("xoooxoooxooo....", HAT)], Q),
        "prechorus" => drums(&[("xoooxoooxoooxooo", HAT)], Q),
        "chorus" => drums(
            &[("xo.oxo.oxo.oxo.o", HAT), ("..x...x...x...x.", OPEN)],
            Q,
        ),
        "outro" if i < 4 => drums(&[("o.g.o.g.o.g.o.g.", HAT)], Q),
        _ => vec![],
    });
    clip(s, b.hats, &name, at, len, &hats);

    let perc: Vec<Ev> = bars(n, 4.0, |i| match key {
        "intro" if i >= 4 => drums(&[("..g...g...g...g.", SHAKER)], Q),
        "verse" if i < 8 => drums(&[("g.o.g.o.g.o.g.o.", SHAKER)], Q),
        "verse" => drums(
            &[("g.o.g.o.g.o.g.o.", SHAKER), ("...o......o.....", RIM)],
            Q,
        ),
        "prechorus" if i < 7 => drums(&[("gogogogogogogogo", SHAKER)], Q),
        "chorus" => drums(
            &[
                ("gogogogogogogogo", SHAKER),
                (
                    if i % 2 == 0 {
                        "...o..o....o.o.."
                    } else {
                        "...o..o......o.o"
                    },
                    RIM,
                ),
            ],
            Q,
        ),
        _ => vec![],
    });
    clip(s, b.perc, &name, at, len, &perc);

    // Crashes on the downbeats that open something; tom fills at the ends of phrases.
    let fills: Vec<Ev> = bars(n, 4.0, |i| {
        let mut v = Vec::new();
        let opens = match key {
            "verse" | "outro" => i == 0,
            "chorus" => i % 8 == 0,
            _ => false,
        };
        if opens {
            v.extend(drums(&[("X...............", CRASH)], Q));
        }
        if key == "chorus" && i % 8 == 7 && !(last && i == n - 1) {
            v.extend(drums(
                &[
                    ("..........x.....", TOM_HI),
                    ("............o.x.", TOM_MID),
                    ("..............xx", TOM_LO),
                ],
                Q,
            ));
        }
        if key == "verse" && i == n - 1 {
            v.extend(drums(&[("............o.x.", TOM_MID), ("..............xX", TOM_LO)], Q));
        }
        v
    });
    clip(s, b.fills, &name, at, len, &fills);
}
