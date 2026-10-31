use super::chord::Role;
use super::part::{ArpDirection, BassStyle, Rhythm};
use super::*;
use crate::scale::{Key, ScaleKind};
use crate::tempo::TempoMap;
use crate::time::{Ticks, TimeSignature, PPQ};

fn pcs(sym: &str) -> Vec<u8> {
    let c = parse_chord(sym).unwrap_or_else(|e| panic!("{e}"));
    let mut v: Vec<u8> = c.tones.iter().map(|t| (c.root + t.interval) % 12).collect();
    v.sort_unstable();
    v
}

fn map44() -> TempoMap {
    let mut p = crate::Project::new("t");
    p.time_signature = TimeSignature::new(4, 4);
    p.tempo_map()
}

const BAR: i64 = PPQ * 4;

#[test]
fn chord_symbols_read_as_a_lead_sheet_writes_them() {
    // C=0 D=2 E=4 F=5 G=7 A=9 B=11.
    assert_eq!(pcs("C"), vec![0, 4, 7]);
    assert_eq!(pcs("Cm"), vec![0, 3, 7]);
    assert_eq!(pcs("C-7"), vec![0, 3, 7, 10]);
    assert_eq!(pcs("Cmaj7"), vec![0, 4, 7, 11]);
    assert_eq!(pcs("CM7"), vec![0, 4, 7, 11]);
    assert_eq!(pcs("C7"), vec![0, 4, 7, 10]);
    assert_eq!(pcs("CmMaj7"), vec![0, 3, 7, 11]);
    assert_eq!(pcs("Cm(maj7)"), vec![0, 3, 7, 11]);
    assert_eq!(pcs("Cdim"), vec![0, 3, 6]);
    assert_eq!(pcs("Cdim7"), vec![0, 3, 6, 9]);
    assert_eq!(pcs("C°7"), vec![0, 3, 6, 9]);
    assert_eq!(pcs("Cm7b5"), vec![0, 3, 6, 10]);
    assert_eq!(pcs("Cø7"), vec![0, 3, 6, 10]);
    assert_eq!(pcs("Caug"), vec![0, 4, 8]);
    assert_eq!(pcs("C+"), vec![0, 4, 8]);
    assert_eq!(pcs("Csus4"), vec![0, 5, 7]);
    assert_eq!(pcs("Csus2"), vec![0, 2, 7]);
    assert_eq!(pcs("C7sus4"), vec![0, 5, 7, 10]);
    assert_eq!(pcs("C6"), vec![0, 4, 7, 9]);
    assert_eq!(pcs("Cm6"), vec![0, 3, 7, 9]);
    assert_eq!(pcs("C6/9"), vec![0, 2, 4, 7, 9]);
    assert_eq!(pcs("Cadd9"), vec![0, 2, 4, 7]);
    assert_eq!(pcs("C9"), vec![0, 2, 4, 7, 10]);
    assert_eq!(pcs("Cm9"), vec![0, 2, 3, 7, 10]);
    assert_eq!(pcs("Cmaj9"), vec![0, 2, 4, 7, 11]);
    // A dominant eleventh drops its third; a minor eleventh keeps it.
    assert_eq!(pcs("C11"), vec![0, 2, 5, 7, 10]);
    assert_eq!(pcs("Cm11"), vec![0, 2, 3, 5, 7, 10]);
    assert_eq!(pcs("C13"), vec![0, 2, 4, 7, 9, 10]);
    assert_eq!(pcs("C7(b9,#11)"), vec![0, 1, 4, 6, 7, 10]);
    assert_eq!(pcs("C7#9"), vec![0, 3, 4, 7, 10]);
    assert_eq!(pcs("C7b5"), vec![0, 4, 6, 10]);
    assert_eq!(pcs("C5"), vec![0, 7]);
    assert_eq!(pcs("C7alt"), vec![0, 1, 3, 4, 6, 8, 10]);
    // Roots with accidentals; slash basses.
    assert_eq!(parse_chord("F#m7").unwrap().root, 6);
    assert_eq!(parse_chord("Dbmaj7").unwrap().root, 1);
    let c = parse_chord("Eb/G").unwrap();
    assert_eq!((c.root, c.bass), (3, 7));
    assert!(c.contains(67) && c.contains(70) && !c.contains(68));
    assert_eq!(parse_chord("Ab/C").unwrap().bass, 0);
    // Errors say what is wrong.
    assert!(parse_chord("H7").is_err());
    assert!(parse_chord("cm").is_err(), "a root is upper case");
    assert!(parse_chord("C/X").is_err());
    assert!(parse_chord("Cq").is_err());
    assert!(parse_chord("C8").is_err());
}

