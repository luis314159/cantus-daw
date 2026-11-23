//! Round 42 (ADR 0155): the MIDI Tuning Standard against outside bytes — the MMA's own
//! example table of frequency words (its nine consistent rows; the other five are the spec's
//! arithmetic slips, `microtuning-verified.md` §4.2), and a decoder ported from ODDSound's
//! 0BSD `libMTSClient::parseMIDIData` (its behaviour, as an independent oracle) — plus the
//! three checksum conventions and every message's layout.

#![allow(clippy::needless_range_loop)] // tables read key by key, as the tests say them

use cantus_core::tuning::{Temperament, Tuning, TuningSystem, TuningTable};
use cantus_core::tuning_mts::*;

/// The MMA's example (bytes, Hz as printed) — the rows its own formula agrees with.
const MMA_ROWS: [([u8; 3], f64); 9] = [
    ([0x00, 0x00, 0x00], 8.1758),
    ([0x01, 0x00, 0x00], 8.6620),
    ([0x0C, 0x00, 0x00], 16.3516),
    ([0x3C, 0x00, 0x00], 261.6256),
    ([0x3D, 0x00, 0x00], 277.1827),
    ([0x44, 0x7F, 0x7F], 439.9984),
    ([0x45, 0x00, 0x00], 440.0000),
    ([0x45, 0x00, 0x01], 440.0016),
    ([0x78, 0x00, 0x00], 8372.0190),
];

#[test]
fn the_mma_s_example_words_decode_to_its_frequencies() {
    for (w, hz) in MMA_ROWS {
        let got = pitch_hz(decode_word(w).unwrap());
        assert!((got - hz).abs() < 1e-3, "{w:02X?}: {got} vs {hz}");
        // And they encode back to the same bytes.
        assert_eq!(encode_word(decode_word(w).unwrap()), w);
    }
    assert_eq!(decode_word([0x7F, 0x7F, 0x7F]), None, "no change");
}

#[test]
fn words_round_trip_and_stay_in_range() {
    for semi in 0..128u8 {
        for frac in [0u16, 1, 8191, 16383] {
            let w = [semi, (frac >> 7) as u8, (frac & 0x7F) as u8];
            if w == [0x7F, 0x7F, 0x7F] {
                continue;
            }
            assert_eq!(encode_word(decode_word(w).unwrap()), w);
        }
    }
    // Diapasons (our computation from the spec's formula).
    for (hz, w) in [
        (415.0, [0x43, 0x7E, 0x30]),
        (430.0, [0x44, 0x4D, 0x07]),
        (432.0, [0x44, 0x57, 0x2B]),
        (442.0, [0x45, 0x0A, 0x06]),
        (443.0, [0x45, 0x0F, 0x07]),
        (261.6255653, [0x3C, 0x00, 0x00]),
    ] {
        let p = 69.0 + 12.0 * cantus_dsp::math::log2(hz / 440.0f64);
        assert_eq!(encode_word(p), w, "{hz}");
    }
    assert_eq!(
        encode_word(500.0),
        [0x7F, 0x7F, 0x7E],
        "never the reserved word"
    );
    assert_eq!(encode_word(-3.0), [0, 0, 0]);
}

/// ODDSound's client parser (0BSD), its behaviour ported for the oracle: the 14-bit fraction
/// over 16383, the two-byte offset over 8192 below and 8191 above.
fn libmts_pitches(buf: &[u8]) -> [f64; 128] {
    let mut p: [f64; 128] = std::array::from_fn(|k| k as f64);
    let sub = buf[4];
    match sub {
        0x01 => {
            for (k, w) in buf[22..22 + 384].chunks_exact(3).enumerate() {
                let v = (i32::from(w[0]) << 14) | (i32::from(w[1]) << 7) | i32::from(w[2]);
                p[k] = f64::from((v >> 14) & 127) + f64::from(v & 16383) / 16383.0;
            }
        }
        0x02 => {
            let n = usize::from(buf[6]);
            for c in buf[7..7 + 4 * n].chunks_exact(4) {
                let v = (i32::from(c[1]) << 14) | (i32::from(c[2]) << 7) | i32::from(c[3]);
                p[usize::from(c[0])] = f64::from((v >> 14) & 127) + f64::from(v & 16383) / 16383.0;
            }
        }
        0x08 => {
            for (pc, &b) in buf[8..20].iter().enumerate() {
                for k in (pc..128).step_by(12) {
                    p[k] = k as f64 + (f64::from(b) - 64.0) * 0.01;
                }
            }
        }
        0x09 => {
            for (pc, w) in buf[8..32].chunks_exact(2).enumerate() {
                let v = (i32::from(w[0]) << 7) | i32::from(w[1]);
                let d = f64::from(v - 8192) / if v > 8192 { 8191.0 } else { 8192.0 };
                for k in (pc..128).step_by(12) {
                    p[k] = k as f64 + d;
                }
            }
        }
        _ => unreachable!(),
    }
    p
}

