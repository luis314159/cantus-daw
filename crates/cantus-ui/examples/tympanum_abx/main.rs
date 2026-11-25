//! Tympanum's blind listening kit (round 42; the owner: *"¿No hay ABX de Tympanum?"*):
//! Tympanum as it ships — the modelled kit's generated voice and its *Hybrid* voice —
//! against the recording each kit was measured from (ADR 0124: the studio kit and DRSKit,
//! CC BY 4.0; the jazz kit and Virtuosity Drums, CC0), the same MIDI through each, at one
//! loudness. Chorda's kit's method and page (`abx_kit/`, `docs/round40/abx5.md`).
//!
//! ```text
//! cargo run --release -p cantus-ui --example tympanum_abx -- <dir> [--downloads <dir>]
//!     [--key <dir>] [--wav | --flac] [--wav-twin <dir>]
//! ```
//!
//! The four packs (`drskit-tympanum`, `virtuosity-drums`, `tympanum-hybrid-studio`,
//! `tympanum-hybrid-jazz`) are installed through Cantus's own pack code (download, SHA-256,
//! unpack — what *Sound packs…* does) into `<downloads>/content` unless they are there at the
//! manifest's version (default `$CANTUS_REFS/tympanum`, tools/sound-refs), read by the app's
//! kit loader (`tympanum_load`), and played by the engine's offline render (the export's
//! path), each kit's own mix of its microphones — what a user hears on loading it. A modelled
//! kit stands as its design draws it and a recorded kit as it was miked: when a pair's
//! modelled voices are the recording's mirror (the hats on the other side), they are mirrored
//! (left and right swapped, nothing else), decided per pair from its items' left/right
//! balance and written in the key.
//!
//! Twelve items (`items.rs`): eight pieces alone at two or three velocities, four grooves.
//! Each item's three renders are cut to one length, faded out over 100 ms and brought to
//! −23 LUFS (ITU-R BS.1770), lowered together if a peak would pass −1 dBFS; 80 ms of silence
//! before the first hit.
//!
//! Written into `<dir>` (what a listener receives; nothing in it says which is which):
//! `audio/<64 random bits>.flac` (or `.wav` with `--wav`, the kit to publish as a claude.ai
//! page), `index.html` (the page: part 1 ABX generated vs recording, part 2 preferences —
//! generated vs recording and hybrid vs recording, every item, two catch questions — and an
//! optional ABX hybrid vs recording), `LEEME.md` / `README.md`. Into `--key` (default
//! `<dir>-key`, never sent): `key.json` (which source each token and file is, the items, the
//! packs and the residuals' model hashes) and `levels.csv` (each file's loudness before and
//! after, its stereo balance and width, its first onset), `hits.csv` (each piece's hits'
//! levels). `--wav-twin <dir>`: the same kit (the same id, names, samples and page) with
//! 24-bit WAV clips into `<dir>` too — the one to publish, whose answers pool with the FLAC
//! kit's.

mod items;
#[path = "../abx_kit/kit.rs"]
#[allow(dead_code)] // shared by the listening kits: each uses part of it
mod kit;
#[path = "../abx_kit/names.rs"]
mod names;

use cantus_core::clip_lanes::{ClipLane, LaneKind};
use cantus_core::inst::tympanum::voice::{hybrid_pack, HYBRID, RESIDUAL_FILE};
use cantus_core::inst::tympanum::{KitRef, KIT_KEY, VOICE_KEY};
use cantus_core::*;
use cantus_engine::{render, EngineHandle, RenderOptions};
use items::{Item, Pair};
use kit::js;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// `$CANTUS_REFS` joined with `sub`: the sound references' home outside every round's folder
/// (`tools/sound-refs`; default `F:/projects/cantus-refs` on Windows, `~/cantus-refs` elsewhere).
fn refs_dir(sub: &str) -> PathBuf {
    let root = std::env::var_os("CANTUS_REFS")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            if cfg!(windows) {
                PathBuf::from("F:/projects/cantus-refs")
            } else {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join("cantus-refs")
            }
        });
    root.join(sub)
}

