//! Tympanum's modelled kit measured against a recorded one, piece by piece (round 40 phase
//! 3, after checkpoint B: *"suena a batería, aún no diría que real"*).
//!
//! ```text
//! cargo run --release -p cantus-ui --example tympanum_measure -- hits <out> <content root> <kit> [<kit> …]
//! cargo run --release -p cantus-ui --example tympanum_measure -- ab <out> <content root> <pack/kit>
//! ```
//!
//! A kit is `model:studio` or `pack:<pack>/<kit>`. `hits` plays every articulation at four
//! velocities, alone, through the engine's offline render, and measures each hit: the
//! attack (its time to peak, the first 3 ms's band and centroid), the octave bands in six
//! windows after the onset, each band's decay (T20, T40), the strongest low partial early
//! and late (the pitch glide), the level — into `<out>/<kit>.txt` and one side-by-side
//! `<out>/compare.txt`. `ab` renders the groove (`tympanum_groove`'s) through the recorded
//! kit and the modelled studio kit at the same loudness into `<out>/`.

#[path = "tympanum_measure/analyse.rs"]
mod analyse;
#[path = "tympanum_measure/groove.rs"]
mod groove;
#[path = "tympanum_measure/hits.rs"]
mod hits;

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("");
    let out = PathBuf::from(args.get(2).expect("an output folder"));
    let root = PathBuf::from(args.get(3).expect("a content root"));
    std::fs::create_dir_all(&out).unwrap();
    cantus_ui::tympanum_load::install(root);
    match mode {
        "hits" => {
            let kits: Vec<String> = args[4..].to_vec();
            hits::run(&out, &kits);
        }
        "ab" => {
            let kit = args.get(4).expect("pack/kit");
            hits::ab(&out, kit);
        }
        "model" => {
            let m = args.get(4).map(String::as_str).unwrap_or("studio");
            let name = args
                .get(5)
                .cloned()
                .unwrap_or_else(|| format!("modelled-{m}"));
            hits::model_only(&out, m, &name);
        }
        "phrase" => {
            let kit = args.get(4).expect("a kit");
            hits::phrase(&out, kit);
        }
        "wav" => {
            // Recorded hits from files (a DrumGizmo kit's multichannel hits): every channel
            // summed, or `--ch 6,7` (1-based) only those.
            let mut chans: Option<Vec<usize>> = None;
            let mut files = Vec::new();
            let mut it = args[4..].iter();
            while let Some(a) = it.next() {
                if a == "--ch" {
                    let list = it.next().expect("channels");
                    chans = Some(list.split(',').filter_map(|c| c.parse().ok()).collect());
                } else {
                    files.push(PathBuf::from(a));
                }
            }
            let mut report = String::new();
            for f in &files {
                let (fs, x) = read_wav(f, chans.as_deref());
                analyse::set_rate(fs);
                let h = analyse::measure(&x);
                report += &format!(
                    "{}
{}
",
                    f.display(),
                    analyse::text(&h)
                );
            }
            print!("{report}");
            std::fs::write(out.join("wav.txt"), report).unwrap();
        }
        _ => eprintln!("usage: tympanum_measure hits|ab|wav <out> <content root> <kit…|file…>"),
    }
}

/// A WAV's channels summed (or `chans`, 1-based), and its rate: 16/24-bit PCM or 32-bit
/// float.
fn read_wav(path: &std::path::Path, chans: Option<&[usize]>) -> (f64, Vec<f64>) {
    let b = std::fs::read(path).expect("read");
    assert!(&b[..4] == b"RIFF" && &b[8..12] == b"WAVE", "not a WAV");
    let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]]);
    let mut i = 12;
    let (mut fmt, mut ch, mut rate, mut bits) = (0u16, 0usize, 0u32, 0u16);
    loop {
        let id = &b[i..i + 4];
        let len = u32_at(i + 4) as usize;
        if id == b"fmt " {
            fmt = u16_at(i + 8);
            ch = u16_at(i + 10) as usize;
            rate = u32_at(i + 12);
            bits = u16_at(i + 22);
            if fmt == 0xFFFE {
                fmt = u16_at(i + 32);
            }
        } else if id == b"data" {
            let data = &b[i + 8..(i + 8 + len).min(b.len())];
            let bps = bits as usize / 8;
            let frames = data.len() / (bps * ch);
            let pick: Vec<usize> = match chans {
                Some(c) => c.iter().map(|c| c - 1).collect(),
                None => (0..ch).collect(),
            };
            let x = (0..frames)
                .map(|f| {
                    pick.iter()
                        .map(|&c| {
                            let o = (f * ch + c) * bps;
                            let s = &data[o..o + bps];
                            match (fmt, bits) {
                                (3, 32) => f32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64,
                                (1, 16) => i16::from_le_bytes([s[0], s[1]]) as f64 / 32768.0,
                                (1, 24) => {
                                    (i32::from_le_bytes([0, s[0], s[1], s[2]]) >> 8) as f64
                                        / 8_388_608.0
                                }
                                _ => panic!("format {fmt}/{bits}"),
                            }
                        })
                        .sum()
                })
                .collect();
            return (rate as f64, x);
        }
        i += 8 + len + (len & 1);
    }
}
