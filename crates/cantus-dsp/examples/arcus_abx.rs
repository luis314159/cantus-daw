//! Arcus's side of the blind ABX (round 42): `tools/arcus/abx_phrase.mid` rendered by the
//! cello (its first track) and the double bass (its second) at their defaults — the
//! "pizzicato" marker plucks the notes after it — written at 48 kHz, 24 bits, peak −1 dBFS
//! (`abx-cello.wav`, `abx-bass.wav`) and loudness-matched to −23 LUFS (`*-lufs23.wav`), the
//! names the recorded strings' renders use. Run by `tools/arcus/abx.sh`:
//!
//! ```text
//! cargo run -p cantus-dsp --release --example arcus_abx -- <abx_phrase.mid> <out dir>
//! ```

use cantus_dsp::arcus::tables::Instrument;
use cantus_dsp::arcus::{Arcus, ArcusSettings, Art};
use cantus_dsp::math;
use cantus_dsp::terminus::loudness::LoudnessMeter;
use std::path::Path;

const SR: f32 = 48_000.0;

#[derive(Copy, Clone, Debug)]
enum Ev {
    On(u8, u8),
    Off(u8),
    Cc1(u8),
    Pizz,
}

/// A track's events in seconds (one tempo, read from the file's first track).
fn read_midi(path: &str) -> Vec<Vec<(f64, Ev)>> {
    let d = std::fs::read(path).expect("the MIDI file");
    assert_eq!(&d[0..4], b"MThd", "not a MIDI file");
    let ppq = u16::from_be_bytes([d[12], d[13]]) as f64;
    let mut at = 14;
    let mut tracks = Vec::new();
    let mut tempo = 500_000.0f64;
    while at + 8 <= d.len() {
        assert_eq!(&d[at..at + 4], b"MTrk");
        let len = u32::from_be_bytes([d[at + 4], d[at + 5], d[at + 6], d[at + 7]]) as usize;
        let body = &d[at + 8..at + 8 + len];
        at += 8 + len;
        let (mut i, mut tick, mut status) = (0usize, 0u64, 0u8);
        let mut ev = Vec::new();
        let vlq = |i: &mut usize| {
            let mut v = 0u64;
            loop {
                let b = body[*i];
                *i += 1;
                v = (v << 7) | (b & 0x7F) as u64;
                if b & 0x80 == 0 {
                    return v;
                }
            }
        };
        while i < body.len() {
            tick += vlq(&mut i);
            let secs = tick as f64 / ppq * tempo / 1e6;
            let mut b = body[i];
            if b == 0xFF {
                let kind = body[i + 1];
                i += 2;
                let n = vlq(&mut i) as usize;
                let data = &body[i..i + n];
                i += n;
                if kind == 0x51 {
                    tempo =
                        ((data[0] as u32) << 16 | (data[1] as u32) << 8 | data[2] as u32) as f64;
                } else if kind == 0x06 && data == b"pizzicato" {
                    ev.push((secs, Ev::Pizz));
                }
                continue;
            }
            if b & 0x80 != 0 {
                status = b;
                i += 1;
            } else {
                b = status;
            }
            match b & 0xF0 {
                0x90 if body[i + 1] > 0 => ev.push((secs, Ev::On(body[i], body[i + 1]))),
                0x80 | 0x90 => ev.push((secs, Ev::Off(body[i]))),
                0xB0 if body[i] == 1 => ev.push((secs, Ev::Cc1(body[i + 1]))),
                _ => {}
            }
            i += if matches!(b & 0xF0, 0xC0 | 0xD0) {
                1
            } else {
                2
            };
        }
        tracks.push(ev);
    }
    tracks
}

fn render(inst: Instrument, ev: &[(f64, Ev)]) -> (Vec<f32>, Vec<f32>) {
    let mut a = Arcus::new(SR);
    a.set(&ArcusSettings {
        instrument: inst,
        ..ArcusSettings::default()
    });
    let end = ev.iter().map(|e| e.0).fold(0.0, f64::max) + 3.0;
    let frames = (end * SR as f64) as usize;
    let (mut l, mut r) = (vec![0.0f32; frames], vec![0.0f32; frames]);
    let mut pizz = false;
    let (mut at, mut i) = (0usize, 0usize);
    while at < frames {
        while i < ev.len() && (ev[i].0 * SR as f64) as usize <= at {
            match ev[i].1 {
                Ev::On(k, v) => {
                    let id = 9_000 + i as u64;
                    if pizz {
                        a.note_art(id, Some(Art::Pizzicato), None);
                    }
                    a.note_on(k, v as f32 / 127.0, id);
                }
                Ev::Off(k) => {
                    // The note this key started last.
                    let id = ev[..i]
                        .iter()
                        .enumerate()
                        .rev()
                        .find(|(_, e)| matches!(e.1, Ev::On(kk, _) if kk == k))
                        .map(|(j, _)| 9_000 + j as u64)
                        .unwrap_or(0);
                    a.note_off(k, id);
                }
                Ev::Cc1(v) => a.controller(0, v as f32 / 127.0),
                Ev::Pizz => pizz = true,
            }
            i += 1;
        }
        let next = ev.get(i).map_or(frames, |e| (e.0 * SR as f64) as usize);
        let n = 256.min(next.max(at + 1) - at).min(frames - at);
        a.process(&mut l[at..at + n], &mut r[at..at + n]);
        at += n;
    }
    (l, r)
}

fn write(path: &Path, l: &[f32], r: &[f32], gain: f32) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR as u32,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).expect("create");
    for (a, b) in l.iter().zip(r) {
        for x in [a, b] {
            let v = ((x * gain).clamp(-1.0, 1.0) * 8_388_607.0).round() as i32;
            w.write_sample(v).expect("write");
        }
    }
    w.finalize().expect("finalize");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let midi = args
        .get(1)
        .map_or("tools/arcus/abx_phrase.mid", |s| s.as_str());
    let out = Path::new(args.get(2).map_or("abx", |s| s.as_str()));
    std::fs::create_dir_all(out).expect("out dir");
    let tracks = read_midi(midi);
    for (name, inst, ev) in [
        ("cello", Instrument::Cello, &tracks[0]),
        ("bass", Instrument::Bass, &tracks[1]),
    ] {
        let (l, r) = render(inst, ev);
        let peak = l.iter().chain(&r).fold(0f32, |m, x| m.max(x.abs()));
        let to_peak = math::db_to_gain(-1.0) / peak.max(1e-9);
        let mut meter = LoudnessMeter::new(SR);
        meter.process(&l, &r);
        let lufs = meter.integrated().unwrap_or(-70.0);
        let to_lufs = math::db_to_gain((-23.0 - lufs) as f32);
        write(&out.join(format!("abx-{name}.wav")), &l, &r, to_peak);
        write(&out.join(format!("abx-{name}-lufs23.wav")), &l, &r, to_lufs);
        println!(
            "{name}: peak {:.1} dBFS, {lufs:.1} LUFS raw; written at −1 dBFS peak and at −23 LUFS (peak {:.1} dBFS there)",
            20.0 * math::log10(peak.max(1e-9)),
            20.0 * math::log10((peak * to_lufs).max(1e-9))
        );
    }
}
