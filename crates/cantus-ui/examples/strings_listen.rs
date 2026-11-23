//! r42:strings-rec — the recorded strings for the owner's ear (ADR 0158), and the ABX
//! references for Arcus: `cargo run --release -p cantus-ui --example strings_listen --
//! <sources> <out> [abx.mid]` (or `tools/content/strings/render.sh`).
//!
//! `<sources>` is what `tools/content/strings/fetch_sources.sh` fetched. Each strings pack of
//! the bundled manifest is laid out there as an install lays it out (its zip's entries
//! unpacked, VSCO's files where they are) and its instruments written by the same build an
//! install runs (`stringsbuild`); each instrument is read as the picker reads it and played
//! by the engine offline. Into `<out>`, as 48 kHz 24-bit WAV, peak-normalised to −1 dBFS and
//! again at −23 LUFS (`*-lufs23.wav`):
//!
//! - `<pack>-<instrument>-line.wav`: four bars in the instrument's middle register — a legato
//!   pair (the scripted slur), a long note under a mod-wheel swell (piano to forte and back),
//!   détaché notes (the bow turning), and the same line's last bar as written.
//! - `abx-cello.wav`, `abx-bass.wav`: the round's ABX phrase (`tools/arcus/abx_phrase.mid`,
//!   the notes Arcus renders): the Bigcat cello on its "Cello" track, the Smolken bass on its
//!   "Double bass" track, the bar after the "pizzicato" marker plucked by the notes' own
//!   articulation, the phrase's 30 ms overlaps slurred by the scripted legato.

use cantus_content::stringsbuild;
use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::note_art::{Articulation, NoteArt};
use cantus_core::*;
use cantus_engine::{render, RenderOptions, SampleCache};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const RATE: u32 = 48_000;

/// Lay `pack` out under `sources` as an install would and write its instruments: the
/// folder.
fn lay_out(sources: &Path, pack: &cantus_content::manifest::Pack) -> PathBuf {
    use cantus_content::manifest::Source;
    let build = pack.strings.as_ref().expect("a strings pack");
    let dir = match &pack.source {
        Source::Files(_) => sources.join("vsco"),
        Source::Zip(z) => {
            let name = z.url.rsplit('/').next().unwrap_or("x.zip");
            let local = [
                "bigcat.zip",
                "smolken.zip",
                "meatbass.zip",
                "sneakybass.zip",
            ]
            .into_iter()
            .find(|l| {
                let size = std::fs::metadata(sources.join(l))
                    .map(|m| m.len())
                    .unwrap_or(0);
                size == z.size
            })
            .unwrap_or_else(|| {
                panic!("{name}: no local copy of its size in {}", sources.display())
            });
            let dir = sources.join("x").join(&pack.id);
            if !dir.join(".laid").exists() {
                use cantus_project::dawproject::zip;
                let mut f = std::fs::File::open(sources.join(local)).unwrap();
                for e in zip::directory(&mut f).unwrap() {
                    let Some(rel) = e.name.strip_prefix(z.strip.as_str()) else {
                        continue;
                    };
                    if !rel.to_ascii_lowercase().ends_with(".wav") {
                        continue;
                    }
                    let to = dir.join(rel);
                    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
                    let bytes = zip::read_entry(&mut f, &e, 1 << 30).unwrap();
                    std::fs::write(&to, bytes).unwrap();
                }
                std::fs::write(dir.join(".laid"), b"").unwrap();
            }
            dir
        }
        _ => panic!("{}: an archive kind this tool does not lay out", pack.id),
    };
    stringsbuild::build(build, &dir).expect("the instruments written");
    dir
}

/// A project with one Exemplar track playing `sfz`, and its clip to fill.
fn project_of(sfz: &Path, name: &str, bpm: f64) -> (Project, Clip) {
    let imp = cantus_ui::views::inst::sampler::read_instrument(sfz).expect("the instrument reads");
    let spec = cantus_ui::views::samples_content::sounds::spec_of(&imp);
    let mut p = Project::new(name);
    p.tempo_bpm = bpm;
    let mut t = p.make_track(name);
    t.strip = Some(p.strips[0].id);
    t.instrument = InstrumentSpec::Native(spec);
    p.tracks.push(t);
    let c = p.make_clip("line", Ticks::ZERO, Ticks::beats(16));
    (p, c)
}

fn rendered(p: &Project, end: Ticks) -> Vec<f32> {
    let mut snap = Snapshot::compile(p, 1);
    let errors = SampleCache::new().attach(&mut snap);
    assert!(errors.is_empty(), "{errors:?}");
    render(
        Arc::new(snap),
        &RenderOptions {
            sample_rate: RATE,
            end: Some(end),
            tail_seconds: 1.5,
            ..Default::default()
        },
    )
    .unwrap()
    .samples
}

/// Write `x` peak-normalised to −1 dBFS, and a copy at −23 LUFS beside it.
fn write_pair(out: &Path, name: &str, x: &[f32]) {
    let peak = x.iter().fold(0f32, |m, v| m.max(v.abs())).max(1e-9);
    let at = |g: f32| x.iter().map(|v| v * g).collect::<Vec<f32>>();
    let a = at(cantus_dsp::math::powf(10f32, -1.0 / 20.0) / peak);
    cantus_content::wav::write_stereo_24(&out.join(format!("{name}.wav")), RATE, &a).unwrap();
    let (l, r): (Vec<f32>, Vec<f32>) = x.chunks(2).map(|f| (f[0], f[1])).unzip();
    let mut m = cantus_dsp::terminus::loudness::LoudnessMeter::new(RATE as f32);
    m.process(&l, &r);
    if let Some(lufs) = m.integrated() {
        let g = cantus_dsp::math::powf(10f32, (-23.0 - lufs as f32) / 20.0);
        let b = at(g);
        cantus_content::wav::write_stereo_24(&out.join(format!("{name}-lufs23.wav")), RATE, &b)
            .unwrap();
        println!(
            "{name}: peak {:.1} dBFS, {lufs:.1} LUFS before",
            20.0 * cantus_dsp::math::log10(peak)
        );
    }
}

