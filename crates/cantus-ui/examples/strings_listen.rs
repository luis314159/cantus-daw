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
//!
//! `strings_listen -- fit <sources> <deltas.json>` levels every instrument and preset as the
//! house's `preset_tool -- fit` levels the factory presets: its role's phrase
//! (`preset_audition`, the mod wheel at half) to its role's target (`preset_measure`: a bass
//! −16, a section's sustain a pad's −19, a short note a pluck's −21 LUFS), never above the
//! headroom test's dense riff at velocity 127 **with the mod wheel full** under −1 dBFS. The
//! decibels each needs go to `deltas.json`, which `merge.py` adds to its gain.

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
fn write_pair(out: &Path, name: &str, x: &[f32]) -> Option<f64> {
    let peak = x.iter().fold(0f32, |m, v| m.max(v.abs())).max(1e-9);
    let at = |g: f32| x.iter().map(|v| v * g).collect::<Vec<f32>>();
    let a = at(cantus_dsp::math::powf(10f32, -1.0 / 20.0) / peak);
    cantus_content::wav::write_stereo_24(&out.join(format!("{name}.wav")), RATE, &a).unwrap();
    let (l, r): (Vec<f32>, Vec<f32>) = x.chunks(2).map(|f| (f[0], f[1])).unzip();
    let mut m = cantus_dsp::terminus::loudness::LoudnessMeter::new(RATE as f32);
    m.process(&l, &r);
    let lufs = m.integrated();
    if let Some(lufs) = lufs {
        let g = cantus_dsp::math::powf(10f32, (-23.0 - lufs as f32) / 20.0);
        let b = at(g);
        cantus_content::wav::write_stereo_24(&out.join(format!("{name}-lufs23.wav")), RATE, &b)
            .unwrap();
        println!(
            "{name}: peak {:.1} dBFS, {lufs:.1} LUFS before",
            20.0 * cantus_dsp::math::log10(peak)
        );
    }
    lufs
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
            len += overlap;
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
    // By its name, or by its channel (the phrase: the cello on channel 1, the bass on 2).
    let channel = if name == "Cello" { 0 } else { 1 };
    let t = imp
        .tracks
        .iter()
        .find(|t| t.label.english().contains(name))
        .or_else(|| {
            imp.tracks
                .iter()
                .find(|t| t.channel == channel && !t.notes.is_empty())
        })
        .unwrap_or_else(|| {
            let names: Vec<String> = imp.tracks.iter().map(|t| t.label.english()).collect();
            panic!("the track {name} among {names:?}")
        });
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

/// The role a strings instrument plays (`preset_tags::Role`): a short note a pluck, a bass a
/// bass, everything else bowed and held a pad.
fn role_of(sfz: &str) -> cantus_ui::preset_tags::Role {
    use cantus_ui::preset_tags::Role;
    let id = sfz.rsplit('/').next().unwrap_or(sfz);
    if ["pizzicato", "spiccato", "staccato", "sneakybass"]
        .iter()
        .any(|a| id.contains(a))
    {
        Role::Pluck
    } else if id.contains("bass") {
        Role::Bass
    } else {
        Role::Pad
    }
}

/// Every clip of the tracks playing `spec` with the mod wheel held at `v`.
fn with_wheel(p: &mut Project, v: f32) {
    for t in &mut p.tracks {
        for c in &mut t.clips {
            c.lanes.retain(|l| l.kind != LaneKind::Cc(1));
            c.lanes.push(ClipLane::new(
                LaneKind::Cc(1),
                vec![AutoPoint::new(Ticks::ZERO, v)],
            ));
        }
    }
}

fn render_all(p: &Project, start: Ticks, end: Option<Ticks>, tail: f64) -> (Vec<f32>, f32) {
    let mut snap = Snapshot::compile(p, 0);
    let errors = SampleCache::new().attach(&mut snap);
    assert!(errors.is_empty(), "{errors:?}");
    let r = render(
        Arc::new(snap),
        &RenderOptions {
            start,
            end,
            tail_seconds: tail,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .unwrap();
    (r.samples, r.peak)
}

/// The decibels `spec` needs: its role's target on its phrase, within the riff's headroom.
fn fit_one(spec: &NativeSpec, sfz: &str) -> (f64, f64, f64) {
    use cantus_ui::preset_audition as aud;
    use cantus_ui::preset_catalog::{Entry, Payload, Source, Unit};
    let role = role_of(sfz);
    let e = Entry {
        id: sfz.to_string(),
        unit: Unit::Instrument(InstrumentKind::Sampler),
        name: sfz.to_string(),
        source: Source::Factory,
        styles: Vec::new(),
        roles: vec![role],
        payload: Payload::Native(spec.clone()),
    };
    let empty = Project::new("none");
    let mut prep = aud::prepare(&empty, aud::Target::None, &e, None, aud::TEMPO);
    with_wheel(&mut prep.project, 0.5);
    let (x, _) = render_all(&prep.project, prep.start, Some(prep.end), aud::TAIL);
    let lufs = cantus_ui::preset_measure::lufs(&x, 2, 48_000.0);
    let mut riff = aud::riff_project(spec);
    with_wheel(&mut riff, 1.0);
    let (_, peak) = render_all(&riff, Ticks::ZERO, None, 1.0);
    let peak_db = 20.0 * f64::from(peak.max(1e-9)).log10();
    let want = cantus_ui::preset_measure::role_target(role) - lufs;
    let room = cantus_ui::preset_measure::HEADROOM_DB - peak_db;
    (want.min(room), lufs, peak_db)
}

fn fit_all(sources: &Path, out: &Path) {
    cantus_ui::i18n::set_thread_locale(cantus_ui::i18n::Locale::En);
    let manifest = cantus_content::Manifest::bundled();
    let mut deltas = serde_json::Map::new();
    for pack in manifest.packs.iter().filter(|p| p.strings.is_some()) {
        let dir = lay_out(sources, pack);
        for inst in &pack.strings.as_ref().unwrap().instruments {
            let imp = cantus_ui::views::inst::sampler::read_instrument(&dir.join(&inst.sfz))
                .expect("the instrument reads");
            let spec = cantus_ui::views::samples_content::sounds::spec_of(&imp);
            let (d, lufs, peak) = fit_one(&spec, &inst.sfz);
            println!(
                "{} {}: {lufs:.1} LUFS on its phrase, riff peak {peak:.1} dBFS at ff: {d:+.1} dB",
                pack.id, inst.sfz
            );
            deltas.insert(
                format!("{}/{}", pack.id, inst.sfz),
                serde_json::json!((d * 10.0).round() / 10.0),
            );
        }
    }
    std::fs::write(out, serde_json::to_string_pretty(&deltas).unwrap()).unwrap();
}

fn main() {
    let mut args = std::env::args().skip(1).peekable();
    if args.peek().map(String::as_str) == Some("fit") {
        args.next();
        let sources = PathBuf::from(args.next().expect("fit <sources> <deltas.json>"));
        let out = PathBuf::from(args.next().expect("the deltas' file"));
        fit_all(&sources, &out);
        return;
    }
    let sources = PathBuf::from(
        args.next()
            .expect("usage: strings_listen <sources> <out> [abx.mid]"),
    );
    let out = PathBuf::from(args.next().expect("the output folder"));
    let mid = args.next().map(PathBuf::from);
    std::fs::create_dir_all(&out).unwrap();
    let manifest = cantus_content::Manifest::bundled();
    // Each line's loudness before normalising: what `merge.py` levels the instruments by.
    let mut levels = serde_json::Map::new();
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
            let lufs = write_pair(
                &out,
                &format!("{}-{id}-line", pack.id),
                &rendered(&p, Ticks::beats(16)),
            );
            if let Some(l) = lufs {
                levels.insert(format!("{}/{}", pack.id, inst.sfz), serde_json::json!(l));
            }
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
    std::fs::write(
        out.join("levels.json"),
        serde_json::to_string_pretty(&levels).unwrap(),
    )
    .unwrap();
}