fn werck3_in_e_flat() -> TuningTable {
    Tuning {
        a4_hz: 415.0,
        system: TuningSystem::Temperament {
            id: Temperament::WerckmeisterIii,
            key: 3,
            follow_song_key: false,
        },
        map: None,
    }
    .offsets()
}

#[test]
fn our_messages_decode_alike_in_an_independent_reader() {
    let t = werck3_in_e_flat();
    let ours = |bytes: &[u8]| {
        let mut p = equal_pitches();
        parse(bytes).unwrap().apply(&mut p);
        p
    };
    // Bulk dump and single-note changes: libMTS's /16383 is at most 0.0062 c off.
    let dump = write_key_dump(ALL_DEVICES, 0, "Werckmeister", &t);
    let (a, b) = (ours(&dump), libmts_pitches(&dump));
    for k in 0..128 {
        assert!((a[k] - b[k]).abs() * 100.0 < 0.0062, "key {k}");
        let want = k as f64 + f64::from(t.0[k]);
        if want >= 0.0 {
            // (Keys 0 and 1 at A415 lie below the word's range: sent as its floor.)
            assert!((a[k] - want).abs() * 100.0 < 0.0031, "key {k}");
        }
    }
    for m in write_table_changes(ALL_DEVICES, 0, &t) {
        let b = libmts_pitches(&m);
        let a = ours(&m);
        for k in 0..128 {
            if b[k] != k as f64 {
                assert!((a[k] - b[k]).abs() * 100.0 < 0.0062, "key {k}");
            }
        }
    }
    // Scale/octave, 1- and 2-byte.
    let dev: [f64; 12] = std::array::from_fn(|pc| f64::from(t.0[60 + pc]) * 100.0);
    for two in [false, true] {
        let m = write_octave(ALL_DEVICES, true, ALL_CHANNELS, &dev, two);
        let (a, b) = (ours(&m), libmts_pitches(&m));
        for k in 0..128 {
            assert!(
                (a[k] - b[k]).abs() * 100.0 < 0.013,
                "two-byte {two}, key {k}"
            );
        }
    }
}

#[test]
fn the_bulk_dump_of_12_tet_is_the_spec_s_and_its_checksum_conventions_are_read() {
    let d = write_key_dump(ALL_DEVICES, 0, "12-TET A440", &TuningTable::EQUAL);
    assert_eq!(d.len(), 408);
    let head = [
        0xF0, 0x7E, 0x7F, 0x08, 0x01, 0x00, 0x31, 0x32, 0x2D, 0x54, 0x45, 0x54, 0x20, 0x41, 0x34,
        0x34, 0x30, 0x20, 0x20, 0x20, 0x20, 0x20, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x02, 0x00,
        0x00,
    ];
    assert_eq!(&d[..head.len()], &head);
    assert_eq!(d[406], 0x12, "CA-020's checksum");
    assert_eq!(d[407], 0xF7);
    for (chk, want) in [
        (0x12, Checksum::Spec),
        (0x77, Checksum::FluidSynth),
        (0x6D, Checksum::ScaleWorkshop),
        (0x00, Checksum::Mismatch),
    ] {
        let mut m = d.clone();
        m[406] = chk;
        match parse(&m).unwrap() {
            MtsMessage::KeyDump {
                checksum,
                name,
                keys,
                ..
            } => {
                assert_eq!(checksum, want);
                assert_eq!(name, "12-TET A440");
                assert!(keys.iter().enumerate().all(|(k, p)| *p == Some(k as f64)));
            }
            m => panic!("{m:?}"),
        }
    }
    // Without the framing bytes too.
    assert!(parse(&d[1..407]).is_ok());
}