/// The four-bar line in the middle of `lo..=hi`: the legato pair, the swell, détaché notes.
fn line(p: &mut Project, c: &mut Clip, lo: u8, hi: u8) {
    let mid = ((lo as u16 + hi as u16) / 2) as u8;
    let key = mid - mid % 12 + 2; // a D near the middle
    let overlap = Ticks(48);
    let notes: [(f64, f64, i8, bool); 8] = [
        (0.0, 2.0, 0, true),
        (2.0, 2.0, 3, false),
        (4.0, 6.0, 7, false),
        (10.0, 1.0, 5, false),
        (11.0, 1.0, 3, false),
        (12.0, 1.0, 2, false),
        (13.0, 1.0, 0, false),
        (14.0, 2.0, -5, false),
    ];
    for (at, len, d, slur) in notes {
        let mut len = Ticks::from_beats_f64(len);
        if slur {
            len = len + overlap;
        }
        let n = p.make_note(
            Ticks::from_beats_f64(at),
            len - Ticks(if slur { 0 } else { 24 }),
            (key as i16 + d as i16) as u8,
            84,
        );
        c.notes.push(n);
    }
    c.lanes.push(ClipLane::new(
        LaneKind::Cc(1),
        vec![
            AutoPoint::new(Ticks::ZERO, 0.45),
            AutoPoint::new(Ticks::beats(4), 0.25),
            AutoPoint::new(Ticks::beats(7), 1.0),
            AutoPoint::new(Ticks::beats(10), 0.35),
            AutoPoint::new(Ticks::beats(16), 0.5),
        ],
    ));
}

/// The ABX phrase's track `name` (its notes, its CC1, the bar after the "pizzicato" marker
/// plucked) into the clip.
fn abx(mid: &[u8], name: &str, p: &mut Project, c: &mut Clip) -> Ticks {
    let imp = cantus_project::midi::import::read(mid, &Default::default()).expect("the phrase");
    let pizz = imp
        .markers
        .iter()
        .find(|(_, m)| m == "pizzicato")
        .map(|(t, _)| *t)
        .unwrap_or(Ticks(i64::MAX));
    let t = imp
        .tracks
        .iter()
        .find(|t| t.label.english().contains(name))
        .unwrap_or_else(|| panic!("the track {name}"));
    for n in &t.notes {
        let mut note = p.make_note(n.start, n.length, n.pitch, n.velocity);
        if n.start >= pizz {
            note.art = Some(NoteArt {
                articulation: Articulation::Pizzicato,
                bow: None,
            });
        }
        c.notes.push(note);
    }
    let points: Vec<AutoPoint> = t
        .controls
        .iter()
        .filter(|(_, k, _)| *k == LaneKind::Cc(1))
        .map(|(tk, _, v)| AutoPoint::new(*tk, *v))
        .collect();
    c.lanes.push(ClipLane::new(LaneKind::Cc(1), points));
    p.tempo_bpm = imp.tempo_bpm;
    c.length = imp.end + Ticks::beats(1);
    imp.end
}

fn main() {
    let mut args = std::env::args().skip(1);
    let sources = PathBuf::from(
        args.next()
            .expect("usage: strings_listen <sources> <out> [abx.mid]"),
    );
    let out = PathBuf::from(args.next().expect("the output folder"));
    let mid = args.next().map(PathBuf::from);
    std::fs::create_dir_all(&out).unwrap();
    let manifest = cantus_content::Manifest::bundled();
    for pack in manifest.packs.iter().filter(|p| p.strings.is_some()) {
        let dir = lay_out(&sources, pack);
        for inst in &pack.strings.as_ref().unwrap().instruments {
            let lo = inst.zones.iter().map(|z| z.lo).min().unwrap_or(36);
            let hi = inst.zones.iter().map(|z| z.hi).max().unwrap_or(72);
            let id = inst
                .sfz
                .rsplit('/')
                .next()
                .unwrap()
                .trim_end_matches(".sfz");
            let (mut p, mut c) = project_of(&dir.join(&inst.sfz), &inst.name, 72.0);
            line(&mut p, &mut c, lo, hi);
            p.tracks[0].clips.push(c);
            write_pair(
                &out,
                &format!("{}-{id}-line", pack.id),
                &rendered(&p, Ticks::beats(16)),
            );
        }
        if let Some(mid) = &mid {
            let bytes = std::fs::read(mid).expect("the ABX phrase");
            for (pid, track, file) in [
                ("karoryfer-bigcat-cello", "Cello", "abx-cello"),
                ("dsmolken-double-bass", "Double bass", "abx-bass"),
            ] {
                if pack.id != pid {
                    continue;
                }
                let inst = &pack.strings.as_ref().unwrap().instruments[0];
                let (mut p, mut c) = project_of(&dir.join(&inst.sfz), track, 72.0);
                let end = abx(&bytes, track, &mut p, &mut c);
                p.tracks[0].clips.push(c);
                write_pair(&out, file, &rendered(&p, end));
            }
        }
    }
}