#[test]
fn the_third_and_seventh_come_first_and_the_fifth_last() {
    let c = parse_chord("G7").unwrap();
    assert_eq!(c.by_priority(), vec![11, 5, 7, 2]);
    assert_eq!(c.tone(Role::Third), Some(11));
    assert_eq!(c.fifth(), Some(2));
}

#[test]
fn pitch_names_round_trip() {
    for p in 0..=127u8 {
        assert_eq!(parse_pitch(&pitch_name(p)), Some(p), "{p}");
    }
    assert_eq!(parse_pitch("C4"), Some(60));
    assert_eq!(parse_pitch("A0"), Some(21));
    assert_eq!(parse_pitch("Cb4"), Some(59));
    assert_eq!(parse_pitch("B#3"), Some(60));
    assert_eq!(parse_pitch("F#-1"), Some(6));
    assert_eq!(parse_pitch("G9"), Some(127));
    assert_eq!(parse_pitch("G#9"), None);
    assert_eq!(parse_pitch("C"), None);
}

#[test]
fn a_chart_reads_bars_shares_holds_and_repeats() {
    let map = map44();
    let chart =
        Chart::parse_bars("| Fm9 | Dbmaj7 | C . . G7 | % | Eb Eb/G | N.C. |", &map, 1).unwrap();
    let spans: Vec<(i64, i64, Option<String>)> = chart
        .chords
        .iter()
        .map(|c| {
            (
                c.start.0,
                c.end.0,
                c.chord.as_ref().map(|c| c.symbol.clone()),
            )
        })
        .collect();
    let s = |x: &str| Some(x.to_string());
    let q = PPQ;
    assert_eq!(
        spans,
        vec![
            (0, BAR, s("Fm9")),
            (BAR, 2 * BAR, s("Dbmaj7")),
            (2 * BAR, 2 * BAR + 3 * q, s("C")),
            (2 * BAR + 3 * q, 3 * BAR, s("G7")),
            (3 * BAR, 3 * BAR + 3 * q, s("C")),
            (3 * BAR + 3 * q, 4 * BAR, s("G7")),
            (4 * BAR, 4 * BAR + 2 * q, s("Eb")),
            (4 * BAR + 2 * q, 5 * BAR, s("Eb/G")),
            (5 * BAR, 6 * BAR, None),
        ]
    );
    assert_eq!(
        chart
            .at(Ticks(BAR + 1))
            .unwrap()
            .chord
            .as_ref()
            .unwrap()
            .symbol,
        "Dbmaj7"
    );
    assert!(chart.at(Ticks(6 * BAR)).is_none());
    // A chart from bar 9 in 3/4.
    let mut p = crate::Project::new("t");
    p.time_signature = TimeSignature::new(3, 4);
    let m = p.tempo_map();
    let c = Chart::parse_bars("C F G", &m, 9).unwrap();
    assert_eq!(c.chords[0].start, Ticks(8 * 3 * PPQ));
    assert_eq!(c.chords[2].end, Ticks(9 * 3 * PPQ));
    // Mistakes.
    assert!(matches!(
        Chart::parse_bars("", &map, 1),
        Err(ChartError::Empty)
    ));
    assert!(matches!(
        Chart::parse_bars(". C", &map, 1),
        Err(ChartError::NothingToRepeat { bar: 1 })
    ));
    assert!(matches!(
        Chart::parse_bars("C | Xm", &map, 1),
        Err(ChartError::Chord { bar: 2, .. })
    ));
}

fn note(part: u64, id: u64, start: i64, len: i64, pitch: u8) -> LintNote {
    LintNote {
        part,
        id,
        start: Ticks(start),
        end: Ticks(start + len),
        pitch,
    }
}