#[test]
fn werckmeister_iii_s_scale_octave_messages_are_these_bytes() {
    // C unchanged, as `microtuning-verified.md` §4.5 lists them (our computation).
    let dev: [f64; 12] = std::array::from_fn(|pc| {
        cantus_core::tuning_temper::deviation_in(Temperament::WerckmeisterIii, 0)[pc]
    });
    let one = write_octave(ALL_DEVICES, true, ALL_CHANNELS, &dev, false);
    assert_eq!(
        one,
        [
            0xF0, 0x7F, 0x7F, 0x08, 0x08, 0x03, 0x7F, 0x7F, 0x40, 0x36, 0x38, 0x3A, 0x36, 0x3E,
            0x34, 0x3C, 0x38, 0x34, 0x3C, 0x38, 0xF7
        ]
    );
    let two = write_octave(ALL_DEVICES, true, ALL_CHANNELS, &dev, true);
    assert_eq!(
        two,
        [
            0xF0, 0x7F, 0x7F, 0x08, 0x09, 0x03, 0x7F, 0x7F, 0x40, 0x00, 0x39, 0x5F, 0x3A, 0x7F,
            0x3C, 0x20, 0x39, 0x5F, 0x3E, 0x60, 0x38, 0x3F, 0x3D, 0x40, 0x3A, 0x7F, 0x38, 0x3F,
            0x3D, 0x40, 0x3A, 0x7F, 0xF7
        ]
    );
    match parse(&two).unwrap() {
        MtsMessage::Octave {
            channels,
            cents,
            two_byte,
            realtime,
            ..
        } => {
            assert_eq!(channels, ALL_CHANNELS);
            assert!(two_byte && realtime);
            for pc in 0..12 {
                assert!((cents[pc] - dev[pc]).abs() < 0.0062, "pc {pc}");
            }
        }
        m => panic!("{m:?}"),
    }
}

#[test]
fn every_message_s_layout_round_trips() {
    let t = werck3_in_e_flat();
    let bank = write_bank_key_dump(5, 2, 9, "bank", &t);
    assert_eq!(bank.len(), 409);
    match parse(&bank).unwrap() {
        MtsMessage::KeyDump {
            device,
            bank,
            program,
            checksum,
            ..
        } => assert_eq!(
            (device, bank, program, checksum),
            (5, Some(2), 9, Checksum::Spec)
        ),
        m => panic!("{m:?}"),
    }
    let cents = [
        0.0, -9.775, -7.82, -5.865, -9.775, -1.955, -11.73, -3.91, -7.82, -11.73, -3.91, -7.82,
    ];
    for two in [false, true] {
        let m = write_octave_dump(1, 0, 3, "W3", &cents, two);
        assert_eq!(m.len(), if two { 49 } else { 37 });
        match parse(&m).unwrap() {
            MtsMessage::OctaveDump {
                cents: c, checksum, ..
            } => {
                assert_eq!(checksum, Checksum::Spec);
                let tol = if two { 0.0062 } else { 0.5 };
                for pc in 0..12 {
                    assert!((c[pc] - cents[pc]).abs() <= tol);
                }
            }
            m => panic!("{m:?}"),
        }
    }
    let ch = write_table_changes(0x10, 4, &t);
    assert_eq!(ch.len(), 2);
    assert_eq!(ch[0].len(), 8 + 4 * 64);
    let mut p = equal_pitches();
    for m in &ch {
        parse(m).unwrap().apply(&mut p);
    }
    for k in 0..128 {
        if k as f64 + f64::from(t.0[k]) >= 0.0 {
            assert!((p[k] - k as f64 - f64::from(t.0[k])).abs() * 100.0 < 0.0031);
        }
    }
    assert_eq!(
        write_dump_request(0, None, 7),
        [0xF0, 0x7E, 0x00, 0x08, 0x00, 0x07, 0xF7]
    );
    assert_eq!(
        parse(&write_dump_request(0, Some(1), 7)).unwrap(),
        MtsMessage::DumpRequest {
            device: 0,
            bank: Some(1),
            program: 7
        }
    );
    // The banked single-note change, non-real-time (07 with 7E).
    let m = [
        0xF0, 0x7E, 0x00, 0x08, 0x07, 0x01, 0x02, 0x01, 0x45, 0x45, 0x00, 0x00, 0xF7,
    ];
    match parse(&m).unwrap() {
        MtsMessage::NoteChange {
            realtime,
            bank,
            program,
            changes,
            ..
        } => {
            assert!(!realtime);
            assert_eq!((bank, program), (Some(1), 2));
            assert_eq!(changes, vec![(0x45, Some(69.0))]);
        }
        m => panic!("{m:?}"),
    }
    // Refusals with their reasons.
    assert_eq!(
        parse(&[0xF0, 0x7E, 0x00, 0x06, 0x01, 0xF7]),
        Err(MtsError::NotTuning)
    );
    assert_eq!(
        parse(&[0xF0, 0x7E, 0x00, 0x08, 0x0A, 0xF7]),
        Err(MtsError::UnknownMessage(0x0A))
    );
    assert!(matches!(parse(&bank[..300]), Err(MtsError::Length { .. })));
}