const SUBJECT: &str = include_str!("subject.json");
const README_EN: &str = include_str!("readme.en.md");
const README_ES: &str = include_str!("readme.es.md");

const SR: u32 = 48_000;
/// Drums sit lower than a piano's phrases: −23 LUFS leaves most items' peaks under the
/// ceiling (a snare at 125 is ~20 dB over its loudness).
const TARGET: f64 = -23.0;
const CEILING_DB: f64 = -1.0;
const FADE_S: f64 = 0.1;
const PRE_ROLL_S: f64 = 0.08;

/// The three voices: an id (the key's, the page's `voice_<id>`).
const VOICES: [&str; 3] = ["model", "recording", "hybrid"];
const PACKS: [&str; 4] = [
    "drskit-tympanum",
    "virtuosity-drums",
    "tympanum-hybrid-studio",
    "tympanum-hybrid-jazz",
];

struct Args {
    dir: PathBuf,
    downloads: PathBuf,
    key: PathBuf,
    flac: bool,
    twin: Option<PathBuf>,
}

fn usage() -> ! {
    eprintln!(
        "usage: tympanum_abx <dir> [--downloads <dir>] [--key <dir>] [--wav | --flac] [--wav-twin <dir>]"
    );
    std::process::exit(2);
}

fn args() -> Args {
    let mut it = std::env::args().skip(1);
    let (mut dir, mut downloads, mut key, mut flac, mut twin) = (None, None, None, true, None);
    while let Some(a) = it.next() {
        let mut val = || it.next().unwrap_or_else(|| usage());
        match a.as_str() {
            "--downloads" => downloads = Some(PathBuf::from(val())),
            "--key" => key = Some(PathBuf::from(val())),
            "--wav" => flac = false,
            "--flac" => flac = true,
            "--wav-twin" => twin = Some(PathBuf::from(val())),
            _ if a.starts_with("--") => usage(),
            _ if dir.is_none() => dir = Some(PathBuf::from(a)),
            _ => usage(),
        }
    }
    let dir = dir.unwrap_or_else(|| usage());
    let key = key.unwrap_or_else(|| {
        let mut k = dir.clone().into_os_string();
        k.push("-key");
        PathBuf::from(k)
    });
    Args {
        dir,
        downloads: downloads.unwrap_or_else(|| refs_dir("tympanum")),
        key,
        flac,
        twin,
    }
}

/// Every pack at the manifest's version, installed through Cantus's own pack code if not.
fn install(root: &Path) -> Result<String, String> {
    let manifest = cantus_content::Manifest::bundled();
    let mut m = cantus_content::Manager::new(root, manifest);
    let mut said = String::new();
    for id in PACKS {
        let want = m.manifest().pack(id).map(|p| p.version).ok_or(id)?;
        if !matches!(m.status(id), cantus_content::Status::Installed { version } if version == want)
        {
            eprintln!("installing {id} (version {want}) into {} …", root.display());
            if !m.install(id) {
                return Err(format!("{id} could not start installing"));
            }
            for f in m.wait_idle(std::time::Duration::from_secs(4 * 3600)) {
                f.result.map_err(|e| format!("{id}: {e}"))?;
            }
        }
        match m.status(id) {
            cantus_content::Status::Installed { version } if version == want => {
                let _ = write!(
                    said,
                    "{}{}:{}",
                    if said.is_empty() { "" } else { "," },
                    js(id),
                    version
                );
            }
            s => return Err(format!("{id} is not installed at version {want} ({s:?})")),
        }
    }
    Ok(format!("{{{said}}}"))
}