/// Round 28's review 1, found by a test: the chorus piano held Fm7 under E♭/G — a chord
/// struck late in the bar outlasted the change, so A♭ rang against the bass's G.
#[test]
fn the_linter_finds_the_demo_fm7_held_under_eb_over_g() {
    let map = map44();
    // The chorus's last bar: Fm7 for two beats, then E♭/G.
    let chart = Chart::parse_bars("Fm7 Eb/G", &map, 1).unwrap();
    let q = PPQ;
    // F3 A♭3 C4 E♭4 struck on beat 2, held for the rest of the bar.
    let notes = [
        note(1, 1, q, 3 * q, 53),
        note(1, 2, q, 3 * q, 56),
        note(1, 3, q, 3 * q, 60),
        note(1, 4, q, 3 * q, 63),
    ];
    let found = lint(
        &notes,
        &LintOptions {
            chart: Some(&chart),
            ..LintOptions::default()
        },
    );
    let ab = found.iter().find(|f| f.note == 2).expect("the Ab is found");
    assert_eq!(ab.severity, Severity::Error);
    assert_eq!(
        ab.issue,
        Issue::RingsIntoChange {
            at: Ticks(2 * q),
            into: 1,
            overlap: Ticks(2 * q),
            against: Some(7)
        }
    );
    // F and C ring into E♭/G too (no semitone there: warnings); E♭ is common, and fine.
    let by = |id| {
        found
            .iter()
            .filter(move |f| f.note == id)
            .collect::<Vec<_>>()
    };
    assert_eq!(by(1).len(), 1);
    assert_eq!(by(1)[0].severity, Severity::Warning);
    assert_eq!(by(3).len(), 1);
    assert!(by(4).is_empty(), "{found:?}");
    // The fixed part — each note stops at the change — is clean.
    let fixed = [
        note(1, 1, q, q, 53),
        note(1, 2, q, q, 56),
        note(1, 3, q, q, 60),
        note(1, 4, q, 3 * q, 63),
    ];
    let o = LintOptions {
        chart: Some(&chart),
        ..LintOptions::default()
    };
    assert!(lint(&fixed, &o).is_empty(), "{:?}", lint(&fixed, &o));
    // A legato overlap under the tolerance is not a finding.
    let legato = [note(1, 1, q, q + q / 16, 56)];
    assert!(lint(&legato, &o).is_empty());
}

#[test]
fn the_linter_finds_clashes_doubles_scale_and_range() {
    let map = map44();
    let chart = Chart::parse_bars("C | G", &map, 1).unwrap();
    let q = PPQ;
    let notes = [
        // F held two beats over C major: a semitone above E — the avoid note.
        note(1, 1, 0, 2 * q, 65),
        // A sixteenth D♯ passing over C: too short to count.
        note(1, 2, 2 * q, q / 4, 63),
        // The same G4 twice at the start of bar 2, and an overlapping one.
        note(1, 3, BAR, q, 67),
        note(1, 4, BAR, q, 67),
        note(1, 5, BAR + q / 2, q, 67),
        // The same pitch in another part is not a double.
        note(2, 6, BAR, q, 67),
        // A♭ under G for a beat: a semitone over G, and out of C major.
        note(1, 7, BAR + 2 * q, q, 68),
        // A pitch the part's instrument cannot play.
        note(3, 8, 0, q, 20),
    ];
    let playable = [(3u64, !0u128 << 21)];
    let o = LintOptions {
        chart: Some(&chart),
        key: Some(Key::new(0, ScaleKind::Major)),
        playable: &playable,
        unpitched: &[3],
        ..LintOptions::default()
    };
    let found = lint(&notes, &o);
    let kinds = |id| {
        found
            .iter()
            .filter(|f| f.note == id)
            .map(|f| (f.issue.kind(), f.severity))
            .collect::<Vec<_>>()
    };
    assert_eq!(kinds(1), vec![("clash", Severity::Warning)]);
    assert_eq!(
        kinds(2),
        vec![("out_of_scale", Severity::Info)],
        "too short to clash"
    );
    assert_eq!(kinds(4), vec![("doubled", Severity::Error)]);
    assert_eq!(
        kinds(5),
        vec![
            ("doubled", Severity::Warning),
            ("doubled", Severity::Warning)
        ]
    );
    assert!(kinds(6).is_empty());
    assert_eq!(
        kinds(7),
        vec![
            ("clash", Severity::Warning),
            ("out_of_scale", Severity::Info)
        ]
    );
    assert_eq!(kinds(8), vec![("out_of_range", Severity::Error)]);
    // Time order.
    assert!(found.windows(2).all(|w| w[0].at <= w[1].at));
    // An unpitched part gets only the range and doubling checks.
    let drums = [note(9, 1, 0, 2 * q, 65), note(9, 2, 0, q, 65)];
    let o = LintOptions {
        chart: Some(&chart),
        key: Some(Key::new(0, ScaleKind::Major)),
        unpitched: &[9],
        ..LintOptions::default()
    };
    let found = lint(&drums, &o);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].issue.kind(), "doubled");
}