#[test]
fn an_unmapped_key_is_sent_as_no_change() {
    let mut t = TuningTable::EQUAL;
    t.0[61] = f32::NAN;
    let d = write_key_dump(ALL_DEVICES, 0, "", &t);
    let at = 6 + 16 + 61 * 3;
    assert_eq!(&d[at..at + 3], &[0x7F, 0x7F, 0x7F]);
    let mut p = equal_pitches();
    p[61] = 99.0;
    parse(&d).unwrap().apply(&mut p);
    assert_eq!(p[61], 99.0, "no change kept the stored pitch");
}

/// r42 fix-engine (review reach 9) — outside bytes: Scale Workshop's MTS exporter test
/// (MIT; `tests/data/scale-workshop/SOURCE.md`). Its scale's 128 frequencies, as its
/// `Scale::getFrequencyRange(0, 128)` gives them, turned into frequency words by our encoder,
/// hash to the SHA-256 its test expects; our bulk dump of them, with its checksum convention,
/// hashes to its dump's; and our reader takes that dump, naming the convention.
#[test]
fn scale_workshop_s_bulk_dump_hashes_are_ours() {
    use sha2::{Digest, Sha256};
    let b64 = |bytes: &[u8]| cantus_core::external::base64_encode(&Sha256::digest(bytes));
    // `test-data.ts`: the intervals' values, the equave last.
    let ratios = [
        cantus_dsp::math::exp2(100.0 / 1200.0),
        528.0 / 440.0,
        cantus_dsp::math::exp2(4.0 / 5.0),
        5.0 / 3.0,
        std::f64::consts::E / 2.0,
        3_486_784_401.0 / 3_276_800_000.0,
        2.0,
    ];
    let size = ratios.len() as i64;
    let ratio = |index: i64| {
        let equaves = index.div_euclid(size);
        let r = index.rem_euclid(size);
        let e = 2f64.powi(equaves as i32);
        if r == 0 {
            e
        } else {
            ratios[r as usize - 1] * e
        }
    };
    // `ftomts`: its MTS value of a frequency.
    let mts = |f: f64| -> f64 {
        if f <= 0.0 {
            0.0
        } else if f > 13_289.656616 {
            127.999878
        } else {
            let v = 69.0 + 12.0 * cantus_dsp::math::log2(f / 440.0);
            (v * 1_000_000.0).round() / 1_000_000.0
        }
    };
    let pitches: [Option<f64>; 128] =
        std::array::from_fn(|k| Some(mts(440.0 * ratio(k as i64 - 69))));
    let words: Vec<u8> = pitches
        .iter()
        .flat_map(|p| encode_word(p.unwrap()))
        .collect();
    assert_eq!(words.len(), 384);
    assert_eq!(b64(&words), "NLwkn1HRQKdNAyYdxl6RQwfz0JvFyShWaB0DHHtPVZo=");
    // The dump: ours, written with CA-020's checksum; Scale Workshop's convention inverts it.
    let mut dump = write_key_dump_of(0x00, 1, "Test Scale", &pitches);
    assert_eq!(dump.len(), 408);
    assert_eq!(&dump[6..22], b"Test Scale      ", "the name padded with spaces");
    assert_eq!(&dump[22..406], words.as_slice());
    let theirs = dump[1..406]
        .iter()
        .filter(|b| **b < 0x80)
        .fold(0xFFu8, |a, b| a ^ b)
        & 0x7F;
    assert_eq!(theirs, !dump[406] & 0x7F, "the inverted spec value");
    dump[406] = theirs;
    assert_eq!(b64(&dump), "Ghut5DZnxeRXQE0TbOJl6Sh66xI2x1QouHhVhIdQXrQ=");
    match parse(&dump).unwrap() {
        MtsMessage::KeyDump {
            checksum,
            name,
            keys,
            ..
        } => {
            assert_eq!(checksum, Checksum::ScaleWorkshop);
            assert_eq!(name, "Test Scale");
            for (k, (got, want)) in keys.iter().zip(&pitches).enumerate() {
                let (g, w) = (got.unwrap(), want.unwrap().clamp(0.0, PITCH_TOP));
                assert!((g - w).abs() <= 0.5 / 16384.0 + 1e-12, "key {k}: {g} vs {w}");
            }
        }
        m => panic!("{m:?}"),
    }
}
