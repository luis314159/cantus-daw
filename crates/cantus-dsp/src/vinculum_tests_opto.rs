//! The opto cell against the published measurements of the classic electro-optical
//! leveller and of its CdS photocell (the owner's decision of 2026-11-17: "attack ~10 ms;
//! release in two stages — ~60 ms for about the first half, then 1–15 s for the rest —
//! with memory").
//!
//! The figures, and where each comes from:
//!
//! - **[UA-M]** Universal Audio, *Model LA-2A Leveling Amplifier* user's guide, rev. 1.3
//!   (2000), <https://www.adorama.com/col/productManuals/UALA2AD.pdf>. p. 1,
//!   Specifications: "Release Time: approximately 0.06 seconds for 50% release, 0.5 to 5
//!   seconds for complete release depending upon the amount of previous reduction";
//!   "Gain Reduction: up to 40 dB". p. 10: the cell "releases quickly (40-80
//!   milliseconds) to approximately half … The remainder of its release can take place
//!   over as much as several seconds"; its "memory": "the release time is slower if the
//!   unit has either been in compression for a while, or the amount of compression is
//!   large".
//! - **[UA-T]** Universal Audio, *Tips & Tricks — Teletronix LA-2A Classic Leveler
//!   Plug-In Collection*, <https://www.uaudio.com/blogs/ua/la-2a-collection-tips-tricks>:
//!   "the average Attack time is 10 milliseconds, and the Release time is about 60
//!   milliseconds for 50% of the release, and anywhere from 1 to 15 seconds for the rest".
//! - **[PE]** PerkinElmer Optoelectronics, *Photoconductive Cells* (the CdS cell
//!   handbook), <https://wp.optics.arizona.edu/milster/wp-content/uploads/sites/48/2016/06/APP_PhotocellIntroduction.pdf>,
//!   pp. 6–7: "All material types show faster speed at higher light levels"; the "light
//!   history effect … depends upon the new light level, and upon the time spent at each
//!   of these light levels. this effect is reversible."
//! - **[NMHR]** J. Najnudel, R. Müller, T. Hélie, D. Roze, *Power-balanced dynamic
//!   modeling of vactrols: application to a VTL5C3/2*, DAFx-23,
//!   <https://www.dafx.de/paper-archive/2023/DAFx23_paper_50.pdf>, §1 and §5.5: "the
//!   attack time decreases with the received light"; in their optical compressor "the
//!   higher the input voltage, the shorter the attack and the longer the release".
//!
//! "Release" is measured on the reduction the needle shows, in dB (the meter of [UA-M]
//! p. 3 reads "the amount of compression in dB"): half released is half the decibels,
//! "complete" is taken as 90 % (the reduction down to a tenth of where it was). Every
//! step is a steady level, so a step response is the measurement.

use super::super::*;
use super::{unit, SR};

/// The reduction, dB, sample by sample, for a list of (level, seconds) steps (`None` is
/// silence), from a new unit.
fn steps(s: &VinculumSettings, list: &[(Option<f32>, f32)]) -> Vec<f32> {
    let mut v = unit(s);
    let mut out = Vec::new();
    let (mut l, mut r) = ([0f32; 1], [0f32; 1]);
    for &(level, secs) in list {
        let x = level.map_or(0.0, crate::math::db_to_gain);
        for _ in 0..(secs * SR) as usize {
            l[0] = x;
            r[0] = x;
            v.process(&mut l, &mut r);
            out.push(v.gain_reduction_db());
        }
    }
    out
}

/// Seconds from `from` until the trace first satisfies `done`.
fn until(t: &[f32], from: usize, done: impl Fn(f32) -> bool) -> f32 {
    t[from..]
        .iter()
        .position(|g| done(*g))
        .map_or(f32::INFINITY, |at| at as f32 / SR)
}

fn opto(character: Character) -> VinculumSettings {
    VinculumSettings {
        model: Model::Opto,
        character,
        threshold_db: -20.0,
        ..VinculumSettings::default()
    }
}

/// Half and complete (90 %) release, s, after `on` seconds at `level` dBFS.
fn release(s: &VinculumSettings, level: f32, on: f32) -> (f32, f32, f32) {
    let t = steps(s, &[(Some(level), on), (None, 40.0)]);
    let at = (on * SR) as usize;
    let gr = t[at - 1];
    (
        gr,
        until(&t, at, |g| g <= 0.5 * gr),
        until(&t, at, |g| g <= 0.1 * gr),
    )
}

#[test]
fn the_opto_attack_is_the_published_ten_milliseconds() {
    // [UA-T]: "the average Attack time is 10 milliseconds". [PE], [NMHR]: the cell
    // quickens as it is lit harder. Measured to 63 % of the settled reduction, at 6, 16
    // and 26 dB over the threshold, both characters: each within 5 – 20 ms, their
    // geometric mean within 7 – 14 ms, never slower at a higher level.
    for character in [Character::Compress, Character::Limit] {
        let mut times = Vec::new();
        for over in [6.0f32, 16.0, 26.0] {
            let t = steps(&opto(character), &[(Some(-20.0 + over), 0.5)]);
            let gr = t[t.len() - 1];
            let a = until(&t, 0, |g| g >= 0.632 * gr) * 1000.0;
            println!("opto {character:?}, {over} dB over: {gr:.1} dB, attack to 63 % {a:.1} ms");
            assert!((5.0..=20.0).contains(&a), "{character:?} {over} dB: {a} ms");
            times.push(a);
        }
        let mean = crate::math::powf(times[0] * times[1] * times[2], 1.0 / 3.0);
        println!("opto {character:?}: the attack's geometric mean {mean:.1} ms");
        assert!(
            (7.0..=14.0).contains(&mean),
            "{character:?}: mean {mean} ms"
        );
        assert!(
            times[0] >= times[1] && times[1] >= times[2],
            "{character:?}: {times:?}"
        );
    }
}