#[test]
fn voicings_keep_the_essentials_and_move_little() {
    let map = map44();
    let chart = Chart::parse_bars("Dm9 | G13 | Cmaj9 | A7alt", &map, 1).unwrap();
    let spec = VoiceSpec {
        voices: 4,
        low: 53,
        high: 77,
        bass: Some((36, 52)),
    };
    let v = voice_chart(&chart, &spec).unwrap();
    assert_eq!(v.len(), 4);
    for (vc, c) in v.iter().zip(&chart.chords) {
        let chord = c.chord.as_ref().unwrap();
        assert_eq!(vc.len(), 5, "{vc:?}");
        assert_eq!(vc[0] % 12, chord.bass, "the bass is the chord's");
        assert!((36..=52).contains(&vc[0]));
        let upper = &vc[1..];
        assert!(upper.windows(2).all(|w| w[0] < w[1]));
        assert!(upper.iter().all(|p| (53..=77).contains(p)));
        // Every voice is a chord tone; the third and the seventh are there.
        assert!(
            upper.iter().all(|p| chord.contains(*p)),
            "{} {upper:?}",
            chord.symbol
        );
        for role in [Role::Third, Role::Seventh] {
            let pc = chord.tone(role).unwrap();
            assert!(
                upper.iter().any(|p| p % 12 == pc),
                "{} lacks {role:?}",
                chord.symbol
            );
        }
    }
    // Smooth: no upper voice leaps more than a fifth between neighbours here.
    for w in v.windows(2) {
        for (a, b) in w[0][1..].iter().zip(&w[1][1..]) {
            assert!(
                (*a as i32 - *b as i32).abs() <= 7,
                "{:?} → {:?}",
                w[0],
                w[1]
            );
        }
    }
    // Deterministic.
    assert_eq!(voice_chart(&chart, &spec).unwrap(), v);
    // A range too narrow is refused, not guessed.
    let narrow = VoiceSpec {
        voices: 3,
        low: 60,
        high: 65,
        bass: None,
    };
    assert!(voice_chart(&chart, &narrow).is_none());
    // More voices than tones double the root first.
    let triad = Chart::parse_bars("C", &map, 1).unwrap();
    let wide = VoiceSpec {
        voices: 5,
        low: 48,
        high: 84,
        bass: None,
    };
    let v = voice_chart(&triad, &wide).unwrap();
    assert_eq!(v[0].len(), 5);
    assert_eq!(
        v[0].iter().filter(|p| *p % 12 == 0).count(),
        2,
        "{:?}",
        v[0]
    );
}

fn spec(pattern: Pattern, rhythm: Rhythm) -> PartSpec {
    PartSpec {
        pattern,
        rhythm,
        gate: 1.0,
        velocity: 90,
        accent: 0,
        swing: 0.0,
        swing_grid: Ticks(PPQ / 2),
        humanize_ticks: 0,
        humanize_velocity: 0,
        seed: 1,
        tie_common: false,
    }
}

#[test]
fn parts_never_ring_into_the_next_chord() {
    let map = map44();
    let chart = Chart::parse_bars("C Am | F G7", &map, 1).unwrap();
    let vs = voice_chart(
        &chart,
        &VoiceSpec {
            voices: 3,
            low: 55,
            high: 76,
            bass: None,
        },
    )
    .unwrap();
    for pattern in [
        Pattern::Block,
        Pattern::Pulse,
        Pattern::Arpeggio {
            direction: ArpDirection::UpDown,
            octaves: 2,
        },
        Pattern::Bass(BassStyle::RootFifth),
    ] {
        for rhythm in [
            Rhythm::Every(Ticks(PPQ * 3 / 4)),
            Rhythm::parse_steps("x..x..x.x-x.X...", Ticks(PPQ / 4)).unwrap(),
        ] {
            let mut s = spec(pattern, rhythm);
            s.humanize_ticks = 40;
            s.swing = 0.7;
            let notes = lay_part(&chart, &vs, &map, &s);
            assert!(!notes.is_empty());
            let lints: Vec<LintNote> = notes
                .iter()
                .enumerate()
                .map(|(i, n)| note(1, i as u64, n.start.0, n.length.0, n.pitch))
                .collect();
            let o = LintOptions {
                chart: Some(&chart),
                ..LintOptions::default()
            };
            let found: Vec<_> = lint(&lints, &o)
                .into_iter()
                .filter(|f| f.issue.kind() != "clash" || f.severity == Severity::Error)
                .collect();
            assert!(found.is_empty(), "{pattern:?}: {found:?}");
            for n in &notes {
                let c = chart.at(n.start).unwrap();
                assert!(n.start + n.length <= c.end, "{pattern:?} {n:?}");
            }
        }
    }
}