/// Each hybrid pack's residual: the model hash it was fitted on, and this build's.
fn residuals(root: &Path) -> String {
    let build = cantus_dsp::tympanum_residual::model_hash();
    let mut o = String::new();
    for kit in ["studio", "jazz"] {
        let pack = hybrid_pack(kit).expect("a kit with a hybrid");
        let bytes = std::fs::read(root.join(pack).join(RESIDUAL_FILE))
            .unwrap_or_else(|e| panic!("{pack}'s residual: {e}"));
        let r = cantus_dsp::tympanum_residual::Residual::decode(&bytes)
            .unwrap_or_else(|e| panic!("{pack}'s residual: {e}"));
        eprintln!(
            "{pack}: fitted on model {} (this build {build}){}",
            r.model,
            if r.model == build {
                ""
            } else {
                " — STALE: it plays, but was fitted on other tables"
            }
        );
        let _ = write!(
            o,
            "{}{}:{{\"fitted_on\":{},\"build\":{build},\"current\":{}}}",
            if o.is_empty() { "" } else { "," },
            js(pack),
            r.model,
            r.model == build
        );
    }
    format!("{{{o}}}")
}

fn ticks(beats: f64) -> Ticks {
    Ticks((beats * PPQ as f64).round() as i64)
}

/// The item as a one-track song: Tympanum on `kit`, generated or hybrid, the notes and the
/// pedal's CC4 as a lane (a step at each change: the value before held to just before it).
fn project(item: &Item, kit: &str, hybrid: bool) -> Project {
    let mut p = Project::new("Tympanum ABX");
    p.tempo_bpm = item.bpm;
    let mut t = p.make_track("Drums");
    let mut spec = NativeSpec::new(InstrumentKind::AcousticDrums);
    spec.set_text(KIT_KEY, Some(kit.to_string())).unwrap();
    if hybrid {
        spec.set_text(VOICE_KEY, Some(HYBRID.to_string())).unwrap();
    }
    t.instrument = InstrumentSpec::Native(spec);
    let mut c = p.make_clip("Item", Ticks::ZERO, ticks(item.end));
    for &(at, key, vel) in &item.hits {
        let n = p.make_note(ticks(at), Ticks(PPQ / 8), key, vel);
        c.notes.push(n);
    }
    let mut pts: Vec<automation::AutoPoint> = Vec::new();
    for &(at, v) in &item.pedal {
        if let Some(last) = pts.last().map(|p| p.value) {
            pts.push(automation::AutoPoint::new(Ticks(ticks(at).0 - 1), last));
        }
        pts.push(automation::AutoPoint::new(ticks(at), v));
    }
    let last = pts.last().map_or(0.0, |p| p.value);
    pts.push(automation::AutoPoint::new(ticks(item.end), last));
    c.lanes.push(ClipLane::new(LaneKind::Cc(4), pts));
    t.clips.push(c);
    p.tracks.push(t);
    p
}

/// One render through the engine's offline path (the export's), as left and right, with
/// the pre-roll's silence before it.
fn render_item(
    engine: &mut EngineHandle,
    item: &Item,
    kit: &str,
    hybrid: bool,
    rev: u64,
) -> (Vec<f32>, Vec<f32>) {
    let p = project(item, kit, hybrid);
    let snap = engine.compile_for_render(&p, rev);
    let r = render(
        snap,
        &RenderOptions {
            sample_rate: SR,
            end: Some(ticks(item.end)),
            tail_seconds: item.tail_s,
            compensate_latency: true,
            ..Default::default()
        },
    )
    .unwrap_or_else(|e| panic!("{} on {kit}: {e:?}", item.id));
    assert_eq!(
        r.cut_notes, 0,
        "{} on {kit}: notes cut short by streaming",
        item.id
    );
    let pre = (PRE_ROLL_S * SR as f64) as usize;
    let (mut l, mut rr) = (vec![0.0; pre], vec![0.0; pre]);
    for f in r.samples.chunks_exact(2) {
        l.push(f[0]);
        rr.push(f[1]);
    }
    (l, rr)
}

