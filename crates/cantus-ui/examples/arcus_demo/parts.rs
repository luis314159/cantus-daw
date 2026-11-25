//! The parts, section by section (the notation is `score.rs`'s). The harmony each bar
//! stands on is written beside it; every voice was led against it by hand.

use super::score::{Part, Who};

use Who::{Bass, Cello, Viola, Violin};

pub const PARTS: &[Part] = &[
    // ---- I · the theme: the cello alone (bars 1–8) -------------------------------------
    // D | D · G | D/F# · Em | A · A (half cadence) · D | D/C# · Bm | Bm/A · G | A7 · D
    Part {
        who: Cello,
        section: 0,
        vel: 70,
        notes: "(A3:1.5 F#3:0.5 D4:2) | (B3:1.5 A3:0.5 G3:1 F#3:1) | (E3:1 G3:1) (A3:1.5 B3:0.5 | \
                C#4:1.5 B3:0.5 A3:2) | (A3:1.5 F#3:0.5 D4:2) | (F#4:1.5 E4:0.5 D4:1 B3:1) | \
                (B3:1.5 A3:0.5 G3:1 E3:1) | D3:3.5 r:0.5",
        cc1: &[
            (0.0, 0.28),
            (6.0, 0.52),
            (12.0, 0.46),
            (16.0, 0.34),
            (22.0, 0.62),
            (27.0, 0.45),
            (31.0, 0.24),
            (32.0, 0.22),
        ],
    },
    // ---- II · the bass enters, pizzicato then arco (bars 9–16) -------------------------
    // Bm | Bm/A · G | Em · F#7 | Bm · Em7 | A7 · D | A/C# · Bm | G · Em7 | A7 · D | A7
    Part {
        who: Cello,
        section: 1,
        vel: 72,
        notes: "(F#4:2 E4:1 D4:1) | (D4:1.5 C#4:0.5 B3:2) | (A#3:1 C#4:1 E4:1.5 D4:0.5) | \
                (D4:1 C#4:1) B3:0.5 C#4:0.5 D4:0.5 E4:0.5 | (F#4:3 E4:1) | \
                (D4:1.5 C#4:0.5 B3:1 D4:1) | (B3:1 G3:1 A3:1.5 C#4:0.5) | D4:2.5 r:1.5",
        cc1: &[
            (0.0, 0.42),
            (8.0, 0.6),
            (12.0, 0.5),
            (16.0, 0.56),
            (19.0, 0.64),
            (28.0, 0.44),
            (32.0, 0.4),
        ],
    },
    Part {
        who: Bass,
        section: 1,
        vel: 82,
        notes: "B1:1p r:1 A1:1p r:1 | G1:1p r:1 E2:1p r:1 | F#1:1p r:1 B1:1p r:1 | \
                E2:1p r:1 A1:1p C#2:1p | D2:2d C#2:2 | B1:2 G1:2 | E2:2 A1:2 | D2:2 A1:2",
        cc1: &[
            (0.0, 0.42),
            (16.0, 0.4),
            (20.0, 0.5),
            (28.0, 0.42),
            (32.0, 0.38),
        ],
    },
    // ---- III · the theme again; the viola's countermelody (bars 17–24) -----------------
    Part {
        who: Cello,
        section: 2,
        vel: 72,
        notes: "(A3:1.5 F#3:0.5 D4:2) | (B3:1.5 A3:0.5 G3:1 F#3:1) | (E3:1 G3:1) (A3:1.5 B3:0.5 | \
                C#4:1.5 B3:0.5 A3:2) | (A3:1.5 F#3:0.5 D4:2) | (F#4:1.5 E4:0.5 D4:1 B3:1) | \
                (B3:1.5 A3:0.5 G3:1 E3:1) | D3:3.5 r:0.5",
        cc1: &[
            (0.0, 0.46),
            (6.0, 0.58),
            (16.0, 0.48),
            (22.0, 0.64),
            (28.0, 0.5),
            (32.0, 0.4),
        ],
    },
    Part {
        who: Viola,
        section: 2,
        vel: 66,
        notes: "(F#4:2 A4:1 G4:1) | (G4:3 A4:1) | (B4:2 A4:2) | (G4:2 E4:2) | (F#4:2 A4:2) | \
                (D5:2 C#5:1 B4:1) | (B4:2 A4:1 G4:1) | F#4:3.5 r:0.5",
        cc1: &[
            (0.0, 0.3),
            (8.0, 0.48),
            (16.0, 0.42),
            (22.0, 0.58),
            (28.0, 0.46),
            (32.0, 0.34),
        ],
    },
    Part {
        who: Bass,
        section: 2,
        vel: 70,
        notes: "D2:4 | G2:3 F#2:1 | E2:2 A1:2 | A1:4 | D2:2 C#2:2 | B1:2 A1:2 | G1:2 A1:2 | \
                D2:3.5 r:0.5",
        cc1: &[(0.0, 0.38), (22.0, 0.48), (32.0, 0.36)],
    },
    // ---- IV · the violin takes the theme an octave up (bars 25–32) ---------------------
    // as I, but the last bar D | A7 runs up into the climax
    Part {
        who: Violin,
        section: 3,
        vel: 72,
        notes: "(A4:1.5 F#4:0.5 D5:2) | (B4:1.5 A4:0.5 G4:1 F#4:1) | (E4:1 G4:1) (A4:1.5 B4:0.5 | \
                C#5:1.5 B4:0.5 A4:2) | (A4:1.5 F#4:0.5 D5:2) | (F#5:1.5 E5:0.5 D5:1 B4:1) | \
                (B4:1.5 A4:0.5 G4:1 E5:1) | D5:2 (A4:0.5 B4:0.5 C#5:0.5 E5:0.5)",
        cc1: &[
            (0.0, 0.42),
            (6.0, 0.58),
            (12.0, 0.5),
            (16.0, 0.46),
            (22.0, 0.68),
            (28.0, 0.56),
            (32.0, 0.68),
        ],
    },
    Part {
        who: Viola,
        section: 3,
        vel: 70,
        notes: "r:1 F#4:0.5s r:1.5 F#4:0.5s r:0.5 | r:1 D4:0.5s r:1.5 D4:0.5s r:0.5 | \
                r:1 E4:0.5s r:1.5 E4:0.5s r:0.5 | r:1 E4:0.5s r:1.5 G4:0.5s r:0.5 | \
                (D4:2 E4:2) | F#4:4 | (D4:2 E4:2) | (F#4:2 G4:2)",
        cc1: &[(0.0, 0.36), (16.0, 0.34), (20.0, 0.44), (32.0, 0.56)],
    },
    Part {
        who: Cello,
        section: 3,
        vel: 78,
        notes: "D3:0.5i A3:0.5i F#3:0.5i A3:0.5i D3:0.5i A3:0.5i F#3:0.5i A3:0.5i | \
                B2:0.5i D3:0.5i G3:0.5i D3:0.5i B2:0.5i D3:0.5i A2:0.5i D3:0.5i | \
                E3:0.5i G3:0.5i B3:0.5i G3:0.5i C#3:0.5i E3:0.5i A3:0.5i E3:0.5i | \
                A2:0.5i C#3:0.5i E3:0.5i A3:0.5i G3:0.5i E3:0.5i C#3:0.5i E3:0.5i | \
                (F#3:2 A3:2) | (D3:2 F#3:2) | (D3:2 C#3:2) | D3:2 E3:1 G3:1",
        cc1: &[(0.0, 0.4), (16.0, 0.4), (22.0, 0.5), (32.0, 0.6)],
    },
    Part {
        who: Bass,
        section: 3,
        vel: 80,
        notes: "D2:1p r:1 A1:1p r:1 | G1:1p r:1 G1:1p F#1:1p | E2:1p r:1 A1:1p r:1 | \
                A1:1p r:1 A1:1p C#2:1p | D2:2d C#2:2 | B1:2 A1:2 | G1:2 A1:2 | D2:2 A1:2",
        cc1: &[(0.0, 0.4), (16.0, 0.42), (32.0, 0.56)],
    },
    // ---- V · the climax: tremolo inside, the violin up to D6 (bars 33–38) --------------
    // D | D/F# · G | Em7 · Em7 | A7 · D/F# | G · Gm/Bb | D/A A7 · D
    Part {
        who: Violin,
        section: 4,
        vel: 84,
        notes:
            "(F#5:1.5 E5:0.5 F#5:1 A5:1) | (B5:1.5 A5:0.5 G5:1 E5:1) | (G5:1 E5:1) (A5:1 C#6:1) | \
                D6:2> (B5:1.5 G5:0.5) | (Bb5:1 A5:1) (F#5:1 E5:1) | D5:3 r:1",
        cc1: &[
            (0.0, 0.64),
            (8.0, 0.76),
            (12.0, 0.9),
            (14.0, 0.92),
            (18.0, 0.74),
            (20.0, 0.62),
            (24.0, 0.48),
        ],
    },
    Part {
        who: Viola,
        section: 4,
        vel: 80,
        notes:
            "A4:4t | (B4:2t G4:2t) | G4:4t | (A4:2t B4:2t) | (G4:2t F#4:1t G4:1t) | F#4:3.5 r:0.5",
        cc1: &[
            (0.0, 0.52),
            (12.0, 0.82),
            (16.0, 0.72),
            (20.0, 0.58),
            (24.0, 0.42),
        ],
    },
    Part {
        who: Cello,
        section: 4,
        vel: 80,
        notes: "D3:4t | (D3:2t E3:2t) | (E3:2t C#3:2t) | D3:4t | (D3:3t C#3:1t) | D3:3.5 r:0.5",
        cc1: &[
            (0.0, 0.54),
            (12.0, 0.84),
            (16.0, 0.74),
            (20.0, 0.6),
            (24.0, 0.44),
        ],
    },
    Part {
        who: Bass,
        section: 4,
        vel: 88,
        notes: "D2:2 F#2:2 | G2:2 E2:2 | E2:2 A1:2 | F#2:2 G2:2 | Bb1:2 A1:2 | D2:3.5 r:0.5",
        cc1: &[
            (0.0, 0.58),
            (12.0, 0.86),
            (16.0, 0.76),
            (20.0, 0.62),
            (24.0, 0.46),
        ],
    },
    // ---- VI · farewell: the bass leaves, then the viola (bars 39–42) -------------------
    // D | D/C# · Bm | Bm/A · G | A7 · Bm | G (the deceptive cadence)
    Part {
        who: Violin,
        section: 5,
        vel: 64,
        notes: "(A4:1.5 F#4:0.5 D5:2) | (F#5:1.5 E5:0.5 D5:1 B4:1) | (B4:1.5 A4:0.5 G4:1 E4:1) | \
                F#4:2 (G4:1 A4:1)",
        cc1: &[(0.0, 0.42), (6.0, 0.5), (12.0, 0.4), (16.0, 0.34)],
    },
    Part {
        who: Viola,
        section: 5,
        vel: 60,
        notes: "A3:4 | D4:4 | (D4:2 C#4:2) | D4:3 r:1",
        cc1: &[(0.0, 0.34), (8.0, 0.3), (12.0, 0.24), (15.0, 0.16)],
    },
    Part {
        who: Cello,
        section: 5,
        vel: 62,
        notes: "(D3:2 A2:2) | (B2:2 F#3:2) | (G2:2 A2:2) | (B2:2 G2:2)",
        cc1: &[(0.0, 0.4), (8.0, 0.36), (16.0, 0.3)],
    },
    Part {
        who: Bass,
        section: 5,
        vel: 60,
        notes: "(D2:2 C#2:2) | (B1:2 A1:2)",
        cc1: &[(0.0, 0.36), (5.0, 0.28), (8.0, 0.14)],
    },
    // ---- VII · coda: the violin and the cello alone (bars 43–46) -----------------------
    // G | Gm/Bb under a held D · A · D (held to the end)
    Part {
        who: Violin,
        section: 6,
        vel: 58,
        notes: "D5:4 | (A4:2 C#5:1 E5:1 | F#5:8)",
        cc1: &[
            (0.0, 0.3),
            (2.0, 0.42),
            (4.0, 0.32),
            (8.0, 0.34),
            (11.0, 0.3),
            (16.0, 0.1),
        ],
    },
    Part {
        who: Cello,
        section: 6,
        vel: 58,
        notes: "(G2:2 Bb2:2) | A2:4 | D3:8",
        cc1: &[(0.0, 0.32), (8.0, 0.3), (11.0, 0.26), (16.0, 0.08)],
    },
];