#[test]
fn a_block_ties_common_tones_when_asked_and_steps_follow_the_bar() {
    let map = map44();
    let chart = Chart::parse_bars("C | Am", &map, 1).unwrap();
    let vs = vec![vec![60, 64, 67], vec![60, 64, 69]];
    let mut s = spec(Pattern::Block, Rhythm::Every(Ticks(PPQ)));
    let plain = lay_part(&chart, &vs, &map, &s);
    assert_eq!(plain.len(), 6);
    s.tie_common = true;
    let tied = lay_part(&chart, &vs, &map, &s);
    assert_eq!(tied.len(), 4, "{tied:?}");
    let c = tied.iter().find(|n| n.pitch == 60).unwrap();
    assert_eq!((c.start, c.length), (Ticks(0), Ticks(2 * BAR)));
    // A step pattern runs on from the chart's start; a tie lengthens the hit before it.
    let p = spec(
        Pattern::Pulse,
        Rhythm::parse_steps("x-..x...", Ticks(PPQ / 2)).unwrap(),
    );
    let one = Chart::parse_bars("C | C", &map, 1).unwrap();
    let notes = lay_part(&one, &[vec![60], vec![60]], &map, &p);
    let starts: Vec<(i64, i64)> = notes.iter().map(|n| (n.start.0, n.length.0)).collect();
    assert_eq!(
        starts,
        vec![
            (0, PPQ),
            (2 * PPQ, PPQ / 2),
            (BAR, PPQ),
            (BAR + 2 * PPQ, PPQ / 2)
        ]
    );
    assert!(Rhythm::parse_steps("....", Ticks(PPQ)).is_none());
    assert!(Rhythm::parse_steps("x?x", Ticks(PPQ)).is_none());
}

/// Review #7: the bass line's fifth is the chord's own — flat in a ø7, sharp in an aug —
/// measured from its root, also over a slash bass.
#[test]
fn the_bass_lines_fifth_is_the_chords_own() {
    let map = map44();
    let chart = Chart::parse_bars("Bm7b5 | Caug | Eb/G | C5", &map, 1).unwrap();
    let vs = vec![vec![35], vec![36], vec![31], vec![36]];
    let s = spec(
        Pattern::Bass(BassStyle::RootFifth),
        Rhythm::Every(Ticks(PPQ * 2)),
    );
    let notes = lay_part(&chart, &vs, &map, &s);
    let pairs: Vec<u8> = notes.iter().map(|n| n.pitch).collect();
    // B → F (the flat fifth); C → G♯; G (under E♭/G) → B♭, E♭'s fifth; C5 → G.
    assert_eq!(pairs, vec![35, 41, 36, 44, 31, 34, 36, 43], "{notes:?}");
}

/// Review #9: a step pattern longer than a bar runs across the bar line as written, counted
/// on from the chart's start, not cut at each bar.
#[test]
fn a_step_pattern_longer_than_a_bar_runs_across_it() {
    let map = map44();
    let chart = Chart::parse_bars("C | C | C", &map, 1).unwrap();
    // 24 sixteenths: a hit on steps 0 and 20, then again from step 24 (bar 2's ninth).
    let mut steps = vec!['.'; 24];
    steps[0] = 'x';
    steps[20] = 'x';
    let text: String = steps.into_iter().collect();
    let p = spec(
        Pattern::Pulse,
        Rhythm::parse_steps(&text, Ticks(PPQ / 4)).unwrap(),
    );
    let notes = lay_part(&chart, &[vec![60]], &map, &p);
    let starts: Vec<i64> = notes.iter().map(|n| n.start.0 / (PPQ / 4)).collect();
    assert_eq!(starts, vec![0, 20, 24, 44], "in sixteenths: {starts:?}");
}