#[test]
fn the_opto_lets_go_of_the_first_half_in_sixty_milliseconds() {
    // [UA-M] p. 1 "approximately 0.06 seconds for 50% release", p. 10 "40-80
    // milliseconds"; [UA-T] "about 60 milliseconds for 50%". Published with no
    // condition on the level or the time worked, so held at every one here: a gentle,
    // a moderate and a deep reduction, after 0.1, 1 and 10 s, both characters.
    for character in [Character::Compress, Character::Limit] {
        for level in [-12.0f32, -4.0, 6.0] {
            for on in [0.1f32, 1.0, 10.0] {
                let (gr, half, _) = release(&opto(character), level, on);
                let ms = half * 1000.0;
                println!("opto {character:?} {gr:.1} dB after {on} s: half in {ms:.0} ms");
                assert!(
                    (40.0..=80.0).contains(&ms),
                    "{character:?} at {gr} dB after {on} s: half in {ms} ms"
                );
            }
        }
    }
}

#[test]
fn the_opto_remembers_how_long_it_worked() {
    // [UA-M] p. 10: slower "if the unit has … been in compression for a while"; [UA-T]
    // "1 to 15 seconds for the rest"; [UA-M] p. 1 "0.5 to 5 seconds for complete
    // release". Complete release (to 10 % of the reduction) after 0.1, 1, 10 and 30 s at
    // the same level: always longer the longer it worked; a burst back within 0.5 – 2 s,
    // a long passage within 4 – 15 s, at least three times the burst's.
    for (character, level) in [(Character::Compress, -4.0f32), (Character::Limit, -4.0)] {
        let mut fulls = Vec::new();
        for on in [0.1f32, 1.0, 10.0, 30.0] {
            let (gr, _, full) = release(&opto(character), level, on);
            println!("opto {character:?} {gr:.1} dB for {on} s: complete in {full:.2} s");
            fulls.push(full);
        }
        assert!(
            fulls.windows(2).all(|w| w[1] > w[0]),
            "{character:?}: {fulls:?}"
        );
        assert!((0.5..=2.0).contains(&fulls[0]), "{character:?}: {fulls:?}");
        assert!((4.0..=15.0).contains(&fulls[3]), "{character:?}: {fulls:?}");
        assert!(fulls[3] >= 3.0 * fulls[0], "{character:?}: {fulls:?}");
    }
}

#[test]
fn the_opto_remembers_how_hard_it_worked() {
    // [UA-M] p. 10: slower "if … the amount of compression is large"; [UA-M] p. 1
    // "depending upon the amount of previous reduction". Ten seconds gentle (about 4 dB)
    // against ten seconds hard (about 15 dB), each character: the hard one lets go
    // measurably slower (complete release at least 1.5 times as long).
    for character in [Character::Compress, Character::Limit] {
        let (soft_gr, gentle) = {
            let s = opto(character);
            let level = if character == Character::Compress {
                -12.0
            } else {
                -16.0
            };
            let (gr, _, full) = release(&s, level, 10.0);
            (gr, full)
        };
        let (hard_gr, hard) = {
            let (gr, _, full) = release(&opto(character), 6.0, 10.0);
            (gr, full)
        };
        println!(
            "opto {character:?} for 10 s: {soft_gr:.1} dB complete in {gentle:.2} s, {hard_gr:.1} dB in {hard:.2} s"
        );
        assert!(soft_gr < 6.0 && hard_gr > 12.0, "{soft_gr} / {hard_gr}");
        assert!(hard >= 1.5 * gentle, "{character:?}: {gentle} s / {hard} s");
    }
}

#[test]
fn a_rested_cell_recovers_fast() {
    // [PE] p. 7: the light history "is reversible". After a long loud passage and half a
    // minute of silence, a burst lets go as a new cell's does (within 25 %); the same
    // burst a second after the passage still carries its memory (at least 1.5 times
    // slower).
    for character in [Character::Compress, Character::Limit] {
        let s = opto(character);
        let burst = |rest: f32| {
            let t = steps(
                &s,
                &[
                    (Some(-4.0), 10.0),
                    (None, rest),
                    (Some(-4.0), 0.1),
                    (None, 30.0),
                ],
            );
            let at = ((10.0 + rest + 0.1) * SR) as usize;
            let gr = t[at - 1];
            until(&t, at, |g| g <= 0.1 * gr)
        };
        let fresh = release(&s, -4.0, 0.1).2;
        let (rested, tired) = (burst(30.0), burst(1.0));
        println!(
            "opto {character:?}: a burst lets go in {fresh:.2} s new, {rested:.2} s after a rest, {tired:.2} s straight after"
        );
        assert!((rested - fresh).abs() <= 0.25 * fresh, "{rested} / {fresh}");
        assert!(tired >= 1.5 * fresh, "{tired} / {fresh}");
    }
}