/// The stereo picture: left over right (dB of energy), side over mid (dB), and the first
/// onset (ms: the first sample within 20 dB of the clip's peak).
fn picture(l: &[f32], r: &[f32]) -> (f64, f64, f64) {
    let e = |x: &mut dyn Iterator<Item = f64>| x.map(|v| v * v).sum::<f64>().max(1e-30);
    let el = e(&mut l.iter().map(|x| *x as f64));
    let er = e(&mut r.iter().map(|x| *x as f64));
    let em = e(&mut l.iter().zip(r).map(|(a, b)| 0.5 * (*a + *b) as f64));
    let es = e(&mut l.iter().zip(r).map(|(a, b)| 0.5 * (*a - *b) as f64));
    let ten = |x: f64| 10.0 * cantus_dsp::math::log10(x);
    let p = kit::peak(l, r) * 0.1;
    let onset = l
        .iter()
        .zip(r)
        .position(|(a, b)| a.abs().max(b.abs()) >= p)
        .unwrap_or(0);
    (
        ten(el / er),
        ten(es / em),
        onset as f64 * 1000.0 / SR as f64,
    )
}

fn sep(s: &mut String) {
    if !s.is_empty() {
        s.push(',');
    }
}

fn main() {
    let a = args();
    let ext = if a.flac { "flac" } else { "wav" };
    let root = a.downloads.join("content");
    let packs = install(&root).unwrap_or_else(|e| {
        eprintln!("the packs could not be installed: {e}");
        std::process::exit(1);
    });
    let residuals = residuals(&root);
    cantus_ui::tympanum_load::install(root.clone());
    let cache = a.downloads.join("stream-cache");
    let _ = std::fs::create_dir_all(&cache);
    cantus_engine::inst::stream::hub().set_cache_dir(cache);
    for pair in [Pair::Studio, Pair::Jazz] {
        let kit = format!("pack:{}", pair.recording());
        let r = KitRef::parse(&kit).expect("pack/kit");
        let def = cantus_engine::inst::tympanum::cache::recorded(&r).unwrap_or_else(|| {
            let why = cantus_engine::inst::tympanum::cache::refused(&r.text());
            panic!("{kit} did not read: {why:?}")
        });
        eprintln!("{kit}: {} mics", def.def.mics.len());
        for line in &def.log {
            eprintln!("  {line}");
        }
    }
    let _ = std::fs::remove_dir_all(a.dir.join("audio"));
    if let Some(t) = &a.twin {
        let _ = std::fs::remove_dir_all(t.join("audio"));
        std::fs::create_dir_all(t.join("audio")).expect("the twin's folder");
    }
    std::fs::create_dir_all(a.dir.join("audio")).expect("the kit's folder");
    std::fs::create_dir_all(&a.key).expect("the key's folder");
    let mut rng = names::Rng::new();
    let kit_id = format!("drums1-{}", &rng.name()[..10]);
    // Each voice's token in the page, and two more for the catch questions' second names.
    let tok: Vec<String> = (0..5).map(|_| rng.token()).collect();
    let all = items::all();
    // The catch questions: two items drawn, one the recording against itself, one the
    // generated voice against itself (one file under two names). (item, voice, token)
    let c1 = rng.below(all.len());
    let c2 = (c1 + 1 + rng.below(all.len() - 1)) % all.len();
    let catch = [(all[c1].id, 1usize, 3usize), (all[c2].id, 0, 4)];
    let count = all.len() * VOICES.len() + catch.len();
    let drawn = loop {
        let v: Vec<String> = (0..count).map(|_| rng.name()).collect();
        if names::names_look_random(&v).is_ok() {
            break v;
        }
    };
    let mut drawn = drawn.into_iter();
    let mut levels = String::from(
        "item,voice,file,lufs_before,peak_before_dbfs,gain_db,lufs_after,peak_after_dbfs,balance_lr_db,side_mid_db,onset_ms\n",
    );
    let (mut list, mut files, mut key_files, mut key_items) =
        (String::new(), String::new(), String::new(), String::new());
    let mut bytes = 0u64;
    let (mut engine, _proc) = EngineHandle::manual(SR as f64);
    let mut rev = 1;
    // Every item's three renders first (generated, recorded, hybrid), as the kits ship.
    let mut rendered: Vec<Vec<(Vec<f32>, Vec<f32>)>> = Vec::new();
    let mut diffs = Vec::new();
    let mut hits = String::from("item,voice,hit,at_s,key,velocity,rms80_db,below_loudest_db\n");
    for item in &all {
        let model = format!("model:{}", item.pair.model());
        let rec = format!("pack:{}", item.pair.recording());
        let mut renders = Vec::new();
        for (kit, hybrid) in [(&model, false), (&rec, false), (&model, true)] {
            renders.push(render_item(&mut engine, item, kit, hybrid, rev));
            rev += 1;
        }
        // The hybrid must be heard: a pack the engine did not read plays the model to the bit.
        let diff = {
            let (g, h) = (&renders[0], &renders[2]);
            let d: f64 =
                g.0.iter()
                    .zip(&h.0)
                    .chain(g.1.iter().zip(&h.1))
                    .map(|(a, b)| ((a - b) as f64).powi(2))
                    .sum();
            let e: f64 = g.0.iter().chain(&g.1).map(|a| (*a as f64).powi(2)).sum();
            10.0 * cantus_dsp::math::log10((d / e.max(1e-30)).max(1e-30))
        };
        assert!(
            diff > -60.0,
            "{}: the hybrid is the model to {diff:.1} dB: its pack was not heard",
            item.id
        );
        // A piece's hits, each one's level as rendered (80 ms from its note), to read the
        // voices' dynamics against the recording's.
        if item.bpm == 60.0 {
            for (v, (l, r)) in renders.iter().enumerate() {
                let lv: Vec<f64> = item
                    .hits
                    .iter()
                    .map(|h| {
                        let at = ((PRE_ROLL_S + h.0) * SR as f64) as usize;
                        let n = (0.08 * SR as f64) as usize;
                        let e: f64 = (at..(at + n).min(l.len()))
                            .map(|i| 0.5 * ((l[i] as f64).powi(2) + (r[i] as f64).powi(2)))
                            .sum();
                        10.0 * cantus_dsp::math::log10((e / n as f64).max(1e-30))
                    })
                    .collect();
                let top = lv.iter().cloned().fold(f64::MIN, f64::max);
                for (j, (h, x)) in item.hits.iter().zip(&lv).enumerate() {
                    let _ = writeln!(
                        hits,
                        "{},{},{j},{:.2},{},{},{x:.2},{:.2}",
                        item.id,
                        VOICES[v],
                        h.0,
                        h.1,
                        h.2,
                        top - x
                    );
                }
            }
        }
        rendered.push(renders);
        diffs.push(diff);
    }
    // The stereo picture: a modelled kit stands as its design draws it and a recorded kit as it
    // was miked, so one may be the other's mirror (the hats on the other side). Which way each
    // pair's modelled voices face is read off its items' left/right balance, and they are
    // mirrored (left and right swapped, nothing else) when that brings them closer to the
    // recording's — a convention, like the level, not a sound.
    let mut mirrored = String::new();
    for pair in [Pair::Studio, Pair::Jazz] {
        let (mut as_is, mut swapped) = (0.0, 0.0);
        for (item, r) in all.iter().zip(&rendered) {
            if item.pair != pair {
                continue;
            }
            let rec = picture(&r[1].0, &r[1].1).0;
            for v in [0, 2] {
                let b = picture(&r[v].0, &r[v].1).0;
                as_is += (b - rec).abs();
                swapped += (-b - rec).abs();
            }
        }
        let mirror = swapped < as_is;
        eprintln!(
            "{}: modelled voices {} (left/right mismatch {as_is:.1} dB as they are, {swapped:.1} mirrored)",
            pair.model(),
            if mirror { "mirrored" } else { "as they are" }
        );
        sep(&mut mirrored);
        let _ = write!(
            mirrored,
            "{}:{{\"mirrored\":{mirror},\"mismatch_as_is_db\":{as_is:.2},\"mismatch_mirrored_db\":{swapped:.2}}}",
            js(pair.model())
        );
        if mirror {
            for (item, r) in all.iter().zip(rendered.iter_mut()) {
                if item.pair == pair {
                    for v in [0, 2] {
                        let (l, rr) = &mut r[v];
                        std::mem::swap(l, rr);
                    }
                }
            }
        }
    }
    for ((item, mut renders), diff) in all.iter().zip(rendered).zip(diffs) {
        let model = format!("model:{}", item.pair.model());
        let rec = format!("pack:{}", item.pair.recording());
        let m = {
            let mut g: Vec<_> = renders.iter_mut().map(|(l, r)| (l, r)).collect();
            kit::match_group(&mut g, SR, FADE_S, TARGET, CEILING_DB)
        };
        let secs = renders[0].0.len() as f64 / SR as f64;
        for (i, ((l, r), ((lu, pk), g))) in renders
            .iter()
            .zip(m.before.iter().zip(&m.gains))
            .enumerate()
        {
            let mut toks = vec![i];
            toks.extend(
                catch
                    .iter()
                    .filter(|(id, v, _)| *id == item.id && *v == i)
                    .map(|c| c.2),
            );
            let (bal, width, onset) = picture(l, r);
            let mut first: Option<PathBuf> = None;
            for t in toks {
                let file = format!("{}.{ext}", drawn.next().expect("a name"));
                let path = a.dir.join("audio").join(&file);
                match &first {
                    None => {
                        let w = if a.flac {
                            kit::write_flac(&path, l, r, SR)
                        } else {
                            kit::write_wav(&path, l, r, SR)
                        };
                        w.unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                        first = Some(path.clone());
                        if let Some(t) = &a.twin {
                            let tw = t.join("audio").join(&file).with_extension("wav");
                            kit::write_wav(&tw, l, r, SR)
                                .unwrap_or_else(|e| panic!("{}: {e}", tw.display()));
                        }
                    }
                    Some(f) => {
                        std::fs::copy(f, &path).expect("the catch question's copy");
                        if let Some(t) = &a.twin {
                            std::fs::copy(
                                t.join("audio")
                                    .join(f.file_name().unwrap())
                                    .with_extension("wav"),
                                t.join("audio").join(&file).with_extension("wav"),
                            )
                            .expect("the twin's catch copy");
                        }
                    }
                }
                bytes += std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                sep(&mut files);
                let _ = write!(
                    files,
                    "{}:{}",
                    js(&format!("{}|{}", item.id, tok[t])),
                    js(&file)
                );
                sep(&mut key_files);
                let _ = write!(
                    key_files,
                    "{}:{{\"item\":{},\"voice\":{},\"catch\":{}}}",
                    js(&file),
                    js(item.id),
                    js(VOICES[i]),
                    t >= 3
                );
                let _ = writeln!(
                    levels,
                    "{},{},{file},{lu:.2},{pk:.2},{g:.2},{:.2},{:.2},{bal:.2},{width:.2},{onset:.1}",
                    item.id,
                    VOICES[i],
                    kit::lufs(l, r, SR),
                    kit::db(kit::peak(l, r)),
                );
            }
        }
        sep(&mut key_items);
        let _ = write!(
            key_items,
            "{}:{{\"model\":{},\"recording\":{},\"bpm\":{},\"notes\":{},\"seconds\":{secs:.2},\"target_lufs\":{:.2},\"hybrid_vs_model_db\":{diff:.1}}}",
            js(item.id),
            js(&model),
            js(&rec),
            item.bpm,
            item.hits.len(),
            m.target
        );
        eprintln!(
            "{:<11} {:>5.1} s at {:.1} LUFS; hybrid − model {diff:.1} dB",
            item.id, secs, m.target
        );
        sep(&mut list);
        list.push_str(&js(item.id));
    }
    assert!(drawn.next().is_none(), "every drawn name used");
    // Which voice each token is: veiled in the page, plain in the key.
    let mut table = String::new();
    let of = [VOICES[0], VOICES[1], VOICES[2], VOICES[1], VOICES[0]];
    for (t, id) in tok.iter().zip(of) {
        sep(&mut table);
        let _ = write!(table, "{}:{}", js(t), js(id));
    }
    let table = format!("{{{table}}}");
    let seed = (rng.next() >> 32) as u32 | 1;
    let pair = |x: usize, y: usize| format!("[{},{}]", js(&tok[x]), js(&tok[y]));
    let catch_js = |c: &(&str, usize, usize)| {
        format!(
            "{{phrase:{},one:{},two:{}}}",
            js(c.0),
            js(&tok[c.1]),
            js(&tok[c.2])
        )
    };
    // Part 1: generated vs recording; the preferences: generated vs recording and hybrid vs
    // recording; the extra part: hybrid vs recording.
    let tests = format!(
        "{{abx:{},pref:[{},{}],extra:{},catch:[{},{}]}}",
        pair(0, 1),
        pair(0, 1),
        pair(2, 1),
        pair(2, 1),
        catch_js(&catch[0]),
        catch_js(&catch[1]),
    );
    let page = kit::fill_page([
        &js(&kit_id),
        &format!("[{list}]"),
        &format!("{{{files}}}"),
        &tests,
        &seed.to_string(),
        &js(&names::veil(&table, seed)),
        &kit::subject_line(SUBJECT),
    ]);
    std::fs::write(a.dir.join("index.html"), &page).expect("index.html");
    std::fs::write(a.dir.join("README.md"), README_EN).expect("README.md");
    std::fs::write(a.dir.join("LEEME.md"), README_ES).expect("LEEME.md");
    if let Some(t) = &a.twin {
        // The same page, its files' names ending in .wav (only the FILES table names files).
        let twin = page.replace(&format!(".{ext}\""), ".wav\"");
        std::fs::write(t.join("index.html"), twin).expect("the twin's index.html");
        std::fs::write(t.join("README.md"), README_EN).expect("README.md");
        std::fs::write(t.join("LEEME.md"), README_ES).expect("LEEME.md");
    }
    bytes += (page.len() + README_EN.len() + README_ES.len()) as u64;
    let created = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let key = format!(
        "{{\n \"kit\": {},\n \"created\": {created},\n \"instrument\": \"tympanum\",\n \"packs\": {packs},\n \"residuals\": {residuals},\n \"stereo\": {{{mirrored}}},\n \"tokens\": {table},\n \"catch\": [{}, {}],\n \"items\": {{{key_items}}},\n \"files\": {{{key_files}}},\n \"bytes\": {bytes}\n}}\n",
        js(&kit_id),
        js(catch[0].0),
        js(catch[1].0),
    );
    std::fs::write(a.key.join("key.json"), key).expect("key.json");
    std::fs::write(a.key.join("levels.csv"), levels).expect("levels.csv");
    std::fs::write(a.key.join("hits.csv"), hits).expect("hits.csv");
    eprintln!(
        "kit {kit_id} in {} — {:.1} MB; the key in {}",
        a.dir.display(),
        bytes as f64 / 1e6,
        a.key.display()
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_subject_is_a_subject() {
        let s = super::kit::subject_line(super::SUBJECT);
        for v in super::VOICES {
            assert!(s.contains(&format!("\"voice_{v}\"")), "{v}");
        }
        // Every item has a name in both languages.
        let j: serde_json::Value = serde_json::from_str(super::SUBJECT).unwrap();
        for i in super::items::all() {
            for l in ["es", "en"] {
                assert!(j["items"][l][i.id].is_string(), "{} in {l}", i.id);
            }
        }
    }

    #[test]
    fn the_readmes_never_name_the_sources() {
        for t in [super::README_EN, super::README_ES] {
            for w in [
                "Tympanum",
                "DRSKit",
                "Virtuosity",
                "cargo",
                "Luis",
                "ronda",
                "round",
            ] {
                assert!(!t.contains(w), "{w}");
            }
        }
    }
}