/// Review #8: a clash is a semitone *above* a chord tone. A ii–V–I on ninths and
/// thirteenths, a major seventh over a triad, a ninth over a minor chord, a ♯11 and a
/// dominant's ♭9 are colours, not clashes; the 11th over a major third, the ♭6 over the
/// fifth and the ♭9 over a major chord's root are.
#[test]
fn the_clash_rule_is_directional() {
    let map = map44();
    let q = PPQ;
    fn o_for(c: &Chart) -> LintOptions<'_> {
        LintOptions {
            chart: Some(c),
            ..LintOptions::default()
        }
    }
    // ii–V–I voiced with its tensions, a melody on the 9ths and 13ths.
    let chart = Chart::parse_bars("Dm9 | G13 | Cmaj9", &map, 1).unwrap();
    let vs = voice_chart(
        &chart,
        &VoiceSpec {
            voices: 5,
            low: 50,
            high: 79,
            bass: Some((36, 52)),
        },
    )
    .unwrap();
    let mut notes = Vec::new();
    for (i, v) in vs.iter().enumerate() {
        for &p in v {
            notes.push(note(1, notes.len() as u64, BAR * i as i64, BAR, p));
        }
    }
    for (bar, p) in [(0, 76u8), (1, 76), (1, 81), (2, 74), (2, 83)] {
        notes.push(note(2, 100 + notes.len() as u64, BAR * bar, BAR, p));
    }
    let found = lint(&notes, &o_for(&chart));
    assert!(found.is_empty(), "{found:?}");
    let colour = |sym: &str, pitch: u8| {
        let c = Chart::parse_bars(sym, &map, 1).unwrap();
        lint(&[note(1, 1, 0, 2 * q, pitch)], &o_for(&c))
    };
    assert!(colour("C", 71).is_empty(), "a major 7th over a triad");
    assert!(colour("Cm", 62).is_empty(), "a 9th over a minor chord");
    assert!(colour("C", 66).is_empty(), "a #11");
    assert!(colour("C7", 61).is_empty(), "a dominant's b9");
    assert!(colour("C7", 68).is_empty(), "a dominant's b13");
    // A semitone below what grates as much (fix review #4); a dominant's #9 is a tension.
    assert!(colour("C7", 63).is_empty(), "a dominant's #9");
    for (sym, pitch, what) in [
        ("C", 65u8, "the 11th over the major 3rd"),
        ("C", 68, "the b6 over the 5th"),
        ("C", 61, "the b9 over a major chord's root"),
        ("Cmaj7", 70, "Bb under the major 7th"),
        ("Cmaj7", 63, "Eb under the major 3rd"),
        ("C", 63, "Eb under the major 3rd"),
        ("C7#5", 69, "A over an augmented fifth"),
        ("C7b5", 67, "G over a flat fifth"),
        ("C13", 68, "Ab under the 13th"),
    ] {
        let f = colour(sym, pitch);
        assert_eq!(f.len(), 1, "{what}: {f:?}");
        assert_eq!(f[0].issue.kind(), "clash", "{what}");
    }
}

/// Review #14: the linter on a big song stays fast — 24 000 notes over 1 600 chords, each
/// note checked against only the chords it overlaps (best of five, optimised test build).
#[test]
fn the_linter_is_fast_on_a_big_song() {
    let map = map44();
    let bar = "Dm9 G13 | Cmaj9 A7alt | ";
    let chart = Chart::parse_bars(&bar.repeat(400), &map, 1).unwrap();
    assert_eq!(chart.chords.len(), 1600);
    let notes: Vec<LintNote> = (0..24_000u64)
        .map(|i| {
            note(
                (i % 6) + 1,
                i,
                (i as i64 / 6) * (PPQ / 2),
                PPQ * 3 / 4,
                48 + (i * 7 % 36) as u8,
            )
        })
        .collect();
    let o = LintOptions {
        chart: Some(&chart),
        key: Some(Key::new(0, ScaleKind::Major)),
        ..LintOptions::default()
    };
    let mut best = f64::MAX;
    for _ in 0..5 {
        let t = std::time::Instant::now();
        let found = lint(&notes, &o);
        best = best.min(t.elapsed().as_secs_f64());
        assert!(!found.is_empty());
    }
    // The bisection takes under 10 ms here; checking every chord for every note took 160.
    assert!(best < 0.04, "{:.0} ms", best * 1000.0);
}
